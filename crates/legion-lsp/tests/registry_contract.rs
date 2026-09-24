use legion_lsp::{
    LanguageServerAdapterRegistry, LspArtifactRuntime, LspDownloadedArtifactMetadata,
    LspDownloadedArtifactResolveError, LspNodeVersion, LspServerBinaryManifest,
    LspServerBinarySource,
};
use legion_protocol::{LanguageId, WorkspaceId};

fn catalog_checksum(source: &LspServerBinarySource) -> String {
    match source {
        LspServerBinarySource::DownloadedArtifact {
            checksum_sha256, ..
        } => checksum_sha256.clone(),
        _ => "0".repeat(64),
    }
}

#[test]
fn registry_rebinds_catalog_to_real_workspace_without_mutating_catalog() {
    let catalog = LanguageServerAdapterRegistry::tier_two();
    let bound = catalog
        .for_workspace(WorkspaceId(77))
        .expect("nonzero workspace identity should bind");

    let python = bound
        .adapters_for_workspace_language(WorkspaceId(77), &LanguageId("python".to_string()))
        .expect("bound Python plans should be selectable");
    assert_eq!(python.len(), 1);
    assert_eq!(python[0].workspace_id, WorkspaceId(77));
    assert!(python[0].is_primary);

    let typescript = bound
        .adapters_for_workspace_language(WorkspaceId(77), &LanguageId("typescript".to_string()))
        .expect("bound TypeScript plans should be selectable");
    assert_eq!(typescript.len(), 2);
    assert!(typescript[0].is_primary);
    assert!(!typescript[1].is_primary);

    let javascript = bound
        .adapters_for_workspace_language(WorkspaceId(77), &LanguageId("javascript".to_string()))
        .expect("bound JavaScript plan should be selectable");
    assert_eq!(javascript.len(), 1);
    assert!(javascript[0].is_primary);
    assert_eq!(javascript[0].server_id.0, 106);
    assert_eq!(
        javascript[0].language_id,
        LanguageId("javascript".to_string())
    );

    let original = catalog
        .adapters_for_language(&LanguageId("python".to_string()))
        .into_iter()
        .next()
        .expect("catalog Python plan should remain available");
    assert_eq!(original.workspace_id, WorkspaceId(1));

    assert!(
        bound
            .adapters_for_workspace_language(WorkspaceId(0), &LanguageId("python".to_string()))
            .is_err()
    );
    assert!(catalog.for_workspace(WorkspaceId(0)).is_err());
}

#[test]
fn tier_two_registry_covers_the_expected_language_smoke_set() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let workspace_id = WorkspaceId(1);

    let rust = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("rust".to_string()))
        .expect("system adapter should resolve");
    assert_eq!(rust.len(), 1);
    let expected_rust_command = std::env::var("CARGO_BIN_EXE_mock_lsp_server")
        .unwrap_or_else(|_| "rust-analyzer".to_string());
    assert_eq!(rust[0].command, expected_rust_command);
    assert!(rust[0].args.is_empty());

    // The primary TypeScript entry is a pinned downloaded artifact now, so the
    // all-or-error materialization of the whole language fails closed until the
    // app materializes it. It previously asserted the bare PATH command
    // `typescript-language-server`; that expectation was the unpinned shape
    // this packet removes, so the assertion is updated rather than deleted.
    assert!(matches!(
        registry.process_configs_for_workspace_language(
            workspace_id,
            &LanguageId("typescript".to_string())
        ),
        Err(LspDownloadedArtifactResolveError::ArtifactNotMaterialized)
    ));
    let typescript = registry.adapters_for_language(&LanguageId("typescript".to_string()));
    assert_eq!(typescript.len(), 2);
    assert_eq!(typescript[0].server_id.0, 102);
    assert!(typescript[0].is_primary);
    assert_eq!(typescript[0].process.args, vec!["--stdio".to_string()]);
    assert_eq!(typescript[1].server_id.0, 103);
    assert!(!typescript[1].is_primary);
    let tailwind = typescript[1]
        .process_config()
        .expect("the tailwind fallback is still a system-path adapter");
    assert_eq!(tailwind.command, "tailwindcss-language-server");
    assert_eq!(tailwind.args, vec!["--stdio".to_string()]);

    // JavaScript, JSX and TSX reuse the same pinned archive, so each of them
    // fails closed the same way instead of resolving a bare PATH command.
    for language in ["javascript", "javascriptreact", "typescriptreact"] {
        assert!(
            matches!(
                registry.process_configs_for_workspace_language(
                    workspace_id,
                    &LanguageId(language.to_string())
                ),
                Err(LspDownloadedArtifactResolveError::ArtifactNotMaterialized)
            ),
            "{language} must fail closed until its pinned archive is materialized"
        );
        let adapters = registry.adapters_for_language(&LanguageId(language.to_string()));
        assert_eq!(adapters.len(), 1, "{language} keeps one adapter");
        assert!(adapters[0].is_primary);
        assert_eq!(adapters[0].process.args, vec!["--stdio".to_string()]);
    }

    let python = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("python".to_string()));
    assert!(matches!(
        python,
        Err(LspDownloadedArtifactResolveError::ArtifactNotMaterialized)
    ));

    let go = registry
        .process_configs_for_workspace_language(workspace_id, &LanguageId("go".to_string()))
        .expect("system adapter should resolve");
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
            metadata,
        } => {
            assert_eq!(binary_name, "pyright-langserver");
            assert_eq!(
                artifact_uri,
                "https://registry.npmjs.org/pyright/-/pyright-1.1.400.tgz"
            );
            assert_eq!(
                checksum_sha256,
                "2ccba7af9c8b14bb81c8fa9bb558d8b5181b586ec4dfc448b78eb4209e7a429a"
            );
            assert_eq!(policy_gate, "policy://lsp-download/pyright");
            assert_eq!(metadata.version, "1.1.400");
            assert_eq!(metadata.package_name, "pyright");
            assert_eq!(metadata.archive_format, "tar.gz");
            assert_eq!(metadata.package_root, std::path::Path::new("package"));
            assert_eq!(
                metadata.entrypoint,
                std::path::Path::new("langserver.index.js")
            );
            assert_eq!(
                metadata.runtime,
                LspArtifactRuntime::Node {
                    minimum_version: LspNodeVersion {
                        major: 14,
                        minor: 0,
                        patch: 0,
                    }
                }
            );
        }
        other => panic!("expected downloaded artifact source, got {other:?}"),
    }

    assert!(matches!(
        python_adapter.process_config(),
        Err(LspDownloadedArtifactResolveError::ArtifactNotMaterialized)
    ));

    let typescript_adapter = registry
        .adapters_for_language(&LanguageId("typescript".to_string()))
        .into_iter()
        .find(|adapter| adapter.is_primary)
        .expect("primary typescript adapter should exist");
    assert_pinned_typescript_source(&typescript_adapter.binary_source);
    assert!(matches!(
        typescript_adapter.process_config(),
        Err(LspDownloadedArtifactResolveError::ArtifactNotMaterialized)
    ));
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

    // The pinned TypeScript server is denied under air gap; the unpinned
    // tailwind PATH fallback is retained. That split is exactly what the
    // manifest exists to report, and it only became observable once entry 102
    // stopped being a bare PATH lookup.
    let typescript = registry.binary_manifest_for_workspace_language(
        workspace_id,
        &LanguageId("typescript".to_string()),
        true,
    );
    assert_eq!(typescript.entries.len(), 1);
    assert_eq!(typescript.entries[0].server_id.0, 103);
    assert_eq!(typescript.denied_downloads.len(), 1);
    assert!(typescript.denied_downloads[0].contains(
        "https://registry.npmjs.org/typescript-language-server/-/typescript-language-server-6.0.0.tgz"
    ));
    assert!(
        typescript.denied_downloads[0].contains("policy://lsp-download/typescript-language-server")
    );
    assert!(
        typescript.denied_downloads[0]
            .contains("6e23b48efc76af4e70928cdfe62ea6e6cfef67ab4c1e7579c4e82dd284fbdfd2")
    );

    // The alias languages reuse the same pinned archive as `typescript`, so
    // their denial records must carry the same identity: archive url, policy
    // gate and digest. Asserting only the count here would have let an alias
    // silently point at a different archive, or at none, while the test still
    // passed.
    for (language, display_name) in [
        ("javascript", "typescript-language-server (JavaScript)"),
        ("javascriptreact", "typescript-language-server (JSX)"),
        ("typescriptreact", "typescript-language-server (TSX)"),
    ] {
        let manifest = registry.binary_manifest_for_workspace_language(
            workspace_id,
            &LanguageId(language.to_string()),
            true,
        );
        assert!(
            manifest.entries.is_empty(),
            "{language} has no system-path fallback under air gap"
        );
        assert_eq!(manifest.denied_downloads.len(), 1);
        let denied = &manifest.denied_downloads[0];
        assert!(
            denied.starts_with(format!("{display_name}:").as_str()),
            "{language} denial must name its own adapter, got {denied}"
        );
        assert!(
            denied.contains(
                "https://registry.npmjs.org/typescript-language-server/-/typescript-language-server-6.0.0.tgz"
            ),
            "{language} denial must name the pinned archive url, got {denied}"
        );
        assert!(
            denied.contains("policy://lsp-download/typescript-language-server"),
            "{language} denial must name the policy gate, got {denied}"
        );
        assert!(
            denied.contains("6e23b48efc76af4e70928cdfe62ea6e6cfef67ab4c1e7579c4e82dd284fbdfd2"),
            "{language} denial must carry the pinned digest, got {denied}"
        );
    }
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
            metadata,
        } => {
            assert_eq!(binary_name, "pyright-langserver");
            assert_eq!(
                artifact_uri,
                "https://registry.npmjs.org/pyright/-/pyright-1.1.400.tgz"
            );
            assert_eq!(
                checksum_sha256,
                "2ccba7af9c8b14bb81c8fa9bb558d8b5181b586ec4dfc448b78eb4209e7a429a"
            );
            assert_eq!(policy_gate, "policy://lsp-download/pyright");
            assert_eq!(metadata.version, "1.1.400");
        }
        other => panic!("expected downloaded artifact source, got {other:?}"),
    }

    // The TypeScript manifest now carries both shapes: the pinned server takes
    // a workspace version pin, the unpinned tailwind fallback does not.
    let typescript = registry.binary_manifest_for_workspace_language(
        workspace_id,
        &LanguageId("typescript".to_string()),
        false,
    );
    assert_eq!(typescript.entries.len(), 2);
    assert!(typescript.denied_downloads.is_empty());
    assert_eq!(typescript.entries[0].server_id.0, 102);
    assert_eq!(
        typescript.entries[0].workspace_version_pin.as_deref(),
        Some("workspace/1")
    );
    assert_pinned_typescript_source(&typescript.entries[0].binary_source);
    assert_eq!(typescript.entries[1].server_id.0, 103);
    assert_eq!(typescript.entries[1].workspace_version_pin, None);
}

#[test]
fn downloaded_pyright_resolves_to_node_and_absolute_entrypoint() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let adapter = registry.adapters_for_language(&LanguageId("python".to_string()))[0];
    let root_path =
        std::env::temp_dir().join(format!("legion-lsp-registry-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root_path);
    std::fs::create_dir_all(root_path.join("package")).expect("package");
    std::fs::write(root_path.join("package/langserver.index.js"), b"entry").expect("entrypoint");
    let node = root_path.join("node");
    std::fs::write(&node, b"node").expect("node");
    assert!(matches!(
        adapter.resolve_downloaded_process(
            &root_path,
            &node,
            "13.9.0",
            &catalog_checksum(&adapter.binary_source),
        ),
        Err(LspDownloadedArtifactResolveError::RuntimeTooOld { .. })
    ));
    let config = adapter
        .resolve_downloaded_process(
            &root_path,
            &node,
            "v18.20.0",
            &catalog_checksum(&adapter.binary_source),
        )
        .expect("materialized package should resolve");
    let entrypoint_argument = |path: &std::path::Path| {
        let value = path.canonicalize().unwrap().to_string_lossy().into_owned();
        if cfg!(windows)
            && let Some(rest) = value.strip_prefix("\\\\?\\")
            && rest.len() >= 2
            && rest.as_bytes()[1] == b':'
        {
            return rest.to_string();
        }
        if cfg!(windows)
            && let Some(rest) = value.strip_prefix("\\\\?\\")
            && let Some(unc) = rest.strip_prefix("UNC\\")
        {
            return format!("\\\\{unc}");
        }
        value
    };
    assert_eq!(
        config.command,
        node.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(
        config.args[0],
        entrypoint_argument(&root_path.join("package/langserver.index.js"))
    );
    assert_eq!(config.args[1], "--stdio");
    std::fs::remove_dir_all(root_path).expect("cleanup");
}

#[test]
fn downloaded_resolver_rejects_mismatched_materializer_receipt() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let adapter = registry.adapters_for_language(&LanguageId("python".to_string()))[0];
    let root_path = std::env::temp_dir().join(format!(
        "legion-lsp-registry-checksum-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root_path);
    std::fs::create_dir_all(root_path.join("package")).expect("package");
    std::fs::write(root_path.join("package/langserver.index.js"), b"entry").expect("entrypoint");
    let node = root_path.join("node");
    std::fs::write(&node, b"node").expect("node");
    assert!(matches!(
        adapter.resolve_downloaded_process(
            &root_path,
            &node,
            "v18.20.0",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        ),
        Err(LspDownloadedArtifactResolveError::ChecksumMismatch)
    ));
    std::fs::remove_dir_all(root_path).expect("cleanup");
}

#[test]
fn downloaded_resolver_rejects_unmaterialized_and_escaping_paths() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let adapter = registry.adapters_for_language(&LanguageId("python".to_string()))[0];
    let root_path = std::env::temp_dir().join(format!(
        "legion-lsp-registry-missing-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root_path);
    std::fs::create_dir_all(&root_path).expect("artifact root");
    let node = root_path.join("node");
    std::fs::write(&node, b"node").expect("node");
    assert!(matches!(
        adapter.resolve_downloaded_process(
            &root_path,
            &node,
            "18.0.0",
            &catalog_checksum(&adapter.binary_source),
        ),
        Err(LspDownloadedArtifactResolveError::MissingPath {
            field: "package_root",
            ..
        })
    ));
    let registry = LanguageServerAdapterRegistry::tier_two();
    let system = registry.adapters_for_language(&LanguageId("rust".to_string()))[0];
    assert!(matches!(
        system.resolve_downloaded_process(
            &root_path,
            &node,
            "18.0.0",
            &catalog_checksum(&system.binary_source),
        ),
        Err(LspDownloadedArtifactResolveError::NotDownloadedArtifact)
    ));
    let escaping = legion_lsp::LanguageServerAdapterPlan::downloaded_package_artifact(
        legion_protocol::LanguageServerId(900),
        WorkspaceId(1),
        LanguageId("python".into()),
        "escaping",
        "pyright-langserver",
        "https://registry.npmjs.org/pyright/-/pyright-1.1.400.tgz",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "policy://test",
        LspDownloadedArtifactMetadata {
            // The version is an exact pin so this fixture keeps testing the
            // package_root escape it was written for. Before this packet the
            // resolver accepted `"test"` as a version; it now rejects any
            // version that is not an exact release, which would otherwise mask
            // the UnsafePath rejection asserted below.
            package_name: "pyright".into(),
            version: "1.1.400".into(),
            archive_format: "tar.gz".into(),
            package_root: "../escape".into(),
            entrypoint: "entry.js".into(),
            runtime: LspArtifactRuntime::Node {
                minimum_version: LspNodeVersion {
                    major: 14,
                    minor: 0,
                    patch: 0,
                },
            },
        },
        vec!["--stdio".into()],
        true,
    );
    assert!(matches!(
        escaping.resolve_downloaded_process(
            &root_path,
            &node,
            "18.0.0",
            &catalog_checksum(&escaping.binary_source),
        ),
        Err(LspDownloadedArtifactResolveError::UnsafePath {
            field: "package_root"
        })
    ));
    std::fs::remove_dir_all(root_path).expect("cleanup");
}

#[test]
fn node_version_parser_enforces_minimum_and_rejects_malformed_values() {
    assert_eq!(
        LspNodeVersion::parse("v18.20.0\n").expect("LF version"),
        LspNodeVersion {
            major: 18,
            minor: 20,
            patch: 0,
        }
    );
    assert_eq!(
        LspNodeVersion::parse("v18.20.0\r\n").expect("CRLF version"),
        LspNodeVersion {
            major: 18,
            minor: 20,
            patch: 0,
        }
    );
    assert_eq!(
        LspNodeVersion::parse("v18.20.0").expect("valid version"),
        LspNodeVersion {
            major: 18,
            minor: 20,
            patch: 0,
        }
    );
    assert!(matches!(
        LspNodeVersion::parse("18.20"),
        Err(LspDownloadedArtifactResolveError::InvalidRuntimeVersion { .. })
    ));
    assert!(matches!(
        LspNodeVersion::parse("18.20.0.1"),
        Err(LspDownloadedArtifactResolveError::InvalidRuntimeVersion { .. })
    ));
    assert!(matches!(
        LspNodeVersion::parse("v18.20.0\nextra"),
        Err(LspDownloadedArtifactResolveError::InvalidRuntimeVersion { .. })
    ));
    assert!(matches!(
        LspNodeVersion::parse("v18.20.0-pre"),
        Err(LspDownloadedArtifactResolveError::InvalidRuntimeVersion { .. })
    ));
    assert!(matches!(
        LspNodeVersion::parse("v18.20.0 trailing"),
        Err(LspDownloadedArtifactResolveError::InvalidRuntimeVersion { .. })
    ));
}

#[test]
fn resolver_preserves_extra_arguments_after_entrypoint() {
    let adapter = legion_lsp::LanguageServerAdapterPlan::downloaded_package_artifact(
        legion_protocol::LanguageServerId(901),
        WorkspaceId(1),
        LanguageId("python".into()),
        "extra-args",
        "pyright-langserver",
        "https://registry.npmjs.org/pyright/-/pyright-1.1.400.tgz",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "policy://test",
        LspDownloadedArtifactMetadata {
            package_name: "pyright".into(),
            version: "1.1.400".into(),
            archive_format: "tar.gz".into(),
            package_root: "package".into(),
            entrypoint: "langserver.index.js".into(),
            runtime: LspArtifactRuntime::Node {
                minimum_version: LspNodeVersion {
                    major: 14,
                    minor: 0,
                    patch: 0,
                },
            },
        },
        vec!["--stdio".into(), "--verbose".into()],
        true,
    );
    let root = std::env::temp_dir().join(format!("legion-lsp-extra-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("package")).expect("package");
    std::fs::write(root.join("package/langserver.index.js"), b"entry").expect("entrypoint");
    let node = root.join("node");
    std::fs::write(&node, b"node").expect("node");
    let config = adapter
        .resolve_downloaded_process(
            &root,
            &node,
            "18.0.0",
            &catalog_checksum(&adapter.binary_source),
        )
        .expect("resolver");
    assert_eq!(config.args[1..], ["--stdio", "--verbose"]);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn resolver_rejects_symlinked_package_escape_and_non_utf8_runtime_path() {
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::symlink;
    let root = std::env::temp_dir().join(format!("legion-lsp-symlink-{}", std::process::id()));
    let outside = std::env::temp_dir().join(format!("legion-lsp-outside-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(&root).expect("root");
    std::fs::create_dir_all(&outside).expect("outside");
    std::fs::write(outside.join("langserver.index.js"), b"entry").expect("outside entrypoint");
    symlink(&outside, root.join("package")).expect("package symlink");
    let node = root.join("node");
    std::fs::write(&node, b"node").expect("node");
    let registry = LanguageServerAdapterRegistry::tier_two();
    let adapter = registry.adapters_for_language(&LanguageId("python".into()))[0];
    assert!(matches!(
        adapter.resolve_downloaded_process(
            &root,
            &node,
            "18.0.0",
            &catalog_checksum(&adapter.binary_source),
        ),
        Err(LspDownloadedArtifactResolveError::WrongPathKind {
            field: "package_root",
            ..
        })
    ));
    std::fs::remove_file(root.join("package")).expect("remove package symlink");
    std::fs::create_dir(root.join("package")).expect("real package");
    std::fs::write(root.join("package/langserver.index.js"), b"entry").expect("entrypoint");
    let non_utf8_node = root.join(std::ffi::OsString::from_vec(vec![
        b'n', b'o', b'd', b'e', 0xff,
    ]));
    match std::fs::write(&non_utf8_node, b"node") {
        Ok(()) => {
            assert!(matches!(
                adapter.resolve_downloaded_process(
                    &root,
                    &non_utf8_node,
                    "18.0.0",
                    &catalog_checksum(&adapter.binary_source),
                ),
                Err(LspDownloadedArtifactResolveError::NonUtf8Path {
                    field: "approved_node"
                })
            ));
        }
        Err(error) => {
            // macOS rejects non-UTF-8 filenames at the filesystem, which is a
            // stronger form of the same gate the resolver enforces on Linux.
            #[cfg(not(target_os = "macos"))]
            panic!("non-utf8 node: {error:?}");
            #[cfg(target_os = "macos")]
            let _ = error;
        }
    }
    std::fs::remove_dir_all(root).expect("cleanup root");
    std::fs::remove_dir_all(outside).expect("cleanup outside");
}

#[test]
fn tier_two_typescript_entry_is_a_pinned_downloaded_artifact() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let typescript = registry.adapters_for_language(&LanguageId("typescript".to_string()));
    let primary = typescript
        .iter()
        .find(|adapter| adapter.is_primary)
        .expect("tier-two TypeScript primary adapter");
    assert_eq!(primary.server_id.0, 102);
    assert_eq!(primary.display_name, "typescript-language-server");
    assert_eq!(primary.language_id, LanguageId("typescript".to_string()));
    assert_pinned_typescript_source(&primary.binary_source);
    // A downloaded artifact has no launchable process configuration until the
    // app materializes and verifies it, so the registry must refuse to hand
    // one out rather than falling back to a PATH lookup.
    assert!(matches!(
        primary.process_config(),
        Err(LspDownloadedArtifactResolveError::ArtifactNotMaterialized)
    ));
}

#[test]
fn pinned_typescript_descriptor_matches_the_retained_archive_identity() {
    // The literals below are restated independently of `legion-lsp`'s
    // constants so a typo on either side fails. Both digests were recomputed
    // with sha256sum from the retained archives —
    // typescript-language-server-6.0.0.tgz (515,598 bytes) and
    // typescript-6.0.3.tgz (4,515,854 bytes) — not transcribed from a registry
    // manifest.
    let server = legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE;
    assert_eq!(server.package_name, "typescript-language-server");
    assert_eq!(server.version, "6.0.0");
    assert_eq!(
        server.checksum_sha256,
        "6e23b48efc76af4e70928cdfe62ea6e6cfef67ab4c1e7579c4e82dd284fbdfd2"
    );
    assert_eq!(
        server.archive_url,
        "https://registry.npmjs.org/typescript-language-server/-/typescript-language-server-6.0.0.tgz"
    );
    assert_eq!(server.archive_format, "tar.gz");
    assert_eq!(server.package_root, "package");
    assert_eq!(server.entrypoint, "lib/cli.mjs");
    assert_eq!(
        server.minimum_node,
        LspNodeVersion {
            major: 22,
            minor: 22,
            patch: 2,
        }
    );

    // The peer compiler archive is recorded as its own descriptor. The adapter
    // type carries one archive and one entrypoint, so this identity is
    // deliberately not folded into the server's metadata.
    let compiler = legion_lsp::TYPESCRIPT_COMPILER_ARCHIVE;
    assert_eq!(compiler.package_name, "typescript");
    assert_eq!(compiler.version, "6.0.3");
    assert_eq!(
        compiler.checksum_sha256,
        "33cd0ee1beaa8c9e9d15a9da836c62ddea4c34a42d7c2d349dbc80d94165d22a"
    );
    assert_eq!(
        compiler.archive_url,
        "https://registry.npmjs.org/typescript/-/typescript-6.0.3.tgz"
    );
    assert_eq!(compiler.entrypoint, "lib/tsserver.js");
    assert_ne!(server.checksum_sha256, compiler.checksum_sha256);
    assert_ne!(server.entrypoint, compiler.entrypoint);

    for archive in [server, compiler] {
        assert!(
            legion_lsp::is_exact_pinned_version(archive.version),
            "{} version {} must be an exact pin",
            archive.package_name,
            archive.version
        );
        assert_eq!(
            archive.checksum_sha256.len(),
            64,
            "{} digest must be 64 characters",
            archive.package_name
        );
        assert!(
            archive
                .checksum_sha256
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
            "{} digest must be lowercase ASCII hex",
            archive.package_name
        );
    }

    // The registry entry carries exactly the pinned archive's identity; no
    // field drifted between the constant and the catalog.
    let registry = LanguageServerAdapterRegistry::tier_two();
    let primary = registry
        .adapters_for_language(&LanguageId("typescript".to_string()))
        .into_iter()
        .find(|adapter| adapter.is_primary)
        .expect("tier-two TypeScript primary adapter");
    match &primary.binary_source {
        LspServerBinarySource::DownloadedArtifact {
            artifact_uri,
            checksum_sha256,
            metadata,
            ..
        } => {
            assert_eq!(artifact_uri, server.archive_url);
            assert_eq!(checksum_sha256, server.checksum_sha256);
            assert_eq!(**metadata, server.metadata());
            assert_ne!(**metadata, compiler.metadata());
        }
        other => panic!("expected a pinned downloaded artifact, got {other:?}"),
    }
}

#[test]
fn resolver_rejects_a_version_range_or_tag_as_an_unpinned_server_version() {
    let digest = legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE.checksum_sha256;
    let root = std::env::temp_dir().join(format!("legion-lsp-unpinned-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("package/lib")).expect("package");
    std::fs::write(root.join("package/lib/cli.mjs"), b"entry").expect("entrypoint");
    let node = root.join("node");
    std::fs::write(&node, b"node").expect("node");

    for unpinned in [
        "^6.0.0",
        "~6.0",
        "~6.0.0",
        ">=6.0.0",
        "6.x",
        "6.0",
        "*",
        "latest",
        "next",
        "6.0.0 || 7.0.0",
        "1.2.3.4",
        "v6.0.0",
    ] {
        let adapter = typescript_plan_with(unpinned, digest);
        assert_eq!(
            adapter.resolve_downloaded_process(&root, &node, "v22.22.2", digest),
            Err(LspDownloadedArtifactResolveError::UnpinnedPackageVersion {
                value: unpinned.to_string(),
            }),
            "{unpinned} must be rejected as an unpinned server version"
        );
    }

    // Positive control through the same fixture: the exact pin resolves, so the
    // rejections above are about pinning and not about a broken fixture.
    let pinned = typescript_plan_with("6.0.0", digest);
    let config = pinned
        .resolve_downloaded_process(&root, &node, "v22.22.2", digest)
        .expect("the exact pin must resolve");
    assert_eq!(
        config.command,
        node.canonicalize().unwrap().to_string_lossy()
    );
    assert!(
        config.args[0].ends_with("cli.mjs"),
        "entrypoint argument was {}",
        config.args[0]
    );
    assert_eq!(config.args[1], "--stdio");

    // A Node older than the pinned engine constraint is still refused, so the
    // pin carries a runtime requirement and not only a name.
    assert!(matches!(
        pinned.resolve_downloaded_process(&root, &node, "v22.22.1", digest),
        Err(LspDownloadedArtifactResolveError::RuntimeTooOld { .. })
    ));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn resolver_rejects_a_checksum_that_is_not_a_sha256_digest() {
    let digest = legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE.checksum_sha256;
    let root = std::env::temp_dir().join(format!("legion-lsp-digest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("package/lib")).expect("package");
    std::fs::write(root.join("package/lib/cli.mjs"), b"entry").expect("entrypoint");
    let node = root.join("node");
    std::fs::write(&node, b"node").expect("node");

    let not_digests = [
        String::new(),
        "6e23b48e".to_string(),
        digest[..63].to_string(),
        format!("{digest}0"),
        format!("{}g", &digest[..63]),
        format!("{} ", &digest[..63]),
        format!("sha256:{}", &digest[7..]),
    ];
    for not_a_digest in &not_digests {
        let adapter = typescript_plan_with("6.0.0", not_a_digest);
        assert_eq!(
            adapter.resolve_downloaded_process(&root, &node, "v22.22.2", digest),
            Err(LspDownloadedArtifactResolveError::InvalidCatalogChecksum),
            "catalog checksum {not_a_digest:?} is not a SHA-256 digest"
        );
    }

    // The materializer receipt is held to the same rule, and is reported as a
    // receipt failure rather than a catalog failure.
    let adapter = typescript_plan_with("6.0.0", digest);
    assert_eq!(
        adapter.resolve_downloaded_process(&root, &node, "v22.22.2", "not-a-digest"),
        Err(LspDownloadedArtifactResolveError::InvalidChecksum)
    );
    // A well-formed receipt that names a different archive is a mismatch.
    assert_eq!(
        adapter.resolve_downloaded_process(&root, &node, "v22.22.2", &"f".repeat(64)),
        Err(LspDownloadedArtifactResolveError::ChecksumMismatch)
    );
    // Positive control: the real digest resolves.
    assert!(
        adapter
            .resolve_downloaded_process(&root, &node, "v22.22.2", digest)
            .is_ok()
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn no_tier_two_typescript_or_javascript_entry_resolves_by_bare_path_name() {
    let registry = LanguageServerAdapterRegistry::tier_two();
    let mut pinned = Vec::new();
    let mut path_resolved = Vec::new();
    for language in [
        "typescript",
        "javascript",
        "javascriptreact",
        "typescriptreact",
    ] {
        for adapter in registry.adapters_for_language(&LanguageId(language.to_string())) {
            match &adapter.binary_source {
                LspServerBinarySource::DownloadedArtifact { .. } => {
                    assert_pinned_typescript_source(&adapter.binary_source);
                    pinned.push(adapter.server_id.0);
                }
                LspServerBinarySource::SystemPath { binary_name } => {
                    path_resolved.push((adapter.server_id.0, binary_name.clone()));
                }
            }
        }
    }
    pinned.sort_unstable();
    assert_eq!(
        pinned,
        vec![102, 106, 107, 108],
        "servers 102, 106, 107 and 108 must each name a pinned artifact"
    );
    // Entry 103 is the one written-down exception: `tailwindcss-language-server`
    // has no retained artifact and no verified digest on this host, so it is
    // still resolved from PATH. Naming it here rather than allowing it with a
    // loose predicate means a future edit that turns 102, 106, 107 or 108 back
    // into a PATH lookup fails this test.
    assert_eq!(
        path_resolved,
        vec![(103, "tailwindcss-language-server".to_string())],
        "the only PATH-resolved TypeScript-family entry is the documented \
         tailwindcss exception"
    );
}

/// A version padded with surrounding whitespace is not an exact pin, and the
/// value the resolver reports is the value it validated.
///
/// `resolve_downloaded_process` used to validate `metadata.version.trim()`
/// while reporting `metadata.version` untrimmed, so `" 6.0.0 "` passed the pin
/// check here and then failed the byte-equality comparison in
/// `crates/legion-app/src/language/startup_authority.rs`
/// (`metadata.version != descriptor.version`) — a rejection a long way from its
/// cause. `metadata.package_name` carried the identical asymmetry: only
/// `trim().is_empty()` was checked here while the same authority compares it
/// byte for byte.
#[test]
fn whitespace_padded_version_is_not_accepted_as_an_exact_pin() {
    assert!(legion_lsp::is_exact_pinned_version("6.0.0"));
    for padded in [
        " 6.0.0",
        "6.0.0 ",
        " 6.0.0 ",
        "\t6.0.0",
        "6.0.0\n",
        "6.0.0\r\n",
    ] {
        assert!(
            !legion_lsp::is_exact_pinned_version(padded),
            "{padded:?} must not be accepted as an exact pinned release"
        );
    }

    let digest = legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE.checksum_sha256;
    let root = std::env::temp_dir().join(format!("legion-lsp-padded-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("package/lib")).expect("package");
    std::fs::write(root.join("package/lib/cli.mjs"), b"entry").expect("entrypoint");
    let node = root.join("node");
    std::fs::write(&node, b"node").expect("node");

    // Positive control through the same fixture: the unpadded pin resolves, so
    // the rejections below are about the padding and not a broken fixture.
    let pinned = typescript_plan_with("6.0.0", digest);
    pinned
        .resolve_downloaded_process(&root, &node, "v22.22.2", digest)
        .expect("the exact pin must resolve through this fixture");

    for padded in [" 6.0.0", "6.0.0 ", " 6.0.0 "] {
        let adapter = typescript_plan_with(padded, digest);
        assert_eq!(
            adapter.resolve_downloaded_process(&root, &node, "v22.22.2", digest),
            Err(LspDownloadedArtifactResolveError::UnpinnedPackageVersion {
                value: padded.to_string(),
            }),
            "{padded:?} must be rejected, and the reported value must be the \
             same bytes that were validated"
        );
    }

    // The same rule on `package_name`, and a padded name is still
    // distinguishable from an absent one.
    assert_eq!(
        plan_with_package_name(" typescript-language-server ", digest)
            .resolve_downloaded_process(&root, &node, "v22.22.2", digest),
        Err(LspDownloadedArtifactResolveError::PaddedPackageName {
            value: " typescript-language-server ".to_string(),
        })
    );
    assert_eq!(
        plan_with_package_name("   ", digest)
            .resolve_downloaded_process(&root, &node, "v22.22.2", digest),
        Err(LspDownloadedArtifactResolveError::EmptyPackageName)
    );
    plan_with_package_name("typescript-language-server", digest)
        .resolve_downloaded_process(&root, &node, "v22.22.2", digest)
        .expect("the unpadded package name must still resolve");

    std::fs::remove_dir_all(root).expect("cleanup");
}

/// Moving the pinned archive descriptors into `legion_lsp::pinned_archives`
/// must not move any caller's path: the crate-root name and the module path
/// resolve to the same item, and the identity itself survived byte for byte.
#[test]
fn pinned_archive_constants_are_reachable_from_the_crate_root_after_extraction() {
    let server: legion_lsp::LspPinnedArchive = legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE;
    let compiler: legion_lsp::LspPinnedArchive = legion_lsp::TYPESCRIPT_COMPILER_ARCHIVE;
    assert_eq!(
        server,
        legion_lsp::pinned_archives::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE
    );
    assert_eq!(
        compiler,
        legion_lsp::pinned_archives::TYPESCRIPT_COMPILER_ARCHIVE
    );
    assert_eq!(
        legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_POLICY_GATE,
        legion_lsp::pinned_archives::TYPESCRIPT_LANGUAGE_SERVER_POLICY_GATE
    );

    assert_eq!(server.package_name, "typescript-language-server");
    assert_eq!(server.version, "6.0.0");
    assert_eq!(
        server.archive_url,
        "https://registry.npmjs.org/typescript-language-server/-/typescript-language-server-6.0.0.tgz"
    );
    assert_eq!(
        server.checksum_sha256,
        "6e23b48efc76af4e70928cdfe62ea6e6cfef67ab4c1e7579c4e82dd284fbdfd2"
    );
    assert_eq!(server.archive_format, "tar.gz");
    assert_eq!(server.package_root, "package");
    assert_eq!(server.entrypoint, "lib/cli.mjs");
    assert_eq!(
        server.minimum_node,
        LspNodeVersion {
            major: 22,
            minor: 22,
            patch: 2,
        }
    );

    assert_eq!(compiler.package_name, "typescript");
    assert_eq!(compiler.version, "6.0.3");
    assert_eq!(
        compiler.archive_url,
        "https://registry.npmjs.org/typescript/-/typescript-6.0.3.tgz"
    );
    assert_eq!(
        compiler.checksum_sha256,
        "33cd0ee1beaa8c9e9d15a9da836c62ddea4c34a42d7c2d349dbc80d94165d22a"
    );
    assert_eq!(compiler.archive_format, "tar.gz");
    assert_eq!(compiler.package_root, "package");
    assert_eq!(compiler.entrypoint, "lib/tsserver.js");
    assert_eq!(
        compiler.minimum_node,
        LspNodeVersion {
            major: 14,
            minor: 17,
            patch: 0,
        }
    );

    assert_eq!(
        legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_POLICY_GATE,
        "policy://lsp-download/typescript-language-server"
    );

    // The moved predicate is still exported from the crate root and still
    // refuses a dist-tag.
    assert!(legion_lsp::is_exact_pinned_version(server.version));
    assert!(!legion_lsp::is_exact_pinned_version("latest"));

    // `LspPinnedArchive::metadata` moved with the type and still builds the
    // catalog metadata the registry registers its entries from.
    let metadata: LspDownloadedArtifactMetadata = server.metadata();
    assert_eq!(metadata.package_name, "typescript-language-server");
    assert_eq!(metadata.version, "6.0.0");
    assert_eq!(metadata.archive_format, "tar.gz");
    assert_eq!(metadata.package_root, std::path::Path::new("package"));
    assert_eq!(metadata.entrypoint, std::path::Path::new("lib/cli.mjs"));
    assert_eq!(
        metadata.runtime,
        LspArtifactRuntime::Node {
            minimum_version: LspNodeVersion {
                major: 22,
                minor: 22,
                patch: 2,
            }
        }
    );
}

/// Builds a TypeScript adapter that differs from the pinned catalog entry only
/// in its `metadata.package_name`, so a rejection is attributable to that
/// field.
fn plan_with_package_name(
    package_name: &str,
    checksum: &str,
) -> legion_lsp::LanguageServerAdapterPlan {
    let archive = legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE;
    let mut metadata = archive.metadata();
    metadata.package_name = package_name.to_string();
    legion_lsp::LanguageServerAdapterPlan::downloaded_package_artifact(
        legion_protocol::LanguageServerId(903),
        WorkspaceId(1),
        LanguageId("typescript".to_string()),
        "typescript-language-server (fixture)",
        archive.package_name,
        archive.archive_url,
        checksum,
        legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_POLICY_GATE,
        metadata,
        vec!["--stdio".to_string()],
        true,
    )
}

/// Builds a TypeScript adapter that differs from the pinned catalog entry only
/// in the fields under test, so a rejection is attributable to that field.
fn typescript_plan_with(version: &str, checksum: &str) -> legion_lsp::LanguageServerAdapterPlan {
    let archive = legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_ARCHIVE;
    let mut metadata = archive.metadata();
    metadata.version = version.to_string();
    legion_lsp::LanguageServerAdapterPlan::downloaded_package_artifact(
        legion_protocol::LanguageServerId(902),
        WorkspaceId(1),
        LanguageId("typescript".to_string()),
        "typescript-language-server (fixture)",
        archive.package_name,
        archive.archive_url,
        checksum,
        legion_lsp::TYPESCRIPT_LANGUAGE_SERVER_POLICY_GATE,
        metadata,
        vec!["--stdio".to_string()],
        true,
    )
}

fn assert_pinned_typescript_source(source: &LspServerBinarySource) {
    match source {
        LspServerBinarySource::DownloadedArtifact {
            binary_name,
            artifact_uri,
            checksum_sha256,
            policy_gate,
            metadata,
        } => {
            assert_eq!(binary_name, "typescript-language-server");
            assert_eq!(
                artifact_uri,
                "https://registry.npmjs.org/typescript-language-server/-/typescript-language-server-6.0.0.tgz"
            );
            assert_eq!(
                checksum_sha256,
                "6e23b48efc76af4e70928cdfe62ea6e6cfef67ab4c1e7579c4e82dd284fbdfd2"
            );
            assert_eq!(
                policy_gate,
                "policy://lsp-download/typescript-language-server"
            );
            assert_eq!(metadata.package_name, "typescript-language-server");
            assert_eq!(metadata.version, "6.0.0");
            assert_eq!(metadata.archive_format, "tar.gz");
            assert_eq!(metadata.package_root, std::path::Path::new("package"));
            assert_eq!(metadata.entrypoint, std::path::Path::new("lib/cli.mjs"));
            assert_eq!(
                metadata.runtime,
                LspArtifactRuntime::Node {
                    minimum_version: LspNodeVersion {
                        major: 22,
                        minor: 22,
                        patch: 2,
                    }
                }
            );
        }
        other => panic!("expected a pinned downloaded artifact, got {other:?}"),
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
