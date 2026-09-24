use legion_editor::{
    CaretAffinity, DirectedCaret, EditorEngine, PreferredX, ShapedVisualRow, TextPosition,
    VerticalCaretStop, VerticalDirection, VerticalLayoutId, VerticalMovementRequest,
    VerticalSourceRow,
};
use legion_protocol::{BufferVersion, FileId, SnapshotId, WorkspaceId};

fn engine_with(text: &str) -> (EditorEngine, legion_protocol::BufferId) {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(1), "vertical.txt", text)
        .unwrap();
    (engine, buffer)
}

fn x(value: f32) -> PreferredX {
    PreferredX::new(value).unwrap()
}

fn row(
    line: u32,
    row_index: u32,
    row_count: u32,
    end: u32,
    stops: &[(u32, f32)],
) -> ShapedVisualRow {
    ShapedVisualRow {
        logical_line: line,
        row_index: Some(row_index),
        row_count: Some(row_count),
        start: TextPosition::new(line as usize, 0),
        end: TextPosition::new(line as usize, end as usize),
        stops: stops
            .iter()
            .map(|(column, rendered_x)| VerticalCaretStop {
                position: TextPosition::new(line as usize, *column as usize),
                x: x(*rendered_x),
                affinity: CaretAffinity::Downstream,
            })
            .collect(),
    }
}

fn ranged_row(
    line: u32,
    row_index: u32,
    row_count: u32,
    start: u32,
    end: u32,
    stops: &[(u32, f32)],
) -> ShapedVisualRow {
    ShapedVisualRow {
        logical_line: line,
        row_index: Some(row_index),
        row_count: Some(row_count),
        start: TextPosition::new(line as usize, start as usize),
        end: TextPosition::new(line as usize, end as usize),
        stops: stops
            .iter()
            .map(|(column, rendered_x)| VerticalCaretStop {
                position: TextPosition::new(line as usize, *column as usize),
                x: x(*rendered_x),
                affinity: CaretAffinity::Downstream,
            })
            .collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn request(
    engine: &EditorEngine,
    buffer: legion_protocol::BufferId,
    carets: Vec<DirectedCaret>,
    layout_id: VerticalLayoutId,
    direction: VerticalDirection,
    extend: bool,
    source_rows: Vec<VerticalSourceRow>,
    target_rows: Vec<ShapedVisualRow>,
) -> VerticalMovementRequest {
    let snapshot = engine.current_snapshot(buffer).unwrap();
    VerticalMovementRequest {
        expected_snapshot_id: snapshot.snapshot_id,
        expected_buffer_version: snapshot.buffer_version,
        expected_carets: carets,
        layout_id,
        direction,
        extend,
        source_rows,
        target_rows,
    }
}

#[test]
fn vertical_movement_retains_each_preferred_x_across_short_lines_and_carets() {
    let (mut engine, buffer) = engine_with("abcdefghij\nxy\nabcdefghij");
    let carets = vec![
        DirectedCaret::new(TextPosition::new(0, 8), None),
        DirectedCaret::new(TextPosition::new(0, 2), None),
    ];
    engine.set_directed_carets(buffer, carets.clone()).unwrap();
    let layout = VerticalLayoutId::new(7).unwrap();
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                carets,
                layout,
                VerticalDirection::Down,
                false,
                vec![
                    VerticalSourceRow {
                        row: row(0, 0, 1, 10, &[(8, 8.0)]),
                        source_x: x(8.0),
                    },
                    VerticalSourceRow {
                        row: row(0, 0, 1, 10, &[(2, 2.0)]),
                        source_x: x(2.0),
                    },
                ],
                vec![
                    row(1, 0, 1, 2, &[(0, 0.0), (2, 2.0)]),
                    row(1, 0, 1, 2, &[(0, 0.0), (2, 2.0)]),
                ],
            ),
        )
        .unwrap();
    let moved = engine.directed_carets(buffer).unwrap();
    assert_eq!(moved[0].head, TextPosition::new(1, 2));
    assert_eq!(moved[1].head, TextPosition::new(1, 2));
    assert_eq!(moved[0].preferred_x.map(PreferredX::get), Some(8.0));
    assert_eq!(moved[1].preferred_x.map(PreferredX::get), Some(2.0));

    let source = moved.clone();
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                source,
                layout,
                VerticalDirection::Down,
                false,
                vec![
                    VerticalSourceRow {
                        row: row(1, 0, 1, 2, &[(2, 2.0)]),
                        source_x: x(2.0),
                    },
                    VerticalSourceRow {
                        row: row(1, 0, 1, 2, &[(2, 2.0)]),
                        source_x: x(2.0),
                    },
                ],
                vec![
                    row(2, 0, 1, 10, &[(2, 2.0), (8, 8.0)]),
                    row(2, 0, 1, 10, &[(2, 2.0), (8, 8.0)]),
                ],
            ),
        )
        .unwrap();
    let restored = engine.directed_carets(buffer).unwrap();
    assert_eq!(restored[0].head, TextPosition::new(2, 8));
    assert_eq!(restored[1].head, TextPosition::new(2, 2));
}

#[test]
fn vertical_movement_accepts_expected_carets_without_preferred_x() {
    let (mut engine, buffer) = engine_with("abcdefghij\nxy\nabcdefghij");
    let carets = vec![DirectedCaret::new(TextPosition::new(0, 8), None)];
    engine.set_directed_carets(buffer, carets.clone()).unwrap();
    let layout = VerticalLayoutId::new(7).unwrap();
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                carets,
                layout,
                VerticalDirection::Down,
                false,
                vec![VerticalSourceRow {
                    row: row(0, 0, 1, 10, &[(8, 8.0)]),
                    source_x: x(8.0),
                }],
                vec![row(1, 0, 1, 2, &[(0, 0.0), (2, 2.0)])],
            ),
        )
        .unwrap();
    let moved = engine.directed_carets(buffer).unwrap();
    assert!(moved[0].preferred_x.is_some());
    let without_preferred_x =
        vec![DirectedCaret::new(moved[0].head, moved[0].anchor).with_affinity(moved[0].affinity)];
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                without_preferred_x,
                layout,
                VerticalDirection::Down,
                false,
                vec![VerticalSourceRow {
                    row: row(1, 0, 1, 2, &[(2, 2.0)]),
                    source_x: x(2.0),
                }],
                vec![row(2, 0, 1, 10, &[(2, 2.0), (8, 8.0)])],
            ),
        )
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0].head,
        TextPosition::new(2, 8)
    );
}

#[test]
fn vertical_shift_reversal_preserves_anchors_and_affinity() {
    let (mut engine, buffer) = engine_with("abcd\nefgh\nijkl");
    let initial = vec![DirectedCaret::new(TextPosition::new(1, 2), None)];
    engine.set_directed_carets(buffer, initial.clone()).unwrap();
    let layout = VerticalLayoutId::new(8).unwrap();
    let move_one = |engine: &EditorEngine, source: Vec<DirectedCaret>, direction| {
        let source_line = source[0].head.line as u32;
        let target_line = if direction == VerticalDirection::Up {
            source_line - 1
        } else {
            source_line + 1
        };
        request(
            engine,
            buffer,
            source,
            layout,
            direction,
            true,
            vec![VerticalSourceRow {
                row: row(source_line, 0, 1, 4, &[(2, 2.0)]),
                source_x: x(2.0),
            }],
            vec![row(target_line, 0, 1, 4, &[(2, 2.0)])],
        )
    };
    engine
        .move_vertically(buffer, move_one(&engine, initial, VerticalDirection::Up))
        .unwrap();
    let up = engine.directed_carets(buffer).unwrap();
    assert_eq!(up[0].anchor, Some(TextPosition::new(1, 2)));
    assert_eq!(up[0].head, TextPosition::new(0, 2));
    engine
        .move_vertically(buffer, move_one(&engine, up, VerticalDirection::Down))
        .unwrap();
    let reversed = engine.directed_carets(buffer).unwrap();
    assert_eq!(reversed[0].anchor, Some(TextPosition::new(1, 2)));
    assert_eq!(reversed[0].head, TextPosition::new(1, 2));
}

#[test]
fn vertical_rejects_stale_source_facts_bad_direction_and_empty_stops_atomically() {
    let (mut engine, buffer) = engine_with("abc\ndef");
    let carets = vec![DirectedCaret::new(TextPosition::new(0, 1), None)];
    engine.set_directed_carets(buffer, carets.clone()).unwrap();
    let layout = VerticalLayoutId::new(9).unwrap();
    let mut stale = request(
        &engine,
        buffer,
        carets.clone(),
        layout,
        VerticalDirection::Down,
        false,
        vec![VerticalSourceRow {
            row: row(0, 0, 1, 3, &[]),
            source_x: x(1.0),
        }],
        vec![row(1, 0, 1, 3, &[(1, 1.0)])],
    );
    stale.expected_carets[0] = DirectedCaret::new(TextPosition::new(0, 2), None);
    assert!(engine.move_vertically(buffer, stale).is_err());
    assert_eq!(engine.directed_carets(buffer).unwrap(), carets);

    let empty = request(
        &engine,
        buffer,
        carets,
        layout,
        VerticalDirection::Down,
        false,
        vec![VerticalSourceRow {
            row: row(0, 0, 1, 3, &[]),
            source_x: x(1.0),
        }],
        vec![row(0, 0, 1, 3, &[])],
    );
    assert!(engine.move_vertically(buffer, empty).is_err());
}

#[test]
fn vertical_rejects_stale_snapshot_and_version_without_mutating_carets() {
    let (mut engine, buffer) = engine_with("abc\ndef");
    let carets = vec![DirectedCaret::new(TextPosition::new(0, 1), None)];
    engine.set_directed_carets(buffer, carets.clone()).unwrap();
    let layout = VerticalLayoutId::new(14).unwrap();
    let source_rows = vec![VerticalSourceRow {
        row: row(0, 0, 1, 3, &[(1, 1.0)]),
        source_x: x(1.0),
    }];
    let target_rows = vec![row(1, 0, 1, 3, &[(1, 1.0)])];
    let mut stale_snapshot = request(
        &engine,
        buffer,
        carets.clone(),
        layout,
        VerticalDirection::Down,
        false,
        source_rows.clone(),
        target_rows.clone(),
    );
    stale_snapshot.expected_snapshot_id = SnapshotId(999);
    assert!(engine.move_vertically(buffer, stale_snapshot).is_err());
    assert_eq!(engine.directed_carets(buffer).unwrap(), carets);

    let mut stale_version = request(
        &engine,
        buffer,
        carets.clone(),
        layout,
        VerticalDirection::Down,
        false,
        source_rows,
        target_rows,
    );
    stale_version.expected_buffer_version = BufferVersion(999);
    assert!(engine.move_vertically(buffer, stale_version).is_err());
    assert_eq!(engine.directed_carets(buffer).unwrap(), carets);
}

#[test]
fn invalid_second_caret_fact_leaves_ordered_state_and_metadata_unchanged() {
    let (mut engine, buffer) = engine_with("abc\ndef");
    let carets = vec![
        DirectedCaret::new(TextPosition::new(0, 1), None),
        DirectedCaret::new(TextPosition::new(0, 2), None),
    ];
    engine.set_directed_carets(buffer, carets.clone()).unwrap();
    let text = engine.text(buffer).unwrap().to_string();
    let version = engine.buffer_version(buffer).unwrap();
    let dirty = engine.is_dirty(buffer).unwrap();
    let history = engine.transaction_log().len();
    let invalid = request(
        &engine,
        buffer,
        carets.clone(),
        VerticalLayoutId::new(20).unwrap(),
        VerticalDirection::Down,
        false,
        vec![
            VerticalSourceRow {
                row: row(0, 0, 1, 3, &[(1, 1.0)]),
                source_x: x(1.0),
            },
            VerticalSourceRow {
                row: row(0, 0, 1, 3, &[(2, 2.0)]),
                source_x: x(2.0),
            },
        ],
        vec![row(1, 0, 1, 3, &[(1, 1.0)]), row(0, 0, 1, 3, &[(2, 2.0)])],
    );
    assert!(engine.move_vertically(buffer, invalid).is_err());
    assert_eq!(engine.directed_carets(buffer).unwrap(), carets);
    assert_eq!(engine.text(buffer).unwrap(), text);
    assert_eq!(engine.buffer_version(buffer).unwrap(), version);
    assert_eq!(engine.is_dirty(buffer).unwrap(), dirty);
    assert_eq!(engine.transaction_log().len(), history);
}

#[test]
fn invalid_preferred_x_and_layout_identity_are_rejected() {
    assert!(PreferredX::new(f32::NAN).is_err());
    assert!(PreferredX::new(f32::INFINITY).is_err());
    assert!(VerticalLayoutId::new(0).is_none());
}

#[test]
fn changed_layout_reseeds_preferred_x_and_nonvertical_paths_clear_it() {
    let (mut engine, buffer) = engine_with("abc\ndef\nghi");
    let source = vec![DirectedCaret::new(TextPosition::new(0, 1), None)];
    engine.set_directed_carets(buffer, source.clone()).unwrap();
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                source,
                VerticalLayoutId::new(15).unwrap(),
                VerticalDirection::Down,
                false,
                vec![VerticalSourceRow {
                    row: row(0, 0, 1, 3, &[(1, 1.0)]),
                    source_x: x(1.0),
                }],
                vec![row(1, 0, 1, 3, &[(1, 1.0)])],
            ),
        )
        .unwrap();
    let moved = engine.directed_carets(buffer).unwrap();
    assert_eq!(moved[0].preferred_x.map(PreferredX::get), Some(1.0));

    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                moved,
                VerticalLayoutId::new(16).unwrap(),
                VerticalDirection::Down,
                false,
                vec![VerticalSourceRow {
                    row: row(1, 0, 1, 3, &[(1, 7.0)]),
                    source_x: x(7.0),
                }],
                vec![row(2, 0, 1, 3, &[(1, 7.0)])],
            ),
        )
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0]
            .preferred_x
            .map(PreferredX::get),
        Some(7.0)
    );

    let current = engine.directed_carets(buffer).unwrap();
    engine
        .move_horizontally(buffer, legion_editor::HorizontalDirection::Left, false)
        .unwrap();
    assert!(
        engine.directed_carets(buffer).unwrap()[0]
            .preferred_x
            .is_none()
    );
    let source = engine.directed_carets(buffer).unwrap();
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                source,
                VerticalLayoutId::new(17).unwrap(),
                VerticalDirection::Up,
                false,
                vec![VerticalSourceRow {
                    row: row(2, 0, 1, 3, &[(0, 0.0)]),
                    source_x: x(0.0),
                }],
                vec![row(1, 0, 1, 3, &[(0, 0.0)])],
            ),
        )
        .unwrap();
    let source = engine.directed_carets(buffer).unwrap();
    engine
        .move_to_boundary(buffer, legion_editor::BoundaryKind::LineStart, false)
        .unwrap();
    assert!(
        engine.directed_carets(buffer).unwrap()[0]
            .preferred_x
            .is_none()
    );
    assert_ne!(source, engine.directed_carets(buffer).unwrap());

    let snapshot = engine.current_snapshot(buffer).unwrap();
    engine
        .set_visual_directed_carets(
            buffer,
            snapshot.snapshot_id,
            snapshot.buffer_version,
            vec![
                DirectedCaret::new(TextPosition::new(1, 1), None)
                    .with_affinity(CaretAffinity::Downstream),
            ],
        )
        .unwrap();
    assert!(
        engine.directed_carets(buffer).unwrap()[0]
            .preferred_x
            .is_none()
    );
    assert_ne!(current, engine.directed_carets(buffer).unwrap());
}

#[test]
fn vertical_validates_wrapped_span_adjacency_and_grapheme_boundaries() {
    let (mut engine, buffer) = engine_with("abcdefghij");
    let source = vec![DirectedCaret::new(TextPosition::new(0, 4), None)];
    engine.set_directed_carets(buffer, source.clone()).unwrap();
    let layout = VerticalLayoutId::new(12).unwrap();
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                source,
                layout,
                VerticalDirection::Down,
                false,
                vec![VerticalSourceRow {
                    row: ranged_row(0, 0, 2, 0, 5, &[(4, 4.0)]),
                    source_x: x(4.0),
                }],
                vec![ranged_row(0, 1, 2, 5, 10, &[(5, 4.0), (10, 9.0)])],
            ),
        )
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0].head,
        TextPosition::new(0, 5)
    );

    let (mut combining, combining_buffer) = engine_with("éx\ny");
    let interior = vec![DirectedCaret::new(TextPosition::new(0, 1), None)];
    combining
        .set_directed_carets(combining_buffer, interior.clone())
        .unwrap();
    let invalid = request(
        &combining,
        combining_buffer,
        interior,
        layout,
        VerticalDirection::Down,
        false,
        vec![VerticalSourceRow {
            row: row(0, 0, 1, 3, &[(1, 1.0)]),
            source_x: x(1.0),
        }],
        vec![row(1, 0, 1, 1, &[(1, 1.0)])],
    );
    assert!(
        combining
            .move_vertically(combining_buffer, invalid)
            .is_err()
    );
}

#[test]
fn successful_wrapped_downstream_move_and_edit_history_capture_affinity_and_x() {
    let (mut engine, buffer) = engine_with("abcdefghij\nkl");
    let source = vec![
        DirectedCaret::new(TextPosition::new(0, 4), None).with_affinity(CaretAffinity::Upstream),
    ];
    engine.set_directed_carets(buffer, source.clone()).unwrap();
    let layout = VerticalLayoutId::new(21).unwrap();
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                source,
                layout,
                VerticalDirection::Down,
                false,
                vec![VerticalSourceRow {
                    row: ranged_row(0, 0, 2, 0, 5, &[(4, 4.0)]),
                    source_x: x(4.0),
                }],
                vec![ranged_row(0, 1, 2, 5, 10, &[(5, 4.0), (10, 9.0)])],
            ),
        )
        .unwrap();
    let expected_after_move = vec![
        DirectedCaret::new(TextPosition::new(0, 5), None)
            .with_affinity(CaretAffinity::Downstream)
            .with_preferred_x(x(4.0)),
    ];
    assert_eq!(engine.directed_carets(buffer).unwrap(), expected_after_move);

    engine
        .apply_edit(
            buffer,
            legion_text::TextEdit::insert(TextPosition::new(0, 0), "!"),
            legion_protocol::TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(0, 6), None)
                .with_affinity(CaretAffinity::Upstream)
        ]
    );
    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.directed_carets(buffer).unwrap(), expected_after_move);
    engine.redo(buffer, None).unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(0, 6), None)
                .with_affinity(CaretAffinity::Upstream)
        ]
    );
}

#[test]
fn vertical_crosses_crlf_using_content_line_boundaries() {
    let (mut engine, buffer) = engine_with("ab\r\ncd");
    let source = vec![DirectedCaret::new(TextPosition::new(0, 1), None)];
    engine.set_directed_carets(buffer, source.clone()).unwrap();
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                source,
                VerticalLayoutId::new(13).unwrap(),
                VerticalDirection::Down,
                false,
                vec![VerticalSourceRow {
                    row: row(0, 0, 1, 2, &[(1, 1.0)]),
                    source_x: x(1.0),
                }],
                vec![row(1, 0, 1, 2, &[(1, 1.0), (2, 2.0)])],
            ),
        )
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0].head,
        TextPosition::new(1, 1)
    );
}

#[test]
fn vertical_preferred_state_is_reset_by_edit_and_restored_by_undo() {
    let (mut engine, buffer) = engine_with("abc\ndef");
    let carets = vec![DirectedCaret::new(TextPosition::new(0, 1), None)];
    engine.set_directed_carets(buffer, carets.clone()).unwrap();
    let layout = VerticalLayoutId::new(10).unwrap();
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                carets,
                layout,
                VerticalDirection::Down,
                false,
                vec![VerticalSourceRow {
                    row: row(0, 0, 1, 3, &[(1, 1.0)]),
                    source_x: x(1.0),
                }],
                vec![row(1, 0, 1, 3, &[(1, 1.0)])],
            ),
        )
        .unwrap();
    let before_edit = engine.directed_carets(buffer).unwrap();
    engine
        .apply_edit(
            buffer,
            legion_text::TextEdit::insert(TextPosition::new(0, 0), "!"),
            legion_protocol::TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    assert!(
        engine.directed_carets(buffer).unwrap()[0]
            .preferred_x
            .is_none()
    );
    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.directed_carets(buffer).unwrap(), before_edit);
    engine.redo(buffer, None).unwrap();
    assert!(
        engine.directed_carets(buffer).unwrap()[0]
            .preferred_x
            .is_none()
    );
}

#[test]
fn optional_row_facts_cannot_forge_a_final_line_noop_or_source_affinity() {
    let (mut engine, buffer) = engine_with("abcdef");
    let source = vec![DirectedCaret::new(TextPosition::new(0, 2), None)];
    engine.set_directed_carets(buffer, source.clone()).unwrap();
    let mut source_row = row(0, 0, 1, 3, &[]);
    source_row.row_index = None;
    source_row.row_count = None;
    let mut target_row = source_row.clone();
    target_row.stops = vec![VerticalCaretStop {
        position: TextPosition::new(0, 2),
        x: x(2.0),
        affinity: CaretAffinity::Upstream,
    }];
    assert!(
        engine
            .move_vertically(
                buffer,
                request(
                    &engine,
                    buffer,
                    source,
                    VerticalLayoutId::new(18).unwrap(),
                    VerticalDirection::Down,
                    false,
                    vec![VerticalSourceRow {
                        row: source_row,
                        source_x: x(2.0),
                    }],
                    vec![target_row],
                ),
            )
            .is_err()
    );

    let (mut affinity_engine, affinity_buffer) = engine_with("abcdef");
    let affinity_source = vec![DirectedCaret::new(TextPosition::new(0, 3), None)];
    affinity_engine
        .set_directed_carets(affinity_buffer, affinity_source.clone())
        .unwrap();
    let wrapped_source = ranged_row(0, 1, 2, 3, 6, &[]);
    assert!(
        affinity_engine
            .move_vertically(
                affinity_buffer,
                request(
                    &affinity_engine,
                    affinity_buffer,
                    affinity_source,
                    VerticalLayoutId::new(19).unwrap(),
                    VerticalDirection::Up,
                    false,
                    vec![VerticalSourceRow {
                        row: wrapped_source,
                        source_x: x(3.0),
                    }],
                    vec![ranged_row(0, 0, 2, 0, 3, &[(3, 1.0)])],
                ),
            )
            .is_err()
    );
}

#[test]
fn streamed_large_buffer_vertical_target_uses_bounded_positions() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let first = "x".repeat(5 * 1024 * 1024 + 11);
    std::fs::write(file.path(), format!("{first}\nend")).unwrap();
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer_streaming(WorkspaceId(1), FileId(2), "large.txt", file.path())
        .unwrap();
    let source = vec![DirectedCaret::new(TextPosition::new(0, first.len()), None)];
    engine.set_directed_carets(buffer, source.clone()).unwrap();
    let layout = VerticalLayoutId::new(11).unwrap();
    engine
        .move_vertically(
            buffer,
            request(
                &engine,
                buffer,
                source,
                layout,
                VerticalDirection::Down,
                false,
                vec![VerticalSourceRow {
                    row: row(0, 0, 1, first.len() as u32, &[(first.len() as u32, 1.0)]),
                    source_x: x(1.0),
                }],
                vec![row(1, 0, 1, 3, &[(3, 1.0)])],
            ),
        )
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0].head,
        TextPosition::new(1, 3)
    );
    assert!(engine.text(buffer).is_err());
}
