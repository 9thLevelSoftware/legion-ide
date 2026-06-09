# Phase 6 Storage And Observability Redaction Audit

## Commands

- `cargo test -p legion-storage`
- `cargo test -p legion-observability`
- `cargo test --workspace --all-targets`

## Result

PASS for storage and observability focused tests; PASS for the full workspace test suite.

## Evidence

- `legion-storage` test `collaboration_audit_storage_roundtrips_metadata_only_and_rejects_raw_source` persists collaboration audit metadata and rejects raw transcript/source markers.
- `legion-storage` file-backed roundtrip test includes collaboration audit persistence and verifies persisted state remains metadata-oriented.
- `legion-observability` test `collaboration_audit_event_is_metadata_only_and_validated` builds and stores metadata-only collaboration audit events, rejects invalid raw-source markers, and validates non-zero core IDs through event sink validation.
- Protocol validation rejects zero session ID, zero event sequence, zero correlation ID, nil causality ID, missing schema version, audit-retained records without metadata-only redaction hints, and raw source/transcript/secret markers.

## Deferred Retention

- Full source snapshots, raw operation transcripts, secrets, and unbounded collaboration payloads are not persisted by default.
- Source-bearing operation content is limited to bounded in-memory runtime/transport DTO payloads.
