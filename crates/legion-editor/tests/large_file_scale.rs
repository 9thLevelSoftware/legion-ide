use legion_editor::{BufferMode, EditorEngine, EditorThresholds, TextEdit, TextPosition};
use legion_protocol::{
    EditorViewportRequest, FileId, SnapshotConsumerKind, TransactionSource, ViewportDimensions,
    ViewportProjectionMode, ViewportScroll, WorkspaceId,
};

/// Build a deterministic string of exactly `byte_count` UTF-8 bytes.
fn large_text(byte_count: usize) -> String {
    let line = "abcdefghijklmnopqrstuvwxyz0123456789_|\n";
    let lines_needed = byte_count / line.len() + 1;
    let mut text = String::with_capacity(lines_needed * line.len());
    for _ in 0..lines_needed {
        text.push_str(line);
    }
    text.truncate(byte_count);
    while !text.is_char_boundary(text.len()) {
        text.pop();
    }
    text
}

/// SCALE.05 — large file opens in degraded mode with correct status metadata.
#[test]
fn large_file_opens_in_degraded_mode_with_file_size_status() {
    const THRESHOLD: usize = 512;

    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: THRESHOLD,
        retention_budget_snapshots: 8,
    });
    let text = large_text(THRESHOLD + 64);
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(1), "big.rs", text)
        .expect("open buffer");

    assert_eq!(
        engine.buffer_mode(buffer).expect("buffer mode"),
        BufferMode::Degraded,
        "file above threshold must open in Degraded mode"
    );

    let projection = engine
        .viewport_projection(EditorViewportRequest {
            buffer_id: buffer,
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 640,
            },
        })
        .expect("viewport projection");

    assert_eq!(
        projection.mode,
        ViewportProjectionMode::DegradedLargeFile,
        "projection mode must be DegradedLargeFile"
    );

    let status = projection
        .large_file_status
        .expect("large_file_status must be present for degraded buffers");
    assert_eq!(
        status.threshold_bytes, THRESHOLD as u64,
        "status threshold_bytes must match the configured threshold"
    );
    assert!(
        status.byte_len >= THRESHOLD as u64,
        "status byte_len ({}) must be >= threshold ({})",
        status.byte_len,
        THRESHOLD
    );
}

/// Normal (small) file must produce a Normal viewport projection with no large-file status.
#[test]
fn normal_file_has_no_large_file_status() {
    const THRESHOLD: usize = 1024;

    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: THRESHOLD,
        retention_budget_snapshots: 8,
    });
    let text = "fn main() {}\n";
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(2), "small.rs", text)
        .expect("open buffer");

    assert_eq!(
        engine.buffer_mode(buffer).expect("buffer mode"),
        BufferMode::Normal,
        "small file must open in Normal mode"
    );

    let projection = engine
        .viewport_projection(EditorViewportRequest {
            buffer_id: buffer,
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 640,
            },
        })
        .expect("viewport projection");

    assert_eq!(
        projection.mode,
        ViewportProjectionMode::Normal,
        "small file projection mode must be Normal"
    );
    assert!(
        projection.large_file_status.is_none(),
        "small file must have no large_file_status"
    );
}

/// SCALE.10 — reading a snapshot lease chunk with the NEW snapshot id (after an edit) must fail
/// with SnapshotLeaseStale, while reading with the original lease snapshot id still works.
#[test]
fn stale_snapshot_lease_after_large_file_edit() {
    const THRESHOLD: usize = 256;

    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: THRESHOLD,
        retention_budget_snapshots: 16,
    });
    let text = large_text(THRESHOLD + 128);
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(3), "leased.rs", text)
        .expect("open buffer");

    // Acquire a lease against the original snapshot.
    let lease = engine
        .lease_snapshot(buffer, SnapshotConsumerKind::Lsp)
        .expect("lease snapshot");
    let original_snapshot_id = lease.snapshot_id;
    let original_buffer_version = lease.buffer_version;

    // Advance the snapshot by editing.
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 0), "!"),
            TransactionSource::User,
            None,
            None,
        )
        .expect("apply edit");

    let current = engine
        .current_snapshot(buffer)
        .expect("current snapshot")
        .clone();
    assert_ne!(
        current.snapshot_id, original_snapshot_id,
        "edit must advance the snapshot"
    );

    // Reading through the lease with the NEW snapshot id must fail as stale.
    let stale_result = engine.read_snapshot_lease_chunk(
        lease.lease_id,
        lease.buffer_id,
        current.snapshot_id,        // wrong: this is the post-edit snapshot
        current.buffer_version,     // wrong: post-edit version
        0,
    );
    assert!(
        matches!(stale_result, Err(legion_editor::EditorError::SnapshotLeaseStale { .. })),
        "reading with post-edit snapshot id must return SnapshotLeaseStale, got: {stale_result:?}"
    );

    // Reading with the ORIGINAL lease identity must still succeed.
    let ok = engine.read_snapshot_lease_chunk(
        lease.lease_id,
        lease.buffer_id,
        original_snapshot_id,
        original_buffer_version,
        0,
    );
    assert!(
        ok.is_ok(),
        "reading with the original lease snapshot id must still succeed, got: {ok:?}"
    );
}

/// Editing a degraded buffer must keep it in Degraded mode.
#[test]
fn large_file_edit_preserves_degraded_mode() {
    const THRESHOLD: usize = 512;

    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: THRESHOLD,
        retention_budget_snapshots: 8,
    });
    let text = large_text(THRESHOLD + 64);
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(4), "persist.rs", text)
        .expect("open buffer");
    assert_eq!(engine.buffer_mode(buffer).expect("mode"), BufferMode::Degraded);

    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 0), "// edit\n"),
            TransactionSource::User,
            None,
            None,
        )
        .expect("apply edit");

    assert_eq!(
        engine.buffer_mode(buffer).expect("mode after edit"),
        BufferMode::Degraded,
        "editing a large file must keep it in Degraded mode"
    );
}

/// Requesting save for a degraded large file must assemble text from chunks and report correct sizes.
#[test]
fn large_file_save_assembles_from_chunks() {
    const THRESHOLD: usize = 512;

    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: THRESHOLD,
        retention_budget_snapshots: 8,
    });
    let text = large_text(THRESHOLD + 64);
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(5), "save_chunks.rs", text.clone())
        .expect("open buffer");

    let save = engine
        .request_save(buffer, None)
        .expect("save should assemble from chunks");

    assert_eq!(
        save.text, text,
        "save.text must match the original buffer text"
    );
    assert_eq!(
        save.payload_byte_len,
        save.text.len() as u64,
        "payload_byte_len must equal text byte length"
    );
}

/// Undo/redo on a large file must round-trip the content through save requests.
#[test]
fn large_file_undo_redo_round_trips_content() {
    const THRESHOLD: usize = 512;
    const INSERT: &str = "INSERTED_LINE\n";

    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: THRESHOLD,
        retention_budget_snapshots: 16,
    });
    let original = large_text(THRESHOLD + 64);
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(6), "roundtrip.rs", original.clone())
        .expect("open buffer");

    // Apply an edit and verify the save reflects the new content.
    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 0), INSERT),
            TransactionSource::User,
            None,
            None,
        )
        .expect("apply edit");

    let expected_after_edit = format!("{INSERT}{original}");
    let save_after_edit = engine
        .request_save(buffer, None)
        .expect("save after edit");
    assert_eq!(
        save_after_edit.text, expected_after_edit,
        "save after edit must include the inserted text"
    );

    // Undo and verify save reverts to original content.
    engine.undo(buffer, None).expect("undo");
    let save_after_undo = engine
        .request_save(buffer, None)
        .expect("save after undo");
    assert_eq!(
        save_after_undo.text, original,
        "save after undo must match original content"
    );

    // Redo and verify save returns to edited content.
    engine.redo(buffer, None).expect("redo");
    let save_after_redo = engine
        .request_save(buffer, None)
        .expect("save after redo");
    assert_eq!(
        save_after_redo.text, expected_after_edit,
        "save after redo must match edited content"
    );
}
