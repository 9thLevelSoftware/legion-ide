//! Full-frame regressions for ordinary vertical caret movement.
//!
//! These tests deliberately drive `DesktopEframeApp::run_headless_full_frame`
//! so the event crosses the same egui and desktop keyboard route as the
//! product.  Setup actions establish valid editor state; only the movement
//! itself is asserted from real `egui::Event::Key` input.

use std::fs;

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};
use legion_protocol::LineWrappingPolicy;
use legion_protocol::TextCoordinate;

fn key_frame(key: egui::Key, modifiers: egui::Modifiers) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1_440.0, 900.0),
        )),
        modifiers,
        events: vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..egui::RawInput::default()
    }
}

fn two_key_frame(first: egui::Key, second: egui::Key) -> egui::RawInput {
    let mut input = key_frame(first, egui::Modifiers::NONE);
    input.events.push(egui::Event::Key {
        key: second,
        physical_key: Some(second),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    input
}

fn press_key(app: &mut DesktopEframeApp, key: egui::Key, modifiers: egui::Modifiers) {
    let _ = app.run_headless_full_frame(key_frame(key, modifiers));
    // Actions are applied at the end of the input frame; settle through the
    // same full-frame route before reading the projection.
    let _ = app.run_headless_full_frame(key_frame_neutral());
}

fn key_frame_neutral() -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1_440.0, 900.0),
        )),
        ..egui::RawInput::default()
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

fn open_app(text: &str) -> (tempfile::TempDir, DesktopEframeApp) {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("vertical.txt");
    fs::write(&file, text).expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");
    let app = DesktopEframeApp::new(runtime);
    (workspace, app)
}

#[test]
fn full_frame_vertical_move_retains_preferred_column_across_short_line() {
    let (_workspace, mut app) = open_app("0123456789\nab\n0123456789\n0123456789");
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(0, 8, 8, 8),
    })
    .expect("initial long-line caret should be valid");
    let before = viewport(&app);
    assert_eq!(
        before.cursor.line, 0,
        "precondition must start on first line"
    );
    assert_eq!(
        before.cursor.character, 8,
        "precondition must start at column 8"
    );

    press_key(&mut app, egui::Key::ArrowDown, egui::Modifiers::NONE);
    let clamped = viewport(&app);
    assert_eq!((clamped.cursor.line, clamped.cursor.character), (1, 2));

    press_key(&mut app, egui::Key::ArrowDown, egui::Modifiers::NONE);
    let retained = viewport(&app);
    assert_eq!(
        (retained.cursor.line, retained.cursor.character),
        (2, 8),
        "Down Down must restore the original preferred column after a short line"
    );

    press_key(&mut app, egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert_eq!(
        (viewport(&app).cursor.line, viewport(&app).cursor.character),
        (1, 2)
    );
    press_key(&mut app, egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert_eq!(
        (viewport(&app).cursor.line, viewport(&app).cursor.character),
        (0, 8),
        "Up Up must return to the original preferred column"
    );
}

#[test]
fn full_frame_vertical_move_preserves_ordered_carets_columns_and_shift_anchors() {
    let (_workspace, mut app) = open_app("abcdefghij\nxy\nabcdefghij\nabcdefghij");
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(0, 8, 8, 8),
    })
    .expect("initial long-line caret should be valid");
    app.handle_action(DesktopAction::AddCursorBelow { buffer_id: None })
        .expect("second caret should be added below");

    let before = viewport(&app);
    assert_eq!(before.cursors.len(), 2, "precondition requires two carets");
    assert_eq!(
        before
            .cursors
            .iter()
            .map(|caret| (caret.line, caret.character))
            .collect::<Vec<_>>(),
        vec![(0, 8), (1, 2)],
        "setup must establish two ordered carets with distinct columns"
    );

    press_key(&mut app, egui::Key::ArrowDown, egui::Modifiers::NONE);
    let moved = viewport(&app);
    assert_eq!(
        moved.cursors.len(),
        2,
        "plain Down must preserve both carets"
    );
    assert_eq!(
        moved
            .cursors
            .iter()
            .map(|caret| (caret.line, caret.character))
            .collect::<Vec<_>>(),
        vec![(1, 2), (2, 2)],
        "each caret must move down using its own starting column"
    );

    press_key(&mut app, egui::Key::ArrowUp, egui::Modifiers::NONE);
    let returned = viewport(&app);
    assert_eq!(
        returned.cursors.len(),
        2,
        "plain Up must preserve both carets"
    );
    assert_eq!(
        returned
            .cursors
            .iter()
            .map(|caret| (caret.line, caret.character))
            .collect::<Vec<_>>(),
        vec![(0, 8), (1, 2)],
        "Up must restore both caret rows and preferred columns"
    );

    press_key(&mut app, egui::Key::ArrowDown, egui::Modifiers::SHIFT);
    let selected = viewport(&app);
    assert_eq!(
        selected.cursors.len(),
        2,
        "Shift+Down must preserve both carets"
    );
    assert_eq!(
        selected.selections.len(),
        2,
        "Shift+Down must retain both anchors"
    );

    press_key(&mut app, egui::Key::ArrowUp, egui::Modifiers::SHIFT);
    let reversed = viewport(&app);
    assert_eq!(
        reversed.cursors.len(),
        2,
        "Shift+Up must preserve both carets"
    );
    assert_eq!(
        reversed.selections.len(),
        2,
        "reversing to both anchors must retain both directed selections"
    );
    for (selection, cursor) in reversed.selections.iter().zip(reversed.cursors.iter()) {
        assert_eq!(
            selection.start, *cursor,
            "reversed selection start must equal its head"
        );
        assert_eq!(
            selection.end, *cursor,
            "reversed selection end must equal its head"
        );
    }

    press_key(&mut app, egui::Key::ArrowDown, egui::Modifiers::SHIFT);
    let reextended = viewport(&app);
    assert_eq!(
        reextended.cursors.len(),
        2,
        "reverse extension must keep both carets"
    );
    assert_eq!(
        reextended.selections.len(),
        2,
        "reversing then extending must reuse both retained shift anchors"
    );
}

#[test]
fn full_frame_vertical_move_processes_two_raw_arrows_sequentially() {
    let (_workspace, mut app) = open_app("0123456789\nab\n0123456789");
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(0, 8, 8, 8),
    })
    .expect("initial caret should be valid");
    let _ = app.run_headless_full_frame(two_key_frame(egui::Key::ArrowDown, egui::Key::ArrowDown));
    let _ = app.run_headless_full_frame(key_frame_neutral());
    let after = viewport(&app);
    assert_eq!((after.cursor.line, after.cursor.character), (2, 8));
}

#[test]
fn full_frame_text_down_text_preserves_event_order_after_paint() {
    let (_workspace, mut app) = open_app("abc\ndef");
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(0, 3, 3, 3),
    })
    .expect("initial caret should be valid");
    let mut input = key_frame(egui::Key::ArrowDown, egui::Modifiers::NONE);
    input.events = vec![
        egui::Event::Text("X".to_owned()),
        egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: Some(egui::Key::ArrowDown),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Text("Y".to_owned()),
    ];
    let _ = app.run_headless_full_frame(input);
    let _ = app.run_headless_full_frame(key_frame_neutral());
    let after = viewport(&app);
    assert_eq!((after.cursor.line, after.cursor.character), (1, 4));
}

#[test]
fn full_frame_vertical_move_keeps_top_edge_caret_still_while_other_caret_moves() {
    let (_workspace, mut app) = open_app("top line\nsecond line\nthird line");
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(0, 3, 3, 3),
    })
    .expect("initial top-edge caret should be valid");
    app.handle_action(DesktopAction::AddCursorBelow { buffer_id: None })
        .expect("second caret should be added below");

    press_key(&mut app, egui::Key::ArrowUp, egui::Modifiers::NONE);
    let moved = viewport(&app);
    assert_eq!(
        moved
            .cursors
            .iter()
            .map(|caret| (caret.line, caret.character))
            .collect::<Vec<_>>(),
        vec![(0, 3), (0, 3)],
        "the edge caret should no-op while the interior caret moves upward"
    );
}

#[test]
fn full_frame_vertical_move_keeps_bottom_edge_caret_still_while_other_caret_moves() {
    let (_workspace, mut app) = open_app("first line\nsecond line\nlast line");
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(2, 2, 2, 2),
    })
    .expect("initial bottom-edge caret should be valid");
    app.handle_action(DesktopAction::AddCursorAbove { buffer_id: None })
        .expect("second caret should be added above");

    press_key(&mut app, egui::Key::ArrowDown, egui::Modifiers::NONE);
    let moved = viewport(&app);
    assert_eq!(
        moved
            .cursors
            .iter()
            .map(|caret| (caret.line, caret.character))
            .collect::<Vec<_>>(),
        vec![(2, 2), (2, 2)],
        "the edge caret should no-op while the interior caret moves downward"
    );
}

#[test]
fn full_frame_vertical_move_on_wrapped_first_line_moves_between_visual_rows() {
    let first = "x".repeat(5_000);
    let (_workspace, mut app) = open_app(&format!("{first}\nnext line"));
    app.handle_action(DesktopAction::SetLineWrappingPolicy {
        policy: LineWrappingPolicy::Viewport,
        wrap_column: None,
    })
    .expect("viewport wrapping should be enabled");
    let _ = app.run_headless_full_frame(key_frame_neutral());
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(0, 4_500, 4_500, 4_500),
    })
    .expect("wrapped first-line caret should be valid");

    press_key(&mut app, egui::Key::ArrowUp, egui::Modifiers::NONE);
    let moved = viewport(&app);
    assert_eq!(
        moved.cursor.line, 0,
        "movement stays on the first logical line"
    );
    assert!(
        moved.cursor.character < 4_500,
        "Up from a later wrapped row must move to the preceding visual row"
    );
}

#[test]
fn full_frame_vertical_move_selects_bounded_stops_on_long_complete_line() {
    let long = "x".repeat(5_000);
    let text = format!("{long}\nshort");
    let (_workspace, mut app) = open_app(&text);
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: cursor(0, 4_500, 4_500, 4_500),
    })
    .expect("long-line caret should be valid");
    press_key(&mut app, egui::Key::ArrowDown, egui::Modifiers::NONE);
    let moved = viewport(&app);
    assert_eq!((moved.cursor.line, moved.cursor.character), (1, 5));
}

#[test]
fn full_frame_input_timing_closes_after_resulting_snapshot_paint() {
    let (_workspace, mut app) = open_app("abc\ndef");
    let _ = app.run_headless_full_frame(key_frame_neutral());
    assert_eq!(app.frame_timing_summary_for_test().sample_count, 0);

    let _ = app.run_headless_full_frame(key_frame(egui::Key::ArrowDown, egui::Modifiers::NONE));
    // The input was handled after the old snapshot was painted.
    assert_eq!(app.frame_timing_summary_for_test().sample_count, 0);

    let _ = app.run_headless_full_frame(key_frame_neutral());
    assert_eq!(app.frame_timing_summary_for_test().sample_count, 1);
}
