# WS19.T1 Legion-Bench v0 Evidence

Date: 2026-06-12
Kanban card: `t_d26cf48d`
Scope: internal harness-aware eval suite baseline and verification contract

## Verdict
Baseline published for Legion-Bench v0.

The v0 suite now exists as a deterministic 20-task eval registry with four task kinds (bug fix, test-add, refactor, multi-file feature), offline recorded-provider mode, live-weekly mode selection, and a fingerprinted suite contract so future harness changes are visible as verification failures.

## What landed in this card
- `xtask/src/legion_bench.rs`
  - Added the Legion-Bench v0 data model, default 20-task suite, deterministic scoring, report write/read helpers, and suite fingerprint verification.
- `xtask/src/main.rs`
  - Added `legion-bench` and `verify-legion-bench` subcommands.
- `xtask/src/lib.rs`
  - Registered the new module.
- `xtask/tests/legion_bench.rs`
  - Added 5 tests covering suite size, task-kind coverage, report round-trip, fingerprint regression detection, and run-mode/provider mapping.

## Baseline report
Generated report:
`target/legion-bench/legion_bench_report.toml`

Summary:
- schema_version: 1
- suite_name: `legion-bench-v0`
- suite_fingerprint: `bench-suite-v1:bd2aa3a7d84d9485`
- mode: `recorded_offline`
- provider_profile: `recorded:gpt-5.5`
- total tasks: 20
- passed: 20
- failed: 0
- regressed: 0
- average_score: 61

## Verification
- `cargo test -p xtask --test legion_bench -- --nocapture` ✅
- `cargo test -p xtask` ✅
- `cargo fmt --all --check` ✅
- `cargo run -p xtask -- legion-bench` ✅
- `cargo run -p xtask -- verify-legion-bench` ✅

## Regression visibility
`verify-legion-bench` recomputes the suite fingerprint from the current task registry and rejects a report if the harness or suite definition has changed. That makes baseline drift visible when the benchmark harness changes.
