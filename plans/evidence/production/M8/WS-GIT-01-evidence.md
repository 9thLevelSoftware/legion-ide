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

## Roadmap 1.7 — Git product surface P2.F5.T1-T4 (2026-08-16)

Covers backlog tasks P2.F5.T1 (gutter diff/blame), P2.F5.T2 (status panel,
hunk staging, commit, push/fetch/pull), P2.F5.T3 (branch/worktree manager and
agent worktree visibility), and P2.F5.T4 (policy-visible network/auth).

### What the acceptance required

| Task | Acceptance | Stop condition |
| --- | --- | --- |
| P2.F5.T1 | GP-1: open Legion repo, edit, see diff, stage hunk, commit, optionally push | Diff/blame data must not be read once and never refreshed |
| P2.F5.T2 | Hunk staging and commit work end-to-end through the UI | Must not bypass the workspace path policy for push/fetch/pull |
| P2.F5.T3 | Agent worktrees visible in the SCM surface; manual worktrees user-managed | Agent worktrees must not be hidden from the user |
| P2.F5.T4 | Every network/auth operation shows its policy decision to the user | No network operation allowed without an audit row |

### What was already true before this change

Most of this feature already existed and was working. Recorded here so the
delta is not overstated:

- **Gutter diff and inline blame were already implemented and painted.**
  `git_hunk_marker_for_line` and `git_inline_blame_label` in
  `crates/legion-desktop/src/view.rs` render a per-line gutter marker and an
  inline blame label inside `render_code_lines`; prev/next hunk navigation
  buttons already existed. `crates/legion-desktop/src/view/code_canvas_painter.rs`
  is only a dyn-safe seam and holds no git logic, so the file named in the
  backlog was never where this lived.
- **A refresh path already existed.** `AppComposition::refresh_git_projection`
  re-runs `collect_git_snapshot` and is called by the `RefreshGit` intent and
  after every mutating git command. The data was never read-once.
- **Hunk staging, commit, and push already worked end-to-end**, with tests:
  `desktop_git_workflow_projects_diff_blame_graph_and_hunk_actions` stages a
  hunk through the desktop runtime and verifies `git diff --cached`;
  golden-path-1 step s6 drives edit, save, refresh, stage, commit through app
  authority; `desktop_git_workflow_pushes_current_branch_to_origin` pushes to a
  real bare remote.
- **Branch and worktree management already existed**: switch/create/delete
  branch, create/remove/prune worktree, all with app-layer tests, plus a
  workspace-trust gate on worktree creation.
- **The agent/manual worktree distinction already existed** in the projection
  (`ProjectGitWorktreeKind`, `GitWorktreeKindProjection`) and was already
  rendered as `kind={:?}` in the SCM rows.
- **The policy substrate already existed**: `CommandTaxonomy::classify` already
  classified `git push`/`fetch`/`pull` as `CommandClass::Network`, and
  `NetworkPolicy` and `SecurityDecision` were already in place.

### What was missing, and what changed

Three genuine gaps were found by reading the code, not assumed:

**1. Push reached the network with no policy decision and no audit row (T4).**
The `PushGitRemote` arm called `push_git_remote` directly. The sibling
`CreateGitWorktree` arm had a workspace-trust gate; push had nothing. Added:

- `decide_git_remote_operation` and its supporting types in
  `crates/legion-security/src/policy.rs`, layered on the existing taxonomy and
  `NetworkPolicy`. Every evaluation returns a `GitRemoteDecision` carrying an
  audit row on both the allow and the deny path, so an allowed operation cannot
  skip the record.
- `classify_git_remote_url` separates remotes that can egress from those that
  cannot. Filesystem paths and `file://` are exempt from allowlist/air-gap
  checks because they never leave the machine; scp-style `host:path` must not be
  mistaken for a Windows drive path.
- `crates/legion-app/src/git_policy.rs`, a self-contained policy module in the
  style of `terminal_policy.rs`, converting a decision into a projection row and
  bounding the retained trail.
- `GitRemotePolicyProjection` and `GitProjection::remote_policy_audit` in
  `crates/legion-ui/src/ui.rs`; rows are re-injected after each
  `refresh_git_projection` rebuild (the same treatment `focused_hunk_id` gets),
  otherwise a refresh would erase a denial the user had not yet read.
- Rendering in `git_rows`, with denied rows prefixed distinctly so a refusal
  cannot read as success.

A denial returns `AppCommandOutcome::GitUpdated` with the reason rather than an
error, matching the existing commit-validation precedent.

**2. Fetch and pull were unreachable dead code (T2).** `fetch_git_remote` and
`pull_git_remote` had existed in `crates/legion-project/src/lib.rs` with zero
callers anywhere in the workspace: no intent, no request, no desktop action, no
palette entry, no test. Wired end to end and gated by the same policy path.
`git-push` had an intent mapping but no palette spec, so it too was unreachable
from the palette; specs were added for all three.

`git_remote_configured_url` was added because the snapshot's `remote_url` only
ever describes `origin`. Authorizing an operation against one remote using a
different remote's target would have been a policy bypass.

**3. Agent worktree classification was untested and separator-sensitive (T3).**
`git_worktree_kind` matched only `target/delegated-tasks/task-` with forward
slashes, and no test anywhere asserted that an agent worktree is classified as
`Agent`. Made public as `git_worktree_kind_for_path`, separators normalized, and
covered with the backslash case (which fails without the change) plus manual
negatives.

### Tests

New tests, with counts as run:

| Suite | Tests | New here |
| --- | --- | --- |
| `cargo test -p legion-security --test git_remote_policy` | 10 passed | all 10 |
| `cargo test -p legion-app --test git_remote_policy_workflow` | 6 passed | all 6 |
| `cargo test -p legion-app --lib git_policy` | 3 passed | all 3 |
| `cargo test -p legion-project --test git_workflow` | 26 passed | 2 |
| `cargo test -p legion-desktop --test git_workflow` | 8 passed | 3 |
| `cargo test -p legion-app --test palette` | 18 passed | 0 (allowlist updated) |

New test names:

- legion-security `git_remote_policy`:
  `remote_urls_are_classified_into_local_and_host_targets`,
  `air_gap_denies_a_non_loopback_push_and_says_so_in_the_audit_row`,
  `allowlisted_host_is_permitted_and_still_emits_an_audit_row`,
  `a_host_outside_the_allowlist_is_denied_even_without_air_gap`,
  `blocklisted_host_is_denied_even_when_it_is_also_allowlisted`,
  `untrusted_workspace_denies_every_remote_operation`,
  `filesystem_remotes_are_allowed_because_they_cannot_egress`,
  `a_remote_without_a_configured_url_is_denied_rather_than_assumed_local`,
  `reclassifying_git_push_away_from_network_denies_instead_of_bypassing`,
  `only_push_publishes_local_content`
- legion-app `git_remote_policy_workflow`:
  `push_to_a_local_remote_is_allowed_and_records_an_allow_row`,
  `push_from_an_untrusted_workspace_is_denied_and_never_reaches_the_remote`,
  `push_to_a_network_remote_is_denied_by_the_air_gapped_default_policy`,
  `fetch_and_pull_are_reachable_as_intents_and_are_policy_gated`,
  `fetch_from_a_local_remote_is_allowed_and_runs`,
  `the_audit_trail_survives_a_projection_refresh`
- legion-app `git_policy` unit tests:
  `a_denied_operation_still_produces_a_projection_row`,
  `an_allowed_operation_also_produces_a_projection_row`,
  `audit_rows_are_bounded_and_drop_the_oldest_first`
- legion-project `git_workflow`:
  `worktree_kind_distinguishes_agent_sandboxes_from_manual_worktrees`,
  `git_snapshot_projects_agent_worktrees_as_agent_kind`
- legion-desktop `git_workflow`:
  `desktop_git_refresh_reflects_edits_made_after_the_first_refresh`,
  `desktop_git_rows_renders_remote_policy_verdicts`,
  `desktop_bridge_translates_fetch_and_pull_actions`

Two of these are deliberately constructed to fail without the change rather
than to restate it:

- `push_from_an_untrusted_workspace_is_denied_and_never_reaches_the_remote`
  points `origin` at a real local bare repository that the trusted test in the
  same file successfully pushes to, then asserts the bare repository is still
  empty. Without the gate the push would land, so the test distinguishes "policy
  stopped it" from "the network was unavailable".
- `worktree_kind_distinguishes_agent_sandboxes_from_manual_worktrees` asserts
  the backslash-spelled path classifies as `Agent`, which the previous
  forward-slash-only match returned `Manual` for.

`desktop_git_refresh_reflects_edits_made_after_the_first_refresh` addresses the
T1 stop condition directly: a single refresh cannot distinguish a live
projection from a cached one, so it drives two refreshes across an edit made
behind the projection's back and asserts the changed-file and hunk sets moved.

### Not claimed

Recorded so this section is not read as broader than it is:

- **The git projection still does not refresh itself.** There is no file
  watcher, no on-save hook, and no timer. Gutter markers and blame reflect the
  last explicit git command until the user refreshes again. The stop condition
  for T1 is about the data being re-readable, which it is and which is now
  tested, but automatic freshness was not built. `collect_git_snapshot` also
  runs synchronously on the dispatch path, so adding a refresh to every save
  would put a git subprocess on a latency-sensitive path; that tradeoff was not
  taken on without a perf measurement.
- **Blame does not follow the active file on its own.** `blame_lines` is
  computed only for the file that was active at collection time, and
  `refresh_git_projection` is not called from file open or tab switch, so
  switching files leaves the previous file's blame in the projection until the
  next refresh. This was found during assessment and is not fixed here.
- **No credential or auth layer was built.** Git subprocesses inherit SSH agent
  and credential-helper configuration from the environment, as documented on
  `push_git_remote`. The "auth" half of the T4 acceptance is satisfied only in
  the sense that the operations that use credentials now show a policy decision.
- **A related hang was found and not fixed:** `git_stdout` waits on the child
  with `Command::output()` and no timeout, and nothing sets
  `GIT_TERMINAL_PROMPT=0`. A remote that prompts for a password can block the
  caller indefinitely, and on the desktop that caller is the UI dispatch path.
  A fix was deliberately not attempted here because it could not be verified
  against a real authenticated remote in this environment, and a plausible fix
  (overriding `GIT_ASKPASS`) risks breaking users who rely on a credential
  helper. Recommended as its own task.
- **`GitInspectionBackend::Gix` is a no-op and its test is vacuous.**
  `git_status_entries_gix` and `git_blame_lines_gix` both delegate to the CLI
  implementations, so `git_snapshot_gix_backend_matches_cli_backend` compares
  each function against itself and can never fail. Three `gix` crates compile
  for nothing. Left alone as outside these four tasks.
- **`GitDiffStrategy::Syntactic` is a label, not a behavior.** It is a file-size
  check plus an extension allowlist; no AST diff exists.
- **No SCM panel widgets were added for staging.** The panel body is still
  `Vec<String>` rows with `.take(N)` caps, and hunk staging is reachable only
  through the command line, palette, and keyboard paths, not a per-hunk button.
  Range staging (as distinct from hunk staging) does not exist.
- **`crates/legion-desktop/src/view/scm_panel.rs` and
  `crates/legion-project/src/git.rs` do not exist and were never created.** Both
  are named in the backlog `files` lists; the real locations are
  `crates/legion-desktop/src/view.rs` and `crates/legion-project/src/lib.rs`.
  The backlog entries were corrected rather than the files invented.
- **No extraction from `crates/legion-app/src/lib.rs` was performed.** New logic
  went into the new `git_policy.rs` module so the file grew only by a dispatch
  method and call sites. A pure move of the git command arms was considered and
  rejected: they are arms of one large `match`, so relocating them is a
  behavioral refactor rather than a move, and that was outside these tasks.

### Backlog status

P2.F5.T1, T2, T3, and T4 are moved to `done`. Each has a passing test named
above that exercises its acceptance, and each stop condition is covered by a
test rather than by assertion. The `files` and `verification` fields were
corrected to real paths and to the commands that actually cover the work.

