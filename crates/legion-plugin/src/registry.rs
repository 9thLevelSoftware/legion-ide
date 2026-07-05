//! Metadata-only signed extension registry.
//!
//! This registry is intentionally narrow: it tracks signed extension manifests
//! and exposes install/update/remove lifecycle operations without any runtime
//! or filesystem authority. Unsigned artifacts are rejected by default.

use std::collections::HashMap;

use legion_protocol::{PluginManifest, PluginTrustDecision};
use thiserror::Error;

/// Metadata-only registry for signed extension manifests.
#[derive(Debug, Default)]
pub struct SignedExtensionRegistry {
    installed: HashMap<String, PluginManifest>,
}

/// Fail-closed registry errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignedExtensionRegistryError {
    /// The manifest did not include a signature.
    #[error("unsigned extension artifacts are rejected")]
    UnsignedArtifact,
    /// The manifest trust metadata does not allow activation.
    #[error("extension artifact is not trusted")]
    UntrustedArtifact,
    /// The artifact is already installed.
    #[error("extension artifact is already installed")]
    AlreadyInstalled,
    /// The artifact is not currently installed.
    #[error("extension artifact is not installed")]
    NotInstalled,
}

impl SignedExtensionRegistry {
    /// Construct an empty signed extension registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Install a signed extension artifact.
    pub fn install(
        &mut self,
        manifest: PluginManifest,
    ) -> Result<(), SignedExtensionRegistryError> {
        self.validate_installable(&manifest)?;
        if self.installed.contains_key(&manifest.manifest_id) {
            return Err(SignedExtensionRegistryError::AlreadyInstalled);
        }
        self.installed
            .insert(manifest.manifest_id.clone(), manifest);
        Ok(())
    }

    /// Update an already installed signed extension artifact.
    pub fn update(&mut self, manifest: PluginManifest) -> Result<(), SignedExtensionRegistryError> {
        self.validate_installable(&manifest)?;
        if !self.installed.contains_key(&manifest.manifest_id) {
            return Err(SignedExtensionRegistryError::NotInstalled);
        }
        self.installed
            .insert(manifest.manifest_id.clone(), manifest);
        Ok(())
    }

    /// Remove an installed signed extension artifact by manifest id.
    pub fn remove(
        &mut self,
        manifest_id: &str,
    ) -> Result<PluginManifest, SignedExtensionRegistryError> {
        self.installed
            .remove(manifest_id)
            .ok_or(SignedExtensionRegistryError::NotInstalled)
    }

    /// Check whether a manifest is installable.
    pub fn is_installable(&self, manifest: &PluginManifest) -> bool {
        self.validate_installable(manifest).is_ok()
    }

    fn validate_installable(
        &self,
        manifest: &PluginManifest,
    ) -> Result<(), SignedExtensionRegistryError> {
        if manifest.signature.is_none() {
            return Err(SignedExtensionRegistryError::UnsignedArtifact);
        }
        if !matches!(
            manifest.trust.decision,
            PluginTrustDecision::Trusted | PluginTrustDecision::ExplicitlyAllowed
        ) {
            return Err(SignedExtensionRegistryError::UntrustedArtifact);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use legion_protocol::{
        CapabilityId, LanguageId, PluginActivationEvent, PluginCommandDescriptor,
        PluginContribution, PluginId, PluginManifest, PluginQuotaDeclaration,
        PluginSignatureMetadata, PluginStateNamespace, PluginTreeSitterGrammarContribution,
        PluginTrustDecision, PluginTrustMetadata, PluginTrustSource,
    };

    use super::{SignedExtensionRegistry, SignedExtensionRegistryError};

    fn signed_manifest(manifest_id: &str) -> PluginManifest {
        let plugin_id = PluginId(17);
        PluginManifest {
            plugin_id,
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
                plugin_id,
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

    fn unsigned_manifest(manifest_id: &str) -> PluginManifest {
        let mut manifest = signed_manifest(manifest_id);
        manifest.signature = None;
        manifest
    }

    #[test]
    fn signed_extension_registry_rejects_unsigned_artifacts_by_default() {
        let registry = SignedExtensionRegistry::new();
        assert!(!registry.is_installable(&unsigned_manifest("manifest-unsigned")));
        let mut registry = SignedExtensionRegistry::new();
        let error = registry
            .install(unsigned_manifest("manifest-unsigned"))
            .expect_err("unsigned artifacts should fail closed");
        assert_eq!(error, SignedExtensionRegistryError::UnsignedArtifact);
    }

    #[test]
    fn signed_extension_registry_supports_install_update_and_remove() {
        let mut registry = SignedExtensionRegistry::new();
        let manifest = signed_manifest("manifest-signed");

        registry
            .install(manifest.clone())
            .expect("signed artifact should install");
        assert!(registry.is_installable(&manifest));

        let updated = signed_manifest("manifest-signed");
        registry
            .update(updated.clone())
            .expect("installed artifact should update");

        let removed = registry
            .remove(&updated.manifest_id)
            .expect("installed artifact should remove");
        assert_eq!(removed.manifest_id, updated.manifest_id);
        assert!(matches!(
            registry.remove(&updated.manifest_id),
            Err(SignedExtensionRegistryError::NotInstalled)
        ));
    }
}
