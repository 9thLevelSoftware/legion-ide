# Plan 13-02 Result: Protocol Workflow Contracts

Status: Complete  
Date: 2026-06-01

## Files changed

- `crates/devil-protocol/src/lib.rs`
- `crates/devil-protocol/tests/dto_contracts.rs`
- `.planning/phases/13-legion-workflow-orchestration/13-02-RESULT.md`

## Implemented contracts

- Added `LegionWorkflowSessionId` and `LegionWorkflowWorkerId`.
- Added metadata-first workflow, worker, backend, dependency, conflict, verification, sign-off, and merge-readiness enums.
- Added `LegionWorkflowWorkerAssignment`, `LegionWorkflowDependencyEdge`, `LegionWorkflowConflictSummary`, `LegionWorkflowVerificationGate`, `LegionWorkflowSignoffRecord`, `LegionWorkflowMergeApproval`, and `LegionWorkflowSession`.
- Added `LegionWorkflowProjection`, projection rows, and `legion_workflow_projection_from_sessions`.
- Added validators for sessions, workers, conflicts, verification gates, and sign-offs.
- Added fail-closed `evaluate_legion_workflow_merge_readiness` for conflicts, dependencies, verification, sign-off, proposal ids, dirty workspace, stale proposal preconditions, audit-before-success, approval, and rollback metadata.

## Verification

- `rg -q "LegionWorkflowSession" crates/devil-protocol/src/lib.rs` — passed.
- `rg -q "validate_legion_workflow" crates/devil-protocol/src/lib.rs` — passed.
- `cargo +1.92.0 test -p devil-protocol --test dto_contracts legion_workflow -- --nocapture` — passed: 3 passed, 0 failed, 93 filtered out.
- `cargo +1.92.0 check -p devil-protocol` — passed.
- `cargo fmt --all --check` — passed.

## Decisions

- Kept all workflow DTOs inside `devil-protocol`; no runtime, app, UI, desktop, tracker, or memory code was activated by this protocol wave.
- Kept provider-backed workers metadata-only by requiring display-safe provider and route identifiers rather than provider payloads.
- Kept merge readiness as a conservative metadata result and not an apply/merge command.

## Blockers

None.
