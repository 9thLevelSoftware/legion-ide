# Phase 2 B14 — Debug keyboard + GUI dogfood checklist

**Date:** 2026-07-23  
**Status:** Ready to land

## Problem

Windowed dogfood still needed a human journal, but debug lacked IDE-standard
keys and the interactive checklist still described pre-B4 simulated-only DAP.

## Delivered

| Item | Location |
| --- | --- |
| Debug keys (session active) | F5 Continue, Shift+F5 Stop, F10 Over, F11 Into, Shift+F11 Out |
| Idle F5 | Still Refresh Explorer |
| Docs | `docs/KEYBOARD_REFERENCE.md` |
| Checklist | `plans/evidence/dogfood/INTERACTIVE-GUI-CHECKLIST.md` (Phase 2 DAP steps) |
| Headless proof | `legion-desktop/tests/debug_keyboard.rs` (F10 → F5 → auto-poll → Shift+F5) |

## Residual

- Human must still fill `YYYY-MM-DD-interactive-gui-journal.md` for Phase 1 ≥3
  journal requirement (this PR does not invent a human session).
