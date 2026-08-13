# 13-03: Agent Workflow Coordinator (Wave 3)

## Outcome
Complete

## Tasks Performed
1. Added `LegionWorkflowCoordinator` in `devil-agent` with metadata-only worker scheduling, completion/blocking state, provider route request emission, proposal-output collection, conflict detection, and merge-readiness evaluation.
2. Added same-target conflict detection for workers in the same dependency layer and dependency-cycle blocking.
3. Added inline regression tests for ready-worker order, dependency cycles, provider route metadata without invocation, same-target conflict blockers, proposal-only outputs, blocked-worker rescheduling, and forbidden app/UI/desktop imports.

## Artifacts Generated
- `crates/devil-agent/src/lib.rs`

## Verifications Passed
- `rg -q "LegionWorkflowCoordinator" crates/devil-agent/src/lib.rs`: true
- `rg -q "devil-app" crates/devil-agent/src/lib.rs; if ($LASTEXITCODE -eq 0) { exit 1 } else { exit 0 }`: passed
- `cargo test -p devil-agent legion_workflow -- --nocapture`: passed, 7 passed
- `cargo check -p devil-agent`: passed

## Decisions
- The coordinator stores only protocol session metadata, completion/blocking ids, provider route request metadata, proposal-only outputs, and conflict metadata.
- Provider-backed workers emit `AssistedAiProviderRouteRequest` with `provider_route.not_invoked` health metadata; no provider invocation path was added.

## Issues
- None.
