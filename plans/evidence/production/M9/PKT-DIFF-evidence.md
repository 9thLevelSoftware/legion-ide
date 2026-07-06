# PKT-DIFF Evidence — Multi-file Proposal Review Surface

Branch: `m9/review-surface`  
Date: 2026-07-06

---

## Status: DONE

All 5 tasks complete + 1 fix round (7 findings resolved). 7 commits produced. All verification gates green.

---

## Verification Results

### `cargo test -p legion-editor` (diff module — Task 1)
```
running 8 tests
test diff::tests::full_file_replacement_produces_one_hunk ... ok
test diff::tests::multi_line_delete_produces_one_hunk ... ok
test diff::tests::single_line_change_produces_one_hunk ... ok
test diff::tests::diff_hunks_to_section_projection_multi_hunk ... ok
test diff::tests::multi_line_insert_produces_one_hunk ... ok
test diff::tests::no_change_produces_no_hunks ... ok
test diff::tests::distant_changes_produce_two_hunks ... ok
test diff::tests::to_section_projection_produces_valid_section ... ok

test result: ok. 8 passed; 0 failed; 0 ignored
```

### `cargo test -p legion-app --test proposal_review_surface` (Tasks 2, 3, 4)
```
running 14 tests
test accepted_hunk_ids_reflects_current_decisions ... ok
test changed_files_have_non_empty_chunks ... ok
test evidence_panel_carries_structured_fields_only ... ok
test evidence_panel_with_test_results_and_commands ... ok
test filter_by_accepted_hunks_retains_correct_targets ... ok
test five_file_proposal_produces_five_sections ... ok
test full_filtering_chain_accepted_targets_only ... ok
test hunk_disposition_state_defaults_to_pending ... ok
test multiple_undo_operations_are_lifo ... ok
test non_batch_proposal_produces_empty_surface ... ok
test partial_accept_filters_to_two_targets ... ok
test partial_hunk_accept_excludes_whole_target_conservative ... ok
test undo_disposition_change_restores_previous ... ok
test undo_on_empty_stack_returns_false ... ok

test result: ok. 14 passed; 0 failed; 0 ignored
```

### `cargo test -p legion-desktop --test keyboard_nav` (Task 5)
```
running 14 tests
test product_mode_switch_accepts_keyboard_activation ... ok
test review_accept_all_and_dismiss_lifecycle ... ok
test review_hunk_accept_records_disposition_for_focused_hunk ... ok
test review_hunk_disposition_actions_noop_with_no_reviews ... ok
test review_hunk_key_dispatch_alt_arrow_right_via_egui ... ok
test review_hunk_next_increments_and_wraps_with_seeded_hunks ... ok
test review_hunk_next_is_noop_with_no_reviews ... ok
test review_hunk_prev_decrements_and_wraps_with_seeded_hunks ... ok
test review_hunk_prev_is_noop_with_no_reviews ... ok
test t4_problem_activate_happy_path ... ok
test t4_problem_activate_with_no_problems_is_noop ... ok
test t4_problem_key_dispatch_via_egui ... ok
test t4_problem_next_increments_selection ... ok
test t4_problem_prev_decrements_selection ... ok

test result: ok. 14 passed; 0 failed; 0 ignored
```

### `cargo run -p xtask -- check-deps`
```
dependency policy checks passed
```

### `cargo check --workspace`
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 05s
```

---

## Root Causes Resolved

### 1. `proposal.rs` orphan module
`crates/legion-app/src/proposal.rs` existed on disk but was not compiled because
`pub mod proposal;` was absent from `lib.rs`. Added the declaration so the module
is now part of the crate compilation graph.

### 2. `CreateFileProposal` wrong fields in test fixture
The test fixture used non-existent fields (`workspace_id`, `file_id`, `content`).
The struct only has `path: CanonicalPath` and `initial_content: Option<String>`.
Fixed by using the correct fields.

---

## Architecture Isolation

| Crate | May Depend On | Dependency Added |
|---|---|---|
| `legion-editor` | `legion-protocol`, `legion-text` | None (already present) |
| `legion-app` | `legion-editor` (new use of diff module) | None new in Cargo.toml |
| `legion-desktop` | `legion-app` | None new |
| `legion-protocol` | none | 5 new DTOs added |

---

## Deliverables Summary

### Task 1: `crates/legion-editor/src/diff.rs`
- LCS-based line-level diff via O(m×n) DP table with iterative traceback
- `compute_line_diff(old_text, new_text) -> Vec<DiffHunk>`
- `DiffHunk::to_section_projection(...)` and `diff_hunks_to_section_projection(...)` bridge to protocol DTOs
- 8 unit tests: no_change, single_line_change, multi_line_insert, multi_line_delete, full_file_replacement, distant_changes_two_hunks, to_section_projection, diff_hunks_to_section_projection_multi_hunk

### Task 2: `crates/legion-app/src/proposal.rs`
- `compute_proposal_diff_surface(proposal, file_contents) -> ProposalDiffSurfaceProjection`
- `filtered_batch_proposal_for_accepted_hunks(proposal, diff_surface, accepted_hunk_ids) -> Option<WorkspaceProposal>`

### Task 3: `crates/legion-app/src/proposal.rs` (continued)
- `ProposalHunkDispositionState` with `set_hunk_disposition`, `undo_last_disposition_change`, `disposition`, `accepted_hunk_ids`, `undo_depth`
- LIFO undo stack with `HunkDispositionUndoEntry`

### Task 4: `crates/legion-protocol/src/lib.rs`
- `TestResultsSummary`, `CommandSummary`, `RiskRuleEvidence`, `ProposalProvenance`, `ProposalEvidencePanel`
- Existing `DesktopProposalEvidencePanelViewModel` in `legion-desktop/src/view/proposal_review.rs` unchanged (already covers structured rendering)

### Task 5: `crates/legion-desktop/src/bridge.rs` + `workflow.rs` + `view.rs`
- New `DesktopAction` variants: `ReviewHunkNext`, `ReviewHunkPrev`, `ReviewHunkAccept`, `ReviewHunkReject`, `ReviewAcceptAll`, `ReviewRejectAll`, `ReviewApply`, `ReviewDismiss`
- `review_hunk_selected_index: usize` in `DesktopRuntime` and `DesktopProjectionViewState`
- `hunk_dispositions: ProposalHunkDispositionState` in `DesktopRuntime`
- Keybindings: `Alt+ArrowRight/Left` (next/prev), `Alt+Y/X` (accept/reject), `Alt+Shift+Y/X` (accept-all/reject-all), `Alt+Enter` (apply), `Alt+Escape` (dismiss)
- `review_hunk_selected_index_for_test()` accessor on both `DesktopRuntime` and `DesktopEframeApp`

### Fix Round: Review findings F1–F7
- F1 (Critical): Stub handlers wired to `ProposalHunkDispositionState` calls
- F2: Conservative all-hunks-required filtering with test
- F3: `ReviewApply` + `ReviewDismiss` actions with handlers
- F4: `workspace_proposal_for_id` + `compute_diff_surface_for_proposal` public methods on AppComposition
- F5: End-to-end filtering chain test
- F6: `DesktopEvidencePanelDtoViewModel` with `From<ProposalEvidencePanel>` impl
- F7: 4 seeded navigation tests with real hunk assertions
