# 13-02: Protocol Workflow Contracts (Wave 2)

## Outcome
Complete

## Tasks Performed
1. Added metadata-first Legion workflow session, worker, dependency, conflict, verification, sign-off, merge approval, readiness, and projection DTOs in `devil-protocol`.
2. Added fail-closed validators for worker/session metadata, provider-backed route references, redaction hints, dependency references, verification evidence, sign-off metadata, and merge readiness.
3. Added DTO contract tests for serde round-trips, metadata-only projection, invalid core metadata, missing provider route metadata, merge-readiness blockers, and approval-gated waiting state.

## Artifacts Generated
- `crates/devil-protocol/src/lib.rs`
- `crates/devil-protocol/tests/dto_contracts.rs`

## Verifications Passed
- `rg -q "LegionWorkflowSession" crates/devil-protocol/src/lib.rs`: true
- `rg -q "validate_legion_workflow" crates/devil-protocol/src/lib.rs`: true
- `cargo test -p devil-protocol --test dto_contracts legion_workflow -- --nocapture`: passed, 5 passed
- `cargo check -p devil-protocol`: passed

## Decisions
- Merge readiness returns a metadata-only `LegionWorkflowMergeReadiness` decision with explicit blockers, rather than a bare boolean, so downstream app/desktop surfaces can display the exact fail-closed reason without raw payloads.
- Provider-backed workers require `AssistedAiTrustProjectionReference` metadata and cannot validate without it.

## Issues
- None.
