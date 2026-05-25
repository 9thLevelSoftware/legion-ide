# Phase 4 Native Agentic AI Execution Context Evidence

Date: 2026-05-25

## Scope

This document records accepted implementation evidence for Phase 4 of [`implementation-plan.md`](../../implementation-plan.md). Phase 4 activates a local-provider, proposal-only, metadata-audited agentic AI runtime slice. The accepted runtime does not grant AI direct mutation authority over editor buffers, disk, terminal, tracker, memory, settings, storage, or workspace state.

Cloud providers, hosted telemetry, hosted embeddings, gateway execution, vector storage, vector retrieval, plugins, terminal execution, collaboration, and remote development remain deferred behind separate gates.

## Acceptance status

- Phase 4 acceptance: Accepted.
- Runtime surface status: Accepted local-provider agent runtime is active through app-owned composition and protocol-mediated runtime crates.
- Provider posture: Deterministic local provider is active for tests; hosted/cloud providers remain disabled or refused.
- Mutation posture: Generated edits route through generalized proposals and require existing proposal lifecycle approval before authority-owned apply.
- Gate behavior: [`xtask`](../../../xtask/src/main.rs) validates Phase 4 evidence status, required artifact listing, artifact file presence, and checked final validation items.

## Governance prerequisites

- [`dependency-policy.md`](../../dependency-policy.md) permits the Phase 4 app composition edges to `devil-agent`, `devil-ai`, `devil-ai-providers`, `devil-tracker`, and `devil-memory`.
- Phase 4 runtime crates keep protocol DTOs as the boundary and do not depend on app, UI, editor, or workspace ownership.
- `xtask check-deps` validates required protocol symbols and the Phase 4 runtime boundary tests.
- Existing proposal-mediated save conflict and dirty-buffer preservation behavior remains covered by app integration tests.

## Architecture map

- `devil-protocol` owns Phase 4 DTOs, validation helpers, storage request/response variants, route metadata, agent transition metadata, replay manifests, and proposal-only assisted edit conversion.
- `devil-security` owns AI provider, network, loopback, air-gap, local-provider, remote-provider, tracker, memory, and tool-planning capability decisions.
- `devil-ai` owns the `ProviderRouter`, validates route metadata, asks the capability broker before provider invocation, refuses remote classes, and returns metadata-only route responses.
- `devil-ai-providers` owns deterministic local and stubbed provider adapters. The local adapter is test-only deterministic behavior; cloud stubs remain inactive.
- `devil-agent` owns deterministic run-state transitions and metadata replay. It emits transition, provider-route, or proposal-only outputs, not direct mutations.
- `devil-tracker` owns metadata-only run ledger records and lookup APIs.
- `devil-memory` owns opt-in memory candidate review and retention metadata. Retention requires explicit consent and deletion removes retained metadata.
- `devil-storage` persists metadata-only Phase 4 runtime audit records and replay manifests through protocol storage requests.
- `devil-observability` emits validated metadata-only Phase 4 runtime and replay events.
- `devil-app` is the composition root. It assembles context, privacy, provider routing, agent transitions, tracker/memory metadata, proposal registration, storage, and observability.
- `devil-ui` remains projection-only. It emits `CommandDispatchIntent` values for AI start, cancel, replay, and inspect commands and does not own provider calls, editor state, storage, or workspace writes.

## State machine

- Accepted states are observing, planning, proposing, waiting for approval, applying, verifying, recovering, blocked, cancelled, completed, and failed.
- Legal transitions are enforced in `devil-agent` and illegal transitions are rejected without recording metadata.
- Cancellation is represented as metadata and does not apply pending proposals.
- Replay reconstructs runtime state from `AgentReplayManifest` transition metadata and rejects raw provider/prompt/source markers.

## Provider router

- Provider calls require context manifest, Privacy Inspector reference, permission-budget reference, proposal intent, principal, workspace trust state, capability id, network target metadata, cancellation token, correlation id, causality id, event sequence, and metadata-only redaction hints.
- Local and local-loopback provider classes may proceed only after capability approval.
- Hosted remote provider classes are refused as metadata in this Phase 4 slice.
- Missing providers, completion-unavailable providers, invalid cancellation tokens, raw health labels, denied capability decisions, and untrusted workspaces fail closed.

## Context and privacy

- Context manifests are metadata-first projections of file identity, buffer descriptors, provider routes, agent steps, selected range descriptors, policy posture, freshness, and omissions.
- Privacy Inspector projections expose included, omitted, redacted, and refused context metadata along with egress posture.
- Users can inspect context selection and policy rationale through app-owned projections and UI intents without UI ownership of runtime behavior.

## Tracker and memory

- Tracker ledger records contain run ids, current state, proposal links, transition metadata, correlation, causality, event sequence, and display-safe labels.
- Memory candidate review does not retain records by default.
- Session or project retention requires explicit consent, and retained metadata can be deleted.
- Memory and tracker validation reject raw prompt, raw source, raw provider payload, and zero event sequence metadata.

## Proposal routing

- AI edit outputs convert to `WorkspaceProposal` using `AssistedAiEditProposalOutput::to_workspace_proposal()`.
- App composition registers AI proposals in the existing proposal coordinator.
- Existing proposal approval and authority-owned apply paths remain responsible for any mutation.
- AI runtime crates do not call `WorkspaceActor`, `EditorSession`, save workflows, or filesystem mutation APIs.

## Validation commands

| Gate | Command | Artifact |
| --- | --- | --- |
| Dependency policy | `cargo run -p xtask -- check-deps` | [`dependency-boundary.txt`](dependency-boundary.txt) |
| Provider router contracts | `cargo test -p devil-ai --all-targets` | [`provider-router-contract-tests.txt`](provider-router-contract-tests.txt) |
| Local provider adapters | `cargo test -p devil-ai-providers --all-targets` | [`local-provider-adapter-tests.txt`](local-provider-adapter-tests.txt) |
| Air-gap provider egress | `cargo test -p devil-security --all-targets` | [`air-gap-provider-egress-tests.txt`](air-gap-provider-egress-tests.txt) |
| Privacy inspector and context manifest | `cargo test -p devil-protocol --test dto_contracts dto_contracts_context_manifest_projection_metadata_only_and_risk_visible`; `cargo test -p devil-protocol --test dto_contracts dto_contracts_privacy_inspector_serializes_metadata_only_and_redacted`; `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_phase4_ai_run_is_context_inspectable_and_proposal_only` | [`privacy-inspector-context-manifest-tests.txt`](privacy-inspector-context-manifest-tests.txt) |
| Agent state machine | `cargo test -p devil-agent --all-targets` | [`agent-state-machine-tests.txt`](agent-state-machine-tests.txt) |
| Tracker run ledger | `cargo test -p devil-tracker --all-targets`; `cargo test -p devil-storage phase4_runtime_audit_and_replay_manifest_roundtrip_metadata_only` | [`tracker-run-ledger-tests.txt`](tracker-run-ledger-tests.txt) |
| Memory retention consent | `cargo test -p devil-memory --all-targets` | [`memory-retention-consent-tests.txt`](memory-retention-consent-tests.txt) |
| Proposal routing regression | `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_phase4_ai_run_is_context_inspectable_and_proposal_only`; `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict` | [`proposal-routing-regression.txt`](proposal-routing-regression.txt) |
| Formatting | `cargo fmt --all --check` | [`cargo-fmt-check.txt`](cargo-fmt-check.txt) |
| Workspace check | `cargo check --workspace --all-targets` | [`cargo-check-workspace-all-targets.txt`](cargo-check-workspace-all-targets.txt) |
| Workspace tests | `cargo test --workspace --all-targets` | [`cargo-test-workspace-all-targets.txt`](cargo-test-workspace-all-targets.txt) |
| Workspace clippy | `cargo clippy --workspace --all-targets -- -D warnings` | [`cargo-clippy-workspace-all-targets.txt`](cargo-clippy-workspace-all-targets.txt) |

## Expected evidence artifacts

- [`agentic-ai-architecture-map.md`](agentic-ai-architecture-map.md): ownership, state machine, provider router, context assembly, tracker, memory, proposal routing, and app composition.
- [`dependency-boundary.txt`](dependency-boundary.txt): `xtask check-deps` output proving Phase 4 edges are accepted and forbidden edges remain blocked.
- [`provider-router-contract-tests.txt`](provider-router-contract-tests.txt): provider routing, cancellation token validation, health metadata validation, structured route output, and refusal tests.
- [`local-provider-adapter-tests.txt`](local-provider-adapter-tests.txt): deterministic local provider behavior without cloud credentials.
- [`air-gap-provider-egress-tests.txt`](air-gap-provider-egress-tests.txt): cloud, hosted telemetry, hosted embeddings, gateway, and unapproved outbound denial evidence.
- [`privacy-inspector-context-manifest-tests.txt`](privacy-inspector-context-manifest-tests.txt): model-call context inspectability and redaction evidence.
- [`agent-state-machine-tests.txt`](agent-state-machine-tests.txt): legal transitions, illegal transitions, cancellation, recovery, replay, and proposal-only mutation evidence.
- [`tracker-run-ledger-tests.txt`](tracker-run-ledger-tests.txt): run ledger persistence and metadata-only validation evidence.
- [`memory-retention-consent-tests.txt`](memory-retention-consent-tests.txt): opt-in retention, candidate review, deletion, and vector-deferral evidence.
- [`proposal-routing-regression.txt`](proposal-routing-regression.txt): proof that AI edit proposals route through generalized proposals and cannot directly mutate editor/workspace state.
- [`observability-redaction-audit.md`](observability-redaction-audit.md): event and storage metadata redaction evidence.
- [`cloud-provider-deferral-audit.md`](cloud-provider-deferral-audit.md): proof cloud providers remain disabled unless the separate gate is accepted.
- [`vector-deferral-audit.md`](vector-deferral-audit.md): proof embeddings/vector storage/vector retrieval remain inactive unless separately accepted.
- [`cargo-fmt-check.txt`](cargo-fmt-check.txt): `cargo fmt --all --check` output.
- [`cargo-check-workspace-all-targets.txt`](cargo-check-workspace-all-targets.txt): `cargo check --workspace --all-targets` output.
- [`cargo-test-workspace-all-targets.txt`](cargo-test-workspace-all-targets.txt): `cargo test --workspace --all-targets` output.
- [`cargo-clippy-workspace-all-targets.txt`](cargo-clippy-workspace-all-targets.txt): `cargo clippy --workspace --all-targets -- -D warnings` output.

## Final validation checklist

- [x] Phase 4 runtime crates use protocol DTOs as boundaries and do not depend on app, UI, editor, or workspace ownership.
- [x] App is the composition root for provider routing, agent state, tracker, memory, storage, observability, and proposal registration.
- [x] UI remains projection-only and emits AI command intents without owning runtime behavior.
- [x] AI cannot directly mutate editor buffers, disk, terminal, tracker, memory, settings, storage, or workspace state.
- [x] Every provider/model call requires context manifest, Privacy Inspector reference, route decision, capability decision, redacted event metadata, and audit/replay linkage.
- [x] Users can inspect selected, omitted, redacted, and refused context metadata and egress posture.
- [x] Air-gap mode denies cloud providers, hosted telemetry, hosted embeddings, gateways, and unapproved outbound network access.
- [x] Local-provider execution is limited to trusted local or loopback routes.
- [x] Agent runs can be cancelled and replayed from metadata-only records.
- [x] Generated edits route through generalized proposals and require explicit approval before authority-owned apply.
- [x] Tracker ledger records and memory candidates reject raw prompt, raw provider payload, raw source, zero event sequence, zero correlation, and nil causality metadata.
- [x] Long-term memory retention remains opt-in and can be deleted.
- [x] Cloud provider activation remains blocked unless its explicit follow-up gate is accepted.
- [x] Embeddings, vector storage, and vector retrieval remain deferred and inactive.
- [x] `cargo run -p xtask -- check-deps` passes.
- [x] `cargo fmt --all --check` passes.
- [x] `cargo check --workspace --all-targets` passes.
- [x] `cargo test --workspace --all-targets` passes.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes.
