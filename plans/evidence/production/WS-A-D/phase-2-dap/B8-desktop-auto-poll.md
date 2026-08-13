# Phase 2 B8 — Desktop auto-poll after non-blocking continue

**Date:** 2026-07-22  
**Status:** Ready to land

## Problem

B7 made live `Continue` non-blocking (`Running` + `:debug-poll`). Desktop still
required a manual poll gesture, so long continues never re-projected stop/stack
without the user typing `:debug-poll`.

## Delivered

| Item | Location |
| --- | --- |
| Predicate | `legion-desktop::debug_auto_poll::debug_needs_auto_poll` (live + Running + session) |
| Frame tick | `DesktopEframeApp::render_app_frame` dispatches `PollDebugSession` + 50ms repaint |
| Bridge | `DesktopAction::PollDebugSession` / `StopDebugSession` → intents |
| Status row | `debug: auto-poll active …` when predicate holds |

## Tests

- unit: auto-poll only when live+Running+session
- `intent_bridge` routes poll/stop via active debug session

## Residual

- System lldb-dap dogfood
- Interactive GUI dogfood of continue→auto-poll→pause
