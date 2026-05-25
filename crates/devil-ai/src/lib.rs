//! AI Orchestrator: prompt assembly, context selection, model request abstraction.

#![warn(missing_docs)]

use std::collections::HashMap;

use devil_protocol::{
    AssistedAiProviderClass, AssistedAiProviderInvocationState, AssistedAiProviderRouteRequest,
    AssistedAiProviderRouteResponse, AssistedAiRefusalMetadata, AssistedAiRequestDisposition,
    AssistedAiRouteDecision, CapabilityBrokerPort, CapabilityRequest, CapabilityResponse,
    ProposalRiskLabel, RedactionHint, validate_assisted_ai_provider_route_request,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Canonical provider identifier used by crate boundaries.
pub type ProviderId = String;

/// Human readable role used in chat-like prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatRole {
    /// Instructions and policy-like context.
    System,
    /// User message content.
    User,
    /// Assistant response text.
    Assistant,
}

/// A single message exchanged with a completion provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Message role.
    pub role: ChatRole,
    /// Message text payload.
    pub content: String,
}

/// Request payload for text generation providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    /// Provider selected for the request.
    pub provider: ProviderId,
    /// Model name or alias expected by the provider.
    pub model: String,
    /// Ordered conversation prompt messages.
    pub messages: Vec<ChatMessage>,
    /// Optional maximum output length constraint.
    pub max_tokens: Option<u32>,
    /// Optional sampling temperature.
    pub temperature: Option<f32>,
    /// Optional provider-specific request metadata.
    pub metadata: HashMap<String, String>,
}

impl ChatCompletionRequest {
    /// Creates a minimal request for a provider and model.
    pub fn new(
        provider: impl Into<ProviderId>,
        model: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: message.into(),
            }],
            max_tokens: None,
            temperature: None,
            metadata: HashMap::new(),
        }
    }

    /// Replaces or inserts an arbitrary key/value metadata pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Sets maximum token output for the request.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Sets sampling temperature for the request.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

/// Provider response for text completion requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    /// Provider identifier that produced the response.
    pub provider: ProviderId,
    /// Model identifier used by the provider.
    pub model: String,
    /// Plain text completion result.
    pub text: String,
    /// Raw fields that providers may attach for observability.
    pub metadata: HashMap<String, String>,
}

/// Request payload for vector embedding providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Provider selected for embedding generation.
    pub provider: ProviderId,
    /// Model name or alias expected by the provider.
    pub model: String,
    /// Input text snippets to embed.
    pub inputs: Vec<String>,
}

impl EmbeddingRequest {
    /// Creates a minimal embedding request.
    pub fn new(
        provider: impl Into<ProviderId>,
        model: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            inputs: vec![input.into()],
        }
    }
}

/// Response payload for embedding providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// Provider identifier that produced the embeddings.
    pub provider: ProviderId,
    /// Model identifier used by the provider.
    pub model: String,
    /// One embedding vector per input entry.
    pub vectors: Vec<Vec<f32>>,
    /// Raw fields that providers may attach for observability.
    pub metadata: HashMap<String, String>,
}

/// Capabilities exposed by a provider implementation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    /// Supports chat/completion style generation.
    pub completion: bool,
    /// Supports vector embedding generation.
    pub embedding: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            completion: true,
            embedding: false,
        }
    }
}

/// Stable provider-side error type used by orchestration code.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// Provider is temporarily unavailable.
    #[error("provider `{provider}` is unavailable: {reason}")]
    ProviderUnavailable {
        /// Provider id.
        provider: ProviderId,
        /// Human-readable reason.
        reason: String,
    },

    /// Provider does not support the requested operation.
    #[error("provider `{provider}` does not support `{operation}`")]
    OperationUnavailable {
        /// Provider id.
        provider: ProviderId,
        /// Operation name.
        operation: String,
    },

    /// Provider returned malformed payload or protocol error.
    #[error("provider `{provider}` request failed: {message}")]
    RequestFailed {
        /// Provider id.
        provider: ProviderId,
        /// Provider error details.
        message: String,
    },

    /// Provider-specific validation or policy rejected the request.
    #[error("request rejected: {message}")]
    RequestRejected {
        /// Human-readable reason.
        message: String,
    },
}

impl ProviderError {
    /// Helper for constructing unavailable errors.
    pub fn unavailable(provider: impl Into<ProviderId>, reason: impl Into<String>) -> Self {
        Self::ProviderUnavailable {
            provider: provider.into(),
            reason: reason.into(),
        }
    }

    /// Helper for constructing unsupported operation errors.
    pub fn unsupported(provider: impl Into<ProviderId>, operation: impl Into<String>) -> Self {
        Self::OperationUnavailable {
            provider: provider.into(),
            operation: operation.into(),
        }
    }
}

/// Minimal provider-agnostic abstraction for model adapters.
pub trait ModelProvider {
    /// Stable identifier reported by each provider implementation.
    fn provider_id(&self) -> ProviderId;

    /// Returns provider feature capabilities.
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    /// Sends a completion request to the provider implementation.
    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError>;

    /// Sends an embedding request to the provider implementation.
    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError>;
}

/// A provider registry resolves provider implementations by identifier.
#[derive(Default)]
pub struct ProviderRegistry {
    providers: HashMap<ProviderId, Box<dyn ModelProvider>>,
}

impl ProviderRegistry {
    /// Creates an empty provider registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Registers a provider implementation.
    pub fn register(&mut self, provider: Box<dyn ModelProvider>) {
        let id = provider.provider_id();
        self.providers.insert(id, provider);
    }

    /// Resolves a provider by identifier.
    pub fn get(&self, provider_id: &str) -> Option<&dyn ModelProvider> {
        self.providers
            .get(provider_id)
            .map(|provider| provider.as_ref())
    }

    /// Returns all registered provider identifiers.
    pub fn provider_ids(&self) -> Vec<ProviderId> {
        self.providers.keys().cloned().collect()
    }
}

/// Policy-bound router for Phase 4 provider invocation.
pub struct ProviderRouter<'a> {
    registry: &'a ProviderRegistry,
    capability_broker: &'a dyn CapabilityBrokerPort,
}

impl<'a> ProviderRouter<'a> {
    /// Creates a provider router over a registry and capability broker.
    pub fn new(
        registry: &'a ProviderRegistry,
        capability_broker: &'a dyn CapabilityBrokerPort,
    ) -> Self {
        Self {
            registry,
            capability_broker,
        }
    }

    /// Routes a metadata-only provider request through policy before invoking a provider.
    pub fn route_completion(
        &self,
        request: AssistedAiProviderRouteRequest,
    ) -> Result<AssistedAiProviderRouteResponse, ProviderError> {
        validate_assisted_ai_provider_route_request(&request).map_err(|error| {
            ProviderError::RequestRejected {
                message: format!("invalid route metadata: {error}"),
            }
        })?;

        if !matches!(
            request.provider_class,
            AssistedAiProviderClass::Local | AssistedAiProviderClass::LocalLoopback
        ) {
            return Ok(self.refused_response(
                &request,
                "provider.remote_deferred",
                "remote provider activation is deferred",
            ));
        }

        let capability_response = self
            .capability_broker
            .handle(CapabilityRequest::Request {
                principal_id: request.principal_id.clone(),
                capability_id: request.required_capability.clone(),
                workspace_trust_state: request.workspace_trust_state.clone(),
                target_path: None,
                decision_id: request.policy_decision_id,
                context: devil_protocol::CapabilityRequestContext {
                    network_target: request.network_target.clone(),
                    ..Default::default()
                },
                correlation_id: request.correlation_id,
            })
            .map_err(|error| ProviderError::RequestRejected {
                message: format!("capability broker failed: {}", error.message),
            })?;

        if !capability_granted(&capability_response) {
            return Ok(self.refused_response(
                &request,
                "capability.denied",
                "provider capability denied by policy",
            ));
        }

        let Some(provider) = self.registry.get(&request.provider_id) else {
            return Ok(self.refused_response(
                &request,
                "provider.missing",
                "provider not registered",
            ));
        };

        if !provider.capabilities().completion {
            return Ok(self.refused_response(
                &request,
                "provider.completion_unavailable",
                "provider does not support completion",
            ));
        }

        let completion = provider.complete(
            ChatCompletionRequest::new(&request.provider_id, &request.model_label, "metadata-only")
                .with_metadata(
                    "context_manifest",
                    request.context_manifest.reference_id.clone(),
                )
                .with_metadata("route_id", request.route_id.clone()),
        )?;

        Ok(AssistedAiProviderRouteResponse {
            route_id: request.route_id.clone(),
            invocation_state: AssistedAiProviderInvocationState::Completed,
            route_decision: allowed_route_decision(),
            provider_id: request.provider_id.clone(),
            model_label: request.model_label.clone(),
            output_labels: vec![format!("response.bytes:{}", completion.text.len())],
            refusal: None,
            correlation_id: request.correlation_id,
            causality_id: request.causality_id,
            event_sequence: request.event_sequence,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: request.schema_version,
        })
    }

    fn refused_response(
        &self,
        request: &AssistedAiProviderRouteRequest,
        reason_code: &str,
        label: &str,
    ) -> AssistedAiProviderRouteResponse {
        let refusal = AssistedAiRefusalMetadata {
            reason_code: reason_code.to_string(),
            label: label.to_string(),
            provider_id: Some(request.provider_id.clone()),
            operation_class: Some(request.operation_class),
            privacy_scope: None,
            capability: Some(request.required_capability.clone()),
            budget_id: None,
            risk_label: ProposalRiskLabel::High,
            reasons: vec![reason_code.to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: request.schema_version,
        };
        AssistedAiProviderRouteResponse {
            route_id: request.route_id.clone(),
            invocation_state: AssistedAiProviderInvocationState::Refused,
            route_decision: AssistedAiRouteDecision {
                disposition: AssistedAiRequestDisposition::Refused,
                provider_invocation: AssistedAiProviderInvocationState::Refused,
                refusal: Some(refusal.clone()),
                reasons: vec![reason_code.to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: request.schema_version,
            },
            provider_id: request.provider_id.clone(),
            model_label: request.model_label.clone(),
            output_labels: vec!["output.not_encoded".to_string()],
            refusal: Some(refusal),
            correlation_id: request.correlation_id,
            causality_id: request.causality_id,
            event_sequence: request.event_sequence,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: request.schema_version,
        }
    }
}

fn capability_granted(response: &CapabilityResponse) -> bool {
    matches!(response, CapabilityResponse::Decision(decision) if decision.granted)
        || matches!(response, CapabilityResponse::Granted(_))
}

fn allowed_route_decision() -> AssistedAiRouteDecision {
    AssistedAiRouteDecision {
        disposition: AssistedAiRequestDisposition::MetadataOnlyReady,
        provider_invocation: AssistedAiProviderInvocationState::Completed,
        refusal: None,
        reasons: vec!["provider.completed.metadata_only".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::{
        AssistedAiOperationClass, AssistedAiProposalTargetIntent, AssistedAiProviderClass,
        AssistedAiTrustProjectionKind, AssistedAiTrustProjectionReference, CancellationTokenId,
        CapabilityId, CausalityId, CorrelationId, EventSequence, FileFingerprint, NetworkTarget,
        PrincipalId, ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel,
        ProposalTargetCoverage, ProposalTargetCoverageKind, WorkspaceTrustState,
    };
    use devil_security::DenyByDefaultBroker;
    use uuid::Uuid;

    struct LocalProvider;

    struct CompletionUnavailableProvider;

    impl ModelProvider for LocalProvider {
        fn provider_id(&self) -> ProviderId {
            "local".to_string()
        }

        fn complete(
            &self,
            request: ChatCompletionRequest,
        ) -> Result<ChatCompletionResponse, ProviderError> {
            Ok(ChatCompletionResponse {
                provider: request.provider,
                model: request.model,
                text: "metadata-only".to_string(),
                metadata: HashMap::new(),
            })
        }

        fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
            Err(ProviderError::unsupported(request.provider, "embed"))
        }
    }

    impl ModelProvider for CompletionUnavailableProvider {
        fn provider_id(&self) -> ProviderId {
            "local".to_string()
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                completion: false,
                embedding: false,
            }
        }

        fn complete(
            &self,
            request: ChatCompletionRequest,
        ) -> Result<ChatCompletionResponse, ProviderError> {
            Err(ProviderError::unsupported(request.provider, "complete"))
        }

        fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
            Err(ProviderError::unsupported(request.provider, "embed"))
        }
    }

    fn reference(
        reference_id: &str,
        kind: AssistedAiTrustProjectionKind,
    ) -> AssistedAiTrustProjectionReference {
        AssistedAiTrustProjectionReference {
            reference_id: reference_id.to_string(),
            kind,
            projection_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: reference_id.to_string(),
            },
            schema_version: 1,
        }
    }

    fn route_request(provider_class: AssistedAiProviderClass) -> AssistedAiProviderRouteRequest {
        AssistedAiProviderRouteRequest {
            route_id: "route-1".to_string(),
            provider_id: "local".to_string(),
            model_label: "test-model".to_string(),
            provider_class,
            operation_class: AssistedAiOperationClass::ProposeEdit,
            context_manifest: reference("ctx", AssistedAiTrustProjectionKind::ContextManifest),
            privacy_inspector: reference(
                "privacy",
                AssistedAiTrustProjectionKind::PrivacyInspector,
            ),
            permission_budget: reference("budget", AssistedAiTrustProjectionKind::PermissionBudget),
            proposal_intent: AssistedAiProposalTargetIntent {
                payload_kind: ProposalPayloadKind::TextEdit,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: Vec::new(),
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                required_capability: CapabilityId("ai.proposal.create".to_string()),
                risk_label: ProposalRiskLabel::Low,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                labels: vec!["proposal.intent".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            policy_decision_id: None,
            required_capability: CapabilityId("ai.provider.invoke".to_string()),
            network_target: Some(NetworkTarget {
                scheme: "http".to_string(),
                host: "localhost".to_string(),
                port: Some(11434),
            }),
            cancellation_token: CancellationTokenId(
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            ),
            health_labels: vec!["healthy".to_string()],
            cost_labels: vec!["local".to_string()],
            principal_id: PrincipalId("principal".to_string()),
            workspace_trust_state: WorkspaceTrustState::Trusted,
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(
                Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            ),
            event_sequence: EventSequence(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn router_invokes_local_provider_only_after_policy_approval() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(LocalProvider));
        let broker = DenyByDefaultBroker::default();
        let router = ProviderRouter::new(&registry, &broker);

        let response = router
            .route_completion(route_request(AssistedAiProviderClass::LocalLoopback))
            .expect("route completes");

        assert_eq!(
            response.invocation_state,
            AssistedAiProviderInvocationState::Completed
        );
        assert_eq!(
            response.output_labels,
            vec!["response.bytes:13".to_string()]
        );
    }

    #[test]
    fn router_refuses_remote_provider_without_invocation() {
        let registry = ProviderRegistry::new();
        let broker = DenyByDefaultBroker::default();
        let router = ProviderRouter::new(&registry, &broker);

        let response = router
            .route_completion(route_request(AssistedAiProviderClass::HostedRemote))
            .expect("remote route refusal is represented as metadata");

        assert_eq!(
            response.invocation_state,
            AssistedAiProviderInvocationState::Refused
        );
        assert_eq!(
            response.output_labels,
            vec!["output.not_encoded".to_string()]
        );
    }

    #[test]
    fn router_refuses_missing_local_provider_as_metadata() {
        let registry = ProviderRegistry::new();
        let broker = DenyByDefaultBroker::default();
        let router = ProviderRouter::new(&registry, &broker);

        let response = router
            .route_completion(route_request(AssistedAiProviderClass::LocalLoopback))
            .expect("missing provider refusal is metadata");

        assert_eq!(
            response.invocation_state,
            AssistedAiProviderInvocationState::Refused
        );
        assert_eq!(
            response.refusal.as_ref().unwrap().reason_code,
            "provider.missing"
        );
    }

    #[test]
    fn router_refuses_provider_without_completion_capability() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(CompletionUnavailableProvider));
        let broker = DenyByDefaultBroker::default();
        let router = ProviderRouter::new(&registry, &broker);

        let response = router
            .route_completion(route_request(AssistedAiProviderClass::LocalLoopback))
            .expect("completion refusal is metadata");

        assert_eq!(
            response.invocation_state,
            AssistedAiProviderInvocationState::Refused
        );
        assert_eq!(
            response.refusal.as_ref().unwrap().reason_code,
            "provider.completion_unavailable"
        );
    }

    #[test]
    fn router_rejects_nil_cancellation_token_before_provider_invocation() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(LocalProvider));
        let broker = DenyByDefaultBroker::default();
        let router = ProviderRouter::new(&registry, &broker);
        let mut request = route_request(AssistedAiProviderClass::LocalLoopback);
        request.cancellation_token = CancellationTokenId(Uuid::nil());

        let error = router
            .route_completion(request)
            .expect_err("nil cancellation token is invalid metadata");

        assert!(matches!(
            error,
            ProviderError::RequestRejected { message } if message.contains("cancellation_token")
        ));
    }

    #[test]
    fn router_rejects_raw_health_metadata_before_provider_invocation() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(LocalProvider));
        let broker = DenyByDefaultBroker::default();
        let router = ProviderRouter::new(&registry, &broker);
        let mut request = route_request(AssistedAiProviderClass::LocalLoopback);
        request
            .health_labels
            .push("provider_payload leaked".to_string());

        let error = router
            .route_completion(request)
            .expect_err("raw provider metadata is invalid");

        assert!(matches!(
            error,
            ProviderError::RequestRejected { message } if message.contains("health_labels")
        ));
    }
}
