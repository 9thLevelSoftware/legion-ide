use legion_app::AppComposition;
use legion_index::{
    DEFAULT_GRAMMAR_VERSION, DEFAULT_MODEL_VERSION, ParseRequest, ParserWorker, SourceDocument,
    TREE_SITTER_EXTRACTION_VERSION, TreeSitterParser, tree_sitter_supports_language,
};
use legion_protocol::{
    CanonicalPath, CapabilityId, FileContentVersion, FileId, LanguageId, PluginActivationEvent,
    PluginCommandDescriptor, PluginContribution, PluginManifest, PluginQuotaDeclaration,
    PluginStateNamespace, PluginTreeSitterGrammarContribution, PluginTrustDecision,
    PluginTrustMetadata, PluginTrustSource, SemanticGrammarVersion, SemanticModelVersion,
    SemanticPrivacyScope, WorkspaceGeneration, WorkspaceId,
};

fn plugin_manifest(language_id: LanguageId) -> PluginManifest {
    PluginManifest {
        plugin_id: legion_protocol::PluginId(41),
        name: "phase5.grammar.test".to_string(),
        version: "0.1.0".to_string(),
        schema_version: 1,
        min_abi_version: 1,
        max_abi_version: 1,
        module_hash: "sha256:grammar-module".to_string(),
        manifest_id: "manifest:grammar:test".to_string(),
        trust: PluginTrustMetadata {
            source: PluginTrustSource::ExplicitLocalAllow,
            decision: PluginTrustDecision::ExplicitlyAllowed,
            reason: "grammar test allow".to_string(),
        },
        signature: None,
        activation_events: vec![PluginActivationEvent::Startup],
        contributions: vec![
            PluginContribution::Command(PluginCommandDescriptor {
                command_id: "phase5.run".to_string(),
                title: "Phase 5 Run".to_string(),
                required_capability: CapabilityId("plugin.command".to_string()),
            }),
            PluginContribution::TreeSitterGrammar(PluginTreeSitterGrammarContribution {
                language_id,
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
            plugin_id: legion_protocol::PluginId(41),
            namespace: "state".to_string(),
        },
        quotas: PluginQuotaDeclaration {
            max_fuel: 1000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4096,
            max_host_calls: 1,
            max_events: 4,
            max_output_bytes: 64,
        },
    }
}

fn document(language_id: &str) -> SourceDocument {
    SourceDocument::with_versions(
        WorkspaceId(7),
        FileId(11),
        CanonicalPath("/workspace/src/plugin_grammar.rs".to_string()),
        LanguageId(language_id.to_string()),
        FileContentVersion(1),
        WorkspaceGeneration(1),
        Some(legion_protocol::SnapshotId(101)),
        SemanticPrivacyScope::Workspace,
        "fn plugin_loaded() { println!(\"hello\"); }",
    )
}

#[test]
fn app_plugin_manifest_registers_tree_sitter_grammar_through_phase_five_channel() {
    let mut app = AppComposition::new();
    let language_id = LanguageId("rust-plugin-app-test".to_string());
    assert!(!tree_sitter_supports_language(&language_id));

    let plugin_id = app
        .load_plugin_manifest(plugin_manifest(language_id.clone()))
        .expect("plugin manifest should load");
    assert_eq!(plugin_id, legion_protocol::PluginId(41));
    assert!(tree_sitter_supports_language(&language_id));

    let parser = TreeSitterParser::new();
    let outcome = parser
        .parse(ParseRequest {
            document: document("rust-plugin-app-test"),
            grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        })
        .expect("plugin grammar should parse after app registration");

    assert_eq!(
        outcome.syntax_tree.cache_key.parser_version,
        TREE_SITTER_EXTRACTION_VERSION
    );
    assert!(outcome.syntax_tree.node_count > 0);
}
