# P9.F4.T3 — Consented Corpus Legion-Bench Comparison

Date: 2026-06-15
Kanban card: `t_5a7fc4ec`
Source backlog row: `P9.F4.T3`

## Verdict

Archived the recorded Legion-Bench baseline comparison for the consented-corpus training flywheel.

## Evidence archived

- Recorded benchmark report: `target/legion-bench/legion_bench_report.toml`
- Bench mode: `recorded_offline`
- Provider profile: `recorded:gpt-5.5`
- Suite: `legion-bench-v0`
- Suite fingerprint: `bench-suite-v1:fb767be844a28833`
- Summary: `24 passed / 24 total / 0 failed / 0 regressed`
- Average score: `64`

## Commands run

```bash
cargo run -p xtask -- legion-bench --mode recorded
cargo run -p xtask -- verify-legion-bench
```

## Result

Both commands passed. The archived report is the reproducible baseline comparison used by the training flywheel, and the verification step confirms the report still matches the current suite fingerprint.

## Notes

- The repo’s consent-gated training path remains metadata-first, so this archive records the comparison side of the flywheel rather than introducing any raw-trace or unsanctioned-corpus handling.
- The benchmark report can be regenerated with the recorded mode command above and re-verified with `verify-legion-bench`.