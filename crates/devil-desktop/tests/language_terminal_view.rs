use devil_desktop::{
    bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge},
    view::DesktopProjectionViewModel,
};
use devil_protocol::{
    BufferId, CorrelationId, EventSequence, LanguageToolingOperationKind,
    LanguageToolingOperationProjection, LanguageToolingProjection, LanguageToolingStatusKind,
    RedactionHint, TerminalOutputRowProjection, TerminalPanelProjection, TerminalPanelStatus,
    TerminalPanelStatusKind, TerminalRuntimeState, TerminalSessionId, TextCoordinate,
    TimestampMillis,
};
use devil_ui::{ActiveBufferProjection, CommandDispatchIntent, Shell};

fn position(byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line: 0,
        character: byte_offset as u32,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

#[test]
fn desktop_language_panel_renders_projection() {
    let mut snapshot = Shell::empty("language-terminal").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        buffer_id: Some(BufferId(9)),
        ..ActiveBufferProjection::empty()
    };
    snapshot.language_tooling_projection = LanguageToolingProjection {
        buffer_id: Some(BufferId(9)),
        status: LanguageToolingStatusKind::Ready,
        status_message: "Completion ready".to_string(),
        operations: vec![LanguageToolingOperationProjection {
            operation_id: "language:completion:1".to_string(),
            kind: LanguageToolingOperationKind::Completion,
            status: LanguageToolingStatusKind::Ready,
            request_id: None,
            proposal_id: None,
            message: "semantic projection refreshed".to_string(),
            correlation_id: Some(CorrelationId(1)),
            causality_id: None,
            generated_at: TimestampMillis(1),
            schema_version: 1,
        }],
        ..LanguageToolingProjection::empty()
    };
    snapshot.terminal_panel_projection = TerminalPanelProjection {
        active_session_id: Some(TerminalSessionId(12)),
        runtime_state: Some(TerminalRuntimeState::Running),
        status: TerminalPanelStatus {
            kind: TerminalPanelStatusKind::Running,
            message: "Terminal fixture running".to_string(),
        },
        output_rows: vec![TerminalOutputRowProjection {
            session_id: TerminalSessionId(12),
            sequence: EventSequence(1),
            redacted_payload: "fixture terminal ready".to_string(),
            byte_count: 22,
            is_stderr: false,
            truncated: false,
            redaction: RedactionHint::MetadataOnly,
            schema_version: 1,
        }],
        ..TerminalPanelProjection::empty()
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert!(
        model
            .language_rows
            .iter()
            .any(|row| row.contains("language op language:completion:1"))
    );
    assert!(
        model
            .terminal_rows
            .iter()
            .any(|row| row.contains("fixture terminal ready"))
    );
}

#[test]
fn desktop_language_actions_dispatch_intents() {
    let mut snapshot = Shell::empty("bridge").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        buffer_id: Some(BufferId(9)),
        ..ActiveBufferProjection::empty()
    };
    snapshot.terminal_panel_projection = TerminalPanelProjection {
        active_session_id: Some(TerminalSessionId(12)),
        ..TerminalPanelProjection::empty()
    };
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::RequestHover {
                position: position(3),
            },
            &snapshot
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RequestHover {
            buffer_id: BufferId(9),
            position: position(3),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::TerminalInput {
                payload: "echo ready".to_string(),
            },
            &snapshot
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::TerminalInput {
            session_id: TerminalSessionId(12),
            payload: "echo ready".to_string(),
        })
    );
}

#[test]
fn desktop_terminal_panel_renders_and_dispatches() {
    let mut snapshot = Shell::empty("terminal").projection_snapshot();
    snapshot.terminal_panel_projection = TerminalPanelProjection {
        active_session_id: Some(TerminalSessionId(12)),
        runtime_state: Some(TerminalRuntimeState::Running),
        status: TerminalPanelStatus {
            kind: TerminalPanelStatusKind::Running,
            message: "Terminal fixture running".to_string(),
        },
        output_rows: vec![TerminalOutputRowProjection {
            session_id: TerminalSessionId(12),
            sequence: EventSequence(1),
            redacted_payload: "fixture terminal ready".to_string(),
            byte_count: 22,
            is_stderr: false,
            truncated: false,
            redaction: RedactionHint::MetadataOnly,
            schema_version: 1,
        }],
        ..TerminalPanelProjection::empty()
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert!(
        model
            .terminal_rows
            .iter()
            .any(|row| row.contains("fixture terminal ready"))
    );

    let bridge = DesktopCommandBridge::new();
    assert_eq!(
        bridge.translate(
            DesktopAction::TerminalInput {
                payload: "echo ready".to_string(),
            },
            &snapshot
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::TerminalInput {
            session_id: TerminalSessionId(12),
            payload: "echo ready".to_string(),
        })
    );
}
