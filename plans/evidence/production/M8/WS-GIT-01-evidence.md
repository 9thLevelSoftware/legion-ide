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

## Merged-tree standing-gate run (2026-07-05, branch m8/git-residual)

Context: main merged (LSP substrate #34, terminal #36, containment #37, CI
fixes #35/#38); working directory C:/Users/dasbl/RustroverProjects/
legion-ide-git; Windows 11; builds -j 4. Merge resolutions: single
workspace-form sha2 in legion-app (resolving the WS-LANG-01 direct-version
duplicate; hex workspace-lift flagged for hygiene), legion-ui export union,
ledger row union (main PR-LANG-001 + branch PR-LANG-002 enrichments).

| Gate | Result |
| --- | --- |
| cargo fmt --all --check | PASS |
| xtask check-deps / docs-hygiene / claim-audit / no-egui-textedit / verify-kanban-backlog | PASS |
| xtask release-pipeline --dry-run + verify-release-pipeline | PASS |
| cargo check --workspace --all-targets | PASS |
| cargo test --workspace --all-targets --no-fail-fast | PASS (197 test binaries, 0 failures) |
| cargo clippy --workspace --all-targets -- -D warnings | PASS (after machine-applied map_or/borrow fixes; git_nav/local_history/commit_validation suites re-run green) |
| xtask perf-harness + verify-perf-harness | PASS |
| cargo deny check | PASS |
| xtask rust-analyzer-smoke | PASS (real rust-analyzer 1.95.0) |
