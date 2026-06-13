use legion_protocol::{
    CanonicalPath, DebugAdapterLaunchRequest, DebugBreakpointId, DebugBreakpointRecord,
    DebugSessionState, DebugStepKind, EventSequence, ProtocolTextRange, TextCoordinate,
    WorkspaceId,
};
use legion_terminal::{DapAdapterFixtureConfig, DapAdapterFixtureRuntime};

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
        correlation_id: legion_protocol::CorrelationId(77),
        causality_id: legion_protocol::CausalityId(
            uuid::Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap(),
        ),
        sequence: EventSequence(1),
        schema_version: 1,
    };

    let launched = runtime
        .launch(DebugAdapterLaunchRequest {
            workspace_id: WorkspaceId(11),
            configuration_id: legion_protocol::DebugConfigurationId(
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
        entry.category == legion_protocol::DebugConsoleCategory::Adapter
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

#[test]
fn dap_fixture_launch_smokes_tier_two_adapter_labels() {
    let runtime = DapAdapterFixtureRuntime::new(DapAdapterFixtureConfig::enabled());
    let breakpoint = DebugBreakpointRecord {
        breakpoint_id: DebugBreakpointId("bp-tier-two".to_string()),
        workspace_id: WorkspaceId(12),
        session_id: None,
        path: CanonicalPath("C:/repo/src/main.rs".to_string()),
        range: range(12),
        enabled: true,
        condition: None,
        hit_condition: None,
        log_message: None,
        verified: false,
        message: None,
        correlation_id: legion_protocol::CorrelationId(78),
        causality_id: legion_protocol::CausalityId(
            uuid::Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
        ),
        sequence: EventSequence(1),
        schema_version: 1,
    };

    for adapter_type in ["debugpy", "delve", "js-debug"] {
        let launched = runtime
            .launch(DebugAdapterLaunchRequest {
                workspace_id: WorkspaceId(12),
                configuration_id: legion_protocol::DebugConfigurationId(format!(
                    "cargo:sample:bin:{adapter_type}"
                )),
                adapter_type: adapter_type.to_string(),
                breakpoints: vec![breakpoint.clone()],
                schema_version: 1,
            })
            .expect("tier-two adapter smoke should succeed");

        assert_eq!(launched.audit.adapter_type, adapter_type);
        assert!(
            launched
                .console
                .iter()
                .any(|entry| entry.message_label.contains(adapter_type))
        );

        let stepped = runtime
            .step(launched.audit.session_id.clone(), DebugStepKind::Continue)
            .expect("tier-two adapter step smoke should succeed");
        assert_eq!(stepped.audit.adapter_type, adapter_type);
        assert!(
            stepped
                .console
                .iter()
                .any(|entry| entry.message_label.contains(adapter_type))
        );
    }
}
