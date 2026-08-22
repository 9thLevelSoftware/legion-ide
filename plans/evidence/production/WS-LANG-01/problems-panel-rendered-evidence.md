# Renderer-backed Problems panel evidence — 2026-08-22

**Readiness row:** `PR-LANG-001` (Rust language workflow)
**Named gap this addresses:** "renderer-backed diagnostics panel UX evidence"
**Status change:** none. `PR-LANG-001` stays **Substrate validated**. See
"What this does not close" below.

## Why this file exists

`PR-LANG-001`'s ledger entry names two remaining promotion blockers. One is
3-OS real-server smoke. The other is renderer-backed diagnostics panel UX
evidence, and until now the product had none: diagnostics were covered at the
projection layer (`legion-app/tests/language_tooling_workflow.rs`,
`legion-desktop/tests/diagnostics_harness.rs`) and at the action layer
(`legion-desktop/tests/keyboard_nav.rs` — `ProblemNext`, `ProblemActivate`),
and nothing rendered a frame to check that a row appeared, where it appeared,
or what happened when it was clicked.

`crates/legion-desktop/tests/problems_panel_rendering.rs` (6 tests) closes that
by driving the real `DesktopEframeApp`, reading the real accessibility tree,
and clicking real coordinates.

## What the tests found

Writing them surfaced four defects that every existing diagnostics test passed
straight through, because none of them rendered anything.

### 1. A hover emptied the panel of every diagnostic in the workspace

`AppComposition`'s index-backed language read assigned its own `problems` list
straight into the projection. That leg runs on hover and completion, only ever
produces rows for the buffer being read, and produces none at all when the
index has nothing to say. So one hover replaced every LSP diagnostic in the
workspace with an empty list, and nothing republished them until that file's
server spoke again — the panel simply emptied while the errors were still in
the code.

`ingest_lsp_diagnostics` already states the rule for this shared multi-file
list: replace only the rows this producer owns for this file, leave the rest
alone. The read leg now follows the same rule. Guarded by
`a_language_read_for_another_buffer_leaves_the_problems_alone`, which fails with
`left: 0, right: 1` against the old code.

Two related resets — the ones that rebuild the projection when a read names a
different buffer — dropped `problems` for the same reason and now carry them,
via one shared `language_projection_for_new_identity`.

### 2. Every row was static text to assistive technology

The rows were `egui::Label::new(..).sense(Sense::click())`. egui publishes a
plain label as static text: no `Action::Click`, no focus. Every row in the panel
reached a screen reader — and anything else reading the accessibility tree — as
a sentence that could not be activated, while the mouse opened the file
perfectly. `clicking_a_rendered_problem_row_opens_the_file_at_its_line` found no
clickable node at all, which is exactly what a screen-reader user would have
found.

Rows are now `selectable_label`. That test also asserts the destination —
`t4_problem_activate_happy_path` only asserted `DesktopWorkflowOutcome::Opened`,
so a row that opened the wrong file at the wrong line passed it.

### 3. Keyboard selection was invisible to everything that asks

The focused row was marked by writing `\u{203a}` into the label text, which no
accessibility client can see, and which left every unselected row's name
beginning with two spaces. egui 0.34's `Button` (which now backs
`selectable_label`) reports itself with `WidgetInfo::labeled` and drops the
selected flag, so the row restates its own widget info to publish it. Guarded by
`the_rendered_panel_marks_the_keyboard_focused_row`, which reads `toggled`
rather than text.

### 4. Rows showed the Windows verbatim path prefix

Every row named its file as `\\?\C:\...`. `crate::path_display::display_path`
was written for exactly this and is already used by the breadcrumb and status
bar; the Problems panel was missed.

## What this does **not** close

**The panel still cannot tell a reader what is wrong.**
`legion_lsp::redacted_diagnostic_message` replaces every server message with a
per-severity placeholder, so a panel full of rustc errors reads
`Error <path>:<line> LSP error diagnostic` and nothing else. Severity is already
an icon and location is already a path, so the one field that would distinguish
a missing semicolon from a borrow-check failure is the one that is blank.

That redaction is deliberate and guarded:
`language_tooling_workflow::language_tooling_ingests_lsp_diagnostic_projection_and_preserves_lexical_rows`
feeds a diagnostic whose text embeds `SECRET_SOURCE_BODY` and asserts it does
not survive, because a server's message can quote the source it is complaining
about and ADR-0028 keeps raw source out of these records.

It is a real conflict between a stated privacy posture and a panel that cannot
do its job, and resolving it is a product decision about how much of a server's
prose may be shown on a local surface — not something to settle by deleting the
guard. `a_projected_diagnostic_is_visible_in_the_rendered_problems_panel` pins
the behaviour as it stands, in both directions, so a future change to the policy
has to be deliberate.

Until that is decided, `PR-LANG-001` should not move to product-workflow
validated on the strength of this file. The panel is now reachable, operable,
accessible and honest about location; it is not yet informative.

## Verification

- `cargo test -p legion-desktop --test problems_panel_rendering` — 6 pass
- `cargo test -p legion-app -p legion-lsp -p legion-desktop` — no failures
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo check -p legion-app --no-default-features` — clean
- All nine standing gates pass: `docs-hygiene`, `claim-audit`,
  `verify-kanban-backlog`, `verify-readiness-consistency`, `check-deps`,
  `extract-before-modify`, `intent-reachability`, `deferred-surfaces`,
  `no-egui-textedit`
