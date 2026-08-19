# PKT-EXTERNAL-AGENT Evidence — P6.F4.T2 / P6.F4.T3

**Branch:** `feat/external-agent-containment`
**Tasks:** `P6.F4.T2` (run one external agent in a Legion-governed worktree/sandbox),
`P6.F4.T3` (convert external edits into proposals and external logs into evidence)
**Status:** DONE

Both stop conditions describe an escape, so the work is the refusals. 25 of the
31 tests added are refusals; the 6 positive tests exist so the refusals cannot be
achieved by refusing everything.

---

## 1. Containment model

An external agent never touches the main workspace. Three boundaries, in three
crates, composed by `legion-app`:

### 1.1 The lease (`legion-agent`)

The agent runs inside a disposable leased worktree
(`DelegatedTaskSandboxOrchestrator`). `ExternalAgentScope::new(lease_root,
main_workspace_root, allowed_tools)` refuses at construction if the lease *is*
the main workspace or *contains* it (`validate_not_main_workspace`), so a
misconfiguration can never make every main-workspace read "in scope".

### 1.2 Read/write authorization (`legion-agent::external::ExternalAgentSession`)

Every filesystem request the agent makes goes through the session, which applies
two independent guards in order and audits both outcomes:

1. **`resolve_lease_relative_read` (new, `worktree.rs`)** — the boundary guard.
   Resolves relative requests against the **lease root**, not the host process's
   working directory, then delegates to `validate_containment` for `..`
   collapsing and symlink-following resolution on both sides. This is the only
   guard that catches a traversal request or an in-lease symlink aimed outside.
2. **`validate_delegated_task_tool_call`** — the tool allowlist and the
   forbidden-path list.

**Why guard 1 was needed.** `validate_containment` resolves a *relative* path
against `std::env::current_dir()`. That is the right base for a delegated write
target the host chose and the wrong base for a path an external agent typed: the
agent names paths relative to its lease and the host may be running anywhere.
Measured against the host's CWD, `src/lib.rs` reads as an escape and the `..` in
`../../etc/passwd` is counted against the wrong boundary.

**Honest limit on guard 2.** After guard 1 returns, guard 2's own containment
check (`target_is_within_scope`) is structurally satisfied — it can no longer
fail. It is kept as defense in depth but is deliberately not relied on: it
compares path components lexically, so `<lease>/../../etc/passwd` passes it and a
symlink spelled inside the lease passes it too. Guard 2's live contributions here
are the tool allowlist and the forbidden-path list. See the masking finding in
§4.1.

`.git` is denied by name. In a leased worktree `.git` is a link file whose
contents name the *main* repository's git directory; it is genuinely inside the
lease, so the boundary guard allows it and only the forbidden-path list refuses
it.

### 1.3 Transport admission — the honest half

`ExternalAgentFilesystemAccess` has two shapes:

* `HostBrokered` — the agent holds no filesystem handle; every read is a request
  the host answers (the local adapter bridge shape ratified by ADR-0043). The
  session is then the *actual* boundary, because there is no other way for the
  agent to obtain file content.
* `DirectProcess { os_read_enforced }` — the agent process reads the filesystem
  itself. The session's decisions are advisory for such an agent.

`ExternalAgentSession::begin` **refuses** `DirectProcess { os_read_enforced:
false }`. This is not conservatism, it is the stop condition: a process holding
real file descriptors routes nothing through the decision layer, so admitting one
means an external agent that can read outside its assigned scope.

**Finding: no sandbox backend confines reads today.** New
`legion_sandbox::os_read_enforcement(backend)` reports this per backend, and every
arm returns `BrokerOnly` with its own reason:

| Backend | Reason reads are unconfined |
| --- | --- |
| Seatbelt | `generate_sbpl_profile` emits `(allow file-read* (subpath "/"))` |
| BubblewrapLandlock | the Landlock ruleset handles write access rights only (`AccessFs::from_write`) |
| RestrictedToken | `spawn_sandboxed_windows` reports `filesystem_read_enforced: false` |
| AppContainer | read scoping not implemented |
| DocumentedFallback | a documented fallback does not scope reads |

Consequently `ExternalAgentSession::begin` refuses every direct-process external
agent on every platform today, and will stop doing so only when a backend can
truthfully report `OsEnforced`. That wiring is asserted end-to-end in
`no_sandbox_backend_admits_a_direct_filesystem_external_agent`.

### 1.4 Sandbox read scope (`legion-sandbox`)

`SandboxScope` gains `readable_roots` and `denied_read_paths`;
`ActivatedSandbox::authorize_read` fails closed outside the readable set and
audits every decision. Deny is evaluated **before** allow, so a denied prefix
cannot be re-opened by widening the readable roots. Widening the read surface
never widens the write surface — `authorize_write` still consults only
`workspace_root`.

`legion-agent` may not depend on `legion-sandbox`
(`plans/dependency-policy.md`), so the two crates each own their own guard and
`legion-app` composes them. No new dependency edge and no new crate (ADR-0046).

### 1.5 The exit from the lease (`legion-agent` + `legion-app`)

Nothing the agent writes is a workspace mutation.

* `external_edits_to_proposals` re-authorizes every edit through the session and
  converts **every** edit into a `WorkspaceProposal` or none of them. Duplicate
  paths are rejected: two proposals for one file mean whichever applied second
  silently discards the reviewed content of the first.
* `legion_app::proposal::admit_external_edits` is the admission gate.
  `ExternalEditAdmission` has private fields and `admit_external_edits` is its
  only constructor, so an apply path that requires an admission cannot be reached
  with an edit that skipped the gate — the unproposed case is unrepresentable,
  not merely checked. `admitted_external_proposals` takes admissions rather than
  paths for the same reason.
* The gate checks both directions (every edit covered by exactly one proposal,
  every proposal covering exactly one edit), re-derives the content fingerprint
  from the bytes about to be admitted and compares it with the hash the reviewed
  proposal carries, and rejects unsafe paths lexically (absolute, drive-prefixed,
  backslash-separated, or containing `..`).
* `external_logs_to_evidence_records` converts every log into exactly one
  metadata-only evidence row, rejecting empty and duplicate labels because both
  silently produce fewer distinct rows than logs.

---

## 2. Negative tests and what each proves

### `crates/legion-agent/tests/external_agent_containment.rs` (18 tests)

| Test | What breaks without the guard |
| --- | --- |
| `a_read_of_an_absolute_path_outside_the_lease_is_refused` | an agent reads any absolute path on the host |
| `a_read_of_a_main_workspace_file_is_refused` | the lease exists but the main workspace is still readable |
| `a_relative_traversal_out_of_the_lease_is_refused` | `../../outside/secret.txt` reads outside the lease |
| `a_read_through_an_in_lease_symlink_pointing_outside_is_refused` | a symlink/junction spelled inside the lease reads outside it |
| `a_read_of_the_lease_git_link_is_refused` | the agent learns the main repository's location from `.git` |
| `a_lease_that_is_the_main_workspace_is_refused` | leasing the workspace makes every workspace read in-scope |
| `a_lease_that_contains_the_main_workspace_is_refused` | same escape by containment instead of identity |
| `a_direct_filesystem_agent_without_os_read_enforcement_is_refused` | an agent with real file descriptors runs while nothing confines its reads |
| `a_write_from_a_read_only_scope_is_refused` | a read grant becomes a write grant |
| `every_refused_request_leaves_an_audit_row` | refusals happen with nothing for a reviewer to see |
| `one_out_of_lease_edit_aborts_the_whole_batch` | two of three edits convert and the escaping one is unaccounted for |
| `two_edits_to_the_same_path_are_refused` | the second proposal to apply discards the first's reviewed content |
| `two_logs_sharing_a_label_are_refused` | two logs collapse to one row for any consumer keying by evidence id |
| `an_unlabelled_log_is_refused` | a row exists that cannot be traced to its log |
| `log_text_never_reaches_the_evidence_row_verbatim` | raw agent output is persisted in evidence |

Positive (non-vacuity): `an_in_lease_relative_read_resolves_against_the_lease_not_the_process_cwd`,
`every_external_log_becomes_an_evidence_row`,
`a_scoped_external_agent_run_reads_writes_and_proposes_without_bypassing_policy`.

### `crates/legion-app/tests/external_edit_admission.rs` (13 tests)

| Test | What breaks without the guard |
| --- | --- |
| `an_edit_with_no_proposal_is_refused` | the stop condition itself: an unreviewed edit lands |
| `a_batch_with_one_unproposed_edit_admits_nothing` | a smuggled `.github/workflows/release.yml` rides along in an approved batch |
| `content_swapped_after_review_is_refused` | the reviewed path lands with substituted bytes |
| `a_proposal_that_no_edit_produced_is_refused` | an approved-looking path with no agent behind it lands |
| `a_traversal_edit_path_is_refused` | `../../etc/profile` lands with a matching proposal |
| `an_absolute_edit_path_is_refused` | `/etc/profile` lands with a matching proposal |
| `a_backslash_separated_edit_path_is_refused` | a Windows separator carries traversal past a `/`-only check |
| `a_drive_prefixed_edit_path_is_refused` | `C:/Windows/...` lands with a matching proposal |
| `a_proposal_carrying_no_content_hash_is_refused` | path matching becomes the only binding to the review |
| `two_proposals_covering_one_path_are_refused` | ambiguous review provenance for one file |
| `two_edits_to_one_path_are_refused` | same, from the edit side |
| `no_sandbox_backend_admits_a_direct_filesystem_external_agent` | the two crates are each internally consistent and still admit an unconfined agent between them |

Positive: `a_governed_run_admits_exactly_the_edits_its_proposals_cover`.

### `crates/legion-sandbox/src/lib.rs` unit tests (6 added)

`read_outside_scope_fails_closed_and_audits`,
`read_of_a_name_prefixed_sibling_directory_is_refused` (a
`String::starts_with` boundary would let `/workspace/project-secrets` through),
`read_traversal_out_of_scope_is_refused`,
`denied_read_prefix_wins_over_an_enclosing_readable_root`,
`an_extra_readable_root_does_not_become_writable`,
`no_backend_claims_os_level_read_enforcement`, plus
`read_inside_scope_is_allowed_and_audited`.

---

## 3. Mutation results

Fourteen mutations. Each was applied to the working tree, the affected test
target run, then reverted with `git checkout`; `git status --porcelain` was empty
at the end (verified).

| # | Mutation | Tests killed |
| --- | --- | --- |
| M1 | `resolve_lease_relative_read`: lease anchoring removed (resolve against process CWD) | 5 — `an_in_lease_relative_read_resolves_against_the_lease_not_the_process_cwd`, `a_scoped_external_agent_run_...`, `a_read_of_the_lease_git_link_is_refused`, `a_write_from_a_read_only_scope_is_refused`, `two_edits_to_the_same_path_are_refused` |
| M2 | `ExternalAgentSession::authorize`: containment guard removed | 4 — `a_relative_traversal_out_of_the_lease_is_refused`, `a_read_through_an_in_lease_symlink_pointing_outside_is_refused`, `one_out_of_lease_edit_aborts_the_whole_batch`, `every_refused_request_leaves_an_audit_row` |
| M3 | `ExternalAgentSession::authorize`: tool/forbidden-path scope check removed | 3 — `a_write_from_a_read_only_scope_is_refused`, `a_read_of_the_lease_git_link_is_refused`, `every_refused_request_leaves_an_audit_row` |
| M4 | `ExternalAgentScope::new`: `validate_not_main_workspace` removed | 2 — `a_lease_that_is_the_main_workspace_is_refused`, `a_lease_that_contains_the_main_workspace_is_refused` |
| M5 | `ExternalAgentSession::begin`: direct-process refusal removed | 2 — `a_direct_filesystem_agent_without_os_read_enforcement_is_refused`, `no_sandbox_backend_admits_a_direct_filesystem_external_agent` |
| M6 | `ActivatedSandbox::authorize_read`: readable-scope boundary removed | 3 — `read_outside_scope_fails_closed_and_audits`, `read_of_a_name_prefixed_sibling_directory_is_refused`, `read_traversal_out_of_scope_is_refused` |
| M7 | `ActivatedSandbox::authorize_read`: denied-read-prefix guard disabled | 1 — `denied_read_prefix_wins_over_an_enclosing_readable_root` |
| M8 | `os_read_enforcement` returns `OsEnforced` unconditionally | 2 — `no_backend_claims_os_level_read_enforcement`, `no_sandbox_backend_admits_a_direct_filesystem_external_agent` |
| M9 | `admit_external_edits`: missing-proposal guard removed (unproposed edits skipped) | 2 — `an_edit_with_no_proposal_is_refused`, `a_batch_with_one_unproposed_edit_admits_nothing` |
| M10 | `admit_external_edits`: content-fingerprint binding removed | 1 — `content_swapped_after_review_is_refused` |
| M11 | `admit_external_edits`: orphan-proposal guard removed | 1 — `a_proposal_that_no_edit_produced_is_refused` |
| M12 | `admit_external_edits`: `validate_workspace_relative_path` removed | 4 — traversal, absolute, backslash, drive-prefixed |
| M13 | `external_logs_to_evidence_records`: duplicate-label guard removed | 1 — `two_logs_sharing_a_label_are_refused` |
| M14 | `external_edits_to_proposals`: per-edit re-authorization removed | 2 — `one_out_of_lease_edit_aborts_the_whole_batch`, `a_scoped_external_agent_run_...` |

Every mutation killed at least one test. No mutation was silent.

---

## 4. Findings

### 4.1 Masking: the absolute-path read tests do not test the containment guard

Under **M2** (containment guard removed from `authorize`),
`a_read_of_an_absolute_path_outside_the_lease_is_refused` and
`a_read_of_a_main_workspace_file_is_refused` **survived**. They are masked by
`validate_delegated_task_tool_call`: with containment gone, an absolute
out-of-lease path is joined onto the lease root, `Path::join` with an absolute
argument replaces the base, and `target_is_within_scope`'s lexical
`starts_with` check then correctly refuses it.

So those two tests, on their own, would pass with the containment guard removed.
They are kept — they describe the behaviour a reader cares about — but they are
**not** the evidence that the boundary guard works. That evidence is
`a_relative_traversal_out_of_the_lease_is_refused` and
`a_read_through_an_in_lease_symlink_pointing_outside_is_refused`, which isolate
the guard because both are spelled entirely inside the lease and therefore pass
the lexical scope check. The isolation runs the other way too: `.git` and the
read-only-scope write are inside the lease, so only the scope check refuses them
(killed by M3, not by M2).

The masking is documented in the `authorize_read` doc comment so a future reader
does not mistake `target_is_within_scope` for the boundary.

### 4.2 The Windows symlink test was vacuous before it was fixed

The first version of `a_read_through_an_in_lease_symlink_pointing_outside_is_refused`
used `std::os::windows::fs::symlink_dir` with a skip-on-failure guard, copied
from the existing pattern in `containment_canonicalization.rs`. Windows only
grants `symlink_dir` with `SeCreateSymbolicLinkPrivilege` (Developer Mode or
elevation), so on this host — and on any CI runner without it — the test printed
`skipping: symlink creation not permitted on this host` and passed while
asserting nothing. Verified directly with `--nocapture`.

Fixed by falling back to a directory **junction** (`mklink /J`), which is the
same class of reparse point for containment purposes (`std::fs::canonicalize`
resolves it through `GetFinalPathNameByHandle` exactly as it does a symlink) and
requires no privilege. The test now runs for real on Windows, confirmed by the
absence of the skip line and by M2 killing it.

Note: `crates/legion-agent/tests/containment_canonicalization.rs` still uses the
skip-only pattern, so its three symlink tests are still vacuous on unprivileged
Windows hosts. Not changed here — out of scope for these two tasks — but it is
the same defect and worth a follow-up.

### 4.3 No platform enforces read scope; the honest response is refusal

`filesystem_read_enforced` is `false` in every arm of `spawn_sandboxed`, and the
macOS SBPL profile explicitly grants `(allow file-read* (subpath "/"))`. Rather
than describe the session's decisions as containment for a process that never
consults them, `ExternalAgentSession::begin` refuses the direct-process transport
outright. The `HostBrokered` transport is not a weaker version of this — for a
brokered agent the session *is* the only path to file content, so the boundary is
real.

---

## 5. Verification run

All commands run on Windows 11, `-j 6`.

* `cargo test -p legion-agent` — pass (17 targets, 150 tests)
* `cargo test -p legion-sandbox` — pass (26 tests)
* `cargo test -p legion-app` — pass

`cargo test -p legion-app` needs two helper binaries that a single-package
invocation does not build (`legion-debug --bin fake_dap_adapter`, `legion-lsp
--bin mock_lsp_server`). Without them,
`debug_workflow_live_fake_adapter_sets_live_projection_flag` and the two
`language_restart_policy` crash tests fail with an explicit "run under
`cargo test --workspace --all-targets`" message. Pre-existing and unrelated to
this change — confirmed by reproducing before building the binaries and passing
after — but noted so a future reader does not mistake it for a regression.
* `cargo clippy --workspace --all-targets -j 6 -- -D warnings` — clean
* `cargo fmt --all` — applied
* `cargo run -q -p xtask -- check-deps` — pass
* `cargo run -q -p xtask -- docs-hygiene` — pass
* `cargo run -q -p xtask -- claim-audit` — pass
* `cargo run -q -p xtask -- extract-before-modify` — pass
* `cargo run -q -p xtask -- verify-kanban-backlog` — pass
* `cargo run -q -p xtask -- verify-readiness-consistency` — pass

No chokepoint file was modified: all new logic lives in `external.rs`,
`worktree.rs`, `evidence.rs`, `proposal.rs`, and `legion-sandbox/src/lib.rs`.
`legion-agent/src/lib.rs` changed only by re-exports.
