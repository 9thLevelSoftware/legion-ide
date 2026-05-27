# Plan 03-04 Result: Save-All Conflict And Dirty-Close Hardening

Status: Complete

## Files Changed

- `crates/devil-app/src/lib.rs`
- `crates/devil-app/tests/daily_editing_contracts.rs`
- `crates/devil-desktop/src/bridge.rs`
- `crates/devil-desktop/src/view.rs`
- `crates/devil-desktop/src/workflow.rs`
- `crates/devil-desktop/tests/save_all_conflict.rs`
- `crates/devil-desktop/tests/desktop_workflow.rs`
- `crates/devil-ui/src/ui.rs`
- `.planning/phases/03-daily-editing-mvp/WAVE-CHECKLIST.md`
- `.planning/phases/03-daily-editing-mvp/03-04-SUMMARY.md`
- `.planning/ROADMAP.md`
- `.planning/STATE.md`

## Implementation Summary

- Expanded app save-all outcomes with aggregate status, per-buffer status, file id/path, proposal rejection metadata, and final dirty state.
- Kept save-all sequential and proposal-mediated through `SaveWorkflowService`.
- Fixed save-all sequencing by refreshing app-owned workspace generation metadata after each successful save, so later buffers do not fail only because an earlier save advanced global workspace state.
- Recorded missing buffer metadata as an explicit rejected save-all item instead of silently skipping it.
- Added desktop save-all warning/detail status rows for rejected/stale/conflict/denied outcomes.
- Added dirty-close prompt save/cancel actions; cancellation preserves dirty text and tabs, and no discard action is exposed.
- Preserved the Phase 2 external overwrite rejection behavior and added a source-boundary assertion for desktop save routing.
- Derived `SearchScopeProjection::Default` to keep the affected package clippy gate clean.

## Verification

| Command | Result |
| --- | --- |
| `rg -q "SaveAll" crates/devil-app/src/lib.rs` | Passed |
| `rg -q "SaveRejected" crates/devil-desktop/src/workflow.rs` | Passed |
| `rg -q "conflict" crates/devil-desktop/src/view.rs` | Passed |
| `rg -q "dispatch_ui_intent" crates/devil-desktop/src/workflow.rs` | Passed |
| `cargo fmt --all --check` | Passed |
| `cargo test -p devil-app daily_editing_save_all -- --nocapture` | Passed; 4 tests passed across app unit/integration filters |
| `cargo test -p devil-desktop save_all_conflict -- --nocapture` | Passed; 5 tests passed |
| `cargo test -p devil-desktop desktop_workflow_external_overwrite_save_rejects_and_preserves_dirty_projection -- --exact` | Passed; 1 test passed |
| `cargo test -p devil-desktop daily_editing_controls -- --nocapture` | Passed; 4 tests passed |
| `cargo test -p devil-desktop intent_bridge -- --nocapture` | Passed; 10 tests passed |
| `cargo test -p devil-desktop desktop_workflow -- --nocapture` | Passed; 4 tests passed |
| `cargo check -p devil-app --all-targets` | Passed |
| `cargo check -p devil-desktop --all-targets` | Passed |
| `cargo clippy -p devil-app -p devil-desktop --all-targets -- -D warnings` | Passed |
| `git diff --check` | Passed with CRLF normalization warnings only |

## Decisions

- Save-all updates only app-held workspace generation metadata after successful saves; file fingerprints/content versions remain file-specific.
- Rejected save-all items keep their dirty editor text and expose proposal response metadata for desktop warning rows.
- Dirty-close cancel is an app-owned state transition. Dirty-close save routes through existing app save authority and clears the prompt only on successful save.
- Discard remains intentionally unavailable because this plan did not add a verified app-owned discard contract.

## Issues

- None.
