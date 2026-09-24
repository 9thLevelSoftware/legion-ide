use legion_editor::{EditorEngine, EditorError, TextPosition};
use legion_protocol::{FileId, WorkspaceId};

fn engine_with(text: impl Into<String>) -> (EditorEngine, legion_protocol::BufferId) {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(7), FileId(7), "window.txt", text)
        .expect("buffer opens");
    (engine, buffer)
}

#[test]
fn editor_window_delegates_nonzero_line_and_rejects_unknown_buffer() {
    let (engine, buffer) = engine_with("first\nsecond αβ\nthird");
    let window = engine
        .line_window_around_byte(buffer, "first\nsecond ".len(), 8)
        .unwrap();
    assert_eq!(window.line, 1);
    assert_eq!(window.line_start_byte, "first\n".len());
    assert!(window.start_byte <= window.caret_byte);
    assert!(window.caret_byte <= window.end_byte);
    assert!(window.text.len() <= 8);

    let missing = legion_protocol::BufferId(u128::MAX);
    assert!(matches!(
        engine.line_window_around_byte(missing, 0, 8),
        Err(EditorError::BufferNotFound(id)) if id == missing
    ));
}

#[test]
fn editor_window_preserves_text_budget_errors_and_large_snapshot_mode() {
    let (engine, buffer) = engine_with("αβγ");
    assert!(matches!(
        engine.line_window_around_byte(buffer, 0, 0),
        Err(EditorError::Text(
            legion_text::TextError::InvalidWindowBudget { .. }
        ))
    ));

    let large = "x".repeat(legion_text::DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES * 2 + 1);
    let (engine, large_buffer) = engine_with(large);
    let window = engine
        .line_window_around_byte(
            large_buffer,
            legion_text::DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES,
            32,
        )
        .unwrap();
    assert!(window.text.len() <= 32);
    assert_eq!(
        window.caret_byte,
        legion_text::DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES
    );
}

#[test]
fn editor_buffer_byte_offset_rejects_invalid_positions_without_clamping() {
    let (engine, buffer) = engine_with("first\nsecond");
    assert_eq!(
        engine
            .buffer_byte_offset(buffer, TextPosition::new(1, 3))
            .unwrap(),
        "first\nsec".len()
    );
    assert!(matches!(
        engine.buffer_byte_offset(buffer, TextPosition::new(3, 0)),
        Err(EditorError::Text(
            legion_text::TextError::LineOutOfBounds { .. }
        ))
    ));
    assert!(matches!(
        engine.buffer_byte_offset(buffer, TextPosition::new(1, 99)),
        Err(EditorError::Text(
            legion_text::TextError::ColumnOutOfBounds { .. }
        ))
    ));
}
