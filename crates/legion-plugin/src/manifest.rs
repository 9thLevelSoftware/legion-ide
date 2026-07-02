//! Manifest-level permission review helpers for plugin installs.

use legion_protocol::{CapabilityId, PluginContribution, PluginManifest};

/// Build structured permission-review rows for a plugin manifest install prompt.
///
/// Each row is metadata-only and designed to be rendered directly in the desktop
/// extensions panel before install approval.
pub fn plugin_manifest_permission_review_rows(manifest: &PluginManifest) -> Vec<String> {
    manifest
        .requested_capabilities
        .iter()
        .enumerate()
        .map(|(index, capability)| {
            let reason = permission_reason_for_capability(manifest, capability)
                .unwrap_or_else(|| format!("requested capability {}", capability.0));
            format!(
                "permission review {}: capability={} reason={}",
                index + 1,
                capability.0,
                reason
            )
        })
        .collect()
}

fn permission_reason_for_capability(
    manifest: &PluginManifest,
    capability: &CapabilityId,
) -> Option<String> {
    manifest
        .contributions
        .iter()
        .find_map(|contribution| match contribution {
            PluginContribution::Command(command) if &command.required_capability == capability => {
                Some(format!("command {}", command.command_id))
            }
            PluginContribution::TreeSitterGrammar(grammar)
                if &grammar.required_capability == capability =>
            {
                Some(format!("tree-sitter grammar {}", grammar.grammar_name))
            }
            PluginContribution::Formatter(formatter)
                if formatter.command_id == capability.0 || capability.0 == "plugin.formatter" =>
            {
                Some(format!("formatter {}", formatter.command_id))
            }
            PluginContribution::LanguageProvider(provider)
                if capability.0 == "plugin.language.provider" =>
            {
                Some(format!("language provider {}", provider.provider_kind))
            }
            PluginContribution::LspRegistration(lsp)
                if capability.0 == "plugin.lsp.registration" =>
            {
                Some(format!("lsp registration {}", lsp.server_label))
            }
            PluginContribution::WorkspaceScanner(scanner)
                if capability.0 == "plugin.workspace.scanner" =>
            {
                Some(format!("workspace scanner {}", scanner.label))
            }
            PluginContribution::PassiveAiContextProvider(provider)
                if capability.0 == "plugin.ai.context" =>
            {
                Some(format!("passive ai context provider {}", provider.key))
            }
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use legion_protocol::{
        CapabilityId, LanguageId, PluginActivationEvent, PluginCommandDescriptor,
        PluginContribution, PluginId, PluginManifest, PluginQuotaDeclaration, PluginStateNamespace,
        PluginTreeSitterGrammarContribution, PluginTrustDecision, PluginTrustMetadata,
        PluginTrustSource,
    };

    use super::plugin_manifest_permission_review_rows;

    fn manifest() -> PluginManifest {
        let plugin_id = PluginId(7);
        PluginManifest {
            plugin_id,
            name: "phase8.desktop".to_string(),
            version: "0.1.0".to_string(),
            schema_version: 1,
            min_abi_version: 1,
            max_abi_version: 1,
            module_hash: "sha256:phase8:7".to_string(),
            manifest_id: "manifest:phase8:7".to_string(),
            trust: PluginTrustMetadata {
                source: PluginTrustSource::ExplicitLocalAllow,
                decision: PluginTrustDecision::ExplicitlyAllowed,
                reason: "desktop plugin management test allow".to_string(),
            },
            signature: None,
            activation_events: vec![PluginActivationEvent::OnCommand {
                command: "phase8.run".to_string(),
            }],
            contributions: vec![
                PluginContribution::Command(PluginCommandDescriptor {
                    command_id: "phase8.run".to_string(),
                    title: "Phase 8 Run".to_string(),
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
                plugin_id,
                namespace: "state".to_string(),
            },
            quotas: PluginQuotaDeclaration {
                max_fuel: 1000,
                max_wall_time_ms: 50,
                max_memory_pages: 8,
                max_storage_bytes: 4096,
                max_host_calls: 4,
                max_events: 4,
                max_output_bytes: 512,
            },
        }
    }

    #[test]
    fn plugin_manifest_permission_review_rows_are_structured() {
        let rows = plugin_manifest_permission_review_rows(&manifest());
        assert_eq!(rows.len(), 2);
        assert!(rows[0].contains("permission review 1"));
        assert!(rows[0].contains("capability=plugin.command"));
        assert!(rows[0].contains("reason=command phase8.run"));
        assert!(rows[1].contains("capability=plugin.grammar.tree_sitter"));
        assert!(rows[1].contains("reason=tree-sitter grammar rust-plugin-grammar"));
    }
}
