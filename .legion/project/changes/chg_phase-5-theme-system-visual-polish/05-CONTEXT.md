# Phase 5: Theme System & Visual Polish — Context

## Phase Goal
Complete the theme system by routing all hard-coded colors through theme tokens, polish the tab bar with close buttons and overflow handling, and implement the code minimap.

## Scope Reduction — What's Already Built

The FINISH-PLAN.md listed 5 deliverables for Phase 5. Three are already complete:

1. **Theme type system** — Complete: `Theme` struct in `legion-desktop/src/theme.rs` with `BackgroundTokens` (12 fields), `BorderTokens` (4), `TextTokens` (5), `AccentTokens` (8), `SpacingScale`, `RadiusScale`, `TypographyScale`. Thread-local `ACTIVE_THEME` with `tokens()` global accessor.

2. **Dark and light themes** — Complete: `Theme::dark()` and `Theme::light()` constructors with full color palettes. Light theme uses appropriately adjusted colors for readability on white backgrounds.

3. **Theme selection wiring** — Complete: `ThemePreference` enum (Dark/Light/System), `ThemePreferenceProjection` in legion-ui, `SetThemePreference` intent, settings panel with pill buttons, `theme::install()` applying to egui visuals, session persistence via `WorkbenchSettingsRecord.theme_preference`.

## What Remains — 3 Deliverables

### 1. Hard-Coded Colors → Theme Tokens
**Current state:**
- Diagnostic severity colors in `view.rs` are hard-coded `Color32::from_rgb(...)` at lines ~2926-2929 and ~2990-2993 (Error=red, Warning=orange, Info=blue, Hint=gray)
- Find match highlights are hard-coded at lines ~2855-2856 (yellow/orange with premultiplied alpha)
- Breadcrumb accent is hard-coded at line ~2581 (`Color32::from_rgb(75, 156, 211)`)
- Fold-range gutter indicator is hard-coded at line ~3034 (gray with alpha)
- These colors don't change when switching dark↔light theme, violating the "all panels update" success criterion

**What to build:**
- Add semantic color accessors to `Theme` (diagnostic severity colors, search match colors, breadcrumb accent, fold indicator)
- Replace all hard-coded `Color32::from_rgb(...)` in `view.rs` with theme token accessors
- Verify no raw color literals remain outside `theme.rs`

### 2. Tab Bar Polish
**Current state:**
- `render_tab_strip()` at `view.rs:2217-2288` renders basic tabs with active/inactive styling
- Close is only via right-click context menu (no × button on tab)
- No overflow handling when many tabs open (tabs overflow without scrolling or chevrons)
- No tab reordering (no drag-and-drop or keyboard reorder)

**What to build:**
- Add × close button on each tab (click to close)
- Add overflow handling: horizontal scroll or chevron indicators when tabs exceed available width
- Add tab drag-to-reorder within the tab strip

### 3. Code Minimap
**Current state:**
- Settings toggle exists: `EditorSettingsProjection.minimap_visible`, `CommandDispatchIntent::ToggleMinimap`, settings panel checkbox
- Rendering stub at `view.rs:2404-2405` shows text label "minimap" — no actual minimap
- No overview ruler exists

**What to build:**
- Minimap panel rendering: scaled-down buffer lines as colored blocks, proportional to buffer size
- Viewport indicator rectangle showing the currently visible portion
- Click-to-scroll: clicking the minimap scrolls the editor to that position
- Use theme tokens for minimap colors (code background, text representation, viewport indicator)

## Key Design Decisions

1. **Semantic color groups, not per-use-case tokens** — Add `DiagnosticTokens` and `SearchTokens` to the `Theme` struct rather than individual fields. This keeps the token system organized by domain.

2. **Tab close button as inline × glyph** — Following VS Code convention: small × on hover/active tabs, click stops event propagation before firing CloseTab.

3. **Tab overflow via horizontal scroll** — `egui::ScrollArea::horizontal()` around the tab strip. Simpler than chevron buttons and handles edge cases automatically.

4. **Minimap as right-side panel** — Rendered as a narrow column to the right of the code area when enabled. Uses the same `code_line` data but at reduced scale (~1px per line).

5. **Architecture proposals: skipped** — Standard editor visual polish; only one reasonable approach per feature.

## Plan Structure
- **Plan 05-01 (Wave 1)**: Theme Color Consolidation — add semantic tokens to `theme.rs`, route all hard-coded colors in `view.rs`
- **Plan 05-02 (Wave 2)**: Tab Bar Polish — close buttons, overflow scroll, drag reorder
- **Plan 05-03 (Wave 3)**: Code Minimap — rendering, viewport indicator, click navigation
