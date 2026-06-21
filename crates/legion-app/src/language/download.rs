//! Capability-gated rust-analyzer download decision and artifact verification (design §5).
//!
//! This module implements the **decision + verification** layer, not a live HTTP
//! fetch. The live fetch is exercised only in the `--ignored` smoke test (Task 12).
//! Default `NetworkPolicy` has `air_gap: true`, so the broker denies unless the
//! operator has explicitly permitted the release host.

use legion_protocol::{
    CapabilityBrokerPort, CapabilityCommandClass, CapabilityDecisionId, CapabilityId,
    CapabilityRequest, CapabilityRequestContext, CapabilityResponse, CorrelationId, NetworkTarget,
    PrincipalId, WorkspaceTrustState,
};

/// A request to fetch a rust-analyzer artifact (design §5).
#[derive(Debug, Clone)]
pub struct RustAnalyzerDownloadRequest {
    /// Host the artifact is fetched from (e.g. `"static.rust-lang.org"`).
    pub release_host: String,
    /// Full artifact URI.
    pub artifact_uri: String,
    /// Pinned SHA-256 of the expected artifact (lowercase hex).
    pub expected_sha256: String,
    /// Expected `--version` string after install (e.g. `"rust-analyzer 2024-11-18"`).
    pub expected_version: String,
}

/// Outcome of a gated download decision.
#[derive(Debug, Clone)]
pub enum DownloadDecision {
    /// The capability broker denied the network capability.
    Denied {
        /// Human-readable refusal reason for health-record projection.
        reason: String,
    },
    /// The capability broker granted the network capability.
    Allowed {
        /// Decision id recorded into the health record for audit.
        decision_id: CapabilityDecisionId,
    },
}

/// Asks the capability broker whether a rust-analyzer download may proceed.
///
/// Builds the `CapabilityRequestContext` from `req` (network class, LSP binary,
/// network target) and calls `broker.handle(...)`. Fails closed on any error.
///
/// # Security
/// Requests the `network.fetch` capability — a binary download is network
/// egress, and `network.fetch` / `network.egress` are the capabilities the
/// broker routes through its air-gap (`network_target_decision`) check. Default
/// `NetworkPolicy` is air-gapped (`air_gap: true`), so this returns `Denied`
/// for any non-loopback release host unless the operator has explicitly
/// allowlisted it in their security policy.
pub fn evaluate_rust_analyzer_download(
    req: &RustAnalyzerDownloadRequest,
    broker: &dyn CapabilityBrokerPort,
    principal_id: PrincipalId,
    workspace_trust_state: WorkspaceTrustState,
    correlation_id: CorrelationId,
) -> DownloadDecision {
    let context = CapabilityRequestContext {
        command_class: Some(CapabilityCommandClass::Network),
        command_binary: Some("rust-analyzer".into()),
        lsp_server_binary: Some("rust-analyzer".into()),
        network_target: Some(NetworkTarget {
            scheme: "https".into(),
            host: req.release_host.clone(),
            port: Some(443),
        }),
        ..CapabilityRequestContext::default()
    };

    match broker.handle(CapabilityRequest::Request {
        principal_id,
        capability_id: CapabilityId("network.fetch".into()),
        workspace_trust_state,
        target_path: None,
        decision_id: None,
        context,
        correlation_id,
    }) {
        Ok(CapabilityResponse::Decision(d)) => {
            if d.granted {
                DownloadDecision::Allowed { decision_id: d.decision_id }
            } else {
                DownloadDecision::Denied { reason: d.reason.unwrap_or_default() }
            }
        }
        Ok(CapabilityResponse::Granted(g)) => DownloadDecision::Allowed { decision_id: g.decision_id },
        Ok(CapabilityResponse::Denied(den)) => DownloadDecision::Denied { reason: den.reason },
        Err(e) => DownloadDecision::Denied {
            reason: format!("capability broker error (fail closed): {e:?}"),
        },
    }
}

/// Verifies downloaded bytes against the pinned SHA-256. Fails closed: returns
/// `false` on any mismatch, including case differences (case-insensitive compare).
///
/// # Example
/// ```
/// use legion_app::language::verify_downloaded_artifact;
/// assert!(verify_downloaded_artifact(b"hello", "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"));
/// ```
pub fn verify_downloaded_artifact(bytes: &[u8], expected_sha256: &str) -> bool {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize()).eq_ignore_ascii_case(expected_sha256)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn verify_correct_hash_passes() {
        // sha256("hello") = 2cf24dba...
        let expected = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(b"hello");
            hex::encode(h.finalize())
        };
        assert!(verify_downloaded_artifact(b"hello", &expected));
    }

    #[test]
    fn verify_wrong_hash_fails() {
        let expected = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(b"hello");
            hex::encode(h.finalize())
        };
        assert!(!verify_downloaded_artifact(b"world", &expected));
    }

    #[test]
    fn verify_uppercase_hex_passes() {
        let expected = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(b"hello");
            hex::encode(h.finalize()).to_uppercase()
        };
        assert!(verify_downloaded_artifact(b"hello", &expected));
    }
}
