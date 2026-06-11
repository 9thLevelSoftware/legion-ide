use legion_lsp::{
    JsonRpcEnvelope, LspClient, LspFramer, LspProcessHandle, LspProcessLauncher, LspRuntimeError,
    LspServerProcessConfig, LspSupervisor, LspSupervisorConfig,
};
use legion_protocol::{
    BufferId, BufferVersion, CancellationTokenId, CapabilityDecisionId, CapabilityId, CausalityId,
    CorrelationId, FileFingerprint, FileId, LanguageId, LanguageServerId,
    LspConfiguredServerIdentity, LspHealthState, LspLaunchPolicyDecision, LspRequestId,
    LspResultStatus, LspSupervisionEventKind, LspSupervisionLifecycleState,
    LspWorkspaceTrustPosture, RedactionHint, SemanticPrivacyScope, SnapshotId, WorkspaceId,
    WorkspaceRootId, WorkspaceTrustState,
};
use serde_json::json;
use uuid::Uuid;

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "test".to_string(),
        value: value.to_string(),
    }
}

fn identity() -> LspConfiguredServerIdentity {
    LspConfiguredServerIdentity {
        server_id: LanguageServerId(7),
        workspace_id: WorkspaceId(55),
        root_id: Some(WorkspaceRootId(5)),
        language_id: LanguageId("rust".to_string()),
        display_name: "rust-analyzer".to_string(),
        command_hash: fingerprint("rust-analyzer"),
        args_hash: Some(fingerprint("args")),
        env_hash: Some(fingerprint("env")),
        cwd_hash: Some(fingerprint("cwd")),
        settings_hash: Some(fingerprint("settings")),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn posture(trust: WorkspaceTrustState, privacy_scope_allowed: bool) -> LspWorkspaceTrustPosture {
    LspWorkspaceTrustPosture {
        workspace_id: WorkspaceId(55),
        workspace_trust_state: trust,
        privacy_scope: SemanticPrivacyScope::Workspace,
        privacy_scope_allowed,
        required_capability: CapabilityId("process.spawn".to_string()),
        decision_id: Some(CapabilityDecisionId(99)),
        diagnostics: Vec::new(),
        schema_version: 1,
    }
}

fn launch_policy(
    trust: WorkspaceTrustState,
    privacy_scope_allowed: bool,
    runtime_activation_accepted: bool,
) -> LspLaunchPolicyDecision {
    LspLaunchPolicyDecision::evaluate(
        identity(),
        posture(trust, privacy_scope_allowed),
        runtime_activation_accepted,
        CorrelationId(91),
        CausalityId(Uuid::from_u128(92)),
        Vec::new(),
        1,
    )
}

fn process_config() -> LspServerProcessConfig {
    LspServerProcessConfig {
        command: "mock-lsp-server".to_string(),
        args: vec!["--stdio".to_string()],
        cwd: None,
        env: vec![("RUST_LOG".to_string(), "warn".to_string())],
    }
}

fn operation_context(
    request_number: u128,
    timeout_ms: u64,
) -> legion_protocol::LspOperationContext {
    legion_protocol::LspOperationContext {
        request_id: LspRequestId(Uuid::from_u128(request_number)),
        workspace_id: WorkspaceId(55),
        file_id: FileId(11),
        buffer_id: BufferId(12),
        snapshot_id: SnapshotId(13),
        buffer_version: BufferVersion(14),
        language_id: LanguageId("rust".to_string()),
        correlation_id: CorrelationId(request_number as u64),
        causality_id: CausalityId(Uuid::from_u128(request_number + 1000)),
        timeout_ms,
        cancellation_token: CancellationTokenId(Uuid::from_u128(request_number + 2000)),
        content_hash: Some(fingerprint(&format!("content-{request_number}"))),
        privacy_scope: SemanticPrivacyScope::Workspace,
        schema_version: 1,
    }
}

#[derive(Default)]
struct RecordingLauncher {
    spawn_calls: usize,
    fail_spawns: bool,
    spawned_commands: Vec<String>,
}

impl LspProcessLauncher for RecordingLauncher {
    fn spawn(
        &mut self,
        config: &LspServerProcessConfig,
    ) -> Result<Box<dyn LspProcessHandle>, LspRuntimeError> {
        self.spawn_calls += 1;
        self.spawned_commands.push(config.command.clone());
        if self.fail_spawns {
            return Err(LspRuntimeError::SpawnFailed {
                code: "mock.spawn_failed".to_string(),
            });
        }
        Ok(Box::new(RecordingProcess { running: true }))
    }
}

struct RecordingProcess {
    running: bool,
}

impl LspProcessHandle for RecordingProcess {
    fn is_running(&mut self) -> bool {
        self.running
    }

    fn kill(&mut self) {
        self.running = false;
    }
}

#[test]
fn lifecycle_lsp_supervisor_rejects_untrusted_workspace_without_spawning_process() {
    let mut launcher = RecordingLauncher::default();
    let mut supervisor = LspSupervisor::new(LspSupervisorConfig {
        launch_policy: launch_policy(WorkspaceTrustState::Untrusted, true, true),
        process: process_config(),
        initial_backoff_ms: 25,
        max_backoff_ms: 400,
        max_restart_attempts: 3,
    });

    let events = supervisor.ensure_started(&mut launcher);

    assert_eq!(launcher.spawn_calls, 0);
    assert_eq!(
        supervisor.lifecycle_state(),
        LspSupervisionLifecycleState::Disabled
    );
    assert_eq!(supervisor.health_state(), LspHealthState::Unavailable);
    assert!(events.iter().any(|event| {
        event.kind == LspSupervisionEventKind::LaunchRefused
            && event.lifecycle_state == LspSupervisionLifecycleState::Disabled
            && event.correlation_id != CorrelationId(0)
            && event.redaction_hints.contains(&RedactionHint::MetadataOnly)
    }));
}

#[test]
fn lifecycle_lsp_supervisor_starts_running_server_and_emits_lifecycle_changed() {
    let mut launcher = RecordingLauncher::default();
    let mut supervisor = LspSupervisor::new(LspSupervisorConfig {
        launch_policy: launch_policy(WorkspaceTrustState::Trusted, true, true),
        process: process_config(),
        initial_backoff_ms: 25,
        max_backoff_ms: 400,
        max_restart_attempts: 3,
    });

    let events = supervisor.ensure_started(&mut launcher);

    assert_eq!(launcher.spawn_calls, 1);
    assert_eq!(
        launcher.spawned_commands,
        vec!["mock-lsp-server".to_string()]
    );
    assert_eq!(
        supervisor.lifecycle_state(),
        LspSupervisionLifecycleState::Running
    );
    assert_eq!(supervisor.health_state(), LspHealthState::Healthy);
    assert!(events.iter().any(|event| {
        event.kind == LspSupervisionEventKind::LifecycleChanged
            && event.lifecycle_state == LspSupervisionLifecycleState::Running
    }));
}

#[test]
fn lifecycle_lsp_supervisor_recovers_from_crash_with_bounded_backoff() {
    let mut launcher = RecordingLauncher {
        fail_spawns: true,
        ..RecordingLauncher::default()
    };
    let mut supervisor = LspSupervisor::new(LspSupervisorConfig {
        launch_policy: launch_policy(WorkspaceTrustState::Trusted, true, true),
        process: process_config(),
        initial_backoff_ms: 25,
        max_backoff_ms: 80,
        max_restart_attempts: 2,
    });

    let first = supervisor.ensure_started(&mut launcher);
    let second = supervisor.ensure_started(&mut launcher);
    let third = supervisor.ensure_started(&mut launcher);

    assert_eq!(launcher.spawn_calls, 2);
    assert_eq!(
        supervisor.lifecycle_state(),
        LspSupervisionLifecycleState::CircuitOpen
    );
    assert_eq!(supervisor.health_state(), LspHealthState::Unavailable);
    assert!(first.iter().any(|event| {
        event.kind == LspSupervisionEventKind::RestartBackoffUpdated
            && event.restart_backoff.as_ref().is_some_and(|backoff| {
                backoff.restart_attempts == 1 && backoff.next_backoff_ms == 25
            })
    }));
    assert!(second.iter().any(|event| {
        event.kind == LspSupervisionEventKind::RestartBackoffUpdated
            && event.restart_backoff.as_ref().is_some_and(|backoff| {
                backoff.restart_attempts == 2
                    && backoff.next_backoff_ms == 50
                    && !backoff.circuit_breaker_open
            })
    }));
    assert!(third.iter().any(|event| {
        event.lifecycle_state == LspSupervisionLifecycleState::CircuitOpen
            && event.restart_backoff.as_ref().is_some_and(|backoff| {
                backoff.restart_attempts == 2 && backoff.circuit_breaker_open
            })
    }));
}

#[test]
fn transport_lsp_framer_round_trips_content_length_messages() {
    let envelope = JsonRpcEnvelope::request(1, "initialize", json!({"processId": null}));

    let frame = LspFramer::encode(&envelope).unwrap();
    let decoded = LspFramer::decode(&frame).unwrap();

    assert!(
        std::str::from_utf8(&frame)
            .unwrap()
            .starts_with("Content-Length: ")
    );
    assert_eq!(decoded, envelope);
}

#[test]
fn transport_lsp_framer_accepts_case_insensitive_content_length_header() {
    let envelope = JsonRpcEnvelope::request(1, "initialize", json!({"processId": null}));
    let frame = LspFramer::encode(&envelope).unwrap();
    let lower_case_frame = String::from_utf8(frame)
        .unwrap()
        .replacen("Content-Length", "content-length", 1)
        .into_bytes();

    let decoded = LspFramer::decode(&lower_case_frame).unwrap();

    assert_eq!(decoded, envelope);
}

#[test]
fn transport_lsp_framer_rejects_oversized_payload_length() {
    let frame = format!(
        "Content-Length: {}\r\n\r\n{{}}",
        LspFramer::MAX_FRAME_PAYLOAD_BYTES + 1
    );

    assert!(LspFramer::decode(frame.as_bytes()).is_err());
}

#[test]
fn transport_lsp_client_correlates_out_of_order_responses_by_id() {
    let mut client = LspClient::new();
    let first = client.prepare_request(
        "textDocument/hover",
        json!({"position": {"line": 1, "character": 2}}),
        operation_context(1, 100),
    );
    let second = client.prepare_request(
        "textDocument/hover",
        json!({"position": {"line": 3, "character": 4}}),
        operation_context(2, 100),
    );

    let second_response =
        JsonRpcEnvelope::response(second.json_rpc_id, json!({"contents": "second"}));
    let first_response = JsonRpcEnvelope::response(first.json_rpc_id, json!({"contents": "first"}));

    let correlated_second = client.correlate_response(second_response).unwrap();
    let correlated_first = client.correlate_response(first_response).unwrap();

    assert_eq!(correlated_second.request_id, second.request_id);
    assert_eq!(correlated_second.status, LspResultStatus::Fresh);
    assert_eq!(correlated_second.result["contents"], "second");
    assert_eq!(correlated_first.request_id, first.request_id);
    assert_eq!(correlated_first.status, LspResultStatus::Fresh);
    assert_eq!(correlated_first.result["contents"], "first");
}

#[test]
fn transport_lsp_client_times_out_request_after_budget() {
    let mut client = LspClient::new();
    let pending = client.prepare_request(
        "textDocument/hover",
        json!({"position": {"line": 1, "character": 2}}),
        operation_context(3, 10),
    );

    let timeout = client.resolve_timeout(pending.request_id, 11).unwrap();

    assert_eq!(timeout.request_id, pending.request_id);
    assert_eq!(timeout.status, LspResultStatus::Timeout);
    assert!(
        client
            .correlate_response(JsonRpcEnvelope::response(
                pending.json_rpc_id,
                json!({"contents": "late"})
            ))
            .is_err()
    );
}

#[test]
fn transport_lsp_client_maps_json_rpc_errors_to_unavailable_status() {
    let mut client = LspClient::new();
    let pending = client.prepare_request(
        "textDocument/hover",
        json!({"position": {"line": 1, "character": 2}}),
        operation_context(4, 100),
    );

    let correlated = client
        .correlate_response(JsonRpcEnvelope::error_response(
            pending.json_rpc_id,
            -32601,
            "method not found",
        ))
        .unwrap();

    assert_eq!(correlated.request_id, pending.request_id);
    assert_eq!(correlated.status, LspResultStatus::Unavailable);
    assert!(correlated.error.is_some());
}

#[test]
fn transport_lsp_client_cancels_pending_request_and_rejects_late_response() {
    let mut client = LspClient::new();
    let pending = client.prepare_request(
        "textDocument/hover",
        json!({"position": {"line": 1, "character": 2}}),
        operation_context(5, 100),
    );

    let cancelled = client.cancel_request(pending.request_id).unwrap();

    assert_eq!(cancelled.request_id, pending.request_id);
    assert_eq!(cancelled.json_rpc_id, pending.json_rpc_id);
    assert_eq!(cancelled.response.status, LspResultStatus::Cancelled);
    assert_eq!(
        cancelled.notification.method.as_deref(),
        Some("$/cancelRequest")
    );
    assert_eq!(
        cancelled.notification.params.as_ref().unwrap()["id"],
        pending.json_rpc_id
    );
    assert!(matches!(
        client.correlate_response(JsonRpcEnvelope::response(
            pending.json_rpc_id,
            json!({"contents": "late"})
        )),
        Err(LspRuntimeError::UnknownResponseId { .. })
    ));
}
