//! Provider adapters: Ollama, llama.cpp, OpenAI, Anthropic, future gateway.

#![warn(missing_docs)]

pub mod capabilities;

use std::collections::{HashMap, hash_map::DefaultHasher};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use legion_ai::tool_calls::{
    ToolCallingProvider, ToolCompletionRequest, ToolCompletionResponse, ToolCompletionStopReason,
    ToolConversationTurn, ToolDefinition, ToolTurnBlock,
};
use legion_ai::{
    BatchJobRequest, BatchJobResponse, ChatCompletionRequest, ChatCompletionResponse, ChatRole,
    EmbeddingRequest, EmbeddingResponse, InlinePredictionRequest, InlinePredictionResponse,
    ModelProvider, ProviderCapabilities, ProviderError, ProviderId,
};
use legion_protocol::{
    AssistedAiOperationClass, AssistedAiProviderAvailabilityState, AssistedAiProviderCapability,
    AssistedAiProviderClass, AssistedAiProviderTier, AssistedAiRefusalMetadata,
    AssistedAiSupportLabel, AssistedAiWorkspaceConsent, CapabilityId,
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
/// Native OpenAI Responses API provider slot.
pub const OPENAI_PROVIDER_ID: &str = "openai";
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
    registry.register(Box::new(OpenAiResponsesProvider::from_env(
        OPENAI_PROVIDER_ID,
    )));
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

/// Reasons a provider activation was denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistedAiProviderActivationDenial {
    /// Explicit workspace consent is required but has not been granted.
    ConsentRequired,
    /// A BYOK credential is required but has not been provided.
    CredentialRequired,
    /// Provider tier is always denied (Copilot NES, Mercury, or future hosted).
    HostedDenied,
    /// Workspace is air-gapped; all remote providers are denied.
    AirGapDenied,
}

/// Map a provider's class and id to its activation policy tier.
pub fn provider_tier(class: AssistedAiProviderClass, _provider_id: &str) -> AssistedAiProviderTier {
    match class {
        AssistedAiProviderClass::Local => AssistedAiProviderTier::LocalDefault,
        AssistedAiProviderClass::LocalLoopback => AssistedAiProviderTier::LocalLoopbackOptIn,
        AssistedAiProviderClass::ByokRemote => AssistedAiProviderTier::ByokConsentRequired,
        AssistedAiProviderClass::HostedRemote
        | AssistedAiProviderClass::Gateway
        | AssistedAiProviderClass::Unknown => AssistedAiProviderTier::HostedDenied,
    }
}

/// Evaluate whether a provider may be activated given its tier, workspace consent, and credential.
///
/// Returns `Ok(())` when all preconditions are met. Returns an error describing
/// the first unmet precondition otherwise.
pub fn can_activate_provider(
    tier: AssistedAiProviderTier,
    consent: &AssistedAiWorkspaceConsent,
    has_credential: bool,
) -> Result<(), AssistedAiProviderActivationDenial> {
    match tier {
        AssistedAiProviderTier::LocalDefault => Ok(()),
        AssistedAiProviderTier::LocalLoopbackOptIn => Ok(()),
        AssistedAiProviderTier::ByokConsentRequired => match consent {
            AssistedAiWorkspaceConsent::Denied => {
                Err(AssistedAiProviderActivationDenial::AirGapDenied)
            }
            AssistedAiWorkspaceConsent::NotRequired | AssistedAiWorkspaceConsent::Pending => {
                Err(AssistedAiProviderActivationDenial::ConsentRequired)
            }
            AssistedAiWorkspaceConsent::Granted { .. } => {
                if has_credential {
                    Ok(())
                } else {
                    Err(AssistedAiProviderActivationDenial::CredentialRequired)
                }
            }
        },
        AssistedAiProviderTier::HostedDenied => {
            Err(AssistedAiProviderActivationDenial::HostedDenied)
        }
    }
}

/// Static metadata rows for the provider setup UI.
///
/// Returns one row per known provider showing tier, consent requirements,
/// and credential requirements as display-safe labels.
pub fn provider_setup_rows() -> Vec<String> {
    inline_prediction_provider_capabilities()
        .into_iter()
        .map(|cap| {
            let tier = provider_tier(cap.provider_class, &cap.provider_id);
            let tier_label = match tier {
                AssistedAiProviderTier::LocalDefault => {
                    "tier=LocalDefault consent=NotRequired credential=NotRequired activation=AlwaysActive"
                }
                AssistedAiProviderTier::LocalLoopbackOptIn => {
                    "tier=LocalLoopbackOptIn consent=NotRequired credential=NotRequired activation=RuntimeDetected"
                }
                AssistedAiProviderTier::ByokConsentRequired => {
                    "tier=ByokConsentRequired consent=Required credential=Required activation=ConsentAndCredential"
                }
                AssistedAiProviderTier::HostedDenied => {
                    "tier=HostedDenied consent=N/A credential=N/A activation=AlwaysDenied"
                }
            };
            format!("{}: {}", cap.provider_id, tier_label)
        })
        .collect()
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
            batch: true,
            inline_prediction: true,
            tool_use: false,
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

    fn batch_complete(&self, request: BatchJobRequest) -> Result<BatchJobResponse, ProviderError> {
        let BatchJobRequest {
            provider: _,
            model,
            batch_id,
            job_type,
            requests,
            metadata: _,
        } = request;
        let batch_job_type = job_type.clone();
        let responses = requests
            .into_iter()
            .map(|request| self.complete(request))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(BatchJobResponse {
            provider: self.id.clone(),
            model,
            batch_id,
            job_type,
            responses,
            metadata: HashMap::from([
                ("batch.mode".to_string(), "deterministic-local".to_string()),
                ("batch.job_type".to_string(), batch_job_type),
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

/// Connection-establishment timeout applied to every blocking HTTP request.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Overall request timeout applied to every blocking HTTP request.
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Installs the process-wide rustls crypto provider if one is not already set.
fn install_default_crypto_provider() -> Result<(), String> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }
    match rustls::crypto::ring::default_provider().install_default() {
        Ok(()) => Ok(()),
        Err(_) if rustls::crypto::CryptoProvider::get_default().is_some() => Ok(()),
        Err(error) => Err(format!(
            "failed to install rustls crypto provider: {error:?}"
        )),
    }
}

/// Returns a process-wide blocking reqwest client configured with connect and
/// request timeouts so a hung endpoint cannot block the calling thread forever.
fn shared_blocking_client() -> Result<&'static reqwest::blocking::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            // Building a rustls-backed client requires a default crypto
            // provider; install it eagerly so the first HTTPS request cannot
            // panic the reqwest runtime thread.
            install_default_crypto_provider()?;
            reqwest::blocking::Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .timeout(HTTP_REQUEST_TIMEOUT)
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| error.clone())
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
        let mut request = shared_blocking_client()
            .map_err(|message| ProviderError::RequestFailed {
                provider: "http".to_string(),
                message,
            })?
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
            batch: false,
            inline_prediction: false,
            tool_use: false,
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

/// Native OpenAI Responses API provider adapter.
#[derive(Debug, Clone)]
pub struct OpenAiResponsesProvider<T = ReqwestProviderHttpTransport> {
    id: ProviderId,
    base_url: String,
    api_key: Option<String>,
    transport: T,
}

impl Default for OpenAiResponsesProvider<ReqwestProviderHttpTransport> {
    fn default() -> Self {
        Self::from_env(OPENAI_PROVIDER_ID)
    }
}

impl OpenAiResponsesProvider<ReqwestProviderHttpTransport> {
    /// Creates a native OpenAI Responses adapter from environment configuration.
    pub fn from_env(id: impl Into<ProviderId>) -> Self {
        let api_key = first_configured_value([
            std::env::var(format!("{PRODUCT_ENV_PREFIX}_OPENAI_API_KEY")).ok(),
            std::env::var(format!("{LEGACY_PRODUCT_ENV_PREFIX}_OPENAI_API_KEY")).ok(),
            std::env::var("OPENAI_API_KEY").ok(),
        ]);
        let base_url = first_configured_value([
            std::env::var(format!("{PRODUCT_ENV_PREFIX}_OPENAI_BASE_URL")).ok(),
            std::env::var(format!("{LEGACY_PRODUCT_ENV_PREFIX}_OPENAI_BASE_URL")).ok(),
            std::env::var("OPENAI_BASE_URL").ok(),
        ])
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        Self::with_transport(id, base_url, api_key, ReqwestProviderHttpTransport)
    }
}

impl<T> OpenAiResponsesProvider<T>
where
    T: ProviderHttpTransport,
{
    /// Creates a native OpenAI Responses adapter with an injected transport.
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
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| {
                ProviderError::unavailable(self.id.clone(), "OpenAI API key is not configured")
            })
    }

    fn metadata_flag(metadata: &HashMap<String, String>, key: &str, default: bool) -> bool {
        metadata
            .get(key)
            .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            })
            .unwrap_or(default)
    }

    fn metadata_json(metadata: &HashMap<String, String>, key: &str) -> Option<Value> {
        metadata.get(key).and_then(|value| {
            if value.trim().is_empty() {
                None
            } else {
                serde_json::from_str(value).ok()
            }
        })
    }

    fn request_parts(request: &ChatCompletionRequest) -> (Option<String>, Vec<Value>) {
        let mut instructions = Vec::new();
        let mut input = Vec::new();
        for message in &request.messages {
            match message.role {
                ChatRole::System => instructions.push(message.content.clone()),
                _ => input.push(json!({
                    "role": chat_role_label(&message.role),
                    "content": message.content,
                })),
            }
        }
        let instructions = (!instructions.is_empty()).then(|| instructions.join("\n\n"));
        (instructions, input)
    }

    fn request_payload(&self, request: &ChatCompletionRequest) -> Value {
        let (instructions, input) = Self::request_parts(request);
        let mut payload = json!({
            "model": request.model,
            "input": input,
            "store": Self::metadata_flag(&request.metadata, "openai.responses.store", false),
        });
        if let Some(max_tokens) = request.max_tokens {
            payload["max_output_tokens"] = json!(max_tokens);
        }
        if let Some(temperature) = request.temperature {
            payload["temperature"] = json!(temperature);
        }
        if let Some(instructions) = request
            .metadata
            .get("openai.responses.instructions")
            .cloned()
            .or(instructions)
        {
            payload["instructions"] = json!(instructions);
        }
        if let Some(previous_response_id) = request
            .metadata
            .get("openai.responses.previous_response_id")
            .filter(|value| !value.trim().is_empty())
        {
            payload["previous_response_id"] = json!(previous_response_id);
        }
        if let Some(tools) = Self::metadata_json(&request.metadata, "openai.responses.tools_json") {
            payload["tools"] = tools;
        }
        if let Some(response_format) =
            Self::metadata_json(&request.metadata, "openai.responses.response_format_json")
        {
            payload["response_format"] = response_format;
        }
        payload
    }

    fn extract_output_text(response: &Value) -> Result<String, ProviderError> {
        if let Some(text) = response.get("output_text").and_then(Value::as_str) {
            return Ok(text.to_string());
        }
        let Some(output) = response.get("output").and_then(Value::as_array) else {
            return Err(ProviderError::RequestFailed {
                provider: "openai-responses".to_string(),
                message: "OpenAI Responses response missing output_text and output".to_string(),
            });
        };
        let mut text = String::new();
        for item in output {
            if item.get("type").and_then(Value::as_str) != Some("message") {
                continue;
            }
            let Some(content) = item.get("content").and_then(Value::as_array) else {
                continue;
            };
            for entry in content {
                if entry.get("type").and_then(Value::as_str) != Some("output_text") {
                    continue;
                }
                if let Some(segment) = entry.get("text").and_then(Value::as_str) {
                    text.push_str(segment);
                }
            }
        }
        if text.is_empty() {
            Err(ProviderError::RequestFailed {
                provider: "openai-responses".to_string(),
                message: "OpenAI Responses response missing assistant text".to_string(),
            })
        } else {
            Ok(text)
        }
    }
}

impl<T> ModelProvider for OpenAiResponsesProvider<T>
where
    T: ProviderHttpTransport,
{
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            completion: true,
            embedding: false,
            batch: true,
            inline_prediction: false,
            tool_use: false,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let bearer_token = Some(self.bearer_token()?);
        let response = self.transport.post_json(
            &self.endpoint("/responses"),
            bearer_token,
            self.request_payload(&request),
        )?;
        let text = Self::extract_output_text(&response)?;
        Ok(ChatCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            text,
            metadata: provider_metadata("openai-responses", &self.base_url),
        })
    }

    fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::unavailable(
            request.provider,
            "OpenAI Responses API does not provide embeddings",
        ))
    }

    fn batch_complete(&self, request: BatchJobRequest) -> Result<BatchJobResponse, ProviderError> {
        let BatchJobRequest {
            provider: _,
            model,
            batch_id,
            job_type,
            requests,
            metadata: _,
        } = request;
        let batch_job_type = job_type.clone();
        let responses = requests
            .into_iter()
            .map(|request| self.complete(request))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(BatchJobResponse {
            provider: self.id.clone(),
            model,
            batch_id,
            job_type,
            responses,
            metadata: HashMap::from([
                ("batch.mode".to_string(), "openai-responses".to_string()),
                ("batch.job_type".to_string(), batch_job_type),
            ]),
        })
    }

    fn predict_inline(
        &self,
        request: InlinePredictionRequest,
    ) -> Result<InlinePredictionResponse, ProviderError> {
        Err(ProviderError::unavailable(
            request.provider,
            "OpenAI Responses API is not configured for inline prediction",
        ))
    }
}

/// Configured OpenAI-compatible dialect provider adapter.
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
            batch: false,
            inline_prediction: false,
            tool_use: true,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let bearer_token = self.bearer_token()?;
        let mut payload = json!({
            "model": request.model,
            "messages": request.messages.iter().map(|message| {
                json!({
                    "role": chat_role_label(&message.role),
                    "content": message.content,
                })
            }).collect::<Vec<_>>(),
        });
        if let Some(max_tokens) = request.max_tokens {
            payload["max_tokens"] = json!(max_tokens);
        }
        if let Some(temperature) = request.temperature {
            payload["temperature"] = json!(temperature);
        }
        let response =
            self.transport
                .post_json(&self.endpoint("/chat/completions"), bearer_token, payload)?;
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

impl<T> ToolCallingProvider for OpenAiCompatibleProvider<T>
where
    T: ProviderHttpTransport,
{
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        let bearer_token = self.bearer_token()?;

        // Build the messages array: optional system message, then all turns.
        let mut messages: Vec<Value> = Vec::new();
        if !request.system.is_empty() {
            messages.push(json!({
                "role": "system",
                "content": request.system,
            }));
        }
        messages.extend(request.turns.iter().flat_map(serialize_openai_tool_turn));

        // Convert tool definitions to OpenAI function format.
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t: &ToolDefinition| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect();

        let mut payload = json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "messages": messages,
        });
        if !tools.is_empty() {
            payload["tools"] = json!(tools);
        }

        let response =
            self.transport
                .post_json(&self.endpoint("/chat/completions"), bearer_token, payload)?;

        // Navigate to choices[0].
        let choice = response
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .ok_or_else(|| ProviderError::RequestFailed {
                provider: self.id.clone(),
                message: "OpenAI tool response missing choices[0]".to_string(),
            })?;

        let message = choice
            .get("message")
            .ok_or_else(|| ProviderError::RequestFailed {
                provider: self.id.clone(),
                message: "OpenAI tool response missing choices[0].message".to_string(),
            })?;

        let mut blocks: Vec<ToolTurnBlock> = Vec::new();

        // Extract text content (may be absent or null when tool_calls is present).
        if let Some(text) = message.get("content").and_then(Value::as_str)
            && !text.is_empty()
        {
            blocks.push(ToolTurnBlock::Text(text.to_string()));
        }

        // Extract tool_calls array and convert to ToolUse blocks.
        if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
            for call in tool_calls {
                let id = call
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let func = call
                    .get("function")
                    .ok_or_else(|| ProviderError::RequestFailed {
                        provider: self.id.clone(),
                        message: "OpenAI tool_call missing function object".to_string(),
                    })?;
                let name = func
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                // arguments is a JSON *string* — parse it explicitly.
                let arguments_str = func
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                let input: Value =
                    serde_json::from_str(arguments_str).map_err(|e| {
                        ProviderError::RequestFailed {
                            provider: self.id.clone(),
                            message: format!(
                                "OpenAI tool_call arguments is not valid JSON: {e}. Raw: {arguments_str:?}"
                            ),
                        }
                    })?;
                blocks.push(ToolTurnBlock::ToolUse { id, name, input });
            }
        }

        // Map finish_reason to stop reason.
        let finish_reason = choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .unwrap_or("stop");

        let stop_reason = match finish_reason {
            "tool_calls" => ToolCompletionStopReason::ToolUse,
            "length" => ToolCompletionStopReason::MaxTokens,
            _ => ToolCompletionStopReason::EndTurn,
        };

        Ok(ToolCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            blocks,
            stop_reason,
        })
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
    credential_kind: AnthropicCredentialKind,
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

/// Distinguishes how an Anthropic credential must be sent on the wire.
///
/// Anthropic API keys are sent via the `x-api-key` header, while OAuth/session
/// auth tokens are sent via the `Authorization: Bearer` header. Sending an API
/// key as a bearer token is rejected by the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnthropicCredentialKind {
    /// Credential sent via the `x-api-key` header.
    ApiKey,
    /// Credential sent via the `Authorization: Bearer` header.
    AuthToken,
}

/// An Anthropic credential paired with the header kind it must be sent as.
#[derive(Debug, Clone, Copy)]
pub struct AnthropicCredential<'a> {
    /// The secret credential value.
    pub value: &'a str,
    /// Whether the value is an API key or an auth token.
    pub kind: AnthropicCredentialKind,
}

/// Shared HTTP transport abstraction for Anthropic Messages API calls.
pub trait AnthropicMessagesTransport: Clone + Send + Sync + 'static {
    /// POST a JSON payload and return the parsed JSON response.
    fn post_json(
        &self,
        endpoint: &str,
        credential: Option<AnthropicCredential<'_>>,
        beta_header: Option<&str>,
        payload: Value,
    ) -> Result<Value, ProviderError>;

    /// POST a JSON payload and return the raw text response body.
    fn post_text(
        &self,
        endpoint: &str,
        credential: Option<AnthropicCredential<'_>>,
        beta_header: Option<&str>,
        payload: Value,
    ) -> Result<String, ProviderError>;

    /// POST a streaming SSE payload and invoke `on_event` as each event is parsed.
    ///
    /// Default implementation buffers the full body via [`Self::post_text`] then
    /// parses events. Progressive transports may override for mid-body callbacks.
    fn stream_sse_text(
        &self,
        endpoint: &str,
        credential: Option<AnthropicCredential<'_>>,
        beta_header: Option<&str>,
        payload: Value,
        on_event: &mut dyn FnMut(AnthropicSseEvent),
    ) -> Result<(), ProviderError> {
        let body = self.post_text(endpoint, credential, beta_header, payload)?;
        for event in parse_anthropic_sse_events(&body)? {
            on_event(event);
        }
        Ok(())
    }
}

/// Applies an optional Anthropic credential to a blocking request builder,
/// selecting the correct authentication header for the credential kind.
fn apply_anthropic_credential(
    request: reqwest::blocking::RequestBuilder,
    credential: Option<AnthropicCredential<'_>>,
) -> reqwest::blocking::RequestBuilder {
    match credential.filter(|credential| !credential.value.trim().is_empty()) {
        Some(credential) => match credential.kind {
            AnthropicCredentialKind::ApiKey => request.header("x-api-key", credential.value),
            AnthropicCredentialKind::AuthToken => request.bearer_auth(credential.value),
        },
        None => request,
    }
}

impl AnthropicMessagesTransport for ReqwestProviderHttpTransport {
    fn post_json(
        &self,
        endpoint: &str,
        credential: Option<AnthropicCredential<'_>>,
        beta_header: Option<&str>,
        payload: Value,
    ) -> Result<Value, ProviderError> {
        let mut request = shared_blocking_client()
            .map_err(|message| ProviderError::RequestFailed {
                provider: "http".to_string(),
                message,
            })?
            .post(endpoint)
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .json(&payload);
        if let Some(beta_header) = beta_header.filter(|value| !value.trim().is_empty()) {
            request = request.header("anthropic-beta", beta_header);
        }
        request = apply_anthropic_credential(request, credential);
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
        credential: Option<AnthropicCredential<'_>>,
        beta_header: Option<&str>,
        payload: Value,
    ) -> Result<String, ProviderError> {
        let mut request = shared_blocking_client()
            .map_err(|message| ProviderError::RequestFailed {
                provider: "http".to_string(),
                message,
            })?
            .post(endpoint)
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .json(&payload);
        if let Some(beta_header) = beta_header.filter(|value| !value.trim().is_empty()) {
            request = request.header("anthropic-beta", beta_header);
        }
        request = apply_anthropic_credential(request, credential);
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

    fn stream_sse_text(
        &self,
        endpoint: &str,
        credential: Option<AnthropicCredential<'_>>,
        beta_header: Option<&str>,
        payload: Value,
        on_event: &mut dyn FnMut(AnthropicSseEvent),
    ) -> Result<(), ProviderError> {
        let mut request = shared_blocking_client()
            .map_err(|message| ProviderError::RequestFailed {
                provider: "http".to_string(),
                message,
            })?
            .post(endpoint)
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .header("accept", "text/event-stream")
            .json(&payload);
        if let Some(beta_header) = beta_header.filter(|value| !value.trim().is_empty()) {
            request = request.header("anthropic-beta", beta_header);
        }
        request = apply_anthropic_credential(request, credential);
        let mut response = request
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

        // Progressive read: fire events as complete SSE records arrive (not only
        // after the full body is buffered). Accumulate *bytes* so UTF-8 sequences
        // that straddle HTTP chunk boundaries are not corrupted.
        let mut carry_bytes: Vec<u8> = Vec::new();
        let mut current_event: Option<String> = None;
        let mut current_data = String::new();
        let mut buf = [0u8; 2048];
        loop {
            let n = response
                .read(&mut buf)
                .map_err(|error| ProviderError::RequestFailed {
                    provider: "http".to_string(),
                    message: error.to_string(),
                })?;
            if n == 0 {
                break;
            }
            carry_bytes.extend_from_slice(&buf[..n]);
            // Process complete UTF-8 lines; leave incomplete trailing bytes in carry.
            loop {
                let Some(nl) = carry_bytes.iter().position(|b| *b == b'\n') else {
                    break;
                };
                let mut line_bytes = carry_bytes.drain(..=nl).collect::<Vec<u8>>();
                if line_bytes.last() == Some(&b'\n') {
                    line_bytes.pop();
                }
                if line_bytes.last() == Some(&b'\r') {
                    line_bytes.pop();
                }
                let line = String::from_utf8_lossy(&line_bytes);
                if line.is_empty() {
                    if let Some(event) =
                        flush_anthropic_sse_event(current_event.take(), &mut current_data)?
                    {
                        on_event(event);
                    }
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
        }
        // Flush any final complete lines still buffered as valid UTF-8.
        if !carry_bytes.is_empty()
            && let Ok(tail) = std::str::from_utf8(&carry_bytes)
        {
            for line in tail.split('\n') {
                let line = line.trim_end_matches('\r');
                if line.is_empty() {
                    if let Some(event) =
                        flush_anthropic_sse_event(current_event.take(), &mut current_data)?
                    {
                        on_event(event);
                    }
                    continue;
                }
                if let Some(rest) = line.strip_prefix("event:") {
                    current_event = Some(rest.trim().to_string());
                } else if let Some(rest) = line.strip_prefix("data:") {
                    if !current_data.is_empty() {
                        current_data.push('\n');
                    }
                    current_data.push_str(rest.trim_start());
                }
            }
        }
        if let Some(event) = flush_anthropic_sse_event(current_event.take(), &mut current_data)? {
            on_event(event);
        }
        Ok(())
    }
}

/// Parse a full Anthropic SSE body into ordered events.
fn parse_anthropic_sse_events(body: &str) -> Result<Vec<AnthropicSseEvent>, ProviderError> {
    let mut events = Vec::new();
    let mut current_event: Option<String> = None;
    let mut current_data = String::new();
    for line in body.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if let Some(event) = flush_anthropic_sse_event(current_event.take(), &mut current_data)?
            {
                events.push(event);
            }
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
    if let Some(event) = flush_anthropic_sse_event(current_event.take(), &mut current_data)? {
        events.push(event);
    }
    Ok(events)
}

fn flush_anthropic_sse_event(
    event: Option<String>,
    data: &mut String,
) -> Result<Option<AnthropicSseEvent>, ProviderError> {
    let Some(event) = event else {
        data.clear();
        return Ok(None);
    };
    if event == "ping" {
        data.clear();
        return Ok(None);
    }
    let payload = if data.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str::<Value>(data).map_err(|error| ProviderError::RequestFailed {
            provider: "anthropic".to_string(),
            message: error.to_string(),
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
    data.clear();
    Ok(Some(parsed))
}

impl Default for AnthropicMessagesClient<ReqwestProviderHttpTransport> {
    fn default() -> Self {
        Self::from_env(ANTHROPIC_PROVIDER_ID)
    }
}

impl AnthropicMessagesClient<ReqwestProviderHttpTransport> {
    /// Creates an Anthropic adapter from environment configuration.
    pub fn from_env(id: impl Into<ProviderId>) -> Self {
        let (api_key, credential_kind) = Self::credential_from_env();
        let base_url = first_configured_value([
            std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_BASE_URL")).ok(),
            std::env::var(format!("{LEGACY_PRODUCT_ENV_PREFIX}_ANTHROPIC_BASE_URL")).ok(),
            std::env::var("ANTHROPIC_BASE_URL").ok(),
        ])
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
        Self::with_transport_kind(
            id,
            base_url,
            api_key,
            credential_kind,
            ReqwestProviderHttpTransport,
        )
    }

    /// Reads the Anthropic credential from the environment, preserving the
    /// existing precedence while recording whether the value is an API key
    /// (`x-api-key`) or an auth token (`Authorization: Bearer`).
    fn credential_from_env() -> (Option<String>, AnthropicCredentialKind) {
        let candidates = [
            (
                std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_API_KEY")).ok(),
                AnthropicCredentialKind::ApiKey,
            ),
            (
                std::env::var(format!("{PRODUCT_ENV_PREFIX}_ANTHROPIC_AUTH_TOKEN")).ok(),
                AnthropicCredentialKind::AuthToken,
            ),
            (
                std::env::var(format!("{LEGACY_PRODUCT_ENV_PREFIX}_ANTHROPIC_API_KEY")).ok(),
                AnthropicCredentialKind::ApiKey,
            ),
            (
                std::env::var(format!("{LEGACY_PRODUCT_ENV_PREFIX}_ANTHROPIC_AUTH_TOKEN")).ok(),
                AnthropicCredentialKind::AuthToken,
            ),
            (
                std::env::var("ANTHROPIC_API_KEY").ok(),
                AnthropicCredentialKind::ApiKey,
            ),
            (
                std::env::var("ANTHROPIC_AUTH_TOKEN").ok(),
                AnthropicCredentialKind::AuthToken,
            ),
        ];
        for (value, kind) in candidates {
            if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
                return (Some(value), kind);
            }
        }
        (None, AnthropicCredentialKind::ApiKey)
    }
}

impl<T> AnthropicMessagesClient<T>
where
    T: AnthropicMessagesTransport,
{
    /// Creates an Anthropic adapter with an injected transport.
    ///
    /// The credential is treated as an API key (`x-api-key`). Use
    /// [`AnthropicMessagesClient::with_transport_kind`] to supply an auth token
    /// that must be sent via `Authorization: Bearer`.
    pub fn with_transport(
        id: impl Into<ProviderId>,
        base_url: impl Into<String>,
        api_key: Option<String>,
        transport: T,
    ) -> Self {
        Self::with_transport_kind(
            id,
            base_url,
            api_key,
            AnthropicCredentialKind::ApiKey,
            transport,
        )
    }

    /// Creates an Anthropic adapter with an injected transport and an explicit
    /// credential kind controlling which authentication header is sent.
    pub fn with_transport_kind(
        id: impl Into<ProviderId>,
        base_url: impl Into<String>,
        api_key: Option<String>,
        credential_kind: AnthropicCredentialKind,
        transport: T,
    ) -> Self {
        Self {
            id: id.into(),
            base_url: normalize_base_url(base_url.into()),
            api_key,
            credential_kind,
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

    fn credential(&self) -> Result<AnthropicCredential<'_>, ProviderError> {
        Ok(AnthropicCredential {
            value: self.bearer_token()?,
            kind: self.credential_kind,
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
            "cache_control": Self::cache_control(),
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
            "cache_control": Self::cache_control(),
        });
        if let Some(system) = system {
            payload["system"] = json!(system);
        }
        if !extras.tools.is_empty() {
            payload["tools"] = json!(extras.tools);
        }
        payload
    }

    fn cache_control() -> Value {
        json!({"type": "ephemeral"})
    }

    fn usage_metadata(response: &Value) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        let Some(usage) = response.get("usage") else {
            return metadata;
        };
        for (field, label) in [
            ("input_tokens", "provider.usage.input_tokens"),
            ("output_tokens", "provider.usage.output_tokens"),
            (
                "cache_creation_input_tokens",
                "provider.usage.cache_creation_input_tokens",
            ),
            (
                "cache_read_input_tokens",
                "provider.usage.cache_read_input_tokens",
            ),
        ] {
            if let Some(value) = usage.get(field).and_then(Value::as_u64) {
                metadata.insert(label.to_string(), value.to_string());
            }
        }
        metadata
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

    fn extract_assistant_blocks(
        response: &Value,
    ) -> Result<(Vec<ToolTurnBlock>, ToolCompletionStopReason), ProviderError> {
        let content = response
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::RequestFailed {
                provider: "anthropic".to_string(),
                message: "response missing content blocks".to_string(),
            })?;

        let mut blocks = Vec::new();
        for block in content {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(Value::as_str) {
                        blocks.push(ToolTurnBlock::Text(text.to_string()));
                    }
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let input = block
                        .get("input")
                        .cloned()
                        .unwrap_or(Value::Object(Default::default()));
                    blocks.push(ToolTurnBlock::ToolUse { id, name, input });
                }
                _ => {} // skip thinking blocks and other unknown types
            }
        }

        let stop_reason = match response.get("stop_reason").and_then(Value::as_str) {
            Some("tool_use") => ToolCompletionStopReason::ToolUse,
            Some("end_turn") => ToolCompletionStopReason::EndTurn,
            Some("max_tokens") => ToolCompletionStopReason::MaxTokens,
            _ => ToolCompletionStopReason::EndTurn,
        };

        Ok((blocks, stop_reason))
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
        parse_anthropic_sse_events(body)
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
        let credential = self.credential()?;
        let response = self.transport.post_json(
            &self.endpoint("/v1/messages"),
            Some(credential),
            Self::beta_header_for_extras(&extras),
            self.completion_payload(&request, false, &extras),
        )?;
        let text = Self::extract_assistant_text(&response)?;
        let mut metadata = provider_metadata("anthropic", &self.base_url);
        metadata.extend(Self::usage_metadata(&response));
        Ok(ChatCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            text,
            metadata,
        })
    }

    /// Streams a completion request and returns the parsed SSE event sequence.
    pub fn stream_events_with_extras(
        &self,
        request: ChatCompletionRequest,
        extras: AnthropicRequestExtras,
    ) -> Result<Vec<AnthropicSseEvent>, ProviderError> {
        let credential = self.credential()?;
        let body = self.transport.post_text(
            &self.endpoint("/v1/messages"),
            Some(credential),
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

    /// Streams text deltas and invokes `on_delta` as each SSE text delta is parsed.
    ///
    /// When the transport is [`ReqwestProviderHttpTransport`], the response body is
    /// read progressively so callbacks can fire before the full HTTP body is buffered.
    /// Other transports fall back to buffering the full SSE body first.
    pub fn stream_text_deltas_with_callback(
        &self,
        request: ChatCompletionRequest,
        extras: AnthropicRequestExtras,
        mut on_delta: impl FnMut(&str),
    ) -> Result<Vec<String>, ProviderError> {
        let credential = self.credential()?;
        let payload = self.completion_payload(&request, true, &extras);
        let beta = Self::beta_header_for_extras(&extras);
        let mut chunks = Vec::new();
        let mut push = |text: &str| {
            if text.is_empty() {
                return;
            }
            on_delta(text);
            chunks.push(text.to_string());
        };
        self.transport.stream_sse_text(
            &self.endpoint("/v1/messages"),
            Some(credential),
            beta,
            payload,
            &mut |event| {
                if let AnthropicSseEvent::ContentBlockDelta(text) = event {
                    push(&text);
                }
            },
        )?;
        Ok(chunks)
    }

    /// Counts the input tokens for a completion request using Anthropic's token-count endpoint.
    pub fn count_tokens_with_extras(
        &self,
        request: ChatCompletionRequest,
        extras: AnthropicRequestExtras,
    ) -> Result<u32, ProviderError> {
        let credential = self.credential()?;
        let response = self.transport.post_json(
            &self.endpoint("/v1/messages/count_tokens"),
            Some(credential),
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
            batch: true,
            inline_prediction: false,
            tool_use: true,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        AnthropicMessagesClient::complete(self, request)
    }

    fn batch_complete(&self, request: BatchJobRequest) -> Result<BatchJobResponse, ProviderError> {
        let BatchJobRequest {
            provider: _,
            model,
            batch_id,
            job_type,
            requests,
            metadata: _,
        } = request;
        let batch_job_type = job_type.clone();
        let responses = requests
            .into_iter()
            .map(|request| AnthropicMessagesClient::complete(self, request))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(BatchJobResponse {
            provider: self.id.clone(),
            model,
            batch_id,
            job_type,
            responses,
            metadata: HashMap::from([
                ("batch.mode".to_string(), "anthropic-messages".to_string()),
                ("batch.job_type".to_string(), batch_job_type),
            ]),
        })
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

/// Serialize a `ToolConversationTurn` into the Anthropic Messages API wire format.
fn serialize_tool_turn(turn: &ToolConversationTurn) -> Value {
    let content: Vec<Value> = turn
        .blocks
        .iter()
        .map(|block| match block {
            ToolTurnBlock::Text(text) => json!({
                "type": "text",
                "text": text,
            }),
            ToolTurnBlock::ToolUse { id, name, input } => json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            }),
            ToolTurnBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "content": content,
                "is_error": is_error,
            }),
        })
        .collect();
    json!({
        "role": turn.role,
        "content": content,
    })
}

/// Serialize a `ToolConversationTurn` into one or more OpenAI chat-completions
/// wire-format messages.
///
/// A single turn may expand into multiple messages:
/// - An "assistant" turn collapses to ONE `role:"assistant"` message: `Text` blocks
///   concatenated into `content`; `ToolUse` blocks emitted as the `tool_calls` array.
/// - A "user" turn with `ToolResult` blocks expands to one `role:"tool"` message per
///   result (wire-order requirement — a 400 is returned if tool messages don't immediately
///   follow the assistant message that issued the tool_calls). Any `Text` blocks become a
///   `role:"user"` message emitted *after* the tool messages.
fn serialize_openai_tool_turn(turn: &ToolConversationTurn) -> Vec<Value> {
    let mut messages = Vec::new();
    match turn.role.as_str() {
        "assistant" => {
            let mut text_parts: Vec<&str> = Vec::new();
            let mut tool_calls: Vec<Value> = Vec::new();
            for block in &turn.blocks {
                match block {
                    ToolTurnBlock::Text(t) => text_parts.push(t.as_str()),
                    ToolTurnBlock::ToolUse { id, name, input } => {
                        tool_calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                // arguments must be a JSON string, not a parsed object.
                                "arguments": input.to_string()
                            }
                        }));
                    }
                    ToolTurnBlock::ToolResult { .. } => {} // should not appear in assistant turns
                }
            }
            let mut msg = json!({ "role": "assistant" });
            if !text_parts.is_empty() {
                msg["content"] = json!(text_parts.join("\n"));
            }
            if !tool_calls.is_empty() {
                msg["tool_calls"] = json!(tool_calls);
            }
            messages.push(msg);
        }
        _ => {
            // "user" role: ToolResult blocks → role:"tool" messages first (wire-order),
            // then Text blocks → role:"user" message.
            for block in &turn.blocks {
                if let ToolTurnBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } = block
                {
                    // OpenAI has no native is_error field — prefix with "ERROR: " instead.
                    let wire_content = if *is_error {
                        format!("ERROR: {content}")
                    } else {
                        content.clone()
                    };
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id,
                        "content": wire_content,
                    }));
                }
            }
            let user_texts: Vec<&str> = turn
                .blocks
                .iter()
                .filter_map(|b| {
                    if let ToolTurnBlock::Text(t) = b {
                        Some(t.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            if !user_texts.is_empty() {
                messages.push(json!({
                    "role": "user",
                    "content": user_texts.join("\n"),
                }));
            }
        }
    }
    messages
}

impl<T> ToolCallingProvider for AnthropicMessagesClient<T>
where
    T: AnthropicMessagesTransport,
{
    fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, ProviderError> {
        let credential = self.credential()?;

        let messages: Vec<Value> = request.turns.iter().map(serialize_tool_turn).collect();

        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t: &ToolDefinition| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect();

        let mut payload = json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "messages": messages,
            "stream": false,
        });

        if !request.system.is_empty() {
            payload["system"] = json!(request.system);
        }

        if !tools.is_empty() {
            payload["tools"] = json!(tools);
        }

        let response = self.transport.post_json(
            &self.endpoint("/v1/messages"),
            Some(credential),
            None,
            payload,
        )?;

        let (blocks, stop_reason) = Self::extract_assistant_blocks(&response)?;

        Ok(ToolCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            blocks,
            stop_reason,
        })
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
            batch: false,
            inline_prediction: true,
            tool_use: false,
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
pub struct StdioMcpTransport {
    config: StdioMcpTransportConfig,
    session: Arc<Mutex<Option<StdioMcpSession>>>,
}

impl Clone for StdioMcpTransport {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            session: Arc::new(Mutex::new(None)),
        }
    }
}

impl fmt::Debug for StdioMcpTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StdioMcpTransport")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Per-request timeout for a single stdio MCP round trip.
const STDIO_MCP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

struct StdioMcpSession {
    child: Child,
    stdin: ChildStdin,
    responses: mpsc::Receiver<std::io::Result<String>>,
    reader: Option<JoinHandle<()>>,
}

impl Drop for StdioMcpSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // The reader thread owns the child's stdout; killing the child closes
        // the pipe so the blocking read returns EOF and the thread exits.
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
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
        // Read stdout on a dedicated thread so a wedged or silent server cannot
        // block the caller indefinitely: the request loop enforces a deadline
        // over the channel rather than on a blocking pipe read.
        let (sender, responses) = mpsc::channel::<std::io::Result<String>>();
        let reader = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        if sender.send(Ok(line)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        break;
                    }
                }
            }
        });
        Ok(StdioMcpSession {
            child,
            stdin,
            responses,
            reader: Some(reader),
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
        let deadline = Instant::now() + STDIO_MCP_REQUEST_TIMEOUT;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| {
                    McpClientError::Transport(
                        "stdio MCP request timed out waiting for response".to_string(),
                    )
                })?;
            let line = match session.responses.recv_timeout(remaining) {
                Ok(Ok(line)) => line,
                Ok(Err(error)) => return Err(McpClientError::Transport(error.to_string())),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(McpClientError::Transport(
                        "stdio MCP request timed out waiting for response".to_string(),
                    ));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let status = session.child.try_wait().ok().flatten();
                    return Err(McpClientError::Transport(match status {
                        Some(status) => format!("stdio MCP server exited with {status}"),
                        None => "stdio MCP server closed stdout".to_string(),
                    }));
                }
            };
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

fn ensure_rustls_crypto_provider() -> Result<(), McpClientError> {
    install_default_crypto_provider().map_err(McpClientError::Transport)
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
        ensure_rustls_crypto_provider()?;
        if self.config.endpoint.trim().is_empty() {
            return Err(McpClientError::Transport(
                "Streamable HTTP MCP endpoint must not be empty".to_string(),
            ));
        }
        let response = shared_blocking_client()
            .map_err(McpClientError::Transport)?
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
        credential_kind: Option<AnthropicCredentialKind>,
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
                    credential_kind: None,
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
            } else if endpoint.ends_with("/responses") {
                Ok(json!({
                    "id": "resp_123",
                    "output": [
                        {
                            "type": "message",
                            "role": "assistant",
                            "content": [
                                { "type": "output_text", "text": "responses answer" }
                            ]
                        }
                    ]
                }))
            } else if endpoint.ends_with("/v1/messages/count_tokens") {
                Ok(json!({ "input_tokens": 73 }))
            } else if endpoint.ends_with("/v1/messages") {
                Ok(json!({
                    "content": [
                        { "type": "text", "text": "anthropic answer" }
                    ],
                    "usage": {
                        "input_tokens": 17,
                        "output_tokens": 5,
                        "cache_creation_input_tokens": 0,
                        "cache_read_input_tokens": 17
                    }
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
            credential: Option<AnthropicCredential<'_>>,
            beta_header: Option<&str>,
            payload: Value,
        ) -> Result<Value, ProviderError> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(RecordedProviderCall {
                    endpoint: endpoint.to_string(),
                    bearer_token: credential.map(|credential| credential.value.to_string()),
                    credential_kind: credential.map(|credential| credential.kind),
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
                    "usage": {
                        "input_tokens": 17,
                        "output_tokens": 5,
                        "cache_creation_input_tokens": 0,
                        "cache_read_input_tokens": 17
                    }
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
            credential: Option<AnthropicCredential<'_>>,
            beta_header: Option<&str>,
            payload: Value,
        ) -> Result<String, ProviderError> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(RecordedProviderCall {
                    endpoint: endpoint.to_string(),
                    bearer_token: credential.map(|credential| credential.value.to_string()),
                    credential_kind: credential.map(|credential| credential.kind),
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
    fn shared_blocking_client_builds_with_timeouts() {
        assert!(shared_blocking_client().is_ok());
    }

    #[test]
    fn openai_compatible_provider_omits_unset_sampling_options() {
        let transport = RecordingProviderTransport::default();
        let provider = LlamaCppProvider::with_transport(
            LLAMA_CPP_PROVIDER_ID,
            "http://localhost:8080/v1/",
            None,
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
        // Unset sampling options must be omitted, not serialized as JSON null.
        assert!(calls[0].payload.get("max_tokens").is_none());
        assert!(calls[0].payload.get("temperature").is_none());
    }

    #[test]
    fn openai_responses_store_defaults_to_false_without_metadata() {
        let transport = RecordingProviderTransport::default();
        let provider = OpenAiResponsesProvider::with_transport(
            OPENAI_PROVIDER_ID,
            "https://provider.example/v1/",
            Some("test-key".to_string()),
            transport.clone(),
        );

        provider
            .complete(ChatCompletionRequest::new(
                OPENAI_PROVIDER_ID,
                "gpt-test",
                "hello",
            ))
            .expect("responses completion parses");

        let calls = transport.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].payload["store"], false);
    }

    #[test]
    fn anthropic_api_key_uses_x_api_key_and_auth_token_uses_bearer() {
        let api_key_transport = RecordingProviderTransport::default();
        let api_key_client = AnthropicMessagesClient::with_transport(
            ANTHROPIC_PROVIDER_ID,
            "https://api.anthropic.com/",
            Some("sk-ant-key".to_string()),
            api_key_transport.clone(),
        );
        api_key_client
            .complete(ChatCompletionRequest::new(
                ANTHROPIC_PROVIDER_ID,
                "claude-test",
                "hi",
            ))
            .expect("anthropic completion parses");
        let api_calls = api_key_transport.calls();
        assert_eq!(api_calls.len(), 1);
        assert_eq!(
            api_calls[0].credential_kind,
            Some(AnthropicCredentialKind::ApiKey)
        );
        assert_eq!(api_calls[0].bearer_token, Some("sk-ant-key".to_string()));

        let token_transport = RecordingProviderTransport::default();
        let token_client = AnthropicMessagesClient::with_transport_kind(
            ANTHROPIC_PROVIDER_ID,
            "https://api.anthropic.com/",
            Some("oauth-token".to_string()),
            AnthropicCredentialKind::AuthToken,
            token_transport.clone(),
        );
        token_client
            .complete(ChatCompletionRequest::new(
                ANTHROPIC_PROVIDER_ID,
                "claude-test",
                "hi",
            ))
            .expect("anthropic completion parses");
        let token_calls = token_transport.calls();
        assert_eq!(token_calls.len(), 1);
        assert_eq!(
            token_calls[0].credential_kind,
            Some(AnthropicCredentialKind::AuthToken)
        );
        assert_eq!(token_calls[0].bearer_token, Some("oauth-token".to_string()));
    }

    #[test]
    fn openai_responses_provider_posts_stateful_requests_and_parses_output_text() {
        let transport = RecordingProviderTransport::default();
        let provider = OpenAiResponsesProvider::with_transport(
            OPENAI_PROVIDER_ID,
            "https://provider.example/v1/",
            Some("test-key".to_string()),
            transport.clone(),
        );

        let request = ChatCompletionRequest {
            provider: OPENAI_PROVIDER_ID.to_string(),
            model: "gpt-test".to_string(),
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: "You are a careful assistant.".to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: "Hello, responses.".to_string(),
                },
            ],
            max_tokens: Some(64),
            temperature: Some(0.15),
            metadata: std::collections::HashMap::from([
                (
                    "openai.responses.previous_response_id".to_string(),
                    "resp_prev_123".to_string(),
                ),
                ("openai.responses.store".to_string(), "false".to_string()),
                (
                    "openai.responses.tools_json".to_string(),
                    json!([
                        {
                            "type": "function",
                            "name": "lookup_docs",
                            "description": "Lookup docs",
                            "parameters": {
                                "type": "object",
                                "properties": {
                                    "query": { "type": "string" }
                                },
                                "required": ["query"],
                                "additionalProperties": false
                            }
                        }
                    ])
                    .to_string(),
                ),
                (
                    "openai.responses.response_format_json".to_string(),
                    json!({
                        "type": "json_schema",
                        "name": "final_answer",
                        "schema": {
                            "type": "object",
                            "properties": {
                                "answer": { "type": "string" }
                            },
                            "required": ["answer"],
                            "additionalProperties": false
                        }
                    })
                    .to_string(),
                ),
            ]),
        };

        let completion = provider
            .complete(request)
            .expect("OpenAI Responses completion parses");
        assert_eq!(completion.text, "responses answer");
        assert_eq!(
            completion.metadata.get("provider.kind"),
            Some(&"openai-responses".to_string())
        );

        let calls = transport.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].endpoint, "https://provider.example/v1/responses");
        assert_eq!(calls[0].bearer_token, Some("test-key".to_string()));
        assert_eq!(calls[0].payload["model"], "gpt-test");
        assert_eq!(calls[0].payload["store"], false);
        assert_eq!(calls[0].payload["max_output_tokens"], 64);
        assert!(calls[0].payload.get("temperature").is_some());
        assert_eq!(calls[0].payload["previous_response_id"], "resp_prev_123");
        assert_eq!(
            calls[0].payload["instructions"],
            "You are a careful assistant."
        );
        assert_eq!(calls[0].payload["input"][0]["role"], "user");
        assert_eq!(calls[0].payload["input"][0]["content"], "Hello, responses.");
        assert_eq!(calls[0].payload["tools"][0]["type"], "function");
        assert_eq!(calls[0].payload["response_format"]["type"], "json_schema");
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
                OPENAI_PROVIDER_ID.to_string(),
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
                OPENAI_PROVIDER_ID.to_string(),
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
        assert_eq!(
            completion.metadata.get("provider.usage.input_tokens"),
            Some(&"17".to_string())
        );
        assert_eq!(
            completion
                .metadata
                .get("provider.usage.cache_read_input_tokens"),
            Some(&"17".to_string())
        );
        assert_eq!(
            completion
                .metadata
                .get("provider.usage.cache_creation_input_tokens"),
            Some(&"0".to_string())
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
        assert_eq!(calls[0].payload["cache_control"]["type"], "ephemeral");
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
        assert_eq!(calls[1].payload["cache_control"]["type"], "ephemeral");
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
    fn openai_and_anthropic_batch_jobs_round_trip_repo_summary_requests() {
        let openai_transport = RecordingProviderTransport::default();
        let openai = OpenAiResponsesProvider::with_transport(
            OPENAI_PROVIDER_ID,
            "https://api.openai.com/v1/",
            Some("openai-key".to_string()),
            openai_transport.clone(),
        );
        let anthropic_transport = RecordingProviderTransport::default();
        let anthropic = AnthropicMessagesClient::with_transport(
            ANTHROPIC_PROVIDER_ID,
            "https://api.anthropic.com/",
            Some("anthropic-key".to_string()),
            anthropic_transport.clone(),
        );

        let batch_request = BatchJobRequest::new(
            OPENAI_PROVIDER_ID,
            "batch-model",
            "batch-repo-summary-2",
            "repo-summary",
            vec![
                ChatCompletionRequest::new(
                    OPENAI_PROVIDER_ID,
                    "batch-model",
                    "summarize repo chunk one",
                ),
                ChatCompletionRequest::new(
                    OPENAI_PROVIDER_ID,
                    "batch-model",
                    "summarize repo chunk two",
                ),
            ],
        )
        .with_metadata("source", "ws10");

        let openai_response = openai
            .batch_complete(batch_request.clone())
            .expect("openai batch job succeeds");
        let anthropic_response = anthropic
            .batch_complete(BatchJobRequest {
                provider: ANTHROPIC_PROVIDER_ID.to_string(),
                ..batch_request
            })
            .expect("anthropic batch job succeeds");

        assert_eq!(openai_response.batch_id, "batch-repo-summary-2");
        assert_eq!(openai_response.job_type, "repo-summary");
        assert_eq!(openai_response.responses.len(), 2);
        assert_eq!(
            openai_response.metadata.get("batch.mode"),
            Some(&"openai-responses".to_string())
        );
        assert_eq!(anthropic_response.batch_id, "batch-repo-summary-2");
        assert_eq!(anthropic_response.job_type, "repo-summary");
        assert_eq!(anthropic_response.responses.len(), 2);
        assert_eq!(
            anthropic_response.metadata.get("batch.mode"),
            Some(&"anthropic-messages".to_string())
        );

        let openai_calls = openai_transport.calls();
        assert_eq!(openai_calls.len(), 2);
        assert!(
            openai_calls
                .iter()
                .all(|call| call.endpoint == "https://api.openai.com/v1/responses")
        );
        let anthropic_calls = anthropic_transport.calls();
        assert_eq!(anthropic_calls.len(), 2);
        assert!(
            anthropic_calls
                .iter()
                .all(|call| call.endpoint == "https://api.anthropic.com/v1/messages")
        );
    }

    #[test]
    fn anthropic_completion_payload_serialization_is_byte_stable_for_equivalent_requests() {
        let transport = RecordingProviderTransport::default();
        let provider = AnthropicMessagesClient::with_transport(
            ANTHROPIC_PROVIDER_ID,
            "https://api.anthropic.com/",
            Some("anthropic-key".to_string()),
            transport,
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

        let first = provider.completion_payload(&request, false, &extras);
        let second = provider.completion_payload(&request, false, &extras);

        let first_bytes = serde_json::to_string(&first).expect("serialize first payload");
        let second_bytes = serde_json::to_string(&second).expect("serialize second payload");

        assert_eq!(first_bytes, second_bytes);
        assert_eq!(first["cache_control"]["type"], "ephemeral");
        assert_eq!(first["messages"][0]["role"], "user");
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

    // ---- D3 Anthropic tool-calling tests ----

    /// A fixed-response Anthropic transport for tool-calling tests.
    #[derive(Debug, Clone)]
    struct FixedAnthropicTransport {
        response: Value,
        calls: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    }

    impl FixedAnthropicTransport {
        fn new(response: Value) -> Self {
            Self {
                response,
                calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Vec<Value> {
            self.calls.lock().expect("calls lock").clone()
        }
    }

    impl AnthropicMessagesTransport for FixedAnthropicTransport {
        fn post_json(
            &self,
            _endpoint: &str,
            _credential: Option<AnthropicCredential<'_>>,
            _beta_header: Option<&str>,
            payload: Value,
        ) -> Result<Value, ProviderError> {
            self.calls.lock().expect("calls lock").push(payload);
            Ok(self.response.clone())
        }

        fn post_text(
            &self,
            _endpoint: &str,
            _credential: Option<AnthropicCredential<'_>>,
            _beta_header: Option<&str>,
            payload: Value,
        ) -> Result<String, ProviderError> {
            self.calls.lock().expect("calls lock").push(payload);
            Err(ProviderError::RequestFailed {
                provider: "fixed".to_string(),
                message: "streaming not supported in FixedAnthropicTransport".to_string(),
            })
        }
    }

    #[test]
    fn extract_assistant_blocks_parses_tool_use_response() {
        let response = json!({
            "content": [
                {
                    "type": "tool_use",
                    "id": "tool-abc",
                    "name": "Read",
                    "input": { "path": "src/main.rs" }
                }
            ],
            "stop_reason": "tool_use"
        });
        let (blocks, stop_reason) =
            AnthropicMessagesClient::<FixedAnthropicTransport>::extract_assistant_blocks(&response)
                .expect("parsing succeeds");
        assert_eq!(stop_reason, ToolCompletionStopReason::ToolUse);
        assert_eq!(blocks.len(), 1);
        let ToolTurnBlock::ToolUse { id, name, input } = &blocks[0] else {
            panic!("expected ToolUse block, got {:?}", blocks[0]);
        };
        assert_eq!(id, "tool-abc");
        assert_eq!(name, "Read");
        assert_eq!(input, &json!({ "path": "src/main.rs" }));
    }

    #[test]
    fn serialize_tool_turn_produces_anthropic_wire_format() {
        use legion_ai::tool_calls::{ToolConversationTurn, ToolTurnBlock};

        // Assistant turn with ToolUse block.
        let assistant_turn = ToolConversationTurn {
            role: "assistant".to_string(),
            blocks: vec![ToolTurnBlock::ToolUse {
                id: "tool-1".to_string(),
                name: "Read".to_string(),
                input: json!({ "path": "foo.rs" }),
            }],
        };
        let serialized = serialize_tool_turn(&assistant_turn);
        assert_eq!(serialized["role"], "assistant");
        assert_eq!(serialized["content"][0]["type"], "tool_use");
        assert_eq!(serialized["content"][0]["id"], "tool-1");
        assert_eq!(serialized["content"][0]["name"], "Read");
        assert_eq!(serialized["content"][0]["input"]["path"], "foo.rs");

        // User turn with ToolResult block.
        let user_turn = ToolConversationTurn {
            role: "user".to_string(),
            blocks: vec![ToolTurnBlock::ToolResult {
                tool_use_id: "tool-1".to_string(),
                content: "fn main() {}".to_string(),
                is_error: false,
            }],
        };
        let serialized = serialize_tool_turn(&user_turn);
        assert_eq!(serialized["role"], "user");
        assert_eq!(serialized["content"][0]["type"], "tool_result");
        assert_eq!(serialized["content"][0]["tool_use_id"], "tool-1");
        assert_eq!(serialized["content"][0]["content"], "fn main() {}");
        assert_eq!(serialized["content"][0]["is_error"], false);

        // User turn with Text block.
        let text_turn = ToolConversationTurn {
            role: "user".to_string(),
            blocks: vec![ToolTurnBlock::Text("hello".to_string())],
        };
        let serialized = serialize_tool_turn(&text_turn);
        assert_eq!(serialized["role"], "user");
        assert_eq!(serialized["content"][0]["type"], "text");
        assert_eq!(serialized["content"][0]["text"], "hello");
    }

    #[test]
    fn anthropic_complete_with_tools_end_to_end_with_fixed_transport() {
        use legion_ai::tool_calls::{ToolCallingProvider, ToolCompletionRequest, ToolDefinition};

        let tool_use_response = json!({
            "content": [
                {
                    "type": "tool_use",
                    "id": "tu-001",
                    "name": "Read",
                    "input": { "path": "Cargo.toml" }
                }
            ],
            "stop_reason": "tool_use"
        });

        let transport = FixedAnthropicTransport::new(tool_use_response);
        let client = AnthropicMessagesClient::with_transport(
            "anthropic",
            "https://api.anthropic.com",
            Some("test-key".to_string()),
            transport.clone(),
        );

        let request = ToolCompletionRequest {
            provider: "anthropic".to_string(),
            model: "claude-opus-4-5".to_string(),
            system: "You are a helpful assistant.".to_string(),
            turns: vec![],
            tools: vec![ToolDefinition {
                name: "Read".to_string(),
                description: "Read a file".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }),
            }],
            max_tokens: 1024,
        };

        let response = client
            .complete_with_tools(request)
            .expect("complete_with_tools succeeds");

        assert_eq!(response.stop_reason, ToolCompletionStopReason::ToolUse);
        assert_eq!(response.blocks.len(), 1);
        let ToolTurnBlock::ToolUse { id, name, .. } = &response.blocks[0] else {
            panic!("expected ToolUse block");
        };
        assert_eq!(id, "tu-001");
        assert_eq!(name, "Read");

        // Verify the payload sent to the transport contained the tool definitions.
        let calls = transport.calls();
        assert_eq!(calls.len(), 1);
        let payload = &calls[0];
        assert!(payload.get("tools").is_some(), "payload must include tools");
        assert_eq!(payload["tools"][0]["name"], "Read");
        assert_eq!(payload["system"], "You are a helpful assistant.");
    }

    #[test]
    fn anthropic_tool_calling_live_smoke() {
        use legion_ai::tool_calls::{ToolCallingProvider, ToolCompletionRequest, ToolDefinition};

        let api_key = match std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
        {
            Some(key) => key,
            None => {
                println!(
                    "SKIP: ANTHROPIC_API_KEY is not set — skipping live Anthropic tool-calling smoke test"
                );
                return;
            }
        };

        let client = AnthropicMessagesClient::with_transport(
            "anthropic-live",
            "https://api.anthropic.com",
            Some(api_key),
            ReqwestProviderHttpTransport,
        );
        let request = ToolCompletionRequest {
            provider: "anthropic-live".to_string(),
            model: "claude-haiku-4-5".to_string(),
            system: "Use the get_weather tool to answer the user's question.".to_string(),
            turns: vec![legion_ai::tool_calls::ToolConversationTurn {
                role: "user".to_string(),
                blocks: vec![legion_ai::tool_calls::ToolTurnBlock::Text(
                    "What is the weather in London?".to_string(),
                )],
            }],
            tools: vec![ToolDefinition {
                name: "get_weather".to_string(),
                description: "Get the current weather for a location".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "location": { "type": "string", "description": "City name" }
                    },
                    "required": ["location"]
                }),
            }],
            max_tokens: 256,
        };

        let response = client
            .complete_with_tools(request)
            .expect("live tool-calling smoke test succeeds");
        assert!(
            !response.blocks.is_empty(),
            "live response must contain at least one block"
        );
        println!(
            "Live smoke test passed: stop_reason={:?}, blocks={}",
            response.stop_reason,
            response.blocks.len()
        );
    }

    // ---- D4: OpenAI tool-calling tests ----

    /// Fixed-response `ProviderHttpTransport` for OpenAI tool-calling unit tests.
    #[derive(Debug, Clone)]
    struct FixedOpenAiTransport {
        response: Value,
        calls: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    }

    impl FixedOpenAiTransport {
        fn new(response: Value) -> Self {
            Self {
                response,
                calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }
        fn calls(&self) -> Vec<Value> {
            self.calls.lock().expect("calls lock").clone()
        }
    }

    impl ProviderHttpTransport for FixedOpenAiTransport {
        fn post_json(
            &self,
            _endpoint: &str,
            _bearer_token: Option<&str>,
            payload: Value,
        ) -> Result<Value, ProviderError> {
            self.calls.lock().expect("calls lock").push(payload);
            Ok(self.response.clone())
        }
    }

    /// Build a provider backed by a fixed OpenAI-format response.
    fn openai_tool_provider(response: Value) -> OpenAiCompatibleProvider<FixedOpenAiTransport> {
        OpenAiCompatibleProvider::with_transport(
            "openai-test",
            "https://api.openai.com/v1",
            Some("test-key".to_string()),
            FixedOpenAiTransport::new(response),
        )
    }

    /// Minimal single-user-turn request for finish-reason and similar simple tests.
    fn simple_openai_request(model: &str) -> ToolCompletionRequest {
        ToolCompletionRequest {
            provider: "openai-test".to_string(),
            model: model.to_string(),
            system: String::new(),
            turns: vec![ToolConversationTurn {
                role: "user".to_string(),
                blocks: vec![ToolTurnBlock::Text("test".to_string())],
            }],
            tools: vec![],
            max_tokens: 64,
        }
    }

    // --- Serialization ---

    #[test]
    fn openai_tool_calling_serializes_request_correctly() {
        let response = json!({
            "choices": [{ "message": { "content": "ok" }, "finish_reason": "stop" }]
        });
        let transport = FixedOpenAiTransport::new(response);
        let provider = OpenAiCompatibleProvider::with_transport(
            "openai-test",
            "https://api.openai.com/v1",
            Some("test-key".to_string()),
            transport.clone(),
        );

        let request = ToolCompletionRequest {
            provider: "openai-test".to_string(),
            model: "gpt-4o-mini".to_string(),
            system: "You are a helpful assistant.".to_string(),
            turns: vec![ToolConversationTurn {
                role: "user".to_string(),
                blocks: vec![ToolTurnBlock::Text(
                    "What is the weather in London?".to_string(),
                )],
            }],
            tools: vec![ToolDefinition {
                name: "get_weather".to_string(),
                description: "Get the current weather".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": { "location": { "type": "string" } },
                    "required": ["location"]
                }),
            }],
            max_tokens: 256,
        };
        provider.complete_with_tools(request).expect("completes");

        let calls = transport.calls();
        assert_eq!(calls.len(), 1);
        let payload = &calls[0];

        // System message must be first.
        assert_eq!(payload["messages"][0]["role"], "system");
        assert_eq!(
            payload["messages"][0]["content"],
            "You are a helpful assistant."
        );
        // User message follows.
        assert_eq!(payload["messages"][1]["role"], "user");
        assert_eq!(
            payload["messages"][1]["content"],
            "What is the weather in London?"
        );
        // Tools in OpenAI function format.
        assert_eq!(payload["tools"][0]["type"], "function");
        assert_eq!(payload["tools"][0]["function"]["name"], "get_weather");
        assert!(payload["tools"][0]["function"]["parameters"].is_object());
        // Model and max_tokens present.
        assert_eq!(payload["model"], "gpt-4o-mini");
        assert_eq!(payload["max_tokens"], 256);
    }

    // --- Deserialization ---

    #[test]
    fn openai_tool_calling_parses_tool_use_response() {
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call-abc",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\": \"London\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let provider = openai_tool_provider(response);
        let resp = provider
            .complete_with_tools(simple_openai_request("gpt-4o-mini"))
            .expect("parses OK");

        assert_eq!(resp.stop_reason, ToolCompletionStopReason::ToolUse);
        assert_eq!(resp.blocks.len(), 1);
        let ToolTurnBlock::ToolUse { id, name, input } = &resp.blocks[0] else {
            panic!("expected ToolUse block, got {:?}", resp.blocks[0]);
        };
        assert_eq!(id, "call-abc");
        assert_eq!(name, "get_weather");
        assert_eq!(input["location"], "London");
    }

    // --- tool_call_id round-trip ---

    #[test]
    fn openai_tool_call_id_round_trips_as_tool_message() {
        let transport = FixedOpenAiTransport::new(json!({
            "choices": [{ "message": { "content": "done" }, "finish_reason": "stop" }]
        }));
        let provider = OpenAiCompatibleProvider::with_transport(
            "openai-test",
            "https://api.openai.com/v1",
            Some("test-key".to_string()),
            transport.clone(),
        );

        // Feed back a ToolResult whose tool_use_id matches the previous tool_call id.
        provider
            .complete_with_tools(ToolCompletionRequest {
                provider: "openai-test".to_string(),
                model: "gpt-4o-mini".to_string(),
                system: String::new(),
                turns: vec![
                    ToolConversationTurn {
                        role: "user".to_string(),
                        blocks: vec![ToolTurnBlock::Text("do it".to_string())],
                    },
                    ToolConversationTurn {
                        role: "assistant".to_string(),
                        blocks: vec![ToolTurnBlock::ToolUse {
                            id: "call-xyz".to_string(),
                            name: "read".to_string(),
                            input: json!({"path": "foo.rs"}),
                        }],
                    },
                    ToolConversationTurn {
                        role: "user".to_string(),
                        blocks: vec![ToolTurnBlock::ToolResult {
                            tool_use_id: "call-xyz".to_string(),
                            content: "file content here".to_string(),
                            is_error: false,
                        }],
                    },
                ],
                tools: vec![],
                max_tokens: 256,
            })
            .expect("round-trip succeeds");

        let calls = transport.calls();
        let messages = calls[0]["messages"].as_array().expect("messages is array");

        // Tool result message must have tool_call_id matching the assistant's id.
        let tool_msg = messages
            .iter()
            .find(|m| m["role"] == "tool")
            .expect("must have a role:tool message");
        assert_eq!(tool_msg["tool_call_id"], "call-xyz");
        assert_eq!(tool_msg["content"], "file content here");

        // Assistant message must have the tool_calls array with matching id.
        let assistant_msg = messages
            .iter()
            .find(|m| m["role"] == "assistant")
            .expect("must have role:assistant message");
        assert_eq!(assistant_msg["tool_calls"][0]["id"], "call-xyz");
        assert_eq!(assistant_msg["tool_calls"][0]["function"]["name"], "read");
    }

    // --- Multi-tool_call turn ---

    #[test]
    fn openai_tool_calling_multi_tool_call_parsed() {
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        { "id": "call-1", "type": "function", "function": { "name": "read",  "arguments": "{\"path\": \"a.rs\"}" } },
                        { "id": "call-2", "type": "function", "function": { "name": "grep", "arguments": "{\"pattern\": \"fn main\"}" } }
                    ]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let provider = openai_tool_provider(response);
        let resp = provider
            .complete_with_tools(simple_openai_request("gpt-4o"))
            .expect("parses");

        assert_eq!(resp.stop_reason, ToolCompletionStopReason::ToolUse);
        assert_eq!(resp.blocks.len(), 2, "expected 2 ToolUse blocks");

        let ToolTurnBlock::ToolUse {
            id: id1,
            name: name1,
            ..
        } = &resp.blocks[0]
        else {
            panic!("block 0 not ToolUse");
        };
        let ToolTurnBlock::ToolUse {
            id: id2,
            name: name2,
            ..
        } = &resp.blocks[1]
        else {
            panic!("block 1 not ToolUse");
        };
        assert_eq!(id1, "call-1");
        assert_eq!(name1, "read");
        assert_eq!(id2, "call-2");
        assert_eq!(name2, "grep");
    }

    // --- Tool-message ordering (the 400 trap) ---

    #[test]
    fn openai_tool_result_messages_appear_immediately_after_assistant_tool_calls() {
        let transport = FixedOpenAiTransport::new(json!({
            "choices": [{ "message": { "content": "ok" }, "finish_reason": "stop" }]
        }));
        let provider = OpenAiCompatibleProvider::with_transport(
            "openai-test",
            "https://api.openai.com/v1",
            Some("test-key".to_string()),
            transport.clone(),
        );

        // Two-round tool call conversation:
        // user → assistant(c1) → tool(c1) → assistant(c2) → tool(c2)
        provider
            .complete_with_tools(ToolCompletionRequest {
                provider: "openai-test".to_string(),
                model: "gpt-4o-mini".to_string(),
                system: String::new(),
                turns: vec![
                    ToolConversationTurn {
                        role: "user".to_string(),
                        blocks: vec![ToolTurnBlock::Text("start".to_string())],
                    },
                    ToolConversationTurn {
                        role: "assistant".to_string(),
                        blocks: vec![ToolTurnBlock::ToolUse {
                            id: "c1".to_string(),
                            name: "read".to_string(),
                            input: json!({"path": "a"}),
                        }],
                    },
                    ToolConversationTurn {
                        role: "user".to_string(),
                        blocks: vec![ToolTurnBlock::ToolResult {
                            tool_use_id: "c1".to_string(),
                            content: "result1".to_string(),
                            is_error: false,
                        }],
                    },
                    ToolConversationTurn {
                        role: "assistant".to_string(),
                        blocks: vec![ToolTurnBlock::ToolUse {
                            id: "c2".to_string(),
                            name: "grep".to_string(),
                            input: json!({"pattern": "fn"}),
                        }],
                    },
                    ToolConversationTurn {
                        role: "user".to_string(),
                        blocks: vec![ToolTurnBlock::ToolResult {
                            tool_use_id: "c2".to_string(),
                            content: "result2".to_string(),
                            is_error: false,
                        }],
                    },
                ],
                tools: vec![],
                max_tokens: 256,
            })
            .expect("round-trip succeeds");

        let calls = transport.calls();
        let messages = calls[0]["messages"].as_array().expect("messages array");
        // Expected wire order: user, assistant(c1), tool(c1), assistant(c2), tool(c2)
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "c1");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "c1");
        assert_eq!(messages[3]["role"], "assistant");
        assert_eq!(messages[3]["tool_calls"][0]["id"], "c2");
        assert_eq!(messages[4]["role"], "tool");
        assert_eq!(messages[4]["tool_call_id"], "c2");
    }

    // --- Finish reason mapping ---

    #[test]
    fn openai_finish_reason_stop_maps_to_end_turn() {
        let r = json!({"choices":[{"message":{"content":"done"},"finish_reason":"stop"}]});
        let resp = openai_tool_provider(r)
            .complete_with_tools(simple_openai_request("gpt-4o-mini"))
            .unwrap();
        assert_eq!(resp.stop_reason, ToolCompletionStopReason::EndTurn);
    }

    #[test]
    fn openai_finish_reason_length_maps_to_max_tokens() {
        let r = json!({"choices":[{"message":{"content":"done"},"finish_reason":"length"}]});
        let resp = openai_tool_provider(r)
            .complete_with_tools(simple_openai_request("gpt-4o-mini"))
            .unwrap();
        assert_eq!(resp.stop_reason, ToolCompletionStopReason::MaxTokens);
    }

    #[test]
    fn openai_finish_reason_tool_calls_maps_to_tool_use() {
        let r = json!({
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{"id":"x","type":"function","function":{"name":"f","arguments":"{}"}}]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let resp = openai_tool_provider(r)
            .complete_with_tools(simple_openai_request("gpt-4o-mini"))
            .unwrap();
        assert_eq!(resp.stop_reason, ToolCompletionStopReason::ToolUse);
    }

    // --- Malformed arguments → hard error ---

    #[test]
    fn openai_malformed_arguments_returns_provider_error() {
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "c1",
                        "type": "function",
                        "function": { "name": "read", "arguments": "not valid json {{{" }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let err = openai_tool_provider(response)
            .complete_with_tools(simple_openai_request("gpt-4o-mini"))
            .expect_err("malformed JSON must fail");

        assert!(
            matches!(err, ProviderError::RequestFailed { .. }),
            "expected RequestFailed, got {err:?}"
        );
        let msg = format!("{err}");
        assert!(
            msg.contains("not valid JSON") || msg.contains("arguments"),
            "error message should describe the JSON parse failure: {msg}"
        );
    }

    // --- is_error → "ERROR: " prefix ---

    #[test]
    fn openai_error_tool_result_gets_error_prefix() {
        let transport = FixedOpenAiTransport::new(json!({
            "choices": [{ "message": { "content": "noted" }, "finish_reason": "stop" }]
        }));
        let provider = OpenAiCompatibleProvider::with_transport(
            "openai-test",
            "https://api.openai.com/v1",
            Some("test-key".to_string()),
            transport.clone(),
        );

        provider
            .complete_with_tools(ToolCompletionRequest {
                provider: "openai-test".to_string(),
                model: "gpt-4o-mini".to_string(),
                system: String::new(),
                turns: vec![
                    ToolConversationTurn {
                        role: "assistant".to_string(),
                        blocks: vec![ToolTurnBlock::ToolUse {
                            id: "c1".to_string(),
                            name: "read".to_string(),
                            input: json!({"path": "x"}),
                        }],
                    },
                    ToolConversationTurn {
                        role: "user".to_string(),
                        blocks: vec![ToolTurnBlock::ToolResult {
                            tool_use_id: "c1".to_string(),
                            content: "file not found".to_string(),
                            is_error: true,
                        }],
                    },
                ],
                tools: vec![],
                max_tokens: 64,
            })
            .expect("ok");

        let calls = transport.calls();
        let messages = calls[0]["messages"].as_array().unwrap();
        let tool_msg = messages
            .iter()
            .find(|m| m["role"] == "tool")
            .expect("tool message");
        assert_eq!(
            tool_msg["content"], "ERROR: file not found",
            "is_error:true result must be prefixed with 'ERROR: '"
        );
    }

    // --- capabilities() advertises tool_use ---

    #[test]
    fn openai_compatible_capabilities_has_tool_use() {
        let provider = OpenAiCompatibleProvider::with_transport(
            "openai-test",
            "https://api.openai.com/v1",
            Some("test-key".to_string()),
            RecordingProviderTransport::default(),
        );
        assert!(
            provider.capabilities().tool_use,
            "OpenAiCompatibleProvider must advertise tool_use capability"
        );
    }

    // --- Live smoke test (optional, gated on OPENAI_API_KEY) ---

    #[test]
    fn openai_tool_calling_live_smoke() {
        let api_key = match std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
        {
            Some(key) => key,
            None => {
                println!(
                    "SKIP: OPENAI_API_KEY is not set — skipping live OpenAI tool-calling smoke test"
                );
                return;
            }
        };

        let model = std::env::var("LEGION_SMOKE_OPENAI_MODEL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "gpt-4o-mini".to_string());

        let provider = OpenAiCompatibleProvider::with_transport(
            "openai-live",
            "https://api.openai.com/v1",
            Some(api_key),
            ReqwestProviderHttpTransport,
        );

        let weather_tool = ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get the current weather for a location".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "location": { "type": "string", "description": "City name" }
                },
                "required": ["location"]
            }),
        };

        // Round 1: model should call get_weather.
        let request = ToolCompletionRequest {
            provider: "openai-live".to_string(),
            model: model.clone(),
            system: "Use the get_weather tool to answer the user's question.".to_string(),
            turns: vec![ToolConversationTurn {
                role: "user".to_string(),
                blocks: vec![ToolTurnBlock::Text(
                    "What is the weather in London?".to_string(),
                )],
            }],
            tools: vec![weather_tool.clone()],
            max_tokens: 256,
        };
        let response = provider
            .complete_with_tools(request)
            .expect("live tool-calling smoke test succeeds");

        assert!(
            !response.blocks.is_empty(),
            "live response must contain at least one block"
        );
        assert_eq!(
            response.stop_reason,
            ToolCompletionStopReason::ToolUse,
            "expected ToolUse stop reason"
        );
        let ToolTurnBlock::ToolUse { id, .. } = &response.blocks[0] else {
            panic!("expected ToolUse block from live response");
        };
        let tool_call_id = id.clone();

        // Round 2: feed back a result, expect EndTurn.
        let final_request = ToolCompletionRequest {
            provider: "openai-live".to_string(),
            model: model.clone(),
            system: "Use the get_weather tool to answer the user's question.".to_string(),
            turns: vec![
                ToolConversationTurn {
                    role: "user".to_string(),
                    blocks: vec![ToolTurnBlock::Text(
                        "What is the weather in London?".to_string(),
                    )],
                },
                ToolConversationTurn {
                    role: "assistant".to_string(),
                    blocks: response.blocks.clone(),
                },
                ToolConversationTurn {
                    role: "user".to_string(),
                    blocks: vec![ToolTurnBlock::ToolResult {
                        tool_use_id: tool_call_id,
                        content: "Sunny, 22°C".to_string(),
                        is_error: false,
                    }],
                },
            ],
            tools: vec![weather_tool],
            max_tokens: 256,
        };
        let final_response = provider
            .complete_with_tools(final_request)
            .expect("second live call succeeds");

        assert_eq!(
            final_response.stop_reason,
            ToolCompletionStopReason::EndTurn,
            "final response should be EndTurn after tool result"
        );
        println!(
            "OpenAI live smoke test passed: model={model}, blocks={}, stop_reason={:?}",
            final_response.blocks.len(),
            final_response.stop_reason
        );
    }
}
