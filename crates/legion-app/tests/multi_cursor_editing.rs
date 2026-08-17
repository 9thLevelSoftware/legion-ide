//! Multiple cursors through the real app dispatch path.
//!
//! The buffer has stored `Vec<Cursor>` and the projection has reported all of
//! them since long before this test existed — but nothing created a second and
//! every edit went to one place, so the capability was invisible. These check
//! the part that was missing rather than the part that already worked.

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{BufferId, PrincipalId, TextCoordinate, WorkspaceTrustState};
use legion_ui::CommandDispatchIntent;

fn app_with_text(text: &str) -> (AppComposition, BufferId) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("sample.rs");
    std::fs::write(&path, text).expect("fixture written");

    let mut app = AppComposition::new();
    app.open_workspace(
        dir.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("multi-cursor-test".to_string()),
    )
    .expect("workspace opens");
    app.open_file(path.to_string_lossy()).expect("file opens");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    std::mem::forget(dir);
    (app, buffer_id)
}

fn cursor_lines(app: &AppComposition, buffer_id: BufferId) -> Vec<(usize, usize)> {
    app.editor()
        .cursors(buffer_id)
        .expect("cursors")
        .iter()
        .map(|cursor| (cursor.position.line, cursor.position.column))
        .collect()
}

fn text_of(app: &AppComposition, buffer_id: BufferId) -> String {
    app.editor()
        .text(buffer_id)
        .expect("buffer text")
        .to_string()
}

fn add_below(app: &mut AppComposition, buffer_id: BufferId) {
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::AddCursorBelow { buffer_id })
        .expect("add cursor below dispatches");
    assert!(matches!(outcome, AppCommandOutcome::CursorSet(_)));
}

fn insert(app: &mut AppComposition, buffer_id: BufferId, text: &str) {
    app.dispatch_ui_intent(CommandDispatchIntent::Insert {
        buffer_id,
        at: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
        text: text.to_string(),
    })
    .expect("insert dispatches");
}

#[test]
fn adding_a_cursor_below_gives_two() {
    let (mut app, buffer_id) = app_with_text("one\ntwo\nthree\n");
    add_below(&mut app, buffer_id);
    assert_eq!(cursor_lines(&app, buffer_id), vec![(0, 0), (1, 0)]);
}

#[test]
fn adding_repeatedly_builds_a_column() {
    let (mut app, buffer_id) = app_with_text("one\ntwo\nthree\n");
    add_below(&mut app, buffer_id);
    add_below(&mut app, buffer_id);
    assert_eq!(
        cursor_lines(&app, buffer_id),
        vec![(0, 0), (1, 0), (2, 0)],
        "each press extends the set from every existing cursor"
    );
}

#[test]
fn typing_reaches_every_cursor() {
    let (mut app, buffer_id) = app_with_text("one\ntwo\nthree\n");
    add_below(&mut app, buffer_id);
    add_below(&mut app, buffer_id);
    insert(&mut app, buffer_id, "// ");
    assert_eq!(
        text_of(&app, buffer_id),
        "// one\n// two\n// three\n",
        "this is the entire point of multi-cursor"
    );
}

#[test]
fn a_single_cursor_still_takes_the_ordinary_path() {
    let (mut app, buffer_id) = app_with_text("one\ntwo\n");
    insert(&mut app, buffer_id, "X");
    assert_eq!(
        text_of(&app, buffer_id),
        "Xone\ntwo\n",
        "multi-cursor is an addition to editing, not a replacement, and the \
         common case must not be diverted through the rare branch"
    );
}

#[test]
fn clearing_collapses_to_one_without_moving_it() {
    let (mut app, buffer_id) = app_with_text("one\ntwo\nthree\n");
    add_below(&mut app, buffer_id);
    add_below(&mut app, buffer_id);
    app.dispatch_ui_intent(CommandDispatchIntent::ClearExtraCursors { buffer_id })
        .expect("clear dispatches");
    assert_eq!(
        cursor_lines(&app, buffer_id),
        vec![(0, 0)],
        "leaving a multi-cursor set must not also move the caret"
    );
}

#[test]
fn a_cursor_with_nowhere_to_go_does_not_duplicate() {
    let (mut app, buffer_id) = app_with_text("only\n");
    add_below(&mut app, buffer_id);
    add_below(&mut app, buffer_id);
    let cursors = cursor_lines(&app, buffer_id);
    assert_eq!(
        cursors.len(),
        cursors
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        "two cursors in one place would type every character twice: {cursors:?}"
    );
}

#[test]
fn a_multi_cursor_edit_is_one_undoable_change() {
    let (mut app, buffer_id) = app_with_text("one\ntwo\n");
    add_below(&mut app, buffer_id);
    insert(&mut app, buffer_id, "X");
    assert_eq!(text_of(&app, buffer_id), "Xone\nXtwo\n");

    app.dispatch_ui_intent(CommandDispatchIntent::Undo { buffer_id })
        .expect("undo dispatches");
    assert_eq!(
        text_of(&app, buffer_id),
        "one\ntwo\n",
        "one keystroke should take one undo to reverse, not one per cursor"
    );
}
