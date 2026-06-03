## Legion IDE 2026 Implementation Plan

### Executive Summary

The architecture review in [`plans/architecture-review-2026-ide-roadmap-v0.1.md`](plans/architecture-review-2026-ide-roadmap-v0.1.md:10) establishes that Legion IDE has crossed beyond a spike-only baseline, but remains a deterministic local IDE core rather than a full 2026-class, AI-native, distributed development platform. The strongest current foundations are proposal-mediated saves via [`SaveWorkflowService`](crates/devil-app/src/lib.rs:935), fail-closed workspace mutation via [`WorkspaceActor::save_file_with_proposal()`](crates/devil-project/src/lib.rs:1620), projection-only UI via [`ActiveBufferProjection`](crates/devil-ui/src/ui.rs:86), deterministic editor ownership via [`EditorEngine`](crates/devil-editor/src/lib.rs:312), default-deny security via [`DenyByDefaultBroker`](crates/devil-security/src/lib.rs:668), and metadata-oriented observability via [`proposal_audit_record()`](crates/devil-observability/src/lib.rs:394).

The strategic architectural vision is to evolve Legion IDE from a local-first deterministic core into an evented, distributed, policy-mediated IDE substrate. The platform must preserve one non-negotiable rule: AI, LSP, plugins, collaboration, terminal tools, and remote agents must never mutate editor or workspace state directly. All non-user-direct mutation must flow through typed proposals, editor transactions, workspace VFS authority, redacted events, and durable audit metadata, as stated in [`plans/architecture-review-2026-ide-roadmap-v0.1.md`](plans/architecture-review-2026-ide-roadmap-v0.1.md:24).

The resulting target platform has five major missing capability layers:

1. Agentic AI execution embedded across editor, workspace, semantic index, tracker, terminal, policy, and replay context.
2. Predictive semantic fabric backed by incremental parsing, symbol graphs, LSP fusion, typed caches, and invalidation.
3. WASM extension ecosystem with manifest validation, WASI capability mediation, ABI versioning, quotas, and deterministic contribution points.
4. Edge-executed remote development with workspace agents, encrypted transport, remote filesystem/process/LSP/index services, and local optimistic projections.
5. Real-time multiplayer collaboration with operation logs or CRDTs, presence, version vectors, conflict-safe file operations, shared proposals, and collaborative AI approvals.

The correct execution strategy is not to graft these capabilities into [`crates/devil-app/src/lib.rs`](crates/devil-app/src/lib.rs). The roadmap must instead promote the existing proposal, protocol, event, storage, and policy patterns into universal platform contracts consumed by every advanced subsystem. The first implementation handoff must prioritize Phase 0, Phase 1, and Phase 2, because governance completeness, scalable text projection, and generalized proposals are prerequisites for every subsequent capability.

---

## Strategic Program Timeline

This is a dependency-ordered engineering roadmap rather than a calendar commitment. For planning purposes, the full program should be treated as a multi-quarter platform initiative with hard phase gates. Phases may overlap only where they do not violate sequencing constraints in [`plans/architecture-review-2026-ide-roadmap-v0.1.md`](plans/architecture-review-2026-ide-roadmap-v0.1.md:415).

| Phase | Strategic Goal | Suggested Duration | Primary Milestone | Dependency Gate |
|---|---:|---:|---|---|
| Phase 0 | Rebaseline governance and architecture truth | 2-3 weeks | M0: Architecture governance locked | No runtime surface without ADR, policy, contracts, tests, events |
| Phase 1 | Scale editor and text substrate | 5-8 weeks | M1: Viewport/chunked editor substrate | Large files avoid full-source UI projection |
| Phase 2 | Generalize proposal-mediated mutation | 6-9 weeks | M2: Universal proposal lifecycle | All future mutation clients share one proposal model |
| Phase 3 | Build predictive semantic fabric | 8-12 weeks | M3: Index/LSP semantic substrate | Live editor snapshots supersede background work |
| Phase 4 | Implement native agentic AI execution context | 8-12 weeks | M4: Policy-bound AI control plane | AI cannot mutate directly and is replayable |
| Phase 5 | Create WASM isolated extension ecosystem | 8-12 weeks | M5: Sandboxed plugin runtime | Plugins have no ambient access and mutate only via proposals |
| Phase 6 | Add real-time multiplayer collaboration | 10-14 weeks | M6: Convergent collaborative operation layer | Concurrent edits converge deterministically |
| Phase 7 | Build edge-executed remote development | 10-16 weeks | M7: Edge workspace agent and local projection | Remote writes cannot bypass proposals or fingerprint checks |
| Phase 8 | Product hardening and ecosystem readiness | 6-10 weeks | M8: Durable governance and operational readiness | Replay, migration, privacy, diagnostics, and policy gates pass |

---

## Phase 0 - Rebaseline Governance and Architecture Truth

### Objective

Make architecture truth enforceable before activating any 2026 runtime surface. This phase prevents dependency drift, stale review assumptions, and premature activation of placeholder crates such as [`crates/devil-index/src/lib.rs`](crates/devil-index/src/lib.rs), [`crates/devil-agent/src/lib.rs`](crates/devil-agent/src/lib.rs), [`crates/devil-tracker/src/lib.rs`](crates/devil-tracker/src/lib.rs), and [`crates/devil-memory/src/lib.rs`](crates/devil-memory/src/lib.rs).

### Technical Deliverables

- Mark stale claims in [`plans/architecture-review-full-codebase-v0.1.md`](plans/architecture-review-full-codebase-v0.1.md:61) and [`plans/architecture-review-phases-5-6-v0.1.md`](plans/architecture-review-phases-5-6-v0.1.md:25) as historical where they contradict current proposal-mediated saves and projection-only UI behavior.
- Expand [`plans/dependency-policy.md`](plans/dependency-policy.md:9) so every current crate and planned runtime crate has an explicit dependency-policy section.
- Update [`xtask::validate_dependency_policy()`](xtask/src/main.rs:117) so missing crate policy is a hard violation.
- Move hardcoded dependency requirements from [`Policy::from_markdown()`](xtask/src/main.rs:245) into [`plans/dependency-policy.md`](plans/dependency-policy.md:9) where feasible.
- Require ADRs before activating index, agent, tracker, memory, plugin, LSP, terminal, collaboration, or remote surfaces.
- Add architecture-gate tests proving:
  - UI remains projection-only and does not own editor or workspace state.
  - Saves remain proposal-mediated through [`SaveWorkflowService::save_active_buffer()`](crates/devil-app/src/lib.rs:938).
  - Source snapshots are not persisted by default.
  - Placeholder crates remain inert until ADR, policy, protocol contracts, and tests exist.

### Acceptance Criteria

- [`cargo run -p xtask -- check-deps`](xtask/src/main.rs) fails if any workspace crate lacks policy coverage.
- [`plans/dependency-policy.md`](plans/dependency-policy.md:9) covers all workspace crates in [`Cargo.toml`](Cargo.toml).
- Protocol symbols required by policy are present in [`crates/devil-protocol/src/lib.rs`](crates/devil-protocol/src/lib.rs).
- Stale save-flow claims are explicitly annotated as historical.
- CI phase gates pass: [`cargo fmt --all --check`](Cargo.toml), [`cargo check --workspace --all-targets`](Cargo.toml), [`cargo test --workspace --all-targets`](Cargo.toml), and [`cargo clippy --workspace --all-targets -- -D warnings`](Cargo.toml).

### Resource Allocation

- Architecture Lead: 0.6 FTE
- Rust Platform Engineer: 1.0 FTE
- QA/Automation Engineer: 0.5 FTE
- Security/Policy Engineer: 0.4 FTE
- Technical Program Manager: 0.3 FTE

---

## Phase 1 - Scale the Editor and Text Substrate

### Objective

Remove the core scalability bottleneck: full-buffer materialization in UI and text infrastructure. The review identifies full text in [`ActiveBufferProjection`](crates/devil-ui/src/ui.rs:86), full-cache budget behavior via [`DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES`](crates/devil-text/src/lib.rs:22), and full line-index materialization via [`LineIndex`](crates/devil-text/src/lib.rs:457) as incompatible with large files, predictive semantics, collaboration replay, remote latency hiding, and AI retrieval.

### Technical Deliverables

- Replace full active-buffer projection with viewport projections:
  - visible line slices,
  - cursor projections,
  - selection projections,
  - decoration spans,
  - fold ranges,
  - semantic token overlays,
  - lazy line metrics.
- Refactor [`LineIndex`](crates/devil-text/src/lib.rs:457) into incremental or chunked line-metric infrastructure.
- Introduce chunked snapshots with stable snapshot IDs, chunk hashes, and lease semantics for LSP, index, plugins, AI, collaboration, storage, and observability consumers.
- Add large-file degraded mode above [`DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES`](crates/devil-text/src/lib.rs:22): viewport-only rendering, bounded search, disabled expensive overlays, and explicit user-visible status.
- Add non-blocking transaction event streams from [`EditorEngine`](crates/devil-editor/src/lib.rs:312) to semantic, LSP, collaboration, storage, and observability consumers.
- Expand performance tests in [`crates/devil-editor/tests/performance_suite.rs`](crates/devil-editor/tests/performance_suite.rs) to simulate input while indexing, LSP, AI retrieval, and collaboration replay workloads are active.

### Acceptance Criteria

- Very large files open without sending full source text to UI projections.
- UI receives full source text only in explicitly bounded small-buffer mode.
- Edits update viewport state, line metrics, and snapshot chunks incrementally.
- Concurrent indexing or AI retrieval workloads cannot block editor input.
- The known 100MB workload gap documented in [`plans/evidence/phase-0/text-index-stress-baseline.md`](plans/evidence/phase-0/text-index-stress-baseline.md:31) is converted into measured degraded-mode behavior rather than treated as a green benchmark.
- Existing save/conflict behavior in [`crates/devil-app/tests/workspace_vfs_integration.rs`](crates/devil-app/tests/workspace_vfs_integration.rs:73) remains intact.

### Resource Allocation

- Editor/Text Lead: 1.0 FTE
- Rust Performance Engineer: 1.0 FTE
- UI Engineer: 0.8 FTE
- Observability Engineer: 0.4 FTE
- QA/Performance Engineer: 0.7 FTE

---

## Phase 2 - Generalize Proposal-Mediated Mutation

### Objective

Promote save-specific proposal mediation into a universal mutation substrate without weakening current save guarantees. Manual saves already flow through [`SaveWorkflowService::save_active_buffer()`](crates/devil-app/src/lib.rs:963), proposal construction in [`SaveProposalCoordinator::build_save_proposal()`](crates/devil-app/src/lib.rs:151), and fail-closed disk mutation through [`WorkspaceActor::save_file_with_proposal()`](crates/devil-project/src/lib.rs:1622). Phase 2 generalizes that path for editor transactions, workspace edits, future LSP actions, future AI patches, future plugin commands, future collaboration operations, and future remote workspace operations.

Phase 2 is contract and substrate work only. Placeholder runtime crates such as [`crates/devil-index/src/lib.rs`](crates/devil-index/src/lib.rs), [`crates/devil-agent/src/lib.rs`](crates/devil-agent/src/lib.rs), [`crates/devil-tracker/src/lib.rs`](crates/devil-tracker/src/lib.rs), and [`crates/devil-memory/src/lib.rs`](crates/devil-memory/src/lib.rs) remain inert except for DTOs, stubs, and contract tests.

### Current Baseline and Non-Negotiable Constraints

- Preserve the current save chain from [`AppComposition::save_active_buffer()`](crates/devil-app/src/lib.rs:214) to [`SaveWorkflowService::save_active_buffer()`](crates/devil-app/src/lib.rs:963) to [`WorkspaceActor::save_file_with_proposal()`](crates/devil-project/src/lib.rs:1622).
- Preserve mandatory save preconditions carried by [`WorkspaceSaveRequest`](crates/devil-project/src/lib.rs:133): expected fingerprint, file content version, workspace generation, buffer version, snapshot id, payload length, correlation id, and causality id.
- Preserve fail-closed non-atomic writes through [`NonAtomicSaveFallbackPolicy::Disabled`](crates/devil-project/src/lib.rs:172).
- Preserve dirty editor text on stale, denied, conflict, rejected, or failed outcomes, matching [`workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict()`](crates/devil-app/tests/workspace_vfs_integration.rs:280) and [`workspace_vfs_integration_untrusted_save_is_denied_without_disk_mutation()`](crates/devil-app/tests/workspace_vfs_integration.rs:87).
- Replace save-only panic behavior in [`proposal_file_identity()`](crates/devil-app/src/lib.rs:1357) with total payload visitors.
- Keep UI projection-only; approval state is app/protocol projection data, not editor or workspace ownership inside [`Shell`](crates/devil-ui/src/ui.rs:228).

### Development Workstreams

#### Workstream 2.1 - Governance and ADR Gate

1. Create [`plans/adrs/ADR-0016-generalized-proposal-service.md`](plans/adrs/ADR-0016-generalized-proposal-service.md) before implementation. It must decide lifecycle states, approval model, multi-file atomicity, rollback limits, partial-failure records, payload redaction, and ownership boundaries.
2. Record evidence in [`plans/evidence/phase-2/proposal-mutation-substrate.md`](plans/evidence/phase-2/proposal-mutation-substrate.md), including validation commands, test coverage, static-search results, and deferred runtime integrations.
3. Do not create a new runtime crate by default. Keep the first generalized service in [`crates/devil-app/src/lib.rs`](crates/devil-app/src/lib.rs) or a later app-domain module unless the ADR explicitly approves a crate and updates [`Cargo.toml`](Cargo.toml), [`plans/dependency-policy.md`](plans/dependency-policy.md), and [`xtask::validate_dependency_policy()`](xtask/src/main.rs:117).

#### Workstream 2.2 - Protocol Contract Expansion

1. Extend [`ProposalPayload`](crates/devil-protocol/src/lib.rs:1377) and [`ProposalPayloadKind`](crates/devil-protocol/src/lib.rs:1544) so every mutation source can be represented without ad hoc side channels: text edit, create, delete, rename, save, format, code action, terminal command, AI patch, plugin action, collaboration operation, and remote workspace operation.
2. Add batch-level DTOs for affected targets, ordered apply steps, dependency edges, rollback steps, partial-failure records, and preview warnings. These DTOs must be metadata-first and must not require storing raw source by default.
3. Extend [`ProposalRequest`](crates/devil-protocol/src/lib.rs:2457) beyond validate, preview, and apply with approve, reject, cancel, and rollback requests, or document why approval is represented outside the port.
4. Keep [`ProposalResponse`](crates/devil-protocol/src/lib.rs:2468) as the single lifecycle response model for created, validated, previewed, approved, rejected, applied, denied, failed, rolled back, stale, conflict, and cancelled outcomes.
5. Add round-trip and golden tests in [`crates/devil-protocol/tests/dto_contracts.rs`](crates/devil-protocol/tests/dto_contracts.rs:653) for every new payload, lifecycle transition, batch record, partial failure, rollback record, and required field.

#### Workstream 2.3 - Generalized Proposal Service Extraction

1. Replace [`SaveProposalCoordinator`](crates/devil-app/src/lib.rs:120) with a generalized app-domain proposal service that implements [`ProposalPort`](crates/devil-protocol/src/lib.rs:2989).
2. Move ID allocation, event context storage, sequence allocation, lifecycle transition creation, validation dispatch, preview dispatch, and audit/event emission out of save-specific naming.
3. Make [`SaveWorkflowService`](crates/devil-app/src/lib.rs:959) consume the generalized service through save-specific adapters rather than owning proposal lifecycle policy.
4. Keep manual save behavior equivalent from the caller perspective: stale, conflict, and denial still return `Ok(AppSaveOutcome::Rejected(_))` from [`AppComposition::save_active_buffer()`](crates/devil-app/src/lib.rs:214), and dirty editor text remains intact.

#### Workstream 2.4 - Total Payload Visitors and Preview Models

1. Replace [`proposal_file_identity()`](crates/devil-app/src/lib.rs:1357) with a total visitor that returns affected targets instead of panicking for [`ProposalPayload::TextEdit`](crates/devil-protocol/src/lib.rs:1380), [`ProposalPayload::CreateFile`](crates/devil-protocol/src/lib.rs:1382), or [`ProposalPayload::TerminalCommand`](crates/devil-protocol/src/lib.rs:1394).
2. Define target classification for each payload: open-buffer targets, closed-file targets, path-only targets, terminal/session targets, remote targets, collaboration targets, and no-file metadata-only targets.
3. Generate preview summaries with bounded metadata: affected file IDs, path hashes or lengths, operation counts, replacement byte counts, capability IDs, principal, trust state, warning codes, and redaction hints.
4. Terminal, AI, plugin, collaboration, and remote proposals must validate and preview through stubs in Phase 2, but their real runtime apply paths remain denied until later ADR-gated phases.

#### Workstream 2.5 - Apply Orchestration, Atomicity, and Rollback

1. Introduce a proposal apply planner that separates validation, preview, approval, preflight, apply, commit, rollback, and audit emission.
2. Route open-buffer text mutations through editor transactions owned by [`EditorEngine`](crates/devil-editor/src/lib.rs:312). Never mutate editor text from workspace, UI, plugins, AI, LSP, remote, or collaboration code.
3. Route closed-file create, delete, rename, and save operations through [`WorkspaceActor`](crates/devil-project/src/lib.rs:410) and VFS methods that require proposal context, capability, expected versions, and non-zero observability IDs.
4. Define multi-file atomicity as prepare-all before mutate-any where possible. If true atomicity is unavailable across heterogeneous targets, apply must either fail before mutation or emit explicit partial-failure and rollback records.
5. Rollback open-buffer mutations through editor transaction undo groups and rollback closed-file mutations through VFS backups or inverse operations. If rollback cannot restore exact prior state, return a failed rollback response with preserved user buffers and durable metadata.
6. Keep save-specific fingerprint conflict checks as the reference implementation for stale detection and extend the same model to create, delete, rename, and closed-file edit operations.

#### Workstream 2.6 - Policy, Capability, and Trust Integration

1. Evaluate every proposal through [`DenyByDefaultBroker`](crates/devil-security/src/lib.rs:668) or an injected capability broker before preview approval can become apply.
2. Require principal, capability, workspace trust state, target path or target hash, correlation ID, and causality ID for every privileged proposal.
3. Map capabilities consistently: file writes use `fs.write`, terminal commands use `terminal.*`, language server mutations use `lsp.*`, plugins use `plugin.*`, network or remote access uses `network.*` or `remote.*`, and future AI tool execution uses policy-scoped capabilities only.
4. Deny stale or missing capability decisions by default. Denials must produce [`ProposalResponse::Denied`](crates/devil-protocol/src/lib.rs:2492) plus redacted event metadata.
5. Do not allow terminal, LSP, plugin, AI, remote, or collaboration stubs to bypass the proposal service even when their real runtime phases are not active.

#### Workstream 2.7 - Observability, Audit, and Storage Metadata

1. Emit lifecycle events for every transition using metadata-only helpers such as [`event_metadata_record()`](crates/devil-observability/src/lib.rs:376) and [`proposal_audit_record()`](crates/devil-observability/src/lib.rs:394).
2. Extend event helpers as needed for approved, rejected, cancelled, rollback-started, rollback-failed, partial-failure, and conflict transitions.
3. Persist only metadata by default: proposal ID, lifecycle state, affected IDs, byte counts, hashes, redaction hints, principal, capability, correlation ID, causality ID, event sequence, diagnostics, and retention labels.
4. Reject zero [`CorrelationId`](crates/devil-protocol/src/lib.rs:161), nil [`CausalityId`](crates/devil-protocol/src/lib.rs:178), or zero [`EventSequence`](crates/devil-protocol/src/lib.rs:165) at event construction boundaries.
5. Add tests proving proposal event ordering remains deterministic around save, validation failure, policy denial, stale rejection, conflict, rollback, and partial failure.

#### Workstream 2.8 - UI Approval Projection Without UI Ownership

1. Add app-level projection DTOs for proposal list, selected proposal preview, affected targets, warnings, approve/reject/cancel actions, and post-apply result.
2. Route UI commands as intents, following [`CommandDispatchIntent`](crates/devil-ui/src/ui.rs:141), and keep all lifecycle decisions in app/protocol services.
3. Ensure the UI can render proposals for non-file targets without receiving raw source text or owning editor/workspace state.

### Implementation Sequence

1. Land ADR and evidence scaffolding: [`plans/adrs/ADR-0016-generalized-proposal-service.md`](plans/adrs/ADR-0016-generalized-proposal-service.md) and [`plans/evidence/phase-2/proposal-mutation-substrate.md`](plans/evidence/phase-2/proposal-mutation-substrate.md).
2. Expand [`crates/devil-protocol/src/lib.rs`](crates/devil-protocol/src/lib.rs) first, then update [`crates/devil-protocol/tests/dto_contracts.rs`](crates/devil-protocol/tests/dto_contracts.rs) until every DTO has deterministic serialization coverage.
3. Refactor [`SaveProposalCoordinator`](crates/devil-app/src/lib.rs:120) into a generalized service while keeping existing save integration tests green.
4. Replace [`proposal_file_identity()`](crates/devil-app/src/lib.rs:1357) with total visitors and add tests for text-edit, create, terminal, and multi-file payloads that previously would have panicked.
5. Add apply planning and rollback records behind save and test-double payloads before wiring any future subsystem runtime.
6. Integrate policy decisions through [`DenyByDefaultBroker`](crates/devil-security/src/lib.rs:668) and add deny-by-default tests for untrusted workspace, missing principal, missing capability, stale decision, unsupported runtime payload, and blocked path.
7. Wire observability and storage metadata through [`proposal_audit_record()`](crates/devil-observability/src/lib.rs:394), [`event_metadata_record()`](crates/devil-observability/src/lib.rs:376), and storage repository requests.
8. Add application integration tests in [`crates/devil-app/tests/workspace_vfs_integration.rs`](crates/devil-app/tests/workspace_vfs_integration.rs) proving current save behavior did not regress and generalized proposals share the same lifecycle.
9. Run phase gates and archive outputs in [`plans/evidence/phase-2/proposal-mutation-substrate.md`](plans/evidence/phase-2/proposal-mutation-substrate.md).

### Out of Scope for Phase 2

- No active LSP coordinator, AI agent runtime, WASM plugin host, collaboration engine, remote workspace agent, terminal PTY runtime, or semantic index activation.
- No raw source persistence for audit, replay, or preview by default.
- No relaxation of fail-closed save behavior or workspace fingerprint checks.
- No direct editor or workspace mutation from UI, AI, LSP, plugin, collaboration, remote, terminal, or storage code.

### Acceptance Criteria

- Stale proposals cannot apply, including stale save, stale closed-file edit, stale multi-file batch, and stale test-double future-subsystem proposal.
- External overwrites cannot be clobbered, preserving guarantees tested by [`workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict()`](crates/devil-app/tests/workspace_vfs_integration.rs:280).
- Multi-file proposals either apply atomically or emit explicit rollback and partial-failure records.
- AI, LSP, plugin, remote, collaboration, terminal, text edit, file operation, and save proposal types share one lifecycle contract through [`ProposalResponse`](crates/devil-protocol/src/lib.rs:2468).
- Rejected, denied, stale, conflicted, failed, or rolled-back mutation proposals preserve dirty editor text.
- Non-zero correlation, causality, and event sequence IDs remain mandatory for every mutation audit record.
- [`proposal_file_identity()`](crates/devil-app/src/lib.rs:1357) no longer panics for valid generalized payloads because it has been replaced by total visitors.
- Full phase gates pass: [`cargo run -p xtask -- check-deps`](xtask/src/main.rs), [`cargo fmt --all --check`](Cargo.toml), [`cargo check --workspace --all-targets`](Cargo.toml), [`cargo test --workspace --all-targets`](Cargo.toml), and [`cargo clippy --workspace --all-targets -- -D warnings`](Cargo.toml).

### Validation Plan

- Protocol contracts: [`cargo test -p devil-protocol --test dto_contracts`](crates/devil-protocol/tests/dto_contracts.rs).
- Save/conflict regression: [`cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict`](crates/devil-app/tests/workspace_vfs_integration.rs:280).
- Untrusted write regression: [`cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_untrusted_save_is_denied_without_disk_mutation`](crates/devil-app/tests/workspace_vfs_integration.rs:87).
- Workspace boundary regression: [`cargo test -p devil-project --test path_boundary`](crates/devil-project/tests/path_boundary.rs).
- Security policy regression: [`cargo test -p devil-security`](crates/devil-security/src/lib.rs).
- Architecture gate: [`cargo run -p xtask -- check-deps`](xtask/src/main.rs).
- Full workspace gates: [`cargo fmt --all --check`](Cargo.toml), [`cargo check --workspace --all-targets`](Cargo.toml), [`cargo test --workspace --all-targets`](Cargo.toml), and [`cargo clippy --workspace --all-targets -- -D warnings`](Cargo.toml).

### Resource Allocation

- Application Architecture Lead: 1.0 FTE
- Protocol Engineer: 0.8 FTE
- Workspace/VFS Engineer: 0.8 FTE
- Observability Engineer: 0.6 FTE
- QA/Contract Test Engineer: 0.8 FTE
- Security/Policy Engineer: 0.4 FTE

---

## Phase 3 - Build the Predictive Semantic Fabric

### Objective

Activate [`crates/devil-index/src/lib.rs`](crates/devil-index/src/lib.rs) as a bounded, cancellable semantic fabric powering zero-latency navigation, LSP fusion, AI context, refactoring previews, and test impact analysis. This phase must wait for chunked snapshots, incremental line metrics, and bounded background event streams from Phase 1.

### Technical Deliverables

- Implement actor-owned indexing with bounded queues, priority scheduling, cancellation tokens, and backpressure.
- Add repository discovery, ignore handling, file fingerprints, shallow lexical index, and symbol file map.
- Add tree-sitter parsing workers and syntax tree caches keyed by content hash and grammar version.
- Extract symbols, references, imports, exports, call edges, type relationships, test links, diagnostics links, and ownership metadata into a normalized graph.
- Integrate LSP diagnostics, completions, hover, semantic tokens, definitions, references, rename, formatting, and code actions through protocol DTOs in [`crates/devil-protocol/src/lib.rs`](crates/devil-protocol/src/lib.rs).
- Add low-latency semantic query APIs for UI navigation, completion ranking, AI context selection, agent planning, test impact, and refactoring previews.
- Defer vector indexing until syntax-aware chunking, provenance, privacy scope, model identity, and invalidation contracts are accepted.

### Acceptance Criteria

- Live editor snapshots supersede slower background repository scans.
- Obsolete parse, LSP, embedding, and ranking work is cancelled.
- Completion, hover, diagnostics, and symbol lookup remain responsive under active editing.
- Index records invalidate by content hash, grammar version, model version, and privacy scope.
- Semantic work cannot block editor input or save workflows.

### Resource Allocation

- Semantic Systems Lead: 1.0 FTE
- Rust Indexing Engineer: 1.0 FTE
- LSP Engineer: 0.8 FTE
- Performance Engineer: 0.6 FTE
- QA/Benchmark Engineer: 0.6 FTE

---

## Phase 4 - Implement Native Agentic AI Execution Context

### Objective

Move from provider abstraction to a policy-bound AI execution plane. [`ModelProvider`](crates/devil-ai/src/lib.rs:216) and provider stubs in [`crates/devil-ai-providers/src/lib.rs`](crates/devil-ai-providers/src/lib.rs:23) are currently not an AI orchestrator, and [`crates/devil-agent/src/lib.rs`](crates/devil-agent/src/lib.rs) is placeholder-only. This phase builds the agent runtime only after semantic fabric and generalized proposals exist.

### Technical Deliverables

- Expand provider capability contracts for streaming, structured output, embeddings, reranking, tool planning, context-window metadata, cost metadata, cancellation, and provider health.
- Implement local-provider adapters before cloud adapters.
- Gate cloud providers behind explicit policy, redacted events, provider allowlists, and air-gap enforcement.
- Implement [`crates/devil-agent/src/lib.rs`](crates/devil-agent/src/lib.rs) as a state-machine runtime with states for observing, planning, proposing, waiting for approval, applying, verifying, recovering, and blocked.
- Add context manifests citing editor snapshots, semantic symbols, retrieved chunks, tracker tasks, memory records, diagnostics, terminal summaries, and user selections.
- Implement a Privacy Inspector backed by tracker and event metadata showing provider, model, files, ranges, symbols, memory items, prompt categories, proposal outputs, and policy decisions.
- Route generated edits through generalized proposals and command execution through capability policy.
- Persist AI run records, selected context, tool calls, approvals, proposal IDs, verification outputs, and redacted provider metadata in tracker storage.

### Acceptance Criteria

- AI cannot mutate editor buffers, disk, terminal, tracker, memory, settings, or storage directly.
- Every model call has redacted event metadata and a context manifest.
- Users can inspect what context was sent and why.
- Air-gap mode prevents cloud providers, hosted telemetry, and non-approved outbound access.
- Agent runs can be cancelled, resumed, and replayed from metadata.
- Cloud-provider activation requires explicit ADR, dependency-policy updates, contract tests, and security review.

### Resource Allocation

- AI Platform Lead: 1.0 FTE
- Agent Runtime Engineer: 1.0 FTE
- Security/Privacy Engineer: 0.8 FTE
- Tracker/Storage Engineer: 0.8 FTE
- Observability Engineer: 0.5 FTE
- QA/Adversarial Test Engineer: 0.7 FTE

---

## Phase 5 - Create a WASM Isolated Extension Ecosystem

### Objective

Introduce untrusted extensibility only through a phase-gated WASM runtime. No plugin runtime crate exists today, and the plugin architecture in [`plans/ide-core-architecture-spec-v0.1.md`](plans/ide-core-architecture-spec-v0.1.md:525) remains future-facing. Extensions must have no ambient access to filesystem, process, network, editor, workspace, storage, or AI services.

### Technical Deliverables

- Add a plugin runtime crate only after updating [`plans/dependency-policy.md`](plans/dependency-policy.md:9), ADRs, and protocol contracts.
- Define WASM ABI and host-call schemas in [`crates/devil-protocol/src/lib.rs`](crates/devil-protocol/src/lib.rs) for commands, menus, panels, status items, editor decorations, snippets, language providers, formatters, LSP registrations, workspace scanners, and passive AI context providers.
- Implement manifest parsing, trust/signature metadata, capability declaration, activation events, compatibility ranges, and contribution registration.
- Run plugins under WASI with no ambient capabilities.
- Expose host calls only through capability-checked protocol requests.
- Route extension reads through scoped context providers and writes through generalized proposals.
- Add per-plugin state namespaces, storage quotas, CPU budgets, memory budgets, cancellation, crash isolation, and redacted plugin events.

### Acceptance Criteria

- A malicious extension cannot access source, network, process, storage, editor state, workspace state, AI, or memory outside granted capabilities.
- Extension crashes do not crash the IDE.
- Extension mutations are proposal-mediated.
- Extension commands are observable and cancellable where applicable.
- ABI compatibility is covered by golden manifests and host-call schema tests.
- VS Code extension compatibility remains out of scope unless future ADRs explicitly reverse the clean-slate constraint.

### Resource Allocation

- Extension Platform Lead: 1.0 FTE
- Runtime/Sandbox Engineer: 1.0 FTE
- Protocol Engineer: 0.6 FTE
- Security Engineer: 1.0 FTE
- Storage Engineer: 0.4 FTE
- QA/Sandbox Test Engineer: 0.8 FTE

---

## Phase 6 - Add Real-Time Multiplayer Collaboration

### Objective

Introduce collaboration as a platform substrate, not a bypass around editor or workspace ownership. No collaboration crate, operation log, CRDT, presence, shared proposal, or conflict policy exists today. This phase depends on scalable text snapshots, generalized proposals, and deterministic operation replay.

### Technical Deliverables

- Create collaboration ADRs for CRDT versus operation-log strategy, identity, permissions, conflict policy, offline behavior, and retention.
- Extend [`crates/devil-protocol/src/lib.rs`](crates/devil-protocol/src/lib.rs) with collaborator identity, workspace session, document operation, presence, cursor, selection, version vector, operation acknowledgement, and shared proposal DTOs.
- Insert collaboration operation layer between editor transactions and downstream consumers.
- Convert local editor transactions into collaborative operations and merge remote operations into editor transactions without blocking input.
- Add shared proposal flows for multi-user AI actions, LSP refactors, plugin commands, and file operations.
- Add awareness UI projections for presence, selections, comments, proposal approvals, agent activity, and conflict states.
- Persist operation metadata and causal order without storing full source snapshots by default.

### Acceptance Criteria

- Concurrent edits converge deterministically.
- Undo semantics are explicit for local and collaborative operations.
- Dirty buffers are never overwritten by remote operations without conflict handling.
- Shared proposals record approvers, denials, policy decisions, and applied operation IDs.
- Collaboration survives disconnect and can replay metadata to recover state.
- Collaboration does not block local keystrokes or viewport rendering.

### Resource Allocation

- Collaboration Systems Lead: 1.0 FTE
- Distributed Systems Engineer: 1.0 FTE
- Editor Engineer: 0.8 FTE
- UI Engineer: 0.8 FTE
- Storage/Replay Engineer: 0.7 FTE
- QA/Convergence Engineer: 0.8 FTE

---

## Phase 7 - Build Edge-Executed Cloud Remote Development

### Objective

Add remote development after local safety, semantic fabric, proposals, and collaboration are stable. Remote development must be modeled as capability-scoped remote ports, not raw network helpers. Remote filesystem, process, terminal, LSP, semantic index, and edge workspace execution must remain clients of the same proposal, policy, event, and storage contracts.

### Technical Deliverables

- Create remote-development ADRs for edge workspace agent, local UI projection model, transport, authentication, authorization, secrets, storage, process isolation, and enterprise policy.
- Add protocol DTOs for remote workspace lifecycle, remote filesystem snapshots, remote file operations, remote process/PTTY sessions, remote LSP sessions, remote semantic index queries, latency hints, and offline resume.
- Implement an edge workspace agent owning filesystem, process, LSP, semantic index, and terminal execution.
- Treat remote reads and writes as capability-scoped workspace requests.
- Reuse generalized proposals for remote edits and file operations.
- Use local optimistic projections, remote acknowledgement, operation logs, and version vectors to hide latency while preserving conflict safety.
- Add encrypted transport, session resumption, edge cache invalidation, remote observability correlation, and provider egress controls.
- Integrate remote development with collaboration identity, operation, and proposal semantics.

### Acceptance Criteria

- Local UI remains responsive during latency, reconnect, and degraded-network events.
- Remote file writes cannot bypass proposal and fingerprint preconditions.
- Remote terminals and LSP servers cannot launch in untrusted workspaces or outside policy.
- Remote events correlate with local events using non-zero causality and correlation IDs.
- Offline resume reconciles operation logs without silent data loss.
- Edge workspace agent enforces capability, path, process, and egress policies.

### Resource Allocation

- Remote Platform Lead: 1.0 FTE
- Distributed Systems Engineer: 1.0 FTE
- Security/Transport Engineer: 1.0 FTE
- Workspace/VFS Engineer: 0.8 FTE
- LSP/Terminal Engineer: 0.7 FTE
- QA/Resilience Engineer: 0.8 FTE

---

## Phase 8 - Product Hardening, Governance, and Ecosystem Readiness

### Objective

Move from scaffolding to production readiness across storage, diagnostics, replay, privacy, policy, migrations, and operational health. This phase closes the gap between internal platform capability and broad external usage.

### Technical Deliverables

- Move storage from [`InMemoryStorageRepositoryPort`](crates/devil-storage/src/lib.rs) to durable, migrated stores for sessions, trust, file metadata, proposal audit, event metadata, tracker, memory, semantic indexes, plugin state, collaboration logs, and remote session metadata.
- Expand [`crates/devil-cli/src/main.rs`](crates/devil-cli/src/main.rs:5) into diagnostics for dependency graph, protocol schema, event summaries, proposal audit, storage health, index health, plugin sandbox state, AI run manifests, collaboration replay, and remote traces.
- Add privacy-preserving metrics with subsystem budgets for edit, render, index, LSP, AI, plugin, collaboration, and remote operations.
- Add enterprise policy profiles for air-gap, local-only AI, approved provider lists, plugin allowlists, remote workspace restrictions, collaboration data retention, and audit export.
- Add corruption recovery drills, replay drills, downgrade/migration tests, provider egress tests, sandbox escape tests, collaboration convergence tests, and remote reconnect tests.

### Acceptance Criteria

- Metadata-only replay reconstructs critical user flows without retaining raw source by default.
- Storage migrations are reversible or recoverable.
- Privacy and retention defaults are enforced by tests.
- Every active subsystem has event coverage and operational health diagnostics.
- Enterprise policy profiles can be validated automatically in CI.
- Full phase gates pass: [`cargo run -p xtask -- check-deps`](xtask/src/main.rs), [`cargo fmt --all --check`](Cargo.toml), [`cargo check --workspace --all-targets`](Cargo.toml), [`cargo test --workspace --all-targets`](Cargo.toml), [`cargo clippy --workspace --all-targets -- -D warnings`](Cargo.toml), and CI [`cargo deny check`](deny.toml).

### Resource Allocation

- Platform Hardening Lead: 1.0 FTE
- Storage Engineer: 1.0 FTE
- Observability Engineer: 1.0 FTE
- Security/Compliance Engineer: 0.8 FTE
- CLI/Tooling Engineer: 0.6 FTE
- QA/Release Engineer: 1.0 FTE

---

## Cross-Phase Role Model

| Role | Core Accountability | Peak Allocation |
|---|---|---:|
| Chief/Principal Architect | Architecture gates, ADR approval, dependency direction, sequencing enforcement | 0.5-1.0 FTE |
| Technical Program Manager | Milestone planning, risk tracking, cross-team dependencies, release readiness | 0.5-1.0 FTE |
| Rust Platform Engineers | Protocol, app composition, workspace, storage, policy, runtime integration | 3-5 FTE |
| Editor/Text Engineers | Chunked snapshots, viewport projections, incremental line metrics, performance | 2-3 FTE |
| UI Engineers | Projection rendering, tabs/panels, overlays, presence, proposal UX, privacy inspector | 1-3 FTE |
| Semantic/LSP Engineers | Indexing, tree-sitter, symbol graph, diagnostics, completion, semantic cache | 2-3 FTE |
| AI/Agent Engineers | Provider routing, context manifests, agent state machine, verification, replay | 2-3 FTE |
| Security/Privacy Engineers | Capability broker, air-gap, plugin sandbox, remote auth, egress policy | 1-3 FTE |
| Distributed Systems Engineers | Collaboration operations, remote transport, edge agent, offline resume | 2-3 FTE |
| QA/Automation Engineers | Contract tests, phase gates, benchmarks, adversarial tests, replay drills | 2-4 FTE |
| DevEx/CLI Engineers | Diagnostics, schema tools, policy introspection, health checks | 1-2 FTE |

Recommended steady-state team size for the full program is 12-18 engineers plus architecture, TPM, product, design, and security leadership. The minimum viable execution pod for Phases 0-2 is 5-7 engineers.

---

## Required ADR Backlog

Before implementation, create or revise the ADRs identified in [`plans/architecture-review-2026-ide-roadmap-v0.1.md`](plans/architecture-review-2026-ide-roadmap-v0.1.md:394):

1. Streaming text storage, chunked snapshots, and viewport projection ADR.
2. Generalized proposal service and multi-file atomicity ADR.
3. Durable event, audit, replay, and storage retention ADR.
4. Semantic fabric, tree-sitter, symbol graph, lexical index, and vector store ADR.
5. LSP runtime supervision and semantic cache ADR.
6. AI provider router, context manifest, and Privacy Inspector ADR.
7. Agent state machine, tool policy, approval, and recovery ADR.
8. Tracker schema and AI run ledger revision based on [`plans/adrs/ADR-0008-tracker-schema.md`](plans/adrs/ADR-0008-tracker-schema.md:1).
9. Memory consent and retention revision based on [`plans/adrs/ADR-0009-memory-consent.md`](plans/adrs/ADR-0009-memory-consent.md:1).
10. WASM plugin ABI, WASI sandbox, capability host calls, and plugin storage ADR.
11. Collaboration operation log or CRDT ADR.
12. Remote edge workspace agent, transport, and edge execution ADR.
13. Enterprise policy and air-gap enforcement ADR.
14. Dependency policy completeness and architecture gate ADR.

---

## Non-Negotiable Sequencing Constraints

- Do not enable AI-generated edits until generalized proposals, observability, tracker records, and privacy manifests are implemented.
- Do not enable untrusted extensions until WASM sandboxing, capability host calls, storage quotas, and proposal-only mutation are implemented.
- Do not enable remote workspace writes until local proposal and conflict semantics are generalized and tested.
- Do not enable collaborative editing until the editor text model supports operation replay, snapshot leases, and convergence tests.
- Do not enable predictive semantic indexing on the live edit path until text supports incremental line metrics, chunked snapshots, and bounded background queues.
- Do not activate placeholder crates without dependency-policy entries, ADR acceptance, protocol contracts, and tests.
- Do not move editor session or text ownership into [`crates/devil-ui/src/ui.rs`](crates/devil-ui/src/ui.rs).
- Do not bypass proposal-mediated saves through direct platform file helpers.

---

## Risk Assessment and Mitigation Plan

| Risk | Impact | Likelihood | Phase Exposure | Mitigation |
|---|---|---:|---|---|
| Governance drift through missing dependency-policy entries | Architecture erosion and hidden coupling | High | Phase 0 onward | Make [`xtask::validate_dependency_policy()`](xtask/src/main.rs:117) fail on missing policy and require ADR/policy/contract tests before crate activation |
| Full-buffer materialization blocks large files and semantic workloads | Editor lag, memory pressure, inability to support remote/collab/AI | High | Phase 1 | Replace full projections with viewport slices; add chunked snapshots, incremental line metrics, degraded mode, and performance gates |
| Proposal leakage reappears through LSP, plugin, AI, or remote edits | Data loss, nondeterminism, audit gaps | High | Phase 2 onward | Centralize proposal lifecycle; add import-boundary tests and mutation-path audits; preserve existing save-conflict tests |
| Multi-file proposal atomicity is under-specified | Partial writes and unrecoverable user state | Medium | Phase 2 | Define atomicity ADR, rollback records, partial-failure semantics, closed/open-buffer coordination, and audit trails |
| Semantic fabric competes with input latency | Core IDE feels slow under indexing and LSP load | High | Phase 3 | Use bounded queues, cancellation, priority scheduling, snapshot leases, and stale-work invalidation |
| AI control plane becomes nondeterministic or over-privileged | User trust failure and security exposure | High | Phase 4 | Agents are proposal clients only; enforce context manifests, approval states, redacted events, replayable run records, and air-gap gates |
| Cloud provider egress violates privacy expectations | Compliance and trust failure | Medium | Phase 4 | Default to local providers; require provider allowlist, redaction, privacy inspector, air-gap tests, and explicit outbound policy |
| WASM sandbox escape or ambient access | Source exposure, process/network abuse | Medium | Phase 5 | No ambient WASI capabilities; capability-checked host calls; quotas; adversarial sandbox tests; crash isolation |
| Collaboration convergence failures | Data corruption, broken team workflows | Medium | Phase 6 | Choose CRDT/operation-log ADR; build convergence test suite; persist causal metadata; define undo semantics explicitly |
| Remote edge agent bypasses local safety model | Data loss or remote execution abuse | Medium | Phase 7 | Treat remote agent as capability-scoped client; reuse proposals, fingerprints, policy, event IDs, encrypted transport, and operation logs |
| Durable storage migration/corruption | Loss of sessions, audit trails, trust settings, plugin state | Medium | Phase 8 | Migration tests, recovery drills, backups/export, checksums, downgrade strategy, metadata-only replay validation |
| Observability records leak sensitive source | Privacy regression | Medium | All phases | Default metadata-only redaction; reject zero IDs; retention labels; privacy tests; no raw snapshots by default |
| Cross-platform OS behavior diverges | Windows-first implementation blocks parity | Medium | Phases 3-8 | Keep OS code in platform boundaries; add support matrix; validate watcher, process, PTY, path, keychain, and network behavior per OS |

---

## Milestone Definition of Done Summary

| Milestone | Definition of Done |
|---|---|
| M0: Governance Locked | Dependency policy covers all crates; missing policy fails; stale docs corrected; ADR requirement enforced |
| M1: Scalable Editor Substrate | UI receives viewport/chunk projections; large files avoid full materialization; background work cannot block input |
| M2: Universal Proposal Lifecycle | All mutation sources share typed proposals, preview, approval, apply, rollback, conflict, stale, denied, and audit flows |
| M3: Predictive Semantic Fabric | Indexing, LSP fusion, symbol graph, and query APIs work with cancellation and live snapshot priority |
| M4: Policy-Bound AI Control Plane | AI provider routing, agent state machine, context manifests, privacy inspector, air-gap, and replay records exist |
| M5: Sandboxed Plugin Runtime | WASM ABI, manifests, capabilities, quotas, crash isolation, and proposal-mediated plugin writes are validated |
| M6: Collaborative Operation Layer | Concurrent edits converge; shared proposals and presence exist; metadata replay recovers collaboration state |
| M7: Edge Remote Development | Remote agent provides filesystem/process/LSP/index/terminal under encrypted, policy-scoped, proposal-mediated contracts |
| M8: Ecosystem Readiness | Durable storage, migrations, diagnostics, metrics, policy profiles, replay drills, and CI gates validate production readiness |

---

## Immediate Next Handoff

The next engineering handoff should start with Phase 0, Phase 1, and Phase 2 only. Those phases remove the primary governance, scalability, and mutation-safety blockers and create the stable substrate required for semantic prediction, agentic AI, WASM plugins, collaboration, and edge remote development. Any attempt to parallelize AI, plugin, remote, or collaboration implementation before these foundations are accepted should be treated as an architectural stop condition.
