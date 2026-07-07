# PKT-INLINE Evidence — P4.F4 Inline Edit Loop

Branch: `m9/inline-edit`  
Date: 2026-07-06

## Commits

| Task | Hash | Message |
|------|------|---------|
| T1 | c03e593 | feat: inline edit instruction with streaming diff overlay (P4.F4.T1) |
| T2 | a999fbb | feat: per-hunk accept/reject through proposal pipeline (P4.F4.T2) |
| T3 | be41c79 | feat: undo integrates editor history with checkpoint ledger (P4.F4.T3) |

## Task Coverage

| Task | Spec Function | Implementation | Test |
|------|---------------|----------------|------|
| T1 | `InlineEditInstruction` (protocol type) | `legion-protocol/src/lib.rs` | `inline_edit_from_instruction_creates_overlay` |
| T1 | `InlineEditDiffHunk` (protocol type) | `legion-protocol/src/lib.rs` | `streaming_chunks_accumulate_into_hunks` |
| T1 | `InlineEditOverlayViewModel` | `view/inline_edit.rs` | `inline_edit_from_instruction_creates_overlay` |
| T1 | `check_inline_edit_anchor_freshness()` | `view/inline_edit.rs` | `stale_anchor_prevents_application`, `fresh_anchor_allows_application` |
| T1 | `accumulate_inline_edit_chunks()` | `view/inline_edit.rs` | `streaming_chunks_accumulate_into_hunks`, `incomplete_hunk_not_eligible_for_accept` |
| T2 | `set_inline_edit_hunk_disposition()` | `view/inline_edit.rs` | `accept_hunk_sets_disposition`, `reject_hunk_sets_disposition`, `streaming_hunk_rejects_disposition_change` |
| T2 | `inline_edit_to_workspace_proposal()` | `view/inline_edit.rs` | `inline_edit_to_proposal_includes_only_accepted_hunks` |
| T2 | `build_inline_edit_audit_record()` | `view/inline_edit.rs` | `inline_edit_apply_produces_audit_record` |
| T2 | `DesktopAction` bridge variants (4) | `bridge.rs` | `inline_edit_bridge_actions_are_noop_stubs` |
| T3 | `InlineEditApplyResult` | `view/inline_edit.rs` | `apply_uses_single_undo_group`, `apply_creates_checkpoint_with_matching_proposal_id` |
| T3 | `apply_inline_edit_with_undo_group()` | `view/inline_edit.rs` | `checkpoint_targets_cover_all_applied_hunks`, `undo_group_and_checkpoint_are_correlated` |

## Test Results

```
running 15 tests
test accept_hunk_sets_disposition ... ok
test apply_creates_checkpoint_with_matching_proposal_id ... ok
test apply_uses_single_undo_group ... ok
test checkpoint_targets_cover_all_applied_hunks ... ok
test fresh_anchor_allows_application ... ok
test incomplete_hunk_not_eligible_for_accept ... ok
test inline_edit_apply_produces_audit_record ... ok
test inline_edit_bridge_actions_are_noop_stubs ... ok
test inline_edit_from_instruction_creates_overlay ... ok
test inline_edit_to_proposal_includes_only_accepted_hunks ... ok
test reject_hunk_sets_disposition ... ok
test stale_anchor_prevents_application ... ok
test streaming_chunks_accumulate_into_hunks ... ok
test streaming_hunk_rejects_disposition_change ... ok
test undo_group_and_checkpoint_are_correlated ... ok

test result: ok. 15 passed; 0 failed
```

Full workspace: all crates pass (`cargo test --workspace` — zero failures).

## Architecture Decisions

**Stale-anchor detection** uses `SnapshotId` + `BufferVersion` equality. Both must match; if either changes the anchor is stale and hunks must not be applied. This avoids misaligned line-range substitutions.

**Streaming format** is line-oriented with `---SEP---` and `---END---` sentinels. Chunks without `---END---` produce `complete: false` hunks, which are ineligible for disposition. No binary/structured format is needed since the streaming source is an AI text response.

**`FileId(buffer_id.0)` mapping** — desktop layer only has `BufferId` (u128 wrapper); the proposal payload uses `FileId` (also u128 wrapper). The desktop emits `FileId(buffer_id.0)` and the app layer is responsible for canonical mapping. This is consistent with how other desktop→app bridges handle the mismatch.

**`uuid` as production dependency** — `apply_inline_edit_with_undo_group` generates `Uuid::now_v7()` for the undo group, and `build_inline_edit_audit_record` uses `Uuid::nil()` as a deterministic sentinel. Both are production paths, so `uuid` was promoted from `[dev-dependencies]` to `[dependencies]` in `legion-desktop/Cargo.toml`.

**`checkpoint` field on `InlineEditApplyResult`** — the brief specifies `proposal_id`, `undo_group_id`, `checkpoint_id`, `applied_hunk_count`, and `audit_record`. An additional `checkpoint: DurableCheckpoint` field is included to allow tests (and callers) to directly persist the checkpoint via `CheckpointStore::save_checkpoint` without a round-trip through a store reference. The field is cheap to clone and makes the checkpoint observable at the call site.

**`set_inline_edit_hunk_disposition` in T1 commit** — the function is specified in T2 but is required by the T1 test `incomplete_hunk_not_eligible_for_accept`. It was included in the T1 commit alongside `InlineEditError` to keep tests self-contained per-commit.

**Bridge variants stub as `Noop`** — the 4 new `DesktopAction` variants (`AcceptInlineEditHunk`, `RejectInlineEditHunk`, `ApplyInlineEdit`, `DismissInlineEdit`) translate to `DesktopBridgeOutput::Noop`. The spec notes they are intercepted by `DesktopRuntime` before reaching the bridge; the Noop translation is the correct bridge-layer behavior.

## Isolation Table

| Concern | Location | Notes |
|---------|----------|-------|
| No direct buffer mutation | `view/inline_edit.rs` | All functions return view models or proposal payloads; zero `Buffer::edit` calls |
| Proposal pipeline | `inline_edit_to_workspace_proposal()` | Returns `WorkspaceProposal`; app layer calls `apply_workspace_proposal` |
| Fingerprint/version guards | `check_inline_edit_anchor_freshness()` | Called before applying; stale returns early |
| No secrets on disk | entire crate | No keys, tokens, or credentials |
| Undo group UUID | `apply_inline_edit_with_undo_group()` | `Uuid::now_v7()` — time-ordered, no PII |
| Checkpoint targets | `apply_inline_edit_with_undo_group()` | One `CheckpointTarget` per accepted hunk with pre-mutation `original_text` |
