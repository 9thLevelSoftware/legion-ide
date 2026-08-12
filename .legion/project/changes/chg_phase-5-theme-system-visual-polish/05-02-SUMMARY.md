# Plan 05-02 Summary: Tab Bar Polish

## Result
**Status**: Complete
**Wave**: 2
**Agent**: engineering-senior-developer
**Completed**: 2026-08-11

## Completed Tasks
- Rewrote `render_tab_strip()` with per-tab close buttons (x on hover/active, bullet for dirty tabs)
- Added horizontal `ScrollArea` wrapping with drag-to-scroll disabled (mouse-wheel only) for reorder compatibility
- Added drag-to-reorder interaction with `TabDragState` tracked via egui `data_mut` pattern
- Added `DesktopAction::ReorderTab { buffer_id, new_index }` to bridge.rs
- Added `CommandDispatchIntent::ReorderTab { buffer_id, new_index }` to ui.rs
- Wired bridge translation for ReorderTab with `with_known_tab` guard
- Added `AppCommandRequest::ReorderTab` with full handler chain in lib.rs (intent translation, pre-flight, handler, reorder_tab method)
- Added ReorderTab assertion in intent_bridge test

## Files Modified
- `crates/legion-desktop/src/view.rs` — rewrote render_tab_strip with close buttons, scroll area, drag reorder
- `crates/legion-desktop/src/bridge.rs` — added ReorderTab action + bridge translation
- `crates/legion-ui/src/ui.rs` — added ReorderTab intent variant
- `crates/legion-desktop/tests/intent_bridge.rs` — added ReorderTab bridge test assertion
- `crates/legion-app/src/lib.rs` — added ReorderTab request variant, translation, handler, reorder_tab method

## Verification Results
- `cargo check -p legion-desktop`: 0 errors, 0 warnings
- `cargo check -p legion-app`: 0 errors, 0 warnings
- `cargo test -p legion-desktop --lib`: 44 passed, 0 failed
- `cargo test -p legion-desktop --test intent_bridge`: 19 passed, 0 failed

## Key Decisions
- Used non-deprecated `scroll_source` API instead of `drag_to_scroll(false)`
- Close button rendered as separate transparent Button adjacent to tab label
- Multiplication sign (U+00D7) for close glyph, bullet (U+2022) for dirty indicator
- Drag state tracked via egui `data_mut` with `insert_temp`/`get_temp` (matches existing codebase pattern)
- Extended legion-app with ReorderTab handler for exhaustive match compatibility

## Issues Encountered
None.

## Escalations
None.

## Handoff Context
- **Key outputs**: Tab bar now has close buttons, horizontal scroll, drag-to-reorder
- **Decisions made**: ScrollArea uses mouse-wheel-only scrolling; drag reserved for reorder; close button is adjacent widget not overlay
- **Open questions**: None
- **Conventions established**: Tab drag state uses egui `data_mut` pattern with `TabDragState` struct

## Requirements Covered
- "Tab bar has close button per tab"
- "Tab bar handles overflow with scrolling"
- "Tabs can be reordered via drag"
