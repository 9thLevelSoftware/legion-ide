# Live language-server evidence — rounds r08 and r09

Packet `s2-02a-r09-live-language-server-evidence`. Worktree
`D:/legion-ide-completion`, branch `codex/full-product-resume`, `HEAD`
`51af391a45d33cf15a359c1e4b91bc314d768cde` at the time this record was written.

## Classification

**Component and integration evidence against real language servers. Not product
acceptance.**

The ten tests drive `AppComposition` in process. There is no packaged product, no
window, no OS-level input, and no external oracle reading the disk from outside
the product. The layer is `component` / `integrated` and the input route is not
`native-input`, so this evidence cannot qualify as product evidence under
`qualifies_as_product_evidence` and can never promote an `acceptance` value.
`COMP-LANG-009` and `COMP-LANG-010` remain `implementation: partial` /
`acceptance: unassessed`, and nothing in this record proposes otherwise.

## Why this record lists more than one run

The owner's round instruction named three runs. Five are listed below because the
round-r08 ledger holds four transcripts, not three: runs 2 and 3 are two separate
invocations taken in the same loaded host window, differing in command line
(run 3 adds `--test-threads=1` and drops the TypeScript target) and in failure
count (3 of 4 Python tests failed in run 2, 4 of 4 in run 3). Run 5 is this
round's own confirming invocation, taken by the round-r09 cargo lane. The count
is five transcripts, not an inconsistency.

## Host facts, recomputed for this record

Recomputed on this Windows 11 host on 2026-09-09 by the packet implementer, not
copied from an earlier record.

| Fact | Value as measured | Compared against | Result |
| --- | --- | --- | --- |
| `node --version` | `v24.19.0` | — | recorded as observed |
| `(Get-Command node).Source` | `C:\Program Files\nodejs\node.exe` | — | recorded as observed; see the Node identity section |
| `typescript-6.0.3.tgz` SHA-256 | `33cd0ee1beaa8c9e9d15a9da836c62ddea4c34a42d7c2d349dbc80d94165d22a` | `typescript-bundle/verified-archives.json` `sha256` | matches |
| `typescript-6.0.3.tgz` bytes | `4515854` | `verified-archives.json` `bytes` | matches |
| `typescript-language-server-6.0.0.tgz` SHA-256 | `6e23b48efc76af4e70928cdfe62ea6e6cfef67ab4c1e7579c4e82dd284fbdfd2` | `verified-archives.json` `sha256` | matches |
| `typescript-language-server-6.0.0.tgz` bytes | `515598` | `verified-archives.json` `bytes` | matches |
| `pyright-1.1.400.tgz` SHA-256 | `2ccba7af9c8b14bb81c8fa9bb558d8b5181b586ec4dfc448b78eb4209e7a429a` | no SHA-256 is recorded in `pyright-1.1.400-metadata.json` | no counterpart to compare; recorded as observed |
| `pyright-1.1.400.tgz` SHA-1 | `9833249250639ae2a436369e5a5d66e67fbd811c` | `pyright-1.1.400-metadata.json` `dist.shasum` | matches |
| `pyright-1.1.400.tgz` SHA-512, base64 | `sha512-e0NFMPWhMvPn4siTAN6MsWZ6S/ePydUp1BOXbkh+8I+H+/KulnBb+2ceHNkI32nxZWalQxfUMNoGCHdGMXsmUA==` | `pyright-1.1.400-metadata.json` `dist.integrity` | matches |
| `git rev-parse HEAD` | `51af391a45d33cf15a359c1e4b91bc314d768cde` | — | recorded as observed |

The three archives live at
`.superpowers/sdd/2026-09-04-full-product-completion/typescript-bundle/typescript-6.0.3.tgz`,
`.superpowers/sdd/2026-09-04-full-product-completion/typescript-bundle/typescript-language-server-6.0.0.tgz`
and `.superpowers/sdd/2026-09-04-full-product-completion/pyright-1.1.400.tgz`.
No archive was replaced, refreshed, or re-downloaded for this record.

## Node identity: the version is established, the path is not

The Node **version** used by the lane is established as `v24.19.0`. The Node
**path** is not, and this record does not assert one.

Two Node executables exist on this host and both report the same version:

- `C:\Program Files\nodejs\node.exe`, which is what `(Get-Command node).Source`
  resolves for an ordinary shell here. `node --version` on it prints `v24.19.0`.
- `C:/Users/dasbl/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe`,
  the path named at
  `.superpowers/sdd/2026-09-04-full-product-completion/legion-completion-round.js:27`,
  quoted verbatim from that line:

  ```js
  const NODE = 'C:/Users/dasbl/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe'
  ```

  Invoking that executable directly with `--version` also prints `v24.19.0`.

That constant reaches the cargo lane as instruction text, not as an inherited
environment variable: line 94 of the same script composes the lane's brief with
`'Export LEGION_TEST_NODE_RUNTIME=' + NODE + ' in every shell you use.\n'`, so
whether the lane exported that value or a different absolute path is a property
of the lane's shell, not of the script.

None of the five transcripts — the four round-r08 logs or the round-r09 log —
carries an `ENV:` line or otherwise records the value of
`LEGION_TEST_NODE_RUNTIME`, so **which of the two binaries the lane used cannot
be determined from the logs**. What the logs do establish is
that *some* absolute, existing Node executable was selected: `selected_node()` in
both test files reads `LEGION_TEST_NODE_RUNTIME`, `expect`s it to be present, and
asserts `is_absolute()` and `is_file()` before canonicalising
(`crates/legion-app/tests/typescript_app_startup.rs:33-40`,
`crates/legion-app/tests/python_app_startup.rs:33-40`), so an unset or bogus
variable would have panicked rather than passed.

Every claim below that mentions Node therefore says `v24.19.0` and does not name
a path.

## Run 1 — round r08, first invocation, green

| Field | Value |
| --- | --- |
| Source log | `.superpowers/sdd/2026-09-04-full-product-completion/round-r08-test-s2-02a-live-language-servers.log` |
| `CMD` | `cargo test -p legion-app --test typescript_app_startup --test python_app_startup -j 1 --no-fail-fast -- --ignored --nocapture` |
| `CWD` | `D:/legion-ide-completion` |
| `EXIT` | `0` |
| Host condition | not recorded in the log; no concurrent build is attributed to it |
| `python_app_startup` result line | `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 23.47s` |
| `typescript_app_startup` result line | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.76s` |

| Test | Target | Outcome in this run |
| --- | --- | --- |
| `explicit_typescript_startup_is_lazy_live_and_restart_preserves_dirty_text` | `typescript_app_startup` | passed |
| `native_typescript_rename_is_reviewable_and_applies_only_after_approval` | `typescript_app_startup` | passed |
| `native_typescript_rename_conflict_does_not_partially_apply` | `typescript_app_startup` | passed |
| `native_typescript_formatting_is_reviewable_before_disk_mutation` | `typescript_app_startup` | passed |
| `native_typescript_missing_import_code_action_is_reviewable_and_applies_after_approval` | `typescript_app_startup` | passed |
| `native_typescript_organize_imports_is_reviewable_and_removes_only_unused_imports` | `typescript_app_startup` | passed |
| `explicit_python_startup_is_lazy_live_and_restart_preserves_dirty_text` | `python_app_startup` | passed |
| `native_python_rename_is_reviewable_cancelable_and_saves_cross_file_edit` | `python_app_startup` | passed |
| `native_python_rename_external_overwrite_rejects_without_partial_mutation` | `python_app_startup` | passed |
| `native_python_diagnostic_clears_after_editor_replace_and_save` | `python_app_startup` | passed |

The log also contains `Compiling legion-app v0.1.0 (D:\legion-ide-completion\crates\legion-app)`
in the same invocation, so both test binaries were linked at that tree before
running.

## Run 2 — round r08, rerun while the MSI build loaded the host, partially failed

| Field | Value |
| --- | --- |
| Source log | `.superpowers/sdd/2026-09-04-full-product-completion/round-r08-fixtest1-s2-02a-fix1-tests.log` |
| `CMD` | `cargo test -p legion-app --test typescript_app_startup --test python_app_startup -j 1 --no-fail-fast -- --ignored --nocapture` |
| `CWD` | `D:/legion-ide-completion` |
| `EXIT` | `101` |
| Host condition | taken while the MSI package build loaded this host outside the cargo lane |
| `python_app_startup` result line | `test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.39s` |
| `typescript_app_startup` result line | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 23.17s` |
| Trailing error | `error: 1 target failed:` naming `-p legion-app --test python_app_startup` |

| Test | Target | Outcome in this run |
| --- | --- | --- |
| `explicit_typescript_startup_is_lazy_live_and_restart_preserves_dirty_text` | `typescript_app_startup` | passed |
| `native_typescript_rename_is_reviewable_and_applies_only_after_approval` | `typescript_app_startup` | passed |
| `native_typescript_rename_conflict_does_not_partially_apply` | `typescript_app_startup` | passed |
| `native_typescript_formatting_is_reviewable_before_disk_mutation` | `typescript_app_startup` | passed |
| `native_typescript_missing_import_code_action_is_reviewable_and_applies_after_approval` | `typescript_app_startup` | passed |
| `native_typescript_organize_imports_is_reviewable_and_removes_only_unused_imports` | `typescript_app_startup` | passed |
| `explicit_python_startup_is_lazy_live_and_restart_preserves_dirty_text` | `python_app_startup` | **failed** |
| `native_python_rename_is_reviewable_cancelable_and_saves_cross_file_edit` | `python_app_startup` | **failed** |
| `native_python_rename_external_overwrite_rejects_without_partial_mutation` | `python_app_startup` | **failed** |
| `native_python_diagnostic_clears_after_editor_replace_and_save` | `python_app_startup` | passed |

All three failures panicked at `crates\legion-app\tests\python_app_startup.rs:54:9`
on the `assert_ne!` in `wait_for_live`, printing `left: Refused` and
`right: Refused`, and carrying the projection quoted in *The recorded failure
projection* below.

## Run 3 — round r08, serialized Python rerun in the same loaded window, fully failed

| Field | Value |
| --- | --- |
| Source log | `.superpowers/sdd/2026-09-04-full-product-completion/round-r08-fixtest1-s2-02a-fix1-tests-python-serial.log` |
| `CMD` | `cargo test -p legion-app --test python_app_startup -j 1 --no-fail-fast -- --ignored --nocapture --test-threads=1` |
| `CWD` | `D:/legion-ide-completion` |
| `EXIT` | `101` |
| Host condition | same loaded host window as run 2 |
| `python_app_startup` result line | `test result: FAILED. 0 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 37.50s` |
| `typescript_app_startup` result line | none — the target is not on this command line |

| Test | Target | Outcome in this run |
| --- | --- | --- |
| `explicit_typescript_startup_is_lazy_live_and_restart_preserves_dirty_text` | `typescript_app_startup` | not executed — target absent from the command line |
| `native_typescript_rename_is_reviewable_and_applies_only_after_approval` | `typescript_app_startup` | not executed — target absent from the command line |
| `native_typescript_rename_conflict_does_not_partially_apply` | `typescript_app_startup` | not executed — target absent from the command line |
| `native_typescript_formatting_is_reviewable_before_disk_mutation` | `typescript_app_startup` | not executed — target absent from the command line |
| `native_typescript_missing_import_code_action_is_reviewable_and_applies_after_approval` | `typescript_app_startup` | not executed — target absent from the command line |
| `native_typescript_organize_imports_is_reviewable_and_removes_only_unused_imports` | `typescript_app_startup` | not executed — target absent from the command line |
| `explicit_python_startup_is_lazy_live_and_restart_preserves_dirty_text` | `python_app_startup` | **failed** |
| `native_python_rename_is_reviewable_cancelable_and_saves_cross_file_edit` | `python_app_startup` | **failed** |
| `native_python_rename_external_overwrite_rejects_without_partial_mutation` | `python_app_startup` | **failed** |
| `native_python_diagnostic_clears_after_editor_replace_and_save` | `python_app_startup` | **failed** |

`native_python_diagnostic_clears_after_editor_replace_and_save` is the one Python
test that passed in run 2 and failed here; the two runs differ in the same loaded
window only by serialization and by the absent TypeScript target.

## Run 4 — round r08, coordinator rerun on an idle host, green

| Field | Value |
| --- | --- |
| Source log | `.superpowers/sdd/2026-09-04-full-product-completion/round-r08-post-python-idle.log` |
| `CMD` | `cargo test -p legion-app --test python_app_startup -j 1 --no-fail-fast -- --ignored --nocapture` |
| `CWD` | `/d/legion-ide-completion` — written in POSIX form in this log, where runs 1–3 write `D:/legion-ide-completion`; the two spellings name the same directory on this host |
| `EXIT` | `0` |
| Host condition | idle host, per the round-r08 coordinator note in `progress.md` |
| `python_app_startup` result line | `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 24.53s` |
| `typescript_app_startup` result line | none — the target is not on this command line |

| Test | Target | Outcome in this run |
| --- | --- | --- |
| `explicit_typescript_startup_is_lazy_live_and_restart_preserves_dirty_text` | `typescript_app_startup` | not executed — target absent from the command line |
| `native_typescript_rename_is_reviewable_and_applies_only_after_approval` | `typescript_app_startup` | not executed — target absent from the command line |
| `native_typescript_rename_conflict_does_not_partially_apply` | `typescript_app_startup` | not executed — target absent from the command line |
| `native_typescript_formatting_is_reviewable_before_disk_mutation` | `typescript_app_startup` | not executed — target absent from the command line |
| `native_typescript_missing_import_code_action_is_reviewable_and_applies_after_approval` | `typescript_app_startup` | not executed — target absent from the command line |
| `native_typescript_organize_imports_is_reviewable_and_removes_only_unused_imports` | `typescript_app_startup` | not executed — target absent from the command line |
| `explicit_python_startup_is_lazy_live_and_restart_preserves_dirty_text` | `python_app_startup` | passed |
| `native_python_rename_is_reviewable_cancelable_and_saves_cross_file_edit` | `python_app_startup` | passed |
| `native_python_rename_external_overwrite_rejects_without_partial_mutation` | `python_app_startup` | passed |
| `native_python_diagnostic_clears_after_editor_replace_and_save` | `python_app_startup` | passed |

## Run 5 — round r09 confirming run, green, but on an already-built binary

This run **executed**. The round-r09 cargo lane ran exactly this packet's command
and its complete transcript is in the ledger. The table below is transcribed from
that transcript, line by line.

| Field | Value |
| --- | --- |
| Source log | `.superpowers/sdd/2026-09-04-full-product-completion/round-r09-test-packet-tests-03-s2-02a-legion-app-live-language-servers.log` |
| This packet's owned copies | The lane wrote **one combined transcript** covering both targets, so no split exists to record. That single file is copied byte-for-byte to `.superpowers/sdd/2026-09-04-full-product-completion/s2-02a-r09-typescript-app-startup.log` and to `.superpowers/sdd/2026-09-04-full-product-completion/s2-02a-r09-python-app-startup.log`. All three files share SHA-256 `7319a460c3057111848a8fbfc8f4b6b699d33cbc50d42822a420b686b867a363`; each contains both targets in full, and neither owned copy is a per-target extract. |
| `CMD` | `cargo test -p legion-app --test typescript_app_startup --test python_app_startup -j 1 --no-fail-fast -- --ignored --nocapture` |
| `CWD` | `D:/legion-ide-completion` |
| `EXIT` | `0` |
| Host condition | **not recorded.** The transcript carries no host-condition line and no `ENV:` line. Whether packet `s1-02g-r09`'s driver build or native run was in flight when the lane took this run cannot be read out of it, and that is exactly the load condition that produced runs 2 and 3. This record therefore states the host condition as unrecorded, not as idle. |
| Build state | The transcript opens with ``Finished `test` profile [unoptimized + debuginfo] target(s) in 0.37s`` and carries **no `Compiling` line**, so the lane re-ran already-built test binaries rather than rebuilding at round-r09 `HEAD`. The binaries it ran are named in the transcript as `target\debug\deps\python_app_startup-08100240498e019d.exe` and `target\debug\deps\typescript_app_startup-d9da698c6a613b6d.exe`. What this run establishes is the behaviour of those binaries, executed during round r09; it is not independent proof that a rebuild at `51af391a45d33cf15a359c1e4b91bc314d768cde` would produce them. |
| `python_app_startup` result line | `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.44s` |
| `typescript_app_startup` result line | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.73s` |

| Test | Target | Outcome in this run |
| --- | --- | --- |
| `explicit_typescript_startup_is_lazy_live_and_restart_preserves_dirty_text` | `typescript_app_startup` | passed |
| `native_typescript_rename_is_reviewable_and_applies_only_after_approval` | `typescript_app_startup` | passed |
| `native_typescript_rename_conflict_does_not_partially_apply` | `typescript_app_startup` | passed |
| `native_typescript_formatting_is_reviewable_before_disk_mutation` | `typescript_app_startup` | passed |
| `native_typescript_missing_import_code_action_is_reviewable_and_applies_after_approval` | `typescript_app_startup` | passed |
| `native_typescript_organize_imports_is_reviewable_and_removes_only_unused_imports` | `typescript_app_startup` | passed |
| `explicit_python_startup_is_lazy_live_and_restart_preserves_dirty_text` | `python_app_startup` | passed |
| `native_python_rename_is_reviewable_cancelable_and_saves_cross_file_edit` | `python_app_startup` | passed |
| `native_python_rename_external_overwrite_rejects_without_partial_mutation` | `python_app_startup` | passed |
| `native_python_diagnostic_clears_after_editor_replace_and_save` | `python_app_startup` | passed |

All ten names print `... ok` in the transcript; none is missing from it, and no
name in it is absent from the table. The run was taken once and is reported once.
It was not re-run to obtain this result.

## The recorded failure projection

Every Python failure in runs 2 and 3 — seven failing test executions in total —
printed the same projection. Quoted verbatim from the logs:

```
failure_reason: Some("language-server launch configuration is invalid: Node executable identity check timed out")
```

with, on the same projection line, `restart_count: 0, max_auto_restarts: 3` and
`lifecycle: Refused`, and a health record of
`init_status: Unavailable, capabilities: [], version: None`.

This observation is filed as **`DEF-0003`** by packet
`s2-02b-def-0003-node-probe-timeout` in round r09. This record refers to it by id
only. **This packet asserts nothing about its severity, its cause, whether it is
a product defect or a host artefact, whether it invalidates a required outcome,
or how it should be repaired.** Those are that packet's to argue.

## What these runs establish

Across runs 1, 2, 4 and 5 — the four invocations that reached a green target —
real language-server processes were launched by the product composition in
process and answered real requests:

- **Pyright**, materialized by the product from the retained archive
  `pyright-1.1.400.tgz` (SHA-1 matching `dist.shasum`, SHA-512 matching
  `dist.integrity` in `pyright-1.1.400-metadata.json`) through
  `AppComposition::configure_downloaded_language_server_local` with the tier-two
  primary Python adapter, under Node `v24.19.0`. Four named Python tests passed
  together three times (run 1, run 4 and run 5).
- **typescript-language-server 6.0.0 with typescript 6.0.3**, configured through
  the ordinary `CommandDispatchIntent::ConfigureTypeScriptToolchain` intent from
  the retained archives whose SHA-256 digests match `verified-archives.json`,
  under Node `v24.19.0`. Six named TypeScript tests passed together three times
  (run 1, run 2 and run 5).

The oracles in those tests are real rather than mirrors of the implementation:
they wait for `LspResultStatus::Fresh` from the launched process, require real
completion, hover and diagnostic payloads, and in the Python external-overwrite
case read the files back through `std::fs::read_to_string` after mutating them
behind the product.

They also establish, as a measured fact and not an inference, that these ten
tests do **not** produce a stable result under all host conditions on this host:
in chronological order the same Python four passed 4/4 (run 1), failed 3/4
(run 2) and 4/4 (run 3) in a loaded window, then passed 4/4 (run 4) and 4/4
(run 5), all on the same tree.

## What these runs do not establish

- **They are not product acceptance and cannot become it.** See *Classification*.
  No `acceptance` value in `plans/completion/requirements.json` may move because
  of them. `COMP-LANG-009` and `COMP-LANG-010` stay `partial` / `unassessed`, and
  `COMP-LANG-002` is unchanged.
- **They do not make any language-server release approved.**
  `BLK-2026-09-08-06` (owner-selected TypeScript/JavaScript language-server,
  browser and debug-adapter releases) and `BLK-2026-09-08-07` (an approved
  `tailwindcss-language-server` release with a published version and SHA-256) are
  **owner selections**. Executing a retained pinned archive is not the owner
  choosing a release. Both blockers stand untouched by this record.
- **They say nothing about JavaScript, Rust, or any other language beyond what
  the six TypeScript tests and four Python tests execute**, and nothing about the
  browser or debug-adapter rows.
- **They establish no Node path**, only the version `v24.19.0`. See *Node
  identity*.
- **They establish nothing about a packaged product.** No installer, no window,
  no OS-level input, no external process oracle.
- **Run 5 does not establish a rebuild at round-r09 `HEAD`.** Its transcript
  carries no `Compiling` line, so it exercised already-built test binaries; and
  it records no host condition, so it is not evidence of a green result *on an
  idle host*. What it establishes is that the ten named tests passed once, during
  round r09, against those binaries.

## Proposed `BLK-2026-09-08-05` transition

For the record role to accept, amend or reject. This packet proposes it and does
not apply it; `.superpowers/sdd/2026-09-04-full-product-completion/blockers.json`
is the record role's file.

The blocker's stated prerequisite is *"A native Node runtime at or above 22.22.2
approved on this host plus the retained TypeScript fixtures, so that `cargo test
-p legion-app --test typescript_app_startup -- --ignored` can execute."* Its
stated `why` is that both targets *"compiled and executed nothing"* and that the
TypeScript workflow therefore *"has no execution evidence"*.

**The `why` is no longer true.** Node on this host is `v24.19.0`, above
`22.22.2`; the retained TypeScript and Pyright archives are present with matching
digests; and the command has executed five times, four times in round r08 and
once in round r09, producing the transcripts recorded above. Holding the blocker
open on the "compiled and executed nothing / no execution evidence" premise would
now be wrong.

The word **approved** in the stated prerequisite is a separate matter and this
packet does not treat it as satisfied. Nothing in the ledger records an owner
approving a Node runtime on this host; a version being present is not an
approval, exactly as a retained archive is not an owner selection for
`BLK-2026-09-08-06` / `BLK-2026-09-08-07`. Whether that word is met is an owner
determination, and the record role should settle it before accepting the
transition below.

Proposed text:

> `BLK-2026-09-08-05` moves from `blocked` to `resolved` on the ground that its
> stated `why` — that both targets "compiled and executed nothing" and that the
> TypeScript workflow "has no execution evidence" — is closed. Node on this host
> is `v24.19.0`, the retained TypeScript and Pyright archives are present with
> digests matching their metadata, and
> `cargo test -p legion-app --test typescript_app_startup --test python_app_startup -j 1 --no-fail-fast -- --ignored --nocapture`
> executed four times in round r08 and once in round r09 — the round-r09 run
> being the lane transcript
> `.superpowers/sdd/2026-09-04-full-product-completion/round-r09-test-packet-tests-03-s2-02a-legion-app-live-language-servers.log`,
> `EXIT=0`, python `4 passed; 0 failed` in `22.44s`, typescript
> `6 passed; 0 failed` in `17.73s`. All five are recorded in
> `plans/evidence/full-product-resume-2026-09-09/live-language-server-evidence.md`.
> The "approved" element of the prerequisite is untouched by this transition: no
> owner approval of a Node runtime exists in the ledger, and that determination
> belongs to the owner, not to this record.
>
> Resolving it changes no `acceptance` value. `COMP-LANG-001`, `COMP-LANG-002`
> and `COMP-LANG-009` remain `unassessed`; what now exists for them is component
> and integration evidence against real language servers, not product acceptance.
> The executions were not uniformly green: of five invocations, three exited `0`
> and two exited `101`, with seven Python test executions refusing startup under
> host load. That observation is filed separately as `DEF-0003` by packet
> `s2-02b-def-0003-node-probe-timeout`; this transition takes no position on it
> and does not depend on its outcome. `BLK-2026-09-08-06` and
> `BLK-2026-09-08-07` are untouched and still `blocked`.

If the record role prefers to hold `BLK-2026-09-08-05` open — because the
round-r09 run re-used already-built binaries rather than rebuilding at
`51af391a45d33cf15a359c1e4b91bc314d768cde`, or because the "approved" element is
an owner determination that has not been made — that is a defensible
alternative. What is not defensible is holding it open on the premise that the
tests cannot execute.

## Provenance of this record

- The five run tables are transcribed from five lane transcripts: the four named
  round-r08 lane logs, and the round-r09 lane log
  `round-r09-test-packet-tests-03-s2-02a-legion-app-live-language-servers.log`
  for run 5. No count, duration, exit code or test outcome in them was carried
  over from an earlier report; each was read out of the log named in its own
  table.
- The host facts were recomputed on 2026-09-09 by this packet, not copied.
- No `.rs` file, no file under `plans/completion/`, no blockers file, no retained
  archive, none of the four round-r08 logs and not the round-r09 lane log was
  modified by this packet. The round-r09 lane log was read and copied; its bytes
  are unchanged, as the shared SHA-256 in the run-5 table shows.
- Round-r08's `s2-02a` deliverables, which reported these same runs as
  `not executed`, were rejected and reverted and are not the source of anything
  here; they are retained in the ledger as
  `s2-02a-live-language-server-evidence-rejected.patch` and
  `s2-02a-live-language-server-evidence-rejected-newfiles/` for reference only.
