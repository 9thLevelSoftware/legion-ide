//! Provider adapters: Ollama, llama.cpp, OpenAI, Anthropic, future gateway.

#![warn(missing_docs)]

use std::collections::{HashMap, hash_map::DefaultHasher};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use legion_ai::{
    ChatCompletionRequest, ChatCompletionResponse, ChatRole, EmbeddingRequest, EmbeddingResponse,
    InlinePredictionRequest, InlinePredictionResponse, ModelProvider, ProviderCapabilities,
    ProviderError, ProviderId,
};
use legion_protocol::{
    AssistedAiOperationClass, AssistedAiProviderAvailabilityState, AssistedAiProviderCapability,
    AssistedAiProviderClass, AssistedAiRefusalMetadata, AssistedAiSupportLabel, CapabilityId,
    DelegatedTaskToolPermissionProfile, DelegatedTaskToolPermissionRequest, FileFingerprint,
    LEGACY_PRODUCT_ENV_PREFIX, McpJsonRpcEnvelope, McpListChangedKind, McpPromptDescriptor,
    McpPromptName, McpRegistrySnapshot, McpResourceDescriptor, McpResourceUri, McpServerId,
    McpToolDescriptor, McpToolName, PRODUCT_ENV_PREFIX, PermissionBudgetActionClass,
    ProposalRiskLabel, RedactionHint, SemanticPrivacyScope, TimestampMillis,
    validate_mcp_json_rpc_envelope, validate_mcp_registry_snapshot,
};
use legion_security::mcp_tool_permission_allows_runtime;
use serde_json::{Value, json};
use thiserror::Error;

/// Deterministic local provider id used by Phase 4 contract tests.
pub const DETERMINISTIC_LOCAL_PROVIDER_ID: &str = "deterministic-local";
/// Ollama inline prediction provider slot.
pub const OLLAMA_PROVIDER_ID: &str = "ollama";
/// llama.cpp OpenAI-compatible loopback provider slot.
pub const LLAMA_CPP_PROVIDER_ID: &str = "llama-cpp";
/// OpenAI-compatible inline prediction provider slot.
pub const OPENAI_COMPATIBLE_PROVIDER_ID: &str = "openai-compatible";
/// Anthropic Messages API provider slot.
pub const ANTHROPIC_PROVIDER_ID: &str = "anthropic";
const ANTHROPIC_API_VERSION: &str = "2023-06-01";
const ANTHROPIC_STRUCTURED_OUTPUTS_BETA: &str = "structured-outputs-2025-11-13";
/// GitHub Copilot NES inline prediction provider slot.
pub const COPILOT_NES_PROVIDER_ID: &str = "copilot-nes";
/// Mercury inline prediction provider slot.
pub const MERCURY_PROVIDER_ID: &str = "mercury";
/// Codestral inline prediction provider slot.
pub const CODESTRAL_PROVIDER_ID: &str = "codestral";

/// Provider registry with local, loopback, and BYOK-capable model adapters.
pub fn make_provider_registry() -> legion_ai::ProviderRegistry {
    let mut registry = legion_ai::ProviderRegistry::new();
    registry.register(Box::new(DeterministicLocalProvider::new(
        DETERMINISTIC_LOCAL_PROVIDER_ID,
    )));
    registry.register(Box::new(OllamaProvider::default()));
    registry.register(Box::new(LlamaCppProvider::default()));
    registry.register(Box::new(OpenAiCompatibleProvider::from_env(
        OPENAI_COMPATIBLE_PROVIDER_ID,
    )));
    registry.register(Box::new(AnthropicMessagesClient::from_env(
        ANTHROPIC_PROVIDER_ID,
    )));
    registry
}

/// Provider registry for Phase 6 inline prediction slots.
pub fn make_inline_prediction_registry() -> legion_ai::ProviderRegistry {
    let mut registry = make_provider_registry();
    registry.register(Box::new(UnavailableInlineProvider::new(
        COPILOT_NES_PROVIDER_ID,
        "Copilot NES",
        AssistedAiProviderClass::HostedRemote,
    )));
    registry.register(Box::new(UnavailableInlineProvider::new(
        MERCURY_PROVIDER_ID,
        "Mercury",
        AssistedAiProviderClass::HostedRemote,
    )));
    registry.register(Box::new(UnavailableInlineProvider::new(
        CODESTRAL_PROVIDER_ID,
        "Codestral",
        AssistedAiProviderClass::ByokRemote,
    )));
    registry
}

/// Metadata-only provider capability entries for Phase 6 inline prediction.
pub fn inline_prediction_provider_capabilities() -> Vec<AssistedAiProviderCapability> {
    vec![
        provider_capability(
            DETERMINISTIC_LOCAL_PROVIDER_ID,
            "Deterministic local Zeta2-style",
            AssistedAiProviderClass::Local,
            AssistedAiProviderAvailabilityState::Available,
        ),
        provider_capability(
            OLLAMA_PROVIDER_ID,
            "Ollama",
            AssistedAiProviderClass::LocalLoopback,
            AssistedAiProviderAvailabilityState::Unavailable,
        ),
        provider_capability(
            LLAMA_CPP_PROVIDER_ID,
            "llama.cpp",
            AssistedAiProviderClass::LocalLoopback,
            AssistedAiProviderAvailabilityState::Unavailable,
        ),
        provider_capability(
            OPENAI_COMPATIBLE_PROVIDER_ID,
            "OpenAI-compatible",
            AssistedAiProviderClass::ByokRemote,
            AssistedAiProviderAvailabilityState::Unavailable,
        ),
        provider_capability(
            COPILOT_NES_PROVIDER_ID,
            "Copilot NES",
            AssistedAiProviderClass::HostedRemote,
            AssistedAiProviderAvailabilityState::Unavailable,
        ),
        provider_capability(
            MERCURY_PROVIDER_ID,
            "Mercury",
            AssistedAiProviderClass::HostedRemote,
            AssistedAiProviderAvailabilityState::Unavailable,
        ),
        provider_capability(
            CODESTRAL_PROVIDER_ID,
            "Codestral",
            AssistedAiProviderClass::ByokRemote,
            AssistedAiProviderAvailabilityState::Unavailable,
        ),
    ]
}

/// Deterministic local provider for policy/router tests without cloud credentials.
pub struct DeterministicLocalProvider {
    id: ProviderId,
}

impl DeterministicLocalProvider {
    /// Creates the deterministic local provider.
    pub fn new(id: impl Into<ProviderId>) -> Self {
        Self { id: id.into() }
    }
}

impl ModelProvider for DeterministicLocalProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            completion: true,
            embedding: true,
            inline_prediction: true,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let answer_fingerprint = metadata_hash(
            "deterministic-answer",
            &json!({
                "messages": request
                    .messages
                    .iter()
                    .map(|message| format!("{}:{}", chat_role_label(&message.role), message.content))
                    .collect::<Vec<_>>(),
                "metadata": request.metadata,
            }),
        )
        .value;
        let answer_label = format!("deterministic-answer:{answer_fingerprint}");
        Ok(ChatCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            text: answer_label.clone(),
            metadata: HashMap::from([
                ("answer.label".to_string(), answer_label),
                ("redaction".to_string(), "metadata-only".to_string()),
            ]),
        })
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Ok(EmbeddingResponse {
            provider: self.id.clone(),
            model: request.model,
            vectors: request
                .inputs
                .iter()
                .map(|input| deterministic_local_embedding(input))
                .collect(),
            metadata: HashMap::from([
                ("embedding".to_string(), "deterministic-local".to_string()),
                ("redaction".to_string(), "metadata-only".to_string()),
            ]),
        })
    }

    fn predict_inline(
        &self,
        request: InlinePredictionRequest,
    ) -> Result<InlinePredictionResponse, ProviderError> {
        legion_ai::DeterministicInlinePredictionProvider::new(self.id.clone())
            .predict_inline(request)
    }
}

fn deterministic_local_embedding(input: &str) -> Vec<f32> {
    const DIMENSIONS: usize = 16;
    let mut vector = vec![0.0_f32; DIMENSIONS];
    let mut token_count = 0.0_f32;

    for token in input
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        token_count += 1.0;
        let mut hasher = DefaultHasher::new();
        token.to_ascii_lowercase().hash(&mut hasher);
        let hash = hasher.finish();
        let index = (hash as usize) % DIMENSIONS;
        let sign = if (hash >> 8) & 1 == 0 { 1.0 } else { -1.0 };
        vector[index] += sign;
    }

    if token_count == 0.0 {
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        vector[(hasher.finish() as usize) % DIMENSIONS] = 1.0;
        return vector;
    }

    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

/// Synchronous JSON transport used by provider adapters.
pub trait ProviderHttpTransport: Clone + Send + Sync + 'static {
    /// Post a JSON request body and return the JSON response body.
    fn post_json(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        payload: Value,
    ) -> Result<Value, ProviderError>;
}

/// Reqwest-backed provider HTTP transport.
#[derive(Debug, Clone, Default)]
pub struct ReqwestProviderHttpTransport;

impl ProviderHttpTransport for ReqwestProviderHttpTransport {
    fn post_json(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        payload: Value,
    ) -> Result<Value, ProviderError> {
        let mut request = reqwest::blocking::Client::new()
            .post(endpoint)
            .json(&payload);
        if let Some(token) = bearer_token.filter(|token| !token.trim().is_empty()) {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .map_err(|error| ProviderError::RequestFailed {
                provider: "http".to_string(),
                message: error.to_string(),
            })?;
        if !response.status().is_success() {
            return Err(ProviderError::RequestFailed {
                provider: "http".to_string(),
                message: format!("{endpoint} returned {}", response.status()),
            });
        }
        response
            .json::<Value>()
            .map_err(|error| ProviderError::RequestFailed {
                provider: "http".to_string(),
                message: error.to_string(),
            })
    }
}

/// Configured Ollama loopback provider adapter.
#[derive(Debug, Clone)]
pub struct OllamaProvider<T = ReqwestProviderHttpTransport> {
    id: ProviderId,
    base_url: String,
    transport: T,
}

impl Default for OllamaProvider<ReqwestProviderHttpTransport> {
    fn default() -> Self {
        Self::new(
            OLLAMA_PROVIDER_ID,
            std::env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
        )
    }
}

impl OllamaProvider<ReqwestProviderHttpTransport> {
    /// Creates an Ollama adapter from an endpoint and the default HTTP transport.
    pub fn new(id: impl Into<ProviderId>, base_url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            base_url: normalize_base_url(base_url.into()),
            transport: ReqwestProviderHttpTransport,
        }
    }
}

impl<T> OllamaProvider<T>
where
    T: ProviderHttpTransport,
{
    /// Creates an Ollama adapter with an injected transport.
    pub fn with_transport(
        id: impl Into<ProviderId>,
        base_url: impl Into<String>,
        transport: T,
    ) -> Self {
        Self {
            id: id.into(),
            base_url: normalize_base_url(base_url.into()),
            transport,
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }
}

impl<T> ModelProvider for OllamaProvider<T>
where
    T: ProviderHttpTransport,
{
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            completion: true,
            embedding: true,
            inline_prediction: false,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let payload = json!({
            "model": request.model,
            "prompt": chat_prompt(&request),
            "stream": false,
            "options": {
                "temperature": request.temperature.unwrap_or(0.0),
            },
        });
        let response = self
            .transport
            .post_json(&self.endpoint("/api/generate"), None, payload)?;
        let text = response
            .get("response")
            .and_then(Value::as_str)
            .or_else(|| response.get("text").and_then(Value::as_str))
            .ok_or_else(|| ProviderError::RequestFailed {
                provider: self.id.clone(),
                message: "Ollama generate response missing response text".to_string(),
            })?
            .to_string();
        Ok(ChatCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            text,
            metadata: provider_metadata("ollama", &self.base_url),
        })
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        let mut vectors = Vec::with_capacity(request.inputs.len());
        for input in &request.inputs {
            let response = self.transport.post_json(
                &self.endpoint("/api/embeddings"),
                None,
                json!({
                    "model": request.model,
                    "prompt": input,
                }),
            )?;
            vectors.push(parse_embedding_vector(&self.id, &response, "embedding")?);
        }
        Ok(EmbeddingResponse {
            provider: self.id.clone(),
            model: request.model,
            vectors,
            metadata: provider_metadata("ollama", &self.base_url),
        })
    }

    fn predict_inline(
        &self,
        request: InlinePredictionRequest,
    ) -> Result<InlinePredictionResponse, ProviderError> {
        Err(ProviderError::unavailable(
            request.provider,
            "Ollama inline prediction provider is not configured",
        ))
    }
}

/// Configured OpenAI-compatible BYOK provider adapter.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider<T = ReqwestProviderHttpTransport> {
    id: ProviderId,
    base_url: String,
    api_key: Option<String>,
    auth_policy: OpenAiCompatibleAuthPolicy,
    metadata_kind: &'static str,
    transport: T,
}

#[derive(Debug, Clone, Copy)]
enum OpenAiCompatibleAuthPolicy {
    Required,
    Optional,
}

impl OpenAiCompatibleProvider<ReqwestProviderHttpTransport> {
    /// Creates a BYOK OpenAI-compatible adapter from environment configuration.
    pub fn from_env(id: impl Into<ProviderId>) -> Self {
        let api_key = first_configured_value([
            std::env::var(format!("{PRODUCT_ENV_PREFIX}_OPENAI_COMPATIBLE_API_KEY")).ok(),
            std::env::var(format!(
                "{LEGACY_PRODUCT_ENV_PREFIX}_OPENAI_COMPATIBLE_API_KEY"
            ))
            .ok(),
            std::env::var("OPENAI_API_KEY").ok(),
        ]);
        let base_url = first_configured_value([
            std::env::var(format!("{PRODUCT_ENV_PREFIX}_OPENAI_COMPATIBLE_BASE_URL")).ok(),
            std::env::var(format!(
                "{LEGACY_PRODUCT_ENV_PREFIX}_OPENAI_COMPATIBLE_BASE_URL"
            ))
            .ok(),
            std::env::var("OPENAI_BASE_URL").ok(),
        ])
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        Self::with_transport(id, base_url, api_key, ReqwestProviderHttpTransport)
    }
}

impl<T> OpenAiCompatibleProvider<T>
where
    T: ProviderHttpTransport,
{
    /// Creates an OpenAI-compatible adapter with an injected transport.
    pub fn with_transport(
        id: impl Into<ProviderId>,
        base_url: impl Into<String>,
        api_key: Option<String>,
        transport: T,
    ) -> Self {
        Self::with_transport_and_auth_policy(
            id,
            base_url,
            api_key,
            OpenAiCompatibleAuthPolicy::Required,
            "openai-compatible",
            transport,
        )
    }

    fn with_transport_and_auth_policy(
        id: impl Into<ProviderId>,
        base_url: impl Into<String>,
        api_key: Option<String>,
        auth_policy: OpenAiCompatibleAuthPolicy,
        metadata_kind: &'static str,
        transport: T,
    ) -> Self {
        Self {
            id: id.into(),
            base_url: normalize_base_url(base_url.into()),
            api_key,
            auth_policy,
            metadata_kind,
            transport,
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn bearer_token(&self) -> Result<Option<&str>, ProviderError> {
        if let Some(api_key) = self.api_key.as_deref().filter(|key| !key.trim().is_empty()) {
            return Ok(Some(api_key));
        }
        match self.auth_policy {
            OpenAiCompatibleAuthPolicy::Required => Err(ProviderError::unavailable(
                self.id.clone(),
                "OpenAI-compatible API key is not configured",
            )),
            OpenAiCompatibleAuthPolicy::Optional => Ok(None),
        }
    }
}

impl<T> ModelProvider for OpenAiCompatibleProvider<T>
where
    T: ProviderHttpTransport,
{
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            completion: true,
            embedding: true,
            inline_prediction: false,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let bearer_token = self.bearer_token()?;
        let response = self.transport.post_json(
            &self.endpoint("/chat/completions"),
            bearer_token,
            json!({
                "model": request.model,
                "messages": request.messages.iter().map(|message| {
                    json!({
                        "role": chat_role_label(&message.role),
                        "content": message.content,
                    })
                }).collect::<Vec<_>>(),
                "max_tokens": request.max_tokens,
                "temperature": request.temperature,
            }),
        )?;
        let text = response
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
            .ok_or_else(|| ProviderError::RequestFailed {
                provider: self.id.clone(),
                message: "OpenAI-compatible chat response missing message content".to_string(),
            })?
            .to_string();
        Ok(ChatCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            text,
            metadata: provider_metadata(self.metadata_kind, &self.base_url),
        })
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        let bearer_token = self.bearer_token()?;
        let response = self.transport.post_json(
            &self.endpoint("/embeddings"),
            bearer_token,
            json!({
                "model": request.model,
                "input": request.inputs,
            }),
        )?;
        let data = response
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::RequestFailed {
                provider: self.id.clone(),
                message: "OpenAI-compatible embedding response missing data".to_string(),
            })?;
        let mut vectors = Vec::with_capacity(data.len());
        for item in data {
            vectors.push(parse_embedding_vector(&self.id, item, "embedding")?);
        }
        Ok(EmbeddingResponse {
            provider: self.id.clone(),
            model: request.model,
            vectors,
            metadata: provider_metadata(self.metadata_kind, &self.base_url),
        })
    }

    fn predict_inline(
        &self,
        request: InlinePredictionRequest,
    ) -> Result<InlinePredictionResponse, ProviderError> {
        Err(ProviderError::unavailable(
            request.provider,
            "OpenAI-compatible inline prediction provider is not configured",
        ))
    }
}

/// Configured llama.cpp OpenAI-compatible loopback provider adapter.
#[derive(Debug, Clone)]
pub struct LlamaCppProvider<T = ReqwestProviderHttpTransport> {
    inner: OpenAiCompatibleProvider<T>,
}

impl Default for LlamaCppProvider<ReqwestProviderHttpTransport> {
    fn default() -> Self {
        Self::from_env(LLAMA_CPP_PROVIDER_ID)
    }
}

impl LlamaCppProvider<ReqwestProviderHttpTransport> {
    /// Creates a llama.cpp adapter from environment configuration.
    ///
    /// Current product-prefixed names take priority over legacy product-prefixed names.
    /// names, then unprefixed `LLAMA_CPP_*` names. The default endpoint is the
    /// llama.cpp `llama-server` OpenAI-compatible base URL.
    pub fn from_env(id: impl Into<ProviderId>) -> Self {
        let api_key = first_configured_value([
            std::env::var(format!("{PRODUCT_ENV_PREFIX}_LLAMA_CPP_API_KEY")).ok(),
            std::env::var(format!("{LEGACY_PRODUCT_ENV_PREFIX}_LLAMA_CPP_API_KEY")).ok(),
            std::env::var("LLAMA_CPP_API_KEY").ok(),
        ]);
        let base_url = first_configured_value([
            std::env::var(format!("{PRODUCT_ENV_PREFIX}_LLAMA_CPP_BASE_URL")).ok(),
            std::env::var(format!("{LEGACY_PRODUCT_ENV_PREFIX}_LLAMA_CPP_BASE_URL")).ok(),
            std::env::var("LLAMA_CPP_BASE_URL").ok(),
        ])
        .unwrap_or_else(|| "http://localhost:8080/v1".to_string());
        Self::with_transport(id, base_url, api_key, ReqwestProviderHttpTransport)
    }
}

impl<T> LlamaCppProvider<T>
where
    T: ProviderHttpTransport,
{
    /// Creates a llama.cpp adapter with an injected transport.
    pub fn with_transport(
        id: impl Into<ProviderId>,
        base_url: impl Into<String>,
        api_key: Option<String>,
        transport: T,
    ) -> Self {
        Self {
            inner: OpenAiCompatibleProvider::with_transport_and_auth_policy(
                id,
                base_url,
                api_key,
                OpenAiCompatibleAuthPolicy::Optional,
                "llama-cpp",
                transport,
            ),
        }
    }
}

impl<T> ModelProvider for LlamaCppProvider<T>
where
    T: ProviderHttpTransport,
{
    fn provider_id(&self) -> ProviderId {
        self.inner.provider_id()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        self.inner.complete(request)
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        self.inner.embed(request)
    }

    fn predict_inline(
        &self,
        request: InlinePredictionRequest,
    ) -> Result<InlinePredictionResponse, ProviderError> {
        Err(ProviderError::unavailable(
            request.provider,
            "llama.cpp inline prediction provider is not configured",
        ))
    }
}

/// Configured Anthropic Messages API client adapter.
#[derive(Debug, Clone)]
pub struct AnthropicMessagesClient<T = ReqwestProviderHttpTransport> {
    id: ProviderId,
    base_url: String,
    api_key: Option<String>,
    transport: T,
}

/// Anthropic-specific request extras used for strict tools, structured outputs, and thinking.
#[derive(Debug, Clone, Default)]
pub struct AnthropicRequestExtras {
    /// Strict tool definitions passed through to the Messages API.
    pub tools: Vec<Value>,
    /// Structured output configuration passed through to `output_config`.
    pub output_config: Option<Value>,
    /// Optional thinking configuration passed through to the request body.
    pub thinking: Option<Value>,
}

/// SSE event kinds emitted by the Anthropic Messages API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnthropicSseEvent {
    /// The assistant message has started.
    MessageStart,
    /// A content block has started.
    ContentBlockStart,
    /// A text delta was emitted for the active content block.
    ContentBlockDelta(String),
    /// A content block has finished.
    ContentBlockStop,
    /// Top-level message metadata changed.
    MessageDelta,
    /// The assistant message has ended.
    MessageStop,
    /// A streaming event we do not currently interpret.
    Unknown(String),
}

/// JSON Schema structured-output helper for Anthropic request bodies.
pub fn anthropic_json_schema_output_config(name: impl Into<String>, schema: Value) -> Value {
    json!({
        "format": {
            "type": "json_schema",
            "name": name.into(),
            "schema": schema,
        }
    })
}

/// Strict tool definition helper for Anthropic request bodies.
pub fn anthropic_strict_tool_definition(
    name: impl Into<String>,
    description: impl Into<String>,
    input_schema: Value,
) -> Value {
    json!({
        "name": name.into(),
        "description": description.into(),
        "input_schema": input_schema,
        "strict": true,
    })
}

/// Shared HTTP transport abstraction for Anthropic Messages API calls.
pub trait AnthropicMessagesTransport: Clone + Send + Sync + 'static {
    /// POST a JSON payload and return the parsed JSON response.
    fn post_json(
        &self,
        endpoint: &str,
        api_key: Option<&str>,
        beta_header: Option<&str>,
        payload: Value,
    ) -> Result<Value, ProviderError>;

    /// POST a JSON payload and return the raw text response body.
    fn post_text(
        &self,
        endpoint: &str,
        api_key: Option<&str>,
        beta_header: Option<&str>,
        payload: Value,
    ) -> Result<String, ProviderError>;
}

impl AnthropicMessagesTransport for ReqwestProviderHttpTransport {
    fn post_json(
        &self,
        endpoint: &str,
        api_key: Option<&str>,
        beta_header: Option<&str>,
        payload: Value,
    ) -> Result<Value, ProviderError> {
        let mut request = reqwest::blocking::Client::new()
            .post(endpoint)
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .json(&payload);
        if let Some(beta_header) = beta_header.filter(|value| !value.trim().is_empty()) {
            request = request.header("anthropic-beta", beta_header);
        }
        if let Some(token) = api_key.filter(|token| !token.trim().is_empty()) {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .map_err(|error| ProviderError::RequestFailed {
                provider: "http".to_string(),
                message: error.to_string(),
            })?;
        if !response.status().is_success() {
            return Err(ProviderError::RequestFailed {
                provider: "http".to_string(),
                message: format!("{endpoint} returned {}", response.status()),
            });
        }
        response
            .json::<Value>()
            .map_err(|error| ProviderError::RequestFailed {
                provider: "http".to_string(),
                message: error.to_string(),
            })
    }

    fn post_text(
        &self,
        endpoint: &str,
        bearer_token: Option<&str>,
        beta_header: Option<&str>,
        payload: Value,
    ) -> Result<String, ProviderError> {
        let mut request = reqwest::blocking::Client::new()
            .post(endpoint)
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .json(&payload);
        if let Some(beta_header) = beta_header.filter(|value| !value.trim().is_empty()) {
            request = request.header("anthropic-beta", beta_header);
        }
        if let Some(token) = bearer_token.filter(|token| !token.trim().is_empty()) {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .map_err(|error| ProviderError::RequestFailed {
                provider: "http".to_string(),
                message: error.to_string(),
            })?;
        if !response.status().is_success() {
            return Err(ProviderError::RequestFailed {
                provider: "http".to_string(),
                message: format!("{endpoint} returned {}", response.status()),
            });
        }
        response
            .text()
            .map_err(|error| ProviderError::RequestFailed {
                provider: "http".to_string(),
                message: error.to_string(),
            })
    }
}

impl Default for AnthropicMessagesClient<ReqwestProviderHttpTransport> {
    fn default() -> Self {
        Self::from_env(ANTHROPIC_PROVIDER_ID)
    }
}

impl AnthropicMessagesClient<ReqwestProviderHttpTransport> {
    /// Creates an Anthropic adapter from environment configuration.
    pub fn from_env(id: impl Into<ProviderId>) -> Self {
        let api_key = first_configured_value([
            std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_API_KEY")).ok(),
            std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_AUTH_TOKEN")).ok(),
            std::env::var(format!("{LEGACY_PRODUCT_ENV_PREFIX}_ANTHROPIC_API_KEY")).ok(),
            std::env::var(format!("{LEGACY_PRODUCT_ENV_PREFIX}_ANTHROPIC_AUTH_TOKEN")).ok(),
            std::env::var("ANTHROPIC_API_KEY").ok(),
            std::env::var("ANTHROPIC_AUTH_TOKEN").ok(),
        ]);
        let base_url = first_configured_value([
            std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_BASE_URL")).ok(),
            std::env::var(format!("{LEGACY_PRODUCT_ENV_PREFIX}_ANTHROPIC_BASE_URL")).ok(),
            std::env::var("ANTHROPIC_BASE_URL").ok(),
        ])
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
        Self::with_transport(id, base_url, api_key, ReqwestProviderHttpTransport)
    }
}

impl<T> AnthropicMessagesClient<T>
where
    T: AnthropicMessagesTransport,
{
    /// Creates an Anthropic adapter with an injected transport.
    pub fn with_transport(
        id: impl Into<ProviderId>,
        base_url: impl Into<String>,
        api_key: Option<String>,
        transport: T,
    ) -> Self {
        Self {
            id: id.into(),
            base_url: normalize_base_url(base_url.into()),
            api_key,
            transport,
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn bearer_token(&self) -> Result<&str, ProviderError> {
        self.api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ProviderError::unavailable(
                    self.id.clone(),
                    "Anthropic API key or auth token is not configured",
                )
            })
    }

    fn request_messages(request: &ChatCompletionRequest) -> (Option<String>, Vec<Value>) {
        let mut system_messages = Vec::new();
        let mut messages = Vec::new();
        for message in &request.messages {
            match message.role {
                ChatRole::System => system_messages.push(message.content.clone()),
                ChatRole::User | ChatRole::Assistant => messages.push(json!({
                    "role": chat_role_label(&message.role),
                    "content": [{
                        "type": "text",
                        "text": message.content,
                    }],
                })),
            }
        }
        let system = if system_messages.is_empty() {
            None
        } else {
            Some(system_messages.join("\n"))
        };
        (system, messages)
    }

    fn completion_payload(
        &self,
        request: &ChatCompletionRequest,
        stream: bool,
        extras: &AnthropicRequestExtras,
    ) -> Value {
        let (system, messages) = Self::request_messages(request);
        let mut payload = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(1024),
            "messages": messages,
            "stream": stream,
        });
        if let Some(system) = system {
            payload["system"] = json!(system);
        }
        if let Some(temperature) = request.temperature {
            payload["temperature"] = json!(temperature);
        }
        if !extras.tools.is_empty() {
            payload["tools"] = json!(extras.tools);
        }
        if let Some(output_config) = extras.output_config.clone() {
            payload["output_config"] = output_config;
        }
        if let Some(thinking) = extras.thinking.clone() {
            payload["thinking"] = thinking;
        }
        payload
    }

    fn count_tokens_payload(
        &self,
        request: &ChatCompletionRequest,
        extras: &AnthropicRequestExtras,
    ) -> Value {
        let (system, messages) = Self::request_messages(request);
        let mut payload = json!({
            "model": request.model,
            "messages": messages,
        });
        if let Some(system) = system {
            payload["system"] = json!(system);
        }
        if !extras.tools.is_empty() {
            payload["tools"] = json!(extras.tools);
        }
        payload
    }

    fn beta_header_for_extras(extras: &AnthropicRequestExtras) -> Option<&'static str> {
        let uses_strict_tools = extras
            .tools
            .iter()
            .any(|tool| tool.get("strict").and_then(Value::as_bool) == Some(true));
        if uses_strict_tools || extras.output_config.is_some() {
            Some(ANTHROPIC_STRUCTURED_OUTPUTS_BETA)
        } else {
            None
        }
    }

    fn extract_assistant_text(response: &Value) -> Result<String, ProviderError> {
        let content = response
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::RequestFailed {
                provider: "anthropic".to_string(),
                message: "Anthropic response missing content blocks".to_string(),
            })?;
        let mut text = String::new();
        for block in content {
            if block.get("type").and_then(Value::as_str) == Some("text")
                && let Some(chunk) = block.get("text").and_then(Value::as_str)
            {
                text.push_str(chunk);
            }
        }
        if text.is_empty() {
            return Err(ProviderError::RequestFailed {
                provider: "anthropic".to_string(),
                message: "Anthropic response missing text content".to_string(),
            });
        }
        Ok(text)
    }

    fn extract_input_tokens(response: &Value) -> Result<u32, ProviderError> {
        response
            .get("input_tokens")
            .and_then(Value::as_u64)
            .map(|value| value as u32)
            .ok_or_else(|| ProviderError::RequestFailed {
                provider: "anthropic".to_string(),
                message: "Anthropic token-count response missing input_tokens".to_string(),
            })
    }

    fn parse_sse_events(body: &str) -> Result<Vec<AnthropicSseEvent>, ProviderError> {
        let mut events = Vec::new();
        let mut current_event: Option<String> = None;
        let mut current_data = String::new();
        let flush =
            |events: &mut Vec<AnthropicSseEvent>, event: Option<String>, data: &mut String| {
                let Some(event) = event else {
                    data.clear();
                    return Ok::<(), ProviderError>(());
                };
                if event == "ping" {
                    data.clear();
                    return Ok(());
                }
                let payload = if data.trim().is_empty() {
                    Value::Null
                } else {
                    serde_json::from_str::<Value>(data).map_err(|error| {
                        ProviderError::RequestFailed {
                            provider: "anthropic".to_string(),
                            message: error.to_string(),
                        }
                    })?
                };
                let parsed = match event.as_str() {
                    "message_start" => AnthropicSseEvent::MessageStart,
                    "content_block_start" => AnthropicSseEvent::ContentBlockStart,
                    "content_block_delta" => {
                        let text = payload
                            .get("delta")
                            .and_then(|delta| delta.get("text"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        AnthropicSseEvent::ContentBlockDelta(text)
                    }
                    "content_block_stop" => AnthropicSseEvent::ContentBlockStop,
                    "message_delta" => AnthropicSseEvent::MessageDelta,
                    "message_stop" => AnthropicSseEvent::MessageStop,
                    other => AnthropicSseEvent::Unknown(other.to_string()),
                };
                events.push(parsed);
                data.clear();
                Ok(())
            };

        for line in body.lines() {
            let line = line.trim_end();
            if line.is_empty() {
                flush(&mut events, current_event.take(), &mut current_data)?;
                continue;
            }
            if let Some(rest) = line.strip_prefix("event:") {
                current_event = Some(rest.trim().to_string());
                continue;
            }
            if let Some(rest) = line.strip_prefix("data:") {
                if !current_data.is_empty() {
                    current_data.push('\n');
                }
                current_data.push_str(rest.trim_start());
            }
        }
        flush(&mut events, current_event.take(), &mut current_data)?;
        Ok(events)
    }

    /// Sends a completion request using the native Anthropic Messages API.
    pub fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        self.complete_with_extras(request, AnthropicRequestExtras::default())
    }

    /// Sends a completion request with Anthropic-only extras.
    pub fn complete_with_extras(
        &self,
        request: ChatCompletionRequest,
        extras: AnthropicRequestExtras,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let bearer_token = self.bearer_token()?;
        let response = self.transport.post_json(
            &self.endpoint("/v1/messages"),
            Some(bearer_token),
            Self::beta_header_for_extras(&extras),
            self.completion_payload(&request, false, &extras),
        )?;
        let text = Self::extract_assistant_text(&response)?;
        Ok(ChatCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            text,
            metadata: provider_metadata("anthropic", &self.base_url),
        })
    }

    /// Streams a completion request and returns the parsed SSE event sequence.
    pub fn stream_events_with_extras(
        &self,
        request: ChatCompletionRequest,
        extras: AnthropicRequestExtras,
    ) -> Result<Vec<AnthropicSseEvent>, ProviderError> {
        let bearer_token = self.bearer_token()?;
        let body = self.transport.post_text(
            &self.endpoint("/v1/messages"),
            Some(bearer_token),
            Self::beta_header_for_extras(&extras),
            self.completion_payload(&request, true, &extras),
        )?;
        Self::parse_sse_events(&body)
    }

    /// Streams a completion request and returns only text deltas, in arrival order.
    pub fn stream_text_deltas_with_extras(
        &self,
        request: ChatCompletionRequest,
        extras: AnthropicRequestExtras,
    ) -> Result<Vec<String>, ProviderError> {
        Ok(self
            .stream_events_with_extras(request, extras)?
            .into_iter()
            .filter_map(|event| match event {
                AnthropicSseEvent::ContentBlockDelta(text) => Some(text),
                _ => None,
            })
            .collect())
    }

    /// Counts the input tokens for a completion request using Anthropic's token-count endpoint.
    pub fn count_tokens_with_extras(
        &self,
        request: ChatCompletionRequest,
        extras: AnthropicRequestExtras,
    ) -> Result<u32, ProviderError> {
        let bearer_token = self.bearer_token()?;
        let response = self.transport.post_json(
            &self.endpoint("/v1/messages/count_tokens"),
            Some(bearer_token),
            Self::beta_header_for_extras(&extras),
            self.count_tokens_payload(&request, &extras),
        )?;
        Self::extract_input_tokens(&response)
    }

    /// Counts the input tokens for a completion request using Anthropic's token-count endpoint.
    pub fn count_tokens(&self, request: ChatCompletionRequest) -> Result<u32, ProviderError> {
        self.count_tokens_with_extras(request, AnthropicRequestExtras::default())
    }
}

impl<T> ModelProvider for AnthropicMessagesClient<T>
where
    T: AnthropicMessagesTransport,
{
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            completion: true,
            embedding: false,
            inline_prediction: false,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        AnthropicMessagesClient::complete(self, request)
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::unsupported(
            self.id.clone(),
            format!("embed:{}", request.model),
        ))
    }

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

fn normalize_base_url(value: String) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn first_configured_value<const N: usize>(values: [Option<String>; N]) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .find(|value| !value.trim().is_empty())
}

fn chat_prompt(request: &ChatCompletionRequest) -> String {
    request
        .messages
        .iter()
        .map(|message| format!("{}: {}", chat_role_label(&message.role), message.content))
        .collect::<Vec<_>>()
        .join("\n")
}

fn chat_role_label(role: &ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
    }
}

fn provider_metadata(kind: &str, base_url: &str) -> HashMap<String, String> {
    HashMap::from([
        ("provider.kind".to_string(), kind.to_string()),
        (
            "endpoint.fingerprint".to_string(),
            metadata_hash("provider-endpoint", &json!(base_url)).value,
        ),
        ("redaction".to_string(), "metadata-only".to_string()),
    ])
}

fn parse_embedding_vector(
    provider_id: &str,
    value: &Value,
    field: &str,
) -> Result<Vec<f32>, ProviderError> {
    let array =
        value
            .get(field)
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::RequestFailed {
                provider: provider_id.to_string(),
                message: format!("embedding response missing {field} vector"),
            })?;
    array
        .iter()
        .map(|entry| {
            entry
                .as_f64()
                .map(|number| number as f32)
                .ok_or_else(|| ProviderError::RequestFailed {
                    provider: provider_id.to_string(),
                    message: "embedding vector contains a non-numeric value".to_string(),
                })
        })
        .collect()
}

struct UnavailableInlineProvider {
    id: ProviderId,
    label: String,
    provider_class: AssistedAiProviderClass,
}

impl UnavailableInlineProvider {
    fn new(
        id: impl Into<ProviderId>,
        label: impl Into<String>,
        provider_class: AssistedAiProviderClass,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            provider_class,
        }
    }

    fn unavailable(&self) -> ProviderError {
        ProviderError::unavailable(
            self.id.clone(),
            format!(
                "{} inline prediction provider is not configured",
                self.label
            ),
        )
    }
}

impl ModelProvider for UnavailableInlineProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        let _ = self.provider_class;
        ProviderCapabilities {
            completion: false,
            embedding: false,
            inline_prediction: true,
        }
    }

    fn complete(
        &self,
        _request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        Err(self.unavailable())
    }

    fn embed(&self, _request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Err(self.unavailable())
    }

    fn predict_inline(
        &self,
        _request: InlinePredictionRequest,
    ) -> Result<InlinePredictionResponse, ProviderError> {
        Err(self.unavailable())
    }
}

fn provider_capability(
    provider_id: &str,
    provider_label: &str,
    provider_class: AssistedAiProviderClass,
    availability: AssistedAiProviderAvailabilityState,
) -> AssistedAiProviderCapability {
    let available = availability == AssistedAiProviderAvailabilityState::Available;
    AssistedAiProviderCapability {
        provider_id: provider_id.to_string(),
        provider_label: provider_label.to_string(),
        provider_class,
        supported_operations: vec![AssistedAiOperationClass::InlinePrediction],
        model_capability_labels: if provider_id == DETERMINISTIC_LOCAL_PROVIDER_ID {
            vec!["zeta2-style.next-edit.deterministic".to_string()]
        } else {
            vec!["inline.next-edit.slot".to_string()]
        },
        tool_capability_labels: Vec::new(),
        context_window_label: if available {
            "metadata-derived-small".to_string()
        } else {
            "unconfigured".to_string()
        },
        cost_budget_label: if available {
            "local-free".to_string()
        } else {
            "unavailable".to_string()
        },
        risk_budget_label: if available {
            "low".to_string()
        } else {
            "high-unconfigured".to_string()
        },
        privacy_retention_label: "metadata-only-default".to_string(),
        byok_support: match provider_class {
            AssistedAiProviderClass::ByokRemote => AssistedAiSupportLabel::ApprovalRequired,
            _ => AssistedAiSupportLabel::Unsupported,
        },
        local_execution_support: match provider_class {
            AssistedAiProviderClass::Local | AssistedAiProviderClass::LocalLoopback => {
                AssistedAiSupportLabel::Supported
            }
            _ => AssistedAiSupportLabel::Unsupported,
        },
        offline_support: if provider_class == AssistedAiProviderClass::Local {
            AssistedAiSupportLabel::Supported
        } else {
            AssistedAiSupportLabel::Unsupported
        },
        air_gap_support: if provider_class == AssistedAiProviderClass::Local {
            AssistedAiSupportLabel::Supported
        } else {
            AssistedAiSupportLabel::Unsupported
        },
        redaction_requirements: vec!["metadata-only".to_string()],
        consent_requirements: if available {
            vec!["not-required.local-deterministic".to_string()]
        } else {
            vec!["configuration-required".to_string()]
        },
        availability,
        refusal: (!available).then(|| unavailable_refusal(provider_id, provider_label)),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn unavailable_refusal(provider_id: &str, provider_label: &str) -> AssistedAiRefusalMetadata {
    AssistedAiRefusalMetadata {
        reason_code: "provider.not_configured".to_string(),
        label: format!("{provider_label} inline prediction provider is not configured"),
        provider_id: Some(provider_id.to_string()),
        operation_class: Some(AssistedAiOperationClass::InlinePrediction),
        privacy_scope: Some(SemanticPrivacyScope::MetadataOnly),
        capability: Some(CapabilityId("ai.inline_prediction.invoke".to_string())),
        budget_id: None,
        risk_label: ProposalRiskLabel::High,
        reasons: vec!["provider.not_configured".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

/// MCP client errors.
#[derive(Debug, Error)]
pub enum McpClientError {
    /// Registry metadata failed protocol validation.
    #[error("invalid MCP registry metadata: {0}")]
    InvalidRegistry(String),
    /// JSON-RPC envelope failed protocol validation.
    #[error("invalid MCP JSON-RPC envelope: {0}")]
    InvalidEnvelope(String),
    /// MCP list response could not be converted into registry metadata.
    #[error("invalid MCP list response: {0}")]
    InvalidListResponse(String),
    /// Tool was not found in the current registry snapshot.
    #[error("unknown MCP tool: {server_id}/{tool_name}")]
    UnknownTool {
        /// MCP server id.
        server_id: String,
        /// MCP tool name.
        tool_name: String,
    },
    /// Resource was not found in the current registry snapshot.
    #[error("unknown MCP resource: {server_id}/{uri}")]
    UnknownResource {
        /// MCP server id.
        server_id: String,
        /// MCP resource URI.
        uri: String,
    },
    /// Prompt was not found in the current registry snapshot.
    #[error("unknown MCP prompt: {server_id}/{prompt_name}")]
    UnknownPrompt {
        /// MCP server id.
        server_id: String,
        /// MCP prompt name.
        prompt_name: String,
    },
    /// Tool call did not have an approved permission token.
    #[error("MCP tool permission required: {request_id}")]
    PermissionRequired {
        /// Permission request id.
        request_id: String,
    },
    /// Transport failed.
    #[error("MCP transport error: {0}")]
    Transport(String),
}

/// MCP transport port.
pub trait McpTransport {
    /// Send a JSON-RPC envelope and return the response JSON.
    fn send(&self, envelope: &McpJsonRpcEnvelope) -> Result<Value, McpClientError>;
}

/// MCP stdio transport configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdioMcpTransportConfig {
    /// Command executable.
    pub command: String,
    /// Command arguments.
    pub args: Vec<String>,
}

/// MCP Streamable HTTP transport configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamableHttpMcpTransportConfig {
    /// MCP endpoint URL.
    pub endpoint: String,
}

/// Process-backed MCP stdio transport.
#[derive(Clone)]
pub struct StdioMcpTransport {
    config: StdioMcpTransportConfig,
    session: Arc<Mutex<Option<StdioMcpSession>>>,
}

impl fmt::Debug for StdioMcpTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StdioMcpTransport")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

struct StdioMcpSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Drop for StdioMcpSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl StdioMcpTransport {
    /// Create a stdio transport from command metadata.
    pub fn new(config: StdioMcpTransportConfig) -> Self {
        Self {
            config,
            session: Arc::new(Mutex::new(None)),
        }
    }

    fn spawn_session(&self) -> Result<StdioMcpSession, McpClientError> {
        if self.config.command.trim().is_empty() {
            return Err(McpClientError::Transport(
                "stdio MCP command must not be empty".to_string(),
            ));
        }
        let mut child = Command::new(&self.config.command)
            .args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| McpClientError::Transport(error.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpClientError::Transport("stdio MCP stdin unavailable".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpClientError::Transport("stdio MCP stdout unavailable".to_string()))?;
        Ok(StdioMcpSession {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    fn send_on_session(
        &self,
        session: &mut StdioMcpSession,
        envelope: &McpJsonRpcEnvelope,
    ) -> Result<Value, McpClientError> {
        let mut payload = serde_json::to_vec(envelope)
            .map_err(|error| McpClientError::Transport(error.to_string()))?;
        payload.push(b'\n');
        session
            .stdin
            .write_all(&payload)
            .and_then(|()| session.stdin.flush())
            .map_err(|error| McpClientError::Transport(error.to_string()))?;

        let expected_id = envelope.id.as_deref();
        let mut line = String::new();
        loop {
            line.clear();
            let byte_count = session
                .stdout
                .read_line(&mut line)
                .map_err(|error| McpClientError::Transport(error.to_string()))?;
            if byte_count == 0 {
                let status = session.child.try_wait().ok().flatten();
                return Err(McpClientError::Transport(match status {
                    Some(status) => format!("stdio MCP server exited with {status}"),
                    None => "stdio MCP server closed stdout".to_string(),
                }));
            }
            if line.trim().is_empty() {
                continue;
            }
            let response: Value = serde_json::from_str(&line)
                .map_err(|error| McpClientError::Transport(error.to_string()))?;
            if response.get("id").and_then(Value::as_str) == expected_id {
                return Ok(response);
            }
        }
    }
}

impl McpTransport for StdioMcpTransport {
    fn send(&self, envelope: &McpJsonRpcEnvelope) -> Result<Value, McpClientError> {
        validate_mcp_json_rpc_envelope(envelope)
            .map_err(|error| McpClientError::InvalidEnvelope(error.message))?;
        let mut guard = self
            .session
            .lock()
            .map_err(|_| McpClientError::Transport("stdio MCP session lock poisoned".into()))?;
        if guard.is_none() {
            *guard = Some(self.spawn_session()?);
        }
        match self.send_on_session(guard.as_mut().expect("stdio session present"), envelope) {
            Ok(response) => Ok(response),
            Err(error) => {
                *guard = None;
                Err(error)
            }
        }
    }
}

/// Blocking Streamable HTTP MCP transport.
#[derive(Debug, Clone)]
pub struct StreamableHttpMcpTransport {
    config: StreamableHttpMcpTransportConfig,
}

impl StreamableHttpMcpTransport {
    /// Create a Streamable HTTP transport from endpoint metadata.
    pub fn new(config: StreamableHttpMcpTransportConfig) -> Self {
        Self { config }
    }
}

impl McpTransport for StreamableHttpMcpTransport {
    fn send(&self, envelope: &McpJsonRpcEnvelope) -> Result<Value, McpClientError> {
        validate_mcp_json_rpc_envelope(envelope)
            .map_err(|error| McpClientError::InvalidEnvelope(error.message))?;
        if self.config.endpoint.trim().is_empty() {
            return Err(McpClientError::Transport(
                "Streamable HTTP MCP endpoint must not be empty".to_string(),
            ));
        }
        let response = reqwest::blocking::Client::new()
            .post(&self.config.endpoint)
            .json(envelope)
            .send()
            .map_err(|error| McpClientError::Transport(error.to_string()))?;
        if !response.status().is_success() {
            return Err(McpClientError::Transport(format!(
                "Streamable HTTP MCP endpoint returned {}",
                response.status()
            )));
        }
        response
            .json::<Value>()
            .map_err(|error| McpClientError::Transport(error.to_string()))
    }
}

/// Deterministic MCP client over an injected transport.
pub struct McpClient<T> {
    registry: McpRegistrySnapshot,
    transport: T,
}

impl<T> McpClient<T>
where
    T: McpTransport,
{
    /// Create a client from a validated registry and transport.
    pub fn new(registry: McpRegistrySnapshot, transport: T) -> Result<Self, McpClientError> {
        validate_mcp_registry_snapshot(&registry)
            .map_err(|error| McpClientError::InvalidRegistry(error.message))?;
        Ok(Self {
            registry,
            transport,
        })
    }

    /// Return the current registry snapshot.
    pub fn registry(&self) -> &McpRegistrySnapshot {
        &self.registry
    }

    /// Apply a list-changed notification and mark the registry stale for reload.
    pub fn apply_list_changed_notification(&mut self, kind: McpListChangedKind) {
        self.registry.last_notification_kind = Some(kind);
        self.registry.list_version = self.registry.list_version.saturating_add(1);
    }

    /// Replace the registry after a successful reload.
    pub fn replace_registry(
        &mut self,
        registry: McpRegistrySnapshot,
    ) -> Result<(), McpClientError> {
        validate_mcp_registry_snapshot(&registry)
            .map_err(|error| McpClientError::InvalidRegistry(error.message))?;
        self.registry = registry;
        Ok(())
    }

    /// Build a `tools/list` request.
    pub fn list_tools_request(&self, request_id: impl Into<String>) -> McpJsonRpcEnvelope {
        McpJsonRpcEnvelope::request(request_id, "tools/list", json!({}))
    }

    /// Build a `resources/list` request.
    pub fn list_resources_request(&self, request_id: impl Into<String>) -> McpJsonRpcEnvelope {
        McpJsonRpcEnvelope::request(request_id, "resources/list", json!({}))
    }

    /// Build a `prompts/list` request.
    pub fn list_prompts_request(&self, request_id: impl Into<String>) -> McpJsonRpcEnvelope {
        McpJsonRpcEnvelope::request(request_id, "prompts/list", json!({}))
    }

    /// Send a `tools/list` request through the transport.
    pub fn list_tools(&self, request_id: impl Into<String>) -> Result<Value, McpClientError> {
        self.transport.send(&self.list_tools_request(request_id))
    }

    /// Send a `resources/list` request through the transport.
    pub fn list_resources(&self, request_id: impl Into<String>) -> Result<Value, McpClientError> {
        self.transport
            .send(&self.list_resources_request(request_id))
    }

    /// Send a `prompts/list` request through the transport.
    pub fn list_prompts(&self, request_id: impl Into<String>) -> Result<Value, McpClientError> {
        self.transport.send(&self.list_prompts_request(request_id))
    }

    /// Reload the changed primitive list after an MCP `notifications/*/list_changed` event.
    pub fn reload_after_list_changed(
        &mut self,
        kind: McpListChangedKind,
        request_id: impl Into<String>,
        generated_at: TimestampMillis,
    ) -> Result<&McpRegistrySnapshot, McpClientError> {
        self.apply_list_changed_notification(kind);
        let request_id = request_id.into();
        let response = match kind {
            McpListChangedKind::Tools => self.list_tools(request_id)?,
            McpListChangedKind::Resources => self.list_resources(request_id)?,
            McpListChangedKind::Prompts => self.list_prompts(request_id)?,
        };

        let mut registry = self.registry.clone();
        match kind {
            McpListChangedKind::Tools => {
                registry.tools = parse_tools_list_response(&registry, &response)?;
            }
            McpListChangedKind::Resources => {
                registry.resources = parse_resources_list_response(&registry, &response)?;
            }
            McpListChangedKind::Prompts => {
                registry.prompts = parse_prompts_list_response(&registry, &response)?;
            }
        }
        registry.last_notification_kind = None;
        registry.generated_at = generated_at;
        registry.registry_id = format!(
            "mcp-registry:{}:{}",
            registry.server.server_id.0, registry.list_version
        );
        self.replace_registry(registry)?;
        Ok(&self.registry)
    }

    /// Build a `tools/call` request after validating the tool exists.
    pub fn tool_call_request(
        &self,
        request_id: impl Into<String>,
        server_id: &McpServerId,
        tool_name: &McpToolName,
        arguments: Value,
    ) -> Result<McpJsonRpcEnvelope, McpClientError> {
        self.ensure_tool(server_id, tool_name)?;
        Ok(McpJsonRpcEnvelope::request(
            request_id,
            "tools/call",
            json!({
                "name": tool_name.0,
                "arguments": arguments,
            }),
        ))
    }

    /// Build a `resources/read` request after validating the resource exists.
    pub fn resource_read_request(
        &self,
        request_id: impl Into<String>,
        server_id: &McpServerId,
        uri: &McpResourceUri,
    ) -> Result<McpJsonRpcEnvelope, McpClientError> {
        self.ensure_resource(server_id, uri)?;
        Ok(McpJsonRpcEnvelope::request(
            request_id,
            "resources/read",
            json!({ "uri": uri.0 }),
        ))
    }

    /// Build a `prompts/get` request after validating the prompt exists.
    pub fn prompt_get_request(
        &self,
        request_id: impl Into<String>,
        server_id: &McpServerId,
        prompt_name: &McpPromptName,
        arguments: Value,
    ) -> Result<McpJsonRpcEnvelope, McpClientError> {
        self.ensure_prompt(server_id, prompt_name)?;
        Ok(McpJsonRpcEnvelope::request(
            request_id,
            "prompts/get",
            json!({
                "name": prompt_name.0,
                "arguments": arguments,
            }),
        ))
    }

    /// Invoke an MCP tool only when an app-owned permission token allows runtime use.
    pub fn call_tool_with_permission(
        &self,
        request_id: impl Into<String>,
        server_id: &McpServerId,
        tool_name: &McpToolName,
        arguments: Value,
        permission: &DelegatedTaskToolPermissionRequest,
    ) -> Result<Value, McpClientError> {
        let tool = self.find_tool(server_id, tool_name)?;
        if !mcp_tool_permission_allows_runtime(permission)
            || !permission_matches_mcp_tool(permission, tool)
        {
            return Err(McpClientError::PermissionRequired {
                request_id: permission.request_id.clone(),
            });
        }
        let request = self.tool_call_request(request_id, server_id, tool_name, arguments)?;
        self.transport.send(&request)
    }

    fn find_tool(
        &self,
        server_id: &McpServerId,
        tool_name: &McpToolName,
    ) -> Result<&McpToolDescriptor, McpClientError> {
        self.registry
            .tools
            .iter()
            .find(|tool| &tool.server_id == server_id && &tool.name == tool_name)
            .ok_or_else(|| McpClientError::UnknownTool {
                server_id: server_id.0.clone(),
                tool_name: tool_name.0.clone(),
            })
    }

    fn ensure_tool(
        &self,
        server_id: &McpServerId,
        tool_name: &McpToolName,
    ) -> Result<(), McpClientError> {
        self.find_tool(server_id, tool_name).map(|_| ())
    }

    fn ensure_resource(
        &self,
        server_id: &McpServerId,
        uri: &McpResourceUri,
    ) -> Result<(), McpClientError> {
        if self
            .registry
            .resources
            .iter()
            .any(|resource| &resource.server_id == server_id && &resource.uri == uri)
        {
            Ok(())
        } else {
            Err(McpClientError::UnknownResource {
                server_id: server_id.0.clone(),
                uri: uri.0.clone(),
            })
        }
    }

    fn ensure_prompt(
        &self,
        server_id: &McpServerId,
        prompt_name: &McpPromptName,
    ) -> Result<(), McpClientError> {
        if self
            .registry
            .prompts
            .iter()
            .any(|prompt| &prompt.server_id == server_id && &prompt.name == prompt_name)
        {
            Ok(())
        } else {
            Err(McpClientError::UnknownPrompt {
                server_id: server_id.0.clone(),
                prompt_name: prompt_name.0.clone(),
            })
        }
    }
}

fn permission_matches_mcp_tool(
    permission: &DelegatedTaskToolPermissionRequest,
    tool: &McpToolDescriptor,
) -> bool {
    let expected_target_id = format!("mcp-tool:{}|{}", tool.server_id.0, tool.name.0);
    permission.target_id.as_deref() == Some(expected_target_id.as_str())
        && permission.capability.as_ref() == Some(&tool.capability)
}

fn parse_tools_list_response(
    registry: &McpRegistrySnapshot,
    response: &Value,
) -> Result<Vec<McpToolDescriptor>, McpClientError> {
    let tools = response
        .get("result")
        .and_then(|result| result.get("tools"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            McpClientError::InvalidListResponse("tools/list response missing result.tools".into())
        })?;

    tools
        .iter()
        .map(|tool| {
            let name = required_string(tool, "name")?;
            let existing = registry
                .tools
                .iter()
                .find(|existing| existing.name.0 == name);
            let schema = tool
                .get("inputSchema")
                .or_else(|| tool.get("input_schema"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            Ok(McpToolDescriptor {
                server_id: registry.server.server_id.clone(),
                name: McpToolName(name.clone()),
                description_label: display_string(tool, "description", &name),
                input_schema_hash: metadata_hash("mcp-input-schema", &schema),
                risk_label: existing
                    .map(|existing| existing.risk_label)
                    .unwrap_or(ProposalRiskLabel::Unknown),
                required_permission_profile: existing
                    .map(|existing| existing.required_permission_profile)
                    .unwrap_or(DelegatedTaskToolPermissionProfile::Write),
                action_class: existing
                    .map(|existing| existing.action_class)
                    .unwrap_or(PermissionBudgetActionClass::InvokeLocalTool),
                capability: existing
                    .map(|existing| existing.capability.clone())
                    .unwrap_or_else(|| CapabilityId("mcp.tool.call".to_string())),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: existing
                    .map(|existing| existing.schema_version)
                    .unwrap_or(1),
            })
        })
        .collect()
}

fn parse_resources_list_response(
    registry: &McpRegistrySnapshot,
    response: &Value,
) -> Result<Vec<McpResourceDescriptor>, McpClientError> {
    let resources = response
        .get("result")
        .and_then(|result| result.get("resources"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            McpClientError::InvalidListResponse(
                "resources/list response missing result.resources".into(),
            )
        })?;

    resources
        .iter()
        .map(|resource| {
            let uri = required_string(resource, "uri")?;
            let existing = registry
                .resources
                .iter()
                .find(|existing| existing.uri.0 == uri);
            Ok(McpResourceDescriptor {
                server_id: registry.server.server_id.clone(),
                uri: McpResourceUri(uri.clone()),
                name_label: display_string(resource, "name", &uri),
                mime_type_label: display_string(resource, "mimeType", "application/octet-stream"),
                subscribable: resource
                    .get("subscribable")
                    .and_then(Value::as_bool)
                    .or_else(|| existing.map(|existing| existing.subscribable))
                    .unwrap_or(false),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: existing
                    .map(|existing| existing.schema_version)
                    .unwrap_or(1),
            })
        })
        .collect()
}

fn parse_prompts_list_response(
    registry: &McpRegistrySnapshot,
    response: &Value,
) -> Result<Vec<McpPromptDescriptor>, McpClientError> {
    let prompts = response
        .get("result")
        .and_then(|result| result.get("prompts"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            McpClientError::InvalidListResponse(
                "prompts/list response missing result.prompts".into(),
            )
        })?;

    prompts
        .iter()
        .map(|prompt| {
            let name = required_string(prompt, "name")?;
            let arguments = prompt
                .get("arguments")
                .and_then(Value::as_array)
                .map(|arguments| {
                    arguments
                        .iter()
                        .filter_map(|argument| argument.get("name").and_then(Value::as_str))
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let existing = registry
                .prompts
                .iter()
                .find(|existing| existing.name.0 == name);
            Ok(McpPromptDescriptor {
                server_id: registry.server.server_id.clone(),
                name: McpPromptName(name.clone()),
                description_label: display_string(prompt, "description", &name),
                argument_labels: if arguments.is_empty() {
                    existing
                        .map(|existing| existing.argument_labels.clone())
                        .unwrap_or_default()
                } else {
                    arguments
                },
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: existing
                    .map(|existing| existing.schema_version)
                    .unwrap_or(1),
            })
        })
        .collect()
}

fn required_string(value: &Value, field: &str) -> Result<String, McpClientError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            McpClientError::InvalidListResponse(format!("list item missing non-empty {field}"))
        })
}

fn display_string(value: &Value, field: &str, fallback: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn metadata_hash(algorithm: &str, value: &Value) -> FileFingerprint {
    let mut hasher = DefaultHasher::new();
    value.to_string().hash(&mut hasher);
    FileFingerprint {
        algorithm: algorithm.to_string(),
        value: format!("{:016x}", hasher.finish()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_ai::ChatMessage;
    use legion_ai::InlinePredictionRequest;
    use legion_protocol::{
        AssistedAiOperationClass, AssistedAiProviderClass, AssistedAiProviderInvocationState,
        AssistedAiSupportLabel, BufferId, BufferVersion, CancellationTokenId, CapabilityId,
        CausalityId, CorrelationId, DelegatedTaskToolPermissionDecision,
        DelegatedTaskToolPermissionProfile, DelegatedTaskToolPermissionRequestInput, EventSequence,
        FileContentVersion, FileFingerprint, FileId, InlinePredictionFingerprintMetadata,
        InlinePredictionLatencyMetadata, InlinePredictionProviderMetadata,
        InlinePredictionRequestId, InlinePredictionRequestMetadata, InlinePredictionTriggerKind,
        LanguageId, McpPromptDescriptor, McpResourceDescriptor, McpServerDescriptor,
        McpToolDescriptor, McpTransportKind, PermissionBudgetActionClass, ProposalRiskLabel,
        RedactionHint, SnapshotId, TextCoordinate, TimestampMillis, WorkspaceGeneration,
        WorkspaceId, WorkspaceTrustState, delegated_task_tool_permission_request,
    };
    fn test_inline_prediction_request(
        max_prediction_bytes: u32,
        provider_id: &str,
    ) -> InlinePredictionRequest {
        InlinePredictionRequest {
            provider: provider_id.to_string(),
            model: "inline-test".to_string(),
            metadata: InlinePredictionRequestMetadata {
                request_id: InlinePredictionRequestId(format!("inline:req:{provider_id}")),
                workspace_id: WorkspaceId(11),
                buffer_id: BufferId(22),
                file_id: Some(FileId(33)),
                language_id: LanguageId("rust".to_string()),
                cursor: TextCoordinate {
                    line: 3,
                    character: 4,
                    byte_offset: Some(80),
                    utf16_offset: Some(80),
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
                    provider_id: provider_id.to_string(),
                    model_label: "inline-test".to_string(),
                    provider_class: AssistedAiProviderClass::Local,
                    operation_class: AssistedAiOperationClass::InlinePrediction,
                    invocation_state: AssistedAiProviderInvocationState::Planned,
                    latency: InlinePredictionLatencyMetadata {
                        queued_ms: 0,
                        inference_ms: 0,
                        total_ms: 0,
                        timed_out: false,
                    },
                    health_labels: vec!["test".to_string()],
                    cost_labels: vec!["local".to_string()],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                },
                max_prediction_bytes,
                timeout_ms: 100,
                requested_at: TimestampMillis(2000),
                cancellation_token: CancellationTokenId(
                    "55555555-5555-5555-5555-555555555555".parse().unwrap(),
                ),
                required_capability: CapabilityId("ai.inline_prediction.invoke".to_string()),
                principal_id: legion_protocol::PrincipalId("principal".to_string()),
                workspace_trust_state: WorkspaceTrustState::Trusted,
                correlation_id: CorrelationId(7),
                causality_id: CausalityId("66666666-6666-6666-6666-666666666666".parse().unwrap()),
                event_sequence: EventSequence(3),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
        }
    }

    #[test]
    fn configured_provider_value_prefers_legion_then_legacy_then_standard_names() {
        assert_eq!(
            first_configured_value([
                Some("legion".to_string()),
                Some("legacy".to_string()),
                Some("standard".to_string())
            ]),
            Some("legion".to_string())
        );
        assert_eq!(
            first_configured_value([
                Some("   ".to_string()),
                Some("legacy".to_string()),
                Some("standard".to_string())
            ]),
            Some("legacy".to_string())
        );
        assert_eq!(
            first_configured_value([None, None, Some("standard".to_string())]),
            Some("standard".to_string())
        );
    }

    #[test]
    fn deterministic_local_provider_completes_without_cloud_credentials() {
        let provider = DeterministicLocalProvider::new(DETERMINISTIC_LOCAL_PROVIDER_ID);

        let response = provider
            .complete(ChatCompletionRequest::new(
                DETERMINISTIC_LOCAL_PROVIDER_ID,
                "local-test",
                "metadata-only request",
            ))
            .expect("deterministic local completion succeeds");

        assert_eq!(response.provider, DETERMINISTIC_LOCAL_PROVIDER_ID);
        assert_eq!(response.model, "local-test");
        assert_eq!(
            response.metadata.get("redaction"),
            Some(&"metadata-only".to_string())
        );
        assert!(!response.metadata.contains_key("token"));
        assert!(!response.metadata.contains_key("api_key"));
    }

    #[test]
    fn deterministic_local_provider_returns_deterministic_embedding_vectors() {
        let provider = DeterministicLocalProvider::new(DETERMINISTIC_LOCAL_PROVIDER_ID);

        let first = provider
            .embed(EmbeddingRequest::new(
                DETERMINISTIC_LOCAL_PROVIDER_ID,
                "local-embedding",
                "input token",
            ))
            .expect("deterministic local embeddings are available");
        let second = provider
            .embed(EmbeddingRequest::new(
                DETERMINISTIC_LOCAL_PROVIDER_ID,
                "local-embedding",
                "input token",
            ))
            .expect("deterministic local embeddings are stable");

        assert_eq!(first.provider, DETERMINISTIC_LOCAL_PROVIDER_ID);
        assert_eq!(first.model, "local-embedding");
        assert_eq!(first.vectors.len(), 1);
        assert_eq!(first.vectors[0].len(), 16);
        assert_eq!(first.vectors, second.vectors);
        assert_eq!(
            first.metadata.get("redaction"),
            Some(&"metadata-only".to_string())
        );
        assert!(first.vectors[0].iter().any(|value| *value != 0.0));
    }

    #[derive(Debug, Clone, Default)]
    struct RecordingProviderTransport {
        calls: std::sync::Arc<std::sync::Mutex<Vec<RecordedProviderCall>>>,
    }

    #[derive(Debug, Clone)]
    struct RecordedProviderCall {
        endpoint: String,
        bearer_token: Option<String>,
        anthropic_version: Option<String>,
        anthropic_beta: Option<String>,
        payload: Value,
    }

    impl RecordingProviderTransport {
        fn calls(&self) -> Vec<RecordedProviderCall> {
            self.calls.lock().expect("calls lock").clone()
        }
    }

    impl ProviderHttpTransport for RecordingProviderTransport {
        fn post_json(
            &self,
            endpoint: &str,
            bearer_token: Option<&str>,
            payload: Value,
        ) -> Result<Value, ProviderError> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(RecordedProviderCall {
                    endpoint: endpoint.to_string(),
                    bearer_token: bearer_token.map(str::to_string),
                    anthropic_version: None,
                    anthropic_beta: None,
                    payload: payload.clone(),
                });
            if endpoint.ends_with("/api/generate") {
                Ok(json!({ "response": "ollama answer" }))
            } else if endpoint.ends_with("/api/embeddings") {
                Ok(json!({ "embedding": [0.25, 0.75] }))
            } else if endpoint.ends_with("/chat/completions") {
                Ok(json!({
                    "choices": [
                        { "message": { "content": "openai-compatible answer" } }
                    ]
                }))
            } else if endpoint.ends_with("/v1/messages/count_tokens") {
                Ok(json!({ "input_tokens": 73 }))
            } else if endpoint.ends_with("/v1/messages") {
                Ok(json!({
                    "content": [
                        { "type": "text", "text": "anthropic answer" }
                    ],
                    "usage": { "input_tokens": 17, "output_tokens": 5 }
                }))
            } else if endpoint.ends_with("/embeddings") {
                Ok(json!({
                    "data": [
                        { "embedding": [0.125, 0.875] }
                    ]
                }))
            } else {
                Err(ProviderError::RequestFailed {
                    provider: "recording".to_string(),
                    message: format!("unexpected endpoint {endpoint}"),
                })
            }
        }
    }

    impl AnthropicMessagesTransport for RecordingProviderTransport {
        fn post_json(
            &self,
            endpoint: &str,
            bearer_token: Option<&str>,
            beta_header: Option<&str>,
            payload: Value,
        ) -> Result<Value, ProviderError> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(RecordedProviderCall {
                    endpoint: endpoint.to_string(),
                    bearer_token: bearer_token.map(str::to_string),
                    anthropic_version: Some(ANTHROPIC_API_VERSION.to_string()),
                    anthropic_beta: beta_header.map(str::to_string),
                    payload: payload.clone(),
                });
            if endpoint.ends_with("/v1/messages/count_tokens") {
                Ok(json!({ "input_tokens": 73 }))
            } else if endpoint.ends_with("/v1/messages") {
                Ok(json!({
                    "content": [
                        { "type": "text", "text": "anthropic answer" }
                    ],
                    "usage": { "input_tokens": 17, "output_tokens": 5 }
                }))
            } else {
                Err(ProviderError::RequestFailed {
                    provider: "recording".to_string(),
                    message: format!("unexpected anthropic endpoint {endpoint}"),
                })
            }
        }

        fn post_text(
            &self,
            endpoint: &str,
            bearer_token: Option<&str>,
            beta_header: Option<&str>,
            payload: Value,
        ) -> Result<String, ProviderError> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(RecordedProviderCall {
                    endpoint: endpoint.to_string(),
                    bearer_token: bearer_token.map(str::to_string),
                    anthropic_version: Some(ANTHROPIC_API_VERSION.to_string()),
                    anthropic_beta: beta_header.map(str::to_string),
                    payload: payload.clone(),
                });
            if endpoint.ends_with("/v1/messages") {
                Ok(concat!(
                    "event: message_start\n",
                    "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-opus-4-8\"}}\n\n",
                    "event: content_block_start\n",
                    "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
                    "event: content_block_delta\n",
                    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
                    "event: content_block_delta\n",
                    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\n",
                    "event: message_delta\n",
                    "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"usage\":{\"input_tokens\":17,\"output_tokens\":5}}}\n\n",
                    "event: message_stop\n",
                    "data: {\"type\":\"message_stop\"}\n"
                )
                .to_string())
            } else {
                Err(ProviderError::RequestFailed {
                    provider: "recording".to_string(),
                    message: format!("unexpected text endpoint {endpoint}"),
                })
            }
        }
    }

    #[test]
    fn ollama_provider_posts_completion_and_embedding_requests() {
        let transport = RecordingProviderTransport::default();
        let provider = OllamaProvider::with_transport(
            OLLAMA_PROVIDER_ID,
            "http://localhost:11434/",
            transport.clone(),
        );

        let completion = provider
            .complete(
                ChatCompletionRequest::new(OLLAMA_PROVIDER_ID, "llama-test", "explain")
                    .with_temperature(0.2),
            )
            .expect("ollama completion parses");
        let embeddings = provider
            .embed(EmbeddingRequest::new(
                OLLAMA_PROVIDER_ID,
                "nomic-embed-text",
                "embed me",
            ))
            .expect("ollama embedding parses");

        assert_eq!(completion.text, "ollama answer");
        assert_eq!(
            completion.metadata.get("redaction"),
            Some(&"metadata-only".to_string())
        );
        assert_eq!(embeddings.vectors, vec![vec![0.25, 0.75]]);
        let calls = transport.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].endpoint, "http://localhost:11434/api/generate");
        assert_eq!(calls[0].bearer_token, None);
        assert_eq!(calls[0].payload["model"], "llama-test");
        assert_eq!(calls[0].payload["stream"], false);
        assert_eq!(calls[1].endpoint, "http://localhost:11434/api/embeddings");
        assert_eq!(calls[1].payload["prompt"], "embed me");
    }

    #[test]
    fn openai_compatible_provider_posts_byok_completion_and_embedding_requests() {
        let transport = RecordingProviderTransport::default();
        let provider = OpenAiCompatibleProvider::with_transport(
            OPENAI_COMPATIBLE_PROVIDER_ID,
            "https://provider.example/v1/",
            Some("test-key".to_string()),
            transport.clone(),
        );

        let completion = provider
            .complete(
                ChatCompletionRequest::new(OPENAI_COMPATIBLE_PROVIDER_ID, "gpt-test", "explain")
                    .with_max_tokens(32),
            )
            .expect("OpenAI-compatible completion parses");
        let embeddings = provider
            .embed(EmbeddingRequest::new(
                OPENAI_COMPATIBLE_PROVIDER_ID,
                "text-embedding-test",
                "embed me",
            ))
            .expect("OpenAI-compatible embedding parses");

        assert_eq!(completion.text, "openai-compatible answer");
        assert_eq!(
            completion.metadata.get("redaction"),
            Some(&"metadata-only".to_string())
        );
        assert!(
            !completion
                .metadata
                .values()
                .any(|value| value == "test-key")
        );
        assert_eq!(embeddings.vectors, vec![vec![0.125, 0.875]]);
        let calls = transport.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0].endpoint,
            "https://provider.example/v1/chat/completions"
        );
        assert_eq!(calls[0].bearer_token, Some("test-key".to_string()));
        assert_eq!(calls[0].payload["messages"][0]["role"], "user");
        assert_eq!(calls[1].endpoint, "https://provider.example/v1/embeddings");
        assert_eq!(calls[1].payload["input"][0], "embed me");
    }

    #[test]
    fn llama_cpp_provider_posts_loopback_requests_without_bearer_by_default() {
        let transport = RecordingProviderTransport::default();
        let provider = LlamaCppProvider::with_transport(
            LLAMA_CPP_PROVIDER_ID,
            "http://localhost:8080/v1/",
            None,
            transport.clone(),
        );

        let completion = provider
            .complete(
                ChatCompletionRequest::new(LLAMA_CPP_PROVIDER_ID, "local-gguf", "explain")
                    .with_max_tokens(24),
            )
            .expect("llama.cpp completion parses");
        let embeddings = provider
            .embed(EmbeddingRequest::new(
                LLAMA_CPP_PROVIDER_ID,
                "local-embedding-gguf",
                "embed me",
            ))
            .expect("llama.cpp embedding parses");

        assert_eq!(completion.provider, LLAMA_CPP_PROVIDER_ID);
        assert_eq!(completion.text, "openai-compatible answer");
        assert_eq!(
            completion.metadata.get("provider.kind"),
            Some(&"llama-cpp".to_string())
        );
        assert_eq!(
            completion.metadata.get("redaction"),
            Some(&"metadata-only".to_string())
        );
        assert_eq!(embeddings.vectors, vec![vec![0.125, 0.875]]);
        let calls = transport.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0].endpoint,
            "http://localhost:8080/v1/chat/completions"
        );
        assert_eq!(calls[0].bearer_token, None);
        assert_eq!(calls[0].payload["messages"][0]["role"], "user");
        assert_eq!(calls[0].payload["max_tokens"], 24);
        assert_eq!(calls[1].endpoint, "http://localhost:8080/v1/embeddings");
        assert_eq!(calls[1].bearer_token, None);
        assert_eq!(calls[1].payload["input"][0], "embed me");
    }

    #[test]
    fn llama_cpp_provider_can_attach_optional_local_bearer_token() {
        let transport = RecordingProviderTransport::default();
        let provider = LlamaCppProvider::with_transport(
            LLAMA_CPP_PROVIDER_ID,
            "http://127.0.0.1:8080/v1",
            Some("local-token".to_string()),
            transport.clone(),
        );

        provider
            .complete(ChatCompletionRequest::new(
                LLAMA_CPP_PROVIDER_ID,
                "local-gguf",
                "explain",
            ))
            .expect("llama.cpp completion parses");

        let calls = transport.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].bearer_token, Some("local-token".to_string()));
    }

    #[test]
    fn provider_registry_exposes_configured_adapters() {
        let registry = make_provider_registry();
        let mut ids = registry.provider_ids();
        ids.sort();

        assert_eq!(
            ids,
            vec![
                ANTHROPIC_PROVIDER_ID.to_string(),
                DETERMINISTIC_LOCAL_PROVIDER_ID.to_string(),
                LLAMA_CPP_PROVIDER_ID.to_string(),
                OLLAMA_PROVIDER_ID.to_string(),
                OPENAI_COMPATIBLE_PROVIDER_ID.to_string(),
            ]
        );
    }

    #[test]
    fn inline_prediction_registry_exposes_required_provider_slots() {
        let registry = make_inline_prediction_registry();
        let mut ids = registry.provider_ids();
        ids.sort();

        assert_eq!(
            ids,
            vec![
                ANTHROPIC_PROVIDER_ID.to_string(),
                CODESTRAL_PROVIDER_ID.to_string(),
                COPILOT_NES_PROVIDER_ID.to_string(),
                DETERMINISTIC_LOCAL_PROVIDER_ID.to_string(),
                LLAMA_CPP_PROVIDER_ID.to_string(),
                MERCURY_PROVIDER_ID.to_string(),
                OLLAMA_PROVIDER_ID.to_string(),
                OPENAI_COMPATIBLE_PROVIDER_ID.to_string(),
            ]
        );

        let capabilities = inline_prediction_provider_capabilities();
        assert_eq!(capabilities.len(), 7);
        let deterministic = capabilities
            .iter()
            .find(|capability| capability.provider_id == DETERMINISTIC_LOCAL_PROVIDER_ID)
            .expect("deterministic local capability is present");
        assert_eq!(
            deterministic.availability,
            legion_protocol::AssistedAiProviderAvailabilityState::Available
        );
        assert!(
            deterministic
                .supported_operations
                .contains(&legion_protocol::AssistedAiOperationClass::InlinePrediction)
        );
        let llama_cpp = capabilities
            .iter()
            .find(|capability| capability.provider_id == LLAMA_CPP_PROVIDER_ID)
            .expect("llama.cpp capability is present");
        assert_eq!(
            llama_cpp.provider_class,
            AssistedAiProviderClass::LocalLoopback
        );
        assert_eq!(
            llama_cpp.local_execution_support,
            AssistedAiSupportLabel::Supported
        );
        assert_eq!(llama_cpp.byok_support, AssistedAiSupportLabel::Unsupported);

        for capability in capabilities
            .iter()
            .filter(|capability| capability.provider_id != DETERMINISTIC_LOCAL_PROVIDER_ID)
        {
            assert_eq!(
                capability.availability,
                legion_protocol::AssistedAiProviderAvailabilityState::Unavailable
            );
            assert!(
                capability.refusal.is_some(),
                "{} must explain why it is unavailable",
                capability.provider_id
            );
        }
    }

    #[test]
    fn deterministic_local_provider_predicts_bounded_inline_result() {
        let provider = DeterministicLocalProvider::new(DETERMINISTIC_LOCAL_PROVIDER_ID);
        let response = provider
            .predict_inline(test_inline_prediction_request(
                16,
                DETERMINISTIC_LOCAL_PROVIDER_ID,
            ))
            .expect("deterministic inline provider succeeds");

        assert_eq!(response.provider, DETERMINISTIC_LOCAL_PROVIDER_ID);
        assert_eq!(
            response.result.provider.operation_class,
            legion_protocol::AssistedAiOperationClass::InlinePrediction
        );
        let ghost_text = response.result.ghost_text.as_ref().expect("ghost text");
        assert!(ghost_text.byte_len <= 16);
        legion_protocol::validate_inline_prediction_result(&response.result)
            .expect("deterministic result satisfies protocol validator");
    }

    #[test]
    fn anthropic_messages_client_posts_native_messages_completion_count_tokens_and_streaming_requests()
     {
        let transport = RecordingProviderTransport::default();
        let provider = AnthropicMessagesClient::with_transport(
            ANTHROPIC_PROVIDER_ID,
            "https://api.anthropic.com/",
            Some("anthropic-key".to_string()),
            transport.clone(),
        );
        let request = ChatCompletionRequest {
            provider: ANTHROPIC_PROVIDER_ID.to_string(),
            model: "claude-opus-4-8".to_string(),
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: "system prompt".to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: "write a haiku".to_string(),
                },
            ],
            max_tokens: Some(42),
            temperature: Some(0.2),
            metadata: std::collections::HashMap::from([(
                "conversation_id".to_string(),
                "anthropic-fixture".to_string(),
            )]),
        };
        let extras = AnthropicRequestExtras {
            tools: vec![anthropic_strict_tool_definition(
                "lookup_docs",
                "Lookup docs",
                json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
            )],
            output_config: Some(anthropic_json_schema_output_config(
                "final_answer",
                json!({
                    "type": "object",
                    "properties": {
                        "answer": { "type": "string" }
                    },
                    "required": ["answer"],
                    "additionalProperties": false
                }),
            )),
            thinking: Some(json!({
                "type": "enabled",
                "budget_tokens": 64
            })),
        };

        let completion = provider
            .complete_with_extras(request.clone(), extras.clone())
            .expect("anthropic completion parses");
        let input_tokens = provider
            .count_tokens_with_extras(request.clone(), extras.clone())
            .expect("anthropic count_tokens parses");
        let deltas = provider
            .stream_text_deltas_with_extras(request.clone(), extras)
            .expect("anthropic streaming parses");

        assert_eq!(completion.provider, ANTHROPIC_PROVIDER_ID);
        assert_eq!(completion.text, "anthropic answer");
        assert_eq!(
            completion.metadata.get("provider.kind"),
            Some(&"anthropic".to_string())
        );
        assert_eq!(input_tokens, 73);
        assert_eq!(deltas, vec!["Hello".to_string(), " world".to_string()]);

        let calls = transport.calls();
        assert_eq!(calls.len(), 3);

        assert_eq!(calls[0].endpoint, "https://api.anthropic.com/v1/messages");
        assert_eq!(calls[0].bearer_token, Some("anthropic-key".to_string()));
        assert_eq!(
            calls[0].anthropic_version,
            Some(ANTHROPIC_API_VERSION.to_string())
        );
        assert_eq!(
            calls[0].anthropic_beta,
            Some(ANTHROPIC_STRUCTURED_OUTPUTS_BETA.to_string())
        );
        assert_eq!(calls[0].payload["system"], "system prompt");
        assert_eq!(calls[0].payload["messages"][0]["role"], "user");
        assert_eq!(
            calls[0].payload["messages"][0]["content"][0]["type"],
            "text"
        );
        assert_eq!(
            calls[0].payload["messages"][0]["content"][0]["text"],
            "write a haiku"
        );
        assert_eq!(calls[0].payload["max_tokens"], 42);
        assert_eq!(calls[0].payload["stream"], false);
        assert_eq!(
            calls[0].payload["output_config"]["format"]["type"],
            "json_schema"
        );
        assert_eq!(calls[0].payload["tools"][0]["strict"], true);
        assert_eq!(calls[0].payload["thinking"]["budget_tokens"], 64);

        assert_eq!(
            calls[1].endpoint,
            "https://api.anthropic.com/v1/messages/count_tokens"
        );
        assert_eq!(calls[1].bearer_token, Some("anthropic-key".to_string()));
        assert_eq!(
            calls[1].anthropic_version,
            Some(ANTHROPIC_API_VERSION.to_string())
        );
        assert_eq!(
            calls[1].anthropic_beta,
            Some(ANTHROPIC_STRUCTURED_OUTPUTS_BETA.to_string())
        );
        assert_eq!(calls[1].payload["system"], "system prompt");
        assert!(calls[1].payload.get("max_tokens").is_none());
        assert_eq!(calls[1].payload["messages"][0]["role"], "user");
        assert_eq!(calls[1].payload["tools"][0]["strict"], true);

        assert_eq!(calls[2].endpoint, "https://api.anthropic.com/v1/messages");
        assert_eq!(calls[2].bearer_token, Some("anthropic-key".to_string()));
        assert_eq!(
            calls[2].anthropic_version,
            Some(ANTHROPIC_API_VERSION.to_string())
        );
        assert_eq!(
            calls[2].anthropic_beta,
            Some(ANTHROPIC_STRUCTURED_OUTPUTS_BETA.to_string())
        );
        assert_eq!(calls[2].payload["stream"], true);
        assert_eq!(
            calls[2].payload["output_config"]["format"]["name"],
            "final_answer"
        );
        assert_eq!(calls[2].payload["tools"][0]["name"], "lookup_docs");
    }

    #[test]
    #[ignore]
    fn anthropic_messages_client_live_smoke_round_trip() {
        if std::env::var("ANTHROPIC_API_KEY").is_err()
            && std::env::var("ANTHROPIC_AUTH_TOKEN").is_err()
            && std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_API_KEY")).is_err()
            && std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_AUTH_TOKEN")).is_err()
        {
            eprintln!(
                "skipping Anthropic live smoke: no API key or auth token in the test environment"
            );
            return;
        }

        let provider = AnthropicMessagesClient::from_env(ANTHROPIC_PROVIDER_ID);
        let model = std::env::var("ANTHROPIC_LIVE_MODEL")
            .unwrap_or_else(|_| "claude-3-haiku-20240307".to_string());
        let request = ChatCompletionRequest::new(
            ANTHROPIC_PROVIDER_ID,
            model,
            "Reply with one short sentence.",
        )
        .with_max_tokens(1);

        let tokens = provider
            .count_tokens(request.clone())
            .expect("live Anthropic token count");
        let response = provider
            .complete(request)
            .expect("live Anthropic completion");

        assert!(tokens > 0, "live token count must be non-zero");
        assert!(
            !response.text.trim().is_empty(),
            "live completion must produce text"
        );
    }

    #[test]
    fn unconfigured_external_provider_slots_refuse_inline_prediction_explicitly() {
        let registry = make_inline_prediction_registry();

        for provider_id in [
            OLLAMA_PROVIDER_ID,
            LLAMA_CPP_PROVIDER_ID,
            OPENAI_COMPATIBLE_PROVIDER_ID,
            COPILOT_NES_PROVIDER_ID,
            MERCURY_PROVIDER_ID,
            CODESTRAL_PROVIDER_ID,
        ] {
            let provider = registry.get(provider_id).expect("provider slot exists");
            let error = provider
                .predict_inline(test_inline_prediction_request(32, provider_id))
                .expect_err("unconfigured provider must refuse explicitly");

            assert!(matches!(
                error,
                ProviderError::ProviderUnavailable { provider, reason }
                    if provider == provider_id && reason.contains("not configured")
            ));
        }
    }

    #[derive(Debug, Clone)]
    struct MemoryMcpTransport;

    impl McpTransport for MemoryMcpTransport {
        fn send(&self, envelope: &McpJsonRpcEnvelope) -> Result<Value, McpClientError> {
            validate_mcp_json_rpc_envelope(envelope)
                .map_err(|error| McpClientError::InvalidEnvelope(error.message))?;
            Ok(json!({
                "jsonrpc": "2.0",
                "id": envelope.id,
                "result": {
                    "method": envelope.method,
                    "payload_class": "metadata_only"
                }
            }))
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct ReloadMcpTransport;

    impl McpTransport for ReloadMcpTransport {
        fn send(&self, envelope: &McpJsonRpcEnvelope) -> Result<Value, McpClientError> {
            validate_mcp_json_rpc_envelope(envelope)
                .map_err(|error| McpClientError::InvalidEnvelope(error.message))?;
            match envelope.method.as_str() {
                "tools/list" => Ok(json!({
                    "jsonrpc": "2.0",
                    "id": envelope.id,
                    "result": {
                        "tools": [
                            {
                                "name": "write_file",
                                "description": "write file after reload",
                                "inputSchema": { "type": "object" }
                            },
                            {
                                "name": "read_metadata",
                                "description": "read metadata",
                                "inputSchema": { "type": "object" }
                            }
                        ]
                    }
                })),
                "resources/list" => Ok(json!({
                    "jsonrpc": "2.0",
                    "id": envelope.id,
                    "result": {
                        "resources": [
                            {
                                "uri": "workspace://metadata",
                                "name": "workspace metadata",
                                "mimeType": "application/json",
                                "subscribable": true
                            }
                        ]
                    }
                })),
                "prompts/list" => Ok(json!({
                    "jsonrpc": "2.0",
                    "id": envelope.id,
                    "result": {
                        "prompts": [
                            {
                                "name": "review",
                                "description": "review prompt",
                                "arguments": [{ "name": "scope" }]
                            }
                        ]
                    }
                })),
                _ => Err(McpClientError::Transport(format!(
                    "unexpected method {}",
                    envelope.method
                ))),
            }
        }
    }

    fn mcp_registry() -> McpRegistrySnapshot {
        let server_id = McpServerId("mcp:test".to_string());
        McpRegistrySnapshot {
            registry_id: "mcp-registry:test:1".to_string(),
            server: McpServerDescriptor {
                server_id: server_id.clone(),
                transport_kind: McpTransportKind::StreamableHttp,
                display_label: "Test MCP".to_string(),
                endpoint_label: "https://mcp.invalid".to_string(),
                tools_list_changed: true,
                resources_list_changed: true,
                prompts_list_changed: true,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            tools: vec![McpToolDescriptor {
                server_id: server_id.clone(),
                name: McpToolName("write_file".to_string()),
                description_label: "write file".to_string(),
                input_schema_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "schema".to_string(),
                },
                risk_label: ProposalRiskLabel::High,
                required_permission_profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::InvokeLocalTool,
                capability: CapabilityId("mcp.tool.call".to_string()),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            resources: vec![McpResourceDescriptor {
                server_id: server_id.clone(),
                uri: McpResourceUri("workspace://metadata".to_string()),
                name_label: "workspace metadata".to_string(),
                mime_type_label: "application/json".to_string(),
                subscribable: false,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            prompts: vec![McpPromptDescriptor {
                server_id,
                name: McpPromptName("review".to_string()),
                description_label: "review prompt".to_string(),
                argument_labels: vec!["scope".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            last_notification_kind: None,
            list_version: 1,
            generated_at: TimestampMillis(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn mcp_client_builds_json_rpc_requests_and_requires_tool_permission() {
        let registry = mcp_registry();
        let server_id = registry.server.server_id.clone();
        let tool_name = registry.tools[0].name.clone();
        let mut client = McpClient::new(registry, MemoryMcpTransport).expect("valid registry");

        let tools_request = client.list_tools_request("list:tools");
        assert_eq!(tools_request.jsonrpc, "2.0");
        assert_eq!(tools_request.method, "tools/list");
        assert_eq!(
            client.list_tools("list:tools").unwrap()["result"]["method"],
            "tools/list"
        );

        client.apply_list_changed_notification(McpListChangedKind::Tools);
        assert_eq!(
            client.registry().last_notification_kind,
            Some(McpListChangedKind::Tools)
        );
        assert_eq!(client.registry().list_version, 2);

        let confirm =
            delegated_task_tool_permission_request(DelegatedTaskToolPermissionRequestInput {
                request_id: "permission:mcp:confirm".to_string(),
                profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::InvokeLocalTool,
                capability: Some(CapabilityId("mcp.tool.call".to_string())),
                target_id: Some("mcp-tool:mcp:test|write_file".to_string()),
                decision: DelegatedTaskToolPermissionDecision::Confirm,
                labels: vec!["mcp.permission".to_string()],
                schema_version: 1,
            });
        assert!(matches!(
            client.call_tool_with_permission(
                "tool:call:1",
                &server_id,
                &tool_name,
                json!({"path_hash": "abc"}),
                &confirm,
            ),
            Err(McpClientError::PermissionRequired { .. })
        ));

        let allow =
            delegated_task_tool_permission_request(DelegatedTaskToolPermissionRequestInput {
                request_id: "permission:mcp:allow".to_string(),
                profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::InvokeLocalTool,
                capability: Some(CapabilityId("mcp.tool.call".to_string())),
                target_id: Some("mcp-tool:mcp:test|write_file".to_string()),
                decision: DelegatedTaskToolPermissionDecision::Allow,
                labels: vec!["mcp.permission".to_string()],
                schema_version: 1,
            });
        let response = client
            .call_tool_with_permission(
                "tool:call:2",
                &server_id,
                &tool_name,
                json!({"path_hash": "abc"}),
                &allow,
            )
            .expect("approved tool call reaches transport");
        assert_eq!(response["result"]["method"], "tools/call");
    }

    #[test]
    fn mcp_client_rejects_permission_for_different_mcp_tool_target() {
        let registry = mcp_registry();
        let server_id = registry.server.server_id.clone();
        let tool_name = registry.tools[0].name.clone();
        let client = McpClient::new(registry, MemoryMcpTransport).expect("valid registry");
        let allow_for_other_tool =
            delegated_task_tool_permission_request(DelegatedTaskToolPermissionRequestInput {
                request_id: "permission:mcp:other-tool".to_string(),
                profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::InvokeLocalTool,
                capability: Some(CapabilityId("mcp.tool.call".to_string())),
                target_id: Some("mcp-tool:mcp:test|read_metadata".to_string()),
                decision: DelegatedTaskToolPermissionDecision::Allow,
                labels: vec!["mcp.permission".to_string()],
                schema_version: 1,
            });

        assert!(matches!(
            client.call_tool_with_permission(
                "tool:call:wrong-target",
                &server_id,
                &tool_name,
                json!({"path_hash": "abc"}),
                &allow_for_other_tool,
            ),
            Err(McpClientError::PermissionRequired { request_id })
                if request_id == "permission:mcp:other-tool"
        ));
    }

    #[test]
    fn mcp_client_rejects_permission_for_different_mcp_tool_capability() {
        let registry = mcp_registry();
        let server_id = registry.server.server_id.clone();
        let tool_name = registry.tools[0].name.clone();
        let client = McpClient::new(registry, MemoryMcpTransport).expect("valid registry");
        let allow_for_other_capability =
            delegated_task_tool_permission_request(DelegatedTaskToolPermissionRequestInput {
                request_id: "permission:mcp:other-capability".to_string(),
                profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::InvokeLocalTool,
                capability: Some(CapabilityId("mcp.resource.read".to_string())),
                target_id: Some("mcp-tool:mcp:test|write_file".to_string()),
                decision: DelegatedTaskToolPermissionDecision::Allow,
                labels: vec!["mcp.permission".to_string()],
                schema_version: 1,
            });

        assert!(matches!(
            client.call_tool_with_permission(
                "tool:call:wrong-capability",
                &server_id,
                &tool_name,
                json!({"path_hash": "abc"}),
                &allow_for_other_capability,
            ),
            Err(McpClientError::PermissionRequired { request_id })
                if request_id == "permission:mcp:other-capability"
        ));
    }

    #[test]
    fn stdio_mcp_transport_reuses_one_process_across_requests() {
        if !cfg!(windows) {
            return;
        }
        let script = "$ErrorActionPreference='Stop';\
            $pidValue=$PID;\
            $count=0;\
            while(($line=[Console]::In.ReadLine()) -ne $null){\
                $req=$line|ConvertFrom-Json;\
                $count++;\
                $response=@{jsonrpc='2.0';id=$req.id;result=@{pid=$pidValue;count=$count;method=$req.method}}|ConvertTo-Json -Compress -Depth 8;\
                [Console]::Out.WriteLine($response);\
                [Console]::Out.Flush();\
            }";
        let transport = StdioMcpTransport::new(StdioMcpTransportConfig {
            command: "powershell.exe".to_string(),
            args: vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                script.to_string(),
            ],
        });

        let first = transport
            .send(&McpJsonRpcEnvelope::request(
                "stdio:1",
                "tools/list",
                json!({}),
            ))
            .expect("first request succeeds");
        let second = transport
            .send(&McpJsonRpcEnvelope::request(
                "stdio:2",
                "resources/list",
                json!({}),
            ))
            .expect("second request succeeds");

        assert_eq!(first["result"]["pid"], second["result"]["pid"]);
        assert_eq!(first["result"]["count"], 1);
        assert_eq!(second["result"]["count"], 2);
    }

    #[test]
    fn mcp_client_reloads_registry_after_list_changed_notification() {
        let registry = mcp_registry();
        let mut client = McpClient::new(registry, ReloadMcpTransport).expect("valid registry");

        let reloaded = client
            .reload_after_list_changed(
                McpListChangedKind::Tools,
                "reload:tools",
                TimestampMillis(9),
            )
            .expect("tools/list reload succeeds")
            .clone();

        assert_eq!(reloaded.last_notification_kind, None);
        assert_eq!(reloaded.list_version, 2);
        assert_eq!(reloaded.tools.len(), 2);
        let existing = reloaded
            .tools
            .iter()
            .find(|tool| tool.name.0 == "write_file")
            .expect("existing tool is preserved");
        assert_eq!(existing.risk_label, ProposalRiskLabel::High);
        let discovered = reloaded
            .tools
            .iter()
            .find(|tool| tool.name.0 == "read_metadata")
            .expect("new tool is discovered");
        assert_eq!(discovered.risk_label, ProposalRiskLabel::Unknown);
        assert_eq!(
            discovered.required_permission_profile,
            DelegatedTaskToolPermissionProfile::Write
        );

        let reloaded = client
            .reload_after_list_changed(
                McpListChangedKind::Resources,
                "reload:resources",
                TimestampMillis(10),
            )
            .expect("resources/list reload succeeds");
        assert!(reloaded.resources[0].subscribable);
    }
}
