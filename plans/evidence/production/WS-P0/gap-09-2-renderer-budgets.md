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

## Verification

```text
cargo test -p xtask --test perf_harness perf_harness_zero_override_does_not_disarm_renderer_paint_rows
cargo test -p xtask --test perf_harness perf_harness_zero_override_does_not_zero_large_file_paint_descriptor
```

Ledger row statuses are unchanged.
