//! Integration test: real read requests + stale-snapshot rejection (WS-LANG-01 LANG.07/08).
//!
//! Drives the mock LSP server through `RustAnalyzerSession::launch`,
//! `initialize`, and `request_read`, asserting that:
//!   1. The returned `LspReadOutcome.result` is well-formed JSON.
//!   2. The `is_stale_response` gate correctly identifies stale vs. fresh
//!      responses based on the `issued_snapshot` in the outcome.
//!
//! Requires the mock binary to be built first:
//!   cargo build -p legion-lsp --bin mock_lsp_server

use legion_app::language::{
    RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession, is_stale_response,
};
use legion_protocol::{LanguageId, LanguageServerId, SnapshotId};

mod lsp_mock;

#[test]
fn completion_request_returns_well_formed_result() {
    let mock_path = lsp_mock::mock_server_path()
        .expect("mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`");

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
        .request_read("textDocument/completion", params)
        .expect("request_read should succeed against the mock server");

    // The result must be valid JSON (object, array, or null — all valid LSP completion responses).
    assert!(
        outcome.result.is_object() || outcome.result.is_array() || outcome.result.is_null(),
        "result should be a JSON object, array, or null; got: {:?}",
        outcome.result
    );
}

#[test]
fn stale_snapshot_gate_fires_on_real_read_path() {
    let mock_path = lsp_mock::mock_server_path()
        .expect("mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`");

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
        .request_read("textDocument/hover", params)
        .expect("request_read should succeed against the mock server");

    // A different snapshot simulates the buffer having advanced — the response is stale.
    let some_different_snapshot = SnapshotId(outcome.issued_snapshot.0.wrapping_add(1));
    assert!(
        is_stale_response(outcome.issued_snapshot, some_different_snapshot),
        "response issued against {:?} should be stale when buffer is at {:?}",
        outcome.issued_snapshot,
        some_different_snapshot
    );

    // The same snapshot means the response is fresh — safe to ingest.
    assert!(
        !is_stale_response(outcome.issued_snapshot, outcome.issued_snapshot),
        "response issued against {:?} should be fresh when buffer snapshot matches",
        outcome.issued_snapshot
    );
}
