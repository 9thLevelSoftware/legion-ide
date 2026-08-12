# Plan Critique -- Phase 13: Legion Workflow Orchestration

## Auto-Refine Summary

- Mode: `--auto-refine`
- Refine cycles used: 0
- Verdict: PASS
- Rule chain: no schema blockers, no wave overlap blockers, no critical pre-mortem risks, no high-impact weak assumptions, and no high-impact decision-completeness gaps.
- Outcome: no plan rewrites required after the generated plan set.

## Schema Conformance

| Plan | verification_commands | files_forbidden | expected_artifacts | Harness Sections | Status |
|------|-----------------------|-----------------|--------------------|------------------|--------|
| 13-01 | PASS | PASS | PASS | PASS | OK |
| 13-02 | PASS | PASS | PASS | PASS | OK |
| 13-03 | PASS | PASS | PASS | PASS | OK |
| 13-04 | PASS | PASS | PASS | PASS | OK |
| 13-05 | PASS | PASS | PASS | PASS | OK |
| 13-06 | PASS | PASS | PASS | PASS | OK |
| 13-07 | PASS | PASS | PASS | PASS | OK |

## Wave Overlap

All seven plans are sequential single-plan waves.

| Wave | Plan | Overlap Status |
|------|------|----------------|
| 1 | 13-01 | PASS |
| 2 | 13-02 | PASS |
| 3 | 13-03 | PASS |
| 4 | 13-04 | PASS |
| 5 | 13-05 | PASS |
| 6 | 13-06 | PASS |
| 7 | 13-07 | PASS |

## Pre-Mortem Analysis

Failure scenarios analyzed: 5  
Critical risks: 0  
Watch items: 5

| # | Headline | Plan Section | Risk Score | Mitigation |
|---|----------|--------------|------------|------------|
| 1 | Phase 13 failed because protocol DTOs forced downstream crates to infer merge-readiness semantics differently. | 13-02 Task 2 | 4 | Plan 13-02 requires `evaluate_legion_workflow_merge_readiness` and fail-closed contract tests before app work. |
| 2 | Phase 13 failed because workflow coordination bypassed provider consent by invoking provider-backed workers directly. | 13-03 Task 2, 13-05 Task 1 | 4 | Plans require provider route metadata only and tests for no direct invocation. |
| 3 | Phase 13 failed because conflict resolution became an implicit git merge. | 13-05 Task 2, 13-07 Task 2 | 4 | Plans define merge readiness as proposal/approval metadata and require autonomous merge unsupported evidence. |
| 4 | Phase 13 failed because UI/desktop gained runtime authority while adding command-center actions. | 13-06 Task 2 | 4 | Plan 13-06 forbids app/protocol/agent edits and validates snapshot ids before app requests. |
| 5 | Phase 13 failed because final evidence overstated readiness after a targeted test failure. | 13-07 Task 2 | 4 | Plan 13-07 blocks acceptance if any prior result is missing, partial, failed, or unresolved. |

## Assumption Hunting

Assumptions extracted: 8  
Critical assumptions: 0  
Warning assumptions: 2  
Accepted assumptions: 6

### Warning Assumptions

| # | Assumption | Category | Impact | Evidence | Challenge Action |
|---|------------|----------|--------|----------|------------------|
| 1 | Phase 12 delegated runtime remains the correct substrate for Phase 13 worker orchestration. | Dependency | High | Moderate | Plans 13-03 and 13-05 require reading Phase 12 summaries and live source before implementation. |
| 2 | Existing `xtask check-deps` can accept an additive Phase 13 evidence gate without making planning artifacts fail prematurely. | Technical | Medium | Moderate | Plan 13-07 sequences evidence writing before final `xtask` rerun and blocks on check failure. |

### Accepted Assumptions

- Protocol crate is the right home for shared workflow DTOs because existing delegated-task, artifact, verification, and readiness DTOs live there.
- `devil-agent` can coordinate metadata without depending on app/UI crates because dependency policy already forbids those edges.
- Tracker and memory can store workflow metadata without raw payload retention because their current records are already metadata-oriented.
- `AppComposition` is the right authority layer for workflow execution state and merge readiness because existing saves/proposals/delegated tasks are app-owned.
- UI/desktop can expose command-center data as projections and app requests using the existing delegated-task command-center pattern.
- Final acceptance should use standard AGENTS.md gates plus targeted tests because Phase 13 touches protocol, runtime, app, and desktop boundaries.

## Decision-Completeness Check

| Plan | Required Sections | Executor-Owned Decisions | Verdict |
|------|-------------------|--------------------------|---------|
| 13-01 | Present | None high impact | OK |
| 13-02 | Present | None high impact | OK |
| 13-03 | Present | None high impact | OK |
| 13-04 | Present | None high impact | OK |
| 13-05 | Present | None high impact | OK |
| 13-06 | Present | None high impact | OK |
| 13-07 | Present | None high impact | OK |

## Completeness Check

- Error handling: PASS. Each code-modifying plan names blocked/error states.
- Edge cases: PASS. Plans enumerate empty ids, stale metadata, dirty workspace, unresolved conflicts, missing evidence/sign-off, invalid correlation/causality, unknown ids, and raw-payload restrictions.
- UI states: PASS for Plan 13-06. Empty, populated, blocked, unknown-id, approval-gated, and unsupported autonomous merge states are specified.
- API/contract coverage: PASS. Protocol validators and app integration tests are named before downstream use.

## Recommended Actions

1. Execute Plan 13-01 first; do not start runtime implementation before ADR/policy governance exists.
2. Keep Plans 13-02 through 13-07 sequential because they intentionally reuse high-risk files and authority boundaries.
3. During Plan 13-07, do not mark `Phase 13 acceptance: Accepted` until all prior result artifacts and final gates are present and passing.

## Rule-Chain Trace Per Plan

| Plan | Verdict | Rule Chain |
|------|---------|------------|
| 13-01 | OK | Required fields present; no forbidden overlap; markdown-only governance plan has executable checks; no high-impact decision gaps. |
| 13-02 | OK | Required fields present; protocol write scope is isolated; validators/tests specified; no wave overlap. |
| 13-03 | OK | Required fields present; agent write scope is isolated; app/UI authority is forbidden and checked; no wave overlap. |
| 13-04 | OK | Required fields present; tracker/memory write scope is isolated; consent and redaction checks specified; no wave overlap. |
| 13-05 | OK | Required fields present; app authority write scope is isolated; dirty/stale/conflict/evidence/sign-off blockers specified; no wave overlap. |
| 13-06 | OK | Required fields present; UI/desktop projection scope is isolated; unknown-id/no-authority tests specified; no wave overlap. |
| 13-07 | OK | Required fields present; final gate scope is isolated; acceptance marker is gated on prior results and broad commands; no wave overlap. |
