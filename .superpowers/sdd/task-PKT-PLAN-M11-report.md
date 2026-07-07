# PKT-PLAN M11 Report

Status: DONE_WITH_CONCERNS

## Scope

Implemented P6.F1 only: editable plan lifecycle, durable audited plan revision ledger, approved-plan DAG/session construction, desktop bridge request wiring, tests, kanban, and evidence.

No PKT-WORKERS work was started. The existing workflow worker execution path was not changed.

## Implementation

- Added `legion-agent::coordinator` with `legion_workflow_session_from_approved_plan` and `LegionWorkflowSessionBuilderConfig`.
- Built one workflow assignment per task node with stable worker IDs of the form `{plan_id}/tasks/{index}`.
- Validated `TaskNode.depends_on` against known task IDs and rejected unknown dependencies.
- Added additive `PersistedState::protocol_plan_revisions` with `#[serde(default)]`, kept schema version 3, and rebuilt `PlanRevisionLedger` on load.
- Added app lifecycle APIs for create, revise, approve, reject, DAG generation, and approved-plan session creation.
- Added no-default library cfg support through `offline_ai`.
- Added desktop action/request variants `SubmitLegionWorkflowPlanRevision`, `ApproveLegionWorkflowPlan`, and `RejectLegionWorkflowPlan`.
- Added bridge/view helper coverage for plan editor submit/approve/reject actions.
- Marked `P6.F1.T1`, `P6.F1.T2`, and `P6.F1.T3` done with `plans/evidence/production/M11/PKT-PLAN-evidence.md`.

## TDD RED

```powershell
cargo test -p legion-agent --test coordinator
```

RED: compile failure because `legion_agent::coordinator` did not exist.

```powershell
cargo test -p legion-storage --test plan_revisions
```

RED: compile failure because the repository/persistence API for plan revisions did not exist.

```powershell
cargo test -p legion-app --test legion_workflow_plan_lifecycle
```

RED: compile failure because the plan lifecycle APIs and approved-plan session builder path did not exist.

```powershell
cargo test -p legion-desktop --test plan_editor
```

RED: first run timed out during concurrent red compilation; later compile surfaced missing desktop request handling until bridge/workflow wiring was implemented.

## GREEN / Verification

```powershell
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j 1 -p legion-agent --test coordinator
```

PASS: 3 tests.

```powershell
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j 1 -p legion-storage --test plan_revisions
```

PASS: 3 tests.

```powershell
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j 1 -p legion-app --test legion_workflow_plan_lifecycle
```

PASS: 4 tests.

```powershell
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j 1 -p legion-desktop --test plan_editor
```

PASS: 3 tests.

```powershell
cargo check -p legion-app --lib --no-default-features
```

PASS.

```powershell
cargo run -p xtask -- verify-kanban-backlog
```

PASS: `kanban backlog ok: 10 epic(s), 38 feature(s), 146 task(s)`.

```powershell
git diff --check
```

PASS: no whitespace errors. Git emitted Windows line-ending warnings for touched files.

## Concerns

```powershell
cargo fmt --all --check
```

FAIL: repo-wide rustfmt check reports unrelated pre-existing formatting diffs. The first reported diff was `crates/legion-agent/src/agent_loop.rs:906`; other reported unrelated paths included `crates/legion-ai-providers/src/lib.rs`, `crates/legion-app/src/bin/update_drill.rs`, `crates/legion-observability/*`, and `xtask/*`.

I did not mass-format unrelated code in this P6.F1 packet.

```powershell
cargo check -p legion-app --no-default-features
```

FAIL: existing package binary `crates/legion-app/src/bin/golden_path_3.rs` references optional AI/delegate symbols in a no-default package build. The packet API surface was checked with `cargo check -p legion-app --lib --no-default-features`, which passed.

## Evidence

Full packet evidence: `plans/evidence/production/M11/PKT-PLAN-evidence.md`

## Post-Review Fixes (2026-07-07)

- Fixed file-backed storage load for malformed `protocol_plan_revisions`: persisted duplicate or invalid plan revisions now fail closed through the existing storage corruption quarantine path before any flush can rewrite the primary storage file with an empty plan ledger.
- Fixed app plan revision write ordering: `AppComposition::record_plan_revision` now validates against a cloned ledger, persists the revision successfully, then swaps the app-visible ledger forward. If persistence fails, the app-visible latest revision and DAG state remain unchanged.

## Post-Review Verification (2026-07-07)

```powershell
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j 1 -p legion-storage --test plan_revisions
```

PASS: 4 tests, including `file_backed_storage_quarantines_duplicate_persisted_plan_revisions_without_overwrite`.

```powershell
CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j 1 -p legion-app --test legion_workflow_plan_lifecycle
```

PASS: 5 tests, including `failed_plan_revision_persistence_does_not_advance_app_visible_ledger`.

```powershell
git diff --check
```

PASS: no whitespace errors. Git emitted Windows line-ending warnings for touched files.

```powershell
cargo fmt --all --check
```

FAIL: same repo-wide unrelated rustfmt drift already documented above; first reported diff remains `crates/legion-agent/src/agent_loop.rs:906`.
