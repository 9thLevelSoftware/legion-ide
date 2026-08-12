# T2 follow-on — Product AI streaming (Anthropic SSE → rail projection)

**Date:** 2026-07-21

## Changes

| Item | Detail |
| --- | --- |
| **`ProductChatCompletion`** | Carries `stream_chunks` + `streamed` flag |
| **Anthropic path** | Prefers `stream_text_deltas_with_extras` (Messages SSE); falls back to non-stream `complete` |
| **Ollama path** | Single-chunk completion (no product SSE adapter yet) |
| **`ProductAiStreamProjection`** | Retained on `AppComposition` after Assist proposal / Delegate chat |
| **Desktop** | Agent Comm Stream + Model Picker show last stream metadata; body via `render_streaming_assistant_rows` |

## Honest limits

- The HTTP call is still **blocking** on the UI/worker thread; tokens are not painted mid-flight — chunks are projected after the stream finishes.
- Real-time frame-by-frame streaming requires an async/worker channel (later slice).

## Verification

```text
cargo check -p legion-app --lib
cargo check -p legion-desktop --lib
cargo test -p legion-app --test assist_inline_prediction_workflow --test delegated_task_integration --test control_trust_surfaces
cargo test -p legion-desktop --test assistant_rail
cargo run -p xtask -- docs-hygiene
```
