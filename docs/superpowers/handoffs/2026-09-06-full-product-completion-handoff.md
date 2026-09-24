# Full Product Completion Handoff

**User-requested stop:** Do not resume implementation automatically. Resume only
when the user asks. The full goal is unfinished, not completed or externally blocked.

This handoff records the state of the full-product completion effort at the
2026-09-06 stop point. It is intended to let the coordinator resume without
reconstructing the worktree history. It does not certify the product, promote
any readiness row, or close the completion goal.

## Working identity and boundaries

| Item | Current value |
|---|---|
| Completion worktree | `D:/legion-ide-completion` |
| Branch | `codex/full-product-completion` |
| Pre-checkpoint HEAD | `1d11e5a` |
| Original worktree | `D:/legion-ide` (untouched; preserve it) |
| Full scope | 59 package rows and 419 requirements; still unassessed as a complete candidate matrix |
| Native GUI | Paused by user; no GUI qualification claim |
| Remote state | No push, publish, merge, or release mutation |
| Cargo/process ownership | Root is the sole Cargo/process owner; all workers used bounded Luna tasks |

The original planning worktree remains separate. Do not copy changes back to
`D:/legion-ide`. This document is included in the checkpoint commit; use
`git log -1 --format=fuller -- docs/superpowers/handoffs/2026-09-06-full-product-completion-handoff.md`
to locate that commit. A checkpoint commit is not a production release.

## Current implementation state

The worktree contains several active implementation streams. They are useful
source and focused-test evidence, but they do not collectively establish full
product completion.

| Stream | Current state |
|---|---|
| S1 renderer/input | Renderer, navigation, post-paint/input, metrics, and related vendor/source work has accumulated through the S1 stream. Several focused checks and commits exist in the ledger, but complete renderer-backed, cross-platform product qualification remains open. |
| S2 LSP runtime | App/LSP supervision, document synchronization, write-side proposal flow, CAS applyEdit arbitration, diagnostics retention, code actions, mixed commands, and Organize Imports migration are in the current tree. The LSP composition fixture reached 22/22 in `s2-lsp-composition-root-r2.log`. |
| TypeScript | Retained local archives and an approved local Node path support real-server focused tests. The native startup/action matrix reached 5/5 in `s2-typescript-native-root-r23.log`; this is app plus real-server evidence, not packaged/native GUI evidence. |
| Organize Imports | Normal intent uses request-owned scope, private origin-buffer lookup, descendant filtering, sole-edit preview, opaque alternatives, resolve handling and mixed-command sidecars. Focused app/integration checks pass. The new real TypeScript scenario fails because the returned/applied edit combines imports but retains the unused name; investigate before claiming the real workflow complete. |
| Python | Three new ignored tests plus the existing startup test cover diagnostics, editor correction/save, cross-file rename preview/cancel/approve/apply/save, external-overwrite rejection, and restart. See final checkpoint results below. Python formatting and normal Python toolchain settings remain missing; do not assume Pyright supplies formatter integration. |

Retained artifacts and runtime prerequisites include the pinned TypeScript and
Pyright archives and the approved local Node runtime path. No network download
or external artifact mutation is authorized by this handoff.

## Verified evidence before the stop

The latest accepted focused baselines are:

- `s2-app-lib-root-r20.log`: 418/418 app library tests passed in 0.55s after
  20.68s compilation.
- `s2-lsp-composition-root-r2.log`: 22/22 app LSP composition tests passed.
- `s2-typescript-native-root-r23.log`: 5/5 native TypeScript startup/action
  tests passed in 18.13s after 36.23s compilation. The r22 4/5 result is
  historical; the missing definition/use rename edit was repaired with a real
  workspace synchronization barrier. The native fixture was unchanged and no
  completion warmup was added. The deterministic once-issued UI rename-ordering
  regression is included.
- `s2-typescript-codeaction-root-r1.log`: the real TypeScript missing-import
  test passed.
- The app unit suite now covers the initial mixed-command queue/backpressure
  path, CAS callback arbitration, per-attempt resolve cancellation/retry, the
  shared 32-operation cap, and resolved mixed-action retention/execution.

These are component and implementation-seam results. They do not establish
packaged completion, native GUI behavior, all 59 packages, or all 419
requirements.

The final app checkpoint, `checkpoint-app-tests.log`, passed **424/424 app unit
tests and 6/6 cross-file integration tests** (45.47s compilation; 0.57s and
0.18s execution). This includes the completed Organize Imports migration and
failure reporting. The prior r3 integration failure selected the wrong status
row; the assertion now checks the actual Failed row and absence of a command.
`cargo fmt --all --check` passed after one formatting-only adjustment.
Final real-server results are recorded in the checkpoint results section below.

## Files and local prerequisites

Read these plans before choosing the next feature:

- `docs/superpowers/plans/2026-09-04-full-product-completion.md`
- `docs/superpowers/plans/2026-09-04-manual-language-completion.md`
- `docs/superpowers/plans/2026-09-04-ai-team-completion.md`
- `docs/superpowers/plans/2026-09-04-production-qualification.md`
- `docs/superpowers/plans/2026-09-04-completion-traceability.md`
- `docs/superpowers/specs/2026-09-04-product-completion-design.md`
- `plans/completion/requirements.json` (419 acceptance entries remain unassessed).

Implementation map:

- `crates/legion-app/src/language/lsp_reads.rs`: real request admission, deferred
  synchronization, response ingestion, organize/select/resolve/execute paths.
- `code_actions.rs`, `code_action_commands.rs`, `code_action_diagnostics.rs` in
  that directory: bounded opaque authority, mixed command retention, original
  diagnostic payloads. `server_apply_edits.rs` and `apply_edit_decision.rs`:
  proposal authority and shared cross-thread timeout/claim arbitration.
- `crates/legion-app/src/lib.rs`: normal intents, document-sync ledger,
  proposal application, sidecar claiming, and toolchain settings orchestration.
- `crates/legion-app/src/language/typescript_bundle.rs`, `startup_authority.rs`,
  `materialize.rs`: pinned local archives and explicit runtime approval.
- `crates/legion-desktop/src/view/streamed_layout.rs` and
  `streamed_layout/worker.rs`: bounded renderer layout/measurement work.
- New real-server tests: `crates/legion-app/tests/typescript_app_startup.rs`
  (six ignored scenarios), `python_app_startup.rs` (four ignored scenarios).

Local evidence root: `.superpowers/sdd/2026-09-04-full-product-completion/`.
Its detailed `progress.md`, raw logs, and retained archives are ignored local
artifacts, not committed prerequisites available in a fresh clone. Preserve
them on this machine. Do not force-add caches or downloads to Git.

- Node: `C:/Users/dasbl/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe`
  (v24.19.0 at verification).
- TypeScript archives: `typescript-bundle/` under that evidence root,
  typescript-language-server 6.0.0 and TypeScript 6.0.3.
- Pyright archive: `pyright-1.1.400.tgz` under that evidence root.
- TS language-server SHA-256:
  `6e23b48efc76af4e70928cdfe62ea6e6cfef67ab4c1e7579c4e82dd284fbdfd2`.
- TypeScript SHA-256:
  `33cd0ee1beaa8c9e9d15a9da836c62ddea4c34a42d7c2d349dbc80d94165d22a`.

Run from `D:/legion-ide-completion` in PowerShell, using **one Cargo owner and
`-j 1`**. Parallel compiler jobs previously exhausted memory.

```powershell
cargo test -p legion-app --lib --test lsp_rename_across_files -j 1
$env:LEGION_TEST_NODE_RUNTIME='C:/Users/dasbl/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe'
cargo test -p legion-app --test typescript_app_startup --test python_app_startup -j 1 --no-fail-fast -- --ignored --nocapture
cargo test -p legion-app --all-targets -j 1 --no-fail-fast
```

## Design rulings to preserve

These decisions were explicitly reviewed and should not be reversed during
resume:

1. The full 59-package/419-requirement scope remains active. Passing focused
   tests never removes or accepts an unrun requirement.
2. Component evidence is reported as component evidence. Do not call the
   native TypeScript matrix packaged completion, native GUI qualification, or
   a whole-product gate.
3. Native GUI automation remains paused by the user. Do not resume it by
   automatic continuation.
4. LSP UI remains projection-only. App authority owns sessions, document sync,
   proposal lifecycle, and workspace mutation; LSP workers do not mutate
   buffers or disk directly.
5. Write-side LSP operations remain proposal-mediated with authoritative
   buffer, snapshot, version, workspace-generation, and fingerprint
   preconditions. Stale, conflict, denial, or failed outcomes preserve dirty
   text and fail closed.
6. Organize Imports is a code-action request using `source.organizeImports`.
   Descendant kinds remain in the organize lane. The request must retain its
   own authority and origin buffer; do not select from the active-tab UI as a
   fallback.
7. A sole edit-only Organize Imports result may use the dedicated auto-preview
   convenience. Multiple alternatives require opaque response/action selection.
8. Mixed and resolved commands retain the canonical operation kind and
   response/action identity in their sidecars. The sidecar is a bounded,
   one-use authority; consume-before-apply is intentional. A stale or failed
   action requires a fresh request rather than restoring possibly stale command
   authority.
9. Explicit selection of an advertised command may issue
   `workspace/executeCommand`; a synthetic pre-command edit proposal is not
   required by the approved design. Any server workspace/applyEdit mutation
   remains behind the existing proposal/CAS/review authority.
10. Resolve attempts require unique attempt identity, cancellation/retry
    isolation, and the shared 32-operation bound. Late responses must not
    release a newer retry or resurrect a consumed candidate.
11. The actual current code and logs outrank historical ADR prose. Preserve
    historical results as historical and update evidence only from named logs.
12. Do not invent signing credentials, download artifacts, push, publish, or
    commit with an unreviewed scope expansion.

## Ordered resume procedure

Resume in this order; keep Cargo/process execution serialized under root
ownership.

1. Reopen this handoff and `.superpowers/sdd/2026-09-04-full-product-completion/progress.md`.
   Revalidate branch, `HEAD`, worktree status, retained archives, approved
   Node path, and absence of unrelated Cargo/rustc processes.
2. Inspect the checkpoint commit and any newer working-tree diff. Confirm that only
   the intended S1 renderer, S2 LSP/TypeScript/Organize Imports, and Python
   test changes are present, plus this handoff. Preserve unrelated user work.
3. Consult final checkpoint results before repeating work. Organize Imports
   has passed the focused app tests; inspect any residual issues against:
   normal intent request scope, origin-buffer selection, capability/authority
   arming, sole-edit preview failure reporting, alternative chooser behavior,
   resolved-command retention, mixed sidecar operation kind, and command
   branch status. Keep the approved consume-before-apply and explicit-command
   rulings above.
4. Repair any failures explicitly listed in the final checkpoint section,
   preserving the real workflow oracle. Do not repeat already-passing checks
   unless source changes or an unresolved risk requires it.
5. Next feature gaps include Python formatter provisioning/routing and normal
   Python toolchain settings, then remaining Rust/Python navigation/refactoring,
   S2-04 build/test adapters and S2-05 debugging. Pyright alone does not provide
   Python formatting. Reuse the existing proposal-mediated edit pipeline and
   design the secondary formatter capability route before implementing it.
6. Re-run the affected app/LSP integration suites, including composition,
   cross-file rename, diagnostics/code-action, deferred document-sync, and
   mixed/resolved command paths. Inspect failures rather than weakening tests.
7. Update only the relevant evidence prose after logs are authoritative. Keep
   ADR-0018 and ADR-0034 component-only and preserve historical evidence; do
   not promote readiness or alter requirement acceptance.
8. Reconcile the full 419-requirement matrix and all 59 package rows. For every
   row, bind implementation, source, focused tests, product workflow evidence,
   platform evidence, and remaining gaps. A missing native GUI or platform
   artifact remains unassessed, not passed.
9. Run the applicable repository gates in the documented order: formatting,
   dependency policy, docs hygiene, claim audit, workspace checks/tests,
   clippy with `-D warnings`, cargo-deny, and the required xtask phase gates.
   Include release/update-drill and recorded evaluation gates where their
   prerequisites are satisfied. Do not treat a single-OS pass as the 3-OS
   standing gate.
10. Keep native GUI paused unless the user separately resumes it. If resumed,
    collect the required renderer-backed evidence across supported platforms
    and preserve the run logs/screenshots; do not infer GUI success from
    headless tests.
11. Perform independent review of consequential changes. Checkpoint commits
    are allowed when authorized; they do not require or imply full completion.
    Mark the goal complete only after all original requirements are proved.
    No push or publication is authorized by this document.

## Outstanding gates and risks

- Full 59-package and 419-requirement acceptance remains unassessed.
- Native GUI/product workflow qualification is still absent and paused.
- See final checkpoint results for the new Python/TypeScript tests. The full
  workspace all-target suite, clippy, and 21 xtask phase gates have not been
  rerun for this combined checkpoint. Older passing results are not proof of
  the complete current tree.
- Cross-platform standing gates, release/signing evidence, and any externally
  dependent credentials remain subject to their existing runbook and ledger
  conditions.
- Historical OOM behavior occurred when broad Cargo work was run in parallel;
  retain the root ruling to use serialized `cargo -j1` execution where needed.
- S1 renderer changes have earlier focused evidence, but the final combined
  desktop suite and native GUI acceptance remain unverified.

## Final checkpoint results

Root app tests: 424 unit tests and 6 cross-file integration tests passed.
Formatting: passed. Both real-server test binaries compiled. The combined
opt-in run exited 101: **Python 1/4 passed; TypeScript 5/6 passed**. Compilation
took 38.36s; Python ran 32.92s and TypeScript 18.68s. All five previously
verified TypeScript scenarios still pass, as does existing Pyright startup.

First resume items, in priority order:

1. `native_typescript_organize_imports_is_reviewable_and_removes_only_unused_imports`
   applied `import { unused, used } from "./lib";` while the test expects
   `unused` removed. Inspect the actual server action kind/payload and request
   scope to distinguish sort/combine behavior from remove-unused behavior.
   Keep the real unused-import requirement; do not simply weaken the assertion.
2. Both new Python rename tests fail before creating a proposal. The visible
   production failure is `unsupported WorkspaceEdit shape: annotated edits
   (annotationId) are not supported`. Implement annotated edit handling in the
   translator with preserved annotation/review semantics and existing mutation
   preconditions. Do not strip annotation metadata blindly or bypass proposals.
3. `native_python_diagnostic_clears_after_editor_replace_and_save` waits for
   `cannot be assigned` in a metadata-only projection. The diagnostic actually
   arrives on `main.py` at UTF-16 13..20 with code `reportAssignmentType`, severity
   Error, source Pyright, and redacted message `LSP error diagnostic`. Correct the
   test to identify the diagnostic through its retained code/file/range, then
   verify correction, clear and save. Preserve metadata-only privacy behavior.
4. Pyright stderr reports the Windows Python execution alias cannot find Python.
   This did not prevent server startup or the annotated-edit response. Explicit
   Python interpreter configuration is a remaining product/toolchain task; do
   not treat it as the cause of the translator refusal without evidence.

The new tests remain ignored/opt-in and are committed intact, including their
failing assertions. This is a resumable development checkpoint, not a release.
Authoritative checkpoint logs are committed under
`plans/evidence/full-product-checkpoint-2026-09-06/`:

- `checkpoint-app-tests.log`
- `checkpoint-native-tests.log`
- `checkpoint-format-final.log`

All agent work and root test sessions are stopped at handoff. No background
implementation, push, or publication is scheduled. Resume only on user request.
