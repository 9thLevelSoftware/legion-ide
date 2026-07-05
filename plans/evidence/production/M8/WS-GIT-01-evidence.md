# WS-GIT-01 Evidence — PKT-GIT M8 Milestone

**Branch:** `m8/git-residual`
**Commit:** (see git log)
**Date:** 2026-07-04

## Scope

PKT-GIT implements GIT.03, GIT.06, GIT.07 (subset), GIT.09, GIT.10, and GIT.12 from the M8 residual queue.

## Tasks Completed

| Task | Description | Tests |
|------|-------------|-------|
| GIT.03 | Diff review keyboard navigation — next/prev hunk, next/prev file typed intents; selection state in app layer | `cargo test -p legion-app --test git_nav_workflow` (5 pass) |
| GIT.06 | Commit message/author validation — non-empty summary hard error; author name/email from git config; CC prefix advisory warning | `cargo test -p legion-app --test commit_validation_workflow` (8 pass) |
| GIT.07 (subset) | "Git: New Worktree" palette command routing through `create_git_worktree` project function | `cargo test -p legion-app --test worktree_creation_workflow` (3 pass) |
| GIT.09 | Local history snapshots on successful save; bounded retention (50 entries / 50 MiB); palette command; restore via proposal route | `cargo test -p legion-app --test local_history_workflow` (4 pass) |
| GIT.10 | jj non-goal declaration | `plans/product-readiness-ledger.md` row PR-LANG-003 |
| GIT.12 | Worktree state evidence export — metadata-only TOML to `.legion/evidence/` | `cargo test -p legion-app --test worktree_evidence_workflow` (3 pass) |

## Key Constraints Preserved

- No direct writes: all restore/mutation paths go through `proposal_coordinator.build_save_proposal` → `workspace.save_file_with_proposal` with fingerprints, versions, generation, correlation/causality IDs.
- Metadata-only audit records: `LocalHistoryMetadataStore` stores only identity metadata; content blobs stay on disk in `.legion/local-history/`.
- Commit operations never touch the network.
- legion-ui / legion-desktop projection-only: navigation state (`focused_git_hunk_id`) lives in `AppComposition`, not in the desktop layer.
- No weakening of existing tests, policies, or redaction.

## Test Run Evidence

```
cargo test -j 4 -p legion-app --test local_history_workflow
    test local_history_records_entry_after_save ... ok
    test local_history_records_multiple_saves ... ok
    test local_history_retention_cap_is_enforced ... ok
    test restore_from_local_history_uses_proposal_route ... ok
    test result: ok. 4 passed; 0 failed

cargo test -j 4 -p legion-app --test git_nav_workflow
    test result: ok. 5 passed; 0 failed

cargo test -j 4 -p legion-app --test commit_validation_workflow
    test result: ok. 8 passed; 0 failed

cargo test -j 4 -p legion-app --test worktree_creation_workflow
    test result: ok. 3 passed; 0 failed

cargo test -j 4 -p legion-app --test worktree_evidence_workflow
    test result: ok. 3 passed; 0 failed

cargo run -p xtask -- claim-audit     → claim audit passed
cargo run -p xtask -- docs-hygiene    → documentation hygiene checks passed
cargo run -p xtask -- verify-kanban-backlog → kanban backlog ok
```

## Readiness Gates Informed

- PR-LANG-002: local history, commit validation, and worktree navigation add substrate evidence for GIT SCM surface.
- PR-LANG-003: jj non-goal declared (new row).
