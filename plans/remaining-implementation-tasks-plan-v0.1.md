# Devil IDE Remaining Implementation Tasks Plan v0.1

Status: Planning handoff  
Created: 2026-05-16  
Scope: Finish the remaining work in [`plans/implementation-plan.md`](plans/implementation-plan.md:1) after reviewing the documentation and evidence under [`plans/`](plans/).

---

## 1. Reviewed documentation baseline

The current documentation set establishes three different layers of truth that must be reconciled during execution:

1. Strategic target: [`plans/implementation-plan.md`](plans/implementation-plan.md:1) and [`plans/architecture-review-2026-ide-roadmap-v0.1.md`](plans/architecture-review-2026-ide-roadmap-v0.1.md:10) define the 2026-class IDE roadmap.
2. Foundational history: [`plans/foundational-core-ide-platform-roadmap-v0.1.md`](plans/foundational-core-ide-platform-roadmap-v0.1.md:56), [`plans/foundational-core-ide-platform-implementation-plan-v0.1.md`](plans/foundational-core-ide-platform-implementation-plan-v0.1.md:48), and [`plans/milestone-0-feasibility-proofs.md`](plans/milestone-0-feasibility-proofs.md:1) capture the local IDE core build-out and accepted feasibility gates.
3. Current state evidence: [`plans/evidence/phase-0/native-shell-proof-summary.md`](plans/evidence/phase-0/native-shell-proof-summary.md:1), [`plans/evidence/phase-1/editor-text-substrate.md`](plans/evidence/phase-1/editor-text-substrate.md:1), [`plans/evidence/phase-2/proposal-mutation-substrate.md`](plans/evidence/phase-2/proposal-mutation-substrate.md:1), and [`plans/evidence/phase-3/predictive-semantic-fabric.md`](plans/evidence/phase-3/predictive-semantic-fabric.md:1) show what has actually been accepted or remains incomplete.

Older review findings in [`plans/architecture-review-full-codebase-v0.1.md`](plans/architecture-review-full-codebase-v0.1.md:22) and [`plans/architecture-review-phases-5-6-v0.1.md`](plans/architecture-review-phases-5-6-v0.1.md:25) contain historical save/UI concerns that are now corrected for manual save and projection-only UI. Treat those historical findings as traceability, not current blockers, unless they describe broader non-save proposal generalization.

---

## 2. Current implementation status

| Area | Current status | Planning consequence |
| --- | --- | --- |
| Governance and architecture truth | Milestone 0 is accepted in [`plans/milestone-0-feasibility-proofs.md`](plans/milestone-0-feasibility-proofs.md:3), and dependency policy is enforced through [`xtask/src/main.rs`](xtask/src/main.rs:70). | Phase 0 of [`plans/implementation-plan.md`](plans/implementation-plan.md:39) is effectively complete, with cleanup tasks only. |
| Editor and text substrate | Phase 1 evidence records degraded large-file mode, viewport projection, chunk descriptors, and non-blocking fake consumers in [`plans/evidence/phase-1/editor-text-substrate.md`](plans/evidence/phase-1/editor-text-substrate.md:7). | Phase 1 is accepted enough to unblock proposal and semantic work; renderer-backed UI measurements remain follow-up evidence, not a blocker. |
| Proposal mutation substrate | Phase 2 now has DTOs, routing, lifecycle state, generic save apply, registered open-buffer text edit apply, closed-file create/delete/rename apply, workspace-authorized audit-failure rollback checkpoints, batch preflight/contracts, recoverable app lifecycle snapshots, and live proposal ledger projection in [`plans/evidence/phase-2/proposal-mutation-substrate.md`](plans/evidence/phase-2/proposal-mutation-substrate.md:1). Runtime batch mutation, multi-file atomicity, multi-edit workspace edits, format/code-action execution, and future runtimes remain gated. | Do not accept Phase 3 or activate AI/plugin/remote/collaboration writes until the remaining Phase 2 gated runtime surfaces have ADR/policy/test evidence or are explicitly deferred. |
| Proposal execution handoff | [`plans/proposal-execution-lsp-runtime-gating-plan-v0.1.md`](plans/proposal-execution-lsp-runtime-gating-plan-v0.1.md:145) gives the concrete remaining checklist for proposal execution and LSP gating. | Treat this as the first actionable task list. |
| Semantic fabric | [`plans/evidence/phase-3/predictive-semantic-fabric.md`](plans/evidence/phase-3/predictive-semantic-fabric.md:13) says partial [`crates/devil-index`](crates/devil-index/src/lib.rs:1) behavior exists, but Phase 3 and LSP supervision are not accepted. | Do not mark Phase 3 accepted until all artifacts and checklist items in [`plans/evidence/phase-3/predictive-semantic-fabric.md`](plans/evidence/phase-3/predictive-semantic-fabric.md:104) are complete. |
| Semantic boundary remediation | [`plans/semantic-index-boundary-remediation-plan-v0.1.md`](plans/semantic-index-boundary-remediation-plan-v0.1.md:1) identifies current boundary problems: live filesystem discovery, full-source copies, cache freshness/privacy risk, and missing metadata persistence contracts. | This is the first Phase 3 implementation package after Phase 2B. |
| Agentic AI, plugins, collaboration, remote, hardening | Phases 4-8 in [`plans/implementation-plan.md`](plans/implementation-plan.md:280) remain future platform work. | Keep placeholder crates inert until their ADR, dependency-policy, protocol, test, and evidence gates are complete. |

---

## 3. Non-negotiable execution guardrails

1. Maintain proposal-mediated mutation. Manual saves already flow through [`SaveWorkflowService::save_active_buffer()`](crates/devil-app/src/lib.rs:1321) and [`WorkspaceActor::save_file_with_proposal()`](crates/devil-project/src/lib.rs:1622); every future mutation client must use the same safety thesis.
2. Keep UI projection-only. [`Shell`](crates/devil-ui/src/ui.rs:228) may render projection state and emit intents, but it must not own editor sessions, workspace state, or mutation authority.
3. Preserve editor ownership. [`EditorEngine`](crates/devil-editor/src/lib.rs:312) remains the editor transaction authority.
4. Preserve fail-closed workspace saves. [`WorkspaceSaveRequest`](crates/devil-project/src/lib.rs:133) requires fingerprint, file content version, workspace generation, buffer version, snapshot identity, correlation, and causality context.
5. Keep placeholder runtime crates inert until activation gates are met. [`plans/dependency-policy.md`](plans/dependency-policy.md:113) explicitly says planned runtime surfaces are placeholders only.
6. Keep semantic and LSP work non-blocking. Phase 3 must not block editor input, viewport projection, proposal validation, or save workflows as stated in [`plans/evidence/phase-3/predictive-semantic-fabric.md`](plans/evidence/phase-3/predictive-semantic-fabric.md:136).
7. Keep vector indexing deferred. [`plans/evidence/phase-3/predictive-semantic-fabric.md`](plans/evidence/phase-3/predictive-semantic-fabric.md:139) keeps embeddings, vector storage, and model-backed retrieval inactive.
8. Run the phase gates after every execution package: `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, and `cargo clippy --workspace --all-targets -- -D warnings`.

---

## 4. Execution roadmap for remaining implementation work

### R0 — Rebaseline and ADR reconciliation

Goal: remove ambiguity before code execution resumes.

Tasks:

1. Create a one-page status ledger under [`plans/`](plans/) mapping the accepted evidence state to each phase in [`plans/implementation-plan.md`](plans/implementation-plan.md:25).
2. Update [`plans/adrs/ADR-0002-ui-editor-rendering.md`](plans/adrs/ADR-0002-ui-editor-rendering.md:3) from provisional to accepted with reservations or explicitly supersede it with a renderer integration ADR, because Spike 1A is now accepted with reservations in [`plans/spikes/SPIKE-001A-result.md`](plans/spikes/SPIKE-001A-result.md:60).
3. Update or supersede [`plans/adrs/ADR-0005-storage-backends.md`](plans/adrs/ADR-0005-storage-backends.md:3) before durable semantic, tracker, memory, plugin, collaboration, remote, or replay storage implementation.
4. Add or schedule missing ADRs from [`plans/implementation-plan.md`](plans/implementation-plan.md:480): durable event/audit/replay/storage retention; AI provider router and Privacy Inspector; agent state machine; tracker run ledger revision; memory retention revision; WASM plugin ABI; collaboration operation log or CRDT; remote edge workspace agent; enterprise policy.
5. Reconcile old required ADR identifiers in [`plans/ide-core-architecture-spec-v0.1.md`](plans/ide-core-architecture-spec-v0.1.md:960) with newer ADRs such as [`plans/adrs/ADR-0016-generalized-proposal-service.md`](plans/adrs/ADR-0016-generalized-proposal-service.md:1), [`plans/adrs/ADR-0017-semantic-fabric-indexing.md`](plans/adrs/ADR-0017-semantic-fabric-indexing.md:1), and [`plans/adrs/ADR-0018-lsp-runtime-supervision.md`](plans/adrs/ADR-0018-lsp-runtime-supervision.md:1).
6. Decide whether hardcoded dependency checks in [`xtask/src/main.rs`](xtask/src/main.rs:70) should remain or be fully represented by [`plans/dependency-policy.md`](plans/dependency-policy.md:9), because repository rules still say [`xtask`](xtask/src/main.rs:70) parses policy and hardcodes required edges.

Exit criteria:

- The status ledger explicitly says Phase 0 and Phase 1 are accepted, Phase 2 is partially accepted, Phase 3 is not accepted, and Phases 4-8 are future-gated.
- ADR status ambiguity is removed or tracked as an explicit blocker.
- `cargo run -p xtask -- check-deps` still passes.

---

### R1 — Complete Phase 2B: generalized proposal execution

Goal: finish the remaining Phase 2 mutation substrate so future LSP, AI, plugin, collaboration, and remote outputs can safely become proposals without direct mutation.

Source checklist: [`plans/evidence/phase-2/proposal-mutation-substrate.md`](plans/evidence/phase-2/proposal-mutation-substrate.md:62) and [`plans/proposal-execution-lsp-runtime-gating-plan-v0.1.md`](plans/proposal-execution-lsp-runtime-gating-plan-v0.1.md:145).

Status note, 2026-05-22: lifecycle state, generic save equivalence, deny-by-default validation, open-buffer text edit apply, closed-file create/delete/rename apply, single-file workspace-edit delegation, audit-before-success rollback with workspace-authorized file checkpoints, and live proposal ledger projection are implemented and evidenced. Runtime batch mutation/rollback, multi-file atomicity, multi-edit workspace edits, format/code-action execution, and later ADR-gated runtime sources remain deferred rather than accepted.

Work packages:

1. Proposal lifecycle state store
   - Turn [`AppProposalCoordinator`](crates/devil-app/src/lib.rs:194) into a real app-domain service with persisted or recoverable proposal state.
   - Enforce validated, previewed, approved, applied, rejected, denied, stale, conflict, cancelled, failed, and rolled-back transitions through [`ProposalRequest`](crates/devil-protocol/src/lib.rs:3729) and [`ProposalResponse`](crates/devil-protocol/src/lib.rs:3747).
   - Reject missing lifecycle context rather than accepting stateless helper calls.

2. Generic save apply equivalence
   - Make generic apply for [`ProposalPayload::SaveFile`](crates/devil-protocol/src/lib.rs:1509) equivalent to the manual save path or keep it denied with a documented migration rationale.
   - Preserve caller behavior from [`SaveWorkflowService::save_active_buffer()`](crates/devil-app/src/lib.rs:1321) and preserve all preconditions used by [`WorkspaceActor::save_file_with_proposal()`](crates/devil-project/src/lib.rs:1622).

3. Deny-by-default validation for all payloads
   - Implement route-specific validation for [`ProposalPayload`](crates/devil-protocol/src/lib.rs:1509) variants: text edit, create, delete, rename, save, format, code action, workspace edit, terminal command, and batch.
   - Require principal, capability, target coverage, non-zero correlation, causality, trust, version preconditions, and rollback metadata before any privileged apply.
   - Keep terminal, plugin, remote, collaboration, command-like, and mixed routes denied until later phase gates.

4. Open-buffer apply
   - Route open-buffer mutations through [`EditorEngine`](crates/devil-editor/src/lib.rs:312) transaction APIs only.
   - Add stale snapshot rejection, buffer-version checks, undo-group rollback metadata, audit events, and dirty-buffer preservation.
   - Add integration tests for old snapshot rejection and rollback after multi-step failure.

5. Closed-file and workspace apply
    - Add workspace VFS apply paths for create, delete, rename, format, and workspace-edit proposals.
    - Require expected fingerprint or expected absence, file content version, workspace generation, path policy, trust, capability, and rollback metadata.
    - Preserve fail-closed non-atomic behavior, workspace-authorized rollback checkpoints, and dirty-open-buffer protection.

6. Batch planner
   - Implement prepare, preflight, apply, commit, audit, rollback, and finalize steps.
   - Validate [`ProposalBatchAtomicity`](crates/devil-protocol/src/lib.rs:1532), [`ProposalBatchRollbackPolicy`](crates/devil-protocol/src/lib.rs:1543), dependency edges, target coverage, route support, and partial-failure records before mutating.
   - Mixed-route batches stay denied until each route has an accepted executor.

7. Audit-before-success
   - Ensure success is impossible until metadata-only event and proposal audit records are emitted or staged.
   - Preserve event ID rejection behavior for zero [`CorrelationId`](crates/devil-protocol/src/lib.rs:161), nil [`CausalityId`](crates/devil-protocol/src/lib.rs:178), and zero [`EventSequence`](crates/devil-protocol/src/lib.rs:165).

8. UI proposal projection
   - Add app/protocol projection DTOs for proposal lists, selected preview, affected targets, warnings, approve, reject, cancel, and post-apply result.
   - Keep [`CommandDispatchIntent`](crates/devil-ui/src/ui.rs:141) as an intent-only UI boundary.

Acceptance evidence:

- Save equivalence test for manual save and generic save proposal execution.
- Open-buffer stale rejection and rollback tests.
- Closed-file conflict and path-policy tests.
- Batch all-or-nothing or exact partial-failure tests before runtime batch mutation is enabled.
- Terminal/plugin/remote/collaboration denial tests before those target kinds become first-class executable routes.
- Audit-before-success storage-failure test.
- Updated [`plans/evidence/phase-2/proposal-mutation-substrate.md`](plans/evidence/phase-2/proposal-mutation-substrate.md:1) showing which routes are accepted and which remain future-gated.

Stop condition:

- If any generalized proposal path can mutate editor or workspace state without preconditions, rollback metadata, policy, and audit, stop and revert the package.

---

### R2 — Complete Phase 3A: semantic-index boundary remediation

Goal: make [`crates/devil-index`](crates/devil-index/src/lib.rs:1) consume workspace/text/protocol authority instead of acting as an independent filesystem/text owner.

Source checklist: [`plans/semantic-index-boundary-remediation-plan-v0.1.md`](plans/semantic-index-boundary-remediation-plan-v0.1.md:46).

Work packages:

1. Workspace-authoritative discovery DTOs
   - Add protocol DTOs for workspace discovery records, snapshots, deltas, policy decisions, skip reasons, generated/binary/vendored/oversized flags, trust labels, privacy labels, and metadata-only decisions.
   - Extend [`WorkspaceActor`](crates/devil-project/src/lib.rs:410) to expose or emit discovery snapshots from its existing tree state.
   - Replace production [`RepositoryScanner::scan()`](crates/devil-index/src/lib.rs:786) usage with an importer that consumes workspace-authored DTOs.
   - Move live filesystem scanner behavior behind test fixtures or remove production access to [`std::fs`](crates/devil-index/src/lib.rs:7).

2. Chunk leases and descriptor-first indexing
   - Replace normal full-source [`SourceDocument`](crates/devil-index/src/lib.rs:935) indexing with descriptor-only metadata, changed ranges, chunk leases, and explicitly degraded bounded full text.
   - Keep [`SourceDocument::from_text_snapshot()`](crates/devil-index/src/lib.rs:1007) as a compatibility or fixture-only adapter that marks partial/degraded freshness and never persists full text.
   - Ensure large snapshots over the full-cache budget do not require [`TextSnapshot::try_full_text()`](crates/devil-text/src/lib.rs:340) for indexing.

3. Freshness and privacy cache authority
   - Prevent [`SyntaxTreeCache::get_or_parse()`](crates/devil-index/src/lib.rs:1152) from reusing file-specific outcomes across different file identities, snapshot versions, workspace generations, privacy scopes, schema versions, or parser options.
   - Either widen [`SyntaxCacheKey`](crates/devil-index/src/lib.rs:1068) for file-specific outcomes or split cacheable parser artifacts from per-file semantic extraction.
   - Add tests for identical content in different files and privacy downgrade invalidation.

4. Metadata-only semantic persistence
   - Add semantic metadata DTOs and storage requests only after protocol contract tests are accepted.
   - Store namespace, workspace, file, language, content hash, disk fingerprint reference, versions, snapshot, grammar version, model metadata version, privacy scope, provenance, symbol hashes, bounded display labels, graph edges, freshness, redaction hints, and tombstones.
   - Reject full source, chunk payloads, syntax trees with source, embeddings, vectors, provider outputs, and proposal edit bodies by default.

Acceptance evidence:

- [`plans/evidence/phase-3/semantic-fabric-architecture-map.md`](plans/evidence/phase-3/semantic-fabric-architecture-map.md)
- [`plans/evidence/phase-3/index-dependency-boundary.txt`](plans/evidence/phase-3/index-dependency-boundary.txt)
- [`plans/evidence/phase-3/repository-discovery-ignore-fingerprint.md`](plans/evidence/phase-3/repository-discovery-ignore-fingerprint.md)
- [`plans/evidence/phase-3/lexical-symbol-map-tests.txt`](plans/evidence/phase-3/lexical-symbol-map-tests.txt)
- [`plans/evidence/phase-3/tree-sitter-cache-tests.txt`](plans/evidence/phase-3/tree-sitter-cache-tests.txt)
- [`plans/evidence/phase-3/privacy-redaction-audit.md`](plans/evidence/phase-3/privacy-redaction-audit.md)
- [`plans/evidence/phase-3/vector-deferral-audit.md`](plans/evidence/phase-3/vector-deferral-audit.md)

Stop condition:

- If production [`crates/devil-index`](crates/devil-index/src/lib.rs:1) scans disk, mints workspace file identity, persists source, or depends directly on workspace/editor/app/UI crates, stop Phase 3 acceptance.

---

### R3 — Complete Phase 3B: predictive semantic fabric and LSP supervision

Goal: satisfy the full Phase 3 acceptance checklist in [`plans/evidence/phase-3/predictive-semantic-fabric.md`](plans/evidence/phase-3/predictive-semantic-fabric.md:118).

Work packages:

1. Actor-owned semantic scheduling
   - Expand [`IndexingActor`](crates/devil-index/src/lib.rs:251) to report accepted, coalesced, cancelled, rejected, degraded, and resync-required outcomes.
   - Ensure live snapshot work supersedes background scans by generation, content hash, and cancellation token.

2. Lexical maps and graph records
   - Complete symbol-file lookup before tree-sitter enrichment.
   - Populate normalized graph records for symbols, references, imports, exports, call edges, type relationships, test links, diagnostics links, ownership metadata, provenance, freshness, and privacy scope.
   - Maintain query freshness/degradation status through [`SemanticQueryResponse`](crates/devil-protocol/src/lib.rs:3180).

3. Tree-sitter integration
   - Add parser workers only after dependency and policy review.
   - Key syntax caches by content hash and grammar version, with freshness validation for file identity, privacy, snapshot, workspace generation, schema, and parser options.

4. Query APIs
   - Complete UI navigation, completion ranking, AI context selection, agent planning, test impact, and refactoring preview queries.
   - Ensure refactoring previews produce proposal-ready payloads without applying edits; [`build_rename_preview_payload()`](crates/devil-index/src/lib.rs:2005) is the current pattern.

5. LSP runtime supervision
   - Introduce LSP runtime only after dependency policy authorizes it and [`plans/evidence/phase-3/predictive-semantic-fabric.md`](plans/evidence/phase-3/predictive-semantic-fabric.md:11) remains not accepted until evidence exists.
   - Implement supervised workers, bounded queues, cancellation, timeout behavior, stale-response suppression, circuit breaking, DTO-only output, and proposal-only mutation routing as required by [`plans/proposal-execution-lsp-runtime-gating-plan-v0.1.md`](plans/proposal-execution-lsp-runtime-gating-plan-v0.1.md:111).
   - Formatting, rename, organize imports, quick fixes, refactors, and workspace edits from LSP must become [`WorkspaceProposal`](crates/devil-protocol/src/lib.rs:1472) values before preview or apply.

6. Phase 3 acceptance update
   - Capture every required artifact listed by [`PHASE3_REQUIRED_ARTIFACTS`](xtask/src/main.rs:21).
   - Only then update [`plans/evidence/phase-3/predictive-semantic-fabric.md`](plans/evidence/phase-3/predictive-semantic-fabric.md:11) from not accepted to accepted, remove the scaffold disclaimer, and check every checklist item.

Stop condition:

- If LSP or semantic workers can write buffers, write files, execute commands, block saves, block input, or return fresh query status across stale privacy/version boundaries, Phase 3 remains not accepted.

---

### R4 — Phase 4: native agentic AI execution context

Goal: implement a policy-bound AI control plane after generalized proposals and semantic fabric are accepted.

Gate requirements:

- R1, R2, and R3 complete.
- AI provider router, Privacy Inspector, agent state machine, tracker run ledger, memory retention, and air-gap ADRs accepted or revised.

Work packages:

1. Expand [`ModelProvider`](crates/devil-ai/src/lib.rs:216) capability contracts for streaming, structured output, embeddings, reranking, tool planning, context windows, cost metadata, cancellation, and provider health.
2. Implement local-provider adapters before cloud adapters; cloud providers require explicit allowlist, redaction, policy, and air-gap tests.
3. Implement [`crates/devil-agent`](crates/devil-agent/src/lib.rs:1) as a state machine: observing, planning, proposing, waiting for approval, applying, verifying, recovering, and blocked.
4. Add context manifests that cite editor snapshots, semantic symbols, retrieved chunks, tracker tasks, memory records, diagnostics, terminal summaries, and user selections.
5. Implement Privacy Inspector data from tracker and event metadata.
6. Persist AI run records, selected context, tool calls, approvals, proposal IDs, verification outputs, and redacted provider metadata in tracker storage.
7. Route generated edits through generalized proposals only and command execution through capability policy only.

Acceptance evidence:

- AI cannot mutate editor buffers, disk, terminal, tracker, memory, settings, or storage directly.
- Every model call has redacted event metadata and a context manifest.
- Users can inspect what context was sent and why.
- Air-gap mode prevents unapproved outbound access.
- Agent runs can be cancelled, resumed, and replayed from metadata.

---

### R5 — Phase 5: WASM isolated extension ecosystem

Goal: introduce untrusted extensibility without ambient access.

Gate requirements:

- Proposal execution complete.
- Plugin ADR and dependency-policy activation accepted.
- Durable plugin state and sandbox audit contracts accepted.

Work packages:

1. Create or activate a plugin runtime crate only after [`plans/dependency-policy.md`](plans/dependency-policy.md:113), ADRs, and protocol contracts are updated.
2. Define WASM ABI and host-call schemas for commands, menus, panels, status items, editor decorations, snippets, language providers, formatters, LSP registrations, workspace scanners, and passive AI context providers.
3. Implement manifest parsing, trust/signature metadata, capability declaration, activation events, compatibility ranges, and contribution registration.
4. Run plugins under WASI with no ambient filesystem, process, network, editor, workspace, storage, AI, or memory capabilities.
5. Expose host calls only through capability-checked protocol requests.
6. Route extension reads through scoped context providers and writes through generalized proposals.
7. Add per-plugin namespaces, storage quotas, CPU/memory budgets, cancellation, crash isolation, and redacted plugin events.

Acceptance evidence:

- Malicious extension tests prove no ambient source, network, process, storage, editor, workspace, AI, or memory access.
- Extension crashes do not crash the IDE.
- Extension mutations are proposal-mediated.
- ABI compatibility is covered by golden manifest and host-call schema tests.

---

### R6 — Phase 6: real-time multiplayer collaboration

Goal: add collaboration as a platform substrate without bypassing editor or workspace ownership.

Gate requirements:

- Scalable text substrate accepted.
- Generalized proposals accepted.
- Deterministic operation replay and storage retention decisions accepted.

Work packages:

1. Create collaboration ADRs for CRDT versus operation log, identity, permissions, conflict policy, offline behavior, undo semantics, and retention.
2. Extend [`crates/devil-protocol`](crates/devil-protocol/src/lib.rs:1) with collaborator identity, workspace session, document operation, presence, cursor, selection, version vector, operation acknowledgement, and shared proposal DTOs.
3. Insert a collaboration operation layer between editor transactions and downstream consumers.
4. Convert local transactions into collaborative operations and merge remote operations into editor transactions without blocking input.
5. Add shared proposal flows for multi-user AI actions, LSP refactors, plugin commands, and file operations.
6. Add UI projections for presence, selections, comments, proposal approvals, agent activity, and conflict states.
7. Persist operation metadata and causal order without storing full source snapshots by default.

Acceptance evidence:

- Concurrent edits converge deterministically.
- Undo semantics are explicit for local and collaborative operations.
- Dirty buffers are never silently overwritten by remote operations.
- Shared proposals record approvers, denials, policy decisions, and applied operation IDs.

---

### R7 — Phase 7: edge-executed remote development

Goal: add remote development as capability-scoped remote ports, not raw network helpers.

Gate requirements:

- Local proposal and conflict semantics generalized and tested.
- Collaboration identity and operation model accepted.
- Enterprise policy and transport ADRs accepted.

Work packages:

1. Create remote ADRs for edge workspace agent, local projection model, transport, authentication, authorization, secrets, storage, process isolation, and enterprise policy.
2. Add DTOs for remote workspace lifecycle, remote filesystem snapshots, remote file operations, remote process and PTY sessions, remote LSP sessions, remote semantic index queries, latency hints, and offline resume.
3. Implement an edge workspace agent owning remote filesystem, process, LSP, semantic index, and terminal execution.
4. Treat remote reads and writes as capability-scoped workspace requests.
5. Reuse generalized proposals for remote edits and file operations.
6. Use local optimistic projections, remote acknowledgement, operation logs, and version vectors to hide latency while preserving conflict safety.
7. Add encrypted transport, session resumption, edge cache invalidation, remote observability correlation, and provider egress controls.

Acceptance evidence:

- Local UI remains responsive during latency and reconnect events.
- Remote writes cannot bypass proposal and fingerprint preconditions.
- Remote terminals and LSP servers cannot launch in untrusted workspaces.
- Remote events correlate with local events using non-zero causality and correlation IDs.

---

### R8 — Phase 8: product hardening, governance, and ecosystem readiness

Goal: convert platform capability into production readiness.

Gate requirements:

- Active runtime surfaces have accepted evidence.
- Durable storage, replay, policy, diagnostics, and privacy ADRs accepted.

Work packages:

1. Move storage from in-memory repositories to durable, migrated stores for sessions, trust, file metadata, proposal audit, event metadata, tracker, memory, semantic indexes, plugin state, collaboration logs, and remote session metadata.
2. Expand [`crates/devil-cli`](crates/devil-cli/src/main.rs:1) into diagnostics for dependency graph, protocol schema, event summaries, proposal audit, storage health, index health, plugin sandbox state, AI run manifests, collaboration replay, and remote traces.
3. Add privacy-preserving metrics with budgets for edit, render, index, LSP, AI, plugin, collaboration, and remote operations.
4. Add enterprise policy profiles for air-gap, local-only AI, approved providers, plugin allowlists, remote restrictions, collaboration retention, and audit export.
5. Add corruption recovery drills, replay drills, downgrade/migration tests, provider egress tests, sandbox escape tests, collaboration convergence tests, and remote reconnect tests.
6. Add CI coverage for all global phase gates plus `cargo deny check`.

Acceptance evidence:

- Metadata-only replay reconstructs critical flows without source retention by default.
- Storage migrations are reversible or recoverable.
- Privacy and retention defaults are enforced by tests.
- Every active subsystem has event coverage and operational health diagnostics.
- Enterprise policy profiles validate automatically in CI.

---

## 5. Dependency order and parallelization rules

| Order | Work | May parallelize with | Must not parallelize with |
| --- | --- | --- | --- |
| 0 | R0 rebaseline and ADR reconciliation | Test/evidence cleanup only | Runtime activation work that depends on unresolved ADRs |
| 1 | R1 generalized proposal execution | UI projection design for proposal views | LSP apply, AI edits, plugin writes, collaboration writes, remote writes |
| 2 | R2 semantic-index boundary remediation | R1 tests that do not use semantic mutation | LSP supervision acceptance or Phase 3 acceptance claim |
| 3 | R3 semantic fabric and LSP supervision | Phase 3 evidence artifact authoring | AI control plane activation |
| 4 | R4 agentic AI | Privacy Inspector UI design and tracker schema work | Plugin, collaboration, or remote mutation shortcuts |
| 5 | R5 plugins | R8 diagnostics scaffolding for plugin health | Untrusted extension execution without sandbox quotas |
| 6 | R6 collaboration | Presence projection UI prototypes | Remote writes or collaborative AI approvals before convergence tests |
| 7 | R7 remote development | Enterprise policy profile design | Remote mutation before local and collaborative conflict semantics are accepted |
| 8 | R8 hardening | Non-invasive diagnostics throughout earlier phases | Treating hardening as a substitute for missing phase evidence |

---

## 6. Validation command set

Run after each package, archiving outputs under the relevant evidence directory:

- `cargo run -p xtask -- check-deps`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny check` once Phase 8 CI hardening begins

Focused checks to preserve during proposal and semantic work:

- `cargo test -p devil-protocol --test dto_contracts`
- `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict`
- `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_untrusted_save_is_denied_without_disk_mutation`
- `cargo test -p devil-project --test path_boundary`
- `cargo test -p devil-editor --test performance_suite -- --list`
- `cargo test -p devil-index --all-targets`

---

## 7. Immediate next handoff

Start with R0 and R1 only.

The first implementation handoff should not begin Phase 3 acceptance work until generalized proposal execution is complete enough to handle save-equivalent generic apply, open-buffer apply, closed-file apply, batch planning, rollback, deny-by-default validation, and audit-before-success. This matches the sequencing constraint in [`plans/implementation-plan.md`](plans/implementation-plan.md:550): the safety substrate must be stable before semantic prediction, AI, WASM plugins, collaboration, or remote development proceed.
