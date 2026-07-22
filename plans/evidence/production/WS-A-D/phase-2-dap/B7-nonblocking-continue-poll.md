# Phase 2 B7 — Non-blocking continue + poll

**Date:** 2026-07-22  
**Status:** Landed (#81)

## Problem

B6 `continue_until_stopped` blocked the command path until the next stop (up to
the request timeout). Long-running debugees would freeze the UI command
dispatch loop.

## Delivered

| Item | Location |
| --- | --- |
| Non-blocking continue | Live `DebugStep::Continue` returns **Running** immediately |
| Background waiter | Thread runs `continue_until_stopped` (30s budget) |
| Poll | `PollDebugSession` / `:debug-poll` drains result via `try_recv` |
| Deny concurrent step | While awaiting stop, step is denied (poll or stop) |
| Abandon on stop | Dropping awaiting receiver lets worker Drop the session (kill child) |

## Commands

```text
:debug-step continue   → Running (non-blocking)
:debug-poll            → still Running, or Paused with stack after stop
:debug-stop            → disconnect (works during await)
```

## Tests

- `debug_workflow` live path: continue → Running → poll loop → Paused
- `debug_projection` parses `:debug-poll`

## Residual

- Desktop auto-poll timer (UI must call poll today)
- System lldb-dap dogfood
