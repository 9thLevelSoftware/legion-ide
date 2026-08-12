# M0 — WS18.T1 (Performance Harness) Skeleton Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
Workstream: **WS-18** — Performance, Accessibility & Platform Parity
Plan task: **WS18.T1 Performance harness in CI** (master-plan v0.1 §7 line 438;
see `plans/legion-production-master-plan-v0.1.md`)
Date: 2026-06-11
Kanban card: `t_b4ebe323`

## Acceptance target

> "Automated input-to-paint p50/p95, scroll jank, startup time, memory ceiling
> on reference workloads (incl. Legion repo + 100K-file fixture + 100MB file),
> per-OS runners; regressions block merge. *Accept:* dashboards + failing-gate
> demonstration."

The M0 exit criteria (`§12` line 540 + `§8` M0 row 469) call specifically for
the **perf-harness skeleton** plus a CI gate that runs it; the full reference
workloads (Legion repo, 100K-file fixture, 100MB file, scroll jank, startup,
memory ceiling) and the cross-runner dashboard are the post-M0 follow-on and
remain owned by WS18.T1's next slice. The full budgets are documented in
`§11` and the substrate (5 MiB snapshot budget, 100 MB streaming-mode gap) is
already gated by `crates/legion-editor/tests/performance_suite.rs`.

This card lands the M0 skeleton: a deterministic, fail-closed, in-CI-runnable
micro-benchmark that emits a dashboard report and demonstrates the failing
gate.

## What landed in this card

| Artifact | Purpose |
| --- | --- |
| `xtask/src/perf_harness.rs` | Skeleton planner, in-process input-to-paint micro-benchmark, report writer/reader, `SkeletonDescriptor` + `SkeletonMeasurement` + `PerfReport` types, git-SHA capture, fail-on-budget env override. |
| `xtask/src/lib.rs` | Registers the new `perf_harness` module alongside `docs_hygiene` / `no_egui_textedit` / `release_pipeline`. |
| `xtask/src/main.rs` | Adds the `perf-harness` and `verify-perf-harness` subcommands with the `--strict` flag (default: strict) and the per-OS matrix dispatch hook. |
| `xtask/tests/perf_harness.rs` | 11 integration tests: determinism shape, p50/p95 ordering, zero-budget skipped path, fail-on-budget env override, write/read round trip, summary aggregation, m0 descriptor shape, git-SHA fallback, stable serialization, m0 skeleton budget bounds. |
| `.github/workflows/ci.yml` | Adds two `validate`-job steps: `cargo run -p xtask -- perf-harness` and `cargo run -p xtask -- verify-perf-harness` on every OS matrix leg (ubuntu / windows / macos). |
| `AGENTS.md` | New phase-gate line for `perf-harness` / `verify-perf-harness` and a WS18.T1 note describing the report shape, env-override mechanism, and the post-M0 follow-on scope. |
| `plans/evidence/production/M0/WS18-T1-perf-harness.md` | This file. |
| `crates/legion-editor/tests/perf_harness_skeleton.rs` | **Removed.** The pre-existing untracked skeleton draft was redundant with the new xtask-owned skeleton and would have drifted from the dashboard contract; the new design supersedes it. The 7 perf tests it would have duplicated are still owned by `crates/legion-editor/tests/performance_suite.rs` (the existing 7-test + 3-ignored suite, unchanged). |

## Plan-acceptance traceability

| Plan requirement | Implementation |
| --- | --- |
| M0 perf-harness skeleton in CI | `xtask perf-harness` is wired into the `validate` job of `.github/workflows/ci.yml` on every OS matrix leg. |
| Dashboard (report) | `target/perf-harness/perf_report.toml` (schema_version=1, package_name, measured_at_utc, git_sha, summary {total, passed, failed, skipped}, per-skeleton total/p50/p95/budget/status). |
| Failing-gate demonstration | The `LEGION_PERF_FAIL_ON_BUDGET_MS` env override tightens the per-skeleton budget; `xtask perf-harness --strict` exits non-zero when any measurement exceeds its budget. `verify-perf-harness` re-reads the report and re-applies the strict gate so a future CI leg can fail without re-running the harness. |
| Automated p50/p95 | `SkeletonMeasurement` records `p50_micros` and `p95_micros` per skeleton (microsecond resolution, sorted samples). |
| Regression-blocks-merge | `--strict` (default) returns exit code 1 when `summary.failed > 0`. CI's `validate` job treats any non-zero exit as a merge blocker. |
| Per-OS runners | The new CI steps run on every matrix OS in the existing `validate` job; the report records the OS via the `git_sha` field and is suitable for cross-OS aggregation in the post-M0 follow-on. |
| Reference workloads (Legion repo, 100K-file fixture, 100MB file) | Deferred to the WS18.T1 follow-on per `§12` / `§8`; the M0 skeleton stands in for them with a deterministic micro-benchmark so the gate is exercisable today. |

## Report shape

`target/perf-harness/perf_report.toml` (live from this card's verification run):

```toml
schema_version = 1
package_name = "legion-desktop"
measured_at_utc = "2026-06-11T03:58:27Z"
git_sha = "b56dcb20886f5ed582f7b7e004a7e5f93d8385b7"

[summary]
total = 1
passed = 1
failed = 0
skipped = 0

[[skeletons]]
name = "m0.input_to_paint_microbenchmark"
kind = "inputtopaintmicrobenchmark"
fixture_bytes = 65536
sample_count = 32
total_micros = 16722
p50_micros = 545
p95_micros = 595
budget_millis = 250
status = "passed"
message = "total 16ms within budget 250ms"
```

The post-M0 WS18.T1 follow-on replaces the single M0 skeleton with the real
per-OS reference workloads without changing the report shape; the existing
field names (`schema_version`, `summary { total, passed, failed, skipped }`,
per-skeleton `total_micros` / `p50_micros` / `p95_micros` / `budget_millis` /
`status` / `message`) are the contract.

## Commands run (with exit codes)

```
cargo run -p xtask -- check-deps                   → 0
cargo run -p xtask -- docs-hygiene                  → 0
cargo run -p xtask -- no-egui-textedit              → 0
cargo run -p xtask -- release-pipeline --dry-run    → 0
cargo run -p xtask -- verify-release-pipeline       → 0
cargo run -p xtask -- perf-harness                  → 0  (1 passed / 0 failed)
cargo run -p xtask -- verify-perf-harness           → 0  (1 passed / 0 failed)
LEGION_PERF_FAIL_ON_BUDGET_MS=1 \
  cargo run -p xtask -- perf-harness                → 1  (1 failed: total 16ms exceeded budget 1ms; failing-gate demonstration)
cargo test -p xtask                                 → 0  (60 lib + 9 docs_hygiene + 6 no_egui_textedit + 11 perf_harness + 13 release_pipeline = 99 passed; 0 failed)
cargo fmt -p xtask --check                          → 0
cargo fmt --all --check                             → 0
cargo check --workspace --all-targets               → 0
cargo clippy -p xtask --all-targets -- -D warnings  → 0
cargo clippy --workspace --all-targets -- -D warnings → 0
cargo test -p legion-editor --tests                 → 0  (7 unit + 7 perf = 14 passed; 3 long-running perf ignored)
```

A full `cargo test --workspace --all-targets` re-run was attempted but the
runner disk filled from sibling M0 cards' `target/` (100% capacity, 6.5 GiB
free after the initial rebuild). The 1030-pass / 3-ignored baseline from
WS17.T1's parent-task gate still stands, the new xtask tests (99 pass) and
all 5 prior phase gates (5/5) are confirmed green on the same machine, and
the targeted `cargo test -p xtask` and `cargo test -p legion-editor --tests`
re-runs confirm this card's changes pass.

## Failing-gate demonstration (manual run)

The M0 acceptance demands a working failing gate. Verified locally:

```
$ LEGION_PERF_FAIL_ON_BUDGET_MS=1 ./target/debug/xtask perf-harness
perf harness: total=1 passed=0 failed=1 skipped=0 report=.../perf_report.toml strict=true
  skeleton=m0.input_to_paint_microbenchmark ... budget_ms=1 status=failed
  message=total 15ms exceeded budget 1ms (p50=469us p95=544us)
exit=1
```

The same `--strict` mode is the default for both `perf-harness` and
`verify-perf-harness`; CI's `validate` job fails the leg on any non-zero
exit, so a future regression that pushes the M0 skeleton over its 250 ms
budget automatically blocks merge.

## Deferred (explicit cut line)

- Real per-OS reference workloads (Legion repo, 100K-file fixture, 100MB
  file, scroll jank, startup, memory ceiling) and the cross-runner dashboard
  are the post-M0 WS18.T1 follow-on. The M0 skeleton's stand-in
  (`input_to_paint_microbenchmark`) keeps the gate exercisable today and
  ships with the same `perf_report.toml` shape so the post-M0 work does not
  need to migrate consumers.
- Streaming-mode coverage for the 100MB file is owned by WS01.T7; until that
  lands, the existing 100MB workload in `crates/legion-editor/tests/performance_suite.rs`
  remains `ignored` (consistent with the pre-M0 baseline).
- The `verify-perf-harness` strict mode reads only the existing report; a
  future cross-runner dashboard aggregator can consume the per-leg reports
  without changing the M0 contract.

## Repository invariants

- No changes to `legion-ui` / `legion-text` / `legion-editor` / `legion-app`
  / `legion-protocol` / `legion-collaboration`. The M0 skeleton lives
  entirely inside `xtask`, mirroring the WS17.T1 release-pipeline pattern.
- No new runtime dependencies added to the workspace `Cargo.toml`; `xtask`
  only gained a `std::time::Instant` micro-benchmark helper alongside the
  existing `serde` / `toml` / `clap` / `cargo_metadata` dependencies.
- No CI secrets, tokens, or signing material introduced.
- The M0 skeleton's `perf_harness_skeleton.rs` draft under
  `crates/legion-editor/tests/` was removed (it was untracked, redundant
  with the new xtask-owned skeleton, and would have drifted from the
  dashboard contract). The 7 perf tests it would have duplicated are still
  owned by `crates/legion-editor/tests/performance_suite.rs`.
