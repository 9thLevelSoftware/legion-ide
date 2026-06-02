# Phase 13 Review: Legion Workflow Orchestration

## Review Outcome

PASS after remediation on 2026-05-28.

Phase 13 is accepted for the current checkout. The review found one blocker in merge-readiness and resumed dependency scheduling. The blocker was fixed before acceptance, targeted regression coverage was added, and full repository gates passed after the fix.

## Review Scope

- Phase artifacts reviewed: `13-CONTEXT.md`, `13-01-RESULT.md` through `13-07-RESULT.md`, `WAVE-CHECKLIST.md`, Phase 13 evidence, final gates, runbook, ADR-0031, dependency policy, `STATE.md`, and `ROADMAP.md`.
- Code reviewed: `devil-protocol`, `devil-agent`, `devil-tracker`, `devil-memory`, `devil-app`, `devil-ui`, `devil-desktop`, and `xtask` Phase 13 acceptance gates.
- Reviewer lenses applied: senior engineering, security, QA/verification, test-results analysis, and project-shepherd acceptance.
- Nested Codex review was attempted but unavailable because the local `codex exec review` path failed authentication refresh with `invalid_grant`. This review therefore relies on local source inspection, artifacts, and direct command evidence.

## Findings

### BLOCKER-1: Incomplete Workers Could Reach Merge-Ready

Status: fixed.

Evidence:

- `crates/devil-protocol/src/lib.rs` previously allowed `evaluate_legion_workflow_merge_readiness` to return `Ready` when proposal, verification, sign-off, approval, dependency, conflict, audit, and rollback metadata passed, even if one or more workers were still `Ready` or `ProviderRouteRequired`.
- `crates/devil-agent/src/lib.rs` rebuilt `LegionWorkflowCoordinator` with empty completed/blocked worker sets. On a resumed session, already-completed workers could be rescheduled and dependency successors could remain unscheduled because dependency readiness only checked the coordinator's transient completed list.

Impact:

This violated the Phase 13 requirement that workflow team completion, dependency tracking, verification, sign-off, and approval-gated merge readiness be wired end-to-end. A multi-worker workflow could present merge readiness before all worker metadata had completed, or fail to make progress on a resumed dependency chain.

Fix:

- Added `LegionWorkflowMergeReadinessBlocker::IncompleteWorker` and completed-session validation requiring all workers to be `Completed`.
- Seeded `LegionWorkflowCoordinator` completed/blocked worker state from persisted session worker states.
- Allowed dependency readiness to honor persisted `LegionWorkflowDependencyState::Satisfied`.
- Added regression coverage for protocol incomplete-worker readiness and app dependency-chain resume behavior.

Key fixed locations:

- `crates/devil-protocol/src/lib.rs:20428`
- `crates/devil-protocol/src/lib.rs:20929`
- `crates/devil-protocol/src/lib.rs:20972`
- `crates/devil-agent/src/lib.rs:421`
- `crates/devil-agent/src/lib.rs:479`
- `crates/devil-agent/src/lib.rs:581`
- `crates/devil-app/tests/legion_workflow_integration.rs:278`
- `crates/devil-protocol/tests/dto_contracts.rs:8823`

## Verification

Targeted Phase 13 checks:

- `cargo test -p devil-protocol --test dto_contracts legion_workflow -- --nocapture` passed, 5 tests.
- `cargo test -p devil-agent legion_workflow -- --nocapture` passed, 8 tests.
- `cargo test -p devil-tracker legion_workflow -- --nocapture` passed, 4 tests.
- `cargo test -p devil-memory legion_workflow -- --nocapture` passed, 4 tests.
- `cargo test -p devil-app --test legion_workflow_integration -- --nocapture` passed, 9 tests.
- `cargo test -p devil-ui legion_workflow -- --nocapture` passed, 4 tests.
- `cargo test -p devil-desktop --test legion_workflow_command_center -- --nocapture` passed, 4 tests.

Repository gates:

- `cargo run -p xtask -- check-deps` passed.
- `cargo fmt --all --check` passed.
- `cargo check --workspace --all-targets` passed.
- `cargo test --workspace --all-targets` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo deny check` passed with known warning-level duplicate dependency diagnostics.
- `git diff --check` passed.
- `git worktree prune` then `git worktree list --porcelain` showed only the main worktree.

## Residual Risks

- Provider-backed workers still emit route-required metadata only; no provider invocation, autonomous merge, or autonomous apply is accepted in Phase 13.
- `cargo deny check` still emits duplicate dependency warnings under the repository's warning-level policy.
- Full raw command logs are intentionally not retained in evidence artifacts; review records command labels and outcomes only.

