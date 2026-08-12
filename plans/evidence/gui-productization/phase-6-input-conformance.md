# GUI Phase 6 input conformance evidence

## Status

status: passed
scope: keyboard map, selection routing, multi-cursor projection, and mouse-selection helpers

## Commands

- `cargo test -p legion-desktop workflow::tests::editor_keyboard_control_actions_move_cursor_and_extend_selection -- --nocapture`: passed, 1 test.
- `cargo test -p legion-editor engine_preserves_multiple_cursors_and_selections_in_projection -- --nocapture`: passed, 1 test.
- `cargo test -p legion-desktop --test projection_rendering -- --nocapture`: passed, 18 tests.
- `cargo test -p legion-desktop --test daily_editing_controls -- --nocapture`: passed, 4 tests.

## Coverage Notes

- Keyboard navigation now has scripted coverage for cursor movement, shift-selection extension, and page scrolling in `crates/legion-desktop/src/workflow.rs`.
- Multi-cursor and multi-selection state is preserved and projected through `EditorEngine::viewport_projection` in `crates/legion-editor/src/lib.rs`.
- Mouse selection helpers remain covered by the existing `projection_rendering` suite, including word and line range calculations and drag-selection anchoring.

## Residual Risk

- This evidence validates the scripted input and projection helpers, not a full interactive GUI drag-and-drop session under a live windowing backend.

## IME + CJK addendum

### Status

status: partial
scope: IME composition routing, candidate positioning, CJK fallback fonts, and Tab suppression during active composition

### Commands

- `cargo test -p legion-desktop workflow::tests::editor_text_input_routes_text_clipboard_and_ime_commit -- --nocapture`: passed.
- `cargo test -p legion-desktop --lib workflow::tests -- --nocapture`: passed, 6 workflow tests.
- `cargo test -p legion-desktop`: passed, 17 unit tests plus 31 integration tests.

### Manual IME test script

Use the same steps on all three desktop targets, substituting the native IME provider available on that OS:

1. Open the desktop app and focus the code editor pane.
2. Switch to a CJK IME (macOS: system Pinyin/Japanese/Korean input; Windows: Microsoft IME or installed CJK IME; Linux: ibus/fcitx CJK input).
3. Start composition on a visible code line and confirm:
   - the candidate window appears anchored near the caret,
   - preedit/composition text renders inline in the editor,
   - Tab does not move focus or trigger editor shortcuts while composition is active,
   - Backspace edits the IME composition instead of deleting committed buffer text.
4. Commit the composition and confirm the resulting text is inserted at the projected cursor.
5. Repeat once with a non-Latin font-heavy string (for example: 漢字かなカナ) to confirm fallback glyph coverage.

### Notes

- The runtime font fallback now searches common host OS CJK font locations and appends the first match to egui's proportional and monospace font families.
- The tab-suppression workaround is intentionally local and documented against upstream egui IME issues so it can be removed when the backend behavior fully stabilizes.
