//! Integration test: document sync + diagnostics pump (WS-LANG-01 LANG.05/06).
//!
//! Drives the mock LSP server through `RustAnalyzerSession::launch`,
//! `initialize`, `did_open`, and `pump_diagnostics`, asserting that at least
//! one `publishDiagnostics` notification is collected.
//!
//! Requires the mock binary to be built first:
//!   cargo build -p legion-lsp --bin mock_lsp_server

use std::time::Duration;

use legion_app::language::{RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession};
use legion_protocol::{LanguageId, LanguageServerId};

mod lsp_mock;

#[test]
fn did_open_then_pump_collects_diagnostics() {
    let mock_path = lsp_mock::mock_server_path()
        .expect("mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`, \
                 or run under `cargo test --workspace --all-targets` which builds it");

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
        .did_open("file:///workspace/src/lib.rs", "rust", 1, "fn main() {}")
        .expect("did_open should succeed");

    let diags = session.pump_diagnostics(
        "file:///workspace/src/lib.rs",
        Duration::from_secs(5),
    );
    assert!(!diags.is_empty(), "mock emits one publishDiagnostics");
}
