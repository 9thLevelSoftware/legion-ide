use legion_lsp::{LanguageServerAdapterRegistry, LspServerBinaryManifest, LspServerBinarySource};
use legion_protocol::{LanguageId, WorkspaceId};

#[test]
fn tier_two_registry_covers_the_expected_language_smoke_set() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let workspace_id = WorkspaceId(1);

    let rust = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("rust".to_string()));
    assert_eq!(rust.len(), 1);
    let expected_rust_command = std::env::var("CARGO_BIN_EXE_mock_lsp_server")
        .unwrap_or_else(|_| "rust-analyzer".to_string());
    assert_eq!(rust[0].command, expected_rust_command);
    assert!(rust[0].args.is_empty());

    let typescript = registry.process_configs_for_workspace_language(
        workspace_id,
        &LanguageId("typescript".to_string()),
    );
    assert_eq!(typescript.len(), 2);
    assert_eq!(typescript[0].command, "typescript-language-server");
    assert_eq!(typescript[0].args, vec!["--stdio".to_string()]);
    assert_eq!(typescript[1].command, "tailwindcss-language-server");
    assert_eq!(typescript[1].args, vec!["--stdio".to_string()]);

    let python = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("python".to_string()));
    assert_eq!(python.len(), 1);
    assert_eq!(python[0].command, "pyright-langserver");
    assert_eq!(python[0].args, vec!["--stdio".to_string()]);

    let go = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("go".to_string()));
    assert_eq!(go.len(), 1);
    assert_eq!(go[0].command, "gopls");
    assert!(go[0].args.is_empty());
}

#[test]
fn downloaded_artifact_entries_keep_binary_policy_metadata() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let python_adapter = registry
        .adapters_for_language(&LanguageId("python".to_string()))
        .into_iter()
        .next()
        .expect("python adapter should exist");

    match &python_adapter.binary_source {
        LspServerBinarySource::DownloadedArtifact {
            binary_name,
            artifact_uri,
            checksum_sha256,
            policy_gate,
        } => {
            assert_eq!(binary_name, "pyright-langserver");
            assert!(artifact_uri.contains("pyright-1.1.400"));
            assert!(checksum_sha256.starts_with("sha256:"));
            assert_eq!(policy_gate, "policy://lsp-download/pyright");
        }
        other => panic!("expected downloaded artifact source, got {other:?}"),
    }

    let process = python_adapter.process_config();
    assert_eq!(process.command, "pyright-langserver");
    assert_eq!(process.args, vec!["--stdio".to_string()]);
}

#[test]
fn air_gap_manifest_denies_downloads_but_keeps_system_binaries() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let workspace_id = WorkspaceId(1);

    let rust = registry.binary_manifest_for_workspace_language(
        workspace_id,
        &LanguageId("rust".to_string()),
        true,
    );
    let expected_rust_command = std::env::var("CARGO_BIN_EXE_mock_lsp_server")
        .unwrap_or_else(|_| "rust-analyzer".to_string());
    assert_manifest_system_path_only(&rust, &expected_rust_command);

    let python = registry.binary_manifest_for_workspace_language(
        workspace_id,
        &LanguageId("python".to_string()),
        true,
    );
    assert!(python.entries.is_empty());
    assert_eq!(python.denied_downloads.len(), 1);
    assert!(python.denied_downloads[0].contains("pyright"));
    assert!(python.denied_downloads[0].contains("policy://lsp-download/pyright"));
}

#[test]
fn manifest_records_workspace_version_pin_for_downloaded_artifacts() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let workspace_id = WorkspaceId(1);

    let python = registry.binary_manifest_for_workspace_language(
        workspace_id,
        &LanguageId("python".to_string()),
        false,
    );
    assert_eq!(python.entries.len(), 1);
    let entry = &python.entries[0];
    assert_eq!(entry.workspace_version_pin.as_deref(), Some("workspace/1"));
    match &entry.binary_source {
        LspServerBinarySource::DownloadedArtifact {
            binary_name,
            artifact_uri,
            checksum_sha256,
            policy_gate,
        } => {
            assert_eq!(binary_name, "pyright-langserver");
            assert!(artifact_uri.contains("pyright-1.1.400"));
            assert_eq!(checksum_sha256, "sha256:pyright-1.1.400");
            assert_eq!(policy_gate, "policy://lsp-download/pyright");
        }
        other => panic!("expected downloaded artifact source, got {other:?}"),
    }
}

fn assert_manifest_system_path_only(manifest: &LspServerBinaryManifest, expected_command: &str) {
    assert_eq!(manifest.entries.len(), 1);
    assert!(manifest.denied_downloads.is_empty());
    let entry = &manifest.entries[0];
    assert_eq!(entry.workspace_version_pin, None);
    match &entry.binary_source {
        LspServerBinarySource::SystemPath { binary_name } => {
            assert_eq!(binary_name, expected_command);
        }
        other => panic!("expected system path source, got {other:?}"),
    }
}
