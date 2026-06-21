//! Real rust-analyzer end-to-end workflow smoke (WS-LANG-01 LANG.03/12).
//!
//! Drives a live rust-analyzer through [`RustAnalyzerSession`]:
//!   launch → initialize → did_open → pump_diagnostics →
//!   completion → hover → definition → references → formatting →
//!   rename → note_crash_and_should_restart
//!
//! Marked `#[ignore]` so the normal gate skips it. Run explicitly with:
//!   cargo test -p legion-app --test rust_analyzer_workflow -- --ignored --nocapture
//!
//! The test skips cleanly if rust-analyzer is not on PATH.
//!
//! # Deadlock prevention (hazard #1)
//! `RustAnalyzerSession::initialize` already sends `"capabilities": {}` (empty),
//! which prevents rust-analyzer from sending server→client registration/
//! configuration requests that would deadlock our synchronous read loop.
//!
//! # Rename note (hazard #4)
//! We issue `textDocument/rename` via `request_read` and assert the raw JSON
//! result is a well-formed WorkspaceEdit-shaped object. Converting it to a
//! `WorkspaceEditProposalPayload` (position→byte and uri→FileIdentity mapping)
//! is deliberately deferred and is covered by Task 8's unit tests.

use std::fs;
use std::time::Duration;

use legion_app::language::{
    DiscoveredBinary, RestartPolicy, RustAnalyzerDiscovery, RustAnalyzerLaunchConfig,
    RustAnalyzerSession,
};
use legion_lsp::{LspServerProcessConfig, LspSupervisorConfig};
use legion_protocol::{
    CapabilityDecisionId, CapabilityId, CausalityId, CorrelationId, FileFingerprint, LanguageId,
    LanguageServerId, LspConfiguredServerIdentity, LspLaunchPolicyDecision,
    LspWorkspaceTrustPosture, RedactionHint, SemanticPrivacyScope, WorkspaceId, WorkspaceRootId,
    WorkspaceTrustState,
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
    FileFingerprint { algorithm: "workflow-smoke".to_string(), value: value.to_string() }
}

fn identity(command: &str) -> LspConfiguredServerIdentity {
    LspConfiguredServerIdentity {
        server_id: LanguageServerId(201),
        workspace_id: WorkspaceId(2),
        root_id: Some(WorkspaceRootId(2)),
        language_id: LanguageId("rust".to_string()),
        display_name: "rust-analyzer-workflow-smoke".to_string(),
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
            workspace_id: WorkspaceId(2),
            workspace_trust_state: WorkspaceTrustState::Trusted,
            privacy_scope: SemanticPrivacyScope::Workspace,
            privacy_scope_allowed: true,
            required_capability: CapabilityId("process.spawn".to_string()),
            decision_id: Some(CapabilityDecisionId(2)),
            diagnostics: Vec::new(),
            schema_version: 1,
        },
        true,
        CorrelationId(2),
        CausalityId(Uuid::from_u128(2)),
        Vec::new(),
        1,
    )
}

/// Creates a tiny throwaway fixture crate and returns
/// (fixture_dir, src/lib.rs path, root_uri, lib_rs_uri).
fn create_fixture_crate() -> (
    std::path::PathBuf,
    std::path::PathBuf,
    String,
    String,
) {
    let fixture_dir = std::env::temp_dir().join(format!(
        "ws-lang-01-ra-app-smoke-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(fixture_dir.join("src")).expect("create fixture dirs");
    fs::write(
        fixture_dir.join("Cargo.toml"),
        "[package]\nname = \"ra-app-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write fixture Cargo.toml");
    // A simple library with a named function we can rename.
    let lib_rs = fixture_dir.join("src").join("lib.rs");
    fs::write(
        &lib_rs,
        "/// Adds two integers.\npub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .expect("write fixture lib.rs");

    let root_uri = path_to_file_uri(&fixture_dir);
    let lib_rs_uri = path_to_file_uri(&lib_rs);
    (fixture_dir, lib_rs, root_uri, lib_rs_uri)
}

/// Converts an absolute path to an LSP `file://` URI with forward slashes.
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
fn rust_analyzer_full_workflow() {
    // --- Discovery ---
    let Some(bin) = discovered() else {
        eprintln!("rust-analyzer not found on PATH; skipping");
        return;
    };

    let version = RustAnalyzerDiscovery::probe_version(&bin);
    eprintln!("rust-analyzer binary: {}", bin.display());
    eprintln!("rust-analyzer version: {:?}", version);

    // --- Fixture crate ---
    let (fixture_dir, _lib_rs, root_uri, lib_rs_uri) = create_fixture_crate();
    eprintln!("fixture dir: {}", fixture_dir.display());
    eprintln!("rootUri:    {root_uri}");
    eprintln!("lib.rs URI: {lib_rs_uri}");

    let command = bin.to_string_lossy().into_owned();

    // --- Launch ---
    let config = RustAnalyzerLaunchConfig {
        discovery: RustAnalyzerDiscovery {
            configured_path: Some(bin.clone()),
            ..Default::default()
        },
        supervisor: LspSupervisorConfig {
            launch_policy: launch_policy(&command),
            process: LspServerProcessConfig {
                command: command.clone(),
                args: Vec::new(),
                cwd: Some(fixture_dir.clone()),
                env: Vec::new(),
            },
            initial_backoff_ms: 50,
            max_backoff_ms: 1000,
            max_restart_attempts: 1,
        },
        server_id: LanguageServerId(201),
        language_id: LanguageId("rust".to_string()),
    };

    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = RustAnalyzerSession::launch(config, &mut launcher)
        .expect("RustAnalyzerSession::launch should succeed");
    eprintln!("session launched");

    // --- Initialize ---
    // RustAnalyzerSession::initialize already sends `capabilities: {}` (empty),
    // preventing server→client registration requests (hazard #1).
    session.initialize(&root_uri).expect("initialize should succeed");
    let health = session.health();
    eprintln!("health after initialize: status={:?}, restarts={}", health.init_status, health.restart_count);
    assert_eq!(
        health.init_status,
        legion_protocol::LspResultStatus::Fresh,
        "init_status should be Fresh"
    );

    // --- did_open ---
    let lib_src = "/// Adds two integers.\npub fn add(a: i32, b: i32) -> i32 { a + b }\n";
    session
        .did_open(&lib_rs_uri, "rust", 1, lib_src)
        .expect("did_open should succeed");
    eprintln!("sent did_open for {lib_rs_uri}");

    // --- pump_diagnostics ---
    // Generous 60s timeout. Clean code may yield zero diagnostics;
    // any of the following is acceptable: diagnostics received OR timeout.
    // The real proof is that initialize/didOpen completed without errors.
    let diags = session.pump_diagnostics(&lib_rs_uri, Duration::from_secs(60));
    eprintln!("pump_diagnostics returned {} notification(s)", diags.len());
    // We do NOT assert diags is non-empty: clean Rust code produces zero diagnostics.
    // The session staying alive is the proof.

    // --- completion ---
    // Request at position (1, 10) — inside the function body.
    let completion_params = serde_json::json!({
        "textDocument": { "uri": lib_rs_uri },
        "position": { "line": 1, "character": 10 },
    });
    let completion_outcome = session
        .request_read("textDocument/completion", completion_params)
        .expect("completion request_read should not error");
    eprintln!("completion result type: {}", json_type_name(&completion_outcome.result));
    // Accept object (CompletionList), array (flat list), or null (nothing at cursor).
    assert!(
        completion_outcome.result.is_object()
            || completion_outcome.result.is_array()
            || completion_outcome.result.is_null(),
        "completion result should be object/array/null; got: {:?}",
        completion_outcome.result
    );

    // --- hover ---
    let hover_params = serde_json::json!({
        "textDocument": { "uri": lib_rs_uri },
        "position": { "line": 1, "character": 7 },
    });
    let hover_outcome = session
        .request_read("textDocument/hover", hover_params)
        .expect("hover request_read should not error");
    eprintln!("hover result type: {}", json_type_name(&hover_outcome.result));
    assert!(
        hover_outcome.result.is_object() || hover_outcome.result.is_null(),
        "hover result should be object or null; got: {:?}",
        hover_outcome.result
    );

    // --- definition ---
    let definition_params = serde_json::json!({
        "textDocument": { "uri": lib_rs_uri },
        "position": { "line": 1, "character": 7 },
    });
    let definition_outcome = session
        .request_read("textDocument/definition", definition_params)
        .expect("definition request_read should not error");
    eprintln!("definition result type: {}", json_type_name(&definition_outcome.result));
    assert!(
        definition_outcome.result.is_array()
            || definition_outcome.result.is_object()
            || definition_outcome.result.is_null(),
        "definition result should be array/object/null; got: {:?}",
        definition_outcome.result
    );

    // --- references ---
    let references_params = serde_json::json!({
        "textDocument": { "uri": lib_rs_uri },
        "position": { "line": 1, "character": 7 },
        "context": { "includeDeclaration": true },
    });
    let references_outcome = session
        .request_read("textDocument/references", references_params)
        .expect("references request_read should not error");
    eprintln!("references result type: {}", json_type_name(&references_outcome.result));
    assert!(
        references_outcome.result.is_array() || references_outcome.result.is_null(),
        "references result should be array or null; got: {:?}",
        references_outcome.result
    );

    // --- formatting ---
    let formatting_params = serde_json::json!({
        "textDocument": { "uri": lib_rs_uri },
        "options": { "tabSize": 4, "insertSpaces": true },
    });
    let formatting_outcome = session
        .request_read("textDocument/formatting", formatting_params)
        .expect("formatting request_read should not error");
    eprintln!("formatting result type: {}", json_type_name(&formatting_outcome.result));
    assert!(
        formatting_outcome.result.is_array() || formatting_outcome.result.is_null(),
        "formatting result should be array or null; got: {:?}",
        formatting_outcome.result
    );

    // --- rename ---
    // Issue rename at the `add` function name position (line 1, char 7).
    // We assert the raw JSON result is a well-formed WorkspaceEdit-shaped object.
    //
    // NOTE: Converting the raw WorkspaceEdit JSON into a `WorkspaceEditProposalPayload`
    // (which requires position→byte and uri→FileIdentity mapping) is deliberately
    // deferred. Proposal routing from a STRUCTURED payload is covered by Task 8's
    // unit tests; full JSON translation is deferred to that task.
    let rename_params = serde_json::json!({
        "textDocument": { "uri": lib_rs_uri },
        "position": { "line": 1, "character": 7 },
        "newName": "add_renamed",
    });
    let rename_outcome = session
        .request_read("textDocument/rename", rename_params)
        .expect("rename request_read should not error");
    eprintln!("rename result type: {}", json_type_name(&rename_outcome.result));
    // A successful rename returns a WorkspaceEdit object; no changes → null is also valid.
    assert!(
        rename_outcome.result.is_object() || rename_outcome.result.is_null(),
        "rename result should be object or null; got: {:?}",
        rename_outcome.result
    );
    if rename_outcome.result.is_object() {
        // Verify it has the expected WorkspaceEdit shape (changes or documentChanges).
        let has_changes = rename_outcome.result.get("changes").is_some()
            || rename_outcome.result.get("documentChanges").is_some();
        eprintln!("rename WorkspaceEdit has_changes: {has_changes}");
        assert!(
            has_changes,
            "rename WorkspaceEdit should have 'changes' or 'documentChanges' field; got: {:?}",
            rename_outcome.result
        );
    }

    // --- note_crash_and_should_restart (hazard #5) ---
    // Exercise the restart policy/state machine. This does NOT kill the process;
    // it only advances the policy counters on the live session.
    let policy = RestartPolicy { max_restarts: 1, backoff_base_ms: 50 };
    let backoff = session.note_crash_and_should_restart(&policy);
    eprintln!("note_crash_and_should_restart backoff: {:?}", backoff);
    // With max_restarts=1 and 0 prior restarts, first call must return Some(backoff).
    assert!(backoff.is_some(), "first crash within budget should return a backoff");
    assert_eq!(
        session.health().restart_count,
        1,
        "restart_count should be 1 after first note_crash"
    );
    // Second call exhausts the budget (max_restarts=1, already at count=1).
    let backoff2 = session.note_crash_and_should_restart(&policy);
    assert!(backoff2.is_none(), "second crash should exhaust budget and return None");
    eprintln!("restart budget exhausted as expected");

    eprintln!("workflow smoke PASSED");

    // --- Cleanup ---
    drop(session); // kills the child process
    let _ = fs::remove_dir_all(&fixture_dir); // best-effort cleanup
}

fn json_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}
