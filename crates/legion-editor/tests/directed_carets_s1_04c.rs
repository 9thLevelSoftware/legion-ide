use legion_editor::{
    BoundaryKind, Cursor, DirectedCaret, EditorEngine, EditorError, Selection, TextEdit,
    TextPosition, TextRange,
};
use legion_protocol::{
    EditorViewportRequest, FileId, SnapshotConsumerKind, TransactionSource, ViewportDimensions,
    ViewportScroll, WorkspaceId,
};
use uuid::Uuid;

fn engine_with(text: &str) -> (EditorEngine, legion_protocol::BufferId) {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(1), "test.txt", text)
        .expect("buffer opens");
    (engine, buffer)
}

#[test]
fn boundary_navigation_preserves_direction_and_handles_crlf_lone_cr_and_trailing_newline() {
    let (mut engine, buffer) = engine_with("éx\r\ny\rzz\n");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 3), None),
                DirectedCaret::new(TextPosition::new(1, 1), Some(TextPosition::new(1, 0))),
            ],
        )
        .unwrap();
    engine
        .move_to_boundary(buffer, BoundaryKind::LineEnd, true)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(0, 3), Some(TextPosition::new(0, 3))),
            DirectedCaret::new(TextPosition::new(1, 1), Some(TextPosition::new(1, 0))),
        ]
    );
    engine
        .move_to_boundary(buffer, BoundaryKind::DocumentEnd, true)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0],
        DirectedCaret::new(TextPosition::new(3, 0), Some(TextPosition::new(0, 3)))
    );
    engine
        .move_to_boundary(buffer, BoundaryKind::DocumentStart, false)
        .unwrap();
    assert!(
        engine
            .directed_carets(buffer)
            .unwrap()
            .iter()
            .all(|caret| caret.anchor.is_none())
    );
}

#[test]
fn boundary_navigation_repeated_shift_reverses_each_unequal_line_caret() {
    let (mut engine, buffer) = engine_with("aé\nlonger");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 1), None),
                DirectedCaret::new(TextPosition::new(1, 4), None),
            ],
        )
        .unwrap();
    engine
        .move_to_boundary(buffer, BoundaryKind::LineStart, true)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(0, 0), Some(TextPosition::new(0, 1))),
            DirectedCaret::new(TextPosition::new(1, 0), Some(TextPosition::new(1, 4))),
        ]
    );
    engine
        .move_to_boundary(buffer, BoundaryKind::LineEnd, true)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0],
        DirectedCaret::new(TextPosition::new(0, 3), Some(TextPosition::new(0, 1)))
    );
    engine
        .move_to_boundary(buffer, BoundaryKind::LineStart, false)
        .unwrap();
    assert!(
        engine
            .directed_carets(buffer)
            .unwrap()
            .iter()
            .all(|caret| caret.anchor.is_none())
    );
}

#[test]
fn boundary_navigation_handles_empty_and_trailing_newline_documents() {
    let (mut engine, empty) = engine_with("");
    engine
        .move_to_boundary(empty, BoundaryKind::DocumentEnd, true)
        .unwrap();
    assert_eq!(
        engine.directed_carets(empty).unwrap(),
        vec![DirectedCaret::new(
            TextPosition::zero(),
            Some(TextPosition::zero())
        )]
    );
    let (mut engine, trailing) = engine_with("a\n");
    engine
        .move_to_boundary(trailing, BoundaryKind::DocumentEnd, false)
        .unwrap();
    assert_eq!(
        engine.directed_carets(trailing).unwrap(),
        vec![DirectedCaret::new(TextPosition::new(1, 0), None)]
    );
}

#[test]
fn native_directed_replacement_collapses_all_anchor_directions_and_history_restores_them() {
    let (mut engine, buffer) = engine_with("abcd\nefgh");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 3), Some(TextPosition::new(0, 1))),
                DirectedCaret::new(TextPosition::new(1, 1), Some(TextPosition::new(1, 3))),
            ],
        )
        .unwrap();
    engine.replace_directed_carets(buffer, "X", None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "aXd\neXh");
    assert!(
        engine
            .directed_carets(buffer)
            .unwrap()
            .iter()
            .all(|caret| caret.anchor.is_none())
    );
    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "abcd\nefgh");
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(0, 3), Some(TextPosition::new(0, 1))),
            DirectedCaret::new(TextPosition::new(1, 1), Some(TextPosition::new(1, 3))),
        ]
    );
    engine.redo(buffer, None).unwrap();
    assert!(
        engine
            .directed_carets(buffer)
            .unwrap()
            .iter()
            .all(|caret| caret.anchor.is_none())
    );
}

#[test]
fn directed_carets_are_one_authoritative_ordered_state() {
    let (mut engine, buffer) = engine_with("abcdef");
    let carets = vec![
        DirectedCaret::new(TextPosition::new(0, 4), Some(TextPosition::new(0, 2))),
        DirectedCaret::new(TextPosition::new(0, 1), None),
    ];
    engine.set_directed_carets(buffer, carets.clone()).unwrap();
    assert_eq!(engine.directed_carets(buffer).unwrap(), carets);
    assert_eq!(
        engine.cursors(buffer).unwrap(),
        vec![
            Cursor {
                position: TextPosition::new(0, 4)
            },
            Cursor {
                position: TextPosition::new(0, 1)
            }
        ]
    );
    assert_eq!(
        engine.selections(buffer).unwrap(),
        vec![Selection {
            range: TextRange::new(TextPosition::new(0, 2), TextPosition::new(0, 4))
        }]
    );
}

#[test]
fn invalid_or_empty_directed_replacement_is_atomic() {
    let (mut engine, buffer) = engine_with("abcdef");
    let original = engine.directed_carets(buffer).unwrap().to_vec();
    assert!(matches!(
        engine.set_directed_carets(buffer, Vec::new()),
        Err(EditorError::InvalidEdit(_))
    ));
    assert_eq!(engine.directed_carets(buffer).unwrap(), original);
    assert!(matches!(
        engine.set_directed_carets(
            buffer,
            vec![DirectedCaret::new(TextPosition::new(0, 99), None)]
        ),
        Err(EditorError::Text(_))
    ));
    assert_eq!(engine.directed_carets(buffer).unwrap(), original);
}

#[test]
fn edit_maps_directed_heads_and_anchors_and_undo_restores_them() {
    let (mut engine, buffer) = engine_with("abcd");
    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(
                TextPosition::new(0, 2),
                Some(TextPosition::new(0, 2)),
            )],
        )
        .unwrap();
    engine
        .apply_edits(
            buffer,
            vec![
                TextEdit::new(
                    TextRange::new(TextPosition::new(0, 1), TextPosition::new(0, 2)),
                    "XX",
                ),
                TextEdit::new(
                    TextRange::new(TextPosition::new(0, 2), TextPosition::new(0, 3)),
                    "Y",
                ),
            ],
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "aXXYd");
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0],
        DirectedCaret::new(TextPosition::new(0, 4), Some(TextPosition::new(0, 3)))
    );
    engine.undo(buffer, None).unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0],
        DirectedCaret::new(TextPosition::new(0, 2), Some(TextPosition::new(0, 2)))
    );
}

#[test]
fn multi_cursor_insertions_keep_each_anchor_at_its_own_insert() {
    let (mut engine, buffer) = engine_with("abcde");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 1), Some(TextPosition::new(0, 1))),
                DirectedCaret::new(TextPosition::new(0, 3), Some(TextPosition::new(0, 3))),
            ],
        )
        .unwrap();
    engine
        .apply_edits(
            buffer,
            vec![
                TextEdit::insert(TextPosition::new(0, 1), "X"),
                TextEdit::insert(TextPosition::new(0, 3), "Y"),
            ],
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "aXbcYde");
    let carets = engine.directed_carets(buffer).unwrap();
    assert_eq!(
        carets,
        vec![
            DirectedCaret::new(TextPosition::new(0, 2), Some(TextPosition::new(0, 1))),
            DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 4))),
        ]
    );
}

#[test]
fn shorter_replacement_maps_later_caret_offsets_without_wrap() {
    let (mut engine, buffer) = engine_with("abcdef");
    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(
                TextPosition::new(0, 5),
                Some(TextPosition::new(0, 4)),
            )],
        )
        .unwrap();
    engine
        .apply_edits(
            buffer,
            vec![TextEdit::new(
                TextRange::new(TextPosition::new(0, 1), TextPosition::new(0, 4)),
                "Z",
            )],
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "aZef");
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0],
        DirectedCaret::new(TextPosition::new(0, 3), Some(TextPosition::new(0, 2)))
    );
}

#[test]
fn empty_selection_clears_anchors_but_retains_heads() {
    let (mut engine, buffer) = engine_with("abcdef");
    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(
                TextPosition::new(0, 4),
                Some(TextPosition::new(0, 1)),
            )],
        )
        .unwrap();
    engine.set_selections(buffer, Vec::new()).unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![DirectedCaret::new(TextPosition::new(0, 4), None)]
    );
}

#[test]
fn grouped_edits_undo_and_redo_as_one_caret_aware_unit() {
    let (mut engine, buffer) = engine_with("ab");
    let group = Uuid::now_v7();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 2), "1"),
            TransactionSource::User,
            Some(group),
            None,
        )
        .unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 3), "2"),
            TransactionSource::User,
            Some(group),
            None,
        )
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "ab12");
    assert_eq!(engine.undo_len(buffer).unwrap(), 1);
    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "ab");
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![DirectedCaret::new(TextPosition::new(0, 0), None)]
    );
    engine.redo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "ab12");
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![DirectedCaret::new(TextPosition::new(0, 0), None)]
    );
}

#[test]
fn none_and_noncontiguous_groups_remain_distinct() {
    let (mut engine, buffer) = engine_with("a");
    let group = Uuid::now_v7();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 1), "b"),
            TransactionSource::User,
            Some(group),
            None,
        )
        .unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 2), "c"),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 3), "d"),
            TransactionSource::User,
            Some(group),
            None,
        )
        .unwrap();
    assert_eq!(engine.undo_len(buffer).unwrap(), 3);
    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "abc");
    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "ab");
}

#[test]
fn failed_batch_preserves_text_carets_and_history() {
    let (mut engine, buffer) = engine_with("abc");
    let group = Uuid::now_v7();
    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(
                TextPosition::new(0, 2),
                Some(TextPosition::new(0, 1)),
            )],
        )
        .unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 2), "x"),
            TransactionSource::User,
            Some(group),
            None,
        )
        .unwrap();
    let before = engine.directed_carets(buffer).unwrap();
    let history = engine.undo_len(buffer).unwrap();
    assert!(
        engine
            .apply_edits(
                buffer,
                vec![TextEdit::new(
                    TextRange::new(TextPosition::new(0, 99), TextPosition::new(0, 99)),
                    "bad"
                )],
                TransactionSource::User,
                Some(group),
                None
            )
            .is_err()
    );
    assert_eq!(engine.text(buffer).unwrap(), "abxc");
    assert_eq!(engine.directed_carets(buffer).unwrap(), before);
    assert_eq!(engine.undo_len(buffer).unwrap(), history);
}

#[test]
fn mapping_preserves_unicode_and_crlf_byte_boundaries() {
    let (mut engine, buffer) = engine_with("a\r\nβc");
    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(
                TextPosition::new(1, 2),
                Some(TextPosition::new(1, 0)),
            )],
        )
        .unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 1), "é"),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "aé\r\nβc");
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0],
        DirectedCaret::new(TextPosition::new(1, 2), Some(TextPosition::new(1, 0)))
    );
}

#[test]
fn invalid_mid_scalar_and_bad_anchor_leave_every_entry_unchanged() {
    let (mut engine, buffer) = engine_with("éx");
    let original = engine.directed_carets(buffer).unwrap();
    assert!(
        engine
            .set_directed_carets(
                buffer,
                vec![
                    DirectedCaret::new(TextPosition::new(0, 2), None),
                    DirectedCaret::new(TextPosition::new(0, 1), None),
                ],
            )
            .is_err()
    );
    assert_eq!(engine.directed_carets(buffer).unwrap(), original);
    assert!(
        engine
            .set_directed_carets(
                buffer,
                vec![DirectedCaret::new(
                    TextPosition::new(0, 2),
                    Some(TextPosition::new(0, 99)),
                )],
            )
            .is_err()
    );
    assert_eq!(engine.directed_carets(buffer).unwrap(), original);
}

#[test]
fn directed_projection_preserves_forward_backward_and_zero_width_ranges() {
    let (mut engine, buffer) = engine_with("éabcd");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 6), Some(TextPosition::new(0, 2))),
                DirectedCaret::new(TextPosition::new(0, 2), Some(TextPosition::new(0, 6))),
                DirectedCaret::new(TextPosition::new(0, 4), Some(TextPosition::new(0, 4))),
            ],
        )
        .unwrap();
    let projection = engine
        .viewport_projection(EditorViewportRequest {
            buffer_id: buffer,
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 500,
                height_px: 64,
            },
        })
        .unwrap();
    assert_eq!(projection.selections.len(), 3);
    assert_eq!(projection.selections[0].start.byte_offset, Some(2));
    assert_eq!(projection.selections[0].end.byte_offset, Some(6));
    assert_eq!(projection.selections[0].start.utf16_offset, Some(1));
    assert_eq!(projection.selections[0].end.utf16_offset, Some(5));
    assert_eq!(projection.selections[1].start.byte_offset, Some(2));
    assert_eq!(projection.selections[1].end.byte_offset, Some(6));
    assert_eq!(projection.selections[2].start.byte_offset, Some(4));
    assert_eq!(projection.selections[2].end.byte_offset, Some(4));
}

#[test]
fn mapping_matrix_and_same_offset_insertions_are_deterministic() {
    let (mut engine, buffer) = engine_with("abcdef");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 1), None),
                DirectedCaret::new(TextPosition::new(0, 2), None),
                DirectedCaret::new(TextPosition::new(0, 3), None),
                DirectedCaret::new(TextPosition::new(0, 4), None),
                DirectedCaret::new(TextPosition::new(0, 5), None),
            ],
        )
        .unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::new(
                TextRange::new(TextPosition::new(0, 2), TextPosition::new(0, 4)),
                "XYZ",
            ),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "abXYZef");
    assert_eq!(
        engine
            .cursors(buffer)
            .unwrap()
            .into_iter()
            .map(|c| c.position)
            .collect::<Vec<_>>(),
        vec![
            TextPosition::new(0, 1),
            TextPosition::new(0, 5),
            TextPosition::new(0, 5),
            TextPosition::new(0, 5),
            TextPosition::new(0, 6)
        ]
    );

    let (mut engine, buffer) = engine_with("ab");
    engine
        .set_cursors(
            buffer,
            vec![Cursor {
                position: TextPosition::new(0, 1),
            }],
        )
        .unwrap();
    engine
        .apply_edits(
            buffer,
            vec![
                TextEdit::insert(TextPosition::new(0, 1), "X"),
                TextEdit::insert(TextPosition::new(0, 1), "Y"),
            ],
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "aXYb");
    assert_eq!(
        engine.primary_cursor(buffer).unwrap(),
        TextPosition::new(0, 3)
    );
}

#[test]
fn anchor_mapping_matrix_preserves_left_affinity_at_all_replacement_points() {
    let (mut engine, buffer) = engine_with("abcdef");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 1), Some(TextPosition::new(0, 1))),
                DirectedCaret::new(TextPosition::new(0, 2), Some(TextPosition::new(0, 2))),
                DirectedCaret::new(TextPosition::new(0, 3), Some(TextPosition::new(0, 3))),
                DirectedCaret::new(TextPosition::new(0, 4), Some(TextPosition::new(0, 4))),
                DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 5))),
            ],
        )
        .unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::new(
                TextRange::new(TextPosition::new(0, 2), TextPosition::new(0, 4)),
                "XYZ",
            ),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(0, 1), Some(TextPosition::new(0, 1))),
            DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 2))),
            DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 2))),
            DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 5))),
            DirectedCaret::new(TextPosition::new(0, 6), Some(TextPosition::new(0, 6))),
        ]
    );
}

#[test]
fn failed_different_group_does_not_break_original_group_coalescing() {
    let (mut engine, buffer) = engine_with("a");
    let original = Uuid::now_v7();
    let different = Uuid::now_v7();
    engine
        .set_cursors(
            buffer,
            vec![Cursor {
                position: TextPosition::new(0, 1),
            }],
        )
        .unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 1), "b"),
            TransactionSource::User,
            Some(original),
            None,
        )
        .unwrap();
    assert!(
        engine
            .apply_edit(
                buffer,
                TextEdit::new(
                    TextRange::new(TextPosition::new(0, 99), TextPosition::new(0, 99)),
                    "bad",
                ),
                TransactionSource::User,
                Some(different),
                None,
            )
            .is_err()
    );
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 2), "c"),
            TransactionSource::User,
            Some(original),
            None,
        )
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "abc");
    assert_eq!(engine.undo_len(buffer).unwrap(), 1);
    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "a");
    assert_eq!(
        engine.primary_cursor(buffer).unwrap(),
        TextPosition::new(0, 1)
    );
    engine.redo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "abc");
    assert_eq!(
        engine.primary_cursor(buffer).unwrap(),
        TextPosition::new(0, 3)
    );
}

#[test]
fn grouped_moving_carets_restore_first_pre_and_final_post_exactly() {
    let (mut engine, buffer) = engine_with("abcd");
    let group = Uuid::now_v7();
    let before = vec![
        DirectedCaret::new(TextPosition::new(0, 1), Some(TextPosition::new(0, 0))),
        DirectedCaret::new(TextPosition::new(0, 3), Some(TextPosition::new(0, 4))),
    ];
    engine.set_directed_carets(buffer, before.clone()).unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 0), "X"),
            TransactionSource::User,
            Some(group),
            None,
        )
        .unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 0), "Y"),
            TransactionSource::User,
            Some(group),
            None,
        )
        .unwrap();
    let final_carets = engine.directed_carets(buffer).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "YXabcd");
    assert_eq!(
        final_carets,
        vec![
            DirectedCaret::new(TextPosition::new(0, 3), Some(TextPosition::new(0, 0))),
            DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 6))),
        ]
    );
    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "abcd");
    assert_eq!(engine.directed_carets(buffer).unwrap(), before);
    engine.redo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "YXabcd");
    assert_eq!(engine.directed_carets(buffer).unwrap(), final_carets);
}

#[test]
fn multi_caret_backward_undo_redo_restores_exact_directions() {
    let (mut engine, buffer) = engine_with("abcdef");
    let before = vec![
        DirectedCaret::new(TextPosition::new(0, 4), Some(TextPosition::new(0, 1))),
        DirectedCaret::new(TextPosition::new(0, 2), Some(TextPosition::new(0, 5))),
    ];
    engine.set_directed_carets(buffer, before.clone()).unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 0), "Q"),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    let after = engine.directed_carets(buffer).unwrap();
    assert_eq!(
        after,
        vec![
            DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 2))),
            DirectedCaret::new(TextPosition::new(0, 3), Some(TextPosition::new(0, 6)))
        ]
    );
    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.directed_carets(buffer).unwrap(), before);
    engine.redo(buffer, None).unwrap();
    assert_eq!(engine.directed_carets(buffer).unwrap(), after);
}

#[test]
fn history_navigation_breaks_coalescing_and_rebranch_clears_redo() {
    let (mut engine, buffer) = engine_with("a");
    let first = Uuid::now_v7();
    let second = Uuid::now_v7();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 1), "b"),
            TransactionSource::User,
            Some(first),
            None,
        )
        .unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 2), "c"),
            TransactionSource::User,
            Some(second),
            None,
        )
        .unwrap();
    assert_eq!(engine.undo_len(buffer).unwrap(), 2);
    engine.undo(buffer, None).unwrap();
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 2), "x"),
            TransactionSource::User,
            Some(first),
            None,
        )
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "abx");
    assert_eq!(engine.redo_len(buffer).unwrap(), 0);
    assert_eq!(engine.undo_len(buffer).unwrap(), 2);
}

#[test]
fn lease_survives_history_eviction_and_new_group_starts_undo_suffix() {
    let policy = legion_editor::SnapshotRetentionPolicy {
        max_snapshot_count: 5,
        max_estimated_bytes: usize::MAX,
        eviction_preference: legion_editor::SnapshotEvictionPreference::UndoThenRedo,
    };
    let mut engine = EditorEngine::with_snapshot_retention_policy(policy);
    let b1 = engine
        .open_buffer(WorkspaceId(1), FileId(10), "a", "abcdef")
        .unwrap();
    let b2 = engine
        .open_buffer(WorkspaceId(1), FileId(11), "b", "uvwxyz")
        .unwrap();
    let first = Uuid::now_v7();
    engine
        .apply_edit(
            b1,
            TextEdit::insert(TextPosition::new(0, 6), "1"),
            TransactionSource::User,
            Some(first),
            None,
        )
        .unwrap();
    let lease = engine.lease_snapshot(b1, SnapshotConsumerKind::Ui).unwrap();
    engine
        .apply_edit(
            b2,
            TextEdit::insert(TextPosition::new(0, 6), "2"),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    engine
        .apply_edit(
            b2,
            TextEdit::insert(TextPosition::new(0, 7), "4"),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    engine
        .apply_edit(
            b1,
            TextEdit::insert(TextPosition::new(0, 7), "3"),
            TransactionSource::User,
            Some(first),
            None,
        )
        .unwrap();
    assert_eq!(engine.undo_len(b1).unwrap(), 0);
    let chunk = engine
        .read_snapshot_lease_chunk(
            lease.lease_id,
            b1,
            lease.snapshot_id,
            lease.buffer_version,
            0,
        )
        .unwrap();
    assert!(chunk.text.contains("abcdef"));
    let next = Uuid::now_v7();
    engine
        .apply_edit(
            b1,
            TextEdit::insert(TextPosition::new(0, 8), "N"),
            TransactionSource::User,
            Some(next),
            None,
        )
        .unwrap();
    assert_eq!(engine.text(b1).unwrap(), "abcdef13N");
    assert_eq!(engine.undo_len(b1).unwrap(), 1);
    engine.undo(b1, None).unwrap();
    assert_eq!(engine.text(b1).unwrap(), "abcdef13");
    assert_eq!(engine.redo_len(b1).unwrap(), 1);
    engine.redo(b1, None).unwrap();
    assert_eq!(engine.text(b1).unwrap(), "abcdef13N");
    assert_eq!(engine.undo_len(b1).unwrap(), 1);
    let retained_before_release = engine.retained_snapshot_count();
    engine.release_snapshot_lease(lease.lease_id).unwrap();
    assert_eq!(
        engine.retained_snapshot_count(),
        retained_before_release.saturating_sub(1),
        "released lease must remove its drained history descriptor while preserving live history"
    );
}

#[test]
fn streamed_large_unicode_buffer_maps_without_full_cache_materialization() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.txt");
    let mut text = String::from("😀\n");
    text.push_str(&"x".repeat(5 * 1024 * 1024));
    std::fs::write(&path, text).unwrap();
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer_streaming(WorkspaceId(1), FileId(50), "large.txt", &path)
        .unwrap();
    assert!(engine.text(buffer).is_err());
    engine
        .set_cursors(
            buffer,
            vec![Cursor {
                position: TextPosition::new(0, 4),
            }],
        )
        .unwrap();
    engine
        .move_to_boundary(buffer, BoundaryKind::LineEnd, false)
        .unwrap();
    assert_eq!(
        engine.primary_cursor(buffer).unwrap(),
        TextPosition::new(0, 4)
    );
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 4), "é"),
            TransactionSource::User,
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        engine.primary_cursor(buffer).unwrap(),
        TextPosition::new(0, 6)
    );
    let projection = engine
        .viewport_projection(EditorViewportRequest {
            buffer_id: buffer,
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 400,
                height_px: 32,
            },
        })
        .unwrap();
    assert_eq!(projection.cursor.utf16_offset, Some(3));
    assert_eq!(
        projection.mode,
        legion_protocol::ViewportProjectionMode::StreamingLargeFile
    );
}
