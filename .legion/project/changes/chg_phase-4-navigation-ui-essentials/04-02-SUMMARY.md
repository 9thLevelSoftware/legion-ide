# Plan 04-02 Summary: Find/Replace Wiring & Rendering

## Result
**Status**: Complete
**Wave**: 2
**Agent**: engineering-senior-developer
**Completed**: 2026-08-11

## Completed Tasks

- **Task 1: Wire Find/Replace in App Layer + Bridge** — Added 12 `DesktopAction` find/replace variants to `bridge.rs`, bridged 1:1 to `CommandDispatchIntent`. Added `buffer_search_state: BufferSearchState` field to `AppComposition`. Implemented all find intent handlers: ToggleFindBar toggles visibility, SetFindQuery runs find_matches on active buffer text, FindNext/FindPrevious advance/retreat with cursor positioning, ReplaceOne/ReplaceAll apply edits and re-search. Built `FindBarProjection` from `BufferSearchState` in snapshot assembly.

- **Task 2: Render Find Bar UI** — Added `render_find_bar()` in `view.rs` as a floating `egui::Area` anchored top-right. Find row: query text field, match counter ("N of M" / "No results"), prev/next buttons (▲/▼), case-sensitive (Aa), whole-word (Ab|), regex (.*) toggles, close button (✕). Replace row: replacement text field, Replace and Replace All buttons. Keyboard: Enter → FindNext, Escape → CloseFindBar.

- **Task 3: Match Highlights + Central Keyboard Dispatch** — Added `paint_find_match_highlights()` painting colored rectangles on code lines: yellow (`Color32::from_rgba_premultiplied(255, 235, 59, 80)`) for all matches, orange (`Color32::from_rgba_premultiplied(255, 152, 0, 120)`) for current match. Added `dispatch_keybindings()` reading `default_keymap()` and dispatching matching key combos via string → `egui::Key` conversion. Ctrl+F, Ctrl+H, F3, Shift+F3, Escape all routed.

## Files Modified
- `crates/legion-app/src/lib.rs` — BufferSearchState management, find intent handling, FindBarProjection snapshot assembly
- `crates/legion-desktop/src/view.rs` — render_find_bar() UI, paint_find_match_highlights() overlays, dispatch_keybindings() central keyboard dispatch
- `crates/legion-desktop/src/bridge.rs` — 12 DesktopAction find/replace variants, bridge dispatch mappings

## Verification Results
All verification commands passed:
- `cargo check -p legion-app` — exit 0
- `cargo check -p legion-desktop` — exit 0
- `cargo test -p legion-desktop` — all tests passed (7 integration + 2 terminal panel + 1 doctest)

## Verification Commands
| Command | Exit Code | Result |
|---------|-----------|--------|
| `cargo check -p legion-app` | 0 | PASS |
| `cargo check -p legion-desktop` | 0 | PASS |
| `cargo test -p legion-desktop` | 0 | PASS — all tests passed |

## Key Decisions
- Find bar uses `egui::Area` anchored top-right (VS Code convention), consistent with existing popup patterns (completion, hover).
- Match highlights paint BEFORE diagnostic underlines so highlights appear behind underlines.
- Central keyboard dispatch runs BEFORE existing hardcoded key checks, allowing keymap overrides.
- Context-dependent actions (Undo/Redo needing buffer_id) remain inline; keymap handles stateless actions.

## Issues Encountered
None.

## Escalations
None.

## Handoff Context
- **Key outputs**: End-to-end find/replace (Ctrl+F/Ctrl+H), central keyboard dispatch with 21 default bindings
- **Decisions made**: Top-right anchored find bar, yellow/orange match highlighting, keymap-before-hardcoded dispatch order
- **Open questions**: None
- **Conventions established**: Desktop actions bridge 1:1 to command intents; highlight painting happens in per-line render loop before diagnostics

## Requirements Covered
- In-editor find (Ctrl+F) with regex matching, match highlighting (yellow/orange), prev/next navigation
- Find-and-replace (Ctrl+H) with replace-one and replace-all
- Default keybinding map: 21 entries dispatched centrally
