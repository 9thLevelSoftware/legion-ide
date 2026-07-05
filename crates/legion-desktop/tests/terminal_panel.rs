use legion_desktop::view::terminal_panel::TerminalPanelRenderModel;
use legion_protocol::{
    EventSequence, RedactionHint, TerminalOutputRowProjection, TerminalPanelProjection,
    TerminalPanelStatus, TerminalPanelStatusKind, TerminalRuntimeState,
    TerminalScrollbackProjection, TerminalSessionId,
};

/// All status kind labels must be lowercase human text — no Rust debug strings (no PascalCase).
#[test]
fn status_label_is_human_readable_for_all_kinds() {
    let all_kinds = [
        TerminalPanelStatusKind::Disabled,
        TerminalPanelStatusKind::Denied,
        TerminalPanelStatusKind::Idle,
        TerminalPanelStatusKind::Starting,
        TerminalPanelStatusKind::Running,
        TerminalPanelStatusKind::Exited,
        TerminalPanelStatusKind::Failed,
        TerminalPanelStatusKind::Degraded,
        TerminalPanelStatusKind::Unavailable,
        TerminalPanelStatusKind::Crashed,
        TerminalPanelStatusKind::PolicyBlocked,
    ];

    for kind in all_kinds {
        let mut projection = TerminalPanelProjection::empty();
        projection.status = TerminalPanelStatus {
            kind,
            message: String::new(),
        };
        let model = TerminalPanelRenderModel::from_projection(&projection, 10);
        let label = &model.status_label;

        // Must start with "status=" prefix.
        assert!(
            label.starts_with("status="),
            "status_label must start with 'status='; got: {label:?}"
        );
        let text = &label["status=".len()..];

        // Must not be a Rust debug string (no PascalCase: no uppercase letter after lowercase).
        let has_pascal = text.chars().enumerate().any(|(i, c)| {
            i > 0 && c.is_uppercase() && text.chars().nth(i - 1).is_some_and(|p| p.is_lowercase())
        });
        assert!(
            !has_pascal,
            "status_label must not contain PascalCase (no Rust debug format); kind={kind:?}, label={label:?}"
        );

        // Must not be empty.
        assert!(
            !text.is_empty(),
            "status_label text must not be empty for kind={kind:?}"
        );
    }
}

fn row(sequence: u64, payload: &str, stderr: bool) -> TerminalOutputRowProjection {
    TerminalOutputRowProjection {
        session_id: TerminalSessionId(42),
        sequence: EventSequence(sequence),
        redacted_payload: payload.to_string(),
        byte_count: payload.len() as u64,
        is_stderr: stderr,
        truncated: false,
        redaction: RedactionHint::MetadataOnly,
        schema_version: 1,
    }
}

#[test]
fn terminal_panel_render_model_exposes_grid_status_and_scrollback() {
    let mut projection = TerminalPanelProjection::empty();
    projection.active_session_id = Some(TerminalSessionId(42));
    projection.runtime_state = Some(TerminalRuntimeState::Running);
    projection.status = TerminalPanelStatus {
        kind: TerminalPanelStatusKind::Running,
        message: "Terminal running".to_string(),
    };
    projection.output_rows = vec![row(1, "hello", false), row(2, "warn", true)];
    projection.scrollback = TerminalScrollbackProjection {
        visible_row_count: 2,
        omitted_row_count: 5,
        byte_limit: 4096,
        truncated: true,
        schema_version: 1,
    };

    let model = TerminalPanelRenderModel::from_projection(&projection, 100);

    assert_eq!(model.status_label, "status=running");
    assert_eq!(model.active_session_label.as_deref(), Some("session=42"));
    assert_eq!(model.runtime_label.as_deref(), Some("runtime=Running"));
    assert_eq!(model.scrollback_label, "visible=2 omitted=5 matches=0");
    assert!(model.scrollback_truncated);
    assert_eq!(model.grid.rows.len(), 2);
    assert_eq!(model.grid.rows[1].stream_label, "stderr");
    assert_eq!(model.copy_all_visible(), Some("hello\nwarn".to_string()));
}
