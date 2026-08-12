# Legion IDE — Architecture Analysis

> Generated 2026-08-07 · 943 source files · 29 crates · ~196k LOC Rust

## 1. System Overview

Legion IDE is a proprietary, Rust-native code editor with integrated AI-assisted
development, multi-agent workflow orchestration, and defense-in-depth security.
The codebase is a Cargo workspace of 29 crates plus an `xtask` automation suite,
targeting Windows, macOS, and Linux.

**Key design principles:**
- **Projection-only UI** — the renderer never owns editor state; it reads
  snapshots and emits intents.
- **Proposal-mediated writes** — every file mutation flows through a
  proposal/risk-assessment pipeline before touching disk.
- **Metadata-only egress** — AI provider traffic carries fingerprints and byte
  counts, never raw source text.
- **Fail-closed security** — unknown capabilities are denied; sandboxes report
  honest enforcement gaps.
- **Hexagonal ports** — 11 service-port traits in `legion-protocol` define
  internal boundaries; implementations are injected, not imported.

## 2. Crate Dependency Graph

```
                          legion-protocol  (29k LOC — shared types, DTOs, port traits)
                                │
        ┌───────────┬───────────┼───────────┬───────────┬────────────┐
        ▼           ▼           ▼           ▼           ▼            ▼
   legion-text  legion-platform legion-security legion-storage legion-observability
     (rope)      (OS abstraction) (policy engine) (persistence)  (event sinks)
        │              │           │            │           │
        ▼              │           │            │           │
   legion-editor       │           │            │           │
     (buffers,         │           │            │           │
      undo/redo)       │           │            │           │
        │              │           │            │           │
        ├──────────────┼───────────┼────────────┼───────────┘
        ▼              ▼           ▼            ▼
   legion-index    legion-lsp  legion-plugin  legion-terminal
   (semantic idx)  (LSP runtime) (WASM host)  (PTY runtime)
        │              │           │            │
        ├──────────────┼───────────┼────────────┤
        ▼              ▼           ▼            ▼
                    legion-app  (50k LOC — composition root)
                        │
            ┌───────────┼───────────┐
            ▼           ▼           ▼
       legion-ai   legion-agent  legion-sandbox
       (provider   (workflow     (OS isolation)
        routing)    coordinator)
            │           │
            ▼           ▼
       legion-ai-providers
       (6 provider adapters + MCP)
                        │
                        ▼
                   legion-desktop  (25k LOC — egui shell)
                        │
                   legion-ui  (projection snapshots)
```

**Supporting crates:** `legion-collaboration` (operation log), `legion-remote` /
`legion-remote-transport` (remote dev + mTLS transport), `legion-memory`
(long-term context), `legion-tracker` (task/plan ledger), `legion-retention`
(encrypted source vault), `legion-telemetry` (hosted spool), `legion-vscode-compat`
(VSIX manifest normalization), `legion-debug` (DAP adapter), `legion-cli` (evidence
checker).

## 3. Layer Architecture

### Layer 0 — Protocol (`legion-protocol`)

The foundation crate: ~620 structs, ~270 enums, 11 port traits, ~80 validators.
Zero runtime behavior — pure types and validation functions. Every other crate
imports it; it imports only `serde`, `uuid`, and `thiserror`.

**Port traits** define the hexagonal service boundaries: `WorkspacePort`,
`EditorPort`, `ProposalPort`, `TerminalPort`, `LspPort`, `SemanticPort`,
`CapabilityBrokerPort`, `EventSinkPort`, `StorageRepositoryPort`, `PluginPort`,
`ProjectInfoPort`.

**Core domain types:** `WorkspaceProposal` (13 payload variants), `ProposalLifecycleState`
(12 states), `ViewportProjection` (the richest struct — buffers, cursors, selections,
scroll, overlays, large-file status), `LegionToolKind` (7 tool types), `RiskRuleId`
(7 deterministic rules), `ApprovalLevel` (Auto/Ask/RequireExplicit/Deny).

### Layer 1 — Primitives

| Crate | LOC | Purpose |
|-------|-----|---------|
| `legion-text` | 2.6k | Rope-backed `TextBuffer` (ropey), immutable `TextSnapshot`, `LineIndex` with UTF-8/UTF-16 conversion, chunk materialization (64 KiB SHA-256), binary detection |
| `legion-platform` | 2.9k | OS abstraction: filesystem, process, watcher, PTY, environment, time; Windows ConPTY parity |
| `legion-security` | 4.8k | `SecurityPolicy` + `DenyByDefaultBroker`: capability-prefix routing (`ai.`, `fs.`, `terminal.`, `plugin.`, `network.`, `remote.`, `cloud.`, `telemetry.`, `retention.*`), trust-gated decisions, `OrgPolicyBundle` (signed admin policy), `ProposalApplyGate`, `DeterministicRiskRuleEngine`, secret scanning |
| `legion-storage` | 6.3k | Workspace metadata, trust persistence, OS keyring secrets (`BYOK`), durable checkpoint store for rollback, plan revision ledger |
| `legion-observability` | 4.3k | Tracing, metrics, event log, performance counters; metadata-only redaction; rejects zero correlation/causality/sequence |

### Layer 2 — Editor Infrastructure

| Crate | LOC | Purpose |
|-------|-----|---------|
| `legion-editor` | 4.1k | Multi-buffer engine: transactional edits, undo/redo (snapshot-restore), save lifecycle, snapshot leasing, viewport projection, LCS line diff |
| `legion-index` | 7.0k | Semantic indexing engine: tree-sitter parsing, lexical symbol maps, fuzzy scoring, structural search/rewrite, deterministic parser-cache fallbacks |
| `legion-lsp` | 4.6k | JSON-RPC framing, supervised LSP lifecycle, diagnostics/completion/hover/code-action projections |
| `legion-plugin` | 1.4k | WASM plugin host (wasmtime): manifest validation, capability/quota metadata enforcement, fail-closed responses |
| `legion-terminal` | 2.7k | PTY runtime: policy-gated launch (`TerminalLaunchPolicyContract`), command taxonomy classification, output redaction, ConPTY/grid/OSC parsing |
| `legion-debug` | 2.3k | DAP client: adapter resolution, JSON framing, live debug sessions, evidence extraction |
| `legion-project` | 9.0k | Workspace model: file tree, trust-aware VFS resolution, file watcher, tantivy-backed search |

### Layer 3 — AI & Agent

| Crate | LOC | Purpose |
|-------|-----|---------|
| `legion-ai` | 2.9k | Provider-agnostic orchestration: `ModelProvider` trait, `ProviderRouter` (policy-bound, metadata-only responses), `ContextManifestRecord` assembly, redaction, markdown streaming, tool-calling protocol |
| `legion-ai-providers` | 5.9k | 6 provider adapters (deterministic-local, ollama, llama-cpp, openai, openai-compatible, anthropic) with activation tiers (LocalDefault → ByokConsentRequired → HostedDenied), MCP client (stdio + streamable-HTTP), Anthropic SSE streaming |
| `legion-agent` | 6.6k | DAG-scheduled workflow engine: `WorkflowDag` from approved plans, `LegionWorkflowCoordinator` (worker lifecycle, dependency-cycle detection, conflict detection), `parallel_worker_lanes()` scheduler, synchronous tool-use loop with budget, evidence extraction (SHA-256 hashes), scope enforcement, worktree sandbox orchestration |
| `legion-sandbox` | 2.0k | OS-level process isolation: macOS Seatbelt (SBPL), Linux Landlock v5 + bubblewrap, Windows job objects; honest enforcement reporting with caveat labels |

### Layer 4 — Application Composition (`legion-app`, 50k LOC)

The composition root that wires all layers together. `AppComposition` owns the
workspace, editor engine, proposal pipeline, language tooling, terminal runtime,
debug adapter, AI routing, and agent coordinator.

**Key modules:**
- `language/` — rust-analyzer lifecycle, download policy, document sync, edit
  proposal routing, stderr redaction
- `proposal.rs` — multi-file diff computation, partial acceptance, hunk
  disposition, risk rule evaluation
- `terminal_policy.rs` — scrollback limits, environment sanitization, deny
  prefixes
- `updater.rs` — auto-updater client (Ed25519 verification, staging, journal,
  rollback)
- `test_explorer.rs` — cargo test discovery, trust-gated execution
- `diagnostics.rs` — crash report assembly, metadata-only export
- `offline_ai.rs` — offline-mode AI stub (compiled when `ai` feature is off)

### Layer 5 — Desktop Shell

| Crate | LOC | Purpose |
|-------|-----|---------|
| `legion-ui` | 8.8k | Projection-only shell: `Shell` + `ShellProjectionSnapshot` (~40 fields covering every UI panel), `CommandDispatchIntent` intents, `DockLayout` panel registry, `PaletteProjection`, `TestExplorerProjection`, workflow board/fleet card projections |
| `legion-desktop` | 25k | egui/eframe renderer: `DesktopCommandBridge` (~120 `DesktopAction` variants → validated intents), `ProjectionView` rendering pipeline, token-based theme system (dark/light with 12 background slots, 8 accent colors, CJK fallback), view submodules for code canvas, assistant rail, ghost text, inline edit, plan editor, proposal review, fleet board, sandbox panel, terminal panel, worker panel |

### Supporting Crates

| Crate | LOC | Purpose |
|-------|-----|---------|
| `legion-collaboration` | 1.8k | Deterministic operation log for collaborative editing: document bindings, version vectors, causal gap detection, presence projection |
| `legion-remote` | 2.7k | Remote development runtime: workspace lifecycle, filesystem operations, cloud lane tasks, offline resume |
| `legion-remote-transport` | 2.0k | mTLS transport carriers: certificate management, flow control, replay windows, connection health |
| `legion-memory` | 1.3k | Opt-in long-term memory: embedding references, consent-gated retention, snapshot schema versioning |
| `legion-tracker` | 0.5k | Metadata ledger: agent runs, workflow sessions, plan approvals, verification gates |
| `legion-retention` | 2.9k | Encrypted raw-source vault: ChaCha20-Poly1305 at-rest encryption, key rotation, tombstone lifecycle, consent-gated access audit |
| `legion-telemetry` | 1.3k | Hosted telemetry spool: consent-gated export batches, durable local spool |
| `legion-vscode-compat` | 1.0k | VSIX manifest normalization to protocol DTOs; no Node.js execution |
| `legion-cli` | 1.6k | Evidence checker CLI: phase gate validation, evidence TOML parsing |

## 4. Key Data Flows

### Save Flow
```
User edit → EditorEngine::apply_edit (transactional, snapshot for undo)
  → AppComposition::save_active_buffer
    → SaveWorkflowService
      → WorkspaceActor::save_file_with_proposal
        → ProposalPort (risk assessment, fingerprint preconditions)
          → DeterministicRiskRuleEngine (7 rules → ApprovalLevel)
            → SecurityPolicy::decide (capability gate)
              → Disk write (atomic, fail-closed on non-atomic fallback)
```

### AI Assist Flow
```
User trigger → ProviderRouter::route
  → CapabilityBrokerPort::decide (trust + consent + provider class)
    → ContextManifestRecord assembly (privacy labels, egress tracking)
      → redact_model_bound_output (secret scanning)
        → ModelProvider::complete (metadata-only response fingerprint)
          → Proposal creation (inline edit or delegated task)
```

### Delegated Task Flow
```
Approved plan → WorkflowDag (dependency graph)
  → LegionWorkflowCoordinator::next_ready_workers
    → parallel_worker_lanes (scheduler)
      → DelegatedTaskSandboxOrchestrator (worktree allocation)
        → SandboxEnforcementReport (honest enforcement)
          → Agent loop (synchronous tool-use, budgeted)
            → Evidence extraction (SHA-256 hashed)
              → Proposal recording (metadata-redacted)
```

## 5. Security Architecture

**Four-layer defense in depth:**

1. **Policy layer** (`legion-security`) — `DenyByDefaultBroker` evaluates every
   `CapabilityRequest` against `SecurityPolicy`. Three-state workspace trust
   (Trusted/Untrusted/Unknown). Untrusted workspaces are denied terminal, file
   write, network, plugin, LSP, and AI operations. `OrgPolicyBundle` adds signed
   admin-distributable policy with a `ProductMode` ceiling
   (Manual < Assist < Delegates < Automate < LegionWorkflows).

2. **Risk assessment** (`legion-protocol` + `legion-security`) —
   `DeterministicRiskRuleEngine` evaluates 7 rule IDs: path-scope escape, file
   count, deletion ratio, dependency/lockfile touch, migration proximity, secrets
   proximity, binary changes. Graduated approval ladder with vacuous-truth guard.

3. **Sandbox layer** (`legion-sandbox`) — OS-level process containment with
   platform-specific backends and honest enforcement gap reporting.

4. **Runtime enforcement** (`legion-terminal`, `legion-agent`) — command
   taxonomy classification, output redaction (API keys, bearer tokens, GitHub/Slack
   tokens), per-session timeout/size limits, scope enforcement for tool calls.

**Secret scanning:** `scan_payload_for_sensitive_markers` detects PEM keys, API
keys, bearer tokens, and provider-prefixed credentials in trace/diff/log payloads
before retention or export.

## 6. Test Coverage Map

| Crate | Source Files | Test Files | Ratio |
|-------|-------------|------------|-------|
| `legion-app` | 21 | 44 | 2.1:1 |
| `legion-desktop` | 35 | 55 | 1.6:1 |
| `legion-agent` | 15 | 13 | 0.9:1 |
| `legion-protocol` | 9 | 7 | 0.8:1 |
| `legion-lsp` | 4 | 12 | 3.0:1 |
| `legion-project` | 1 | 9 | 9.0:1 |
| `legion-security` | 3 | 8 | 2.7:1 |
| `legion-ai-providers` | 3 | 5 | 1.7:1 |
| `legion-ai` | 7 | 6 | 0.9:1 |
| `legion-terminal` | 5 | 6 | 1.2:1 |
| `legion-editor` | 2 | 4 | 2.0:1 |
| **Total** | **157** | **193** | **1.2:1** |

**Golden-path smokes** (GP-1 through GP-4) exercise end-to-end product flows:
fixture edit/LSP/git (GP-1), scope/sandbox/kill-switch with AI-assist routing
(GP-2), delegate task loop with scope denial and evidence TOML (GP-3), automate
multi-agent workflow evidence (GP-4).

**21 standing gates** run on every PR via `xtask`: dependency policy, docs
hygiene, claim audit, no-egui-textedit, release pipeline, format, check, test,
clippy, cargo-deny, rust-analyzer smoke, golden paths 1–4, perf harness, update
drill, kanban backlog, readiness consistency.

## 7. CI/CD Pipeline

| Workflow | Trigger | Matrix | Status |
|----------|---------|--------|--------|
| `legion-gates.yml` | push to main, PRs | ubuntu/windows/macos | Merge-blocking |
| `legion-smoke.yml` | weekly + dispatch | ubuntu/windows/macos | Independent (non-blocking) |
| `legion-bench.yml` | weekly | ubuntu | Recorded-mode synthetic scoring |
| `legion-preview.yml` | manual | ubuntu/windows/macos | Unsigned beta preview builds |

## 8. Configuration & Environment

- **Rust edition 2024**, MSRV 1.92, resolver v3
- **Build profile:** `dev` uses `line-tables-only` debug info (100GB PDB
  mitigation); `dev-full` profile available for variable-level debugging
- **Feature flags:** `ai` (default on `legion-app`; `offline` disables hosted
  provider calls), `test-helpers` (test seams on `AppComposition`)
- **Workers config:** `config/workers.example.yaml`
- **Release pipeline:** `xtask/release-pipeline.example.toml` → TOML descriptors +
  `version_stamp.toml` + `release-manifest.v1.toml` (Ed25519 signed)

## 9. Risk Hotspots

| File | LOC | Risk |
|------|-----|------|
| `legion-app/src/lib.rs` | ~1,839+ | Composition root with 30-crate fan-in; high coupling surface |
| `legion-protocol/src/lib.rs` | ~27k | Monolithic types file; every crate breaks when this changes |
| `legion-desktop/src/view.rs` | 8,547 | Largest single view file; rendering bottleneck |
| `legion-desktop/src/workflow.rs` | 5,035 | Desktop runtime orchestration; complex state machine |
| `legion-ai-providers/src/lib.rs` | 5,721 | 6 provider adapters in one file; network boundary code |
| `legion-desktop/src/bridge.rs` | 3,105 | ~120 action variants; validation surface |
| `legion-app/tests/workspace_vfs_integration.rs` | 5,408 | Largest test file; long test cycles |

## 10. Languages & Frameworks

- **Primary:** Rust (374 files, ~196k LOC)
- **UI framework:** egui/eframe 0.34
- **Text engine:** ropey (rope data structure)
- **Parsing:** tree-sitter 0.26 (Rust grammar)
- **Search:** tantivy 0.26 (full-text), globset (glob matching)
- **Plugin runtime:** wasmtime 46
- **Crypto:** ChaCha20-Poly1305 (retention vault), Ed25519 (updater/release signing)
- **TLS:** rustls 0.23 (remote transport)
- **Keyring:** OS-native via keyring 3
- **Testing:** Python pytest (evals/training, local-only), Rust native tests
- **CI:** GitHub Actions (3-OS matrix)

## 11. Module Ownership & Domain Boundaries

| Domain | Owner Crate(s) | Boundary |
|--------|---------------|----------|
| Text editing | `legion-text`, `legion-editor` | `EditorPort` trait |
| Workspace & files | `legion-project`, `legion-storage` | `WorkspacePort`, `StorageRepositoryPort` |
| Proposals & risk | `legion-app/proposal`, `legion-security` | `ProposalPort`, `CapabilityBrokerPort` |
| Language tooling | `legion-lsp`, `legion-index` | `LspPort`, `SemanticPort` |
| AI integration | `legion-ai`, `legion-ai-providers` | `ModelProvider` trait, `ProviderRouter` |
| Agent workflows | `legion-agent` | `LegionWorkflowCoordinator` |
| Terminal | `legion-terminal` | `TerminalPort` |
| Plugin system | `legion-plugin` | `PluginPort` |
| Debugging | `legion-debug` | DAP framing layer |
| UI rendering | `legion-ui`, `legion-desktop` | `ShellProjectionSnapshot` (read-only) |
| Security | `legion-security`, `legion-sandbox` | `DenyByDefaultBroker`, `SandboxEnforcementReport` |
| Collaboration | `legion-collaboration` | Operation log runtime |
| Remote dev | `legion-remote`, `legion-remote-transport` | mTLS transport carriers |

## 12. Setup & Build

```bash
# Prerequisites: Rust 1.92+, platform GUI libs (see legion-gates.yml for Linux packages)

# Build
cargo build --workspace

# Test (all crates, all targets)
cargo test --workspace --all-targets

# Run standing gates
cargo run -p xtask -- check-deps
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check

# Run golden-path smokes
cargo run -p xtask -- golden-path-1
cargo run -p xtask -- golden-path-2
cargo run -p xtask -- golden-path-3
cargo run -p xtask -- golden-path-4

# Run the CLI shell (proof-of-concept)
cargo run -p legion-app -- <path>

# Run the desktop app
cargo run -p legion-desktop

# Full debug info for a specific crate
cargo build --profile dev-full -p <crate>
```
