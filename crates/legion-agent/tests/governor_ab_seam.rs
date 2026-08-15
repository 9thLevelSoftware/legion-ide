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
use legion_ai::tool_calls::ScriptedToolCallingProviderBuilder;
use legion_protocol::{
    CanonicalPath, CapabilityDecision, CapabilityDecisionId, CapabilityId, CapabilityRequest,
    CapabilityResponse, DelegatedTaskLoopBudget, DelegatedTaskLoopStepRecord,
    DelegatedTaskRiskTolerance, DelegatedTaskScope, DelegatedTaskScopeTargetKind, LegionToolKind,
    ProtocolResult,
};
use tempfile::TempDir;

/// Serializes the two cases in this binary. Isolating the binary keeps other
/// suites safe; this keeps these two from racing each other.
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

    // SAFETY: this binary runs in its own process and this is its only test
    // touching the environment, so no other thread can observe the change.
    unsafe { std::env::set_var("LEGION_AI_GOVERNORS", "off") };
    assert!(!legion_ai::small_model_governors_enabled());
    let without = proposals_for_fragment_edit(&dir);

    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };
    assert!(legion_ai::small_model_governors_enabled());
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
    for value in ["", "0", "false", "no", "OFFF", "on"] {
        unsafe { std::env::set_var("LEGION_AI_GOVERNORS", value) };
        assert!(
            legion_ai::small_model_governors_enabled(),
            "{value:?} must not be read as off"
        );
    }
    for value in ["off", "OFF", "Off"] {
        unsafe { std::env::set_var("LEGION_AI_GOVERNORS", value) };
        assert!(
            !legion_ai::small_model_governors_enabled(),
            "{value:?} must disable governors"
        );
    }
    unsafe { std::env::remove_var("LEGION_AI_GOVERNORS") };
}
