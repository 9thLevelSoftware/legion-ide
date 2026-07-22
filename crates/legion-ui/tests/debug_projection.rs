use legion_protocol::{
    BufferId, CanonicalPath, DebugBreakpointId, DebugConfigurationId, DebugSessionId,
    DebugSessionState, DebugWatchId, TextCoordinate, TimestampMillis, WorkspaceId,
};
use legion_ui::{
    CommandDispatchIntent, DebugBreakpointProjection, DebugConfigurationProjection,
    DebugConsoleProjection, DebugInlineValueProjection, DebugProjection, DebugStackFrameProjection,
    DebugStatusKindProjection, DebugStatusProjection, DebugStepKindProjection,
    DebugVariableProjection, DebugWatchProjection, Shell,
};

#[test]
fn shell_carries_debug_projection_and_routes_debug_commands_without_authority() {
    let mut snapshot = Shell::empty("debug").projection_snapshot();
    let session_id = DebugSessionId("debug-1".to_string());
    let source_path = CanonicalPath("C:/repo/src/main.rs".to_string());
    snapshot.active_buffer_projection.workspace_id = Some(WorkspaceId(11));
    snapshot.active_buffer_projection.buffer_id = Some(BufferId(7));
    snapshot.active_buffer_projection.file_path = Some(source_path.clone());
    snapshot.active_buffer_projection.small_buffer_preview = Some("abcdefghijklmnop".to_string());
    let debug_projection = DebugProjection {
        status: DebugStatusProjection {
            kind: DebugStatusKindProjection::Paused,
            message: "Paused at breakpoint".to_string(),
        },
        active_session_id: Some(session_id.clone()),
        session_state: Some(DebugSessionState::Paused),
        live_adapter: false,
        configurations: vec![DebugConfigurationProjection {
            configuration_id: DebugConfigurationId("cargo:sample:bin:sample".to_string()),
            name: "Debug sample".to_string(),
            adapter_type: "lldb-dap".to_string(),
            program_label: "target/debug/sample".to_string(),
            cargo_package: Some("sample".to_string()),
            cargo_target: Some("sample".to_string()),
            deterministic: true,
        }],
        breakpoints: vec![DebugBreakpointProjection {
            breakpoint_id: DebugBreakpointId("bp-1".to_string()),
            session_id: Some(session_id.clone()),
            path: source_path.clone(),
            line: 42,
            enabled: true,
            condition: Some("x > 1".to_string()),
            hit_condition: Some("3".to_string()),
            log_message: Some("x={x}".to_string()),
            verified: true,
            message: Some("verified".to_string()),
        }],
        stack_frames: vec![DebugStackFrameProjection {
            session_id: session_id.clone(),
            frame_id: 1,
            name: "sample::main".to_string(),
            path: Some(source_path.clone()),
            line: Some(42),
        }],
        variables: vec![DebugVariableProjection {
            session_id: session_id.clone(),
            name: "x".to_string(),
            value_label: "2".to_string(),
            type_label: Some("i32".to_string()),
            has_children: false,
        }],
        watches: vec![DebugWatchProjection {
            watch_id: DebugWatchId("watch-1".to_string()),
            session_id: session_id.clone(),
            expression_label: "x + 1".to_string(),
            value_label: "3".to_string(),
            type_label: Some("i32".to_string()),
        }],
        console: vec![DebugConsoleProjection {
            session_id: session_id.clone(),
            category_label: "stdout".to_string(),
            message_label: "started".to_string(),
        }],
        inline_values: vec![DebugInlineValueProjection {
            session_id: session_id.clone(),
            path: source_path,
            line: 42,
            expression_label: "x".to_string(),
            value_label: "2".to_string(),
        }],
        diagnostics: vec!["metadata-only diagnostic".to_string()],
        generated_at: TimestampMillis(123),
        schema_version: 1,
    };
    snapshot.debug_projection = debug_projection.clone();

    let mut shell = Shell::new(snapshot);
    assert_eq!(shell.debug_projection, debug_projection);
    assert_eq!(
        shell.projection_snapshot().debug_projection,
        debug_projection
    );
    let before_commands = shell.projection_snapshot();

    assert_eq!(
        shell
            .handle_command(":debug-configs")
            .expect("debug config command should parse"),
        Some(CommandDispatchIntent::RefreshDebugConfigurations)
    );
    assert_eq!(
        shell
            .handle_command(":debug-launch cargo:sample:bin:sample")
            .expect("debug launch command should parse"),
        Some(CommandDispatchIntent::LaunchDebugSession {
            configuration_id: DebugConfigurationId("cargo:sample:bin:sample".to_string()),
        })
    );

    assert_eq!(
        shell
            .handle_command(":debug-breakpoint 41,x > 1,3,log {x}")
            .expect("debug breakpoint command should parse"),
        Some(CommandDispatchIntent::ToggleDebugBreakpoint {
            buffer_id: BufferId(7),
            line: 41,
            condition: Some("x > 1".to_string()),
            hit_condition: Some("3".to_string()),
            log_message: Some("log {x}".to_string()),
        })
    );

    for (command, kind) in [
        (":debug-step over", DebugStepKindProjection::Over),
        (":debug-step in", DebugStepKindProjection::Into),
        (":debug-step out", DebugStepKindProjection::Out),
        (":debug-step back", DebugStepKindProjection::Back),
        (":debug-step continue", DebugStepKindProjection::Continue),
    ] {
        assert_eq!(
            shell
                .handle_command(command)
                .expect("debug step command should parse"),
            Some(CommandDispatchIntent::DebugStep {
                session_id: DebugSessionId("debug-1".to_string()),
                kind,
            })
        );
    }

    assert_eq!(
        shell
            .handle_command(":debug-run-to-cursor 12")
            .expect("run-to-cursor command should parse"),
        Some(CommandDispatchIntent::DebugRunToCursor {
            session_id: DebugSessionId("debug-1".to_string()),
            buffer_id: BufferId(7),
            position: TextCoordinate {
                line: 0,
                character: 12,
                byte_offset: Some(12),
                utf16_offset: None,
            },
        })
    );
    assert_eq!(
        shell
            .handle_command(":debug-eval selected_expression")
            .expect("evaluate command should parse"),
        Some(CommandDispatchIntent::DebugEvaluateSelection {
            session_id: DebugSessionId("debug-1".to_string()),
            expression_label: "selected_expression".to_string(),
        })
    );
    assert_eq!(
        shell
            .handle_command(":debug-watch selected_expression")
            .expect("add watch command should parse"),
        Some(CommandDispatchIntent::DebugAddWatch {
            session_id: DebugSessionId("debug-1".to_string()),
            expression_label: "selected_expression".to_string(),
        })
    );
    assert_eq!(
        shell
            .handle_command(":debug-stop")
            .expect("debug stop command should parse"),
        Some(CommandDispatchIntent::StopDebugSession {
            session_id: DebugSessionId("debug-1".to_string()),
        })
    );
    assert_eq!(
        shell
            .handle_command(":debug-poll")
            .expect("debug poll command should parse"),
        Some(CommandDispatchIntent::PollDebugSession {
            session_id: DebugSessionId("debug-1".to_string()),
        })
    );
    assert_eq!(shell.projection_snapshot(), before_commands);

    let mut replacement = Shell::empty("replacement").projection_snapshot();
    replacement.debug_projection = DebugProjection {
        status: DebugStatusProjection {
            kind: DebugStatusKindProjection::Running,
            message: "Running".to_string(),
        },
        active_session_id: Some(DebugSessionId("debug-2".to_string())),
        session_state: Some(DebugSessionState::Running),
        generated_at: TimestampMillis(456),
        ..DebugProjection::empty()
    };
    shell.replace_projection_snapshot(replacement.clone());

    assert_eq!(
        shell.projection_snapshot().debug_projection,
        replacement.debug_projection
    );
}
