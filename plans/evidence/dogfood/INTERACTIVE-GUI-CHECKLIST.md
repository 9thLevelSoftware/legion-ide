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
| 2 | Edit a file; save; confirm dirty → clean; external overwrite conflict | | See note below |
| 3 | Focus BYOK field; type; confirm key not inserted into buffer | | |
| 4 | Terminal: type command, see output, kill if needed | | |
| 5 | Assist: Deterministic proposal appears | | |
| 6 | Assist Auto with Ollama (if installed): streaming status then proposal | | |
| 7 | Delegate chat: Streaming… then reply | | |
| 8 | Git panel opens / status rows; stage a hunk, then commit it | | See note below |
| 9 | Debug: refresh configs; Launch (`F5` idle+configs, toolbar, or `:debug-launch`) | | B17 |
| 10 | Debug dual-mode banner: **SIMULATED** (fixture) or **live adapter** | | Honest cut line |
| 11 | Debug: Continue (`F5` or toolbar); live path shows Running then auto-poll Paused | | B7/B8 |
| 12 | Debug: F9 toggle BP; Step Over (`F10`); Stop (`Shift+F5`) | | B11/B14/B15 |
| 13 | Sandbox panel: Windows caveats visible if Job Object-only | | See note below |

> **Why 1a is called out separately.** Until 2026-08-17 this checklist went
> straight from "expand nested dirs" to "edit a file", and never named the step
> in between. Clicking a file row selected it and opened nothing — quick-open
> (`Ctrl+P` / `Ctrl+O`) was the only way into a buffer — and because no line of
> this checklist and no test asserted the mouse path, the tree looked correct
> while the app was unusable for anyone who reached for the mouse first. Fixed
> in `crates/legion-desktop/src/workflow.rs` (`ActivateExplorerFile`) and
> guarded by `crates/legion-desktop/tests/explorer_activation.rs`. Keep this
> row: an affordance that is never checked is an affordance that can rot back.

> **Why row 2 now says "See note below".** Driven through the rendered UI for
> the first time on 2026-08-20. The behaviour was right and the *reporting* was
> not. Typing marks the tab (`Unsaved changes` on its accessibility node), the
> published Ctrl/Cmd+S binding saves, and a save that would overwrite a change
> made outside the editor is refused with the file on disk left alone and the
> edits still in the buffer — the safety property holds. But the refusal was
> rendered as `format!("Save rejected: {response:?}")`: about fifteen hundred
> characters of lifecycle ids, version preconditions, fingerprint hashes and the
> extended-length `\\?\` path, saying nothing about what happened or whether
> the edits survived. `save_all_conflict.rs` had covered the same authority
> through `runtime.handle_action` and could not see any of that, because it
> never rendered a frame. Humanised in
> `crates/legion-desktop/src/save_rejection.rs` and guarded end to end by
> `crates/legion-desktop/tests/save_row_2.rs`.
>
> One harness defect fell out of it, and it is worth knowing about: the shared
> `full_frame_input` helper built a frame whose `RawInput::modifiers` were
> always default, while egui answers `input.modifiers` from that field rather
> than from the event in the queue. Every modifier chord sent through the shared
> rig therefore arrived as a bare keypress. The first run of this row appeared
> to show that Ctrl+S did not save; it does. Any rendered-UI test that asserts a
> chord *does* something would have been testing nothing, and one asserting a
> chord does *not* do something would have passed vacuously.

> **Why row 8 now names staging and committing.** The row used to stop at
> "status rows", and against a real repository neither half of it held.
> `GitProjection` is populated only by an explicit `RefreshGit`, and nothing
> issued one on workspace open or on selecting the surface — so opening Source
> Control over a tree with three changed files rendered "No source-control
> status", and the remote verbs (gated on a projected branch label) were absent
> with it. Underneath that, the panel had no stage, unstage or commit control at
> all: `StageGitHunk` and `UnstageGitHunk` reached `git apply --cached` through
> app authority and no rendered control ever pushed either, so Push was the only
> write the panel could perform and it could only push what some other tool had
> staged. Fixed in `crates/legion-desktop/src/view/source_control.rs` and the
> activity rail in `crates/legion-desktop/src/view.rs`, guarded by
> `crates/legion-desktop/tests/source_control_reachability.rs`, which asks git —
> not the projection — whether the index changed.
>
> That sentence used to end by saying untracked files have no stage control and
> no path-level authority exists to give them one. Both halves stopped being
> true in this same change and the note is corrected rather than left standing:
> `StageGitPath` and `UnstageGitPath` are routed through the renderer, the
> bridge, app authority and `legion-project` to `git add --` and
> `git restore --staged --`, so an untracked file, a modified binary, a
> mode-only change and anything else `git diff` emits no hunk for now has a
> Stage control of its own.
>
> What an operator should still expect to be *absent*, so a missing button reads
> as a decision rather than a defect:
>
> - **Directories.** An untracked directory is reported as one row ending in
>   `/`, and staging it would add every file underneath, unseen.
> - **Renames and copies.** A status beginning `R` or `C` names two paths, and
>   one control cannot say which one it acts on.
> - **Files with hidden textual hunks, when the hunk list is truncated.** Those
>   have hunk controls of their own; offering whole-path staging beside them is
>   one click from staging every hunk in a file somebody meant to stage one
>   hunk of.
>
> Each of those is named on screen rather than silently omitted, and each has a
> test in `source_control_reachability.rs`. Past the twelve-control budget,
> hunks and files are reachable through "Show the other N …" rather than being
> hidden — a note that named what it could not reach was the earlier behaviour
> and is not what to check for now.

> **Why rows 9-12 were never passable.** Until 2026-08-20 the shipped app could
> not start a debug session at all. `DebugWorkflow::runtime_enabled` was set by
> exactly two callers — `enable_debug_fixture_for_tests` and
> `enable_debug_live_fake_for_tests` — and both are test seams, so every Launch
> from the toolbar, from `F5`, or from `:debug-launch` returned
> `Denied: Debug runtime is disabled`. Four debug suites were green throughout,
> because each of them called a seam in its first three lines. The runtime now
> enables itself on an explicit launch the broker has approved, the same lazy
> trust-gated shape the terminal uses
> (`crates/legion-app/src/debug_workflow.rs`), and rows 9-12 are guarded from
> the rendered UI by `crates/legion-desktop/tests/debug_reachability.rs`, which
> uses no debug seam on the fixture path. Two smaller repairs came with it: the
> dual-mode banner claimed "Debugger is simulated in this build" whenever no
> session was running — including immediately after disconnecting from a live
> adapter — and `F9` stamped `Idle` over the status of a session that was still
> paused.

> **What row 13 caught.** Driven through the rendered UI for the first time on
> 2026-08-20. The Sandbox panel exists and is reachable — Delegate mode,
> confirm, submit a task — but it draws only the first five of its rows and
> collapses the rest behind "N more rows". On Windows the line saying the
> sandbox enforces process lifetime and *nothing else* was the ninth, so what a
> reader actually saw was `RestrictedToken`, "profile compiled fail-closed", and
> "filesystem scope limited to workspace root" — the last of which is false on a
> Job-Object-only host, as `docs/SECURITY.md` records. The panel now states its
> platform limits in the third row, where they cannot be truncated away, and no
> longer repeats requested-scope wording as if it were enforcement. Guarded by
> `crates/legion-desktop/tests/sandbox_reachability.rs` (rendered) and the unit
> tests in `crates/legion-desktop/src/view/sandbox_panel.rs`.

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
