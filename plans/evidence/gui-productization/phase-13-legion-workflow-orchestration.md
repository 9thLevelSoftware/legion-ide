# Phase 13 Legion Workflow Orchestration Evidence

## Acceptance Status

- Phase 13 acceptance: Accepted

This document is the final evidence ledger for Phase 13: Legion Workflow Orchestration. It ties each implementation slice to the result artifact and command evidence produced during the phase.

## Scope

Phase 13 coordinates a Legion Workflow team across local and provider-backed worker metadata while preserving the existing Legion IDE authority boundaries.

The accepted scope is metadata-first workflow orchestration. It includes protocol DTOs, a bounded agent coordinator, tracker and memory evidence records, app-owned execution and merge-readiness routing, projection-only UI rows, desktop review actions, final gate evidence, and an operational runbook.

The phase does not authorize direct main-workspace mutation, direct provider invocation from the coordinator, autonomous merge, autonomous apply, raw payload retention by default, or UI/desktop ownership of workflow execution.

## Required Markers

- Legion workflow orchestration: approval-gated
- Autonomous merge: unsupported until approval
- Provider-backed workers: routed through assisted-AI consent
- Final gate outputs archived from current commands

## Required Artifacts

- `plans/adrs/ADR-0031-legion-workflow-orchestration.md`
- `plans/evidence/gui-productization/phase-13-governance.md`
- `plans/evidence/gui-productization/phase-13-final-gates.md`
- `plans/evidence/gui-productization/phase-13-runbook.md`
- `.planning/phases/13-legion-workflow-orchestration/13-01-RESULT.md`
- `.planning/phases/13-legion-workflow-orchestration/13-02-RESULT.md`
- `.planning/phases/13-legion-workflow-orchestration/13-03-RESULT.md`
- `.planning/phases/13-legion-workflow-orchestration/13-04-RESULT.md`
- `.planning/phases/13-legion-workflow-orchestration/13-05-RESULT.md`
- `.planning/phases/13-legion-workflow-orchestration/13-06-RESULT.md`
- `.planning/phases/13-legion-workflow-orchestration/13-07-RESULT.md`

## Required Commands

- `cargo run -p xtask -- check-deps`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Slice Evidence

### 13-01 Governance And Activation Gate

Plan 13-01 created `ADR-0031-legion-workflow-orchestration.md`, updated `plans/dependency-policy.md`, and added `phase-13-governance.md`.

The governance evidence states that app composition owns workflow execution and proposal lifecycle state. It also records the required marker `Autonomous merge: unsupported until approval`.

Verification recorded in `13-01-RESULT.md`:

- `rg -q "Legion Workflow orchestration" plans/adrs/ADR-0031-legion-workflow-orchestration.md`: passed.
- `rg -q "Phase 13" plans/dependency-policy.md`: passed.
- `rg -q "Autonomous merge: unsupported until approval" plans/evidence/gui-productization/phase-13-governance.md`: passed.
- `cargo run -p xtask -- check-deps`: passed.

### 13-02 Protocol Workflow Contracts

Plan 13-02 added Legion workflow DTOs to `devil-protocol` for sessions, workers, dependency edges, conflicts, verification gates, sign-off, merge approvals, readiness, and projections.

The protocol layer remains metadata-first. It validates route metadata, redaction hints, nonzero workflow metadata, provider route references, dependency references, verification evidence, sign-off metadata, and approval-gated readiness.

Verification recorded in `13-02-RESULT.md`:

- `rg -q "LegionWorkflowSession" crates/devil-protocol/src/lib.rs`: passed.
- `rg -q "validate_legion_workflow" crates/devil-protocol/src/lib.rs`: passed.
- `cargo test -p devil-protocol --test dto_contracts legion_workflow -- --nocapture`: passed, 5 tests.
- `cargo check -p devil-protocol`: passed.

### 13-03 Agent Workflow Coordinator

Plan 13-03 added `LegionWorkflowCoordinator` in `devil-agent`.

The coordinator schedules ready workers, detects dependency cycles, records same-target conflicts, emits provider route metadata, and collects proposal-only output metadata. It does not import app, UI, or desktop authority.

Verification recorded in `13-03-RESULT.md` and refreshed during Phase 13 review remediation:

- `rg -q "LegionWorkflowCoordinator" crates/devil-agent/src/lib.rs`: passed.
- `rg -q "devil-app" crates/devil-agent/src/lib.rs; if ($LASTEXITCODE -eq 0) { exit 1 } else { exit 0 }`: passed.
- `cargo test -p devil-agent legion_workflow -- --nocapture`: passed, 8 tests.
- `cargo check -p devil-agent`: passed.

### 13-04 Tracker Memory Evidence Ledger

Plan 13-04 added metadata-only workflow records to `devil-tracker` and consent-gated outcome candidate handling to `devil-memory`.

Tracker records use session, worker, proposal, conflict, verification, sign-off, correlation, causality, event sequence, hashes, labels, and redaction metadata. Memory candidates are proposed first and retained only with explicit session or project consent.

Verification recorded in `13-04-RESULT.md`:

- `rg -q "LegionWorkflow" crates/devil-tracker/src/lib.rs`: passed.
- `rg -q "LegionWorkflow" crates/devil-memory/src/lib.rs`: passed.
- `cargo test -p devil-tracker legion_workflow -- --nocapture`: passed, 4 tests.
- `cargo test -p devil-memory legion_workflow -- --nocapture`: passed, 4 tests.
- `cargo check -p devil-tracker -p devil-memory`: passed.

### 13-05 App Workflow Composition

Plan 13-05 routed workflow sessions through `AppComposition`.

The app owns seeded sessions, workflow execution, projection generation, verification records, sign-off records, conflict resolution, tracker records, memory outcome candidates, and merge-readiness evaluation. Local workers route through isolated delegated-task proposal generation. Provider-backed workers emit route-required metadata and do not invoke providers directly.

Verification recorded in `13-05-RESULT.md` and refreshed during Phase 13 review remediation:

- `rg -q "LegionWorkflow" crates/devil-app/src/lib.rs`: passed.
- `rg -q "execute_legion_workflow" crates/devil-app/src/lib.rs`: passed.
- `cargo test -p devil-app --test legion_workflow_integration -- --nocapture`: passed, 9 tests.
- `cargo check -p devil-app --all-targets`: passed.

### 13-06 Desktop Workflow Command Center

Plan 13-06 added projection-only command center support across `devil-ui` and `devil-desktop`.

The UI accepts workflow projections and emits command intents. Desktop renders rows, validates review actions from projection data, reports operational health counts, and translates requests without executing workers, invoking providers, launching terminals, applying proposals, or merging.

Verification recorded in `13-06-RESULT.md`:

- `rg -q "LegionWorkflow" crates/devil-ui/src/ui.rs`: passed.
- `rg -q "legion workflow command center" crates/devil-desktop/src/view.rs`: passed.
- `rg -q "Autonomous merge" crates/devil-desktop/src/health.rs`: passed.
- `cargo test -p devil-ui legion_workflow -- --nocapture`: passed, 4 tests.
- `cargo test -p devil-desktop --test legion_workflow_command_center -- --nocapture`: passed, 4 tests.
- `cargo check -p devil-desktop --all-targets`: passed.

### 13-07 Evidence Final Gate And Readiness

Plan 13-07 adds `xtask` enforcement for Phase 13 evidence markers, this final evidence ledger, final gate archive, runbook, and result artifact.

Final command outcomes are archived in `plans/evidence/gui-productization/phase-13-final-gates.md`.

## Safety Boundary Proof

- `devil-ui` remains projection-only and does not gain app, editor, project, storage, terminal, provider, or workspace mutation dependencies.
- `devil-desktop` renders workflow status and request rows but does not own workflow sessions, proposal lifecycle, provider routing, or merge authority.
- `devil-agent` does not depend on `devil-app`, `devil-ui`, or `devil-desktop`.
- Local worker output becomes proposal metadata before review.
- Provider-backed workers are represented by route metadata and assisted-AI trust references.
- Merge readiness is represented as a protocol/app decision with explicit blockers.
- Dirty workspace state, unresolved conflicts, missing verification, missing sign-off, missing audit, missing rollback metadata, or absent approval blocks readiness.
- Tracker and memory surfaces store metadata, hashes, labels, redaction hints, and identifiers rather than raw prompts, raw source bodies, provider payloads, or worker logs.

## Final Validation Checklist

- [x] Governance, ADR, and dependency policy evidence are complete.
- [x] Protocol DTO contracts are implemented and tested.
- [x] Agent coordinator remains bounded and proposal-only.
- [x] Tracker and memory retain metadata-only workflow evidence.
- [x] App composition owns workflow execution, verification, sign-off, conflict handling, and readiness.
- [x] UI and desktop expose command-center projections without authority creep.
- [x] Final gate outputs are archived from current commands.
- [x] Autonomous merge remains unsupported until approval.

## Residual Limits

- Autonomous merge and autonomous apply remain unsupported.
- Provider-backed workers require assisted-AI consent and route metadata before any future provider invocation.
- The desktop command center validates workflow actions against projection-safe identifiers and labels.
- Memory retention requires explicit consent before outcome candidates become retained records.
- Conflict resolution and merge readiness are metadata decisions until an app-owned proposal approval path performs a reviewed mutation.
