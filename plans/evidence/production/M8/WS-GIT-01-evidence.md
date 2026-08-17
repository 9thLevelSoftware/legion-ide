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
| `cargo test -p legion-security --test git_remote_policy` | 15 passed | all 15 |
| `cargo test -p legion-app --test git_remote_policy_workflow` | 10 passed | all 10 |
| `cargo test -p legion-app --lib git_policy` | 3 passed | all 3 |
| `cargo test -p legion-project --test git_workflow` | 26 passed | 2 |
| `cargo test -p legion-desktop --test git_workflow` | 9 passed | 4 |
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

Consent-path tests (legion-security `git_remote_policy`):
`user_consent_permits_a_host_that_air_gap_would_otherwise_deny`,
`a_blocklisted_host_stays_denied_even_after_consent`,
`git_remote_consent_does_not_touch_the_general_network_allowlist`,
`consent_matching_is_case_insensitive_and_trimmed`,
`consent_does_not_override_workspace_trust`.

Consent-path tests (legion-app `git_remote_policy_workflow`):
`a_denied_push_succeeds_after_the_user_grants_consent_for_the_host`,
`revoking_consent_restores_the_denial`,
`consent_is_refused_in_an_untrusted_workspace`,
`consent_is_scoped_to_the_host_that_was_granted`. Desktop:
`desktop_bridge_grants_consent_for_the_denied_host`.

`a_denied_push_succeeds_after_the_user_grants_consent_for_the_host` proves the
whole loop end to end — denied, granted, push physically lands in the bare
repository, and the audit reads
`[("push", false), ("consent-grant", true), ("push", true)]`. It is hermetic:
`origin` is configured as `https://git.legion.test/...` while a
`url.<bare-path>.pushInsteadOf` entry redirects the transport to a local bare
repo. `git remote get-url` still reports the `https://` URL, which is the value
policy reads, so the remote classifies as a non-loopback host and is denied by
default — but a granted push has somewhere real to land, with no network
service involved. A plain `insteadOf` would not work here: it also rewrites what
`get-url` reports, and the remote would classify as a local path and skip the
host checks entirely.

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
  computed only for the file active at collection time, and
  `refresh_git_projection` is not called from file open or tab switch, so
  switching files leaves the previous file's blame rows in the projection until
  the next refresh. An earlier draft of this section called that a correctness
  bug; on re-reading the renderer that was overstated and is corrected here.
  `git_inline_blame_label` matches on `line.path == relative_path`, so stale rows
  from another file are filtered out and the gutter shows *no* blame rather than
  another file's blame. It is a freshness gap, not misattribution. The fix would
  put a synchronous `collect_git_snapshot` on every tab switch, which is a
  latency tradeoff that wants a measurement rather than a guess, so it is
  recorded rather than taken.
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
- **`crates/legion-app/src/lib.rs` grew by a net 81 lines** (+95 / −14 against
  merge base `7609c775`), measured with `git diff --numstat`. The two dispatch
  methods were moved verbatim into `crates/legion-app/src/git_remote.rs` as an
  `impl AppComposition` continuation once the first draft measured +179, which
  would have exceeded the ~120-line chokepoint budget. Pure policy and
  projection logic lives in `git_policy.rs`; `lib.rs` retains only the request
  variants, intent routing, and two one-line call sites.
- **A pure move of the git *command arms* was still not performed.** They are
  arms of one large `match`, so relocating them is a behavioral refactor rather
  than a move, and that remains outside these tasks.

### Consent path: making the default deny survivable

Review found that the gate as first written was not fail-closed but fail-shut.
`NetworkPolicy::default()` is air-gapped with a `localhost`-only allowlist, and
nothing in the product could write that allowlist — the only broker mutator was
`pin_workspace_path_roots`. A push to any real forge was therefore denied
permanently, with no path to permit it. That is a missing feature wearing a
policy decision's clothes, and it is worse than the unaudited push it replaced,
because Phase 1's exit criterion is developing Legion in Legion and pushing is
part of that loop.

Added an explicit consent path rather than loosening the default:

- `NetworkPolicy::consented_git_remote_hosts` — deliberately separate from
  `allowlist`. The allowlist is operator configuration covering every network
  capability; this records a user consent event for one host and grants nothing
  outside the git push/fetch/pull path, so consenting to a git remote can never
  widen hosted AI provider, telemetry, or gateway egress.
- `DenyByDefaultBroker::consent_git_remote_host` / `revoke_git_remote_host`,
  following the `pin_workspace_path_roots` precedent, surfaced through
  `WorkspaceActor` so the app can reach them.
- Intents `GrantGitRemoteHost` / `RevokeGitRemoteHost`, commands
  `:git-allow-remote <host>` / `:git-revoke-remote <host>`, and a desktop
  `Allow <host>` button that appears only while a host-naming denial is the
  standing verdict and grants exactly the host that was refused.
- Consent is itself audited: grants and withdrawals join the same visible trail
  as the operations they govern, so a change of verdict is never unexplained.

**Ordering, and a deliberate deviation.** Consent is checked *before* air-gap
and *after* the blocklist. Review asked that air-gap still deny when it is
deliberately on. It is placed above air-gap instead, for two reasons. First,
`air_gap` defaults to `true` and there is still no settings surface that can
author a policy, so ordering consent below it would leave the grant inert and
reproduce the fail-shut state exactly. Second, air-gap's purpose is to stop
egress nobody asked for; a host the user named, granted, and can revoke is the
opposite of that. The operator blocklist is still checked first, so an
administrator can hard-block a host that no user grant reopens, and consent does
not bypass workspace trust. If enforcement under a deliberately-enabled air-gap
is preferred over a working grant, moving the consent check below the air-gap
check is a one-line change — but it needs a way to author `air_gap` first, or
the feature returns to being unusable.

**What consent does not do:** it is held in the broker for the process lifetime
and is not persisted, so grants do not survive a restart. Persistence belongs
with the settings surface that does not yet exist, and is recorded here rather
than faked.

### Gate run (2026-08-16, Windows 11)

| Gate | Result |
| --- | --- |
| `cargo fmt --all` | PASS (no diff after formatting) |
| `cargo test --workspace --all-targets --no-fail-fast` | PASS — 255 test binaries, 2885 tests passed, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (after fixing one `redundant_closure` in the new policy code) |
| `cargo run -p xtask -- verify-kanban-backlog` | PASS — 10 epics, 41 features, 160 tasks |
| `cargo run -p xtask -- docs-hygiene` | PASS |
| `cargo run -p xtask -- claim-audit` | PASS |
| `cargo run -p xtask -- verify-readiness-consistency` | PASS — 160 tasks cross-checked |
| `cargo run -p xtask -- golden-path-1` | PASS (T1 acceptance names GP-1, so it was run explicitly) |

One pre-existing guard caught a real regression during this work and is
recorded rather than hidden: adding the three git remote palette specs tripped
`palette_command_mode_covers_registered_command_catalog`, the catalog-drift
guard in `crates/legion-app/tests/palette.rs`. That is the guard working as
designed; the three commands were added to its allowlist alongside the other
git mutations.

### Backlog status

P2.F5.T1, T2, T3, and T4 are moved to `done`. Each has a passing test named
above that exercises its acceptance, and each stop condition is covered by a
test rather than by assertion. The `files` and `verification` fields were
corrected to real paths and to the commands that actually cover the work.

