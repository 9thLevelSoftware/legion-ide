//! Tool-calling contract for the two local-runtime providers (roadmap 3.1).
//!
//! Both providers are driven through the public `with_transport` seam, so no
//! test here needs Ollama or `llama-server` running. That is the point: this
//! file states what the adapters do with a *given* response, which is the part
//! that can be pinned down without a machine-dependent integration.
//!
//! Four things actually happen on this path and each has a case for both
//! providers: a well-formed tool call, a reply with no tool call at all,
//! arguments that never parsed, and a transport that failed outright.
//!
//! Note on the malformed case: what happens depends on `LEGION_AI_GOVERNORS`,
//! which is a process-wide measurement seam (`legion_ai::governance`). The
//! tests read it and assert the contract for the arm they are actually running
//! in rather than assuming the default, because the bench runner sets it and a
//! test that assumed either value would fail on the other machine.
//!
//! `LEGION_AI_TOOL_TRANSPORT=schema` is deliberately *not* accommodated: it
//! replaces the tool wire format with a grammar, so there is no equivalent
//! contract to assert, and a run with it set is asking these adapters to do a
//! different thing. The grammar payload is covered by unit tests in
//! `ollama_tools`, which take the schema as an argument rather than reading the
//! environment.

use legion_ai::tool_calls::{
    ToolCallingProvider, ToolCompletionRequest, ToolCompletionResponse, ToolCompletionStopReason,
    ToolConversationTurn, ToolDefinition, ToolTurnBlock,
};
use legion_ai::{ModelProvider, ProviderError};
use legion_ai_providers::{LlamaCppProvider, OllamaProvider, ProviderHttpTransport};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Transport doubles
// ---------------------------------------------------------------------------

/// Returns one fixed body and records what was posted where.
#[derive(Debug, Clone)]
struct FixedTransport {
    response: Value,
    calls: Arc<Mutex<Vec<(String, Value)>>>,
}

impl FixedTransport {
    fn new(response: Value) -> Self {
        Self {
            response,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<(String, Value)> {
        self.calls.lock().expect("calls lock").clone()
    }
}

impl ProviderHttpTransport for FixedTransport {
    fn post_json(
        &self,
        endpoint: &str,
        _bearer_token: Option<&str>,
        payload: Value,
    ) -> Result<Value, ProviderError> {
        self.calls
            .lock()
            .expect("calls lock")
            .push((endpoint.to_string(), payload));
        Ok(self.response.clone())
    }
}

/// Fails every request, the way an unreachable local server does.
#[derive(Debug, Clone, Default)]
struct FailingTransport;

impl ProviderHttpTransport for FailingTransport {
    fn post_json(
        &self,
        endpoint: &str,
        _bearer_token: Option<&str>,
        _payload: Value,
    ) -> Result<Value, ProviderError> {
        Err(ProviderError::RequestFailed {
            provider: "http".to_string(),
            message: format!("connection refused: {endpoint}"),
        })
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn read_tool() -> ToolDefinition {
    ToolDefinition {
        name: "read".to_string(),
        description: "Read a file".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"],
        }),
    }
}

fn request(provider: &str, model: &str) -> ToolCompletionRequest {
    ToolCompletionRequest {
        provider: provider.to_string(),
        model: model.to_string(),
        system: "You are Legion.".to_string(),
        turns: vec![ToolConversationTurn {
            role: "user".to_string(),
            blocks: vec![ToolTurnBlock::Text("Read a.rs".to_string())],
        }],
        tools: vec![read_tool()],
        max_tokens: 256,
        legion_tools: true,
    }
}

fn ollama(response: Value) -> (OllamaProvider<FixedTransport>, FixedTransport) {
    let transport = FixedTransport::new(response);
    let provider =
        OllamaProvider::with_transport("ollama-test", "http://localhost:11434", transport.clone());
    (provider, transport)
}

fn llama_cpp(response: Value) -> (LlamaCppProvider<FixedTransport>, FixedTransport) {
    let transport = FixedTransport::new(response);
    // No API key: `llama-server` is unauthenticated by default, and a local
    // provider that demanded a credential would be unusable out of the box.
    let provider = LlamaCppProvider::with_transport(
        "llama-cpp-test",
        "http://localhost:8080/v1",
        None,
        transport.clone(),
    );
    (provider, transport)
}

/// The single dispatchable call in a response, or a panic naming what was there
/// instead.
fn only_tool_use(response: &ToolCompletionResponse) -> (&str, &str, &Value) {
    let uses: Vec<_> = response
        .blocks
        .iter()
        .filter(|block| block.is_dispatchable_tool_use())
        .collect();
    assert_eq!(
        uses.len(),
        1,
        "expected exactly one dispatchable call, got blocks: {:?}",
        response.blocks
    );
    match uses[0] {
        ToolTurnBlock::ToolUse { id, name, input } => (id, name, input),
        other => panic!("expected ToolUse, got {other:?}"),
    }
}

/// Assert the contract for arguments that never parsed, in whichever
/// measurement arm this process is running.
///
/// Governed: a typed, non-dispatchable block the loop can feed back. Baseline
/// (`LEGION_AI_GOVERNORS=off`): the hard provider error this was before the
/// SmallCode port. The invariant common to both — unvalidated arguments never
/// reach dispatch — is what actually matters, and is asserted either way.
fn assert_unparseable_arguments_are_refused(
    outcome: Result<ToolCompletionResponse, ProviderError>,
    expected_raw: &str,
) {
    if legion_ai::governance::small_model_governors_enabled() {
        let response = outcome.expect("malformed arguments are recoverable, not a transport error");
        assert!(
            !response
                .blocks
                .iter()
                .any(ToolTurnBlock::is_dispatchable_tool_use),
            "no dispatchable call may come from unparseable arguments: {:?}",
            response.blocks
        );
        let malformed = response
            .blocks
            .iter()
            .find_map(|block| match block {
                ToolTurnBlock::MalformedToolCall {
                    name,
                    raw_arguments,
                    diagnostic,
                    ..
                } => Some((name, raw_arguments, diagnostic)),
                _ => None,
            })
            .expect("a MalformedToolCall block is surfaced");
        assert_eq!(malformed.0, "read");
        assert_eq!(malformed.1, expected_raw);
        assert!(
            malformed.2.contains("valid JSON"),
            "the diagnostic must tell the model what to fix: {}",
            malformed.2
        );
        assert_eq!(
            response.stop_reason,
            ToolCompletionStopReason::ToolUse,
            "the diagnostic must reach the loop rather than ending the run"
        );
    } else {
        let error = outcome.expect_err("the baseline arm fails hard on unparseable arguments");
        assert!(
            matches!(error, ProviderError::RequestFailed { .. }),
            "expected RequestFailed, got {error:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Ollama — native /api/chat
// ---------------------------------------------------------------------------

#[test]
fn ollama_advertises_tool_use() {
    let (provider, _) = ollama(json!({}));
    assert!(
        provider.capabilities().tool_use,
        "OllamaProvider implements ToolCallingProvider and must say so"
    );
}

#[test]
fn ollama_parses_a_well_formed_native_tool_call() {
    let (provider, transport) = ollama(json!({
        "model": "llama3.2",
        "created_at": "2026-08-17T00:00:00Z",
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [
                { "function": { "name": "read", "arguments": { "path": "a.rs" } } }
            ]
        },
        "done_reason": "stop",
        "done": true
    }));

    let response = provider
        .complete_with_tools(request("ollama-test", "llama3.2"))
        .expect("a well-formed native tool call parses");

    assert_eq!(response.stop_reason, ToolCompletionStopReason::ToolUse);
    let (id, name, input) = only_tool_use(&response);
    assert_eq!(name, "read");
    assert_eq!(input["path"], "a.rs");
    assert!(
        !id.is_empty(),
        "the loop pairs results by id, so one must always exist even though Ollama sends none"
    );

    // The request must have gone to the native endpoint in the native shape.
    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    let (endpoint, payload) = &calls[0];
    assert_eq!(endpoint, "http://localhost:11434/api/chat");
    assert_eq!(payload["stream"], false);
    assert_eq!(payload["options"]["num_predict"], 256);
    assert_eq!(payload["tools"][0]["function"]["name"], "read");
}

#[test]
fn ollama_reply_with_no_tool_call_ends_the_turn() {
    let (provider, _) = ollama(json!({
        "message": { "role": "assistant", "content": "Nothing to do here." },
        "done_reason": "stop",
        "done": true
    }));

    let response = provider
        .complete_with_tools(request("ollama-test", "llama3.2"))
        .expect("a plain reply is not an error");

    assert_eq!(response.stop_reason, ToolCompletionStopReason::EndTurn);
    assert!(
        !response
            .blocks
            .iter()
            .any(ToolTurnBlock::is_dispatchable_tool_use),
        "no call was made, so none may be reported"
    );
    assert_eq!(
        response.blocks,
        vec![ToolTurnBlock::Text("Nothing to do here.".to_string())],
        "the model's prose survives as prose"
    );
}

#[test]
fn ollama_unparseable_arguments_never_reach_dispatch() {
    let (provider, _) = ollama(json!({
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [
                { "function": { "name": "read", "arguments": "not valid json {{{" } }
            ]
        },
        "done_reason": "stop",
        "done": true
    }));

    assert_unparseable_arguments_are_refused(
        provider.complete_with_tools(request("ollama-test", "llama3.2")),
        "not valid json {{{",
    );
}

#[test]
fn ollama_transport_failure_is_a_provider_error() {
    let provider =
        OllamaProvider::with_transport("ollama-test", "http://localhost:11434", FailingTransport);

    let error = provider
        .complete_with_tools(request("ollama-test", "llama3.2"))
        .expect_err("an unreachable local server is a provider error");
    assert!(
        matches!(error, ProviderError::RequestFailed { .. }),
        "expected RequestFailed, got {error:?}"
    );
}

#[test]
fn ollama_response_of_the_wrong_shape_still_fails_hard() {
    // A body with no assistant message at all is a transport-level failure —
    // there is nothing here the model could be asked to correct.
    let (provider, _) = ollama(json!({ "done": true, "done_reason": "stop" }));
    let error = provider
        .complete_with_tools(request("ollama-test", "llama3.2"))
        .expect_err("a response missing `message` is malformed transport");
    assert!(
        matches!(error, ProviderError::RequestFailed { .. }),
        "expected RequestFailed, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// llama.cpp — delegated to the OpenAI-compatible dialect
// ---------------------------------------------------------------------------

#[test]
fn llama_cpp_parses_a_well_formed_tool_call_through_the_openai_dialect() {
    let (provider, transport) = llama_cpp(json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call-1",
                    "type": "function",
                    "function": { "name": "read", "arguments": "{\"path\": \"a.rs\"}" }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    }));

    let response = provider
        .complete_with_tools(request("llama-cpp-test", "qwen2.5-coder"))
        .expect("delegation reaches the OpenAI-compatible parser");

    assert_eq!(response.stop_reason, ToolCompletionStopReason::ToolUse);
    let (id, name, input) = only_tool_use(&response);
    assert_eq!(id, "call-1");
    assert_eq!(name, "read");
    assert_eq!(input["path"], "a.rs");
    assert_eq!(
        response.provider, "llama-cpp-test",
        "delegation must not relabel the response as the inner provider"
    );

    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    let (endpoint, payload) = &calls[0];
    assert_eq!(
        endpoint, "http://localhost:8080/v1/chat/completions",
        "llama-server serves the OpenAI dialect, so the OpenAI path is the right one"
    );
    assert_eq!(payload["max_tokens"], 256);
    assert_eq!(payload["tools"][0]["function"]["name"], "read");
}

#[test]
fn llama_cpp_reply_with_no_tool_call_ends_the_turn() {
    let (provider, _) = llama_cpp(json!({
        "choices": [{
            "message": { "role": "assistant", "content": "Nothing to do here." },
            "finish_reason": "stop"
        }]
    }));

    let response = provider
        .complete_with_tools(request("llama-cpp-test", "qwen2.5-coder"))
        .expect("a plain reply is not an error");

    assert_eq!(response.stop_reason, ToolCompletionStopReason::EndTurn);
    assert!(
        !response
            .blocks
            .iter()
            .any(ToolTurnBlock::is_dispatchable_tool_use)
    );
}

#[test]
fn llama_cpp_unparseable_arguments_never_reach_dispatch() {
    let (provider, _) = llama_cpp(json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call-1",
                    "type": "function",
                    "function": { "name": "read", "arguments": "not valid json {{{" }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    }));

    assert_unparseable_arguments_are_refused(
        provider.complete_with_tools(request("llama-cpp-test", "qwen2.5-coder")),
        "not valid json {{{",
    );
}

#[test]
fn llama_cpp_transport_failure_is_a_provider_error() {
    let provider = LlamaCppProvider::with_transport(
        "llama-cpp-test",
        "http://localhost:8080/v1",
        None,
        FailingTransport,
    );

    let error = provider
        .complete_with_tools(request("llama-cpp-test", "qwen2.5-coder"))
        .expect_err("an unreachable local server is a provider error");
    assert!(
        matches!(error, ProviderError::RequestFailed { .. }),
        "expected RequestFailed, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// Shared behavior
// ---------------------------------------------------------------------------

/// A small model that writes its call as prose must be recovered on both local
/// runtimes, not just the OpenAI-compatible one.
///
/// This is the reliability behavior the whole local-model path depends on, and
/// the reason `OllamaProvider` routes through `legion_ai::normalize` rather
/// than parsing strictly. Guarded on the measurement arm: with the governors
/// off, prose is prose by design.
#[test]
fn both_local_providers_recover_a_call_written_as_prose() {
    if !legion_ai::governance::small_model_governors_enabled() {
        return;
    }
    let prose = "<tool_call>{\"name\":\"Read\",\"arguments\":{\"file_path\":\"a.rs\"}}</tool_call>";

    let (ollama_provider, _) = ollama(json!({
        "message": { "role": "assistant", "content": prose },
        "done_reason": "stop",
        "done": true
    }));
    let ollama_response = ollama_provider
        .complete_with_tools(request("ollama-test", "llama3.2"))
        .expect("prose recovery does not fail the completion");

    let (_, name, input) = only_tool_use(&ollama_response);
    assert_eq!(name, "read", "the near-miss name resolves to Legion's tool");
    assert_eq!(input["path"], "a.rs", "arguments are canonicalized with it");
    assert_eq!(
        ollama_response.stop_reason,
        ToolCompletionStopReason::ToolUse,
        "a recovered call must not be reported as the end of the turn"
    );

    let (llama_provider, _) = llama_cpp(json!({
        "choices": [{
            "message": { "role": "assistant", "content": prose },
            "finish_reason": "stop"
        }]
    }));
    let llama_response = llama_provider
        .complete_with_tools(request("llama-cpp-test", "qwen2.5-coder"))
        .expect("prose recovery does not fail the completion");

    let (_, name, input) = only_tool_use(&llama_response);
    assert_eq!(name, "read");
    assert_eq!(input["path"], "a.rs");
    assert_eq!(
        llama_response.stop_reason,
        ToolCompletionStopReason::ToolUse
    );
}
