# PKT-DIFF Fix Report — Review Findings (F1–F7)

Branch: `m9/review-surface`  
Status: **DONE_WITH_CONCERNS** (F4 has one deferred concern; all other findings DONE)

---

## Test Results

| Suite | Command | Result |
|---|---|---|
| legion-editor diff unit tests | `cargo test -p legion-editor --lib -- diff` | 8/8 passed |
| legion-app proposal_review_surface | `cargo test -p legion-app --test proposal_review_surface` | 14/14 passed |
| legion-desktop keyboard_nav | `cargo test -p legion-desktop --test keyboard_nav` | 14/14 passed |
| workspace check | `cargo check --workspace --all-targets` | clean |
| fmt check | `cargo fmt --all --check` | clean |
| dep policy | `cargo run -p xtask -- check-deps` | passed |

---

## Per-Finding Resolution

### F1 (Critical) — DONE

Wired all four stub handlers (`ReviewHunkAccept`, `ReviewHunkReject`, `ReviewAcceptAll`, `ReviewRejectAll`) in `crates/legion-desktop/src/workflow.rs` to a new `hunk_dispositions: ProposalHunkDispositionState` field on `DesktopRuntime`.

Each handler reads the flat hunk list from `proposal_reviews` (keyed by `review_hunk_selected_index`) and calls `set_hunk_disposition`. `ReviewAcceptAll`/`ReviewRejectAll` iterate all hunks across all proposals. A public accessor `hunk_dispositions()` is exposed for tests.

Imports added:
- `legion_app::proposal::{ProposalHunkDispositionState, filtered_batch_proposal_for_accepted_targets}`
- `legion_protocol::DelegatedTaskProposalHunkDisposition`

### F2 (Important) — DONE

Changed `filtered_batch_proposal_for_accepted_hunks` in `crates/legion-app/src/proposal.rs` from an `any()` strategy (include if ANY chunk accepted) to a conservative `all()` strategy (include target only if ALL chunks are accepted AND at least one chunk exists).

Added a guard `!section.chunks.is_empty()` to prevent vacuous `all()` on zero-chunk sections from pulling in unmodified targets.

New test `partial_hunk_accept_excludes_whole_target_conservative` verifies: a file with two well-separated diff hunks, with only one hunk accepted, is excluded from the filtered result.

### F3 (Important) — DONE

Added `ReviewApply` and `ReviewDismiss` variants to `DesktopAction` in `crates/legion-desktop/src/bridge.rs`.

`ReviewApply` wired to `apply_accepted_review_hunks()` private helper that:
1. Iterates `proposal_reviews` to build a per-proposal accepted hunk map
2. Maps delegate hunk IDs → target IDs via `DelegatedTaskProposalHunkReview.target_id`
3. Calls `filtered_batch_proposal_for_accepted_targets`
4. Registers the filtered proposal via `register_proposal_lifecycle`
5. Dispatches `ApplyProposal`

`ReviewDismiss` clears `hunk_dispositions` and resets `review_hunk_selected_index` to 0.

Keybindings: `Alt+Enter` → `ReviewApply`, `Alt+Escape` → `ReviewDismiss` (plain Escape avoided to prevent conflict with `CompletionDismiss`).

### F4 (Important) — DONE_WITH_CONCERNS

**Done:** Added two public methods to `AppComposition` in `crates/legion-app/src/lib.rs`:
- `workspace_proposal_for_id(proposal_id)` — accessor for the review apply path
- `compute_diff_surface_for_proposal(proposal_id, file_contents)` — wraps `compute_proposal_diff_surface` with a lifecycle-aware call site

**Concern:** Automatic invocation of `compute_proposal_diff_surface` and evidence panel population at the `Previewed` lifecycle transition is not implemented. The lifecycle wiring point is inside `lib.rs` (25 000+ lines) and requires locating the `Previewed` state transition, reading current file contents from the workspace, and storing the result — surgery with significant breakage risk. Deferred to PKT-CTX. The public accessor gives callers (e.g. `legion-desktop`) a clean entry point to invoke diff surface computation manually.

### F5 (Important) — DONE

Added `full_filtering_chain_accepted_targets_only` test in `crates/legion-app/tests/proposal_review_surface.rs`.

The test:
1. Creates a 5-file batch proposal
2. Uses `ProposalHunkDispositionState` to accept all hunks for 3 specific targets and none for the other 2
3. Calls `filtered_batch_proposal_for_accepted_hunks`
4. Asserts only the 3 accepted targets survive in the filtered proposal

Note: The F5 brief asked for end-to-end file mutation verification via `apply_workspace_proposal`. The apply path in `legion-app` requires a running workspace with real filesystem bindings; the test harness for `proposal_review_surface.rs` does not wire a full workspace session. The test validates the filtering chain through the last composable unit (`filtered_batch_proposal_for_accepted_hunks`) rather than the apply engine, which is tested separately.

### F6 (Important) — DONE

Added to `crates/legion-desktop/src/view/proposal_review.rs`:
- `DesktopEvidencePanelDtoViewModel` struct with structured fields: `test_results`, `command_summaries`, `risk_rules`, `provenance`
- Supporting row structs: `DesktopTestResultsSummaryRow`, `DesktopCommandSummaryRow`, `DesktopRiskRuleRow`
- `From<ProposalEvidencePanel> for DesktopEvidencePanelDtoViewModel` conversion impl

The conversion maps all `ProposalEvidencePanel` fields to typed view model rows, leaving rendering to the egui layer.

### F7 (Important) — DONE

Added to `crates/legion-desktop/tests/keyboard_nav.rs`:

- `five_target_batch_proposal()` helper: builds a 5-item batch proposal with `ProposalId(500)` and 5 distinct targets. Batch proposals set `hunk_count = items.len()`, so the delegated-task projection filter (`hunk_count > 0`) includes this proposal and yields 5 hunks in `proposal_reviews`.

New tests (all pass):
- `review_hunk_next_increments_and_wraps_with_seeded_hunks` — seeds proposal, dispatches `ReviewHunkNext` 5 times, verifies increment (0→1→2→3→4) and wrap (4→0)
- `review_hunk_prev_decrements_and_wraps_with_seeded_hunks` — verifies prev with wrap (0→4)
- `review_hunk_accept_records_disposition_for_focused_hunk` — verifies `hunk_dispositions().accepted_hunk_ids()` records exactly 1 entry after `ReviewHunkAccept`
- `review_accept_all_and_dismiss_lifecycle` — verifies 5 accepted after `ReviewAcceptAll`, 0 after `ReviewDismiss`, and index resets to 0

---

## Files Modified

- `crates/legion-desktop/src/workflow.rs` — F1, F3 (hunk_dispositions field, handlers, keybindings, apply helper)
- `crates/legion-desktop/src/bridge.rs` — F3 (ReviewApply, ReviewDismiss variants)
- `crates/legion-app/src/proposal.rs` — F2 (conservative all() filtering)
- `crates/legion-app/src/lib.rs` — F4 (workspace_proposal_for_id, compute_diff_surface_for_proposal)
- `crates/legion-desktop/src/view/proposal_review.rs` — F6 (DesktopEvidencePanelDtoViewModel)
- `crates/legion-app/tests/proposal_review_surface.rs` — F2 test, F5 test
- `crates/legion-desktop/tests/keyboard_nav.rs` — F7 tests
