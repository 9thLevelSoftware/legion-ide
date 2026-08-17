//! Full, degraded and streaming buffers must be distinguishable (P1.F4.T3).
//!
//! A streamed buffer and a degraded one look identical once loaded — same
//! text, same deferred overlays — and only the streamed one never had the
//! whole file in memory. That difference decides what the editor can honestly
//! offer, so the projection has to carry it.

use legion_editor::{EditorEngine, EditorThresholds};
use legion_protocol::{
    BufferId, EditorViewportRequest, FileId, ViewportDimensions, ViewportProjectionMode,
    ViewportScroll, WorkspaceId,
};

fn viewport_request(buffer_id: BufferId) -> EditorViewportRequest {
    EditorViewportRequest {
        buffer_id,
        scroll: ViewportScroll {
            top_line: 0,
            left_column: 0,
        },
        dimensions: ViewportDimensions {
            width_px: 1_200,
            height_px: 400,
        },
    }
}

/// A buffer small enough for full features.
#[test]
fn a_small_buffer_projects_as_normal() {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(
            WorkspaceId(1),
            FileId(1),
            "small.txt",
            String::from("one\ntwo\n"),
        )
        .expect("open");
    let viewport = engine
        .viewport_projection(viewport_request(buffer))
        .expect("viewport");
    assert_eq!(viewport.mode, ViewportProjectionMode::Normal);
    assert!(!viewport.mode.defers_whole_file_work());
}

/// Large, but handed over as a complete `String`.
#[test]
fn a_large_in_memory_buffer_projects_as_degraded() {
    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: 64,
        ..EditorThresholds::default()
    });
    let text = "line of text\n".repeat(64);
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(2), "big.txt", text)
        .expect("open");
    let viewport = engine
        .viewport_projection(viewport_request(buffer))
        .expect("viewport");
    assert_eq!(
        viewport.mode,
        ViewportProjectionMode::DegradedLargeFile,
        "the whole file was in memory before the editor saw it"
    );
    assert!(!engine.buffer_is_streamed(buffer).expect("streamed flag"));
}

/// Large, and read from disk without ever being materialized.
#[test]
fn a_streamed_buffer_projects_as_streaming() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("streamed.txt");
    std::fs::write(&path, "line of text\n".repeat(64)).expect("fixture");

    let mut engine = EditorEngine::with_thresholds(EditorThresholds {
        large_file_threshold_bytes: 64,
        ..EditorThresholds::default()
    });
    let buffer = engine
        .open_buffer_streaming(WorkspaceId(1), FileId(3), "streamed.txt", &path)
        .expect("streaming open");
    let viewport = engine
        .viewport_projection(viewport_request(buffer))
        .expect("viewport");
    assert_eq!(
        viewport.mode,
        ViewportProjectionMode::StreamingLargeFile,
        "reporting this as merely degraded would claim the whole file is \
         available when it never was"
    );
    assert!(engine.buffer_is_streamed(buffer).expect("streamed flag"));
}

/// The regression a third variant invites.
///
/// Every consumer that deferred whole-file work asked `== DegradedLargeFile`.
/// Adding a mode without changing them would have started computing semantic
/// overlays on the largest files in the product — the exact case the deferral
/// exists for.
#[test]
fn both_large_file_modes_defer_whole_file_work() {
    assert!(ViewportProjectionMode::DegradedLargeFile.defers_whole_file_work());
    assert!(ViewportProjectionMode::StreamingLargeFile.defers_whole_file_work());
    assert!(!ViewportProjectionMode::Normal.defers_whole_file_work());
    assert!(!ViewportProjectionMode::BoundedSmallBuffer.defers_whole_file_work());
}

/// A small file opened through the streaming path is not a large file.
#[test]
fn streaming_a_small_file_still_projects_as_normal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("tiny.txt");
    std::fs::write(&path, "one\ntwo\n").expect("fixture");

    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer_streaming(WorkspaceId(1), FileId(4), "tiny.txt", &path)
        .expect("streaming open");
    let viewport = engine
        .viewport_projection(viewport_request(buffer))
        .expect("viewport");
    assert_eq!(
        viewport.mode,
        ViewportProjectionMode::Normal,
        "how a file was read does not by itself cost the user any features"
    );
}
