# Legion Operator Runbook

This runbook is the operational companion to `plans/legion-e2e/00_CONSOLIDATED_E2E_IMPLEMENTATION_PLAN.md`.

## Local verification gates

Run from repo root:

```sh
cargo run -p xtask -- check-deps
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

If any command fails, save exact output under `plans/evidence/legion-e2e/` before fixing.

## Evidence naming

Use this pattern:

- `phase-0-check-deps.txt`
- `phase-0-fmt.txt`
- `phase-1-devil-ui-tests.txt`
- `phase-4-assist-inline-prediction.txt`
- `phase-8-model-download-dry-run.txt`
- `final-workspace-test.txt`
- `final-clippy.txt`

Each evidence file should contain:

1. command;
2. working directory;
3. start/end time;
4. exit code;
5. raw output.

## Subagent execution pattern

For every implementation task:

1. dispatch one implementer subagent with exact files and commands;
2. require a failing test first when the task changes code;
3. run the task-specific gate;
4. dispatch spec-compliance reviewer;
5. dispatch quality/security reviewer;
6. fix reviewer findings before proceeding;
7. commit the task.

Do not ask Kimi to read the entire planning package. Give it the one task section plus the exact source files it needs.

## Safety checks

Before any task touching AI, worker, cloud, or trace code, verify:

- Manual mode exclusion remains tested;
- proposal-only mutation remains tested;
- metadata-only default retention remains tested;
- consent-gated raw trace path remains tested;
- network routes are denied in offline/air-gap policy unless explicitly loopback and allowed.

## PR creation

After all phases and gates pass:

```sh
git status --short
git diff --stat origin/main...HEAD
git push -u origin HEAD
gh pr create --title "feat: implement Legion e2e product plan" --body-file /tmp/legion-pr-body.md
```

The PR body must include:

- summary by phase;
- tests/evidence paths;
- security/authority boundary notes;
- cloud/training operational notes;
- no unsupported planned features in scope.
