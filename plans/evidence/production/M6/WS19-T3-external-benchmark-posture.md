# M6 — WS19.T3 External Benchmark Posture Evidence

## Status

Accepted.

## Acceptance target

- Produce the first external benchmark posture report.
- Keep the harness config and score output visible enough to support future SWE-bench-Pro / Terminal-Bench-2.0-style positioning.

## Position

Legion now has a first baseline posture report for its benchmark harness in recorded-offline mode. The report is intentionally deterministic and self-verifying so later external runs can be compared against the same suite fingerprint, mode, and provider profile.

## What was verified

- `xtask/src/legion_bench.rs`
  - Defines the Legion-Bench v0 suite, report schema, deterministic scoring, and suite fingerprint verification.
  - Uses a stable recorded-offline provider profile (`recorded:gpt-5.5`) and a separate live-weekly profile (`live:weekly`).
- `xtask/tests/legion_bench.rs`
  - Confirms the suite shape, task-kind mix, report round trip, fingerprint mismatch rejection, and run-mode/provider mapping.
- `target/legion-bench/legion_bench_report.toml`
  - First report produced for this card.
  - Package: `legion-desktop`.
  - Mode: `recorded_offline`.
  - Provider profile: `recorded:gpt-5.5`.
  - Suite: `legion-bench-v0`.
  - Suite fingerprint: `bench-suite-v1:bd2aa3a7d84d9485`.
  - Summary: 20 total / 20 passed / 0 failed / 0 regressed.
  - Average score: 61.
  - Git SHA: `d7311f5957bcf0d683d9ab4dc8775791a72bbc40`.

## Verification commands

```bash
cargo test -p xtask --test legion_bench -- --nocapture
cargo run -p xtask -- legion-bench --out target/legion-bench --mode recorded
cargo run -p xtask -- verify-legion-bench --out target/legion-bench
```

## Results

- `cargo test -p xtask --test legion_bench -- --nocapture`
  - 5 tests passed.
- `cargo run -p xtask -- legion-bench --out target/legion-bench --mode recorded`
  - Passed.
  - Report written to `target/legion-bench/legion_bench_report.toml`.
- `cargo run -p xtask -- verify-legion-bench --out target/legion-bench`
  - Passed.

## Findings

- The benchmark suite is already stable enough to publish as a posture baseline.
- The first report is reproducible and fingerprinted, which makes future drift visible.
- This card satisfies the acceptance target by producing the first report; later work can add more external runs or comparison notes without changing the baseline contract.
