//! Phase 8 raw-source retention fixture and production vault primitives.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, AeadCore, KeyInit, Payload},
};
use legion_protocol::{
    CanonicalPath, CausalityId, CorrelationId, EventSequence, FileFingerprint,
    HostedRetentionExportLinkage, HostedTelemetryEndpointDescriptor, RawSourceCaptureRequest,
    RawSourceHostedExportConsent, RawSourceKeyReference, RawSourceKeyRotationRecord,
    RawSourceRetentionAccessAudit, RawSourceRetentionBundleDescriptor,
    RawSourceRetentionConsentGrant, RawSourceRetentionLease, RawSourceRetentionPolicy,
    RawSourceRetentionPurpose, RawSourceRetentionTombstone, RawSourceVaultAlgorithm,
    RawSourceVaultEnvelope, RawSourceVaultRecoveryReport, RawSourceVaultRecoveryState,
    RedactionHint, TimestampMillis, WorkspaceId, validate_hosted_retention_export_linkage,
    validate_raw_source_capture_request, validate_raw_source_hosted_export_consent,
    validate_raw_source_key_reference, validate_raw_source_key_rotation_record,
    validate_raw_source_retention_access_audit, validate_raw_source_vault_envelope,
    validate_raw_source_vault_recovery_report,
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zeroize::Zeroizing;

pub mod privacy;
pub mod training;

const VAULT_FILE_MAGIC: &[u8; 4] = b"DVLT";
const VAULT_FILE_VERSION: u16 = 1;
const CHACHA20_POLY1305_ALGORITHM_ID: u8 = 1;
const CHACHA20_POLY1305_KEY_LEN: usize = 32;
const CHACHA20_POLY1305_NONCE_LEN: usize = 12;
const CHACHA20_POLY1305_TAG_LEN: usize = 16;

/// Retention fixture error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RetentionFixtureError {
    /// Retention fixture is disabled by policy.
    #[error("raw-source retention fixture is disabled")]
    Disabled,
    /// Capture request was rejected.
    #[error("raw-source capture denied: {reason}")]
    CaptureDenied {
        /// Denial reason.
        reason: String,
    },
    /// Bundle was not found.
    #[error("retention bundle not found: {bundle_id}")]
    BundleMissing {
        /// Bundle identifier.
        bundle_id: String,
    },
}

/// Raw-source vault error.
#[derive(Debug, Error)]
pub enum RawSourceVaultError {
    /// Vault capture/read/delete is disabled by policy.
    #[error("raw-source vault is disabled")]
    Disabled,
    /// Vault metadata was rejected.
    #[error("raw-source vault operation denied: {reason}")]
    Denied {
        /// Denial reason.
        reason: String,
    },
    /// Requested bundle was not found.
    #[error("raw-source vault bundle not found: {bundle_id}")]
    BundleMissing {
        /// Bundle identifier.
        bundle_id: String,
    },
    /// Filesystem operation failed.
    #[error("raw-source vault I/O failed: {message}")]
    Io {
        /// Failure details.
        message: String,
    },
    /// Authenticated encryption operation failed.
    #[error("raw-source vault cryptographic operation failed: {message}")]
    Crypto {
        /// Failure details.
        message: String,
    },
}

/// File payload supplied to the raw-source vault by an authorized caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSourceVaultFile {
    /// Canonical path covered by consent scope.
    pub path: CanonicalPath,
    /// Raw bytes to encrypt into the isolated vault.
    pub bytes: Vec<u8>,
}

/// Raw-source vault configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSourceVaultConfig {
    /// Whether capture/read/delete is enabled.
    pub enabled: bool,
    /// Maximum encrypted bytes per bundle.
    pub max_bundle_bytes: u64,
}

impl RawSourceVaultConfig {
    /// Return an enabled vault configuration.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

impl Default for RawSourceVaultConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_bundle_bytes: 5 * 1024 * 1024,
        }
    }
}

/// Key provider for isolated raw-source vault encryption.
pub trait RawSourceVaultKeyProvider {
    /// Return a metadata-only key reference.
    fn key_reference(&self) -> RawSourceKeyReference;
    /// Return key bytes used by the cipher implementation.
    fn key_bytes(&self) -> Vec<u8>;
}

/// OS keyring-backed raw-source vault key provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsKeyringRawSourceKeyProvider {
    service: String,
    account: String,
    key_reference: RawSourceKeyReference,
}

impl OsKeyringRawSourceKeyProvider {
    /// Construct an OS keyring provider from metadata-only key reference fields.
    pub fn new(
        service: impl Into<String>,
        account: impl Into<String>,
        key_reference: RawSourceKeyReference,
    ) -> Result<Self, RawSourceVaultError> {
        validate_raw_source_key_reference(&key_reference).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        Ok(Self {
            service: service.into(),
            account: account.into(),
            key_reference,
        })
    }

    /// Store or rotate a 256-bit vault key in the platform keyring.
    pub fn store_key(&self, key: &[u8]) -> Result<(), RawSourceVaultError> {
        if key.len() != CHACHA20_POLY1305_KEY_LEN {
            return Err(RawSourceVaultError::Denied {
                reason: "OS keyring raw-source keys must be 256-bit".to_string(),
            });
        }
        let encoded = lowercase_hex(key);
        keyring::Entry::new(&self.service, &self.account)
            .map_err(keyring_error)?
            .set_password(&encoded)
            .map_err(keyring_error)
    }

    /// Delete the current vault key from the platform keyring.
    pub fn delete_key(&self) -> Result<(), RawSourceVaultError> {
        keyring::Entry::new(&self.service, &self.account)
            .map_err(keyring_error)?
            .delete_credential()
            .map_err(keyring_error)
    }
}

impl RawSourceVaultKeyProvider for OsKeyringRawSourceKeyProvider {
    fn key_reference(&self) -> RawSourceKeyReference {
        self.key_reference.clone()
    }

    fn key_bytes(&self) -> Vec<u8> {
        let Ok(entry) = keyring::Entry::new(&self.service, &self.account) else {
            return Vec::new();
        };
        let Ok(encoded) = entry.get_password() else {
            return Vec::new();
        };
        decode_hex(&encoded).unwrap_or_default()
    }
}

/// Metadata-only wrapped key descriptor returned by KMS envelope providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSourceWrappedKey {
    /// Metadata-only key reference.
    pub key_reference: RawSourceKeyReference,
    /// Digest of wrapped key bytes, never wrapped key material.
    pub wrapped_key_digest: String,
    /// Wrapped key byte length.
    pub wrapped_key_byte_len: u64,
    /// Provider label.
    pub provider_label: String,
}

/// KMS envelope-provider contract. Cloud adapters are deployment supplied.
pub trait RawSourceKmsEnvelopeProvider {
    /// Wrap a local data key and return metadata-only wrapped-key evidence.
    fn wrap_key(
        &self,
        key_reference: RawSourceKeyReference,
        plaintext_key: &[u8],
    ) -> Result<RawSourceWrappedKey, RawSourceVaultError>;

    /// Unwrap a data key into memory for immediate AEAD use.
    fn unwrap_key(
        &self,
        wrapped: &RawSourceWrappedKey,
    ) -> Result<Zeroizing<Vec<u8>>, RawSourceVaultError>;
}

/// Hosted encrypted raw-source export request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSourceHostedExportRequest {
    /// Hosted raw-source export consent.
    pub consent: RawSourceHostedExportConsent,
    /// Allowlisted HTTPS endpoint.
    pub endpoint: HostedTelemetryEndpointDescriptor,
    /// Retained encrypted bundle descriptor.
    pub descriptor: RawSourceRetentionBundleDescriptor,
    /// Metadata-only vault envelope.
    pub envelope: RawSourceVaultEnvelope,
    /// Opaque encrypted bundle bytes.
    pub encrypted_bundle: Vec<u8>,
}

/// Hosted encrypted raw-source export acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSourceHostedExportAck {
    /// Hosted telemetry/export batch identifier used for linkage.
    pub telemetry_batch_id: String,
    /// Whether endpoint accepted the encrypted bundle.
    pub accepted: bool,
    /// Metadata-only status label.
    pub status: String,
}

/// Client abstraction for hosted encrypted raw-source export.
pub trait RawSourceHostedExportClient {
    /// Upload one encrypted raw-source bundle and return metadata-only acknowledgement.
    fn upload_encrypted_bundle(
        &mut self,
        request: &RawSourceHostedExportRequest,
    ) -> Result<RawSourceHostedExportAck, RawSourceVaultError>;
}

/// Sealed raw-source vault payload and metadata digests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSourceVaultSealedPayload {
    /// Opaque bytes persisted in the vault content file.
    pub bytes: Vec<u8>,
    /// SHA-256 digest of the nonce bytes.
    pub nonce_digest: String,
    /// SHA-256 digest of the persisted sealed file bytes.
    pub ciphertext_digest: FileFingerprint,
    /// SHA-256 digest of the AEAD authentication tag.
    pub tag_digest: String,
    /// Persisted sealed file byte length.
    pub encrypted_byte_len: u64,
}

/// Encryption abstraction for raw-source vault content.
pub trait RawSourceVaultCipher {
    /// Return the production algorithm represented by this cipher.
    fn algorithm(&self) -> RawSourceVaultAlgorithm;
    /// Encrypt plaintext with key bytes and additional authenticated data.
    fn encrypt(
        &self,
        key: &[u8],
        aad: &[u8],
        plaintext: &[u8],
    ) -> Result<RawSourceVaultSealedPayload, RawSourceVaultError>;
    /// Decrypt sealed payload bytes with key bytes and additional authenticated data.
    fn decrypt(
        &self,
        key: &[u8],
        aad: &[u8],
        sealed: &RawSourceVaultSealedPayload,
    ) -> Result<Vec<u8>, RawSourceVaultError>;
}

/// Production ChaCha20-Poly1305 vault cipher.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChaCha20Poly1305VaultCipher;

impl RawSourceVaultCipher for ChaCha20Poly1305VaultCipher {
    fn algorithm(&self) -> RawSourceVaultAlgorithm {
        RawSourceVaultAlgorithm::ChaCha20Poly1305
    }

    fn encrypt(
        &self,
        key: &[u8],
        aad: &[u8],
        plaintext: &[u8],
    ) -> Result<RawSourceVaultSealedPayload, RawSourceVaultError> {
        let cipher = chacha20poly1305_cipher(key)?;
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext_and_tag = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| crypto_error("AEAD encryption failed"))?;
        if ciphertext_and_tag.len() < CHACHA20_POLY1305_TAG_LEN {
            return Err(crypto_error("AEAD tag was not produced"));
        }

        let tag = &ciphertext_and_tag[ciphertext_and_tag.len() - CHACHA20_POLY1305_TAG_LEN..];
        let mut bytes = Vec::with_capacity(
            VAULT_FILE_MAGIC.len() + 2 + 2 + CHACHA20_POLY1305_NONCE_LEN + ciphertext_and_tag.len(),
        );
        bytes.extend_from_slice(VAULT_FILE_MAGIC);
        bytes.extend_from_slice(&VAULT_FILE_VERSION.to_le_bytes());
        bytes.push(CHACHA20_POLY1305_ALGORITHM_ID);
        bytes.push(nonce.len() as u8);
        bytes.extend_from_slice(&nonce);
        bytes.extend_from_slice(&ciphertext_and_tag);

        Ok(RawSourceVaultSealedPayload {
            nonce_digest: sha256_label(&nonce),
            ciphertext_digest: sha256_fingerprint(&bytes),
            tag_digest: sha256_label(tag),
            encrypted_byte_len: bytes.len() as u64,
            bytes,
        })
    }

    fn decrypt(
        &self,
        key: &[u8],
        aad: &[u8],
        sealed: &RawSourceVaultSealedPayload,
    ) -> Result<Vec<u8>, RawSourceVaultError> {
        let cipher = chacha20poly1305_cipher(key)?;
        let parsed = parse_sealed_vault_file(&sealed.bytes)?;
        if parsed.algorithm_id != CHACHA20_POLY1305_ALGORITHM_ID {
            return Err(crypto_error("unsupported vault cipher algorithm"));
        }
        let nonce = Nonce::from_slice(parsed.nonce);
        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: parsed.ciphertext_and_tag,
                    aad,
                },
            )
            .map_err(|_| crypto_error("AEAD authentication failed"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistedVaultIndex {
    schema_version: u16,
    bundles: HashMap<String, RawSourceRetentionBundleDescriptor>,
    tombstones: HashMap<String, RawSourceRetentionTombstone>,
    key_references: HashMap<String, String>,
    #[serde(default)]
    envelopes: HashMap<String, RawSourceVaultEnvelope>,
    #[serde(default)]
    lease_expirations: HashMap<String, TimestampMillis>,
}

/// Deterministic metadata-only retention fixture vault.
#[derive(Debug, Clone)]
pub struct RetentionFixtureVault {
    policy: RawSourceRetentionPolicy,
    bundles: HashMap<String, RawSourceRetentionBundleDescriptor>,
    tombstones: HashMap<String, RawSourceRetentionTombstone>,
}

impl RetentionFixtureVault {
    /// Construct a fixture vault with explicit raw-source retention policy.
    pub fn new(policy: RawSourceRetentionPolicy) -> Self {
        Self {
            policy,
            bundles: HashMap::new(),
            tombstones: HashMap::new(),
        }
    }

    /// Return current bundle count.
    pub fn bundle_count(&self) -> usize {
        self.bundles.len()
    }

    /// Return a reference to a retained bundle descriptor by id, or `None` if not found.
    pub fn lookup_bundle(&self, bundle_id: &str) -> Option<&RawSourceRetentionBundleDescriptor> {
        self.bundles.get(bundle_id)
    }

    /// Capture a descriptor-only deterministic bundle without storing raw source content.
    pub fn capture_descriptor(
        &mut self,
        grant: RawSourceRetentionConsentGrant,
        request: RawSourceCaptureRequest,
    ) -> Result<(RawSourceRetentionLease, RawSourceRetentionBundleDescriptor), RetentionFixtureError>
    {
        if !self.policy.capture_enabled {
            return Err(RetentionFixtureError::Disabled);
        }
        validate_raw_source_capture_request(&self.policy, &grant, &request).map_err(|err| {
            RetentionFixtureError::CaptureDenied {
                reason: err.message,
            }
        })?;
        // Lease expiry is an absolute epoch-ms timestamp: the configured TTL
        // measured from now, clamped to never outlive the consent grant.
        let now_ms = legion_protocol::TimestampMillis::now().0;
        let expires_at = legion_protocol::TimestampMillis(
            now_ms
                .saturating_add(self.policy.ttl_ms)
                .min(grant.expires_at.0),
        );
        let lease = RawSourceRetentionLease {
            lease_id: format!(
                "lease:{}:{}",
                request.workspace_id.0, request.correlation_id.0
            ),
            consent: grant,
            expires_at,
            schema_version: 1,
        };
        let descriptor = RawSourceRetentionBundleDescriptor {
            bundle_id: format!(
                "bundle:{}:{}",
                request.workspace_id.0, request.correlation_id.0
            ),
            lease_id: lease.lease_id.clone(),
            workspace_id: request.workspace_id,
            purpose: request.purpose,
            encrypted_byte_len: request.max_bytes,
            integrity: FileFingerprint {
                algorithm: "fixture-sha256".to_string(),
                value: format!("{}:{}", request.workspace_id.0, request.max_bytes),
            },
            schema_version: 1,
        };
        self.bundles
            .insert(descriptor.bundle_id.clone(), descriptor.clone());
        Ok((lease, descriptor))
    }

    /// Record metadata-only access to a retained bundle descriptor.
    pub fn audit_access(
        &self,
        audit: RawSourceRetentionAccessAudit,
    ) -> Result<RawSourceRetentionAccessAudit, RetentionFixtureError> {
        validate_raw_source_retention_access_audit(&audit).map_err(|err| {
            RetentionFixtureError::CaptureDenied {
                reason: err.message,
            }
        })?;
        if !self.bundles.contains_key(&audit.bundle_id) {
            return Err(RetentionFixtureError::BundleMissing {
                bundle_id: audit.bundle_id,
            });
        }
        Ok(audit)
    }

    /// Delete a bundle descriptor and persist a metadata-only tombstone.
    pub fn delete_bundle(
        &mut self,
        tombstone: RawSourceRetentionTombstone,
    ) -> Result<RawSourceRetentionTombstone, RetentionFixtureError> {
        if tombstone.bundle_id.trim().is_empty()
            || tombstone.reason.trim().is_empty()
            || tombstone.event_sequence.0 == 0
            || tombstone.correlation_id.0 == 0
            || tombstone.causality_id.0.is_nil()
            || tombstone.schema_version == 0
        {
            return Err(RetentionFixtureError::CaptureDenied {
                reason: "retention tombstone metadata is invalid".to_string(),
            });
        }
        if !self.bundles.contains_key(&tombstone.bundle_id) {
            return Err(RetentionFixtureError::BundleMissing {
                bundle_id: tombstone.bundle_id,
            });
        }
        self.bundles.remove(&tombstone.bundle_id);
        self.tombstones
            .insert(tombstone.bundle_id.clone(), tombstone.clone());
        Ok(tombstone)
    }
}

/// Unified deletion/read interface for privacy inspector wiring.
///
/// Both `RetentionFixtureVault` and `FileBackedRawSourceVault` implement this
/// trait so callers can wire privacy-inspector deletion handles without
/// depending on a concrete vault type.
pub trait RawSourceVault {
    /// Delete a retained bundle and record a metadata-only tombstone.
    fn vault_delete_bundle(
        &mut self,
        tombstone: RawSourceRetentionTombstone,
    ) -> Result<RawSourceRetentionTombstone, RawSourceVaultError>;

    /// Read a retained bundle descriptor by id.
    fn vault_read_bundle_descriptor(
        &self,
        bundle_id: &str,
    ) -> Result<RawSourceRetentionBundleDescriptor, RawSourceVaultError>;
}

impl RawSourceVault for RetentionFixtureVault {
    fn vault_delete_bundle(
        &mut self,
        tombstone: RawSourceRetentionTombstone,
    ) -> Result<RawSourceRetentionTombstone, RawSourceVaultError> {
        self.delete_bundle(tombstone).map_err(|err| match err {
            RetentionFixtureError::Disabled => RawSourceVaultError::Disabled,
            RetentionFixtureError::CaptureDenied { reason } => {
                RawSourceVaultError::Denied { reason }
            }
            RetentionFixtureError::BundleMissing { bundle_id } => {
                RawSourceVaultError::BundleMissing { bundle_id }
            }
        })
    }

    fn vault_read_bundle_descriptor(
        &self,
        bundle_id: &str,
    ) -> Result<RawSourceRetentionBundleDescriptor, RawSourceVaultError> {
        self.lookup_bundle(bundle_id)
            .cloned()
            .ok_or_else(|| RawSourceVaultError::BundleMissing {
                bundle_id: bundle_id.to_string(),
            })
    }
}

impl<K: RawSourceVaultKeyProvider, C: RawSourceVaultCipher> RawSourceVault
    for FileBackedRawSourceVault<K, C>
{
    fn vault_delete_bundle(
        &mut self,
        tombstone: RawSourceRetentionTombstone,
    ) -> Result<RawSourceRetentionTombstone, RawSourceVaultError> {
        self.delete_bundle(tombstone)
    }

    fn vault_read_bundle_descriptor(
        &self,
        bundle_id: &str,
    ) -> Result<RawSourceRetentionBundleDescriptor, RawSourceVaultError> {
        self.read_bundle_descriptor(bundle_id)
    }
}

/// File-backed encrypted raw-source vault.
#[derive(Debug)]
pub struct FileBackedRawSourceVault<K, C> {
    root: PathBuf,
    policy: RawSourceRetentionPolicy,
    config: RawSourceVaultConfig,
    key_provider: K,
    cipher: C,
    index: PersistedVaultIndex,
}

impl<K: RawSourceVaultKeyProvider, C: RawSourceVaultCipher> FileBackedRawSourceVault<K, C> {
    /// Open an encrypted raw-source vault rooted at `root`.
    pub fn open(
        root: impl AsRef<Path>,
        policy: RawSourceRetentionPolicy,
        config: RawSourceVaultConfig,
        key_provider: K,
        cipher: C,
    ) -> Result<Self, RawSourceVaultError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root).map_err(io_error)?;
        let index_path = root.join("index.json");
        let index = if index_path.exists() {
            let text = fs::read_to_string(&index_path).map_err(io_error)?;
            match serde_json::from_str(&text) {
                Ok(index) => index,
                Err(err) => {
                    // A corrupt index is preserved (quarantined) and the open
                    // fails closed rather than silently starting with empty
                    // metadata that would orphan retained .vault files.
                    let quarantine =
                        root.join(format!("index.json.corrupt-{}", TimestampMillis::now().0));
                    let _ = fs::rename(&index_path, &quarantine);
                    return Err(RawSourceVaultError::Io {
                        message: format!(
                            "decode vault index: {err}; corrupt index quarantined to {}",
                            quarantine.display()
                        ),
                    });
                }
            }
        } else {
            PersistedVaultIndex {
                schema_version: 1,
                ..PersistedVaultIndex::default()
            }
        };
        Ok(Self {
            root,
            policy,
            config,
            key_provider,
            cipher,
            index,
        })
    }

    /// Capture raw-source files into encrypted vault content and descriptor metadata.
    pub fn capture_bundle(
        &mut self,
        grant: RawSourceRetentionConsentGrant,
        request: RawSourceCaptureRequest,
        files: Vec<RawSourceVaultFile>,
    ) -> Result<(RawSourceRetentionLease, RawSourceRetentionBundleDescriptor), RawSourceVaultError>
    {
        if !self.config.enabled || !self.policy.capture_enabled {
            return Err(RawSourceVaultError::Disabled);
        }
        validate_raw_source_capture_request(&self.policy, &grant, &request).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        let plaintext = Zeroizing::new(self.pack_files(&request, files)?);
        if plaintext.is_empty() || plaintext.len() as u64 > self.config.max_bundle_bytes {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source bundle is empty or exceeds configured vault limit".to_string(),
            });
        }
        let bundle_id = format!(
            "bundle:{}:{}",
            request.workspace_id.0, request.correlation_id.0
        );
        if self.index.bundles.contains_key(&bundle_id) {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source vault bundle id already exists".to_string(),
            });
        }
        let key_reference = self.key_provider.key_reference();
        validate_raw_source_key_reference(&key_reference).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        let key = self.key_bytes()?;
        // Lease expiry is an absolute epoch-ms timestamp: the configured TTL
        // measured from now, clamped to never outlive the consent grant.
        let now_ms = TimestampMillis::now().0;
        let expires_at = TimestampMillis(
            now_ms
                .saturating_add(self.policy.ttl_ms)
                .min(grant.expires_at.0),
        );
        let lease = RawSourceRetentionLease {
            lease_id: format!(
                "lease:{}:{}",
                request.workspace_id.0, request.correlation_id.0
            ),
            consent: grant,
            expires_at,
            schema_version: 1,
        };
        let aad = vault_aad(
            &bundle_id,
            &lease.lease_id,
            request.workspace_id,
            request.purpose,
            self.cipher.algorithm(),
            &key_reference,
        );
        let sealed = self.cipher.encrypt(&key, &aad, &plaintext)?;
        let envelope = RawSourceVaultEnvelope {
            bundle_id: bundle_id.clone(),
            workspace_id: request.workspace_id,
            purpose: request.purpose,
            algorithm: self.cipher.algorithm(),
            key_reference: key_reference.clone(),
            nonce_digest: sealed.nonce_digest.clone(),
            ciphertext_digest: sealed.ciphertext_digest.clone(),
            tag_digest: sealed.tag_digest.clone(),
            aad_digest: sha256_label(&aad),
            encrypted_byte_len: sealed.encrypted_byte_len,
            schema_version: 1,
        };
        validate_raw_source_vault_envelope(&envelope).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        if sealed.encrypted_byte_len > self.config.max_bundle_bytes {
            return Err(RawSourceVaultError::Denied {
                reason: "encrypted raw-source bundle exceeds configured vault limit".to_string(),
            });
        }
        let descriptor = RawSourceRetentionBundleDescriptor {
            bundle_id: bundle_id.clone(),
            lease_id: lease.lease_id.clone(),
            workspace_id: request.workspace_id,
            purpose: request.purpose,
            encrypted_byte_len: sealed.encrypted_byte_len,
            integrity: sealed.ciphertext_digest,
            schema_version: 1,
        };

        // Stage the ciphertext to a temp file and the metadata in memory, then
        // persist the index first. The ciphertext is only published (renamed
        // into place) after the index is durable, so a flush failure cannot
        // leave an orphaned .vault file unreferenced by the persisted index.
        let bundle_path = self.bundle_path(&bundle_id);
        let temp_path = self
            .root
            .join(format!("{}.vault.tmp", safe_name(&bundle_id)));
        fs::write(&temp_path, &sealed.bytes).map_err(io_error)?;

        self.index
            .key_references
            .insert(bundle_id.clone(), key_reference.key_id.clone());
        self.index.envelopes.insert(bundle_id.clone(), envelope);
        self.index
            .lease_expirations
            .insert(bundle_id.clone(), lease.expires_at);
        self.index
            .bundles
            .insert(bundle_id.clone(), descriptor.clone());

        if let Err(err) = self.flush_index() {
            self.revert_bundle_index(&bundle_id);
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }
        if let Err(err) = fs::rename(&temp_path, &bundle_path) {
            // The index was persisted but the ciphertext could not be published;
            // roll back so we never keep a descriptor referencing a missing file.
            self.revert_bundle_index(&bundle_id);
            let _ = self.flush_index();
            let _ = fs::remove_file(&temp_path);
            return Err(io_error(err));
        }
        Ok((lease, descriptor))
    }

    /// Read a retained bundle descriptor by id.
    pub fn read_bundle_descriptor(
        &self,
        bundle_id: &str,
    ) -> Result<RawSourceRetentionBundleDescriptor, RawSourceVaultError> {
        self.index.bundles.get(bundle_id).cloned().ok_or_else(|| {
            RawSourceVaultError::BundleMissing {
                bundle_id: bundle_id.to_string(),
            }
        })
    }

    /// Read encrypted bundle bytes without decrypting them into metadata paths.
    pub fn read_encrypted_bundle(&self, bundle_id: &str) -> Result<Vec<u8>, RawSourceVaultError> {
        self.read_bundle_descriptor(bundle_id)?;
        fs::read(self.bundle_path(bundle_id)).map_err(io_error)
    }

    /// Read metadata-only AEAD envelope by bundle id.
    pub fn read_vault_envelope(
        &self,
        bundle_id: &str,
    ) -> Result<RawSourceVaultEnvelope, RawSourceVaultError> {
        self.index
            .envelopes
            .get(bundle_id)
            .cloned()
            .ok_or_else(|| RawSourceVaultError::Denied {
                reason: "raw-source vault envelope metadata is missing".to_string(),
            })
    }

    /// Decrypt bundle bytes for an authorized caller after audit validation.
    pub fn decrypt_bundle_for_authorized_read(
        &self,
        audit: RawSourceRetentionAccessAudit,
    ) -> Result<Vec<u8>, RawSourceVaultError> {
        validate_raw_source_retention_access_audit(&audit).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        let descriptor = self.read_bundle_descriptor(&audit.bundle_id)?;
        let envelope = self.read_vault_envelope(&audit.bundle_id)?;
        validate_raw_source_vault_envelope(&envelope).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        if envelope.bundle_id != descriptor.bundle_id
            || envelope.workspace_id != descriptor.workspace_id
            || envelope.purpose != descriptor.purpose
            || envelope.encrypted_byte_len != descriptor.encrypted_byte_len
            || envelope.ciphertext_digest != descriptor.integrity
        {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source vault envelope does not match descriptor metadata".to_string(),
            });
        }
        let current_key_reference = self.key_provider.key_reference();
        validate_raw_source_key_reference(&current_key_reference).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        if current_key_reference != envelope.key_reference {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source vault key reference does not match envelope".to_string(),
            });
        }
        let encrypted = self.read_encrypted_bundle(&audit.bundle_id)?;
        let encrypted_fingerprint = sha256_fingerprint(&encrypted);
        if encrypted.len() as u64 != envelope.encrypted_byte_len
            || encrypted_fingerprint != envelope.ciphertext_digest
        {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source vault ciphertext digest mismatch".to_string(),
            });
        }
        let aad = vault_aad(
            &descriptor.bundle_id,
            &descriptor.lease_id,
            descriptor.workspace_id,
            descriptor.purpose,
            envelope.algorithm,
            &envelope.key_reference,
        );
        if sha256_label(&aad) != envelope.aad_digest {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source vault authenticated metadata digest mismatch".to_string(),
            });
        }
        let sealed = RawSourceVaultSealedPayload {
            bytes: encrypted,
            nonce_digest: envelope.nonce_digest,
            ciphertext_digest: envelope.ciphertext_digest,
            tag_digest: envelope.tag_digest,
            encrypted_byte_len: envelope.encrypted_byte_len,
        };
        let key = self.key_bytes()?;
        self.cipher.decrypt(&key, &aad, &sealed)
    }

    /// Rotate one retained bundle to a new metadata-only key reference.
    ///
    /// The current vault provider must still decrypt the existing envelope. Callers should reopen or
    /// recompose the vault with a provider for the new reference before future reads.
    pub fn rotate_bundle_key<N: RawSourceVaultKeyProvider>(
        &mut self,
        bundle_id: &str,
        new_key_provider: &N,
        event_sequence: EventSequence,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
    ) -> Result<RawSourceKeyRotationRecord, RawSourceVaultError> {
        if !self.config.enabled {
            return Err(RawSourceVaultError::Disabled);
        }
        let mut descriptor = self.read_bundle_descriptor(bundle_id)?;
        let envelope = self.read_vault_envelope(bundle_id)?;
        validate_raw_source_vault_envelope(&envelope).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        if envelope.bundle_id != descriptor.bundle_id
            || envelope.workspace_id != descriptor.workspace_id
            || envelope.purpose != descriptor.purpose
            || envelope.encrypted_byte_len != descriptor.encrypted_byte_len
            || envelope.ciphertext_digest != descriptor.integrity
        {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source vault envelope does not match descriptor metadata".to_string(),
            });
        }
        let previous_key_reference = self.key_provider.key_reference();
        validate_raw_source_key_reference(&previous_key_reference).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        if previous_key_reference != envelope.key_reference {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source vault key reference does not match envelope".to_string(),
            });
        }
        let encrypted = self.read_encrypted_bundle(bundle_id)?;
        let encrypted_fingerprint = sha256_fingerprint(&encrypted);
        if encrypted.len() as u64 != envelope.encrypted_byte_len
            || encrypted_fingerprint != envelope.ciphertext_digest
        {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source vault ciphertext digest mismatch".to_string(),
            });
        }
        let previous_aad = vault_aad(
            &descriptor.bundle_id,
            &descriptor.lease_id,
            descriptor.workspace_id,
            descriptor.purpose,
            envelope.algorithm,
            &envelope.key_reference,
        );
        if sha256_label(&previous_aad) != envelope.aad_digest {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source vault authenticated metadata digest mismatch".to_string(),
            });
        }
        let sealed = RawSourceVaultSealedPayload {
            bytes: encrypted,
            nonce_digest: envelope.nonce_digest.clone(),
            ciphertext_digest: envelope.ciphertext_digest.clone(),
            tag_digest: envelope.tag_digest.clone(),
            encrypted_byte_len: envelope.encrypted_byte_len,
        };
        let previous_key = self.key_bytes()?;
        let plaintext =
            Zeroizing::new(self.cipher.decrypt(&previous_key, &previous_aad, &sealed)?);

        let new_key_reference = new_key_provider.key_reference();
        validate_raw_source_key_reference(&new_key_reference).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        let new_key = key_bytes_from_provider(new_key_provider)?;
        let new_aad = vault_aad(
            &descriptor.bundle_id,
            &descriptor.lease_id,
            descriptor.workspace_id,
            descriptor.purpose,
            self.cipher.algorithm(),
            &new_key_reference,
        );
        let new_sealed = self.cipher.encrypt(&new_key, &new_aad, &plaintext)?;
        if new_sealed.encrypted_byte_len > self.config.max_bundle_bytes {
            return Err(RawSourceVaultError::Denied {
                reason: "rotated raw-source bundle exceeds configured vault limit".to_string(),
            });
        }
        let new_envelope = RawSourceVaultEnvelope {
            bundle_id: descriptor.bundle_id.clone(),
            workspace_id: descriptor.workspace_id,
            purpose: descriptor.purpose,
            algorithm: self.cipher.algorithm(),
            key_reference: new_key_reference.clone(),
            nonce_digest: new_sealed.nonce_digest.clone(),
            ciphertext_digest: new_sealed.ciphertext_digest.clone(),
            tag_digest: new_sealed.tag_digest.clone(),
            aad_digest: sha256_label(&new_aad),
            encrypted_byte_len: new_sealed.encrypted_byte_len,
            schema_version: 1,
        };
        validate_raw_source_vault_envelope(&new_envelope).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        descriptor.encrypted_byte_len = new_sealed.encrypted_byte_len;
        descriptor.integrity = new_sealed.ciphertext_digest.clone();

        let record = RawSourceKeyRotationRecord {
            bundle_id: descriptor.bundle_id.clone(),
            previous_key_reference,
            new_key_reference,
            event_sequence,
            correlation_id,
            causality_id,
            metadata_summary: format!(
                "rotation_complete encrypted_bytes={}",
                new_sealed.encrypted_byte_len
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_raw_source_key_rotation_record(&record).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;

        fs::write(self.bundle_path(bundle_id), &new_sealed.bytes).map_err(io_error)?;
        self.index.key_references.insert(
            bundle_id.to_string(),
            record.new_key_reference.key_id.clone(),
        );
        self.index
            .envelopes
            .insert(bundle_id.to_string(), new_envelope);
        self.index.bundles.insert(bundle_id.to_string(), descriptor);
        self.flush_index()?;
        Ok(record)
    }

    /// Produce a metadata-only recovery drill report for one retained bundle.
    pub fn inspect_bundle_recovery(
        &self,
        bundle_id: &str,
        event_sequence: EventSequence,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
    ) -> Result<RawSourceVaultRecoveryReport, RawSourceVaultError> {
        if !self.config.enabled {
            return Err(RawSourceVaultError::Disabled);
        }
        let descriptor = match self.read_bundle_descriptor(bundle_id) {
            Ok(descriptor) => descriptor,
            Err(RawSourceVaultError::BundleMissing { .. }) => {
                return self.recovery_report(
                    Some(bundle_id),
                    RawSourceVaultRecoveryState::FailedClosed,
                    "bundle_descriptor_missing",
                    event_sequence,
                    correlation_id,
                    causality_id,
                );
            }
            Err(err) => return Err(err),
        };
        let envelope = match self.read_vault_envelope(bundle_id) {
            Ok(envelope) => envelope,
            Err(_) => {
                return self.recovery_report(
                    Some(bundle_id),
                    RawSourceVaultRecoveryState::FailedClosed,
                    "envelope_metadata_missing",
                    event_sequence,
                    correlation_id,
                    causality_id,
                );
            }
        };
        if validate_raw_source_vault_envelope(&envelope).is_err() {
            return self.recovery_report(
                Some(bundle_id),
                RawSourceVaultRecoveryState::Quarantined,
                "envelope_metadata_invalid",
                event_sequence,
                correlation_id,
                causality_id,
            );
        }
        if envelope.bundle_id != descriptor.bundle_id
            || envelope.workspace_id != descriptor.workspace_id
            || envelope.purpose != descriptor.purpose
            || envelope.encrypted_byte_len != descriptor.encrypted_byte_len
            || envelope.ciphertext_digest != descriptor.integrity
        {
            return self.recovery_report(
                Some(bundle_id),
                RawSourceVaultRecoveryState::Quarantined,
                "envelope_descriptor_mismatch",
                event_sequence,
                correlation_id,
                causality_id,
            );
        }
        let encrypted = match fs::read(self.bundle_path(bundle_id)) {
            Ok(encrypted) => encrypted,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return self.recovery_report(
                    Some(bundle_id),
                    RawSourceVaultRecoveryState::FailedClosed,
                    "ciphertext_missing",
                    event_sequence,
                    correlation_id,
                    causality_id,
                );
            }
            Err(_) => {
                return self.recovery_report(
                    Some(bundle_id),
                    RawSourceVaultRecoveryState::FailedClosed,
                    "ciphertext_unavailable",
                    event_sequence,
                    correlation_id,
                    causality_id,
                );
            }
        };
        let encrypted_fingerprint = sha256_fingerprint(&encrypted);
        if encrypted.len() as u64 != envelope.encrypted_byte_len
            || encrypted_fingerprint != envelope.ciphertext_digest
        {
            return self.recovery_report(
                Some(bundle_id),
                RawSourceVaultRecoveryState::Quarantined,
                "ciphertext_digest_mismatch",
                event_sequence,
                correlation_id,
                causality_id,
            );
        }
        let aad = vault_aad(
            &descriptor.bundle_id,
            &descriptor.lease_id,
            descriptor.workspace_id,
            descriptor.purpose,
            envelope.algorithm,
            &envelope.key_reference,
        );
        if sha256_label(&aad) != envelope.aad_digest {
            return self.recovery_report(
                Some(bundle_id),
                RawSourceVaultRecoveryState::Quarantined,
                "authenticated_metadata_digest_mismatch",
                event_sequence,
                correlation_id,
                causality_id,
            );
        }
        let current_key_reference = self.key_provider.key_reference();
        if validate_raw_source_key_reference(&current_key_reference).is_err()
            || current_key_reference != envelope.key_reference
        {
            return self.recovery_report(
                Some(bundle_id),
                RawSourceVaultRecoveryState::FailedClosed,
                "key_reference_unavailable",
                event_sequence,
                correlation_id,
                causality_id,
            );
        }
        let sealed = RawSourceVaultSealedPayload {
            bytes: encrypted,
            nonce_digest: envelope.nonce_digest,
            ciphertext_digest: envelope.ciphertext_digest,
            tag_digest: envelope.tag_digest,
            encrypted_byte_len: envelope.encrypted_byte_len,
        };
        let key = match self.key_bytes() {
            Ok(key) => key,
            Err(_) => {
                return self.recovery_report(
                    Some(bundle_id),
                    RawSourceVaultRecoveryState::FailedClosed,
                    "key_unavailable",
                    event_sequence,
                    correlation_id,
                    causality_id,
                );
            }
        };
        if self.cipher.decrypt(&key, &aad, &sealed).is_err() {
            return self.recovery_report(
                Some(bundle_id),
                RawSourceVaultRecoveryState::FailedClosed,
                "authentication_unavailable",
                event_sequence,
                correlation_id,
                causality_id,
            );
        }
        self.recovery_report(
            Some(bundle_id),
            RawSourceVaultRecoveryState::Recovered,
            "metadata_verified",
            event_sequence,
            correlation_id,
            causality_id,
        )
    }

    /// Export an encrypted raw-source bundle to a hosted endpoint by descriptor reference only.
    pub fn export_encrypted_bundle_hosted<Cli: RawSourceHostedExportClient>(
        &self,
        bundle_id: &str,
        consent: RawSourceHostedExportConsent,
        endpoint: HostedTelemetryEndpointDescriptor,
        client: &mut Cli,
    ) -> Result<HostedRetentionExportLinkage, RawSourceVaultError> {
        if !self.config.enabled || !self.policy.capture_enabled {
            return Err(RawSourceVaultError::Disabled);
        }
        validate_raw_source_hosted_export_consent(&consent).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        let now = TimestampMillis::now();
        if consent.issued_at.0 > now.0 || consent.expires_at.0 <= now.0 {
            return Err(RawSourceVaultError::Denied {
                reason: "hosted raw-source export consent is not current".to_string(),
            });
        }
        if !endpoint.allowlisted || !endpoint.endpoint_label.starts_with("https://") {
            return Err(RawSourceVaultError::Denied {
                reason: "hosted raw-source export requires an allowlisted HTTPS endpoint"
                    .to_string(),
            });
        }
        if consent.endpoint_id != endpoint.endpoint_id {
            return Err(RawSourceVaultError::Denied {
                reason: "hosted raw-source export consent endpoint does not match upload endpoint"
                    .to_string(),
            });
        }
        let descriptor = self.read_bundle_descriptor(bundle_id)?;
        let envelope = self.read_vault_envelope(bundle_id)?;
        if consent.workspace_id != descriptor.workspace_id || consent.purpose != descriptor.purpose
        {
            return Err(RawSourceVaultError::Denied {
                reason: "hosted raw-source export consent does not match retained bundle"
                    .to_string(),
            });
        }
        let encrypted_bundle = self.read_encrypted_bundle(bundle_id)?;
        if encrypted_bundle.len() as u64 != descriptor.encrypted_byte_len
            || sha256_fingerprint(&encrypted_bundle) != descriptor.integrity
        {
            return Err(RawSourceVaultError::Denied {
                reason: "hosted raw-source export ciphertext digest mismatch".to_string(),
            });
        }
        let request = RawSourceHostedExportRequest {
            consent,
            endpoint,
            descriptor: descriptor.clone(),
            envelope,
            encrypted_bundle,
        };
        let ack = client.upload_encrypted_bundle(&request)?;
        if !ack.accepted || ack.telemetry_batch_id.trim().is_empty() {
            return Err(RawSourceVaultError::Denied {
                reason: "hosted raw-source export was not accepted".to_string(),
            });
        }
        let linkage = HostedRetentionExportLinkage {
            telemetry_batch_id: ack.telemetry_batch_id,
            bundle_id: descriptor.bundle_id,
            raw_source_consent_verified: true,
            schema_version: 1,
        };
        validate_hosted_retention_export_linkage(&linkage).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        Ok(linkage)
    }

    /// Delete encrypted content and keep a metadata-only tombstone.
    pub fn delete_bundle(
        &mut self,
        tombstone: RawSourceRetentionTombstone,
    ) -> Result<RawSourceRetentionTombstone, RawSourceVaultError> {
        if tombstone.bundle_id.trim().is_empty()
            || tombstone.reason.trim().is_empty()
            || tombstone.event_sequence.0 == 0
            || tombstone.correlation_id.0 == 0
            || tombstone.causality_id.0.is_nil()
            || tombstone.schema_version == 0
        {
            return Err(RawSourceVaultError::Denied {
                reason: "retention tombstone metadata is invalid".to_string(),
            });
        }
        if !self.index.bundles.contains_key(&tombstone.bundle_id) {
            return Err(RawSourceVaultError::BundleMissing {
                bundle_id: tombstone.bundle_id,
            });
        }
        // A missing ciphertext is recoverable: still drop the index entries and
        // record the tombstone rather than leaving a dangling descriptor.
        match fs::remove_file(self.bundle_path(&tombstone.bundle_id)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(io_error(err)),
        }
        self.index.bundles.remove(&tombstone.bundle_id);
        self.index.key_references.remove(&tombstone.bundle_id);
        self.index.envelopes.remove(&tombstone.bundle_id);
        self.index.lease_expirations.remove(&tombstone.bundle_id);
        self.index
            .tombstones
            .insert(tombstone.bundle_id.clone(), tombstone.clone());
        self.flush_index()?;
        Ok(tombstone)
    }

    /// Remove all descriptors whose lease TTL has expired and record tombstones.
    pub fn purge_expired(&mut self, now: TimestampMillis) -> Result<usize, RawSourceVaultError> {
        let bundle_ids = self
            .index
            .bundles
            .keys()
            .filter(|bundle_id| {
                self.index
                    .lease_expirations
                    .get(*bundle_id)
                    .is_some_and(|expires_at| expires_at.0 <= now.0)
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut removed = 0usize;
        for bundle_id in bundle_ids {
            let tombstone = RawSourceRetentionTombstone {
                bundle_id: bundle_id.clone(),
                reason: "ttl_expired".to_string(),
                deleted_at: now,
                event_sequence: legion_protocol::EventSequence(now.0.max(1)),
                correlation_id: legion_protocol::CorrelationId(now.0.max(1)),
                causality_id: legion_protocol::CausalityId(uuid::Uuid::now_v7()),
                schema_version: 1,
            };
            // Accumulate per-bundle outcomes instead of aborting the whole purge
            // on the first failure; skip bundles that fail and continue.
            if self.delete_bundle(tombstone).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn pack_files(
        &self,
        request: &RawSourceCaptureRequest,
        files: Vec<RawSourceVaultFile>,
    ) -> Result<Vec<u8>, RawSourceVaultError> {
        if files.is_empty() {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source capture requires at least one file".to_string(),
            });
        }
        let mut packed = Vec::new();
        let mut total_bytes: u64 = 0;
        for file in files {
            if !request.paths.contains(&file.path) {
                return Err(RawSourceVaultError::Denied {
                    reason: "raw-source file is outside capture request scope".to_string(),
                });
            }
            // Enforce the consent-bound byte budget on the raw source bytes so a
            // caller cannot exceed `request.max_bytes` up to the vault config limit.
            total_bytes = total_bytes.saturating_add(file.bytes.len() as u64);
            if total_bytes > request.max_bytes {
                return Err(RawSourceVaultError::Denied {
                    reason: "raw-source capture exceeds consent-bound byte budget".to_string(),
                });
            }
            packed.extend_from_slice(file.path.0.as_bytes());
            packed.push(0);
            packed.extend_from_slice(&(file.bytes.len() as u64).to_le_bytes());
            packed.extend_from_slice(&file.bytes);
        }
        Ok(packed)
    }

    /// Revert in-memory index mutations staged for a single bundle capture.
    fn revert_bundle_index(&mut self, bundle_id: &str) {
        self.index.bundles.remove(bundle_id);
        self.index.lease_expirations.remove(bundle_id);
        self.index.envelopes.remove(bundle_id);
        self.index.key_references.remove(bundle_id);
    }

    fn bundle_path(&self, bundle_id: &str) -> PathBuf {
        self.root.join(format!("{}.vault", safe_name(bundle_id)))
    }

    fn flush_index(&self) -> Result<(), RawSourceVaultError> {
        let text =
            serde_json::to_string_pretty(&self.index).map_err(|err| RawSourceVaultError::Io {
                message: format!("encode vault index: {err}"),
            })?;
        // Atomic write: a partially written or disk-full update must never
        // truncate the live index. Write to a temp file, fsync it, then rename
        // it over the target and fsync the parent directory.
        let index_path = self.root.join("index.json");
        let temp_path = self.root.join("index.json.tmp");
        {
            let mut file = fs::File::create(&temp_path).map_err(io_error)?;
            use std::io::Write as _;
            file.write_all(text.as_bytes()).map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
        }
        fs::rename(&temp_path, &index_path).map_err(|err| {
            let _ = fs::remove_file(&temp_path);
            io_error(err)
        })?;
        // Best-effort durability of the rename itself. Syncing a directory
        // handle is not supported on all platforms (notably Windows), so a
        // failure here is intentionally ignored.
        if let Ok(dir) = fs::File::open(&self.root) {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    fn key_bytes(&self) -> Result<Zeroizing<Vec<u8>>, RawSourceVaultError> {
        key_bytes_from_provider(&self.key_provider)
    }

    fn recovery_report(
        &self,
        bundle_id: Option<&str>,
        state: RawSourceVaultRecoveryState,
        metadata_summary: &str,
        event_sequence: EventSequence,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
    ) -> Result<RawSourceVaultRecoveryReport, RawSourceVaultError> {
        let report = RawSourceVaultRecoveryReport {
            recovery_id: format!(
                "recovery:{}:{}",
                bundle_id
                    .map(safe_name)
                    .unwrap_or_else(|| "vault".to_string()),
                event_sequence.0
            ),
            bundle_id: bundle_id.map(ToString::to_string),
            state,
            event_sequence,
            correlation_id,
            causality_id,
            metadata_summary: metadata_summary.to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_raw_source_vault_recovery_report(&report).map_err(|err| {
            RawSourceVaultError::Denied {
                reason: err.message,
            }
        })?;
        Ok(report)
    }
}

fn key_bytes_from_provider<P: RawSourceVaultKeyProvider>(
    provider: &P,
) -> Result<Zeroizing<Vec<u8>>, RawSourceVaultError> {
    let key = Zeroizing::new(provider.key_bytes());
    if key.is_empty() {
        return Err(RawSourceVaultError::Denied {
            reason: "raw-source vault key bytes are required".to_string(),
        });
    }
    Ok(key)
}

impl<K: RawSourceVaultKeyProvider> FileBackedRawSourceVault<K, ChaCha20Poly1305VaultCipher> {
    /// Open a production ChaCha20-Poly1305 raw-source vault rooted at `root`.
    pub fn open_production(
        root: impl AsRef<Path>,
        policy: RawSourceRetentionPolicy,
        config: RawSourceVaultConfig,
        key_provider: K,
    ) -> Result<Self, RawSourceVaultError> {
        Self::open(
            root,
            policy,
            config,
            key_provider,
            ChaCha20Poly1305VaultCipher,
        )
    }
}

fn chacha20poly1305_cipher(key: &[u8]) -> Result<ChaCha20Poly1305, RawSourceVaultError> {
    if key.len() != CHACHA20_POLY1305_KEY_LEN {
        return Err(crypto_error(
            "ChaCha20-Poly1305 requires a 256-bit vault key",
        ));
    }
    ChaCha20Poly1305::new_from_slice(key).map_err(|_| crypto_error("invalid vault key"))
}

fn vault_aad(
    bundle_id: &str,
    lease_id: &str,
    workspace_id: WorkspaceId,
    purpose: RawSourceRetentionPurpose,
    algorithm: RawSourceVaultAlgorithm,
    key_reference: &RawSourceKeyReference,
) -> Vec<u8> {
    format!(
        "legion.raw-source.vault.aad.v1\0bundle_id={bundle_id}\0lease_id={lease_id}\0workspace_id={}\0purpose={purpose:?}\0algorithm={algorithm:?}\0key_id={}\0key_version={}\0provider_label={}\0rotation_generation={}\0schema_version={}",
        workspace_id.0,
        key_reference.key_id,
        key_reference.key_version,
        key_reference.provider_label,
        key_reference.rotation_generation,
        key_reference.schema_version,
    )
    .into_bytes()
}

struct ParsedSealedVaultFile<'a> {
    algorithm_id: u8,
    nonce: &'a [u8],
    ciphertext_and_tag: &'a [u8],
}

fn parse_sealed_vault_file(bytes: &[u8]) -> Result<ParsedSealedVaultFile<'_>, RawSourceVaultError> {
    const HEADER_LEN: usize = 8;
    if bytes.len() <= HEADER_LEN + CHACHA20_POLY1305_NONCE_LEN + CHACHA20_POLY1305_TAG_LEN {
        return Err(crypto_error("sealed vault payload is truncated"));
    }
    if &bytes[..VAULT_FILE_MAGIC.len()] != VAULT_FILE_MAGIC {
        return Err(crypto_error("sealed vault payload has invalid magic"));
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != VAULT_FILE_VERSION {
        return Err(crypto_error("unsupported sealed vault file version"));
    }
    let algorithm_id = bytes[6];
    let nonce_len = bytes[7] as usize;
    if nonce_len != CHACHA20_POLY1305_NONCE_LEN {
        return Err(crypto_error(
            "sealed vault payload has invalid nonce length",
        ));
    }
    let nonce_start = HEADER_LEN;
    let nonce_end = nonce_start + nonce_len;
    if bytes.len() <= nonce_end + CHACHA20_POLY1305_TAG_LEN {
        return Err(crypto_error("sealed vault payload has no ciphertext"));
    }
    Ok(ParsedSealedVaultFile {
        algorithm_id,
        nonce: &bytes[nonce_start..nonce_end],
        ciphertext_and_tag: &bytes[nonce_end..],
    })
}

fn sha256_fingerprint(bytes: &[u8]) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: sha256_label(bytes),
    }
}

fn sha256_label(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{}", lowercase_hex(&digest))
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 0x0f) as usize] as char);
    }
    text
}

fn decode_hex(text: &str) -> Result<Vec<u8>, RawSourceVaultError> {
    let text = text.trim();
    if !text.len().is_multiple_of(2) {
        return Err(RawSourceVaultError::Denied {
            reason: "hex key material has odd length".to_string(),
        });
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for chunk in text.as_bytes().chunks_exact(2) {
        let high = hex_value(chunk[0])?;
        let low = hex_value(chunk[1])?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_value(value: u8) -> Result<u8, RawSourceVaultError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(RawSourceVaultError::Denied {
            reason: "hex key material contains non-hex byte".to_string(),
        }),
    }
}

fn crypto_error(message: impl Into<String>) -> RawSourceVaultError {
    RawSourceVaultError::Crypto {
        message: message.into(),
    }
}

fn safe_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn io_error(err: std::io::Error) -> RawSourceVaultError {
    RawSourceVaultError::Io {
        message: err.to_string(),
    }
}

fn keyring_error(err: keyring::Error) -> RawSourceVaultError {
    RawSourceVaultError::Denied {
        reason: format!("OS keyring operation failed: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use legion_protocol::{
        CanonicalPath, CausalityId, CorrelationId, EventSequence,
        HostedTelemetryEndpointDescriptor, PrincipalId, RawSourceHostedExportConsent,
        RawSourceRetentionPurpose, RedactionHint, TimestampMillis, WorkspaceId,
    };

    use super::*;

    #[derive(Debug, Clone)]
    struct TestKeyProvider {
        key: Vec<u8>,
        key_version: String,
        rotation_generation: u64,
    }

    impl TestKeyProvider {
        fn new(key: &[u8], key_version: &str) -> Self {
            Self::with_generation(key, key_version, 1)
        }

        fn with_generation(key: &[u8], key_version: &str, rotation_generation: u64) -> Self {
            Self {
                key: key.to_vec(),
                key_version: key_version.to_string(),
                rotation_generation,
            }
        }

        fn wrong_key_same_reference() -> Self {
            Self::new(b"fedcba9876543210fedcba9876543210", "v1")
        }
    }

    impl Default for TestKeyProvider {
        fn default() -> Self {
            Self::new(b"0123456789abcdef0123456789abcdef", "v1")
        }
    }

    impl RawSourceVaultKeyProvider for TestKeyProvider {
        fn key_reference(&self) -> RawSourceKeyReference {
            RawSourceKeyReference {
                key_id: "key:test".to_string(),
                key_version: self.key_version.clone(),
                provider_label: "test-keyring".to_string(),
                rotation_generation: self.rotation_generation,
                schema_version: 1,
            }
        }

        fn key_bytes(&self) -> Vec<u8> {
            self.key.clone()
        }
    }

    struct TestKmsProvider {
        key: Vec<u8>,
    }

    impl RawSourceKmsEnvelopeProvider for TestKmsProvider {
        fn wrap_key(
            &self,
            key_reference: RawSourceKeyReference,
            plaintext_key: &[u8],
        ) -> Result<RawSourceWrappedKey, RawSourceVaultError> {
            if plaintext_key != self.key {
                return Err(RawSourceVaultError::Denied {
                    reason: "unexpected plaintext key".to_string(),
                });
            }
            Ok(RawSourceWrappedKey {
                key_reference,
                wrapped_key_digest: sha256_label(plaintext_key),
                wrapped_key_byte_len: plaintext_key.len() as u64,
                provider_label: "test-kms-envelope".to_string(),
            })
        }

        fn unwrap_key(
            &self,
            wrapped: &RawSourceWrappedKey,
        ) -> Result<Zeroizing<Vec<u8>>, RawSourceVaultError> {
            if wrapped.wrapped_key_digest != sha256_label(&self.key) {
                return Err(RawSourceVaultError::Denied {
                    reason: "wrapped key digest mismatch".to_string(),
                });
            }
            Ok(Zeroizing::new(self.key.clone()))
        }
    }

    struct AcceptingHostedRawExport {
        saw_plaintext: bool,
    }

    impl RawSourceHostedExportClient for AcceptingHostedRawExport {
        fn upload_encrypted_bundle(
            &mut self,
            request: &RawSourceHostedExportRequest,
        ) -> Result<RawSourceHostedExportAck, RawSourceVaultError> {
            let body = String::from_utf8_lossy(&request.encrypted_bundle);
            self.saw_plaintext = body.contains("fn main") || body.contains("secret");
            Ok(RawSourceHostedExportAck {
                telemetry_batch_id: format!("raw-export:{}", request.descriptor.bundle_id),
                accepted: !self.saw_plaintext,
                status: "accepted".to_string(),
            })
        }
    }

    fn temp_vault_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("legion-retention-{name}-{}", uuid::Uuid::now_v7()))
    }

    fn policy(enabled: bool) -> RawSourceRetentionPolicy {
        RawSourceRetentionPolicy {
            capture_enabled: enabled,
            allowed_purposes: vec![RawSourceRetentionPurpose::SupportBundle],
            max_bundle_bytes: 4096,
            ttl_ms: 60_000,
            schema_version: 1,
        }
    }

    fn grant() -> RawSourceRetentionConsentGrant {
        RawSourceRetentionConsentGrant {
            principal_id: PrincipalId("tester".to_string()),
            workspace_id: WorkspaceId(1),
            purpose: RawSourceRetentionPurpose::SupportBundle,
            path_scope: vec![CanonicalPath("C:/repo/src/main.rs".to_string())],
            expires_at: TimestampMillis(60_000),
            correlation_id: CorrelationId(1),
            schema_version: 1,
        }
    }

    fn request() -> RawSourceCaptureRequest {
        RawSourceCaptureRequest {
            workspace_id: WorkspaceId(1),
            principal_id: PrincipalId("tester".to_string()),
            purpose: RawSourceRetentionPurpose::SupportBundle,
            paths: vec![CanonicalPath("C:/repo/src/main.rs".to_string())],
            max_bytes: 1024,
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(uuid::Uuid::from_u128(
                0x018f_0000_0000_7000_8000_3000_0000_0001,
            )),
            schema_version: 1,
        }
    }

    fn request_with_correlation(correlation_id: u64) -> RawSourceCaptureRequest {
        RawSourceCaptureRequest {
            correlation_id: CorrelationId(correlation_id),
            causality_id: CausalityId(uuid::Uuid::now_v7()),
            ..request()
        }
    }

    fn authorized_read_audit(bundle_id: &str, sequence: u64) -> RawSourceRetentionAccessAudit {
        RawSourceRetentionAccessAudit {
            bundle_id: bundle_id.to_string(),
            principal_id: PrincipalId("tester".to_string()),
            action: "authorized_read".to_string(),
            event_sequence: EventSequence(sequence),
            correlation_id: CorrelationId(sequence),
            causality_id: CausalityId(uuid::Uuid::now_v7()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn hosted_export_consent() -> RawSourceHostedExportConsent {
        let now = TimestampMillis::now().0.max(2);
        RawSourceHostedExportConsent {
            grant_id: "hosted-raw-grant".to_string(),
            principal_id: PrincipalId("tester".to_string()),
            workspace_id: WorkspaceId(1),
            endpoint_id: "support-endpoint".to_string(),
            purpose: RawSourceRetentionPurpose::SupportBundle,
            issued_at: TimestampMillis(now - 1),
            expires_at: TimestampMillis(now + 60_000),
            revoked: false,
            correlation_id: CorrelationId(1),
            schema_version: 1,
        }
    }

    fn hosted_endpoint() -> HostedTelemetryEndpointDescriptor {
        HostedTelemetryEndpointDescriptor {
            endpoint_id: "support-endpoint".to_string(),
            endpoint_label: "https://support.invalid/raw".to_string(),
            region: "local-test".to_string(),
            allowlisted: true,
            schema_version: 1,
        }
    }

    #[test]
    fn retention_fixture_is_default_denied() {
        let mut vault = RetentionFixtureVault::new(policy(false));
        assert!(matches!(
            vault.capture_descriptor(grant(), request()),
            Err(RetentionFixtureError::Disabled)
        ));
    }

    #[test]
    fn retention_fixture_captures_descriptor_without_raw_content() {
        let mut vault = RetentionFixtureVault::new(policy(true));
        let (_lease, descriptor) = vault
            .capture_descriptor(grant(), request())
            .expect("capture descriptor");
        assert_eq!(vault.bundle_count(), 1);
        assert_eq!(descriptor.encrypted_byte_len, 1024);
        assert!(!format!("{descriptor:?}").contains("fn main"));
    }

    #[test]
    fn retention_fixture_rejects_out_of_scope_capture() {
        let mut vault = RetentionFixtureVault::new(policy(true));
        let out_of_scope = RawSourceCaptureRequest {
            paths: vec![CanonicalPath("C:/repo/src/lib.rs".to_string())],
            ..request()
        };
        assert!(matches!(
            vault.capture_descriptor(grant(), out_of_scope),
            Err(RetentionFixtureError::CaptureDenied { .. })
        ));
    }

    #[test]
    fn retention_fixture_audits_access_and_deletes_with_tombstone() {
        let mut vault = RetentionFixtureVault::new(policy(true));
        let (_lease, descriptor) = vault
            .capture_descriptor(grant(), request())
            .expect("capture descriptor");
        let audit = RawSourceRetentionAccessAudit {
            bundle_id: descriptor.bundle_id.clone(),
            principal_id: PrincipalId("tester".to_string()),
            action: "read_descriptor".to_string(),
            event_sequence: EventSequence(2),
            correlation_id: CorrelationId(2),
            causality_id: CausalityId(uuid::Uuid::from_u128(
                0x018f_0000_0000_7000_8000_3000_0000_0002,
            )),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        vault.audit_access(audit).expect("audit access");
        let tombstone = RawSourceRetentionTombstone {
            bundle_id: descriptor.bundle_id,
            reason: "user_deleted".to_string(),
            deleted_at: TimestampMillis(70_000),
            event_sequence: EventSequence(3),
            correlation_id: CorrelationId(3),
            causality_id: CausalityId(uuid::Uuid::from_u128(
                0x018f_0000_0000_7000_8000_3000_0000_0003,
            )),
            schema_version: 1,
        };
        vault.delete_bundle(tombstone).expect("delete bundle");
        assert_eq!(vault.bundle_count(), 0);
    }

    #[test]
    fn retention_fixture_delete_missing_bundle_is_rejected() {
        let mut vault = RetentionFixtureVault::new(policy(true));
        let tombstone = RawSourceRetentionTombstone {
            bundle_id: "bundle-missing".to_string(),
            reason: "user_deleted".to_string(),
            deleted_at: TimestampMillis(70_000),
            event_sequence: EventSequence(3),
            correlation_id: CorrelationId(3),
            causality_id: CausalityId(uuid::Uuid::from_u128(
                0x018f_0000_0000_7000_8000_3000_0000_0003,
            )),
            schema_version: 1,
        };
        assert!(matches!(
            vault.delete_bundle(tombstone),
            Err(RetentionFixtureError::BundleMissing { .. })
        ));
    }

    #[test]
    fn file_backed_vault_encrypts_and_does_not_store_plaintext() {
        let root = temp_vault_root("encrypted");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() { secret(); }".to_vec(),
                }],
            )
            .expect("capture bundle");
        let encrypted = vault
            .read_encrypted_bundle(&descriptor.bundle_id)
            .expect("encrypted bytes");
        let envelope = vault
            .read_vault_envelope(&descriptor.bundle_id)
            .expect("vault envelope");
        assert_eq!(
            envelope.algorithm,
            RawSourceVaultAlgorithm::ChaCha20Poly1305
        );
        assert_eq!(descriptor.integrity.algorithm, "sha256");
        assert!(!descriptor.integrity.value.contains("stable-sum"));
        assert!(!String::from_utf8_lossy(&encrypted).contains("fn main"));

        let decrypted = vault
            .decrypt_bundle_for_authorized_read(authorized_read_audit(&descriptor.bundle_id, 4))
            .expect("authorized decrypt");
        assert!(String::from_utf8_lossy(&decrypted).contains("fn main"));
        let index_text = fs::read_to_string(root.join("index.json")).expect("index text");
        assert!(!index_text.contains("fn main"));
        assert!(!index_text.contains("0123456789abcdef"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_uses_random_nonce_for_same_plaintext() {
        let root = temp_vault_root("random-nonce");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let files = || {
            vec![RawSourceVaultFile {
                path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                bytes: b"fn main() { secret(); }".to_vec(),
            }]
        };
        let first = vault
            .capture_bundle(grant(), request_with_correlation(10), files())
            .expect("first capture")
            .1;
        let second = vault
            .capture_bundle(grant(), request_with_correlation(11), files())
            .expect("second capture")
            .1;
        let first_bytes = vault
            .read_encrypted_bundle(&first.bundle_id)
            .expect("first ciphertext");
        let second_bytes = vault
            .read_encrypted_bundle(&second.bundle_id)
            .expect("second ciphertext");
        let first_envelope = vault
            .read_vault_envelope(&first.bundle_id)
            .expect("first envelope");
        let second_envelope = vault
            .read_vault_envelope(&second.bundle_id)
            .expect("second envelope");
        assert_ne!(first_bytes, second_bytes);
        assert_ne!(first_envelope.nonce_digest, second_envelope.nonce_digest);
        assert_ne!(first.integrity.value, second.integrity.value);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_rejects_wrong_key_and_tampered_ciphertext() {
        let root = temp_vault_root("aead-fail-closed");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() { secret(); }".to_vec(),
                }],
            )
            .expect("capture bundle");

        let wrong_key_vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::wrong_key_same_reference(),
        )
        .expect("reopen with wrong key");
        assert!(matches!(
            wrong_key_vault.decrypt_bundle_for_authorized_read(authorized_read_audit(
                &descriptor.bundle_id,
                12,
            )),
            Err(RawSourceVaultError::Crypto { .. })
        ));

        let mut encrypted = fs::read(vault.bundle_path(&descriptor.bundle_id)).expect("ciphertext");
        let last = encrypted.last_mut().expect("ciphertext byte");
        *last ^= 0x01;
        fs::write(vault.bundle_path(&descriptor.bundle_id), encrypted).expect("tamper ciphertext");
        assert!(matches!(
            vault.decrypt_bundle_for_authorized_read(authorized_read_audit(
                &descriptor.bundle_id,
                13
            )),
            Err(RawSourceVaultError::Denied { .. })
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_rejects_tampered_authenticated_metadata() {
        let root = temp_vault_root("aad-tamper");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() { secret(); }".to_vec(),
                }],
            )
            .expect("capture bundle");
        vault
            .index
            .envelopes
            .get_mut(&descriptor.bundle_id)
            .expect("envelope")
            .aad_digest = "sha256:tampered".to_string();
        assert!(matches!(
            vault.decrypt_bundle_for_authorized_read(authorized_read_audit(
                &descriptor.bundle_id,
                14
            )),
            Err(RawSourceVaultError::Denied { .. })
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_rejects_out_of_scope_and_deletes_ciphertext() {
        let root = temp_vault_root("delete");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let out_of_scope = vec![RawSourceVaultFile {
            path: CanonicalPath("C:/repo/src/lib.rs".to_string()),
            bytes: b"fn lib() {}".to_vec(),
        }];
        assert!(matches!(
            vault.capture_bundle(grant(), request(), out_of_scope),
            Err(RawSourceVaultError::Denied { .. })
        ));

        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() {}".to_vec(),
                }],
            )
            .expect("capture bundle");
        let tombstone = RawSourceRetentionTombstone {
            bundle_id: descriptor.bundle_id.clone(),
            reason: "user_deleted".to_string(),
            deleted_at: TimestampMillis(70_000),
            event_sequence: EventSequence(5),
            correlation_id: CorrelationId(5),
            causality_id: CausalityId(uuid::Uuid::from_u128(
                0x018f_0000_0000_7000_8000_3000_0000_0005,
            )),
            schema_version: 1,
        };
        vault.delete_bundle(tombstone).expect("delete");
        assert!(matches!(
            vault.read_encrypted_bundle(&descriptor.bundle_id),
            Err(RawSourceVaultError::BundleMissing { .. })
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_rejects_duplicate_bundle_id_collision() {
        let root = temp_vault_root("duplicate");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let files = || {
            vec![RawSourceVaultFile {
                path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                bytes: b"fn main() {}".to_vec(),
            }]
        };
        vault
            .capture_bundle(grant(), request(), files())
            .expect("first capture");
        assert!(matches!(
            vault.capture_bundle(grant(), request(), files()),
            Err(RawSourceVaultError::Denied { .. })
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_delete_missing_bundle_is_rejected() {
        let root = temp_vault_root("delete-missing");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let tombstone = RawSourceRetentionTombstone {
            bundle_id: "bundle-missing".to_string(),
            reason: "user_deleted".to_string(),
            deleted_at: TimestampMillis(70_000),
            event_sequence: EventSequence(5),
            correlation_id: CorrelationId(5),
            causality_id: CausalityId(uuid::Uuid::from_u128(
                0x018f_0000_0000_7000_8000_3000_0000_0005,
            )),
            schema_version: 1,
        };
        assert!(matches!(
            vault.delete_bundle(tombstone),
            Err(RawSourceVaultError::BundleMissing { .. })
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_rejects_capture_exceeding_consent_byte_budget() {
        let root = temp_vault_root("byte-budget");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        // request().max_bytes is 1024; supply more raw bytes than the budget.
        let oversized = vec![RawSourceVaultFile {
            path: CanonicalPath("C:/repo/src/main.rs".to_string()),
            bytes: vec![b'a'; 2048],
        }];
        assert!(matches!(
            vault.capture_bundle(grant(), request(), oversized),
            Err(RawSourceVaultError::Denied { .. })
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_delete_tolerates_missing_ciphertext() {
        let root = temp_vault_root("delete-missing-ciphertext");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() {}".to_vec(),
                }],
            )
            .expect("capture bundle");
        fs::remove_file(vault.bundle_path(&descriptor.bundle_id)).expect("remove ciphertext");
        let tombstone = RawSourceRetentionTombstone {
            bundle_id: descriptor.bundle_id.clone(),
            reason: "user_deleted".to_string(),
            deleted_at: TimestampMillis(70_000),
            event_sequence: EventSequence(6),
            correlation_id: CorrelationId(6),
            causality_id: CausalityId(uuid::Uuid::from_u128(
                0x018f_0000_0000_7000_8000_3000_0000_0006,
            )),
            schema_version: 1,
        };
        vault
            .delete_bundle(tombstone)
            .expect("delete tolerates missing ciphertext");
        assert!(matches!(
            vault.read_bundle_descriptor(&descriptor.bundle_id),
            Err(RawSourceVaultError::BundleMissing { .. })
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn purge_expired_only_deletes_bundles_past_recorded_lease_expiry() {
        let root = temp_vault_root("purge-expired");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() {}".to_vec(),
                }],
            )
            .expect("capture bundle");
        assert_eq!(
            vault
                .purge_expired(TimestampMillis(59_999))
                .expect("early purge"),
            0
        );
        assert_eq!(
            vault
                .purge_expired(TimestampMillis(60_000))
                .expect("expired purge"),
            1
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_rotates_bundle_key_and_reopens_with_new_reference() {
        let root = temp_vault_root("key-rotation");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() { rotate_me(); }".to_vec(),
                }],
            )
            .expect("capture bundle");
        let previous_ciphertext = vault
            .read_encrypted_bundle(&descriptor.bundle_id)
            .expect("previous ciphertext");
        let new_provider =
            TestKeyProvider::with_generation(b"abcdef0123456789abcdef0123456789", "v2", 2);

        let record = vault
            .rotate_bundle_key(
                &descriptor.bundle_id,
                &new_provider,
                EventSequence(20),
                CorrelationId(20),
                CausalityId(uuid::Uuid::now_v7()),
            )
            .expect("rotate key");
        validate_raw_source_key_rotation_record(&record).expect("valid rotation record");
        assert_eq!(record.previous_key_reference.key_version, "v1");
        assert_eq!(record.new_key_reference.key_version, "v2");
        assert_eq!(record.new_key_reference.rotation_generation, 2);
        assert!(!format!("{record:?}").contains("rotate_me"));

        let rotated_ciphertext = vault
            .read_encrypted_bundle(&descriptor.bundle_id)
            .expect("rotated ciphertext");
        assert_ne!(previous_ciphertext, rotated_ciphertext);
        let rotated_envelope = vault
            .read_vault_envelope(&descriptor.bundle_id)
            .expect("rotated envelope");
        assert_eq!(rotated_envelope.key_reference.key_version, "v2");
        assert!(matches!(
            vault.decrypt_bundle_for_authorized_read(authorized_read_audit(
                &descriptor.bundle_id,
                21,
            )),
            Err(RawSourceVaultError::Denied { .. })
        ));

        let reopened = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            new_provider,
        )
        .expect("reopen with rotated key");
        let decrypted = reopened
            .decrypt_bundle_for_authorized_read(authorized_read_audit(&descriptor.bundle_id, 22))
            .expect("authorized decrypt after rotation");
        assert!(String::from_utf8_lossy(&decrypted).contains("rotate_me"));
        let index_text = fs::read_to_string(root.join("index.json")).expect("index text");
        assert!(!index_text.contains("rotate_me"));
        assert!(!index_text.contains("abcdef0123456789"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_rotation_fails_closed_when_old_key_cannot_decrypt() {
        let root = temp_vault_root("key-rotation-fail-closed");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() { rotate_me(); }".to_vec(),
                }],
            )
            .expect("capture bundle");
        drop(vault);

        let mut wrong_key_vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::wrong_key_same_reference(),
        )
        .expect("reopen with wrong key");
        let new_provider =
            TestKeyProvider::with_generation(b"abcdef0123456789abcdef0123456789", "v2", 2);
        assert!(matches!(
            wrong_key_vault.rotate_bundle_key(
                &descriptor.bundle_id,
                &new_provider,
                EventSequence(23),
                CorrelationId(23),
                CausalityId(uuid::Uuid::now_v7()),
            ),
            Err(RawSourceVaultError::Crypto { .. })
        ));
        let envelope = wrong_key_vault
            .read_vault_envelope(&descriptor.bundle_id)
            .expect("envelope unchanged");
        assert_eq!(envelope.key_reference.key_version, "v1");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn kms_envelope_contract_wraps_by_reference_without_key_material() {
        let key_provider = TestKeyProvider::default();
        let kms = TestKmsProvider {
            key: key_provider.key_bytes(),
        };
        let wrapped = kms
            .wrap_key(key_provider.key_reference(), &key_provider.key_bytes())
            .expect("wrap key");
        assert_eq!(wrapped.provider_label, "test-kms-envelope");
        assert_eq!(wrapped.wrapped_key_byte_len, 32);
        assert!(!format!("{wrapped:?}").contains("0123456789abcdef"));
        let unwrapped = kms.unwrap_key(&wrapped).expect("unwrap key");
        assert_eq!(&*unwrapped, key_provider.key_bytes().as_slice());
    }

    #[test]
    fn os_keyring_provider_exposes_metadata_reference_without_inline_key() {
        let provider = OsKeyringRawSourceKeyProvider::new(
            "legion-test",
            "workspace-1",
            RawSourceKeyReference {
                key_id: "key:os:test".to_string(),
                key_version: "v1".to_string(),
                provider_label: "local-os-keyring".to_string(),
                rotation_generation: 1,
                schema_version: 1,
            },
        )
        .expect("provider metadata");
        let reference = provider.key_reference();
        assert_eq!(reference.provider_label, "local-os-keyring");
        assert!(!format!("{reference:?}").contains("key_bytes"));
    }

    #[test]
    fn hosted_raw_export_uploads_encrypted_bundle_only_and_records_linkage() {
        let root = temp_vault_root("hosted-export");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() { secret(); }".to_vec(),
                }],
            )
            .expect("capture bundle");
        let mut client = AcceptingHostedRawExport {
            saw_plaintext: false,
        };
        let linkage = vault
            .export_encrypted_bundle_hosted(
                &descriptor.bundle_id,
                hosted_export_consent(),
                hosted_endpoint(),
                &mut client,
            )
            .expect("hosted export");
        assert_eq!(linkage.bundle_id, descriptor.bundle_id);
        assert!(linkage.raw_source_consent_verified);
        assert!(!client.saw_plaintext);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hosted_raw_export_requires_allowlisted_https_and_current_consent() {
        let root = temp_vault_root("hosted-export-denied");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() {}".to_vec(),
                }],
            )
            .expect("capture bundle");
        let mut client = AcceptingHostedRawExport {
            saw_plaintext: false,
        };
        assert!(matches!(
            vault.export_encrypted_bundle_hosted(
                &descriptor.bundle_id,
                RawSourceHostedExportConsent {
                    revoked: true,
                    ..hosted_export_consent()
                },
                hosted_endpoint(),
                &mut client,
            ),
            Err(RawSourceVaultError::Denied { .. })
        ));
        assert!(matches!(
            vault.export_encrypted_bundle_hosted(
                &descriptor.bundle_id,
                hosted_export_consent(),
                HostedTelemetryEndpointDescriptor {
                    endpoint_label: "http://support.invalid/raw".to_string(),
                    ..hosted_endpoint()
                },
                &mut client,
            ),
            Err(RawSourceVaultError::Denied { .. })
        ));
        assert!(matches!(
            vault.export_encrypted_bundle_hosted(
                &descriptor.bundle_id,
                RawSourceHostedExportConsent {
                    endpoint_id: "other-endpoint".to_string(),
                    ..hosted_export_consent()
                },
                hosted_endpoint(),
                &mut client,
            ),
            Err(RawSourceVaultError::Denied { .. })
        ));
        assert!(matches!(
            vault.export_encrypted_bundle_hosted(
                &descriptor.bundle_id,
                RawSourceHostedExportConsent {
                    issued_at: TimestampMillis(1),
                    expires_at: TimestampMillis(2),
                    ..hosted_export_consent()
                },
                hosted_endpoint(),
                &mut client,
            ),
            Err(RawSourceVaultError::Denied { .. })
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn vault_recovery_report_verifies_healthy_bundle_without_plaintext() {
        let root = temp_vault_root("recovery-healthy");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() { recovery_probe(); }".to_vec(),
                }],
            )
            .expect("capture bundle");

        let report = vault
            .inspect_bundle_recovery(
                &descriptor.bundle_id,
                EventSequence(24),
                CorrelationId(24),
                CausalityId(uuid::Uuid::now_v7()),
            )
            .expect("inspect recovery");
        validate_raw_source_vault_recovery_report(&report).expect("valid recovery report");
        assert_eq!(report.state, RawSourceVaultRecoveryState::Recovered);
        assert_eq!(report.metadata_summary, "metadata_verified");
        assert!(!format!("{report:?}").contains("recovery_probe"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn vault_recovery_report_quarantines_corrupt_ciphertext_without_plaintext() {
        let root = temp_vault_root("recovery-corrupt-ciphertext");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() { recovery_probe(); }".to_vec(),
                }],
            )
            .expect("capture bundle");
        let mut encrypted = fs::read(vault.bundle_path(&descriptor.bundle_id)).expect("ciphertext");
        let last = encrypted.last_mut().expect("ciphertext byte");
        *last ^= 0x01;
        fs::write(vault.bundle_path(&descriptor.bundle_id), encrypted).expect("tamper ciphertext");

        let report = vault
            .inspect_bundle_recovery(
                &descriptor.bundle_id,
                EventSequence(25),
                CorrelationId(25),
                CausalityId(uuid::Uuid::now_v7()),
            )
            .expect("inspect corrupt recovery");
        validate_raw_source_vault_recovery_report(&report).expect("valid recovery report");
        assert_eq!(report.state, RawSourceVaultRecoveryState::Quarantined);
        assert_eq!(report.metadata_summary, "ciphertext_digest_mismatch");
        assert!(!format!("{report:?}").contains("recovery_probe"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn vault_recovery_report_fails_closed_for_missing_ciphertext() {
        let root = temp_vault_root("recovery-missing-ciphertext");
        let mut vault = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("open vault");
        let (_lease, descriptor) = vault
            .capture_bundle(
                grant(),
                request(),
                vec![RawSourceVaultFile {
                    path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                    bytes: b"fn main() { recovery_probe(); }".to_vec(),
                }],
            )
            .expect("capture bundle");
        fs::remove_file(vault.bundle_path(&descriptor.bundle_id)).expect("remove ciphertext");

        let report = vault
            .inspect_bundle_recovery(
                &descriptor.bundle_id,
                EventSequence(26),
                CorrelationId(26),
                CausalityId(uuid::Uuid::now_v7()),
            )
            .expect("inspect missing recovery");
        validate_raw_source_vault_recovery_report(&report).expect("valid recovery report");
        assert_eq!(report.state, RawSourceVaultRecoveryState::FailedClosed);
        assert_eq!(report.metadata_summary, "ciphertext_missing");
        assert!(!format!("{report:?}").contains("recovery_probe"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn vault_reopen_recovers_descriptors_without_plaintext_leak() {
        let root = temp_vault_root("reopen");
        let descriptor = {
            let mut vault = FileBackedRawSourceVault::open_production(
                &root,
                policy(true),
                RawSourceVaultConfig::enabled(),
                TestKeyProvider::default(),
            )
            .expect("open vault");
            vault
                .capture_bundle(
                    grant(),
                    request(),
                    vec![RawSourceVaultFile {
                        path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                        bytes: b"fn main() {}".to_vec(),
                    }],
                )
                .expect("capture")
                .1
        };
        let reopened = FileBackedRawSourceVault::open_production(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider::default(),
        )
        .expect("reopen vault");
        let loaded = reopened
            .read_bundle_descriptor(&descriptor.bundle_id)
            .expect("descriptor");
        assert_eq!(loaded.bundle_id, descriptor.bundle_id);
        let index_text = fs::read_to_string(root.join("index.json")).expect("index text");
        assert!(!index_text.contains("fn main"));
        let _ = fs::remove_dir_all(root);
    }
}
