# ADR-0027: Hosted Telemetry And Egress

Status: Accepted for production implementation direction; Phase 8 GA acceptance deferred

## Context

Current observability is metadata-only by default and air-gap policy denies hosted egress. Phase 8 must add hosted telemetry only as an explicit-consent, policy-gated, metadata-only production path.

## Decision

Implement durable telemetry spooling and hosted telemetry export behind explicit consent, endpoint policy, and feature activation. Keep the deterministic metadata-only `legion-telemetry` fixture spool and fake acknowledgement harness for conformance tests.

Production hosted telemetry must require explicit scoped consent, endpoint allowlisting, air-gap denial, structured privacy classification, bounded durable metadata-only spooling, revocation/purge behavior, retry/drop summaries, non-blocking export, and operations diagnostics.

## Required Implementation Gates

- Define telemetry categories, consent hierarchy, revocation, endpoint identity, proxy/region/certificate policy, durable spool manifest, spool TTL, retry/drop behavior, upload acknowledgements, backpressure, and deletion hooks.
- Hosted batches must never contain raw source, transcripts, process output, transport payloads, prompts, provider payloads, secrets, full environment values, or unbounded paths.
- Hosted exporter failures must never block editor input, saves, proposals, terminal lifecycle, or remote dispatch.
- Provide contract tests, egress policy tests, redaction classifier audit, consent/revoke tests, failure-mode tests, operational diagnostics, and Phase 8 evidence before GA acceptance.

## Acceptance Reservation

This ADR accepts the production implementation direction only. Phase 8 GA remains blocked until hosted telemetry is durable, opt-in, metadata-only, air-gap denied, non-blocking, diagnosable, and evidenced.
