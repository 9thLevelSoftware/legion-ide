# Phase 7 Storage And Observability Redaction Audit

## Status

Accepted.

## Commands

- cargo test -p legion-storage remote_audit
- cargo test -p legion-observability remote_audit
- cargo test -p legion-app --test workspace_vfs_integration remote_session

## Findings

- `RemoteAuditRecord` validation rejects zero event sequence, zero correlation ID, nil causality ID, raw source markers, raw transcript markers, process output markers, transport payload markers, and secret markers.
- `remote_audit_recorded_event` emits metadata-only envelopes with non-zero event identity.
- `StorageRepositoryRequest::SaveRemoteAuditRecord` persists metadata-only remote audit records and rejects invalid records.
- App-owned remote composition emits and stores `remote.audit_recorded` events without retaining raw source, transcripts, output, transport payloads, or secrets.
