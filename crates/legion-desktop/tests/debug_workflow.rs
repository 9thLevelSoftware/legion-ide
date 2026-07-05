use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use legion_desktop::{
    bridge::DesktopAction,
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{
    DapClientPhase, DapClientState, DapProtocolMessage, DebugSessionId, decode_dap_message,
    encode_dap_message,
};
use legion_ui::{DebugStatusKindProjection, DebugStepKindProjection};
use serde_json::{Value, json};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-desktop-debug-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "desktop-debug"
version = "0.1.0"
edition = "2024"
"#,
    )
    .expect("write Cargo.toml");
    fs::write(
        root.join("src/main.rs"),
        "fn main() {\n    let count = 3;\n    println!(\"{count}\");\n}\n",
    )
    .expect("write main");
    root
}

#[test]
fn desktop_debug_workflow_projects_right_and_bottom_debug_rows() {
    let root = create_root();
    let source = root.join("src/main.rs");
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        root.clone(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace");
    runtime.enable_debug_fixture_for_tests();

    assert_eq!(
        runtime
            .handle_action(DesktopAction::RefreshDebugConfigurations)
            .expect("refresh debug configs"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    let config_id = runtime
        .projection_snapshot()
        .debug_projection
        .configurations
        .iter()
        .find(|config| config.configuration_id.0 == "cargo:desktop-debug:bin:desktop-debug")
        .expect("cargo debug config should exist")
        .configuration_id
        .clone();

    assert_eq!(
        runtime
            .handle_action(DesktopAction::ToggleDebugBreakpoint {
                line: 1,
                condition: Some("count > 2".to_string()),
                hit_condition: Some("3".to_string()),
                log_message: Some("count changed".to_string()),
            })
            .expect("toggle breakpoint"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::LaunchDebugSession {
                configuration_id: config_id,
            })
            .expect("launch debug"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    let snapshot = runtime.projection_snapshot();
    assert_eq!(
        snapshot.debug_projection.status.kind,
        DebugStatusKindProjection::Paused
    );
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug config"))
    );
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug breakpoint"))
    );
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug frame"))
    );
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug variable"))
    );
    assert!(
        model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug console"))
    );

    let session_id = snapshot
        .debug_projection
        .active_session_id
        .clone()
        .expect("active debug session");
    assert_eq!(
        runtime
            .handle_action(DesktopAction::DebugStep {
                session_id: session_id.clone(),
                kind: DebugStepKindProjection::Over,
            })
            .expect("debug step"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::DebugEvaluateSelection {
                session_id,
                expression_label: "count".to_string(),
            })
            .expect("debug evaluate"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );
    let model = DesktopProjectionViewModel::from_snapshot(&runtime.projection_snapshot());
    assert!(model.debug_rows.iter().any(|row| row.contains("step=over")));
    assert!(model.debug_rows.iter().any(|row| row.contains("evaluate")));

    fs::remove_dir_all(root).ok();
}

/// Minimal protocol-level fake DAP server speaking the real DAP wire format
/// (`Content-Length`-framed messages round-tripped through the protocol codec).
/// It answers initialize, setBreakpoints, launch, threads, stackTrace, scopes,
/// variables, step (`next`), and evaluate, and can be told to fail the launch so
/// error propagation can be observed at the protocol layer.
struct FakeDapServer {
    seq: u64,
    program: String,
    fail_launch: bool,
}

impl FakeDapServer {
    fn new(program: impl Into<String>) -> Self {
        Self {
            seq: 0,
            program: program.into(),
            fail_launch: false,
        }
    }

    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    fn respond(&mut self, request: &DapProtocolMessage) -> DapProtocolMessage {
        let framed = encode_dap_message(request).expect("client request should encode to the wire");
        let decoded =
            decode_dap_message(&framed).expect("fake server should decode the client request");
        let command = decoded.command.clone().unwrap_or_default();
        let success = !(command == "launch" && self.fail_launch);
        let body = match command.as_str() {
            "initialize" => json!({
                "supportsConfigurationDoneRequest": true,
                "supportsConditionalBreakpoints": true,
                "supportsEvaluateForHovers": true,
                "supportsSteppingGranularity": true,
            }),
            "setBreakpoints" => json!({
                "breakpoints": [{ "verified": true, "line": 1, "source": { "path": self.program } }]
            }),
            "launch" if !success => json!({ "error": { "id": 1, "format": "launch denied" } }),
            "launch" => json!({}),
            "threads" => json!({ "threads": [{ "id": 1, "name": "main" }] }),
            "stackTrace" => json!({
                "stackFrames": [{
                    "id": 1,
                    "name": "main",
                    "line": 1,
                    "source": { "path": self.program },
                }]
            }),
            "scopes" => json!({
                "scopes": [{ "name": "Locals", "variablesReference": 1, "expensive": false }]
            }),
            "variables" => json!({
                "variables": [{
                    "name": "count",
                    "value": "3",
                    "type": "i32",
                    "variablesReference": 0,
                }]
            }),
            "next" | "stepIn" | "stepOut" => json!({}),
            "evaluate" => json!({ "result": "3", "type": "i32", "variablesReference": 0 }),
            _ => json!({}),
        };
        let response =
            DapProtocolMessage::response(self.next_seq(), decoded.seq, command, success, body);
        let response_framed =
            encode_dap_message(&response).expect("adapter response should encode to the wire");
        decode_dap_message(&response_framed).expect("client should decode the adapter response")
    }

    fn event(&mut self, name: &str, body: Value) -> DapProtocolMessage {
        let event = DapProtocolMessage::event(self.next_seq(), name, body);
        let framed = encode_dap_message(&event).expect("adapter event should encode to the wire");
        decode_dap_message(&framed).expect("client should decode the adapter event")
    }
}

/// Gated DAP integration test (gated behind the deterministic debug fixture and
/// the trusted-workspace launch policy). Drives the production debug workflow AND
/// a protocol-level fake DAP server validating launch, breakpoint binding,
/// stopped event, step, evaluate, and error propagation from the protocol layer.
#[test]
fn debug_workflow_protocol_dap_server_validates_launch_step_evaluate_and_errors() {
    let root = create_root();
    let source = root.join("src/main.rs");
    let program = source.to_string_lossy().replace('\\', "/");
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        root.clone(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace");
    runtime.enable_debug_fixture_for_tests();

    runtime
        .handle_action(DesktopAction::RefreshDebugConfigurations)
        .expect("refresh debug configs");
    let config_id = runtime
        .projection_snapshot()
        .debug_projection
        .configurations
        .iter()
        .find(|config| config.configuration_id.0 == "cargo:desktop-debug:bin:desktop-debug")
        .expect("cargo debug config should exist")
        .configuration_id
        .clone();
    runtime
        .handle_action(DesktopAction::ToggleDebugBreakpoint {
            line: 1,
            condition: Some("count > 2".to_string()),
            hit_condition: None,
            log_message: None,
        })
        .expect("toggle breakpoint");
    runtime
        .handle_action(DesktopAction::LaunchDebugSession {
            configuration_id: config_id,
        })
        .expect("launch debug");
    let session_id = runtime
        .projection_snapshot()
        .debug_projection
        .active_session_id
        .clone()
        .expect("active debug session");
    runtime
        .handle_action(DesktopAction::DebugStep {
            session_id: session_id.clone(),
            kind: DebugStepKindProjection::Over,
        })
        .expect("debug step");
    runtime
        .handle_action(DesktopAction::DebugEvaluateSelection {
            session_id,
            expression_label: "count".to_string(),
        })
        .expect("debug evaluate");
    let production = runtime.projection_snapshot().debug_projection;
    let production_frame = production
        .stack_frames
        .first()
        .expect("production projection should surface a stack frame");

    // Error propagation: the protocol layer rejects lifecycle mistakes and
    // surfaces a failed launch by returning the client to the Ready phase.
    let mut error_server = FakeDapServer::new(program.clone());
    error_server.fail_launch = true;
    let mut error_client = DapClientState::new(DebugSessionId("debug:protocol:error".to_string()));
    assert!(
        error_client.prepare_launch(json!({})).is_err(),
        "launch before initialize must be rejected by the protocol layer"
    );
    let error_initialize = error_client
        .prepare_initialize("legion-ide", "lldb-dap")
        .expect("initialize request should be accepted");
    error_client
        .match_response(error_server.respond(&error_initialize))
        .expect("initialize response should match");
    let failed_launch = error_client
        .prepare_launch(json!({ "program": program }))
        .expect("launch request should be accepted after initialize");
    let failed_match = error_client
        .match_response(error_server.respond(&failed_launch))
        .expect("a failed launch is still a well-formed response");
    assert!(
        !failed_match.success,
        "failed launch must propagate failure"
    );
    assert_eq!(
        error_client.phase(),
        DapClientPhase::Ready,
        "a failed launch must propagate back to the Ready phase"
    );

    // Happy path: protocol-level launch, breakpoint binding, stopped, step,
    // evaluate, and continue.
    let mut server = FakeDapServer::new(program.clone());
    let mut client = DapClientState::new(DebugSessionId("debug:protocol:debug".to_string()));
    let initialize = client
        .prepare_initialize("legion-ide", "lldb-dap")
        .expect("initialize request should be accepted");
    client
        .match_response(server.respond(&initialize))
        .expect("initialize response should match");
    client
        .apply_event(server.event("initialized", json!({})))
        .expect("initialized event should be accepted");

    let set_breakpoints = client
        .prepare_request(
            "setBreakpoints",
            json!({
                "source": { "path": program },
                "breakpoints": [{ "line": 1, "condition": "count > 2" }],
            }),
        )
        .expect("setBreakpoints request should be accepted");
    let breakpoints_response = server.respond(&set_breakpoints);
    client
        .match_response(breakpoints_response.clone())
        .expect("setBreakpoints response should match");
    assert_eq!(
        breakpoints_response
            .body
            .as_ref()
            .and_then(|body| body.get("breakpoints"))
            .and_then(Value::as_array)
            .and_then(|breakpoints| breakpoints.first())
            .and_then(|breakpoint| breakpoint.get("verified"))
            .and_then(Value::as_bool),
        Some(true)
    );

    let launch = client
        .prepare_launch(json!({ "program": program, "stopOnEntry": false }))
        .expect("launch request should be accepted");
    client
        .match_response(server.respond(&launch))
        .expect("launch response should match");
    assert_eq!(client.phase(), DapClientPhase::Running);

    client
        .apply_event(server.event("stopped", json!({ "reason": "breakpoint", "threadId": 1 })))
        .expect("stopped event should be accepted");
    assert_eq!(client.phase(), DapClientPhase::Paused);
    assert_eq!(client.last_stop_reason(), Some("breakpoint"));

    let threads = client.prepare_threads().expect("threads while paused");
    client
        .match_response(server.respond(&threads))
        .expect("threads response should match");
    let stack_trace = client
        .prepare_stack_trace(1)
        .expect("stackTrace while paused");
    let stack_response = server.respond(&stack_trace);
    client
        .match_response(stack_response.clone())
        .expect("stackTrace response should match");
    let protocol_frame_name = stack_response
        .body
        .as_ref()
        .and_then(|body| body.get("stackFrames"))
        .and_then(Value::as_array)
        .and_then(|frames| frames.first())
        .and_then(|frame| frame.get("name"))
        .and_then(Value::as_str)
        .expect("stack frame should carry a name");

    // Step derived from a real `next` exchange followed by a `stopped` event.
    let step = client
        .prepare_request("next", json!({ "threadId": 1, "granularity": "statement" }))
        .expect("next request should be accepted while paused");
    let step_match = client
        .match_response(server.respond(&step))
        .expect("next response should match");
    assert!(step_match.success);
    client
        .apply_event(server.event("stopped", json!({ "reason": "step", "threadId": 1 })))
        .expect("post-step stopped event should be accepted");
    assert_eq!(client.last_stop_reason(), Some("step"));

    // Evaluate derived from a real `evaluate` exchange.
    let evaluate = client
        .prepare_request(
            "evaluate",
            json!({ "expression": "count", "frameId": 1, "context": "watch" }),
        )
        .expect("evaluate request should be accepted while paused");
    let evaluate_response = server.respond(&evaluate);
    client
        .match_response(evaluate_response.clone())
        .expect("evaluate response should match");
    assert_eq!(
        evaluate_response
            .body
            .as_ref()
            .and_then(|body| body.get("result"))
            .and_then(Value::as_str),
        Some("3")
    );

    let continue_request = client.prepare_continue(1).expect("continue while paused");
    client
        .match_response(server.respond(&continue_request))
        .expect("continue response should match");
    assert_eq!(client.phase(), DapClientPhase::Running);

    // Cross-validate: the protocol-derived frame agrees with the production
    // projection driven through the real desktop debug workflow path.
    assert_eq!(protocol_frame_name, production_frame.name);
    assert!(
        production
            .variables
            .iter()
            .any(|variable| variable.name == "count")
    );

    fs::remove_dir_all(root).ok();
}
