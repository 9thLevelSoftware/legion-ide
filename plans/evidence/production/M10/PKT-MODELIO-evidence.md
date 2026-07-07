# PKT-MODELIO Evidence: Tool-Calling Model I/O

**Branch:** `m10/tool-model-io`
**Date:** 2026-07-06
**Status:** DONE

## Deliverables Completed

### D1 — ToolCallingProvider trait + DTOs

- **File:** `crates/legion-ai/src/tool_calls.rs` (new, 431 lines)
- **Declared in:** `crates/legion-ai/src/lib.rs` (`pub mod tool_calls;`)
- **DTOs added:** `ToolTurnBlock`, `ToolConversationTurn`, `ToolDefinition`, `ToolCompletionRequest`, `ToolCompletionStopReason`, `ToolCompletionResponse`
- **Trait added:** `ToolCallingProvider: ModelProvider` with `complete_with_tools`
- **ProviderCapabilities sweep:** Added `pub tool_use: bool` to struct and `tool_use: false` to all 10 literal sites (Default impl, DeterministicInlinePredictionProvider, CompletionUnavailableProvider, Gp2LocalCompletionProvider, DeterministicLocalProvider, OllamaProvider, OpenAiResponsesProvider, OpenAiCompatibleProvider, UnavailableInlineProvider) plus `tool_use: true` for AnthropicMessagesClient

### D2 — ScriptedToolCallingProvider

- **File:** `crates/legion-ai/src/tool_calls.rs` (same file as D1)
- **Structs added:** `ScriptedTurn`, `ScriptedToolCallingProviderBuilder`, `ScriptedToolCallingProvider`
- **Builder DSL:** `.tool_use()`, `.end_turn()`, `.turn()`, `.expect_prior_result_contains()`, `.build()`
- **Behavior:** Cursor-based (interior mutable `Cell<usize>`); exhaustion returns `RequestFailed`; determinism guards reject mismatched ToolResult content
- **Tests (3):** multi-turn, determinism guard, exhaustion — all pass

### D3 — Anthropic wire-format implementation

- **File:** `crates/legion-ai-providers/src/lib.rs`
- **Added:** `extract_assistant_blocks` (parses `tool_use` and `text` blocks, maps `stop_reason`)
- **Added:** `serialize_tool_turn` (free fn; serializes `ToolConversationTurn` to Anthropic JSON)
- **Added:** `impl ToolCallingProvider for AnthropicMessagesClient<T>`
- **Import added:** `use legion_ai::tool_calls::{...}` (7 types)
- **Tests (4):** parsing test, serialization round-trip, end-to-end with FixedAnthropicTransport, live smoke (gated on `ANTHROPIC_API_KEY` — prints skip message and passes when key is absent)

## Verification Results

```
cargo fmt --check         → clean (0 diffs)
cargo clippy --all-targets -- -D warnings → clean (0 warnings)
cargo test --all-targets -j 4 → 30 suites, 0 failures
cargo test -p legion-app --test manual_zero_egress → ok (1 passed)
```

## Commits

| SHA | Message |
|-----|---------|
| `6b3fa5f` | feat: add ToolCallingProvider trait and tool-calling DTOs |
| `354f88b` | feat: implement Anthropic tool-calling wire format (extract_assistant_blocks + turn serialization) |

## Notes

- D1 and D2 were committed in the same commit since both live in `crates/legion-ai/src/tool_calls.rs`.
- The `tool_use` string now appears in crate code for the first time (D3: `extract_assistant_blocks`).
- `AnthropicMessagesClient` is the only provider with `tool_use: true` in capabilities.
- No new crate dependencies were added; `cargo deny check` not required.
