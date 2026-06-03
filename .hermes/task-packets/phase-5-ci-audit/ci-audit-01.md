# Task Packet: ci-audit-01 — Add CI Python dry-run and fixture-smoke coverage

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
- Source finding IDs: finding-training-dry-runs-not-in-ci

## Objective

Extend CI so Linux runs model dry-runs, eval offline fixture, training fixture smoke, GGUF fixture smoke, and compileall with PYTHONDONTWRITEBYTECODE.

## Dependencies

- training-eval-01

## Allowed Files

- `.github/workflows/ci.yml`

## Forbidden Files

- `crates/**`
- `training/**`
- `evals/**`
- `plans/product-readiness-ledger.md`

## Required Context

- CI already runs Rust gates and evidence gates across OS matrix.
- Heavy training dependencies are not installed in CI.

## Implementation Steps

- Add Linux-only dry-run/fixture-smoke step after Phase 8 evidence gate.
- Set PYTHONDONTWRITEBYTECODE to avoid dirty bytecode artifacts.
- Keep commands lightweight and deterministic.

## Targeted Tests

- `python3 evals/run_eval.py --dry-run`
- `python3 evals/run_eval.py --offline-fixture --dataset evals/fixtures/minimal.jsonl --output /tmp/legion-eval.json`
- `python3 training/qlora_train.py --fixture-smoke --dataset training/fixtures/minimal_traces.jsonl --output-dir /tmp/legion-train-smoke`
- `python3 training/convert_to_gguf.py --fixture-smoke --model-dir /tmp/legion-train-smoke --output /tmp/legion-model.gguf --metadata-output /tmp/legion-gguf.json`
- `python3 -m compileall training evals scripts/models`

## Acceptance Criteria

- CI includes model/training/eval dry-run coverage.
- CI does not install heavy ML dependencies.
- No bytecode artifacts remain dirty after validation.

## Definition of Done

- Workflow YAML lint/write checks pass.
- Targeted Python gates pass locally.

## Known Risks

- compileall may dirty committed pyc files unless bytecode is controlled.

## Stop Conditions

Stop and report if any of these occur:

- CI change requires installing heavy dependencies.
- Workflow scope expands beyond Phase 8 Python/model validation.
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
