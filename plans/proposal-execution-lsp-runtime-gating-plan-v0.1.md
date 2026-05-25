# Proposal Execution and LSP Runtime Gating Plan v0.1

Date: 2026-05-15

## Status

This was a markdown-only implementation handoff for the remaining generalized proposal execution and LSP runtime gating concern. It is superseded by accepted Phase 2 evidence in [`proposal-mutation-substrate.md`](evidence/phase-2/proposal-mutation-substrate.md:1) and accepted Phase 3 evidence in [`predictive-semantic-fabric.md`](evidence/phase-3/predictive-semantic-fabric.md:1).

## Scope and non-goals

### In scope

- Record current-state evidence for proposal execution and LSP gating with clickable source references.
- Define a route matrix for [`ProposalExecutionRoute`](../crates/devil-app/src/lib.rs:123).
- Specify requirements for a generalized proposal execution service that preserves proposal-mediated mutation, validation, preview, version preconditions, rollback metadata, audit, dirty-buffer preservation, and workspace conflict handling.
- Specify fail-closed behavior for unsupported, mixed, terminal, plugin, remote, collaboration, and command-like routes until accepted gates exist.
- Capture LSP runtime gating requirements from [`ADR-0018-lsp-runtime-supervision.md`](adrs/ADR-0018-lsp-runtime-supervision.md:1).
- Define future acceptance tests, validation commands, and evidence artifacts under [`plans/evidence/phase-3`](evidence/phase-3:1).

### Out of scope

- No edits to Rust source, manifests, or evidence acceptance checklists.
- No implementation of generalized proposal execution, LSP runtime supervision, terminal runtime behavior, plugin hosting, collaboration, remote workspaces, or command execution.
- Superseded by the accepted Phase 3 state recorded by [`predictive-semantic-fabric.md`](evidence/phase-3/predictive-semantic-fabric.md:11).

## Current-state evidence

1. Manual save is already proposal mediated. [`SaveWorkflowService::save_active_buffer()`](../crates/devil-app/src/lib.rs:1321) requests editor save text, builds a save proposal, validates and previews it, then writes through [`WorkspaceActor::save_file_with_proposal()`](../crates/devil-project/src/lib.rs:1622). Rejected outcomes are returned through [`AppSaveOutcome::Rejected`](../crates/devil-app/src/lib.rs:1990), preserving dirty editor text.
2. The app has a generalized routing classifier but not generalized execution. [`ProposalExecutionRoute`](../crates/devil-app/src/lib.rs:123) classifies save, editor-buffer, workspace-file, terminal, batch, metadata-only, mixed, and unsupported routes through [`ProposalExecutionRoute::for_payload()`](../crates/devil-app/src/lib.rs:135).
3. Non-save execution is intentionally fail-closed today. [`AppProposalCoordinator::unsupported_response()`](../crates/devil-app/src/lib.rs:550) returns structured unsupported rejection metadata, and [`ProposalRequest::Apply`](../crates/devil-protocol/src/lib.rs:3739) is routed to that rejection path by [`ProposalPort`](../crates/devil-app/src/lib.rs:676) implementation logic at [`AppProposalCoordinator::handle()`](../crates/devil-app/src/lib.rs:677).
4. Affected-target traversal is present and total for existing payload DTOs. [`AppProposalCoordinator::affected_target_coverage()`](../crates/devil-app/src/lib.rs:350) and [`AppProposalCoordinator::visit_payload_targets()`](../crates/devil-app/src/lib.rs:373) cover text edit, file operation, save, format, code action, terminal, workspace edit, and batch payloads without relying on save-only file identity assumptions.
5. Protocol DTOs for the generalized substrate exist. [`WorkspaceProposal`](../crates/devil-protocol/src/lib.rs:1472), [`ProposalPayload`](../crates/devil-protocol/src/lib.rs:1507), [`ProposalTargetCoverage`](../crates/devil-protocol/src/lib.rs:1617), [`BatchProposalPayload`](../crates/devil-protocol/src/lib.rs:1630), [`ProposalRollbackStep`](../crates/devil-protocol/src/lib.rs:1715), [`ProposalPartialFailureRecord`](../crates/devil-protocol/src/lib.rs:1751), [`ProposalLifecycleState`](../crates/devil-protocol/src/lib.rs:1969), [`ProposalLifecycleCommand`](../crates/devil-protocol/src/lib.rs:2141), [`ProposalRequest`](../crates/devil-protocol/src/lib.rs:3728), and [`ProposalResponse`](../crates/devil-protocol/src/lib.rs:3747) define the required contract shape.
6. [`ADR-0016-generalized-proposal-service.md`](adrs/ADR-0016-generalized-proposal-service.md:1) accepts the generalized proposal service direction, including universal lifecycle states, batch atomicity, rollback limits, deny-by-default validation, audit-before-success, metadata-only persistence, and projection-only UI constraints.
7. [`proposal-mutation-substrate.md`](evidence/phase-2/proposal-mutation-substrate.md:62) explicitly records remaining gaps: runtime apply planning beyond saves is denied; open-buffer edit execution through editor transactions and closed-file mutation through workspace VFS remain future work; future AI, plugin, LSP, collaboration, terminal, and remote runtime apply paths remain denied.
8. LSP supervision DTOs, metadata-only supervision records, stale/timeout/degraded result status, and proposal-only edit conversion are now accepted in [`predictive-semantic-fabric.md`](evidence/phase-3/predictive-semantic-fabric.md:11). Process/runtime expansion remains separately gated.
9. Project rules require proposal-mediated mutation and fail-closed saves. [`AGENTS.md`](../AGENTS.md:5) records phase gates, [`AGENTS.md`](../AGENTS.md:8) records projection-only UI, [`AGENTS.md`](../AGENTS.md:9) records save mediation and dirty-text preservation, [`AGENTS.md`](../AGENTS.md:10) records save preconditions and fail-closed fallback, and [`AGENTS.md`](../AGENTS.md:13) keeps placeholder runtime crates inert until gates exist.
10. Dependency policy does not authorize an ad hoc LSP runtime. [`dependency-policy.md`](dependency-policy.md:88) limits Phase 3 internal activation for [`devil-index`](../crates/devil-index/Cargo.toml:1), and [`dependency-policy.md`](dependency-policy.md:113) states planned runtime surfaces are policy placeholders only until activation gates are satisfied.

## Route matrix for [`ProposalExecutionRoute`](../crates/devil-app/src/lib.rs:123)

| Route | Current status | Allowed now | Required future gate | Fail-closed outcome until gate |
| --- | --- | --- | --- | --- |
| [`ProposalExecutionRoute::SaveFile`](../crates/devil-app/src/lib.rs:125) | Implemented only through the manual save workflow using [`SaveWorkflowService::save_active_buffer()`](../crates/devil-app/src/lib.rs:1321). Generic [`ProposalRequest::Apply`](../crates/devil-protocol/src/lib.rs:3739) still rejects through [`AppProposalCoordinator::unsupported_response()`](../crates/devil-app/src/lib.rs:550). | Manual save validation, preview, audit emission, and workspace write through [`WorkspaceActor::save_file_with_proposal()`](../crates/devil-project/src/lib.rs:1622). | Generalized service must wrap the current save semantics without weakening expected fingerprint, file content version, workspace generation, buffer version, snapshot id, capability, principal, correlation, causality, conflict, and dirty-buffer guarantees. | Stale, denied, conflict, failed, or unsupported outcomes return rejected proposal metadata and preserve dirty editor text through [`AppSaveOutcome::Rejected`](../crates/devil-app/src/lib.rs:1990). |
| [`ProposalExecutionRoute::EditorBuffer`](../crates/devil-app/src/lib.rs:126) | Classified for [`ProposalTargetKind::OpenBuffer`](../crates/devil-protocol/src/lib.rs:1559) targets, but non-save validation and apply are rejected. | No generalized open-buffer mutation is allowed. | Must apply only through editor-owned transactions such as [`EditorEngine::apply_edit()`](../crates/devil-app/src/lib.rs:2092), with buffer version, snapshot id, undo or rollback group metadata, preview, approval, audit, and stale-response checks. | Reject unsupported, stale, missing-precondition, or policy-denied requests before text mutation. |
| [`ProposalExecutionRoute::WorkspaceFile`](../crates/devil-app/src/lib.rs:127) | Classified for [`ProposalTargetKind::ClosedFile`](../crates/devil-protocol/src/lib.rs:1561) and [`ProposalTargetKind::PathOnly`](../crates/devil-protocol/src/lib.rs:1563), but non-save execution is rejected. | No generalized closed-file create, delete, rename, format, code action, or workspace edit apply is allowed. | Must route through workspace VFS authority with capability checks, path policy, expected fingerprint or content preconditions, conflict detection, rollback records, audit-before-success, and dirty-open-buffer protection. | Reject before disk mutation if any precondition, capability, target coverage, rollback plan, conflict, or trust gate is missing. |
| [`ProposalExecutionRoute::Terminal`](../crates/devil-app/src/lib.rs:128) | Classified for [`ProposalTargetKind::TerminalSession`](../crates/devil-protocol/src/lib.rs:1565), but terminal apply is rejected. | No terminal command execution is allowed through proposals. | Requires a terminal runtime ADR, dependency-policy entry, capability contract, command preview model, bounded output policy, cancellation model, audit contract, and tests. | Reject as unsupported or policy-denied with metadata-only diagnostics; never spawn or write terminal state. |
| [`ProposalExecutionRoute::Batch`](../crates/devil-app/src/lib.rs:129) | [`ProposalPayload::Batch`](../crates/devil-protocol/src/lib.rs:1529) is classified and targets are derived, but runtime planning and apply are rejected. | Target derivation and deterministic preview metadata only. | Must validate [`ProposalBatchAtomicity`](../crates/devil-protocol/src/lib.rs:1532), [`ProposalBatchRollbackPolicy`](../crates/devil-protocol/src/lib.rs:1543), dependency edges, target coverage, rollback steps, and partial-failure records before any mutation. | Reject if declared atomicity cannot be honored, rollback metadata is incomplete, target coverage is partial without policy approval, or any item route lacks an accepted executor. |
| [`ProposalExecutionRoute::MetadataOnly`](../crates/devil-app/src/lib.rs:130) | Classified for [`ProposalTargetKind::MetadataOnly`](../crates/devil-protocol/src/lib.rs:1573), but non-save validation and apply are still rejected by current coordinator behavior. | Metadata-only target derivation can be previewed as data, but no generalized lifecycle apply is accepted. | May apply only after proving there is no editor, disk, terminal, plugin, remote, collaboration, or command side effect and after audit persistence succeeds. | Reject if the payload carries hidden mutation, command execution, incomplete redaction, missing correlation, missing causality, or storage failure. |
| [`ProposalExecutionRoute::Mixed`](../crates/devil-app/src/lib.rs:131) | Mixed editor, workspace, terminal, metadata, or other route coverage is classified but not executable. | No mixed-route apply is allowed. | Requires all contained routes to be individually accepted plus cross-authority ordering, preflight, rollback, partial-failure, conflict, and audit semantics. | Reject before mutation unless the generalized service can prove every route can prepare and either commit atomically or record exact declared partial-failure and rollback outcomes. |
| [`ProposalExecutionRoute::Unsupported`](../crates/devil-app/src/lib.rs:132) | Empty coverage and target kinds such as [`ProposalTargetKind::RemoteWorkspace`](../crates/devil-protocol/src/lib.rs:1567), [`ProposalTargetKind::CollaborationSession`](../crates/devil-protocol/src/lib.rs:1569), and [`ProposalTargetKind::Plugin`](../crates/devil-protocol/src/lib.rs:1571) are unsupported by classifier policy. | No apply, no side effects, and no runtime activation. | Requires accepted ADR, dependency-policy entry, protocol DTOs, capability contracts, ownership tests, redaction policy, and phase evidence for each runtime surface. | Reject as unsupported with metadata-only diagnostics. |

## Generalized proposal execution service requirements

### Service boundary and ownership

- The generalized service should remain app-domain orchestration unless a later ADR explicitly approves a new crate and updates [`dependency-policy.md`](dependency-policy.md:1), [`Cargo.toml`](../Cargo.toml:1), and [`xtask`](../xtask/src/main.rs:43).
- UI remains projection-only. [`Shell`](../crates/devil-ui/src/ui.rs:228) may render proposal projection state and dispatch user intents, but must not apply proposals, mutate buffers, own workspace VFS state, or write files.
- Runtime sources such as LSP, semantic workers, plugins, terminals, collaboration, remote agents, and AI providers may produce DTOs or proposals only; they must not call editor or workspace mutation authorities directly.

### Lifecycle and state machine

- Every mutation must enter as [`WorkspaceProposal`](../crates/devil-protocol/src/lib.rs:1472) and progress through explicit [`ProposalRequest`](../crates/devil-protocol/src/lib.rs:3728) transitions to [`ProposalResponse`](../crates/devil-protocol/src/lib.rs:3747).
- The service must enforce [`ProposalLifecycleState`](../crates/devil-protocol/src/lib.rs:1969) states as a real state machine, not independent helper calls.
- Apply must require validated and previewed state, approval when policy demands approval, non-expired proposal state through [`WorkspaceProposal::is_expired()`](../crates/devil-protocol/src/lib.rs:1501), and non-stale version context through [`WorkspaceProposal::is_stale()`](../crates/devil-protocol/src/lib.rs:1495).
- Missing or stale lifecycle context must return denied, rejected, stale, or failed metadata rather than panicking or falling through to direct mutation.

### Validation and preconditions

- Deny by default for missing principal, missing capability, missing correlation, missing causality, untrusted workspace, missing target coverage, partial target coverage without explicit policy, stale preconditions, unsupported routes, invalid dependency edges, absent rollback metadata, or storage/audit failure.
- Save-equivalent file mutation preconditions remain mandatory: expected fingerprint, file content version, workspace generation, buffer version when an open buffer is involved, snapshot id when an editor snapshot is involved, required capability, principal, correlation, and causality.
- Open-buffer edits must require target buffer identity, buffer version, snapshot id, edit range validation, editor transaction preflight, and rollback or undo-group metadata.
- Closed-file edits must require workspace identity, file identity or canonical path, path policy, expected fingerprint or expected absence for create, expected file content version when known, workspace generation, capability, and conflict policy.
- Batch validation must reject impossible atomicity claims before mutation using [`ProposalBatchAtomicity`](../crates/devil-protocol/src/lib.rs:1532), [`ProposalBatchRollbackPolicy`](../crates/devil-protocol/src/lib.rs:1543), [`ProposalRollbackStep`](../crates/devil-protocol/src/lib.rs:1715), and [`ProposalPartialFailureRecord`](../crates/devil-protocol/src/lib.rs:1751).

### Preview, approval, and redaction

- Preview must be deterministic, bounded, redacted, and target-complete enough to support user or policy approval.
- Preview must disclose affected target IDs, paths when policy allows, ranges, byte counts, warnings, and rollback limits without persisting full source snapshots by default.
- Approval is an app/protocol lifecycle command through [`ProposalLifecycleCommand`](../crates/devil-protocol/src/lib.rs:2141); UI submits intent only.
- If preview cannot be generated safely, the proposal must be denied or rejected without mutation.

### Apply planning

- Apply must split into prepare, preflight, mutate, commit, audit, and finalize steps.
- Prepare must acquire current version context and capability decisions and must not mutate state.
- Preflight must prove target coverage, route support, policy allow state, rollback material availability, and conflict-free current state.
- Open-buffer mutation must use editor-owned transactions and preserve dirty text on every rejected, denied, stale, conflict, failed, cancelled, or rolled-back path.
- Closed-file mutation must use workspace VFS authority and preserve existing fail-closed write behavior from [`WorkspaceActor::save_file_with_proposal()`](../crates/devil-project/src/lib.rs:1622).
- Commit must occur only after audit metadata required for success is persisted or atomically staged.
- Failure after partial mutation must emit typed rollback or partial-failure metadata and must not erase unsaved editor contents.

### Audit, observability, and storage

- Every lifecycle response must produce metadata-only event and audit records before success is reported.
- Existing save event behavior through [`SaveWorkflowService::observe_proposal_response()`](../crates/devil-app/src/lib.rs:1431), [`event_metadata_record()`](../crates/devil-observability/src/lib.rs:376), and [`proposal_audit_record()`](../crates/devil-observability/src/lib.rs:394) is the baseline.
- Storage failure in an audit-required path must fail closed using [`ProposalFailureReason::StorageFailed`](../crates/devil-protocol/src/lib.rs:2032) or an equivalent typed failure.
- Events and stored metadata must keep raw source, full terminal command payloads, secrets, and unbounded previews out of durable records by default.

## Explicit fail-closed policy

- Unsupported routes return [`ProposalResponse::Rejected`](../crates/devil-protocol/src/lib.rs:3763) with [`ProposalRejectionReason::Unsupported`](../crates/devil-protocol/src/lib.rs:2006) or policy-denied metadata before any side effect.
- Mixed routes are denied unless every target route has an accepted executor and the orchestrator can honor declared atomicity, rollback, and audit guarantees.
- Terminal routes are denied until a terminal runtime ADR, dependency policy, capability taxonomy, cancellation model, redaction model, and contract tests exist.
- Plugin routes are denied until plugin-host governance, sandboxing, policy, DTOs, audit, and tests exist.
- Remote workspace routes are denied until remote authority, network policy, trust, latency, conflict, rollback, and audit gates exist.
- Collaboration routes are denied until merge semantics, authorship, causality, conflict, privacy, rollback, and evidence gates exist.
- Command-like LSP code actions are denied or surfaced as metadata-only unavailable actions until a separate command execution surface is accepted.
- Metadata-only proposals may not smuggle editor, disk, terminal, network, plugin, remote, or collaboration side effects.
- Any proposal that cannot express rollback or partial-failure semantics safely is denied before mutation.

## LSP runtime gating requirements

### Governance gate

- LSP runtime behavior remains inactive until [`predictive-semantic-fabric.md`](evidence/phase-3/predictive-semantic-fabric.md:11) is updated by a future implementation subtask with required evidence and without prematurely marking Phase 3 or LSP accepted.
- [`ADR-0018-lsp-runtime-supervision.md`](adrs/ADR-0018-lsp-runtime-supervision.md:13) is accepted for governance and implementation gating, not as proof that runtime behavior exists.
- Any LSP crate, dependency, process runner, or runtime surface must be authorized by [`dependency-policy.md`](dependency-policy.md:113), protocol DTO contracts, tests, and evidence before activation.

### Worker supervision and ownership

- LSP ownership stays outside UI, editor text ownership, and workspace VFS authority as specified by [`ADR-0018-lsp-runtime-supervision.md`](adrs/ADR-0018-lsp-runtime-supervision.md:17).
- Supervisors must own language-server process lifecycle, capability negotiation, health state, restart budgets, request routing, bounded shutdown, and circuit breaking.
- Workers may own JSON-RPC request state, server lifecycle state, capability cache, and in-flight cancellation state, but not buffers, workspace files, proposal application, or UI projection state.

### Request identity, cancellation, and stale suppression

- Every request must carry workspace identity, file identity, buffer identity when open, snapshot identity, buffer version, language id, timeout budget, correlation, causality, and cancellation token per [`ADR-0018-lsp-runtime-supervision.md`](adrs/ADR-0018-lsp-runtime-supervision.md:25).
- Cancellation must trigger on user cancellation, snapshot supersession, document sync incompatibility, timeout, server restart, trust revocation, or shutdown.
- Completion, hover, definition, reference, formatting, code action, rename, semantic-token, and diagnostics refresh must discard stale responses or publish stale metadata only.

### Bounded queues and timeout behavior

- Request queues must be bounded per server, workspace, and feature class as required by [`ADR-0018-lsp-runtime-supervision.md`](adrs/ADR-0018-lsp-runtime-supervision.md:41).
- Queue saturation must not block editor input, viewport projection, proposal validation, or save workflows.
- Timeouts must produce typed timeout, stale, degraded, unavailable, or cancelled outcomes rather than panics or blocking retries, consistent with [`ADR-0018-lsp-runtime-supervision.md`](adrs/ADR-0018-lsp-runtime-supervision.md:49).
- Repeated failures must trip bounded restart and circuit-breaker behavior rather than corrupting editor, workspace, save, or index state.

### DTO-only output and proposal-only mutation routing

- Diagnostics, completion, hover, definition, reference, semantic-token, formatting, rename, and code-action flows must use protocol DTOs before runtime behavior depends on them.
- Formatting, rename, organize imports, quick fixes, refactors, and workspace edits from LSP must be translated into [`WorkspaceProposal`](../crates/devil-protocol/src/lib.rs:1472) payloads before preview, approval, or application.
- LSP workers must never mutate editor buffers, write workspace files, execute commands, or route around the proposal service.
- If LSP output cannot be represented safely as proposal DTOs with target coverage, version preconditions, capability requirements, rollback expectations, preview summaries, and privacy metadata, it must be denied with metadata-only diagnostics per [`ADR-0018-lsp-runtime-supervision.md`](adrs/ADR-0018-lsp-runtime-supervision.md:63).

## Future implementation handoff checklist

- [x] Keep Phase 3 and LSP supervision in not-accepted state until evidence satisfies [`predictive-semantic-fabric.md`](evidence/phase-3/predictive-semantic-fabric.md:118).
- [x] Replace save-specific proposal orchestration with an app-domain generalized service while preserving [`SaveWorkflowService::save_active_buffer()`](../crates/devil-app/src/lib.rs:1321) caller behavior and [`WorkspaceActor::save_file_with_proposal()`](../crates/devil-project/src/lib.rs:1622) preconditions.
- [x] Make generic apply for [`ProposalPayload::SaveFile`](../crates/devil-protocol/src/lib.rs:1519) equivalent to the existing manual save path or explicitly keep generic apply denied with documented rationale until migration completes.
- [x] Implement deny-by-default validation for [`ProposalPayload::TextEdit`](../crates/devil-protocol/src/lib.rs:1511), [`ProposalPayload::CreateFile`](../crates/devil-protocol/src/lib.rs:1513), [`ProposalPayload::DeleteFile`](../crates/devil-protocol/src/lib.rs:1515), [`ProposalPayload::RenameFile`](../crates/devil-protocol/src/lib.rs:1517), [`ProposalPayload::FormatFile`](../crates/devil-protocol/src/lib.rs:1521), [`ProposalPayload::CodeAction`](../crates/devil-protocol/src/lib.rs:1523), [`ProposalPayload::WorkspaceEdit`](../crates/devil-protocol/src/lib.rs:1525), [`ProposalPayload::TerminalCommand`](../crates/devil-protocol/src/lib.rs:1527), and [`ProposalPayload::Batch`](../crates/devil-protocol/src/lib.rs:1529).
- [x] Add open-buffer apply through editor transactions with stale snapshot rejection, undo-group rollback, audit, and dirty-buffer preservation.
- [x] Add closed-file apply through workspace VFS with expected fingerprint, file content version, workspace generation, path policy, capability checks, conflict handling, workspace-authorized rollback checkpoints, and audit for create/delete/rename routes.
- [x] Add batch planning that validates route support, deterministic ordering, dependency edges, atomicity, rollback policy, target coverage, preflight, commit, rollback, and partial-failure records before enabling accepted reversible runtime batch mutation.
- [x] Keep terminal, plugin, remote, collaboration, command-like, and mixed routes denied until their corresponding ADRs, dependency policy entries, protocol DTOs, ownership contracts, and tests exist.
- [x] Add accepted reversible batch mutation/rollback, multi-file workspace-edit execution, and edit-only code-action execution after exact preflight/apply/audit/rollback evidence exists. Raw format execution remains lowered through proposal-safe edit payloads.
- [ ] Add LSP supervision only after proving supervised workers, bounded queues, cancellation, timeout behavior, stale-response suppression, DTO-only output, and proposal-only mutation routing.
- [x] Archive validation outputs in [`plans/evidence/phase-3`](evidence/phase-3:1) and mark acceptance after every checklist item is satisfied.

## Acceptance tests for future implementation subtasks

### Proposal execution tests

- Save equivalence: manual save and generic save proposal execution preserve expected fingerprint, file content version, workspace generation, buffer version, snapshot id, principal, capability, correlation, causality, audit, and dirty-buffer guarantees.
- Open-buffer stale rejection: an edit proposal targeting an old buffer version or snapshot id is rejected before editor mutation.
- Open-buffer rollback: a multi-step edit failure rolls back through editor-owned transaction metadata and preserves dirty text.
- Closed-file conflict: a closed-file edit proposal rejects when expected fingerprint or workspace generation no longer matches and does not mutate disk.
- Closed-file path policy: create, delete, rename, format, and workspace edit proposals reject blocked paths and untrusted workspaces before mutation.
- Batch all-or-nothing: a batch with one failing item mutates nothing or records exact rollback and partial-failure metadata that matches declared atomicity.
- Mixed-route denial: editor plus workspace plus terminal batches reject until the orchestrator can prove safe cross-authority ordering and rollback.
- Terminal denial: terminal command proposals remain rejected and do not spawn processes until terminal runtime gates exist.
- Plugin, remote, and collaboration denial: target kinds represented by [`ProposalTargetKind::Plugin`](../crates/devil-protocol/src/lib.rs:1571), [`ProposalTargetKind::RemoteWorkspace`](../crates/devil-protocol/src/lib.rs:1567), and [`ProposalTargetKind::CollaborationSession`](../crates/devil-protocol/src/lib.rs:1569) reject before side effects.
- Metadata-only safety: metadata-only proposals apply only if no mutation side effect exists and audit metadata is persisted successfully.
- Audit-before-success: storage failure during required audit returns typed failure and no mutation success is reported.

### LSP runtime gating tests

- Trust gate: untrusted workspace cannot launch or connect supervised LSP workers.
- Supervision: server crash, malformed response, protocol violation, timeout, and restart storm are isolated to worker health state.
- Queue bounds: saturated queues degrade or reject low-priority work without blocking editor input, viewport projection, proposal validation, or save workflows.
- Cancellation: superseded snapshot, timeout, user cancellation, trust revocation, and shutdown cancel in-flight requests and suppress stale results.
- DTO contracts: diagnostics, completion, hover, definition, reference, semantic-token, formatting, rename, and code-action responses round-trip through protocol DTOs.
- Proposal routing: edit-producing formatting, rename, organize-imports, quick-fix, and refactor outputs become proposals and never mutate buffers or disk directly.
- Command-like denial: command-only and mixed command/edit code actions are denied or policy-routed as unavailable metadata until command execution gates exist.
- Non-blocking saves: saves do not wait for LSP diagnostics, formatting, server health, document sync, or semantic graph freshness.

## Validation commands and expected artifacts

These commands are for future implementation subtasks. This markdown-only subtask does not run them.

| Gate | Command | Expected artifact under [`plans/evidence/phase-3`](evidence/phase-3:1) |
| --- | --- | --- |
| Dependency policy | `cargo run -p xtask -- check-deps` | [`check-deps.txt`](evidence/phase-3/check-deps.txt) and [`index-dependency-boundary.txt`](evidence/phase-3/index-dependency-boundary.txt) |
| Formatting | `cargo fmt --all --check` | [`cargo-fmt-check.txt`](evidence/phase-3/cargo-fmt-check.txt) |
| Workspace check | `cargo check --workspace --all-targets` | [`cargo-check-workspace-all-targets.txt`](evidence/phase-3/cargo-check-workspace-all-targets.txt) |
| Workspace tests | `cargo test --workspace --all-targets` | [`cargo-test-workspace-all-targets.txt`](evidence/phase-3/cargo-test-workspace-all-targets.txt) |
| Workspace clippy | `cargo clippy --workspace --all-targets -- -D warnings` | [`cargo-clippy-workspace-all-targets.txt`](evidence/phase-3/cargo-clippy-workspace-all-targets.txt) |
| Protocol DTO contracts | `cargo test -p devil-protocol --test dto_contracts` | [`devil-protocol-dto-contracts.txt`](evidence/phase-3/devil-protocol-dto-contracts.txt) |
| Proposal routing regression | `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_non_save_proposals_are_structurally_rejected_without_panic` | [`proposal-routing-regression.txt`](evidence/phase-3/proposal-routing-regression.txt) |
| Batch coverage regression | `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_batch_affected_targets_are_visited_in_item_order` | [`proposal-routing-regression.txt`](evidence/phase-3/proposal-routing-regression.txt) |
| Save conflict regression | `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict` | [`save-conflict-regression.txt`](evidence/phase-3/save-conflict-regression.txt) |
| Index and semantic tests | `cargo test -p devil-index --all-targets` | [`devil-index-tests.txt`](evidence/phase-3/devil-index-tests.txt), [`lexical-symbol-map-tests.txt`](evidence/phase-3/lexical-symbol-map-tests.txt), [`tree-sitter-cache-tests.txt`](evidence/phase-3/tree-sitter-cache-tests.txt), [`normalized-graph-contract-tests.txt`](evidence/phase-3/normalized-graph-contract-tests.txt), and [`semantic-query-api-tests.txt`](evidence/phase-3/semantic-query-api-tests.txt) |
| LSP supervision tests | future LSP supervisor test command after runtime gates exist | [`lsp-supervision-tests.txt`](evidence/phase-3/lsp-supervision-tests.txt) |
| Editor latency and background work | `cargo test -p devil-editor --test performance_suite -- --list` plus accepted Phase 3 performance runs | [`editor-semantic-latency.txt`](evidence/phase-3/editor-semantic-latency.txt) |
| Privacy and redaction audit | future static and integration audit after runtime gates exist | [`privacy-redaction-audit.md`](evidence/phase-3/privacy-redaction-audit.md) |
| Vector deferral audit | future static audit proving vector runtime remains inactive | [`vector-deferral-audit.md`](evidence/phase-3/vector-deferral-audit.md) |

## Required Phase 3 evidence artifacts to preserve

Future acceptance must keep or update the required artifact set already listed by [`predictive-semantic-fabric.md`](evidence/phase-3/predictive-semantic-fabric.md:104):

- [`semantic-fabric-architecture-map.md`](evidence/phase-3/semantic-fabric-architecture-map.md)
- [`index-dependency-boundary.txt`](evidence/phase-3/index-dependency-boundary.txt)
- [`repository-discovery-ignore-fingerprint.md`](evidence/phase-3/repository-discovery-ignore-fingerprint.md)
- [`lexical-symbol-map-tests.txt`](evidence/phase-3/lexical-symbol-map-tests.txt)
- [`tree-sitter-cache-tests.txt`](evidence/phase-3/tree-sitter-cache-tests.txt)
- [`normalized-graph-contract-tests.txt`](evidence/phase-3/normalized-graph-contract-tests.txt)
- [`semantic-query-api-tests.txt`](evidence/phase-3/semantic-query-api-tests.txt)
- [`lsp-supervision-tests.txt`](evidence/phase-3/lsp-supervision-tests.txt)
- [`proposal-routing-regression.txt`](evidence/phase-3/proposal-routing-regression.txt)
- [`privacy-redaction-audit.md`](evidence/phase-3/privacy-redaction-audit.md)
- [`vector-deferral-audit.md`](evidence/phase-3/vector-deferral-audit.md)

## Gating decision summary

- Current manual saves are allowed and must remain equivalent.
- Generalized single-route open-buffer text edits, closed-file create/delete/rename, and save-file proposal apply are allowed only through the accepted proposal lifecycle and authority-specific executors.
- Accepted reversible batch mutation/rollback, multi-file workspace edits, and edit-only code-action execution are evidenced in Phase 2. Raw format, command-bearing code actions, terminal/plugin/remote/collaboration/AI routes, and mixed routes without accepted executors remain deferred.
- Terminal, plugin, remote, collaboration, command-like, and mixed routes are future-gated and fail closed.
- LSP supervision DTOs, metadata-only supervision records, stale/timeout/degraded result statuses, and proposal-only edit output conversion are accepted for Phase 3. Process-launching expansion remains separately gated.
- Phase 3 is accepted with evidence under [`plans/evidence/phase-3`](evidence/phase-3:1) satisfying the final checklist in [`predictive-semantic-fabric.md`](evidence/phase-3/predictive-semantic-fabric.md:118).
