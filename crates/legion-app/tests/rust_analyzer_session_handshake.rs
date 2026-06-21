//! Integration test: RustAnalyzerSession launch + handshake → health record (WS-LANG-01 LANG.03/04).
//!
//! Drives the mock LSP server through `RustAnalyzerSession::launch` and
//! `initialize`, then asserts that the health record reflects the correct
//! binary provenance and init status.
//!
//! The mock binary (`mock_lsp_server`) lives in `crates/legion-lsp`.
//! It must be built before running this test in isolation:
//!   cargo build -p legion-lsp --bin mock_lsp_server
//!
//! When running `cargo test --workspace --all-targets` the workspace gate
//! builds all binaries, including the mock, so the test runs in full.

use legion_app::language::{RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession};
use legion_protocol::{LanguageId, LanguageServerId, LspResultStatus, LspServerBinaryProvenance};

mod lsp_mock;

#[test]
fn launch_and_initialize_populates_health_record() {
    let mock_path = lsp_mock::mock_server_path()
        .expect("mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`, \
                 or run under `cargo test --workspace --all-targets` which builds it");

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

    let health = session.health();
    assert_eq!(
        health.binary_provenance,
        LspServerBinaryProvenance::Configured,
        "binary_provenance should be Configured since we used configured_path"
    );
    assert_eq!(
        health.init_status,
        LspResultStatus::Fresh,
        "init_status should be Fresh after a successful initialize handshake"
    );
    assert_eq!(
        health.restart_count, 0,
        "restart_count should be 0 for a fresh session"
    );
}
