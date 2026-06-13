use legion_protocol::{
    CanonicalPath, DapClientPhase, DapClientState, DapProtocolMessage, DebugAdapterLaunchRequest,
    DebugBreakpointId, DebugBreakpointRecord, DebugConfigurationId, DebugConsoleCategory,
    DebugSessionId, DebugSessionState, DebugStepKind, EventSequence, ProtocolTextRange,
    TextCoordinate, WorkspaceId,
};
use legion_terminal::{DapAdapterFixtureConfig, DapAdapterFixtureRuntime};
use serde_json::json;
use uuid::Uuid;

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

fn breakpoint(session_id: Option<DebugSessionId>) -> DebugBreakpointRecord {
    DebugBreakpointRecord {
        breakpoint_id: DebugBreakpointId("bp-main".to_string()),
        workspace_id: WorkspaceId(11),
        session_id,
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
            Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap(),
        ),
        sequence: EventSequence(1),
        schema_version: 1,
    }
}

fn launch_request(session_id: Option<DebugSessionId>) -> DebugAdapterLaunchRequest {
    DebugAdapterLaunchRequest {
        workspace_id: WorkspaceId(11),
        configuration_id: DebugConfigurationId("cargo:sample:bin:sample".to_string()),
        adapter_type: "lldb-dap".to_string(),
        breakpoints: vec![breakpoint(session_id)],
        schema_version: 1,
    }
}

#[test]
fn dap_client_state_machine_tracks_initialize_launch_stop_and_reads() {
    let fixture = DapAdapterFixtureRuntime::new(DapAdapterFixtureConfig::enabled());
    let mut client = DapClientState::new(DebugSessionId("debug-1".to_string()));

    let initialize = client
        .prepare_initialize("legion-ide", "lldb-dap")
        .expect("initialize request should be accepted");
    assert_eq!(client.phase(), DapClientPhase::Initializing);

    let capabilities = json!({
        "supportsConfigurationDoneRequest": true,
        "supportsContinueRequest": true,
        "supportsEvaluateForHovers": true,
        "supportsSetVariable": false
    });
    let initialize_response =
        DapProtocolMessage::response(2, initialize.seq, "initialize", true, capabilities.clone());
    let matched_initialize = client
        .match_response(initialize_response)
        .expect("initialize response should match");
    assert_eq!(matched_initialize.command, "initialize");
    assert_eq!(client.phase(), DapClientPhase::Ready);
    assert_eq!(client.capabilities(), Some(&capabilities));

    client
        .apply_event(DapProtocolMessage::event(3, "initialized", json!({})))
        .expect("initialized event should be accepted");
    assert_eq!(client.last_event(), Some("initialized"));
    assert_eq!(client.phase(), DapClientPhase::Ready);

    let launched = fixture
        .launch(launch_request(Some(DebugSessionId("debug-1".to_string()))))
        .expect("fixture launch should succeed");

    let launch = client
        .prepare_launch(json!({
            "name": "Debug sample",
            "program": "target/debug/sample",
            "stopOnEntry": false,
        }))
        .expect("launch request should be accepted after initialize");
    assert_eq!(client.phase(), DapClientPhase::Launching);

    let launch_response = DapProtocolMessage::response(
        4,
        launch.seq,
        "launch",
        true,
        json!({ "sessionId": launched.audit.session_id.0 }),
    );
    client
        .match_response(launch_response)
        .expect("launch response should match");
    assert_eq!(client.phase(), DapClientPhase::Running);

    client
        .apply_event(DapProtocolMessage::event(
            5,
            "stopped",
            json!({ "reason": "breakpoint", "threadId": 1 }),
        ))
        .expect("stopped event should be accepted");
    assert_eq!(client.phase(), DapClientPhase::Paused);
    assert_eq!(client.last_stop_reason(), Some("breakpoint"));

    let threads = client
        .prepare_threads()
        .expect("threads request should be accepted while paused");
    let stack_trace = client
        .prepare_stack_trace(1)
        .expect("stackTrace request should be accepted while paused");
    let scopes = client
        .prepare_scopes(11)
        .expect("scopes request should be accepted while paused");
    let variables = client
        .prepare_variables(1)
        .expect("variables request should be accepted while paused");

    assert_eq!(client.pending_request_count(), 4);

    client
        .match_response(DapProtocolMessage::response(
            6,
            threads.seq,
            "threads",
            true,
            json!({
                "threads": [{ "id": 1, "name": "main" }]
            }),
        ))
        .expect("threads response should match");
    assert_eq!(client.phase(), DapClientPhase::Paused);

    client
        .match_response(DapProtocolMessage::response(
            7,
            stack_trace.seq,
            "stackTrace",
            true,
            json!({
                "stackFrames": launched.stack_frames.iter().map(|frame| json!({
                    "id": frame.frame_id,
                    "name": frame.name,
                    "line": frame.range.map(|range| range.start.line).unwrap_or_default(),
                    "path": frame.path.as_ref().map(|path| path.0.clone()),
                })).collect::<Vec<_>>()
            }),
        ))
        .expect("stackTrace response should match");

    client
        .match_response(DapProtocolMessage::response(
            8,
            scopes.seq,
            "scopes",
            true,
            json!({
                "scopes": [{
                    "name": "Locals",
                    "presentationHint": "locals",
                    "variablesReference": 1,
                    "expensive": false
                }]
            }),
        ))
        .expect("scopes response should match");

    client
        .match_response(DapProtocolMessage::response(
            9,
            variables.seq,
            "variables",
            true,
            json!({
                "variables": launched.variables.iter().map(|variable| json!({
                    "name": variable.name,
                    "value": variable.value_label,
                    "type": variable.type_label,
                    "variablesReference": variable.variables_reference,
                    "presentationHint": {
                        "lazy": variable.has_children
                    }
                })).collect::<Vec<_>>()
            }),
        ))
        .expect("variables response should match");

    let stepped = fixture
        .step(launched.audit.session_id.clone(), DebugStepKind::Over)
        .expect("fixture step should succeed");
    assert_eq!(stepped.audit.state, DebugSessionState::Paused);
    assert_eq!(stepped.console[0].category, DebugConsoleCategory::Adapter);
    assert!(
        stepped
            .console
            .iter()
            .any(|entry| entry.message_label.contains("step=over"))
    );

    let continue_request = client
        .prepare_continue(1)
        .expect("continue request should be accepted while paused");
    let continue_response = DapProtocolMessage::response(
        10,
        continue_request.seq,
        "continue",
        true,
        json!({ "allThreadsContinued": true }),
    );
    client
        .match_response(continue_response)
        .expect("continue response should match");
    assert_eq!(client.phase(), DapClientPhase::Running);

    client
        .apply_event(DapProtocolMessage::event(
            11,
            "continued",
            json!({ "threadId": 1 }),
        ))
        .expect("continued event should be accepted");
    assert_eq!(client.phase(), DapClientPhase::Running);
    assert_eq!(client.last_stop_reason(), None);

    client
        .apply_event(DapProtocolMessage::event(12, "terminated", json!({})))
        .expect("terminated event should be accepted");
    assert_eq!(client.phase(), DapClientPhase::Terminated);
    assert_eq!(client.last_event(), Some("terminated"));
}

#[test]
fn dap_client_state_machine_rejects_lifecycle_mistakes() {
    let mut client = DapClientState::new(DebugSessionId("debug-2".to_string()));

    assert!(client.prepare_launch(json!({})).is_err());
    assert!(client.prepare_threads().is_err());

    let initialize = client
        .prepare_initialize("legion-ide", "lldb-dap")
        .expect("initialize request should be accepted");
    let initialize_response = DapProtocolMessage::response(
        2,
        initialize.seq,
        "initialize",
        true,
        json!({
            "supportsConfigurationDoneRequest": true
        }),
    );
    client
        .match_response(initialize_response)
        .expect("initialize response should match");
    assert_eq!(client.phase(), DapClientPhase::Ready);

    let stray_event = DapProtocolMessage::response(3, 99, "launch", true, json!({}));
    assert!(client.apply_event(stray_event).is_err());

    let attach = client
        .prepare_attach(json!({"processId": 1234}))
        .expect("attach request should be accepted after initialize");
    client
        .match_response(DapProtocolMessage::response(
            4,
            attach.seq,
            "attach",
            true,
            json!({}),
        ))
        .expect("attach response should match");
    assert_eq!(client.phase(), DapClientPhase::Running);
}
