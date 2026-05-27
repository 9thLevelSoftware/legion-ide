# Plan 03-02 Result: Desktop Tabs Explorer And Viewport Controls

Status: Complete

## Files Changed

- `crates/devil-desktop/src/bridge.rs`
- `crates/devil-desktop/src/view.rs`
- `crates/devil-desktop/src/workflow.rs`
- `crates/devil-desktop/tests/daily_editing_controls.rs`
- `crates/devil-desktop/tests/intent_bridge.rs`
- `crates/devil-desktop/tests/projection_rendering.rs`
- `.planning/phases/03-daily-editing-mvp/WAVE-CHECKLIST.md`

## Implementation Summary

- Added desktop bridge actions for tab switching/closing, save-all, cursor, selection, viewport scroll, explorer path toggling, and explorer file reveal.
- Kept explorer expansion adapter-local while routing explorer reveal through `CommandDispatchIntent::RevealInExplorer`.
- Added daily-editing view-model rows for tab strips, editor status, dirty-close prompts, explorer expansion/selection markers, and viewport metadata.
- Wired rendered tab/explorer controls and keyboard shortcuts through `DesktopRuntime::handle_action` and app-owned authority.
- Mapped new app outcomes into desktop workflow outcomes for save-all, tab switch/close, dirty-close prompts, cursor/selection/scroll updates, and adapter-local explorer toggles.
- Added runtime and projection tests for tabs, save-all routing, dirty-close prompt projection, prompt text suppression, viewport dispatch, explorer reveal/toggle, bridge errors, and degraded/empty states.

## Verification

| Command | Result |
| --- | --- |
| `rg -q "SwitchTab" crates/devil-desktop/src/bridge.rs` | Passed |
| `rg -q "close_dirty" crates/devil-desktop/src/workflow.rs` | Passed |
| `rg -q "tab_rows" crates/devil-desktop/src/view.rs` | Passed |
| `rg -q "SetViewportScroll" crates/devil-desktop/src/bridge.rs` | Passed |
| `rg -q "explorer_expansion" crates/devil-desktop/src/workflow.rs` | Passed |
| `cargo fmt --all --check` | Passed |
| `cargo test -p devil-desktop daily_editing_controls -- --nocapture` | Passed; 4 tests passed |
| `cargo test -p devil-desktop intent_bridge -- --nocapture` | Passed; 9 tests passed |
| `cargo test -p devil-desktop projection_rendering -- --nocapture` | Passed; 4 tests passed |
| `cargo test -p devil-desktop desktop_workflow -- --nocapture` | Passed; 4 tests passed |
| `cargo test -p devil-desktop close_dirty_prompt_disables_editor_text_input -- --nocapture` | Passed; 1 test passed |
| `cargo check -p devil-desktop --all-targets` | Passed |
| `git diff --check` | Passed with CRLF normalization warnings only |

## Decisions

- Explorer projections are flat rows with child `FileId` references, so desktop view-model expansion resolves child ids from the same projection instead of constructing an owned tree.
- Save-all desktop tests assert that the edited buffer is saved through app authority and allow other tab outcomes to preserve app-owned rejection semantics.
- Dirty-close prompt state remains app-owned in projection; the desktop runtime only uses prompt presence to suppress editor text input while a modal prompt is active.

## Issues

- None.
