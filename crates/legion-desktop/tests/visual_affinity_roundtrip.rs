//! Full-frame visual caret affinity round trips through the desktop route.
//!
//! The pointer positions in these tests come from the actual rendered egui
//! `Galley` shape emitted by the frame. No fixed-cell approximation is used.

use std::fs;
use std::sync::Arc;

use legion_desktop::{
    bridge::DesktopAction,
    view::editor_cursor_rect_for_galley_with_affinity,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};
use legion_protocol::{CaretAffinity, LineWrappingPolicy, TextCoordinate};

const WRAPPED_LINE: &str = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty";

fn frame(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1440.0, 900.0),
        )),
        events,
        ..egui::RawInput::default()
    }
}

fn open_app(text: &str) -> (tempfile::TempDir, DesktopEframeApp) {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let file = workspace.path().join("visual-affinity.txt");
    fs::write(&file, text).expect("fixture");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open fixture");
    (workspace, DesktopEframeApp::new(runtime))
}

fn configure_wrapping(app: &mut DesktopEframeApp) {
    app.handle_action(DesktopAction::SetLineWrappingPolicy {
        policy: LineWrappingPolicy::FixedColumn,
        wrap_column: Some(12),
    })
    .expect("wrapping policy should route through app authority");
}

fn click(app: &mut DesktopEframeApp, position: egui::Pos2) -> egui::FullOutput {
    let _ = app.run_headless_full_frame(frame(vec![egui::Event::PointerMoved(position)]));
    let _ = app.run_headless_full_frame(frame(vec![
        egui::Event::PointerMoved(position),
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]));
    let _ = app.run_headless_full_frame(frame(vec![
        egui::Event::PointerMoved(position),
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]));
    app.run_headless_full_frame(frame(Vec::new()))
}

fn first_text_shape(
    output: &egui::FullOutput,
    text: &str,
) -> Option<(egui::Pos2, Arc<egui::Galley>)> {
    fn find(shape: &egui::epaint::Shape, text: &str) -> Option<(egui::Pos2, Arc<egui::Galley>)> {
        match shape {
            egui::epaint::Shape::Text(text_shape)
                if text_shape.galley.job.text == text
                    && text_shape.galley.job.wrap.max_width.is_finite() =>
            {
                Some((text_shape.pos, Arc::clone(&text_shape.galley)))
            }
            egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, text)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|clipped| find(&clipped.shape, text))
}

fn matching_shape_debug(output: &egui::FullOutput, prefix: &str) -> Vec<String> {
    fn collect(shape: &egui::epaint::Shape, prefix: &str, rows: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(text_shape)
                if text_shape.galley.job.text.starts_with(prefix) =>
            {
                rows.push(format!(
                    "text_len={} rows={} size={:?} wrap={:?} pos={:?}",
                    text_shape.galley.job.text.len(),
                    text_shape.galley.rows.len(),
                    text_shape.galley.size(),
                    text_shape.galley.job.wrap.max_width,
                    text_shape.pos
                ));
            }
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, prefix, rows);
                }
            }
            _ => {}
        }
    }
    let mut rows = Vec::new();
    for clipped in &output.shapes {
        collect(&clipped.shape, prefix, &mut rows);
    }
    rows
}

fn boundary_pointer(output: &egui::FullOutput, downstream: bool) -> (egui::Pos2, usize) {
    let (origin, galley) = first_text_shape(output, WRAPPED_LINE).unwrap_or_else(|| {
        panic!(
            "the actual code-line text shape should be rendered; candidates={:?}",
            matching_shape_debug(output, "alpha")
        )
    });
    assert!(
        galley.rows.len() >= 2,
        "fixture must wrap in the rendered galley: rows={} size={:?} job_wrap={:?}",
        galley.rows.len(),
        galley.size(),
        galley.job.wrap.max_width
    );
    let boundary = galley.rows[0].glyphs.len();
    assert!(boundary > 0 && boundary < WRAPPED_LINE.chars().count());
    let row = if downstream {
        &galley.rows[1]
    } else {
        &galley.rows[0]
    };
    let x = if downstream {
        row.pos.x + row.x_offset(0) + 0.25
    } else {
        row.pos.x + row.x_offset(row.glyphs.len()) - 0.25
    };
    (origin + egui::vec2(x, row.rect().center().y), boundary)
}

fn boundary_coordinate(boundary: usize) -> TextCoordinate {
    let byte = WRAPPED_LINE
        .chars()
        .take(boundary)
        .map(char::len_utf8)
        .sum::<usize>() as u64;
    let utf16 = WRAPPED_LINE
        .chars()
        .take(boundary)
        .map(char::len_utf16)
        .sum::<usize>() as u64;
    TextCoordinate {
        line: 0,
        character: byte as u32,
        byte_offset: Some(byte),
        utf16_offset: Some(utf16),
    }
}

fn primary_projection(app: &DesktopEframeApp) -> legion_protocol::ViewportProjection {
    app.runtime_snapshot()
        .active_buffer_projection
        .viewport
        .expect("active viewport should be projected")
}

#[test]
fn full_frame_click_round_trips_upstream_and_downstream_at_wrapped_boundary() {
    let (_workspace, mut app) = open_app(WRAPPED_LINE);
    configure_wrapping(&mut app);
    let initial = app.run_headless_full_frame(frame(Vec::new()));
    assert_eq!(
        app.runtime_snapshot()
            .settings_projection
            .editor
            .line_wrapping_policy,
        LineWrappingPolicy::FixedColumn
    );
    let (upstream_pointer, boundary) = boundary_pointer(&initial, false);
    let (downstream_pointer, downstream_boundary) = boundary_pointer(&initial, true);
    assert_eq!(boundary, downstream_boundary);

    let upstream_output = click(&mut app, upstream_pointer);
    let upstream = primary_projection(&app);
    assert_eq!(upstream.cursor, boundary_coordinate(boundary));
    assert_eq!(upstream.cursor_affinities, vec![CaretAffinity::Upstream]);
    let upstream_ime = upstream_output
        .platform_output
        .ime
        .expect("primary caret should publish IME geometry");

    let downstream_output = click(&mut app, downstream_pointer);
    let downstream = primary_projection(&app);
    assert_eq!(downstream.cursor, boundary_coordinate(boundary));
    assert_eq!(
        downstream.cursor_affinities,
        vec![CaretAffinity::Downstream]
    );
    let downstream_ime = downstream_output
        .platform_output
        .ime
        .expect("primary caret should publish downstream IME geometry");
    assert_ne!(
        upstream_ime.cursor_rect.top(),
        downstream_ime.cursor_rect.top(),
        "the same logical boundary must paint and publish on different wrapped rows"
    );
    assert!(
        downstream_ime.cursor_rect.top() > upstream_ime.cursor_rect.top(),
        "downstream affinity must publish the following wrapped row"
    );
}

#[test]
fn full_frame_primary_affinity_round_trip_survives_secondary_caret_on_later_row() {
    let (_workspace, mut app) = open_app(&format!("{WRAPPED_LINE}\nsecondary"));
    configure_wrapping(&mut app);
    let initial = app.run_headless_full_frame(frame(Vec::new()));
    let (_, boundary) = boundary_pointer(&initial, true);
    app.handle_action(DesktopAction::SetCursor {
        buffer_id: None,
        cursor: boundary_coordinate(boundary),
    })
    .expect("primary setup cursor should route through authority");
    app.handle_action(DesktopAction::AddCursorBelow { buffer_id: None })
        .expect("secondary setup cursor should route through authority");
    let settled = app.run_headless_full_frame(frame(Vec::new()));
    let viewport = primary_projection(&app);
    assert_eq!(viewport.cursors.len(), 2);
    assert_eq!(
        viewport.cursor_affinities,
        vec![CaretAffinity::Upstream, CaretAffinity::Upstream]
    );
    assert_eq!(viewport.cursors[0], boundary_coordinate(boundary));
    assert_eq!(viewport.cursor.line, 0);
    let ime = settled
        .platform_output
        .ime
        .expect("primary caret should own IME geometry with secondary present");
    let (primary_origin, primary_galley) =
        first_text_shape(&settled, WRAPPED_LINE).expect("primary line shape should be rendered");
    let expected_primary = editor_cursor_rect_for_galley_with_affinity(
        &legion_desktop::view::DesktopCodeLineViewModel {
            number: 1,
            text: WRAPPED_LINE.to_string(),
            highlights: Vec::new(),
            truncation_state: legion_protocol::ViewportLineTruncationState::None,
            byte_range: legion_protocol::ByteRange::new(0, WRAPPED_LINE.len() as u64),
            utf16_range: legion_protocol::Utf16Range {
                start: legion_protocol::Utf16Position {
                    line: 0,
                    character: 0,
                },
                end: legion_protocol::Utf16Position {
                    line: 0,
                    character: WRAPPED_LINE.encode_utf16().count() as u32,
                },
            },
            line_start_byte_offset: Some(0),
            logical_end_byte: Some(WRAPPED_LINE.len() as u64),
            line_start_utf16_offset: Some(0),
        },
        &primary_galley,
        primary_origin,
        boundary,
        CaretAffinity::Upstream,
    );
    assert!((ime.cursor_rect.top() - expected_primary.top()).abs() < 0.1);
    let (secondary_origin, secondary_galley) =
        first_text_shape(&settled, "secondary").expect("secondary later-row shape should render");
    assert!(
        ime.cursor_rect.top() < secondary_origin.y + secondary_galley.rows[0].rect().top(),
        "primary IME row must remain ahead of the later secondary caret row"
    );
}
