//! Focused red regression for native Backspace grapheme semantics.

use std::fs;

use legion_desktop::bridge::DesktopAction;
use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};
use legion_protocol::TextCoordinate;

fn key_event(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }
}

fn key_frame(key: egui::Key) -> egui::RawInput {
    egui::RawInput {
        events: vec![key_event(key)],
        ..Default::default()
    }
}

#[test]
fn native_backspace_removes_combining_grapheme_as_one_unit() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("grapheme.txt");
    fs::write(&file, "a\u{301}").expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");

    let mut app = DesktopEframeApp::new(runtime);
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: TextCoordinate {
            line: 0,
            // Editor authority currently uses UTF-8 byte columns.
            character: 3,
            byte_offset: Some(3),
            utf16_offset: Some(2),
        },
    })
    .expect("set cursor after combining grapheme");
    let before = app.runtime_snapshot();
    assert_eq!(
        before
            .active_buffer_projection
            .viewport
            .as_ref()
            .unwrap()
            .cursor
            .character,
        3
    );
    app.run_headless_input(key_frame(egui::Key::Backspace));

    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some(""),
        "Backspace should remove the complete a+combining-mark grapheme"
    );
}

#[test]
fn native_forward_delete_at_multiple_carets_deletes_after_each_caret() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("multi.txt");
    fs::write(&file, "ab\ncd").expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");

    let mut app = DesktopEframeApp::new(runtime);
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: TextCoordinate {
            line: 0,
            character: 1,
            byte_offset: Some(1),
            utf16_offset: Some(1),
        },
    })
    .expect("set primary cursor");
    app.handle_action(DesktopAction::AddCursorBelow { buffer_id: None })
        .expect("add second cursor");
    let before = app.runtime_snapshot();
    assert_eq!(
        before
            .active_buffer_projection
            .viewport
            .as_ref()
            .unwrap()
            .cursors
            .len(),
        2
    );
    app.run_headless_input(key_frame(egui::Key::Delete));

    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("a\nc"),
        "forward Delete should remove the character after each caret"
    );
}

#[test]
fn native_backspace_on_large_buffer_remains_available_without_full_text_cache() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("large.txt");
    let mut contents = String::from("ab");
    contents.push_str(&"x".repeat(6 * 1024 * 1024));
    fs::write(&file, contents).expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");

    let mut app = DesktopEframeApp::new(runtime);
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: TextCoordinate {
            line: 0,
            character: 2,
            byte_offset: Some(2),
            utf16_offset: Some(2),
        },
    })
    .expect("set cursor");
    let before = app.runtime_snapshot();
    let before_viewport = before
        .active_buffer_projection
        .viewport
        .as_ref()
        .expect("large-buffer viewport");
    assert!(before_viewport.mode != legion_protocol::ViewportProjectionMode::Normal);
    assert!(
        before_viewport
            .line_slices
            .first()
            .expect("visible large-buffer line")
            .visible_text
            .starts_with("ab")
    );
    let before_version = before
        .active_buffer_projection
        .viewport
        .as_ref()
        .expect("large-buffer viewport")
        .buffer_version;
    app.run_headless_input(key_frame(egui::Key::Backspace));
    let after = app.runtime_snapshot();
    assert_ne!(
        after
            .active_buffer_projection
            .viewport
            .as_ref()
            .expect("large-buffer viewport after Backspace")
            .buffer_version,
        before_version
    );
    let after_viewport = after
        .active_buffer_projection
        .viewport
        .as_ref()
        .expect("large-buffer viewport after Backspace");
    assert!(
        after_viewport
            .line_slices
            .first()
            .expect("visible large-buffer line after Backspace")
            .visible_text
            .starts_with("ax")
    );
}
