use legion_editor::{Cursor, EditorEngine, Selection, TextEdit, TextPosition, TextRange};
use legion_protocol::{FileId, TransactionSource, WorkspaceId};

fn engine_with(text: &str) -> (EditorEngine, legion_protocol::BufferId) {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(1), "test.txt", text)
        .expect("buffer opens");
    (engine, buffer)
}

fn cursor_positions(engine: &EditorEngine, buffer: legion_protocol::BufferId) -> Vec<TextPosition> {
    engine
        .cursors(buffer)
        .expect("cursors are available")
        .iter()
        .map(|cursor| cursor.position)
        .collect()
}

#[test]
fn empty_cursor_set_is_rejected_without_losing_primary() {
    let (mut engine, buffer) = engine_with("abcdef");
    engine
        .set_cursors(
            buffer,
            vec![Cursor {
                position: TextPosition::new(0, 4),
            }],
        )
        .expect("valid cursor set");

    assert!(engine.set_cursors(buffer, Vec::new()).is_err());
    assert_eq!(
        engine.primary_cursor(buffer).unwrap(),
        TextPosition::new(0, 4)
    );
    assert_eq!(
        cursor_positions(&engine, buffer),
        vec![TextPosition::new(0, 4)]
    );
}

#[test]
fn forward_selection_sets_primary_cursor_to_selection_end() {
    let (mut engine, buffer) = engine_with("abcdef");
    engine
        .set_selections(
            buffer,
            vec![Selection {
                range: TextRange::new(TextPosition::new(0, 1), TextPosition::new(0, 4)),
            }],
        )
        .expect("valid selection");

    assert_eq!(
        engine.primary_cursor(buffer).unwrap(),
        TextPosition::new(0, 4)
    );
    assert_eq!(
        cursor_positions(&engine, buffer),
        vec![TextPosition::new(0, 4)]
    );
}

#[test]
fn multi_cursor_heads_transform_through_insert_undo_and_redo() {
    let (mut engine, buffer) = engine_with("abcdef");
    let old_heads = vec![TextPosition::new(0, 1), TextPosition::new(0, 4)];
    engine
        .set_cursors(
            buffer,
            old_heads
                .iter()
                .copied()
                .map(|position| Cursor { position })
                .collect(),
        )
        .expect("valid multi-cursor set");

    engine
        .apply_edit(
            buffer,
            TextEdit::insert(TextPosition::new(0, 0), "XX"),
            TransactionSource::User,
            None,
            None,
        )
        .expect("insert succeeds");
    let new_heads = vec![TextPosition::new(0, 3), TextPosition::new(0, 6)];
    assert_eq!(engine.text(buffer).unwrap(), "XXabcdef");
    assert_eq!(cursor_positions(&engine, buffer), new_heads);

    engine.undo(buffer, None).expect("undo succeeds");
    assert_eq!(engine.text(buffer).unwrap(), "abcdef");
    assert_eq!(cursor_positions(&engine, buffer), old_heads);

    engine.redo(buffer, None).expect("redo succeeds");
    assert_eq!(engine.text(buffer).unwrap(), "XXabcdef");
    assert_eq!(cursor_positions(&engine, buffer), new_heads);
}
