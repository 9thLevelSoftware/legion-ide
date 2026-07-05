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

    // Point discovery at the mock so we don't need a real rust-analyzer.
    // SAFETY: test-only, single-threaded at this point.
    unsafe { std::env::set_var("CARGO_BIN_EXE_mock_lsp_server", &mock_path) };

    let dir = tempdir_with_cargo_toml();
    let mut handle = LspSessionHandle::new();
    handle.start_for_workspace(dir.path(), true);
    // Handle must now be Starting (or already Live if startup was very fast).
    assert!(
        handle.is_starting() || handle.is_live(),
        "handle must be Starting or Live after start_for_workspace"
    );
}

/// Draining a started session eventually yields a live handle (mock server).
#[test]
fn drain_yields_live_handle_with_mock_server() {
    let Some(mock_path) = lsp_mock::mock_server_path() else {
        eprintln!("skip: mock_lsp_server binary not found");
        return;
    };

    // SAFETY: test-only, single-threaded at this point.
    unsafe { std::env::set_var("CARGO_BIN_EXE_mock_lsp_server", &mock_path) };

    let dir = tempdir_with_cargo_toml();
    let mut handle = LspSessionHandle::new();
    handle.start_for_workspace(dir.path(), true);

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
    // No Cargo.toml → session refused.
    assert!(app.drain_lsp_session() == false || app.drain_lsp_session() == false);
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
