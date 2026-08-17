# 100MB large-file workload: first real measurement (P1.F4.T5)

Date: 2026-08-16. Roadmap 1.5 / ADR-0048 budgets.

## What changed

The 100MB workload was an `#[ignore]`d, report-only test in
`crates/legion-editor/tests/performance_suite.rs`. It printed numbers nobody
read and asserted only structure — mode, bounded payload, chunk count — never
latency.

It is now `m9.large_file_100mb` in the standard `perf-harness` run, measured by
a real subprocess (`legion-app --bin large_file_perf`) rather than a synthetic
stand-in, because `xtask` cannot depend on `legion-editor` and a stand-in for a
100MB file measures the stand-in.

The workload is not behind an opt-in flag, which P1.F4.T5's stop condition
forbids. It costs about a minute of wall clock per run.

## The result, on the development machine

| metric | measured | ADR-0048 budget | verdict |
| --- | ---: | ---: | --- |
| open | 187 ms | — | fine |
| viewport at line 500,000 | 3.0 ms | scroll p95 < 32 ms | comfortably within |
| **edit p50** | **23.2 ms** | **keypress p50 < 16 ms** | **over by 45%** |
| edit p95 | 24.5 ms | keypress p95 < 32 ms | within |
| viewport payload | 1,296 B | — | bounded, as intended |

Streaming open and scrolling are not the problem. **Typing is.** The median
keystroke in a 100MB file takes 23 ms against a 16 ms budget.

## What this means

**P1.F4.T2's acceptance is not met.** It reads "100MB file opens within budget,
scrolls, and does not block typing." The first two hold; the third does not, and
the reason it was never caught is that nothing measured typing latency at this
size — the ignored test sampled edits and printed the percentiles without
comparing them to anything.

**This bears on PR-UI-001.** Roadmap 1.10 promotes that row on `perf-harness
--strict` enforcing ADR-0048 budgets on real workloads. This is a real workload
and it fails one, so the promotion cannot claim the budgets are met at 100MB
until either the latency comes down or the budget is explicitly scoped to
exclude files this size — and if it is scoped, that scoping belongs in
ADR-0048, not in a passing test.

**CI is unaffected today, deliberately.** Hosted runners set
`LEGION_PERF_FAIL_ON_BUDGET_MS=0`, so every budget is report-only there and
noisy VMs cannot red a PR. The failure is visible in the uploaded report and
gates locally under strict budgets, which is the same treatment every other
workload gets. Making this one workload red CI while the rest are advisory
would be a policy change dressed as a measurement.

## What has not been done

The latency has not been investigated or improved. The measurement exists,
reports honestly, and names the gap; closing it is separate work. A single
machine is also a single machine — the 3-OS matrix (P8.F4.T2) will say whether
23 ms is representative or generous.

## Reproduction

```
cargo run -p xtask -- perf-harness          # strict: fails on the p50 budget
LEGION_PERF_FAIL_ON_BUDGET_MS=0 \
  cargo run -p xtask -- perf-harness        # report-only, as hosted CI runs it
```

The row is `m9.large_file_100mb` in `target/perf-harness/perf_report.toml`, and
the subprocess's own numbers land in `large-file-perf.toml` beside it.
