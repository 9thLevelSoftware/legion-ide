# ADR-0025: Production Remote Network Transport

Status: Accepted for production implementation direction; Phase 8 GA acceptance deferred

## Context

Phase 7 accepted only a deterministic edge workspace harness. Phase 8 must add production remote network transport without granting transport code local editor, workspace, UI, or disk mutation authority.

## Decision

Implement production remote transport as a typed `RemoteTransportEnvelope` carrier behind explicit policy and feature activation. Keep the deterministic metadata-only `legion-remote-transport` fixture as the conformance backend for tests.

Production transport must be app-composed, endpoint-allowlisted, encrypted/authenticated, identity-bound, schema-negotiated, replay-protected, resumable, flow-controlled, metadata-audited, and unable to mutate editor, workspace, UI, or disk state directly. Local file/editor changes received through remote traffic must continue through proposal/workspace/editor authorities and existing save/write preconditions.

First GA scope should prefer outbound `remote.transport.connect`. Inbound `remote.transport.listen` remains denied unless a later evidence update accepts listener binding, firewall, endpoint identity, and platform threat-model evidence.

## Required Implementation Gates

- Define endpoint policy, peer identity, credential/certificate reference lifecycle, TLS/mTLS or accepted equivalent, schema negotiation, replay protection, reconnect/resume, flow control, heartbeat, degraded state, and cancellation.
- Implement remote agent package manifests with version compatibility, integrity hash, authority binding, startup health, shutdown, upgrade, rollback, and fail-closed mismatch behavior.
- Keep raw socket helpers out of app/UI and preserve proposal-mediated file mutation.
- Persist only metadata summaries for health, denial, reconnect, resume, backpressure, package status, and audit.
- Provide protocol contract tests, security tests, replay/order tests, redaction/storage tests, ownership tests, fault drills, platform evidence, and Phase 8 evidence before GA acceptance.

## Acceptance Reservation

This ADR accepts the production implementation direction only. Phase 8 GA remains blocked until the required runtime, policy, security, platform, fault, and release evidence artifacts are archived and the Phase 8 architecture map is updated from `Not accepted` to `Accepted`.
