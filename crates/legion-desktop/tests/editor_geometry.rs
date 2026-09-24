use legion_desktop::view::{
    DesktopCodeLineViewModel, drag_anchor_for_line_pointer_with_galley, drag_selection_range,
    editor_coordinate_from_galley_pointer, editor_cursor_rect_for_galley,
    editor_galley_for_geometry, editor_selection_columns_for_line,
    editor_selection_rects_for_galley, editor_visual_coordinate_from_galley_pointer,
    line_range_for_code_line, word_range_for_coordinate,
};
use legion_protocol::{
    ByteRange, CaretAffinity, TextCoordinate, Utf16Position, Utf16Range,
    ViewportLineTruncationState,
};

fn line(text: &str) -> DesktopCodeLineViewModel {
    DesktopCodeLineViewModel {
        number: 1,
        text: text.to_string(),
        highlights: Vec::new(),
        truncation_state: ViewportLineTruncationState::None,
        byte_range: ByteRange::new(0, text.len() as u64),
        utf16_range: Utf16Range {
            start: Utf16Position {
                line: 0,
                character: 0,
            },
            end: Utf16Position {
                line: 0,
                character: text.encode_utf16().count() as u32,
            },
        },
        line_start_byte_offset: Some(0),
        logical_end_byte: Some(text.len() as u64),
        line_start_utf16_offset: None,
    }
}

fn cursor(character: u32) -> TextCoordinate {
    TextCoordinate {
        line: 0,
        character,
        byte_offset: None,
        utf16_offset: None,
    }
}

#[test]
fn shaped_ascii_pointer_and_cursor_use_galley_positions() {
    let context = egui::Context::default();
    let model = line("wide ascii text");
    let galley = editor_galley_for_geometry(&context, &model, f32::INFINITY);
    let origin = egui::pos2(40.0, 12.0);
    let position = galley.pos_from_cursor(egui::text::CCursor::new(5));
    let coordinate = editor_coordinate_from_galley_pointer(
        &model,
        &galley,
        origin + egui::vec2(position.min.x + 0.1, position.center().y),
        origin,
    )
    .expect("ASCII pointer should map");
    assert_eq!(coordinate.character, 5);
    let caret = editor_cursor_rect_for_galley(&model, &galley, origin, 5);
    assert!(caret.top() >= origin.y);
    assert!(caret.height() > 0.0);
    assert!(caret.height() < galley.size().y + 0.1);
}

#[test]
fn shaped_geometry_maps_byte_columns_for_combining_and_emoji() {
    let context = egui::Context::default();
    let model = line("a\u{301}👩\u{200d}💻z");
    let galley = editor_galley_for_geometry(&context, &model, f32::INFINITY);
    let origin = egui::Pos2::ZERO;
    for scalar in [0usize, 1, 2, 3, 4, 5, 6] {
        let position = galley.pos_from_cursor(egui::text::CCursor::new(scalar));
        let coordinate = editor_coordinate_from_galley_pointer(
            &model,
            &galley,
            egui::pos2(position.min.x + 0.1, position.center().y),
            origin,
        )
        .expect("Unicode boundary should map");
        let expected = [0, 3, 3, 10, 10, 14, 15][scalar];
        assert_eq!(coordinate.character, expected, "scalar boundary {scalar}");
    }
    let rects = editor_selection_rects_for_galley(&model, &galley, origin, 1, 3, false);
    assert_eq!(rects.len(), 1);
    assert!(rects[0].width() > 0.0);
}

#[test]
fn wrapped_second_row_has_actual_cursor_and_split_selection_geometry() {
    let context = egui::Context::default();
    let model = line("one two three four five");
    let galley = editor_galley_for_geometry(&context, &model, 36.0);
    assert!(
        galley.rows.len() >= 2,
        "fixture must wrap into multiple rows"
    );
    let origin = egui::pos2(8.0, 20.0);
    let caret = editor_cursor_rect_for_galley(&model, &galley, origin, 10);
    assert!(caret.top() > origin.y, "caret should be on the wrapped row");
    assert!(caret.height() < galley.size().y);
    let second_row_position = galley.pos_from_cursor(egui::text::CCursor::new(10));
    let pointer = origin
        + egui::vec2(
            second_row_position.min.x + 0.1,
            second_row_position.center().y,
        );
    let coordinate = editor_coordinate_from_galley_pointer(&model, &galley, pointer, origin)
        .expect("wrapped pointer should map");
    assert_eq!(coordinate.character, 10);
    let anchor = drag_anchor_for_line_pointer_with_galley(
        &model,
        &galley,
        pointer.x,
        egui::Vec2::ZERO,
        pointer.y,
        origin,
    )
    .expect("wrapped drag anchor should map");
    assert_eq!(anchor.character, 10);
    let rects = editor_selection_rects_for_galley(&model, &galley, origin, 0, 18, false);
    assert!(rects.len() >= 2, "selection should split at wrapped rows");
    assert!(
        rects
            .windows(2)
            .all(|rows| rows[0].bottom() <= rows[1].top())
    );
}

#[test]
fn wrapped_boundary_pointer_preserves_visual_affinity_and_paints_row() {
    let context = egui::Context::default();
    let model = line("one two three four five");
    let galley = editor_galley_for_geometry(&context, &model, 36.0);
    assert!(galley.rows.len() >= 2);
    let origin = egui::Pos2::ZERO;
    let (upstream_coordinate, upstream_affinity) = editor_visual_coordinate_from_galley_pointer(
        &model,
        &galley,
        egui::pos2(
            galley.rows[0].rect().right() - 0.1,
            galley.rows[0].rect().center().y,
        ),
        origin,
    )
    .expect("preceding wrapped row should map");
    let (downstream_coordinate, downstream_affinity) =
        editor_visual_coordinate_from_galley_pointer(
            &model,
            &galley,
            egui::pos2(
                galley.rows[1].rect().left() + 0.1,
                galley.rows[1].rect().center().y,
            ),
            origin,
        )
        .expect("following wrapped row should map");
    assert_eq!(upstream_affinity, CaretAffinity::Upstream);
    assert_eq!(downstream_affinity, CaretAffinity::Downstream);
    assert_eq!(upstream_coordinate.line, downstream_coordinate.line);
    assert_eq!(
        upstream_coordinate.character,
        downstream_coordinate.character
    );
    assert_eq!(
        upstream_coordinate.byte_offset,
        downstream_coordinate.byte_offset
    );
    let boundary_byte = upstream_coordinate
        .byte_offset
        .expect("shaped boundary should carry an absolute byte offset");
    let boundary = model.text[..boundary_byte as usize].chars().count();
    let upstream_rect = legion_desktop::view::editor_cursor_rect_for_galley_with_affinity(
        &model,
        &galley,
        origin,
        boundary,
        upstream_affinity,
    );
    let downstream_rect = legion_desktop::view::editor_cursor_rect_for_galley_with_affinity(
        &model,
        &galley,
        origin,
        boundary,
        downstream_affinity,
    );
    assert!(downstream_rect.top() > upstream_rect.top());
}

#[test]
fn empty_line_has_visible_caret_and_newline_selection_geometry() {
    let context = egui::Context::default();
    let model = line("");
    let galley = editor_galley_for_geometry(&context, &model, f32::INFINITY);
    let caret = editor_cursor_rect_for_galley(&model, &galley, egui::Pos2::ZERO, 0);
    assert!(caret.height() > 0.0);
    let selection =
        editor_selection_rects_for_galley(&model, &galley, egui::Pos2::ZERO, 0, 0, true);
    assert_eq!(selection.len(), 1);
    assert!(selection[0].height() > 0.0);
    assert!(
        editor_selection_rects_for_galley(&model, &galley, egui::Pos2::ZERO, 0, 0, false)
            .is_empty()
    );
}

#[test]
fn visible_slice_geometry_preserves_absolute_byte_and_utf16_offsets() {
    let mut model = line("é🙂");
    model.byte_range = legion_protocol::ByteRange::new(100, 106);
    model.number = 3;
    model.utf16_range = legion_protocol::Utf16Range {
        start: legion_protocol::Utf16Position {
            line: 2,
            character: 0,
        },
        end: legion_protocol::Utf16Position {
            line: 2,
            character: 3,
        },
    };
    model.line_start_byte_offset = Some(100);
    model.line_start_utf16_offset = Some(60);
    let context = egui::Context::default();
    let galley = editor_galley_for_geometry(&context, &model, f32::INFINITY);
    let coordinate = editor_coordinate_from_galley_pointer(
        &model,
        &galley,
        galley.pos_from_cursor(egui::text::CCursor::new(2)).min,
        egui::Pos2::ZERO,
    )
    .expect("visible slice pointer should map");
    assert_eq!(coordinate.line, 2);
    assert_eq!(coordinate.character, 6);
    assert_eq!(coordinate.byte_offset, Some(106));
    assert_eq!(coordinate.utf16_offset, Some(63));
}

#[test]
fn leading_multibyte_slice_needs_logical_line_byte_base() {
    // The logical line begins at snapshot byte 100 / UTF-16 offset 200. The
    // visible slice begins after an omitted four-byte, two-unit `🙂` prefix,
    // so the logical line-local byte column at the visible end is 4 + 5 = 9.
    let mut model = line("🙂z");
    model.number = 8;
    model.truncation_state = ViewportLineTruncationState::Leading;
    model.byte_range = legion_protocol::ByteRange::new(104, 109);
    model.utf16_range = legion_protocol::Utf16Range {
        start: legion_protocol::Utf16Position {
            line: 7,
            character: 2,
        },
        end: legion_protocol::Utf16Position {
            line: 7,
            character: 5,
        },
    };
    model.line_start_byte_offset = Some(100);
    model.line_start_utf16_offset = Some(200);
    let context = egui::Context::default();
    let galley = editor_galley_for_geometry(&context, &model, f32::INFINITY);
    let coordinate = editor_coordinate_from_galley_pointer(
        &model,
        &galley,
        galley.pos_from_cursor(egui::text::CCursor::new(2)).min,
        egui::Pos2::ZERO,
    )
    .expect("leading visible slice pointer should map");
    assert_eq!(coordinate.character, 9);
    assert_eq!(coordinate.line, 7);
    assert_eq!(coordinate.byte_offset, Some(109));
    assert_eq!(coordinate.utf16_offset, Some(205));
}

fn sliced_line(state: ViewportLineTruncationState, text: &str) -> DesktopCodeLineViewModel {
    let mut model = line(text);
    model.number = 1;
    model.truncation_state = state;
    model.byte_range = legion_protocol::ByteRange::new(100, 100 + text.len() as u64);
    model.line_start_byte_offset = Some(90);
    model.logical_end_byte = Some(90 + text.len() as u64);
    model
}

fn byte_coordinate(line: u32, character: u32, byte: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte),
        utf16_offset: None,
    }
}

fn logical_byte_coordinate(line: u32, character: u32, byte: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte),
        utf16_offset: None,
    }
}

#[test]
fn selection_columns_clip_leading_and_trailing_endpoints_to_visible_slice() {
    let leading = sliced_line(ViewportLineTruncationState::Leading, "cdef");
    let selection = legion_protocol::ProtocolTextRange {
        start: byte_coordinate(0, 0, 90),
        end: byte_coordinate(0, 14, 104),
    };
    assert_eq!(
        editor_selection_columns_for_line(&leading, selection),
        Some((0, 4, false))
    );

    let mut trailing = sliced_line(ViewportLineTruncationState::Trailing, "abcd");
    trailing.line_start_byte_offset = Some(100);
    let selection = legion_protocol::ProtocolTextRange {
        start: byte_coordinate(0, 0, 100),
        end: byte_coordinate(0, 10, 110),
    };
    assert_eq!(
        editor_selection_columns_for_line(&trailing, selection),
        Some((0, 4, false))
    );
}

#[test]
fn selection_columns_cover_both_sided_slice_and_reject_nonoverlap_or_caret() {
    let line = sliced_line(ViewportLineTruncationState::Both, "cdef");
    let covering = legion_protocol::ProtocolTextRange {
        start: byte_coordinate(0, 0, 90),
        end: byte_coordinate(0, 20, 110),
    };
    assert_eq!(
        editor_selection_columns_for_line(&line, covering),
        Some((0, 4, false))
    );
    let outside = legion_protocol::ProtocolTextRange {
        start: byte_coordinate(0, 0, 90),
        end: byte_coordinate(0, 5, 95),
    };
    assert_eq!(editor_selection_columns_for_line(&line, outside), None);
    let caret = legion_protocol::ProtocolTextRange {
        start: byte_coordinate(0, 12, 102),
        end: byte_coordinate(0, 12, 102),
    };
    assert_eq!(editor_selection_columns_for_line(&line, caret), None);
}

#[test]
fn selection_columns_clip_character_only_ranges_with_known_leading_origin() {
    let line = sliced_line(ViewportLineTruncationState::Leading, "cdef");
    let selection = legion_protocol::ProtocolTextRange {
        start: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
        end: TextCoordinate {
            line: 0,
            character: 14,
            byte_offset: None,
            utf16_offset: None,
        },
    };
    assert_eq!(
        editor_selection_columns_for_line(&line, selection),
        Some((0, 4, false))
    );
}

#[test]
fn selection_columns_paint_only_interior_empty_lines() {
    let mut empty = line("");
    empty.number = 2;
    empty.byte_range = ByteRange::new(10, 10);
    empty.line_start_byte_offset = Some(10);
    let interior = legion_protocol::ProtocolTextRange {
        start: TextCoordinate {
            line: 0,
            character: 1,
            byte_offset: None,
            utf16_offset: None,
        },
        end: TextCoordinate {
            line: 2,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
    };
    assert_eq!(
        editor_selection_columns_for_line(&empty, interior),
        Some((0, 0, true))
    );

    let exclusive_end = legion_protocol::ProtocolTextRange {
        start: interior.start,
        end: TextCoordinate {
            line: 1,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
    };
    assert_eq!(
        editor_selection_columns_for_line(&empty, exclusive_end),
        None
    );
}

#[test]
fn selection_columns_fail_closed_without_origin_for_leading_slice_character_ranges() {
    let mut line = sliced_line(ViewportLineTruncationState::Leading, "cdef");
    line.line_start_byte_offset = None;
    let selection = legion_protocol::ProtocolTextRange {
        start: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
        end: TextCoordinate {
            line: 0,
            character: 4,
            byte_offset: None,
            utf16_offset: None,
        },
    };
    assert_eq!(editor_selection_columns_for_line(&line, selection), None);
}

#[test]
fn leading_slice_word_and_line_ranges_keep_logical_offsets() {
    let mut model = sliced_line(ViewportLineTruncationState::Leading, "beta_value");
    model.line_start_byte_offset = Some(90);
    let word = word_range_for_coordinate(&model, logical_byte_coordinate(0, 14, 104))
        .expect("visible word should map");
    assert_eq!(word.start.character, 10);
    assert_eq!(word.end.character, 20);
    assert_eq!(word.start.byte_offset, Some(100));
    let full = line_range_for_code_line(&model).expect("logical origin should map");
    assert_eq!(full.start.character, 10);
    assert_eq!(full.end.character, 20);
    assert_eq!(full.start.byte_offset, Some(100));
    assert_eq!(full.end.byte_offset, Some(110));
}

#[test]
fn line_range_fails_closed_without_leading_slice_origin() {
    let mut model = sliced_line(ViewportLineTruncationState::Both, "cdef");
    model.line_start_byte_offset = None;
    assert_eq!(line_range_for_code_line(&model), None);
}

#[test]
fn combining_only_selection_has_visible_width() {
    let context = egui::Context::default();
    let model = line("a\u{301}b");
    let galley = editor_galley_for_geometry(&context, &model, f32::INFINITY);
    let rects = editor_selection_rects_for_galley(&model, &galley, egui::Pos2::ZERO, 1, 2, false);
    assert_eq!(rects.len(), 1);
    assert!(rects[0].width() > 0.0);
}

#[test]
fn reverse_drag_preserves_directed_anchor_and_head() {
    let range = drag_selection_range(Some(cursor(12)), cursor(0), cursor(3));
    assert_eq!(range.start.character, 12);
    assert_eq!(range.end.character, 3);
}
