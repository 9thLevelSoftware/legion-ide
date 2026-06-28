use legion_index::{
    DEFAULT_GRAMMAR_VERSION, DEFAULT_MODEL_VERSION, LEXICAL_EXTRACTION_VERSION, ParseRequest,
    ParserWorker, SourceDocument, TREE_SITTER_EXTRACTION_VERSION, TreeSitterParser,
    register_plugin_tree_sitter_grammars, reset_plugin_tree_sitter_grammar_registry_for_tests,
    tree_sitter_supports_language,
};
use legion_protocol::{
    CanonicalPath, FileContentVersion, FileId, LanguageId, PluginContribution, PluginId,
    PluginTreeSitterGrammarContribution, SemanticGrammarVersion, SemanticModelVersion,
    SemanticPrivacyScope, WorkspaceGeneration, WorkspaceId,
};

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
fn plugin_grammar_registry_marks_language_supported_but_uses_lexical_fallback() {
    reset_plugin_tree_sitter_grammar_registry_for_tests();

    let language_id = LanguageId("rust-plugin".to_string());
    assert!(!tree_sitter_supports_language(&language_id));

    let loaded = register_plugin_tree_sitter_grammars(
        PluginId(41),
        &[PluginContribution::TreeSitterGrammar(
            PluginTreeSitterGrammarContribution {
                language_id: language_id.clone(),
                grammar_name: "rust-plugin-grammar".to_string(),
                artifact_uri: "file:///tmp/rust-plugin-grammar.wasm".to_string(),
                artifact_hash: "sha256:rust-plugin-grammar".to_string(),
                required_capability: legion_protocol::CapabilityId(
                    "plugin.grammar.tree_sitter".to_string(),
                ),
            },
        )],
    );
    assert_eq!(loaded, 1);
    assert!(tree_sitter_supports_language(&language_id));

    let parser = TreeSitterParser::new();
    let outcome = parser
        .parse(ParseRequest {
            document: document("rust-plugin"),
            grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        })
        .expect("plugin grammar should parse via lexical fallback");

    // A registered plugin grammar has no loaded-grammar worker yet, so the parser must NOT
    // parse it as bundled Rust; it falls back to the lexical parser with a diagnostic.
    assert_eq!(
        outcome.syntax_tree.cache_key.parser_version,
        LEXICAL_EXTRACTION_VERSION
    );
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|diag| diag.code == "index.tree_sitter.plugin_grammar_unsupported"),
        "plugin-grammar fallback must emit a diagnostic"
    );
}

#[test]
fn bundled_rust_still_parses_via_tree_sitter() {
    reset_plugin_tree_sitter_grammar_registry_for_tests();

    let parser = TreeSitterParser::new();
    let outcome = parser
        .parse(ParseRequest {
            document: document("rust"),
            grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        })
        .expect("bundled rust should parse via tree-sitter path");

    assert_eq!(
        outcome.syntax_tree.cache_key.parser_version,
        TREE_SITTER_EXTRACTION_VERSION
    );
    assert!(outcome.syntax_tree.node_count > 0);
}
