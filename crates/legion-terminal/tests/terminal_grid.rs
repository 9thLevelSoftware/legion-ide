use legion_protocol::{
    EventSequence, RedactionHint, TerminalOutputRowProjection, TerminalPanelProjection,
    TerminalScrollbackProjection, TerminalSessionId,
};
use legion_terminal::grid::{TerminalGrid, TerminalGridSelection};

fn row(sequence: u64, payload: &str, stderr: bool, truncated: bool) -> TerminalOutputRowProjection {
    TerminalOutputRowProjection {
        session_id: TerminalSessionId(7),
        sequence: EventSequence(sequence),
        redacted_payload: payload.to_string(),
        byte_count: payload.len() as u64,
        is_stderr: stderr,
        truncated,
        redaction: RedactionHint::MetadataOnly,
        schema_version: 1,
    }
}

#[test]
fn terminal_grid_projects_rows_badges_and_scrollback_summary() {
    let mut projection = TerminalPanelProjection::empty();
    projection.output_rows = vec![row(1, "ready", false, false), row(2, "warning", true, true)];
    projection.scrollback = TerminalScrollbackProjection {
        visible_row_count: 2,
        omitted_row_count: 9,
        byte_limit: 1024,
        truncated: true,
        schema_version: 1,
    };

    let grid = TerminalGrid::from_projection(&projection, 100);

    assert_eq!(grid.rows.len(), 2);
    assert_eq!(grid.rows[0].sequence_label, "   1");
    assert_eq!(grid.rows[0].stream_label, "stdout");
    assert_eq!(grid.rows[1].stream_label, "stderr");
    assert!(grid.rows[1].badges.iter().any(|badge| badge == "truncated"));
    assert_eq!(grid.scrollback.visible_row_count, 2);
    assert_eq!(grid.scrollback.omitted_row_count, 9);
    assert!(grid.scrollback.truncated);
}

#[test]
fn terminal_grid_selection_copy_returns_bounded_payloads_only() {
    let mut projection = TerminalPanelProjection::empty();
    projection.output_rows = vec![
        row(10, "first", false, false),
        row(11, "second", false, false),
    ];

    let grid = TerminalGrid::from_projection(&projection, 100);

    assert_eq!(
        grid.copy_selection(TerminalGridSelection::Row(EventSequence(10))),
        Some("first".to_string())
    );
    assert_eq!(
        grid.copy_selection(TerminalGridSelection::AllVisible),
        Some("first\nsecond".to_string())
    );
    assert_eq!(
        grid.copy_selection(TerminalGridSelection::Row(EventSequence(999))),
        None
    );
}

#[test]
fn terminal_grid_applies_row_limit_without_losing_scrollback_metadata() {
    let mut projection = TerminalPanelProjection::empty();
    projection.output_rows = vec![
        row(1, "one", false, false),
        row(2, "two", false, false),
        row(3, "three", false, false),
    ];
    projection.scrollback.omitted_row_count = 4;

    let grid = TerminalGrid::from_projection(&projection, 2);

    assert_eq!(grid.rows.len(), 2);
    assert_eq!(grid.rows[0].payload, "one");
    assert_eq!(grid.rows[1].payload, "two");
    assert_eq!(grid.scrollback.omitted_row_count, 4);
}
