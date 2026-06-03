use legion_desktop::{
    bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge},
    view::DesktopProjectionViewModel,
};
use legion_protocol::{
    BufferId, CanonicalPath, CorrelationId, EventSequence, FileId, LanguageBreadcrumbProjection,
    LanguageCodeLensProjection, LanguageInlayHintProjection, LanguageQuickFixProjection,
    LanguageStickyScopeProjection, LanguageToolingOperationKind,
    LanguageToolingOperationProjection, LanguageToolingProjection, LanguageToolingStatusKind,
    ProtocolDiagnosticSeverity, ProtocolTextRange, RedactionHint, TerminalOutputRowProjection,
    TerminalPanelProjection, TerminalPanelStatus, TerminalPanelStatusKind, TerminalRuntimeState,
    TerminalSessionId, TextCoordinate, TimestampMillis, WorkspaceId,
};
use legion_ui::{
    ActiveBufferProjection, CommandDispatchIntent, SearchScopeProjection,
    SearchStatusKindProjection, SearchStatusProjection, Shell, StructuralSearchCaptureProjection,
    StructuralSearchMatchProjection, StructuralSearchProjection,
};

fn position(byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line: 0,
        character: byte_offset as u32,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

fn range(start: u64, end: u64) -> ProtocolTextRange {
    ProtocolTextRange {
        start: position(start),
        end: position(end),
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
        quick_fixes: vec![LanguageQuickFixProjection {
            action_id: "quickfix:index.lexical.todo:0:0:0".to_string(),
            title: "Prepare code action for index.lexical.todo".to_string(),
            kind_label: "quickfix.diagnostic".to_string(),
            problem_code_label: Some("index.lexical.todo".to_string()),
            problem_range: None,
            severity: ProtocolDiagnosticSeverity::Hint,
            source_label: Some("legion-index".to_string()),
            proposal_id: None,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        breadcrumbs: vec![LanguageBreadcrumbProjection {
            breadcrumb_id: "breadcrumb:outline-main".to_string(),
            label: "main".to_string(),
            kind_label: "function".to_string(),
            range: None,
            depth: 0,
            source_label: "legion-index".to_string(),
            schema_version: 1,
        }],
        sticky_scopes: vec![LanguageStickyScopeProjection {
            scope_id: "sticky:outline-main".to_string(),
            label: "main".to_string(),
            kind_label: "function".to_string(),
            range: None,
            depth: 0,
            active: true,
            source_label: "legion-index".to_string(),
            schema_version: 1,
        }],
        inlay_hints: vec![LanguageInlayHintProjection {
            hint_id: "inlay:outline-main".to_string(),
            label: ": function".to_string(),
            kind_label: "symbol-kind".to_string(),
            position: position(4),
            range: None,
            padding_left: true,
            padding_right: false,
            source_label: "legion-index".to_string(),
            schema_version: 1,
        }],
        code_lenses: vec![LanguageCodeLensProjection {
            lens_id: "codelens:outline-main:references".to_string(),
            title: "1 reference".to_string(),
            command_label: "Find references".to_string(),
            kind_label: "references".to_string(),
            range: None,
            data_label: Some("references=1".to_string()),
            source_label: "legion-index".to_string(),
            schema_version: 1,
        }],
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
            .language_rows
            .iter()
            .any(|row| row.contains("quick fix quickfix:index.lexical.todo:0:0:0"))
    );
    assert!(
        model
            .language_rows
            .iter()
            .any(|row| row.contains("breadcrumb breadcrumb:outline-main main"))
    );
    assert!(
        model
            .language_rows
            .iter()
            .any(|row| row.contains("sticky scope sticky:outline-main main active=true"))
    );
    assert!(
        model
            .language_rows
            .iter()
            .any(|row| row.contains("inlay hint inlay:outline-main : function"))
    );
    assert!(
        model
            .language_rows
            .iter()
            .any(|row| row.contains("code lens codelens:outline-main:references 1 reference"))
    );
    assert!(
        model
            .terminal_rows
            .iter()
            .any(|row| row.contains("fixture terminal ready"))
    );
}

#[test]
fn desktop_structural_search_panel_renders_projection() {
    let mut snapshot = Shell::empty("structural-search").projection_snapshot();
    snapshot.structural_search_projection = StructuralSearchProjection {
        query_id: Some("structural-search:1".to_string()),
        scope: SearchScopeProjection::Workspace,
        pattern_label: "fn $NAME ( )".to_string(),
        rewrite_label: Some("fn renamed_$NAME ( )".to_string()),
        status: SearchStatusProjection {
            kind: SearchStatusKindProjection::Completed,
            message: "Found 1 structural matches".to_string(),
        },
        matches: vec![StructuralSearchMatchProjection {
            query_id: "structural-search:1".to_string(),
            workspace_id: WorkspaceId(7),
            file_id: FileId(11),
            file_path: CanonicalPath("src/main.rs".to_string()),
            range: range(4, 14),
            captures: vec![StructuralSearchCaptureProjection {
                name: "NAME".to_string(),
                value: "alpha".to_string(),
                range: range(7, 12),
            }],
            snippet: "fn alpha()".to_string(),
            replacement_preview: Some("fn renamed_alpha ( )".to_string()),
        }],
        result_limit: 10,
        omitted_match_count: 0,
        omitted_file_count: 0,
        diagnostics: vec!["structural_search.suppressed: skipped ignored match".to_string()],
        proposal_id: Some(legion_protocol::ProposalId(42)),
        generated_at: TimestampMillis(1),
        schema_version: 1,
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(
        model
            .structural_search_rows
            .iter()
            .any(|row| row.contains("structural search: Completed matches=1 proposal=Some(42)"))
    );
    assert!(model.structural_search_rows.iter().any(|row| {
        row.contains("structural match src/main.rs:0 fn alpha() -> fn renamed_alpha ( )")
    }));
    assert!(
        model
            .structural_search_rows
            .iter()
            .any(|row| row.contains("capture NAME=alpha"))
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
