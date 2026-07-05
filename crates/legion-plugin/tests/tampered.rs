use legion_plugin::{WasmPluginHost, registry::SignedExtensionRegistry};
use legion_protocol::{
    CapabilityId, LanguageId, PluginActivationEvent, PluginCommandDescriptor, PluginContribution,
    PluginId, PluginManifest, PluginQuotaDeclaration, PluginSignatureMetadata,
    PluginStateNamespace, PluginTreeSitterGrammarContribution, PluginTrustDecision,
    PluginTrustMetadata, PluginTrustSource,
};

fn signed_manifest(manifest_id: &str) -> PluginManifest {
    PluginManifest {
        plugin_id: PluginId(17),
        name: "signed.extension.fixture".to_string(),
        version: "1.0.0".to_string(),
        schema_version: 1,
        min_abi_version: 1,
        max_abi_version: 1,
        module_hash: "sha256:signed-extension-fixture".to_string(),
        manifest_id: manifest_id.to_string(),
        trust: PluginTrustMetadata {
            source: PluginTrustSource::ExplicitLocalAllow,
            decision: PluginTrustDecision::ExplicitlyAllowed,
            reason: "fixture".to_string(),
        },
        signature: Some(PluginSignatureMetadata {
            signer: "fixture-signer".to_string(),
            algorithm: "ed25519".to_string(),
            signature_digest: "sha256:signed-manifest".to_string(),
        }),
        activation_events: vec![PluginActivationEvent::OnCommand {
            command: "signed.extension.run".to_string(),
        }],
        contributions: vec![
            PluginContribution::Command(PluginCommandDescriptor {
                command_id: "signed.extension.run".to_string(),
                title: "Signed Extension Run".to_string(),
                required_capability: CapabilityId("plugin.command".to_string()),
            }),
            PluginContribution::TreeSitterGrammar(PluginTreeSitterGrammarContribution {
                language_id: LanguageId("rust-plugin".to_string()),
                grammar_name: "rust-plugin-grammar".to_string(),
                artifact_uri: "file:///tmp/rust-plugin-grammar.wasm".to_string(),
                artifact_hash: "sha256:rust-plugin-grammar".to_string(),
                required_capability: CapabilityId("plugin.grammar.tree_sitter".to_string()),
            }),
        ],
        requested_capabilities: vec![
            CapabilityId("plugin.command".to_string()),
            CapabilityId("plugin.grammar.tree_sitter".to_string()),
        ],
        storage_namespace: PluginStateNamespace {
            plugin_id: PluginId(17),
            namespace: "state".to_string(),
        },
        quotas: PluginQuotaDeclaration {
            max_fuel: 1_000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4_096,
            max_host_calls: 4,
            max_events: 4,
            max_output_bytes: 512,
        },
    }
}

fn tampered_manifest(manifest_id: &str) -> PluginManifest {
    let mut manifest = signed_manifest(manifest_id);
    manifest.trust.decision = PluginTrustDecision::ChecksumMismatch;
    manifest.trust.reason = "tampered artifact checksum mismatch".to_string();
    manifest
}

#[test]
fn tampered_artifact_is_rejected_before_fixture_loading() {
    let mut registry = SignedExtensionRegistry::new();
    let manifest = tampered_manifest("manifest-tampered");

    let error = registry
        .install(manifest.clone())
        .expect_err("tampered artifacts must be refused");
    assert_eq!(
        error,
        legion_plugin::registry::SignedExtensionRegistryError::UntrustedArtifact
    );
    assert!(!registry.is_installable(&manifest));

    let mut host = WasmPluginHost::new();
    let error = host
        .load_fixture(manifest, "/definitely/not/a/real/tampered-fixture.wasm")
        .expect_err("tampered artifacts must be rejected before file access or execution");
    assert_eq!(error.code, "plugin_trust_denied");
    assert_eq!(
        error.message,
        "plugin manifest is not trusted for activation"
    );
}
