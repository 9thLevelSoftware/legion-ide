# Task Packet: training-eval-01 — Implement optional training/eval/gguf harnesses

## Project

- Name: legion-ide
- Repository: /Users/christopherwilloughby/devil-ide
- Coordinator: GPT-5.5
- Implementer: Kimi 2.6

## Phase

- ID: phase-4-training-eval
- Title: Optional real training and eval harnesses
- Objective: Replace dry-run-only training/eval scaffolds with CI-safe fixture-smoke paths and optional real execution paths using lazy heavyweight imports.

## Origin

- Origin: audit_gap
- Source finding IDs: finding-training-eval-real-execution-deferred

## Objective

Add offline fixture and optional endpoint/training/conversion paths while preserving dry-run safety.

## Dependencies

- None

## Allowed Files

- `evals/run_eval.py`
- `evals/README.md`
- `evals/fixtures/minimal.jsonl`
- `training/qlora_train.py`
- `training/convert_to_gguf.py`
- `training/README.md`
- `training/fixtures/minimal_traces.jsonl`
- `pyproject.toml`
- `docs/OPERATOR_RUNBOOK.md`

## Forbidden Files

- `.github/workflows/ci.yml`
- `crates/**`
- `plans/product-readiness-ledger.md`

## Required Context

- Dry-run commands must remain lightweight for CI.
- Heavy training dependencies must be optional and lazily imported.

## Implementation Steps

- Add eval offline fixture mode and optional OpenAI-compatible endpoint mode.
- Add QLoRA fixture-smoke, dataset validation, and lazy real-mode dependency checks.
- Add GGUF fixture-smoke and explicit subprocess argument construction.
- Document install and operator commands.

## Targeted Tests

- `python3 evals/run_eval.py --dry-run`
- `python3 evals/run_eval.py --offline-fixture --dataset evals/fixtures/minimal.jsonl --output /tmp/legion-eval.json`
- `python3 training/qlora_train.py --dry-run`
- `python3 training/qlora_train.py --fixture-smoke --dataset training/fixtures/minimal_traces.jsonl --output-dir /tmp/legion-train-smoke`
- `python3 training/convert_to_gguf.py --dry-run`
- `python3 training/convert_to_gguf.py --fixture-smoke --model-dir /tmp/legion-train-smoke --output /tmp/legion-model.gguf --metadata-output /tmp/legion-gguf.json`
- `python3 -m compileall training evals scripts/models`

## Acceptance Criteria

- Scripts are not dry-run-only.
- Fixture-smoke paths pass without heavyweight dependencies.
- Real modes fail clearly with install instructions when optional deps/tools are absent.
- No raw secrets are emitted in reports.

## Definition of Done

- All targeted Python commands pass.
- Documentation includes usage examples.

## Known Risks

- Real GPU training must not accidentally run in CI.

## Stop Conditions

Stop and report if any of these occur:

- Need to install heavyweight dependencies.
- Scope expands into Rust trace export.
- Validation fails after two fix attempts.
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
