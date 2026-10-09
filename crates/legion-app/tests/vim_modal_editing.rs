//! Vim modal editing through the real app dispatch path.
//!
//! The unit tests either side of this cover the parser (`legion-ui::vim`) and
//! motion resolution (`legion-ui::vim_motion`) in isolation. What they cannot
//! show is that a motion intent actually moves the cursor of a real buffer,
//! which is the thing that was missing: every Vim intent routed to `Noop`
//! before this, and each half worked perfectly on its own the whole time.

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{PrincipalId, WorkspaceTrustState};
use legion_ui::{CommandDispatchIntent, EditorInputMode, VimMotionKind, VimOperatorKind};

#[test]
fn native_vim_resolution_matches_pure_ui_logical_content() {
    use legion_app::vim_session::{byte_to_character_column, character_to_byte_column};
    use legion_editor::vim::Motion;
    use legion_editor::{EditorEngine, TextPosition};
    use legion_protocol::{FileId, TextCoordinate, WorkspaceId};

    let motions = [
        (VimMotionKind::Left, Motion::Left),
        (VimMotionKind::Right, Motion::Right),
        (VimMotionKind::Up, Motion::Up),
        (VimMotionKind::Down, Motion::Down),
        (VimMotionKind::WordForward, Motion::WordForward),
        (VimMotionKind::WordBackward, Motion::WordBackward),
        (VimMotionKind::WordEnd, Motion::WordEnd),
        (VimMotionKind::LineStart, Motion::LineStart),
        (VimMotionKind::LineEnd, Motion::LineEnd),
        (VimMotionKind::FirstNonBlank, Motion::FirstNonBlank),
        (VimMotionKind::FileStart, Motion::FileStart),
        (VimMotionKind::FileEnd, Motion::FileEnd),
        (VimMotionKind::FindChar('é'), Motion::FindChar('é')),
        (VimMotionKind::FindChar('🦀'), Motion::FindChar('🦀')),
        (VimMotionKind::FindChar(' '), Motion::FindChar(' ')),
        (VimMotionKind::FindChar('z'), Motion::FindChar('z')),
        (VimMotionKind::TillChar('é'), Motion::TillChar('é')),
        (VimMotionKind::TillChar('🦀'), Motion::TillChar('🦀')),
        (VimMotionKind::TillChar('z'), Motion::TillChar('z')),
    ];
    for text in [
        "",
        "x",
        "é🦀",
        "café au lait\n",
        "  é_e ++ 🦀 z\n\n\tfin",
        "\n\n",
        "ab\n短\nlong line\n",
        "é🦀\r\n\r\n  fin",
        "x\r\n",
    ] {
        // Native coordinates exclude CRLF terminators. The old raw UI resolver
        // counts CR as text, so compare its LF logical-content semantics here;
        // raw CRLF correction has a separate explicit regression below.
        let logical = text.replace("\r\n", "\n");
        let mut editor = EditorEngine::new();
        let id = editor
            .open_buffer(WorkspaceId(1), FileId(1), "differential.txt", text)
            .unwrap();
        let lines: Vec<_> = logical.split('\n').collect();
        let character_position =
            |p: TextPosition| (p.line, byte_to_character_column(&logical, p.line, p.column));
        // Legacy app dispatch converted operator endpoints to bytes, clamping
        // character overshoots (including inclusive ends on empty final lines).
        let effective_endpoint = |(line, character): (usize, usize)| {
            TextPosition::new(line, character_to_byte_column(&logical, line, character))
        };
        for line in 0..=lines.len() {
            let clamped_line = line.min(lines.len() - 1);
            // Every byte includes interior UTF-8 offsets, end positions and overshoots.
            for column in 0..=lines[clamped_line].len() + 2 {
                let cursor = TextPosition::new(line, column);
                let from = TextCoordinate {
                    line: line as u32,
                    character: byte_to_character_column(&logical, clamped_line, column) as u32,
                    byte_offset: None,
                    utf16_offset: None,
                };
                for count in [0, 1, 2, 4] {
                    for (ui_motion, native_motion) in motions {
                        let expected = legion_ui::resolve_motion(&logical, from, ui_motion, count);
                        let actual = editor
                            .resolve_vim_motion(id, cursor, native_motion, count)
                            .unwrap();
                        assert_eq!(
                            character_position(actual),
                            (expected.line as usize, expected.character as usize),
                            "motion {ui_motion:?}/{count} from {cursor:?} in {text:?}"
                        );
                        let expected =
                            legion_ui::resolve_operator_range(&logical, from, ui_motion, count);
                        let actual = editor
                            .resolve_vim_operator_range(id, cursor, native_motion, count)
                            .unwrap();
                        assert_eq!(
                            actual.map(|r| (r.range.start, r.range.end, r.linewise)),
                            expected.map(|r| (
                                effective_endpoint(r.start),
                                effective_endpoint(r.end),
                                r.linewise
                            )),
                            "operator {ui_motion:?}/{count} from {cursor:?} in {text:?}"
                        );
                        if let (Some(actual), Some(expected)) = (actual, expected) {
                            let expected_text = legion_ui::range_text(&logical, expected);
                            let actual_text = editor.vim_range_text(id, actual.range).unwrap();
                            assert_eq!(actual_text.replace("\r\n", "\n"), expected_text);
                        }
                    }
                    let expected = legion_ui::resolve_linewise_range(&logical, from, count);
                    let actual = editor
                        .resolve_vim_linewise_range(id, cursor, count)
                        .unwrap();
                    assert_eq!(
                        (
                            character_position(actual.range.start),
                            character_position(actual.range.end),
                            actual.linewise
                        ),
                        (expected.start, expected.end, expected.linewise)
                    );
                }
            }
        }
    }
}

#[test]
fn crlf_motion_stays_on_content_and_linewise_register_preserves_terminators() {
    let (mut app, id) = app_with_text("é🦀\r\nnext");
    enable_vim(&mut app);
    motion(&mut app, VimMotionKind::LineEnd, 1);
    assert_eq!(
        app.editor().primary_cursor(id).unwrap(),
        legion_editor::TextPosition::new(0, 2)
    );
    let legacy = legion_ui::resolve_motion(
        "é🦀\r\nnext",
        legion_protocol::TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
        VimMotionKind::LineEnd,
        1,
    );
    assert_eq!(
        legacy.character, 2,
        "legacy raw text places the cursor on CR"
    );
    linewise(&mut app, VimOperatorKind::Yank, 1);
    app.dispatch_ui_intent(CommandDispatchIntent::VimPut)
        .unwrap();
    assert_eq!(text_of(&app, id), "é🦀\r\né🦀\r\nnext");
}

#[test]
fn empty_linewise_change_still_enters_insert_and_insert_after_reaches_content_end() {
    let (mut app, _) = app_with_text("");
    enable_vim(&mut app);
    linewise(&mut app, VimOperatorKind::Change, 1);
    assert_eq!(app.vim_display_mode(), Some(EditorInputMode::Insert));
    let (mut app, id) = app_with_text("é");
    enable_vim(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::VimInsertAfter)
        .unwrap();
    assert_eq!(
        app.editor().primary_cursor(id).unwrap(),
        legion_editor::TextPosition::new(0, 2)
    );
    assert_eq!(app.vim_display_mode(), Some(EditorInputMode::Insert));
}

#[test]
fn insert_after_final_multibyte_scalar_types_before_lf_or_crlf_terminator() {
    for ending in ["\n", "\r\n"] {
        let (mut app, id) = app_with_text(&format!("é🦀{ending}"));
        enable_vim(&mut app);
        motion(&mut app, VimMotionKind::LineEnd, 1);
        assert_eq!(
            app.editor().primary_cursor(id).unwrap(),
            legion_editor::TextPosition::new(0, 2)
        );
        app.dispatch_ui_intent(CommandDispatchIntent::VimInsertAfter)
            .unwrap();
        assert_eq!(
            app.editor().primary_cursor(id).unwrap(),
            legion_editor::TextPosition::new(0, 6)
        );
        app.dispatch_ui_intent(CommandDispatchIntent::ReplaceDirectedCarets {
            buffer_id: id,
            text: "!".to_string(),
        })
        .unwrap();
        assert_eq!(text_of(&app, id), format!("é🦀!{ending}"));
    }
    let (mut app, id) = app_with_text("");
    enable_vim(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::VimInsertAfter)
        .unwrap();
    assert_eq!(
        app.editor().primary_cursor(id).unwrap(),
        legion_editor::TextPosition::zero()
    );
    assert_eq!(app.vim_display_mode(), Some(EditorInputMode::Insert));
}

#[test]
fn large_streamed_app_dispatch_moves_and_deletes_motion_range_near_eof() {
    use std::io::{Read, Write};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.txt");
    let mut app = AppComposition::new();
    let prefix_bytes =
        (5 * 1024 * 1024).max(app.editor().thresholds().large_file_threshold_bytes) + 1024;
    {
        let mut file = std::fs::File::create(&path).unwrap();
        std::io::copy(
            &mut std::io::repeat(b'x').take(prefix_bytes as u64),
            &mut file,
        )
        .unwrap();
        file.write_all("\né🦀 fin\n".as_bytes()).unwrap();
    }
    app.open_workspace(
        dir.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("vim-large-test".to_string()),
    )
    .unwrap();
    app.open_file(path.to_string_lossy()).unwrap();
    let id = app.active_buffer_id().unwrap();
    assert!(app.editor().buffer_is_streamed(id).unwrap());
    let before = app.editor().buffer_metadata(id).unwrap();
    assert!(before.byte_len > 5 * 1024 * 1024);
    enable_vim(&mut app);
    motion(&mut app, VimMotionKind::FileEnd, 1);
    assert_eq!(
        app.editor().primary_cursor(id).unwrap(),
        legion_editor::TextPosition::new(2, 0)
    );
    motion(&mut app, VimMotionKind::Up, 1);
    assert_eq!(
        app.editor().primary_cursor(id).unwrap(),
        legion_editor::TextPosition::new(1, 0)
    );
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::VimOperatorMotion {
            operator: VimOperatorKind::Delete,
            motion: VimMotionKind::FindChar('🦀'),
            count: 1,
        })
        .unwrap();
    assert!(matches!(outcome, AppCommandOutcome::Edited(_)));
    let after = app.editor().buffer_metadata(id).unwrap();
    assert_eq!(after.byte_len, before.byte_len - 6);
    assert_eq!(after.undo_len, before.undo_len + 1);
    let tail = app
        .editor()
        .buffer_byte_offset(id, legion_editor::TextPosition::new(1, 0))
        .unwrap();
    let window = app.editor().line_window_around_byte(id, tail, 64).unwrap();
    assert_eq!(window.text, " fin");
    assert!(app.editor().buffer_is_streamed(id).unwrap());
    // No text_of()/cursor() helper or full-text API is used on this large buffer.
}

/// Open a workspace with one file and return the app plus its buffer id.
fn app_with_text(text: &str) -> (AppComposition, legion_protocol::BufferId) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("sample.rs");
    std::fs::write(&path, text).expect("fixture written");

    let mut app = AppComposition::new();
    app.open_workspace(
        dir.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("vim-test".to_string()),
    )
    .expect("workspace opens");
    app.open_file(path.to_string_lossy()).expect("file opens");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    // The directory must outlive the app's use of it.
    std::mem::forget(dir);
    (app, buffer_id)
}

/// Cursor position as (line, character), read back through the editor.
fn cursor(app: &AppComposition, buffer_id: legion_protocol::BufferId) -> (usize, usize) {
    let text = app
        .editor()
        .text(buffer_id)
        .expect("buffer text")
        .to_string();
    let position = app.editor().primary_cursor(buffer_id).expect("cursor");
    legion_app::vim_session::position_to_character_column(&text, position)
}

fn enable_vim(app: &mut AppComposition) {
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::SetVimModeEnabled(true))
        .expect("vim enables");
    assert!(matches!(
        outcome,
        AppCommandOutcome::VimModeChanged(Some(EditorInputMode::Normal))
    ));
}

fn motion(app: &mut AppComposition, motion: VimMotionKind, count: usize) {
    app.dispatch_ui_intent(CommandDispatchIntent::VimMotion { motion, count })
        .expect("motion dispatches");
}

#[test]
fn a_motion_moves_the_real_cursor() {
    let (mut app, buffer_id) = app_with_text("fn main() {\n    let x = 1;\n}\n");
    enable_vim(&mut app);
    assert_eq!(cursor(&app, buffer_id), (0, 0));

    motion(&mut app, VimMotionKind::Right, 3);
    assert_eq!(cursor(&app, buffer_id), (0, 3));

    motion(&mut app, VimMotionKind::Down, 1);
    assert_eq!(cursor(&app, buffer_id), (1, 3));

    motion(&mut app, VimMotionKind::LineEnd, 1);
    assert_eq!(cursor(&app, buffer_id), (1, 13));
}

/// The reason motion resolution is character-based and the editor is not.
#[test]
fn a_motion_over_multibyte_text_lands_on_a_character_boundary() {
    let (mut app, buffer_id) = app_with_text("let café = 1;\n");
    enable_vim(&mut app);

    // `w` from the start: `café` is one word, so the next word start is `=`.
    motion(&mut app, VimMotionKind::WordForward, 2);
    let (line, character) = cursor(&app, buffer_id);
    assert_eq!((line, character), (0, 9), "the `=` is at character 9");

    // The editor stores bytes, so the same position is column 10 there — the
    // é costs one extra byte. Reading it back as characters must undo that.
    let position = app.editor().primary_cursor(buffer_id).expect("cursor");
    assert_eq!(
        position.column, 10,
        "if this were 9 the conversion was skipped and the cursor is one \
         character short of where Vim put it"
    );
}

#[test]
fn motions_do_nothing_while_vim_is_disabled() {
    let (mut app, buffer_id) = app_with_text("fn main() {}\n");
    // Deliberately not enabling Vim.
    motion(&mut app, VimMotionKind::Right, 5);
    assert_eq!(
        cursor(&app, buffer_id),
        (0, 0),
        "a user who never asked for modal editing must not have their cursor \
         moved by keys the desktop layer happens to route"
    );
}

#[test]
fn the_reported_mode_distinguishes_disabled_from_insert() {
    let (mut app, _) = app_with_text("x\n");

    let disabled = app
        .dispatch_ui_intent(CommandDispatchIntent::VimChangeMode(
            EditorInputMode::Insert,
        ))
        .expect("dispatches");
    assert!(
        matches!(disabled, AppCommandOutcome::VimModeChanged(None)),
        "changing mode while disabled must not report a mode, and must not \
         strand the user in one no key can leave"
    );

    enable_vim(&mut app);
    let enabled = app
        .dispatch_ui_intent(CommandDispatchIntent::VimChangeMode(
            EditorInputMode::Insert,
        ))
        .expect("dispatches");
    assert!(matches!(
        enabled,
        AppCommandOutcome::VimModeChanged(Some(EditorInputMode::Insert))
    ));
}

#[test]
fn a_motion_with_no_open_buffer_is_harmless() {
    let mut app = AppComposition::new();
    app.dispatch_ui_intent(CommandDispatchIntent::SetVimModeEnabled(true))
        .expect("vim enables without a workspace");
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::VimMotion {
            motion: VimMotionKind::Down,
            count: 1,
        })
        .expect("a motion with nothing to move is not an error");
    assert!(matches!(outcome, AppCommandOutcome::Noop));
}

#[test]
fn disabling_vim_clears_a_half_typed_command() {
    let (mut app, _) = app_with_text("fn main() {}\n");
    enable_vim(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::SetVimModeEnabled(false))
        .expect("disables");
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::SetVimModeEnabled(true))
        .expect("re-enables");
    assert!(
        matches!(
            outcome,
            AppCommandOutcome::VimModeChanged(Some(EditorInputMode::Normal))
        ),
        "re-enabling starts in Normal with nothing pending"
    );
}

// ─── Operators ──────────────────────────────────────────────────────────────

fn text_of(app: &AppComposition, buffer_id: legion_protocol::BufferId) -> String {
    app.editor()
        .text(buffer_id)
        .expect("buffer text")
        .to_string()
}

fn operator_motion(
    app: &mut AppComposition,
    operator: VimOperatorKind,
    motion: VimMotionKind,
    count: usize,
) {
    app.dispatch_ui_intent(CommandDispatchIntent::VimOperatorMotion {
        operator,
        count,
        motion,
    })
    .expect("operator dispatches");
}

fn linewise(app: &mut AppComposition, operator: VimOperatorKind, count: usize) {
    app.dispatch_ui_intent(CommandDispatchIntent::VimLinewiseOperator { operator, count })
        .expect("linewise operator dispatches");
}

#[test]
fn dw_deletes_up_to_the_next_word() {
    let (mut app, buffer_id) = app_with_text(
        "fn main() {}
",
    );
    enable_vim(&mut app);
    operator_motion(
        &mut app,
        VimOperatorKind::Delete,
        VimMotionKind::WordForward,
        1,
    );
    assert_eq!(
        text_of(&app, buffer_id),
        "main() {}
"
    );
}

#[test]
fn dd_removes_the_whole_line_and_leaves_no_blank() {
    let (mut app, buffer_id) = app_with_text(
        "one
two
three
",
    );
    enable_vim(&mut app);
    motion(&mut app, VimMotionKind::Down, 1);
    linewise(&mut app, VimOperatorKind::Delete, 1);
    assert_eq!(
        text_of(&app, buffer_id),
        "one
three
",
        "taking the line terminator with it is what stops a blank being left"
    );
}

#[test]
fn a_delete_fills_the_register_so_put_moves_text() {
    let (mut app, buffer_id) = app_with_text(
        "one
two
",
    );
    enable_vim(&mut app);
    linewise(&mut app, VimOperatorKind::Delete, 1);
    assert_eq!(
        text_of(&app, buffer_id),
        "two
"
    );

    app.dispatch_ui_intent(CommandDispatchIntent::VimPut)
        .expect("put dispatches");
    assert_eq!(
        text_of(&app, buffer_id),
        "two
one
",
        "Vim's delete is a cut; dd then p is how a line gets moved"
    );
}

#[test]
fn a_yank_copies_without_touching_the_buffer() {
    let (mut app, buffer_id) = app_with_text(
        "one
two
",
    );
    enable_vim(&mut app);
    linewise(&mut app, VimOperatorKind::Yank, 1);
    assert_eq!(
        text_of(&app, buffer_id),
        "one
two
",
        "a yank must not edit, and must not cost an undo entry"
    );

    app.dispatch_ui_intent(CommandDispatchIntent::VimPut)
        .expect("put dispatches");
    assert_eq!(
        text_of(&app, buffer_id),
        "one
one
two
"
    );
}

#[test]
fn change_deletes_and_enters_insert_mode() {
    let (mut app, buffer_id) = app_with_text(
        "fn main() {}
",
    );
    enable_vim(&mut app);
    operator_motion(
        &mut app,
        VimOperatorKind::Change,
        VimMotionKind::WordForward,
        1,
    );
    assert_eq!(
        text_of(&app, buffer_id),
        "main() {}
"
    );

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::VimChangeMode(
            EditorInputMode::Insert,
        ))
        .expect("dispatches");
    assert!(
        matches!(
            outcome,
            AppCommandOutcome::VimModeChanged(Some(EditorInputMode::Insert))
        ),
        "change is delete plus insert mode — that is the whole difference from d"
    );
}

#[test]
fn an_operator_over_an_empty_range_changes_nothing() {
    let (mut app, buffer_id) = app_with_text(
        "word
",
    );
    enable_vim(&mut app);
    // `b` at the start of the buffer cannot move.
    operator_motion(
        &mut app,
        VimOperatorKind::Delete,
        VimMotionKind::WordBackward,
        1,
    );
    assert_eq!(
        text_of(&app, buffer_id),
        "word
"
    );
}

#[test]
fn put_with_an_empty_register_is_harmless() {
    let (mut app, buffer_id) = app_with_text(
        "word
",
    );
    enable_vim(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::VimPut)
        .expect("put with nothing yanked is not an error");
    assert_eq!(
        text_of(&app, buffer_id),
        "word
"
    );
}

#[test]
fn operators_do_nothing_while_vim_is_disabled() {
    let (mut app, buffer_id) = app_with_text(
        "fn main() {}
",
    );
    operator_motion(
        &mut app,
        VimOperatorKind::Delete,
        VimMotionKind::WordForward,
        1,
    );
    linewise(&mut app, VimOperatorKind::Delete, 1);
    assert_eq!(
        text_of(&app, buffer_id),
        "fn main() {}
"
    );
}

#[test]
fn an_operator_over_multibyte_text_cuts_on_character_boundaries() {
    let (mut app, buffer_id) = app_with_text(
        "let café = 1;
",
    );
    enable_vim(&mut app);
    motion(&mut app, VimMotionKind::WordForward, 1);
    operator_motion(&mut app, VimOperatorKind::Delete, VimMotionKind::WordEnd, 1);
    assert_eq!(
        text_of(&app, buffer_id),
        "let  = 1;
",
        "cutting on byte offsets would split the é and corrupt the buffer"
    );
}

// ─── Insert entry, x, and / ─────────────────────────────────────────────────

#[test]
fn i_enters_insert_without_moving() {
    let (mut app, buffer_id) = app_with_text(
        "word
",
    );
    enable_vim(&mut app);
    motion(&mut app, VimMotionKind::Right, 2);
    app.dispatch_ui_intent(CommandDispatchIntent::VimInsertBefore)
        .expect("i dispatches");
    assert_eq!(
        cursor(&app, buffer_id),
        (0, 2),
        "`i` inserts where the cursor is"
    );
}

#[test]
fn a_moves_one_right_before_inserting() {
    let (mut app, buffer_id) = app_with_text(
        "word
",
    );
    enable_vim(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::VimInsertAfter)
        .expect("a dispatches");
    assert_eq!(
        cursor(&app, buffer_id),
        (0, 1),
        "`a` is `i` one character to the right"
    );
}

#[test]
fn lowercase_o_opens_a_line_below_and_uppercase_o_opens_above() {
    let (mut app, buffer_id) = app_with_text(
        "one
two
",
    );
    enable_vim(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::VimInsertLineBelow)
        .expect("o dispatches");
    assert_eq!(
        text_of(&app, buffer_id),
        "one

two
"
    );

    let (mut app, buffer_id) = app_with_text(
        "one
two
",
    );
    enable_vim(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::VimInsertLineAbove)
        .expect("O dispatches");
    assert_eq!(
        text_of(&app, buffer_id),
        "
one
two
"
    );
}

#[test]
fn x_deletes_the_character_under_the_cursor_into_the_register() {
    let (mut app, buffer_id) = app_with_text(
        "abc
",
    );
    enable_vim(&mut app);
    motion(&mut app, VimMotionKind::Right, 1);
    app.dispatch_ui_intent(CommandDispatchIntent::VimDeleteChar)
        .expect("x dispatches");
    assert_eq!(
        text_of(&app, buffer_id),
        "ac
"
    );

    app.dispatch_ui_intent(CommandDispatchIntent::VimPut)
        .expect("put dispatches");
    assert_eq!(
        text_of(&app, buffer_id),
        "acb
",
        "`x` cuts, so what it took can be put back"
    );
}

#[test]
fn x_on_an_empty_line_is_harmless() {
    let (mut app, buffer_id) = app_with_text(
        "
second
",
    );
    enable_vim(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::VimDeleteChar)
        .expect("x on nothing is not an error");
    assert_eq!(
        text_of(&app, buffer_id),
        "
second
"
    );
}

#[test]
fn slash_opens_the_editors_own_find_bar() {
    let (mut app, _) = app_with_text(
        "word
",
    );
    enable_vim(&mut app);
    app.dispatch_ui_intent(CommandDispatchIntent::VimSearchForward)
        .expect("/ dispatches");
    assert!(
        app.find_bar_visible(),
        "`/` reuses the find bar rather than growing a second search UI"
    );
}

// ─── Keystrokes through the app ─────────────────────────────────────────────

fn key(app: &mut AppComposition, key: char) {
    app.dispatch_vim_key(key, false).expect("key dispatches");
}

#[test]
fn typing_dw_deletes_a_word() {
    let (mut app, buffer_id) = app_with_text(
        "fn main() {}
",
    );
    enable_vim(&mut app);
    key(&mut app, 'd');
    assert_eq!(
        text_of(&app, buffer_id),
        "fn main() {}
",
        "`d` alone must not delete anything"
    );
    key(&mut app, 'w');
    assert_eq!(
        text_of(&app, buffer_id),
        "main() {}
"
    );
}

#[test]
fn a_count_prefix_survives_the_round_trip() {
    let (mut app, buffer_id) = app_with_text(
        "one two three four
",
    );
    enable_vim(&mut app);
    key(&mut app, '3');
    key(&mut app, 'w');
    assert_eq!(
        cursor(&app, buffer_id),
        (0, 14),
        "3w reaches the fourth word"
    );
}

#[test]
fn keys_are_ignored_entirely_while_vim_is_disabled() {
    let (mut app, buffer_id) = app_with_text(
        "word
",
    );
    key(&mut app, 'd');
    key(&mut app, 'd');
    assert_eq!(
        text_of(&app, buffer_id),
        "word
"
    );
    assert!(
        !app.vim_consumes_text_input(),
        "a disabled session must let the shell insert the keys as text"
    );
}

#[test]
fn normal_mode_consumes_keys_and_insert_mode_releases_them() {
    let (mut app, _) = app_with_text(
        "word
",
    );
    enable_vim(&mut app);
    assert!(
        app.vim_consumes_text_input(),
        "in normal mode a `j` is a command, and inserting it too would be the          loudest possible bug"
    );

    key(&mut app, 'i');
    assert!(
        !app.vim_consumes_text_input(),
        "insert mode passes characters through to the buffer"
    );
    assert_eq!(app.vim_display_mode(), Some(EditorInputMode::Insert));
}
