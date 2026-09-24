use std::path::PathBuf;

use legion_app::AppComposition;
use legion_editor::TextPosition;
use legion_protocol::{PrincipalId, TextCoordinate, WorkspaceTrustState};
use legion_ui::CommandDispatchIntent;

fn coordinate(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: None,
        utf16_offset: None,
    }
}

fn workspace() -> (tempfile::TempDir, AppComposition, PathBuf, PathBuf) {
    let root = tempfile::tempdir().expect("temporary workspace");
    let source = root.path().join("source.rs");
    let target = root.path().join("target.rs");
    std::fs::write(&source, "fn source() {}\n").expect("source file");
    std::fs::write(&target, "a😀b\n").expect("unicode target file");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("navigation-test".to_string()),
    )
    .expect("open workspace");
    (root, app, source, target)
}

#[test]
fn invalid_open_path_position_restores_prior_tab_and_caret_but_keeps_target_open() {
    let (_root, mut app, source, target) = workspace();
    app.dispatch_ui_intent(CommandDispatchIntent::OpenPathAtPosition {
        path: source.to_string_lossy().into_owned(),
        position: coordinate(0, 3),
    })
    .expect("open source at valid position");
    let source_buffer = app.active_buffer_id().expect("source buffer");
    let source_file = app.active_file_id().expect("source file");
    let source_caret = app
        .editor()
        .primary_cursor(source_buffer)
        .expect("source caret");

    let result = app.dispatch_ui_intent(CommandDispatchIntent::OpenPathAtPosition {
        path: target.to_string_lossy().into_owned(),
        position: coordinate(8, 0),
    });
    assert!(result.is_err(), "line overflow must reject navigation");
    assert_eq!(app.active_buffer_id(), Some(source_buffer));
    assert_eq!(app.active_file_id(), Some(source_file));
    assert_eq!(
        app.editor()
            .primary_cursor(source_buffer)
            .expect("restored source caret"),
        source_caret
    );

    let result = app.dispatch_ui_intent(CommandDispatchIntent::OpenPathAtPosition {
        path: target.to_string_lossy().into_owned(),
        position: coordinate(0, 2),
    });
    assert!(
        result.is_err(),
        "interior UTF-16 surrogate must reject navigation"
    );
    assert_eq!(app.active_buffer_id(), Some(source_buffer));
    assert_eq!(app.active_file_id(), Some(source_file));
    assert_eq!(
        app.editor()
            .primary_cursor(source_buffer)
            .expect("restored source caret"),
        source_caret
    );

    let target_file = app
        .open_file(target.to_string_lossy())
        .expect("target remains retained as an open tab");
    let target_buffer = app
        .editor()
        .buffer_for_file(app.workspace_id().expect("workspace"), target_file)
        .expect("target buffer");
    assert_eq!(
        app.editor()
            .primary_cursor(target_buffer)
            .expect("target cursor after open"),
        TextPosition::new(0, 0)
    );

    app.dispatch_ui_intent(CommandDispatchIntent::OpenPathAtPosition {
        path: target.to_string_lossy().into_owned(),
        position: coordinate(0, 3),
    })
    .expect("valid coordinate after the UTF-16 surrogate");
    assert_eq!(app.active_file_id(), Some(target_file));
    assert_eq!(
        app.editor()
            .primary_cursor(target_buffer)
            .expect("target cursor"),
        TextPosition::new(0, 5)
    );
}
