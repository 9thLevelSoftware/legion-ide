//! Restart/backoff policy tests (WS-LANG-01 LANG.10).
//!
//! `restart_backoff_grows_until_cap` and `backoff_clamps_shift_at_16` cover the
//! pure `RestartPolicy` logic. `note_crash_drives_restart_count_then_exhausts`
//! drives the session-level state machine through the mock LSP server.
//!
//! The mock binary (`mock_lsp_server`) lives in `crates/legion-lsp`.
//! It must be built before running this test in isolation:
//!   cargo build -p legion-lsp --bin mock_lsp_server

use legion_app::language::{
    RestartPolicy, RustAnalyzerDiscovery, RustAnalyzerLaunchConfig, RustAnalyzerSession,
};
use legion_protocol::{LanguageId, LanguageServerId, LspResultStatus};

mod lsp_mock;

#[test]
fn restart_backoff_grows_until_cap() {
    // Uses a session-free policy check via the public helper on the policy.
    let policy = RestartPolicy {
        max_restarts: 2,
        backoff_base_ms: 100,
    };
    assert_eq!(policy.backoff_for_attempt(0).as_millis(), 100);
    assert_eq!(policy.backoff_for_attempt(1).as_millis(), 200);
    assert!(policy.is_exhausted(2));
}

#[test]
fn backoff_clamps_shift_at_16() {
    let policy = RestartPolicy {
        max_restarts: 100,
        backoff_base_ms: 1,
    };
    assert_eq!(policy.backoff_for_attempt(16).as_millis(), 65536);
    assert_eq!(policy.backoff_for_attempt(20).as_millis(), 65536); // min(20,16)=16
}

#[test]
fn note_crash_drives_restart_count_then_exhausts() {
    let mock = lsp_mock::mock_server_path().expect(
        "mock_lsp_server not found — run `cargo build -p legion-lsp --bin mock_lsp_server`, \
                 or run under `cargo test --workspace --all-targets` which builds it",
    );

    let config = RustAnalyzerLaunchConfig {
        discovery: RustAnalyzerDiscovery {
            configured_path: Some(mock),
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

    let policy = RestartPolicy {
        max_restarts: 2,
        backoff_base_ms: 100,
    };

    // First crash: restart permitted, count -> 1.
    assert!(session.note_crash_and_should_restart(&policy).is_some());
    assert_eq!(session.health().restart_count, 1);

    // Second crash: restart permitted, count -> 2.
    assert!(session.note_crash_and_should_restart(&policy).is_some());
    assert_eq!(session.health().restart_count, 2);

    // Third crash: budget exhausted -> None and init_status flips to Unavailable.
    assert!(session.note_crash_and_should_restart(&policy).is_none());
    assert_eq!(session.health().init_status, LspResultStatus::Unavailable);
}
