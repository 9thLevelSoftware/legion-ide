# 100MB large-file workload: first real measurement (P1.F4.T5)

Date: 2026-08-16. Roadmap 1.5 / ADR-0048 budgets.

## What changed

The 100MB workload was an `#[ignore]`d, report-only test in
`crates/legion-editor/tests/performance_suite.rs`. It printed numbers nobody
read and asserted only structure — mode, bounded payload, chunk count — never
latency.

It is now `m9.large_file_100mb` in the standard `perf-harness` run. The harness
creates a 100MB text fixture and opens it through the real
`legion-desktop --manual-perf` renderer path rather than measuring only the text
model. Headless or build-blocked hosts record an honest skipped row instead of
inventing renderer numbers.

The workload is not behind an opt-in flag, which P1.F4.T5's stop condition
forbids. It costs about a minute of wall clock per run.

## Measurement status

This change does not record a renderer number from this worktree. A valid
`legion-desktop --manual-perf` run requires a working desktop renderer; when the
host is headless or the desktop build is unavailable, the harness records a
`measured = false` skipped row instead of promoting stale or synthetic values.
The committed evidence is therefore report-only until a host produces the
subprocess report.

The historical text-model baseline below is retained for context only. It is
not the PR-16 renderer-backed measurement and must not be used as paint-budget
evidence.

| historical text-model metric | prior baseline | ADR-0048 budget | interpretation |
| --- | ---: | ---: | --- |
| open | 187 ms | — | not renderer evidence |
| viewport at line 500,000 | 3.0 ms | scroll p95 < 32 ms | not renderer evidence |
| edit p50 | 23.2 ms | keypress p50 < 16 ms | not renderer evidence |
| edit p95 | 24.5 ms | keypress p95 < 32 ms | not renderer evidence |
| viewport payload | 1,296 B | — | not renderer evidence |

## What this means

**P1.F4.T2 remains unaccepted by this evidence.** The renderer-backed row now
exists, but the acceptance question is deliberately unanswered until a host
produces `large_file_manual_renderer_perf.toml`. The old ignored test's values
cannot answer an input-to-paint question because they did not exercise the
desktop renderer.

**This does not promote PR-UI-001.** A renderer-backed row is necessary evidence,
not proof that the budget is met. Promotion still requires a measured report
from the target host(s), with any budget failure or host blocker retained in
the report rather than replaced by a passing fixture value.

**CI is unaffected today, deliberately.** Hosted runners set
`LEGION_PERF_FAIL_ON_BUDGET_MS=0`, so every budget is report-only there and
noisy VMs cannot red a PR. The failure is visible in the uploaded report and
gates locally under strict budgets, which is the same treatment every other
workload gets. Making this one workload red CI while the rest are advisory
would be a policy change dressed as a measurement.

## What has not been done

The latency has not been investigated or improved. The harness now names the
measurement and the blocker honestly; closing the gap and collecting a
cross-platform baseline are separate work.

## Reproduction

```
cargo run -p xtask -- perf-harness          # records measured or skipped row
LEGION_PERF_FAIL_ON_BUDGET_MS=0 \
  cargo run -p xtask -- perf-harness        # report-only, as hosted CI runs it
```

The row is `m9.large_file_100mb` in `target/perf-harness/perf_report.toml`, and
the renderer subprocess report lands in
`large_file_manual_renderer_perf.toml` beside it. The generated 100MB fixture
is `large-file-100mb.txt` in the same output directory and is not committed.
