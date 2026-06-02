# 13-06: Desktop Workflow Command Center (Wave 6)

## Outcome
Complete

## Tasks Performed
1. Added projection-only Legion workflow support to `devil-ui`: snapshot field, shell state, empty projection, rendering rows, and command intents for inspection, linked proposals, verification, sign-off, conflict resolution, and merge-readiness requests.
2. Added desktop bridge actions, app requests, validation errors, and projection validation for workflow sessions, linked proposals, verification/sign-off/conflict metadata labels, and merge-readiness requests.
3. Added desktop workflow statuses and outcomes for Legion workflow command-center requests without worker execution, provider invocation, terminal execution, proposal apply, or autonomous merge.
4. Added desktop view rows for workflow sessions, workers, provider routes, dependencies, conflicts, verification, sign-off, linked proposals, merge readiness, and explicit autonomous-merge unsupported labels.
5. Added operational health counts for Legion workflows plus the required `Autonomous merge: unsupported until approval` unsupported-surface label.
6. Added focused UI and desktop regression tests for projection roundtrip, command parsing, rows, bridge validation, unknown-id denial, approval-gated readiness, and no autonomous merge action.

## Files Changed
- `crates/devil-ui/src/ui.rs`
- `crates/devil-desktop/src/bridge.rs`
- `crates/devil-desktop/src/workflow.rs`
- `crates/devil-desktop/src/view.rs`
- `crates/devil-desktop/src/health.rs`
- `crates/devil-desktop/tests/legion_workflow_command_center.rs`
- `crates/devil-app/src/lib.rs` (mechanical compatibility only: new snapshot field initialization and explicit no-op routing for projection-only Legion UI intents)

## Verifications Passed
- `rg -q "LegionWorkflow" crates/devil-ui/src/ui.rs`: passed
- `rg -q "legion workflow command center" crates/devil-desktop/src/view.rs`: passed
- `rg -q "Autonomous merge" crates/devil-desktop/src/health.rs`: passed
- `cargo test -p devil-ui legion_workflow -- --nocapture`: passed, 4 passed
- `cargo test -p devil-desktop --test legion_workflow_command_center -- --nocapture`: passed, 4 passed
- `cargo check -p devil-desktop --all-targets`: passed

## Decisions
- Desktop bridge validates exact workflow sessions and linked proposal ids from the workflow projection. Protocol projection rows do not expose individual verification, sign-off, or conflict ids, so desktop validates those metadata requests fail-closed against display-safe labels.
- Workflow request outcomes are request/inspection statuses only; desktop does not call worker execution, providers, terminal/process APIs, proposal apply, or merge.
- Adding `legion_workflow_projection` to `ShellProjectionSnapshot` required a mechanical `devil-app` snapshot initializer and explicit no-op routing for the new UI intents so app matches remain exhaustive. No app workflow authority was moved into UI or desktop.

## Issues
- None remaining.
