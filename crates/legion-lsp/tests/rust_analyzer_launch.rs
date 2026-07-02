use legion_lsp::LanguageServerAdapterRegistry;
use legion_protocol::{LanguageId, WorkspaceId};

fn mock_lsp_server_command() -> String {
    std::env::var("CARGO_BIN_EXE_mock_lsp_server").unwrap_or_else(|_| "rust-analyzer".to_string())
}

#[test]
fn rust_analyzer_launch_is_resolved_through_the_product_registry() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let workspace_id = WorkspaceId(1);
    let rust_configs = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("rust".to_string()));

    assert_eq!(
        rust_configs.len(),
        1,
        "expected a single rust launch config"
    );
    assert_eq!(
        rust_configs[0].command,
        mock_lsp_server_command(),
        "the rust launch config should come from the production product registry"
    );
    assert!(
        rust_configs[0].args.is_empty(),
        "the registry-backed mock server should launch without extra args"
    );
}
