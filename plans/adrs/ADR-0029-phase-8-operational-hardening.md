# ADR-0029: Phase 8 Operational Hardening

Status: Accepted for production implementation direction; Phase 8 GA acceptance deferred

## Context

Phase 8 requires production readiness across any activated transport, terminal, telemetry, retention, storage, diagnostics, policy profile, platform, supply-chain, and release surfaces.

## Decision

Implement operational hardening as an evidence-first release gate. Keep the existing `xtask` and `devil-cli` acceptance governance: Phase 8 cannot be marked accepted while required artifacts are missing, final checklist items are unchecked, or the architecture map still declares scaffold evidence.

Production hardening requires explicit storage migrations, dry-run, backup/checksum/recovery, read-only diagnostics by default, metadata-only replay, enterprise policy profile CI, platform matrix evidence, performance budgets, failure drills, cargo-deny review, rollout controls, rollback playbooks, canary criteria, and incident runbooks.

## Required Implementation Gates

- Define storage migration/recovery requirements, diagnostics coverage, health events, release stages, feature flags, kill switches, SLOs, evidence capture, and GA stop conditions.
- Ensure hardening does not substitute for capability-specific ADR, policy, protocol, runtime, ownership, privacy, and security evidence.
- Archive platform, fault, performance, cargo-deny, release readiness, rollback, canary, and incident evidence under `plans/evidence/phase-8/` before acceptance.
- Run and archive the full workspace gate suite after any acceptance-status change.

## Acceptance Reservation

This ADR accepts the production implementation direction only. Phase 8 GA remains blocked until all required artifacts exist, the Phase 8 checklist is checked, release readiness is signed off, and `cargo run -p xtask -- check-deps` passes after the final acceptance flip.
