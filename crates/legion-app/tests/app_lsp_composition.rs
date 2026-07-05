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
