# M6 — WS19.T4 Telemetry-to-Flywheel (Consented) Evidence

## Status

Accepted.

## Acceptance target

- Accumulate consented acceptance/rejection signals into a training corpus path that stays metadata-first by default.
- Keep the QLoRA pipeline on a reproducible fixture-smoke path that can graduate to scheduled real runs.
- Preserve a reproducible Legion-Bench comparison path for trained specialist candidates.

## Position

The repo now has a reproducible consent-gated flywheel path that stays fail-closed by default:

- `training/qlora_train.py` has a dry-run mode that explicitly reports `consent_required=true`, `redaction_required=true`, and `raw_trace_default=disabled`.
- The checked-in consented fixture corpus at `training/fixtures/minimal_traces.jsonl` drives a reproducible QLoRA fixture-smoke run that emits a manifest instead of launching a heavy GPU job.
- `training/convert_to_gguf.py` has a fixture-smoke conversion path that records the conversion command plan and metadata manifest without shell interpolation.
- `evals/run_eval.py` covers both the offline specialist suite and the reviewer suite, including the seeded-bug / high-risk / needs-human cases used to validate the telemetry loop.
- `xtask verify-legion-bench` validates the recorded-offline Legion-Bench v0 report so candidate comparisons remain fingerprinted and reproducible.

## What was verified

- `training/qlora_train.py`
  - Dry-run output advertises the consent/redaction gates and keeps raw trace handling disabled by default.
  - Fixture-smoke mode loads `training/fixtures/minimal_traces.jsonl`, validates the dataset, and writes a deterministic `manifest.json`.
- `training/convert_to_gguf.py`
  - Fixture-smoke mode builds an explicit command list, writes a metadata manifest, and never falls back to shell interpolation.
- `evals/run_eval.py`
  - Offline fixture mode and reviewer fixture mode both produce deterministic JSON summaries.
- `evals/test_run_eval.py`
  - Confirms reviewer fixtures flag the seeded bad patch and write the expected output file.
- `target/legion-bench/legion_bench_report.toml`
  - The recorded-offline Legion-Bench report exists and verifies against the suite fingerprint.

## Verification commands

```bash
python3 training/qlora_train.py --dry-run
python3 training/qlora_train.py --fixture-smoke --dataset training/fixtures/minimal_traces.jsonl --output-dir /tmp/ws19t4-*/train-out
python3 training/convert_to_gguf.py --fixture-smoke --model-dir /tmp/ws19t4-*/train-out --output /tmp/ws19t4-*/model.gguf --metadata-output /tmp/ws19t4-*/convert-manifest.json
python3 evals/run_eval.py --offline-fixture --dataset evals/fixtures/minimal.jsonl --output /tmp/ws19t4-*/offline-eval.json
python3 evals/run_eval.py --reviewer-fixture --dataset evals/fixtures/reviewer_seeded_bug.jsonl --output /tmp/ws19t4-*/reviewer-eval.json
python3 -m unittest evals.test_run_eval
cargo run -p xtask -- verify-legion-bench --out target/legion-bench
```

## Results

- Dry-run training returned the consent posture explicitly:
  - `method=qlora`
  - `consent_required=true`
  - `redaction_required=true`
  - `raw_trace_default=disabled`
- Fixture-smoke training returned a deterministic manifest with:
  - dataset `training/fixtures/minimal_traces.jsonl`
  - `example_count=3`
  - `mode=fixture-smoke`
  - `heavy_deps=false`
- Fixture-smoke conversion returned a deterministic command manifest with two commands:
  - `python3 convert_hf_to_gguf.py --outfile ...`
  - `llama-quantize ...`
- Offline eval returned:
  - `schema_compliance_rate=1.0`
  - `patch_apply_rate=0.6666666666666666`
  - `verification_pass_rate=0.6666666666666666`
  - `refusal_rate=0.3333333333333333`
- Reviewer eval returned:
  - `seeded_bug_detection_rate=1.0`
  - `high_risk_label_rate=0.3333333333333333`
  - `needs_human_label_rate=0.3333333333333333`
- `python3 -m unittest evals.test_run_eval`
  - 2 tests passed.
- `cargo run -p xtask -- verify-legion-bench --out target/legion-bench`
  - Passed.
  - Report verified: `target/legion-bench/legion_bench_report.toml`

## Findings

- The consent-to-training path is reproducible in fixture-smoke form and remains metadata-first by default.
- The reviewer fixture and offline eval suite provide deterministic coverage for the telemetry loop’s acceptance/rejection signal handling.
- The Legion-Bench comparison remains fingerprinted and verifiable, which keeps future specialist candidates comparable against the same baseline.
