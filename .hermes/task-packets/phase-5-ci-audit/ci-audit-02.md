# Task Packet: ci-audit-02 — Update engineering audit and status artifacts

## Project

- Name: legion-ide
- Repository: /Users/christopherwilloughby/devil-ide
- Coordinator: GPT-5.5
- Implementer: Kimi 2.6

## Phase

- ID: phase-5-ci-audit
- Title: CI and audit/status reconciliation
- Objective: Add CI coverage for model/training/eval dry-run and fixture-smoke paths, then update audit/status artifacts to reflect resolved findings.

## Origin

- Origin: audit_gap
- Source finding IDs: finding-product-readiness-ledger-stale, finding-unified-beta-e2e-test-missing, finding-cloud-lane-production-transport-missing, finding-training-eval-real-execution-deferred, finding-agents-placeholder-note-stale, finding-training-dry-runs-not-in-ci

## Objective

Mark resolved findings, update validation evidence, regenerate audit/status HTML/markdown artifacts, and record final gate outputs.

## Dependencies

- ci-audit-01

## Allowed Files

- `ENGINEERING_AUDIT.yaml`
- `ENGINEERING_AUDIT.html`
- `ENGINEERING_STATUS.md`
- `ENGINEERING_PLAN.yaml`
- `ENGINEERING_PLAN.html`
- `plans/evidence/legion-e2e/**`

## Forbidden Files

- `crates/**`
- `training/**`
- `evals/**`
- `.github/workflows/ci.yml`

## Required Context

- Implementation artifacts must use Legion names.
- ENGINEERING_AUDIT.html is generated from ENGINEERING_AUDIT.yaml.

## Implementation Steps

- Update finding statuses and validation matrix.
- Update status counts and verified gates.
- Generate ENGINEERING_AUDIT.html and ENGINEERING_PLAN.html.
- Capture final gate output.

## Targeted Tests

- `python3 $HOME/.hermes/skills/software-development/gpt55-kimi-engineering-workflow/scripts/validate_engineering_audit.py ENGINEERING_AUDIT.yaml`
- `python3 $HOME/.hermes/skills/software-development/gpt55-kimi-engineering-workflow/scripts/validate_engineering_plan.py ENGINEERING_PLAN.yaml`

## Acceptance Criteria

- Audit and plan YAML validate.
- HTML reports regenerate.
- Status counts match implemented/resolved state.

## Definition of Done

- Coordinator verifies final artifacts and diff scope.

## Known Risks

- Audit YAML may still contain stale devil-* references from prior branch state.

## Stop Conditions

Stop and report if any of these occur:

- Audit validation fails after two fix attempts.
- Need to change implementation code to make a status true.
- Task exceeds 45 minutes.

## Timebox

45 minutes.

## Output Format Required

- Summary
- Files changed
- Tests run and exact results
- Acceptance checklist
- Blockers or deviations

## Hard Rules

- Implement only this task packet.
- Modify only allowed files.
- Do not create branches, commit, push, open PRs, merge PRs, or modify CI unless explicitly listed in allowed files.
- Do not broaden scope to adjacent tasks.
- Stop after two failed fix attempts.
