# Phase 6 Collaboration Architecture Map

## Acceptance Status

- Phase 6 acceptance: Accepted.

Phase 6 implementation evidence is accepted for the local deterministic collaboration runtime and app-owned composition scope. App-owned transport/composition, shared proposal gating, reconnect/session shutdown semantics, projection-only UI integration, metadata-only storage/observability, and p95/p99 editor overhead budgets are implemented and evidenced. Remote workspace authority, terminal/process execution, hosted telemetry, and raw-source retention remain deferred future surfaces.

## Runtime Surface Status

- `legion-collaboration` is an active workspace crate for deterministic in-memory operation-log collaboration.
- Runtime application is default-off through `CollaborationRuntimeConfig::default()` and must be explicitly enabled by an app-owned composition root.
- `legion-collaboration` depends only on protocol and utility dependencies; it has no app, UI, editor, project, remote, terminal, process, or workspace-authority dependency.
- `legion-app` owns collaboration session composition, deterministic local protocol-envelope transport handling, presence projection output, editor transaction application, shared proposal approval gates, and metadata-only audit linkage.
- Pure collaboration proposal targets remain fail-closed unless paired with an existing accepted proposal executor route and app-owned approval evidence.
- UI collaboration awareness is projection-only and emits command intents without owning editor text or workspace mutation.

## Governance Prerequisites

- Accepted ADRs: `plans/adrs/ADR-0020-collaboration-operation-model.md`, `plans/adrs/ADR-0021-collaboration-identity-permissions-retention.md`.
- Dependency policy: `plans/dependency-policy.md` includes Phase 6 protocol symbols and collaboration dependency boundaries.
- Contract tests: `cargo test -p legion-protocol --test dto_contracts` covers Phase 6 DTO serialization, shared approvals, replay manifests, audit validation, and identity metadata.
- Evidence gate: `cargo run -p xtask -- check-deps` validates accepted Phase 6 evidence artifacts and rejects unchecked final checklist items.

## Architecture Map

- `legion-protocol` owns cross-domain collaboration DTOs for sessions, participants, operations, version vectors, acknowledgements, gaps, presence, shared proposal approvals, replay manifests, transport envelopes, and metadata-only audit records.
- `legion-collaboration` owns deterministic operation ordering, replay, duplicate suppression, causal gap detection, resync acknowledgement, presence storage, metadata-only replay manifests, and metadata-only audit summaries.
- `legion-security` owns collaboration-specific capability policy and denies runtime sessions, operation publishing, shared proposal approval, replay/audit export, and non-loopback egress by default.
- `legion-storage` persists collaboration audit records as metadata-only records and rejects zero identifiers or raw-source/transcript markers.
- `legion-observability` emits collaboration audit events as metadata-only envelopes with non-zero correlation, causality, and sequence metadata.
- `legion-editor` accepts validated collaboration participant edits only through the existing editor transaction API with `TransactionSource::CollaborationParticipant`.
- `legion-app` applies accepted collaboration document operations through `EditorEngine::apply_protocol_edits()` and never lets UI or collaboration runtime own editor text.
- `legion-ui` consumes collaboration presence projections and emits app-owned intents only.
- Durable file writes remain proposal-mediated through existing save/workspace preconditions.

## Lifecycle

- Session states are encoded as created, joining, active, degraded, reconnecting, conflict, closing, closed, and denied.
- Runtime lifecycle APIs enforce reconnecting, closing, and closed fail-closed behavior for text operations and presence mutation.
- Document operations carry workspace/file/buffer/snapshot/version/epoch/vector/principal/capability/correlation/causality preconditions.
- Acknowledgements encode accepted, duplicate, stale, gap, resync, and denied outcomes.
- Audit and replay records are metadata-only by default and carry retention and redaction hints.

## Expected Evidence Artifacts

- `collaboration-architecture-map.md`
- `dependency-boundary.txt`
- `protocol-dto-contract-tests.txt`
- `collaboration-convergence-tests.txt`
- `undo-semantics-tests.txt`
- `dirty-buffer-conflict-tests.txt`
- `shared-proposal-approval-tests.txt`
- `presence-ui-projection-tests.txt`
- `collaboration-security-capability-tests.txt`
- `disconnect-reconnect-replay-tests.txt`
- `storage-observability-redaction-audit.md`
- `future-surface-deferral-audit.md`
- `performance-budget-tests.txt`
- `cargo-fmt-check.txt`
- `cargo-check-workspace-all-targets.txt`
- `cargo-test-workspace-all-targets.txt`
- `cargo-clippy-workspace-all-targets.txt`
- `cargo-deny-check.txt`

## Validation Command Mapping

- Protocol DTOs: `cargo test -p legion-protocol --test dto_contracts`.
- Runtime convergence, undo, duplicate, gap, replay, presence, reconnect, leave, and shutdown: `cargo test -p legion-collaboration --all-targets`.
- Dirty-buffer, save/proposal, app-owned collaboration composition, and editor-transaction regressions: `cargo test --workspace --all-targets` including `legion-app` integration and `legion-editor` tests.
- UI projection-only: `cargo test -p legion-ui`.
- Security capabilities: `cargo test -p legion-security`.
- Storage/observability redaction: `cargo test -p legion-storage` and `cargo test -p legion-observability`.
- Governance: `cargo run -p xtask -- check-deps`.
- Global gates: fmt, check, test, clippy, and cargo-deny pass locally; cargo-deny reports warning-level duplicate dependency findings that match the repository baseline policy.

## Final Validation Checklist

- [x] Collaboration ADRs are accepted.
- [x] Dependency policy activates only approved Phase 6 boundaries.
- [x] Runtime convergence tests pass for required participant counts in the deterministic in-memory harness.
- [x] Dirty-buffer and proposal-mediated save regressions pass.
- [x] Storage and observability redaction audits prove metadata-only defaults.
- [x] UI projection-only tests pass.
- [x] Every expected evidence artifact exists and is current.
- [x] Global validation gates pass locally with warning-level cargo-deny findings.
- [x] App-owned deterministic transport/composition acceptance is implemented and tested.
- [x] Shared proposal executor integration is gated by authorized app-owned approval evidence.
- [x] Reconnect/session shutdown and p95/p99 editor overhead acceptance evidence is current.
