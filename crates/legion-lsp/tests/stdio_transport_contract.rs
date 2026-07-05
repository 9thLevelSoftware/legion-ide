use std::path::PathBuf;
use std::process::Command;

use legion_lsp::{
    LspRuntimeError, LspServerProcessConfig, LspStdioProcess, LspStdioSession, LspStdioSpawner,
    LspSupervisorConfig, LspTextDocumentIdentity, code_lens_request, completion_request,
    declaration_request, definition_request, document_symbol_request, folding_range_request,
    hover_request, implementation_request, inlay_hint_request, project_code_lens_response,
    project_completion_response, project_document_symbol_response, project_hover_response,
    project_inlay_hint_response, project_location_response, project_workspace_symbol_response,
    references_request, semantic_tokens_full_request, signature_help_request,
    type_definition_request, workspace_symbol_request,
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

/// A server→client REQUEST (`client/registerCapability`) arriving while the
/// client blocks on its own request must be ANSWERED, not dropped: the mock
/// only responds to the original request after its registration is
/// acknowledged with a null result — modeling rust-analyzer's behavior when
/// configured for client-side file watching (PR #47 review, Codex P2).
#[test]
fn stdio_lsp_session_answers_server_register_capability_during_blocking_request() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = LspStdioSession::start(mock_server_config(), &mut launcher).unwrap();
    session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            operation_context(700, 5000),
        )
        .unwrap();

    let response = session
        .request(
            "mock.registerThenDiagnose",
            json!({}),
            operation_context(701, 5000),
        )
        .unwrap();

    assert_eq!(
        response.status,
        LspResultStatus::Fresh,
        "mock acks only after its registerCapability request is answered"
    );
    let registered_hash =
        legion_lsp::lsp_diagnostic_uri_fingerprint("file:///workspace/src/registered.rs");
    assert!(
        session
            .diagnostic_notifications()
            .iter()
            .any(|n| n.uri_hash == registered_hash),
        "diagnostics emitted after the registration ack must be buffered"
    );
}

/// The same server→client request arriving during a `pump_until` (the GP-1
/// s3 shape: pumping for diagnostics while rust-analyzer registers its
/// client-side watcher) must be answered so the pump can complete.
#[test]
fn stdio_lsp_session_answers_server_requests_while_pumping() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = LspStdioSession::start(mock_server_config(), &mut launcher).unwrap();
    session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            operation_context(710, 5000),
        )
        .unwrap();

    // Notification-shaped trigger: no response frame will arrive, so every
    // subsequent frame is consumed by the pump path.
    session
        .send_notification("mock.registerThenDiagnose", json!({}))
        .unwrap();

    let registered_hash =
        legion_lsp::lsp_diagnostic_uri_fingerprint("file:///workspace/src/registered.rs");
    let outcome = session
        .pump_until(
            std::time::Instant::now() + std::time::Duration::from_secs(5),
            &mut |acc| {
                acc.diagnostics
                    .iter()
                    .any(|n| n.uri_hash == registered_hash)
            },
        )
        .unwrap();
    assert_eq!(
        outcome,
        legion_lsp::PumpOutcome::PredicateMet,
        "pump must answer the server's registerCapability request and then observe diagnostics"
    );
}

/// A server→client request the client does not implement must receive a
/// JSON-RPC MethodNotFound (-32601) error — the protocol-correct signal that
/// lets the server degrade gracefully instead of waiting forever.
#[test]
fn stdio_lsp_session_rejects_unknown_server_requests_with_method_not_found() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = LspStdioSession::start(mock_server_config(), &mut launcher).unwrap();
    session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            operation_context(720, 5000),
        )
        .unwrap();

    let response = session
        .request(
            "mock.unknownServerRequest",
            json!({}),
            operation_context(721, 5000),
        )
        .unwrap();

    assert_eq!(
        response.status,
        LspResultStatus::Fresh,
        "mock acks only after receiving a -32601 answer to its unknown request"
    );
    let unknown_hash =
        legion_lsp::lsp_diagnostic_uri_fingerprint("file:///workspace/src/unknown.rs");
    assert!(
        session
            .diagnostic_notifications()
            .iter()
            .any(|n| n.uri_hash == unknown_hash),
        "diagnostics emitted after the MethodNotFound answer must be buffered"
    );
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
fn stdio_lsp_session_times_out_when_server_goes_silent() {
    // Regression for F2: a server that accepts a request and then sends
    // nothing must not hang the caller. The read path is bounded by the
    // request's timeout budget even when no further frame ever arrives.
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = LspStdioSession::start(mock_server_config(), &mut launcher).unwrap();
    session
        .initialize(
            json!({"processId": null, "capabilities": {}}),
            operation_context(40, 5000),
        )
        .unwrap();

    let budget_ms = 200;
    let start = std::time::Instant::now();
    // `mock.silent` is accepted by the mock but never answered.
    let response = session
        .request("mock.silent", json!({}), operation_context(41, budget_ms))
        .expect("silent server must resolve as Timeout, not hang or error");
    let elapsed = start.elapsed();

    assert_eq!(response.status, LspResultStatus::Timeout);
    // The call must return close to the budget, not block indefinitely. A
    // generous upper bound keeps the assertion robust on slow CI while still
    // failing hard if the read genuinely hangs.
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "silent server hung the caller for {elapsed:?}"
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(budget_ms),
        "timeout fired before the budget elapsed: {elapsed:?}"
    );
    // The process is left running; only the request is abandoned.
    assert!(session.is_running());
}

// Live smoke against a real rust-analyzer. Gated by `#[ignore]` so it never
// runs in the default `cargo test` pass; a dedicated CI job runs it with
// `cargo test -- --ignored` and `LEGION_RUN_RUST_ANALYZER_SMOKE=1`. When it
// *is* run, a missing/unconfigured binary is a hard failure rather than a
// silent return so the real-server launch path cannot quietly rot.
#[test]
#[ignore = "live rust-analyzer smoke; run via dedicated CI job with `--ignored` and LEGION_RUN_RUST_ANALYZER_SMOKE=1"]
fn rust_analyzer_initializes_against_legion_repo_when_opted_in() {
    assert_eq!(
        std::env::var("LEGION_RUN_RUST_ANALYZER_SMOKE").as_deref(),
        Ok("1"),
        "rust-analyzer smoke must be run with LEGION_RUN_RUST_ANALYZER_SMOKE=1",
    );

    let ra = std::env::var("RUST_ANALYZER").unwrap_or_else(|_| "rust-analyzer".to_string());
    assert!(
        Command::new(&ra).arg("--version").output().is_ok(),
        "rust-analyzer binary `{ra}` is not available; install it or set RUST_ANALYZER",
    );

    let temp_root = std::env::temp_dir().join(format!(
        "legion-ra-smoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(temp_root.join("src")).unwrap();
    let source = r#"pub struct Alpha;

pub fn add(left: u32, right: u32) -> u32 {
    left + right
}

pub fn add_one(value: u32) -> u32 {
    add(value, 1)
}

pub fn completion_fixture() {
    let _ = Alp;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        assert_eq!(add(1, 2), 3);
    }
}
"#;
    std::fs::write(
        temp_root.join("Cargo.toml"),
        "[package]\nname = \"legion_ra_smoke\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(temp_root.join("src/lib.rs"), source).unwrap();

    let document = LspTextDocumentIdentity {
        uri: format!("file://{}", temp_root.join("src/lib.rs").display()),
        language_id: LanguageId("rust".to_string()),
        workspace_id: WorkspaceId(55),
        file_id: FileId(5),
        snapshot_id: SnapshotId(6),
        buffer_version: BufferVersion(7),
        content_hash: Some(fingerprint("content")),
    };

    let position_of = |needle: &str| -> Utf16Position {
        source
            .lines()
            .enumerate()
            .find_map(|(line_index, line)| {
                line.find(needle).map(|byte_index| Utf16Position {
                    line: line_index as u32,
                    character: line[..byte_index].chars().count() as u32,
                })
            })
            .unwrap_or_else(|| panic!("missing `{needle}` in smoke fixture"))
    };
    let position_after = |needle: &str| -> Utf16Position {
        source
            .lines()
            .enumerate()
            .find_map(|(line_index, line)| {
                line.find(needle).map(|byte_index| Utf16Position {
                    line: line_index as u32,
                    character: line[..byte_index + needle.len()].chars().count() as u32,
                })
            })
            .unwrap_or_else(|| panic!("missing `{needle}` in smoke fixture"))
    };

    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = LspStdioSession::start(
        LspSupervisorConfig {
            launch_policy: launch_policy(WorkspaceTrustState::Trusted, true, true),
            process: legion_lsp::LspServerProcessConfig {
                command: ra,
                args: Vec::new(),
                cwd: Some(temp_root.clone()),
                env: Vec::new(),
            },
            initial_backoff_ms: 25,
            max_backoff_ms: 400,
            max_restart_attempts: 3,
        },
        &mut launcher,
    )
    .unwrap();
    let response = session
        .initialize(
            json!({
                "processId": null,
                "rootUri": format!("file://{}", temp_root.display()),
                "capabilities": {},
            }),
            operation_context(5, 10_000),
        )
        .unwrap();

    assert_eq!(response.status, LspResultStatus::Fresh);
    assert!(response.result["capabilities"].is_object());

    session.send_notification("initialized", json!({})).unwrap();
    session
        .send_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": document.uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": source,
                }
            }),
        )
        .unwrap();

    let completion_pos = position_after("Alp");
    let completion = completion_request(100, &document, completion_pos)
        .params
        .expect("completion params");
    let response = session
        .request(
            "textDocument/completion",
            completion,
            operation_context(11, 15_000),
        )
        .expect("completion response");
    let completion_rows = project_completion_response(&response.result, 10);
    assert!(
        completion_rows
            .iter()
            .any(|row| row.label.starts_with("Alpha")),
        "expected an `Alpha` completion at `Alp`, got {completion_rows:?}",
    );

    let hover_pos = position_of("pub fn add(");
    let call_pos = position_of("add(value, 1)");
    let hover = hover_request(101, &document, hover_pos)
        .params
        .expect("hover params");
    let response = session
        .request("textDocument/hover", hover, operation_context(12, 15_000))
        .expect("hover response");
    let hover = project_hover_response(&response.result, Some(document.file_id))
        .expect("hover over `pub fn add(` should project a result");
    assert!(!hover.label.is_empty(), "hover label should be non-empty");
    assert!(
        hover.label.contains("add"),
        "hover label should describe `add`, got {:?}",
        hover.label,
    );

    let definition = definition_request(102, &document, call_pos)
        .params
        .expect("definition params");
    let response = session
        .request(
            "textDocument/definition",
            definition,
            operation_context(13, 15_000),
        )
        .expect("definition response");
    let definitions = project_location_response(&response.result, 10);
    assert!(
        !definitions.is_empty(),
        "go-to-definition on the `add` call should resolve a location",
    );

    let references = references_request(103, &document, position_of("pub fn add("), true)
        .params
        .expect("references params");
    let response = session
        .request(
            "textDocument/references",
            references,
            operation_context(14, 15_000),
        )
        .expect("references response");
    let references = project_location_response(&response.result, 10);
    assert!(
        !references.is_empty(),
        "find-references on `add` should resolve at least the definition and call site",
    );

    let declaration = declaration_request(104, &document, call_pos)
        .params
        .expect("declaration params");
    let response = session
        .request(
            "textDocument/declaration",
            declaration,
            operation_context(15, 15_000),
        )
        .expect("declaration response");
    let declarations = project_location_response(&response.result, 10);
    assert!(
        !declarations.is_empty(),
        "go-to-declaration on the `add` call should resolve a location",
    );

    let implementation = implementation_request(105, &document, call_pos)
        .params
        .expect("implementation params");
    let response = session
        .request(
            "textDocument/implementation",
            implementation,
            operation_context(16, 15_000),
        )
        .expect("implementation response");
    // `implementation` on a free function is legitimately empty, so this
    // surface is only smoke-exercised (the request must round-trip without
    // error); we deliberately do not assert on its contents.
    let _locations = project_location_response(&response.result, 10);

    let type_definition = type_definition_request(106, &document, call_pos)
        .params
        .expect("type-definition params");
    let response = session
        .request(
            "textDocument/typeDefinition",
            type_definition,
            operation_context(17, 15_000),
        )
        .expect("type-definition response");
    // `typeDefinition` at a call expression is environment-dependent and may
    // be empty, so this surface is only smoke-exercised.
    let _locations = project_location_response(&response.result, 10);

    let signature_help = signature_help_request(107, &document, call_pos)
        .params
        .expect("signature help params");
    let response = session
        .request(
            "textDocument/signatureHelp",
            signature_help,
            operation_context(18, 15_000),
        )
        .expect("signature help response");
    let signatures = response
        .result
        .get("signatures")
        .and_then(|value| value.as_array())
        .expect("signature help should carry a `signatures` array");
    assert!(
        !signatures.is_empty(),
        "signature help inside the `add(..)` call should offer at least one signature",
    );

    let response = session
        .request(
            "textDocument/documentSymbol",
            document_symbol_request(108, &document)
                .params
                .expect("document symbol params"),
            operation_context(19, 15_000),
        )
        .expect("document symbol response");
    let outline = project_document_symbol_response(&response.result, 20);
    assert!(
        outline.iter().any(|symbol| symbol.label.contains("add")),
        "document outline should include the `add` function, got {outline:?}",
    );

    let response = session
        .request(
            "workspace/symbol",
            workspace_symbol_request(109, "add")
                .params
                .expect("workspace symbol params"),
            operation_context(20, 15_000),
        )
        .expect("workspace symbol response");
    let workspace_symbols = project_workspace_symbol_response(&response.result, 20);
    assert!(
        workspace_symbols
            .iter()
            .any(|symbol| symbol.label.contains("add")),
        "workspace symbol search for `add` should match a symbol, got {workspace_symbols:?}",
    );

    let response = session
        .request(
            "textDocument/inlayHint",
            inlay_hint_request(
                110,
                &document,
                legion_protocol::Utf16Range {
                    start: Utf16Position {
                        line: 0,
                        character: 0,
                    },
                    end: Utf16Position {
                        line: 40,
                        character: 0,
                    },
                },
            )
            .params
            .expect("inlay hint params"),
            operation_context(21, 15_000),
        )
        .expect("inlay hint response");
    // Inlay hints depend on rust-analyzer's `inlayHints` config, which may be
    // disabled by default; this surface is only smoke-exercised.
    let _hints = project_inlay_hint_response(&response.result, "rust-analyzer", 20);

    let response = session
        .request(
            "textDocument/codeLens",
            code_lens_request(111, &document)
                .params
                .expect("code lens params"),
            operation_context(22, 15_000),
        )
        .expect("code lens response");
    // Code lenses (Run/Debug) depend on rust-analyzer's `lens` config, which
    // may be disabled by default; this surface is only smoke-exercised.
    let _lenses = project_code_lens_response(&response.result, "rust-analyzer", 20);

    let response = session
        .request(
            "textDocument/foldingRange",
            folding_range_request(112, &document)
                .params
                .expect("folding range params"),
            operation_context(23, 15_000),
        )
        .expect("folding range response");
    let folding_ranges = response
        .result
        .as_array()
        .expect("folding range response should be an array");
    assert!(
        !folding_ranges.is_empty(),
        "folding ranges should be reported for the multi-line fixture",
    );

    let response = session
        .request(
            "textDocument/semanticTokens/full",
            semantic_tokens_full_request(113, &document)
                .params
                .expect("semantic tokens params"),
            operation_context(24, 15_000),
        )
        .expect("semantic tokens response");
    let semantic_tokens = response
        .result
        .get("data")
        .and_then(|data| data.as_array())
        .expect("semantic tokens response should carry a `data` array");
    assert!(
        !semantic_tokens.is_empty(),
        "semantic tokens should be produced for the non-trivial fixture",
    );
}
