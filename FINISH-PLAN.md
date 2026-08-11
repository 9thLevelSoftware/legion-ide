# Legion IDE — Finish Plan

Source-only audit of all 28 Rust crates, completed 2026-08-11.
Zero `todo!()`, zero `unimplemented!()`, zero stub functions across the entire codebase.

## Codebase Summary

| Metric | Count |
|--------|-------|
| Source lines | 196,221 |
| Test lines | 77,115 |
| Crates | 28 |
| Test functions | 2,216 |
| Binary entry points | 3 (legion-app, legion-cli, legion-desktop) |

Every crate is **FUNCTIONAL** — real logic, real tests, real platform integration.

## Crate Inventory

### Core Editor
| Crate | Source | Tests | What it does |
|-------|--------|-------|-------------|
| legion-text | 2,576 | ~2,400 | Rope-backed buffer, line index, UTF-16, chunked snapshots |
| legion-editor | 4,108 | ~4,200 | Multi-buffer engine: undo/redo, transactions, viewport projection, lexical completion, diff |
| legion-ui | 8,814 | ~8,800 | Shell projection/intent layer: command palette, dock, 40+ intents, fleet board |
| legion-terminal | 2,659 | ~2,700 | PTY lifecycle, credential redaction, OSC parsing |
| legion-desktop | 25,050 | ~5,800 | egui desktop shell rendering all panels |

### AI & Agents
| Crate | Source | Tests | What it does |
|-------|--------|-------|-------------|
| legion-ai | 2,876 | ~2,200 | Provider routing, manifest assembly, streaming, tool-calling, secret redaction |
| legion-ai-providers | 5,832 | ~2,200 | 6 adapters (Ollama, OpenAI, Anthropic, llama.cpp, etc.), SSE, MCP client |
| legion-agent | 5,588 | ~2,200 | DAG scheduler, delegated task loop, 7 tool executors |
| legion-sandbox | 1,922 | ~1,100 | OS-level sandbox (Linux/macOS/Windows), fail-closed |
| legion-security | 4,808 | ~1,100 | Deny-by-default broker, 20+ capability namespaces, risk engine, secret scanner |

### Infrastructure
| Crate | Source | Tests | What it does |
|-------|--------|-------|-------------|
| legion-protocol | 29,092 | 10,900 | 989 public items: port traits, identifiers, proposal lifecycle, tool schemas |
| legion-project | 9,044 | 3,015 | Workspace actor: file save w/ conflict detection, git integration, Tantivy search |
| legion-index | 7,027 | 3,432 | Tree-sitter parsing, outline, fuzzy search, PageRank, hybrid retrieval, structural search |
| legion-storage | 6,260 | 303 | 20+ record type CRUD, checkpoints, dock layouts, semantic metadata, OS keyring secrets |
| legion-lsp | 4,601 | 3,118 | Complete LSP client: JSON-RPC framing, circuit breaker, hover/completion/definition/rename |
| legion-platform | 2,907 | 27 | ConPTY/Unix PTY, atomic writes, process spawn w/ timeout, secret filtering |

### Auxiliary Systems
| Crate | Source | Tests | What it does |
|-------|--------|-------|-------------|
| legion-app | 49,878 | 25,587 | Composition root: wires all crates, file mgmt, palette, search, git, debug, terminal, LSP, AI |
| legion-observability | 4,273 | ~300 | Event envelopes, redacting sinks, SHA-256, crash capture, diagnostics export |
| legion-retention | 2,947 | ~150 | ChaCha20-Poly1305 vault, OS keyring keys, key rotation, privacy deletion |
| legion-remote | 2,748 | ~200 | Remote workspace: filesystem ops, SSH/devcontainer, proposal-gated mutations |
| legion-debug | 2,278 | ~500 | DAP framing, live debug sessions, breakpoints, stepping, stack inspection |
| legion-remote-transport | 1,990 | ~200 | rustls mTLS, certificate pinning, flow control, resume tokens |
| legion-collaboration | 1,757 | ~200 | OT engine, multi-participant convergence, fail-closed conflicts |
| legion-cli | 1,633 | ~100 | Diagnostic CLI: phase gates, doctor, storage check, evidence validation |
| legion-plugin | 1,388 | ~400 | WASM runtime (Wasmtime), ABI validation, trust enforcement, quota tracking |
| legion-telemetry | 1,328 | ~100 | Durable spool, atomic writes, HTTP export, retry handling |
| legion-memory | 1,329 | ~100 | Consent-gated retention, compaction, trace export, secret detection |
| legion-vscode-compat | 987 | 133 | VS Code extension compatibility: tier classification, Open VSX resolver |
| legion-tracker | 521 | ~100 | Agent run ledger, workflow tracking, merge-readiness invariants |

## Gaps — What's Missing

### Critical (can't use daily without these)

1. ~~**Syntax highlighting rendering**~~ — DONE (Phase 1). `highlight_captures_from_text()` now dispatches to all bundled grammars via `language_for_path()`. Desktop renderer wired via `legion-app`.
2. ~~**Only Rust grammar shipped**~~ — DONE (Phase 1). 9 grammar crates added: Python, TypeScript, Go, C, JSON, TOML, Markdown, Bash, JavaScript. 13 language IDs mapped from 25+ file extensions.
3. ~~**VT100/xterm terminal emulation**~~ — DONE (Phase 2). Full VT100 emulator with CSI/SGR/DEC modes, cell grid rendering, keyboard translation.
4. ~~**LSP not wired to editor**~~ — DONE (Phase 3). Diagnostic underlines, completion popup, hover tooltip, go-to-definition (Ctrl+Click/F12), inlay hints, definition picker all rendered in desktop layer.

### Important (expected in any modern editor)

5. **File tree / project explorer** — No file tree panel. Files only openable via command palette.
6. **Settings UI** — Settings types exist but no GUI for editing preferences.
7. **Find & replace in editor** — Workspace search is built but no Ctrl+F / Ctrl+H in the editor.
8. **Keybinding system** — 40+ intents defined but no user-configurable keymap.
9. **Persistent state** — `InMemoryStorage` is primary backend. Open tabs, recent files, etc. lost on restart.

### Nice to Have

10. **Theme system** — No dark/light selection with syntax color schemes.
11. **Minimap** — No code overview ruler.
12. **Tab management polish** — Tab reordering, split views, close behavior.
13. **Extension host** — Metadata analysis built, no Node.js runtime for VS Code extensions.
14. **Remote server** — Client code is production-grade, no server counterpart.

## Plan — 6 Phases

### Phase 1: Syntax Highlighting ✓ COMPLETE
Multi-language tree-sitter grammar dispatch implemented. 9 grammar crates, 13 language IDs, 25+ file extensions, per-language OnceLock dispatch table, 17+ tests all passing. Desktop rendering path wired via `language_for_path()` in `legion-app`.

- ✓ Added 9 tree-sitter grammars (Python, TypeScript, Go, C, JSON, TOML, Markdown, Bash, JavaScript)
- ✓ Per-language dispatch table with OnceLock pattern in `legion-index`
- ✓ `language_for_path()` maps 25+ file extensions to 13 language IDs
- ✓ Desktop renderer wired: `legion-app` calls `language_for_path()` instead of hardcoded Rust
- Remaining: color scheme (dark + light) for highlight categories → deferred to Phase 5 (Theme System)

### Phase 2: Terminal Emulation ✓ COMPLETE
VT100/xterm escape sequence interpreter with full CSI/SGR/DEC mode support. 2D cell grid with per-cell color attributes, scrollback buffer, alt screen. egui renderer with 256-color palette and per-cell TextFormat. Keyboard translation for arrow keys, function keys, Ctrl combos.

- ✓ VT100 state machine: CSI cursor/erase/scroll, SGR 16/256/RGB colors, DEC private modes (alt screen 1049, cursor visibility 25, application cursor keys 1)
- ✓ Cell grid model: 2D array of cells with character + attributes (fg, bg, bold, dim, italic, underline, strikethrough, inverse), scrollback buffer (1000 lines)
- ✓ Parser handles partial escape sequences across PTY read boundaries
- ✓ Pipeline wired: PTY → OSC parse → credential redact → VT100 emulator → cell grid → egui colored render
- ✓ Keyboard: arrow keys, F1-F12, Home/End/PgUp/PgDn, Ctrl+A-Z → terminal escape sequences
- ✓ 56 new VT100 tests, 2,403 lines across 8 files in 5 crates

### Phase 3: Live LSP Integration ✓ COMPLETE
Desktop rendering layer wired to LSP projections. Diagnostic underlines, completion popup, hover tooltip, go-to-definition (Ctrl+Click + F12), inlay hints, definition picker. didClose notification added to LSP client.

- ✓ Diagnostic underlines: severity-colored (red/orange/blue/gray) line_segment overlays per problem
- ✓ Diagnostic hover tooltip: severity, message, source, code via `egui::Tooltip::always_open()`
- ✓ Completion popup: `egui::Area` dropdown with kind badge, label, detail, arrow/Enter/Tab/Escape nav
- ✓ Hover tooltip: label + summary via `egui::Area`, Escape dismisses, position-change debouncing
- ✓ Go-to-definition: Ctrl+Click and F12, single result auto-navigates, multi-result shows picker
- ✓ Inlay hints: ghost text at 50% opacity with `: ` prefix for type hints
- ✓ Ctrl+hover underline: link-colored word underline on Ctrl+hover
- ✓ `did_close_notification` added to `legion-lsp` with test
- 3 files changed, 333 insertions across 2 plans (03-01 diagnostics + 03-02 completion/hover/definition/hints)

### Phase 4: Navigation & UI Essentials ✓ COMPLETE
File tree, find/replace, keybindings, settings panel, session persistence — all built.

- ✓ File tree panel built end-to-end (completed pre-Phase 4: `FileTreeNode`, `ExplorerProjection`, `render_project_tree_panel()`)
- ✓ In-editor find (Ctrl+F) with regex matching, match highlighting (yellow/orange), prev/next navigation → `legion-editor`, `legion-ui`, `legion-app`, `legion-desktop`
- ✓ Find-and-replace (Ctrl+H) with replace-one and replace-all → same crates
- ✓ Default keybinding map: 21 entries in `default_keymap()`, central dispatch in desktop view → `legion-ui`, `legion-desktop`
- ✓ Settings panel built (completed pre-Phase 4: `render_settings_panel()` with theme, zoom, font, toggles)
- ✓ Session persistence built (completed pre-Phase 4: `DesktopSessionStore`, `WorkspaceSessionRecord`)
- 8 files changed, 1020 insertions across 2 plans (04-01 type layer + 04-02 wiring/rendering), 10 tests

### Phase 5: Theme System & Visual Polish
Dark/light themes, consistent syntax colors, rendering polish.

- Define `ThemeDefinition` type (editor, syntax, UI chrome colors) → `legion-protocol`, `legion-ui`
- Ship 2 built-in themes: dark and light → `legion-app`
- Wire theme selection through settings, apply to egui + syntax → `legion-desktop`
- Code minimap / overview ruler → `legion-desktop`
- Tab bar polish: reordering, close buttons, overflow → `legion-desktop`
- **Done when:** switch dark/light, all panels update, syntax colors match

### Phase 6: Integration Testing & Release
End-to-end testing and release artifacts.

- Integration tests: open project → edit → syntax colors → LSP diagnostics → terminal commit → `legion-app`, `legion-desktop`
- Dog-food on own codebase, file bugs, fix blockers → all crates
- CI pipeline: build, test, package for Windows/macOS/Linux → xtask, CI
- Release binaries with auto-updater → `legion-app`
- **Done when:** install from release artifact on clean machine, open project, edit, run, commit

## Deliberately Deferred

These have production-grade client code already built. They need server-side infrastructure that's out of scope for the IDE finish:

- Remote development server
- Plugin marketplace
- VS Code extension runtime (Node.js host)
- Collaboration transport
- Hosted telemetry backend
