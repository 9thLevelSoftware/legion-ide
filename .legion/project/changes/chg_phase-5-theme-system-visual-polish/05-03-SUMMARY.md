# Plan 05-03 Summary: Code Minimap

## Result
**Status**: Complete
**Wave**: 3
**Agent**: engineering-senior-developer
**Completed**: 2026-08-11

## Completed Tasks
- Deleted minimap stub from status info block (was `ui.label(theme::label("minimap"))`)
- Restructured editor layout: horizontal split with code ScrollArea left, minimap column right
- Implemented `render_minimap()` function with scaled buffer overview rendering
- Small files: colored bars per line using `small_buffer_preview` line lengths at 30% opacity
- Large/degraded files: centered "..." placeholder with viewport indicator
- Viewport indicator rectangle showing current scroll position (fill: `bg.hover` at 60%, border: `border.strong`)
- Click-to-scroll: dispatches `SetViewportScroll` centering clicked line in viewport
- Drag-to-scroll: continuous scroll updates while mouse pressed on minimap
- Minimap width: 100px fixed, collapses to 0 when hidden
- Added `minimap_toggle_persists_through_settings` test in daily_editing_contracts

## Files Modified
- `crates/legion-desktop/src/view.rs` -- restructured editor layout, deleted stub, added render_minimap()
- `crates/legion-app/tests/daily_editing_contracts.rs` -- added minimap toggle round-trip test

## Verification Results
- `cargo check -p legion-desktop`: 0 errors, 0 warnings
- `cargo check -p legion-app`: 0 errors, 0 warnings
- `cargo test -p legion-desktop`: 54 passed, 0 failed
- `cargo test -p legion-app --test daily_editing_contracts`: 12 passed, 0 failed

## Key Decisions
- Layout split via `UiBuilder::new().max_rect().layout()` pattern (matches existing codebase at workflow.rs)
- Used `theme::dim()` for opacity-reduced colors (viewport indicator, line bars)
- Viewport indicator uses `StrokeKind::Inside` per egui 0.34 API
- Click-to-scroll centers the clicked line, preserving current `left_column`
- Large files show placeholder rather than attempting to render without full buffer data

## Issues Encountered
None.

## Escalations
None.

## Handoff Context
- **Key outputs**: Minimap renders when `minimap_visible` is true, right of code area
- **Decisions made**: 100px fixed width, bars at 30% opacity, viewport indicator at 60% opacity
- **Open questions**: None
- **Conventions established**: Layout split pattern for side panels adjacent to code area

## Requirements Covered
- "Code minimap renders scaled buffer overview"
- "Minimap shows viewport indicator"
- "Click minimap to scroll"
