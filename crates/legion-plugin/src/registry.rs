//! Signed extension registry: install / update / remove for extension artifacts.
//!
//! P7.F2.T1 and P7.F2.T3.
//!
//! This registry holds no runtime and no filesystem authority. It takes the
//! bytes that *would* be executed, verifies them, and only then records the
//! manifest as installed. Nothing here compiles, instantiates, or runs an
//! artifact, so a refusal issued by this module necessarily happens before any
//! extension code runs.
//!
//! Verification is fail-closed and reuses the one Ed25519 primitive in the
//! workspace (`legion_security::verify_ed25519_signature`, ADR-0042) rather
//! than inventing a second scheme. In order:
//!
//! 1. no signature at all -> [`SignedExtensionRegistryError::UnsignedArtifact`]
//! 2. non-Ed25519 algorithm -> [`SignedExtensionRegistryError::UnsupportedAlgorithm`]
//! 3. signer with no trust anchor -> [`SignedExtensionRegistryError::UnknownSigner`]
//! 4. artifact bytes that do not hash to `manifest.module_hash` ->
//!    [`SignedExtensionRegistryError::ArtifactChecksumMismatch`]
//! 5. signature that does not verify over the canonical payload ->
//!    [`SignedExtensionRegistryError::SignatureVerificationFailed`]
//! 6. trust metadata that does not permit activation ->
//!    [`SignedExtensionRegistryError::UntrustedArtifact`]
//!
//! Step 4 catches a swapped payload whose manifest was not updated; step 5
//! catches a swapped payload whose manifest *was* updated to match, which is
//! the realistic tamper and the one only a signature can refuse.

use std::collections::HashMap;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use legion_protocol::{PluginManifest, PluginTrustDecision};
use legion_security::{ed25519_verifying_key, sign_ed25519_detached, verify_ed25519_signature};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::manifest::{ExtensionInstallApproval, ExtensionPermissionReviewError};

/// Signature algorithm accepted for extension artifacts.
///
/// Deliberately the same string as [`legion_security::POLICY_BUNDLE_SIGNATURE_ALGORITHM`]:
/// release manifests, policy bundles, and extension artifacts all verify under
/// the single ADR-0042 Ed25519 scheme. Any other value is refused rather than
/// treated as "unsigned is fine".
pub const EXTENSION_SIGNATURE_ALGORITHM: &str = "ed25519";

/// Domain separation tag for the extension signing payload.
const EXTENSION_SIGNING_DOMAIN: &str = "legion.extension.artifact.v1";

/// One accepted extension signer.
///
/// Holds public key material only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionSigningKey {
    /// Signer id a manifest names in `signature.signer`.
    pub signer: String,
    /// Base64 (standard alphabet) encoding of the 32-byte Ed25519 public key.
    pub verifying_key_b64: String,
}

/// Trust anchors for extension signers.
///
/// An empty keyring denies everything: there is no "no anchors configured, so
/// allow" branch. This is the deny-by-default posture the rest of the product
/// uses for capability brokering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtensionKeyring {
    anchors: Vec<ExtensionSigningKey>,
}

impl ExtensionKeyring {
    /// Construct an empty keyring, which trusts no signer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one trust anchor.
    pub fn with_anchor(
        mut self,
        signer: impl Into<String>,
        verifying_key_b64: impl Into<String>,
    ) -> Self {
        self.anchors.push(ExtensionSigningKey {
            signer: signer.into(),
            verifying_key_b64: verifying_key_b64.into(),
        });
        self
    }

    /// Signer ids this keyring accepts.
    pub fn signers(&self) -> Vec<&str> {
        self.anchors
            .iter()
            .map(|anchor| anchor.signer.as_str())
            .collect()
    }

    fn anchor(&self, signer: &str) -> Option<&ExtensionSigningKey> {
        self.anchors.iter().find(|anchor| anchor.signer == signer)
    }
}

/// A candidate extension artifact presented for install.
///
/// Carries the bytes that would later be executed, so verification can bind the
/// signature to the real payload instead of to a self-reported label.
#[derive(Debug, Clone)]
pub struct SignedExtensionArtifact {
    /// Manifest describing the extension.
    pub manifest: PluginManifest,
    /// The artifact bytes (the wasm module) this manifest describes.
    pub artifact_bytes: Vec<u8>,
    /// Base64 (standard alphabet) detached Ed25519 signature over
    /// [`extension_signing_payload`].
    pub signature_b64: String,
}

impl SignedExtensionArtifact {
    /// Assemble an artifact envelope.
    pub fn new(
        manifest: PluginManifest,
        artifact_bytes: impl Into<Vec<u8>>,
        signature_b64: impl Into<String>,
    ) -> Self {
        Self {
            manifest,
            artifact_bytes: artifact_bytes.into(),
            signature_b64: signature_b64.into(),
        }
    }
}

/// Hex-encoded SHA-256 of `bytes`, prefixed `sha256:` to match manifest fields.
pub fn extension_artifact_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(7 + digest.len() * 2);
    out.push_str("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Canonical bytes an extension signature is computed over.
///
/// Deterministic, domain-separated, and length-prefixed on the capability list
/// so two different manifests cannot serialize to the same payload. Trust
/// metadata is deliberately excluded: trust is a local decision made by this
/// installation, not something a remote signer gets to assert. The signature
/// metadata itself is excluded because it is the output.
pub fn extension_signing_payload(manifest: &PluginManifest, artifact_bytes: &[u8]) -> Vec<u8> {
    let mut payload = String::new();
    payload.push_str(EXTENSION_SIGNING_DOMAIN);
    payload.push('\n');
    payload.push_str(&format!("manifest_id={}\n", manifest.manifest_id));
    payload.push_str(&format!("plugin_id={}\n", manifest.plugin_id.0));
    payload.push_str(&format!("name={}\n", manifest.name));
    payload.push_str(&format!("version={}\n", manifest.version));
    payload.push_str(&format!("schema_version={}\n", manifest.schema_version));
    payload.push_str(&format!("min_abi_version={}\n", manifest.min_abi_version));
    payload.push_str(&format!("max_abi_version={}\n", manifest.max_abi_version));
    payload.push_str(&format!("module_hash={}\n", manifest.module_hash));
    payload.push_str(&format!(
        "requested_capabilities={}\n",
        manifest.requested_capabilities.len()
    ));
    for capability in &manifest.requested_capabilities {
        payload.push_str(&format!("capability={}\n", capability.0));
    }
    payload.push_str(&format!(
        "artifact_digest={}\n",
        extension_artifact_digest(artifact_bytes)
    ));
    payload.into_bytes()
}

/// Sign an artifact with a raw 32-byte Ed25519 seed.
///
/// Test and packaging helper. The seed is borrowed and never retained; only the
/// public detached signature is returned.
pub fn sign_extension_artifact(
    manifest: &PluginManifest,
    artifact_bytes: &[u8],
    seed: &[u8; 32],
) -> String {
    let payload = extension_signing_payload(manifest, artifact_bytes);
    BASE64.encode(sign_ed25519_detached(&payload, seed))
}

/// Public trust anchor for an extension signing seed.
pub fn extension_verifying_key_b64(seed: &[u8; 32]) -> String {
    BASE64.encode(ed25519_verifying_key(seed))
}

/// Fail-closed registry errors. Every variant is a refusal, never a warning.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignedExtensionRegistryError {
    /// The manifest did not include a signature.
    #[error("unsigned extension artifacts are rejected")]
    UnsignedArtifact,
    /// The manifest named a signature algorithm this build does not accept.
    #[error("extension signature algorithm `{algorithm}` is not accepted")]
    UnsupportedAlgorithm {
        /// The rejected algorithm string.
        algorithm: String,
    },
    /// No trust anchor exists for the named signer.
    #[error("extension signer `{signer}` has no trust anchor")]
    UnknownSigner {
        /// The unknown signer id.
        signer: String,
    },
    /// The trust anchor was not a usable Ed25519 public key.
    #[error("extension trust anchor for `{signer}` is malformed: {reason}")]
    MalformedTrustAnchor {
        /// The signer whose anchor is broken.
        signer: String,
        /// Why the anchor could not be used.
        reason: String,
    },
    /// The detached signature could not be decoded.
    #[error("extension signature is malformed: {reason}")]
    MalformedSignature {
        /// Why the signature could not be decoded.
        reason: String,
    },
    /// The artifact bytes do not hash to the manifest's declared module hash.
    #[error("extension artifact digest `{actual}` does not match manifest `{expected}`")]
    ArtifactChecksumMismatch {
        /// Digest the manifest declared.
        expected: String,
        /// Digest the presented bytes actually produced.
        actual: String,
    },
    /// The signature does not verify over the canonical payload.
    #[error("extension signature verification failed: {reason}")]
    SignatureVerificationFailed {
        /// Why verification failed.
        reason: String,
    },
    /// The manifest trust metadata does not allow activation.
    #[error("extension artifact is not trusted")]
    UntrustedArtifact,
    /// The install was attempted without a complete per-capability review.
    #[error("extension permission review incomplete: {0}")]
    PermissionReview(#[from] ExtensionPermissionReviewError),
    /// The approval was issued for a different manifest.
    #[error("permission approval for `{expected}` does not cover `{actual}`")]
    ApprovalMismatch {
        /// Manifest the approval covers.
        expected: String,
        /// Manifest being installed.
        actual: String,
    },
    /// The artifact is already installed.
    #[error("extension artifact is already installed")]
    AlreadyInstalled,
    /// The artifact is not currently installed.
    #[error("extension artifact is not installed")]
    NotInstalled,
}

/// A verified artifact. Only produced by [`SignedExtensionRegistry::verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedExtensionArtifact {
    manifest_id: String,
    signer: String,
    artifact_digest: String,
}

impl VerifiedExtensionArtifact {
    /// Manifest id that verified.
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    /// Signer whose anchor validated the signature.
    pub fn signer(&self) -> &str {
        &self.signer
    }

    /// Digest of the exact bytes that verified.
    pub fn artifact_digest(&self) -> &str {
        &self.artifact_digest
    }
}

/// An installed extension record.
///
/// `PluginManifest` is not `PartialEq`, so this record deliberately is not
/// either; comparisons in tests go through the identifying fields.
#[derive(Debug, Clone)]
pub struct InstalledExtension {
    /// The manifest recorded at install time.
    pub manifest: PluginManifest,
    /// Digest of the verified bytes.
    pub artifact_digest: String,
    /// Signer whose anchor validated the artifact.
    pub signer: String,
    /// Capabilities the user individually granted during permission review.
    pub granted_capabilities: Vec<String>,
}

/// Registry for signed extension artifacts.
#[derive(Debug, Default)]
pub struct SignedExtensionRegistry {
    keyring: ExtensionKeyring,
    installed: HashMap<String, InstalledExtension>,
}

impl SignedExtensionRegistry {
    /// Construct a registry with no trust anchors, which trusts no signer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a registry over a keyring of accepted signers.
    pub fn with_keyring(keyring: ExtensionKeyring) -> Self {
        Self {
            keyring,
            installed: HashMap::new(),
        }
    }

    /// The trust anchors this registry accepts.
    pub fn keyring(&self) -> &ExtensionKeyring {
        &self.keyring
    }

    /// Verify an artifact without installing it.
    ///
    /// Pure byte inspection: no filesystem access, no wasm compilation, no
    /// instantiation. A failure here is a refusal that precedes any execution.
    pub fn verify(
        &self,
        artifact: &SignedExtensionArtifact,
    ) -> Result<VerifiedExtensionArtifact, SignedExtensionRegistryError> {
        let manifest = &artifact.manifest;

        // 1. Unsigned artifacts never install. This is the P7.F2.T1 stop
        //    condition and it is checked before anything else.
        let Some(signature) = manifest.signature.as_ref() else {
            return Err(SignedExtensionRegistryError::UnsignedArtifact);
        };

        // 2. Exactly one accepted scheme.
        if signature.algorithm != EXTENSION_SIGNATURE_ALGORITHM {
            return Err(SignedExtensionRegistryError::UnsupportedAlgorithm {
                algorithm: signature.algorithm.clone(),
            });
        }

        // 3. Deny-by-default signer lookup: an empty keyring accepts nobody.
        let Some(anchor) = self.keyring.anchor(&signature.signer) else {
            return Err(SignedExtensionRegistryError::UnknownSigner {
                signer: signature.signer.clone(),
            });
        };

        // 4. The presented bytes must be the bytes the manifest describes.
        let artifact_digest = extension_artifact_digest(&artifact.artifact_bytes);
        if artifact_digest != manifest.module_hash {
            return Err(SignedExtensionRegistryError::ArtifactChecksumMismatch {
                expected: manifest.module_hash.clone(),
                actual: artifact_digest,
            });
        }

        // 5. The signature must verify over the canonical payload. This is the
        //    guard a tamperer cannot satisfy by recomputing hashes.
        let key_bytes = BASE64
            .decode(anchor.verifying_key_b64.as_bytes())
            .map_err(|err| SignedExtensionRegistryError::MalformedTrustAnchor {
                signer: signature.signer.clone(),
                reason: err.to_string(),
            })?;
        let signature_bytes = BASE64
            .decode(artifact.signature_b64.as_bytes())
            .map_err(|err| SignedExtensionRegistryError::MalformedSignature {
                reason: err.to_string(),
            })?;
        let payload = extension_signing_payload(manifest, &artifact.artifact_bytes);
        verify_ed25519_signature(&payload, &signature_bytes, &key_bytes).map_err(
            |err| match err {
                legion_security::Ed25519VerifyFailure::InvalidKey(reason) => {
                    SignedExtensionRegistryError::MalformedTrustAnchor {
                        signer: signature.signer.clone(),
                        reason,
                    }
                }
                legion_security::Ed25519VerifyFailure::VerifyFailed(reason) => {
                    SignedExtensionRegistryError::SignatureVerificationFailed { reason }
                }
            },
        )?;

        // 6. Local trust posture has the last word even over a valid signature.
        if !matches!(
            manifest.trust.decision,
            PluginTrustDecision::Trusted | PluginTrustDecision::ExplicitlyAllowed
        ) {
            return Err(SignedExtensionRegistryError::UntrustedArtifact);
        }

        Ok(VerifiedExtensionArtifact {
            manifest_id: manifest.manifest_id.clone(),
            signer: signature.signer.clone(),
            artifact_digest: manifest.module_hash.clone(),
        })
    }

    /// Whether an artifact would verify. Never mutates state.
    pub fn is_installable(&self, artifact: &SignedExtensionArtifact) -> bool {
        self.verify(artifact).is_ok()
    }

    /// Install a verified artifact under an itemised permission approval.
    ///
    /// Both proofs are required: the [`ExtensionInstallApproval`] can only be
    /// produced by an [`crate::manifest::ExtensionPermissionReview`] in which
    /// every requested capability was individually granted (P7.F2.T2), and
    /// verification must succeed (P7.F2.T1, P7.F2.T3).
    pub fn install(
        &mut self,
        artifact: &SignedExtensionArtifact,
        approval: &ExtensionInstallApproval,
    ) -> Result<&InstalledExtension, SignedExtensionRegistryError> {
        let verified = self.verify(artifact)?;
        Self::check_approval(&artifact.manifest, approval)?;
        if self.installed.contains_key(&verified.manifest_id) {
            return Err(SignedExtensionRegistryError::AlreadyInstalled);
        }
        Ok(self.record(artifact, verified, approval))
    }

    /// Update an already installed artifact.
    ///
    /// A fresh approval is required: an update that requests new capabilities
    /// cannot ride in on the approval given for the previous version.
    pub fn update(
        &mut self,
        artifact: &SignedExtensionArtifact,
        approval: &ExtensionInstallApproval,
    ) -> Result<&InstalledExtension, SignedExtensionRegistryError> {
        let verified = self.verify(artifact)?;
        Self::check_approval(&artifact.manifest, approval)?;
        if !self.installed.contains_key(&verified.manifest_id) {
            return Err(SignedExtensionRegistryError::NotInstalled);
        }
        Ok(self.record(artifact, verified, approval))
    }

    /// Remove an installed artifact by manifest id.
    pub fn remove(
        &mut self,
        manifest_id: &str,
    ) -> Result<InstalledExtension, SignedExtensionRegistryError> {
        self.installed
            .remove(manifest_id)
            .ok_or(SignedExtensionRegistryError::NotInstalled)
    }

    /// Look up an installed extension.
    pub fn installed(&self, manifest_id: &str) -> Option<&InstalledExtension> {
        self.installed.get(manifest_id)
    }

    /// Installed extensions, ordered by manifest id for deterministic display.
    pub fn installed_extensions(&self) -> Vec<&InstalledExtension> {
        let mut records: Vec<&InstalledExtension> = self.installed.values().collect();
        records.sort_by(|left, right| left.manifest.manifest_id.cmp(&right.manifest.manifest_id));
        records
    }

    fn check_approval(
        manifest: &PluginManifest,
        approval: &ExtensionInstallApproval,
    ) -> Result<(), SignedExtensionRegistryError> {
        if approval.manifest_id() != manifest.manifest_id {
            return Err(SignedExtensionRegistryError::ApprovalMismatch {
                expected: approval.manifest_id().to_string(),
                actual: manifest.manifest_id.clone(),
            });
        }
        for capability in &manifest.requested_capabilities {
            if !approval.granted().contains(capability) {
                return Err(SignedExtensionRegistryError::PermissionReview(
                    ExtensionPermissionReviewError::Undecided {
                        capability: capability.0.clone(),
                    },
                ));
            }
        }
        Ok(())
    }

    fn record(
        &mut self,
        artifact: &SignedExtensionArtifact,
        verified: VerifiedExtensionArtifact,
        approval: &ExtensionInstallApproval,
    ) -> &InstalledExtension {
        let manifest_id = verified.manifest_id.clone();
        let record = InstalledExtension {
            manifest: artifact.manifest.clone(),
            artifact_digest: verified.artifact_digest,
            signer: verified.signer,
            granted_capabilities: approval
                .granted()
                .iter()
                .map(|capability| capability.0.clone())
                .collect(),
        };
        self.installed.insert(manifest_id.clone(), record);
        self.installed
            .get(&manifest_id)
            .expect("record just inserted must be present")
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

    use crate::manifest::{
        ExtensionInstallApproval, ExtensionPermissionDecision, ExtensionPermissionReview,
    };

    use super::{
        EXTENSION_SIGNATURE_ALGORITHM, ExtensionKeyring, SignedExtensionArtifact,
        SignedExtensionRegistry, SignedExtensionRegistryError, extension_artifact_digest,
        extension_verifying_key_b64, sign_extension_artifact,
    };

    const SEED: [u8; 32] = [7u8; 32];
    const OTHER_SEED: [u8; 32] = [9u8; 32];
    const ARTIFACT: &[u8] = b"\0asm\x01\0\0\0legion-grammar-extension";

    fn manifest_for(manifest_id: &str, artifact_bytes: &[u8]) -> PluginManifest {
        let plugin_id = PluginId(17);
        PluginManifest {
            plugin_id,
            name: "signed.extension.fixture".to_string(),
            version: "1.0.0".to_string(),
            schema_version: 1,
            min_abi_version: 1,
            max_abi_version: 1,
            module_hash: extension_artifact_digest(artifact_bytes),
            manifest_id: manifest_id.to_string(),
            trust: PluginTrustMetadata {
                source: PluginTrustSource::ExplicitLocalAllow,
                decision: PluginTrustDecision::ExplicitlyAllowed,
                reason: "fixture".to_string(),
            },
            signature: Some(PluginSignatureMetadata {
                signer: "legion-first-party".to_string(),
                algorithm: EXTENSION_SIGNATURE_ALGORITHM.to_string(),
                signature_digest: "detached".to_string(),
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

    fn signed_artifact(manifest_id: &str) -> SignedExtensionArtifact {
        let manifest = manifest_for(manifest_id, ARTIFACT);
        let signature = sign_extension_artifact(&manifest, ARTIFACT, &SEED);
        SignedExtensionArtifact::new(manifest, ARTIFACT.to_vec(), signature)
    }

    fn registry() -> SignedExtensionRegistry {
        SignedExtensionRegistry::with_keyring(
            ExtensionKeyring::new()
                .with_anchor("legion-first-party", extension_verifying_key_b64(&SEED)),
        )
    }

    fn approval(manifest: &PluginManifest) -> ExtensionInstallApproval {
        let mut review = ExtensionPermissionReview::for_manifest(manifest);
        for index in 0..review.rows().len() {
            review.decide_at(index, ExtensionPermissionDecision::Granted);
        }
        review.approval(manifest).expect("granted review approves")
    }

    #[test]
    fn signed_extension_registry_rejects_unsigned_artifacts_by_default() {
        let mut artifact = signed_artifact("manifest-unsigned");
        artifact.manifest.signature = None;
        let approval = approval(&artifact.manifest);

        let mut registry = registry();
        assert!(!registry.is_installable(&artifact));
        let error = registry
            .install(&artifact, &approval)
            .expect_err("unsigned artifacts must fail closed");
        assert_eq!(error, SignedExtensionRegistryError::UnsignedArtifact);
        assert!(registry.installed("manifest-unsigned").is_none());
    }

    #[test]
    fn signed_extension_registry_rejects_unknown_signers() {
        let artifact = signed_artifact("manifest-unknown-signer");
        let approval = approval(&artifact.manifest);

        // An empty keyring is deny-all, not allow-all.
        let mut empty = SignedExtensionRegistry::new();
        let error = empty
            .install(&artifact, &approval)
            .expect_err("an empty keyring must trust nobody");
        assert_eq!(
            error,
            SignedExtensionRegistryError::UnknownSigner {
                signer: "legion-first-party".to_string()
            }
        );
    }

    #[test]
    fn signed_extension_registry_rejects_non_ed25519_algorithms() {
        let mut artifact = signed_artifact("manifest-bad-algorithm");
        artifact
            .manifest
            .signature
            .as_mut()
            .expect("fixture is signed")
            .algorithm = "rsa-pkcs1".to_string();
        let approval = approval(&artifact.manifest);

        let mut registry = registry();
        let error = registry
            .install(&artifact, &approval)
            .expect_err("only the ADR-0042 scheme is accepted");
        assert_eq!(
            error,
            SignedExtensionRegistryError::UnsupportedAlgorithm {
                algorithm: "rsa-pkcs1".to_string()
            }
        );
    }

    #[test]
    fn signed_extension_registry_rejects_a_valid_signature_from_the_wrong_key() {
        let manifest = manifest_for("manifest-wrong-key", ARTIFACT);
        let signature = sign_extension_artifact(&manifest, ARTIFACT, &OTHER_SEED);
        let artifact = SignedExtensionArtifact::new(manifest.clone(), ARTIFACT.to_vec(), signature);
        let approval = approval(&manifest);

        let mut registry = registry();
        let error = registry
            .install(&artifact, &approval)
            .expect_err("a signature from an untrusted key must be refused");
        assert!(matches!(
            error,
            SignedExtensionRegistryError::SignatureVerificationFailed { .. }
        ));
    }

    #[test]
    fn signed_extension_registry_rejects_locally_untrusted_artifacts() {
        // Correctly signed by a trusted signer, but revoked locally. Local
        // trust posture wins: a signature is not a trust decision.
        let mut manifest = manifest_for("manifest-revoked", ARTIFACT);
        manifest.trust.decision = PluginTrustDecision::Revoked;
        manifest.trust.source = PluginTrustSource::Revoked;
        let signature = sign_extension_artifact(&manifest, ARTIFACT, &SEED);
        let artifact = SignedExtensionArtifact::new(manifest.clone(), ARTIFACT.to_vec(), signature);
        let approval = approval(&manifest);

        let mut registry = registry();
        let error = registry
            .install(&artifact, &approval)
            .expect_err("a revoked artifact must be refused despite a valid signature");
        assert_eq!(error, SignedExtensionRegistryError::UntrustedArtifact);
    }

    #[test]
    fn signed_extension_registry_refuses_install_without_itemised_approval() {
        let artifact = signed_artifact("manifest-no-approval");

        // A review where only one of the two capabilities was granted cannot
        // produce an approval at all, so an install cannot be attempted.
        let mut review = ExtensionPermissionReview::for_manifest(&artifact.manifest);
        review.decide_at(0, ExtensionPermissionDecision::Granted);
        assert!(review.approval(&artifact.manifest).is_err());

        // And an approval built for another manifest is refused at install.
        let other = signed_artifact("manifest-other");
        let foreign_approval = approval(&other.manifest);
        let mut registry = registry();
        let error = registry
            .install(&artifact, &foreign_approval)
            .expect_err("an approval for another manifest must not install this one");
        assert_eq!(
            error,
            SignedExtensionRegistryError::ApprovalMismatch {
                expected: "manifest-other".to_string(),
                actual: "manifest-no-approval".to_string(),
            }
        );
    }

    #[test]
    fn signed_extension_registry_supports_install_update_and_remove() {
        let artifact = signed_artifact("manifest-signed");
        let approval = approval(&artifact.manifest);

        let mut registry = registry();
        let installed = registry
            .install(&artifact, &approval)
            .expect("signed artifact should install");
        assert_eq!(installed.signer, "legion-first-party");
        assert_eq!(installed.granted_capabilities.len(), 2);
        assert_eq!(registry.installed_extensions().len(), 1);

        assert_eq!(
            registry
                .install(&artifact, &approval)
                .expect_err("a second install of the same manifest is refused"),
            SignedExtensionRegistryError::AlreadyInstalled
        );

        let updated_bytes = b"\0asm\x01\0\0\0legion-grammar-extension-v2".to_vec();
        let mut updated_manifest = manifest_for("manifest-signed", &updated_bytes);
        updated_manifest.version = "1.1.0".to_string();
        let updated_signature = sign_extension_artifact(&updated_manifest, &updated_bytes, &SEED);
        let updated = SignedExtensionArtifact::new(
            updated_manifest.clone(),
            updated_bytes.clone(),
            updated_signature,
        );
        let updated_approval = approval_for(&updated_manifest);
        let record = registry
            .update(&updated, &updated_approval)
            .expect("installed artifact should update");
        assert_eq!(record.manifest.version, "1.1.0");
        assert_eq!(
            record.artifact_digest,
            extension_artifact_digest(&updated_bytes)
        );

        let removed = registry
            .remove("manifest-signed")
            .expect("installed artifact should remove");
        assert_eq!(removed.manifest.manifest_id, "manifest-signed");
        assert_eq!(
            registry
                .remove("manifest-signed")
                .expect_err("removing twice is refused"),
            SignedExtensionRegistryError::NotInstalled
        );
        assert!(registry.installed_extensions().is_empty());
    }

    #[test]
    fn signed_extension_registry_refuses_update_for_uninstalled_artifacts() {
        let artifact = signed_artifact("manifest-not-installed");
        let approval = approval(&artifact.manifest);
        let mut registry = registry();
        assert_eq!(
            registry
                .update(&artifact, &approval)
                .expect_err("updating something never installed is refused"),
            SignedExtensionRegistryError::NotInstalled
        );
    }

    fn approval_for(manifest: &PluginManifest) -> ExtensionInstallApproval {
        approval(manifest)
    }
}
