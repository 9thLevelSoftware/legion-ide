# Phase 7 Remote Development Architecture Map

## Acceptance Status

- Phase 7 acceptance: Accepted.

Phase 7 implementation evidence is accepted for the deterministic edge workspace runtime harness and app-owned local projection scope. The accepted slice includes remote protocol DTOs, dependency-policy activation, default-off `devil-remote` runtime activation, app-owned remote session composition, proposal-gated remote fixture filesystem mutation, bounded remote process/PTY/LSP/semantic descriptors, reconnect/offline metadata, security policy gates, metadata-only storage/observability, and validation artifacts. Production network transport, standalone local terminal runtime, hosted telemetry, raw-source retention, and Phase 8 operational hardening remain deferred.

## Runtime Surface Status

- `devil-remote` is an active workspace crate for deterministic edge workspace harness behavior.
- Runtime application is default-off through `RemoteRuntimeConfig::default()` and must be explicitly enabled by app-owned composition.
- Remote fixture filesystem mutation requires proposal IDs plus expected fingerprint, file content version, workspace generation, buffer version, snapshot id, capability decision, principal, correlation ID, and causality ID.
- Remote process, PTY, LSP, and semantic-query behavior is descriptor-only, bounded, cancellable where applicable, and unable to mutate local state directly.
- Production remote network transport, standalone local terminal runtime, hosted telemetry, raw-source retention, and Phase 8 production hardening remain deferred.
- UI must remain projection-only and must not depend on app, editor, project, storage, remote, terminal, or LSP internals.

## Governance Prerequisites

- Accepted ADRs: `plans/adrs/ADR-0022-remote-edge-workspace-agent.md`, `plans/adrs/ADR-0023-remote-transport-security.md`, and `plans/adrs/ADR-0024-remote-execution-boundary.md`.
- Dependency policy: `plans/dependency-policy.md` activates the limited Phase 7 `devil-remote` boundary and required Phase 7 protocol DTO symbols.
- Contract tests: `cargo test -p devil-protocol --test dto_contracts` covers remote DTO serialization and fail-closed validation helpers.
- Evidence gate: `cargo run -p xtask -- check-deps` validates accepted Phase 7 evidence artifacts and rejects unchecked final checklist items.

## Architecture Map

- `devil-protocol` owns remote identity, lifecycle, transport envelope, filesystem snapshot, filesystem operation, write precondition, process, PTY, LSP, semantic query, operation checkpoint, offline resume, and metadata-only audit DTOs.
- `devil-remote` owns deterministic remote session lifecycle, fixture filesystem metadata, proposal-gated remote fixture mutation, bounded descriptor validation, reconnect/offline state, and metadata-only remote audit summaries.
- `devil-security` owns remote capability policy and denies remote sessions, filesystem, execution, LSP, semantic query, egress, audit, and offline resume by default.
- `devil-observability` and `devil-storage` own metadata-only remote event and storage records.
- `devil-app` owns remote session composition, explicit runtime enablement, remote transport dispatch, and audit persistence.
- `devil-ui` consumes projections and emits intents only; no remote ownership is introduced.
- Durable writes remain proposal-mediated through existing proposal, editor, and workspace authority paths.

## Expected Evidence Artifacts

- `remote-architecture-map.md`
- `dependency-boundary.txt`
- `protocol-dto-contract-tests.txt`
- `remote-security-threat-model.md`
- `transport-security-tests.txt`
- `remote-agent-lifecycle-tests.txt`
- `remote-filesystem-proposal-tests.txt`
- `remote-stale-conflict-tests.txt`
- `remote-process-terminal-policy-tests.txt`
- `remote-lsp-policy-tests.txt`
- `remote-semantic-index-query-tests.txt`
- `latency-reconnect-offline-resume-tests.txt`
- `collaboration-remote-integration-tests.txt`
- `storage-observability-redaction-audit.md`
- `performance-budget-tests.txt`
- `future-surface-deferral-audit.md`
- `cargo-fmt-check.txt`
- `cargo-check-workspace-all-targets.txt`
- `cargo-test-workspace-all-targets.txt`
- `cargo-clippy-workspace-all-targets.txt`
- `cargo-deny-check.txt`
- `xtask-check-deps.txt`

## Validation Command Mapping

- Protocol DTOs: `cargo test -p devil-protocol --test dto_contracts`.
- Runtime lifecycle, filesystem, execution, reconnect, offline resume, and audit: `cargo test -p devil-remote`.
- App-owned composition and local disk preservation: `cargo test -p devil-app --test workspace_vfs_integration remote`.
- Security capabilities: `cargo test -p devil-security remote`.
- Storage/observability redaction: `cargo test -p devil-storage remote_audit` and `cargo test -p devil-observability remote_audit`.
- Governance: `cargo run -p xtask -- check-deps`.
- Global gates: `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, and cargo-deny baseline notes.

## Final Validation Checklist

- [x] Phase 7 ADRs are accepted.
- [x] Dependency policy activates only approved Phase 7 runtime boundaries.
- [x] `devil-remote` exists only after approved ADR, policy, protocol, tests, and evidence gates.
- [x] Remote writes and file operations are proposal-mediated and enforce all required preconditions.
- [x] Remote process, PTY, terminal, LSP, semantic query, and egress behavior is policy-gated, bounded, cancellable, and redacted.
- [x] Offline resume and reconnect reconcile deterministically or surface explicit conflicts.
- [x] Storage and observability redaction audits prove metadata-only defaults.
- [x] UI projection-only and dependency-boundary tests pass.
- [x] Every expected evidence artifact exists and is current.
- [x] Global validation gates pass locally with accepted cargo-deny baseline notes where applicable.
