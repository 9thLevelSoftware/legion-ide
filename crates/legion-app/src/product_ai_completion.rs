//! Live product-AI completions: which backend answers, and what it returns.
//!
//! Moved out of `lib.rs`, which is a chokepoint file with a line budget, under
//! the roadmap's extract-before-modify rule. A self-contained region: given an
//! already-resolved backend, produce a chat completion, an Assist edit proposal,
//! or a Delegate chat reply — and a fixture label when there is no live backend.
//!
//! Every entry point here takes `Option<ProductAiLiveBackend>` rather than a
//! `ProductAiProviderPreference`, and that is load-bearing rather than
//! stylistic. Resolving a preference probes the host, so a second resolution can
//! answer differently from the one the broker authorized — on `Auto`, an
//! authorized Ollama route could send a buffer excerpt to Anthropic instead.
//! Taking the backend leaves nothing here that could re-resolve.

use super::*;

/// Product completion bound to the **authorized** live backend only.
///
/// Auto selects Ollama when loopback is reachable, otherwise Anthropic BYOK when a
/// key exists. Completion never falls through from an Ollama-authorized route to
/// Anthropic (or the reverse): that would bypass the capability/network decision
/// built for the selected backend. Offline / no-provider returns `None` for
/// fixture fallbacks.
///
/// Anthropic uses progressive Messages **SSE** when available (`on_delta` fires as
/// chunks arrive). Ollama remains a single-chunk completion.
#[cfg(feature = "ai")]
pub(crate) fn complete_product_chat(
    backend: Option<ProductAiLiveBackend>,
    system: &str,
    user: &str,
    max_tokens: u32,
    temperature: f32,
    mut on_delta: Option<&mut dyn FnMut(&str)>,
) -> Option<ProductChatCompletion> {
    use legion_ai::{ChatCompletionRequest, ChatMessage, ChatRole, ModelProvider};
    use legion_ai_providers::OllamaProvider;

    // The backend is passed in, already resolved and already authorized. This
    // used to take the preference and resolve it again, which is a second answer
    // to a question the broker had already been asked -- and on `Auto` the two
    // answers can differ.
    let backend = backend?;

    match backend {
        ProductAiLiveBackend::Ollama => {
            let model = ollama_model_label();
            let client = OllamaProvider::default();
            let request = ChatCompletionRequest {
                provider: "ollama".to_string(),
                model: model.clone(),
                messages: vec![
                    ChatMessage {
                        role: ChatRole::System,
                        content: system.to_string(),
                    },
                    ChatMessage {
                        role: ChatRole::User,
                        content: user.to_string(),
                    },
                ],
                max_tokens: Some(max_tokens),
                temperature: Some(temperature),
                metadata: Default::default(),
            };
            if let Ok(response) = client.complete(request)
                && !response.text.trim().is_empty()
            {
                let text = response.text.trim().to_string();
                if let Some(cb) = on_delta.as_mut() {
                    cb(&text);
                }
                return Some(ProductChatCompletion {
                    provider_id: "ollama".to_string(),
                    model: response.model,
                    stream_chunks: vec![text.clone()],
                    text,
                    streamed: false,
                });
            }
            None
        }
        ProductAiLiveBackend::Anthropic => {
            let client = anthropic_client_with_keyring_fallback();
            let request = ChatCompletionRequest {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
                messages: vec![
                    ChatMessage {
                        role: ChatRole::System,
                        content: system.to_string(),
                    },
                    ChatMessage {
                        role: ChatRole::User,
                        content: user.to_string(),
                    },
                ],
                max_tokens: Some(max_tokens),
                temperature: Some(temperature),
                metadata: Default::default(),
            };
            // Progressive SSE: on_delta fires as text deltas arrive on the wire.
            let mut delta_sink = |text: &str| {
                if let Some(cb) = on_delta.as_mut() {
                    cb(text);
                }
            };
            if let Ok(chunks) = client.stream_text_deltas_with_callback(
                request.clone(),
                Default::default(),
                &mut delta_sink,
            ) {
                let chunks: Vec<String> = chunks.into_iter().filter(|d| !d.is_empty()).collect();
                let text = chunks.join("");
                if !text.trim().is_empty() {
                    let streamed = chunks.len() > 1;
                    return Some(ProductChatCompletion {
                        provider_id: "anthropic".to_string(),
                        model: request.model.clone(),
                        stream_chunks: if chunks.is_empty() {
                            vec![text.clone()]
                        } else {
                            chunks
                        },
                        text: text.trim().to_string(),
                        streamed,
                    });
                }
            }
            if let Ok(response) = client.complete(request)
                && !response.text.trim().is_empty()
            {
                let text = response.text.trim().to_string();
                if let Some(cb) = on_delta.as_mut() {
                    cb(&text);
                }
                return Some(ProductChatCompletion {
                    provider_id: "anthropic".to_string(),
                    model: response.model,
                    stream_chunks: vec![text.clone()],
                    text,
                    streamed: false,
                });
            }
            None
        }
    }
}

/// Result of resolving assist edit proposal body text (live model or fixture).
#[derive(Debug, Clone)]
pub(crate) struct AssistedEditProposalSource {
    pub(crate) provider_id: String,
    pub(crate) summary: String,
    pub(crate) details: Vec<String>,
    pub(crate) replacement: String,
}

pub(crate) fn deterministic_assisted_edit_proposal() -> AssistedEditProposalSource {
    AssistedEditProposalSource {
        provider_id: DETERMINISTIC_LOCAL_PROVIDER_ID.to_string(),
        summary: "Phase 4 local AI edit proposal".to_string(),
        details: vec![
            "Generated by deterministic local provider (no live credentials)".to_string(),
            "Proposal is registered only; app/editor/workspace own apply".to_string(),
        ],
        replacement: "/* phase4 local AI proposal */\n".to_string(),
    }
}

/// Display-safe record of the last / live product AI stream for rail projection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductAiStreamProjection {
    /// Provider that produced the stream (`ollama`, `anthropic`, or empty).
    pub provider_id: String,
    /// Model label.
    pub model: String,
    /// Operation that produced the stream (`assist.proposal`, `delegate.chat`, …).
    pub operation: String,
    /// Ordered stream chunks (SSE deltas or single full response).
    pub chunks: Vec<String>,
    /// Whether the provider used multi-delta streaming.
    pub streamed: bool,
    /// True while a background or progressive stream is still receiving deltas.
    pub in_flight: bool,
    /// Final accumulated text (bounded for projection rows).
    pub text_preview: String,
}

/// Resolve assist edit text via product preference routing (Ollama / Anthropic / fixture).
#[cfg(feature = "ai")]
pub(crate) fn resolve_assisted_edit_proposal_text(
    backend: Option<ProductAiLiveBackend>,
    instruction_label: &str,
    buffer_excerpt: &str,
    file_path: &str,
    on_delta: Option<&mut dyn FnMut(&str)>,
) -> (
    AssistedEditProposalSource,
    Option<ProductAiStreamProjection>,
) {
    let system = "You are Legion's Assist mode. Propose a small, reviewable code edit. \
Respond with ONLY the exact text to insert at the top of the file (as a comment or code), \
no markdown fences, no explanation.";
    let user = format!(
        "Instruction: {instruction_label}\nFile: {file_path}\n\nCurrent buffer (excerpt):\n{buffer_excerpt}"
    );
    // The backend the caller already authorized. Resolving again here is the
    // same defect closed in Delegate chat and inline prediction: the broker was
    // asked about one destination and a second probe can answer differently.
    match complete_product_chat(backend, system, &user, 512, 0.2, on_delta) {
        Some(completion) => {
            let mut text = completion.text.clone();
            if !text.ends_with('\n') {
                text.push('\n');
            }
            let stream = product_stream_from_completion(&completion, "assist.proposal");
            (
                AssistedEditProposalSource {
                    provider_id: completion.provider_id.clone(),
                    summary: format!("Assist edit proposal from {}", completion.provider_id),
                    details: vec![
                        format!("model={}", completion.model),
                        // The backend that actually answered, not the
                        // preference that suggested it. On `Auto` they differ,
                        // and this line is metadata on a proposal a person will
                        // review.
                        format!(
                            "backend={}",
                            match backend {
                                Some(ProductAiLiveBackend::Ollama) => "ollama",
                                Some(ProductAiLiveBackend::Anthropic) => "anthropic",
                                None => "none",
                            }
                        ),
                        format!(
                            "streamed={} chunks={}",
                            completion.streamed,
                            completion.stream_chunks.len()
                        ),
                        "Proposal is registered only; app/editor/workspace own apply".to_string(),
                    ],
                    replacement: text,
                },
                Some(stream),
            )
        }
        None => (deterministic_assisted_edit_proposal(), None),
    }
}

#[cfg(not(feature = "ai"))]
pub(crate) fn resolve_assisted_edit_proposal_text(
    _backend: Option<ProductAiLiveBackend>,
    _instruction_label: &str,
    _buffer_excerpt: &str,
    _file_path: &str,
    _on_delta: Option<&mut dyn FnMut(&str)>,
) -> (
    AssistedEditProposalSource,
    Option<ProductAiStreamProjection>,
) {
    (deterministic_assisted_edit_proposal(), None)
}

/// Resolve Delegate chat assistant body via product preference routing.
#[cfg(feature = "ai")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_delegate_chat_reply(
    backend: Option<ProductAiLiveBackend>,
    prompt_label: &str,
    buffer_excerpt: &str,
    file_path: &str,
    citation_count: usize,
    route_id: &str,
    route_labels: &[String],
    on_delta: Option<&mut dyn FnMut(&str)>,
) -> (String, Option<ProductAiStreamProjection>) {
    let system = "You are Legion's Delegate chat assistant. Answer helpfully and concisely \
about the user's workspace code. Prefer concrete references to the cited file. \
Do not invent file paths. Keep the reply under ~800 characters.";
    let user = format!(
        "Question: {prompt_label}\nFile: {file_path}\nCitations available: {citation_count}\n\nBuffer excerpt:\n{buffer_excerpt}"
    );
    match complete_product_chat(backend, system, &user, 512, 0.2, on_delta) {
        Some(completion) => {
            let stream = product_stream_from_completion(&completion, "delegate.chat");
            (bounded_label(completion.text, 1_200), Some(stream))
        }
        None => (
            format!(
                "Delegate provider answer ready via {citation_count} citation(s); route={route_id} labels={} (backend={}; fixture — enable Ollama loopback or Anthropic BYOK for a live reply)",
                route_labels.join(","),
                match backend {
                    Some(ProductAiLiveBackend::Ollama) => "ollama",
                    Some(ProductAiLiveBackend::Anthropic) => "anthropic",
                    None => "none",
                }
            ),
            None,
        ),
    }
}

#[cfg(not(feature = "ai"))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_delegate_chat_reply(
    _backend: Option<ProductAiLiveBackend>,
    _prompt_label: &str,
    _buffer_excerpt: &str,
    _file_path: &str,
    citation_count: usize,
    route_id: &str,
    route_labels: &[String],
    _on_delta: Option<&mut dyn FnMut(&str)>,
) -> (String, Option<ProductAiStreamProjection>) {
    (
        format!(
            "Delegate provider answer ready via {citation_count} citation(s); route={route_id} labels={}",
            route_labels.join(",")
        ),
        None,
    )
}

pub(crate) fn product_stream_from_completion(
    completion: &ProductChatCompletion,
    operation: &str,
) -> ProductAiStreamProjection {
    ProductAiStreamProjection {
        provider_id: completion.provider_id.clone(),
        model: completion.model.clone(),
        operation: operation.to_string(),
        chunks: completion.stream_chunks.clone(),
        streamed: completion.streamed,
        in_flight: false,
        text_preview: bounded_label(completion.text.as_str(), 480),
    }
}

/// Whether product AI will attempt a live (non-fixture) completion for `preference`.
#[cfg(feature = "ai")]
pub(crate) fn product_ai_will_attempt_live(preference: ProductAiProviderPreference) -> bool {
    product_ai_selected_live_backend(preference).is_some()
}

#[cfg(not(feature = "ai"))]
pub(crate) fn product_ai_will_attempt_live(_preference: ProductAiProviderPreference) -> bool {
    false
}
