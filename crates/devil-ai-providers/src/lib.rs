//! Provider adapters: Ollama, llama.cpp, OpenAI, Anthropic, future gateway.

#![warn(missing_docs)]

use devil_ai::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
    ModelProvider, ProviderCapabilities, ProviderError, ProviderId,
};

/// Known stub adapter names used by milestone scaffolding.
pub const OLLAMA_STUB_ID: &str = "ollama-stub";
/// Known stub adapter names used by milestone scaffolding.
pub const OPENAI_STUB_ID: &str = "openai-stub";

/// Lightweight provider-agnostic adapter constructor for milestone scaffolding.
pub fn make_stub_registry() -> devil_ai::ProviderRegistry {
    let mut registry = devil_ai::ProviderRegistry::new();
    registry.register(Box::new(OllamaStub::new(OLLAMA_STUB_ID)));
    registry.register(Box::new(OpenAiStub::new(OPENAI_STUB_ID)));
    registry
}

macro_rules! stub_provider_impl {
    ($name:ident, $capability:expr) => {
        /// Stub provider implementation placeholder.
        pub struct $name {
            id: ProviderId,
        }

        impl $name {
            /// Creates the stub provider instance.
            pub fn new(id: impl Into<ProviderId>) -> Self {
                Self { id: id.into() }
            }

            fn stub_completion_error(&self, message: impl Into<String>) -> ProviderError {
                ProviderError::RequestRejected {
                    message: format!("{0}: {1}", self.id, message.into()),
                }
            }

            fn stub_embedding_error(&self, message: impl Into<String>) -> ProviderError {
                ProviderError::RequestRejected {
                    message: format!("{0}: {1}", self.id, message.into()),
                }
            }
        }

        impl ModelProvider for $name {
            fn provider_id(&self) -> ProviderId {
                self.id.clone()
            }

            fn capabilities(&self) -> ProviderCapabilities {
                $capability
            }

            fn complete(
                &self,
                _request: ChatCompletionRequest,
            ) -> Result<ChatCompletionResponse, ProviderError> {
                Err(self.stub_completion_error("provider not implemented in milestone scope"))
            }

            fn embed(
                &self,
                _request: EmbeddingRequest,
            ) -> Result<EmbeddingResponse, ProviderError> {
                Err(self.stub_embedding_error("provider not implemented in milestone scope"))
            }
        }
    };
}

stub_provider_impl!(
    OllamaStub,
    ProviderCapabilities {
        completion: true,
        embedding: false,
    }
);

stub_provider_impl!(
    OpenAiStub,
    ProviderCapabilities {
        completion: true,
        embedding: false,
    }
);
