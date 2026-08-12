# Phase 4: Navigation & UI Essentials — Context

## Phase Goal
Add in-editor find/replace and a default keybinding map. The file tree, settings panel, and session persistence are already built and functional.

## Scope Reduction — What's Already Built

The FINISH-PLAN.md listed 6 deliverables for Phase 4. Three are already complete:

1. **File tree panel** — Complete end-to-end: `FileTreeNode` protocol types, `WorkspaceActor::tree_snapshot()` + `scan_shallow()` + `poll_watcher_events()` in legion-project, `ExplorerProjection` + `ExplorerNodeProjection` in legion-ui, `PanelId::ProjectExplorer` registered as left-side pinned default in all dock modes, `render_project_tree_panel()` → `render_explorer_controls()` → `render_explorer_node()` in legion-desktop with expand/collapse and selection, `RefreshExplorer` + `ToggleExplorerPath` + `SelectExplorerFile` bridge actions all wired.

2. **Settings panel** — Full GUI exists: `render_settings_panel()` at view.rs:3547 with theme (Dark/Light/System), zoom (+/-/Reset), font size, toast verbosity, 10+ editor toggle checkboxes, telemetry consent, font diagnostics, defaults reset.

3. **Persistent state / session restore** — `DesktopSessionStore` in legion-desktop saves/loads `WorkspaceSessionRecord` (open tabs, active tab, explorer expansion, panel state, dock layouts, workbench settings, memory snapshot). Session saved after significant state transitions (file open/close, completion accept, definition navigate). Restored on startup via `DesktopRuntime::open()`.

## What Remains — 2 Deliverables

### 1. In-Editor Find/Replace (Ctrl+F / Ctrl+H)
**Current state:**
- Workspace-level search exists via palette (`PaletteMode::Search`) with `RunSearch` intent and `SearchProjection` — but this is a workspace file grep, not in-editor find
- `SearchPattern` in legion-project has `find_ranges(&self, text: &str) -> Vec<Range<usize>>` for regex matching
- `CommandDispatchIntent::Replace` exists but is a single-range replacement, not a find-and-replace workflow
- No `ToggleFindBar`, `FindNext`, `FindPrevious`, `ReplaceOne`, `ReplaceAll` intents
- No find bar UI in the editor surface
- No match highlight overlays in the viewport

**What to build:**
- `BufferSearchState` in legion-editor with regex-based find_matches
- Find/replace intents in legion-ui `CommandDispatchIntent`
- `FindBarProjection` with match info projected to renderers
- Find bar UI in legion-desktop (text field, match counter, prev/next buttons, replace field)
- Match highlighting in the editor viewport (yellow background on all matches, orange on current)

### 2. Default Keybinding Map
**Current state:**
- ~6 keyboard shortcuts hardcoded inline via `egui::Key` checks (F12, Escape, arrow keys in completion popup)
- Display-only `shortcut_label` strings on palette results ("Ctrl+S", "F5", etc.) — not binding logic
- No keymap data structure, no central dispatch, no user-configurable keybinding
- `keybinding_profile_label` exists in `WorkbenchLayoutSettings` DTO but is a telemetry label, not a registry

**What to build:**
- `KeyCombo` type (key + modifiers) and `KeybindingEntry` mapping to `DesktopAction`
- `default_keymap()` function with ~20 common shortcuts
- Central keyboard dispatch in the desktop render loop that checks key events against the map

## Key Design Decisions

1. **Find state lives in the app layer, not in EditorEngine** — `AppComposition` holds a `BufferSearchState` that gets projected as `FindBarProjection`. The editor provides text; the app runs the regex and tracks matches. This follows the existing projection pattern (e.g., `LanguageToolingProjection`).

2. **Match highlighting happens in the desktop renderer** — The desktop view reads match positions from `FindBarProjection` and paints highlight rectangles, same pattern as diagnostic underlines and inlay hints.

3. **Keybinding map uses `DesktopAction` directly** — The map produces `DesktopAction` variants (not `CommandDispatchIntent`), keeping it at the same level as existing hardcoded checks. Context-dependent actions (GoToDefinition needing cursor position) stay inline.

4. **No new crate dependencies** — `regex` is already in workspace Cargo.toml. Add it as a dependency to `legion-editor` and/or `legion-ui` as needed.

5. **Architecture proposals: skipped** — Standard editor find/replace bar; only one reasonable approach.

## Plan Structure
- **Plan 04-01 (Wave 1)**: Find/Replace + Keybinding Type Layer — intents, projections, keybinding types in `legion-ui`; buffer search engine in `legion-editor`
- **Plan 04-02 (Wave 2)**: Find/Replace + Keybinding Wiring & Rendering — app-layer wiring in `legion-app`; find bar UI, match highlights, keyboard dispatch in `legion-desktop`
