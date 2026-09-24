# GAP-09.1 — Measured renderer reports (Windows reference host)

**Date:** 2026-09-01  
**Wave:** 2 proof surface  
**Task:** GAP-09.1  
**SHA:** `804fb18b051bf194539e7bc3dd08ea5ebe069bfc` (`origin/main` at measurement)  
**OS:** windows / x86_64 / `profile = "reference"`

## What this is

A local `cargo run -p xtask -- perf-harness` run that produced renderer-backed
paint reports through `legion-desktop --manual-perf` (egui projection render),
not `legion-app --bin large_file_perf` (EditorEngine text-model).

| Report | Keypress p50 | Keypress p95 | Scroll p95 | Status |
| --- | ---: | ---: | ---: | --- |
| [`manual_renderer_perf.toml`](../../perf-harness-trend/reports/windows-804fb18b-manual_renderer_perf.toml) (`Cargo.toml`) | 4.693 ms | 14.966 ms | 4.367 ms | passed vs 16/32/32 ms |
| [`large_file_manual_renderer_perf.toml`](../../perf-harness-trend/reports/windows-804fb18b-large_file_manual_renderer_perf.toml) (100MB fixture) | 4.000 ms | 18.762 ms | 3.900 ms | passed vs 16/32/32 ms |

Trend entry: `plans/evidence/perf-harness-trend/entries/windows-804fb18b051b-20260901-200729Z.toml`. The `m9.large_file_100mb` row is p50=4000µs / p95=18762µs from the desktop renderer path. The 2026-08-19 archived m9 p50=269µs was the old text-model edit and is not this measurement.

## What this is not

- Not 3-OS paint (Windows reference host only)
- Not a windowed `eframe::run_native` paint (the `--manual-perf` harness uses `egui::Context::run_ui`)
- Not GAP-09.2 (budgets not newly armed; hosted `LEGION_PERF_FAIL_ON_BUDGET_MS=0` unchanged)
- Not GAP-09.3 (LexicalIndexer still on the open path)
- Not a ledger promotion of PR-UI-001 / PR-UI-002

The overall harness exited 1 because `p8.legion_repo` search was over the 120s budget (161s). That row is product search, not paint. Renderer rows passed.

## Verification

```text
cargo run -p xtask -- perf-harness
cargo run -p xtask -- verify-perf-harness
```

Ledger row statuses are unchanged.
