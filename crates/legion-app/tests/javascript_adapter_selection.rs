use legion_app::AppComposition;
use legion_protocol::{LspSessionLifecycleKind, PrincipalId, WorkspaceTrustState};
use legion_ui::CommandDispatchIntent;

#[test]
fn javascript_jsx_and_tsx_select_configured_registry_adapters() {
    for (name, expected_language) in [
        ("main.js", "javascript"),
        ("view.jsx", "javascriptreact"),
        ("view.tsx", "typescriptreact"),
    ] {
        let root = tempfile::tempdir().expect("workspace");
        let file = root.path().join(name);
        std::fs::write(&file, "const value = 1;\n").expect("source");
        let mut app = AppComposition::new();
        app.open_workspace(
            root.path(),
            WorkspaceTrustState::Trusted,
            PrincipalId("js-selection-test".to_string()),
        )
        .expect("open workspace");
        app.open_file(file.to_string_lossy()).expect("open source");
        let binary = root.path().join("typescript-language-server");
        std::fs::write(&binary, b"fixture").expect("server fixture");
        let server_id = match expected_language {
            "javascript" => 106,
            "javascriptreact" => 107,
            "typescriptreact" => 108,
            _ => unreachable!(),
        };
        app.configure_language_server_binary(legion_protocol::LanguageServerId(server_id), &binary)
            .expect("configure exact server fixture");
        app.dispatch_ui_intent(CommandDispatchIntent::LspStartSession)
            .expect("explicit start dispatch");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let health = loop {
            app.drain_lsp_session();
            if let Some(health) = app.lsp_server_health_record() {
                break health;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "configured {expected_language} adapter never reached observable health state"
            );
            std::thread::sleep(std::time::Duration::from_millis(25));
        };
        let status = app.lsp_session_status_projection();
        assert_ne!(status.lifecycle, LspSessionLifecycleKind::Idle);
        assert_eq!(health.server_id.0, server_id);
        assert_eq!(health.language_id.0, expected_language);
        assert!(
            matches!(
                status.lifecycle,
                LspSessionLifecycleKind::Starting
                    | LspSessionLifecycleKind::Live
                    | LspSessionLifecycleKind::BackingOff
                    | LspSessionLifecycleKind::Refused
                    | LspSessionLifecycleKind::Failed
            ),
            "{name} ({expected_language}) reached unexpected lifecycle {:?}",
            status.lifecycle
        );
    }
}
