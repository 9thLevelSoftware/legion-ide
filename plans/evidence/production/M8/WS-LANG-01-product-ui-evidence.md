# WS-LANG-01 Product UI Evidence — PKT-LSP-B (M8)

**Branch:** m8/lsp-read-ui
**Commit range:** 400396c..d731f63
**Date:** 2026-07-05 (fix round 2026-07-05)
**Session:** https://claude.ai/code/session_01HMw3X3iusfbbZhaWDm9Q4B

---

## Summary

PKT-LSP-B extended the WS-LANG-01 substrate foundation with a desktop product UI path.
Work is split into three test tiers captured below:

- **T6** — Completion popup desktop state machine (8 tests, all pass).
- **T7** — Hover tooltip + go-to-definition desktop state machine (7 tests, all pass).
- **T8** — Product composition smoke, `#[ignore]` gated; exercises `AppComposition`
  startup → diagnostics → completion projection → stale discard via `is_stale_response`.

The pre-existing `cargo run -p xtask -- rust-analyzer-smoke` xtask command runs the full
`legion-app::rust_analyzer_workflow` suite with `--ignored`, which now includes T8.

**Status:** Substrate-validated for the fixture path.  Product-ready claim is blocked on
real-server 3-OS smoke (deferred, same constraint as WS-LANG-01).  Write-side actions
(rename, code-action, format UI) are explicitly out of scope for this branch.

---

## Verification Table

### T6 — LSP completion popup desktop state machine

Command: `cargo test -p legion-desktop --test completion_popup`
CWD: `C:\Users\dasbl\RustroverProjects\legion-ide-lsp-b`
Start: 2026-07-04  End: 2026-07-04  Exit: 0

Trimmed output:

```
running 8 tests
test completion_dismiss_with_no_completions_is_noop ... ok
test completion_dismiss_closes_open_popup ... ok
test completion_next_with_no_completions_is_noop ... ok
test completion_next_wraps_around ... ok
test completion_prev_wraps_to_last ... ok
test completion_accept_with_no_completions_is_noop ... ok
test completion_accept_inserts_label_through_editor ... ok
test completion_popup_dismissed_on_tab_switch ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Contracts verified:
- `CompletionDismiss` with no data → Noop (no panic).
- `CompletionDismiss` closes open popup and calls `refresh_projection`.
- `CompletionNext` wraps from last → 0; `CompletionPrev` wraps from 0 → last.
- `CompletionAccept` inserts the selected label through editor authority → `Edited` outcome.
- `CompletionAccept` with no completions → Noop (guard condition).
- Tab switch dismisses stale popup (`completion_popup_dismissed_on_tab_switch`).
- Pre-sync of `last_completion_count` prevents re-open after dismiss.

---

### T7 — Hover tooltip + go-to-definition desktop state machine

Command: `cargo test -p legion-desktop --test hover_definition`
CWD: `C:\Users\dasbl\RustroverProjects\legion-ide-lsp-b`
Start: 2026-07-04  End: 2026-07-04  Exit: 0

Trimmed output:

```
running 7 tests
test hover_dismiss_with_no_hover_is_noop ... ok
test hover_dismiss_closes_open_tooltip ... ok
test hover_tooltip_shows_when_hover_data_arrives ... ok
test hover_tooltip_dismissed_on_tab_switch ... ok
test navigate_to_definition_with_no_definitions_is_noop ... ok
test go_to_definition_action_fires_language_tooling_request ... ok
test request_hover_action_fires_language_tooling_request ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Contracts verified:
- `HoverDismiss` with no data → Noop (no panic).
- `HoverDismiss` closes tooltip and pre-syncs `last_hover_id` so `refresh_projection`
  does not immediately re-open the same hover on the next frame.
- `set_hover_tooltip_visible_for_test(true)` syncs `last_hover_id` from current snapshot
  so tests leave consistent state.
- Tab switch clears `hover_tooltip_visible` but keeps `last_hover_id` so old tab's hover
  does not re-appear on the new tab.
- `hover_tooltip_shows_when_hover_data_arrives`: tooltip auto-shows when new hover id
  arrives and tooltip was not previously visible.
- `GoToDefinition` dispatches `LanguageToolingUpdated` through the bridge even without
  a live server.
- `NavigateToDefinition` with no definitions → Noop (guard condition).

---

### T8 — Product composition smoke (`#[ignore]`, requires rust-analyzer on PATH)

Command: `cargo run -p xtask -- rust-analyzer-smoke`
(internally: `cargo test -p legion-app --test rust_analyzer_workflow -- --ignored`)
CWD: `C:\Users\dasbl\RustroverProjects\legion-ide-lsp-b`
Gate: `#[ignore = "requires rust-analyzer on PATH; run with --ignored"]`
Test name: `rust_analyzer_product_composition_smoke`

Not run as part of this evidence capture (requires real rust-analyzer binary).
The test was added to `crates/legion-app/tests/rust_analyzer_workflow.rs` at commit
`d678395` and exercises:

1. Discovery — skip if `rust-analyzer` not on PATH.
2. `AppComposition::new()` + `open_workspace(WorkspaceTrustState::Trusted)`.
3. Drain LSP pump until `LspResultStatus::Fresh` or server-unavailable timeout.
4. `open_file()` to obtain an active `buffer_id`.
5. D2: `lsp_server_health_record()` is `Some` after a live startup.
6. D3: `language_tooling_projection.problems` is accessible.
7. `dispatch_ui_intent(CommandDispatchIntent::RequestCompletion { buffer_id, position })`
   + drain until completions arrive or timeout.
8. Stale discard: `is_stale_response(SnapshotId(1), SnapshotId(2)) == true`
   and `is_stale_response(SnapshotId(2), SnapshotId(2)) == false`.

For real-server evidence, run `cargo run -p xtask -- rust-analyzer-smoke` with
`rust-analyzer` available on PATH.  The existing WS-LANG-01 evidence
(`plans/evidence/production/WS-LANG-01/WS-LANG-01-evidence.md`) records a successful
real single-OS (Windows) smoke run covering the prior `rust_analyzer_full_workflow` test.

---

## Full legion-desktop test suite

Command: `cargo test -p legion-desktop`
CWD: `C:\Users\dasbl\RustroverProjects\legion-ide-lsp-b`
Start: 2026-07-04  End: 2026-07-04  Exit: 0
Result: All test suites pass; 0 failures.

(Full per-suite pass counts available in session output; targeted test counts above are
the load-bearing evidence for PKT-LSP-B.)

---

## Fix Round — 2026-07-05 (commit d731f63)

Fix-round addressing all findings from `lsp-b-review-report.md`.

### C1 — `unsafe set_var` removed

Both tests that previously called `unsafe { std::env::set_var }` now pass the mock
server path via `start_for_workspace_with_server_path(dir, true, Some(mock_path))`.
No environment mutation in the test process.

### I1 — Debounce state moved to `AppComposition`

Completion and hover debounce fields (`lsp_ui_completion_debounce`,
`lsp_ui_hover_debounce`, `lsp_ui_last_completion_count`, `lsp_ui_last_hover_id`)
moved from `DesktopRuntime` to `AppComposition`. Desktop now calls
`app.tick_lsp_debounces(Instant::now())` each frame and dispatches returned
`LspDebounceEvent` values. Methods added to `AppComposition`:
`arm_lsp_completion_debounce`, `disarm_lsp_completion_debounce`,
`arm_lsp_hover_debounce`, `disarm_lsp_hover_debounce`, `tick_lsp_debounces`,
`pre_sync_lsp_completion_count`, `pre_sync_lsp_hover_id`, `last_lsp_completion_count`,
`last_lsp_hover_id`.

### I2/T7 — Capability gating added

`lsp_server_supports_capability(capability: &str) -> bool` added to `AppComposition`
(fail-closed: empty capability list → `false`).  `issue_lsp_hover_request`,
`issue_lsp_definition_request`, and `issue_lsp_completion_request` now gate on the
respective capability before issuing.  Capabilities are parsed from the `initialize`
response JSON in `session.rs::initialize()`.

Verification: `cargo test -p legion-app --test app_lsp_composition`

```
running 17 tests
... (all ok)
test t7_capability_gated_requests_skip_when_unsupported ... ok
test t7_capability_gated_partial_support ... ok
test result: ok. 17 passed; 0 failed
```

### I3 — T3 edit→diagnostics cycle test

`t3_diagnostics_projection_add_then_clear_cycle` added to `app_lsp_composition.rs`.
Injects diagnostics, asserts non-empty projection, clears, asserts empty.

### I4 — T5 snapshot health-flow tests

- `t5_refused_health_in_snapshot`: asserts `Unavailable` record appears in
  `shell_projection_snapshot.language_tooling_projection.lsp_health_records` after
  a refused session.
- `t5_injected_live_health_in_snapshot`: asserts live health record (via
  `set_lsp_health_for_test`) appears in snapshot.

### T4 — Problems panel keyboard navigation

`ProblemNext`, `ProblemPrev`, `ProblemActivate` added to `DesktopAction` and wired in
`bridge.rs` and `workflow.rs`. `problems_selected_index: usize` tracks focused row in
`DesktopRuntime`; forwarded to `DesktopProjectionViewState` for rendering with `› ` prefix.

Verification: `cargo test -p legion-desktop --test keyboard_nav`

```
running 4 tests
test t4_problem_activate_with_no_problems_is_noop ... ok
test product_mode_switch_accepts_keyboard_activation ... ok
test t4_problem_next_increments_selection ... ok
test t4_problem_prev_decrements_selection ... ok
test result: ok. 4 passed; 0 failed
```

### M2 — Double-drain fixed

`assert!(!app.drain_lsp_session())` replaces the double-call assertion.

### M3 — `accept_completion` honors `insertText`

`insert_text: Option<String>` added to `LanguageCompletionProjection` (protocol DTO).
`completion_projection_for_item` in `legion-lsp` populates it from the LSP `insertText`
field when present and different from the label. `accept_completion` uses
`insert_text.as_deref().unwrap_or(&label)` for the inserted text.

### M4 — `lsp_health_rows` formatted-output test

`m4_lsp_health_rows_formatted_output` in `language_health_view.rs` injects a health
record via `AppComposition::set_lsp_health_for_test`, takes a snapshot, converts via
`DesktopProjectionViewModel::from_snapshot`, and asserts the row string contains
`lsp server=`, `provenance=`, `version=`, `status=`, `restarts=`, the injected version,
and "ready".

Verification: `cargo test -p legion-desktop --test language_health_view`

```
running 7 tests
... (all ok)
test m4_lsp_health_rows_formatted_output ... ok
test result: ok. 7 passed; 0 failed
```

### M5 — Delete/backspace re-arms completion debounce

`DesktopAction::DeleteRange { range }` added to `completion_debounce_info` match arm,
returning `Some(range.start)` so backspace/delete trigger debounce re-arm.

### Regression suite

- `cargo test -p legion-app --test app_lsp_composition` — 17/17 pass
- `cargo test -p legion-app --test rust_analyzer_session_handshake` — 2/2 pass
- `cargo test -p legion-desktop --test keyboard_nav` — 4/4 pass
- `cargo test -p legion-desktop --test language_health_view` — 7/7 pass
- `cargo test -p legion-desktop --test completion_popup` — 8/8 pass
- `cargo test -p legion-desktop --test hover_definition` — 7/7 pass
- `cargo test -p legion-protocol --test dto_contracts` — 111/111 pass
- `cargo test -p legion-lsp` — 8 pass, 1 ignored (live rust-analyzer smoke)

## Merged-tree standing-gate run (2026-07-05, branch m8/lsp-read-ui)

Context: main merged (SEARCH #39, GIT #40, containment #37, CI fixes #35/#38);
working directory C:/Users/dasbl/RustroverProjects/legion-ide-lsp-b; Windows
11; builds -j 4. Merge resolutions: legion-app Cargo.toml feature/dev-deps
union (both lanes independently converged on the identical test-helpers
design), AppComposition struct-field + constructor union (LSP session state +
palette usage repo), TROUBLESHOOTING.md section union, ledger PR-LANG row
union. Recovery note: the first merge commit briefly contained unresolved
markers in three files due to a shell-tooling failure masking git status —
caught by a repo-wide marker sweep before any push, resolved, amended.

| Gate | Result |
| --- | --- |
| cargo fmt --all --check | PASS |
| xtask check-deps / docs-hygiene / claim-audit / no-egui-textedit / verify-kanban-backlog | PASS |
| xtask release-pipeline --dry-run + verify-release-pipeline | PASS |
| cargo check --workspace --all-targets | PASS |
| cargo test --workspace --all-targets --no-fail-fast | PASS |
| cargo clippy --workspace --all-targets -- -D warnings | PASS (after while-let drain, collapsed if, boxed Live variant; app_lsp_composition re-run 17/17) |
| xtask perf-harness + verify-perf-harness | PASS |
| cargo deny check | PASS |
| xtask rust-analyzer-smoke | PASS (real rust-analyzer 1.95.0) |

---

## P2.F1.T3 closure — 2026-08-16

The task read "wire diagnostics to gutter/problems panel through desktop
harness," with the stop condition "stop if diagnostics are wired only to the app
API and not to the desktop harness." Three things had to be true, and only two
were.

**Acceptance — a real Rust diagnostic appears and clears after fixing.** GP-1
step s3 does exactly this against a real rust-analyzer, on a throwaway copy of
`fixtures/gp1-rust`:

```
cargo run -p xtask -- golden-path-1        # exit 0
  s3 passed (31716ms): error introduced, detected, fixed, cleared
```

**Stop condition — reached through the desktop harness, not just the app API.**
`crates/legion-desktop/tests/diagnostics_harness.rs::
desktop_runtime_projects_publish_diagnostics_and_clears_them_again` drives a
`DesktopRuntime`, not an `AppComposition`, so the projection is proven to reach
the surface that paints.

**The gutter had no test.** `paint_diagnostic_underlines` in
`crates/legion-desktop/src/view.rs` decided, per line, which characters a
diagnostic underlines — clamping multi-line ranges to the visible line, skipping
empty spans — and nothing exercised it. A painter cannot be tested without a
frame, so the decision was extracted to `diagnostic_underline_span(line_zero,
line_chars, range) -> Option<(u32, u32)>`, a pure function the painter now calls.
Five tests in `view.rs` cover it:

```
cargo test -p legion-desktop --lib
test view::tests::a_single_line_diagnostic_underlines_its_own_columns ... ok
test view::tests::a_diagnostic_on_another_line_underlines_nothing ... ok
test view::tests::a_multi_line_diagnostic_is_clipped_to_each_line_it_crosses ... ok
test view::tests::an_empty_span_is_not_painted ... ok
test view::tests::a_range_ending_at_the_start_of_a_later_line_does_not_underline_it ... ok
test result: ok. 79 passed; 0 failed
```

The clipping case is the one worth having: a range spanning lines 2–4 must
underline from column 6 to end-of-line on line 2, the whole of line 3, and
columns 0–3 on line 4. A range ending at character 0 of a later line underlines
nothing on that line, which is the off-by-one that would otherwise have painted a
stray mark under the first character of the line after an error.

**Workspace state at closure:** `cargo test --workspace --all-targets
--no-fail-fast` — 257 suites, 2963 passed, 0 failed. `cargo clippy --workspace
--all-targets -- -D warnings` — clean.

**Not claimed:** the underline geometry is measured in characters against a
fixed `char_width`, which is correct for the monospace fonts the editor ships
and wrong for proportional ones. That is the existing painter's assumption, not
a new one, and it is unchanged here.

---

## P2.F1.T4 closure — 2026-08-16

The task named eight features: completion, hover, definition, references,
symbols, inlay hints, code lenses, runnables. Acceptance: "each LSP feature is
reachable from the editor and is covered by a test." Stop condition: "do not
implement features that are not requested by the LSP server's capability set."

### What was and was not already true

Three of the eight — completion, hover, definition — reached the live language
server, via `issue_lsp_*_request`, gated on the capability the server advertised
at `initialize`. That is the pattern this task extends.

References and outline were reachable from the editor (`:references`,
`:outline`, and `DesktopAction::RefreshOutline`) but were answered entirely by
Legion's own `LexicalIndexer` — the language server was never asked. Inlay hints
and code lenses had a protocol DTO, a projector in `legion-lsp`, an ingest
method in `legion-app`, and no way whatsoever to ask for them: nothing in the
shell, the intent enum, or the desktop bridge could reach them.

### The extraction commit came first

`3146bef` moved 556 lines — `drain_lsp_session` through
`issue_lsp_rename_request_inner` — from `lib.rs` into
`crates/legion-app/src/language/lsp_reads.rs`, unchanged except for widening one
method to `pub(crate)`, which a child module calling into the crate root
requires. Cross-cutting rule 1. The feature diff below is readable because of it.

### What changed

- Four `LspReadKind` variants (`References`, `Outline`, `InlayHints`,
  `CodeLens`) and four `issue_lsp_*_request` methods, each gated on its own
  advertised capability — `referencesProvider`, `documentSymbolProvider`,
  `inlayHintProvider`, `codeLensProvider`.
- Drain-side routing for all four to the ingest methods that already existed.
- `:references` and `:outline` now ask the server as well as the index. The
  index answer still returns synchronously so the panel is never empty while the
  server thinks; the server's answer merges in on the next drain. This is
  exactly how completion, hover and definition already behaved.
- Two new intents, `RefreshInlayHints` and `RefreshCodeLenses`, reachable as
  `:inlayhints` and `:codelens` and as `DesktopAction`s.
- **Runnables were named wrongly.** rust-analyzer publishes Run and Debug as
  code lenses whose `command` is `rust-analyzer.runSingle` — a handle into
  rust-analyzer's private protocol, not a command. Putting the handle in a field
  called `command_label` makes the lens describe itself wrongly everywhere it is
  shown or written to the audit log. `runnable_command_line` now assembles the
  real invocation from the lens's `cargoArgs`/`executableArgs`, element by
  element, and marks the lens `lsp.codelens.runnable`, which is the marker
  activation gates on. A runnable command with no `cargoArgs` falls through to
  the ordinary path rather than advertising a Run action with nothing behind it.
  See "Not claimed" below: this fixes the naming, not the execution.

### Tests

```
cargo test -p legion-app --test app_lsp_composition        # 22 passed
  t4_new_reads_skip_when_the_server_does_not_advertise_them
  t4_new_reads_fire_only_for_their_own_advertised_capability
  t4_inlay_hint_range_runs_one_line_past_the_last
  t4_read_source_label_does_not_invent_a_server_name

cargo test -p legion-app --test lsp_read_drain_routing     # 6 passed
  a_references_result_lands_in_the_projection_as_locations
  a_document_symbol_result_lands_in_the_projection_as_an_outline
  an_inlay_hint_result_lands_in_the_projection_attributed_to_the_server
  a_code_lens_result_lands_in_the_projection_and_carries_its_command
  a_result_does_not_leak_into_a_feature_it_was_not_tagged_for
  the_new_refresh_intents_dispatch_and_are_recorded_as_their_own_operations

cargo test -p legion-ui --test lsp_read_commands           # 4 passed
cargo test -p legion-lsp --test code_lens_runnables        # 5 passed
```

Two of those are worth naming. `a_result_does_not_leak_into_a_feature_it_was_not_tagged_for`
feeds an outline-shaped payload under the references tag and asserts the outline
stays empty — without it, four tests each asserting only their own field would
still pass if the routing collapsed every kind into one ingest, which is the
failure mode that silently kills a feature.
`t4_new_reads_fire_only_for_their_own_advertised_capability` advertises three of
four capabilities and withholds the fourth, proving the gate reads its own key
rather than merely checking that some capability list is non-empty. That gate is
the stop condition.

### Workspace state at closure

`cargo test --workspace --all-targets --no-fail-fast` — 260 suites, 2982 passed,
0 failed. `cargo clippy --workspace --all-targets -- -D warnings` — clean.
`docs-hygiene`, `claim-audit`, `verify-kanban-backlog`,
`verify-readiness-consistency`, `check-deps`, `no-egui-textedit` — all exit 0.

### Not claimed

**Inlay hints are requested for the whole document, not the viewport.** The
editor does not plumb its visible line range down to this layer, so
`whole_document_utf16_range` asks for everything. That is correct and, on a
large file, wasteful — inlay hints are range-scoped by the server precisely so a
client need not pay for hints nobody can see. `issue_lsp_inlay_hint_request`
takes a range rather than computing one so that narrowing it later is a caller
change, not a redesign.

**No real-server run of the four new features.** They are covered by injected
worker results through the real drain path, which proves the routing; GP-1 does
not yet exercise inlay hints or runnables against a live rust-analyzer. Adding a
GP-1 stage for them is separate work.

**Runnables are projected, not executed.** `ActivateLanguageCodeLens` and the
test explorer both pass the lens's `command_label` to `TerminalWorkflow::launch`,
which spawns the configured shell from `effective_shell_command()` and uses the
label only for the projection message and the audit line. Activating a Run lens
opens a terminal; it does not run the test. Wiring real execution is separate
work, and its right shape is an argv vector handed to the process API without a
shell — not this string.

**The label is checked, not escaped.** Because a caller may one day run it,
`is_plain_command_argument` refuses any argument carrying a control character or
a shell metacharacter, and such a lens is not treated as runnable at all. A
refusal rather than an escape on purpose: escaping is a game the defender
eventually loses, and a real cargo argument contains none of those characters.
`bounded_lsp_label` truncates by byte length and does nothing else.

### Correction (2026-08-16, same day)

An earlier draft of this section said that without this change
`ActivateLanguageCodeLens` and the test explorer "would have launched the literal
string `rust-analyzer.runSingle` in a shell," and that the terminal policy gate
was "what mediates it." Both are wrong, and the error was mine — caught in
review. Nothing executes `command_label`; there is no such mediation because
there is no execution. The change fixes what a runnable lens is *called* and
recorded as. It does not make runnables run, and the paragraph above now says so.
