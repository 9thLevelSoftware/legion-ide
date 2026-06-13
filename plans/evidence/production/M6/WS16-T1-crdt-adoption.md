# M6 — WS16.T1 CRDT Adoption Evidence

## Status

Accepted.

## Acceptance target

- ADR that decides between Loro, yrs, and a homegrown anchor-layer implementation.
- Benchmark evidence recorded for the current operation-log runtime baseline.

## Decision

- Adopt the homegrown anchor-layer approach over the existing operation-log runtime.
- Do not add Loro or yrs as the anchor-layer substrate at this stage.

## What was verified

- `xtask/src/legion_bench.rs`
  - Provides the Legion-Bench v0 suite and report writer/reader.
  - Emits a deterministic recorded-offline baseline suitable for this decision record.
- `xtask/tests/legion_bench.rs`
  - Confirms the benchmark suite shape, round-trip behavior, and fingerprint validation.
- `target/legion-bench/legion_bench_report.toml`
  - Records the current baseline run for `legion-desktop`.
  - Summary: 20 total / 20 passed / 0 failed / 0 regressed.
  - Average score: 61.
  - Suite fingerprint: `bench-suite-v1:bd2aa3a7d84d9485`.
  - Git SHA: `c81eabeba532abb831bf55b6e5419f50f7a727e2`.
- Existing collaboration/runtime contract coverage
  - ADR-0040 still defines the accepted operation/anchor layer boundary.
  - No `loro`, `yrs`, `diamond-types`, or `automerge` dependency exists in the workspace today.

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

- The current deterministic operation-log baseline is already stable enough to anchor the post-GA collaboration decision.
- No evidence surfaced that would justify swapping in Loro or yrs at the anchor layer right now.
- The benchmark report is reproducible and self-verifying, which makes it suitable as the evidence artifact for this ADR.
