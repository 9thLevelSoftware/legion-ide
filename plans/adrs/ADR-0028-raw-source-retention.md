# ADR-0028: Raw-Source Retention

Status: Accepted for production implementation direction; Phase 8 GA acceptance deferred

## Context

The accepted storage and observability posture is metadata-only by default. Phase 8 may add raw-source retention only as a controlled, encrypted, explicit-consent exception isolated from normal metadata paths.

## Decision

Implement an encrypted raw-source vault behind explicit consent, purpose, scope, TTL, policy, and feature activation. Keep the deterministic descriptor-only `devil-retention` fixture vault for policy and lifecycle conformance tests.

Production raw-source retention must be default-deny, explicit-consent, purpose-bound, TTL-bound, encrypted, integrity-checked, key-reference-bound, access-audited, deletable, recoverable, isolated from normal audit/telemetry/storage records, and referenced by descriptor rather than inlined content.

## Required Implementation Gates

- Define accepted purposes, consent scope, capture limits, encryption/key management, access control, tombstones, revocation, deletion guarantees, TTL scanner, backup/restore, corruption recovery, and hosted upload posture.
- Hosted raw bundle upload is not accepted unless separate raw-source export consent, endpoint policy, encryption decision, audit, air-gap denial, and platform evidence are present.
- Normal telemetry, observability, remote, terminal, AI, plugin, collaboration, diagnostics, and storage metadata records must continue rejecting raw-source markers.
- Provide contract tests, lifecycle tests, privacy audits, migration/recovery tests, deletion/revocation evidence, and Phase 8 evidence before GA acceptance.

## Acceptance Reservation

This ADR accepts the production implementation direction only. Phase 8 GA remains blocked until encrypted vault behavior, scoped consent, deletion guarantees, recovery, privacy, and release evidence are archived.
