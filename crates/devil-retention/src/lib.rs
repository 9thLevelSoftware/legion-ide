//! Deterministic Phase 8 raw-source retention fixture vault.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use devil_protocol::{
    CanonicalPath, FileFingerprint, RawSourceCaptureRequest, RawSourceRetentionAccessAudit,
    RawSourceRetentionBundleDescriptor, RawSourceRetentionConsentGrant, RawSourceRetentionLease,
    RawSourceRetentionPolicy, RawSourceRetentionTombstone, TimestampMillis,
    validate_raw_source_capture_request, validate_raw_source_retention_access_audit,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    /// Return a metadata-safe key reference.
    fn key_reference(&self) -> String;
    /// Return key bytes used by the cipher implementation.
    fn key_bytes(&self) -> Vec<u8>;
}

/// Encryption abstraction for raw-source vault content.
pub trait RawSourceVaultCipher {
    /// Encrypt plaintext with key bytes.
    fn encrypt(&self, key: &[u8], plaintext: &[u8]) -> Vec<u8>;
    /// Decrypt ciphertext with key bytes.
    fn decrypt(&self, key: &[u8], ciphertext: &[u8]) -> Vec<u8>;
}

/// Deterministic XOR cipher used by tests and local development until a reviewed crypto dependency lands.
#[derive(Debug, Clone, Copy, Default)]
pub struct XorVaultCipher;

impl RawSourceVaultCipher for XorVaultCipher {
    fn encrypt(&self, key: &[u8], plaintext: &[u8]) -> Vec<u8> {
        xor_bytes(key, plaintext)
    }

    fn decrypt(&self, key: &[u8], ciphertext: &[u8]) -> Vec<u8> {
        xor_bytes(key, ciphertext)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistedVaultIndex {
    schema_version: u16,
    bundles: HashMap<String, RawSourceRetentionBundleDescriptor>,
    tombstones: HashMap<String, RawSourceRetentionTombstone>,
    key_references: HashMap<String, String>,
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
        let lease = RawSourceRetentionLease {
            lease_id: format!(
                "lease:{}:{}",
                request.workspace_id.0, request.correlation_id.0
            ),
            consent: grant,
            expires_at: request
                .paths
                .first()
                .map_or(devil_protocol::TimestampMillis(self.policy.ttl_ms), |_| {
                    devil_protocol::TimestampMillis(self.policy.ttl_ms)
                }),
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
        self.bundles.remove(&tombstone.bundle_id);
        self.tombstones
            .insert(tombstone.bundle_id.clone(), tombstone.clone());
        Ok(tombstone)
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
            serde_json::from_str(&text).map_err(|err| RawSourceVaultError::Io {
                message: format!("decode vault index: {err}"),
            })?
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
        let plaintext = self.pack_files(&request, files)?;
        if plaintext.is_empty() || plaintext.len() as u64 > self.config.max_bundle_bytes {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source bundle is empty or exceeds configured vault limit".to_string(),
            });
        }
        let key = self.key_provider.key_bytes();
        if key.is_empty() || self.key_provider.key_reference().trim().is_empty() {
            return Err(RawSourceVaultError::Denied {
                reason: "raw-source vault key reference is required".to_string(),
            });
        }
        let ciphertext = self.cipher.encrypt(&key, &plaintext);
        let bundle_id = format!(
            "bundle:{}:{}",
            request.workspace_id.0, request.correlation_id.0
        );
        fs::write(self.bundle_path(&bundle_id), &ciphertext).map_err(io_error)?;
        let lease = RawSourceRetentionLease {
            lease_id: format!(
                "lease:{}:{}",
                request.workspace_id.0, request.correlation_id.0
            ),
            consent: grant,
            expires_at: TimestampMillis(self.policy.ttl_ms),
            schema_version: 1,
        };
        let descriptor = RawSourceRetentionBundleDescriptor {
            bundle_id: bundle_id.clone(),
            lease_id: lease.lease_id.clone(),
            workspace_id: request.workspace_id,
            purpose: request.purpose,
            encrypted_byte_len: ciphertext.len() as u64,
            integrity: FileFingerprint {
                algorithm: "devil-vault-stable-sum-v1".to_string(),
                value: stable_sum(&ciphertext),
            },
            schema_version: 1,
        };
        self.index
            .key_references
            .insert(bundle_id.clone(), self.key_provider.key_reference());
        self.index.bundles.insert(bundle_id, descriptor.clone());
        self.flush_index()?;
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
        let encrypted = self.read_encrypted_bundle(&audit.bundle_id)?;
        Ok(self
            .cipher
            .decrypt(&self.key_provider.key_bytes(), &encrypted))
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
        self.index.bundles.remove(&tombstone.bundle_id);
        self.index.key_references.remove(&tombstone.bundle_id);
        let _ = fs::remove_file(self.bundle_path(&tombstone.bundle_id));
        self.index
            .tombstones
            .insert(tombstone.bundle_id.clone(), tombstone.clone());
        self.flush_index()?;
        Ok(tombstone)
    }

    /// Remove all descriptors whose lease TTL has expired and record tombstones.
    pub fn purge_expired(&mut self, now: TimestampMillis) -> Result<usize, RawSourceVaultError> {
        if now.0 < self.policy.ttl_ms {
            return Ok(0);
        }
        let bundle_ids = self.index.bundles.keys().cloned().collect::<Vec<_>>();
        let mut removed = 0usize;
        for bundle_id in bundle_ids {
            let tombstone = RawSourceRetentionTombstone {
                bundle_id: bundle_id.clone(),
                reason: "ttl_expired".to_string(),
                deleted_at: now,
                event_sequence: devil_protocol::EventSequence(now.0.max(1)),
                correlation_id: devil_protocol::CorrelationId(now.0.max(1)),
                causality_id: devil_protocol::CausalityId(uuid::Uuid::now_v7()),
                schema_version: 1,
            };
            self.delete_bundle(tombstone)?;
            removed += 1;
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
        for file in files {
            if !request.paths.contains(&file.path) {
                return Err(RawSourceVaultError::Denied {
                    reason: "raw-source file is outside capture request scope".to_string(),
                });
            }
            packed.extend_from_slice(file.path.0.as_bytes());
            packed.push(0);
            packed.extend_from_slice(&(file.bytes.len() as u64).to_le_bytes());
            packed.extend_from_slice(&file.bytes);
        }
        Ok(packed)
    }

    fn bundle_path(&self, bundle_id: &str) -> PathBuf {
        self.root.join(format!("{}.vault", safe_name(bundle_id)))
    }

    fn flush_index(&self) -> Result<(), RawSourceVaultError> {
        let text =
            serde_json::to_string_pretty(&self.index).map_err(|err| RawSourceVaultError::Io {
                message: format!("encode vault index: {err}"),
            })?;
        fs::write(self.root.join("index.json"), text).map_err(io_error)
    }
}

fn xor_bytes(key: &[u8], input: &[u8]) -> Vec<u8> {
    input
        .iter()
        .enumerate()
        .map(|(index, byte)| byte ^ key[index % key.len()])
        .collect()
}

fn stable_sum(bytes: &[u8]) -> String {
    let sum = bytes
        .iter()
        .fold(0u64, |acc, byte| acc.wrapping_add(*byte as u64));
    format!("sum:{sum}:len:{}", bytes.len())
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use devil_protocol::{
        CanonicalPath, CausalityId, CorrelationId, EventSequence, PrincipalId,
        RawSourceRetentionPurpose, RedactionHint, TimestampMillis, WorkspaceId,
    };

    use super::*;

    #[derive(Debug, Clone)]
    struct TestKeyProvider;

    impl RawSourceVaultKeyProvider for TestKeyProvider {
        fn key_reference(&self) -> String {
            "key:test".to_string()
        }

        fn key_bytes(&self) -> Vec<u8> {
            b"test-key".to_vec()
        }
    }

    fn temp_vault_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("devil-retention-{name}-{}", uuid::Uuid::now_v7()))
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
    fn file_backed_vault_encrypts_and_does_not_store_plaintext() {
        let root = temp_vault_root("encrypted");
        let mut vault = FileBackedRawSourceVault::open(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider,
            XorVaultCipher,
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
        assert!(!String::from_utf8_lossy(&encrypted).contains("fn main"));

        let audit = RawSourceRetentionAccessAudit {
            bundle_id: descriptor.bundle_id.clone(),
            principal_id: PrincipalId("tester".to_string()),
            action: "authorized_read".to_string(),
            event_sequence: EventSequence(4),
            correlation_id: CorrelationId(4),
            causality_id: CausalityId(uuid::Uuid::from_u128(
                0x018f_0000_0000_7000_8000_3000_0000_0004,
            )),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let decrypted = vault
            .decrypt_bundle_for_authorized_read(audit)
            .expect("authorized decrypt");
        assert!(String::from_utf8_lossy(&decrypted).contains("fn main"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_backed_vault_rejects_out_of_scope_and_deletes_ciphertext() {
        let root = temp_vault_root("delete");
        let mut vault = FileBackedRawSourceVault::open(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider,
            XorVaultCipher,
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
    fn vault_reopen_recovers_descriptors_without_plaintext_leak() {
        let root = temp_vault_root("reopen");
        let descriptor = {
            let mut vault = FileBackedRawSourceVault::open(
                &root,
                policy(true),
                RawSourceVaultConfig::enabled(),
                TestKeyProvider,
                XorVaultCipher,
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
        let reopened = FileBackedRawSourceVault::open(
            &root,
            policy(true),
            RawSourceVaultConfig::enabled(),
            TestKeyProvider,
            XorVaultCipher,
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
