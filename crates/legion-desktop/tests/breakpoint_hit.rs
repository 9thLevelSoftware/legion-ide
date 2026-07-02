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
        "legion-desktop-breakpoint-hit-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "desktop-breakpoint-hit"
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
fn breakpoint_hit_surfaces_locals_in_the_debug_panel() {
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
    let configuration_id = runtime
        .projection_snapshot()
        .debug_projection
        .configurations
        .iter()
        .find(|config| {
            config.configuration_id.0 == "cargo:desktop-breakpoint-hit:bin:desktop-breakpoint-hit"
        })
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
            .handle_action(DesktopAction::LaunchDebugSession { configuration_id })
            .expect("launch debug"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );

    let snapshot = runtime.projection_snapshot();
    assert_eq!(
        snapshot.debug_projection.status.kind,
        DebugStatusKindProjection::Paused
    );
    assert_eq!(snapshot.debug_projection.breakpoints.len(), 1);
    assert!(
        snapshot.debug_projection.breakpoints[0]
            .session_id
            .is_none()
    );

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
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
    assert!(model.debug_rows.iter().any(|row| row.contains("count")));

    let session_id = snapshot
        .debug_projection
        .active_session_id
        .clone()
        .expect("active debug session");
    assert_eq!(
        runtime
            .handle_action(DesktopAction::DebugStep {
                session_id,
                kind: DebugStepKindProjection::Continue,
            })
            .expect("continue debug"),
        DesktopWorkflowOutcome::DebugProjectionUpdated
    );

    let continued = runtime.projection_snapshot();
    assert_eq!(
        continued.debug_projection.status.kind,
        DebugStatusKindProjection::Paused
    );
    assert!(
        continued
            .debug_projection
            .console
            .iter()
            .any(|entry| entry.message_label.contains("continue"))
    );

    fs::remove_dir_all(root).ok();
}

/// Minimal protocol-level fake DAP server. It speaks the real DAP wire format
/// (`Content-Length`-framed `DapProtocolMessage`s round-tripped through
/// `encode_dap_message`/`decode_dap_message`) and answers the handshake a real
/// debugger drives: initialize, setBreakpoints binding, launch, threads,
/// stackTrace, scopes, and variables.
struct FakeDapServer {
    seq: u64,
    program: String,
}

impl FakeDapServer {
    fn new(program: impl Into<String>) -> Self {
        Self {
            seq: 0,
            program: program.into(),
        }
    }

    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    /// Decode a framed client request and produce the framed adapter response,
    /// returning the decoded response message.
    fn respond(&mut self, request: &DapProtocolMessage) -> DapProtocolMessage {
        let framed = encode_dap_message(request).expect("client request should encode to the wire");
        let decoded =
            decode_dap_message(&framed).expect("fake server should decode the client request");
        let command = decoded.command.clone().unwrap_or_default();
        let body = match command.as_str() {
            "initialize" => json!({
                "supportsConfigurationDoneRequest": true,
                "supportsConditionalBreakpoints": true,
                "supportsEvaluateForHovers": true,
            }),
            "setBreakpoints" => {
                let bound = decoded
                    .arguments
                    .as_ref()
                    .and_then(|args| args.get("breakpoints"))
                    .and_then(Value::as_array)
                    .map(|breakpoints| {
                        breakpoints
                            .iter()
                            .map(|breakpoint| {
                                let line =
                                    breakpoint.get("line").and_then(Value::as_u64).unwrap_or(0);
                                json!({
                                    "verified": true,
                                    "line": line,
                                    "source": { "path": self.program },
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                json!({ "breakpoints": bound })
            }
            "launch" => json!({}),
            "threads" => json!({ "threads": [{ "id": 1, "name": "main" }] }),
            "stackTrace" => json!({
                "stackFrames": [{
                    "id": 1,
                    "name": "main",
                    "line": 1,
                    "source": { "path": self.program },
                }],
                "totalFrames": 1,
            }),
            "scopes" => json!({
                "scopes": [{
                    "name": "Locals",
                    "variablesReference": 1,
                    "expensive": false,
                }]
            }),
            "variables" => json!({
                "variables": [{
                    "name": "count",
                    "value": "3",
                    "type": "i32",
                    "variablesReference": 0,
                }]
            }),
            _ => json!({}),
        };
        let response =
            DapProtocolMessage::response(self.next_seq(), decoded.seq, command, true, body);
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
/// the trusted-workspace launch policy). Drives the production breakpoint+launch
/// path AND a protocol-level fake DAP server, then asserts the breakpoint
/// binding, stopped event, stack frame, and variables derived from the actual
/// protocol events agree with the production debug projection.
#[test]
fn breakpoint_hit_protocol_dap_server_binds_breakpoint_and_surfaces_locals() {
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
    let configuration_id = runtime
        .projection_snapshot()
        .debug_projection
        .configurations
        .iter()
        .find(|config| {
            config.configuration_id.0 == "cargo:desktop-breakpoint-hit:bin:desktop-breakpoint-hit"
        })
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
        .handle_action(DesktopAction::LaunchDebugSession { configuration_id })
        .expect("launch debug");

    let production = runtime.projection_snapshot().debug_projection;
    assert_eq!(production.status.kind, DebugStatusKindProjection::Paused);
    let production_frame = production
        .stack_frames
        .first()
        .expect("production projection should surface a stack frame");
    assert!(
        production
            .variables
            .iter()
            .any(|variable| variable.name == "count"),
        "production projection should surface the `count` local"
    );

    // Protocol-level fake DAP server handshake.
    let mut server = FakeDapServer::new(program.clone());
    let mut client = DapClientState::new(DebugSessionId("debug:protocol:breakpoint".to_string()));

    let initialize = client
        .prepare_initialize("legion-ide", "lldb-dap")
        .expect("initialize request should be accepted");
    client
        .match_response(server.respond(&initialize))
        .expect("initialize response should match");
    assert_eq!(client.phase(), DapClientPhase::Ready);
    client
        .apply_event(server.event("initialized", json!({})))
        .expect("initialized event should be accepted");

    // Breakpoint binding derived from a real setBreakpoints exchange.
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
    let bound = breakpoints_response
        .body
        .as_ref()
        .and_then(|body| body.get("breakpoints"))
        .and_then(Value::as_array)
        .and_then(|breakpoints| breakpoints.first())
        .expect("adapter should report a bound breakpoint");
    assert_eq!(bound.get("verified").and_then(Value::as_bool), Some(true));
    assert_eq!(bound.get("line").and_then(Value::as_u64), Some(1));

    // Launch request derived from the protocol exchange.
    let launch = client
        .prepare_launch(json!({ "program": program, "stopOnEntry": false }))
        .expect("launch request should be accepted after initialize");
    client
        .match_response(server.respond(&launch))
        .expect("launch response should match");
    assert_eq!(client.phase(), DapClientPhase::Running);

    // Stopped event derived from the protocol.
    client
        .apply_event(server.event("stopped", json!({ "reason": "breakpoint", "threadId": 1 })))
        .expect("stopped event should be accepted");
    assert_eq!(client.phase(), DapClientPhase::Paused);
    assert_eq!(client.last_stop_reason(), Some("breakpoint"));

    // Stack frame derived from the protocol.
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
    let protocol_frame = stack_response
        .body
        .as_ref()
        .and_then(|body| body.get("stackFrames"))
        .and_then(Value::as_array)
        .and_then(|frames| frames.first())
        .expect("adapter should report a stack frame");
    let protocol_frame_name = protocol_frame
        .get("name")
        .and_then(Value::as_str)
        .expect("stack frame should carry a name");

    // Variables derived from the protocol.
    let scopes = client.prepare_scopes(1).expect("scopes while paused");
    client
        .match_response(server.respond(&scopes))
        .expect("scopes response should match");
    let variables = client.prepare_variables(1).expect("variables while paused");
    let variables_response = server.respond(&variables);
    client
        .match_response(variables_response.clone())
        .expect("variables response should match");
    let protocol_variable = variables_response
        .body
        .as_ref()
        .and_then(|body| body.get("variables"))
        .and_then(Value::as_array)
        .and_then(|variables| variables.first())
        .expect("adapter should report a variable");
    let protocol_variable_name = protocol_variable
        .get("name")
        .and_then(Value::as_str)
        .expect("variable should carry a name");

    // Cross-validate: the protocol-derived frame and variable agree with the
    // production debug projection driven through the real desktop launch path.
    assert_eq!(protocol_frame_name, production_frame.name);
    assert!(
        production
            .variables
            .iter()
            .any(|variable| variable.name == protocol_variable_name)
    );

    fs::remove_dir_all(root).ok();
}
