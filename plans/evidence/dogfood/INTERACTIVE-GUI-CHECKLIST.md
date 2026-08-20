# Interactive GUI dogfood checklist (Phase 1 + Phase 2 DAP)

Use this for a **human-driven eframe** session. Automated substitutes exist
(B10 headless continue, B13 system launch dogfood) but do **not** replace a
windowed journal for Phase 1 “≥3 template-complete journals.”

## Setup

```text
git checkout main && git pull
cargo run -p legion-desktop -- <path-to-legion-repo-or-fixture>
```

Optional live DAP:

```text
# fake adapter (CI-grade contract, no system LLDB required)
set LEGION_DAP_USE_FAKE=1
set LEGION_DAP_MODE=live

# or a real adapter
set LEGION_DAP_ADAPTER=C:\path\to\lldb-dap.exe
# set LEGION_DAP_DOGFOOD=1   # only for fail-closed cargo dogfood tests
```

Record branch, SHA (`git rev-parse HEAD`), OS, and whether Ollama/Anthropic keys are present.

## Checklist (copy into journal)

| # | Action | Pass? | Notes |
|---|--------|-------|-------|
| 1 | Open this repo; expand nested dirs (crates/…) | | Watcher should not thrash |
| 1a | **Click a file row; confirm it opens in the editor** | | See note below |
| 2 | Edit a file; save; confirm dirty → clean; external overwrite conflict | | |
| 3 | Focus BYOK field; type; confirm key not inserted into buffer | | |
| 4 | Terminal: type command, see output, kill if needed | | |
| 5 | Assist: Deterministic proposal appears | | |
| 6 | Assist Auto with Ollama (if installed): streaming status then proposal | | |
| 7 | Delegate chat: Streaming… then reply | | |
| 8 | Git panel opens / status rows | | |
| 9 | Debug: refresh configs; Launch (`F5` idle+configs, toolbar, or `:debug-launch`) | | B17 |
| 10 | Debug dual-mode banner: **SIMULATED** (fixture) or **live adapter** | | Honest cut line |
| 11 | Debug: Continue (`F5` or toolbar); live path shows Running then auto-poll Paused | | B7/B8 |
| 12 | Debug: F9 toggle BP; Step Over (`F10`); Stop (`Shift+F5`) | | B11/B14/B15 |
| 13 | Sandbox panel: Windows caveats visible if Job Object-only | | |

> **Why rows 5–7 were unreachable, not merely unexercised.** Until 2026-08-20
> nobody had driven Assist or Delegate from a rendered frame, and both were
> broken from the outside while every projection test passed. Assist's rail
> blocked on `assisted_ai_projection.providers` being non-empty and offered
> **Settings** as the fix — but that projection describes a Phase-4 assisted-AI
> *run*, is populated only by one, no rendered control starts one, and choosing
> a preferred provider in Settings never adds a row to it. The block could not
> be cleared from inside the app, so Assist mode showed one dead-end card
> forever, hiding a `Predict` control that works with zero configuration through
> the always-registered deterministic local provider. Delegate chat was worse:
> `send_delegate_chat` has been implemented since Phase 5 — retrieval-backed,
> citation-carrying, able to stream a live reply — and had **no rendered control
> at all**, so row 7 could not be exercised even in principle. Both are fixed in
> `crates/legion-desktop/src/view.rs` and guarded by
> `crates/legion-desktop/tests/assist_delegate_reachability.rs`.

> **Why 1a is called out separately.** Until 2026-08-17 this checklist went
> straight from "expand nested dirs" to "edit a file", and never named the step
> in between. Clicking a file row selected it and opened nothing — quick-open
> (`Ctrl+P` / `Ctrl+O`) was the only way into a buffer — and because no line of
> this checklist and no test asserted the mouse path, the tree looked correct
> while the app was unusable for anyone who reached for the mouse first. Fixed
> in `crates/legion-desktop/src/workflow.rs` (`ActivateExplorerFile`) and
> guarded by `crates/legion-desktop/tests/explorer_activation.rs`. Keep this
> row: an affordance that is never checked is an affordance that can rot back.

## Commands / keys (debug)

| Action | UI | Key | Shell |
|--------|----|-----|-------|
| Refresh configs | Refresh configs | — | `:debug-configs` |
| Launch | Launch | `F5` (idle + configs) | `:debug-launch <id>` |
| Toggle BP | — | `F9` | `:debug-breakpoint …` |
| Continue | Continue | `F5` (session active) | `:debug-step continue` |
| Step over | Step Over | `F10` | `:debug-step over` |
| Step into | Step Into | `F11` | `:debug-step into` |
| Step out | Step Out | `Shift+F11` | `:debug-step out` |
| Poll | Poll | (auto on live Running) | `:debug-poll` |
| Stop | Stop | `Shift+F5` | `:debug-stop` |
| Idle F5 | — | Refresh explorer | — |

## Journal destination

Copy template from `plans/dogfood/legion-on-legion-weekly-journal-template.md` to:

```text
plans/evidence/dogfood/YYYY-MM-DD-interactive-gui-journal.md
```

## Product-readiness impact

Mark floor bugs vs known cut lines (Windows sandbox residual, unsigned installers,
no VSIX). Debug is **substrate validated** (PR-LANG-002), not full product-ready
debugger UX — do not flip ledger rows without evidence.
