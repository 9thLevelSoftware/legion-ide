# ADR-0021: Collaboration Identity, Permissions, and Retention

## Status

Draft for Phase 6 implementation review.

## Context

Collaboration needs participant identity, admission, operation publishing, presence publishing, shared proposal approval, replay metadata, and audit export controls without granting remote filesystem, process, terminal, hosted egress, or raw-source persistence authority.

## Decision

Represent collaboration permissions explicitly in protocol DTOs and require principal, capability decision, non-zero correlation ID, non-nil causality ID, workspace/document binding, participant role, retention label, and redaction hints on operation or audit paths. Durable collaboration records default to metadata-only summaries and identifiers.

## Retention Policy

- Source-bearing operation content is allowed only in bounded in-memory transport/runtime payloads until a later accepted encrypted/session-scoped storage decision exists.
- Audit and replay records keep IDs, ordering, hashes, ranges, byte counts, redaction hints, retention labels, conflict/gap status, and proposal links.
- Raw source snapshots, full transcripts, secrets, and unbounded comments are not persisted by default.

## Consequences

- Missing or stale capability decisions deny collaboration actions before side effects.
- Observer participants may publish presence but not document operations.
- Shared proposal approvals link proposal IDs, participant IDs, policy decisions, operation IDs, and denial reasons without bypassing proposal execution gates.
