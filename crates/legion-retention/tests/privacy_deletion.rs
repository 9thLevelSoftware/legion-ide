// P4.F2.T4 — privacy inspector deletion handles wired to retention vault tests.

use legion_protocol::{
    CanonicalPath, CausalityId, CorrelationId, EventSequence, PrincipalId,
    RawSourceCaptureRequest, RawSourceRetentionConsentGrant, RawSourceRetentionPolicy,
    RawSourceRetentionPurpose, TimestampMillis, WorkspaceId,
};
use legion_retention::{
    RetentionFixtureVault, RawSourceVaultError,
    privacy::{execute_privacy_deletion, format_deletion_handle, lookup_retention_bundle},
    training::build_raw_source_deletion_tombstone,
};

// ---------------------------------------------------------------------------
// Shared test fixtures
// ---------------------------------------------------------------------------

fn fixture_policy() -> RawSourceRetentionPolicy {
    RawSourceRetentionPolicy {
        capture_enabled: true,
        allowed_purposes: vec![RawSourceRetentionPurpose::SupportBundle],
        max_bundle_bytes: 4096,
        ttl_ms: 60_000,
        schema_version: 1,
    }
}

fn fixture_grant() -> RawSourceRetentionConsentGrant {
    RawSourceRetentionConsentGrant {
        principal_id: PrincipalId("test-user".to_string()),
        workspace_id: WorkspaceId(1),
        purpose: RawSourceRetentionPurpose::SupportBundle,
        path_scope: vec![CanonicalPath("C:/repo/src/main.rs".to_string())],
        expires_at: TimestampMillis(999_999_999),
        correlation_id: CorrelationId(1),
        schema_version: 1,
    }
}

fn fixture_request() -> RawSourceCaptureRequest {
    RawSourceCaptureRequest {
        workspace_id: WorkspaceId(1),
        principal_id: PrincipalId("test-user".to_string()),
        purpose: RawSourceRetentionPurpose::SupportBundle,
        paths: vec![CanonicalPath("C:/repo/src/main.rs".to_string())],
        max_bytes: 1024,
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(uuid::Uuid::from_u128(
            0x018f_0000_0000_7000_8000_0000_0000_0001,
        )),
        schema_version: 1,
    }
}

/// Create a vault with one captured bundle descriptor; returns (vault, bundle_id).
fn vault_with_bundle() -> (RetentionFixtureVault, String) {
    let mut vault = RetentionFixtureVault::new(fixture_policy());
    let (_lease, descriptor) = vault
        .capture_descriptor(fixture_grant(), fixture_request())
        .expect("capture descriptor");
    (vault, descriptor.bundle_id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// `execute_privacy_deletion` must remove the bundle and return a metadata-only handle.
#[test]
fn privacy_deletion_removes_bundle_from_fixture_vault() {
    let (mut vault, bundle_id) = vault_with_bundle();
    assert_eq!(vault.bundle_count(), 1);

    let handle = execute_privacy_deletion(
        &mut vault,
        &bundle_id,
        "privacy_inspector_user_request",
        TimestampMillis(5_000),
        EventSequence(10),
        CorrelationId(10),
        CausalityId(uuid::Uuid::from_u128(0x018f_0000_0000_7000_8000_0000_0000_0010)),
    )
    .expect("execute privacy deletion");

    assert_eq!(
        vault.bundle_count(),
        0,
        "bundle must be removed from the vault after deletion"
    );
    assert!(
        handle.contains(&bundle_id),
        "deletion handle must include bundle_id; got: {handle}"
    );
    assert!(
        handle.contains("privacy_inspector_user_request"),
        "deletion handle must include reason; got: {handle}"
    );
    // Handle must never contain raw source bytes.
    assert!(
        !handle.contains("fn main"),
        "deletion handle must not contain raw source content"
    );
}

/// `execute_privacy_deletion` on a non-existent bundle must return `BundleMissing`.
#[test]
fn privacy_deletion_on_missing_bundle_returns_error() {
    let mut vault = RetentionFixtureVault::new(fixture_policy());

    let result = execute_privacy_deletion(
        &mut vault,
        "bundle:never:existed",
        "privacy_inspector_user_request",
        TimestampMillis(5_000),
        EventSequence(1),
        CorrelationId(1),
        CausalityId(uuid::Uuid::from_u128(0x018f_0000_0000_7000_8000_0000_0000_0002)),
    );

    assert!(
        matches!(result, Err(RawSourceVaultError::BundleMissing { .. })),
        "missing bundle must return BundleMissing error; got: {result:?}"
    );
}

/// `lookup_retention_bundle` must return the descriptor that was captured.
#[test]
fn lookup_retention_bundle_returns_descriptor() {
    let (vault, bundle_id) = vault_with_bundle();

    let descriptor = lookup_retention_bundle(&vault, &bundle_id)
        .expect("lookup must succeed for known bundle");

    assert_eq!(
        descriptor.bundle_id, bundle_id,
        "descriptor must match the requested bundle_id"
    );
    assert_eq!(descriptor.workspace_id, WorkspaceId(1));

    // Lookup on a missing bundle must return BundleMissing.
    let missing = lookup_retention_bundle(&vault, "bundle:not:found");
    assert!(
        matches!(missing, Err(RawSourceVaultError::BundleMissing { .. })),
        "missing bundle must return BundleMissing; got: {missing:?}"
    );
}

/// `format_deletion_handle` must produce a stable, metadata-only string.
#[test]
fn format_deletion_handle_is_metadata_only() {
    let tombstone = build_raw_source_deletion_tombstone(
        "bundle:test:42",
        "privacy_inspector_test",
        TimestampMillis(7_000),
        EventSequence(7),
        CorrelationId(7),
        CausalityId(uuid::Uuid::from_u128(0x018f_0000_0000_7000_8000_0000_0000_0007)),
        1,
    )
    .expect("build tombstone");

    let handle = format_deletion_handle(&tombstone);

    assert!(
        handle.contains("bundle:test:42"),
        "handle must include bundle_id; got: {handle}"
    );
    assert!(
        handle.contains("privacy_inspector_test"),
        "handle must include reason; got: {handle}"
    );
    assert!(
        handle.contains("7000"),
        "handle must include deleted_at timestamp; got: {handle}"
    );
    // Handle must never contain raw source bytes or key material.
    assert!(
        !handle.contains("0123456789abcdef"),
        "handle must not contain key material"
    );
}
