//! Training-oriented raw-source retention helpers.
//!
//! These helpers keep deletion handles and hosted export linkage metadata-only so higher-level
//! training workflows can request retention actions without ever carrying raw source bytes.

use legion_protocol::{
    CausalityId, CorrelationId, EventSequence, HostedRetentionExportLinkage,
    RawSourceRetentionTombstone, TimestampMillis, validate_hosted_retention_export_linkage,
};
use thiserror::Error;

use crate::RawSourceVaultError;

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
    use uuid::Uuid;

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

    #[test]
    fn hosted_export_linkage_requires_verified_raw_source_consent() {
        let err = build_hosted_raw_source_export_linkage(
            "raw-export:batch-1",
            "bundle:ws-1:42",
            false,
            1,
        )
        .expect_err("export linkage must require verified raw-source consent");

        assert!(matches!(err, RawSourceVaultError::Denied { .. }));
    }
}
