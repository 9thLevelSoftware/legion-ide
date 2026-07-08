//! Integration tests for the native delegated task execution loop.
//!
//! Every test verifies:
//! - The loop returns the expected `DelegatedTaskLoopResult` variant.
//! - Every ToolCallRequest has a matching ToolCallResult (or ToolCallRejected)
//!   with the same `causality_id`.
//! - `correlation_id` is constant across the entire run.
//! - `event_sequence` is monotonically increasing.

use std::path::Path;

use legion_agent::agent_loop::{
    DelegatedTaskAuditSink, DelegatedTaskCancellationProbe, DelegatedTaskLoopConfig,
    DelegatedTaskLoopResult, DelegatedToolHost, run_delegated_task_loop,
};
use legion_ai::tool_calls::ScriptedToolCallingProviderBuilder;
use legion_protocol::{
    CanonicalPath, CapabilityDecision, CapabilityDecisionId, CapabilityId, CapabilityRequest,
    CapabilityResponse, DelegatedTaskLoopBudget, DelegatedTaskLoopStepKind,
    DelegatedTaskLoopStepRecord, DelegatedTaskRiskTolerance, DelegatedTaskScope,
    DelegatedTaskScopeTargetKind, LegionToolKind, ProposalPayload, ProtocolResult,
};
use tempfile::TempDir;

// ─── Test fakes ───────────────────────────────────────────────────────────────

/// Records every audit step emitted by the loop.
struct RecordingAuditSink {
    steps: Vec<DelegatedTaskLoopStepRecord>,
}

impl RecordingAuditSink {
    fn new() -> Self {
        Self { steps: Vec::new() }
    }
}

impl DelegatedTaskAuditSink for RecordingAuditSink {
    fn record_step(&mut self, step: DelegatedTaskLoopStepRecord) {
        self.steps.push(step);
    }
}

/// Never cancels.
struct NeverCancelled;

impl DelegatedTaskCancellationProbe for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// Cancels after `threshold` calls to `is_cancelled`.
struct CancelAfterN {
    threshold: u32,
    counter: std::cell::Cell<u32>,
}

impl CancelAfterN {
    fn new(threshold: u32) -> Self {
        Self {
            threshold,
            counter: std::cell::Cell::new(0),
        }
    }
}

impl DelegatedTaskCancellationProbe for CancelAfterN {
    fn is_cancelled(&self) -> bool {
        let n = self.counter.get();
        self.counter.set(n + 1);
        n >= self.threshold
    }
}

/// No-op tool host — terminal commands and MCP calls return empty strings.
struct NoOpToolHost;

impl DelegatedToolHost for NoOpToolHost {
    fn run_terminal_command(
        &self,
        _command: &str,
        _workdir: Option<&Path>,
        _timeout_seconds: Option<u32>,
    ) -> Result<String, String> {
        Ok(String::new())
    }

    fn call_mcp_tool(
        &self,
        _server_id: &str,
        _tool_name: &str,
        _arguments: &serde_json::Value,
    ) -> Result<String, String> {
        Ok(String::new())
    }
}

/// Tool host that spends enough real time for wall-clock budget tests.
struct DelayedToolHost {
    delay: std::time::Duration,
}

impl DelegatedToolHost for DelayedToolHost {
    fn run_terminal_command(
        &self,
        _command: &str,
        _workdir: Option<&Path>,
        _timeout_seconds: Option<u32>,
    ) -> Result<String, String> {
        std::thread::sleep(self.delay);
        Ok(String::new())
    }

    fn call_mcp_tool(
        &self,
        _server_id: &str,
        _tool_name: &str,
        _arguments: &serde_json::Value,
    ) -> Result<String, String> {
        std::thread::sleep(self.delay);
        Ok(String::new())
    }
}

/// Always-allow capability broker.
struct AllowAllBroker;

impl legion_protocol::CapabilityBrokerPort for AllowAllBroker {
    fn handle(&self, request: CapabilityRequest) -> ProtocolResult<CapabilityResponse> {
        let cap_id = match &request {
            CapabilityRequest::Request { capability_id, .. } => capability_id.clone(),
            _ => CapabilityId("unknown".to_string()),
        };
        Ok(CapabilityResponse::Decision(CapabilityDecision {
            decision_id: CapabilityDecisionId(1),
            granted: true,
            capability: cap_id,
            reason: None,
        }))
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Build a repo-scoped `DelegatedTaskScope` rooted at `workspace_root`.
fn repo_scope(workspace_root: &Path) -> DelegatedTaskScope {
    DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Repo,
        workspace_root: CanonicalPath(workspace_root.to_string_lossy().into_owned()),
        target_path: None,
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![
            LegionToolKind::Read,
            LegionToolKind::Grep,
            LegionToolKind::Glob,
            LegionToolKind::Outline,
            LegionToolKind::EditAsProposal,
            LegionToolKind::TerminalCommand,
            LegionToolKind::McpPassthrough,
        ],
        forbidden_paths: vec![],
        schema_version: 1,
    }
}

/// Build a loop config with default budget and the given worktree directory.
fn default_config(dir: &TempDir) -> DelegatedTaskLoopConfig {
    let root = dir.path().to_path_buf();
    DelegatedTaskLoopConfig {
        system_prompt: "You are a helpful assistant.".to_string(),
        initial_message: "Do the task.".to_string(),
        model: "test-model".to_string(),
        provider: "test".to_string(),
        budget: DelegatedTaskLoopBudget::default(),
        workspace_root: root.clone(),
        worktree_root: root.clone(),
        scope: repo_scope(&root),
        forbidden_paths: vec![],
    }
}

/// Assert all ToolCallRequest steps have a matching ToolCallResult or
/// ToolCallRejected with the same causality_id.
fn assert_audit_pairing(steps: &[DelegatedTaskLoopStepRecord]) {
    let request_causality_ids: Vec<String> = steps
        .iter()
        .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRequest)
        .map(|s| s.causality_id.clone())
        .collect();

    for causality_id in &request_causality_ids {
        let has_result = steps.iter().any(|s| {
            &s.causality_id == causality_id
                && matches!(
                    s.kind,
                    DelegatedTaskLoopStepKind::ToolCallResult
                        | DelegatedTaskLoopStepKind::ToolCallRejected
                )
        });
        assert!(
            has_result,
            "ToolCallRequest with causality_id {causality_id} has no matching ToolCallResult or ToolCallRejected"
        );
    }
}

/// Assert step_index values are strictly increasing (never repeating).
fn assert_step_index_strictly_increasing(steps: &[DelegatedTaskLoopStepRecord]) {
    let indices: Vec<u32> = steps.iter().map(|s| s.step_index).collect();
    for w in indices.windows(2) {
        assert!(
            w[1] > w[0],
            "step_index is not strictly increasing: {} -> {}",
            w[0],
            w[1]
        );
    }
}

/// Assert event_sequence values are strictly increasing (never repeating).
fn assert_event_sequence_monotonic(steps: &[DelegatedTaskLoopStepRecord]) {
    let seqs: Vec<u32> = steps.iter().map(|s| s.event_sequence).collect();
    for w in seqs.windows(2) {
        assert!(
            w[1] > w[0],
            "event_sequence is not strictly increasing: {} -> {}",
            w[0],
            w[1]
        );
    }
}

/// Assert correlation_id is constant across all steps.
fn assert_correlation_id_constant(steps: &[DelegatedTaskLoopStepRecord]) {
    let Some(first) = steps.first() else {
        return;
    };
    for step in steps {
        assert_eq!(
            step.correlation_id, first.correlation_id,
            "correlation_id changed between steps"
        );
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

/// 1. Basic tool-use loop: model reads a file then ends. Assert Completed + audit pairing.
#[test]
fn basic_tool_use_loop_completes() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "Hello, world!").unwrap();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({"path": "hello.txt"}))
        .end_turn("Task complete: file says Hello, world!")
        .build("test");

    let config = default_config(&dir);
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert!(
        matches!(result, DelegatedTaskLoopResult::Completed { .. }),
        "expected Completed, got {result:?}"
    );

    if let DelegatedTaskLoopResult::Completed { final_message, .. } = &result {
        assert!(
            final_message.contains("Hello"),
            "final message should contain 'Hello'"
        );
    }

    // Audit pairing: every ToolCallRequest has a matching ToolCallResult.
    assert_audit_pairing(&sink.steps);
    assert_event_sequence_monotonic(&sink.steps);
    assert_step_index_strictly_increasing(&sink.steps);
    assert_correlation_id_constant(&sink.steps);

    // At least one ToolCallRequest and one ToolCallResult should be recorded.
    let req_count = sink
        .steps
        .iter()
        .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRequest)
        .count();
    let res_count = sink
        .steps
        .iter()
        .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallResult)
        .count();
    assert_eq!(req_count, 1, "expected exactly 1 ToolCallRequest");
    assert_eq!(res_count, 1, "expected exactly 1 ToolCallResult");
}

/// 2. Scope denial blocks the loop. Assert Blocked + ToolCallRejected audit step.
#[test]
fn scope_denial_blocks_the_loop() {
    let dir = TempDir::new().unwrap();
    let outside_dir = TempDir::new().unwrap(); // A completely separate temp dir

    // Script: model tries to read a file outside the workspace
    let outside_path = outside_dir
        .path()
        .join("secret.txt")
        .to_string_lossy()
        .into_owned();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({"path": outside_path}))
        .build("test");

    let config = default_config(&dir);
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    // Should be Blocked (containment or scope denial is non-retryable)
    assert!(
        matches!(result, DelegatedTaskLoopResult::Blocked { .. }),
        "expected Blocked result, got {result:?}"
    );

    // Must have a ToolCallRejected audit step
    let rejected_count = sink
        .steps
        .iter()
        .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRejected)
        .count();
    assert!(
        rejected_count > 0,
        "expected at least one ToolCallRejected step"
    );

    assert_audit_pairing(&sink.steps);
    assert_event_sequence_monotonic(&sink.steps);
}

/// 3. Budget exhaustion: max_tool_calls = 2, script has 3 tool_use turns.
#[test]
fn budget_exhaustion_terminates_loop() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "A").unwrap();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({"path": "a.txt"}))
        .tool_use("t2", "read", serde_json::json!({"path": "a.txt"}))
        .tool_use("t3", "read", serde_json::json!({"path": "a.txt"}))
        .end_turn("done")
        .build("test");

    let root = dir.path().to_path_buf();
    let config = DelegatedTaskLoopConfig {
        system_prompt: "".to_string(),
        initial_message: "do it".to_string(),
        model: "test-model".to_string(),
        provider: "test".to_string(),
        budget: DelegatedTaskLoopBudget {
            max_model_turns: 10,
            max_tool_calls: 2,
            max_consecutive_retries: 3,
            max_tool_output_bytes: 100_000,
            max_total_tool_output_bytes: 5_000_000,
            wall_clock_limit_ms: 0,
        },
        workspace_root: root.clone(),
        worktree_root: root.clone(),
        scope: repo_scope(&root),
        forbidden_paths: vec![],
    };
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert!(
        matches!(result, DelegatedTaskLoopResult::BudgetExhausted { .. }),
        "expected BudgetExhausted, got {result:?}"
    );

    // At most 2 tool calls should have been executed
    let executed_count = sink
        .steps
        .iter()
        .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallResult)
        .count();
    assert!(
        executed_count <= 2,
        "expected at most 2 tool executions, got {executed_count}"
    );
}

/// 4. Cancellation: probe returns true after the first model turn.
#[test]
fn cancellation_stops_the_loop() {
    let dir = TempDir::new().unwrap();

    // The model would keep going forever, but we cancel after turn 0
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({"path": "nonexistent.txt"}))
        .tool_use("t2", "read", serde_json::json!({"path": "nonexistent.txt"}))
        .end_turn("done")
        .build("test");

    let config = default_config(&dir);
    let mut sink = RecordingAuditSink::new();

    // Cancel after the first cancellation check (which happens before model turn 1)
    let cancel = CancelAfterN::new(0);

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &cancel,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert!(
        matches!(result, DelegatedTaskLoopResult::Cancelled),
        "expected Cancelled, got {result:?}"
    );
}

/// 5. Audit pairing: multi-turn loop, check pairing + monotonic event_sequence.
#[test]
fn audit_pairing_is_maintained_across_multi_turn_loop() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("file1.txt"), "content1").unwrap();
    std::fs::write(dir.path().join("file2.txt"), "content2").unwrap();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({"path": "file1.txt"}))
        .tool_use("t2", "read", serde_json::json!({"path": "file2.txt"}))
        .end_turn("Both files read successfully.")
        .build("test");

    let config = default_config(&dir);
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert!(
        matches!(result, DelegatedTaskLoopResult::Completed { .. }),
        "expected Completed, got {result:?}"
    );

    // Strict audit assertions
    assert_audit_pairing(&sink.steps);
    assert_event_sequence_monotonic(&sink.steps);
    assert_step_index_strictly_increasing(&sink.steps);
    assert_correlation_id_constant(&sink.steps);

    // Verify pairing: both tool calls should have request+result pairs
    let req_cids: std::collections::HashSet<String> = sink
        .steps
        .iter()
        .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRequest)
        .map(|s| s.causality_id.clone())
        .collect();
    let res_cids: std::collections::HashSet<String> = sink
        .steps
        .iter()
        .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallResult)
        .map(|s| s.causality_id.clone())
        .collect();

    // Every request causality_id should appear in results
    for cid in &req_cids {
        assert!(
            res_cids.contains(cid),
            "causality_id {cid} present in ToolCallRequest but not in any ToolCallResult"
        );
    }
}

/// 6. Retry budget: broker denies but that's PolicyDenied (non-retryable), causes Blocked.
///    Then a separate test uses schema errors (InvalidArguments = retryable) to exhaust retries.
#[test]
fn retry_budget_exhausted_by_invalid_arguments() {
    let dir = TempDir::new().unwrap();

    // Script: model repeatedly sends a read call with a missing 'path' field (InvalidArguments)
    let bad_input = serde_json::json!({"not_a_path": "value"});

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", bad_input.clone())
        .tool_use("t2", "read", bad_input.clone())
        .tool_use("t3", "read", bad_input.clone())
        .tool_use("t4", "read", bad_input.clone())
        .end_turn("done")
        .build("test");

    let root = dir.path().to_path_buf();
    let config = DelegatedTaskLoopConfig {
        system_prompt: "".to_string(),
        initial_message: "do it".to_string(),
        model: "test-model".to_string(),
        provider: "test".to_string(),
        budget: DelegatedTaskLoopBudget {
            max_model_turns: 10,
            max_tool_calls: 200,
            max_consecutive_retries: 2, // low retry budget
            max_tool_output_bytes: 100_000,
            max_total_tool_output_bytes: 5_000_000,
            wall_clock_limit_ms: 0,
        },
        workspace_root: root.clone(),
        worktree_root: root.clone(),
        scope: repo_scope(&root),
        forbidden_paths: vec![],
    };
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert!(
        matches!(result, DelegatedTaskLoopResult::BudgetExhausted { .. }),
        "expected BudgetExhausted (retry budget), got {result:?}"
    );

    // All rejected steps should be ToolCallRejected
    let rejected_count = sink
        .steps
        .iter()
        .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRejected)
        .count();
    assert!(
        rejected_count >= 2,
        "expected at least 2 ToolCallRejected steps (consecutive retry budget), got {rejected_count}"
    );

    // Must emit a BudgetExhausted audit step with max_consecutive_retries reason.
    let exhausted = sink
        .steps
        .iter()
        .find(|s| s.kind == DelegatedTaskLoopStepKind::BudgetExhausted)
        .expect("must have a BudgetExhausted step when consecutive retries are exhausted");
    assert_eq!(
        exhausted.reason.as_deref(),
        Some("max_consecutive_retries"),
        "BudgetExhausted reason should be max_consecutive_retries"
    );

    assert_audit_pairing(&sink.steps);
}

/// 7. max_total_tool_output_bytes: audit-pairing holds when cumulative output budget is exceeded.
///    The ToolCallRequest must have a paired ToolCallResult with the same causality_id.
#[test]
fn max_total_tool_output_bytes_emits_paired_tool_call_result() {
    let dir = TempDir::new().unwrap();
    // File content is well over 10 bytes so a single read exceeds the cumulative budget.
    std::fs::write(
        dir.path().join("big.txt"),
        "This content is definitely more than ten bytes long.",
    )
    .unwrap();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({"path": "big.txt"}))
        .end_turn("done")
        .build("test");

    let root = dir.path().to_path_buf();
    let config = DelegatedTaskLoopConfig {
        system_prompt: "".to_string(),
        initial_message: "do it".to_string(),
        model: "test-model".to_string(),
        provider: "test".to_string(),
        budget: DelegatedTaskLoopBudget {
            max_model_turns: 10,
            max_tool_calls: 100,
            max_consecutive_retries: 3,
            max_tool_output_bytes: 100_000,
            max_total_tool_output_bytes: 10, // very low: one read exceeds it
            wall_clock_limit_ms: 0,
        },
        workspace_root: root.clone(),
        worktree_root: root.clone(),
        scope: repo_scope(&root),
        forbidden_paths: vec![],
    };
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert!(
        matches!(result, DelegatedTaskLoopResult::BudgetExhausted { .. }),
        "expected BudgetExhausted, got {result:?}"
    );

    // The ToolCallRequest emitted before dispatch must have a paired ToolCallResult.
    assert_audit_pairing(&sink.steps);
    assert_event_sequence_monotonic(&sink.steps);

    // Verify the pairing explicitly: same causality_id on ToolCallRequest and ToolCallResult.
    let req = sink
        .steps
        .iter()
        .find(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRequest)
        .expect("must have a ToolCallRequest step");
    let paired_result = sink.steps.iter().find(|s| {
        s.kind == DelegatedTaskLoopStepKind::ToolCallResult && s.causality_id == req.causality_id
    });
    assert!(
        paired_result.is_some(),
        "ToolCallRequest causality_id {} has no paired ToolCallResult",
        req.causality_id
    );
}

/// 8. max_model_turns: BudgetExhausted step carries a strictly higher event_sequence than
///    all preceding steps, preserving the monotonically-increasing invariant.
#[test]
fn max_model_turns_budget_exhausted_event_sequence_is_monotonic() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("file.txt"), "content").unwrap();

    // Script one tool_use turn; after processing it the loop tries a second model turn
    // which hits max_model_turns = 1 and emits BudgetExhausted.
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({"path": "file.txt"}))
        .end_turn("done") // never reached
        .build("test");

    let root = dir.path().to_path_buf();
    let config = DelegatedTaskLoopConfig {
        system_prompt: "".to_string(),
        initial_message: "do it".to_string(),
        model: "test-model".to_string(),
        provider: "test".to_string(),
        budget: DelegatedTaskLoopBudget {
            max_model_turns: 1,
            max_tool_calls: 100,
            max_consecutive_retries: 3,
            max_tool_output_bytes: 100_000,
            max_total_tool_output_bytes: 5_000_000,
            wall_clock_limit_ms: 0,
        },
        workspace_root: root.clone(),
        worktree_root: root.clone(),
        scope: repo_scope(&root),
        forbidden_paths: vec![],
    };
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert!(
        matches!(result, DelegatedTaskLoopResult::BudgetExhausted { .. }),
        "expected BudgetExhausted, got {result:?}"
    );

    // The full sequence — including the BudgetExhausted step — must be strictly increasing.
    assert_event_sequence_monotonic(&sink.steps);

    // Sanity: a BudgetExhausted step with reason "max_model_turns" must be present.
    let exhausted = sink
        .steps
        .iter()
        .find(|s| s.kind == DelegatedTaskLoopStepKind::BudgetExhausted)
        .expect("must have a BudgetExhausted step");
    assert_eq!(
        exhausted.reason.as_deref(),
        Some("max_model_turns"),
        "BudgetExhausted reason should be max_model_turns"
    );
}

/// 9. wall_clock_limit_ms: BudgetExhausted with wall-clock reason fires before the loop completes.
///    The delayed tool host makes the elapsed time deterministic before the
///    check at the top of the second loop iteration.
#[test]
fn wall_clock_limit_fires_budget_exhausted() {
    let dir = TempDir::new().unwrap();

    // Script a tool-use turn so the loop spends real time before the second iteration.
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "terminal-command",
            serde_json::json!({"command": "noop"}),
        )
        .end_turn("done") // never reached — wall clock fires first
        .build("test");
    let tool_host = DelayedToolHost {
        delay: std::time::Duration::from_millis(20),
    };

    let root = dir.path().to_path_buf();
    let config = DelegatedTaskLoopConfig {
        system_prompt: "".to_string(),
        initial_message: "do it".to_string(),
        model: "test-model".to_string(),
        provider: "test".to_string(),
        budget: DelegatedTaskLoopBudget {
            max_model_turns: 50,
            max_tool_calls: 200,
            max_consecutive_retries: 3,
            max_tool_output_bytes: 100_000,
            max_total_tool_output_bytes: 5_000_000,
            wall_clock_limit_ms: 1, // 1 ms — fires before the model responds a second time
        },
        workspace_root: root.clone(),
        worktree_root: root.clone(),
        scope: repo_scope(&root),
        forbidden_paths: vec![],
    };
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &tool_host,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert!(
        matches!(result, DelegatedTaskLoopResult::BudgetExhausted { .. }),
        "expected BudgetExhausted (wall clock), got {result:?}"
    );

    if let DelegatedTaskLoopResult::BudgetExhausted { reason } = &result {
        assert!(
            reason.contains("wall clock"),
            "expected wall-clock reason, got: {reason}"
        );
    }

    // Must emit a BudgetExhausted audit step with wall_clock_limit_ms reason.
    let exhausted = sink
        .steps
        .iter()
        .find(|s| s.kind == DelegatedTaskLoopStepKind::BudgetExhausted)
        .expect("must have a BudgetExhausted step");
    assert_eq!(
        exhausted.reason.as_deref(),
        Some("wall_clock_limit_ms"),
        "BudgetExhausted reason should be wall_clock_limit_ms"
    );

    assert_event_sequence_monotonic(&sink.steps);
}

// ─── Proposal-surfacing tests (PKT-PROPOSAL-SURFACE) ─────────────────────────

/// 10. Single edit-as-proposal: loop returns exactly 1 proposal with the
///     correct target path in the Completed variant.
#[test]
fn proposal_surfacing_single_edit() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({"path": "src/main.rs"}))
        .tool_use(
            "t2",
            "edit-as-proposal",
            serde_json::json!({
                "path": "src/main.rs",
                "replacement": "fn main() { /* surfaced */ }\n",
            }),
        )
        .end_turn("Done: read and proposed an edit.")
        .build("test");

    let config = default_config(&dir);
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    match result {
        DelegatedTaskLoopResult::Completed {
            final_message,
            proposals,
        } => {
            assert!(
                final_message.contains("Done"),
                "unexpected final_message: {final_message}"
            );
            assert_eq!(
                proposals.len(),
                1,
                "expected exactly 1 proposal, got {}",
                proposals.len()
            );
            let proposal = &proposals[0];
            let targets_main_rs = match &proposal.payload {
                ProposalPayload::CreateFile(p) => {
                    p.path.0.ends_with("main.rs") || p.path.0.contains("src/main.rs")
                }
                _ => false,
            };
            assert!(
                targets_main_rs,
                "proposal payload does not target src/main.rs"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    assert_audit_pairing(&sink.steps);
    assert_event_sequence_monotonic(&sink.steps);
}

/// 11. Multi-edit: 2 edit-as-proposal calls → Completed carries 2 proposals in order.
#[test]
fn proposal_surfacing_multi_edit() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/a.rs"), "pub fn a() {}").unwrap();
    std::fs::write(dir.path().join("src/b.rs"), "pub fn b() {}").unwrap();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "edit-as-proposal",
            serde_json::json!({
                "path": "src/a.rs",
                "replacement": "pub fn a() { /* patched */ }\n",
            }),
        )
        .tool_use(
            "t2",
            "edit-as-proposal",
            serde_json::json!({
                "path": "src/b.rs",
                "replacement": "pub fn b() { /* patched */ }\n",
            }),
        )
        .end_turn("Both files proposed.")
        .build("test");

    let config = default_config(&dir);
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    match result {
        DelegatedTaskLoopResult::Completed { proposals, .. } => {
            assert_eq!(
                proposals.len(),
                2,
                "expected 2 proposals, got {}",
                proposals.len()
            );
            // First proposal targets a.rs, second targets b.rs (in submission order).
            let first_targets_a = match &proposals[0].payload {
                ProposalPayload::CreateFile(p) => p.path.0.ends_with("a.rs"),
                _ => false,
            };
            let second_targets_b = match &proposals[1].payload {
                ProposalPayload::CreateFile(p) => p.path.0.ends_with("b.rs"),
                _ => false,
            };
            assert!(first_targets_a, "first proposal should target a.rs");
            assert!(second_targets_b, "second proposal should target b.rs");
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    assert_audit_pairing(&sink.steps);
}

/// 12. Blocked run discards proposals: edit-as-proposal succeeds (proposal
///     accumulated), then a read outside scope returns Blocked — the proposals
///     must not appear in the result (they are partial, unreviewed work).
#[test]
fn blocked_run_discards_proposals() {
    let dir = TempDir::new().unwrap();
    let outside_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

    let outside_path = outside_dir
        .path()
        .join("secret.txt")
        .to_string_lossy()
        .into_owned();

    let provider = ScriptedToolCallingProviderBuilder::new()
        // Turn 1: edit succeeds → proposal accumulated
        .tool_use(
            "t1",
            "edit-as-proposal",
            serde_json::json!({
                "path": "src/main.rs",
                "replacement": "fn main() { /* blocked run */ }\n",
            }),
        )
        // Turn 2: read from outside scope → Blocked (non-retryable)
        .tool_use("t2", "read", serde_json::json!({"path": outside_path}))
        .build("test");

    let config = default_config(&dir);
    let mut sink = RecordingAuditSink::new();

    let result = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert!(
        matches!(result, DelegatedTaskLoopResult::Blocked { .. }),
        "expected Blocked (scope denial after proposal), got {result:?}"
    );
    // Blocked variant carries no proposals field — verified by the match above.
    assert_audit_pairing(&sink.steps);
}
