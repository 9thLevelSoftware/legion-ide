//! The org policy bundle's MCP allowlist reaches the delegated tool loop (P9.F2.T3).
//!
//! `legion-agent` is forbidden from depending on `legion-security` in production
//! (`plans/dependency-policy.md`), so an org bundle can only reach the tool-call
//! chokepoint through the injected `CapabilityBrokerPort`. Every MCP call mints
//! the same capability id, `delegate.tool.mcp-passthrough`, which means the
//! server id and tool name have to travel in the request *context* or a per-tool
//! allowlist is unenforceable here no matter what the bundle says.
//!
//! The first test below is the isolation test for exactly that: it uses a broker
//! that grants everything, so the only thing it can fail on is the operands not
//! arriving.

use std::cell::RefCell;
use std::path::Path;

use legion_agent::agent_loop::{
    DelegatedTaskAuditSink, DelegatedTaskCancellationProbe, DelegatedTaskLoopConfig,
    DelegatedToolHost, run_delegated_task_loop,
};
use legion_ai::tool_calls::ScriptedToolCallingProviderBuilder;
use legion_protocol::{
    CanonicalPath, CapabilityDecision, CapabilityDecisionId, CapabilityId, CapabilityRequest,
    CapabilityResponse, DelegatedTaskLoopBudget, DelegatedTaskLoopStepKind,
    DelegatedTaskLoopStepRecord, DelegatedTaskRiskTolerance, DelegatedTaskScope,
    DelegatedTaskScopeTargetKind, LegionToolKind, ProtocolResult,
};
use legion_security::{BundleEnforcementPolicy, McpToolAllowlistPolicy};
use tempfile::TempDir;

// ─── Fakes ────────────────────────────────────────────────────────────────────

struct RecordingAuditSink {
    steps: Vec<DelegatedTaskLoopStepRecord>,
}

impl DelegatedTaskAuditSink for RecordingAuditSink {
    fn record_step(&mut self, step: DelegatedTaskLoopStepRecord) {
        self.steps.push(step);
    }
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
        Ok("mcp result".to_string())
    }
}

/// What one broker call saw.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SeenRequest {
    capability: String,
    mcp_server_id: Option<String>,
    mcp_tool_name: Option<String>,
}

/// Applies only the bundle's MCP allowlist, recording what it saw.
///
/// Deliberately *not* a `DenyByDefaultBroker`: that broker's base matrix has no
/// `delegate.tool.*` arm and refuses every delegated tool call outright, so a
/// test built on it would pass whether or not the allowlist did anything. This
/// broker grants everything the allowlist does not object to, which makes the
/// allowlist the only thing under test.
struct McpAllowlistBroker {
    policy: BundleEnforcementPolicy,
    seen: RefCell<Vec<SeenRequest>>,
}

impl McpAllowlistBroker {
    fn new(policy: McpToolAllowlistPolicy) -> Self {
        Self {
            policy: BundleEnforcementPolicy {
                mcp: policy,
                ..BundleEnforcementPolicy::default()
            },
            seen: RefCell::new(Vec::new()),
        }
    }

    fn seen(&self) -> Vec<SeenRequest> {
        self.seen.borrow().clone()
    }
}

impl legion_protocol::CapabilityBrokerPort for McpAllowlistBroker {
    fn handle(&self, request: CapabilityRequest) -> ProtocolResult<CapabilityResponse> {
        let CapabilityRequest::Request {
            capability_id,
            context,
            ..
        } = &request
        else {
            return Ok(CapabilityResponse::Decision(CapabilityDecision {
                decision_id: CapabilityDecisionId(0),
                granted: false,
                capability: CapabilityId("unknown".to_string()),
                reason: Some("unexpected request variant".to_string()),
            }));
        };

        self.seen.borrow_mut().push(SeenRequest {
            capability: capability_id.0.clone(),
            mcp_server_id: context.mcp_server_id.clone(),
            mcp_tool_name: context.mcp_tool_name.clone(),
        });

        let refusal = self.policy.refusal(&capability_id.0, context);
        Ok(CapabilityResponse::Decision(CapabilityDecision {
            decision_id: CapabilityDecisionId(1),
            granted: refusal.is_none(),
            capability: capability_id.clone(),
            reason: refusal.map(|(_, reason)| reason),
        }))
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn enterprise_mcp_policy() -> McpToolAllowlistPolicy {
    McpToolAllowlistPolicy {
        enforced: true,
        allowed_servers: vec!["legion-internal".to_string()],
        allowed_tools: vec!["legion-internal/search_docs".to_string()],
        tool_capability_prefixes: Vec::new(),
    }
}

fn config(dir: &TempDir) -> DelegatedTaskLoopConfig {
    let root = dir.path().to_path_buf();
    DelegatedTaskLoopConfig {
        system_prompt: "You are a helpful assistant.".to_string(),
        initial_message: "Do the task.".to_string(),
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
            allowed_tools: vec![LegionToolKind::McpPassthrough],
            forbidden_paths: vec![],
            schema_version: 1,
        },
        forbidden_paths: vec![],
    }
}

fn run_mcp_call(
    server_id: &str,
    tool_name: &str,
    broker: &McpAllowlistBroker,
) -> RecordingAuditSink {
    let dir = TempDir::new().expect("tempdir");
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "t1",
            "mcp-passthrough",
            serde_json::json!({
                "server_id": server_id,
                "tool_name": tool_name,
                "arguments": {},
            }),
        )
        .end_turn("done")
        .build("test");

    let mut sink = RecordingAuditSink { steps: Vec::new() };
    run_delegated_task_loop(
        &config(&dir),
        &provider,
        &NoOpToolHost,
        &mut sink,
        &NeverCancelled,
        broker,
    )
    .expect("loop must not error");
    sink
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn the_broker_receives_the_mcp_server_and_tool_identity() {
    // Isolation test. The broker grants the call, so the only thing that can
    // fail here is the operands not reaching it — which is precisely what an
    // org bundle's per-tool allowlist depends on.
    let broker = McpAllowlistBroker::new(enterprise_mcp_policy());
    run_mcp_call("legion-internal", "search_docs", &broker);

    let seen = broker.seen();
    let mcp_calls: Vec<&SeenRequest> = seen
        .iter()
        .filter(|request| request.capability == "delegate.tool.mcp-passthrough")
        .collect();
    assert_eq!(
        mcp_calls.len(),
        1,
        "expected exactly one MCP broker call, saw {seen:?}"
    );
    assert_eq!(
        mcp_calls[0].mcp_server_id.as_deref(),
        Some("legion-internal"),
        "the MCP server id must reach the broker"
    );
    assert_eq!(
        mcp_calls[0].mcp_tool_name.as_deref(),
        Some("search_docs"),
        "the MCP tool name must reach the broker"
    );
}

#[test]
fn an_allowlisted_mcp_tool_is_executed() {
    let broker = McpAllowlistBroker::new(enterprise_mcp_policy());
    let sink = run_mcp_call("legion-internal", "search_docs", &broker);

    assert!(
        sink.steps
            .iter()
            .any(|step| step.kind == DelegatedTaskLoopStepKind::ToolCallResult),
        "an allowlisted tool must run: {:?}",
        sink.steps
    );
    assert!(
        !sink
            .steps
            .iter()
            .any(|step| step.kind == DelegatedTaskLoopStepKind::ToolCallRejected),
        "an allowlisted tool must not be rejected"
    );
}

#[test]
fn a_tool_outside_the_allowlist_is_refused_at_the_loop_chokepoint() {
    let broker = McpAllowlistBroker::new(enterprise_mcp_policy());
    let sink = run_mcp_call("evil-corp", "exfiltrate", &broker);

    assert!(
        sink.steps
            .iter()
            .any(|step| step.kind == DelegatedTaskLoopStepKind::ToolCallRejected),
        "a tool outside the org allowlist must be rejected: {:?}",
        sink.steps
    );
    assert!(
        !sink
            .steps
            .iter()
            .any(|step| step.kind == DelegatedTaskLoopStepKind::ToolCallResult),
        "a refused tool must not also produce a result"
    );
}

#[test]
fn an_allowlisted_server_does_not_admit_a_tool_outside_the_allowlist() {
    let broker = McpAllowlistBroker::new(enterprise_mcp_policy());
    let sink = run_mcp_call("legion-internal", "run_shell", &broker);

    assert!(
        sink.steps
            .iter()
            .any(|step| step.kind == DelegatedTaskLoopStepKind::ToolCallRejected),
        "a trusted server must not admit an unlisted tool: {:?}",
        sink.steps
    );
}
