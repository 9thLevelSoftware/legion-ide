use devil_protocol::{
    CanonicalPath, DebugAdapterLaunchRequest, DebugBreakpointId, DebugBreakpointRecord,
    DebugSessionState, DebugStepKind, EventSequence, ProtocolTextRange, TextCoordinate,
    WorkspaceId,
};
use devil_terminal::{DapAdapterFixtureConfig, DapAdapterFixtureRuntime};

fn range(line: u32) -> ProtocolTextRange {
    ProtocolTextRange {
        start: TextCoordinate {
            line,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
        end: TextCoordinate {
            line,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
    }
}

#[test]
fn dap_fixture_launch_projects_debugger_state_without_raw_values() {
    let runtime = DapAdapterFixtureRuntime::new(DapAdapterFixtureConfig::enabled());
    let breakpoint = DebugBreakpointRecord {
        breakpoint_id: DebugBreakpointId("bp-main".to_string()),
        workspace_id: WorkspaceId(11),
        session_id: None,
        path: CanonicalPath("C:/repo/src/main.rs".to_string()),
        range: range(7),
        enabled: true,
        condition: Some("count > 2".to_string()),
        hit_condition: Some("3".to_string()),
        log_message: Some("count changed".to_string()),
        verified: false,
        message: None,
        correlation_id: devil_protocol::CorrelationId(77),
        causality_id: devil_protocol::CausalityId(
            uuid::Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap(),
        ),
        sequence: EventSequence(1),
        schema_version: 1,
    };

    let launched = runtime
        .launch(DebugAdapterLaunchRequest {
            workspace_id: WorkspaceId(11),
            configuration_id: devil_protocol::DebugConfigurationId(
                "cargo:sample:bin:sample".to_string(),
            ),
            adapter_type: "lldb-dap".to_string(),
            breakpoints: vec![breakpoint],
            schema_version: 1,
        })
        .expect("fixture launch should succeed");

    assert_eq!(launched.audit.state, DebugSessionState::Paused);
    assert!(launched.breakpoints[0].verified);
    assert_eq!(launched.stack_frames[0].name, "main");
    assert!(launched.variables.iter().all(|variable| {
        variable.value_label == "metadata-only" && !variable.value_label.contains("secret")
    }));
    assert_eq!(launched.inline_values[0].value_label, "metadata-only");
    assert!(launched.console.iter().any(|entry| {
        entry.category == devil_protocol::DebugConsoleCategory::Adapter
            && entry.message_label.contains("lldb-dap")
    }));

    let stepped = runtime
        .step(launched.audit.session_id.clone(), DebugStepKind::Over)
        .expect("fixture step should succeed");
    assert_eq!(stepped.audit.state, DebugSessionState::Paused);
    assert!(
        stepped
            .console
            .iter()
            .any(|entry| { entry.message_label.contains("step=over") })
    );
}
