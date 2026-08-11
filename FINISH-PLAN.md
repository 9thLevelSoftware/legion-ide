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

1. **Syntax highlighting rendering** — `legion-index` produces highlight spans from tree-sitter but `legion-desktop` doesn't paint them. All code is monochrome.
2. **Only Rust grammar shipped** — Only `tree-sitter-rust` is a dependency. No TS, Python, Go, C, JSON, TOML, Markdown, HTML/CSS, Bash.
3. **VT100/xterm terminal emulation** — PTY works but no CSI/SGR escape interpreter. Can't run vim, htop, or even colored ls.
4. **LSP not wired to editor** — LSP client is complete, app has composition code, but live diagnostics/completions/hover aren't connected in the desktop rendering layer.

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

### Phase 1: Syntax Highlighting
Make code look like code. The tree-sitter pipeline already produces highlight spans — wire them to egui color rendering.

- Add tree-sitter grammars: TypeScript, Python, Go, C, JSON, TOML, Markdown, HTML/CSS, Bash → `Cargo.toml`, `legion-index`
- Register grammars in plugin grammar registry, map file extensions to languages → `legion-index`
- Pipe `highlight_spans` from `SyntaxTreeCache` through `ShellProjectionSnapshot` to desktop renderer → `legion-ui`, `legion-desktop`
- Build default color scheme (dark + light) for highlight categories → `legion-desktop`
- **Done when:** open .rs, .py, .ts, .go files and each shows colored syntax

### Phase 2: Terminal Emulation
Make the terminal usable. Interpret CSI/SGR escapes so colored output and fullscreen programs work.

- Implement VT100/xterm state machine: CSI (cursor, erase, scroll), SGR (colors, bold), DEC modes (alt screen, cursor visibility) → `legion-terminal`
- Build terminal grid model: cell grid with character + attribute, dirty tracking → `legion-terminal`
- Render grid in egui: monospace font with color attributes, cursor, selection → `legion-desktop`
- Wire keyboard: translate egui keys to terminal escape sequences → `legion-desktop`
- **Done when:** `ls --color`, `htop`, `vim` render correctly

### Phase 3: Live LSP Integration
Wire the LSP client to the editor for diagnostics, completions, and hover.

- Auto-launch language server when workspace opens → `legion-app`
- Route `publishDiagnostics` to inline error/warning markers → `legion-app`, `legion-desktop`
- Trigger completion on typing / Ctrl+Space, show popup, insert on accept → `legion-app`, `legion-ui`, `legion-desktop`
- Show hover on mouse hover → `legion-desktop`
- Go-to-definition on Ctrl+Click / F12 → `legion-app`, `legion-desktop`
- Inlay hints rendering → `legion-desktop`
- **Done when:** open Rust project, see red squiggles on errors, get completions, hover shows types

### Phase 4: Navigation & UI Essentials
File tree, find/replace, keybindings, persistent state.

- Build file tree panel from workspace file listing APIs → `legion-ui`, `legion-desktop`
- In-editor find (Ctrl+F) with match highlighting and navigation → `legion-editor`, `legion-desktop`
- Find-and-replace (Ctrl+H) → `legion-editor`, `legion-desktop`
- Default keybinding map wired through intent system → `legion-ui`
- Serialize open tabs / recent files / window geometry / dock layout on close, restore on startup → `legion-storage`, `legion-app`
- Simple settings panel → `legion-desktop`
- **Done when:** navigate via tree, Ctrl+F finds text, close and reopen restores tabs

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
