//! App-owned extension catalog: verification, permission review, install.
//!
//! P7.F2.T1 and P7.F2.T2. This module is the only place in the product where an
//! extension moves from "offered" to "installed". It owns:
//!
//! * the [`SignedExtensionRegistry`] and its trust anchors,
//! * one [`ExtensionPermissionReview`] per catalog entry, and
//! * the projection the desktop extensions panel renders.
//!
//! The renderer holds none of this. It receives
//! [`legion_protocol::ExtensionCatalogEntry`] values and sends intents back;
//! every decision below is made here.
//!
//! ## Fail-closed properties this module is responsible for
//!
//! * An artifact with no signature is never installed. The refusal comes from
//!   [`SignedExtensionRegistry::verify`] before any capability is granted.
//! * An artifact whose bytes do not match its signature is never installed, and
//!   is never written to disk or handed to the wasm host.
//! * Permissions are granted one capability at a time. There is no method here
//!   that grants an extension wholesale.

use std::collections::BTreeMap;

use legion_plugin::{
    ExtensionKeyring, ExtensionPermissionDecision, ExtensionPermissionReview,
    SignedExtensionArtifact, SignedExtensionRegistry, SignedExtensionRegistryError,
    extension_artifact_digest, extension_verifying_key_b64, sign_extension_artifact,
};
use legion_protocol::{
    CapabilityId, ExtensionCatalogEntry, ExtensionInstallState, ExtensionPermissionProjection,
    ExtensionPermissionState, ExtensionSignatureState, LanguageId, PluginActivationEvent,
    PluginContribution, PluginId, PluginManifest, PluginQuotaDeclaration, PluginSignatureMetadata,
    PluginStateNamespace, PluginTreeSitterGrammarContribution, PluginTrustDecision,
    PluginTrustMetadata, PluginTrustSource,
};

/// Signer id for first-party bundled extensions.
pub const FIRST_PARTY_SIGNER: &str = "legion-first-party";

/// Development signing seed for the bundled first-party extension.
///
/// KNOWN LIMITATION (pre-GA): this seed is committed so the bundled artifact's
/// signature is reproducible from source and reviewable in a diff. It means a
/// developer build's first-party anchor can be forged locally. It does **not**
/// weaken any of the refusals this module is responsible for — unsigned and
/// tampered artifacts are refused identically either way — but GA must replace
/// this with a committed detached signature produced by the release signing
/// infrastructure (`xtask::signing`, ADR-0042) and delete the seed. The only
/// change required is to feed [`BundledExtension::signature_b64`] from a
/// constant instead of from [`sign_extension_artifact`].
const DEVELOPMENT_SIGNING_SEED: [u8; 32] = [
    0x4c, 0x65, 0x67, 0x69, 0x6f, 0x6e, 0x2d, 0x65, 0x78, 0x74, 0x65, 0x6e, 0x73, 0x69, 0x6f, 0x6e,
    0x2d, 0x64, 0x65, 0x76, 0x2d, 0x73, 0x65, 0x65, 0x64, 0x2d, 0x76, 0x31, 0x00, 0x00, 0x00, 0x01,
];

/// The bundled first-party grammar extension's artifact bytes.
///
/// A tree-sitter grammar description, shipped as data rather than as executable
/// code: the safest possible thing to make installable through the product UI
/// while the install path itself is what is being proven.
const BUNDLED_GRAMMAR_ARTIFACT: &[u8] =
    include_bytes!("../assets/extensions/legion-json-grammar/grammar.json");

/// One extension-catalog operation requested through app command routing.
///
/// The permission variant carries exactly one capability. Widening it to a list
/// would be the "trust this extension" toggle P7.F2.T2 forbids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionCatalogRequest {
    /// Decide exactly one capability for one extension.
    SetPermission {
        /// Manifest id being reviewed.
        manifest_id: String,
        /// The single capability decided.
        capability: CapabilityId,
        /// Whether that one capability was granted.
        granted: bool,
    },
    /// Install a verified, fully approved extension.
    Install {
        /// Manifest id to install.
        manifest_id: String,
    },
    /// Update an installed extension under a fresh approval.
    Update {
        /// Manifest id to update.
        manifest_id: String,
    },
    /// Remove an installed extension.
    Remove {
        /// Manifest id to remove.
        manifest_id: String,
    },
}

/// What changed in the extension catalog. Metadata only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionCatalogChange {
    /// One permission row was decided.
    PermissionRecorded {
        /// Manifest id reviewed.
        manifest_id: String,
        /// The capability decided.
        capability: String,
        /// Whether it was granted.
        granted: bool,
    },
    /// An extension was installed.
    Installed {
        /// Manifest id installed.
        manifest_id: String,
    },
    /// An extension was updated.
    Updated {
        /// Manifest id updated.
        manifest_id: String,
    },
    /// An extension was removed.
    Removed {
        /// Manifest id removed.
        manifest_id: String,
    },
}

impl ExtensionCatalogChange {
    /// Metadata-only status line for the desktop status bar.
    pub fn status_message(&self) -> String {
        match self {
            Self::PermissionRecorded {
                capability,
                granted,
                ..
            } => format!(
                "Extension permission {} for {capability}",
                if *granted { "granted" } else { "denied" }
            ),
            Self::Installed { manifest_id } => format!("Extension installed {manifest_id}"),
            Self::Updated { manifest_id } => format!("Extension updated {manifest_id}"),
            Self::Removed { manifest_id } => format!("Extension removed {manifest_id}"),
        }
    }
}

/// A catalog candidate: an artifact the product offers to install.
#[derive(Debug, Clone)]
pub struct ExtensionCandidate {
    /// Display name for the panel.
    pub display_name: String,
    /// The signed artifact envelope.
    pub artifact: SignedExtensionArtifact,
}

/// Errors surfaced by catalog operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExtensionCatalogError {
    /// No catalog entry with that manifest id.
    #[error("no extension catalog entry for `{manifest_id}`")]
    UnknownExtension {
        /// The unknown manifest id.
        manifest_id: String,
    },
    /// The extension does not request that capability.
    #[error("extension `{manifest_id}` does not request capability `{capability}`")]
    UnknownCapability {
        /// Manifest the decision targeted.
        manifest_id: String,
        /// Capability that has no review row.
        capability: String,
    },
    /// The registry refused the operation.
    #[error("{0}")]
    Registry(String),
}

impl From<SignedExtensionRegistryError> for ExtensionCatalogError {
    fn from(error: SignedExtensionRegistryError) -> Self {
        Self::Registry(error.to_string())
    }
}

/// The bundled first-party extension, assembled and signed at startup.
pub struct BundledExtension;

impl BundledExtension {
    /// Manifest id of the bundled grammar extension.
    pub const MANIFEST_ID: &'static str = "legion.bundled.json-grammar";

    /// Build the bundled extension's manifest.
    pub fn manifest() -> PluginManifest {
        let plugin_id = PluginId(1);
        PluginManifest {
            plugin_id,
            name: "Legion JSON Grammar".to_string(),
            version: "1.0.0".to_string(),
            schema_version: 1,
            min_abi_version: 1,
            max_abi_version: 1,
            module_hash: extension_artifact_digest(BUNDLED_GRAMMAR_ARTIFACT),
            manifest_id: Self::MANIFEST_ID.to_string(),
            trust: PluginTrustMetadata {
                source: PluginTrustSource::FirstParty,
                decision: PluginTrustDecision::Trusted,
                reason: "first-party bundled grammar".to_string(),
            },
            signature: Some(PluginSignatureMetadata {
                signer: FIRST_PARTY_SIGNER.to_string(),
                algorithm: legion_plugin::EXTENSION_SIGNATURE_ALGORITHM.to_string(),
                signature_digest: "detached".to_string(),
            }),
            activation_events: vec![PluginActivationEvent::Startup],
            contributions: vec![PluginContribution::TreeSitterGrammar(
                PluginTreeSitterGrammarContribution {
                    language_id: LanguageId("json".to_string()),
                    grammar_name: "legion-json-grammar".to_string(),
                    artifact_uri: "bundled:legion-json-grammar".to_string(),
                    artifact_hash: extension_artifact_digest(BUNDLED_GRAMMAR_ARTIFACT),
                    required_capability: CapabilityId("plugin.grammar.tree_sitter".to_string()),
                },
            )],
            requested_capabilities: vec![CapabilityId("plugin.grammar.tree_sitter".to_string())],
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

    /// The bundled extension's detached signature, base64 encoded.
    pub fn signature_b64() -> String {
        sign_extension_artifact(
            &Self::manifest(),
            BUNDLED_GRAMMAR_ARTIFACT,
            &DEVELOPMENT_SIGNING_SEED,
        )
    }

    /// The bundled extension as a catalog candidate.
    pub fn candidate() -> ExtensionCandidate {
        ExtensionCandidate {
            display_name: "Legion JSON Grammar".to_string(),
            artifact: SignedExtensionArtifact::new(
                Self::manifest(),
                BUNDLED_GRAMMAR_ARTIFACT.to_vec(),
                Self::signature_b64(),
            ),
        }
    }
}

/// App-owned catalog of extensions the product offers.
#[derive(Debug)]
pub struct ExtensionCatalog {
    registry: SignedExtensionRegistry,
    candidates: Vec<ExtensionCandidate>,
    reviews: BTreeMap<String, ExtensionPermissionReview>,
}

impl Default for ExtensionCatalog {
    fn default() -> Self {
        Self::with_bundled_extensions()
    }
}

impl ExtensionCatalog {
    /// Build a catalog with no candidates and the first-party trust anchor.
    pub fn empty() -> Self {
        Self {
            registry: SignedExtensionRegistry::with_keyring(ExtensionKeyring::new().with_anchor(
                FIRST_PARTY_SIGNER,
                extension_verifying_key_b64(&DEVELOPMENT_SIGNING_SEED),
            )),
            candidates: Vec::new(),
            reviews: BTreeMap::new(),
        }
    }

    /// Build the shipping catalog: the bundled first-party extension only.
    pub fn with_bundled_extensions() -> Self {
        let mut catalog = Self::empty();
        catalog.offer(BundledExtension::candidate());
        catalog
    }

    /// Add a candidate to the catalog and open an all-undecided review for it.
    pub fn offer(&mut self, candidate: ExtensionCandidate) {
        let manifest_id = candidate.artifact.manifest.manifest_id.clone();
        self.reviews.insert(
            manifest_id,
            ExtensionPermissionReview::for_manifest(&candidate.artifact.manifest),
        );
        self.candidates.push(candidate);
    }

    /// Record the user's decision for exactly one capability.
    ///
    /// One capability per call, by construction: there is no bulk grant.
    pub fn set_permission_decision(
        &mut self,
        manifest_id: &str,
        capability: &CapabilityId,
        granted: bool,
    ) -> Result<(), ExtensionCatalogError> {
        let review = self.reviews.get_mut(manifest_id).ok_or_else(|| {
            ExtensionCatalogError::UnknownExtension {
                manifest_id: manifest_id.to_string(),
            }
        })?;
        let decision = if granted {
            ExtensionPermissionDecision::Granted
        } else {
            ExtensionPermissionDecision::Denied
        };
        if !review.decide(capability, decision) {
            return Err(ExtensionCatalogError::UnknownCapability {
                manifest_id: manifest_id.to_string(),
                capability: capability.0.clone(),
            });
        }
        Ok(())
    }

    /// Install a catalog entry.
    ///
    /// Both proofs are demanded by the registry: a verified signature over the
    /// exact bytes, and an approval that only a fully granted itemised review
    /// can produce.
    pub fn install(&mut self, manifest_id: &str) -> Result<String, ExtensionCatalogError> {
        let (artifact, approval) = self.approved(manifest_id)?;
        self.registry.install(&artifact, &approval)?;
        Ok(manifest_id.to_string())
    }

    /// Update an installed catalog entry, under a fresh itemised approval.
    pub fn update(&mut self, manifest_id: &str) -> Result<String, ExtensionCatalogError> {
        let (artifact, approval) = self.approved(manifest_id)?;
        self.registry.update(&artifact, &approval)?;
        Ok(manifest_id.to_string())
    }

    /// Remove an installed extension.
    pub fn remove(&mut self, manifest_id: &str) -> Result<String, ExtensionCatalogError> {
        self.registry.remove(manifest_id)?;
        Ok(manifest_id.to_string())
    }

    /// Whether an extension is currently installed.
    pub fn is_installed(&self, manifest_id: &str) -> bool {
        self.registry.installed(manifest_id).is_some()
    }

    /// Apply one routed catalog request.
    ///
    /// This is the single entry point app command routing uses, so every
    /// renderer-originated extension operation funnels through the same
    /// verification and approval checks.
    pub fn apply(
        &mut self,
        request: ExtensionCatalogRequest,
    ) -> Result<ExtensionCatalogChange, ExtensionCatalogError> {
        match request {
            ExtensionCatalogRequest::SetPermission {
                manifest_id,
                capability,
                granted,
            } => {
                self.set_permission_decision(&manifest_id, &capability, granted)?;
                Ok(ExtensionCatalogChange::PermissionRecorded {
                    manifest_id,
                    capability: capability.0,
                    granted,
                })
            }
            ExtensionCatalogRequest::Install { manifest_id } => {
                self.install(&manifest_id)?;
                Ok(ExtensionCatalogChange::Installed { manifest_id })
            }
            ExtensionCatalogRequest::Update { manifest_id } => {
                self.update(&manifest_id)?;
                Ok(ExtensionCatalogChange::Updated { manifest_id })
            }
            ExtensionCatalogRequest::Remove { manifest_id } => {
                self.remove(&manifest_id)?;
                Ok(ExtensionCatalogChange::Removed { manifest_id })
            }
        }
    }

    /// Project the catalog for the extensions panel.
    pub fn projection(&self) -> Vec<ExtensionCatalogEntry> {
        self.candidates
            .iter()
            .map(|candidate| self.project_candidate(candidate))
            .collect()
    }

    fn approved(
        &self,
        manifest_id: &str,
    ) -> Result<
        (
            SignedExtensionArtifact,
            legion_plugin::ExtensionInstallApproval,
        ),
        ExtensionCatalogError,
    > {
        let candidate = self.candidate(manifest_id)?;
        let review = self.reviews.get(manifest_id).ok_or_else(|| {
            ExtensionCatalogError::UnknownExtension {
                manifest_id: manifest_id.to_string(),
            }
        })?;
        let approval = review
            .approval(&candidate.artifact.manifest)
            .map_err(|error| ExtensionCatalogError::Registry(error.to_string()))?;
        Ok((candidate.artifact.clone(), approval))
    }

    fn candidate(&self, manifest_id: &str) -> Result<&ExtensionCandidate, ExtensionCatalogError> {
        self.candidates
            .iter()
            .find(|candidate| candidate.artifact.manifest.manifest_id == manifest_id)
            .ok_or_else(|| ExtensionCatalogError::UnknownExtension {
                manifest_id: manifest_id.to_string(),
            })
    }

    fn project_candidate(&self, candidate: &ExtensionCandidate) -> ExtensionCatalogEntry {
        let manifest = &candidate.artifact.manifest;
        let manifest_id = manifest.manifest_id.clone();

        // The projection reports the *verified* posture, so a renderer never
        // sees an install affordance for an artifact the registry would refuse.
        let (signature_state, blocked_reason) = match self.registry.verify(&candidate.artifact) {
            Ok(verified) => (
                ExtensionSignatureState::VerifiedSigned {
                    signer: verified.signer().to_string(),
                },
                None,
            ),
            Err(SignedExtensionRegistryError::UnsignedArtifact) => (
                ExtensionSignatureState::Unsigned,
                Some("unsigned artifacts are refused".to_string()),
            ),
            Err(error) => (
                ExtensionSignatureState::VerificationFailed {
                    reason: error.to_string(),
                },
                Some(error.to_string()),
            ),
        };

        let installed = self.registry.installed(&manifest_id);
        let install_state = match installed {
            None => ExtensionInstallState::Available,
            Some(record) if record.manifest.version == manifest.version => {
                ExtensionInstallState::Installed
            }
            Some(_) => ExtensionInstallState::UpdateAvailable,
        };

        let review = self.reviews.get(&manifest_id);
        let permissions = legion_plugin::plugin_manifest_permission_review_rows(manifest)
            .into_iter()
            .map(|row| ExtensionPermissionProjection {
                ordinal: row.ordinal,
                title: row.title,
                reason: row.reason,
                risk_label: row.risk.label().to_string(),
                state: review
                    .and_then(|review| review.decision_for(&row.capability))
                    .map(|decision| match decision {
                        ExtensionPermissionDecision::Granted => ExtensionPermissionState::Granted,
                        ExtensionPermissionDecision::Denied => ExtensionPermissionState::Denied,
                        ExtensionPermissionDecision::Undecided => {
                            ExtensionPermissionState::Undecided
                        }
                    })
                    .unwrap_or(ExtensionPermissionState::Undecided),
                capability: row.capability,
            })
            .collect();

        ExtensionCatalogEntry {
            manifest_id,
            display_name: candidate.display_name.clone(),
            version: manifest.version.clone(),
            signature_state,
            install_state,
            permissions,
            blocked_reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant_all(catalog: &mut ExtensionCatalog, manifest_id: &str) {
        let capabilities: Vec<CapabilityId> = catalog
            .projection()
            .into_iter()
            .find(|entry| entry.manifest_id == manifest_id)
            .expect("entry exists")
            .permissions
            .into_iter()
            .map(|permission| permission.capability)
            .collect();
        for capability in capabilities {
            catalog
                .set_permission_decision(manifest_id, &capability, true)
                .expect("capability is reviewable");
        }
    }

    #[test]
    fn the_bundled_extension_verifies_and_is_offered_but_not_pre_installed() {
        let catalog = ExtensionCatalog::with_bundled_extensions();
        let entries = catalog.projection();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.manifest_id, BundledExtension::MANIFEST_ID);
        assert_eq!(
            entry.signature_state,
            ExtensionSignatureState::VerifiedSigned {
                signer: FIRST_PARTY_SIGNER.to_string()
            }
        );
        assert_eq!(entry.install_state, ExtensionInstallState::Available);
        assert_eq!(entry.blocked_reason, None);

        // Signed is not the same as approved: permissions start undecided.
        assert_eq!(entry.permissions.len(), 1);
        assert_eq!(
            entry.permissions[0].state,
            ExtensionPermissionState::Undecided
        );
        assert!(
            !entry.can_install(),
            "a signed extension still needs its permissions granted"
        );
    }

    #[test]
    fn granting_every_permission_installs_the_bundled_extension() {
        let mut catalog = ExtensionCatalog::with_bundled_extensions();
        grant_all(&mut catalog, BundledExtension::MANIFEST_ID);

        let entry = catalog.projection().remove(0);
        assert!(entry.can_install());

        catalog
            .install(BundledExtension::MANIFEST_ID)
            .expect("a signed, fully granted extension installs");
        assert!(catalog.is_installed(BundledExtension::MANIFEST_ID));

        let entry = catalog.projection().remove(0);
        assert_eq!(entry.install_state, ExtensionInstallState::Installed);
        assert!(entry.can_remove());

        catalog
            .remove(BundledExtension::MANIFEST_ID)
            .expect("an installed extension removes");
        assert!(!catalog.is_installed(BundledExtension::MANIFEST_ID));
    }

    #[test]
    fn install_is_refused_while_any_permission_is_undecided() {
        let mut catalog = ExtensionCatalog::with_bundled_extensions();
        let error = catalog
            .install(BundledExtension::MANIFEST_ID)
            .expect_err("an undecided review must not install");
        assert!(
            error.to_string().contains("undecided"),
            "unexpected error: {error}"
        );
        assert!(!catalog.is_installed(BundledExtension::MANIFEST_ID));
    }

    #[test]
    fn install_is_refused_when_a_permission_is_denied() {
        let mut catalog = ExtensionCatalog::with_bundled_extensions();
        catalog
            .set_permission_decision(
                BundledExtension::MANIFEST_ID,
                &CapabilityId("plugin.grammar.tree_sitter".to_string()),
                false,
            )
            .expect("capability is reviewable");
        let error = catalog
            .install(BundledExtension::MANIFEST_ID)
            .expect_err("a denied permission must not install");
        assert!(
            error.to_string().contains("denied"),
            "unexpected error: {error}"
        );
        assert!(!catalog.is_installed(BundledExtension::MANIFEST_ID));
    }

    /// P7.F2.T1 stop condition at the app layer.
    #[test]
    fn an_unsigned_candidate_is_never_installable_however_many_permissions_are_granted() {
        let mut catalog = ExtensionCatalog::empty();
        let mut manifest = BundledExtension::manifest();
        manifest.manifest_id = "unsigned.candidate".to_string();
        manifest.signature = None;
        catalog.offer(ExtensionCandidate {
            display_name: "Unsigned Candidate".to_string(),
            artifact: SignedExtensionArtifact::new(
                manifest,
                BUNDLED_GRAMMAR_ARTIFACT.to_vec(),
                String::new(),
            ),
        });
        grant_all(&mut catalog, "unsigned.candidate");

        let entry = catalog.projection().remove(0);
        assert_eq!(entry.signature_state, ExtensionSignatureState::Unsigned);
        assert!(entry.every_permission_granted());
        assert!(
            !entry.can_install(),
            "an unsigned artifact must stay uninstallable after full consent"
        );

        let error = catalog
            .install("unsigned.candidate")
            .expect_err("the install path itself must also refuse");
        assert!(
            error.to_string().contains("unsigned"),
            "unexpected error: {error}"
        );
        assert!(!catalog.is_installed("unsigned.candidate"));
    }

    /// P7.F2.T3 stop condition at the app layer.
    #[test]
    fn a_tampered_candidate_is_refused_not_warned_about() {
        let mut catalog = ExtensionCatalog::empty();
        let bundled = BundledExtension::candidate();
        let mut tampered_bytes = BUNDLED_GRAMMAR_ARTIFACT.to_vec();
        tampered_bytes.extend_from_slice(b"\n// injected");
        let mut manifest = bundled.artifact.manifest.clone();
        manifest.manifest_id = "tampered.candidate".to_string();
        manifest.module_hash = extension_artifact_digest(&tampered_bytes);
        catalog.offer(ExtensionCandidate {
            display_name: "Tampered Candidate".to_string(),
            artifact: SignedExtensionArtifact::new(
                manifest,
                tampered_bytes,
                bundled.artifact.signature_b64.clone(),
            ),
        });
        grant_all(&mut catalog, "tampered.candidate");

        let entry = catalog.projection().remove(0);
        assert!(matches!(
            entry.signature_state,
            ExtensionSignatureState::VerificationFailed { .. }
        ));
        assert!(entry.blocked_reason.is_some());
        assert!(!entry.can_install());

        let error = catalog
            .install("tampered.candidate")
            .expect_err("a tampered artifact must be refused");
        assert!(
            error.to_string().contains("signature verification failed"),
            "unexpected error: {error}"
        );
        assert!(!catalog.is_installed("tampered.candidate"));
    }

    #[test]
    fn a_decision_for_an_unrequested_capability_is_rejected() {
        let mut catalog = ExtensionCatalog::with_bundled_extensions();
        let error = catalog
            .set_permission_decision(
                BundledExtension::MANIFEST_ID,
                &CapabilityId("plugin.workspace.scanner".to_string()),
                true,
            )
            .expect_err("an unrequested capability has no review row");
        assert_eq!(
            error,
            ExtensionCatalogError::UnknownCapability {
                manifest_id: BundledExtension::MANIFEST_ID.to_string(),
                capability: "plugin.workspace.scanner".to_string(),
            }
        );
    }

    #[test]
    fn operations_on_an_unknown_extension_are_rejected() {
        let mut catalog = ExtensionCatalog::with_bundled_extensions();
        assert_eq!(
            catalog.install("nope").expect_err("unknown id"),
            ExtensionCatalogError::UnknownExtension {
                manifest_id: "nope".to_string()
            }
        );
        assert!(catalog.remove("nope").is_err());
    }
}
