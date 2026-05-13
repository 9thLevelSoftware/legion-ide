# Foundational Core IDE Platform Implementation Plan v0.1

## Objective

Transform the roadmap in [`plans/foundational-core-ide-platform-roadmap-v0.1.md`](plans/foundational-core-ide-platform-roadmap-v0.1.md) into a handoff-ready technical implementation specification for a standalone, deterministic, low-latency core IDE. This plan preserves local-first operation, no AI runtime dependency, protocol-mediated mutation, trust-gated execution, and phased LSP/plugin introduction.

## Context Reviewed

- [`plans/foundational-core-ide-platform-roadmap-v0.1.md`](plans/foundational-core-ide-platform-roadmap-v0.1.md): roadmap purpose, assumptions, non-goals, phases, milestones, risk register, validation matrix, and stop conditions.
- [`plans/ide-core-architecture-spec-v0.1.md`](plans/ide-core-architecture-spec-v0.1.md): system layers, workspace/VFS/editor/LSP/plugin ownership, data flows, required protocol contracts, validation strategy, implementation gaps, and sequencing constraints.
- [`Cargo.toml`](../Cargo.toml): current Rust 2024 workspace members and shared dependencies.
- [`plans/dependency-policy.md`](plans/dependency-policy.md) and [`xtask/src/main.rs`](../xtask/src/main.rs): dependency direction and protocol-symbol enforcement.
- [`crates/devil-protocol/src/lib.rs`](../crates/devil-protocol/src/lib.rs), [`crates/devil-text/src/lib.rs`](../crates/devil-text/src/lib.rs), [`crates/devil-editor/src/lib.rs`](../crates/devil-editor/src/lib.rs), [`crates/devil-platform/src/lib.rs`](../crates/devil-platform/src/lib.rs), [`crates/devil-project/src/lib.rs`](../crates/devil-project/src/lib.rs), [`crates/devil-ui/src/ui.rs`](../crates/devil-ui/src/ui.rs), and [`crates/devil-app/src/main.rs`](../crates/devil-app/src/main.rs): current spike/scaffold implementation state.
- [`plans/architecture-freeze-v0.1.md`](plans/architecture-freeze-v0.1.md), [`plans/milestone-0-feasibility-proofs.md`](plans/milestone-0-feasibility-proofs.md), [`plans/SPIKE-001A-native-shell-proof.md`](plans/SPIKE-001A-native-shell-proof.md), [`plans/spikes/SPIKE-001A-result.md`](plans/spikes/SPIKE-001A-result.md), and [`plans/SPIKE-000-platform-boundary-proof.md`](plans/SPIKE-000-platform-boundary-proof.md): Phase 0 proof and freeze gates.

## Current Architecture Summary

The repository is already a Rust 2024 multi-crate workspace. The current implementation is spike-level: protocol contracts are minimal, the text model is string-backed, the editor is a single-session model, platform helpers perform direct open/save, workspace/project logic is scaffolded, and app startup currently bypasses the planned workspace/VFS proposal path.

The target architecture is actor-oriented and policy-scoped. UI/editor input remains latency-critical. Workspace scanning, file watching, LSP, terminal, persistence, and plugin work must use bounded async queues, cancellation, policy checks, and observable event envelopes. Editor owns buffers and transactions; workspace owns identity, trust, tree, and VFS; platform owns OS abstractions only; LSP and plugin systems consume protocol contracts and capability services instead of UI or internal state.

## Assumptions and Non-Goals

### Assumptions

1. Windows is the primary delivery platform until parity is validated.
2. New runtime crates for LSP, terminal, and plugins are ADR-gated.
3. Every non-user direct mutation enters versioned proposal validation and workspace VFS authority.
4. Existing spike code may be replaced where a phase defines production ownership.
5. CI runs dependency, format, check, test, and clippy gates at every phase exit.

### Non-Goals

- No VS Code compatibility layer, generalized marketplace, remote collaboration, hosted dependency, or autonomous AI execution.
- No semantic indexing, embeddings, or vector retrieval in this foundational roadmap.
- No unrestricted third-party plugin runtime in V1.

## Global Critical Path

1. Close freeze and evidence gates.
2. Expand protocol contracts before consumers.
3. Implement platform traits and security policy before workspace/VFS.
4. Implement workspace identity, trust, tree, and watcher before editor save semantics.
5. Implement production text model before LSP sync and plugin context providers.
6. Complete save/conflict/proposal lifecycle before LSP code actions, terminal commands, plugin edits, or future AI seams.
7. Add observability and replay gates before broad extensibility.

## Phase Blueprint

### Phase 0 - Freeze Closure and Evidence Baseline

Goal: convert spike state into an accepted implementation baseline.

Tasks:
1. Run [`cargo run -p xtask -- check-deps`](../xtask/src/main.rs) and archive output.
2. Produce protocol gap list for workspace IDs, file identity, editor versions, proposals, terminal DTOs, LSP DTOs, plugin manifests, and event envelopes.
3. Execute native shell proof for input-to-paint latency, frame variance, CPU/GPU utilization, memory growth, IME, clipboard, accessibility, and focus behavior.
4. Collect text/index stress metrics for edit throughput, latency under load, snapshot memory growth, rollback cycles, and index lag.
5. Complete platform boundary proof by mapping each [`devil-platform`](../crates/devil-platform/src/lib.rs) API to OS-only ownership.
6. Update [`plans/spikes/SPIKE-001A-result.md`](plans/spikes/SPIKE-001A-result.md) with PASS, PASS WITH RESERVATIONS, or FAIL plus evidence owners and fallback criteria.

Exit criteria: M0 accepted; no unresolved blocker is treated as cleared.

QA: existing unit tests, dependency check, build/check/test/clippy, shell latency proof, text stress proof.

### Phase 1 - Protocol Contract Expansion

Goal: establish serializable, versioned contracts before subsystem implementation.

Tasks:
1. Add opaque identifiers and version wrappers in [`crates/devil-protocol/src/lib.rs`](../crates/devil-protocol/src/lib.rs) for workspace, root, file content, buffer version, snapshot, terminal session, proposal, correlation, language server, plugin, capability decision, and event sequence.
2. Add workspace DTOs for open/close, trust state, canonical path, file identity, file metadata, file tree node, tree delta, watcher event, config snapshot, and conflict state.
3. Add editor DTOs for buffer lifecycle, coordinate encodings, byte/UTF-16 offsets, ranges, edit batches, transaction source, transaction descriptor, undo group, overlays, diagnostics, completion requests, and completion items.
4. Add proposal DTOs for text edit, create, delete, rename, save, format, code action, and terminal command proposals with principal, capability, correlation, version preconditions, preview summary, and expiry.
5. Add LSP DTOs for server config/status, document sync, diagnostics, hover, completion, formatting, semantic tokens, symbol locations, and code actions.
6. Add terminal/plugin/capability DTOs for launch/output/input/resize/exit, manifests, activation events, capabilities, grants, denials, contributions, and context providers.
7. Add service-port traits or message enums for workspace, editor, proposal, terminal, LSP, capability broker, event sink, and storage repositories.
8. Extend [`plans/dependency-policy.md`](plans/dependency-policy.md) and [`xtask/src/main.rs`](../xtask/src/main.rs) to enforce new protocol boundaries.

Exit criteria: every persisted or cross-process DTO has serde round-trip tests and golden schema tests where stable.

QA: serialization tests, required-field tests, stale-version tests, coordinate tests, mock port compilation, dependency-policy gate.

### Phase 2 - Platform Services and Security Policy Foundations

Goal: replace spike helpers with OS-service traits and default-deny policies.

Tasks:
1. Replace direct helper-centric APIs in [`crates/devil-platform/src/lib.rs`](../crates/devil-platform/src/lib.rs) with filesystem, watcher, process, PTY, environment, path normalization, and time service traits.
2. Expand platform errors for permission denied, not found, encoding, symlink loop, path too long, atomic replace unsupported, watcher overflow, process spawn failure, PTY unavailable, timeout, and cancellation.
3. Implement trust state, path policy, command taxonomy, terminal policy, LSP launch policy, plugin capability policy, file-write policy, network policy, and deny-by-default broker stub in [`crates/devil-security/src/lib.rs`](../crates/devil-security/src/lib.rs).
4. Remove app-facing raw [`open_text_file`](../crates/devil-platform/src/lib.rs) and [`save_text_file`](../crates/devil-platform/src/lib.rs) usage from [`crates/devil-app/src/main.rs`](../crates/devil-app/src/main.rs), replacing it with workspace/VFS port composition stubs.
5. Update dependency policy so platform never depends on editor/project/UI/security domain logic.
6. Update platform boundary proof after refactor.

Exit criteria: no direct caller bypasses typed traits for filesystem/process calls; deny/allow policy matrix tests pass.

QA: fake filesystem/process/PTY services, policy matrix tests, app composition tests, static search for raw helper usage, canonicalize/read/list/hash latency budget.

### Phase 3 - Workspace VFS and Tree Foundation

Goal: implement trusted workspace ownership, identity, shallow tree, watcher intake, and initial persistence.

Tasks:
1. Add workspace actor state in [`crates/devil-project/src/lib.rs`](../crates/devil-project/src/lib.rs): workspace ID, generation, root, trust, config snapshot, file ID map, tree, watcher, and session state.
2. Add VFS resolver that canonicalizes paths, enforces root boundaries and trust policy, maps paths to file IDs, records fingerprints, and returns protocol file metadata.
3. Implement shallow discovery with ignore, hidden, generated, binary, large-file, and unreadable flags.
4. Implement watcher debounce, stable rename correlation, overflow marker, bounded rescan, and recovery state.
5. Add storage repositories in [`crates/devil-storage/src/lib.rs`](../crates/devil-storage/src/lib.rs) for recent workspaces, trust decisions, metadata, and sessions.
6. Add explorer projection types in [`crates/devil-ui/src/ui.rs`](../crates/devil-ui/src/ui.rs) without direct filesystem ownership.
7. Wire workspace actor, platform service, security service, storage, and UI projection in [`crates/devil-app/src/main.rs`](../crates/devil-app/src/main.rs).

Exit criteria: trusted workspace open, shallow tree, trust toggles, and watcher updates work under bounded rescan.

QA: path policy tests, symlink/long-path tests, watcher overflow tests, storage migration/corruption tests, open-workspace integration test, first-tree-projection performance budget.

### Phase 4 - Production Text Model and Editor Transactions

Goal: replace string-backed text with scalable rope or piece table and deterministic multi-buffer transactions.

Tasks:
1. Replace [`TextBuffer`](../crates/devil-text/src/lib.rs) internals with rope or piece-table storage.
2. Add immutable snapshot descriptors with snapshot ID, buffer version, content hash, length, line count, memory estimate, and retention pin reason.
3. Implement line index with byte, UTF-8, and UTF-16 conversions, CRLF handling, surrogate-pair-sensitive LSP mapping, and invalid conversion reporting.
4. Replace single [`EditorSession`](../crates/devil-editor/src/lib.rs) with multi-buffer editor engine, lifecycle, versions, dirty state, transaction groups, undo/redo groups, selections, cursors, overlays, and snapshot retention budget.
5. Expand transaction pipeline with pre/post snapshots, transaction/source IDs, buffer/file/workspace IDs, changed byte and UTF-16 ranges, undo group, timestamp, and causality trace.
6. Replace direct persistence snapshot flow with save request DTO emission to workspace/proposal ports.
7. Add large-file edit, undo/redo, snapshot retention, and mixed workload benchmarks.

Exit criteria: deterministic transaction logs include pre/post snapshots and causality IDs; editor performs no raw writes.

QA: property tests against reference string model, UTF-16 golden tests, undo/redo invariants, rollback tests, large-file latency and memory benchmarks.

### Phase 5 - Multi-Tab UI Shell and Session Restore

Goal: build production shell projections for explorer, tabs, editor viewport, panels, status, command palette, and session restore.

Tasks:
1. Replace spike [`Shell`](../crates/devil-ui/src/ui.rs) with projection-only shell state.
2. Add tab groups, file/buffer binding, dirty indicators, pinned/preview flags, active tab, split metadata, and close/save prompts.
3. Replace direct command-to-editor mutation with protocol-port dispatch for open, close, save, split, reveal, and search.
4. Persist open tabs, active buffer, layout, explorer expansion, panel state, and last workspace in [`crates/devil-storage/src/lib.rs`](../crates/devil-storage/src/lib.rs).
5. Load session after trust check and persist session on shutdown/stable changes.
6. Re-run production UI latency checks.

Exit criteria: multi-tab workflow operates without UI owning workspace/editor internals; restart restores expected layout and context.

QA: tab/projection/session tests, open/save/close/restart integration, focus/resize/tab switch/render latency checks.

### Phase 6 - Save Pipeline, Conflict State, and Proposal Lifecycle

Goal: make every durable mutation explicit, versioned, previewable, auditable, and safe.

Tasks:
1. Implement save pipeline in [`crates/devil-project/src/lib.rs`](../crates/devil-project/src/lib.rs): resolve file ID, verify capability, compare fingerprint, temp-write, flush, atomic replace, fallback, metadata/hash update, workspace generation update, event emission.
2. Add editor conflict states: clean, dirty, saving, save failed, disk changed clean, conflict dirty, reload available, keep-both pending, compare pending.
3. Ensure proposal DTOs carry buffer version, file content version, workspace generation, trust decision, required capability, principal, correlation ID, preview summary, and diagnostics.
4. Apply open-buffer edits through editor transactions and closed-file mutations through VFS, with shared proposal audit metadata and rollback on batch failure.
5. Add proposal preview UX state with affected files, diff summary, warnings, approve/reject/cancel, and post-apply result.
6. Add observability events for proposal created, validated, approved, rejected, applied, failed, rolled back, and conflict.

Exit criteria: all mutation paths emit transaction metadata and proposal audit logs; stale proposals fail closed.

QA: fingerprint/conflict/proposal validation tests, save/conflict/preview/apply integration, atomic fallback tests, save latency and preview generation benchmarks.

### Phase 7 - Integrated Terminal Emulation

Goal: introduce terminal as policy-gated, cancellable adjacent core service with bounded transcript.

Tasks:
1. Finalize terminal ADR before creating [`crates/devil-terminal/Cargo.toml`](../crates/devil-terminal/Cargo.toml).
2. If approved, add terminal actor, session registry, bounded output ring buffer, input/resize/cancel messages, shell profile, environment sanitization, and transcript limits.
3. Implement Windows-primary PTY/process adapter through platform traits.
4. Enforce command classes, environment restrictions, untrusted workspace denial, dangerous escalation, timeout, working-directory boundary, and output limits.
5. Add terminal panel projection with tabs, output viewport, scrollback/search, running/stopped indicators, kill/restart, and denied state.
6. Emit terminal observability events with bounded/redacted transcript metadata.

Exit criteria: terminal cannot bypass trust, command, environment, timeout, cancellation, or transcript policy.

QA: terminal actor tests, command classifier tests, PTY/process smoke tests, denied launch tests, output flood memory test, cancellation latency test.

### Phase 8 - LSP Coordinator and Feature Delivery

Goal: add language services through normalized DTOs, stale-response suppression, overlays, and proposal-only mutation.

Tasks:
1. After ADR-0011 acceptance, create [`crates/devil-lsp/Cargo.toml`](../crates/devil-lsp/Cargo.toml) and [`crates/devil-lsp/src/lib.rs`](../crates/devil-lsp/src/lib.rs) with dependencies only on protocol, platform, security, observability, and allowed runtime/serialization crates.
2. Implement LSP coordinator, per-language runtime config, server route table, lifecycle state, bounded restart policy, capability cache, and trust-gated launch.
3. Implement JSON-RPC process supervision, framed IO, request IDs, cancellation tokens, timeout policy, and crash handling.
4. Implement document sync: didOpen/didChange/didSave/didClose, per-buffer version mapping, incremental sync when safe, full sync fallback when ambiguous, and debounce that never delays editor input.
5. Normalize diagnostics, completion, hover, signature help, definitions, references, rename, formatting, semantic tokens, and code actions into protocol DTOs.
6. Add priority lanes and cancellation for stale completion/hover/semantic/code-action requests.
7. Convert formatting, rename, and code actions into workspace edit proposals.
8. Add UI status, problems, completion, hover, go-to, and code-action preview projections.

Exit criteria: trusted workspace can start LSP; untrusted cannot; diagnostics/completion/hover/format/code action work through overlays and proposals; stale responses never apply.

LSP integration strategy:
- Server discovery uses built-in defaults plus trusted workspace settings only.
- Transport is JSON-RPC over supervised child processes through platform process traits.
- Sync policy uses per-buffer monotonic versions, incremental sync when safe, and full sync fallback for ambiguity.
- Mutations from format, rename, and code actions always become workspace edit proposals.
- Backpressure uses bounded queues, cancellation tokens, priority lanes, and stale-response suppression.
- Security blocks launch in untrusted workspaces and routes all process starts through command policy.

QA: document sync tests, UTF-16 fallback tests, fake LSP server tests, Rust Analyzer smoke test, stale response tests, proposal bridge tests, completion/diagnostic latency benchmarks.

### Phase 9 - Observability, Replay, and Quality Hardening

Goal: make behavior measurable, privacy-aware, replayable, and CI-enforced.

Tasks:
1. Implement event envelopes in [`crates/devil-observability/src/lib.rs`](../crates/devil-observability/src/lib.rs) with event ID, parent ID, causality chain, subsystem, severity, retention label, redaction hints, timestamps, and schema version.
2. Add latency histograms for open, scan, edit, render, save, proposal, LSP, terminal, plugin activation, and storage migration.
3. Persist event metadata with retention, compaction, corruption detection, and repair in [`crates/devil-storage/src/lib.rs`](../crates/devil-storage/src/lib.rs).
4. Add CLI diagnostics for dependency graph, protocol symbols, event summary, performance summary, storage health, and replay drill.
5. Add CI gates in [`xtask/src/main.rs`](../xtask/src/main.rs) for performance thresholds, event envelope coverage, protocol golden schemas, and storage migration validation.
6. Run replay drills for open workspace, edit/save, conflict, terminal denial, LSP completion, and code action proposal.

Exit criteria: performance budgets and corruption recovery are enforced in CI; replay works from metadata without full source snapshots by default.

QA: event/redaction/metrics tests, replay drills, CLI integration tests, storage corruption tests, event overhead and replay query benchmarks.

### Phase 10 - Plugin and Future AI-Seams Completion

Goal: add controlled extensibility after core stability while preserving non-AI coupling and proposal-only mutation.

Tasks:
1. After ADR-0012 acceptance, create [`crates/devil-plugin/Cargo.toml`](../crates/devil-plugin/Cargo.toml) and [`crates/devil-plugin/src/lib.rs`](../crates/devil-plugin/src/lib.rs) for first-party/trusted plugins only.
2. Implement plugin manifest parser/validator with ID, version, compatibility, activation events, contributions, requested capabilities, storage namespace, and checksum/source metadata.
3. Implement registry discovery, deterministic activation, lifecycle states, cancellation, deactivation, and failure isolation.
4. Replace broker stub with production grant/deny/prompt decisions for workspace read/write, editor read/write, process, terminal, network, UI contribution, and storage.
5. Implement declarative contribution registry for commands, menus, keybindings, status items, panels, decorations, snippets, themes, language definitions, formatters, LSP registrations, scanners, and context providers.
6. Implement passive context providers with sensitivity labels, retention hints, scope, version, and no mutation capability.
7. Convert plugin edits/file/process/terminal actions into capability requests and proposals.
8. Add plugin-scoped storage namespaces, quotas, migrations, clear/reset, and cross-plugin isolation.

Exit criteria: plugins consume protocol services and cannot mutate directly; manifest validation and capability grants/denials pass.

Extensibility framework strategy:
- V1 runtime is first-party/trusted only.
- Manifest-declared capabilities are default-deny and mediated by the capability broker.
- Contributions are declarative and removable on deactivation.
- Plugin writes become proposals validated by editor/workspace versions.
- Future AI is a protocol client, not a privileged subsystem.

QA: manifest tests, capability matrix tests, contribution registry tests, plugin command integration, denied capability audit, plugin storage isolation, activation latency and plugin-bus backpressure benchmarks.

## Phase Transition Gates

| Transition | Entry Requirement | Exit Requirement | Stop Condition |
|---|---|---|---|
| Phase 0 to 1 | Freeze evidence complete | M0 accepted | Any unresolved freeze blocker. |
| Phase 1 to 2 | Protocol contracts approved | Serde/schema tests pass | Missing protocol symbol or ambiguous version field. |
| Phase 2 to 3 | Platform/security traits accepted | Raw helper bypass removed | Path/trust bypass. |
| Phase 3 to 4 | Workspace/VFS/trust/watcher usable | Open workspace/tree/watcher stable | Path escape or unrecovered watcher overflow. |
| Phase 4 to 5 | Multi-buffer editor stable | Transactions and budgets accepted | Unbounded snapshots or undo/redo failure. |
| Phase 5 to 6 | UI preview/session projection stable | Multi-tab restore works | UI directly mutates text/workspace. |
| Phase 6 to 7 | Proposal/save/conflict lifecycle complete | Mutation paths audited | Stale proposal applies or external overwrite clobbers. |
| Phase 7 to 8 | Terminal policy enforced | Terminal policy gate passes | Process launch without command policy. |
| Phase 8 to 9 | LSP normalized features stable | Proposal-only LSP mutation | Stale LSP response applies. |
| Phase 9 to 10 | Replay/perf gates in CI | Observability accepted | Missing event chain for core flows. |

## Risk Assessment Matrix

| Risk | Likelihood | Impact | Early Warning | Mitigation | Owner |
|---|---:|---:|---|---|---|
| File-system abstraction latency | Medium | High | Canonicalize/list/hash histograms exceed budget | Cache canonical roots, batch metadata, defer hashes, bounded workers | Platform + Project |
| Editor buffer memory growth | High | High | Snapshot memory grows unbounded | Retention budget, pin reasons, eviction, rope/piece-table benchmarks | Text + Editor |
| UTF-8/UTF-16 mismatch | Medium | High | Diagnostics/code actions target wrong ranges | Explicit coordinate types, golden tests, full-sync fallback | Text + LSP |
| Direct mutation leakage | Medium | Critical | Raw writes or mutation outside proposal path | Dependency policy, port boundaries, audit coverage, broker default-deny | App + Workspace + Security |
| Watcher overflow/identity loss | Medium | Medium | Duplicate IDs or frequent overflow | Bounded rescan, rename correlation, stable ID tests | Project + Platform |
| LSP hurts typing latency | High | High | Actor saturation or CPU spikes | Priority queues, cancellation, stale suppression, degradation | LSP + Runtime |
| Terminal policy bypass | Low | Critical | Shell launches in untrusted workspace | Central command policy and PTY through platform traits | Security + Terminal |
| Plugin host becomes unrestricted | Medium | Critical | Runtime gains direct filesystem/process/editor access | First-party only, manifest validation, capability broker | Plugin + Security |
| Storage corruption | Medium | High | Migration or metadata read failure | Namespace isolation, quarantine, repair tests | Storage |
| Observability leaks source | Medium | High | Event payload contains source text | Redaction hints, metadata-only retention, bounded transcripts | Observability + Security |
| Gate drift | Medium | High | New crate/dependency appears before ADR | Phase entry checks and dependency policy | Architecture + Freeze |
| Async deadlock/backpressure | Medium | High | Mailboxes saturate or shutdown hangs | Bounded channels, cancellation, timeout, shutdown drain order | Runtime + App |

## Required Commands After Every Phase

- [`cargo run -p xtask -- check-deps`](../xtask/src/main.rs)
- [`cargo fmt --all`](../Cargo.toml)
- [`cargo check --workspace --all-targets`](../Cargo.toml)
- [`cargo test --workspace --all-targets`](../Cargo.toml)
- [`cargo clippy --workspace --all-targets -- -D warnings`](../Cargo.toml)

## Definitive File Checklist

### Existing files to edit

- [`Cargo.toml`](../Cargo.toml)
- [`plans/dependency-policy.md`](plans/dependency-policy.md)
- [`xtask/src/main.rs`](../xtask/src/main.rs)
- [`crates/devil-protocol/src/lib.rs`](../crates/devil-protocol/src/lib.rs)
- [`crates/devil-platform/src/lib.rs`](../crates/devil-platform/src/lib.rs)
- [`crates/devil-security/src/lib.rs`](../crates/devil-security/src/lib.rs)
- [`crates/devil-project/src/lib.rs`](../crates/devil-project/src/lib.rs)
- [`crates/devil-storage/src/lib.rs`](../crates/devil-storage/src/lib.rs)
- [`crates/devil-text/src/lib.rs`](../crates/devil-text/src/lib.rs)
- [`crates/devil-editor/src/lib.rs`](../crates/devil-editor/src/lib.rs)
- [`crates/devil-ui/src/lib.rs`](../crates/devil-ui/src/lib.rs)
- [`crates/devil-ui/src/ui.rs`](../crates/devil-ui/src/ui.rs)
- [`crates/devil-app/src/main.rs`](../crates/devil-app/src/main.rs)
- [`crates/devil-observability/src/lib.rs`](../crates/devil-observability/src/lib.rs)
- [`crates/devil-cli/src/main.rs`](../crates/devil-cli/src/main.rs)
- [`plans/spikes/SPIKE-001A-result.md`](plans/spikes/SPIKE-001A-result.md)
- [`plans/SPIKE-000-platform-boundary-proof.md`](plans/SPIKE-000-platform-boundary-proof.md)

### New files to create after ADR gates

- [`plans/adrs/ADR-0011-lsp-architecture.md`](plans/adrs/ADR-0011-lsp-architecture.md)
- [`plans/adrs/ADR-0012-plugin-runtime.md`](plans/adrs/ADR-0012-plugin-runtime.md)
- [`plans/adrs/ADR-0013-filesystem-vfs-policy.md`](plans/adrs/ADR-0013-filesystem-vfs-policy.md)
- [`plans/adrs/ADR-0014-workspace-state-watcher-policy.md`](plans/adrs/ADR-0014-workspace-state-watcher-policy.md)
- [`crates/devil-lsp/Cargo.toml`](../crates/devil-lsp/Cargo.toml)
- [`crates/devil-lsp/src/lib.rs`](../crates/devil-lsp/src/lib.rs)
- [`crates/devil-plugin/Cargo.toml`](../crates/devil-plugin/Cargo.toml)
- [`crates/devil-plugin/src/lib.rs`](../crates/devil-plugin/src/lib.rs)
- [`crates/devil-terminal/Cargo.toml`](../crates/devil-terminal/Cargo.toml)
- [`crates/devil-terminal/src/lib.rs`](../crates/devil-terminal/src/lib.rs)

## Handoff Direction

Recommended implementation handoff mode: Code mode, with architecture review at every phase exit. Execute phases strictly in order. Stop on dependency-policy failure, trust/policy bypass, stale mutation application, or critical latency regression. Attach phase evidence before entering the next phase.
