use std::fs;

use legion_app::AppComposition;
use legion_editor::{TextEdit, TextPosition};
use legion_protocol::{
    LanguageToolchainConfigurationStatus, LspSessionLifecycleKind, PrincipalId, WorkspaceTrustState,
};

fn fixture_files() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("fixture directory");
    for name in ["server.tgz", "compiler.tgz", "node", "node2"] {
        fs::write(dir.path().join(name), b"fixture").expect("fixture file");
    }
    dir
}

fn open_trusted_app(root: &std::path::Path) -> AppComposition {
    let mut app = AppComposition::new();
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("toolchain-settings-test".to_string()),
    )
    .expect("open trusted workspace");
    app
}

#[test]
fn configure_toolchain_maps_all_typescript_family_adapters_and_persists_metadata() {
    let files = fixture_files();
    let mut app = open_trusted_app(files.path());
    app.configure_typescript_toolchain(
        files.path().join("server.tgz"),
        files.path().join("compiler.tgz"),
        files.path().join("node"),
    )
    .expect("configure local toolchain");
    app.configure_language_server_binary(
        legion_protocol::LanguageServerId(102),
        files.path().join("node"),
    )
    .expect("install preexisting configured TS route");
    assert_eq!(
        app.language_toolchain_configuration_state(),
        legion_app::LanguageToolchainConfigurationState::Configured
    );
    let snapshot = app
        .shell_projection_snapshot("toolchain")
        .expect("shell projection");
    assert_eq!(
        snapshot
            .language_tooling_projection
            .typescript_toolchain
            .status,
        LanguageToolchainConfigurationStatus::Configured
    );

    let settings = app.language_toolchain_settings();
    let typescript = settings.typescript.as_ref().expect("typescript settings");
    assert_eq!(
        typescript.server_archive.0,
        fs::canonicalize(files.path().join("server.tgz"))
            .expect("canonical server")
            .to_string_lossy()
    );

    let record = app
        .capture_workspace_session_record()
        .expect("capture session");
    assert_eq!(record.language_toolchain_settings, settings);
}

#[test]
fn invalid_input_is_atomic_and_untrusted_workspace_is_rejected() {
    let files = fixture_files();
    let mut app = open_trusted_app(files.path());
    app.configure_typescript_toolchain(
        files.path().join("server.tgz"),
        files.path().join("compiler.tgz"),
        files.path().join("node"),
    )
    .expect("initial configuration");
    let before = app.language_toolchain_settings();
    assert!(
        app.configure_typescript_toolchain(
            files.path().join("server.tgz"),
            files.path().join("missing.tgz"),
            files.path().join("node"),
        )
        .is_err()
    );
    assert_eq!(app.language_toolchain_settings(), before);

    let mut untrusted = AppComposition::new();
    untrusted
        .open_workspace(
            files.path(),
            WorkspaceTrustState::Untrusted,
            PrincipalId("toolchain-settings-test".to_string()),
        )
        .expect("open untrusted workspace");
    assert!(
        untrusted
            .configure_typescript_toolchain(
                files.path().join("server.tgz"),
                files.path().join("compiler.tgz"),
                files.path().join("node"),
            )
            .is_err()
    );
}

#[test]
fn restore_keeps_draft_without_regrant_and_clear_preserves_dirty_buffer() {
    let files = fixture_files();
    fs::write(files.path().join("main.ts"), "const answer = 41;\n").expect("source file");
    let mut app = open_trusted_app(files.path());
    app.open_file(files.path().join("main.ts").to_string_lossy())
        .expect("open source");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 18), "!"))
        .expect("make dirty");
    app.configure_typescript_toolchain(
        files.path().join("server.tgz"),
        files.path().join("compiler.tgz"),
        files.path().join("node"),
    )
    .expect("configure local toolchain");
    let record = app.capture_workspace_session_record().expect("capture");

    let mut restored = open_trusted_app(files.path());
    restored
        .configure_language_server_binary(
            legion_protocol::LanguageServerId(102),
            files.path().join("node2"),
        )
        .expect("install stale configured TS route with distinct executable");
    restored
        .restore_workspace_session_record(&record)
        .expect("restore");
    assert_eq!(
        restored.language_toolchain_configuration_state(),
        legion_app::LanguageToolchainConfigurationState::Draft
    );
    let _ = restored.dispatch_ui_intent(legion_ui::CommandDispatchIntent::LspStartSession);
    for _ in 0..20 {
        restored.drain_lsp_session();
        if restored.lsp_session_status_projection().lifecycle == LspSessionLifecycleKind::Refused {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        restored.lsp_session_status_projection().lifecycle,
        LspSessionLifecycleKind::Refused
    );
    assert!(
        restored
            .lsp_session_status_projection()
            .failure_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("no explicitly configured")),
        "restored draft must refuse for missing explicit approval, not a later process failure"
    );

    restored
        .edit_active_buffer(TextEdit::insert(TextPosition::new(0, 18), "!"))
        .expect("make restored buffer dirty");
    restored.clear_typescript_toolchain();
    assert!(restored.language_toolchain_settings().typescript.is_none());
    let after = restored
        .capture_workspace_session_record()
        .expect("capture after clear");
    assert!(
        after
            .dirty_indicators
            .iter()
            .any(|indicator| indicator.dirty),
        "clearing configuration must preserve dirty editor state"
    );
}

#[test]
fn explicit_reconfigure_reconfirms_unsupported_saved_schema() {
    let files = fixture_files();
    let mut source = open_trusted_app(files.path());
    source
        .configure_typescript_toolchain(
            files.path().join("server.tgz"),
            files.path().join("compiler.tgz"),
            files.path().join("node"),
        )
        .expect("configure");
    let mut record = source.capture_workspace_session_record().expect("capture");
    record.language_toolchain_settings.schema_version = 99;

    let mut app = open_trusted_app(files.path());
    app.restore_workspace_session_record(&record)
        .expect("restore unsupported schema as draft");
    assert_eq!(
        app.language_toolchain_configuration_state(),
        legion_app::LanguageToolchainConfigurationState::Draft
    );
    app.configure_typescript_toolchain(
        files.path().join("server.tgz"),
        files.path().join("compiler.tgz"),
        files.path().join("node"),
    )
    .expect("explicit reconfiguration");
    assert_eq!(app.language_toolchain_settings().schema_version, 1);
    assert_eq!(
        app.language_toolchain_configuration_state(),
        legion_app::LanguageToolchainConfigurationState::Configured
    );
}
