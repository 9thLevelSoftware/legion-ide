//! Minimal CapabilityBrokerPort fixtures for integration tests.

use legion_protocol::{
    CapabilityDecision, CapabilityDecisionId, CapabilityId, CapabilityRequest, CapabilityResponse,
    CapabilityBrokerPort, ProtocolResult,
};

/// A fixture broker that always grants (returns Decision { granted: true }).
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

/// A fixture broker that always denies (returns Decision { granted: false }).
pub struct DenyAll;

impl CapabilityBrokerPort for DenyAll {
    fn handle(&self, request: CapabilityRequest) -> ProtocolResult<CapabilityResponse> {
        let capability_id = match &request {
            CapabilityRequest::Request { capability_id, .. } => capability_id.clone(),
            _ => CapabilityId("unknown".into()),
        };
        Ok(CapabilityResponse::Decision(CapabilityDecision {
            decision_id: CapabilityDecisionId(2),
            granted: false,
            capability: capability_id,
            reason: Some("deny-all fixture: request denied".into()),
        }))
    }
}
