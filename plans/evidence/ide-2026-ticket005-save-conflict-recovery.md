# Ticket05: Save-conflict preservation and recovery

Date: 2026-10-09. Review candidate, not a ticket closure or native qualification.

Worktree: `D:/legion-ide-2026-workers/wave1-save-conflicts`.
Base: `e671d8c62e012646e5632521ae78c6970a987f9c`. No commit, merge, push,
native launch, or remote/provider call was performed. The checkout was clean
before this work. All Cargo commands used this checkout's target directory and
`-j 2`; D: had 1,301,295,108,096 bytes free before the fresh build.

## Scope and acceptance boundary

[Ticket05](../../.scratch/ide-2026-planning/issues/05-preserve-edits-across-save-conflicts.md)
requires open/edit/external overwrite/refused save/work preservation/recovery,
with the existing proposal and version guards. The approved
[specification](../../.scratch/ide-2026-planning/spec.md), A02 at line 388,
names **app workflow plus storage reopen** as its primary boundary. This receipt
provides that evidence for ticket05's external-overwrite scenario. It does not
qualify all of A02's dirty-dependency or migration scenarios, A01's native pilot,
or ticket04's native focus/input gate.

Tests use approved public `AppComposition` operations, editor read-only queries,
the public `HotExitStore`, fresh app instances, and actual isolated filesystem
fixtures. No fabricated provider, IPC, native input, or acceptance oracle is used.
Literal UTF-8 bytes, including Unicode and CRLF, are compared against disk and
independently reopened editor text.

## Demonstrated gap and repair

Reopening a retained buffer read fresh disk metadata but retained the old editor
text. `ActiveDocumentController::bind_opened_file` then replaced the baseline
fingerprint/content version with the external version. That silently changed
what a later ordinary save was authorized to replace.

The behavioral red for
`workspace_vfs_integration_reopening_conflicted_file_does_not_authorize_overwrite`
failed with the dirty text still `seed!`: expected the original size=4 fingerprint,
but the active metadata now held the external size=8 fingerprint. The failed
public app assertion was:

```text
reopening a retained buffer must not silently acknowledge unseen disk changes
test result: FAILED. 0 passed; 1 failed; 63 filtered out
```

The 11-line repair reactivates existing tab metadata when the editor buffer is
retained. Its baseline changes through the existing acknowledged save path,
not by ordinary reactivation. This applies to dirty and clean retained buffers:
a clean buffer's old text also must not authorize overwriting an unseen external
edit. Closing a clean tab and opening it afresh still uses freshly read metadata.
Ordinary reopen remains activation, not a newly added reload operation.

No `SaveWorkflowService`, workspace mutation, editor, desktop adapter, provider,
or storage implementation change was necessary. The affected source hunk is
[the app document binding](../../crates/legion-app/src/lib.rs); tests are in
[workspace VFS integration](../../crates/legion-app/tests/workspace_vfs_integration.rs).

## Recovery evidence

The new end-to-end test:

1. Opens `seed\r\n`, edits to `seed α😀\r\n`, externally overwrites the original
   with `external\r\nbytes\n`, and observes a refused save with dirty text intact.
2. Requests close, receives the dirty-close prompt, and cancels without losing
   the buffer or changing disk.
3. Captures session metadata and dirty snapshots, persists the body through
   `HotExitStore`, and causes an actual storage failure by using a regular file
   as the store directory. The failure preserves live dirty text and the original
   disk bytes; the prior successful store remains reopenable.
4. Drops the app, loads the actual store in a new app, restores exact dirty text,
   and reactivates the original. The original baseline remains intact, so another
   ordinary save is refused and the external original remains untouched.
5. Explicitly uses the existing new-file/edit/save workflow to save a separate
   recovery copy. This save emits created/validated/previewed/applied/audit
   lifecycle events with nonzero identifiers. The original dirty buffer remains
   unsaved and intact; exact original and copy bytes are checked separately.
6. Drops that app and opens the copy in another fresh app. Exact recovered text
   is present and clean.

This is recovery through existing public app operations, not a newly added
native Save As, reload, merge, force-overwrite, or keep-both button. The selected
copy destination is absent in the fixture; generalized recovery-destination
selection UX is not claimed.

The first recovery-test attempt reached and passed the preservation, store
reopen, and disk/copy assertions, then failed on an incorrect test expectation
for `workspace.file_saved`. Source and existing save contracts emit
`proposal.applied` and `proposal.audit_recorded`. Correcting that assertion alone
made the test pass. This is a **corrected test-assumption red**, not a second
product behavioral red or product repair.

## Exact checks

Commands ran from the worktree root. Successful checks were not repeated.

Behavioral red, then green after the supported binding fix (same command twice):

```powershell
cargo test -j 2 --target-dir D:/legion-ide-2026-workers/wave1-save-conflicts/target -p legion-app --test workspace_vfs_integration workspace_vfs_integration_reopening_conflicted_file_does_not_authorize_overwrite -- --exact
```

Results: red 0 passed / 1 failed; green 1 passed / 0 failed.

New full recovery test, then affected retry after correcting the event assumption:

```powershell
cargo test -j 2 --target-dir D:/legion-ide-2026-workers/wave1-save-conflicts/target -p legion-app --test workspace_vfs_integration workspace_vfs_integration_conflict_recovery_survives_store_reopen_and_saves_separate_copy -- --exact
```

Results: assumption failure 0 passed / 1 failed; corrected run 1 passed / 0 failed.

New clean-buffer regression plus eight affected save regressions, once:

```powershell
cargo test -j 2 --target-dir D:/legion-ide-2026-workers/wave1-save-conflicts/target -p legion-app --test workspace_vfs_integration -- --exact workspace_vfs_integration_reopening_clean_retained_text_does_not_acknowledge_external_edit workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict workspace_vfs_integration_untrusted_save_is_denied_without_disk_mutation workspace_vfs_integration_oversized_save_is_rejected_and_preserves_dirty_text workspace_vfs_integration_failed_save_preserves_pending_dirty_text workspace_vfs_integration_open_edit_save_use_engine_and_workspace_ids workspace_vfs_integration_stale_registered_save_preserves_dirty_buffer_and_disk workspace_vfs_integration_conflicted_registered_save_preserves_dirty_buffer_and_disk workspace_vfs_integration_registered_save_audit_failure_fails_closed_and_rolls_back
```

Result: 9 passed / 0 failed. Includes stale buffer versions, disk conflicts,
untrusted and oversized denial, deleted original, repeated successful save, and
registered-save audit failure/rollback.

Affected save-all, dirty-close, and existing hot-exit contracts, once:

```powershell
cargo test -j 2 --target-dir D:/legion-ide-2026-workers/wave1-save-conflicts/target -p legion-app --test daily_editing_contracts -- --exact daily_editing_contracts_save_all_preserves_rejected_dirty_buffers daily_editing_contracts_close_dirty_requires_prompt daily_editing_contracts_hot_exit_restores_dirty_body_without_writing_disk daily_editing_contracts_hot_exit_conflicts_when_disk_fingerprint_changed
```

Result: 4 passed / 0 failed. Total: **15 unique focused tests passed**.

Changed Rust formatting applied, then checked once:

```powershell
rustfmt --edition 2024 --config skip_children=true crates/legion-app/src/lib.rs crates/legion-app/tests/workspace_vfs_integration.rs
rustfmt --check --edition 2024 --config skip_children=true crates/legion-app/src/lib.rs crates/legion-app/tests/workspace_vfs_integration.rs
```

Both exited 0. Default app/test compilation completed as part of the integration
runs. No full workspace suite, offline feature build, native run, or release
qualification was performed.

Final gate commands:

```powershell
cargo run -j 2 --target-dir D:/legion-ide-2026-workers/wave1-save-conflicts/target -p xtask -- check-deps
cargo run -j 2 --target-dir D:/legion-ide-2026-workers/wave1-save-conflicts/target -p xtask -- docs-hygiene
git diff --check
```

`check-deps` and `docs-hygiene` both exited 0. The whitespace guard runs after
finalizing this receipt; its result is included in the coordinator handoff.

## Authority and limits

This repair preserves the existing app/editor/workspace split and existing save
preconditions. It introduces no dependency, new runtime surface, direct write,
automatic conflict overwrite, UI-owned editor session, or provider dispatch.
[ADR-0003](../adrs/ADR-0003-editor-core-text-model.md) and
[ADR-0015](../adrs/ADR-0015-streaming-text-viewport.md) remain the applicable
boundaries; the 5 MiB snapshot-cache budget and existing save materialization and
storage APIs are unchanged. The bounded source hunk does not allocate or copy
source text.

Review remains independent. Root/Pauli own canonical aggregation, ticket closure,
and any commit. No shared status/index/map counts changed. Native recovery UI and
the broader A02 scenarios remain separate evidence work, not blockers inferred
for this approved ticket05 app/storage slice.
