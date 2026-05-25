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
/// Deterministic local provider id used by Phase 4 contract tests.
pub const DETERMINISTIC_LOCAL_PROVIDER_ID: &str = "deterministic-local";

/// Lightweight provider-agnostic adapter constructor for milestone scaffolding.
pub fn make_stub_registry() -> devil_ai::ProviderRegistry {
    let mut registry = devil_ai::ProviderRegistry::new();
    registry.register(Box::new(OllamaStub::new(OLLAMA_STUB_ID)));
    registry.register(Box::new(OpenAiStub::new(OPENAI_STUB_ID)));
    registry.register(Box::new(DeterministicLocalProvider::new(
        DETERMINISTIC_LOCAL_PROVIDER_ID,
    )));
    registry
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
            embedding: false,
        }
    }

    fn complete(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        Ok(ChatCompletionResponse {
            provider: self.id.clone(),
            model: request.model,
            text: "deterministic metadata-only completion".to_string(),
            metadata: [("redaction".to_string(), "metadata-only".to_string())]
                .into_iter()
                .collect(),
        })
    }

    fn embed(&self, _request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::RequestRejected {
            message: "embedding vectors remain deferred in Phase 4".to_string(),
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn deterministic_local_provider_keeps_embedding_vectors_deferred() {
        let provider = DeterministicLocalProvider::new(DETERMINISTIC_LOCAL_PROVIDER_ID);

        let error = provider
            .embed(EmbeddingRequest::new(
                DETERMINISTIC_LOCAL_PROVIDER_ID,
                "local-embedding",
                "input",
            ))
            .expect_err("embeddings remain deferred");

        assert!(matches!(
            error,
            ProviderError::RequestRejected { message } if message.contains("embedding vectors remain deferred")
        ));
    }

    #[test]
    fn cloud_provider_stub_remains_disabled_for_phase4() {
        let provider = OpenAiStub::new(OPENAI_STUB_ID);

        let completion_error = provider
            .complete(ChatCompletionRequest::new(
                OPENAI_STUB_ID,
                "hosted-model",
                "metadata-only request",
            ))
            .expect_err("OpenAI stub is not active");
        assert!(matches!(
            completion_error,
            ProviderError::RequestRejected { message } if message.contains("provider not implemented")
        ));

        let embedding_error = provider
            .embed(EmbeddingRequest::new(
                OPENAI_STUB_ID,
                "hosted-embedding",
                "input",
            ))
            .expect_err("hosted embeddings are not active");
        assert!(matches!(
            embedding_error,
            ProviderError::RequestRejected { message } if message.contains("provider not implemented")
        ));
    }

    #[test]
    fn stub_registry_exposes_only_stubbed_and_deterministic_local_adapters() {
        let registry = make_stub_registry();
        let mut ids = registry.provider_ids();
        ids.sort();

        assert_eq!(
            ids,
            vec![
                DETERMINISTIC_LOCAL_PROVIDER_ID.to_string(),
                OLLAMA_STUB_ID.to_string(),
                OPENAI_STUB_ID.to_string(),
            ]
        );
    }
}
