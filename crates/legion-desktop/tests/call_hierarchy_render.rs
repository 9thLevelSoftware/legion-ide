//! Call-hierarchy results have to be legible on screen, not just projected.
//!
//! `LanguageToolingProjection::call_hierarchy` reuses the row type the
//! reference list uses, so a row carries a file position and nothing that says
//! whether it is a caller or a callee. These tests render the real shell and
//! read the text it actually painted, because the two failure modes that matter
//! here — rows that never reach a surface, and rows whose direction is invisible
//! — both look fine in a projection assertion.

use egui::epaint::Shape;
use legion_desktop::view::ProjectionView;
use legion_protocol::{
    CallHierarchyDirection, CanonicalPath, LanguageLocationProjection, ProtocolTextRange,
    TextCoordinate,
};
use legion_ui::{Shell, ShellProjectionSnapshot};

const SCREEN: egui::Vec2 = egui::vec2(1_440.0, 900.0);

fn coord(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: None,
        utf16_offset: None,
    }
}

fn call(id: &str, label: &str, line: u32, degraded: bool) -> LanguageLocationProjection {
    LanguageLocationProjection {
        location_id: id.to_string(),
        file_id: None,
        path: Some(CanonicalPath("/w/lib.rs".to_string())),
        range: Some(ProtocolTextRange {
            start: coord(line, 0),
            end: coord(line, 4),
        }),
        label: label.to_string(),
        degraded,
        schema_version: 1,
    }
}

fn snapshot_with(
    direction: CallHierarchyDirection,
    calls: Vec<LanguageLocationProjection>,
) -> ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Call hierarchy").projection_snapshot();
    snapshot.language_tooling_projection.call_hierarchy = calls;
    snapshot
        .language_tooling_projection
        .call_hierarchy_direction = Some(direction);
    snapshot
}

fn raw_input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        events,
        ..egui::RawInput::default()
    }
}

fn render(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &ShellProjectionSnapshot,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run_ui(raw_input(events), |ui| {
        let _ = view.render(ui, snapshot);
    })
}

/// Every string the frame actually rasterised.
///
/// Read from the paint shapes rather than the AccessKit tree: this is the text
/// a person sees, so a row that is projected but clipped, collapsed behind a
/// closed panel, or dropped by a row cap does not appear here.
fn painted_text(output: &egui::FullOutput) -> Vec<String> {
    fn collect(shape: &Shape, out: &mut Vec<String>) {
        match shape {
            Shape::Text(text) => out.push(text.galley.text().to_string()),
            Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &output.shapes {
        collect(&clipped.shape, &mut out);
    }
    out
}

fn button_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("the shell should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.label() == Some(label) && node.role() == egui::accesskit::Role::Button)
                .then(|| node.bounds())
                .flatten()
        })
        .unwrap_or_else(|| panic!("control `{label}` should have semantic bounds"));
    egui::pos2(
        ((bounds.x0 + bounds.x1) / 2.0) as f32,
        ((bounds.y0 + bounds.y1) / 2.0) as f32,
    )
}

/// Render the shell, open the Diagnostics surface, and return what it painted.
///
/// The rows live in the language section of the Diagnostics panel — the surface
/// that already lists reference and definition locations — and that panel is
/// behind a rail button, so the test opens it the way a person does instead of
/// reaching past the renderer into view state.
fn painted_with_diagnostics_open(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let primed = render(&ctx, &mut view, snapshot, Vec::new());
    let pos = button_center(&primed, "Diagnostics");
    let press = vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        },
    ];
    let _pressed = render(&ctx, &mut view, snapshot, press);
    let release = vec![egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::default(),
    }];
    let _released = render(&ctx, &mut view, snapshot, release);
    painted_text(&render(&ctx, &mut view, snapshot, Vec::new()))
}

fn painted_containing(painted: &[String], needle: &str) -> Vec<String> {
    painted
        .iter()
        .filter(|text| text.contains(needle))
        .cloned()
        .collect()
}

#[test]
fn incoming_calls_reach_the_screen_labelled_as_callers() {
    let painted = painted_with_diagnostics_open(&snapshot_with(
        CallHierarchyDirection::Incoming,
        vec![call("loc-a", "render_shell", 41, false)],
    ));

    let heading = painted_containing(&painted, "call hierarchy");
    assert_eq!(
        heading.len(),
        1,
        "the panel should paint one call-hierarchy heading; painted={painted:?}"
    );
    assert!(
        heading[0].contains("incoming") && heading[0].contains("caller"),
        "the heading must name the direction; got {}",
        heading[0]
    );
    let rows = painted_containing(&painted, "render_shell");
    assert_eq!(rows.len(), 1, "the call row should paint; got {rows:?}");
    assert!(
        rows[0].starts_with("caller "),
        "each row must carry its own direction; got {}",
        rows[0]
    );
}

#[test]
fn outgoing_calls_paint_differently_from_incoming_ones() {
    // Same rows, opposite question. If the painted text matched, the panel
    // would be answering "who calls this" and "what does this call" with the
    // same picture, which is the specific way this feature misleads.
    let calls = vec![call("loc-a", "render_shell", 41, false)];
    let incoming = painted_with_diagnostics_open(&snapshot_with(
        CallHierarchyDirection::Incoming,
        calls.clone(),
    ));
    let outgoing =
        painted_with_diagnostics_open(&snapshot_with(CallHierarchyDirection::Outgoing, calls));

    let incoming_rows = painted_containing(&incoming, "render_shell");
    let outgoing_rows = painted_containing(&outgoing, "render_shell");
    assert_ne!(incoming_rows, outgoing_rows);
    assert!(
        outgoing_rows[0].starts_with("callee "),
        "outgoing rows are callees; got {}",
        outgoing_rows[0]
    );
    let outgoing_heading = painted_containing(&outgoing, "call hierarchy");
    assert!(
        outgoing_heading[0].contains("outgoing"),
        "the heading must name the direction; got {}",
        outgoing_heading[0]
    );
}

#[test]
fn a_row_pointing_at_a_declaration_says_so_and_a_precise_one_does_not() {
    let painted = painted_with_diagnostics_open(&snapshot_with(
        CallHierarchyDirection::Incoming,
        vec![
            call("loc-a", "precise_caller", 41, false),
            call("loc-b", "guessed_caller", 12, true),
        ],
    ));

    let degraded = painted_containing(&painted, "guessed_caller");
    assert_eq!(degraded.len(), 1, "painted={painted:?}");
    assert!(
        degraded[0].contains("declaration site"),
        "a degraded row points at the declaration, not the call; got {}",
        degraded[0]
    );

    let precise = painted_containing(&painted, "precise_caller");
    assert_eq!(precise.len(), 1, "painted={painted:?}");
    assert!(
        !precise[0].contains("declaration site"),
        "a precise row must not be marked as a guess; got {}",
        precise[0]
    );
}

#[test]
fn an_empty_result_still_says_which_question_was_asked() {
    // A query that found nothing is an answer. Painting nothing at all would be
    // indistinguishable from never having asked.
    let painted =
        painted_with_diagnostics_open(&snapshot_with(CallHierarchyDirection::Incoming, Vec::new()));
    let heading = painted_containing(&painted, "call hierarchy");
    assert_eq!(heading.len(), 1, "painted={painted:?}");
    assert!(heading[0].contains("incoming"), "got {}", heading[0]);
}

#[test]
fn a_shell_with_no_call_hierarchy_query_paints_no_call_hierarchy_rows() {
    // Guards the assertions above from passing on a panel that unconditionally
    // prints a heading, and guards the shell from a permanent empty section.
    let snapshot = Shell::empty("Call hierarchy").projection_snapshot();
    let painted = painted_with_diagnostics_open(&snapshot);
    assert!(
        painted_containing(&painted, "call hierarchy").is_empty(),
        "painted={painted:?}"
    );
}
