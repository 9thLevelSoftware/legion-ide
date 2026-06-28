use legion_editor::{
    BufferMode, EditorEngine, EditorError, EditorThresholds, SnapshotEvictionPreference,
    SnapshotRetentionPolicy, TextPosition,
};
use legion_protocol::{
    EditorViewportRequest, FileId, TransactionSource, ViewportDimensions, ViewportScroll,
    WorkspaceId,
};
use legion_text::{TextEdit, TextError, TextRange};

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
fn batch_edit_deltas_track_cumulative_shift_from_lower_edits() {
    // A lower-offset edit that changes length must shift the recorded range of
    // every higher-offset edit so each delta points at the post-edit buffer.
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(7), "batch.txt", "hello world")
        .expect("open buffer");

    let tx = engine
        .apply_edits(
            buffer,
            vec![
                // Replace "hello" (5 bytes) with "hi" (2 bytes): shrinks by 3.
                TextEdit::new(
                    TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
                    "hi",
                ),
                // Replace "world" (5 bytes) at original offset 6 with "WORLD!".
                TextEdit::new(
                    TextRange::new(TextPosition::new(0, 6), TextPosition::new(0, 11)),
                    "WORLD!",
                ),
            ],
            TransactionSource::User,
            None,
            None,
        )
        .expect("batch edit");

    assert_eq!(engine.text(buffer).expect("text"), "hi WORLD!");
    assert_eq!(tx.deltas.len(), 2);
    // Deltas are ordered ascending by final start offset.
    assert_eq!(tx.deltas[0].byte_range.start, 0);
    assert_eq!(tx.deltas[0].byte_range.end, 2);
    // Higher edit's recorded range reflects the -3 shift from the lower edit.
    assert_eq!(tx.deltas[1].byte_range.start, 3);
    assert_eq!(tx.deltas[1].byte_range.end, 9);
}

#[test]
fn oversize_edit_transitions_buffer_into_degraded_mode_without_losing_undo_history() {
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

    engine
        .apply_edit(
            buffer,
            TextEdit::insert(
                TextPosition::new(0, before.len()),
                "x".repeat(legion_text::DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES),
            ),
            TransactionSource::User,
            None,
            None,
        )
        .expect("oversize edit should degrade instead of failing");

    assert_eq!(
        engine.buffer_mode(buffer).expect("mode"),
        BufferMode::Degraded
    );
    assert!(matches!(
        engine.text(buffer),
        Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
    ));
    assert_eq!(engine.undo_len(buffer).expect("undo len"), undo_len + 1);
    assert_eq!(engine.transaction_log().len(), log_len + 1);

    engine
        .undo(buffer, None)
        .expect("undo back to small buffer");
    assert_eq!(
        engine.buffer_mode(buffer).expect("mode after undo"),
        BufferMode::Normal
    );
    assert_eq!(engine.text(buffer).expect("text after undo"), before);
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

#[test]
fn large_buffers_open_in_degraded_mode_instead_of_failing() {
    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: 64,
        retention_budget_snapshots: 8,
    });
    let text = (0..32)
        .map(|line| format!("line-{line:02}-{}\n", "x".repeat(16)))
        .collect::<String>();

    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(6), "large.txt", text)
        .expect("open degraded buffer");

    assert_eq!(
        engine.buffer_mode(buffer).expect("mode"),
        BufferMode::Degraded
    );
    assert!(matches!(
        engine.text(buffer),
        Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
    ));
}

#[test]
fn undo_redo_remains_correct_for_degraded_chunk_boundary_edits() {
    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: 64,
        retention_budget_snapshots: 16,
    });
    let line_body = "a".repeat(2048);
    let text = (0..96)
        .map(|line| format!("{line:03}:{line_body}\n"))
        .collect::<String>();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(7), "chunked.txt", text)
        .expect("open degraded buffer");

    let chunk = engine
        .snapshot_chunk_descriptors(buffer)
        .expect("chunk descriptors")
        .into_iter()
        .find(|chunk| chunk.line_range.start <= 48 && chunk.line_range.end > 48)
        .expect("chunk for target line");
    assert!(
        chunk.chunk_index > 0,
        "target line should land beyond the first chunk"
    );

    let line_before = format!("{:03}:{}", 48, line_body);
    let expected_after = format!("{}!{}", &line_before[..10], &line_before[10..]);

    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(48, 10), "!"),
            TransactionSource::User,
            None,
            None,
        )
        .expect("apply chunk edit");

    let after = engine
        .viewport_projection(EditorViewportRequest {
            buffer_id: buffer,
            scroll: ViewportScroll {
                top_line: 48,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 16,
            },
        })
        .expect("viewport after edit");
    assert_eq!(after.line_slices[0].visible_text, expected_after);

    engine.undo(buffer, None).expect("undo");
    let undone = engine
        .viewport_projection(EditorViewportRequest {
            buffer_id: buffer,
            scroll: ViewportScroll {
                top_line: 48,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 16,
            },
        })
        .expect("viewport after undo");
    assert_eq!(undone.line_slices[0].visible_text, line_before);

    engine.redo(buffer, None).expect("redo");
    let redone = engine
        .viewport_projection(EditorViewportRequest {
            buffer_id: buffer,
            scroll: ViewportScroll {
                top_line: 48,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 16,
            },
        })
        .expect("viewport after redo");
    assert_eq!(redone.line_slices[0].visible_text, expected_after);
}
