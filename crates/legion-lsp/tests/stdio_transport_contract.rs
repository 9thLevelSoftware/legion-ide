use std::path::PathBuf;
use std::process::Command;

use legion_lsp::{
    LspRuntimeError, LspServerProcessConfig, LspStdioProcess, LspStdioSession, LspStdioSpawner,
    LspSupervisorConfig, LspTextDocumentIdentity, completion_request, definition_request,
    hover_request, project_completion_response, project_hover_response, project_location_response,
    references_request,
};
use legion_protocol::{
    BufferId, BufferVersion, CancellationTokenId, CapabilityDecisionId, CapabilityId, CausalityId,
    CorrelationId, FileFingerprint, FileId, LanguageId, LanguageServerId,
    LspConfiguredServerIdentity, LspRequestId, LspResultStatus, LspSupervisionEventKind,
    LspSupervisionLifecycleState, LspWorkspaceTrustPosture, RedactionHint, SemanticPrivacyScope,
    SnapshotId, Utf16Position, WorkspaceId, WorkspaceRootId, WorkspaceTrustState,
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
        command_hash: fingerprint("command"),
        args_hash: Some(fingerprint("args")),
        env_hash: Some(fingerprint("env")),
        cwd_hash: Some(fingerprint("cwd")),
        settings_hash: Some(fingerprint("settings")),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn document_identity() -> LspTextDocumentIdentity {
    LspTextDocumentIdentity {
        uri: "file:///workspace/src/main.rs".to_string(),
        language_id: LanguageId("rust".to_string()),
        workspace_id: WorkspaceId(55),
        file_id: FileId(5),
        snapshot_id: SnapshotId(6),
        buffer_version: BufferVersion(7),
        content_hash: Some(fingerprint("content")),
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
) -> legion_protocol::LspLaunchPolicyDecision {
    legion_protocol::LspLaunchPolicyDecision::evaluate(
        identity(),
        posture(trust, privacy_scope_allowed),
        runtime_activation_accepted,
        CorrelationId(91),
        CausalityId(Uuid::from_u128(92)),
        Vec::new(),
        1,
    )
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

fn supervisor_config(command: impl Into<String>, args: Vec<String>) -> LspSupervisorConfig {
    LspSupervisorConfig {
        launch_policy: launch_policy(WorkspaceTrustState::Trusted, true, true),
        process: LspServerProcessConfig {
            command: command.into(),
            args,
            cwd: Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")),
            env: Vec::new(),
        },
        initial_backoff_ms: 25,
        max_backoff_ms: 400,
        max_restart_attempts: 3,
    }
}

fn mock_server_config() -> LspSupervisorConfig {
    supervisor_config(env!("CARGO_BIN_EXE_mock_lsp_server"), Vec::new())
}

fn mock_server_config_with_progress() -> LspSupervisorConfig {
    let mut config = mock_server_config();
    config
        .process
        .env
        .push(("MOCK_LSP_EMIT_PROGRESS".to_string(), "1".to_string()));
    config
}

fn mock_server_config_with_diagnostics() -> LspSupervisorConfig {
    let mut config = mock_server_config();
    config
        .process
        .env
        .push(("MOCK_LSP_EMIT_DIAGNOSTICS".to_string(), "1".to_string()));
    config
}

#[derive(Default)]
struct CountingSpawner {
    spawn_calls: usize,
}

impl LspStdioSpawner for CountingSpawner {
    fn spawn_stdio(
        &mut self,
        _config: &LspServerProcessConfig,
    ) -> Result<LspStdioProcess, LspRuntimeError> {
        self.spawn_calls += 1;
        Err(LspRuntimeError::SpawnFailed {
            code: "test.unexpected_spawn".to_string(),
        })
    }
}

#[test]
fn stdio_lsp_session_initializes_against_mock_server() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = LspStdioSession::start(mock_server_config(), &mut launcher).unwrap();

    let response = session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            operation_context(1, 5000),
        )
        .unwrap();

    assert_eq!(response.status, LspResultStatus::Fresh);
    assert_eq!(response.result["serverInfo"]["name"], "mock-lsp-server");
    assert!(session.is_ready());
    assert_eq!(
        session.lifecycle_state(),
        LspSupervisionLifecycleState::Running
    );
    assert!(session.supervision_events().iter().any(|event| {
        event.kind == LspSupervisionEventKind::LifecycleChanged
            && event.lifecycle_state == LspSupervisionLifecycleState::Running
            && event.redaction_hints.contains(&RedactionHint::MetadataOnly)
    }));
}

#[test]
fn stdio_lsp_session_reuses_one_process_across_multiple_requests() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = LspStdioSession::start(mock_server_config(), &mut launcher).unwrap();
    session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            operation_context(2, 5000),
        )
        .unwrap();

    let first = session
        .request("mock.echo", json!({"value": 1}), operation_context(3, 5000))
        .unwrap();
    let second = session
        .request(
            "mock.noise",
            json!({"value": 2}),
            operation_context(4, 5000),
        )
        .unwrap();

    assert_eq!(first.status, LspResultStatus::Fresh);
    assert_eq!(first.result["echo"]["value"], 1);
    assert_eq!(second.status, LspResultStatus::Fresh);
    assert_eq!(second.result["noise"], 3);
    assert!(session.is_running());
}

#[test]
fn stdio_lsp_session_sends_cancel_request_and_rejects_late_response() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = LspStdioSession::start(mock_server_config(), &mut launcher).unwrap();
    session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            operation_context(6, 5000),
        )
        .unwrap();

    let pending = session
        .send_request(
            "mock.echo",
            json!({"value": "late"}),
            operation_context(7, 5000),
        )
        .unwrap();
    let cancelled = session.cancel_request(pending.request_id).unwrap();

    assert_eq!(cancelled.response.status, LspResultStatus::Cancelled);
    assert_eq!(
        cancelled.notification.method.as_deref(),
        Some("$/cancelRequest")
    );
    assert!(matches!(
        session.read_response_for(&pending),
        Err(LspRuntimeError::UnknownResponseId { .. })
    ));
}

#[test]
fn stdio_lsp_session_records_progress_notifications_as_metadata() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session =
        LspStdioSession::start(mock_server_config_with_progress(), &mut launcher).unwrap();

    session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            operation_context(8, 5000),
        )
        .unwrap();

    let progress = session.progress_notifications();
    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0].kind, "begin");
    assert_eq!(progress[0].token_hash.algorithm, "lsp.progress.token");
    assert!(progress[0].label_hash.is_some());
    assert!(
        progress[0]
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
    );
}

#[test]
fn stdio_lsp_session_records_diagnostic_notifications_as_metadata() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session =
        LspStdioSession::start(mock_server_config_with_diagnostics(), &mut launcher).unwrap();

    session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            operation_context(9, 5000),
        )
        .unwrap();

    let diagnostics = session.diagnostic_notifications();
    assert_eq!(diagnostics.len(), 1);
    let notification = &diagnostics[0];
    assert_eq!(notification.uri_hash.algorithm, "lsp.diagnostic.uri");
    assert_eq!(notification.diagnostic_count, 1);
    assert_eq!(notification.error_count, 1);
    assert_eq!(notification.warning_count, 0);
    assert_eq!(notification.source_hashes.len(), 1);
    assert_eq!(notification.diagnostic_hashes.len(), 1);
    assert!(
        notification
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
    );
    assert!(!format!("{notification:?}").contains("SECRET_DIAGNOSTIC_BODY"));
    assert!(!format!("{notification:?}").contains("file:///workspace/src/main.rs"));
}

#[test]
fn stdio_lsp_session_round_trips_read_side_requests_against_mock_server() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = LspStdioSession::start(mock_server_config(), &mut launcher).unwrap();
    session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            operation_context(10, 5000),
        )
        .unwrap();
    let document = document_identity();
    let position = Utf16Position {
        line: 0,
        character: 7,
    };

    let completion = completion_request(100, &document, position)
        .params
        .expect("completion params");
    let response = session
        .request(
            "textDocument/completion",
            completion,
            operation_context(11, 5000),
        )
        .expect("completion response");
    let completions = project_completion_response(&response.result, 10);
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].label, "mockCompletion");

    let hover = hover_request(101, &document, position)
        .params
        .expect("hover params");
    let response = session
        .request("textDocument/hover", hover, operation_context(12, 5000))
        .expect("hover response");
    let hover =
        project_hover_response(&response.result, Some(document.file_id)).expect("hover row");
    assert_eq!(hover.label, "fn mockCompletion() -> ()");
    assert!(hover.range.is_some());

    let definition = definition_request(102, &document, position)
        .params
        .expect("definition params");
    let response = session
        .request(
            "textDocument/definition",
            definition,
            operation_context(13, 5000),
        )
        .expect("definition response");
    let definitions = project_location_response(&response.result, 10);
    assert_eq!(definitions.len(), 1);
    assert!(definitions[0].range.is_some());

    let references = references_request(103, &document, position, true)
        .params
        .expect("references params");
    let response = session
        .request(
            "textDocument/references",
            references,
            operation_context(14, 5000),
        )
        .expect("references response");
    let references = project_location_response(&response.result, 10);
    assert_eq!(references.len(), 2);
}

#[test]
fn stdio_lsp_session_refuses_policy_denied_launch_without_spawning() {
    let mut config = mock_server_config();
    config.launch_policy = launch_policy(WorkspaceTrustState::Untrusted, true, true);
    let mut launcher = CountingSpawner::default();

    let err = match LspStdioSession::start(config, &mut launcher) {
        Ok(_) => panic!("policy-denied launch unexpectedly started"),
        Err(err) => err,
    };

    assert_eq!(launcher.spawn_calls, 0);
    match err {
        LspRuntimeError::SupervisionRefused { events } => {
            assert!(events.iter().any(|event| {
                event.kind == LspSupervisionEventKind::LaunchRefused
                    && event.redaction_hints.contains(&RedactionHint::MetadataOnly)
            }));
        }
        other => panic!("expected supervision refusal, got {other:?}"),
    }
}

#[test]
fn rust_analyzer_initializes_against_legion_repo_when_opted_in() {
    if std::env::var("LEGION_RUN_RUST_ANALYZER_SMOKE").as_deref() != Ok("1") {
        eprintln!("skipping rust-analyzer smoke; set LEGION_RUN_RUST_ANALYZER_SMOKE=1 to run");
        return;
    }

    let ra = std::env::var("RUST_ANALYZER").unwrap_or_else(|_| "rust-analyzer".to_string());
    if Command::new(&ra).arg("--version").output().is_err() {
        eprintln!("skipping rust-analyzer smoke; `{ra}` is not available");
        return;
    }

    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session =
        LspStdioSession::start(supervisor_config(ra, Vec::new()), &mut launcher).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let response = session
        .initialize(
            json!({
                "processId": null,
                "rootUri": format!("file://{}", root.display()),
                "capabilities": {},
            }),
            operation_context(5, 10_000),
        )
        .unwrap();

    assert_eq!(response.status, LspResultStatus::Fresh);
    assert!(response.result["capabilities"].is_object());
}
