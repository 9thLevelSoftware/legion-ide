//! The raw-versus-governed measurement seam.
//!
//! The roadmap's central AI claim is that the same local model completes more
//! tasks with the small-model reliability layer than without. That comparison
//! is only meaningful if the switch actually changes behavior, so this asserts
//! it does.
//!
//! **Why this lives in its own test binary:** `LEGION_AI_GOVERNORS` is
//! process-global. Cargo runs tests inside one binary on parallel threads, so
//! flipping it in a shared binary makes sibling tests observe the wrong arm —
//! which is exactly how this test first failed. Each test binary gets its own
//! process, so isolating it here is what makes it safe. Do not move these
//! cases into a shared test file.

use std::path::Path;

use legion_agent::agent_loop::{
    DelegatedTaskAuditSink, DelegatedTaskCancellationProbe, DelegatedTaskLoopConfig,
    DelegatedTaskLoopResult, DelegatedToolHost, run_delegated_task_loop,
};
use legion_ai::tool_calls::{ScriptedToolCallingProviderBuilder, ToolCallingProvider};
use legion_protocol::{
    CanonicalPath, CapabilityDecision, CapabilityDecisionId, CapabilityId, CapabilityRequest,
    CapabilityResponse, DelegatedTaskLoopBudget, DelegatedTaskLoopStepRecord,
    DelegatedTaskRiskTolerance, DelegatedTaskScope, DelegatedTaskScopeTargetKind, LegionToolKind,
    ProtocolResult,
};
use tempfile::TempDir;

/// Serializes every env-mutating test in this binary.
///
/// Isolating the binary keeps other suites safe; this keeps these tests from
/// racing each other. Every test that touches `LEGION_AI_GOVERNORS` must hold
/// this lock for the whole of its env-mutating block — that is the invariant
/// each `SAFETY` comment below refers to.
///
/// Deliberately stated here and not restated per test: an earlier version
/// counted the tests inline and the count was wrong within one commit of
/// being written.
static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Sink;
impl DelegatedTaskAuditSink for Sink {
    fn record_step(&mut self, _step: DelegatedTaskLoopStepRecord) {}
}

struct NeverCancelled;
impl DelegatedTaskCancellationProbe for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

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

struct AllowAllBroker;
impl legion_protocol::CapabilityBrokerPort for AllowAllBroker {
    fn handle(&self, request: CapabilityRequest) -> ProtocolResult<CapabilityResponse> {
        let capability = match &request {
            CapabilityRequest::Request { capability_id, .. } => capability_id.clone(),
            _ => CapabilityId("unknown".to_string()),
        };
        Ok(CapabilityResponse::Decision(CapabilityDecision {
            decision_id: CapabilityDecisionId(1),
            granted: true,
            capability,
            reason: None,
        }))
    }
}

fn config(dir: &TempDir) -> DelegatedTaskLoopConfig {
    let root = dir.path().to_path_buf();
    DelegatedTaskLoopConfig {
        system_prompt: "system".to_string(),
        initial_message: "task".to_string(),
        model: "test-model".to_string(),
        provider: "test".to_string(),
        budget: DelegatedTaskLoopBudget::default(),
        workspace_root: root.clone(),
        worktree_root: root.clone(),
        scope: DelegatedTaskScope {
            target_kind: DelegatedTaskScopeTargetKind::Repo,
            workspace_root: CanonicalPath(root.to_string_lossy().into_owned()),
            target_path: None,
            risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
            allowed_tools: vec![LegionToolKind::Read, LegionToolKind::EditAsProposal],
            forbidden_paths: vec![],
            schema_version: 1,
        },
        forbidden_paths: vec![],
    }
}

/// Run one fragment edit and report how many proposals it produced.
fn proposals_for_fragment_edit(dir: &TempDir) -> usize {
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "edit-as-proposal",
            serde_json::json!({
                "path": "m.rs",
                "old_str": "fn main() {}",
                "new_str": "fn main() { todo!() }"
            }),
        )
        .end_turn("done")
        .build("test");

    let result = run_delegated_task_loop(
        &config(dir),
        &provider,
        &NoOpToolHost,
        &mut Sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error in either arm");

    match result {
        DelegatedTaskLoopResult::Completed { proposals, .. } => proposals.len(),
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[test]
fn the_measurement_switch_changes_edit_behavior() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("m.rs"), "fn main() {}\n").unwrap();

    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block, so no
    // other thread in this binary can observe a partial state.
    unsafe { std::env::set_var("LEGION_AI_GOVERNORS", "off") };
    assert!(!legion_ai::governance::small_model_governors_enabled());
    let without = proposals_for_fragment_edit(&dir);

    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };
    assert!(legion_ai::governance::small_model_governors_enabled());
    let with = proposals_for_fragment_edit(&dir);

    assert_eq!(
        without, 0,
        "governors off must reproduce the pre-port contract, where a fragment edit is refused"
    );
    assert_eq!(with, 1, "governors on must resolve the same edit");
}

#[test]
fn only_the_exact_off_value_disables_governors() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // A typo must not silently run the wrong arm and quietly invalidate a
    // benchmark result.
    //
    // SAFETY (both loops and the reset below): holds `ENV_GUARD` across the
    // whole env-mutating block, so no other thread in this binary can observe
    // a partial state.
    for value in ["", "0", "false", "no", "OFFF", "on"] {
        unsafe { std::env::set_var("LEGION_AI_GOVERNORS", value) };
        assert!(
            legion_ai::governance::small_model_governors_enabled(),
            "{value:?} must not be read as off"
        );
    }
    for value in ["off", "OFF", "Off"] {
        unsafe { std::env::set_var("LEGION_AI_GOVERNORS", value) };
        assert!(
            !legion_ai::governance::small_model_governors_enabled(),
            "{value:?} must disable governors"
        );
    }
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };
}

/// The advertised schema must move with the enforcement.
///
/// With governors off the executor rejects fragment edits. If the model were
/// still shown a schema advertising `old_str`/`new_str`, it would be refused
/// for following the contract it was given — penalising the raw arm for an
/// interface that did not exist before the port, and biasing the comparison
/// toward the governed arm.
#[test]
fn the_advertised_edit_schema_matches_the_arm() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let dir = TempDir::new().unwrap();
    let scope = config(&dir).scope;
    let edit_schema = || {
        legion_agent::agent_loop::tool_definitions_for_tests(&scope)
            .into_iter()
            .find(|tool| tool.name == "edit-as-proposal")
            .expect("edit-as-proposal is advertised")
            .input_schema
    };

    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block, so no
    // other thread in this binary can observe a partial state.
    unsafe { std::env::set_var("LEGION_AI_GOVERNORS", "off") };
    let raw = edit_schema();
    assert_eq!(
        raw["required"],
        serde_json::json!(["path", "replacement"]),
        "the raw arm must advertise the pre-port contract"
    );
    let raw_properties = raw["properties"].as_object().expect("properties");
    assert!(
        !raw_properties.contains_key("old_str") && !raw_properties.contains_key("new_str"),
        "the raw arm must not advertise a capability it will refuse"
    );

    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };
    let governed = edit_schema();
    assert_eq!(governed["required"], serde_json::json!(["path"]));
    assert!(
        governed["properties"]
            .as_object()
            .expect("properties")
            .contains_key("old_str"),
        "the governed arm advertises the fragment form"
    );
}

/// Ordering is part of the pre-port contract, not just the rule set.
///
/// A malformed edit was rejected as retryable `InvalidArguments` before any
/// path check ran. If the raw arm only enforced `replacement` at execution
/// time, a malformed edit aimed at a forbidden path would terminate the run as
/// `Blocked` instead — penalising the raw arm for a failure the pre-port loop
/// would have let the model correct.
#[test]
fn the_raw_arm_rejects_a_malformed_edit_before_checking_the_path() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("m.rs"),
        "fn main() {}
",
    )
    .unwrap();

    // No `replacement`, and a path the scope forbids.
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "edit-as-proposal",
            serde_json::json!({"path": "../outside.rs", "old_str": "a", "new_str": "b"}),
        )
        .end_turn("done")
        .build("test");

    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block, so no
    // other thread in this binary can observe a partial state.
    unsafe { std::env::set_var("LEGION_AI_GOVERNORS", "off") };
    let result = run_delegated_task_loop(
        &config(&dir),
        &provider,
        &NoOpToolHost,
        &mut Sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };

    assert!(
        matches!(result, DelegatedTaskLoopResult::Completed { .. }),
        "the missing required field must be caught first as retryable feedback, not converted \n         into a terminal block by the later path check; got {result:?}"
    );
}

/// Malformed structured arguments were a hard provider error before the port.
/// Leaving recovery on in the raw arm would run a governed reliability
/// mechanism inside the supposedly ungoverned baseline.
#[test]
fn the_raw_arm_hard_fails_on_malformed_structured_arguments() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let response = serde_json::json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "c1",
                    "type": "function",
                    "function": {"name": "read", "arguments": "not valid json {{{"}
                }]
            },
            "finish_reason": "tool_calls"
        }]
    });

    let request = || legion_ai::tool_calls::ToolCompletionRequest {
        provider: "openai-compatible".to_string(),
        model: "m".to_string(),
        system: String::new(),
        turns: vec![legion_ai::tool_calls::ToolConversationTurn {
            role: "user".to_string(),
            blocks: vec![legion_ai::tool_calls::ToolTurnBlock::Text("go".to_string())],
        }],
        tools: vec![],
        max_tokens: 64,
        legion_tools: false,
    };

    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block, so no
    // other thread in this binary can observe a partial state.
    unsafe { std::env::set_var("LEGION_AI_GOVERNORS", "off") };
    let raw = legion_ai_providers::OpenAiCompatibleProvider::with_transport(
        "openai-test",
        "https://api.openai.com/v1",
        Some("k".to_string()),
        FixedResponseTransport(response.clone()),
    )
    .complete_with_tools(request());
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };
    assert!(
        raw.is_err(),
        "the raw arm must reproduce the pre-port hard failure, got {raw:?}"
    );

    let governed = legion_ai_providers::OpenAiCompatibleProvider::with_transport(
        "openai-test",
        "https://api.openai.com/v1",
        Some("k".to_string()),
        FixedResponseTransport(response),
    )
    .complete_with_tools(request())
    .expect("the governed arm recovers instead of failing the completion");
    assert!(
        governed.blocks.iter().any(|block| matches!(
            block,
            legion_ai::tool_calls::ToolTurnBlock::MalformedToolCall { .. }
        )),
        "the governed arm surfaces a non-dispatchable malformed call"
    );
}

/// Returns the same response to every request.
#[derive(Clone)]
struct FixedResponseTransport(serde_json::Value);

impl legion_ai_providers::ProviderHttpTransport for FixedResponseTransport {
    fn post_json(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        _payload: serde_json::Value,
    ) -> Result<serde_json::Value, legion_ai::ProviderError> {
        Ok(self.0.clone())
    }
}

/// Only tools the scope grants may be advertised.
///
/// Offering one the broker will refuse invites the model to spend a turn on it
/// and then take a non-retryable denial, and it has no way to know in advance
/// which tools those are. Under a constrained-decoding transport it is worse
/// than a wasted turn: every advertised tool is an equally legal branch of the
/// grammar, and a benchmark run with `terminal-command` advertised but not
/// granted blocked on all 13 tasks because the model kept picking a branch
/// that could only fail.
#[test]
fn only_granted_tools_are_advertised() {
    let dir = TempDir::new().unwrap();
    let scope = config(&dir).scope;
    assert_eq!(
        scope.allowed_tools,
        vec![LegionToolKind::Read, LegionToolKind::EditAsProposal],
        "fixture precondition"
    );

    let advertised: Vec<String> = legion_agent::agent_loop::tool_definitions_for_tests(&scope)
        .into_iter()
        .map(|tool| tool.name)
        .collect();

    assert!(advertised.contains(&"read".to_string()));
    assert!(advertised.contains(&"edit-as-proposal".to_string()));
    for denied in [
        "terminal-command",
        "mcp-passthrough",
        "grep",
        "glob",
        "outline",
    ] {
        assert!(
            !advertised.contains(&denied.to_string()),
            "`{denied}` is not in the scope and must not be offered: {advertised:?}"
        );
    }
}

// ─── Loop governors (waste containment) ──────────────────────────────────────

/// Audit sink that keeps every step, so these tests assert on the trail the
/// user actually sees rather than on private governor state.
#[derive(Default)]
struct RecordingSink(Vec<DelegatedTaskLoopStepRecord>);

impl DelegatedTaskAuditSink for RecordingSink {
    fn record_step(&mut self, step: DelegatedTaskLoopStepRecord) {
        self.0.push(step);
    }
}

impl RecordingSink {
    fn count(&self, reason: &str) -> usize {
        self.0
            .iter()
            .filter(|s| s.reason.as_deref() == Some(reason))
            .count()
    }
}

fn read_of(path: &str) -> serde_json::Value {
    serde_json::json!({ "path": path })
}

/// Run a script against a fixture worktree and return the audit trail with it.
fn run_recording(
    dir: &TempDir,
    provider: &dyn ToolCallingProvider,
) -> (DelegatedTaskLoopResult, RecordingSink) {
    let mut sink = RecordingSink::default();
    let result = run_delegated_task_loop(
        &config(dir),
        provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        &AllowAllBroker,
    )
    .expect("loop must not error");
    (result, sink)
}

fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("m.rs"), "fn main() {}\n").unwrap();
    dir
}

/// A script of `n` identical reads followed by an end turn.
fn repeated_reads(n: usize) -> ScriptedToolCallingProviderBuilder {
    let mut builder = ScriptedToolCallingProviderBuilder::new();
    for i in 0..n {
        builder = builder.tool_use(&format!("t{i}"), "read", read_of("m.rs"));
    }
    builder
}

#[test]
fn a_repeated_read_is_not_executed_twice() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block.
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };

    let dir = fixture();
    let provider = repeated_reads(3).end_turn("done").build("test");
    let (_result, sink) = run_recording(&dir, &provider);

    assert_eq!(
        sink.count("served_from_dedup_cache"),
        2,
        "the second and third identical reads must be served from cache"
    );
}

#[test]
fn a_deduplicated_call_still_appears_in_the_audit_trail() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block.
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };

    let dir = fixture();
    let provider = repeated_reads(2).end_turn("done").build("test");
    let (_result, sink) = run_recording(&dir, &provider);

    let requests = sink
        .0
        .iter()
        .filter(|s| s.kind == legion_protocol::DelegatedTaskLoopStepKind::ToolCallRequest)
        .count();
    assert_eq!(
        requests, 2,
        "the model made two calls; omitting one to make the trail look tidy \
         would be the audit lying about what the model did"
    );
}

/// An edit on an unread file is never refused for being unread.
///
/// This is the case that made the first version of the hint wrong: it
/// rejected the edit up front, so a model that emits one edit and ends its
/// turn lost the only work the run produced. The hint must cost nothing.
#[test]
fn a_good_edit_on_an_unread_file_is_never_refused() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block.
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };

    let dir = fixture();
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "edit-as-proposal",
            serde_json::json!({
                "path": "m.rs",
                "old_str": "fn main() {}",
                "new_str": "fn main() { todo!() }"
            }),
        )
        .end_turn("done")
        .build("test");

    let (result, _sink) = run_recording(&dir, &provider);

    let DelegatedTaskLoopResult::Completed { proposals, .. } = result else {
        panic!("an edit that resolves must produce a proposal, got {result:?}");
    };
    assert_eq!(proposals.len(), 1);
}

/// A *failed* edit on an unread file is where the hint belongs.
#[test]
fn a_failed_edit_on_an_unread_file_is_told_to_read_it() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block.
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };

    let dir = fixture();
    // An anchor that is not in the file, so resolution fails.
    let bad_edit = serde_json::json!({
        "path": "m.rs",
        "old_str": "fn nonexistent_anchor() {}",
        "new_str": "fn replaced() {}"
    });
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "edit-as-proposal", bad_edit)
        // The guard fails the run if this text never reached the model, so the
        // assertion is that the hint was actually delivered, not merely built.
        .expect_prior_result_contains("You have not read `m.rs`")
        .end_turn("done")
        .build("test");

    let (result, _sink) = run_recording(&dir, &provider);
    assert!(
        matches!(result, DelegatedTaskLoopResult::Completed { .. }),
        "the scripted guard asserts the hint reached the model: {result:?}"
    );
}

#[test]
fn a_run_that_stops_producing_new_information_is_stopped() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block.
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };

    let dir = fixture();
    let provider = repeated_reads(8).end_turn("done").build("test");
    let (result, _sink) = run_recording(&dir, &provider);

    let DelegatedTaskLoopResult::StoppedNoProgress { proposals, .. } = result else {
        panic!("a run making only repeated calls must be stopped, got {result:?}");
    };
    assert!(
        proposals.is_empty(),
        "this run proposed nothing; the point of the variant is that it would \
         have carried proposals had it made any"
    );
}

#[test]
fn an_early_stop_keeps_the_proposals_the_run_produced() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block.
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };

    let dir = fixture();
    let mut builder = ScriptedToolCallingProviderBuilder::new()
        .tool_use("r0", "read", read_of("m.rs"))
        .tool_use(
            "e1",
            "edit-as-proposal",
            serde_json::json!({
                "path": "m.rs",
                "old_str": "fn main() {}",
                "new_str": "fn main() { todo!() }"
            }),
        );
    // Then the model goes in circles.
    for i in 0..6 {
        builder = builder.tool_use(&format!("t{i}"), "read", read_of("m.rs"));
    }
    let provider = builder.end_turn("done").build("test");

    let (result, _sink) = run_recording(&dir, &provider);

    let DelegatedTaskLoopResult::StoppedNoProgress { proposals, .. } = result else {
        panic!("expected the idle-turn governor to stop this run, got {result:?}");
    };
    assert_eq!(
        proposals.len(),
        1,
        "the edit was good before the model started looping; discarding it would \
         make the governor destroy work the budget path it replaces would keep"
    );
}

#[test]
fn the_measurement_arm_sees_no_governors_at_all() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block.
    unsafe { std::env::set_var("LEGION_AI_GOVERNORS", "off") };

    let dir = fixture();
    let provider = repeated_reads(8).end_turn("done").build("test");
    let (result, sink) = run_recording(&dir, &provider);

    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };

    assert_eq!(
        sink.count("served_from_dedup_cache"),
        0,
        "dedup must be off in the measurement arm"
    );
    assert_eq!(
        sink.count("read_before_write"),
        0,
        "the read-first hint must be off in the measurement arm"
    );
    assert!(
        matches!(result, DelegatedTaskLoopResult::Completed { .. }),
        "the idle-turn stop must be off in the measurement arm, so the same \
         script runs to its end: got {result:?}"
    );
}

/// A cache hit costs the model exactly what a fresh call costs.
///
/// The whole cached result is sent back, so serving it free would let repeated
/// large reads deliver many times the configured cumulative ceiling while the
/// budget that exists to stop that reads zero.
#[test]
fn a_cached_result_is_charged_to_the_output_budget() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block.
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };

    let dir = TempDir::new().unwrap();
    // Large enough that a couple of repeats blow a small cumulative ceiling.
    std::fs::write(dir.path().join("m.rs"), "x".repeat(4096)).unwrap();

    let mut config = config(&dir);
    config.budget.max_total_tool_output_bytes = 6_000;

    let mut builder = ScriptedToolCallingProviderBuilder::new();
    for i in 0..4 {
        builder = builder.tool_use(&format!("t{i}"), "read", read_of("m.rs"));
    }
    let provider = builder.end_turn("done").build("test");

    let mut sink = RecordingSink::default();
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
        DelegatedTaskLoopResult::BudgetExhausted { reason } => assert!(
            reason.contains("max_total_tool_output_bytes"),
            "cached repeats must hit the same ceiling as fresh reads: {reason}"
        ),
        other => panic!("expected the output budget to stop this run, got {other:?}"),
    }
    assert!(
        sink.count("served_from_dedup_cache; max_total_tool_output_bytes budget exhausted") == 1,
        "the request that tripped the ceiling must still be paired with a result"
    );
}

/// A cache hit is a successful call and breaks a run of retries.
///
/// Without this, rejections separated by successful repeated reads still
/// accumulate, and the loop eventually reports a consecutive-retry limit the
/// model never actually hit consecutively.
#[test]
fn a_cached_result_resets_the_retry_counter() {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: holds `ENV_GUARD` across the whole env-mutating block.
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };

    let dir = fixture();
    let mut config = config(&dir);
    config.budget.max_consecutive_retries = 2;

    // An edit that cannot resolve, so each attempt is a retryable rejection.
    let bad = serde_json::json!({
        "path": "m.rs",
        "old_str": "fn absent_from_the_file() {}",
        "new_str": "fn replaced() {}"
    });
    // Rejection, cached read, rejection, cached read, rejection. Three
    // rejections in total but never three in a row, so the run must survive.
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("r0", "read", read_of("m.rs"))
        .tool_use("e1", "edit-as-proposal", bad.clone())
        .tool_use("c1", "read", read_of("m.rs"))
        .tool_use("e2", "edit-as-proposal", bad.clone())
        .tool_use("c2", "read", read_of("m.rs"))
        .tool_use("e3", "edit-as-proposal", bad)
        .end_turn("done")
        .build("test");

    let mut sink = RecordingSink::default();
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
        "three non-consecutive rejections must not trip a consecutive-retry \
         limit of two: got {result:?}"
    );
}
