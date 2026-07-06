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
    let mock_path = lsp_mock::mock_server_path().expect(
        "mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`, \
                 or run under `cargo test --workspace --all-targets` which builds it",
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

/// The mock server advertises `hoverProvider` and `definitionProvider` in its
/// initialize response; after initialize the health record must contain
/// capability summaries for those keys.
#[test]
fn initialize_populates_capability_summaries() {
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
    let mut session =
        RustAnalyzerSession::launch(config, &mut launcher).expect("launch should succeed");
    session
        .initialize("file:///workspace")
        .expect("initialize should succeed");

    let health = session.health();
    // The mock advertises hoverProvider=true and definitionProvider=true.
    let hover_cap = health
        .capabilities
        .iter()
        .find(|c| c.capability == "hoverProvider");
    assert!(
        hover_cap.is_some(),
        "capabilities must contain hoverProvider after initialize"
    );
    assert!(
        hover_cap.unwrap().supported,
        "hoverProvider must be supported=true (mock advertises it)"
    );

    let def_cap = health
        .capabilities
        .iter()
        .find(|c| c.capability == "definitionProvider");
    assert!(
        def_cap.is_some(),
        "capabilities must contain definitionProvider after initialize"
    );
    assert!(
        def_cap.unwrap().supported,
        "definitionProvider must be supported=true (mock advertises it)"
    );

    // The mock does NOT advertise completionProvider, so it must be false.
    let comp_cap = health
        .capabilities
        .iter()
        .find(|c| c.capability == "completionProvider");
    // completionProvider is tracked but supported=false (not in mock response).
    if let Some(c) = comp_cap {
        assert!(
            !c.supported,
            "completionProvider must be supported=false (mock does not advertise it)"
        );
    }
    // Whether completionProvider is absent or present-but-false, what matters is
    // that the capability list is non-empty (capabilities were parsed).
    assert!(
        !health.capabilities.is_empty(),
        "capabilities must be non-empty after a successful initialize"
    );
}

/// The product session now initializes with `{"files": {"watcher": "client"}}` to
/// prevent the notify file-watcher wedge on non-existent paths (M8 PKT-S3-WEDGE-R3).
/// This test verifies that `initialize_with_options` with the watcher option
/// succeeds and produces a healthy session — the same path taken by `startup_session`
/// after PKT-0 Task 1.
#[test]
fn initialize_with_watcher_client_option_succeeds() {
    let mock_path = lsp_mock::mock_server_path().expect(
        "mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`",
    );

    let config = RustAnalyzerLaunchConfig {
        discovery: RustAnalyzerDiscovery {
            configured_path: Some(mock_path),
            ..Default::default()
        },
        supervisor: lsp_mock::mock_supervisor_config(),
        server_id: LanguageServerId(8),
        language_id: LanguageId("rust".to_string()),
    };

    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session =
        RustAnalyzerSession::launch(config, &mut launcher).expect("launch should succeed");

    session
        .initialize_with_options(
            "file:///workspace",
            Some(serde_json::json!({"files": {"watcher": "client"}})),
            None,
        )
        .expect("initialize_with_options with watcher=client should succeed");

    let health = session.health();
    assert_eq!(
        health.init_status,
        LspResultStatus::Fresh,
        "init_status must be Fresh after initialize_with_options with watcher=client"
    );
    assert_eq!(
        health.binary_provenance,
        LspServerBinaryProvenance::Configured,
    );
}
