# Foundational Core IDE Platform Roadmap v0.1

## Purpose

This roadmap is the execution blueprint for shipping a standalone, deterministic, low-latency core IDE that remains useful without AI while preparing a stable, policy-scoped surface for future AI tooling and plugin integrations.

Core mission statements for this roadmap:

- Trusted local workspace opening.
- Navigable file tree and workspace state management.
- Multi-tab editor operations with deterministic text transactions.
- Save/conflict handling with atomic write and version preconditions.
- Integrated terminal emulation that is policy-gated and cancellable.
- LSP-powered language intelligence through normalized contracts.
- Future-ready seams (capability broker, workspace proposals, context providers) consumed through protocol, not direct mutation.

---

## Baseline Assumptions

- Rust 2024 multi-crate workspace is the mandatory delivery structure.
- Core IDE ships on Windows first, then parity to macOS and Linux.
- The core must remain fully functional with AI systems disabled.
- No VS Code compatibility layer, no generalized marketplace, no remote collaboration in this roadmap.
- No mutation bypasses: every non-user-initiated write path enters proposal/approval and workspace VFS authority.

## Non-Goals

- No autonomous local/remote AI execution in this phase.
- No generalized third-party extension compatibility.
- No bypass of trust, command policy, or capability checks.
- No hosted service dependency for local IDE functionality.

---

## Required Gates Before Scale

The roadmap cannot exit Phase 0 until all of the following are satisfied and recorded:

1. **Dependency and protocol gates**: `xtask` dependency policy passes and required protocol symbols are present.
2. **Freeze gates pass**: all freeze criteria are satisfied and documented; unresolved freeze blockers block progression until resolved.
3. **SPIKE-001A result decision**: `plans/spikes/SPIKE-001A-result.md` contains findings, a decision (`PASS` / `PASS WITH RESERVATIONS` / `FAIL`), explicit fallback criteria, and evidence owner sign-off.
4. **Native shell proof evidence**: `plans/SPIKE-001A-native-shell-proof.md` evidence is complete and reviewed.
5. **Text + index stress baseline**: large-file edit throughput, undo/redo, rollback behavior, and index responsiveness evidence is collected and reviewed.
6. **Platform boundary proof**: platform services stay OS-owned (filesystem/process/watcher/keychain/pty/path normalization) and remain declarative-only for higher-level concerns.
7. **Storage baseline gate**: minimal storage-backend decision and migration strategy (including corruption recovery and namespace boundaries), aligned to `ADR-0005`, are accepted before persistence implementation.
8. **Repository health baseline**: workspace check/build/test commands are green at baseline for planning-related crates touched so far.

### Hard stop rule

Unresolved blockers may be logged, but they remain hard blockers to Phase 1.
To continue with any previously logged blocker, the governing freeze or phase decision document must be formally amended, re-reviewed, and re-approved with updated evidence and owners.

---

## Execution Phases (End-to-End)

### Phase 0 — Freeze Closure and Evidence Baseline

**Goal:** lock architecture and performance/ownership boundaries so implementation can scale.

| Area | Action | Owner | Exit criteria |
|---|---|---|---|
| Gates | Run dependency direction checks and protocol symbol validation | `devil-app` + `xtask` | `cargo run -p xtask -- check-deps` passes and artifacts logged |
| Contract baseline | Snapshot protocol gaps from current spike state | Architecture lead + protocol engineer | Drafted list of missing DTOs, ports, and IDs is approved |
| Native shell | Execute latency proof (input-to-paint, frame variance, resource usage) | UI/runtime engineer | Proof report complete and reviewed |
| Spike result | Update SPIKE-001A result artifact with findings, pass/fail decision, and fallback handling | UI owner + architecture owner | Decision and fallback handling are explicit and signed-off |
| Track C stress | Collect and archive text/index stress artifacts (throughput, rollback, memory growth, index responsiveness) | Editor + platform engineer | Baseline metrics recorded and reviewed |
| Boundary | Execute platform boundary proof with explicit service-owner matrix | platform engineer + security | No editor/project authority in platform API map |
| Storage gate | Define minimal storage backend, corruption-recovery behavior, and namespace boundaries | storage owner + security + architecture | ADR-backed storage baseline is documented and approved |

**Stop condition:** Any unresolved Phase 0 blocker prevents entering Phase 1.
To proceed with a previously documented blocker, the relevant freeze/phase decision document must be formally amended and re-approved with updated evidence.

Phase-0 evidence set also includes explicit ownership for decision paths under `PASS WITH RESERVATIONS` and `FAIL` fallback execution.

**Phase 0 exit condition:** unresolved blockers can be documented but cannot be treated as cleared.

---

### Phase 1 — Protocol Contract Expansion (Stability Layer)

**Goal:** establish serializable, versioned contracts before subsystem implementation.

**Scope:** expand identifiers, DTOs, and ports for workspace, editor, file-system events, proposals, LSP, terminal, capability, and observability hooks.

| Domain | Core additions | Exit criteria |
|---|---|---|
| IDs and versions | workspace id, generation, root id, file/content/version ids, buffer and snapshot ids, terminal session ids, proposal/correlation ids | IDs used consistently across all new cross-crate messages |
| Workspace DTOs | open/close workspace, trust state, canonical file identity, file metadata, tree node/filters, deltas, conflict DTOs | Contract tests cover serialization and path/canonicalization edge behavior |
| Editor DTOs | buffer lifecycle, transaction/event descriptors, edit batches, coordinate types, overlay descriptors | Tests prevent UTF-8/UTF-16 ambiguity and verify version metadata completeness |
| Proposal DTOs | versioned text/create/delete/rename/save/format/code-action/terminal command proposals | Stale-version proposals are rejected in tests |
| LSP DTOs | config/status/sync/document/diagnostic/completion/hover/formatting actions | Deterministic protocol mapping verified by stable schema tests |
| Terminal DTOs | launch/env policies/output/input/resize/exit/capability | Policy fields are required in all terminal command requests |
| Service ports | workspace/editor/proposal/terminal/LSP/capability/event ports | Consumers compile against ports, not concrete crate internals |
| Observability envelope | correlation IDs, causality chain metadata, retention labels, and redaction hints in cross-boundary events | Correlation fields are present in all event payload contracts |

**Phase 1 entry criteria:** this phase can begin without waiting for ADR-0013 and ADR-0014 acceptance, but downstream implementation in phases that mutate files/workspace/trust must not start until those ADRs are accepted.

**Exit criteria:** every persisted/cross-process DTO has serde round-trip coverage.

---

### Phase 2 — Platform Services and Security Policy Foundations

**Goal:** replace spike helpers with OS-service traits and typed policy surfaces.

| Area | Scope | Exit criteria |
|---|---|---|
| `devil-platform` | filesystem traits (canonicalize/read/write/list/watch hash/symlink), process traits, pty traits, typed errors | No direct caller bypasses typed traits for filesystem/process calls |
| `devil-security` | trust state, path policy, command/terminal policy, default-deny for untrusted workspace | Schema and broker tests confirm blocked terminal/LSP/plugin/file-write behavior for denied principals |
| Composition | remove direct ad-hoc file helper usage in app entry path | app composes through workspace/VFS ports |
| Terminal policy (phase-in) | define command classes and output limits early (prevents bypass later) | Dangerous command class requires explicit escalation in tests |
| Capability-broker stub | add deny-by-default capability/broker stub for terminal/LSP/plugin/file-write policy paths used by security checks | Stale or missing decisions are denied and traced with correlation IDs |

**Acceptance split:** full terminal/LSP/plugin enforcement is deferred until the terminal (when introduced) and LSP/plugin phases.
Phase 2 only requires the policy schema and deny/allow behavior to be exercised through stubs or test doubles.

**Phase 2 cleanup checklist:**
- remove direct `rust.open_text_file()` and `rust.save_text_file()` calls from app-facing paths;
- mark any remaining raw helper calls as spike-only baseline debt with explicit closure milestone.

**Exit criteria:** dependency policy check remains green after service decoupling.

---

### Phase 3 — Workspace VFS and Tree Foundation

**Goal:** implement trusted workspace ownership, identity, file tree, watcher, and initial persistence.

| Area | Scope | Exit criteria |
|---|---|---|
| `devil-project` workspace actor | open/close, trust, config snapshot, generation, file id mapping | Trust state updates are visible to UI and other services |
| File tree | shallow discovery with ignore/hidden/generated/binary/large flags; stable IDs | Repository opens quickly and tree updates without editing block |
| Watcher | raw-event intake with debounce and bounded overflow rescan | Renames preserve identity when metadata permits; overflow recovery is visible |
| Storage contracts | recent workspace, trust, and session metadata repos | Schema migrate/read tests pass |
| UI projection | explorer rows/selection/expansion projection model | Tree expands/collapses without owning filesystem state |

**Phase 3 entry criteria:** ADR-0013 and ADR-0014 are accepted before persistence-sensitive VFS/tree/watcher semantics are finalized.

**Exit criteria:** open workspace with basic shallow tree, trust toggles, and watcher updates works under bounded rescan.

### Deferred indexing and symbol boundary (foundational scope)

This roadmap intentionally retains shallow metadata discovery for the foundational core.

- In scope: repository discovery metadata, shallow ownership boundaries, canonical IDs, and explorer-level visibility.
- Deferred: tree-sitter parsing, symbol extraction, lexical symbol search, semantic ranking, and embedding/vector pipelines.
- Ownership for deferred capabilities moves to a post-roadmap milestone and future ADR sequence.

---

### Phase 4 — Production Text Model and Editor Transactions

**Goal:** replace spike text model with scalable model and deterministic edits.

| Area | Scope | Exit criteria |
|---|---|---|
| `devil-text` | rope/piece-table core, immutable snapshot descriptors, UTF8/UTF16 conversions, line index | Large-file operations stay within measured budgets and preserve snapshot immutability |
| `devil-editor` | multi-buffer registry, transaction groups, undo/redo, overlays, dirty-state | Edits and undo/redo preserve invariants across multiple open buffers |
| Save hooks | save requests become proposal-aware workspace-mediated operations | Editor does not perform raw writes |

**Exit criteria:** deterministic transaction log includes pre/post snapshot and causality ids.

---

### Phase 5 — Multi-Tab UI Shell and Session Restore

**Goal:** production shell projection with explorer, tab model, viewport, and panel surfaces.

| Area | Scope | Exit criteria |
|---|---|---|
| `devil-ui` shell layout | explorer/sidebar/editor/panels/status/command palette projection | Focus and resize operations are non-blocking |
| Tabs | tab/group model, dirty indicators, pinned/preview behavior, activation semantics | Opening same file focuses existing tab unless explicit split requested |
| Commands | command registry for open/close/save/split/search reveal commands | Commands dispatch via protocol ports, never mutate text directly |
| Session restore | restore tabs, focus, layout, explorer expansion from storage | Restart yields expected layout and open context |

**Exit criteria:** multi-tab file workflow operates without coupling to workspace internals.

---

### Phase 6 — Save Pipeline, Conflict State, and Proposal Lifecycle

**Goal:** make every durable mutation explicit, versioned, previewable, and safe.

| Area | Scope | Exit criteria |
|---|---|---|
| Save pipeline | fingerprint precondition checks, atomic temp-write replace, fallback semantics, conflict state | External overwrite never silently clobbered |
| Editor conflict state | dirty/reload/keep-both/compare semantics for open buffers | Dirty open buffer behavior does not lose data |
| Proposal validation | validate mutation preconditions by buffer and file versions | Stale proposals rejected with diagnostics |
| Preview UX | proposal preview with diff-like summary and approval lifecycle | User can approve/reject before application |

**Exit criteria:** all mutation paths produce transaction metadata and proposal audit logs.

---

### Phase 7 — Integrated Terminal Emulation (Adjacent Core Service)

**Goal:** introduce terminal as first-class service with hard policy and bounded transcript.

| Area | Scope | Exit criteria |
|---|---|---|
| Terminal service | terminal actor, session registry, output stream fanout, transcript limits, cancellation | Terminal I/O remains bounded and does not block UI |
| Security | command class taxonomy and escalation policy in security service | Untrusted workspace cannot launch shell or dangerous command classes |
| Platform pty/process backends | pty traits and tested adapters | Smoke tests for launch/write/resize/cancel/exit on primary platform |
| UI terminal projection | panel with tabs/output/scrollback/search status controls | Terminal state visible and restart/kill/reconnect controls available |
| Crate-introduction gate | only introduce `devil-terminal` as a dedicated crate once terminal-specific ADR is reviewed and accepted | Terminal service ownership remains explicit and ADR-gated |

**Exit criteria:** terminal execution cannot bypass trust, command, environment, or timeout policy.

---

### Phase 8 — LSP Coordinator and Feature Delivery

**Goal:** add language services in a normalized, non-mutating fashion.

| Area | Scope | Exit criteria |
|---|---|---|
| `devil-lsp` | ADR-0011 accepted before implementation; server lifecycle, transport, supervision, restart policy | trusted workspace can start LSP, untrusted cannot |
| Sync pipeline | per-buffer versioning, incremental/full sync fallback, stale response suppression | Completion/hover/diagnostic do not apply to stale document versions |
| Feature normalization | diagnostics/hover/completion/rename/format/code actions normalized into internal DTOs | LSP responses consumed by editor/UI through protocol types |
| Mutation bridge | code actions/format become proposal objects | Editor/workspace apply all proposals through validation lifecycle |

**Exit criteria:** core language loop works end-to-end through proposals and overlays.

---

### Phase 9 — Observability, Replay, and Quality Hardening

**Goal:** make behavior measurable and diagnosable without privacy leaks by default.

| Area | Scope | Exit criteria |
|---|---|---|
| Event envelopes | correlation-aware events for all subsystem boundaries | Every critical user flow emits causally ordered events |
| Metrics | latency histograms for edit/render/save/LSP/terminal/open/scan |
| Storage | event metadata retention, schema migrations, compact retention policy | Replay is possible without full source snapshots |
| CLI diagnostics | dependency graph, protocol symbol, event and performance summaries |

**Phase 9 entry criteria:** baseline event-envelope requirements from Phase 1 are extended to policy, storage, and persistence event categories.

**Exit criteria:** performance budgets and corruption recovery scenarios are enforced in CI/QA.

---

### Phase 10 — Plugin and Future AI-Seams Completion

**Goal:** add controlled extensibility after core stability, while preserving non-AI coupling.

| Area | Scope | Exit criteria |
|---|---|---|
| ADR and crates | finalize plugin architecture and scaffold runtime crate (trusted first-party only) | Plugin contributions use manifest validation and capability model |
| Capability broker | grants/denials/prompt surfaces for workspace read/write/process/terminal/UI/storage |
| Protocol context providers | passive data providers with sensitivity labels and retention hints | Context providers cannot emit mutation operations |
| AI boundary | keep AI as external consumer of proposal/overlay/context ports |

**Phase 10 entry criteria:** ADR-0012 acceptance is required before finalizing plugin runtime crate and plugin capability enforcement.

**Exit criteria:** plugin/tooling/API extensions consume protocol services and cannot mutate directly.

---

## Delivery Milestones

| Milestone | Definition of Done |
|---|---|
| M0 | Phase 0 gates accepted |
| M1 | Protocol + platform + VFS foundations ready |
| M2 | Workspace open, trust, tree, and watcher in production |
| M3 | Editor core supports deterministic multi-buffer transactions and overlays |
| M4 | Shell layout with tabs and session restore |
| M5 | Save/conflict/proposal loop complete |
| M6 | Integrated terminal with trust and policy |
| M7 | LSP normalizes diagnostics/completion/format/code action |
| M8 | Observability & QA gates in CI |
| M9 | Plugin hooks and AI-seam validation without coupling |

---

## Suggested Ownership and Governance

Recommended ownership (adjust by team size):

- **Architecture + Freeze:** owns gates, ADR acceptance, dependency policy.
- **Platform + Security:** owns traits, path policy, process/pty and policy enforcement.
- **Project + Storage:** owns workspace actor, tree, watcher, trust persistence.
- **Text + Editor:** owns text model, transactions, overlays.
- **UI:** owns projections, commands, tab/panel ergonomics, event-driven rendering.
- **LSP + Terminal:** owns lifecycle services and protocol normalization; no raw edits.
- **Observability + CLI:** owns metrics, redaction policies, replay summaries.

### Risk Register (high impact)

- **Gate drift:** new crate dependencies introduced before ADR/policy acceptance.
- **Path-policy failures:** Windows normalization/symlink edge cases causing false trust or path escapes.
- **Editor lag under background load:** missed latency budgets from watcher/LSP/terminal work.
- **Proposal leakage:** non-deterministic mutation path re-introduced by plugin/tooling.
- **Replay debt:** missing correlation ids and missing retention policy causing poor diagnosis.

**Mitigations:**

- **Gate drift [Owner: Architecture + Freeze]:** hard ADR preconditions in phase entries and explicit crate-introduction gates.
- **Path-policy failures [Owner: Platform + Security]:** ADR-0013/ADR-0014 canonicalization and watcher identity regression suites.
- **Editor lag under background load [Owner: UI + Text]:** phase-level latency checkpoints, and fallback behavior documented before scaling.
- **Proposal leakage [Owner: App + Workspace]:** remove direct file helper calls in Phase 2 and enforce proposal-only mutation pathways.
- **Replay debt [Owner: Observability + Storage]:** minimal correlation-enveloped events from Phase 1, then full replay/retention gates in Phase 9.
- **Rust-native UI maturity risk [Owner: UI + Architecture]:** enforce `SPIKE-001A-result.md` pass/fail, PASS WITH RESERVATIONS, or FAIL with fallback decision.
- **Storage corruption and migration risk [Owner: Storage]:** ADR-0005 acceptance and corruption/recovery tests before persistence scaling.
- **Async backpressure and deadlock risk [Owner: Runtime + Security]:** bounded queues, timeout policy, and mailbox saturation tests in service integration phases.
- **Air-gap/network-policy bypass risk [Owner: Security]:** ADR-0010 command and network policy checks integrated by early policy phases.

---

## Implementation Sequence and Stop Conditions

Phases run in order 0 through 10. Stop immediately on any of: freeze gate violation, dependency-policy failure, trust/policy bypass, or regression in critical latency budget thresholds. After each phase, run:

- `cargo run -p xtask -- check-deps`
- `cargo fmt --all`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`

Proceeding without clean evidence from the prior phase is treated as a sequencing violation.

### Phase validation matrix

- **Phase 0:** freeze gate evidence, SPIKE-001A result decision, Track C stress artifacts, platform boundary proof, storage gate.
- **Phase 1:** protocol round-trip coverage and event-envelope schema coverage.
- **Phase 2:** policy deny/allow matrix over stubbed broker paths; direct-helper elimination proof.
- **Phase 3:** VFS/tree/watcher identity, bounded overflow rescan, migration and corruption tests.
- **Phase 4:** undo/redo invariants and text throughput under load budgets.
- **Phase 5:** tab restoration and session projection consistency.
- **Phase 6:** save/conflict edge cases and proposal audit completeness.
- **Phase 7:** terminal command policy, transcript bounds, and PTY/process smoke tests.
- **Phase 8:** stale-response suppression and trust-gated LSP startup tests.
- **Phase 9:** replay drills and performance-correlation checks.
- **Phase 10:** plugin manifest validation and ADR-0012-driven grants/denials.

---

## Immediate Next Action

Authorize starting **Phase 0** with updated evidence ownership: assign one owner each for freeze gate evidence, dependency check ownership, SPIKE-001A-result completion, native shell proof validation, text/index stress collection, platform-boundary proof authoring, and storage gate closure.
