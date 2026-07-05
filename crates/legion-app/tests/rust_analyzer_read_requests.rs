//! Integration test: real read requests + stale-snapshot rejection (WS-LANG-01 LANG.07/08).
//!
//! Drives the mock LSP server through `RustAnalyzerSession::launch`,
//! `initialize`, and `request_read`, asserting that:
//!   1. The returned `LspReadOutcome.result` is well-formed JSON.
//!   2. The `is_stale_response` gate correctly identifies stale vs. fresh
//!      responses based on the `issued_snapshot` in the outcome.
//!   3. `request_read` returns `LanguageSessionError::Unavailable` (no transport
//!      write) when the session health is not Fresh (Finding 4 regression).
//!   4. (D1 fix) The `issued_snapshot` in the outcome matches the snapshot
//!      passed to `request_read`, enabling the stale-response gate.
//!
//! Requires the mock binary to be built first:
//!   cargo build -p legion-lsp --bin mock_lsp_server

use legion_app::language::{
    LanguageSessionError, RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession,
    is_stale_response,
};
use legion_protocol::{LanguageId, LanguageServerId, LspResultStatus, SnapshotId};

mod lsp_mock;

#[test]
fn completion_request_returns_well_formed_result() {
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

    let params = serde_json::json!({
        "textDocument": { "uri": "file:///workspace/src/lib.rs" },
        "position": { "line": 0, "character": 0 }
    });
    let outcome = session
        .request_read("textDocument/completion", params, SnapshotId(0))
        .expect("request_read should succeed against the mock server");

    // The result must be valid JSON (object, array, or null — all valid LSP completion responses).
    assert!(
        outcome.result.is_object() || outcome.result.is_array() || outcome.result.is_null(),
        "result should be a JSON object, array, or null; got: {:?}",
        outcome.result
    );
}

/// D1 fix: the `issued_snapshot` in `LspReadOutcome` now equals the snapshot
/// passed to `request_read`, enabling reliable stale-response detection.
#[test]
fn request_read_issued_snapshot_matches_passed_snapshot() {
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

    let request_snapshot = SnapshotId(42);
    let params = serde_json::json!({
        "textDocument": { "uri": "file:///workspace/src/lib.rs" },
        "position": { "line": 0, "character": 0 }
    });
    let outcome = session
        .request_read("textDocument/completion", params, request_snapshot)
        .expect("request_read should succeed");

    assert_eq!(
        outcome.issued_snapshot, request_snapshot,
        "issued_snapshot must equal the snapshot passed to request_read (D1 fix)"
    );
}

/// When the buffer advances to snapshot N+1, a response issued at snapshot N
/// is stale and must be discarded by the `is_stale_response` gate.
#[test]
fn stale_snapshot_gate_exercises_issued_snapshot_from_real_read() {
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

    let issued = SnapshotId(7);
    let params = serde_json::json!({
        "textDocument": { "uri": "file:///workspace/src/lib.rs" },
        "position": { "line": 0, "character": 0 }
    });
    let outcome = session
        .request_read("textDocument/hover", params, issued)
        .expect("request_read should succeed against the mock server");

    // D1: issued_snapshot now equals the snapshot we passed.
    assert_eq!(outcome.issued_snapshot, issued);

    // A buffer advance simulates the stale-response scenario.
    let advanced = SnapshotId(outcome.issued_snapshot.0.wrapping_add(1));
    assert!(
        is_stale_response(outcome.issued_snapshot, advanced),
        "response issued against {:?} should be stale when buffer is at {:?}",
        outcome.issued_snapshot,
        advanced
    );

    // Same snapshot → fresh.
    assert!(
        !is_stale_response(outcome.issued_snapshot, outcome.issued_snapshot),
        "response issued against {:?} should be fresh when buffer snapshot matches",
        outcome.issued_snapshot
    );
}

/// Regression for Finding 4: `request_read` previously could write to the
/// transport even when the session was not in an initialized/live state
/// (e.g. after a crash noted during backoff). After the fix, `request_read`
/// must return `LanguageSessionError::Unavailable` without touching the
/// transport whenever `health.init_status != Fresh`.
#[test]
fn request_read_while_unavailable_returns_typed_error() {
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

    // Confirm session is live before the crash.
    assert_eq!(session.health().init_status, LspResultStatus::Fresh);

    // Simulate a crash — init_status becomes Unavailable (Finding 3 fix).
    let policy = legion_app::language::RestartPolicy {
        max_restarts: 2,
        backoff_base_ms: 10,
    };
    session.note_crash_and_should_restart(&policy);
    assert_eq!(
        session.health().init_status,
        LspResultStatus::Unavailable,
        "precondition: init_status must be Unavailable after crash"
    );

    // Attempt a read request — must return Unavailable, not panic, not write.
    let params = serde_json::json!({
        "textDocument": { "uri": "file:///workspace/src/lib.rs" },
        "position": { "line": 0, "character": 0 }
    });
    let err = session
        .request_read("textDocument/completion", params, SnapshotId(0))
        .expect_err("request_read must fail while session is Unavailable");

    assert!(
        matches!(err, LanguageSessionError::Unavailable),
        "expected LanguageSessionError::Unavailable, got: {err}"
    );
}
