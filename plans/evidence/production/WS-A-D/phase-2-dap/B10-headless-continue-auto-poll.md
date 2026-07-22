# Phase 2 B10 — Headless continue → auto-poll dogfood

**Date:** 2026-07-22  
**Status:** Ready to land

## Problem

B7/B8 made live continue non-blocking and wired desktop frame auto-poll, but
there was no end-to-end proof that a desktop frame loop actually drains stop
after continue (only pure predicate + app-level poll tests).

## Delivered

| Item | Location |
| --- | --- |
| Headless dogfood test | `legion-desktop/tests/live_continue_auto_poll.rs` |
| Path under test | live fake launch → Continue (Running) → `run_headless_full_frame` auto-poll → Paused → Stop |
| Assertions | `live_adapter`, `debug_needs_auto_poll`, status row `auto-poll active`, stack after stop |

## Honest cut line

- This is **headless eframe** (real `egui::Context` + `render_app_frame`), not a
  human windowed GUI session.
- System `lldb-dap` full launch/step vs a host debugee remains residual (B9 is
  handshake-only dogfood).

## Verification

```text
cargo build -p legion-debug --bin fake_dap_adapter
cargo test -p legion-desktop --test live_continue_auto_poll
```
