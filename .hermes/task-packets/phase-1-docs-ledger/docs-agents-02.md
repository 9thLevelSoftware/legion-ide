# Task Packet: docs-agents-02 — Reconcile AGENTS placeholder guidance

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
- Source finding IDs: finding-agents-placeholder-note-stale

## Objective

Update AGENTS.md so active phase-gated crates are not described as placeholders while deferred product surfaces remain explicit.

## Dependencies

- None

## Allowed Files

- `AGENTS.md`

## Forbidden Files

- `.github/workflows/ci.yml`
- `crates/**`
- `training/**`
- `evals/**`

## Required Context

- legion-agent, legion-tracker, and legion-memory have active tests and evidence.
- Collaboration/admin/runtime-extension product surfaces still have explicit cut lines.

## Implementation Steps

- Replace stale placeholder language.
- Keep contributor guidance concise and phase-gated.
- Do not change code or CI.

## Targeted Tests

- `cargo test -p legion-agent --all-targets`
- `cargo test -p legion-tracker --all-targets`
- `cargo test -p legion-memory --all-targets`

## Acceptance Criteria

- AGENTS.md accurately reflects active crates and deferred product surfaces.
- No stale devil-* crate names are introduced.

## Definition of Done

- Documentation-only diff is verified.

## Known Risks

- AGENTS guidance may become too broad for future agents.

## Stop Conditions

Stop and report if any of these occur:

- Need to inspect or modify broad unrelated docs.
- Task exceeds 30 minutes.

## Timebox

30 minutes.

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
