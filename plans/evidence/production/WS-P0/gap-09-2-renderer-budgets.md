# GAP-09.2 — Fail-closed renderer paint budgets

**Date:** 2026-09-01  
**Wave:** 2 proof surface  
**Task:** GAP-09.2

## What this is

Renderer-backed paint rows keep their own budgets when hosted CI sets
`LEGION_PERF_FAIL_ON_BUDGET_MS=0`. That override still report-onlys synthetic
m0/m1 microbenchmarks (shared-VM noise). It no longer reclassifies a
`manual.renderer_input_to_paint` or `m9.large_file_100mb` miss as Skipped.

`.github/workflows/legion-gates.yml` still sets the env var (skeletons), and
the verify step stays hard-fail (`continue-on-error` forbidden). A paint-row
budget miss is a red Standing gates job.

## What this is not

- Not 3-OS windowed `eframe::run_native` paint (still `--manual-perf`)
- Not GAP-09.3 (LexicalIndexer still on open)
- Not a ledger promotion of PR-UI-001/002
- Not arming `p8.legion_repo` search (that row already fail-closes as a product
  workload; the local 161s miss is a separate search-budget question)

## Hosted macos-latest miss (run 33561695219)

Standing gates on ubuntu-latest and windows-latest passed. macos-latest went
red on `manual.renderer_input_to_paint`:

| OS | keypress p50 | keypress p95 | scroll p95 | row |
| --- | ---: | ---: | ---: | --- |
| linux | 4.809 ms | 5.945 ms | ≤5.945 ms | passed |
| macos | 4.643 ms | 55.034 ms | 4.764 ms | failed vs 16/32/32 ms |
| macos `m9.large_file_100mb` | 3.979 ms | 25.828 ms | ≤25.828 ms | passed |

p50 on macos matched linux. The 55 ms p95 was the maximum of 16 samples
(nearest-rank `ceil(0.95 * 16)`), so one cold first keypress / scheduler stall
failed the ADR-0048 budget. Budgets stay 16/32/32 ms. The Manual harness now
discards one unmeasured insert+paint (and one scroll) warmup and uses the same
inclusive percentile as `product_perf` / the in-process harness, so p95 of 16
samples is the second-highest rather than the max.

## Verification

```text
cargo test -p xtask --test perf_harness perf_harness_zero_override_does_not_disarm_renderer_paint_rows
cargo test -p xtask --test perf_harness perf_harness_zero_override_does_not_zero_large_file_paint_descriptor
cargo test -p legion-desktop --lib percentile_p95_of_sixteen_is_second_highest_not_max
cargo test -p legion-desktop --test manual_perf
```

Ledger row statuses are unchanged.
