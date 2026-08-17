//! Native Ollama `/api/chat` tool calling.
//!
//! Ollama is *not* OpenAI-compatible on this path. Its native chat endpoint
//! differs in four ways that each break a strict OpenAI parser:
//!
//! * the assistant message is at `message`, not `choices[0].message`;
//! * `tool_calls[].function.arguments` arrives as a JSON **object**, already
//!   parsed by the server, where OpenAI sends a JSON *string*;
//! * a tool call carries **no id** — Ollama correlates a result by `tool_name`
//!   instead, so ids have to be synthesized for the agent loop;
//! * generation limits live under `options.num_predict`, and the stop reason is
//!   `done_reason`, which never takes OpenAI's `"tool_calls"` value.
//!
//! Ollama also exposes an OpenAI-compatible surface at `/v1`, which
//! [`crate::OpenAiCompatibleProvider`] can already drive (that is what
//! `legion-bench` does). This module exists because the native endpoint is the
//! one Ollama documents and evolves first, and because routing through the
//! compatibility shim costs a translation layer whose failure modes are
//! Ollama's, not ours.
//!
//! Everything downstream of parsing is deliberately identical to the
//! OpenAI-compatible path: the same tolerant recovery in
//! [`legion_ai::normalize`], the same non-dispatchable
//! [`ToolTurnBlock::MalformedToolCall`] for arguments that never parsed, and
//! the same measurement seam (`LEGION_AI_GOVERNORS=off`) that reproduces
//! pre-port behavior. A local model must not get a different reliability story
//! depending on which local runtime is in front of it.

use std::collections::HashMap;

use legion_ai::normalize::{ExtractionInput, extract_tool_calls};
use legion_ai::tool_calls::{
    ToolCallingProvider, ToolCompletionRequest, ToolCompletionResponse, ToolCompletionStopReason,
    ToolConversationTurn, ToolDefinition, ToolTurnBlock,
};
use legion_ai::{ProviderError, ProviderId};
use serde_json::{Value, json};

use crate::{
    OllamaProvider, ProviderHttpTransport, bounded_raw_arguments, schema_constrained_response,
    schema_constrained_tool_schema, schema_constrained_tools_enabled,
};

/// Ollama's native chat endpoint. Distinct from the `/v1/chat/completions`
/// compatibility shim, which speaks OpenAI's dialect instead.
const OLLAMA_CHAT_PATH: &str = "/api/chat";

/// Prefix for tool-call ids this adapter synthesizes.
///
/// Ollama assigns none, but the agent loop pairs every result with the id of
/// the call it answers. Deriving the id from the call's position keeps it
/// stable across a replay of the same response, which a random id would not.
const SYNTHESIZED_CALL_ID_PREFIX: &str = "ollama";

impl<T> ToolCallingProvider for OllamaProvider<T>
where
    T: ProviderHttpTransport,
{
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        // Schema-constrained tool calling, as on the OpenAI-compatible path:
        // the decoder is handed a grammar it must satisfy, so a malformed call
        // becomes unrepresentable rather than repairable. Opt-in, because a
        // model with working native tool use must not be downgraded to it.
        // Ollama spells the grammar `format` and takes the schema inline,
        // where OpenAI wraps it in `response_format.json_schema`.
        let schema = if schema_constrained_tools_enabled() {
            schema_constrained_tool_schema(&request.tools)
        } else {
            None
        };

        let response = self.transport.post_json(
            &self.endpoint(OLLAMA_CHAT_PATH),
            None,
            chat_payload(&request, schema.as_ref()),
        )?;

        let message = assistant_message(&self.id, &response)?;

        if schema.is_some() {
            // Under a grammar the whole reply is one action object, so it is
            // parsed directly rather than run through prose recovery.
            return Ok(schema_constrained_response(
                self.id.clone(),
                request.model.clone(),
                message_text(message),
            ));
        }

        parse_chat_message(&self.id, &request, message, done_reason(&response))
    }
}

/// Build the `/api/chat` request body.
///
/// `schema` set means grammar-constrained mode: `tools` is withheld, because
/// advertising both asks the model to satisfy a grammar *and* choose a tool,
/// and Ollama would apply the grammar to whatever the tool layer produced.
fn chat_payload(request: &ToolCompletionRequest, schema: Option<&Value>) -> Value {
    let mut payload = json!({
        "model": request.model,
        "messages": chat_messages(&request.system, &request.turns),
        // The adapter is synchronous; a streamed body would arrive as
        // newline-delimited JSON that `post_json` cannot decode.
        "stream": false,
        "options": {
            "num_predict": request.max_tokens,
        },
    });

    if let Some(schema) = schema {
        payload["format"] = schema.clone();
    } else {
        let tools = tool_definitions(&request.tools);
        if !tools.is_empty() {
            payload["tools"] = json!(tools);
        }
    }
    payload
}

/// Convert Legion tool definitions into Ollama's `tools` array.
///
/// The shape matches OpenAI's function envelope, which Ollama adopted
/// deliberately; the divergence is in the *response*, not the advertisement.
fn tool_definitions(tools: &[ToolDefinition]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                }
            })
        })
        .collect()
}

/// Serialize the system prompt and conversation into Ollama chat messages.
///
/// Ollama correlates a tool result by **name**, not by id: its `tool` message
/// has a `tool_name` field and no `tool_call_id`. The name is therefore
/// recovered from the assistant turn that issued the call, which is why this
/// walks the whole conversation rather than mapping turn by turn. When no
/// prior call matches the id, `tool_name` is omitted rather than guessed —
/// naming the wrong tool would tell the model a call it never made had
/// returned.
fn chat_messages(system: &str, turns: &[ToolConversationTurn]) -> Vec<Value> {
    let mut messages: Vec<Value> = Vec::new();
    if !system.is_empty() {
        messages.push(json!({ "role": "system", "content": system }));
    }

    let mut tool_name_by_call_id: HashMap<&str, &str> = HashMap::new();

    for turn in turns {
        if turn.role == "assistant" {
            messages.push(assistant_message_json(turn, &mut tool_name_by_call_id));
            continue;
        }

        // Results first, then any prose: a `tool` message answers the
        // assistant message directly above it, and interposing user text
        // between them breaks that adjacency.
        for block in &turn.blocks {
            let ToolTurnBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } = block
            else {
                continue;
            };
            // Ollama has no `is_error` flag on tool messages, so the fact is
            // carried in the text — the same convention the OpenAI-compatible
            // path uses, so a model sees one spelling across both runtimes.
            let wire_content = if *is_error {
                format!("ERROR: {content}")
            } else {
                content.clone()
            };
            let mut message = json!({
                "role": "tool",
                "content": wire_content,
            });
            if let Some(name) = tool_name_by_call_id.get(tool_use_id.as_str()) {
                message["tool_name"] = json!(name);
            }
            messages.push(message);
        }

        let texts: Vec<&str> = turn
            .blocks
            .iter()
            .filter_map(|block| match block {
                ToolTurnBlock::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        if !texts.is_empty() {
            messages.push(json!({
                "role": "user",
                "content": texts.join("\n"),
            }));
        }
    }

    messages
}

/// Collapse one assistant turn into a single Ollama assistant message,
/// recording the name each issued call used so a later result can cite it.
fn assistant_message_json<'a>(
    turn: &'a ToolConversationTurn,
    tool_name_by_call_id: &mut HashMap<&'a str, &'a str>,
) -> Value {
    let mut texts: Vec<&str> = Vec::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    for block in &turn.blocks {
        match block {
            ToolTurnBlock::Text(text) => texts.push(text.as_str()),
            ToolTurnBlock::ToolUse { id, name, input } => {
                tool_name_by_call_id.insert(id.as_str(), name.as_str());
                tool_calls.push(json!({
                    "function": {
                        "name": name,
                        // Ollama takes arguments as an object; stringifying
                        // them the way OpenAI requires would arrive as a
                        // single string-valued argument.
                        "arguments": input,
                    }
                }));
            }
            // A tool result never belongs to an assistant turn.
            ToolTurnBlock::ToolResult { .. } => {}
            // Never replayed: echoing a broken call as a `tool_call` would
            // oblige a result for an id the model never really issued.
            ToolTurnBlock::MalformedToolCall { .. } => {}
        }
    }

    let mut message = json!({
        "role": "assistant",
        "content": texts.join("\n"),
    });
    if !tool_calls.is_empty() {
        message["tool_calls"] = json!(tool_calls);
    }
    message
}

/// Locate the assistant message, or report the response shape as a hard error.
///
/// A body whose *shape* is wrong is a transport-level failure, not something
/// the model can be asked to correct — the same line the OpenAI-compatible
/// path draws. Ollama also answers some failures with HTTP 200 and an `error`
/// string, which would otherwise surface here as a missing message and lose
/// the server's own explanation.
fn assistant_message<'a>(
    provider: &ProviderId,
    response: &'a Value,
) -> Result<&'a Value, ProviderError> {
    if let Some(error) = response
        .get("error")
        .and_then(Value::as_str)
        .filter(|error| !error.trim().is_empty())
    {
        return Err(ProviderError::RequestFailed {
            provider: provider.clone(),
            message: format!("Ollama chat request failed: {error}"),
        });
    }
    response
        .get("message")
        .ok_or_else(|| ProviderError::RequestFailed {
            provider: provider.clone(),
            message: "Ollama chat response missing message".to_string(),
        })
}

/// Assistant text, defaulting to empty — Ollama sends `"content": ""` beside a
/// tool call rather than omitting the field.
fn message_text(message: &Value) -> &str {
    message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// Why generation stopped, as Ollama reports it.
fn done_reason(response: &Value) -> &str {
    response
        .get("done_reason")
        .and_then(Value::as_str)
        .unwrap_or("stop")
}

/// Turn one assistant message into loop blocks and a stop reason.
fn parse_chat_message(
    provider: &ProviderId,
    request: &ToolCompletionRequest,
    message: &Value,
    done_reason: &str,
) -> Result<ToolCompletionResponse, ProviderError> {
    let content = message_text(message);
    let structured_calls = message.get("tool_calls").and_then(Value::as_array);
    let has_structured_calls = structured_calls.is_some_and(|calls| !calls.is_empty());

    // Recover calls the model wrote as prose. Skipped entirely when Ollama
    // already returned structured calls, so a call is never counted twice
    // (ADR-0049). Ollama's reasoning channel is `thinking`, and is consulted
    // only when the visible content is blank.
    let known_tools: Vec<String> = request.tools.iter().map(|tool| tool.name.clone()).collect();
    let recovered = if legion_ai::governance::small_model_governors_enabled() {
        extract_tool_calls(&ExtractionInput {
            content,
            reasoning_content: message.get("thinking").and_then(Value::as_str),
            has_existing_tool_calls: has_structured_calls,
            known_tools: &known_tools,
            legion_tools: request.legion_tools,
        })
    } else {
        // Measurement arm: a call written as prose is prose.
        Default::default()
    };

    let mut blocks: Vec<ToolTurnBlock> = Vec::new();

    let text = if recovered.calls.is_empty() {
        content.to_string()
    } else {
        recovered.residual_content.clone()
    };
    if !text.is_empty() {
        blocks.push(ToolTurnBlock::Text(text));
    }

    for (index, call) in recovered.calls.iter().enumerate() {
        // Recovered calls carry no provider id at all; the same synthesized
        // scheme the OpenAI-compatible path uses keeps ids comparable in an
        // audit trail across providers.
        let id = format!("recovered-{index}");
        match &call.arguments_unparsed {
            Some(raw) => blocks.push(ToolTurnBlock::MalformedToolCall {
                id,
                name: call.name.clone(),
                raw_arguments: bounded_raw_arguments(raw),
                diagnostic:
                    "arguments are not valid JSON. Reply with the same tool call and a valid JSON arguments object."
                        .to_string(),
            }),
            None => blocks.push(ToolTurnBlock::ToolUse {
                id,
                name: call.name.clone(),
                input: call.arguments.clone(),
            }),
        }
    }

    if let Some(calls) = structured_calls {
        for (index, call) in calls.iter().enumerate() {
            blocks.push(structured_call_block(provider, index, call)?);
        }
    }

    // Ollama's `done_reason` never takes OpenAI's `"tool_calls"` value, so a
    // tool call has to be inferred from the payload. Structured calls are
    // decided first for the same reason OpenAI's `finish_reason` is: a run
    // that produced a call has not ended its turn, whatever else the server
    // said. `length` then wins over a *recovered* call, because a truncated
    // reply is the more useful diagnosis.
    let stop_reason = if has_structured_calls {
        ToolCompletionStopReason::ToolUse
    } else if done_reason == "length" {
        ToolCompletionStopReason::MaxTokens
    } else if blocks.iter().any(|block| {
        matches!(
            block,
            ToolTurnBlock::ToolUse { .. } | ToolTurnBlock::MalformedToolCall { .. }
        )
    }) {
        // A model that writes its call as prose reports `stop`, because as far
        // as Ollama is concerned it only produced text. Reporting EndTurn would
        // make the agent loop finish the run without dispatching the call we
        // just recovered, leaving recovery inert exactly where it is needed.
        ToolCompletionStopReason::ToolUse
    } else {
        ToolCompletionStopReason::EndTurn
    };

    Ok(ToolCompletionResponse {
        provider: provider.clone(),
        model: request.model.clone(),
        blocks,
        stop_reason,
    })
}

/// Convert one entry of Ollama's `tool_calls` array into a block.
fn structured_call_block(
    provider: &ProviderId,
    index: usize,
    call: &Value,
) -> Result<ToolTurnBlock, ProviderError> {
    let function = call
        .get("function")
        .ok_or_else(|| ProviderError::RequestFailed {
            provider: provider.clone(),
            message: "Ollama tool_call missing function object".to_string(),
        })?;
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| ProviderError::RequestFailed {
            provider: provider.clone(),
            message: "Ollama tool_call missing function name".to_string(),
        })?
        .to_string();

    // Ollama assigns no id. A proxy in front of it may, so an id that is
    // actually present is preferred over a synthesized one; correlating on the
    // server's own id is always safer than on our count of the array.
    let id = call
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{SYNTHESIZED_CALL_ID_PREFIX}-{index}"));

    let arguments = function.get("arguments");
    match arguments {
        // Ollama parses arguments server-side, so the documented shape is an
        // object and this is the path that runs in practice.
        Some(Value::Object(object)) => Ok(ToolTurnBlock::ToolUse {
            id,
            name,
            input: Value::Object(object.clone()),
        }),
        // A tool taking no arguments legitimately omits the field. That is not
        // a parse failure, and conflating the two would make an argument-less
        // call undispatchable.
        None | Some(Value::Null) => Ok(ToolTurnBlock::ToolUse {
            id,
            name,
            input: json!({}),
        }),
        // A string is how an OpenAI-shaped proxy in front of Ollama sends
        // arguments, so it is parsed rather than refused outright.
        Some(Value::String(raw)) => match serde_json::from_str::<Value>(raw) {
            Ok(Value::Object(object)) => Ok(ToolTurnBlock::ToolUse {
                id,
                name,
                input: Value::Object(object),
            }),
            // Valid JSON that is not an object is as undispatchable as invalid
            // JSON: no tool takes a bare scalar or array as its arguments.
            _ => malformed_arguments_block(provider, id, name, raw.clone()),
        },
        // A scalar or array where the arguments object belongs. Refused rather
        // than coerced — inventing a field name to hang it on would fabricate
        // a call the model did not make.
        Some(other) => malformed_arguments_block(provider, id, name, other.to_string()),
    }
}

/// Refuse arguments that never parsed, in whichever way the current
/// measurement arm requires.
///
/// With the governors on this is a typed, non-dispatchable block the agent
/// loop feeds back as a diagnostic. With `LEGION_AI_GOVERNORS=off` it is a
/// hard provider error, which is what this was before the SmallCode port —
/// recovering it in the baseline arm would leave a governed reliability
/// mechanism running inside the supposedly raw measurement.
fn malformed_arguments_block(
    provider: &ProviderId,
    id: String,
    name: String,
    raw: String,
) -> Result<ToolTurnBlock, ProviderError> {
    if !legion_ai::governance::small_model_governors_enabled() {
        return Err(ProviderError::RequestFailed {
            provider: provider.clone(),
            message: format!("Ollama tool_call arguments are not a JSON object. Raw: {raw:?}"),
        });
    }
    Ok(ToolTurnBlock::MalformedToolCall {
        id,
        name,
        raw_arguments: bounded_raw_arguments(&raw),
        diagnostic:
            "arguments are not a valid JSON object. Reply with the same tool call and a valid JSON arguments object."
                .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: format!("the {name} tool"),
            input_schema: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"],
            }),
        }
    }

    fn request_with(turns: Vec<ToolConversationTurn>) -> ToolCompletionRequest {
        ToolCompletionRequest {
            provider: "ollama-test".to_string(),
            model: "llama3.2".to_string(),
            system: "You are Legion.".to_string(),
            turns,
            tools: vec![tool("read")],
            max_tokens: 512,
            legion_tools: false,
        }
    }

    fn user_turn(text: &str) -> ToolConversationTurn {
        ToolConversationTurn {
            role: "user".to_string(),
            blocks: vec![ToolTurnBlock::Text(text.to_string())],
        }
    }

    #[test]
    fn payload_uses_ollamas_native_chat_shape() {
        let payload = chat_payload(&request_with(vec![user_turn("read a.rs")]), None);

        assert_eq!(payload["model"], "llama3.2");
        assert_eq!(
            payload["stream"], false,
            "a streamed body is newline-delimited JSON the transport cannot decode"
        );
        assert_eq!(
            payload["options"]["num_predict"], 512,
            "Ollama spells the generation limit num_predict, not max_tokens"
        );
        assert!(
            payload.get("max_tokens").is_none(),
            "max_tokens is an OpenAI field and Ollama ignores it"
        );
        assert_eq!(payload["messages"][0]["role"], "system");
        assert_eq!(payload["messages"][0]["content"], "You are Legion.");
        assert_eq!(payload["messages"][1]["role"], "user");
        assert_eq!(payload["messages"][1]["content"], "read a.rs");
        assert_eq!(payload["tools"][0]["type"], "function");
        assert_eq!(payload["tools"][0]["function"]["name"], "read");
        assert!(payload["tools"][0]["function"]["parameters"].is_object());
    }

    #[test]
    fn schema_mode_sends_a_format_grammar_and_withholds_tools() {
        let schema = json!({ "type": "object" });
        let payload = chat_payload(&request_with(vec![user_turn("go")]), Some(&schema));

        assert_eq!(
            payload["format"], schema,
            "Ollama takes the grammar inline as `format`"
        );
        assert!(
            payload.get("tools").is_none(),
            "advertising tools under a grammar asks for two incompatible things at once"
        );
    }

    #[test]
    fn tool_results_are_correlated_by_name_because_ollama_has_no_call_ids() {
        let turns = vec![
            user_turn("read a.rs"),
            ToolConversationTurn {
                role: "assistant".to_string(),
                blocks: vec![ToolTurnBlock::ToolUse {
                    id: "ollama-0".to_string(),
                    name: "read".to_string(),
                    input: json!({ "path": "a.rs" }),
                }],
            },
            ToolConversationTurn {
                role: "user".to_string(),
                blocks: vec![ToolTurnBlock::ToolResult {
                    tool_use_id: "ollama-0".to_string(),
                    content: "fn main() {}".to_string(),
                    is_error: false,
                }],
            },
        ];
        let messages = chat_messages("", &turns);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(
            messages[1]["tool_calls"][0]["function"]["arguments"]["path"], "a.rs",
            "Ollama takes arguments as an object, not as a JSON string"
        );
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["content"], "fn main() {}");
        assert_eq!(
            messages[2]["tool_name"], "read",
            "the result must name the tool it answers — Ollama has no tool_call_id"
        );
    }

    #[test]
    fn an_error_result_carries_the_fact_in_its_text() {
        let turns = vec![ToolConversationTurn {
            role: "user".to_string(),
            blocks: vec![ToolTurnBlock::ToolResult {
                tool_use_id: "unknown".to_string(),
                content: "file not found".to_string(),
                is_error: true,
            }],
        }];
        let messages = chat_messages("", &turns);

        assert_eq!(messages[0]["content"], "ERROR: file not found");
        assert!(
            messages[0].get("tool_name").is_none(),
            "an id with no matching call must not be given a guessed tool name"
        );
    }

    #[test]
    fn a_malformed_call_is_never_replayed_to_the_model() {
        let turns = vec![ToolConversationTurn {
            role: "assistant".to_string(),
            blocks: vec![
                ToolTurnBlock::Text("trying".to_string()),
                ToolTurnBlock::MalformedToolCall {
                    id: "ollama-0".to_string(),
                    name: "read".to_string(),
                    raw_arguments: "{bad".to_string(),
                    diagnostic: "nope".to_string(),
                },
            ],
        }];
        let messages = chat_messages("", &turns);

        assert_eq!(messages[0]["content"], "trying");
        assert!(
            messages[0].get("tool_calls").is_none(),
            "replaying a broken call would oblige a result for a call the model never made"
        );
    }

    #[test]
    fn object_arguments_parse_into_a_dispatchable_call() {
        let message = json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [
                { "function": { "name": "read", "arguments": { "path": "a.rs" } } }
            ]
        });
        let response = parse_chat_message(
            &"ollama-test".to_string(),
            &request_with(vec![user_turn("go")]),
            &message,
            "stop",
        )
        .expect("a well-formed native tool call parses");

        assert_eq!(response.stop_reason, ToolCompletionStopReason::ToolUse);
        assert_eq!(response.blocks.len(), 1);
        let ToolTurnBlock::ToolUse { id, name, input } = &response.blocks[0] else {
            panic!("expected ToolUse, got {:?}", response.blocks[0]);
        };
        assert_eq!(
            id, "ollama-0",
            "Ollama assigns no id, so a positional one is synthesized"
        );
        assert_eq!(name, "read");
        assert_eq!(input["path"], "a.rs");
    }

    #[test]
    fn a_server_supplied_id_is_preferred_over_a_synthesized_one() {
        let message = json!({
            "tool_calls": [
                { "id": "proxy-7", "function": { "name": "read", "arguments": { "path": "a.rs" } } }
            ]
        });
        let response = parse_chat_message(
            &"ollama-test".to_string(),
            &request_with(vec![]),
            &message,
            "stop",
        )
        .expect("parses");
        assert!(
            matches!(&response.blocks[0], ToolTurnBlock::ToolUse { id, .. } if id == "proxy-7"),
            "an id the server actually issued must win over our count of the array"
        );
    }

    #[test]
    fn absent_arguments_stay_dispatchable() {
        let message = json!({
            "tool_calls": [{ "function": { "name": "read" } }]
        });
        let response = parse_chat_message(
            &"ollama-test".to_string(),
            &request_with(vec![]),
            &message,
            "stop",
        )
        .expect("parses");
        assert!(
            matches!(
                &response.blocks[0],
                ToolTurnBlock::ToolUse { input, .. } if input == &json!({})
            ),
            "a tool taking no arguments is not a parse failure"
        );
    }

    #[test]
    fn string_arguments_from_an_openai_shaped_proxy_are_parsed() {
        let message = json!({
            "tool_calls": [
                { "function": { "name": "read", "arguments": "{\"path\": \"a.rs\"}" } }
            ]
        });
        let response = parse_chat_message(
            &"ollama-test".to_string(),
            &request_with(vec![]),
            &message,
            "stop",
        )
        .expect("parses");
        assert!(
            matches!(
                &response.blocks[0],
                ToolTurnBlock::ToolUse { input, .. } if input["path"] == "a.rs"
            ),
            "a stringified argument object is still an argument object"
        );
    }

    #[test]
    fn a_missing_function_name_is_a_shape_failure_not_a_model_error() {
        // Ollama constructs this field itself, so its absence means the
        // response shape is wrong — not that the model got something wrong it
        // could be asked to correct.
        let message = json!({ "tool_calls": [{ "function": { "arguments": {} } }] });
        let error = parse_chat_message(
            &"ollama-test".to_string(),
            &request_with(vec![]),
            &message,
            "stop",
        )
        .expect_err("a nameless call cannot be dispatched or explained");
        assert!(
            matches!(error, ProviderError::RequestFailed { .. }),
            "expected RequestFailed, got {error:?}"
        );
    }

    #[test]
    fn plain_prose_ends_the_turn() {
        let message = json!({ "role": "assistant", "content": "All done." });
        let response = parse_chat_message(
            &"ollama-test".to_string(),
            &request_with(vec![user_turn("go")]),
            &message,
            "stop",
        )
        .expect("parses");

        assert_eq!(response.stop_reason, ToolCompletionStopReason::EndTurn);
        assert_eq!(
            response.blocks,
            vec![ToolTurnBlock::Text("All done.".into())]
        );
    }

    #[test]
    fn a_truncated_reply_reports_max_tokens() {
        let message = json!({ "content": "I will start by" });
        let response = parse_chat_message(
            &"ollama-test".to_string(),
            &request_with(vec![]),
            &message,
            "length",
        )
        .expect("parses");
        assert_eq!(response.stop_reason, ToolCompletionStopReason::MaxTokens);
    }

    #[test]
    fn an_http_200_carrying_an_error_string_is_still_a_failure() {
        let error = assistant_message(
            &"ollama-test".to_string(),
            &json!({ "error": "model 'nope' not found" }),
        )
        .expect_err("Ollama's own explanation must not be lost as a missing message");
        assert!(
            format!("{error}").contains("model 'nope' not found"),
            "the server's message must survive: {error}"
        );
    }
}
