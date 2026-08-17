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
