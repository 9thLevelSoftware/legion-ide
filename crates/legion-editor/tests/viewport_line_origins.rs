use legion_editor::{EditorEngine, EditorError};
use legion_protocol::{
    BufferId, EditorViewportRequest, FileId, ViewportDimensions, ViewportProjectionMode,
    ViewportScroll, WorkspaceId,
};
use legion_text::TextError;

fn request(buffer_id: BufferId, top_line: u32, height_px: u32) -> EditorViewportRequest {
    EditorViewportRequest {
        buffer_id,
        scroll: ViewportScroll {
            top_line,
            left_column: 0,
        },
        dimensions: ViewportDimensions {
            width_px: 800,
            height_px,
        },
    }
}

#[test]
fn line_metrics_report_independent_absolute_byte_and_utf16_origins() {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(
            WorkspaceId(1),
            FileId(1),
            "origins.txt",
            "é\r\n😀éx\r\n前🙂z\r\n",
        )
        .expect("open");

    let projection = engine
        .viewport_projection(request(buffer, 1, 32))
        .expect("viewport");

    assert_eq!(
        projection
            .line_slices
            .iter()
            .map(|slice| slice.line_number)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(projection.line_metrics[0].line_start_byte_offset, Some(4));
    assert_eq!(projection.line_metrics[0].line_start_utf16_offset, Some(3));
    assert_eq!(projection.line_metrics[1].line_start_byte_offset, Some(13));
    assert_eq!(projection.line_metrics[1].line_start_utf16_offset, Some(9));
}

#[test]
fn streamed_large_buffer_reports_exact_line_origins_without_full_cache() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("streamed-origins.txt");
    let prefix = "é\r\n😀éx\r\n前🙂z\r\n";
    let text = format!("{prefix}{}", "a\n".repeat(3 * 1024 * 1024));
    std::fs::write(&path, text).expect("write fixture");

    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer_streaming(WorkspaceId(1), FileId(2), "streamed-origins.txt", &path)
        .expect("streaming open");

    assert!(matches!(
        engine.text(buffer),
        Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
    ));
    let projection = engine
        .viewport_projection(request(buffer, 2, 16))
        .expect("viewport");

    assert_eq!(projection.mode, ViewportProjectionMode::StreamingLargeFile);
    assert_eq!(projection.line_metrics.len(), 1);
    assert_eq!(projection.line_metrics[0].byte_length, 8);
    assert_eq!(projection.line_metrics[0].utf16_length, 4);
    assert_eq!(projection.line_metrics[0].line_start_byte_offset, Some(13));
    assert_eq!(projection.line_metrics[0].line_start_utf16_offset, Some(9));
}
