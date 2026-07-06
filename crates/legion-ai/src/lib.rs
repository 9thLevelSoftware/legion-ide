//! AI Orchestrator: prompt assembly, context selection, model request abstraction.

#![warn(missing_docs)]

/// Advisory model-assisted risk classifier outputs.
pub mod classifier;
/// Context manifest assembly helpers.
pub mod manifest;
pub mod redaction;
pub mod streaming;

pub use manifest::{
    ManifestFileSource, ManifestMemoryRecordSource, ManifestMetadata, ManifestRuleRecordSource,
    ManifestSelectionSource, ManifestSymbolSource, ManifestTerminalExcerpt,
    assemble_context_manifest, assemble_context_manifest_from_sources, collect_diagnostic_context,
    collect_file_context, collect_memory_context, collect_rules_context, collect_selection_context,
    collect_symbol_context, collect_terminal_context,
};

use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

use legion_protocol::{
    AssistedAiOperationClass, AssistedAiProviderClass, AssistedAiProviderInvocationState,
    AssistedAiProviderRouteRequest, AssistedAiProviderRouteResponse, AssistedAiRefusalMetadata,
    AssistedAiRequestDisposition, AssistedAiRouteDecision, CapabilityBrokerPort, CapabilityRequest,
    CapabilityResponse, InlinePredictionFreshness, InlinePredictionGhostText,
    InlinePredictionProviderMetadata, InlinePredictionResult, InlinePredictionResultId,
    InlinePredictionResultState, InlinePredictionRetention, ProposalRiskLabel, ProtocolTextRange,
    RedactionHint, validate_assisted_ai_provider_route_request,
    validate_inline_prediction_request_metadata, validate_inline_prediction_result,
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

/// Request payload for offline batch jobs that bundle multiple completion requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJobRequest {
    /// Provider selected for the batch job.
    pub provider: ProviderId,
    /// Model name or alias expected by the provider.
    pub model: String,
    /// Stable batch identifier used to round-trip offline work.
    pub batch_id: String,
    /// Logical batch job type, such as `repo-summary`.
    pub job_type: String,
    /// Ordered completion requests to execute as part of the batch.
    pub requests: Vec<ChatCompletionRequest>,
    /// Optional provider-specific request metadata.
    pub metadata: HashMap<String, String>,
}

impl BatchJobRequest {
    /// Creates a minimal batch request for a provider, model, batch id, and job type.
    pub fn new(
        provider: impl Into<ProviderId>,
        model: impl Into<String>,
        batch_id: impl Into<String>,
        job_type: impl Into<String>,
        requests: Vec<ChatCompletionRequest>,
    ) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            batch_id: batch_id.into(),
            job_type: job_type.into(),
            requests,
            metadata: HashMap::new(),
        }
    }

    /// Replaces or inserts an arbitrary key/value metadata pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Response payload for offline batch jobs that bundle multiple completion responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJobResponse {
    /// Provider identifier that produced the batch response.
    pub provider: ProviderId,
    /// Model identifier used by the provider.
    pub model: String,
    /// Stable batch identifier used to round-trip offline work.
    pub batch_id: String,
    /// Logical batch job type, such as `repo-summary`.
    pub job_type: String,
    /// Ordered completion responses produced for the batch.
    pub responses: Vec<ChatCompletionResponse>,
    /// Provider-side metadata without raw prompt or source bodies.
    pub metadata: HashMap<String, String>,
}

/// Request payload for inline next-edit prediction providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlinePredictionRequest {
    /// Provider selected for the request.
    pub provider: ProviderId,
    /// Model name or display-safe alias expected by the provider.
    pub model: String,
    /// Protocol request metadata without raw source bodies.
    pub metadata: legion_protocol::InlinePredictionRequestMetadata,
}

/// Response payload for inline next-edit prediction providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlinePredictionResponse {
    /// Provider identifier that produced the response.
    pub provider: ProviderId,
    /// Model identifier used by the provider.
    pub model: String,
    /// Protocol inline prediction result.
    pub result: InlinePredictionResult,
    /// Provider-side metadata without raw prompt or source bodies.
    pub metadata: HashMap<String, String>,
}

/// Capabilities exposed by a provider implementation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    /// Supports chat/completion style generation.
    pub completion: bool,
    /// Supports vector embedding generation.
    pub embedding: bool,
    /// Supports offline batch jobs that round-trip multiple completions.
    pub batch: bool,
    /// Supports inline next-edit prediction distinct from chat/proposal output.
    pub inline_prediction: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            completion: true,
            embedding: false,
            batch: false,
            inline_prediction: false,
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

    /// Sends an offline batch job to the provider implementation.
    fn batch_complete(&self, request: BatchJobRequest) -> Result<BatchJobResponse, ProviderError> {
        Err(ProviderError::unsupported(
            request.provider,
            "batch_complete",
        ))
    }

    /// Sends an inline prediction request to the provider implementation.
    fn predict_inline(
        &self,
        request: InlinePredictionRequest,
    ) -> Result<InlinePredictionResponse, ProviderError> {
        Err(ProviderError::unsupported(
            request.provider,
            "predict_inline",
        ))
    }
}

/// Deterministic local inline predictor for tests and offline metadata-only flows.
pub struct DeterministicInlinePredictionProvider {
    id: ProviderId,
}

impl DeterministicInlinePredictionProvider {
    /// Creates a deterministic inline prediction provider.
    pub fn new(id: impl Into<ProviderId>) -> Self {
        Self { id: id.into() }
    }
}

impl ModelProvider for DeterministicInlinePredictionProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            completion: false,
            embedding: false,
            batch: false,
            inline_prediction: true,
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

    fn predict_inline(
        &self,
        request: InlinePredictionRequest,
    ) -> Result<InlinePredictionResponse, ProviderError> {
        deterministic_inline_prediction(&self.id, request)
    }
}

fn deterministic_inline_prediction(
    provider_id: &str,
    request: InlinePredictionRequest,
) -> Result<InlinePredictionResponse, ProviderError> {
    validate_inline_prediction_request_metadata(&request.metadata).map_err(|error| {
        ProviderError::RequestRejected {
            message: format!("invalid inline prediction metadata: {error}"),
        }
    })?;

    let text = deterministic_inline_text(&request.metadata, request.metadata.max_prediction_bytes);
    let byte_len = text.len() as u32;
    let line_count = text.bytes().filter(|byte| *byte == b'\n').count() as u32 + 1;
    let provider_metadata = InlinePredictionProviderMetadata {
        provider_id: provider_id.to_string(),
        model_label: request.model.clone(),
        operation_class: AssistedAiOperationClass::InlinePrediction,
        invocation_state: AssistedAiProviderInvocationState::Completed,
        latency: legion_protocol::InlinePredictionLatencyMetadata {
            queued_ms: 0,
            inference_ms: 1,
            total_ms: 1,
            timed_out: false,
        },
        ..request.metadata.provider.clone()
    };
    let insert_range = ProtocolTextRange {
        start: request.metadata.cursor,
        end: request.metadata.cursor,
    };
    let result = InlinePredictionResult {
        result_id: InlinePredictionResultId(format!("{}:result", request.metadata.request_id.0)),
        request_id: request.metadata.request_id.clone(),
        state: InlinePredictionResultState::Available,
        retention: InlinePredictionRetention::EphemeralDisplay,
        insert_range,
        ghost_text: Some(InlinePredictionGhostText {
            text,
            byte_len,
            line_count,
            text_fingerprint: legion_protocol::FileFingerprint {
                algorithm: "deterministic-inline-v1".to_string(),
                value: format!(
                    "{}:{}:{}",
                    request.metadata.language_id.0, request.metadata.cursor.line, byte_len
                ),
            },
        }),
        fingerprint: request.metadata.fingerprint.clone(),
        freshness: InlinePredictionFreshness::fresh(request.metadata.schema_version),
        provider: provider_metadata,
        refusal: None,
        generated_at: request.metadata.requested_at,
        expires_at: None,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: request.metadata.schema_version,
    };
    validate_inline_prediction_result(&result).map_err(|error| ProviderError::RequestRejected {
        message: format!("invalid deterministic inline prediction result: {error}"),
    })?;

    Ok(InlinePredictionResponse {
        provider: provider_id.to_string(),
        model: request.model,
        result,
        metadata: [("redaction".to_string(), "metadata-only".to_string())]
            .into_iter()
            .collect(),
    })
}

fn deterministic_inline_text(
    metadata: &legion_protocol::InlinePredictionRequestMetadata,
    max_bytes: u32,
) -> String {
    let line = metadata.cursor.line.saturating_add(1);
    let base = match metadata.language_id.0.to_ascii_lowercase().as_str() {
        "rust" | "typescript" | "javascript" => format!(" // next edit line {line}"),
        "python" => format!("  # next edit line {line}"),
        _ => format!(" next edit line {line}"),
    };
    bounded_ascii_prefix(&base, max_bytes)
}

fn bounded_ascii_prefix(value: &str, max_bytes: u32) -> String {
    let mut end = value.len().min(max_bytes as usize);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
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

        if matches!(
            request.provider_class,
            AssistedAiProviderClass::Gateway | AssistedAiProviderClass::Unknown
        ) {
            return Ok(self.refused_response(
                &request,
                "provider.class_unsupported",
                "provider class is not authorized for direct routing",
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
                context: legion_protocol::CapabilityRequestContext {
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
            ChatCompletionRequest::new(
                &request.provider_id,
                &request.model_label,
                route_prompt(&request),
            )
            .with_metadata(
                "context_manifest",
                request.context_manifest.reference_id.clone(),
            )
            .with_metadata("route_id", request.route_id.clone()),
        )?;

        if completion.provider != request.provider_id || completion.model != request.model_label {
            return Ok(self.refused_response(
                &request,
                "provider.identity_mismatch",
                "provider returned a different provider/model than requested",
            ));
        }

        Ok(AssistedAiProviderRouteResponse {
            route_id: request.route_id.clone(),
            invocation_state: AssistedAiProviderInvocationState::Completed,
            route_decision: allowed_route_decision(request.schema_version),
            provider_id: request.provider_id.clone(),
            model_label: request.model_label.clone(),
            output_labels: route_output_labels(&completion),
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

fn allowed_route_decision(schema_version: u16) -> AssistedAiRouteDecision {
    AssistedAiRouteDecision {
        disposition: AssistedAiRequestDisposition::MetadataOnlyReady,
        provider_invocation: AssistedAiProviderInvocationState::Completed,
        refusal: None,
        reasons: vec!["provider.completed.metadata_only".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn route_prompt(request: &AssistedAiProviderRouteRequest) -> String {
    let prompt = format!(
        "operation={:?}\ncontext_ref={}\nprivacy_ref={}\npermission_ref={}\nintent_labels={}\ntarget_count={}\nredaction=metadata-only",
        request.operation_class,
        request.context_manifest.reference_id,
        request.privacy_inspector.reference_id,
        request.permission_budget.reference_id,
        request.proposal_intent.labels.join(","),
        request.proposal_intent.target_coverage.targets.len(),
    );
    if request.prompt_prefix.is_empty() {
        prompt
    } else {
        format!("{}\n\n{}", request.prompt_prefix, prompt)
    }
}

fn route_output_labels(completion: &ChatCompletionResponse) -> Vec<String> {
    let mut labels = vec![
        format!("answer.fingerprint:{}", stable_label_hash(&completion.text)),
        format!("response.bytes:{}", completion.text.len()),
    ];
    if let Some(answer_label) = completion.metadata.get("answer.label") {
        labels.push(format!(
            "answer.label:{}",
            bounded_metadata_label(answer_label, 96)
        ));
    }
    if let Some(redaction) = completion.metadata.get("redaction")
        && redaction == "metadata-only"
    {
        labels.push("redaction:metadata-only".to_string());
    }
    labels
}

fn stable_label_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn bounded_metadata_label(value: &str, limit: usize) -> String {
    let mut label = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(character, ':' | '.' | '_' | '-' | '/' | '#')
            {
                character
            } else {
                '.'
            }
        })
        .collect::<String>();
    if label.len() > limit {
        label.truncate(limit);
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        AssistedAiOperationClass, AssistedAiProposalTargetIntent, AssistedAiProviderClass,
        AssistedAiProviderInvocationState, AssistedAiTrustProjectionKind,
        AssistedAiTrustProjectionReference, BufferId, BufferVersion, CancellationTokenId,
        CapabilityId, CapabilityNamespace, CausalityId, CorrelationId, EventSequence,
        FileContentVersion, FileFingerprint, FileId, InlinePredictionFingerprintMetadata,
        InlinePredictionLatencyMetadata, InlinePredictionProviderMetadata,
        InlinePredictionRequestId, InlinePredictionRequestMetadata, InlinePredictionResultState,
        InlinePredictionTriggerKind, LanguageId, NetworkTarget, PrincipalId, ProposalPayloadKind,
        ProposalPrivacyLabel, ProposalRiskLabel, ProposalTargetCoverage,
        ProposalTargetCoverageKind, RedactionHint, SnapshotId, TimestampMillis,
        WorkspaceGeneration, WorkspaceId, WorkspaceTrustState, validate_inline_prediction_result,
    };
    use legion_security::{AiProviderPolicy, DenyByDefaultBroker, NetworkPolicy, SecurityPolicy};
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
                batch: false,
                inline_prediction: false,
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

    struct MismatchedIdentityProvider;

    impl ModelProvider for MismatchedIdentityProvider {
        fn provider_id(&self) -> ProviderId {
            "local".to_string()
        }

        fn complete(
            &self,
            _request: ChatCompletionRequest,
        ) -> Result<ChatCompletionResponse, ProviderError> {
            Ok(ChatCompletionResponse {
                provider: "impostor".to_string(),
                model: "other-model".to_string(),
                text: "metadata-only".to_string(),
                metadata: HashMap::new(),
            })
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
            prompt_prefix: String::new(),
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

    fn inline_prediction_request(max_prediction_bytes: u32) -> InlinePredictionRequest {
        InlinePredictionRequest {
            provider: "deterministic-inline".to_string(),
            model: "zeta2-style-deterministic".to_string(),
            metadata: InlinePredictionRequestMetadata {
                request_id: InlinePredictionRequestId("inline:req:ai:1".to_string()),
                workspace_id: WorkspaceId(11),
                buffer_id: BufferId(22),
                file_id: Some(FileId(33)),
                language_id: LanguageId("rust".to_string()),
                cursor: legion_protocol::TextCoordinate {
                    line: 4,
                    character: 8,
                    byte_offset: Some(120),
                    utf16_offset: Some(120),
                },
                selection: None,
                visible_range: None,
                trigger: InlinePredictionTriggerKind::Automatic,
                fingerprint: InlinePredictionFingerprintMetadata {
                    snapshot_id: SnapshotId(66),
                    buffer_version: BufferVersion(55),
                    file_content_version: Some(FileContentVersion(44)),
                    workspace_generation: WorkspaceGeneration(77),
                    content_fingerprint: Some(FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "content".to_string(),
                    }),
                    context_fingerprint: FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "context".to_string(),
                    },
                    schema_version: 1,
                },
                provider: InlinePredictionProviderMetadata {
                    provider_id: "deterministic-inline".to_string(),
                    model_label: "zeta2-style-deterministic".to_string(),
                    provider_class: AssistedAiProviderClass::Local,
                    operation_class: AssistedAiOperationClass::InlinePrediction,
                    invocation_state: AssistedAiProviderInvocationState::Planned,
                    latency: InlinePredictionLatencyMetadata {
                        queued_ms: 0,
                        inference_ms: 0,
                        total_ms: 0,
                        timed_out: false,
                    },
                    health_labels: vec!["deterministic".to_string()],
                    cost_labels: vec!["local".to_string()],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                },
                max_prediction_bytes,
                timeout_ms: 100,
                requested_at: TimestampMillis(2000),
                cancellation_token: CancellationTokenId(
                    Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
                ),
                required_capability: CapabilityId("ai.inline_prediction.invoke".to_string()),
                principal_id: PrincipalId("principal".to_string()),
                workspace_trust_state: WorkspaceTrustState::Trusted,
                correlation_id: CorrelationId(7),
                causality_id: CausalityId(
                    Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap(),
                ),
                event_sequence: EventSequence(3),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
        }
    }

    #[test]
    fn deterministic_inline_prediction_provider_uses_inline_path_not_chat_completion() {
        let provider = DeterministicInlinePredictionProvider::new("deterministic-inline");
        let capabilities = provider.capabilities();
        assert!(!capabilities.completion);
        assert!(capabilities.inline_prediction);

        let chat_error = provider
            .complete(ChatCompletionRequest::new(
                "deterministic-inline",
                "zeta2-style-deterministic",
                "do not route through chat",
            ))
            .expect_err("inline predictor must not accept chat completion");
        assert!(matches!(
            chat_error,
            ProviderError::OperationUnavailable { operation, .. } if operation == "complete"
        ));

        let response = provider
            .predict_inline(inline_prediction_request(18))
            .expect("deterministic inline prediction succeeds");

        assert_eq!(response.provider, "deterministic-inline");
        assert_eq!(
            response.result.state,
            InlinePredictionResultState::Available
        );
        assert_eq!(
            response.result.provider.operation_class,
            AssistedAiOperationClass::InlinePrediction
        );
        let ghost_text = response
            .result
            .ghost_text
            .as_ref()
            .expect("deterministic path returns bounded ghost text");
        assert!(ghost_text.byte_len <= 18);
        assert_eq!(ghost_text.byte_len, ghost_text.text.len() as u32);
        validate_inline_prediction_result(&response.result).expect("protocol result is valid");
    }

    #[test]
    fn bounded_ascii_prefix_truncates_to_char_boundary_before_byte_limit() {
        assert_eq!(bounded_ascii_prefix("abcédef", 4), "abc");
    }

    #[test]
    fn route_prompt_prepends_instruction_prefix() {
        let mut request = route_request(AssistedAiProviderClass::LocalLoopback);
        request.prompt_prefix = "workspace AGENTS.md\nbe precise".to_string();

        let prompt = route_prompt(&request);

        assert!(prompt.starts_with("workspace AGENTS.md\nbe precise\n\noperation=ProposeEdit"));
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
        assert!(
            response
                .output_labels
                .iter()
                .any(|label| label.starts_with("answer.fingerprint:"))
        );
        assert!(
            response
                .output_labels
                .contains(&"response.bytes:13".to_string())
        );
    }

    #[test]
    fn router_refuses_remote_provider_when_policy_denies_target() {
        let registry = ProviderRegistry::new();
        let broker = DenyByDefaultBroker::default();
        let router = ProviderRouter::new(&registry, &broker);

        let mut request = route_request(AssistedAiProviderClass::HostedRemote);
        request.network_target = Some(NetworkTarget {
            scheme: "https".to_string(),
            host: "api.openai.com".to_string(),
            port: Some(443),
        });
        let response = router
            .route_completion(request)
            .expect("remote route refusal is represented as metadata");

        assert_eq!(
            response.invocation_state,
            AssistedAiProviderInvocationState::Refused
        );
        assert_eq!(
            response.output_labels,
            vec!["output.not_encoded".to_string()]
        );
        assert_eq!(
            response.refusal.as_ref().unwrap().reason_code,
            "capability.denied"
        );
    }

    #[test]
    fn router_invokes_remote_byok_provider_when_policy_allows_target() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(LocalProvider));
        let policy = SecurityPolicy {
            network_policy: NetworkPolicy {
                allowlist: vec!["api.openai.com".to_string()],
                air_gap: false,
                local_provider_only: false,
                ..NetworkPolicy::default()
            },
            ai_provider_policy: AiProviderPolicy {
                allow_remote_provider: true,
                ..AiProviderPolicy::default()
            },
            ..SecurityPolicy::default()
        };
        let broker = DenyByDefaultBroker::new(policy, CapabilityNamespace("test".to_string()));
        let router = ProviderRouter::new(&registry, &broker);
        let mut request = route_request(AssistedAiProviderClass::ByokRemote);
        request.network_target = Some(NetworkTarget {
            scheme: "https".to_string(),
            host: "api.openai.com".to_string(),
            port: Some(443),
        });

        let response = router
            .route_completion(request)
            .expect("allowed remote route completes");

        assert_eq!(
            response.invocation_state,
            AssistedAiProviderInvocationState::Completed
        );
        assert!(
            response
                .output_labels
                .iter()
                .any(|label| label.starts_with("answer.fingerprint:"))
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
    fn router_refuses_when_provider_returns_mismatched_identity() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MismatchedIdentityProvider));
        let broker = DenyByDefaultBroker::default();
        let router = ProviderRouter::new(&registry, &broker);

        let response = router
            .route_completion(route_request(AssistedAiProviderClass::LocalLoopback))
            .expect("identity mismatch is represented as metadata");

        assert_eq!(
            response.invocation_state,
            AssistedAiProviderInvocationState::Refused
        );
        assert_eq!(
            response.refusal.as_ref().unwrap().reason_code,
            "provider.identity_mismatch"
        );
    }

    #[test]
    fn route_decision_propagates_request_schema_version() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(LocalProvider));
        let broker = DenyByDefaultBroker::default();
        let router = ProviderRouter::new(&registry, &broker);
        let mut request = route_request(AssistedAiProviderClass::LocalLoopback);
        request.schema_version = 2;

        let response = router.route_completion(request).expect("route completes");

        assert_eq!(response.schema_version, 2);
        assert_eq!(response.route_decision.schema_version, 2);
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

    #[test]
    fn batch_job_request_round_trips_repo_summary_metadata() {
        let request = BatchJobRequest::new(
            "provider-id",
            "model-a",
            "batch-repo-summary-3",
            "repo-summary",
            vec![ChatCompletionRequest::new(
                "provider-id",
                "model-a",
                "summarize this repository",
            )],
        )
        .with_metadata("pipeline", "ws10");

        let json = serde_json::to_value(&request).expect("batch request serializes");
        let decoded: BatchJobRequest = serde_json::from_value(json).expect("batch request decodes");

        assert_eq!(decoded.provider, "provider-id");
        assert_eq!(decoded.model, "model-a");
        assert_eq!(decoded.batch_id, "batch-repo-summary-3");
        assert_eq!(decoded.job_type, "repo-summary");
        assert_eq!(decoded.requests.len(), 1);
        assert_eq!(
            decoded.requests[0].messages[0].content,
            "summarize this repository"
        );
        assert_eq!(decoded.metadata.get("pipeline"), Some(&"ws10".to_string()));
    }
}
