//! Privacy inspector deletion helpers.
//!
//! These functions wire privacy-inspector deletion handles to any
//! [`RawSourceVault`] implementation — fixture or production — without
//! carrying raw source bytes at any point in the call chain.

use legion_protocol::{
    CausalityId, CorrelationId, EventSequence, RawSourceRetentionBundleDescriptor,
    RawSourceRetentionTombstone, TimestampMillis,
};

use crate::{RawSourceVault, RawSourceVaultError};
use crate::training::build_raw_source_deletion_tombstone;

/// Look up a bundle descriptor by id via the vault trait.
///
/// Returns the descriptor or `RawSourceVaultError::BundleMissing` if the
/// bundle does not exist.
pub fn lookup_retention_bundle<V: RawSourceVault>(
    vault: &V,
    bundle_id: &str,
) -> Result<RawSourceRetentionBundleDescriptor, RawSourceVaultError> {
    vault.vault_read_bundle_descriptor(bundle_id)
}

/// Delete a retention record via the vault trait.
///
/// Callers must supply a fully validated tombstone (use
/// [`build_raw_source_deletion_tombstone`] from the `training` module).
pub fn delete_retention_record<V: RawSourceVault>(
    vault: &mut V,
    tombstone: RawSourceRetentionTombstone,
) -> Result<RawSourceRetentionTombstone, RawSourceVaultError> {
    vault.vault_delete_bundle(tombstone)
}

/// Format a metadata-only deletion handle from a confirmed tombstone.
///
/// The returned string is suitable for audit logging. It never contains raw
/// source bytes, encryption keys, or other sensitive material.
pub fn format_deletion_handle(tombstone: &RawSourceRetentionTombstone) -> String {
    format!(
        "deletion_handle bundle_id={} reason={} deleted_at={} event_sequence={} schema_version={}",
        tombstone.bundle_id,
        tombstone.reason,
        tombstone.deleted_at.0,
        tombstone.event_sequence.0,
        tombstone.schema_version,
    )
}

/// Execute a full privacy deletion: verify the bundle exists, build a
/// validated tombstone, delete the bundle, and return a metadata-only handle.
///
/// The handle is suitable for privacy-audit logging and never contains raw
/// source bytes, paths, or cryptographic key material.
pub fn execute_privacy_deletion<V: RawSourceVault>(
    vault: &mut V,
    bundle_id: &str,
    reason: &str,
    deleted_at: TimestampMillis,
    event_sequence: EventSequence,
    correlation_id: CorrelationId,
    causality_id: CausalityId,
) -> Result<String, RawSourceVaultError> {
    // Verify the bundle exists before building the tombstone so callers get
    // a deterministic BundleMissing error before any tombstone is constructed.
    vault.vault_read_bundle_descriptor(bundle_id)?;

    let tombstone = build_raw_source_deletion_tombstone(
        bundle_id,
        reason,
        deleted_at,
        event_sequence,
        correlation_id,
        causality_id,
        1,
    )?;

    let confirmed = vault.vault_delete_bundle(tombstone)?;
    Ok(format_deletion_handle(&confirmed))
}
