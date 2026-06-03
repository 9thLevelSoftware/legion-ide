# Task Packet: docs-ledger-01 — Reconcile product readiness ledger

## Project

- Name: legion-ide
- Repository: /Users/christopherwilloughby/devil-ide
- Coordinator: GPT-5.5
- Implementer: Kimi 2.6

## Phase

- ID: phase-1-docs-ledger
- Title: Reconcile product readiness and AGENTS guidance
- Objective: Replace stale readiness and placeholder documentation with evidence-backed Legion status and explicit deferred cut lines.

## Origin

- Origin: audit_gap
- Source finding IDs: finding-product-readiness-ledger-stale

## Objective

Update plans/product-readiness-ledger.md to reflect current tested Legion surfaces, in-progress release surfaces, and explicitly deferred product promises.

## Dependencies

- None

## Allowed Files

- `plans/product-readiness-ledger.md`

## Forbidden Files

- `.github/workflows/ci.yml`
- `crates/**`
- `training/**`
- `evals/**`

## Required Context

- The repo has been rebranded to Legion; use legion-* crate names.
- Existing audit evidence distinguishes implemented local/product surfaces from deferred cloud/collaboration/admin/runtime-extension execution surfaces.

## Implementation Steps

- Replace all-Not-started rows with evidence-backed statuses.
- Add exact validation commands and evidence references.
- Preserve deferred cut lines where product workflows are not implemented.

## Targeted Tests

- `cargo run -p legion-cli -- evidence check --phase gui-phase8`

## Acceptance Criteria

- Ledger no longer claims every gate is Not started.
- Ledger references exact Legion validation commands.
- Deferred surfaces are explicit and not overstated.

## Definition of Done

- Documentation diff is limited to the ledger.
- Coordinator verifies no stale devil-* commands remain in changed rows.

## Known Risks

- Documentation can overclaim product readiness if deferred cut lines are softened.

## Stop Conditions

Stop and report if any of these occur:

- Need to modify code to make the ledger true.
- Evidence contradicts a claimed status.
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
