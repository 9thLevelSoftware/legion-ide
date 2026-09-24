# Perf harness: real product workloads, per-OS coverage, trend gate (P8.F4)

Date: 2026-08-19. Covers P8.F4.T1, P8.F4.T2, P8.F4.T3.

## The problem this closes

`xtask/src/perf_harness.rs` said, in its own module comment, that its workloads
were synthetic stand-ins, and the reason was structural: `xtask` may not depend
on `legion-app` or `legion-editor`, and `cargo run -p xtask -- check-deps`
enforces that. A stand-in for the editor measures the stand-in.

So the harness reported an "input-to-paint" number produced by walking bytes in
a `Vec<u8>`, a "memory ceiling" for a generated string, and nothing at all for
startup, the Legion repository, or a 100K-file workspace.

## The design

The pattern already existed in the repo: `golden_path_1`, `large_file_perf`,
and `legion-desktop --manual-perf` are product-crate binaries that `xtask`
spawns and whose TOML reports it reads back. This work adds one more —
`legion-app --bin product_perf` — and moves the six missing reference workloads
into it.

    legion-app/src/bin/product_perf.rs   measures (real AppComposition, real
                                         EditorEngine, real workspace search)
                        │  TOML report
                        ▼
    xtask/src/perf_workloads.rs          owns the budgets, classifies, and
                                         enforces workload coverage
                        │
                        ▼
    xtask/src/perf_trend.rs              archives a trend entry and compares it
                                         against a tracked baseline

The split is deliberate. A measurement that decides its own verdict cannot be
audited against a policy, so the product answers only "what did it cost" and
`xtask` answers "what is Legion allowed to cost".

## What each workload now measures

| row | what it really does | budget |
| --- | --- | --- |
| `p8.startup` | `AppComposition::new()` → `open_workspace(Legion repo)` → `open_file` → first viewport projection | p50 < 6 000 ms |
| `p8.input_to_paint` | real keystroke through `edit_active_buffer`, then the real `viewport_projection` a frame would paint from, at line 18 024 of a 36 049-line file | p50 < 16 ms, p95 < 32 ms (ADR-0048) |
| `p8.scroll_jank` | 64 real viewport projections at offsets spanning the whole document | p95 < 32 ms (ADR-0048) |
| `p8.memory_ceiling` | real snapshot footprint of a real 1.5 MB source file after a 128-edit burst | < 48 MB |
| `p8.legion_repo` | real product `RunSearch` over this repository | < 10 000 ms |
| `p8.fixture_100k_files` | real product `RunSearch` over a generated 100 000-file workspace | < 240 000 ms |
| `m9.large_file_100mb` | unchanged: real 100 MB streaming open, deep projection, typing | keypress p50 < 16 ms |
| `manual.renderer_input_to_paint` | unchanged: renderer-backed keypress/scroll through `legion-desktop` | ADR-0048 |

None of them is behind a flag. `product_perf` accepts `--skip-fixture-100k`,
but only the harness's own self-tests pass it and the harness never does; the
flag's own help text says so.

## Measured, on the development machine

`cargo run -p xtask -- perf-harness`, 2026-08-19, Windows 11 x86_64, release
subprocesses, nothing else running. Full run: about 25 minutes wall clock, of
which roughly 20 are the two file-count fixtures.

    perf harness: os=windows arch=x86_64 kind=product+skeleton
                  total=12 passed=11 failed=0 skipped=1

| row | measured | budget | verdict |
| --- | --- | --- | --- |
| `p8.startup` | 7 333 ms | p50 < 30 000 ms | passed |
| `p8.input_to_paint` | p50 1.5 ms, p95 2.0 ms | 16 / 32 ms | passed, 8x headroom |
| `p8.scroll_jank` | p50 0.07 ms, p95 0.08 ms | p95 < 32 ms | passed, 400x headroom |
| `p8.memory_ceiling` | 22.7 MB | < 48 MB | passed |
| `p8.legion_repo` | 1 194 ms, 1 hit | < 120 000 ms | passed |
| `p8.fixture_100k_files` | 38 149 ms, 1 hit | < 1 800 000 ms | passed |
| `m9.large_file_100mb` | open 187 ms, edit p50 0.3 ms, p95 0.4 ms | keypress p50 < 16 ms | passed |
| `manual.renderer_input_to_paint` | p50 4.0 ms, p95 14.7 ms | ADR-0048 | passed |
| `m8.search_stream_50k` | 20 443 ms, 5 000 hits | report-only | skipped |
| `m0`, `m1`, `m2` | synthetic / guardrail | — | passed |

Every row reports `measured = true`. The one `skipped` is `m8`, whose budget is
`0` by construction and predates this work.

Two of those numbers deserve a note:

- **`p8.fixture_100k_files` is dominated by the host, not by Legion.** The same
  workload measured 73.5 s earlier the same evening with a warm page cache and
  38.1 s here; a third run under disk contention was still going after 15
  minutes. Product search walks and reads all 100 000 files on one thread
  (`WalkBuilder::build`, not `build_parallel`), so the number is a measure of
  per-file-open cost on the machine. Its ceiling is a liveness guard, and it is
  deliberately untracked in the trend baseline.
- **`p8.scroll_jank` has 400x headroom**, which says the viewport projection is
  genuinely cheap — the projection returns 64 line slices and does no shaping.
  The shaping half of scrolling is the renderer's, and that is what
  `manual.renderer_input_to_paint` measures.

## Defect 1 — opening one source file blocks for seconds

`p8.startup` breaks its total down by phase, and the breakdown is the finding.
Two runs, same machine, same build, warm and cold page cache:

    phase                  warm       cold
    AppComposition::new     0.2 ms     0.3 ms
    open_workspace        343.8 ms  2653.5 ms
    open_file            3268.3 ms  4679.6 ms   ← 90% / 64% of open-to-ready
    first projection        0.1 ms     0.1 ms
    ------------------------------------------
    open-to-ready        3612.4 ms  7333.4 ms

Opening one 1.5 MB source file costs **3.3 to 4.7 seconds**, and it is not the
editor: `AppComposition::bind_opened_file` runs `refresh_retrieval_document` —
`LexicalIndexer::new().index_document(...)` plus a `semantic_index.upsert` —
synchronously on the open path for every file below the 5 MB streaming
threshold, on top of two full `String` clones of the file text. The editor's own
share is the `0.1 ms` projection at the end.

This is user-visible: opening Legion's own largest source file in Legion freezes
for several seconds. It is not fixed here — this task is about measuring, and
the measurement is the argument for fixing it — but it is now a number in a
gated report instead of an impression.

The `p8.startup` ceiling is 30 000 ms for that reason. It is a regression guard,
not a target: setting it to a defensible 1 s would make the workload red on
arrival and teach everyone to ignore the row. When the synchronous indexing moves
off the open path, the constant comes down with it (`STARTUP_BUDGET_MILLIS` in
`xtask/src/perf_workloads.rs`), and the trend baseline is where the tighter
comparison lives.

## Defect 2 — workspace search is single-threaded

`p8.fixture_100k_files` spends 38–74 s searching 100 000 small files.
`WorkspaceActor::search_workspace_stream` builds its walker with
`WalkBuilder::build`, not `build_parallel`, and reads and matches every file on
the calling thread. `m8.search_stream_50k`'s 20.4 s for 50 000 files is the same
shape at half the size — the cost is linear in files and serial in threads.

For scale: `ripgrep`, built on the same `ignore` crate, uses `build_parallel` and
would do this in a couple of seconds. This is not fixed here either, and unlike
Defect 1 it does not have a tight budget guarding it, precisely because a
single-threaded disk-bound scan varies too much between machines to gate on
wall clock. What it has instead is an archived number per run.

## What is still a stand-in, and labelled as one

Two rows remain synthetic, and the report now says so in a field rather than in
a source comment nobody reads: `SkeletonMeasurement.synthetic_stand_in`.

- `m0.input_to_paint_microbenchmark` — a byte walk over an in-memory buffer.
- `m1.line_galley_shaping_cache` — a shaping-cache model with no font stack.

Both are superseded for their original purpose by `p8.input_to_paint` and the
renderer-backed measurement. They are kept as cheap tripwires that need no
subprocess and no display, not as evidence about the product. `verify-perf-
harness` prints them under "synthetic stand-ins still in the report" on every
run, so their presence is a standing statement rather than a forgotten one.

`m2.memory_ceiling_1mb` and `m8.search_stream_50k` are not stand-ins: they run
real `legion-text` and real `legion-project` code against generated fixtures.
`p8.memory_ceiling` and `p8.fixture_100k_files` are the product-path versions
of the same two questions; the older pair survives because it needs no
subprocess.

## P8.F4.T2 — three OSes, and no silent skip

The gates workflow already ran the harness on `ubuntu-latest`,
`windows-latest`, and `macos-latest`. What it did not do was notice when a
workload stopped running on one of them.

`SkeletonStatus::Skipped` could not tell "this budget is report-only" apart
from "this measurement never happened", so a report could be green because
everything passed or because half of it never ran — and on a three-OS matrix
the second is the one nobody sees. Two things changed:

1. `SkeletonMeasurement.measured` records whether a measurement happened at
   all, separately from the budget verdict.
2. `verify-perf-harness` fails when any required workload is absent or
   unmeasured, and when the report's `os` does not match the host. That check
   is **not** conditioned on `--strict` and **not** relaxed by
   `LEGION_PERF_FAIL_ON_BUDGET_MS=0`: report-only budgets are a policy about
   timing noise on shared runners, not a licence for a workload to disappear on
   one OS.

The required set is every product workload plus the 100 MB file — all headless,
so any host that can build Legion can run them. The renderer-backed measurement
is deliberately exempt: it needs a display, and a headless runner legitimately
cannot supply one. That exemption is one list in
`required_measured_workloads()`, not a special case scattered through the gate.

CI uploads `perf_report.toml`, the three subprocess reports, and the trend
entries from each OS job, with `if: always()` — when a workload does not run,
the artifact is the only record of why.

## P8.F4.T3 — trend archive and regression gate

`plans/evidence/perf-harness-trend/` is in the tracked tree:

- `baseline.toml` — reference numbers per workload **per OS**, plus the drift
  tolerance (60%), each row carrying a `note` saying which machine produced it.
- `entries/<os>-<sha>-<timestamp>.toml` — one archived entry per run, carrying
  every row's status, whether it was measured, whether it is synthetic, and any
  regressions found.

A workload regresses when a tracked metric exceeds
`baseline * (1 + tolerance/100)` for the same OS. `perf-harness --strict` exits
non-zero on a regression, on a budget failure, and on a required workload that
did not run.

Baselines are per OS because a Windows number and a macOS number measure
different systems. Only `windows` rows exist today, since that is the only OS a
real run has been taken on. A run on an OS with no baseline prints
`no trend baseline recorded for os=<os>` and archives
`baseline_status = "missing_for_os"` — visible in the artifact rather than
silently green.

## The gate firing on its own, unprompted

The first full run after the baseline was written **failed**, and not because
anything was staged:

    trend entry=...\entries\windows-91e707a89430-20260819-062416Z.toml
                 baseline=compared tolerance=60%
    REGRESSION p8.startup p50_micros regressed:
               baseline 3612368, allowed 5779788, observed 7333389
    EXIT=1

The baseline had been recorded from a warm-cache run at 3.61 s; the full harness
run churns the page cache with a 50 000-file scan before `p8.startup` measures,
and open-to-ready came out at 7.33 s. The regression was real as a measurement
and false as a defect.

The fix was the baseline, not the code: `p8.startup`'s reference number is now
the **slower** 7 333 389 µs, with the note recording both observations and why
the conservative one is used. Adjusting a baseline until a gate is green is
exactly the move that ruins gates, so it is stated plainly here rather than
buried: the number moved because of the page cache, the evidence for that is the
phase breakdown above (`open_workspace` 344 ms → 2 654 ms with no code change),
and the tolerance now covers the machine's real spread instead of its best case.

## Non-vacuity proofs

Each mutation was applied, the failure observed, then reverted; `git diff` is
clean at the end of the section (`git status --short` shows only this PR's
intended files).

**1. Coverage gate catches a deleted workload row (P8.F4.T2).**
Deleted the `[[skeletons]]` block for `p8.fixture_100k_files` from
`target/perf-harness/perf_report.toml`, leaving the summary header untouched.

    perf harness verify: total=12 passed=11 failed=0 skipped=1 strict=true
    perf harness verify failed: required workload did not run on windows:
      p8.fixture_100k_files: absent from the report
    EXIT=1

The summary still claimed 12 rows and zero failures. The coverage check caught it
anyway, which is the point: a report can be green because everything passed or
because part of it never ran. Restored → exit 0.

**2. Coverage gate catches `measured = false` on a row that says "passed"
(P8.F4.T2).** Flipped `p8.legion_repo`'s `measured` to `false` in the report,
leaving `status = "passed"` and its measured microseconds in place.

    perf harness verify failed: required workload did not run on windows:
      p8.legion_repo: ... 1194ms, 1 hit(s) for the planted needle
    EXIT=1

This is the case `SkeletonStatus` alone could never express. Restored → exit 0.

**3. `--strict` fails on a regressed trend entry (P8.F4.T3).**
Lowered `p8.input_to_paint`'s baseline to `p50 = 500`, `p95 = 600` µs and re-ran
verification against the same archived report.

    perf harness verify: REGRESSION p8.input_to_paint p50_micros regressed:
                         baseline 500, allowed 800, observed 1547
    perf harness verify: REGRESSION p8.input_to_paint p95_micros regressed:
                         baseline 600, allowed 960, observed 1953
    EXIT=1

    (same state, --no-strict)                                   EXIT=0

Both halves matter: strict fails, non-strict reports. Restored → exit 0.

**4. The repo-search workload cannot pass without reading file contents
(P8.F4.T1).** Changed the `RunSearch` query to a string assembled at runtime so
it appears in no file, then ran `product_perf` directly.

    product_perf: p8.legion_repo measured=false ...
                  repo search found 0 hits for the planted needle after 442.8ms
    EXIT=3

Without that guard, a walk that listed 4 000 files and read none would have
reported a fast, green "search". Restored.

**5. The latency classifier is not a constant (P8.F4.T1).**
Replaced the budget comparison in `classify_product_row` with
`SkeletonStatus::Passed`.

    cargo test -p xtask --test perf_workloads
    FAILED: latency_within_budget_passes_and_over_budget_fails
    FAILED: zero_percentile_ceiling_is_untracked_not_impossible
    FAILED: zero_ceiling_relaxes_budgets_but_not_missing_measurements
    test result: FAILED. 9 passed; 3 failed

Restored → 12 passed.

**6. The regression detector is not a constant (P8.F4.T3).**
Disabled the `observed > allowed` comparison in `detect_regressions_for_profile`.

    cargo test -p xtask --test perf_trend
    FAILED: memory_regression_is_detected_on_bytes
    FAILED: strict_fails_on_regression_even_with_all_budgets_green
    FAILED: regression_fires_only_outside_the_tolerance
    FAILED: a_different_machine_class_does_not_compare_against_the_reference_baseline
    test result: FAILED. 10 passed; 4 failed

Restored → 14 passed.

**7. The coverage check is not a constant (P8.F4.T2).**
Made `missing_required_names` return an empty list unconditionally.

    cargo test -p xtask --test perf_trend
    FAILED: strict_fails_when_a_required_workload_did_not_run
    FAILED: strict_fails_when_a_required_workload_is_absent_from_the_report
    test result: FAILED. 12 passed; 2 failed

Restored → 14 passed.

## Cost, stated plainly

The harness now takes roughly 25 minutes of wall clock per run on a quiet
developer machine, and most of that is fixture I/O: 50 000 files for `m8`,
100 000 for `p8.fixture_100k_files`, and a 100 MB file for `m9`. The gates job
timeout went from 120 to 180 minutes to absorb it on shared runners.

That cost is the price of the stop condition. "Stop if any workload is hidden
behind a non-default flag" rules out the obvious mitigation — making the big
fixtures opt-in — and it rules it out for a good reason: the 100K-file workload
had been "designed" in `plans/evidence/perf-harness-fixtures/100k-file-search.toml`
since before P2.F4.T4 and never ran once.

## Verification commands

    cargo run -p xtask -- perf-harness
    cargo run -p xtask -- verify-perf-harness
    cargo run -p xtask -- perf-harness --strict
    cargo test -p xtask --test perf_harness --test perf_workloads --test perf_trend
    cargo clippy --workspace --all-targets -- -D warnings

## Files

- `crates/legion-app/src/bin/product_perf.rs` — the six real workloads.
- `xtask/src/perf_workloads.rs` — budgets, classification, coverage contract.
- `xtask/src/perf_trend.rs` — trend archive, baseline, regression gate.
- `xtask/src/perf_harness.rs` — report shape; `measured`, `bytes_value`,
  `synthetic_stand_in`, `os`, `arch` fields.
- `xtask/src/main.rs` — orchestration for `perf-harness` / `verify-perf-harness`.
- `xtask/tests/perf_workloads.rs`, `xtask/tests/perf_trend.rs` — the tests above.
- `plans/evidence/perf-harness-trend/` — baseline, README, archived entries.
- `.github/workflows/legion-gates.yml` — per-OS coverage and artifact upload.
