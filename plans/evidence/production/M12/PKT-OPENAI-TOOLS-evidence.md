# PKT-OPENAI-TOOLS Evidence

**Campaign:** M12  
**Packet:** PKT-OPENAI-TOOLS — OpenAI tool-calling provider  
**Date:** 2026-07-07  
**Status:** Complete

## What was implemented

`ToolCallingProvider` is now implemented for `OpenAiCompatibleProvider<T>` in
`crates/legion-ai-providers/src/lib.rs`, covering `api.openai.com` and every
OpenAI-compatible server (llama.cpp loopback, etc.) in a single impl.

## Wire format mapping

| Legion DTO | OpenAI wire format |
|---|---|
| `Turn::User(Text)` | `{"role":"user","content":text}` |
| `Turn::Assistant(Text+ToolUse)` | ONE `{"role":"assistant","content":text,"tool_calls":[{id,type:"function",function:{name,arguments:JSON-string}}]}` |
| `Turn::User(ToolResult)` | `{"role":"tool","tool_call_id":id,"content":content}` per result, placed immediately after the assistant message (wire-order enforced) |
| `is_error:true` | `"ERROR: "` prefix on content (no native field) |
| `finish_reason:"tool_calls"` | `ToolCompletionStopReason::ToolUse` |
| `finish_reason:"stop"` | `ToolCompletionStopReason::EndTurn` |
| `finish_reason:"length"` | `ToolCompletionStopReason::MaxTokens` |
| malformed `arguments` string | hard `ProviderError::RequestFailed` |

System prompt → `{"role":"system","content":system}` prepended as first message.  
Tool definitions → `[{type:"function",function:{name,description,parameters:input_schema}}]`.

## Capabilities change

`OpenAiCompatibleProvider::capabilities().tool_use` changed from `false` to `true`.
`OpenAiResponsesProvider` remains `tool_use: false` (text-only, by design).

## New code locations

- **Impl:** `crates/legion-ai-providers/src/lib.rs` — `impl<T: ProviderHttpTransport> ToolCallingProvider for OpenAiCompatibleProvider<T>`
- **Helper:** `serialize_openai_tool_turn()` — converts a `ToolConversationTurn` to 0..N OpenAI wire messages
- **Capability:** `capabilities().tool_use` field in `impl<T> ModelProvider for OpenAiCompatibleProvider<T>`

## Test coverage

### Fake-transport unit tests (11 tests, `legion-ai-providers --lib`)

| Test | Verifies |
|---|---|
| `openai_tool_calling_serializes_request_correctly` | System + user messages, tools in function format, model/max_tokens |
| `openai_tool_calling_parses_tool_use_response` | `tool_calls` → `ToolTurnBlock::ToolUse`, `finish_reason:tool_calls` → ToolUse |
| `openai_tool_call_id_round_trips_as_tool_message` | Tool result `tool_call_id` matches assistant `tool_calls[].id` |
| `openai_tool_calling_multi_tool_call_parsed` | 2 tool_calls → 2 `ToolUse` blocks in order |
| `openai_tool_result_messages_appear_immediately_after_assistant_tool_calls` | Wire-order: tool messages immediately follow assistant message (400 trap) |
| `openai_finish_reason_stop_maps_to_end_turn` | `"stop"` → `EndTurn` |
| `openai_finish_reason_length_maps_to_max_tokens` | `"length"` → `MaxTokens` |
| `openai_finish_reason_tool_calls_maps_to_tool_use` | `"tool_calls"` → `ToolUse` |
| `openai_malformed_arguments_returns_provider_error` | Invalid JSON arguments → hard `ProviderError::RequestFailed` |
| `openai_error_tool_result_gets_error_prefix` | `is_error:true` → `"ERROR: "` prefix |
| `openai_compatible_capabilities_has_tool_use` | `capabilities().tool_use == true` |

### Live smoke test (1 test, optional)

`openai_tool_calling_live_smoke` — gated on `OPENAI_API_KEY` env var, visible SKIP
message when absent (not `#[ignore]`). Default model `gpt-4o-mini`, overridable via
`LEGION_SMOKE_OPENAI_MODEL`. Two round-trips: tool_use call → EndTurn after result.
**Not a standing gate.**

### Cross-check test (1 test, `legion-agent --test openai_tool_loop_cross_check`)

`openai_provider_compatible_with_agent_loop_read_then_end` — runs
`run_delegated_task_loop` with `OpenAiCompatibleProvider<SequentialOpenAiTransport>`.
Scripted: read tool_call → file read → EndTurn. Asserts `Completed` with correct
final message and audit-pairing invariant.

## No new dependencies

`legion-ai-providers` gains no new `[dependencies]`.  
`legion-agent` gains `legion-ai-providers` as a `[dev-dependencies]` entry (pre-existing
workspace member, no new crate introduced).

## Standing gate status

All 18 standing gates remain green. `manual_zero_egress` passes. The live smoke is
intentionally excluded from the standing gate set.
