# M8 — WS-SEARCH-01 Search Polish Evidence

## Status

Accepted (all review rounds complete — all findings addressed with named passing tests).

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
  `DesktopRuntime::open` wires `FilePaletteUsageRepository` at
  `workspace_root/.legion/palette_usage.json`; product-path restart-survival test passes.
- P2.F4.T4: `SkeletonKind::SearchStream50K` in xtask perf harness; `run_search_stream_50k`
  delegates to `classify_skeleton_status` (no longer hardcodes `Skipped`); gate can
  be activated via `LEGION_PERF_FAIL_ON_BUDGET_ENV`; `SkeletonDescriptor` has explicit
  `file_count: Option<usize>` field (no longer misuses `fixture_bytes`).

## Commits

- `b115e0b` feat(search): extract fuzzy scorer to legion-index/src/fuzzy.rs (P2.F4.T3)
- `3ad8304` feat(search): search options, stale markers, and binary/size safeguards
- `3cdfcbb` feat: SEARCH.06 palette usage history + P2.F4.T4 search-stream perf workload
- `9a97879` fix(search): address all M8 PKT-SEARCH code review findings (round 2)
- `8f0f8fe` docs: update WS-SEARCH-01 evidence with fix-round findings
- `9076c15` fix(search): round 3 — wire persistence, add behavior tests, fix tag logic

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
- Desktop header renders active option tags (`[Cc]`/`[W]`/`[.*]`) for non-default options;
  plain case-insensitive search has clean header (NEW-1 fix: `[ci]` else-branch removed).
- App-level behavior tests (all dispatch through `DesktopAction::RunSearch`):
  - `search_options_case_sensitive_yields_different_result_counts`
  - `search_options_use_regex_matches_pattern_literal_does_not`
  - `search_options_invalid_regex_surfaces_validation_error`
  - `search_options_whole_word_restricts_partial_matches`
  - `search_options_header_tags_reflect_active_options`
- `search_glob_filter_restricts_to_matching_files` (legion-project): glob filter test.
- Original tests: `cancellation_stops_workspace_search_walker`,
  `search_skips_binary_files_and_counts_them`, `search_options_literal_case_whole_word`.

```
command: cargo test -p legion-project
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok. 19 passed; 0 failed
```

```
command: cargo test -p legion-desktop --test search_workflow
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok. 11 passed; 0 failed
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

### SEARCH.06 — Palette usage history (IMP-2 fix: wired into product path)

- `crates/legion-storage/src/lib.rs`: `FilePaletteUsageRepository` with atomic-rename write
  pattern, LRU cap at 500 entries (evicts lowest-count), and load-on-open from JSON.
- `InMemoryPaletteUsageRepository` retained for tests.
- `AppComposition.palette_usage` is now `Box<dyn PaletteUsageRepository>`; public
  `set_palette_usage_repository()` method allows swap-in at startup.
- `crates/legion-desktop/src/workflow.rs`: `DesktopRuntime::open` calls
  `app.set_palette_usage_repository(Box::new(FilePaletteUsageRepository::open(&path)))`
  at `workspace_root/.legion/palette_usage.json`.
- Storage-layer tests: `file_palette_usage_round_trip`, `file_palette_usage_restart_retains_ranking_boost`,
  `file_palette_usage_cap_eviction`.
- Product-path test: `palette_usage_persists_ranking_boost_across_desktop_runtime_restart`
  in `crates/legion-desktop/tests/palette_persistence.rs` — records 20 usages via
  DesktopRuntime, verifies `.legion/palette_usage.json` written, reopens fresh runtime,
  asserts ranking boost survived.

```
command: cargo test -p legion-storage
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok. 36 passed; 0 failed
```

```
command: cargo test -p legion-desktop --test palette_persistence
cwd: C:/Users/dasbl/RustroverProjects/legion-ide-search
exit code: 0
output (trimmed): test result: ok. 1 passed; 0 failed
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

## Merged-tree standing-gate run (2026-07-05, branch m8/search-polish)

Context: main merged (includes LSP substrate PR #34, terminal productization
PR #36, CI fixes PR #35/#38); working directory
C:/Users/dasbl/RustroverProjects/legion-ide-search; Windows 11; builds -j 4.
Merge-time findings resolved by the controller before green (review trail in
.superpowers/sdd/progress-m8-campaign.md):

- Dependency-policy pairing added for the perf-workload xtask deps
  (legion-project/platform/security; scoped tooling-only rationale) after
  check-deps correctly rejected the unpaired edges.
- Palette usage persistence wiring relocated from DesktopRuntime::open into
  AppComposition::enable_palette_usage_persistence, removing an improper
  legion-desktop -> legion-storage dependency (projection-only boundary);
  behavior unchanged (persistence test green).
- Clippy gate: sort_by_key + io::Error::other in legion-storage,
  SearchQueryOptions bundling for run_search (was 8 args), literal-bool
  assert in legion-project tests.

| Gate | Result |
| --- | --- |
| cargo fmt --all --check | PASS |
| xtask check-deps / docs-hygiene / claim-audit / no-egui-textedit / verify-kanban-backlog | PASS |
| xtask release-pipeline --dry-run + verify-release-pipeline | PASS |
| cargo check --workspace --all-targets | PASS |
| cargo test --workspace --all-targets --no-fail-fast | PASS (192 test binaries, 0 failures) |
| cargo clippy --workspace --all-targets -- -D warnings | PASS (exit 0) |
| xtask perf-harness + verify-perf-harness | PASS |
| cargo deny check | PASS |
| xtask rust-analyzer-smoke | PASS (real rust-analyzer 1.95.0) |

## 2026-08-16 — P2.F4.T2: multi-file search/replace as a workspace-edit proposal

### What the acceptance required

Backlog `P2.F4.T2` (`plans/kanban/legion-ga-backlog.toml`):

- acceptance: "Replace never mutates files outside proposal review."
- stop_condition: "Stop if any replace path writes without going through a proposal."

### What was already true before this change

A great deal of the pipeline already existed and was not rebuilt:

- **Lexical workspace search** (`P2.F4.T1`, already `done`): `WorkspaceActor::search_workspace_stream`
  in `crates/legion-project/src/lib.rs:4826` already enumerates concrete per-match
  positions across many files (`WorkspaceSearchHit` carries `file_id`,
  `canonical_path`, one-based `line_number`, and a file-absolute `byte_range`).
  `AppComposition::run_search` already projects those into `SearchResultProjection`
  rows. Nothing about search enumeration needed to be written.
- **Multi-file workspace-edit proposals** already existed for two other producers:
  LSP rename (`crates/legion-app/src/language/translate.rs:173`) and structural
  search/rewrite (`legion_index::build_structural_rewrite_preview_payload`,
  `crates/legion-index/src/lib.rs:5603`, driven from
  `AppComposition::create_structural_rewrite_proposal`).
- **The apply pipeline** for `ProposalPayload::WorkspaceEdit` already existed
  (`apply_workspace_edit_proposal`), including the all-targets-preflight-before-any-mutation
  ordering, per-file version preconditions, and rollback of committed mutations.
- **`crates/legion-app/tests/find_replace_workflow.rs`** already covered
  single-buffer find/replace (`ReplaceOne`, `ReplaceAll`, single undo group).
  It is entirely **active-buffer scoped** — no multi-file case.
- `crates/legion-app/tests/structural_search_workflow.rs` already proved the
  *structural* rewrite reaches two files through the proposal pipeline.

### What was genuinely missing

There was **no lexical (textual) multi-file replace at all** — no analogue of
`build_structural_rewrite_preview_payload` for text search hits, and no
`AppComposition` entry point that turns a workspace search into a replace. Greps
for `ReplaceInFiles`, `WorkspaceReplace`, `replace_in_files`, `MultiFileReplace`
returned nothing. Lexical search stopped at a read-only projection.

Note for the record: the backlog `files` list for this task names
`crates/legion-project/src/search.rs` and `crates/legion-protocol/src/proposal.rs`.
Neither file exists; both crates are single-file libs (`src/lib.rs`).

### What changed

Three commits.

**1. Pure move (no behavior change).** `crates/legion-app/src/lib.rs` was ~38,367
lines and is the merge chokepoint for the crate. The lexical search region was
moved verbatim into the new `crates/legion-app/src/search.rs` (700 lines):
`SearchQueryOptions`, `ParsedSearchQuery`, `SearchBuildResult`, `SearchTextInput`,
`SearchLineInput`, `parse_search_query`, `compile_search_globset`,
`normalize_search_limit`, `search_status_for_result`, `build_search_projection`,
`collect_search_results_for_text/_for_line`, `count_chars_up_to`,
`push_bounded_search_result`, `bounded_search_snippet`, and
`AppComposition::{run_search, cancel_search, run_active_file_search,
run_degraded_active_file_search, run_workspace_search}`. Structural search stayed
in `lib.rs`. `SearchQueryOptions` is re-exported from the crate root so
`legion_app::SearchQueryOptions` is unchanged. `lib.rs` is now 37,683 lines.

**2. The feature.** `AppComposition::propose_workspace_replace(query, replacement,
options, limit)` in `crates/legion-app/src/search.rs`, returning
`WorkspaceReplaceProposalOutcome`. It:

- runs the existing workspace search to decide *which* files participate;
- recomputes each file's match ranges against its **live editor buffer text**,
  not the disk text, because the apply path edits buffers and refuses closed
  files — reusing disk offsets would address the wrong text in a dirty buffer;
- builds one `ProposalPayload::WorkspaceEdit` with one `WorkspaceTextEdit` per
  file, `buffer_id` set, per-file `ProposalVersionPreconditions` taken from the
  live buffer, and `ProposalTargetCoverageKind::Complete` coverage naming every
  file;
- drives it to `Previewed` and stops. It never calls apply and never writes.

Two refusals are built in, both fail-closed with no proposal at all:

- **Truncated match set.** If `result_limit` omitted any match, no proposal is
  built. A replace that edits a prefix of the matches is worse than one that
  declines, because the result looks complete.
- **Unreachable matching file.** If any matching file is not open in a buffer
  (or its buffer no longer contains the query), the whole replace is withheld.
  This is not a style choice: the protocol validator denies partial coverage
  outright (`proposal.incomplete_target_coverage` /
  `proposal.omitted_target_coverage` / "workspace-edit proposal requires complete
  target coverage"), so a partially-covering replace is unrepresentable, not
  merely discouraged. Verified empirically — an early revision that emitted
  `ProposalTargetCoverageKind::Partial` was rejected at validation.

**3. Pre-existing coordinate bug found and fixed.** `run_workspace_search`
computed the projected column with
`count_chars_up_to(&hit.line_text, hit.byte_range.start)`, passing a
*file-absolute* byte offset into a *single line's* text. `count_chars_up_to`
falls back to the full char count when the index is out of bounds, so every
workspace-search match on any line after the first was reported as starting and
ending at the end of its line. `SearchResultProjection.range.start.character` was
wrong for essentially every real hit. No existing test asserted on `range`, which
is why it survived `P2.F4.T1`. Fixed by adding
`WorkspaceSearchHit.line_byte_start` in `crates/legion-project/src/lib.rs` (set at
both hit-construction sites, walker and indexed) and subtracting it before
measuring the column. Byte offsets remain absolute, as documented.

### Tests

New file `crates/legion-app/tests/workspace_replace_proposal.rs` — 7 tests:

| Test | Proves |
| --- | --- |
| `workspace_replace_proposes_every_matching_file_and_writes_none` | The load-bearing case: 3 files / 4 matches produce one `WorkspaceEdit` proposal with 3 `file_edits`, `Complete` coverage, 3 targets, `omitted_target_count == 0`, one coverage target per buffer — and all 3 buffers *and* all 3 disk files are byte-identical afterwards. |
| `workspace_replace_apply_edits_every_buffer_and_leaves_disk_untouched` | After approve+apply, all 3 buffers carry the replacement and all 3 disk files are unchanged (a text-edit apply writes to the buffer; saving is separate). |
| `workspace_replace_undo_reverts_every_buffer_not_only_the_active_one` | Reversal reaches every file: switching to each of the 3 buffers and issuing one undo restores all 3 originals, including the two that were never active. |
| `workspace_replace_apply_mutates_nothing_when_one_target_went_stale` (negative) | Invalidating only the middle target makes the whole apply fail, and the two still-valid buffers are *not* edited. No half-applied replace. |
| `workspace_replace_refuses_when_the_match_set_is_truncated` (negative) | A 2-result bound over 4 matches yields no proposal, a "Replace refused" diagnostic, and zero mutations. |
| `workspace_replace_refuses_when_a_matching_file_is_not_open` (negative) | An unreachable match yields no proposal, names the closed file, and leaves both the open buffer and the closed file on disk untouched. |
| `workspace_replace_proposal_is_registered_for_review_before_apply` (negative) | After `RejectProposal`, a subsequent `ProposalRequest::Apply` is refused and every buffer and file still holds its original text. |

Regression test for the coordinate fix, appended to the existing suite:
`daily_editing_search_workspace_reports_column_relative_to_its_own_line` in
`crates/legion-app/tests/daily_editing_search.rs`. It asserts
`range.start.character == 3` / `end.character == 9` for a match on line 2 while
`byte_offset` stays absolute at 15/21. Before the fix it read 12/12.

```
command: cargo test -p legion-app --test workspace_replace_proposal
cwd: D:\legion-ide\.claude\worktrees\agent-aed37416599491fc5
exit code: 0
output: test result: ok. 7 passed; 0 failed
```

```
command: cargo test -p legion-app --test daily_editing_search
cwd: D:\legion-ide\.claude\worktrees\agent-aed37416599491fc5
exit code: 0
output: test result: ok. 10 passed; 0 failed (was 9 before this change)
```

### Gate results (2026-08-16)

Every command below was redirected to a file and its exit code read directly,
never through a pipe.

| Gate | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all` | 0 | PASS |
| `cargo test --workspace --all-targets --no-fail-fast` | 0 | PASS — 2869 passed, 0 failed, 254 test binaries |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | PASS |
| `cargo run -p xtask -- verify-kanban-backlog` | 0 | PASS — 10 epics, 41 features, 160 tasks |
| `cargo run -p xtask -- docs-hygiene` | 0 | PASS |
| `cargo run -p xtask -- claim-audit` | 0 | PASS |
| `cargo run -p xtask -- verify-readiness-consistency` | 0 | PASS |

### Not claimed

Explicitly **not** done, and why:

- **No desktop/UI surface.** There is no `CommandDispatchIntent` variant, no
  search-panel "Replace All" button, and no keybinding. `propose_workspace_replace`
  is an `AppComposition` API only. Wiring `legion-desktop` was outside the task's
  declared `files` and would need its own review; the backlog `files` list names
  only app/project/protocol paths.
- **No regex capture-group expansion in the replacement.** `regex:`/`re:` queries
  match correctly, but the replacement is inserted literally — `$1` is not
  expanded. VS Code-style capture substitution is not implemented and no test
  claims it.
- **Closed files are not opened automatically.** A replace whose match set
  reaches a file that is not open is refused entirely. Auto-opening would change
  tab and activation state (`bind_opened_file` always activates), which is a
  product decision outside this task.
- **Buffer-only matches are not found.** Candidate files come from the disk scan,
  so if a dirty buffer *introduces* a match that is not on disk, that file will
  not participate. This matches the semantics of `P2.F4.T1`'s search itself,
  which also scans disk. The inverse case (disk matches, buffer no longer does)
  *is* detected and causes a refusal.
- **`ProposalRequest::Rollback` was not exercised for this route.** The declared
  reversal for a text-edit route is `ProposalRollbackAction::EditorUndoGroup` —
  editor undo, which the undo test drives. The lifecycle `Rollback` command
  records that a reversal happened; it does not perform one, so a test asserting
  that it restores buffer text would be asserting something false.
- **The `line_number` base inconsistency was left alone.** `SearchResultProjection.line_number`
  is documented as zero-based (`crates/legion-ui/src/ui.rs:1258`) and active-file
  search honours that, but workspace search passes `WorkspaceSearchHit.line_number`
  through unchanged, which is one-based. Workspace rows are therefore off by one
  against their own field contract. This is a real pre-existing defect but it is
  a `P2.F4.T1` UI-semantics change with existing assertions depending on current
  behaviour, so it was reported rather than changed. The replace path does not
  depend on it — edits are built from byte offsets only.
- **No `BatchPreflightRoute::WorkspaceEdit` rollback-proof arm.** Noted while
  reading: `rollback_action_matches_route` has no arm for
  `BatchPreflightRoute::WorkspaceEdit`, so a workspace-edit item inside a *batch*
  can never satisfy `item_has_exact_rollback_proof`. Consistent with
  `ProposalRollbackAvailability::BestEffort` for that payload, but it means batch
  preflight always flags such items. Out of scope here; recorded for whoever owns
  batch rollback proofs.
