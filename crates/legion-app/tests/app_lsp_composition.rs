//! TDD tests for PKT-LSP-B Task 1: product session composition.
//!
//! Tests the `LspSessionHandle` background startup lifecycle and the
//! `AppComposition` drain/health wiring.

use std::time::Duration;

use legion_app::language::LspSessionHandle;

mod lsp_mock;

// ─── Task 1: LspSessionHandle lifecycle ─────────────────────────────────────

/// An idle handle with no workspace has no health record.
#[test]
fn idle_handle_has_no_health_record() {
    let handle = LspSessionHandle::new();
    assert!(handle.is_idle());
    assert!(handle.health_record().is_none());
}

/// Starting for an untrusted workspace refuses immediately.
#[test]
fn untrusted_workspace_refuses_without_spawning() {
    let mut handle = LspSessionHandle::new();
    let dir = tempdir_with_cargo_toml();
    handle.start_for_workspace(dir.path(), false);
    assert!(handle.is_refused_or_failed(), "untrusted must refuse");
    assert!(!handle.is_starting());
    let health = handle.health_record();
    assert!(
        health.is_some(),
        "refused handle must produce health record"
    );
    assert_eq!(
        health.unwrap().init_status,
        legion_protocol::LspResultStatus::Unavailable
    );
}

/// Starting for a workspace with no Cargo.toml refuses immediately.
#[test]
fn no_cargo_toml_refuses_without_spawning() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut handle = LspSessionHandle::new();
    handle.start_for_workspace(dir.path(), true);
    assert!(handle.is_refused_or_failed(), "no Cargo.toml must refuse");
    let reason = handle.failure_reason().unwrap_or("");
    assert!(
        reason.contains("Cargo.toml"),
        "failure reason must mention Cargo.toml; got: {reason:?}"
    );
}

/// A second `start_for_workspace` call on a non-idle handle is a no-op.
#[test]
fn second_start_is_noop_when_already_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut handle = LspSessionHandle::new();
    handle.start_for_workspace(dir.path(), true); // refuses (no Cargo.toml)
    assert!(handle.is_refused_or_failed());
    handle.start_for_workspace(dir.path(), true); // should not change state
    assert!(handle.is_refused_or_failed());
}

/// `drain()` on an idle handle returns false and does not change state.
#[test]
fn drain_on_idle_returns_false() {
    let mut handle = LspSessionHandle::new();
    assert!(!handle.drain());
    assert!(handle.is_idle());
}

/// `drain()` on a refused handle returns false.
#[test]
fn drain_on_refused_returns_false() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut handle = LspSessionHandle::new();
    handle.start_for_workspace(dir.path(), false); // refuses (untrusted)
    assert!(!handle.drain(), "drain on Refused should return false");
}

/// Starting for a trusted workspace with Cargo.toml transitions to Starting.
#[test]
fn trusted_rust_workspace_transitions_to_starting() {
    let Some(mock_path) = lsp_mock::mock_server_path() else {
        eprintln!("skip: mock_lsp_server binary not found");
        return;
    };

    let dir = tempdir_with_cargo_toml();
    let mut handle = LspSessionHandle::new();
    // Pass mock path explicitly so no process-global env mutation is needed.
    handle.start_for_workspace_with_server_path(dir.path(), true, Some(mock_path));
    // Handle must now be Starting (or already Live if startup was very fast).
    assert!(
        handle.is_starting() || handle.is_live(),
        "handle must be Starting or Live after start_for_workspace_with_server_path"
    );
}

/// Draining a started session eventually yields a live handle (mock server).
#[test]
fn drain_yields_live_handle_with_mock_server() {
    let Some(mock_path) = lsp_mock::mock_server_path() else {
        eprintln!("skip: mock_lsp_server binary not found");
        return;
    };

    let dir = tempdir_with_cargo_toml();
    let mut handle = LspSessionHandle::new();
    // Pass mock path explicitly — no unsafe env mutation needed.
    handle.start_for_workspace_with_server_path(dir.path(), true, Some(mock_path));

    // Poll until the result arrives (or timeout).
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !handle.is_live() && !handle.is_refused_or_failed() {
        handle.drain();
        if std::time::Instant::now() > deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(
        handle.is_live(),
        "handle must be Live after successful startup; failure_reason={:?}",
        handle.failure_reason()
    );

    let health = handle
        .health_record()
        .expect("live handle must have health record");
    assert_eq!(
        health.init_status,
        legion_protocol::LspResultStatus::Fresh,
        "live session health must be Fresh"
    );
}

// ─── Task 5 (D2): AppComposition health wiring ──────────────────────────────

/// AppComposition.drain_lsp_session() is a no-op when idle.
#[test]
fn app_composition_drain_lsp_noop_when_idle() {
    let mut app = legion_app::AppComposition::new();
    // drain must not panic and must return false (no state change).
    let changed = app.drain_lsp_session();
    assert!(!changed);
}

/// When the session is refused, the shell_projection_snapshot health flow is
/// observable via `lsp_server_health_record()`.
#[test]
fn app_composition_refused_lsp_projects_health_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        dir.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open_workspace");
    // No Cargo.toml → session refused immediately; drain returns false (no state change).
    assert!(!app.drain_lsp_session());
    // Health record should reflect refused state (Unavailable or None).
    let health = app.lsp_server_health_record();
    // Either None (refused projects None for Idle) or Unavailable.
    if let Some(record) = health {
        assert_eq!(
            record.init_status,
            legion_protocol::LspResultStatus::Unavailable
        );
    }
}

// ─── Task 4 (D3): ProblemsProjection — app-level projection test ─────────────

/// LSP diagnostics ingested via `ingest_lsp_publish_diagnostics_for_buffer`
/// appear in `language_tooling_projection.problems` and carry the buffer's
/// canonical path (so `render_problem_rows` can render clickable rows).
#[test]
fn lsp_diagnostics_appear_in_problems_projection() {
    let root = tempfile::tempdir().expect("tempdir");
    let src_file = root.path().join("main.rs");
    std::fs::write(&src_file, "fn main() {}\n").expect("write src file");

    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        root.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(src_file.to_string_lossy())
        .expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    // Synthetic publishDiagnostics params (mirrors the LSP spec shape).
    let uri = format!("file:///{}", src_file.to_string_lossy().replace('\\', "/"));
    let params = serde_json::json!({
        "uri": uri,
        "diagnostics": [{
            "range": {
                "start": {"line": 0, "character": 0},
                "end": {"line": 0, "character": 4}
            },
            "severity": 1,
            "code": "E0001",
            "source": "test-lsp",
            "message": "test diagnostic"
        }]
    });

    // Ingest through the redaction layer (same path used in production drain).
    let projection = app
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &params, true, None)
        .expect("ingest diagnostics");

    // The problem must appear in the projection.
    assert!(
        !projection.problems.is_empty(),
        "LSP diagnostics must appear in the LanguageToolingProjection"
    );
    let problem = &projection.problems[0];
    // Severity must be preserved.
    assert_eq!(
        problem.severity,
        legion_protocol::ProtocolDiagnosticSeverity::Error,
        "severity must round-trip"
    );
    // Canonical path must be set so the problems panel can render a clickable row.
    assert!(
        problem.path.is_some(),
        "problem path must be set for clickable panel navigation; got: {problem:?}"
    );

    // The problem must also appear in the shell projection snapshot.
    let snapshot = app
        .shell_projection_snapshot("test")
        .expect("shell projection");
    assert!(
        !snapshot.language_tooling_projection.problems.is_empty(),
        "problems must be visible in the shell projection snapshot"
    );
}

/// A `publishDiagnostics` batch with an empty diagnostics array clears the
/// problems for that file.
#[test]
fn lsp_empty_diagnostics_clears_problems() {
    let root = tempfile::tempdir().expect("tempdir");
    let src_file = root.path().join("lib.rs");
    std::fs::write(&src_file, "pub fn foo() {}\n").expect("write src file");

    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        root.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(src_file.to_string_lossy())
        .expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    let uri = format!("file:///{}", src_file.to_string_lossy().replace('\\', "/"));

    // First: add an error.
    let add_params = serde_json::json!({
        "uri": uri,
        "diagnostics": [{ "severity": 1, "message": "initial error", "range": { "start": {"line":0,"character":0}, "end": {"line":0,"character":1} } }]
    });
    let p1 = app
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &add_params, false, None)
        .expect("ingest add");
    assert!(!p1.problems.is_empty(), "error must be present after add");

    // Second: clear diagnostics (empty array).
    let clear_params = serde_json::json!({ "uri": uri, "diagnostics": [] });
    let p2 = app
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &clear_params, false, None)
        .expect("ingest clear");
    // After clearing, only non-LSP problems (e.g. legion-index) should remain.
    let lsp_problems = p2
        .problems
        .iter()
        .filter(|p| p.source_label.as_deref() != Some("legion-index"))
        .count();
    assert_eq!(
        lsp_problems, 0,
        "LSP problems must be cleared after empty diagnostics batch"
    );
}

// ─── Task 3 (I3): ProblemsProjection full add-then-clear cycle ──────────────

/// Verifies the full ProblemsProjection cycle at the app level:
/// ingest a diagnostic → projection non-empty → ingest clear → projection empty.
/// This is the T3 (doc-sync) strengthening required by the fix-round review.
#[test]
fn t3_diagnostics_projection_add_then_clear_cycle() {
    let root = tempfile::tempdir().expect("tempdir");
    let src_file = root.path().join("cycle.rs");
    std::fs::write(&src_file, "fn f() {}\n").expect("write");

    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        root.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(src_file.to_string_lossy())
        .expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    let uri = format!("file:///{}", src_file.to_string_lossy().replace('\\', "/"));

    // Phase 1: inject diagnostic → projection and snapshot must show it.
    let add_params = serde_json::json!({
        "uri": uri,
        "diagnostics": [{
            "range": { "start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1} },
            "severity": 1,
            "message": "cycle-test error"
        }]
    });
    let p1 = app
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &add_params, false, None)
        .expect("ingest add");
    assert!(
        !p1.problems.is_empty(),
        "ProblemsProjection must be non-empty after publishDiagnostics"
    );

    // Snapshot must also reflect the diagnostic.
    let snap1 = app
        .shell_projection_snapshot("test")
        .expect("shell snapshot after add");
    assert!(
        !snap1.language_tooling_projection.problems.is_empty(),
        "shell snapshot must contain the diagnostic after ingest"
    );

    // Phase 2: clear diagnostics → projection and snapshot must be empty.
    let clear_params = serde_json::json!({ "uri": uri, "diagnostics": [] });
    let p2 = app
        .ingest_lsp_publish_diagnostics_for_buffer(buffer_id, &clear_params, false, None)
        .expect("ingest clear");
    let lsp_count = p2
        .problems
        .iter()
        .filter(|p| p.source_label.as_deref() != Some("legion-index"))
        .count();
    assert_eq!(
        lsp_count, 0,
        "ProblemsProjection must be empty after clearing publishDiagnostics"
    );

    let snap2 = app
        .shell_projection_snapshot("test")
        .expect("shell snapshot after clear");
    let snap2_lsp_count = snap2
        .language_tooling_projection
        .problems
        .iter()
        .filter(|p| p.source_label.as_deref() != Some("legion-index"))
        .count();
    assert_eq!(
        snap2_lsp_count, 0,
        "shell snapshot must contain no LSP problems after clear"
    );
}

// ─── Task 5 (I4): lsp_health_records via snapshot path ───────────────────────

/// T5 (a): A refused/unavailable session projects an Unavailable health record
/// through the real `shell_projection_snapshot` path.
#[test]
fn t5_refused_health_in_snapshot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        dir.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    // PKT-LSP-C T1: session start is now lazy (not triggered by open_workspace).
    // Trigger it explicitly; with no Cargo.toml the session refuses immediately.
    app.force_lsp_start_for_test();
    app.drain_lsp_session();

    let health = app
        .lsp_server_health_record()
        .expect("refused session must produce a health record");
    assert_eq!(
        health.init_status,
        legion_protocol::LspResultStatus::Unavailable,
        "refused session must project Unavailable status"
    );

    // Check through the real snapshot path (the path production UI uses).
    let snapshot = app
        .shell_projection_snapshot("test")
        .expect("snapshot must succeed");
    let health_records = &snapshot.language_tooling_projection.lsp_health_records;
    assert!(
        !health_records.is_empty(),
        "lsp_health_records must be non-empty for a refused session"
    );
    assert_eq!(
        health_records[0].init_status,
        legion_protocol::LspResultStatus::Unavailable,
        "snapshot health record must be Unavailable for refused session"
    );
}

/// T5 (b): An injected-live session projects a Fresh health record through
/// the real `shell_projection_snapshot` path (end-to-end snapshot population).
#[test]
fn t5_injected_live_health_in_snapshot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        dir.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open workspace");

    // Inject a live health record with Fresh status (no real server needed).
    let health = fresh_health_record();
    app.set_lsp_health_for_test(health);

    let snapshot = app
        .shell_projection_snapshot("test")
        .expect("snapshot must succeed");
    let health_records = &snapshot.language_tooling_projection.lsp_health_records;
    assert!(
        !health_records.is_empty(),
        "lsp_health_records must be non-empty for a live session"
    );
    assert_eq!(
        health_records[0].init_status,
        legion_protocol::LspResultStatus::Fresh,
        "snapshot health record must be Fresh for a live session"
    );
}

// ─── Task 7 (I2/T7): Capability gating ───────────────────────────────────────

/// When the server advertises no capabilities (or explicitly advertises false),
/// hover, definition, and completion requests must be silently skipped (return false).
#[test]
fn t7_capability_gated_requests_skip_when_unsupported() {
    let root = tempfile::tempdir().expect("tempdir");
    let src_file = root.path().join("main.rs");
    std::fs::write(&src_file, "fn main() {}\n").expect("write");

    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        root.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(src_file.to_string_lossy())
        .expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    // Inject a health record advertising all capabilities as NOT supported.
    let health = health_with_caps(&[
        ("hoverProvider", false),
        ("definitionProvider", false),
        ("completionProvider", false),
    ]);
    app.set_lsp_health_for_test(health);

    let pos = legion_protocol::TextCoordinate {
        line: 0,
        character: 0,
        byte_offset: None,
        utf16_offset: None,
    };
    assert!(
        !app.issue_lsp_hover_request(buffer_id, pos),
        "hover must be silently skipped when hoverProvider=false"
    );
    assert!(
        !app.issue_lsp_definition_request(buffer_id, pos),
        "definition must be silently skipped when definitionProvider=false"
    );
    assert!(
        !app.issue_lsp_completion_request(buffer_id, pos),
        "completion must be silently skipped when completionProvider=false"
    );
}

/// When the server advertises hover=true but definition=false, only hover fires.
#[test]
fn t7_capability_gated_partial_support() {
    let root = tempfile::tempdir().expect("tempdir");
    let src_file = root.path().join("main.rs");
    std::fs::write(&src_file, "fn main() {}\n").expect("write");

    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        root.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open workspace");
    app.open_file(src_file.to_string_lossy())
        .expect("open file");
    let buffer_id = app.active_buffer_id().expect("active buffer");

    // Only hover is supported; definition and completion are not.
    let health = health_with_caps(&[
        ("hoverProvider", true),
        ("definitionProvider", false),
        ("completionProvider", false),
    ]);
    app.set_lsp_health_for_test(health);

    let pos = legion_protocol::TextCoordinate {
        line: 0,
        character: 0,
        byte_offset: None,
        utf16_offset: None,
    };
    // hover returns true because it successfully issues the request.
    assert!(
        app.issue_lsp_hover_request(buffer_id, pos),
        "hover must fire when hoverProvider=true"
    );
    assert!(
        !app.issue_lsp_definition_request(buffer_id, pos),
        "definition must skip when definitionProvider=false"
    );
    assert!(
        !app.issue_lsp_completion_request(buffer_id, pos),
        "completion must skip when completionProvider=false"
    );
}

// ─── helpers ────────────────────────────────────────────────────────────────

fn tempdir_with_cargo_toml() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\n",
    )
    .expect("write Cargo.toml");
    dir
}

/// Build a `LspServerHealthRecord` with the given named capability flags.
fn health_with_caps(caps: &[(&str, bool)]) -> legion_protocol::LspServerHealthRecord {
    let capabilities = caps
        .iter()
        .map(|(name, supported)| legion_protocol::LspCapabilitySummary {
            capability: name.to_string(),
            supported: *supported,
            dynamic_registration: false,
            option_hash: None,
            redaction_hints: Vec::new(),
            schema_version: 1,
        })
        .collect();
    legion_protocol::LspServerHealthRecord {
        server_id: legion_protocol::LanguageServerId(1),
        language_id: legion_protocol::LanguageId("rust".to_string()),
        binary_provenance: legion_protocol::LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: legion_protocol::LspResultStatus::Fresh,
        capabilities,
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: 1,
    }
}

/// Build a `LspServerHealthRecord` with Fresh status and empty capabilities list.
fn fresh_health_record() -> legion_protocol::LspServerHealthRecord {
    legion_protocol::LspServerHealthRecord {
        server_id: legion_protocol::LanguageServerId(1),
        language_id: legion_protocol::LanguageId("rust".to_string()),
        binary_provenance: legion_protocol::LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: legion_protocol::LspResultStatus::Fresh,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: 1,
    }
}
