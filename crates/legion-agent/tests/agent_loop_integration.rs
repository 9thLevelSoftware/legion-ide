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
use legion_ai::tool_calls::{
    ScriptedToolCallingProviderBuilder, ToolCompletionStopReason, ToolTurnBlock,
};
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

/// Records whether the tool host was ever asked to do anything.
///
/// `NoOpToolHost` accepts silently, so a test asserting "the command never ran"
/// against it is asserting nothing. This one refuses and remembers.
struct RefusingToolHost {
    terminal_calls: std::cell::Cell<usize>,
}

impl RefusingToolHost {
    fn new() -> Self {
        Self {
            terminal_calls: std::cell::Cell::new(0),
        }
    }
}

impl DelegatedToolHost for RefusingToolHost {
    fn run_terminal_command(
        &self,
        _command: &str,
        _workdir: Option<&Path>,
        _timeout_seconds: Option<u32>,
    ) -> Result<String, String> {
        self.terminal_calls.set(self.terminal_calls.get() + 1);
        Err("the host must not have been reached".to_string())
    }

    fn call_mcp_tool(
        &self,
        _server_id: &str,
        _tool_name: &str,
        _arguments: &serde_json::Value,
    ) -> Result<String, String> {
        Err("the host must not have been reached".to_string())
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

/// A malformed tool call is recoverable: the loop reports the diagnostic back
/// as text and the model's corrected call succeeds, rather than the whole run
/// dying on one bad JSON string (ADR-0049).
#[test]
fn malformed_tool_call_is_reported_back_and_run_recovers() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "Hello, world!").unwrap();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .turn(
            vec![ToolTurnBlock::MalformedToolCall {
                id: "c1".to_string(),
                name: "read".to_string(),
                raw_arguments: "{not json".to_string(),
                diagnostic: "arguments are not valid JSON".to_string(),
            }],
            ToolCompletionStopReason::ToolUse,
        )
        // The corrected call is only returned if the loop actually fed the
        // rejection back to the model.
        .expect_prior_result_contains("not valid JSON")
        .tool_use("t1", "read", serde_json::json!({"path": "hello.txt"}))
        .end_turn("Recovered: file says Hello, world!")
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
    .expect("a malformed call must not error the loop");

    assert!(
        matches!(result, DelegatedTaskLoopResult::Completed { .. }),
        "expected Completed after recovery, got {result:?}"
    );

    let rejected = sink
        .steps
        .iter()
        .filter(|step| step.reason.as_deref() == Some("malformed_tool_arguments"))
        .count();
    assert_eq!(rejected, 1, "the malformed call is audited exactly once");
    assert_event_sequence_monotonic(&sink.steps);
    assert_step_index_strictly_increasing(&sink.steps);
}

/// A model that can only emit broken arguments is stopped by the retry budget
/// instead of looping until the turn budget drains.
#[test]
fn repeated_malformed_tool_calls_hit_the_retry_budget() {
    let dir = TempDir::new().unwrap();
    let mut builder = ScriptedToolCallingProviderBuilder::new();
    for index in 0..12 {
        builder = builder.turn(
            vec![ToolTurnBlock::MalformedToolCall {
                id: format!("c{index}"),
                name: "read".to_string(),
                raw_arguments: "{still not json".to_string(),
                diagnostic: "arguments are not valid JSON".to_string(),
            }],
            ToolCompletionStopReason::ToolUse,
        );
    }
    let provider = builder.build("test");

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
    .expect("budget exhaustion is a result, not an error");

    match result {
        DelegatedTaskLoopResult::Blocked { reason } => {
            assert!(
                reason.contains("unparseable tool arguments"),
                "block reason should name the cause: {reason}"
            );
        }
        other => panic!("expected Blocked on repeated malformed calls, got {other:?}"),
    }
}

// ─── Fragment edits: a governor-gated behavior, so every test below states
//     what it expects in *both* arms ───────────────────────────────────────────

/// Whether fragment-edit resolution is on — the default arm.
///
/// `LEGION_AI_GOVERNORS=off` reproduces pre-port behavior: an edit must supply
/// the file's complete content, and an `old_str`/`new_str` fragment is refused.
/// That is the arm `legion-bench`'s **raw baseline** runs under, so these tests
/// assert its contract rather than assuming the default. Until 2026-08-17 they
/// asserted only the governed contract, which left the raw configuration — the
/// one half of the Phase 2 exit measurement — never verified.
fn fragment_edits_resolve() -> bool {
    legion_ai::governance::small_model_governors_enabled()
}

/// The raw-arm contract shared by every fragment edit below: the loop completes,
/// nothing is proposed, and the file on disk is untouched.
///
/// The file check is not redundant with "no proposal". The failure this whole
/// feature exists to prevent is a fragment being taken for the file's complete
/// new content, and a raw arm that had regressed into doing that would still
/// leave the worktree clean — so the two assertions catch different things, and
/// the interesting one is that no *destructive* proposal was produced either.
fn assert_fragment_edit_was_refused(result: &DelegatedTaskLoopResult, path: &Path, original: &str) {
    let DelegatedTaskLoopResult::Completed { proposals, .. } = result else {
        panic!("a refused fragment must still complete the run, got {result:?}");
    };
    assert!(
        proposals.is_empty(),
        "the raw baseline cannot resolve a fragment, so it must propose nothing: {proposals:?}"
    );
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        original,
        "edits stay proposals in either arm; the file is never written"
    );
}

/// Fragment edits are resolved against the file rather than treated as whole
/// content (ADR-0049). Without this, `old_str`/`new_str` would replace the
/// entire file with the new fragment.
///
/// Raw: the fragment is refused outright — which is precisely why the raw arm
/// scores worse on edit tasks, and why it is safe rather than destructive.
#[test]
fn fragment_edit_replaces_only_the_matched_text() {
    let dir = TempDir::new().unwrap();
    let original = "fn main() {\n    println!(\"hello\");\n}\n";
    std::fs::write(dir.path().join("main.rs"), original).unwrap();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "edit-as-proposal",
            serde_json::json!({
                "path": "main.rs",
                "old_str": "println!(\"hello\");",
                "new_str": "println!(\"hello, legion\");"
            }),
        )
        .end_turn("Edited.")
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

    if !fragment_edits_resolve() {
        assert_fragment_edit_was_refused(&result, &dir.path().join("main.rs"), original);
        return;
    }

    let DelegatedTaskLoopResult::Completed { proposals, .. } = result else {
        panic!("expected Completed, got {result:?}");
    };
    assert_eq!(proposals.len(), 1, "one edit proposal is surfaced");

    // The proposal must carry the whole file with only the fragment changed —
    // not the fragment alone.
    let content = match &proposals[0].payload {
        ProposalPayload::CreateFile(create) => create.initial_content.clone().unwrap_or_default(),
        other => panic!("expected a file-content payload, got {other:?}"),
    };
    assert_eq!(
        content,
        "fn main() {\n    println!(\"hello, legion\");\n}\n"
    );

    // The file on disk is untouched: edits stay proposals until reviewed.
    assert_eq!(
        std::fs::read_to_string(dir.path().join("main.rs")).unwrap(),
        original
    );
}

/// An ambiguous fragment is refused with retryable feedback, and the model can
/// correct it in the same run — the loop must not die on a near-miss.
///
/// Raw: there is no "ambiguous" diagnostic to feed back, because the fragment
/// was never resolved far enough to be found ambiguous. The script is built per
/// arm for that reason: a scripted guard waiting on feedback the raw arm cannot
/// produce would fail as a *provider* error and read like a loop bug.
#[test]
fn ambiguous_fragment_is_refused_then_corrected() {
    let dir = TempDir::new().unwrap();
    let original = "x = 1\nx = 1\n";
    std::fs::write(dir.path().join("cfg.rs"), original).unwrap();

    let first_edit = serde_json::json!({"path": "cfg.rs", "old_str": "x = 1", "new_str": "x = 2"});

    if !fragment_edits_resolve() {
        let provider = ScriptedToolCallingProviderBuilder::new()
            .tool_use("t1", "edit-as-proposal", first_edit)
            .end_turn("Nothing staged.")
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
        .expect("an unresolvable fragment must not error the loop in either arm");
        assert_fragment_edit_was_refused(&result, &dir.path().join("cfg.rs"), original);
        return;
    }

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "edit-as-proposal", first_edit)
        // Only reachable if the refusal was fed back as retryable feedback.
        .expect_prior_result_contains("ambiguous")
        .tool_use(
            "t2",
            "edit-as-proposal",
            serde_json::json!({
                "path": "cfg.rs",
                "old_str": "x = 1\nx = 1",
                "new_str": "x = 1\nx = 2"
            }),
        )
        .end_turn("Disambiguated.")
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
    .expect("an ambiguous fragment must not error the loop");

    let DelegatedTaskLoopResult::Completed { proposals, .. } = result else {
        panic!("expected Completed, got {result:?}");
    };
    let content = match &proposals[0].payload {
        ProposalPayload::CreateFile(create) => create.initial_content.clone().unwrap_or_default(),
        other => panic!("expected a file-content payload, got {other:?}"),
    };
    assert_eq!(content, "x = 1\nx = 2\n");
}

/// A fragment that does not match is refused with a diagnostic naming the
/// nearest line, so the model can re-read rather than rewrite the file.
///
/// Raw: refused too, but with no locating diagnostic — nothing looked for a
/// nearest line. The guard is therefore only scripted in the governed arm.
#[test]
fn unmatched_fragment_is_refused_with_a_locating_diagnostic() {
    let dir = TempDir::new().unwrap();
    let original = "fn alpha() {}\nfn beta() {}\n";
    std::fs::write(dir.path().join("a.rs"), original).unwrap();

    let edit = serde_json::json!({
        "path": "a.rs",
        "old_str": "fn beta( ) {}",
        "new_str": "fn beta(x: u8) {}"
    });

    let mut builder =
        ScriptedToolCallingProviderBuilder::new().tool_use("t1", "edit-as-proposal", edit);
    if fragment_edits_resolve() {
        builder = builder.expect_prior_result_contains("closest line is 2");
    }
    let provider = builder.end_turn("Understood.").build("test");

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
    .expect("a no-match fragment must not error the loop");

    if !fragment_edits_resolve() {
        assert_fragment_edit_was_refused(&result, &dir.path().join("a.rs"), original);
        return;
    }

    assert!(
        matches!(result, DelegatedTaskLoopResult::Completed { .. }),
        "expected Completed, got {result:?}"
    );
}

/// Two fragment edits to the same file in one run must compose. Without a
/// per-run overlay the second resolves against the untouched worktree, so its
/// proposal silently omits the first edit and both carry preconditions for the
/// same original file — applying either makes the other stale.
#[test]
fn successive_fragment_edits_to_one_file_compose() {
    let dir = TempDir::new().unwrap();
    let original = "let a = 1;\nlet b = 2;\n";
    std::fs::write(dir.path().join("cfg.rs"), original).unwrap();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "edit-as-proposal",
            serde_json::json!({"path": "cfg.rs", "old_str": "let a = 1;", "new_str": "let a = 10;"}),
        )
        .tool_use(
            "t2",
            "edit-as-proposal",
            serde_json::json!({"path": "cfg.rs", "old_str": "let b = 2;", "new_str": "let b = 20;"}),
        )
        .end_turn("Both edits staged.")
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

    if !fragment_edits_resolve() {
        assert_fragment_edit_was_refused(&result, &dir.path().join("cfg.rs"), original);
        return;
    }

    let DelegatedTaskLoopResult::Completed { proposals, .. } = result else {
        panic!("expected Completed, got {result:?}");
    };
    assert_eq!(proposals.len(), 2, "each edit surfaces its own proposal");

    let content_of = |proposal: &legion_protocol::AssistedAiEditProposalOutput| match &proposal
        .payload
    {
        ProposalPayload::CreateFile(create) => create.initial_content.clone().unwrap_or_default(),
        other => panic!("expected a file-content payload, got {other:?}"),
    };

    // The second proposal carries both edits, so applying it produces what the
    // model actually asked for.
    assert_eq!(content_of(&proposals[0]), "let a = 10;\nlet b = 2;\n");
    assert_eq!(content_of(&proposals[1]), "let a = 10;\nlet b = 20;\n");
}

/// A second edit whose anchor exists only because of the first must resolve —
/// proof the overlay is the resolution source, not just a cache.
#[test]
fn a_fragment_can_anchor_on_text_introduced_by_an_earlier_edit() {
    let dir = TempDir::new().unwrap();
    let original = "fn main() {}\n";
    std::fs::write(dir.path().join("m.rs"), original).unwrap();

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "edit-as-proposal",
            serde_json::json!({
                "path": "m.rs",
                "old_str": "fn main() {}",
                "new_str": "fn main() {\n    todo!();\n}"
            }),
        )
        .tool_use(
            "t2",
            "edit-as-proposal",
            serde_json::json!({"path": "m.rs", "old_str": "todo!();", "new_str": "println!(\"hi\");"}),
        )
        .end_turn("Filled in.")
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
    .expect("the second anchor exists only in staged content");

    if !fragment_edits_resolve() {
        assert_fragment_edit_was_refused(&result, &dir.path().join("m.rs"), original);
        return;
    }

    let DelegatedTaskLoopResult::Completed { proposals, .. } = result else {
        panic!("expected Completed, got {result:?}");
    };
    // `last()` rather than indexing on `len() - 1`: with no proposals that
    // subtraction underflows and the test dies with "attempt to subtract with
    // overflow" instead of saying what was actually missing. It did exactly
    // that in the raw arm.
    let last = match &proposals.last().expect("a proposal is staged").payload {
        ProposalPayload::CreateFile(create) => create.initial_content.clone().unwrap_or_default(),
        other => panic!("expected a file-content payload, got {other:?}"),
    };
    assert_eq!(last, "fn main() {\n    println!(\"hi\");\n}\n");
}

/// A command refused after the gates never reaches the host, and says so.
///
/// `ToolCallDispatched` exists to answer the one question the outcome cannot:
/// a command that ran and then failed and a command refused before it ran are
/// both `ToolCallRejected`. The flag was set as soon as the shared gates passed
/// -- but each executor validates its own arguments afterwards, and a `workdir`
/// that escapes the worktree is refused inside `execute_terminal_command`
/// without `run_terminal_command` ever being called. The audit therefore
/// recorded a command as having touched the machine when nothing had.
#[test]
fn a_workdir_refused_inside_the_executor_records_no_dispatch() {
    let dir = TempDir::new().expect("temp dir");
    let escaping = if cfg!(windows) { "C:\\" } else { "/" };

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "terminal-command",
            serde_json::json!({"command": "echo hi", "workdir": escaping}),
        )
        .end_turn("done")
        .build("test");

    let config = default_config(&dir);
    let mut sink = RecordingAuditSink::new();
    let host = RefusingToolHost::new();

    let _ = run_delegated_task_loop(
        &config,
        &provider,
        &host,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert_eq!(
        host.terminal_calls.get(),
        0,
        "the containment check must refuse before the host is asked"
    );
    assert!(
        !sink
            .steps
            .iter()
            .any(|step| step.kind == DelegatedTaskLoopStepKind::ToolCallDispatched),
        "nothing reached the machine, so nothing may be recorded as dispatched"
    );
    assert!(
        sink.steps
            .iter()
            .any(|step| step.kind == DelegatedTaskLoopStepKind::ToolCallRejected),
        "the refusal itself still has to be on the record"
    );
}

/// A command that does reach the host is still recorded as dispatched.
///
/// Without this the check above passes on a loop that never emits the event at
/// all, which is the same audit gap in the other direction.
#[test]
fn a_command_that_reaches_the_host_records_a_dispatch() {
    let dir = TempDir::new().expect("temp dir");

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "terminal-command",
            serde_json::json!({"command": "echo hi"}),
        )
        .end_turn("done")
        .build("test");

    let config = default_config(&dir);
    let mut sink = RecordingAuditSink::new();

    let _ = run_delegated_task_loop(
        &config,
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert!(
        sink.steps
            .iter()
            .any(|step| step.kind == DelegatedTaskLoopStepKind::ToolCallDispatched),
        "the host ran the command, and the audit has to say so"
    );
}

/// A command the host itself rejects still counts as having reached it.
///
/// This is the case the event was added for: the failure is indistinguishable
/// from a refusal in the outcome, and only the dispatch record separates
/// "ran and failed" from "never ran".
#[test]
fn a_host_failure_still_records_a_dispatch() {
    let dir = TempDir::new().expect("temp dir");

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "terminal-command",
            serde_json::json!({"command": "echo hi"}),
        )
        .end_turn("done")
        .build("test");

    let config = default_config(&dir);
    let mut sink = RecordingAuditSink::new();
    let host = RefusingToolHost::new();

    let _ = run_delegated_task_loop(
        &config,
        &provider,
        &host,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");

    assert_eq!(host.terminal_calls.get(), 1, "the host was asked");
    assert!(
        sink.steps
            .iter()
            .any(|step| step.kind == DelegatedTaskLoopStepKind::ToolCallDispatched),
        "it ran and failed, which is not the same as never running"
    );
}
