use legion_editor::{EditorEngine, EditorError};
use legion_protocol::{
    EditorViewportRequest, FileId, ViewportDimensions, ViewportProjectionMode, ViewportScroll,
    WorkspaceId,
};
use legion_text::{DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES, TextError};

const LARGE_TEXT_LINE: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n";
const LARGE_FILE_BYTES: usize = 100 * 1024 * 1024;

fn deterministic_large_text(byte_len: usize) -> String {
    let mut text = String::with_capacity(byte_len);
    while text.len() + LARGE_TEXT_LINE.len() <= byte_len {
        text.push_str(LARGE_TEXT_LINE);
    }
    while text.len() < byte_len {
        text.push('z');
    }
    text
}

#[test]
fn large_file_100mb_open_and_scroll_stays_streaming() {
    let mut engine = EditorEngine::new();
    let text = deterministic_large_text(LARGE_FILE_BYTES);
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(9001), "100mb.txt", text)
        .expect("open 100MB buffer");

    assert!(matches!(
        engine.text(buffer),
        Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
    ));

    let scroll_line = 1_000_000;
    let viewport = engine
        .viewport_projection(EditorViewportRequest {
            buffer_id: buffer,
            scroll: ViewportScroll {
                top_line: scroll_line,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 1_200,
                height_px: 48,
            },
        })
        .expect("viewport projection after scroll");

    assert_eq!(
        engine.buffer_mode(buffer).expect("buffer mode"),
        legion_editor::BufferMode::Normal,
        "100MB files should remain in streaming mode instead of degrading to the full-cache fallback",
    );
    assert_eq!(viewport.mode, ViewportProjectionMode::Normal);
    assert!(viewport.large_file_status.is_none());
    assert_eq!(viewport.visible_range.start.line, scroll_line);
    assert!(!viewport.line_slices.is_empty());
    assert!(
        viewport
            .line_slices
            .iter()
            .all(|slice| slice.visible_text.len() < DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES),
        "streaming viewport slices should stay bounded even at 100MB scale",
    );
}
