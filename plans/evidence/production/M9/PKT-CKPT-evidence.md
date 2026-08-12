# PKT-CKPT Evidence — M9 Checkpoints and Rollback UX

Branch: `m9/checkpoints`

## Task Coverage

| Task | ID | Status | Test |
|------|----|--------|------|
| Durable checkpoint store | P3.F3.T1 | DONE | `checkpoint_auto_created_on_file_mutation_proposal_apply` |
| Checkpoint timeline + restore | P3.F3.T2 | DONE | `checkpoint_timeline_and_restore_middle` |
| Non-conflicting manual edit preservation | P3.F3.T3 | DONE | `checkpoint_restore_scoped_to_targets_preserves_manual_edits` |
| Checkpoint/restore audit ledger | P3.F3.T4 | DONE | `checkpoint_audit_records_created_and_restored` |

## Test Results

```
cargo test -p legion-storage --lib -- checkpoint
running 6 tests
test checkpoint::tests::audit_save_and_query ... ok
test checkpoint::tests::list_ordering_newest_first ... ok
test checkpoint::tests::mark_unavailable ... ok
test checkpoint::tests::save_load_roundtrip ... ok
test checkpoint::tests::delete_removes_checkpoint ... ok
test checkpoint::tests::save_load_roundtrip_with_disk ... ok
test result: ok. 6 passed; 0 failed

cargo test -p legion-app --test checkpoint_restore
running 6 tests
test checkpoint_audit_records_created_and_restored ... ok
test checkpoint_auto_created_on_file_mutation_proposal_apply ... ok
test checkpoint_timeline_and_restore_middle ... ok
test checkpoint_restore_preserves_non_conflicting_manual_edits_after_apply ... ok
test checkpoint_restore_scoped_to_targets_preserves_manual_edits ... ok
test restore_failure_leaves_checkpoint_available_and_no_audit ... ok
test result: ok. 6 passed; 0 failed
```

## Architecture Decisions

### legion-storage constraint compliance
- `CheckpointStore` in `crates/legion-storage/src/checkpoint.rs` uses `CanonicalPath` and `ProposalId` from `legion-protocol` — no dependency on `legion-app` or `legion-project`.
- Atomic rename writes to `.legion/checkpoints/<id>.json` follow the `local_history.rs` blob pattern.
- Audit records written to `.legion/audit/<id>.json` with the same atomic rename pattern.

### Generation refresh (Task 2 root cause)
The workspace actor at `legion-project` increments `state.generation` on every successful file mutation. `AppComposition::open_workspace` re-uses the existing `state.generation` when the same workspace is already open (`existing.workspace_id == workspace_id`). Tests use this to refresh the generation between sequential proposals.

### Desktop wiring
- `DesktopAction::RestoreCheckpoint { checkpoint_id }` added to `bridge.rs`.
- Handler added to `DesktopWorkflowRuntime::handle_action` in `workflow.rs` (direct `self.app.restore_checkpoint` call, not routed through `CommandDispatchIntent`).
- Bridge `translate` has unreachable exhaustiveness arm returning `Noop`.

## Dependency Check

```
cargo run -p xtask -- check-deps
dependency policy checks passed
```

## Fix Rounds

### Round 1 (subagent)
- C1 (Critical): Raw `std::fs` in restore_checkpoint replaced with `WorkspaceRestoreFileOp` enum through workspace layer
- C2 (Critical): `let _ =` swallowing errors replaced with `?` propagation — audit/mark_unavailable only on success
- I1: `durable_checkpoint_timeline_rows` field added to `DesktopProjectionViewState` and wired in `projection_view_state()`
- I2: Pre-restore snapshot via `build_pre_restore_snapshot` for displaced manual edits
- I3: Alt+Z keybinding for most-recent-available checkpoint restore
- I4: TODO placeholder removed, `CommandDispatchIntent` routing deferred to integration packet

### Round 2 (direct)
- C2 failure-path test: `restore_failure_leaves_checkpoint_available_and_no_audit` added
- I1 projection wiring verified compiling
- Clippy collapsible-if and dead-code warnings resolved

## Format Check

```
cargo fmt --all --check
(clean)
```
