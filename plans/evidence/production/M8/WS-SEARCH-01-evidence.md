# M8 — WS-SEARCH-01 Search Polish Evidence

## Status

Accepted (fix round complete — all review findings addressed).

## Acceptance targets

- P2.F4.T3: Enhanced fuzzy scorer in `legion-index/src/fuzzy.rs` with consecutive-run,
  word-boundary, camelCase, path-segment, and filename-region bonuses; all 4
  `palette_fuzzy_score` call sites in legion-app ported to `fuzzy_score_tuple`
  (renamed from `fuzzy_score_legacy` in fix round; doc comment accurately describes
  behavioral difference from the old scorer).
- SEARCH.01 / P2.F4.T1: End-to-end search options threaded through all protocol layers
  (`DesktopAction::RunSearch`, `CommandDispatchIntent::RunSearch`,
  `AppCommandRequest::RunSearch`, `SearchQueryOptions`, `ParsedSearchQuery`,
  `SearchBuildResult`, `SearchProjection`); desktop header renders active option tags
  (`[Cc]`/`[W]`/`[.*]`); glob filter test added (`search_glob_filter_restricts_to_matching_files`);
  cancellation test asserts walker stops.
- SEARCH.10: `stale: bool` field on `SearchResultProjection`; existing results marked
  stale when a new query starts. Stale-marker visibility limitation (zero practical
  window in synchronous model) documented in code comment.
- SEARCH.12: NUL-byte binary sniff in `search_workspace_stream`; `skipped_binary_count`
  propagated from `WorkspaceSearchReport` through `SearchBuildResult` into
  `SearchProjection`; desktop status row renders "N binary files skipped".
- SEARCH.06: Per-workspace palette usage counts with real disk persistence via
  `FilePaletteUsageRepository` (atomic-rename write, LRU cap at 500 entries,
  load-on-open); `AppComposition.palette_usage` is `Box<dyn PaletteUsageRepository>`;
  `set_palette_usage_repository()` allows swap-in at startup.
- P2.F4.T4: `SkeletonKind::SearchStream50K` in xtask perf harness; `run_search_stream_50k`
  delegates to `classify_skeleton_status` (no longer hardcodes `Skipped`); gate can
  be activated via `LEGION_PERF_FAIL_ON_BUDGET_ENV`; `SkeletonDescriptor` has explicit
  `file_count: Option<usize>` field (no longer misuses `fixture_bytes`).

## Commits

- `b115e0b` feat(search): extract fuzzy scorer to legion-index/src/fuzzy.rs (P2.F4.T3)
- `3ad8304` feat(search): search options, stale markers, and binary/size safeguards
- `3cdfcbb` feat: SEARCH.06 palette usage history + P2.F4.T4 search-stream perf workload
- `9a97879` fix(search): address all M8 PKT-SEARCH code review findings

## What was verified

### P2.F4.T3 — Fuzzy scorer (MIN-1 fix: rename)

- `crates/legion-index/src/fuzzy.rs`: `fuzzy_score_legacy` renamed to `fuzzy_score_tuple`
  with accurate doc comment explaining the behavioral difference from `fuzzy_score`.
- Test `tuple_adapter_returns_tuple` (renamed from `legacy_adapter_returns_tuple`) passes.
- All 4 call sites in `crates/legion-app/src/lib.rs` updated.

```
command: cargo test -p legion-index
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok. 14 passed; 0 failed
```

### SEARCH.01 / P2.F4.T1 — Search options + cancellation (IMP-3 + MIN-4 fix)

- Options (`case_sensitive`, `whole_word`, `use_regex`) threaded end-to-end through all
  protocol layers: `DesktopAction::RunSearch`, `CommandDispatchIntent::RunSearch`,
  `AppCommandRequest::RunSearch`, `SearchQueryOptions`, `ParsedSearchQuery`,
  `SearchBuildResult`, `SearchProjection`.
- Desktop header renders active option tags (`[Cc]`/`[W]`/`[.*]`).
- `crates/legion-project/src/lib.rs`: `search_glob_filter_restricts_to_matching_files` test
  added — creates `match_me.rs` and `skip_me.txt` both containing GLOB_NEEDLE; builds
  `WorkspaceSearchFilters` with `include: *.rs`; asserts exactly 1 hit from `match_me.rs`.
- Original tests: `cancellation_stops_workspace_search_walker`,
  `search_skips_binary_files_and_counts_them`, `search_options_literal_case_whole_word`.

```
command: cargo test -p legion-project
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok. 19 passed; 0 failed
```

### SEARCH.10 — Stale markers (MIN-3 fix: limitation documented)

- `crates/legion-ui/src/ui.rs`: `pub stale: bool` field on `SearchResultProjection`.
- `crates/legion-app/src/lib.rs`: results marked stale before new query runs; code comment
  documents synchronous-model visibility limitation (zero practical window).
- `crates/legion-desktop/src/search.rs`: stale rows tagged `[stale]` in view model.

```
command: cargo test -p legion-desktop
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok. 30 passed; 0 failed (unit) + integration tests pass
```

### SEARCH.12 — Binary sniff (IMP-1 fix: projection + desktop rendering)

- `crates/legion-project/src/lib.rs`: inline 8 KiB NUL-byte heuristic; `skipped_binary_count`
  field on `WorkspaceSearchReport`.
- `crates/legion-app/src/lib.rs`: `skipped_binary_count` propagated from
  `WorkspaceSearchReport` into `SearchBuildResult` then `SearchProjection`.
- `crates/legion-ui/src/ui.rs`: `pub skipped_binary_count: usize` on `SearchProjection`.
- `crates/legion-desktop/src/search.rs`: renders "N binary files skipped" when > 0.
- `search_skips_binary_files_and_counts_them` test verifies `skipped_binary_count == 1`.

### SEARCH.06 — Palette usage history (IMP-2 fix: real disk persistence)

- `crates/legion-storage/src/lib.rs`: `FilePaletteUsageRepository` with atomic-rename write
  pattern, LRU cap at 500 entries (evicts lowest-count), and load-on-open from JSON.
- `InMemoryPaletteUsageRepository` retained for tests.
- `AppComposition.palette_usage` is now `Box<dyn PaletteUsageRepository>`; public
  `set_palette_usage_repository()` method allows swap-in at startup.
- New tests: `file_palette_usage_round_trip`, `file_palette_usage_restart_retains_ranking_boost`,
  `file_palette_usage_cap_eviction`.

```
command: cargo test -p legion-storage
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok. 36 passed; 0 failed
```

```
command: cargo test -p legion-app --lib palette_usage_frequency_bonus
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test tests::palette_usage_frequency_bonus_lifts_heavily_used_command ... ok
```

### P2.F4.T4 — 50 K-file search-stream perf workload (IMP-4 + MIN-2 fix)

- `xtask/src/perf_harness.rs`: `SkeletonDescriptor` has `file_count: Option<usize>` field
  (with `#[serde(default)]`); `m8_search_stream_50k()` sets `file_count: Some(50_000)` and
  `fixture_bytes: 0`; `run_search_stream_50k` reads `file_count` instead of `fixture_bytes`.
- `run_search_stream_50k` now calls `classify_skeleton_status(scan_elapsed, skeleton.budget())`
  instead of hardcoding `SkeletonStatus::Skipped`; gate activates via `LEGION_PERF_FAIL_ON_BUDGET_ENV`.
- New unit tests:
  - `search_stream_50k_classify_skeleton_status_report_only_by_default`
  - `search_stream_50k_env_override_activates_gate_failed`
  - `search_stream_50k_env_override_activates_gate_passed`
  - `m8_search_stream_50k_descriptor_uses_file_count_field`
- `xtask/tests/perf_harness.rs` fixtures updated with `file_count: None`.

```
command: cargo test -p xtask
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok. 6 passed (lib unit) + 20 passed (perf_harness integration)
```

## Verification commands

```bash
cargo test -p legion-index
cargo test -p legion-project
cargo test -p legion-storage
cargo test -p legion-desktop
cargo test -p legion-app
cargo test -p xtask
cargo build -p xtask
cargo run -p xtask -- perf-harness --out target/perf
```
