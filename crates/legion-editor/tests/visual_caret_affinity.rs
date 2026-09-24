use legion_editor::{
    BoundaryKind, Cursor, DirectedCaret, EditorEngine, EditorError, HorizontalDirection, TextEdit,
    TextPosition,
};
use legion_protocol::{
    CaretAffinity, EditorViewportRequest, FileId, SnapshotId, TransactionSource,
    ViewportDimensions, ViewportScroll, WorkspaceId,
};

fn engine_with(text: &str) -> (EditorEngine, legion_protocol::BufferId) {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(1), "affinity.txt", text)
        .expect("buffer opens");
    (engine, buffer)
}

fn current_identity(
    engine: &EditorEngine,
    buffer: legion_protocol::BufferId,
) -> (SnapshotId, legion_protocol::BufferVersion) {
    let snapshot = engine.current_snapshot(buffer).expect("current snapshot");
    (snapshot.snapshot_id, snapshot.buffer_version)
}

fn place_visual(
    engine: &mut EditorEngine,
    buffer: legion_protocol::BufferId,
    carets: Vec<DirectedCaret>,
) {
    let (snapshot_id, buffer_version) = current_identity(engine, buffer);
    engine
        .set_visual_directed_carets(buffer, snapshot_id, buffer_version, carets)
        .expect("visual caret placement");
}

#[test]
fn legacy_constructor_defaults_upstream_and_visual_placement_preserves_order() {
    let (mut engine, buffer) = engine_with("abcdef");
    let first =
        DirectedCaret::new(TextPosition::new(0, 4), None).with_affinity(CaretAffinity::Downstream);
    let second = DirectedCaret::new(TextPosition::new(0, 1), Some(TextPosition::new(0, 0)));

    assert_eq!(second.affinity, CaretAffinity::Upstream);
    let identity_before = current_identity(&engine, buffer);
    place_visual(&mut engine, buffer, vec![first, second]);

    assert_eq!(engine.directed_carets(buffer).unwrap(), vec![first, second]);
    assert_eq!(current_identity(&engine, buffer), identity_before);
    let projection = engine
        .viewport_projection(EditorViewportRequest {
            buffer_id: buffer,
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 320,
                height_px: 64,
            },
        })
        .expect("viewport projection");
    assert_eq!(
        projection.cursor_affinities,
        vec![CaretAffinity::Downstream, CaretAffinity::Upstream]
    );
}

#[test]
fn visual_placement_is_atomic_for_stale_snapshot_and_version() {
    let (mut engine, buffer) = engine_with("abcdef");
    let original = engine.directed_carets(buffer).unwrap();
    let original_text = engine.text(buffer).expect("text").to_owned();
    let original_history = engine.undo_len(buffer).expect("undo length");
    let original_transactions = engine.transaction_log().len();
    let (snapshot_id, buffer_version) = current_identity(&engine, buffer);
    let candidate =
        DirectedCaret::new(TextPosition::new(0, 3), None).with_affinity(CaretAffinity::Downstream);

    let error = engine
        .set_visual_directed_carets(
            buffer,
            SnapshotId(snapshot_id.0 + 1),
            buffer_version,
            vec![candidate],
        )
        .expect_err("wrong snapshot must be rejected");
    assert!(matches!(
        error,
        EditorError::StaleVisualCaretPlacement {
            expected_snapshot_id: SnapshotId(id),
            expected_buffer_version,
            ..
        } if id == snapshot_id.0 + 1 && expected_buffer_version == buffer_version
    ));
    assert_eq!(engine.directed_carets(buffer).unwrap(), original);
    assert_eq!(engine.text(buffer).unwrap(), original_text);
    assert_eq!(engine.undo_len(buffer).unwrap(), original_history);
    assert_eq!(engine.transaction_log().len(), original_transactions);

    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 0), "x"),
            TransactionSource::User,
            None,
            None,
        )
        .expect("edit advances version");
    let current = engine.directed_carets(buffer).unwrap();
    let (current_snapshot, current_version) = current_identity(&engine, buffer);
    let stale_version = legion_protocol::BufferVersion(current_version.0.saturating_sub(1));
    let error = engine
        .set_visual_directed_carets(buffer, current_snapshot, stale_version, vec![candidate])
        .expect_err("wrong version must be rejected");
    assert!(matches!(
        error,
        EditorError::StaleVisualCaretPlacement {
            expected_snapshot_id,
            expected_buffer_version,
            actual_snapshot_id,
            actual_buffer_version,
        } if expected_snapshot_id == current_snapshot
            && expected_buffer_version == stale_version
            && actual_snapshot_id == current_snapshot
            && actual_buffer_version == current_version
    ));
    assert_eq!(engine.directed_carets(buffer).unwrap(), current);
}

#[test]
fn invalid_visual_endpoint_does_not_partially_replace_ordered_carets() {
    let (mut engine, buffer) = engine_with("abcdef");
    let original = vec![
        DirectedCaret::new(TextPosition::new(0, 2), None).with_affinity(CaretAffinity::Downstream),
        DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 1))),
    ];
    place_visual(&mut engine, buffer, original.clone());
    let before = engine.directed_carets(buffer).unwrap();
    let (snapshot_id, buffer_version) = current_identity(&engine, buffer);

    let error = engine
        .set_visual_directed_carets(
            buffer,
            snapshot_id,
            buffer_version,
            vec![
                DirectedCaret::new(TextPosition::new(0, 3), None)
                    .with_affinity(CaretAffinity::Downstream),
                DirectedCaret::new(TextPosition::new(0, 99), None),
            ],
        )
        .expect_err("invalid head must be rejected");
    assert!(matches!(error, EditorError::Text(_)));
    assert_eq!(engine.directed_carets(buffer).unwrap(), before);

    let before_history = engine.undo_len(buffer).unwrap();
    let before_transactions = engine.transaction_log().len();
    let error = engine
        .set_visual_directed_carets(buffer, snapshot_id, buffer_version, Vec::new())
        .expect_err("empty visual caret vector must be rejected");
    assert!(matches!(error, EditorError::InvalidEdit(_)));
    assert_eq!(engine.directed_carets(buffer).unwrap(), before);
    assert_eq!(engine.undo_len(buffer).unwrap(), before_history);
    assert_eq!(engine.transaction_log().len(), before_transactions);

    let error = engine
        .set_visual_directed_carets(
            buffer,
            snapshot_id,
            buffer_version,
            vec![DirectedCaret::new(
                TextPosition::new(0, 3),
                Some(TextPosition::new(0, 99)),
            )],
        )
        .expect_err("invalid anchor must be rejected");
    assert!(matches!(error, EditorError::Text(_)));
    assert_eq!(engine.directed_carets(buffer).unwrap(), before);
}

#[test]
fn visual_placement_has_no_text_history_and_coordinate_setters_reset_affinity() {
    let (mut engine, buffer) = engine_with("abcdef");
    let before_text = engine.text(buffer).expect("text").to_owned();
    let before_history = engine.undo_len(buffer).expect("undo length");
    let before_transactions = engine.transaction_log().len();
    place_visual(
        &mut engine,
        buffer,
        vec![
            DirectedCaret::new(TextPosition::new(0, 2), None)
                .with_affinity(CaretAffinity::Downstream),
        ],
    );
    assert_eq!(engine.text(buffer).unwrap(), before_text);
    assert_eq!(engine.undo_len(buffer).unwrap(), before_history);
    assert_eq!(engine.transaction_log().len(), before_transactions);

    engine
        .set_cursors(
            buffer,
            vec![Cursor {
                position: TextPosition::new(0, 3),
            }],
        )
        .expect("coordinate setter");
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![DirectedCaret::new(TextPosition::new(0, 3), None)]
    );
}

#[test]
fn edit_resets_affinity_and_undo_redo_restore_the_captured_caret_vector() {
    let (mut engine, buffer) = engine_with("abcdef");
    let directed =
        DirectedCaret::new(TextPosition::new(0, 2), None).with_affinity(CaretAffinity::Downstream);
    place_visual(&mut engine, buffer, vec![directed]);

    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 0), "x"),
            TransactionSource::User,
            None,
            None,
        )
        .expect("edit");
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![DirectedCaret::new(TextPosition::new(0, 3), None)]
    );

    engine.undo(buffer, None).expect("undo");
    assert_eq!(engine.directed_carets(buffer).unwrap(), vec![directed]);
    engine.redo(buffer, None).expect("redo");
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![DirectedCaret::new(TextPosition::new(0, 3), None)]
    );
}

#[test]
fn horizontal_and_boundary_navigation_reset_visual_affinity() {
    let (mut engine, buffer) = engine_with("abcd\nefgh");
    place_visual(
        &mut engine,
        buffer,
        vec![
            DirectedCaret::new(TextPosition::new(0, 2), None)
                .with_affinity(CaretAffinity::Downstream),
        ],
    );
    engine
        .move_horizontally(buffer, HorizontalDirection::Right, false)
        .expect("horizontal movement");
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0].affinity,
        CaretAffinity::Upstream
    );

    place_visual(
        &mut engine,
        buffer,
        vec![
            DirectedCaret::new(TextPosition::new(1, 2), None)
                .with_affinity(CaretAffinity::Downstream),
        ],
    );
    engine
        .move_to_boundary(buffer, BoundaryKind::LineStart, false)
        .expect("boundary movement");
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0].affinity,
        CaretAffinity::Upstream
    );
}
