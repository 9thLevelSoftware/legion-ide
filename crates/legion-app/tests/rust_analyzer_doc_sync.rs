//! Integration test: document sync + diagnostics pump (WS-LANG-01 LANG.05/06).
//!
//! Drives the mock LSP server through `RustAnalyzerSession::launch`,
//! `initialize`, `did_open`, and `pump_diagnostics`, asserting that at least
//! one `publishDiagnostics` notification is collected for the correct URI.
//!
//! The mock server emits diagnostics for `file:///workspace/src/main.rs`
//! (hard-coded in `src/bin/mock_lsp_server.rs`).  Tests must query that URI to
//! observe results; querying a different URI returns an empty vec.
//!
//! Requires the mock binary to be built first:
//!   cargo build -p legion-lsp --bin mock_lsp_server

use std::time::Duration;

use legion_app::language::{
    LanguageSessionError, RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession,
};
use legion_protocol::{LanguageId, LanguageServerId};

mod lsp_mock;

/// The URI the mock `MOCK_LSP_EMIT_DIAGNOSTICS=1` server always emits for.
const MOCK_DIAG_URI: &str = "file:///workspace/src/main.rs";

#[test]
fn did_open_then_pump_collects_diagnostics() {
    let mock_path = lsp_mock::mock_server_path().expect(
        "mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`, \
                 or run under `cargo test --workspace --all-targets` which builds it",
    );

    let config = RustAnalyzerLaunchConfig {
        discovery: RustAnalyzerDiscovery {
            configured_path: Some(mock_path),
            ..Default::default()
        },
        supervisor: lsp_mock::mock_supervisor_config_with_diagnostics(),
        server_id: LanguageServerId(7),
        language_id: LanguageId("rust".to_string()),
    };
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = RustAnalyzerSession::launch(config, &mut launcher)
        .expect("RustAnalyzerSession::launch should succeed");
    session
        .initialize("file:///workspace")
        .expect("RustAnalyzerSession::initialize should succeed");

    session
        .did_open(MOCK_DIAG_URI, "rust", 1, "fn main() {}")
        .expect("did_open should succeed");

    // Query for the URI the mock actually emits — should find the notification.
    let diags = session.pump_diagnostics(MOCK_DIAG_URI, Duration::from_secs(5));
    assert!(
        !diags.is_empty(),
        "mock emits one publishDiagnostics for {MOCK_DIAG_URI}"
    );
}

/// Regression for Finding 2: `pump_diagnostics` previously ignored the `uri`
/// parameter and short-circuited on any buffered diagnostics, returning
/// unrelated files' diagnostics as if they belonged to the requested URI.
///
/// This test buffers a diagnostic for file A (MOCK_DIAG_URI / main.rs) and
/// then requests diagnostics for file B (other.rs). The result must be empty:
/// file A's diagnostics must NOT be returned as file B's.
#[test]
fn pump_diagnostics_does_not_return_wrong_uri_diagnostics() {
    let mock_path = lsp_mock::mock_server_path().expect(
        "mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`, \
                 or run under `cargo test --workspace --all-targets` which builds it",
    );

    let config = RustAnalyzerLaunchConfig {
        discovery: RustAnalyzerDiscovery {
            configured_path: Some(mock_path),
            ..Default::default()
        },
        // Mock emits diagnostics for MOCK_DIAG_URI (main.rs) at startup.
        supervisor: lsp_mock::mock_supervisor_config_with_diagnostics(),
        server_id: LanguageServerId(7),
        language_id: LanguageId("rust".to_string()),
    };
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = RustAnalyzerSession::launch(config, &mut launcher)
        .expect("RustAnalyzerSession::launch should succeed");
    session
        .initialize("file:///workspace")
        .expect("RustAnalyzerSession::initialize should succeed");

    // File B: a different URI the mock never emits diagnostics for.
    let file_b = "file:///workspace/src/other.rs";
    assert_ne!(file_b, MOCK_DIAG_URI, "test precondition: URIs must differ");

    // Use a short timeout: the mock will never emit diagnostics for file_b, so
    // we don't want to wait long.
    let diags = session.pump_diagnostics(file_b, Duration::from_millis(300));
    assert!(
        diags.is_empty(),
        "pump_diagnostics must not return file A's ({MOCK_DIAG_URI}) diagnostics \
         when asked for file B's ({file_b}); got {} item(s)",
        diags.len()
    );
}

/// Verify that `did_open` returns `Unavailable` when the session health is not Fresh.
#[test]
fn did_open_while_unavailable_returns_typed_error() {
    let mock_path = lsp_mock::mock_server_path().expect(
        "mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`",
    );

    let config = RustAnalyzerLaunchConfig {
        discovery: RustAnalyzerDiscovery {
            configured_path: Some(mock_path),
            ..Default::default()
        },
        supervisor: lsp_mock::mock_supervisor_config(),
        server_id: LanguageServerId(7),
        language_id: LanguageId("rust".to_string()),
    };
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = RustAnalyzerSession::launch(config, &mut launcher)
        .expect("RustAnalyzerSession::launch should succeed");
    session
        .initialize("file:///workspace")
        .expect("RustAnalyzerSession::initialize should succeed");

    // Simulate a crash so init_status becomes Unavailable (Finding 3 fix).
    let policy = legion_app::language::RestartPolicy {
        max_restarts: 2,
        backoff_base_ms: 10,
    };
    session.note_crash_and_should_restart(&policy);
    assert_eq!(
        session.health().init_status,
        legion_protocol::LspResultStatus::Unavailable,
        "init_status must be Unavailable after crash"
    );

    // did_open must not write to the transport; it must return Unavailable.
    let err = session
        .did_open(MOCK_DIAG_URI, "rust", 1, "fn main() {}")
        .expect_err("did_open should fail while session is Unavailable");
    assert!(
        matches!(err, LanguageSessionError::Unavailable),
        "expected LanguageSessionError::Unavailable, got: {err}"
    );
}
