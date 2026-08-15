//! Cross-check test: verify that `run_delegated_task_loop` works end-to-end
//! with `OpenAiCompatibleProvider` over a fake scripted transport.
//!
//! This test exercises loop compatibility, not just DTO mapping.  It scripts
//! the provider transport to return proper OpenAI chat-completions wire-format
//! JSON responses and checks that the loop drives tool execution correctly and
//! returns the expected `Completed` result.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use legion_agent::agent_loop::{
    DelegatedTaskAuditSink, DelegatedTaskCancellationProbe, DelegatedTaskLoopConfig,
    DelegatedTaskLoopResult, DelegatedToolHost, run_delegated_task_loop,
};
use legion_ai::ProviderError;
use legion_ai_providers::{OpenAiCompatibleProvider, ProviderHttpTransport};
use legion_protocol::{
    CanonicalPath, CapabilityDecision, CapabilityDecisionId, CapabilityId, CapabilityRequest,
    CapabilityResponse, DelegatedTaskLoopBudget, DelegatedTaskLoopStepRecord,
    DelegatedTaskRiskTolerance, DelegatedTaskScope, DelegatedTaskScopeTargetKind, LegionToolKind,
    ProposalPayload, ProtocolResult,
};
use serde_json::{Value, json};
use tempfile::TempDir;

// ─── Scripted OpenAI transport ────────────────────────────────────────────────

/// A scripted `ProviderHttpTransport` that returns pre-loaded responses in FIFO
/// order.  Each `post_json` call pops the next response from the queue.
#[derive(Clone)]
struct SequentialOpenAiTransport {
    responses: Arc<Mutex<VecDeque<Value>>>,
}

impl SequentialOpenAiTransport {
    fn from_responses(responses: Vec<Value>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into())),
        }
    }
}

impl ProviderHttpTransport for SequentialOpenAiTransport {
    fn post_json(
        &self,
        _endpoint: &str,
        _bearer_token: Option<&str>,
        _payload: Value,
    ) -> Result<Value, ProviderError> {
        self.responses
            .lock()
            .expect("responses lock")
            .pop_front()
            .ok_or_else(|| ProviderError::RequestFailed {
                provider: "sequential-openai".to_string(),
                message: "SequentialOpenAiTransport: no more scripted responses".to_string(),
            })
    }
}

// ─── Test fakes (minimal copies from agent_loop_integration.rs) ──────────────

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

fn default_config(dir: &TempDir) -> DelegatedTaskLoopConfig {
    let root = dir.path().to_path_buf();
    DelegatedTaskLoopConfig {
        system_prompt: "You are a helpful assistant.".to_string(),
        initial_message: "Do the task.".to_string(),
        model: "gpt-4o-mini".to_string(),
        provider: "openai-test".to_string(),
        budget: DelegatedTaskLoopBudget::default(),
        workspace_root: root.clone(),
        worktree_root: root.clone(),
        scope: repo_scope(&root),
        forbidden_paths: vec![],
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

/// Verify that `run_delegated_task_loop` drives `OpenAiCompatibleProvider`
/// through a read→end scripted conversation and returns `Completed`.
#[test]
fn openai_provider_compatible_with_agent_loop_read_then_end() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "Hello, world!").unwrap();

    let transport = SequentialOpenAiTransport::from_responses(vec![
        // Turn 1: model requests a "read" on hello.txt.
        json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "tc1",
                        "type": "function",
                        "function": {
                            "name": "read",
                            "arguments": "{\"path\": \"hello.txt\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        }),
        // Turn 2: after receiving the file content, model ends naturally.
        json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Task complete: file says Hello, world!"
                },
                "finish_reason": "stop"
            }]
        }),
    ]);

    let provider = OpenAiCompatibleProvider::with_transport(
        "openai-test",
        "https://api.openai.com/v1",
        Some("test-key".to_string()),
        transport,
    );

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

    if let DelegatedTaskLoopResult::Completed {
        final_message,
        proposals,
    } = &result
    {
        assert!(
            final_message.contains("Hello"),
            "final message should reference the file content: {final_message}"
        );
        assert!(
            proposals.is_empty(),
            "no proposals expected for a read-only task"
        );
    }

    // Audit pairing invariant: every ToolCallRequest has a paired ToolCallResult.
    use legion_protocol::DelegatedTaskLoopStepKind;
    let request_cids: Vec<String> = sink
        .steps
        .iter()
        .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRequest)
        .map(|s| s.causality_id.clone())
        .collect();
    for cid in &request_cids {
        let has_result = sink.steps.iter().any(|s| {
            &s.causality_id == cid
                && matches!(
                    s.kind,
                    DelegatedTaskLoopStepKind::ToolCallResult
                        | DelegatedTaskLoopStepKind::ToolCallRejected
                )
        });
        assert!(
            has_result,
            "ToolCallRequest causality_id {cid} has no matching ToolCallResult"
        );
    }
}

/// Provider-to-loop contract for prose-embedded calls (ADR-0049).
///
/// A model that writes its call as prose reports `finish_reason: "stop"`,
/// because the provider only saw text. Both halves have to agree for recovery
/// to do anything: the provider must report the turn as tool use, and the loop
/// must dispatch the recovered call. If either half regresses, the run ends
/// after turn one with the file never read.
#[test]
fn prose_embedded_call_is_recovered_and_dispatched_end_to_end() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "Hello, world!").unwrap();

    let transport = SequentialOpenAiTransport::from_responses(vec![
        // Turn 1: the call is written as prose under a near-miss name, with
        // `stop` as the finish reason — the shape small local models produce.
        json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "I'll read it.\n<tool_call>{\"name\":\"Read\",\"arguments\":{\"file_path\":\"hello.txt\"}}</tool_call>"
                },
                "finish_reason": "stop"
            }]
        }),
        json!({
            "choices": [{
                "message": {"role": "assistant", "content": "Done: Hello, world!"},
                "finish_reason": "stop"
            }]
        }),
    ]);

    let provider = OpenAiCompatibleProvider::with_transport(
        "openai-test",
        "https://api.openai.com/v1",
        Some("test-key".to_string()),
        transport,
    );

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

    // The decisive assertion: the recovered call actually reached dispatch,
    // under Legion's registry name rather than the name the model wrote.
    let read_calls = sink
        .steps
        .iter()
        .filter(|step| step.tool_name.as_deref() == Some("read"))
        .count();
    assert!(
        read_calls > 0,
        "the prose-embedded call must be dispatched as `read`; audit steps: {:?}",
        sink.steps
            .iter()
            .map(|s| (s.kind, s.tool_name.clone()))
            .collect::<Vec<_>>()
    );
}

/// The same contract for a call whose arguments cannot be parsed: the
/// diagnostic has to reach the model instead of ending the run silently.
#[test]
fn prose_embedded_call_with_bad_arguments_feeds_the_diagnostic_back() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "Hello, world!").unwrap();

    let transport = SequentialOpenAiTransport::from_responses(vec![
        json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "<tool_call>{\"function\":{\"name\":\"read\",\"arguments\":\"{not json\"}}</tool_call>"
                },
                "finish_reason": "stop"
            }]
        }),
        // Turn 2: corrected call, only reachable if the loop kept going.
        json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "tc1",
                        "type": "function",
                        "function": {"name": "read", "arguments": "{\"path\": \"hello.txt\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        }),
        json!({
            "choices": [{
                "message": {"role": "assistant", "content": "Recovered."},
                "finish_reason": "stop"
            }]
        }),
    ]);

    let provider = OpenAiCompatibleProvider::with_transport(
        "openai-test",
        "https://api.openai.com/v1",
        Some("test-key".to_string()),
        transport,
    );

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
    .expect("a malformed recovered call must not error the loop");

    assert!(
        matches!(result, DelegatedTaskLoopResult::Completed { .. }),
        "expected Completed after correction, got {result:?}"
    );
    assert_eq!(
        sink.steps
            .iter()
            .filter(|step| step.reason.as_deref() == Some("malformed_tool_arguments"))
            .count(),
        1,
        "the malformed recovered call is audited"
    );
}

/// A model that writes its edit as a SEARCH/REPLACE block — with no tool call
/// at all — must still produce an edit proposal. Models trained on that format
/// emit it unprompted, and without recovery the edit reads as prose and is
/// lost (ADR-0049).
#[test]
fn block_format_edit_written_as_prose_reaches_the_edit_tool() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("lib.rs"), "fn old_name() {}\n").unwrap();

    let transport = SequentialOpenAiTransport::from_responses(vec![
        json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "I'll rename it:\n\nlib.rs\n<<<<<<< SEARCH\nfn old_name() {}\n=======\nfn new_name() {}\n>>>>>>> REPLACE\n"
                },
                "finish_reason": "stop"
            }]
        }),
        json!({
            "choices": [{
                "message": {"role": "assistant", "content": "Renamed."},
                "finish_reason": "stop"
            }]
        }),
    ]);

    let provider = OpenAiCompatibleProvider::with_transport(
        "openai-test",
        "https://api.openai.com/v1",
        Some("test-key".to_string()),
        transport,
    );

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

    let DelegatedTaskLoopResult::Completed { proposals, .. } = result else {
        panic!("expected Completed, got {result:?}");
    };
    assert_eq!(
        proposals.len(),
        1,
        "the block-format edit must reach the edit tool; audit: {:?}",
        sink.steps
            .iter()
            .map(|s| (s.kind, s.tool_name.clone()))
            .collect::<Vec<_>>()
    );

    // Resolved against the file by exact match, not treated as whole content.
    let content = match &proposals[0].payload {
        ProposalPayload::CreateFile(create) => create.initial_content.clone().unwrap_or_default(),
        other => panic!("expected a file-content payload, got {other:?}"),
    };
    assert_eq!(content, "fn new_name() {}\n");
}
