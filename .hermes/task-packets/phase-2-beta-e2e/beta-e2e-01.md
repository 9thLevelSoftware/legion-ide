# Task Packet: beta-e2e-01 — Add unified beta acceptance e2e test

## Project

- Name: legion-ide
- Repository: /Users/christopherwilloughby/devil-ide
- Coordinator: GPT-5.5
- Implementer: Kimi 2.6

## Phase

- ID: phase-2-beta-e2e
- Title: Unified beta acceptance scenario
- Objective: Add a single deterministic e2e test proving the beta acceptance scenario across local edit, policy-gated assistance, VSIX compatibility, evidence, and configuration persistence.

## Origin

- Origin: audit_gap
- Source finding IDs: finding-unified-beta-e2e-test-missing

## Objective

Add a single integration test in legion-desktop that exercises the beta acceptance loop instead of relying only on distributed smaller tests.

## Dependencies

- docs-ledger-01

## Allowed Files

- `crates/legion-desktop/Cargo.toml`
- `crates/legion-desktop/tests/beta_acceptance_e2e.rs`
- `Cargo.lock`
- `plans/evidence/legion-e2e/2026-06-03_beta_acceptance_e2e.txt`

## Forbidden Files

- `.github/workflows/ci.yml`
- `crates/legion-remote/**`
- `training/**`
- `evals/**`

## Required Context

- Existing beta_workflow.rs covers pieces of the scenario.
- New test must use Legion crate names and deterministic fixtures.

## Implementation Steps

- Add test dependencies if needed.
- Build one unified beta acceptance test.
- Capture targeted test output as evidence.

## Targeted Tests

- `cargo test -p legion-desktop --test beta_acceptance_e2e -- --nocapture`

## Acceptance Criteria

- Test passes and is deterministic.
- Evidence file records command, output, and exit code.
- No broad product behavior is weakened.

## Definition of Done

- Targeted test passes.
- Coordinator verifies file scope.

## Known Risks

- E2E test may duplicate existing lower-level tests if not carefully composed.

## Stop Conditions

Stop and report if any of these occur:

- Need to add non-deterministic GUI automation.
- Targeted test fails after two fix attempts.
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
