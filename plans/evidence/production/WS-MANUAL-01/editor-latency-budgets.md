# WS-MANUAL-01 Editor Latency Budgets

Date: 2026-06-19
Scope: Manual-mode daily editing on a trusted local workspace.

## Budget Table

| Interaction | Metric | Budget | Gate owner | Evidence |
| --- | --- | --- | --- | --- |
| Keypress to paint, normal buffer | p50 | <= 16 ms | `xtask perf-harness` renderer-backed Manual measurement | `target/perf-harness/perf_report.toml` |
| Keypress to paint, normal buffer | p95 | <= 32 ms | `xtask perf-harness` renderer-backed Manual measurement | `target/perf-harness/perf_report.toml` |
| Scroll to paint, normal buffer | p95 | <= 32 ms | desktop manual perf scenario | `target/perf-harness/perf_report.toml` |
| Open 1 MiB file | total | <= 250 ms | app/editor integration test plus perf report | `crates/legion-app/tests/daily_editing_contracts.rs` |
| Save normal buffer | total | <= 250 ms | app save workflow integration test | `crates/legion-app/tests/workspace_vfs_integration.rs` |
| Active-file search | total | <= 100 ms | app search integration test | `crates/legion-app/tests/daily_editing_search.rs` |
| LSP completion projection | p95 | <= 100 ms | language tooling workflow test | `crates/legion-app/tests/language_tooling_workflow.rs` |

## Enforcement Rules

- A budget can be report-only only when the evidence row records the blocker and the workstream remains open.
- A budget is green only when the current-tree command listed in the Evidence column passes.
- Manual-mode measurements must be metadata-only. Do not persist raw source, clipboard text, IME composition text, or full buffer contents in evidence files.
- Renderer-backed measurements must exercise `legion-desktop` rendering and app/editor routing, not only synthetic `xtask` loops.
- Large-file degraded-mode measurements remain bounded by WS-MANUAL-02 unless this workstream explicitly names a visible Manual-mode capability reduction.
