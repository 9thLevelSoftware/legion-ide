# Legion IDE — Architecture Analysis

> Generated 2026-08-11 · 857 source files · 30 crates · ~203k LOC Rust
> All 6 finish phases complete. Zero `todo!()`, zero `unimplemented!()`.

## 1. System Overview

Legion IDE is a proprietary, Rust-native code editor with integrated AI-assisted
development, multi-agent workflow orchestration, and defense-in-depth security.
The codebase is a Cargo workspace of 30 crates (29 under `crates/` + `xtask`)
targeting Windows, macOS, and Linux via egui 0.34.2.

**Key design principles:**
- **Projection-only UI** — the renderer never owns editor state; it reads
  `ShellProjectionSnapshot` and emits `CommandDispatchIntent` back.
- **Proposal-mediated writes** — every file mutation flows through a
  proposal/risk-assessment pipeline before touching disk.
- **Metadata-only egress** — AI provider traffic carries fingerprints and byte
  counts, never raw source text.
- **Fail-closed security** — unknown capabilities are denied; sandboxes report
  honest enforcement gaps.
- **Port/adapter uniformity** — every domain boundary is a single-method
  `fn handle(Request) -> ProtocolResult<Response>` trait.

**Edition**: Rust 2024 · rust-version 1.92 · resolver v3

**Binaries**: 3 main entry points (`legion-app`, `legion-cli`, `legion-desktop`)
plus 5 golden-path verification binaries and 4 test fixtures/probes.

## 2. Crate Dependency Graph

Clean DAG — no circular dependencies. 7 layers from leaf to top.

```
Layer 6  legion-desktop (11 deps)
Layer 5  legion-app (21 deps — composition root)
Layer 4  legion-agent, xtask
Layer 3  legion-ai-providers, legion-cli, legion-memory, legion-plugin,
         legion-project, legion-tracker
Layer 2  legion-ai, legion-editor, legion-index, legion-storage,
         legion-terminal
Layer 1  legion-collaboration, legion-debug, legion-lsp, legion-observability,
         legion-platform, legion-remote, legion-remote-transport,
         legion-retention, legion-security, legion-telemetry, legion-text,
         legion-ui, legion-vscode-compat
Layer 0  legion-protocol (0 deps), legion-sandbox (0 deps)
```

### High Fan-In (most depended upon)

| Crate | Dependents |
|-------|-----------|
| legion-protocol | 28 (universal — every crate except legion-sandbox) |
| legion-security | 7 |
| legion-observability | 6 |
| legion-platform | 6 |
| legion-storage | 6 |
| legion-ai | 4 |

### High Fan-Out (most dependencies)

| Crate | Internal deps |
|-------|-------------|
| legion-app | 21 (composition root) |
| legion-desktop | 11 |
| legion-agent | 6 |
| legion-plugin | 5 |
| legion-project | 5 |

### Zero Fan-In (leaf crates — not depended on by other workspace crates)

`legion-cli`, `legion-desktop`, `legion-remote-transport`, `legion-retention`,
`legion-telemetry`, `legion-vscode-compat`, `xtask`

## 3. Layer Architecture

### Layer 0 — Protocol (`legion-protocol`, 29k LOC)

The vocabulary layer. 978+ public items defining every DTO, identifier, trait
port, and lifecycle enum used across crate boundaries.

- **Identifiers**: `ProjectId`, `WorkspaceId`, `BufferId`, `FileId`, `ProposalId`, `CorrelationId`
- **Port traits** (single `handle(Req) -> ProtocolResult<Resp>` method each):
  `WorkspacePort`, `EditorPort`, `ProposalPort`, `TerminalPort`, `LspPort`,
  `SemanticPort`, `CapabilityBrokerPort`, `EventSinkPort`, `StorageRepositoryPort`,
  `PluginPort`, `ProjectInfoPort`
- **Projection DTOs**: `ViewportProjection`, `LanguageToolingProjection`,
  `TerminalPanelProjection`, `LspServerHealthProjection`
- **Workflow contracts**: `LegionWorkflowSession`, `LegionWorkflowState`,
  `LegionWorkflowKillSwitch`, `LegionWorkflowMergeReadiness`

### Layer 1 — Primitives (13 crates)

Each depends only on `legion-protocol`:

| Crate | LOC | Purpose |
|-------|-----|---------|
| legion-text | 2,574 | Rope-backed buffer, line index, UTF-16, chunked snapshots |
| legion-security | 4,808 | DenyByDefaultBroker, 20+ capability namespaces, risk engine, secret scanner |
| legion-observability | 4,267 | Event envelopes, redacting sinks, SHA-256, crash capture |
| legion-platform | 2,907 | ConPTY/Unix PTY, atomic writes, process spawn with timeout |
| legion-ui | 9,002 | Shell projection/intent layer: CommandDispatchIntent (~40+ variants), dock/panel system |
| legion-lsp | 4,613 | LSP client: JSON-RPC framing, circuit breaker, hover/completion/definition/rename |
| legion-terminal | 4,598 | VT100 emulator, cell grid, SGR 16/256/RGB, scrollback, keyboard translation |
| legion-debug | 2,277 | DAP framing, live debug sessions, breakpoints, stepping |
| legion-collaboration | 1,756 | OT engine, multi-participant convergence |
| legion-remote | 2,747 | Remote workspace: filesystem ops, SSH/devcontainer |
| legion-remote-transport | 1,990 | rustls mTLS, certificate pinning, flow control |
| legion-retention | 2,944 | ChaCha20-Poly1305 vault, OS keyring keys, key rotation |
| legion-vscode-compat | 987 | VS Code extension tier classification, Open VSX resolver |
| legion-telemetry | 1,327 | Durable spool, atomic writes, HTTP export |

### Layer 2 — Editor Infrastructure (5 crates)

| Crate | LOC | Purpose | Key Types |
|-------|-----|---------|-----------|
| legion-editor | 4,225 | Multi-buffer engine | `EditorEngine`, `TextEdit`, `TextPosition`, `SaveAcknowledgement` |
| legion-index | 7,704 | Tree-sitter parsing, semantic index | `TreeSitterParser`, `SemanticIndex`, `LexicalIndexer` |
| legion-ai | 2,876 | Provider routing, streaming | Tool-calling, secret redaction, manifest assembly |
| legion-storage | 6,260 | 20+ record type CRUD | `InMemoryStorage`, `FileBackedStorage`, atomic writes |
| legion-terminal | 4,598 | (also Layer 1) | VT100 state machine, cell grid, PTY lifecycle |

### Layer 3 — Agent & Integration (6 crates)

| Crate | LOC | Purpose |
|-------|-----|---------|
| legion-ai-providers | 5,947 | 6 adapters (Ollama, OpenAI, Anthropic, llama.cpp, etc.), SSE, MCP client |
| legion-agent | 6,588 | DAG scheduler, delegated task loop, 7 tool executors |
| legion-plugin | 1,388 | WASM runtime (Wasmtime), ABI validation, trust enforcement, quota tracking |
| legion-project | 9,044 | Workspace actor: file save, conflict detection, git integration, Tantivy search |
| legion-memory | 1,328 | Consent-gated retention, compaction, trace export |
| legion-tracker | 521 | Agent run ledger, workflow tracking |

### Layer 4 — Application Composition (`legion-app`, 51k LOC)

The composition root wiring all crates into a single product state machine.

**`AppComposition`** — ~250 public methods organized by domain:
- **Lifecycle**: `open_workspace`, `open_file`, `save_active_buffer`, `switch_tab`, `close_tab`, `reorder_tab`
- **Editing**: `edit_active_buffer`, `set_buffer_cursor`, `set_buffer_selection`, `dispatch_ui_intent`
- **LSP**: `issue_lsp_completion_request`, `issue_lsp_hover_request`, `issue_lsp_definition_request`, `ingest_lsp_*_response_for_buffer`
- **Search/Git**: `run_search`, `run_structural_search`, `refresh_git_projection`
- **AI/Workflow**: `start_delegated_task`, `execute_legion_workflow`, `start_ai_run`
- **Projections**: `shell_projection_snapshot`, `active_buffer_projection`, `explorer_projection`

### Layer 5 — Desktop Shell (`legion-desktop`, 26k LOC)

Pure rendering adapter — zero product state.

- **`DesktopEframeApp`** — implements `eframe::App`, the egui render loop
- **`DesktopCommandBridge`** — translates raw input → `DesktopAction` → `CommandDispatchIntent`
- **`DesktopProjectionViewModel`** — converts `ShellProjectionSnapshot` → egui-drawable view models
- **View modules**: one file per panel (`fleet_board`, `terminal_panel`, `plan_editor`,
  `proposal_review`, `sandbox_panel`, `risk_strip`, `code_canvas_painter`, `ghost_text`, etc.)
- **Theme**: `DiagnosticTokens`, `SearchTokens`, `ChromeTokens` — semantic color tokens, dark/light
- **Tab bar**: per-tab close buttons, drag-to-reorder with insertion indicator
- **Code minimap**: scaled buffer overview, viewport indicator, click/drag-to-scroll

### Supporting — CLI (`legion-cli`, 1,655 LOC)

Diagnostic CLI: phase evidence gates (`finish-phase1` through `finish-phase6`),
`doctor` (platform health check), `setup`, storage verification.

## 4. Key Data Flows

### Editor Data Flow (read path)
```
AppComposition.active_buffer_projection()
  → EditorEngine.viewport_projection()
    → TextBuffer rope snapshot
    → TreeSitterParser.highlight_captures_from_text()
    → LspPort: diagnostics, inlay hints
  → ShellProjectionSnapshot (legion-ui)
    → DesktopProjectionViewModel.from_snapshot() (legion-desktop)
      → egui render
```

### Editor Data Flow (write path)
```
User input → DesktopCommandBridge → DesktopAction
  → CommandDispatchIntent (legion-ui)
    → AppComposition.dispatch_ui_intent()
      → EditorEngine.apply_edit() / .undo() / .redo()
      → WorkspaceActor: save with conflict detection
      → ProposalPort: risk assessment pipeline
```

### AI Assist Flow
```
User prompt → CommandDispatchIntent::StartDelegatedTask
  → AppComposition.start_delegated_task()
    → DenyByDefaultBroker.decide("ai.*") capability check
    → legion-ai: provider routing + manifest assembly
    → legion-ai-providers: adapter dispatch (Ollama/OpenAI/Anthropic/etc.)
    → legion-agent: DAG scheduler, tool executors
    → ProposalPort: changes flow through proposal lifecycle
```

### Terminal Data Flow
```
CommandDispatchIntent::TerminalLaunch
  → AppComposition.dispatch_ui_intent()
    → DenyByDefaultBroker.decide("terminal.*")
    → PtyService.spawn() (ConPTY on Windows, openpty on Unix)
    → OSC parse → credential redact → VT100 emulator
    → Cell grid → TerminalPanelProjection → egui colored render
```

## 5. Security Architecture

**`DenyByDefaultBroker`** — central capability broker with `SecurityPolicy`:

- **20+ capability namespaces**: `ai.`, `collaboration.`, `remote.`, `telemetry.`,
  `retention.raw_source.`, `storage.migration.`, `cloud.`, `plugin.`, `fs.`,
  `terminal.`, `lsp.`, `network.`
- **~14 sub-policies**: `PathPolicy`, `CommandTaxonomy`, `TerminalPolicy`,
  `LspLaunchPolicy`, `PluginCapabilityPolicy`, `NetworkPolicy`, `AiProviderPolicy`, etc.
- **Trust-gated**: `TrustState::{Trusted, Untrusted, Unknown}` checked before detailed policy
- **Org policy distribution**: `OrgPolicyBundle` with signed/versioned policies,
  `ProductMode` ceiling (Manual < Assist < Delegates < Automate < LegionWorkflows)
- **Path normalization**: UNC prefixes, drive letters, `..` traversal rejection,
  case-folding only on Windows
- **Secret scanning**: `scan_payload_for_sensitive_markers` — AWS keys, `ghp_`, `sk-`,
  `xoxb-`, PEM headers, raw-prompt markers

**Sandbox** (`legion-sandbox`):
- OS-level sandbox: Linux (Landlock), macOS (sandbox-exec), Windows
- Fail-closed enforcement with honest gap reporting
- Hostile plugin fixtures: `capability_probe.wat`, `loop.wat`, `oom.wat`

**Plugin isolation** (`legion-plugin`):
- Wasmtime WASM runtime, ABI validation (`PHASE5_PLUGIN_ABI_VERSION: u16 = 1`)
- Per-invocation host-call quota + cumulative output-byte quota
- Capability membership + security broker check before every host call
- Lifecycle states: Discovered → Rejected/Loaded → Activated → Running → Idle/Crashed

## 6. Test Coverage Map

**2,166 `#[test]` functions** across 29 crates (959 unit, 1,207 integration).

| Crate | Unit | Integration | Total |
|-------|------|------------|-------|
| legion-app | 128 | 363 | 491 |
| legion-desktop | 44 | 292 | 336 |
| legion-protocol | 30 | 129 | 159 |
| legion-terminal | 101 | 21 | 122 |
| legion-security | 69 | 30 | 99 |
| legion-agent | 47 | 43 | 90 |
| legion-lsp | 18 | 71 | 89 |
| legion-index | 31 | 46 | 77 |
| legion-project | 21 | 55 | 76 |
| legion-editor | 31 | 39 | 70 |
| legion-ai-providers | 42 | 25 | 67 |
| legion-storage | 51 | 5 | 56 |
| legion-observability | 40 | 8 | 48 |
| legion-ui | 44 | 3 | 47 |
| legion-ai | 26 | 20 | 46 |
| legion-text | 39 | 5 | 44 |
| (15 others) | 197 | 52 | 249 |

**Golden path binaries** (5): `golden_path_1.rs` through `golden_path_5.rs` in
`crates/legion-app/src/bin/`, each exercising a full user journey through `AppComposition`.

**Phase 6 acceptance tests** (5): `crates/legion-app/tests/phase6_acceptance.rs` —
open→edit→save→syntax→terminal→git commit through `AppComposition`.

**View-model integration tests** (6): `crates/legion-desktop/tests/user_journey_rendering.rs` —
syntax highlights, diagnostics, terminal, tabs, git via `DesktopProjectionViewModel`.

### Test Fixtures
- `fixtures/gp1-rust/` — standalone Rust project for golden path 1
- `crates/legion-plugin/fixtures/hostile/` — WASM hostile-plugin probes (sandbox testing)
- `evals/fixtures/` — AI eval datasets
- `training/fixtures/` — AI training traces

## 7. CI/CD Pipeline

5 GitHub Actions workflows:

| Workflow | Trigger | Jobs |
|----------|---------|------|
| `legion-gates.yml` | push/PR | 3-OS matrix: fmt, check, test, clippy + cargo-deny |
| `legion-smoke.yml` | scheduled | 3-OS smoke tests + golden-path + update-drill |
| `legion-bench.yml` | scheduled | Performance/benchmark recording |
| `legion-preview.yml` | scheduled | Preview builds |
| `legion-release.yml` | v* tags / manual | build-and-test → release (3-OS, `--from-artifacts`) |

### xtask Commands
`CheckDeps`, `DocsHygiene`, `ClaimAudit`, `NoEguiTextedit`, `ReleasePipeline`,
`VerifyReleasePipeline`, `ReleaseManifest`, `PerfHarness`, `VerifyPerfHarness`,
`LegionBench`, `VerifyLegionBench`, `VerifyKanbanBacklog`,
`VerifyReadinessConsistency`, `RustAnalyzerSmoke`, `GoldenPath1`–`GoldenPath5`,
`HostileEvals`, `VerifyHostileEvals`, `UpdateDrill`

## 8. Configuration & Environment

| File | Purpose |
|------|---------|
| `Cargo.toml` | Workspace: 30 members, workspace-level dep pins |
| `deny.toml` | cargo-deny license/advisory policy |
| `ENGINEERING_AUDIT.yaml` | Engineering audit tracking |
| `ENGINEERING_PLAN.yaml` | Engineering plan metadata |
| `pyproject.toml` | Python tooling config (eval/training scripts) |

**Build profiles**:
- `[profile.dev]` — `debug = "line-tables-only"` (keeps target/ under control)
- `[profile.dev-full]` — `debug = "full"` (opt-in for debugger stepping)

**Feature flags** (legion-app):
- `"ai"` — optional: legion-agent, legion-ai, legion-ai-providers, legion-sandbox
- `"test-helpers"` — dev-only test utilities
- `default-features = false` used by legion-desktop to selectively re-enable

## 9. Risk Hotspots

### By File Size (complexity concentration)

| File | Lines | Risk |
|------|-------|------|
| `legion-app/src/lib.rs` | 34,045 | Monolithic composition root — any refactor ripples widely |
| `legion-protocol/src/lib.rs` | 27,396 | Universal hub (28 dependents) — breaking changes are workspace-wide |
| `legion-desktop/src/view.rs` | 9,644 | All panel rendering in one file |
| `legion-project/src/lib.rs` | 9,044 | Workspace actor with git + search + file save |
| `legion-ui/src/ui.rs` | 8,289 | Shell struct + CommandDispatchIntent enum (~40+ variants) |

### Unsafe Code (9 files)

| File | Purpose |
|------|---------|
| `legion-platform/src/lib.rs` | ConPTY FFI, process spawn |
| `legion-sandbox/src/spawn.rs` | OS sandbox enforcement |
| `legion-sandbox/src/spawn_stdio.rs` | Sandboxed stdio |
| `legion-storage/src/lib.rs` | MoveFileExW atomic rename (Windows) |
| `legion-protocol/src/lib.rs` | Inline SHA-256 |
| `legion-desktop/src/workflow.rs` | eframe lifecycle |
| `legion-desktop/src/session.rs` | Session persistence |
| `legion-app/src/terminal_policy.rs` | Terminal policy |
| `legion-telemetry/src/lib.rs` | Atomic spool writes |

### Architectural Risks

1. **legion-app monolith** — 51k LOC in one crate, 34k in lib.rs. High coupling
   risk. AppComposition has ~250 methods — any new feature widens the surface.

2. **legion-protocol universal coupling** — 28/29 crates depend on it. A breaking
   change forces workspace-wide rebuilds and potential cascading fixes.

3. **Zero fan-in crates** — `legion-remote-transport`, `legion-retention`,
   `legion-telemetry`, `legion-vscode-compat` are not consumed by any other workspace
   crate. Verify they're wired via binaries/feature flags or are staged for future use.

4. **InMemoryStorage as primary backend** — `FileBackedStorage` exists but
   `InMemoryStorage` is the default. Open tabs, recent files, preferences are lost
   on restart unless explicitly persisted (session persistence was added in Phase 4).

5. **Pre-existing build issue** — `cargo check -p legion-app --no-default-features`
   fails due to unresolved `legion_sandbox` and `ProductChatCompletion` references
   when the "ai" feature is disabled.

## 10. Languages & Frameworks

| Component | Technology |
|-----------|-----------|
| Language | Rust 2024 edition, rust-version 1.92 |
| GUI framework | egui 0.34.2 / eframe 0.34.2 |
| Text buffer | Custom rope (legion-text) |
| Parsing | tree-sitter 0.26.9 — 10 grammars (Rust, Python, TypeScript, Go, C, JSON, TOML, Markdown, Bash, JavaScript) |
| Full-text search | Tantivy 0.26.1 |
| WASM runtime | Wasmtime 46.0.2 |
| TLS | rustls 0.23 (ring backend) |
| Async runtime | Tokio 1 (rt-multi-thread) |
| Secrets | OS keyring (keyring 3) |
| Encryption | ChaCha20-Poly1305 (legion-retention) |
| Signing | ed25519-dalek 2 |
| HTTP | reqwest 0.13 (rustls, blocking+json) |
| Serialization | serde + serde_json |
| CLI | clap 4.5 |
| OS interop | windows 0.62, nix 0.31, landlock 0.4 |

### Supported Syntax Languages (13 language IDs, 25+ extensions)
Rust, Python, TypeScript, JavaScript, Go, C, JSON, TOML, Markdown, Bash —
dispatched via `language_for_path()` in legion-index.

## 11. Module Ownership & Domain Boundaries

| Domain | Owner Crate(s) | Boundary Trait |
|--------|---------------|----------------|
| Text/Buffer | legion-text, legion-editor | `EditorPort` |
| Workspace/Files | legion-project | `WorkspacePort` |
| Code Intelligence | legion-index, legion-lsp | `SemanticPort`, `LspPort` |
| Security/Capabilities | legion-security | `CapabilityBrokerPort` |
| Storage/Persistence | legion-storage | `StorageRepositoryPort` |
| AI/Providers | legion-ai, legion-ai-providers | (internal to legion-app) |
| Agent Orchestration | legion-agent, legion-tracker | (internal to legion-app) |
| Terminal | legion-terminal, legion-platform | `TerminalPort` |
| Plugins | legion-plugin | `PluginPort` |
| UI/Shell | legion-ui | `CommandDispatchIntent` enum |
| Rendering | legion-desktop | (no port — consumes projections) |
| Proposals | legion-protocol | `ProposalPort` |
| Collaboration | legion-collaboration | (internal) |
| Remote Dev | legion-remote, legion-remote-transport | (internal) |
| Diagnostics | legion-observability | `EventSinkPort` |
| Debug | legion-debug | (internal) |
| Retention/Vault | legion-retention | (internal) |
| Telemetry | legion-telemetry | (internal) |

## 12. Conventions

### Error Handling
- **Library crates**: `thiserror::Error` enums with typed variants — never `anyhow`
- **Binary entry points**: `anyhow::Result` only in `fn main()`
- **Local aliases**: `type StorageResult<T> = Result<T, StorageError>`
- **Fail-closed idiom**: denials are explicit enum returns, not `Err` paths

### Naming
- `*Projection` — UI read-model (display-safe, pre-formatted)
- `*Record` — persisted data
- `*Descriptor` — declarative request payload
- `*Port` — trait boundary between crates
- `Legion*` prefix — cross-cutting workflow concepts

### Code Quality
- `#![warn(missing_docs)]` on every crate — all public items documented
- `RedactionHint` fields on protocol/storage/UI DTOs — display-safety is first-class
- Atomic writes everywhere — temp file → fsync → platform-specific rename
- `validate → durable write → memory` ordering invariant in storage

### Platform Abstraction
- Trait-per-concern: `FileSystemService`, `ProcessService`, `PtyService`, `WatcherService`
- Zero-sized `Native*` implementations (`Default + Clone + Copy`)
- Windows-specific paths via `#[cfg(windows)]` (ConPTY, MoveFileExW)

## 13. Setup & Build

```bash
# Prerequisites
rustup default 1.92   # or newer
# Windows: Visual Studio Build Tools with C++ workload

# Build
cargo build                          # debug build, all crates
cargo build -p legion-desktop        # desktop shell only
cargo build --profile dev-full -p X  # full debuginfo for crate X

# Test
cargo test                           # all tests
cargo test -p legion-app             # app crate only
cargo test -p legion-desktop --test user_journey_rendering  # view-model tests

# Lint
cargo fmt --check
cargo clippy --workspace

# xtask
cargo xtask golden-path-1            # GP-1 verification
cargo xtask check-deps               # dependency audit
cargo xtask docs-hygiene             # doc coverage check
cargo xtask release-pipeline         # release build

# Evidence gates
cargo run -p legion-cli -- evidence check --phase finish-phase1
cargo run -p legion-cli -- evidence check --phase finish-phase6
cargo run -p legion-cli -- doctor    # platform health check
```

## 14. Completion Status

All 6 finish phases are complete as of 2026-08-11:

| Phase | Name | Status |
|-------|------|--------|
| 1 | Syntax Highlighting | Complete — 10 grammars, 13 languages, 25+ extensions |
| 2 | Terminal Emulation | Complete — VT100/xterm, CSI/SGR/DEC, cell grid, keyboard |
| 3 | Live LSP Integration | Complete — diagnostics, completion, hover, go-to-def, inlay hints |
| 4 | Navigation & UI Essentials | Complete — file tree, find/replace, keybindings, settings, session persistence |
| 5 | Theme System & Visual Polish | Complete — dark/light tokens, tab polish, code minimap |
| 6 | Integration Testing & Release | Complete — view-model tests, GP-5, release workflow, acceptance proof |

### Deliberately Deferred
- Remote development server (client code is production-grade)
- Plugin marketplace / VS Code extension Node.js host
- Collaboration transport server
- Hosted telemetry backend
