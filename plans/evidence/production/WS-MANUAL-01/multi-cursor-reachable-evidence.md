# P1.F3.T2 — multi-cursor, reachable and painted

Date: 2026-08-17
Readiness row: PR-UI-002
Task: P1.F3.T2 — row virtualization, gutter lanes, selection, cursor, multi-cursor projection, scroll behavior
Acceptance:
1. "Editor is usable via rendered desktop UI, not only API tests."
2. "A second cursor can be created, typed at, and cleared; every cursor is painted."

## What was already there

More than expected. `legion-editor::multi_cursor` had `normalize`,
`add_vertical`, `insert_at_all` and `delete_before_all` as pure functions;
`CommandDispatchIntent` had `AddCursorAbove`, `AddCursorBelow` and
`ClearExtraCursors`; `AppComposition` handled all three plus multi-cursor
insert; the viewport projection carried the full cursor set; and the renderer
already looped that set when painting. Eight tests in
`crates/legion-app/tests/multi_cursor_editing.rs` passed.

## What was missing

**A keyboard path.** There was no `DesktopAction`, no bridge translation and no
keybinding for any of the three intents. Multi-cursor worked, was tested, and
could not be used — the same shape as the explorer that would not open files,
the session state that never persisted, and the panel sizes that never
restored. Four instances of one pattern in a day: a capability complete
everywhere except where a person reaches it.

**Backspace.** Typing reached every cursor; deleting reached only the caret, so
a multi-cursor edit could be made and not unmade. `delete_before_all` existed in
`legion-editor` with **no callers anywhere**.

**Proof of painting.** The renderer looped the cursor set, but nothing asserted
it, and acceptance clause 2 says "every cursor is painted".

## What changed

- `DesktopAction::AddCursorAbove` / `AddCursorBelow` / `ClearExtraCursors`, each
  translated through `with_resolved_buffer` like the other editing actions —
  the keyboard has no buffer to name.
- Keymap entries `Ctrl+Alt+↑` / `Ctrl+Alt+↓`, matching the convention most
  editors use for stacking cursors down a column. Plain `Ctrl+↑/↓` is scroll in
  many of them and Alt alone is the Windows menu key. `ArrowUp`/`ArrowDown`
  added to `key_label_to_egui`, which had no arrow keys.
- `Esc` collapses a multi-cursor set, gated on there being more than one cursor
  so it stays available to the completion popup, the hover tooltip and Vim's
  mode exit. The completion popup is checked first for the same reason.
- `AppComposition::dispatch_multi_cursor_delete`, mirroring the insert path: one
  whole-buffer edit rather than N, because the positions are valid only against
  the text they were computed from. The incoming range is ignored deliberately —
  it was computed by the renderer from the active cursor alone.
- `docs/KEYBOARD_REFERENCE.md` gained all three bindings.

## Evidence

| Test | Claim |
| --- | --- |
| `ctrl_alt_down_adds_a_cursor_and_escape_clears_it` | The whole loop through the rendered app: bind → add → clear |
| `ctrl_alt_up_adds_a_cursor_above` | The upward binding resolves |
| `escape_with_one_cursor_is_left_for_other_handlers` | Escape is not swallowed when there is nothing to collapse |
| `projection_rendering_paints_every_cursor_in_a_multi_cursor_set` | Acceptance clause 2, measured on painted output |
| `backspace_reaches_every_cursor` | Delete reaches every cursor, and each steps back with its own deletion |
| 8 pre-existing tests in `multi_cursor_editing.rs` | Create, type, clear, undo-as-one-change, single-cursor path unaffected |

Both new load-bearing tests were vacuity-checked:

- Renaming the `AddCursorBelow` keymap label fails
  `ctrl_alt_down_adds_a_cursor_and_escape_clears_it`.
- Forcing the single-cursor arm in the painter fails
  `projection_rendering_paints_every_cursor_in_a_multi_cursor_set` with 5
  hairlines for both one and two cursors.

The painting test counts vertical hairlines as a **delta** between a one-cursor
and a two-cursor render, because the shell paints many of them (separators, icon
strokes) and the only thing differing between the two renders is the cursor
count. It pins `RawInput::time`, since the caret blinks and `paint_code_cursor`
returns early on the off phase — an unpinned clock would make the test pass or
fail by when it ran.

## Two false diagnoses worth recording

Both cost real time and both had the same cause: a scripted edit that silently
did nothing.

1. A probe inserted into `paint_code_cursor` never matched its anchor, so it
   produced no output — which read as "the painter is never called". It was
   being called all along. Python's `str.replace` returns the original string
   when the pattern is absent; the later probes assert insertion instead.
2. The test seeded cursors with
   `if let Some(viewport) = snapshot.…viewport.as_mut()`. `populated_snapshot()`
   carries **no** viewport, so the seeding silently did nothing and the test
   measured an unmodified fixture. It now uses `degraded_snapshot()` — the
   fixture that does carry one — and `expect`s the viewport rather than
   conditionally mutating it.

The general rule this earns: a conditional mutation in a test is a test that can
silently assert nothing.

## Status

Both acceptance clauses now hold, the second measured on painted output rather
than on the projection that feeds it. Clause 1 was already satisfied by the
2026-08-17 dogfood session
(`../../dogfood/2026-08-17-interactive-gui-journal.md`).

Row virtualization, gutter lanes, selection and scroll behavior — the rest of
the task's title — were already in place and are not re-litigated here.
