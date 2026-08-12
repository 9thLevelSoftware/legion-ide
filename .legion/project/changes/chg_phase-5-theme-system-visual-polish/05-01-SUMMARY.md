# Plan 05-01 Summary: Theme Color Consolidation

## Result
**Status**: Complete
**Wave**: 1
**Agent**: engineering-senior-developer
**Completed**: 2026-08-11

## Completed Tasks
- Added `DiagnosticTokens` struct (error, warning, info, hint) to theme.rs
- Added `SearchTokens` struct (match_highlight, current_match) to theme.rs
- Added `ChromeTokens` struct (breadcrumb_accent, fold_indicator) to theme.rs
- Added `diagnostic`, `search`, `chrome` fields to `Theme` struct
- Populated dark values: diagnostic reuses accent palette, search uses unmultiplied alpha, chrome uses breadcrumb blue and fold gray
- Populated light values: adjusted colors for light backgrounds, search highlights slightly more opaque
- Replaced 11 hard-coded color literals in view.rs with theme token accessors
- Added `dark_and_light_diagnostic_tokens_differ` test verifying all new token groups differ across themes

## Files Modified
- `crates/legion-desktop/src/theme.rs` — 3 new token structs, 3 new fields on Theme, dark/light values, differentiation test
- `crates/legion-desktop/src/view.rs` — replaced all hard-coded Color32::from_rgb and from_rgba_premultiplied with tokens().{group}.{field} accessors

## Verification Results
- `cargo check -p legion-desktop`: 0 errors, 0 warnings
- `cargo test -p legion-desktop --lib`: 44 passed, 0 failed
- `cargo test -p legion-desktop --test intent_bridge`: 19 passed, 0 failed
- `rg "Color32::from_rgb" view.rs`: 4 remaining, all terminal ANSI (excluded by plan)
- `rg "Color32::from_rgba_premultiplied" view.rs`: 0 remaining

## Key Decisions
- Dark diagnostic tokens reuse existing accent palette (e.g., accent.red for error)
- All values use const-compatible constructors (from_rgb, from_rgba_unmultiplied_const)
- Light search highlights use higher alpha (100/140 vs 80/120) for visibility

## Issues Encountered
None.

## Escalations
None.

## Handoff Context
- **Key outputs**: `tokens().diagnostic`, `tokens().search`, `tokens().chrome` are the access patterns
- **Decisions made**: Reuse accent palette values for diagnostic tokens, unmultiplied alpha for search/chrome
- **Open questions**: None
- **Conventions established**: All new color tokens use `from_rgba_unmultiplied_const()` for alpha values, `from_rgb()` for opaque values

## Requirements Covered
- "Hard-coded colors route through theme tokens"
- "Dark/light switch updates all panels including diagnostics and search highlights"
