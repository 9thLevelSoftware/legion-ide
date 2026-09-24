use legion_desktop::view::{
    DesktopCodeLineViewModel, VisualNavigationGeometryError, VisualNavigationRowRequest,
    visual_navigation_rows_for_line, visual_navigation_selected_row_for_line,
    visual_navigation_source_row_for_line,
};
use legion_protocol::{
    ByteRange, CaretAffinity, Utf16Position, Utf16Range, ViewportLineTruncationState,
};

fn line(
    text: &str,
    origin: Option<u64>,
    truncation_state: ViewportLineTruncationState,
) -> DesktopCodeLineViewModel {
    DesktopCodeLineViewModel {
        number: 3,
        text: text.to_owned(),
        highlights: Vec::new(),
        truncation_state,
        byte_range: ByteRange::new(origin.unwrap_or(0), origin.unwrap_or(0) + text.len() as u64),
        utf16_range: Utf16Range {
            start: Utf16Position {
                line: 2,
                character: 0,
            },
            end: Utf16Position {
                line: 2,
                character: text.encode_utf16().count() as u32,
            },
        },
        line_start_byte_offset: origin,
        logical_end_byte: Some(origin.unwrap_or(0) + text.len() as u64),
        line_start_utf16_offset: None,
    }
}

fn with_ui<T>(mut f: impl FnMut(&egui::Ui) -> T) -> T {
    let context = egui::Context::default();
    let mut result = None;
    let _ = context.run_ui(egui::RawInput::default(), |ui| result = Some(f(ui)));
    result.expect("test panel should run")
}

#[test]
fn variable_unicode_widths_emit_only_valid_boundaries_and_row_local_x() {
    let model = line(
        "a\u{301}👩\u{200d}💻z",
        Some(40),
        ViewportLineTruncationState::None,
    );
    let valid = [0, 3, 10, 14, 15];
    let rows = with_ui(|ui| visual_navigation_rows_for_line(ui, &model, 240.0, &valid, 32))
        .expect("bounded Unicode line should shape");
    let stops = rows
        .iter()
        .flat_map(|row| row.stops.iter())
        .collect::<Vec<_>>();
    assert_eq!(stops.len(), valid.len());
    assert!(
        stops
            .windows(2)
            .all(|pair| pair[0].x.value <= pair[1].x.value
                || pair[0].position.byte_column > pair[1].position.byte_column)
    );
    assert!(
        stops
            .iter()
            .all(|stop| valid.contains(&stop.position.byte_column))
    );
}

#[test]
fn wrapped_absolute_boundary_has_upstream_and_downstream_stops() {
    let model = line(
        "one two three four",
        Some(100),
        ViewportLineTruncationState::None,
    );
    let valid = (0..=18).collect::<Vec<_>>();
    let rows = with_ui(|ui| visual_navigation_rows_for_line(ui, &model, 36.0, &valid, 128))
        .expect("line should wrap");
    assert!(rows.len() >= 2);
    let shared = rows
        .windows(2)
        .find_map(|pair| {
            (pair[0].end.byte_column == pair[1].start.byte_column)
                .then_some(pair[0].end.byte_column)
        })
        .expect("wrapped rows should share a byte boundary");
    let affinities = rows
        .iter()
        .flat_map(|row| row.stops.iter())
        .filter(|stop| stop.position.byte_column == shared)
        .map(|stop| stop.affinity)
        .collect::<Vec<_>>();
    assert!(affinities.contains(&CaretAffinity::Upstream));
    assert!(affinities.contains(&CaretAffinity::Downstream));
    let upstream = with_ui(|ui| {
        visual_navigation_source_row_for_line(
            ui,
            &model,
            36.0,
            shared,
            CaretAffinity::Upstream,
            &valid,
            128,
        )
    })
    .expect("upstream shared boundary should select prior row");
    let downstream = with_ui(|ui| {
        visual_navigation_source_row_for_line(
            ui,
            &model,
            36.0,
            shared,
            CaretAffinity::Downstream,
            &valid,
            128,
        )
    })
    .expect("downstream shared boundary should select following row");
    assert_ne!(upstream.row.row_index, downstream.row.row_index);
}

#[test]
fn partial_line_origin_is_preserved_and_source_x_is_selected_from_stop() {
    let mut model = line("βγδε", Some(90), ViewportLineTruncationState::Leading);
    model.byte_range = ByteRange::new(100, 100 + model.text.len() as u64);
    let source = with_ui(|ui| {
        visual_navigation_source_row_for_line(
            ui,
            &model,
            120.0,
            12,
            CaretAffinity::Upstream,
            &[10, 12, 14, 16, 18],
            32,
        )
    })
    .expect("partial fragment with absolute origin should shape");
    assert_eq!(source.row.logical_line, 2);
    assert_eq!(source.row.start.byte_column, 10);
    assert_eq!(source.row.row_index, None);
    assert_eq!(source.row.row_count, None);
    assert_eq!(
        source.source_x,
        source
            .row
            .stops
            .iter()
            .find(|stop| stop.position.byte_column == 12)
            .unwrap()
            .x
    );
}

#[test]
fn every_partial_edge_keeps_visual_row_completeness_unknown() {
    for truncation in [
        ViewportLineTruncationState::Leading,
        ViewportLineTruncationState::Trailing,
        ViewportLineTruncationState::Both,
    ] {
        let mut model = line("βγδε", Some(90), truncation);
        model.byte_range = ByteRange::new(100, 100 + model.text.len() as u64);
        let rows = with_ui(|ui| {
            visual_navigation_rows_for_line(ui, &model, 120.0, &[10, 12, 14, 16, 18], 32)
        })
        .expect("bounded partial fragment should shape");
        assert!(
            rows.iter()
                .all(|row| row.row_index.is_none() && row.row_count.is_none())
        );
    }
}

#[test]
fn missing_window_and_stop_budget_are_typed_failures() {
    let model = line("bounded", Some(0), ViewportLineTruncationState::None);
    let missing = with_ui(|ui| visual_navigation_rows_for_line(ui, &model, 120.0, &[99], 32));
    assert_eq!(missing, Err(VisualNavigationGeometryError::NeedMoreWindow));
    let mixed = with_ui(|ui| visual_navigation_rows_for_line(ui, &model, 120.0, &[0, 99], 32));
    assert_eq!(mixed, Err(VisualNavigationGeometryError::NeedMoreWindow));
    let budget = with_ui(|ui| visual_navigation_rows_for_line(ui, &model, 120.0, &[0, 1], 1));
    assert_eq!(
        budget,
        Err(VisualNavigationGeometryError::StopBudgetExceeded)
    );
    let missing_origin = line("bounded", None, ViewportLineTruncationState::Leading);
    let missing_origin =
        with_ui(|ui| visual_navigation_rows_for_line(ui, &missing_origin, 120.0, &[0], 32));
    assert_eq!(
        missing_origin,
        Err(VisualNavigationGeometryError::MissingLineOrigin)
    );
    let unwrapped =
        with_ui(|ui| visual_navigation_rows_for_line(ui, &model, f32::INFINITY, &[0], 32));
    assert!(unwrapped.is_ok());
    let nonfinite = with_ui(|ui| visual_navigation_rows_for_line(ui, &model, f32::NAN, &[0], 32));
    assert_eq!(
        nonfinite,
        Err(VisualNavigationGeometryError::NonFiniteWrapWidth)
    );
    let oversized = line(
        &"x".repeat(96 * 1024 + 1),
        Some(0),
        ViewportLineTruncationState::None,
    );
    let oversized = with_ui(|ui| visual_navigation_rows_for_line(ui, &oversized, 120.0, &[0], 32));
    assert_eq!(
        oversized,
        Err(VisualNavigationGeometryError::FragmentTooLarge)
    );
}

#[test]
fn large_complete_ascii_line_uses_nearest_stops_without_exceeding_budget() {
    let text = "x".repeat(8_192);
    let model = line(&text, Some(0), ViewportLineTruncationState::None);
    let valid = (5_452..=8_192).collect::<Vec<_>>();
    let rows = with_ui(|ui| {
        visual_navigation_source_row_for_line(
            ui,
            &model,
            120.0,
            7_500,
            CaretAffinity::Upstream,
            &valid,
            4_096,
        )
    })
    .expect("nearest complete-line stops should stay within the renderer budget");
    assert!(
        rows.row
            .stops
            .iter()
            .any(|stop| stop.position.byte_column == 7_500)
    );
}

#[test]
fn focused_target_preserves_near_middle_and_end_columns_over_4096_boundaries() {
    let model = line(
        &"x".repeat(8_192),
        Some(0),
        ViewportLineTruncationState::None,
    );
    let boundaries = (0..=8_192).collect::<Vec<_>>();
    for expected in [8_u64, 4_096, 8_192] {
        let source = with_ui(|ui| {
            visual_navigation_selected_row_for_line(
                ui,
                &model,
                f32::INFINITY,
                &boundaries,
                VisualNavigationRowRequest::Source {
                    byte_column: expected,
                    affinity: CaretAffinity::Upstream,
                },
            )
        })
        .expect("source row should retain the requested boundary");
        let target = with_ui(|ui| {
            visual_navigation_selected_row_for_line(
                ui,
                &model,
                f32::INFINITY,
                &boundaries,
                VisualNavigationRowRequest::Target {
                    row_index: 0,
                    preferred_x: source.source_x.expect("source x").value,
                },
            )
        })
        .expect("target row should choose the nearest rendered X");
        assert_eq!(
            target
                .target_stop
                .expect("target stop")
                .position
                .byte_column,
            expected
        );
    }
}

#[test]
fn focused_geometry_rejects_mixed_missing_boundaries_and_partial_fragments() {
    let model = line("bounded", Some(0), ViewportLineTruncationState::None);
    let result = with_ui(|ui| {
        visual_navigation_selected_row_for_line(
            ui,
            &model,
            f32::INFINITY,
            &[0, 1, 99],
            VisualNavigationRowRequest::Source {
                byte_column: 1,
                affinity: CaretAffinity::Upstream,
            },
        )
    });
    assert_eq!(result, Err(VisualNavigationGeometryError::NeedMoreWindow));

    let partial = line("bounded", Some(0), ViewportLineTruncationState::Leading);
    let result = with_ui(|ui| {
        visual_navigation_selected_row_for_line(
            ui,
            &partial,
            f32::INFINITY,
            &[0, 1],
            VisualNavigationRowRequest::Target {
                row_index: 0,
                preferred_x: 0.0,
            },
        )
    });
    assert_eq!(result, Err(VisualNavigationGeometryError::PartialFragment));
}
