use devil_editor::{
    EditorEngine, EditorError, SnapshotEvictionPreference, SnapshotRetentionPolicy, TextPosition,
};
use devil_protocol::{FileId, TransactionSource, WorkspaceId};
use devil_text::{TextEdit, TextError, TextRange};

fn small_policy(max_snapshot_count: usize) -> SnapshotRetentionPolicy {
    SnapshotRetentionPolicy {
        max_snapshot_count,
        max_estimated_bytes: usize::MAX,
        eviction_preference: SnapshotEvictionPreference::UndoThenRedo,
    }
}

#[test]
fn invalid_batch_rolls_back_live_text_version_dirty_and_side_effects() {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(1), "atomic.txt", "hello")
        .expect("open buffer");
    let version = engine.buffer_version(buffer).expect("version");
    let dirty = engine.is_dirty(buffer).expect("dirty");
    let undo_len = engine.undo_len(buffer).expect("undo len");
    let redo_len = engine.redo_len(buffer).expect("redo len");
    let pinned = engine.pinned_snapshot_count();
    let pending = engine.pending_save_requests().len();
    let log_len = engine.transaction_log().len();

    let result = engine.apply_edits(
        buffer,
        vec![
            TextEdit::insert(TextPosition::new(0, 5), "!"),
            TextEdit::insert(TextPosition::new(0, 99), "boom"),
        ],
        TransactionSource::User,
        None,
        None,
    );

    assert!(matches!(
        result,
        Err(EditorError::Text(TextError::ColumnOutOfBounds { .. }))
    ));
    assert_eq!(engine.text(buffer).expect("text"), "hello");
    assert_eq!(engine.buffer_version(buffer).expect("version"), version);
    assert_eq!(engine.is_dirty(buffer).expect("dirty"), dirty);
    assert_eq!(engine.undo_len(buffer).expect("undo len"), undo_len);
    assert_eq!(engine.redo_len(buffer).expect("redo len"), redo_len);
    assert_eq!(engine.pinned_snapshot_count(), pinned);
    assert_eq!(engine.pending_save_requests().len(), pending);
    assert_eq!(engine.transaction_log().len(), log_len);
}

#[test]
fn failed_oversize_batch_preserves_undo_stack_and_transaction_log() {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(2), "oversize.txt", "seed")
        .expect("open buffer");
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 4), "!"),
            TransactionSource::User,
            None,
            None,
        )
        .expect("first edit");
    let undo_len = engine.undo_len(buffer).expect("undo len");
    let log_len = engine.transaction_log().len();
    let before = engine.text(buffer).expect("text").to_string();

    let result = engine.apply_edit(
        buffer,
        TextEdit::insert(
            TextPosition::new(0, before.len()),
            "x".repeat(devil_text::DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES),
        ),
        TransactionSource::User,
        None,
        None,
    );

    assert!(matches!(
        result,
        Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
    ));
    assert_eq!(engine.text(buffer).expect("text"), before);
    assert_eq!(engine.undo_len(buffer).expect("undo len"), undo_len);
    assert_eq!(engine.transaction_log().len(), log_len);
}

#[test]
fn retention_budget_evicts_oldest_unpinned_undo_snapshots() {
    let mut engine = EditorEngine::with_snapshot_retention_policy(small_policy(4));
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(3), "retention.txt", "seed")
        .expect("open buffer");

    for _ in 0..8 {
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 0), "x"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("edit");
    }

    assert!(engine.retained_snapshot_count() <= 4);
    assert!(engine.undo_len(buffer).expect("undo len") <= 3);
    assert_eq!(engine.text(buffer).expect("text"), "xxxxxxxxseed");
}

#[test]
fn current_and_pending_save_snapshots_remain_pinned_under_retention_pressure() {
    let mut engine = EditorEngine::with_snapshot_retention_policy(small_policy(2));
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(4), "pins.txt", "seed")
        .expect("open buffer");
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 4), "!"),
            TransactionSource::User,
            None,
            None,
        )
        .expect("edit");
    let save = engine.request_save(buffer, None).expect("save");
    let pending_snapshot_id = save.snapshot_id;
    let current_snapshot_id = engine
        .current_snapshot(buffer)
        .expect("current")
        .snapshot_id;

    for _ in 0..8 {
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 0), "x"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("edit");
    }

    assert!(
        engine
            .pending_save_requests()
            .iter()
            .any(|request| request.snapshot_id == pending_snapshot_id)
    );
    assert_ne!(
        engine
            .current_snapshot(buffer)
            .expect("current")
            .snapshot_id,
        pending_snapshot_id
    );
    assert_ne!(
        engine
            .current_snapshot(buffer)
            .expect("current")
            .snapshot_id,
        current_snapshot_id
    );
    assert!(engine.pinned_snapshot_count() >= 2);
}

#[test]
fn protocol_descriptor_preserves_utf16_ranges_for_surrogate_pairs() {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(5), "emoji.txt", "a🦀b")
        .expect("open buffer");

    let tx = engine
        .apply_edit(
            buffer,
            TextEdit::new(
                TextRange::new(TextPosition::new(0, 1), TextPosition::new(0, 5)),
                "🙂",
            ),
            TransactionSource::User,
            None,
            None,
        )
        .expect("emoji edit");
    let descriptor = tx.to_protocol_descriptor();
    let changed = descriptor.changed_ranges[0];

    assert_eq!(changed.byte_range.start, 1);
    assert_eq!(changed.byte_range.end, 5);
    assert_eq!(changed.utf16_range.start.line, 0);
    assert_eq!(changed.utf16_range.start.character, 1);
    assert_eq!(changed.utf16_range.end.line, 0);
    assert_eq!(changed.utf16_range.end.character, 3);
    assert_eq!(descriptor.causality_id.0, tx.causality_trace_id);
    assert_ne!(descriptor.causality_id.0, tx.transaction_id);
}
