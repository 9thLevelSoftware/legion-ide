# Perf harness trend archive

This directory stores archived `xtask perf-harness` reports, grouped by host OS.
Each CI run appends a timestamped `perf_report.toml` snapshot under the matching
OS subdirectory so the harness can compare the current run against the latest
prior trend entry.
