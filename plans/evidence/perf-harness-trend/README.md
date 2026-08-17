# Perf harness trend archive

> **Claim repair, 2026-08-16.** This file previously described the archive as
> live: "Each CI run appends a timestamped `perf_report.toml` snapshot under the
> matching OS subdirectory so the harness can compare the current run against the
> latest prior trend entry." No part of that happens. Verified by
> `rg -n "perf-harness-trend|trend" xtask/src -g "*.rs"` (one unrelated comment,
> no implementation) and by listing this directory (README only, no OS
> subdirectories, no archived reports). See
> `plans/evidence/production/PR-UI-001/2026-08-16-promotion-verification.md`.

**Status: empty and unwired.** This directory is the intended destination for
`P8.F4.T3` (trend + regression threshold), which is `todo`.

What actually happens today: the `Standing gates` job in
`.github/workflows/legion-gates.yml` uploads `target/perf-harness/perf_report.toml`
as a per-OS GitHub Actions artifact. Artifacts expire and are not committed here,
so no run-over-run comparison exists, and `xtask perf-harness` contains no
regression-threshold logic to compare against a prior entry.
