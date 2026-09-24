use legion_editor::{BufferMode, DirectedCaret, EditorEngine, HorizontalDirection, TextPosition};
use legion_protocol::{FileId, WorkspaceId};

fn engine_with(text: &str) -> (EditorEngine, legion_protocol::BufferId) {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(1), "horizontal.txt", text)
        .expect("buffer opens");
    (engine, buffer)
}

#[test]
fn plain_horizontal_movement_collapses_each_selection_without_extra_step() {
    let (mut engine, buffer) = engine_with("abcdef");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 5), Some(TextPosition::new(0, 2))),
                DirectedCaret::new(TextPosition::new(0, 1), Some(TextPosition::new(0, 4))),
            ],
        )
        .unwrap();
    engine
        .move_horizontally(buffer, HorizontalDirection::Left, false)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(0, 2), None),
            DirectedCaret::new(TextPosition::new(0, 1), None),
        ]
    );
    engine
        .set_directed_carets(
            buffer,
            vec![DirectedCaret::new(
                TextPosition::new(0, 1),
                Some(TextPosition::new(0, 4)),
            )],
        )
        .unwrap();
    engine
        .move_horizontally(buffer, HorizontalDirection::Right, false)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![DirectedCaret::new(TextPosition::new(0, 4), None)]
    );
}

#[test]
fn extending_reverses_and_crosses_with_ordered_coincident_carets() {
    let (mut engine, buffer) = engine_with("a\r\ncombining é 👩‍💻 🇺🇸 z");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(1, 10), None),
                DirectedCaret::new(TextPosition::new(1, 10), None),
                DirectedCaret::new(TextPosition::new(1, 0), None),
            ],
        )
        .unwrap();
    let version = engine.buffer_version(buffer).unwrap();
    let history = engine.transaction_log().len();
    engine
        .move_horizontally(buffer, HorizontalDirection::Left, true)
        .unwrap();
    engine
        .move_horizontally(buffer, HorizontalDirection::Right, true)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(1, 10), Some(TextPosition::new(1, 10))),
            DirectedCaret::new(TextPosition::new(1, 10), Some(TextPosition::new(1, 10))),
            DirectedCaret::new(TextPosition::new(1, 0), Some(TextPosition::new(1, 0))),
        ]
    );
    assert_eq!(engine.buffer_version(buffer).unwrap(), version);
    assert_eq!(engine.transaction_log().len(), history);
}

#[test]
fn movement_handles_combining_zwj_flags_crlf_and_interior_scalar_offsets() {
    let (mut engine, buffer) = engine_with("é👩‍💻🇺🇸\r\nq");
    // Byte 1 is inside the combining sequence; byte 7 is inside the ZWJ sequence.
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 1), None),
                DirectedCaret::new(TextPosition::new(0, 7), None),
                DirectedCaret::new(TextPosition::new(0, 18), None),
            ],
        )
        .unwrap();
    engine
        .move_horizontally(buffer, HorizontalDirection::Left, false)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(0, 0), None),
            DirectedCaret::new(TextPosition::new(0, 3), None),
            DirectedCaret::new(TextPosition::new(0, 14), None),
        ]
    );
    engine
        .move_horizontally(buffer, HorizontalDirection::Right, false)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(0, 3), None),
            DirectedCaret::new(TextPosition::new(0, 14), None),
            DirectedCaret::new(TextPosition::new(0, 22), None),
        ]
    );
}

#[test]
fn movement_crosses_crlf_and_stays_at_document_edges_in_empty_documents() {
    let (mut engine, buffer) = engine_with("a\r\nb");
    engine
        .set_directed_carets(
            buffer,
            vec![
                DirectedCaret::new(TextPosition::new(0, 1), None),
                DirectedCaret::new(TextPosition::new(1, 0), None),
            ],
        )
        .unwrap();
    engine
        .move_horizontally(buffer, HorizontalDirection::Right, false)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(1, 0), None),
            DirectedCaret::new(TextPosition::new(1, 1), None),
        ]
    );
    engine
        .move_horizontally(buffer, HorizontalDirection::Right, false)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![
            DirectedCaret::new(TextPosition::new(1, 1), None),
            DirectedCaret::new(TextPosition::new(1, 1), None),
        ]
    );

    let (mut empty, empty_buffer) = engine_with("");
    empty
        .move_horizontally(empty_buffer, HorizontalDirection::Left, false)
        .unwrap();
    assert_eq!(
        empty.directed_carets(empty_buffer).unwrap(),
        vec![DirectedCaret::new(TextPosition::new(0, 0), None)]
    );
}

#[test]
fn streamed_large_buffer_moves_at_actual_positions_without_history_or_version_change() {
    let temp = tempfile::NamedTempFile::new().unwrap();
    let text = format!("{}needle\r\nend", "x".repeat(5 * 1024 * 1024 + 17));
    std::fs::write(temp.path(), text.as_bytes()).unwrap();
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer_streaming(WorkspaceId(1), FileId(2), "large.txt", temp.path())
        .unwrap();
    assert!(engine.buffer_is_streamed(buffer).unwrap());
    assert_eq!(engine.buffer_mode(buffer).unwrap(), BufferMode::Degraded);
    assert!(engine.text(buffer).is_err());
    let pos = TextPosition::new(0, 5 * 1024 * 1024 + 17);
    engine
        .set_directed_carets(buffer, vec![DirectedCaret::new(pos, None)])
        .unwrap();
    let version = engine.buffer_version(buffer).unwrap();
    let history = engine.transaction_log().len();
    engine
        .move_horizontally(buffer, HorizontalDirection::Right, false)
        .unwrap();
    assert_eq!(
        engine.directed_carets(buffer).unwrap(),
        vec![DirectedCaret::new(
            TextPosition::new(0, pos.column + 1),
            None
        )]
    );
    assert_eq!(engine.buffer_version(buffer).unwrap(), version);
    assert_eq!(engine.transaction_log().len(), history);
    assert!(engine.text(buffer).is_err());
}
