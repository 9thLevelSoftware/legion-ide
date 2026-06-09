# Phase 4 Observability Redaction Audit

Date: 2026-05-25

## Scope

This audit covers Phase 4 runtime, replay, provider-route, tracker, memory, and proposal-link metadata. It verifies that observability and storage records stay metadata-only and reject raw prompt, raw provider payload, raw source, terminal output, tool-call payload, and reconstructed-file markers.

## Evidence

- `legion-protocol::validate_phase4_runtime_audit_record` rejects zero correlation ids, nil causality ids, zero event sequences, `RedactionHint::None`, zero schema versions, and forbidden raw markers in audit ids, outcome labels, route ids, run ids, step ids, and labels.
- `legion-protocol::validate_agent_replay_manifest` rejects invalid replay metadata and forbidden raw markers in transition reason codes, context references, and provider route ids.
- `legion-observability::phase4_runtime_audit_recorded_event` validates runtime audit records before creating event envelopes.
- `legion-observability::agent_replay_manifest_recorded_event` validates replay manifests before emitting replay metadata.
- `legion-storage` validates Phase 4 runtime audit records and replay manifests before persistence.
- `legion-agent`, `legion-tracker`, and `legion-memory` route their metadata through the protocol validators.

## Test evidence

- `cargo test -p legion-observability phase4_runtime_and_replay_events_are_metadata_only`
- `cargo test -p legion-storage phase4_runtime_audit_and_replay_manifest_roundtrip_metadata_only`
- `cargo test -p legion-agent --all-targets`
- `cargo test -p legion-tracker --all-targets`
- `cargo test -p legion-memory --all-targets`

## Acceptance

- [x] Phase 4 events are metadata-only.
- [x] Runtime audit and replay metadata rejects raw provider/prompt/source markers.
- [x] Storage round-trips valid metadata and fails closed for invalid metadata.
- [x] Replay manifests reconstruct runs from metadata without raw provider responses or source snapshots.
