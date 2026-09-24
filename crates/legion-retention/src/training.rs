//! Raw-trace opt-in ledger, redaction boundary, deletion handles, and export controls.
//!
//! The retention vault already knows how to encrypt, delete, and audit a bundle. What
//! it did not have is an authority for *whether a bundle may exist at all*: the consent
//! grant was a struct the caller passed in, so a caller could construct its own consent
//! and capture against it. This module supplies the missing authority.
//!
//! Every raw-trace capture, export, and attestation goes through a
//! [`RawTraceOptInLedger`] row. The row is the record of the user having opted in; it
//! is looked up (never supplied), it expires, and it can be revoked. Consent grants are
//! *derived* from a live row rather than accepted from the caller, so there is no path
//! that stores a raw trace without an opt-in row backing it.
//!
//! Redaction is enforced before the bytes reach the vault, because the vault seals them
//! and after that no component in this process can see the plaintext again.

use std::collections::BTreeMap;

use legion_observability::training::RawTraceOptInAttestation;
use legion_protocol::{
    CanonicalPath, CausalityId, CorrelationId, EventSequence, HostedRetentionExportLinkage,
    PrincipalId, RawSourceCaptureRequest, RawSourceRetentionBundleDescriptor,
    RawSourceRetentionConsentGrant, RawSourceRetentionLease, RawSourceRetentionPurpose,
    RawSourceRetentionTombstone, TimestampMillis, WorkspaceId,
    validate_hosted_retention_export_linkage,
};
use thiserror::Error;

use crate::privacy::execute_privacy_deletion;
use crate::{
    FileBackedRawSourceVault, RawSourceVaultCipher, RawSourceVaultError, RawSourceVaultFile,
    RawSourceVaultKeyProvider, scan_raw_source_capture_files,
};

/// Schema version stamped on the artifacts this module produces.
pub const RAW_TRACE_OPT_IN_SCHEMA_VERSION: u16 = 1;

/// A recorded raw-trace opt-in.
///
/// This is the row the stop condition refers to: no row, no raw trace. It is bound to a
/// principal, a workspace, a purpose, and a path scope, and it expires. Deleting or
/// revoking the row is what makes future captures and exports fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTraceOptInRow {
    /// Stable opt-in row identifier.
    pub row_id: String,
    /// Principal who opted in.
    pub principal_id: PrincipalId,
    /// Workspace the opt-in covers.
    pub workspace_id: WorkspaceId,
    /// Purpose the opt-in is bound to.
    pub purpose: RawSourceRetentionPurpose,
    /// Canonical paths the opt-in covers.
    pub path_scope: Vec<CanonicalPath>,
    /// When the opt-in was recorded.
    pub granted_at: TimestampMillis,
    /// When the opt-in lapses.
    pub expires_at: TimestampMillis,
    /// Whether this opt-in additionally permits hosted export of the raw trace.
    ///
    /// Separate from retention on purpose: agreeing to keep a raw trace on your own
    /// machine is not agreeing to ship it somewhere else.
    pub export_allowed: bool,
    /// Correlation identifier for the opt-in event.
    pub correlation_id: CorrelationId,
    /// Row schema version.
    pub schema_version: u16,
}

/// Failures raised by the raw-trace opt-in ledger.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RawTraceOptInError {
    /// The row failed validation and was not recorded.
    #[error("raw-trace opt-in row is invalid: {reason}")]
    InvalidRow {
        /// Which field made the row invalid.
        reason: &'static str,
    },
    /// No opt-in row covers the requested workspace, principal, and purpose.
    #[error(
        "no raw-trace opt-in row covers workspace {workspace_id} / principal `{principal_id}` / purpose {purpose:?}"
    )]
    NoOptInRow {
        /// Requested workspace.
        workspace_id: u128,
        /// Requested principal.
        principal_id: String,
        /// Requested purpose.
        purpose: RawSourceRetentionPurpose,
    },
    /// The covering opt-in row has lapsed.
    #[error("raw-trace opt-in row `{row_id}` expired at {expires_at} (now {now})")]
    ExpiredOptInRow {
        /// Row that lapsed.
        row_id: String,
        /// Row expiry.
        expires_at: u64,
        /// Evaluation time.
        now: u64,
    },
    /// The covering opt-in row does not permit hosted export.
    #[error("raw-trace opt-in row `{row_id}` does not permit hosted export")]
    ExportNotPermitted {
        /// Row that refused export.
        row_id: String,
    },
    /// The capture payload carried credentials and must not be retained.
    #[error("raw-trace capture carries detected credentials: {summary}")]
    RedactionRequired {
        /// Display-safe scan summary; rule ids and counts, never matched bytes.
        summary: String,
    },
}

impl From<RawTraceOptInError> for RawSourceVaultError {
    fn from(error: RawTraceOptInError) -> Self {
        RawSourceVaultError::Denied {
            reason: error.to_string(),
        }
    }
}

/// The set of recorded raw-trace opt-ins.
#[derive(Debug, Clone, Default)]
pub struct RawTraceOptInLedger {
    rows: BTreeMap<String, RawTraceOptInRow>,
}

impl RawTraceOptInLedger {
    /// Create an empty ledger.
    ///
    /// Empty means nothing may be retained: the default posture is no raw traces.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of recorded opt-in rows.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Record an opt-in row.
    pub fn record_opt_in(&mut self, row: RawTraceOptInRow) -> Result<(), RawTraceOptInError> {
        validate_row(&row)?;
        self.rows.insert(row.row_id.clone(), row);
        Ok(())
    }

    /// Revoke an opt-in row. Returns whether a row was present.
    ///
    /// Revocation removes the row outright rather than flagging it: a revoked opt-in
    /// that still sits in the ledger is one `if` away from being honoured again.
    pub fn revoke(&mut self, row_id: &str) -> bool {
        self.rows.remove(row_id).is_some()
    }

    /// Look up the live opt-in row covering a workspace, principal, and purpose.
    pub fn active_row(
        &self,
        workspace_id: WorkspaceId,
        principal_id: &PrincipalId,
        purpose: RawSourceRetentionPurpose,
        now: TimestampMillis,
    ) -> Result<&RawTraceOptInRow, RawTraceOptInError> {
        let row = self
            .rows
            .values()
            .find(|row| {
                row.workspace_id == workspace_id
                    && &row.principal_id == principal_id
                    && row.purpose == purpose
            })
            .ok_or_else(|| RawTraceOptInError::NoOptInRow {
                workspace_id: workspace_id.0,
                principal_id: principal_id.0.clone(),
                purpose,
            })?;
        if row.expires_at.0 <= now.0 {
            return Err(RawTraceOptInError::ExpiredOptInRow {
                row_id: row.row_id.clone(),
                expires_at: row.expires_at.0,
                now: now.0,
            });
        }
        Ok(row)
    }

    /// Mint a metadata-only attestation from a live opt-in row.
    ///
    /// `redaction_enforced` is hard-coded true because the only capture path that
    /// accepts these rows — [`capture_raw_trace_under_opt_in`] — scans for credentials
    /// before anything is sealed. If a second capture path is ever added, it must run
    /// the same scan or this flag becomes a lie.
    pub fn attest(
        &self,
        workspace_id: WorkspaceId,
        principal_id: &PrincipalId,
        purpose: RawSourceRetentionPurpose,
        now: TimestampMillis,
    ) -> Result<RawTraceOptInAttestation, RawTraceOptInError> {
        let row = self.active_row(workspace_id, principal_id, purpose, now)?;
        Ok(RawTraceOptInAttestation {
            row_id: row.row_id.clone(),
            workspace_id: row.workspace_id,
            purpose_label: format!("{:?}", row.purpose),
            expires_at: row.expires_at,
            export_allowed: row.export_allowed,
            redaction_enforced: true,
            schema_version: RAW_TRACE_OPT_IN_SCHEMA_VERSION,
        })
    }
}

/// Derive a consent grant from a live opt-in row.
///
/// The grant is never accepted from a caller; it is reconstructed here so its scope can
/// only ever be the row's scope.
fn consent_grant_from_row(row: &RawTraceOptInRow) -> RawSourceRetentionConsentGrant {
    RawSourceRetentionConsentGrant {
        principal_id: row.principal_id.clone(),
        workspace_id: row.workspace_id,
        purpose: row.purpose,
        path_scope: row.path_scope.clone(),
        expires_at: row.expires_at,
        correlation_id: row.correlation_id,
        schema_version: row.schema_version,
    }
}

/// Capture a raw trace into the vault under a live opt-in row.
///
/// Fails closed on every step: no row, an expired row, a payload carrying credentials,
/// or a request outside the row's scope all refuse before any byte is written.
pub fn capture_raw_trace_under_opt_in<K: RawSourceVaultKeyProvider, C: RawSourceVaultCipher>(
    vault: &mut FileBackedRawSourceVault<K, C>,
    ledger: &RawTraceOptInLedger,
    request: RawSourceCaptureRequest,
    files: Vec<RawSourceVaultFile>,
    now: TimestampMillis,
) -> Result<(RawSourceRetentionLease, RawSourceRetentionBundleDescriptor), RawSourceVaultError> {
    let row = ledger.active_row(
        request.workspace_id,
        &request.principal_id,
        request.purpose,
        now,
    )?;

    // Scan before the grant is derived and before the vault seals anything. Once the
    // bundle is encrypted this process can no longer inspect the plaintext, so this is
    // the last point at which a credential can be refused rather than retained.
    let scan = scan_raw_source_capture_files(&files);
    if !scan.is_clean() {
        return Err(RawTraceOptInError::RedactionRequired {
            summary: scan.display_safe_summary(),
        }
        .into());
    }

    vault.capture_bundle(consent_grant_from_row(row), request, files)
}

/// Build a hosted export linkage for a retained raw trace under a live opt-in row.
///
/// `raw_source_consent_verified` is derived from the row's `export_allowed` flag, never
/// from a caller-supplied boolean, so a caller cannot assert its own export consent.
pub fn export_raw_trace_under_opt_in(
    ledger: &RawTraceOptInLedger,
    descriptor: &RawSourceRetentionBundleDescriptor,
    principal_id: &PrincipalId,
    telemetry_batch_id: impl Into<String>,
    now: TimestampMillis,
) -> Result<HostedRetentionExportLinkage, RawSourceVaultError> {
    let row = ledger.active_row(
        descriptor.workspace_id,
        principal_id,
        descriptor.purpose,
        now,
    )?;
    if !row.export_allowed {
        return Err(RawTraceOptInError::ExportNotPermitted {
            row_id: row.row_id.clone(),
        }
        .into());
    }
    build_hosted_raw_source_export_linkage(
        telemetry_batch_id,
        descriptor.bundle_id.clone(),
        true,
        RAW_TRACE_OPT_IN_SCHEMA_VERSION,
    )
}

/// Delete a retained raw trace and return a metadata-only deletion handle.
///
/// Deletion is deliberately *not* gated on a live opt-in row. A user whose opt-in has
/// lapsed or been revoked is exactly the user most likely to want the already-retained
/// bytes gone, and refusing them would turn an expired consent into permanent storage.
pub fn delete_raw_trace_under_opt_in<K: RawSourceVaultKeyProvider, C: RawSourceVaultCipher>(
    vault: &mut FileBackedRawSourceVault<K, C>,
    bundle_id: &str,
    reason: &str,
    deleted_at: TimestampMillis,
    event_sequence: EventSequence,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
) -> Result<String, RawSourceVaultError> {
    execute_privacy_deletion(
        vault,
        bundle_id,
        reason,
        deleted_at,
        event_sequence,
        correlation_id,
        causality_id,
    )
}

/// Build a metadata-only raw-source deletion tombstone.
pub fn build_raw_source_deletion_tombstone(
    bundle_id: impl Into<String>,
    reason: impl Into<String>,
    deleted_at: TimestampMillis,
    event_sequence: EventSequence,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
    schema_version: u16,
) -> Result<RawSourceRetentionTombstone, RawSourceVaultError> {
    let tombstone = RawSourceRetentionTombstone {
        bundle_id: bundle_id.into(),
        reason: reason.into(),
        deleted_at,
        event_sequence,
        correlation_id,
        causality_id,
        schema_version,
    };
    validate_tombstone(&tombstone).map_err(|err| RawSourceVaultError::Denied {
        reason: err.to_string(),
    })?;
    Ok(tombstone)
}

/// Build a metadata-only hosted export linkage after verifying separate raw-source consent.
pub fn build_hosted_raw_source_export_linkage(
    telemetry_batch_id: impl Into<String>,
    bundle_id: impl Into<String>,
    raw_source_consent_verified: bool,
    schema_version: u16,
) -> Result<HostedRetentionExportLinkage, RawSourceVaultError> {
    let linkage = HostedRetentionExportLinkage {
        telemetry_batch_id: telemetry_batch_id.into(),
        bundle_id: bundle_id.into(),
        raw_source_consent_verified,
        schema_version,
    };
    validate_hosted_retention_export_linkage(&linkage).map_err(|err| {
        RawSourceVaultError::Denied {
            reason: err.message,
        }
    })?;
    Ok(linkage)
}

fn validate_row(row: &RawTraceOptInRow) -> Result<(), RawTraceOptInError> {
    if row.row_id.trim().is_empty() {
        return Err(RawTraceOptInError::InvalidRow { reason: "row_id" });
    }
    if row.principal_id.0.trim().is_empty() {
        return Err(RawTraceOptInError::InvalidRow {
            reason: "principal_id",
        });
    }
    if row.workspace_id.0 == 0 {
        return Err(RawTraceOptInError::InvalidRow {
            reason: "workspace_id",
        });
    }
    if row.path_scope.is_empty() {
        return Err(RawTraceOptInError::InvalidRow {
            reason: "path_scope",
        });
    }
    if row.correlation_id.0 == 0 {
        return Err(RawTraceOptInError::InvalidRow {
            reason: "correlation_id",
        });
    }
    if row.schema_version == 0 {
        return Err(RawTraceOptInError::InvalidRow {
            reason: "schema_version",
        });
    }
    // An opt-in that never lapses is not an opt-in decision, it is a permanent one.
    if row.expires_at.0 <= row.granted_at.0 {
        return Err(RawTraceOptInError::InvalidRow {
            reason: "expires_at",
        });
    }
    Ok(())
}

fn validate_tombstone(
    tombstone: &RawSourceRetentionTombstone,
) -> Result<(), TrainingRetentionError> {
    if tombstone.bundle_id.trim().is_empty()
        || tombstone.reason.trim().is_empty()
        || tombstone.deleted_at.0 == 0
        || tombstone.event_sequence.0 == 0
        || tombstone.correlation_id.0 == 0
        || tombstone.causality_id.0.is_nil()
        || tombstone.schema_version == 0
    {
        return Err(TrainingRetentionError::InvalidMetadata);
    }
    Ok(())
}

#[derive(Debug, Error)]
enum TrainingRetentionError {
    #[error("metadata-only raw-source deletion handle is invalid")]
    InvalidMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RawSourceVaultConfig, RawSourceVaultKeyProvider};
    use legion_protocol::{RawSourceKeyReference, RawSourceRetentionPolicy};
    use uuid::Uuid;

    const NOW: TimestampMillis = TimestampMillis(100_000);
    const WORKSPACE: WorkspaceId = WorkspaceId(1);
    const SCOPED_PATH: &str = "C:/repo/src/main.rs";

    #[derive(Debug, Clone, Default)]
    struct TestKeyProvider;

    impl RawSourceVaultKeyProvider for TestKeyProvider {
        fn key_reference(&self) -> RawSourceKeyReference {
            RawSourceKeyReference {
                key_id: "key:test".to_string(),
                key_version: "v1".to_string(),
                provider_label: "test-keyring".to_string(),
                rotation_generation: 1,
                schema_version: 1,
            }
        }

        fn key_bytes(&self) -> Vec<u8> {
            b"0123456789abcdef0123456789abcdef".to_vec()
        }
    }

    fn principal() -> PrincipalId {
        PrincipalId("test-user".to_string())
    }

    fn temp_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "legion-raw-trace-opt-in-{label}-{}-{}",
            std::process::id(),
            TimestampMillis::now().0
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn policy() -> RawSourceRetentionPolicy {
        RawSourceRetentionPolicy {
            capture_enabled: true,
            allowed_purposes: vec![RawSourceRetentionPurpose::Replay],
            max_bundle_bytes: 8192,
            ttl_ms: 600_000,
            schema_version: 1,
        }
    }

    fn open_vault(
        label: &str,
    ) -> (
        std::path::PathBuf,
        FileBackedRawSourceVault<TestKeyProvider, crate::ChaCha20Poly1305VaultCipher>,
    ) {
        open_vault_with_config(label, RawSourceVaultConfig::enabled())
    }

    fn open_vault_with_config(
        label: &str,
        config: RawSourceVaultConfig,
    ) -> (
        std::path::PathBuf,
        FileBackedRawSourceVault<TestKeyProvider, crate::ChaCha20Poly1305VaultCipher>,
    ) {
        let root = temp_root(label);
        let vault =
            FileBackedRawSourceVault::open_production(&root, policy(), config, TestKeyProvider)
                .expect("open vault");
        (root, vault)
    }

    fn opt_in_row() -> RawTraceOptInRow {
        RawTraceOptInRow {
            row_id: "raw-trace-opt-in:ws-1:replay".to_string(),
            principal_id: principal(),
            workspace_id: WORKSPACE,
            purpose: RawSourceRetentionPurpose::Replay,
            path_scope: vec![CanonicalPath(SCOPED_PATH.to_string())],
            granted_at: TimestampMillis(50_000),
            expires_at: TimestampMillis(900_000),
            export_allowed: false,
            correlation_id: CorrelationId(7),
            schema_version: 1,
        }
    }

    fn ledger_with_row(row: RawTraceOptInRow) -> RawTraceOptInLedger {
        let mut ledger = RawTraceOptInLedger::new();
        ledger.record_opt_in(row).expect("record opt-in row");
        ledger
    }

    fn capture_request() -> RawSourceCaptureRequest {
        RawSourceCaptureRequest {
            workspace_id: WORKSPACE,
            principal_id: principal(),
            purpose: RawSourceRetentionPurpose::Replay,
            paths: vec![CanonicalPath(SCOPED_PATH.to_string())],
            max_bytes: 4096,
            correlation_id: CorrelationId(7),
            causality_id: CausalityId(Uuid::from_u128(0x018f_0000_0000_7000_8000_0000_0000_0007)),
            schema_version: 1,
        }
    }

    fn clean_files() -> Vec<RawSourceVaultFile> {
        vec![RawSourceVaultFile {
            path: CanonicalPath(SCOPED_PATH.to_string()),
            bytes: b"fn main() { println!(\"hello\"); }".to_vec(),
        }]
    }

    /// Count sealed ciphertext files under the vault root.
    fn vault_file_count(root: &std::path::Path) -> usize {
        std::fs::read_dir(root)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry
                            .path()
                            .extension()
                            .is_some_and(|extension| extension == "vault")
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Opt-in row is the authority
    // -----------------------------------------------------------------------

    #[test]
    fn capture_under_a_live_opt_in_row_stores_the_bundle() {
        let (root, mut vault) = open_vault("capture-ok");
        let ledger = ledger_with_row(opt_in_row());

        let (lease, descriptor) = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            clean_files(),
            NOW,
        )
        .expect("capture under a live opt-in row");

        assert_eq!(lease.consent.principal_id, principal());
        assert_eq!(descriptor.workspace_id, WORKSPACE);
        assert_eq!(vault_file_count(&root), 1);
        assert!(
            vault.read_encrypted_bundle(&descriptor.bundle_id).is_ok(),
            "the sealed bundle must be readable after capture"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// NEGATIVE: this is the stop condition. An empty ledger must store nothing.
    #[test]
    fn capture_without_an_opt_in_row_is_denied_and_stores_nothing() {
        let (root, mut vault) = open_vault("capture-no-row");
        let ledger = RawTraceOptInLedger::new();

        let err = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            clean_files(),
            NOW,
        )
        .expect_err("capture without an opt-in row must be denied");

        assert!(
            matches!(&err, RawSourceVaultError::Denied { reason } if reason.contains("no raw-trace opt-in row")),
            "unexpected error: {err}"
        );
        assert_eq!(
            vault_file_count(&root),
            0,
            "a denied capture must leave no sealed bundle behind"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// NEGATIVE: an expired opt-in row is not an opt-in row.
    #[test]
    fn capture_under_an_expired_opt_in_row_is_denied_and_stores_nothing() {
        let (root, mut vault) = open_vault("capture-expired");
        let mut row = opt_in_row();
        row.expires_at = TimestampMillis(60_000);
        let ledger = ledger_with_row(row);

        let err = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            clean_files(),
            NOW,
        )
        .expect_err("capture under an expired opt-in row must be denied");

        assert!(
            matches!(&err, RawSourceVaultError::Denied { reason } if reason.contains("expired")),
            "unexpected error: {err}"
        );
        assert_eq!(vault_file_count(&root), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// NEGATIVE: revoking the row must immediately stop new captures.
    #[test]
    fn capture_after_revocation_is_denied_and_stores_nothing() {
        let (root, mut vault) = open_vault("capture-revoked");
        let mut ledger = ledger_with_row(opt_in_row());
        assert!(ledger.revoke("raw-trace-opt-in:ws-1:replay"));
        assert_eq!(ledger.row_count(), 0);

        let err = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            clean_files(),
            NOW,
        )
        .expect_err("capture after revocation must be denied");

        assert!(
            matches!(&err, RawSourceVaultError::Denied { reason } if reason.contains("no raw-trace opt-in row")),
            "unexpected error: {err}"
        );
        assert_eq!(vault_file_count(&root), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// NEGATIVE: an opt-in row for one purpose must not authorize another purpose.
    #[test]
    fn capture_outside_the_opt_in_purpose_is_denied() {
        let (root, mut vault) = open_vault("capture-purpose");
        let mut row = opt_in_row();
        row.purpose = RawSourceRetentionPurpose::SupportBundle;
        let ledger = ledger_with_row(row);

        let err = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            clean_files(),
            NOW,
        )
        .expect_err("a purpose outside the opt-in row must be denied");

        assert!(
            matches!(&err, RawSourceVaultError::Denied { reason } if reason.contains("no raw-trace opt-in row")),
            "unexpected error: {err}"
        );
        assert_eq!(vault_file_count(&root), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// NEGATIVE: an opt-in row cannot be widened by asking for a path it does not cover.
    #[test]
    fn capture_outside_the_opt_in_path_scope_is_denied() {
        let (root, mut vault) = open_vault("capture-scope");
        let ledger = ledger_with_row(opt_in_row());
        let mut request = capture_request();
        request.paths = vec![CanonicalPath("C:/repo/secrets/.env".to_string())];

        let err = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            request,
            vec![RawSourceVaultFile {
                path: CanonicalPath("C:/repo/secrets/.env".to_string()),
                bytes: b"NOTHING_SENSITIVE=1".to_vec(),
            }],
            NOW,
        )
        .expect_err("a path outside the opt-in scope must be denied");

        assert!(
            matches!(&err, RawSourceVaultError::Denied { reason } if reason.contains("outside consent scope")),
            "unexpected error: {err}"
        );
        assert_eq!(vault_file_count(&root), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    // -----------------------------------------------------------------------
    // Redaction boundary
    // -----------------------------------------------------------------------

    /// NEGATIVE: a payload carrying a credential must be refused before it is sealed.
    #[test]
    fn capture_carrying_a_credential_is_denied_and_stores_nothing() {
        let (root, mut vault) = open_vault("capture-secret");
        let ledger = ledger_with_row(opt_in_row());

        let err = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            vec![RawSourceVaultFile {
                path: CanonicalPath(SCOPED_PATH.to_string()),
                bytes: b"AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_vec(),
            }],
            NOW,
        )
        .expect_err("a payload carrying a credential must be denied");

        assert!(
            matches!(&err, RawSourceVaultError::Denied { reason } if reason.contains("detected credentials")),
            "unexpected error: {err}"
        );
        assert_eq!(
            vault_file_count(&root),
            0,
            "a credential-bearing payload must never be sealed"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// NEGATIVE: the redaction boundary belongs to this gate, not to the vault's
    /// configuration. A vault deliberately opened with credential denial switched off
    /// must still not receive a credential through the raw-trace path.
    #[test]
    fn capture_carrying_a_credential_is_denied_even_when_the_vault_would_accept_it() {
        let permissive = RawSourceVaultConfig {
            deny_capture_on_detected_secrets: false,
            ..RawSourceVaultConfig::enabled()
        };
        let (root, mut vault) = open_vault_with_config("capture-secret-permissive", permissive);
        let ledger = ledger_with_row(opt_in_row());

        let err = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            vec![RawSourceVaultFile {
                path: CanonicalPath(SCOPED_PATH.to_string()),
                bytes: b"AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_vec(),
            }],
            NOW,
        )
        .expect_err("the gate's own scan must refuse regardless of vault configuration");

        assert!(
            matches!(&err, RawSourceVaultError::Denied { reason } if reason.contains("detected credentials")),
            "unexpected error: {err}"
        );
        assert_eq!(vault_file_count(&root), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    // -----------------------------------------------------------------------
    // Export controls
    // -----------------------------------------------------------------------

    #[test]
    fn export_is_permitted_when_the_opt_in_row_allows_it() {
        let (root, mut vault) = open_vault("export-ok");
        let mut row = opt_in_row();
        row.export_allowed = true;
        let ledger = ledger_with_row(row);
        let (_lease, descriptor) = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            clean_files(),
            NOW,
        )
        .expect("capture");

        let linkage = export_raw_trace_under_opt_in(
            &ledger,
            &descriptor,
            &principal(),
            "raw-export:batch-1",
            NOW,
        )
        .expect("export under an export-allowed opt-in row");

        assert!(linkage.raw_source_consent_verified);
        assert_eq!(linkage.bundle_id, descriptor.bundle_id);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// NEGATIVE: retention consent is not export consent.
    #[test]
    fn export_is_refused_when_the_opt_in_row_does_not_allow_export() {
        let (root, mut vault) = open_vault("export-denied");
        let ledger = ledger_with_row(opt_in_row());
        let (_lease, descriptor) = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            clean_files(),
            NOW,
        )
        .expect("capture");

        let err = export_raw_trace_under_opt_in(
            &ledger,
            &descriptor,
            &principal(),
            "raw-export:batch-1",
            NOW,
        )
        .expect_err("export must be refused without an export-allowing opt-in row");

        assert!(
            matches!(&err, RawSourceVaultError::Denied { reason } if reason.contains("does not permit hosted export")),
            "unexpected error: {err}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// NEGATIVE: revoking the row must stop export of already-retained bundles.
    #[test]
    fn export_is_refused_after_the_opt_in_row_is_revoked() {
        let (root, mut vault) = open_vault("export-revoked");
        let mut row = opt_in_row();
        row.export_allowed = true;
        let mut ledger = ledger_with_row(row);
        let (_lease, descriptor) = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            clean_files(),
            NOW,
        )
        .expect("capture");
        assert!(ledger.revoke("raw-trace-opt-in:ws-1:replay"));

        let err = export_raw_trace_under_opt_in(
            &ledger,
            &descriptor,
            &principal(),
            "raw-export:batch-1",
            NOW,
        )
        .expect_err("export must be refused after revocation");

        assert!(
            matches!(&err, RawSourceVaultError::Denied { reason } if reason.contains("no raw-trace opt-in row")),
            "unexpected error: {err}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// NEGATIVE: the underlying linkage builder still refuses unverified consent, so a
    /// caller bypassing the ledger cannot mint an export linkage by hand.
    #[test]
    fn hosted_export_linkage_requires_verified_raw_source_consent() {
        let err =
            build_hosted_raw_source_export_linkage("raw-export:batch-1", "bundle:1:7", false, 1)
                .expect_err("export linkage must require verified raw-source consent");

        assert!(matches!(err, RawSourceVaultError::Denied { .. }));
    }

    // -----------------------------------------------------------------------
    // Deletion handles
    // -----------------------------------------------------------------------

    #[test]
    fn deletion_handle_removes_the_ciphertext_and_the_descriptor() {
        let (root, mut vault) = open_vault("delete");
        let ledger = ledger_with_row(opt_in_row());
        let (_lease, descriptor) = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            clean_files(),
            NOW,
        )
        .expect("capture");
        assert_eq!(vault_file_count(&root), 1);

        let handle = delete_raw_trace_under_opt_in(
            &mut vault,
            &descriptor.bundle_id,
            "user_deleted",
            TimestampMillis(200_000),
            EventSequence(5),
            CorrelationId(7),
            CausalityId(Uuid::from_u128(0x018f_0000_0000_7000_8000_3000_0000_0005)),
        )
        .expect("deletion handle");

        assert!(handle.contains(&descriptor.bundle_id));
        assert!(handle.contains("user_deleted"));
        assert_eq!(
            vault_file_count(&root),
            0,
            "deletion must remove the sealed ciphertext from disk"
        );
        assert!(
            matches!(
                vault.read_bundle_descriptor(&descriptor.bundle_id),
                Err(RawSourceVaultError::BundleMissing { .. })
            ),
            "deletion must remove the descriptor"
        );
        assert!(
            vault.read_encrypted_bundle(&descriptor.bundle_id).is_err(),
            "deletion must make the sealed bytes unreadable"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Deletion must still work once the opt-in has lapsed: an expired consent must not
    /// become a reason to keep bytes the user asked to have removed.
    #[test]
    fn deletion_still_works_after_the_opt_in_row_is_revoked() {
        let (root, mut vault) = open_vault("delete-revoked");
        let mut ledger = ledger_with_row(opt_in_row());
        let (_lease, descriptor) = capture_raw_trace_under_opt_in(
            &mut vault,
            &ledger,
            capture_request(),
            clean_files(),
            NOW,
        )
        .expect("capture");
        assert!(ledger.revoke("raw-trace-opt-in:ws-1:replay"));

        delete_raw_trace_under_opt_in(
            &mut vault,
            &descriptor.bundle_id,
            "consent_revoked",
            TimestampMillis(200_000),
            EventSequence(6),
            CorrelationId(7),
            CausalityId(Uuid::from_u128(0x018f_0000_0000_7000_8000_3000_0000_0006)),
        )
        .expect("deletion must succeed after revocation");

        assert_eq!(vault_file_count(&root), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn deletion_handle_is_metadata_only_and_validated() {
        let tombstone = build_raw_source_deletion_tombstone(
            "bundle:ws-1:42",
            "user_deleted",
            TimestampMillis(70_000),
            EventSequence(5),
            CorrelationId(5),
            CausalityId(Uuid::from_u128(0x018f_0000_0000_7000_8000_3000_0000_0005)),
            1,
        )
        .expect("valid deletion handle");

        assert_eq!(tombstone.bundle_id, "bundle:ws-1:42");
        assert_eq!(tombstone.reason, "user_deleted");
        assert_eq!(tombstone.schema_version, 1);
    }

    // -----------------------------------------------------------------------
    // Attestation minting
    // -----------------------------------------------------------------------

    #[test]
    fn attestation_is_minted_from_a_live_opt_in_row() {
        let ledger = ledger_with_row(opt_in_row());

        let attestation = ledger
            .attest(
                WORKSPACE,
                &principal(),
                RawSourceRetentionPurpose::Replay,
                NOW,
            )
            .expect("attestation from a live row");

        assert_eq!(attestation.row_id, "raw-trace-opt-in:ws-1:replay");
        assert_eq!(attestation.purpose_label, "Replay");
        assert_eq!(attestation.expires_at, TimestampMillis(900_000));
        assert!(!attestation.export_allowed);
        assert!(attestation.redaction_enforced);
    }

    /// NEGATIVE: with no row there is no attestation, so the training boundary in
    /// `legion-observability` cannot be unlocked.
    #[test]
    fn attestation_is_refused_without_a_live_opt_in_row() {
        let empty = RawTraceOptInLedger::new();
        assert!(matches!(
            empty.attest(
                WORKSPACE,
                &principal(),
                RawSourceRetentionPurpose::Replay,
                NOW
            ),
            Err(RawTraceOptInError::NoOptInRow { .. })
        ));

        let mut expired_row = opt_in_row();
        expired_row.expires_at = TimestampMillis(60_000);
        let expired = ledger_with_row(expired_row);
        assert!(matches!(
            expired.attest(
                WORKSPACE,
                &principal(),
                RawSourceRetentionPurpose::Replay,
                NOW
            ),
            Err(RawTraceOptInError::ExpiredOptInRow { .. })
        ));
    }

    /// NEGATIVE: a row that never lapses is refused at record time.
    #[test]
    fn opt_in_rows_must_expire() {
        let mut ledger = RawTraceOptInLedger::new();
        let mut row = opt_in_row();
        row.expires_at = row.granted_at;

        assert!(matches!(
            ledger.record_opt_in(row),
            Err(RawTraceOptInError::InvalidRow {
                reason: "expires_at"
            })
        ));
        assert_eq!(ledger.row_count(), 0);
    }

    /// NEGATIVE: an unscoped row is refused, so an opt-in cannot cover the whole disk.
    #[test]
    fn opt_in_rows_must_carry_a_path_scope() {
        let mut ledger = RawTraceOptInLedger::new();
        let mut row = opt_in_row();
        row.path_scope.clear();

        assert!(matches!(
            ledger.record_opt_in(row),
            Err(RawTraceOptInError::InvalidRow {
                reason: "path_scope"
            })
        ));
        assert_eq!(ledger.row_count(), 0);
    }
}
