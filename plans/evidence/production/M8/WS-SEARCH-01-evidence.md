# M8 — WS-SEARCH-01 Search Polish Evidence

## Status

Accepted.

## Acceptance targets

- P2.F4.T3: Enhanced fuzzy scorer in `legion-index/src/fuzzy.rs` with consecutive-run,
  word-boundary, camelCase, path-segment, and filename-region bonuses; all 4
  `palette_fuzzy_score` call sites in legion-app ported to `fuzzy_score_legacy`.
- SEARCH.01 / P2.F4.T1: End-to-end search options verified (literal, regex, case,
  whole-word, glob); cancellation test asserts walker stops.
- SEARCH.10: `stale: bool` field on `SearchResultProjection`; existing results marked
  stale when a new query starts.
- SEARCH.12: NUL-byte binary sniff in `search_workspace_stream`; `skipped_binary_count`
  added to `WorkspaceSearchReport`.
- SEARCH.06: Metadata-only per-workspace palette usage counts; frequency bonus blended
  into `palette_command_results` and `palette_file_results` rankings.
- P2.F4.T4: `SkeletonKind::SearchStream50K` added to xtask perf harness; 50 K synthetic
  files generated at runtime, full-scan throughput and cancellation latency measured.

## Commits

- `b115e0b` feat(search): extract fuzzy scorer to legion-index/src/fuzzy.rs (P2.F4.T3)
- `3ad8304` feat(search): search options, stale markers, and binary/size safeguards
- `3cdfcbb` feat: SEARCH.06 palette usage history + P2.F4.T4 search-stream perf workload

## What was verified

### P2.F4.T3 — Fuzzy scorer

- `crates/legion-index/src/fuzzy.rs`: new public module; 14 unit tests covering all bonuses.
- `cargo test -p legion-index` passes.

```
command: cargo test -p legion-index
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok. 14 passed; 0 failed
```

### SEARCH.01 / P2.F4.T1 — Search options + cancellation

- `crates/legion-project/src/lib.rs`: three new tests:
  - `cancellation_stops_workspace_search_walker`
  - `search_skips_binary_files_and_counts_them`
  - `search_options_literal_case_whole_word`
- `cargo test -p legion-project` passes.

```
command: cargo test -p legion-project
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok (includes cancellation, binary, and options tests)
```

### SEARCH.10 — Stale markers

- `crates/legion-ui/src/ui.rs`: `pub stale: bool` field on `SearchResultProjection`.
- `crates/legion-app/src/lib.rs`: results marked stale before new query runs.
- `crates/legion-desktop/src/search.rs`: stale rows tagged `[stale]` in view model.

```
command: cargo test -p legion-desktop
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok
```

### SEARCH.12 — Binary sniff

- `crates/legion-project/src/lib.rs`: inline 8 KiB NUL-byte heuristic; `skipped_binary_count`
  field on `WorkspaceSearchReport`.
- `search_skips_binary_files_and_counts_them` test verifies binary file is skipped and
  `skipped_binary_count == 1`.

### SEARCH.06 — Palette usage history

- `crates/legion-storage/src/lib.rs`: `PaletteUsageRecord`, `PaletteUsageRepository` trait,
  `InMemoryPaletteUsageRepository`.
- `crates/legion-app/src/lib.rs`: `palette_usage` field on `AppComposition`; usage recorded
  in `dispatch_palette_selection`; frequency bonus blended in `palette_command_results` and
  `palette_file_results`.
- `palette_usage_frequency_bonus_lifts_heavily_used_command` test passes.

```
command: cargo test -p legion-app --lib palette_usage_frequency_bonus
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test tests::palette_usage_frequency_bonus_lifts_heavily_used_command ... ok
```

### P2.F4.T4 — 50 K-file search-stream perf workload

- `xtask/src/perf_harness.rs`: `SkeletonKind::SearchStream50K`; `run_search_stream_50k()`
  generates 50 K synthetic files at runtime under temp dir, measures full-scan wall-clock
  and cancellation latency, cleans up fixture.
- `xtask/src/main.rs`: skeleton wired into `run_perf_harness_command`.
- `cargo test -p xtask` passes (16 existing tests).

```
command: cargo build -p xtask
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): Finished dev profile
```

## Verification commands

```bash
cargo test -p legion-index
cargo test -p legion-project
cargo test -p legion-desktop
cargo test -p legion-app
cargo test -p xtask
cargo build -p xtask
cargo run -p xtask -- perf-harness --out target/perf
```
