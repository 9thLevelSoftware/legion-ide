# Phase 13 Governance Evidence: Legion Workflow Orchestration

Status: Accepted governance baseline  
Date: 2026-06-01

## Activation scope

Phase 13 may add Legion Workflow orchestration only in sequential, evidence-backed waves:

1. Governance and dependency-policy boundary.
2. Protocol DTOs, validators, projections, and readiness helpers.
3. Agent coordinator over existing delegated-task primitives.
4. Tracker/memory metadata evidence records.
5. App-owned workflow composition and approval-gated readiness.
6. UI/desktop command-center projections and app-request intents.
7. Final evidence and release-readiness proof.

## Hard boundaries confirmed

- UI/desktop authority: unsupported. UI and desktop remain projection/request-only.
- Direct main-workspace mutation by workers: unsupported. Worker outputs must become proposals or proposal-preview metadata first.
- Autonomous merge: unsupported until approval.
- Provider invocation from UI/desktop: unsupported. Provider-backed workers must route through assisted-AI consent/provider metadata.
- Raw prompt/source/log/provider payload retention: unsupported by default.
- Terminal/process execution outside terminal policy gates: unsupported.

## Required readiness blockers

Merge readiness must fail closed when any of the following are present:

- unresolved worker or file conflict;
- missing dependency;
- failed, blocked, stale, or missing verification;
- missing required sign-off;
- missing proposal id;
- stale proposal preconditions;
- dirty main workspace;
- missing audit-before-success evidence;
- missing explicit approval;
- missing rollback/checkpoint metadata;
- zero correlation id, nil causality id, zero schema version, or unsafe redaction hints.

## Crate boundary evidence

- `devil-protocol`: metadata-only contracts and validators only.
- `devil-agent`: future coordinator over delegated-task primitives only; no UI/app/editor/project/desktop authority.
- `devil-tracker`: future metadata-only workflow/evidence ledger records through existing storage boundary.
- `devil-memory`: future consent-gated memory summaries only; no raw prompt/source/log retention.
- `devil-app`: future owner of workflow execution state, verification/sign-off routing, proposal lifecycle, conflict blockers, and approval-gated merge readiness.
- `devil-ui`: projection snapshots and command intents only.
- `devil-desktop`: renderer and adapter-local presentation state only.

## Evidence required before completion

Phase 13 completion requires contract tests, coordinator tests, tracker/memory retention tests, app workflow tests, UI/desktop projection tests, dependency-policy checks, and standard repository gates. Evidence must prove that Legion Workflows remain proposal-mediated and approval-gated, with no autonomous merge/apply behavior.

## Governance conclusion

The Phase 13 activation gate is open for protocol contract work only. Runtime orchestration, evidence persistence, app routing, and desktop command-center surfaces remain blocked until their specific plan waves and tests land.
