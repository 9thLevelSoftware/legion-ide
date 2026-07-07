//! Tool-calling DTOs, trait, and scripted test provider.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
    ModelProvider, ProviderCapabilities, ProviderError, ProviderId,
};

/// A block within a tool-calling conversation turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolTurnBlock {
    /// Plain text output from the model.
    Text(String),
    /// Model requests a tool invocation.
    ToolUse {
        /// Unique ID for this tool use (assigned by the model).
        id: String,
        /// Tool name the model wants to call.
        name: String,
        /// JSON arguments for the tool.
        input: Value,
    },
    /// Result of a tool execution, sent back to the model.
    ToolResult {
        /// The tool_use id this result corresponds to.
        tool_use_id: String,
        /// Text content of the result.
        content: String,
        /// Whether the tool call errored.
        is_error: bool,
    },
}

/// A single turn in a tool-calling conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolConversationTurn {
    /// "user" or "assistant"
    pub role: String,
    /// Blocks within this turn.
    pub blocks: Vec<ToolTurnBlock>,
}

/// A tool definition passed to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: Value,
}

/// Request for a tool-calling completion.
#[derive(Debug, Clone)]
pub struct ToolCompletionRequest {
    /// Provider to target.
    pub provider: String,
    /// Model identifier.
    pub model: String,
    /// System prompt.
    pub system: String,
    /// Conversation history (user and assistant turns).
    pub turns: Vec<ToolConversationTurn>,
    /// Available tools.
    pub tools: Vec<ToolDefinition>,
    /// Max tokens to generate.
    pub max_tokens: u32,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCompletionStopReason {
    /// Model wants to use a tool (response contains ToolUse blocks).
    ToolUse,
    /// Model finished its turn naturally.
    EndTurn,
    /// Hit the max_tokens limit.
    MaxTokens,
}

/// Response from a tool-calling completion.
#[derive(Debug, Clone)]
pub struct ToolCompletionResponse {
    /// Provider that produced the response.
    pub provider: String,
    /// Model used.
    pub model: String,
    /// Response blocks (may contain Text and/or ToolUse).
    pub blocks: Vec<ToolTurnBlock>,
    /// Why the model stopped.
    pub stop_reason: ToolCompletionStopReason,
}

/// Subtrait of ModelProvider for providers that support tool-calling conversations.
pub trait ToolCallingProvider: ModelProvider {
    /// Send a tool-calling completion request.
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError>;
}

// ---- D2: ScriptedToolCallingProvider ----

/// A scripted response for one model turn.
pub struct ScriptedTurn {
    /// Blocks to return for this turn.
    pub blocks: Vec<ToolTurnBlock>,
    /// Stop reason to return.
    pub stop_reason: ToolCompletionStopReason,
    /// Optional: before returning this turn, assert that the conversation
    /// contains a tool result whose content includes this substring.
    pub expect_prior_result_contains: Option<String>,
}

/// Builder for constructing scripted tool-calling providers.
pub struct ScriptedToolCallingProviderBuilder {
    turns: Vec<ScriptedTurn>,
    pending_expect: Option<String>,
}

impl Default for ScriptedToolCallingProviderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptedToolCallingProviderBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            turns: Vec::new(),
            pending_expect: None,
        }
    }

    /// Add a turn where the model requests tool use.
    pub fn tool_use(mut self, id: &str, name: &str, input: Value) -> Self {
        let expect = self.pending_expect.take();
        self.turns.push(ScriptedTurn {
            blocks: vec![ToolTurnBlock::ToolUse {
                id: id.to_string(),
                name: name.to_string(),
                input,
            }],
            stop_reason: ToolCompletionStopReason::ToolUse,
            expect_prior_result_contains: expect,
        });
        self
    }

    /// Add a turn where the model emits text and ends.
    pub fn end_turn(mut self, text: &str) -> Self {
        let expect = self.pending_expect.take();
        self.turns.push(ScriptedTurn {
            blocks: vec![ToolTurnBlock::Text(text.to_string())],
            stop_reason: ToolCompletionStopReason::EndTurn,
            expect_prior_result_contains: expect,
        });
        self
    }

    /// Add a turn with custom blocks and stop reason.
    pub fn turn(
        mut self,
        blocks: Vec<ToolTurnBlock>,
        stop_reason: ToolCompletionStopReason,
    ) -> Self {
        let expect = self.pending_expect.take();
        self.turns.push(ScriptedTurn {
            blocks,
            stop_reason,
            expect_prior_result_contains: expect,
        });
        self
    }

    /// Add a determinism guard: before returning the NEXT turn, assert that
    /// a prior tool result contains this substring.
    pub fn expect_prior_result_contains(mut self, needle: &str) -> Self {
        self.pending_expect = Some(needle.to_string());
        self
    }

    /// Build the provider.
    ///
    /// # Panics
    ///
    /// Panics if `expect_prior_result_contains` was called with no following turn.
    pub fn build(self, provider_id: &str) -> ScriptedToolCallingProvider {
        assert!(
            self.pending_expect.is_none(),
            "expect_prior_result_contains called with no following turn"
        );
        ScriptedToolCallingProvider {
            id: provider_id.to_string(),
            turns: self.turns,
            cursor: std::cell::Cell::new(0),
        }
    }
}

/// Deterministic multi-turn tool-calling provider for tests.
///
/// Each call to `complete_with_tools` returns the next scripted turn in order.
/// Supports optional determinism guards that assert a prior `ToolResult` block
/// contains an expected substring before returning the guarded turn.
pub struct ScriptedToolCallingProvider {
    id: ProviderId,
    turns: Vec<ScriptedTurn>,
    cursor: std::cell::Cell<usize>,
}

impl ModelProvider for ScriptedToolCallingProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            completion: false,
            embedding: false,
            batch: false,
            inline_prediction: false,
            tool_use: true,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        Err(ProviderError::unsupported(request.provider, "complete"))
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::unsupported(request.provider, "embed"))
    }
}

impl ToolCallingProvider for ScriptedToolCallingProvider {
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        let cursor = self.cursor.get();
        if cursor >= self.turns.len() {
            return Err(ProviderError::RequestFailed {
                provider: self.id.clone(),
                message: "scripted provider exhausted — no more turns available".to_string(),
            });
        }
        let turn = &self.turns[cursor];

        // Enforce determinism guard if present.
        if let Some(needle) = &turn.expect_prior_result_contains {
            let found = request.turns.iter().any(|t| {
                t.blocks.iter().any(|b| match b {
                    ToolTurnBlock::ToolResult { content, .. } => content.contains(needle.as_str()),
                    _ => false,
                })
            });
            if !found {
                return Err(ProviderError::RequestFailed {
                    provider: self.id.clone(),
                    message: format!(
                        "scripted provider guard failed: expected a prior ToolResult containing {:?}",
                        needle
                    ),
                });
            }
        }

        self.cursor.set(cursor + 1);
        Ok(ToolCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            blocks: turn.blocks.clone(),
            stop_reason: turn.stop_reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_request(turns: Vec<ToolConversationTurn>) -> ToolCompletionRequest {
        ToolCompletionRequest {
            provider: "test".to_string(),
            model: "test-model".to_string(),
            system: String::new(),
            turns,
            tools: vec![],
            max_tokens: 1024,
        }
    }

    #[test]
    fn scripted_provider_multi_turn_script() {
        let provider = ScriptedToolCallingProviderBuilder::new()
            .tool_use("t1", "Read", json!({"path": "foo.rs"}))
            .tool_use("t2", "Grep", json!({"pattern": "fn"}))
            .end_turn("Done")
            .build("test");

        // Turn 1: model requests Read
        let resp1 = provider
            .complete_with_tools(make_request(vec![]))
            .expect("first call succeeds");
        assert_eq!(resp1.stop_reason, ToolCompletionStopReason::ToolUse);
        assert_eq!(resp1.blocks.len(), 1);
        let ToolTurnBlock::ToolUse { id, name, input } = &resp1.blocks[0] else {
            panic!("expected ToolUse block");
        };
        assert_eq!(id, "t1");
        assert_eq!(name, "Read");
        assert_eq!(input, &json!({"path": "foo.rs"}));

        // Turn 2: caller sends ToolResult for t1; model requests Grep
        let resp2 = provider
            .complete_with_tools(make_request(vec![ToolConversationTurn {
                role: "user".to_string(),
                blocks: vec![ToolTurnBlock::ToolResult {
                    tool_use_id: "t1".to_string(),
                    content: "fn main() {}".to_string(),
                    is_error: false,
                }],
            }]))
            .expect("second call succeeds");
        assert_eq!(resp2.stop_reason, ToolCompletionStopReason::ToolUse);
        let ToolTurnBlock::ToolUse {
            id: id2,
            name: name2,
            ..
        } = &resp2.blocks[0]
        else {
            panic!("expected ToolUse block");
        };
        assert_eq!(id2, "t2");
        assert_eq!(name2, "Grep");

        // Turn 3: caller sends ToolResult for t2; model ends turn
        let resp3 = provider
            .complete_with_tools(make_request(vec![ToolConversationTurn {
                role: "user".to_string(),
                blocks: vec![ToolTurnBlock::ToolResult {
                    tool_use_id: "t2".to_string(),
                    content: "fn foo() {}".to_string(),
                    is_error: false,
                }],
            }]))
            .expect("third call succeeds");
        assert_eq!(resp3.stop_reason, ToolCompletionStopReason::EndTurn);
        let ToolTurnBlock::Text(text) = &resp3.blocks[0] else {
            panic!("expected Text block");
        };
        assert_eq!(text, "Done");
    }

    #[test]
    fn scripted_provider_determinism_guard() {
        let provider = ScriptedToolCallingProviderBuilder::new()
            .tool_use("t1", "Read", json!({"path": "foo.rs"}))
            .expect_prior_result_contains("file contents")
            .end_turn("Done")
            .build("test");

        // First call: no guard on turn 0, should succeed.
        let resp1 = provider
            .complete_with_tools(make_request(vec![]))
            .expect("first call succeeds");
        assert_eq!(resp1.stop_reason, ToolCompletionStopReason::ToolUse);

        // Second call: guard checks for "file contents" in prior ToolResult.
        // Provide a result that does NOT contain the expected substring — must fail.
        let err = provider
            .complete_with_tools(make_request(vec![ToolConversationTurn {
                role: "user".to_string(),
                blocks: vec![ToolTurnBlock::ToolResult {
                    tool_use_id: "t1".to_string(),
                    content: "wrong content — no match here".to_string(),
                    is_error: false,
                }],
            }]))
            .expect_err("guard failure expected");
        assert!(
            matches!(err, ProviderError::RequestFailed { .. }),
            "expected RequestFailed, got {err:?}"
        );

        // Retry second call: this time the ToolResult DOES contain the needle.
        let resp2 = provider
            .complete_with_tools(make_request(vec![ToolConversationTurn {
                role: "user".to_string(),
                blocks: vec![ToolTurnBlock::ToolResult {
                    tool_use_id: "t1".to_string(),
                    content: "file contents of foo.rs".to_string(),
                    is_error: false,
                }],
            }]))
            .expect("second call succeeds with matching content");
        assert_eq!(resp2.stop_reason, ToolCompletionStopReason::EndTurn);
    }

    #[test]
    fn scripted_provider_exhaustion() {
        let provider = ScriptedToolCallingProviderBuilder::new()
            .end_turn("Only response")
            .build("test");

        // First call: succeeds.
        provider
            .complete_with_tools(make_request(vec![]))
            .expect("first call succeeds");

        // Second call: script exhausted — must return RequestFailed.
        let err = provider
            .complete_with_tools(make_request(vec![]))
            .expect_err("exhaustion expected");
        let msg = format!("{err}");
        assert!(
            msg.contains("exhausted"),
            "error message should contain 'exhausted', got: {msg}"
        );
    }

    #[test]
    #[should_panic(expected = "expect_prior_result_contains called with no following turn")]
    fn scripted_provider_build_panics_on_trailing_expect() {
        ScriptedToolCallingProviderBuilder::new()
            .end_turn("done")
            .expect_prior_result_contains("dangling guard")
            .build("test");
    }
}
