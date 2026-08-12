# ADR-0023: Remote Transport And Security Policy

## Status

Accepted for Phase 7 deterministic transport, policy, and metadata-only audit validation.

## Context

Remote development requires transport envelopes, identity, authorization, egress, reconnect, and audit records. Existing security policy is deny-by-default and observability/storage reject invalid event metadata.

## Decision

Remote transport uses `RemoteTransportEnvelope` with non-zero correlation IDs, non-nil causality IDs, non-zero event sequences, principal metadata, redaction hints, and schema versions. Invalid envelopes fail closed.

Remote capability policy is added to `legion-security` as `RemoteDevelopmentPolicy`. Remote capabilities are denied by default, require trusted workspaces by default, and are independently gated for session connection, filesystem, execution, LSP, semantic query, audit export, and offline resume. Non-loopback egress is denied by the existing air-gap/local-provider-only policy unless a future accepted policy profile changes it.

Remote audit records use `RemoteAuditRecord`, `validate_remote_audit_record`, `remote_audit_recorded_event`, and `StorageRepositoryRequest::SaveRemoteAuditRecord`. Durable storage is metadata-only and rejects raw source, raw transcripts, process output, transport payload bodies, and secrets.

## Rejected Alternatives

- Ambient remote authority after workspace trust: rejected because each remote action needs explicit capability and feature policy.
- Persisting transport payloads or terminal output for replay: rejected because Phase 7 audit defaults are metadata-only.
- Hosted telemetry: rejected as outside Phase 7 acceptance.

## Consequences

- Remote policy tests prove default denial, untrusted denial, filesystem-only enablement, and air-gap egress denial.
- Observability and storage tests prove remote audit records preserve non-zero event identity and metadata-only redaction.
