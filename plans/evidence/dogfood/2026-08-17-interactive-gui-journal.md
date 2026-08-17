# Interactive GUI Dogfood Journal — 2026-08-17

## Session

- **Branch:** main
- **Commit SHA:** 28c4ee0 (+ uncommitted working tree; see "Changes made" below)
- **OS / Platform:** Microsoft Windows 11 Pro 10.0.26200
- **Build method:** `cargo build -p legion-desktop` → `cargo run -p legion-desktop -- D:\legion-ide`
- **Session type:** **windowed, human-driven.** This is the first entry in this
  directory that is not headless. Every prior journal
  (`2026-07-21-dogfood-journal.md`, `2026-07-21-phase1-floor-journal.md`,
  `2026-07-22-dap-b10-headless-journal.md`,
  `2026-07-22-preview-artifact-journal.md`) states explicitly that it is a
  source/document review with no GUI session.
- **Legion version / channel:** workspace 0.1.0 / pre-beta

## What happened

The owner launched the renderer against this repo and reported:

> "Nothing in the app seems to really work at this point. I can see files via
> the file explorer but cannot interact with them."

That is the finding. Legion built, launched, scanned the workspace, and drew a
correct file tree — and was not usable as an editor, because **clicking a file
did not open it.**

After the fixes below, a second windowed session by the same owner reported:

> "That time I was able to open a file, make changes to it, and save it, close
> it, reopen it to see changes from earlier."

Open → edit → save → close → reopen → changes present. That is the first
confirmed end-to-end editing loop through the rendered desktop UI on record.

The owner's remaining assessment of that session: *"The UI is goddamned
awful… unpolished, hard to decipher between editable fields, buttons, etc."*
Cosmetic and affordance work, not function. Tracked below, partly addressed.

## Checklist result

| # | Action | Pass? | Notes |
|---|--------|-------|-------|
| 1 | Open this repo; expand nested dirs | Yes | Tree correct; `target/` and `.git` excluded |
| 1a | **Click a file row; confirm it opens** | **No → Yes** | The defect. Fixed; see D1 |
| 2 | Edit a file; save; confirm dirty → clean | Yes (after D1) | Verified by the owner and by test |
| 3 | Focus BYOK field; type | Not exercised | |
| 4 | Terminal | Not exercised | Panel showed `status=disabled …`; see D5 |
| 5–13 | Assist / Delegate / Git / Debug / Sandbox | Not exercised | Manual editing was the whole scope |

## Defects found

Ordered by how badly each one blocked using the product.

**D1 — Clicking a file in the explorer did not open it.** The row dispatched
`SelectExplorerFile` → `RevealInExplorer`; the app set `active_file_id` and
rebuilt the tree projection. No buffer was ever opened. Quick-open
(`Ctrl+P`/`Ctrl+O`) worked, so the capability existed and only the mouse path
was missing — the path a person reaches for first. **Fixed**
(`workflow.rs`, `ActivateExplorerFile`); guarded by
`crates/legion-desktop/tests/explorer_activation.rs` (6 tests, one clicking the
real row through the accessibility tree). Removing the fix fails 3 of them.

**D2 — Clicking ⚙ Settings or ? Setup permanently stopped typing.**
`handle_keyboard` disabled editor input whenever *any* widget held egui keyboard
focus. egui gives focus to plain buttons, and a `Button` never surrenders it, so
one click discarded every subsequent keystroke — indefinitely, surviving the
overlay being closed, with nothing on screen to explain it. Recovery (clicking
the canvas, or Escape) existed but was undiscoverable. The guard's own comment
said it was for text fields; egui ships the exact predicate wanted,
`Context::text_edit_focused()`, next to the one that was used. **Fixed.**

**D3 — The unsaved-changes prompt rendered below the bottom of the window.** It
was appended to the central panel after the code canvas, which allocates all
remaining height. Measured at four window heights, Save sat exactly 16px below
the window edge every time — not a small-window problem. Because
`editor_input_enabled` is false while the prompt is active, raising it disabled
typing and put both escapes off-screen: a hard lock-up that looks like a hang.
**Fixed** — it is now a shell-level modal, and answers to Enter and Escape.

**D4 — "Save" in that prompt never closed the tab.** `SaveDirtyClose` issued a
bare `Save`. The file was written and the tab the user had asked to close stayed
open. A test asserted this behaviour, which is how it survived. **Fixed**; the
test now asserts the close, and the button is labelled "Save and close".

**D5 — Five of nine activity-rail icons rendered as `□`.** The rail used
`▤ ⌕ ⑂ ✓ ▷`; egui's bundled font set does not cover them, so the IDE's primary
navigation column was a stack of missing-glyph boxes — and *which* boxes depends
on the host font fallback, so it differs per machine. **Fixed** by painting the
icons as vectors (`view/rail_icons.rs`), which removes the font from the
question.

**D6 — Windows extended-length paths shown verbatim.** The breadcrumb bar and
status bar printed `\\?\D:\legion-ide\CHANGELOG.md`. **Fixed**
(`path_display.rs`); the breadcrumb now shows trailing segments.

**D7 — Developer telemetry rendered as product UI.** A strip above every buffer
printed `sticky headers <none> folding 0 ranges smooth scrolling` — a readout of
the settings struct. The terminal panel led with
`status=disabled visible=0 omitted=0 matches=0`. **Both removed/humanised.**

**D8 — The tab's `×` floated outside the tab.** Tab and close button were
independent widgets laid out side by side, so layout spacing fell between them.
**Fixed** — one frame, one unit.

## Still open

- **No way to close a file without saving.** The prompt offers Save and Cancel
  only; `grep` finds no discard path anywhere in app authority. Deliberately not
  papered over with a renderer-side button.
- **The status bar computes `flags`, `encoding`, `line_ending`, `language` and
  `connection` every frame and renders none of them**, and nothing on it is
  clickable.
- **General visual polish.** The owner's "hard to decipher between editable
  fields, buttons, etc." is only partly addressed: resting controls now carry a
  border, and the active tab no longer impersonates a focused text field, but
  this has not been re-reviewed in a windowed session.

## Why none of this was caught before

Two reasons, both structural:

1. **No test drove the rendered UI end to end.** The harness to do it already
   existed — `DesktopEframeApp::run_headless_full_frame` plus the accessibility
   tree gives real clicks at real coordinates — and was used only for keyboard
   and projection assertions. Every defect above is a property of *rendering and
   hit-testing*, which projection tests cannot see by construction.
2. **`INTERACTIVE-GUI-CHECKLIST.md` went straight from "expand nested dirs" to
   "edit a file"** and never named the step in between. An affordance that is
   never checked is an affordance that can rot. Row 1a has been added.

## Product-readiness impact

**No ledger rows flipped by this entry.** Roadmap 1.11 asks for ≥5 consecutive
days with no P0/P1 defects; this is one session that found eight defects, four
of them blocking. It is evidence that the dogfood gate is doing its job, not
evidence of passing it.

What it does settle is roadmap 1.3's open acceptance clause — *"editor is usable
via rendered desktop UI, not only API tests"* — which had no evidence either way
until today. The answer was no, and is now yes for the open/edit/save loop
specifically, on Windows, by owner report plus regression tests.

## Evidence

| Item | Path |
| --- | --- |
| Explorer activation tests | `crates/legion-desktop/tests/explorer_activation.rs` |
| Shell affordance regressions (D2, D3, D4, D8) | `crates/legion-desktop/tests/shell_affordances.rs` |
| Rail icon geometry tests | `crates/legion-desktop/src/view/rail_icons.rs` |
| Path display tests | `crates/legion-desktop/src/path_display.rs` |
| Checklist gap | `plans/evidence/dogfood/INTERACTIVE-GUI-CHECKLIST.md` row 1a |
