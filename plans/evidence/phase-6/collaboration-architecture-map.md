# Phase 6 Collaboration Architecture Map

## Acceptance Status

- Phase 6 acceptance: Not accepted.

This document is Phase 6 scaffold evidence, not acceptance evidence yet.

## Runtime Surface Status

- Collaboration runtime remains disabled and fail-closed.
- Protocol DTOs and `xtask` governance are present to support implementation review.
- `devil-app` collaboration proposal execution remains unsupported until accepted runtime, ownership, convergence, security, and evidence gates exist.

## Governance Prerequisites

- Accepted ADRs: `plans/adrs/ADR-0020-collaboration-operation-model.md`, `plans/adrs/ADR-0021-collaboration-identity-permissions-retention.md`.
- Dependency policy: `plans/dependency-policy.md` includes Phase 6 protocol symbols and collaboration dependency boundaries.
- Contract tests: `cargo test -p devil-protocol --test dto_contracts` covers Phase 6 DTO serialization and identity metadata validation.
- Evidence gate: `cargo run -p xtask -- check-deps` validates this scaffold in not-accepted mode and will fail closed if this file later claims acceptance without complete artifacts.

## Architecture Map

- `devil-protocol` owns cross-domain collaboration DTOs for sessions, participants, operations, version vectors, acknowledgements, gaps, presence, shared proposal approvals, transport envelopes, and metadata-only audit records.
- Future `devil-collaboration` may depend only on approved boundary crates and must not depend on app, UI, editor, project, remote, terminal, or process internals.
- `devil-app` may compose accepted collaboration through protocol/port boundaries only after the Phase 6 runtime gate is accepted.
- UI remains projection-only and may consume presence/proposal/conflict projections without text ownership.
- Durable file writes remain proposal-mediated through existing save/workspace preconditions.

## Lifecycle

- Session states are encoded as created, joining, active, degraded, reconnecting, conflict, closing, closed, and denied.
- Document operations carry workspace/file/buffer/snapshot/version/epoch/vector/principal/capability/correlation/causality preconditions.
- Acknowledgements encode accepted, duplicate, stale, gap, resync, and denied outcomes.
- Audit records are metadata-only by default and carry retention and redaction hints.

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

- Protocol DTOs: `cargo test -p devil-protocol --test dto_contracts`.
- Governance: `cargo run -p xtask -- check-deps`.
- Final acceptance gates: fmt, check, test, clippy, and cargo-deny outputs listed in the expected artifacts.

## Final Validation Checklist

- [ ] Collaboration ADRs are accepted.
- [ ] Dependency policy activates only approved Phase 6 boundaries.
- [ ] Runtime convergence tests pass for required participant counts.
- [ ] Dirty-buffer and proposal-mediated save regressions pass.
- [ ] Storage and observability redaction audits prove metadata-only defaults.
- [ ] UI projection-only tests pass.
- [ ] Every expected evidence artifact exists and is current.
- [ ] Global validation gates pass.
