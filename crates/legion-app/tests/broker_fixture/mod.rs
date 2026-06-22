//! Minimal CapabilityBrokerPort fixtures for integration tests.

use legion_protocol::{
    CapabilityBrokerPort, CapabilityDecision, CapabilityDecisionId, CapabilityId,
    CapabilityRequest, CapabilityResponse, ProtocolResult,
};

/// A fixture broker that always grants (returns Decision { granted: true }).
///
/// Used to prove the adapter correctly maps a grant to `DownloadDecision::Allowed`.
/// The deny path is exercised against the REAL `DenyByDefaultBroker`, not a fixture.
pub struct AllowAll;

impl CapabilityBrokerPort for AllowAll {
    fn handle(&self, request: CapabilityRequest) -> ProtocolResult<CapabilityResponse> {
        let capability_id = match &request {
            CapabilityRequest::Request { capability_id, .. } => capability_id.clone(),
            _ => CapabilityId("unknown".into()),
        };
        Ok(CapabilityResponse::Decision(CapabilityDecision {
            decision_id: CapabilityDecisionId(1),
            granted: true,
            capability: capability_id,
            reason: None,
        }))
    }
}
