# WS-LANG-01 Write-Side Evidence — PKT-LSP-C (M8)

**Branch:** m8/lsp-write-side
**Commit range:** 96e9729..08a70e2
**Date:** 2026-07-05
**Session:** https://claude.ai/code/session_01HMw3X3iusfbbZhaWDm9Q4B

---

## Summary

PKT-LSP-C is the final M8-planned packet: LSP write-side proposal translation + session
lifecycle UX. Four tasks delivered:

- **T1** — Lazy session start: session starts on first `.rs` file open or explicit palette
  command, not on workspace open. Palette commands "Language Server: Start" and
  "Language Server: Restart" added.
- **T2** — WorkspaceEdit → proposal translation: `translate_workspace_edit()` in
  `crates/legion-app/src/language/translate.rs`. Handles `documentChanges` (modern) and
  `changes` (legacy) LSP formats. Resource ops translated to `WorkspaceFileOperation`.
  Annotated edits rejected with `TranslationError::UnsupportedShape`.
- **T3** — Restart / backoff UX: `BackingOff` state with exponential backoff (base 500 ms,
  3x cap, max 30 s). `LspSessionStatusProjection` DTO added to
  `LanguageToolingProjection`. Lifecycle counters and countdown visible to UI.
- **T4** — stderr ring buffer as redacted projection: bounded ring (100 lines, 512 bytes/line),
  `redact_lsp_stderr_line()` replaces absolute paths with `[REDACTED]`.
  `LspSessionLogProjection` DTO added to `LanguageToolingProjection`.

**Authority boundary adherence:**
- Write-side actions NEVER applied directly — translate only.
- `P2.F1.T5` stays `in-progress` (apply activation is P3.F1.T2).
- All stderr content through `redact_lsp_stderr_line` before projection.
- UI projection-only throughout.

---

## Task 1: Lazy session start

**Files changed:**
- `crates/legion-app/src/lib.rs` — removed eager start from `open_workspace`;
  added lazy trigger in `bind_opened_file` for `.rs` files; added palette commands.
- `crates/legion-app/src/ui.rs` — added `LspStartSession`, `LspRestartSession` intents.
- `crates/legion-app/tests/app_lsp_composition.rs` — updated `t5_refused_health_in_snapshot`
  to call `force_lsp_start_for_test()` (pre-existing test broken by lazy start in T1).
- `crates/legion-app/tests/palette.rs` — added `CommandCase` entries + `Noop` variant
  to cover "Language Server: Start" and "Language Server: Restart" in catalog gate.

**TDD test module:** `language::app_lsp::lsp_lazy_start_tests` (6 tests, all pass)

---

## Task 2: WorkspaceEdit → proposal translation

**Files changed:**
- `crates/legion-app/src/language/translate.rs` (NEW) — `DocumentResolver` trait,
  `ResolvedDocument` struct, `TranslationError` enum, `translate_workspace_edit()`,
  `lsp_position_to_byte_offset()`, `uri_to_canonical_path()`.
- `crates/legion-app/src/language/mod.rs` — added `mod translate; pub use translate::{...}`.

**TDD test module:** `translate::tests` (8 tests, all pass)

---

## Task 3: Restart / backoff UX

**Files changed:**
- `crates/legion-app/src/language/app_lsp.rs` — `BackingOff` state variant,
  `transition_failure()`, `restart_for_workspace()`, `session_status_projection()`,
  `set_backing_off_for_test()`, `backoff_tests` module.
- `crates/legion-protocol/src/lib.rs` — `LspSessionLifecycleKind` enum,
  `LspSessionStatusProjection` struct, `lsp_session_status` field in
  `LanguageToolingProjection`.

**TDD test module:** `language::app_lsp::backoff_tests` (5 tests, all pass)

---

## Task 4: stderr ring buffer as redacted projection

**Files changed:**
- `crates/legion-lsp/src/lib.rs` — added `LspStdioSession::take_stderr()` delegation.
- `crates/legion-app/src/language/session.rs` — added `stderr_ring: Arc<Mutex<VecDeque<String>>>`,
  `take_stderr()`, `stderr_ring()` to `RustAnalyzerSession`.
- `crates/legion-app/src/language/redaction.rs` — added `redact_lsp_stderr_line()` with
  Windows and Unix path detection and redaction.
- `crates/legion-app/src/language/app_lsp.rs` — added `STDERR_RING_CAPACITY` (100),
  `STDERR_LINE_MAX_LEN` (512), `stderr_ring` field in `LspWorkerHandle`, `drain_stderr()`
  background thread function, `stderr_log_projection()`, `inject_stderr_ring_for_test()`,
  `stderr_tests` module.
- `crates/legion-protocol/src/lib.rs` — `LspSessionLogProjection` struct, `lsp_session_log`
  field in `LanguageToolingProjection`.
- `crates/legion-protocol/tests/dto_contracts.rs` — added `lsp_session_status: None` and
  `lsp_session_log: None` to exhaustive struct initializer.
- `crates/legion-app/src/lib.rs` — wire `lsp_session_log` into
  `shell_projection_snapshot()` and the exhaustive `LanguageToolingProjection` initializer.

**TDD test modules:**
- `language::redaction::t4_redaction_tests` (4 tests: Windows path, Unix path, non-path,
  sentinel-secret negative assertion) — all pass
- `language::app_lsp::stderr_tests` (5 tests: idle=None, empty=None, cap-100, injection,
  sentinel-secret negative assertion) — all pass

---

## Commits

| Hash | Title |
| --- | --- |
| 87dd2e5 | feat(lsp): lazy session start + restart palette commands (PKT-LSP-C T1) |
| 56b3579 | feat(lsp): WorkspaceEdit → proposal translation (PKT-LSP-C T2) |
| 96e9729 | feat(lsp): restart/backoff UX + lifecycle status projection (PKT-LSP-C T3) |
| 08a70e2 | feat(lsp): stderr ring buffer as redacted projection (PKT-LSP-C T4) |

---

## Gate Results

CWD: `C:\Users\dasbl\RustroverProjects\legion-ide-gp1`
Command: `pwsh -File C:\Users\dasbl\RustroverProjects\legion-ide\.superpowers\sdd\m8\gp1_gates.ps1`
Full log: `C:\Users\dasbl\RustroverProjects\legion-ide\.superpowers\sdd\m8\gp1-gates.log`

| Gate | Result |
| --- | --- |
| fmt-apply | PASS |
| fmt | PASS |
| check-deps | PASS |
| docs-hygiene | PASS |
| claim-audit | PASS |
| no-egui-textedit | PASS |
| verify-kanban | PASS |
| release-pipeline | PASS |
| verify-release-pipeline | PASS |
| workspace-check | PASS |
| workspace-test | PASS |
| clippy | PASS |
| perf-harness | PASS |
| verify-perf-harness | PASS |
| cargo-deny | PASS |
| rust-analyzer-smoke | PASS |
| golden-path-1 | PASS |

---

## Tests updated for lazy start

The following existing tests assumed eager start behavior (session started on `open_workspace`)
and were updated to use the explicit trigger:

1. `crates/legion-app/tests/app_lsp_composition.rs::t5_refused_health_in_snapshot` —
   added `app.force_lsp_start_for_test()` call before `drain_lsp_session()`. The test was
   broken by T1 (pre-existing regression). Fix: added `force_lsp_start_for_test()` public
   test-helper to `AppComposition`.

2. `crates/legion-app/tests/palette.rs::palette_command_mode_covers_registered_command_catalog` —
   added two `CommandCase` entries for "Language Server: Start" and "Language Server: Restart"
   to satisfy the palette command coverage gate. Added `Noop` variant to `ExpectedOutcome`.

---

## Concerns

None. All authority boundaries respected. `P2.F1.T5` remains `in-progress`.
Apply activation (P3.F1.T2) is the only remaining write-side gate for GA.

---

## P2.F1.T5 closure — 2026-08-16

Acceptance: "rename across files is reviewable and reversible." Stop condition:
"write-side actions must never bypass the proposal pipeline." The task's own
note said it stayed in-progress until apply activation (P3.F1.T2) landed;
P3.F1.T2 is `done`, so the stated blocker is gone.

### What was actually missing

Two things, and neither was the one the note named.

**No production caller asked the language server.** `issue_lsp_rename_request`
existed and had exactly one caller: its own `_for_test` wrapper. The
`RequestRenameProposal` intent ran only `run_language_proposal`, which is
answered by Legion's own `LexicalIndexer`. The same held for formatting and
organize-imports. Every write-side action produced a proposal — the stop
condition was never violated — but the proposal came from the in-house index,
not from rust-analyzer.

**The acceptance had never been tested.** Single-file apply and rollback were
proven in `checkpoint_restore`; a rename spanning two files, reviewed, applied
and reversed, was not.

### The reversal mechanism, corrected

Writing the acceptance test surfaced two facts worth recording, because both
contradict what the obvious test would assume:

- **Applying a text-edit proposal does not write to disk.** It edits the buffer;
  saving is a separate, separately-authorized step. `checkpoint_restore`'s
  post-rollback file assertion is therefore about a file that was never written.
- **The lifecycle `Rollback` command does not reverse the mutation.** It records
  that a reversal happened. The declared reversal for a text-edit route is
  `ProposalRollbackAction::EditorUndoGroup` — editor undo — and
  `collect_checkpoint_targets` returns nothing for a `TextEdit` rollback, so no
  durable checkpoint is created for one. That is coherent: a text edit never
  reached disk, so there is nothing on disk to check-point.

The acceptance test asserts the real contract: apply renames both buffers and
writes nothing; undo reverses both buffers; the ledger then records `RolledBack`.

### What changed

- `RequestRenameProposal`, `RequestFormattingProposal` and
  `RequestOrganizeImportsProposal` now also ask the language server, gated on
  `renameProvider`, `documentFormattingProvider` and `codeActionProvider`. The
  index answer still returns synchronously so no surface is blank.
- `ingest_lsp_rename_result` became `ingest_lsp_write_side_result`, taking an
  `LspWriteSideSpec`. The four write-side actions differ only in name and
  proposal kind; the WorkspaceEdit → translate → validate → preview path is
  identical, and a second copy of it is how one of them quietly stops
  registering its proposal.
- A formatting response is a bare `TextEdit[]`; it is lifted into a
  `WorkspaceEdit` so it takes the same preconditions and review as a rename.
- Organize-imports is a code action narrowed to `source.organizeImports`, which
  is how LSP spells it — not a request of its own.
- **A code action carrying only a `command` is refused, out loud.** Running one
  needs `workspace/executeCommand`, which would let a server mutate the
  workspace outside the proposal pipeline — precisely what the stop condition
  forbids. The refusal is recorded as a failed operation rather than producing
  nothing, because producing nothing looks like the action did not exist.

### Tests

```
cargo test -p legion-app --test lsp_rename_across_files      # 6 passed
  a_cross_file_rename_is_reviewable_before_anything_changes
  a_cross_file_rename_applies_and_reverses_in_both_files
  a_formatting_response_becomes_a_reviewable_proposal
  an_organize_imports_code_action_becomes_a_reviewable_proposal
  a_command_only_code_action_is_refused_rather_than_executed
  write_side_requests_skip_when_the_server_does_not_advertise_them
```

The test derives its document URIs from the app itself
(`document_uri_for_buffer_for_test`) rather than building them from temp paths.
An earlier draft built them by hand and failed on URI resolution — which would
have been the test asserting against its own URI-building bug rather than
against the app's resolver.

### Workspace state at closure

`cargo test --workspace --all-targets --no-fail-fast` — 261 suites, 2988 passed,
0 failed. `cargo clippy --workspace --all-targets -- -D warnings` — clean.

### Not claimed

**Generic LSP code-action selection is not wired.** `RequestCodeActionProposal`
takes an `action_id` chosen from projection data, and the only actions in that
projection are the index's own quick fixes — server-supplied code actions are
not projected as selectable rows. Wiring it would need a listing read kind, a
`quick_fixes` slot in `LspReadProjectionIngest`, a projector from
`LspCodeActionMetadata`, and somewhere to hold the raw actions so a selected one
can be resolved back to its edit. Firing an unnarrowed code-action request and
taking "the first action with an edit" would build a proposal from an action the
user did not choose, so it was not done. Organize-imports is exempt from this
because narrowing to one kind makes "the first editable action" the right one.

**No live-server run of the write-side path.** The tests inject results through
the real drain, which proves routing, translation and proposal registration. A
rust-analyzer rename through GP-1 is separate work.

**Reversal after a save is not this task's mechanism.** Undo reverses the buffer;
once a rename has been saved, the reversal path is local history and the save's
own checkpoint, not the rename proposal's.
