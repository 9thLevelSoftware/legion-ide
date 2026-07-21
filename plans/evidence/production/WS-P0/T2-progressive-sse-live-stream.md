# T2 follow-on — Progressive Anthropic SSE + live stream sink

**Date:** 2026-07-21

## Changes

| Item | Detail |
| --- | --- |
| **Progressive SSE transport** | `ReqwestProviderHttpTransport::stream_sse_text` reads the HTTP body in chunks and parses complete SSE events as they arrive |
| **`stream_text_deltas_with_callback`** | Anthropic client invokes a delta callback mid-body |
| **`complete_product_chat`** | Accepts optional `on_delta` and uses the progressive callback path for Anthropic |
| **`LiveProductAiStreamSink`** | Shared sink with `in_flight` + chunk accumulation |
| **Desktop frame** | `poll_product_ai_stream` + repaint while stream in flight; Agent Comm shows `in-flight` badge |

## Honest limits

- Assist proposal / Delegate chat still **block the calling thread** until the HTTP stream completes (UI can repaint only if another thread drives frames, or after completion).
- Fully non-blocking worker-thread generation (UI never waits) is a follow-on.

## Verification

```text
cargo check -p legion-ai-providers --lib
cargo check -p legion-app --lib
cargo check -p legion-desktop --lib
cargo test -p legion-app --test assist_inline_prediction_workflow --test delegated_task_integration
cargo test -p legion-desktop --test assistant_rail
```
