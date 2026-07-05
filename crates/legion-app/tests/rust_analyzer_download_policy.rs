//! Integration tests for the gated rust-analyzer download decision (WS-LANG-01 LANG.01).
//!
//! TDD: These tests were written before the implementation to prove the moat.

use legion_app::language::{
    DownloadDecision, RustAnalyzerDownloadRequest, evaluate_rust_analyzer_download,
    verify_downloaded_artifact,
};
use legion_protocol::{CapabilityDecisionId, CorrelationId, PrincipalId, WorkspaceTrustState};
use legion_security::DenyByDefaultBroker;

mod broker_fixture;

fn req() -> RustAnalyzerDownloadRequest {
    RustAnalyzerDownloadRequest {
        release_host: "releases.example.invalid".into(),
        artifact_uri: "https://releases.example.invalid/rust-analyzer".into(),
        expected_sha256: sha256_hex(b"binary-bytes"),
        expected_version: "rust-analyzer 1.0.0".into(),
    }
}

fn principal() -> PrincipalId {
    PrincipalId("test-principal".into())
}

fn correlation() -> CorrelationId {
    CorrelationId(42)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// Proves the deny-by-default moat using the REAL `DenyByDefaultBroker`.
///
/// Under `SecurityPolicy::default()` (`air_gap: true`, `allow_untrusted: false`),
/// the broker routes the `network.fetch` capability through
/// `network_target_decision`, which denies any non-loopback host. The
/// `release_host` is `releases.example.invalid` (non-loopback), so the air-gap
/// check fails closed and the request is denied. (A trusted workspace is denied
/// by the air-gap branch; an untrusted one would be denied even earlier — either
/// trust value proves the moat.)
#[test]
fn air_gap_default_denies_download() {
    let broker = DenyByDefaultBroker::default();
    let result = evaluate_rust_analyzer_download(
        &req(),
        &broker,
        principal(),
        WorkspaceTrustState::Trusted,
        correlation(),
    );
    match result {
        DownloadDecision::Denied { .. } => {}
        DownloadDecision::Allowed { .. } => {
            panic!("air-gap must deny rust-analyzer download")
        }
    }
}

/// A fixture broker that unconditionally grants verifies the allowed path.
#[test]
fn explicit_grant_allows_download() {
    let broker = broker_fixture::AllowAll;
    let result = evaluate_rust_analyzer_download(
        &req(),
        &broker,
        principal(),
        WorkspaceTrustState::Trusted,
        correlation(),
    );
    assert!(
        matches!(result, DownloadDecision::Allowed { .. }),
        "explicit-grant broker must return Allowed, got {result:?}"
    );
}

/// Hash mismatch must fail closed; correct hash must verify.
#[test]
fn hash_mismatch_fails_closed() {
    let good_hash = sha256_hex(b"binary-bytes");
    assert!(
        verify_downloaded_artifact(b"binary-bytes", &good_hash),
        "correct hash must verify"
    );
    assert!(
        !verify_downloaded_artifact(b"tampered", &good_hash),
        "mismatched hash must fail closed"
    );
}

/// `DownloadDecision` must carry the decision_id from the broker for audit.
#[test]
fn allowed_decision_carries_decision_id() {
    let broker = broker_fixture::AllowAll;
    let result = evaluate_rust_analyzer_download(
        &req(),
        &broker,
        principal(),
        WorkspaceTrustState::Trusted,
        correlation(),
    );
    match result {
        DownloadDecision::Allowed { decision_id } => {
            // AllowAll fixture returns CapabilityDecisionId(1)
            assert_eq!(decision_id, CapabilityDecisionId(1));
        }
        other => panic!("expected Allowed, got {other:?}"),
    }
}
