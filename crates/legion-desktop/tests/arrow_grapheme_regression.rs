//! Headless regressions for native directed-caret movement.
//!
//! These tests were promoted after the S1-04e deletion commit, where they first
//! captured the historical RED behavior. They now verify the implemented
//! semantic route; they exercise the headless input path and do not qualify a
//! native OS window backend.

use std::fs;

use legion_desktop::bridge::DesktopAction;
use legion_desktop::workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime};
use legion_protocol::TextCoordinate;

fn key_event(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn key_frame(key: egui::Key, modifiers: egui::Modifiers) -> egui::RawInput {
    egui::RawInput {
        events: vec![key_event(key, modifiers)],
        modifiers,
        ..Default::default()
    }
}

fn cursor(line: u32, character: u32, byte_offset: u64, utf16_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(utf16_offset),
    }
}

fn viewport(app: &DesktopEframeApp) -> legion_protocol::ViewportProjection {
    app.runtime_snapshot()
        .active_buffer_projection
        .viewport
        .expect("active viewport should be projected")
}

#[test]
fn native_arrow_left_and_right_move_over_one_combining_grapheme() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("combining.txt");
    // `a` + COMBINING ACUTE is one grapheme and occupies three UTF-8 bytes.
    fs::write(&file, "a\u{301}x").expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");
    let mut app = DesktopEframeApp::new(runtime);

    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        // Explicit byte/UTF-16 coordinates: after `a` + combining acute, before x.
        cursor: cursor(0, 3, 3, 2),
    })
    .expect("initial combining-cluster cursor should be valid");
    assert_eq!(viewport(&app).cursor.byte_offset, Some(3));

    app.run_headless_input(key_frame(egui::Key::ArrowLeft, egui::Modifiers::NONE));
    let after_left = viewport(&app).cursor;
    assert_eq!(
        after_left.byte_offset,
        Some(0),
        "Left should cross the complete combining grapheme"
    );

    app.run_headless_input(key_frame(egui::Key::ArrowRight, egui::Modifiers::NONE));
    let after_right = viewport(&app).cursor;
    assert_eq!(
        after_right.byte_offset,
        Some(3),
        "Right should return to the byte boundary after the complete grapheme"
    );
}

#[test]
fn native_arrows_cross_zwj_flags_crlf_and_streamed_edges() {
    for (name, text, cluster_len) in [
        ("zwj.txt", "👩\u{200d}💻x", "👩\u{200d}💻".len()),
        ("flags.txt", "🇺🇸x", "🇺🇸".len()),
    ] {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let file = workspace.path().join(name);
        fs::write(&file, text).expect("fixture");
        let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
            workspace.path().to_path_buf(),
            Some(file.to_string_lossy().into_owned()),
        ))
        .expect("desktop runtime should open fixture");
        let mut app = DesktopEframeApp::new(runtime);
        app.handle_action(DesktopAction::SetCursor {
            buffer_id: None,
            cursor: cursor(
                0,
                text.len() as u32,
                text.len() as u64,
                text.encode_utf16().count() as u64,
            ),
        })
        .expect("cluster end cursor");
        assert_eq!(
            viewport(&app).cursor.byte_offset,
            Some(text.len() as u64),
            "cluster fixture must start at the supplied end coordinate"
        );
        app.run_headless_input(key_frame(egui::Key::ArrowLeft, egui::Modifiers::NONE));
        assert_eq!(viewport(&app).cursor.byte_offset, Some(cluster_len as u64));
        app.run_headless_input(key_frame(egui::Key::ArrowLeft, egui::Modifiers::NONE));
        assert_eq!(viewport(&app).cursor.byte_offset, Some(0));
    }

    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("crlf.txt");
    fs::write(&file, "a\r\nb").expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");
    let mut app = DesktopEframeApp::new(runtime);
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(1, 1, 4, 4),
    })
    .expect("crlf end cursor");
    app.run_headless_input(key_frame(egui::Key::ArrowLeft, egui::Modifiers::NONE));
    assert_eq!(viewport(&app).cursor.byte_offset, Some(3));
    app.run_headless_input(key_frame(egui::Key::ArrowLeft, egui::Modifiers::NONE));
    assert_eq!(viewport(&app).cursor.byte_offset, Some(1));

    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("large.txt");
    let mut large = "a".repeat(5 * 1024 * 1024 + 1);
    large.push('x');
    fs::write(&file, &large).expect("large fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open large fixture");
    let mut app = DesktopEframeApp::new(runtime);
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(
            0,
            large.chars().count() as u32,
            large.len() as u64,
            large.len() as u64,
        ),
    })
    .expect("large end cursor");
    app.run_headless_input(key_frame(egui::Key::ArrowLeft, egui::Modifiers::NONE));
    assert_eq!(
        viewport(&app).cursor.byte_offset,
        Some((large.len() - 1) as u64)
    );
}

#[test]
fn shift_left_then_shift_right_crosses_the_anchor_and_retains_zero_width_anchor() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("selection.txt");
    fs::write(&file, "abc").expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");
    let mut app = DesktopEframeApp::new(runtime);

    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(0, 3, 3, 3),
    })
    .expect("initial end cursor should be valid");
    assert_eq!(viewport(&app).cursors.len(), 1);

    let shift = egui::Modifiers::SHIFT;
    app.run_headless_input(key_frame(egui::Key::ArrowLeft, shift));
    let selected = viewport(&app);
    assert_eq!(selected.selections.len(), 1);
    assert_eq!(selected.selections[0].start.byte_offset, Some(2));
    assert_eq!(selected.selections[0].end.byte_offset, Some(3));

    app.run_headless_input(key_frame(egui::Key::ArrowRight, shift));
    let crossed = viewport(&app);
    assert_eq!(crossed.selections.len(), 1);
    assert_eq!(crossed.selections[0].start.byte_offset, Some(3));
    assert_eq!(crossed.selections[0].end.byte_offset, Some(3));
    assert_eq!(crossed.cursor.byte_offset, Some(3));

    app.run_headless_input(key_frame(egui::Key::ArrowLeft, shift));
    let reversed = viewport(&app);
    assert_eq!(reversed.selections[0].start.byte_offset, Some(2));
    assert_eq!(reversed.selections[0].end.byte_offset, Some(3));
}

#[test]
fn plain_horizontal_movement_collapses_forward_and_reversed_selections() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("collapse.txt");
    fs::write(&file, "abcd").expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");
    let mut app = DesktopEframeApp::new(runtime);

    app.handle_action(DesktopAction::SetDirectedSelection {
        buffer_id: None,
        anchor: cursor(0, 1, 1, 1),
        head: cursor(0, 3, 3, 3),
    })
    .expect("forward selection");
    app.run_headless_input(key_frame(egui::Key::ArrowLeft, egui::Modifiers::NONE));
    assert_eq!(viewport(&app).cursor.byte_offset, Some(1));

    app.handle_action(DesktopAction::SetDirectedSelection {
        buffer_id: None,
        anchor: cursor(0, 3, 3, 3),
        head: cursor(0, 1, 1, 1),
    })
    .expect("reversed selection");
    app.run_headless_input(key_frame(egui::Key::ArrowRight, egui::Modifiers::NONE));
    assert_eq!(viewport(&app).cursor.byte_offset, Some(3));
}

#[test]
fn ordered_left_text_right_events_refresh_each_projection() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("ordered.txt");
    fs::write(&file, "abc").expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");
    let mut app = DesktopEframeApp::new(runtime);
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(0, 2, 2, 2),
    })
    .expect("initial caret");
    app.run_headless_input(egui::RawInput {
        events: vec![
            key_event(egui::Key::ArrowLeft, egui::Modifiers::NONE),
            egui::Event::Text("X".to_string()),
            key_event(egui::Key::ArrowRight, egui::Modifiers::NONE),
        ],
        ..Default::default()
    });
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .viewport
            .expect("viewport")
            .cursor
            .byte_offset,
        Some(3)
    );
    assert_eq!(
        app.runtime_snapshot()
            .active_buffer_projection
            .small_buffer_text(),
        Some("aXbc")
    );
}

#[test]
fn repeated_horizontal_events_preserve_ordered_carets() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("repeated.txt");
    fs::write(&file, "abcd\nefgh").expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");
    let mut app = DesktopEframeApp::new(runtime);
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(0, 3, 3, 3),
    })
    .expect("initial caret");
    app.handle_action(DesktopAction::AddCursorBelow { buffer_id: None })
        .expect("second caret");
    app.run_headless_input(egui::RawInput {
        events: vec![
            key_event(egui::Key::ArrowLeft, egui::Modifiers::NONE),
            key_event(egui::Key::ArrowLeft, egui::Modifiers::NONE),
        ],
        ..Default::default()
    });
    let viewport = viewport(&app);
    assert_eq!(viewport.cursors.len(), 2);
    assert_eq!(viewport.cursors[0].byte_offset, Some(1));
    assert_eq!(viewport.cursors[1].byte_offset, Some(6));
}

#[test]
fn native_arrow_left_preserves_and_moves_all_carets() {
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
        cursor: cursor(0, 1, 1, 1),
    })
    .expect("initial first caret should be valid");
    app.handle_action(DesktopAction::AddCursorBelow { buffer_id: None })
        .expect("second caret should be added below");
    let before = viewport(&app);
    assert_eq!(before.cursors.len(), 2, "precondition requires two carets");
    assert_eq!(before.cursors[0].byte_offset, Some(1));
    assert_eq!(before.cursors[1].byte_offset, Some(4));

    app.run_headless_input(key_frame(egui::Key::ArrowLeft, egui::Modifiers::NONE));
    let after = viewport(&app);
    assert_eq!(
        after.cursors.len(),
        2,
        "ArrowLeft must preserve the complete ordered caret vector"
    );
    assert_eq!(after.cursors[0].byte_offset, Some(0));
    assert_eq!(after.cursors[1].byte_offset, Some(3));
}
