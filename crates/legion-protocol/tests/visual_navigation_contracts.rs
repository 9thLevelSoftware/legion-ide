use legion_protocol::*;

fn position(line: u32, byte_column: u64) -> VisualNavigationPosition {
    VisualNavigationPosition { line, byte_column }
}

#[test]
fn visual_navigation_request_roundtrips_with_full_layout_identity() {
    let caret = VisualNavigationCaret {
        head: position(2, 4),
        anchor: Some(position(1, 1)),
        affinity: CaretAffinity::Downstream,
        preferred_x: Some(VisualNavigationX { value: 17.5 }),
    };
    let row = VisualNavigationRow {
        logical_line: 2,
        row_index: Some(1),
        row_count: Some(3),
        start: position(2, 0),
        end: position(2, 8),
        stops: vec![VisualNavigationStop {
            position: position(2, 4),
            x: VisualNavigationX { value: 17.5 },
            affinity: CaretAffinity::Downstream,
        }],
    };
    let request = VisualNavigationRequest {
        expected_snapshot_id: SnapshotId(0xfeed_face_dead_beef_0123_4567_89ab_cdef),
        expected_buffer_version: BufferVersion(9),
        expected_carets: vec![caret],
        layout_id: VisualNavigationLayoutId(u128::MAX),
        direction: VisualNavigationDirection::Down,
        extend: true,
        source_rows: vec![VisualNavigationSourceRow {
            row: row.clone(),
            source_x: VisualNavigationX { value: 12.0 },
        }],
        target_rows: vec![row],
    };

    let value = serde_json::to_value(&request).expect("request serializes");
    let decoded: VisualNavigationRequest =
        serde_json::from_value(value).expect("request deserializes");
    assert_eq!(decoded, request);
}

#[test]
fn visual_navigation_projection_roundtrips_logical_line_count() {
    let projection = VisualNavigationProjection {
        snapshot_id: SnapshotId(0x1234),
        buffer_version: BufferVersion(7),
        logical_line_count: 3,
        carets: vec![],
    };
    let value = serde_json::to_value(&projection).expect("projection serializes");
    assert_eq!(value["logical_line_count"], 3);
    let decoded: VisualNavigationProjection =
        serde_json::from_value(value).expect("projection deserializes");
    assert_eq!(decoded, projection);
}

#[test]
fn visual_navigation_x_equality_is_bitwise_and_reflexive() {
    let nan = VisualNavigationX { value: f32::NAN };
    assert_eq!(nan, nan);
    assert_ne!(
        VisualNavigationX { value: 0.0 },
        VisualNavigationX { value: -0.0 }
    );
}

#[test]
fn visual_navigation_window_roundtrips_bounded_origin_and_graphemes() {
    let window = VisualNavigationWindow {
        snapshot_id: SnapshotId(7),
        buffer_version: BufferVersion(3),
        line: 11,
        line_start_byte: 400,
        caret_byte: 415,
        start_byte: 408,
        end_byte: 424,
        logical_end_byte: 460,
        complete_logical_start: false,
        complete_logical_end: false,
        grapheme_boundaries: vec![408, 410, 415, 419, 424],
        text: "visible α".to_string(),
    };
    let encoded = serde_json::to_value(&window).expect("window serializes");
    let decoded: VisualNavigationWindow =
        serde_json::from_value(encoded).expect("window deserializes");
    assert_eq!(decoded, window);
}
