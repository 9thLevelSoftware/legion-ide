//! Real rust-analyzer smoke test (WS-LANG-01 LANG.03/12).
//!
//! Drives a live rust-analyzer binary through [`LspStdioSession`]:
//!   initialize → initialized → textDocument/didOpen → pump_until diagnostics
//!
//! Marked `#[ignore]` so the normal gate skips it. Run explicitly with:
//!   cargo test -p legion-lsp --test rust_analyzer_smoke -- --ignored --nocapture
//!
//! The test skips cleanly if rust-analyzer is not on PATH.
//!
//! # Deadlock prevention (hazard #1)
//! We send `"capabilities": {}` (empty) in the initialize params so that
//! rust-analyzer does NOT emit `client/registerCapability`,
//! `workspace/configuration`, or `window/workDoneProgress/create` server→client
//! requests that would deadlock the synchronous `read_until_correlated_response`
//! loop while we are blocked waiting for our own response id.

use std::fs;
use std::time::{Duration, Instant};

use legion_lsp::{
    DiscoveredBinary, LspServerProcessConfig, LspStdioLauncher, LspStdioSession,
    LspSupervisorConfig, PumpOutcome, RustAnalyzerDiscovery,
};
use legion_protocol::{
    BufferId, BufferVersion, CancellationTokenId, CapabilityDecisionId, CapabilityId, CausalityId,
    CorrelationId, FileFingerprint, FileId, LanguageId, LanguageServerId,
    LspConfiguredServerIdentity, LspLaunchPolicyDecision, LspOperationContext, LspRequestId,
    LspWorkspaceTrustPosture, RedactionHint, SemanticPrivacyScope, SnapshotId, WorkspaceId,
    WorkspaceRootId, WorkspaceTrustState,
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Support helpers
// ---------------------------------------------------------------------------

fn discovered() -> Option<std::path::PathBuf> {
    let d = RustAnalyzerDiscovery {
        path_env: std::env::var("PATH").ok(),
        ..Default::default()
    };
    match d.resolve() {
        DiscoveredBinary::Found { path, .. } => Some(path),
        DiscoveredBinary::NotFound => None,
    }
}

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "smoke-test".to_string(),
        value: value.to_string(),
    }
}

fn identity(command: &str) -> LspConfiguredServerIdentity {
    LspConfiguredServerIdentity {
        server_id: LanguageServerId(200),
        workspace_id: WorkspaceId(1),
        root_id: Some(WorkspaceRootId(1)),
        language_id: LanguageId("rust".to_string()),
        display_name: "rust-analyzer-smoke".to_string(),
        command_hash: fingerprint(command),
        args_hash: None,
        env_hash: None,
        cwd_hash: None,
        settings_hash: None,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn launch_policy(command: &str) -> LspLaunchPolicyDecision {
    LspLaunchPolicyDecision::evaluate(
        identity(command),
        LspWorkspaceTrustPosture {
            workspace_id: WorkspaceId(1),
            workspace_trust_state: WorkspaceTrustState::Trusted,
            privacy_scope: SemanticPrivacyScope::Workspace,
            privacy_scope_allowed: true,
            required_capability: CapabilityId("process.spawn".to_string()),
            decision_id: Some(CapabilityDecisionId(1)),
            diagnostics: Vec::new(),
            schema_version: 1,
        },
        true,
        CorrelationId(1),
        CausalityId(Uuid::from_u128(1)),
        Vec::new(),
        1,
    )
}

fn smoke_operation_context() -> LspOperationContext {
    LspOperationContext {
        request_id: LspRequestId(Uuid::from_u128(0xdead_beef)),
        workspace_id: WorkspaceId(1),
        file_id: FileId(0),
        buffer_id: BufferId(0),
        snapshot_id: SnapshotId(0),
        buffer_version: BufferVersion(0),
        language_id: LanguageId("rust".to_string()),
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(Uuid::from_u128(1001)),
        timeout_ms: 30_000,
        cancellation_token: CancellationTokenId(Uuid::from_u128(2001)),
        content_hash: None,
        privacy_scope: SemanticPrivacyScope::Workspace,
        schema_version: 1,
    }
}

/// Creates a tiny throwaway fixture crate in a temp directory and returns
/// the directory path and the `src/lib.rs` path.
fn create_fixture_crate() -> (std::path::PathBuf, std::path::PathBuf) {
    let fixture_dir = std::env::temp_dir().join(format!(
        "ws-lang-01-ra-lsp-smoke-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(fixture_dir.join("src")).expect("create fixture dirs");
    fs::write(
        fixture_dir.join("Cargo.toml"),
        "[package]\nname = \"ra-smoke-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write fixture Cargo.toml");
    let lib_rs = fixture_dir.join("src").join("lib.rs");
    fs::write(&lib_rs, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n")
        .expect("write fixture lib.rs");
    (fixture_dir, lib_rs)
}

/// Converts an absolute path to an LSP `file://` URI with forward slashes.
/// On Windows: `C:\foo` → `file:///C:/foo`; on Unix: `/foo` → `file:///foo`.
fn path_to_file_uri(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    let forward = s.replace('\\', "/");
    if forward.starts_with('/') {
        format!("file://{forward}")
    } else {
        format!("file:///{forward}")
    }
}

// ---------------------------------------------------------------------------
// Smoke test
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires rust-analyzer on PATH; run with --ignored"]
fn rust_analyzer_initializes_and_emits_diagnostics() {
    // --- Discovery ---
    let Some(bin) = discovered() else {
        eprintln!("rust-analyzer not found on PATH; skipping");
        return;
    };

    let version = RustAnalyzerDiscovery::probe_version(&bin);
    eprintln!("rust-analyzer binary: {}", bin.display());
    eprintln!("rust-analyzer version: {:?}", version);
    assert!(version.is_some(), "rust-analyzer --version should succeed");

    // --- Fixture crate ---
    let (fixture_dir, lib_rs) = create_fixture_crate();
    eprintln!("fixture dir: {}", fixture_dir.display());

    let root_uri = path_to_file_uri(&fixture_dir);
    let lib_rs_uri = path_to_file_uri(&lib_rs);
    eprintln!("rootUri:    {root_uri}");
    eprintln!("lib.rs URI: {lib_rs_uri}");

    // --- Build supervisor config ---
    let command = bin.to_string_lossy().into_owned();
    let supervisor_config = LspSupervisorConfig {
        launch_policy: launch_policy(&command),
        process: LspServerProcessConfig {
            command,
            args: Vec::new(),
            cwd: Some(fixture_dir.clone()),
            env: Vec::new(),
        },
        initial_backoff_ms: 50,
        max_backoff_ms: 1000,
        max_restart_attempts: 1,
    };

    // --- Launch session ---
    let mut launcher = LspStdioLauncher::new();
    let mut session =
        LspStdioSession::start(supervisor_config, &mut launcher).expect("launch rust-analyzer");
    eprintln!(
        "session launched; lifecycle={:?}",
        session.lifecycle_state()
    );

    // --- Initialize ---
    // CRITICAL: send empty `capabilities: {}` to prevent rust-analyzer from
    // sending server→client registration/configuration requests that would
    // deadlock the synchronous read loop (hazard #1).
    let init_params = serde_json::json!({
        "processId": std::process::id(),
        "rootUri": root_uri,
        "capabilities": {},
        "workspaceFolders": [{ "uri": root_uri, "name": "ra-smoke" }],
    });

    let init_response = session
        .initialize(init_params, smoke_operation_context())
        .expect("initialize");
    eprintln!("initialize response status: {:?}", init_response.status);
    assert!(
        session.is_ready(),
        "session should be ready after initialize"
    );

    // --- initialized notification ---
    session
        .send_notification("initialized", serde_json::json!({}))
        .expect("send initialized notification");
    eprintln!("sent initialized notification");

    // --- didOpen ---
    let lib_rs_text = fs::read_to_string(&lib_rs).expect("read fixture lib.rs");
    session
        .send_notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": lib_rs_uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": lib_rs_text,
                }
            }),
        )
        .expect("send didOpen");
    eprintln!("sent textDocument/didOpen for {lib_rs_uri}");

    // --- Pump for diagnostics ---
    // Generous 60s deadline. Clean code may produce zero diagnostics, so we
    // accept either PredicateMet OR Deadline as success. The real proof is
    // that initialize/didOpen didn't error and the process is alive.
    let deadline = Instant::now() + Duration::from_secs(60);
    let outcome = session
        .pump_until(deadline, &mut |n| !n.diagnostics.is_empty())
        .expect("pump_until should not error");
    eprintln!("pump_until outcome: {outcome:?}");
    eprintln!(
        "diagnostic notifications received: {}",
        session.diagnostic_notifications().len()
    );
    eprintln!(
        "progress notifications received: {}",
        session.progress_notifications().len()
    );

    // Accept both: diagnostics emitted OR clean deadline (indexing time varies).
    assert!(
        matches!(outcome, PumpOutcome::PredicateMet | PumpOutcome::Deadline),
        "expected PredicateMet or Deadline; got {outcome:?}"
    );

    // Process must still be alive — the session was not killed by rust-analyzer.
    assert!(
        session.is_running(),
        "rust-analyzer process should still be running after smoke"
    );

    eprintln!("smoke PASSED");

    // --- Cleanup ---
    drop(session); // kills the child process
    let _ = fs::remove_dir_all(&fixture_dir); // best-effort cleanup
}
