# Full Product Resume Handoff — 2026-09-08

This handoff records the paused full-product completion packet. Implementation
is stopped until the user resumes it. The original workspace is
`D:/legion-ide`; preserve it. The current worktree is
`D:/legion-ide-completion` on `codex/full-product-resume`, based at
`84919ba` (`origin/main` after PR217 was merged). The old
`codex/full-product-completion` branch is preserved; do not resume the old
checkpoint at `cffdc9a`.

No full-completion claim is made. The complete scope remains 59 packages and
419 requirements, all unassessed. The inventory is 143 implemented, 233
partial, and 43 absent. The 59 rows comprise 54 primary package IDs plus five
cross-cutting rows: S0-03, S0-04, S1-01, S1-03B, and S4-01. The canonical files
`matrix.json`, `scenarios.json`, `dependencies.json`, `defects.json`, and
`candidate.json` are missing. The candidate-dependent validator therefore
needs a pre-candidate path. Do not invent exemptions or approvals.

The governing documents are the [approved design](../specs/2026-09-04-product-completion-design.md)
and [master implementation plan](../plans/2026-09-04-full-product-completion.md).
The local execution ledger and retained tools are under
`.superpowers/sdd/2026-09-04-full-product-completion/` in the completion worktree.
This ignored directory is not included in the pushed Git history; durable
verification logs are committed in the evidence directory below.

## Current packet and source anchors

The packet removed fabricated local rename tokens and empty formatting,
Organize Imports, and code-action proposals. Relevant anchors are
`crates/legion-app/src/language/local_proposals.rs`,
`crates/legion-app/src/language/code_actions.rs`,
`crates/legion-app/src/language/lsp_reads.rs`,
`crates/legion-app/src/language/translate/change_annotations.rs`, the
workspace-edit annotation DTO in `crates/legion-protocol`,
`crates/legion-desktop/src/view/proposal_cards.rs`, and the focused app,
protocol, and desktop tests.

TypeScript uses the advertised `_typescript.organizeImports` command in
`mode: "All"` with `skipDestructiveCodeActions: false`, normalized Windows paths,
synchronization before the callback-owned reviewed proposal, and CAS
arbitration before mutation. The standard `source.organizeImports` path sorts
but retains unused imports; do not weaken cross-file rename to a one-edit
oracle or restore fake proposals.

Python uses the exact retained Pyright artifact and pinned implicit default:
server ID 104, language `python`, with SHA-256
`2ccba7af9c8b14bb81c8fa9bb558d8b5181b586ec4dfc448b78eb4209e7a429a`.
Annotation handling is bounded to 256 annotations, 4096 targets, and 64 KiB;
other annotation cases are strict. Explicit client explanation confirmation
is `true`. Retained Node must be set in every shell invocation. The exact
archives are in the SDD root: `pyright-1.1.400.tgz` and `typescript-bundle/`;
no fresh download is needed for the old scenarios.

The [bounded-stdin specification](../specs/2026-09-08-bounded-process-stdin.md)
is planning-only commit `7cbbbf9`; its
implementation has not started. The next implementation is real bounded-stdin
backpressure and cancellation coverage, followed by Python formatter
provisioning/settings through app-owned proposal/CAS authority and exact
executable approval.

## Durable evidence

Evidence is under `plans/evidence/full-product-resume-2026-09-07/`:

- `resume-focused-r3.log`: pre-removal focused counts.
- `resume-language-r3.log`: current 439 app and 16/16 language-tooling pass.
- `resume-native-r3.log`: real-server Python 4/4 and TypeScript 6/6 pass.
- `resume-desktop-r1.log`: 243 desktop unit/rendering tests pass.
- `resume-desktop-targeted-r2.log`: targeted renderer 1/1 pass.
- `resume-clippy-r2.log`: workspace all-targets clippy with `-D warnings`, exit 0.

The final terminal-focused checkpoint is `resume-terminal-checkpoint.log`:
6/6 passed, exit 0. The earlier stale command-context defect was found by
review; native r3 passed before the final cleanup fix, and the focused stale
context check then passed. The current language result is the post-fix 439/16
result in `resume-language-r3.log`.

The implementation checkpoint is commit
`aee2dd99bf18533630aa27507698065344b845b9` (`aee2dd9`), following planning
commit `7cbbbf9`. Durable checkpoint logs for formatting, dependency policy,
docs hygiene, claim audit, and extract-before-modify are also copied under the
evidence directory; each completed check passed.

The full-workspace test run was deliberately stopped during build at the
user’s request. Terminal session `17007` exited 1 after Cargo process `26352`
was killed; the durable raw log is
`resume-workspace-tests-r1-interrupted.log`. This is not a suite result and
must not be reported as either a pass or a product failure. The named focused
checks and checkpoint gates above are complete; the full workspace suite and
the remaining production qualification gates have not passed at this checkpoint.

## Verification boundaries

All native results are explicit app/real-server or desktop evidence. They do
not qualify packaged completion, native GUI acceptance, or the full product.
Native GUI work remains paused. Root owns Cargo and process execution, using
serialized `-j 1` runs; all workers were Luna bounded tasks.

For retained real-server scenarios, set the runtime in each shell and use the
archives described above:

```powershell
$env:LEGION_TEST_NODE_RUNTIME='C:/Users/dasbl/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe'
cargo test -p legion-app --test typescript_app_startup --test python_app_startup -j 1 --no-fail-fast -- --ignored --nocapture
```

Do not infer unrun workspace tests, dependencies, release, cross-platform, or
phase gates from recorded checks. Do not turn a clippy pass into a full test
pass or claim the interrupted workspace build as a failure of the product.

## Next concrete work after resume

Recheck the branch state and finish the interrupted full workspace test run:

```powershell
Set-Location D:/legion-ide-completion
git status --short --branch
cargo test --workspace --all-targets -j 1 --no-fail-fast
```

Then implement bounded stdin from `7cbbbf9` with real backpressure and
cancellation tests. Keep the full
59-package/419-requirement goal and missing canonical files visible. Python
formatter provisioning/settings follows through the app formatter proposal/CAS
path with exact executable approval.

The user authorized committing and pushing this checkpoint branch. No PR or
merge was requested. Verify the remote branch state before resuming; future
deployment or merge is not authorized by this handoff.
