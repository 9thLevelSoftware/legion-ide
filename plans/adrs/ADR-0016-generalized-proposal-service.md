# ADR-0016: Generalized Proposal Service

## Status

Accepted for Phase 2 protocol and app-orchestration workstreams.

## Context

Phase 2 of [`plans/implementation-plan.md`](../implementation-plan.md:117) promotes the current save-specific proposal path into a universal mutation substrate. The current save path is the safety baseline: [`SaveProposalCoordinator`](../../crates/devil-app/src/lib.rs:120) builds save proposals, [`SaveWorkflowService::save_active_buffer()`](../../crates/devil-app/src/lib.rs:973) preserves dirty editor text on rejected outcomes, and [`WorkspaceActor::save_file_with_proposal()`](../../crates/devil-project/src/lib.rs:1622) enforces fingerprint, version, capability, correlation, causality, and fail-closed write semantics.

The existing contract in [`WorkspaceProposal`](../../crates/devil-protocol/src/lib.rs:1343) and [`ProposalPayload`](../../crates/devil-protocol/src/lib.rs:1378) is close enough for manual saves but incomplete for multi-file edits, future LSP/code actions, plugin actions, terminal commands, AI patches, collaboration operations, and remote workspace mutations. A save-only assumption remains in [`proposal_file_identity()`](../../crates/devil-app/src/lib.rs:1357), which will be replaced by later Phase 2 app orchestration work rather than by this protocol-only subtask.

UI remains projection-only. [`Shell`](../../crates/devil-ui/src/ui.rs:228) may render proposal state and enqueue command intents, but it must not own proposal execution, editor sessions, text mutation, workspace VFS state, or durable writes.

## Decision

Adopt a generalized proposal service contract centered on [`WorkspaceProposal`](../../crates/devil-protocol/src/lib.rs:1343), [`ProposalRequest`](../../crates/devil-protocol/src/lib.rs:2804), and [`ProposalResponse`](../../crates/devil-protocol/src/lib.rs:2823). The contract is implemented first as protocol DTOs and contract tests; later app-domain orchestration replaces save-specific coordination without weakening save guarantees.

### 1. Universal proposal lifecycle

- Every mutation source uses the same lifecycle states: created, validated, previewed, approved, rejected, applied, denied, failed, rolled back, stale, conflict, and cancelled.
- Lifecycle intents are explicit request DTOs. [`ProposalRequest`](../../crates/devil-protocol/src/lib.rs:2804) continues to support validate, preview, and apply, and adds approve, reject, cancel, and rollback commands through [`ProposalLifecycleCommand`](../../crates/devil-protocol/src/lib.rs:2007).
- Approval is app/protocol state, not UI ownership. UI submits approve, reject, or cancel as command intents; app-owned proposal orchestration decides state transitions and audit emission.
- Cancellation is distinct from rejection: cancellation means work stopped before completion due to user cancellation, supersession, expiry, shutdown, or policy cancellation.

### 2. Batch and multi-file atomicity boundaries

- Multi-target work is represented by [`BatchProposalPayload`](../../crates/devil-protocol/src/lib.rs:1499), embedded as [`ProposalPayload::Batch`](../../crates/devil-protocol/src/lib.rs:1396).
- Batch item order is deterministic. [`ProposalBatchItem::order`](../../crates/devil-protocol/src/lib.rs:1526) is the authoritative apply order, and dependency edges are explicit metadata.
- Atomicity boundaries are explicit via [`ProposalBatchAtomicity`](../../crates/devil-protocol/src/lib.rs:1401): all-or-nothing, prepare-all-before-mutate, or ordered non-atomic with mandatory partial-failure records.
- Cross-target true atomicity is guaranteed only when the responsible app/workspace/editor authority can prepare every step before mutating and can commit under the stated boundary. Heterogeneous editor, workspace, terminal, remote, and future runtime targets must not imply stronger atomicity than the DTO declares.
- A batch that cannot satisfy its declared atomicity must fail before mutating or return failed/denied/stale/conflict with metadata-only diagnostics.

### 3. Rollback semantics and limits

- Rollback policy is explicit via [`ProposalBatchRollbackPolicy`](../../crates/devil-protocol/src/lib.rs:1412): required, best-effort, not-supported, or not-required.
- Rollback plans use deterministic [`ProposalRollbackStep`](../../crates/devil-protocol/src/lib.rs:1584) records and typed [`ProposalRollbackAction`](../../crates/devil-protocol/src/lib.rs:1563) values.
- Open-buffer rollback is through editor-owned transactions or undo groups. Workspace file rollback is through VFS-owned backups, inverse operations, or file snapshots. Terminal, remote, plugin, AI, collaboration, and other future runtimes remain denied or metadata-only until their ADR-gated orchestration exists.
- If rollback cannot restore exact prior state, the service must emit a failed rollback outcome with [`ProposalPartialFailureRecord`](../../crates/devil-protocol/src/lib.rs:1620) metadata and must preserve user buffers rather than discarding dirty editor text.

### 4. Deny-by-default validation

- Validation is deny-by-default for missing principal, missing capability, stale version/fingerprint preconditions, unsupported payload kinds, missing target coverage, invalid atomicity claims, absent rollback metadata when rollback is required, or untrusted workspace decisions.
- Existing save preconditions remain mandatory for save payloads: expected disk fingerprint, file content version, workspace generation, buffer version, snapshot id, required `fs.write` capability, principal, and non-zero correlation/causality through the current save flow.
- Future AI, LSP, plugin, terminal, collaboration, and remote payloads may validate and preview as metadata-only stubs in Phase 2, but apply is denied until their runtime ADRs, policies, contract tests, and ownership boundaries exist.

### 5. Audit-before-success and metadata-only persistence

- Proposal success is not complete until lifecycle audit metadata is emitted or persisted before success is reported to callers. If audit persistence fails in a path that requires durable audit, the proposal must fail closed with [`ProposalFailureReason::StorageFailed`](../../crates/devil-protocol/src/lib.rs:1897) or an equivalent typed failure.
- Audit/event records remain metadata-only: proposal ID, state, payload kind, affected IDs, target counts, byte counts, hashes, redaction hints, principal, capability, correlation ID, causality ID, event sequence, diagnostics, rollback and partial-failure metadata.
- Raw source, full command payloads, secrets, and unbounded previews must not be persisted by default.
- Observability constraints remain mandatory: non-zero [`CorrelationId`](../../crates/devil-protocol/src/lib.rs:73), non-nil [`CausalityId`](../../crates/devil-protocol/src/lib.rs:201), and non-zero [`EventSequence`](../../crates/devil-protocol/src/lib.rs:93).

### 6. UI projection-only constraints

- UI receives proposal lists, selected previews, affected targets, warnings, and lifecycle outcomes as projection data only.
- UI commands are intents for approve, reject, cancel, preview refresh, or selection. UI never applies proposals, mutates editor text, owns workspace actors, or writes files.
- Preview payloads must be bounded and redacted. Affected-target descriptors may disclose IDs, path metadata, hashes, lengths, ranges, and warnings, but not raw source by default.

## Consequences

- **Positive**: All future mutation clients can share one lifecycle and audit model instead of creating ad hoc mutation side channels.
- **Positive**: Batch DTOs make ordering, atomicity limits, rollback policy, target coverage, warnings, and partial failures explicit before app orchestration is replaced.
- **Positive**: Current save guarantees remain the reference implementation and are not weakened by the protocol expansion.
- **Negative**: App orchestration work remains necessary to replace save-specific visitors, dispatchers, validation, and apply planning.
- **Negative**: Some future payloads remain previewable only as denied stubs until their runtime ADR gates land.

## Non-goals

- This ADR does not activate [`crates/devil-agent/src/lib.rs`](../../crates/devil-agent/src/lib.rs:1), [`crates/devil-index/src/lib.rs`](../../crates/devil-index/src/lib.rs:1), [`crates/devil-memory/src/lib.rs`](../../crates/devil-memory/src/lib.rs:1), or [`crates/devil-tracker/src/lib.rs`](../../crates/devil-tracker/src/lib.rs:1).
- This ADR does not replace [`SaveProposalCoordinator`](../../crates/devil-app/src/lib.rs:120), implement generalized apply orchestration, or remove [`proposal_file_identity()`](../../crates/devil-app/src/lib.rs:1357) in the protocol-contract subtask.
- This ADR does not introduce new crates or dependency-policy changes.

## Exit condition

The ADR is fully satisfied when protocol DTOs and contract tests exist for batch proposals and lifecycle commands, app orchestration uses a generalized proposal service instead of save-specific coordination, total payload visitors replace save-only file identity assumptions, multi-target apply either honors declared atomicity or emits rollback/partial-failure metadata, and current save conflict/dirty-buffer guarantees remain green.
