# Perf-harness trend archive

`cargo run -p xtask -- perf-harness` writes one entry per run into
`entries/`, and compares that run against `baseline.toml`. This directory is in
the tracked tree on purpose: a performance budget that can be changed without
anyone seeing the change in a diff is not a budget.

## Files

- `baseline.toml` — the reference numbers, per workload **per OS**, plus the
  drift tolerance. Reviewed like code. Changing a number here is a claim that
  the product got slower on purpose.
- `entries/<os>-<sha>-<timestamp>.toml` — one archived run. CI uploads the
  whole directory as an artifact from each of the three OS jobs.

## What counts as a regression

A workload regresses when a tracked metric exceeds
`baseline * (1 + tolerance_percent / 100)` for the same OS. `cargo run -p xtask
-- perf-harness --strict` exits non-zero when that happens.

`--strict` also fails on any workload that did not run. A workload that did not
run is not a workload that was within budget, and on a three-OS matrix a
shorter report is the failure mode that goes unnoticed.

Metrics with a baseline of `0` are not tracked for that workload. That is
sometimes structural — a single-sample workload has no meaningful p95, a
latency workload has no byte value — and sometimes deliberate: the two search
workloads are dominated by the host's per-file-open cost rather than by Legion,
so a tolerance-based comparison of them would fire on the machine. Their
`note` field says which case applies.

## Why the baseline is per OS and per machine class

A Windows number and a macOS number for the same workload measure different
systems, and so do a developer workstation and a shared GitHub runner. Rows are
therefore keyed by `os` **and** `profile`:

- `profile = "reference"` — a developer workstation.
- `profile = "github-hosted"` — a GitHub Actions runner, detected from the
  `GITHUB_ACTIONS` environment variable the runner sets itself, not from a
  flag.

Grading a hosted run against a workstation baseline at any tolerance would
produce a gate that fires on the hardware rather than on the change. A run with
no matching row prints `no trend baseline recorded for os=<os> profile=<p>` and
archives `baseline_status = "missing_for_os"`, so the gap is visible in the
artifact rather than silently green.

## Recording a baseline for a new OS or runner class

Run the harness on that machine, read the `p50_micros` / `p95_micros` /
`bytes_value` numbers out of the archived entry, and add a `[[workload]]` block
to `baseline.toml` with that `os`, that `profile`, and a `note` saying which
machine produced it. Reviewers can only judge a number if they know where it
came from.

## The first committed entry records a regression

`entries/windows-91e707a89430-20260819-062416Z.toml` carries a `[[regression]]`
block for `p8.startup`. That is deliberate. It is the run in which the gate
fired for the first time, against a baseline recorded from a warm-cache run; the
cause was the page cache, not the code, and the baseline was widened to the
slower observation rather than the number being quietly re-recorded. The full
account is in
`plans/evidence/production/P8.F4/perf-harness-product-workloads.md`.

## History

Until 2026-08-19 this directory held only a README saying the archive was
"empty and unwired", after a 2026-08-16 claim repair found the previous README
describing a trend comparison that no code performed
(`plans/evidence/production/PR-UI-001/2026-08-16-promotion-verification.md`).
P8.F4.T3 built the thing the old README described; see
`plans/evidence/production/P8.F4/perf-harness-product-workloads.md`.

## Entries in the working tree

Every local run adds a file to `entries/`. Those files are evidence, not
build output: commit the ones worth keeping and delete the rest. They are
deliberately not gitignored, so an entry cannot be produced and then quietly
discarded when it says something inconvenient.
