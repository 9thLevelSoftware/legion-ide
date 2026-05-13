//! AI Orchestrator: prompt assembly, context selection, model request abstraction.

#![warn(missing_docs)]

use std::collections::HashMap;

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
