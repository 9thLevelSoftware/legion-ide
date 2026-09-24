use legion_editor::{DeleteDirection, DirectedCaret, EditorEngine, TextPosition};
use legion_protocol::{CorrelationId, FileId, WorkspaceId};

fn engine_with(text: &str) -> (EditorEngine, legion_protocol::BufferId) {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(1), "delete.txt", text)
        .expect("buffer opens");
    (engine, buffer)
}

#[test]
fn directed_deletion_unions_reversed_touching_and_coincident_ranges() {
    let (mut engine, buffer) = engine_with("0123456789");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 2))),
                DirectedCaret::new(TextPosition::new(0, 7), Some(TextPosition::new(0, 5))),
                DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 2))),
                DirectedCaret::new(TextPosition::new(0, 9), None),
            ],
        )
        .unwrap();

    let record = engine
        .delete_directed_carets(buffer, DeleteDirection::Forward, Some(CorrelationId(7)))
        .unwrap();
    assert!(record.is_some());
    assert_eq!(engine.text(buffer).unwrap(), "0178");
    assert_eq!(
        engine
            .directed_carets(buffer)
            .unwrap()
            .iter()
            .map(|caret| caret.head)
            .collect::<Vec<_>>(),
        vec![
            TextPosition::new(0, 2),
            TextPosition::new(0, 2),
            TextPosition::new(0, 2),
            TextPosition::new(0, 4),
        ]
    );
    assert!(
        engine
            .directed_carets(buffer)
            .unwrap()
            .iter()
            .all(|c| c.anchor.is_none())
    );
}

#[test]
fn forward_and_backward_delete_adjacent_graphemes_and_noop_at_edges() {
    let (mut engine, buffer) = engine_with("abc");
    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(TextPosition::new(0, 1), None)],
        )
        .unwrap();
    engine
        .delete_directed_carets(buffer, DeleteDirection::Backward, None)
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "bc");

    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(TextPosition::new(0, 0), None)],
        )
        .unwrap();
    let version = engine.buffer_version(buffer).unwrap();
    let transactions = engine.transaction_log().len();
    let events = engine.drain_transaction_events();
    assert_eq!(events.descriptors.len(), 1);
    let carets = engine.directed_carets(buffer).unwrap();
    assert!(
        engine
            .delete_directed_carets(buffer, DeleteDirection::Backward, None)
            .unwrap()
            .is_none()
    );
    assert_eq!(engine.buffer_version(buffer).unwrap(), version);
    assert_eq!(engine.transaction_log().len(), transactions);
    assert_eq!(engine.directed_carets(buffer).unwrap(), carets);
    let after_events = engine.drain_transaction_events();
    assert!(after_events.descriptors.is_empty());
    assert_eq!(after_events.dropped_before_drain, 0);

    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "abc");
    engine.redo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "bc");

    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(TextPosition::new(0, 0), None)],
        )
        .unwrap();
    engine
        .delete_directed_carets(buffer, DeleteDirection::Forward, None)
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "c");
}

#[test]
fn interior_unicode_cluster_is_deleted_as_a_whole_and_history_restores_carets() {
    let (mut engine, buffer) = engine_with("a\u{301}b\r\nc");
    let original = vec![
        DirectedCaret::new(TextPosition::new(0, 1), None),
        DirectedCaret::new(TextPosition::new(0, 4), Some(TextPosition::new(0, 3))),
    ];
    engine
        .set_directed_carets(buffer, original.clone())
        .unwrap();
    engine
        .delete_directed_carets(buffer, DeleteDirection::Backward, Some(CorrelationId(9)))
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "\r\nc");
    let final_carets = engine.directed_carets(buffer).unwrap();
    engine.undo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "a\u{301}b\r\nc");
    assert_eq!(engine.directed_carets(buffer).unwrap(), original);
    engine.redo(buffer, None).unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "\r\nc");
    assert_eq!(engine.directed_carets(buffer).unwrap(), final_carets);
}

#[test]
fn forward_interior_cluster_and_empty_anchor_delete_the_same_whole_cluster() {
    let (mut engine, buffer) = engine_with("a\u{301}b");
    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(
                TextPosition::new(0, 1),
                Some(TextPosition::new(0, 1)),
            )],
        )
        .unwrap();
    engine
        .delete_directed_carets(buffer, DeleteDirection::Forward, None)
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "b");
}

#[test]
fn crlf_is_deleted_as_one_grapheme_cluster() {
    let (mut engine, buffer) = engine_with("a\r\nb");
    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(TextPosition::new(1, 0), None)],
        )
        .unwrap();
    engine
        .delete_directed_carets(buffer, DeleteDirection::Backward, None)
        .unwrap();
    assert_eq!(engine.text(buffer).unwrap(), "ab");
}

#[test]
fn streamed_large_buffer_deletion_avoids_full_text_cache() {
    let mut engine = EditorEngine::new();
    let text = format!("{}🙂tail", "x".repeat(5 * 1024 * 1024 + 32));
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(2), "large.txt", text)
        .unwrap();
    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(
                TextPosition::new(0, 5 * 1024 * 1024 + 32),
                None,
            )],
        )
        .unwrap();
    let before_len = 5 * 1024 * 1024 + 32 + "🙂tail".len();
    engine
        .delete_directed_carets(buffer, DeleteDirection::Forward, None)
        .unwrap();
    assert_eq!(engine.buffer_version(buffer).unwrap().0, 1);
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0].head.column,
        5 * 1024 * 1024 + 32
    );
    assert!(engine.text(buffer).is_err());
    let lease = engine
        .lease_snapshot(buffer, legion_protocol::SnapshotConsumerKind::Editor)
        .unwrap();
    let last_chunk = engine
        .read_snapshot_lease_chunk(
            lease.lease_id,
            buffer,
            lease.snapshot_id,
            lease.buffer_version,
            lease.chunk_count - 1,
        )
        .unwrap();
    assert!(last_chunk.text.ends_with("tail"));
    assert!(!last_chunk.text.contains('🙂'));
    let after_forward = engine.current_snapshot(buffer).unwrap().byte_len;
    assert_eq!(after_forward, before_len - "🙂".len());

    engine
        .delete_directed_carets(buffer, DeleteDirection::Backward, None)
        .unwrap();
    assert_eq!(engine.buffer_version(buffer).unwrap().0, 2);
    assert_eq!(
        engine.current_snapshot(buffer).unwrap().byte_len,
        before_len - "🙂".len() - 1
    );
    assert_eq!(
        engine.directed_carets(buffer).unwrap()[0].head.column,
        5 * 1024 * 1024 + 31
    );
    assert!(engine.text(buffer).is_err());
    let lease = engine
        .lease_snapshot(buffer, legion_protocol::SnapshotConsumerKind::Editor)
        .unwrap();
    let last_chunk = engine
        .read_snapshot_lease_chunk(
            lease.lease_id,
            buffer,
            lease.snapshot_id,
            lease.buffer_version,
            lease.chunk_count - 1,
        )
        .unwrap();
    assert!(last_chunk.text.ends_with("tail"));
}
