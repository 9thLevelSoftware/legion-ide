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

/// Completion tokens a product chat completion may return.
///
/// Named because it is also *declared* to the capability broker: an org policy
/// bundle can cap tokens per request, and a request that declares none is never
/// compared against that cap at all. A literal in one place and a declaration in
/// another would drift, and the drift would show up as a cap that silently
/// stopped applying.
///
/// Ungated deliberately. The declaration is made on every build; only the code
/// that would consume the tokens is behind `ai`.
pub(crate) const PRODUCT_COMPLETION_MAX_TOKENS: u32 = 512;

/// The wire name of a live backend.
///
/// One place, because this is the discriminator spelled out rather than data
/// about it -- and three call sites spelling it themselves is how one of them
/// ends up saying `claude` while the others say `anthropic`, in metadata a
/// person reads to decide whether to accept a proposal.
#[cfg(feature = "ai")]
pub(crate) fn live_backend_label(backend: ProductAiLiveBackend) -> &'static str {
    match backend {
        ProductAiLiveBackend::Ollama => "ollama",
        ProductAiLiveBackend::Anthropic => "anthropic",
    }
}

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
            //
            // Whether anything reached the surface is recorded, because it
            // decides whether a failure may be retried. A stream that emitted
            // deltas and then failed has already been generated and already
            // been paid for; sending the same prompt again bills a second
            // generation and replaces the partial text on screen with an
            // unrelated answer.
            let emitted_delta = std::cell::Cell::new(false);
            let mut delta_sink = |text: &str| {
                emitted_delta.set(true);
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
            // Only when the stream produced nothing at all.
            //
            // A failure before the first delta cost nothing and is worth
            // retrying without streaming. A failure after one is not a retry,
            // it is a second purchase.
            if emitted_delta.get() {
                return None;
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
    /// The text the model quoted, kept so uniqueness can be re-checked.
    ///
    /// Resolution runs against the excerpt, which is the only text the model
    /// saw. That makes the span right and the uniqueness claim incomplete: an
    /// anchor appearing once in the excerpt can appear again further down the
    /// file, and the prompt asked for text unique in the *file*. Empty when
    /// there is no edit to check.
    pub(crate) anchor: String,
    /// Byte span in the buffer this replacement occupies.
    ///
    /// `(0, 0)` is an insertion at the top of the file, which is all this
    /// path could express before: the model was asked for "the exact text to
    /// insert at the top of the file" and its answer was placed at
    /// `TextRange::byte(0, 0)`. Internally consistent and useless -- Assist
    /// could prepend a comment and nothing else, for the fixture, for Ollama
    /// and for Anthropic alike, which is the insertion path the roadmap says
    /// a real model never ships through.
    pub(crate) span: (usize, usize),
}

pub(crate) fn deterministic_assisted_edit_proposal() -> AssistedEditProposalSource {
    AssistedEditProposalSource {
        provider_id: DETERMINISTIC_LOCAL_PROVIDER_ID.to_string(),
        summary: "Phase 4 local AI edit proposal".to_string(),
        details: vec![
            "Generated by deterministic local provider (no live credentials)".to_string(),
            "Proposal is registered only; app/editor/workspace own apply".to_string(),
        ],
        anchor: String::new(),
        replacement: "/* phase4 local AI proposal */\n".to_string(),
        // The fixture keeps its insertion, and honestly so: it is a canned
        // comment, it says it is a canned comment, and prepending one is
        // exactly what it claims to do. Only a live model is held to
        // producing an edit that resolves.
        span: (0, 0),
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
    // A search/replace block, because that is the format the resolver already
    // reads and the one small local models were measured against. The anchor
    // has to be quoted from the file rather than described, which is what
    // makes the edit checkable: `resolve_edit_span` refuses anything it
    // cannot find exactly once.
    let system = "You are Legion's Assist mode. Propose one small, reviewable edit. \
Reply with exactly one search/replace block and nothing else:\n\n\
<<<<<<< SEARCH\n\
(the exact lines to replace, copied character-for-character from the file)\n\
=======\n\
(the replacement lines)\n\
>>>>>>> REPLACE\n\n\
The SEARCH text must appear exactly once in the file. \
No explanation, no second block.";
    let user = format!(
        "Instruction: {instruction_label}\nFile: {file_path}\n\nCurrent buffer (excerpt):\n{buffer_excerpt}"
    );
    // The backend the caller already authorized. Resolving again here is the
    // same defect closed in Delegate chat and inline prediction: the broker was
    // asked about one destination and a second probe can answer differently.
    match complete_product_chat(
        backend,
        system,
        &user,
        PRODUCT_COMPLETION_MAX_TOKENS,
        0.2,
        on_delta,
    ) {
        Some(completion) => {
            let stream = product_stream_from_completion(&completion, "assist.proposal");
            // Read once, in one match. Four accessors that all cloned out of
            // the same arm was the enum paying rent for something one
            // destructure does, and a `resolved` bool alongside it was paying
            // twice -- the match already knows which arm it took, and the
            // summary is the only thing that needed telling.
            let (span, anchor, replacement, summary, detail) =
                match resolve_assist_placement(buffer_excerpt, file_path, &completion.text) {
                    AssistPlacement::Resolved {
                        span,
                        anchor,
                        replacement,
                        outcome_label,
                    } => (
                        span,
                        anchor,
                        replacement,
                        format!("Assist edit proposal from {}", completion.provider_id),
                        format!("edit={outcome_label} bytes={}..{}", span.0, span.1),
                    ),
                    // An empty replacement over an empty span changes nothing,
                    // and `finish_assisted_edit_proposal_registration` declines
                    // to register it rather than offering an approvable no-op.
                    AssistPlacement::Unresolved { reason } => (
                        (0, 0),
                        String::new(),
                        String::new(),
                        format!(
                            "Assist edit from {} did not resolve",
                            completion.provider_id
                        ),
                        format!("edit=unresolved: {reason}"),
                    ),
                };
            (
                AssistedEditProposalSource {
                    provider_id: completion.provider_id.clone(),
                    summary,
                    details: vec![
                        format!("model={}", completion.model),
                        // The backend that actually answered, not the
                        // preference that suggested it. On `Auto` they differ,
                        // and this line is metadata on a proposal a person will
                        // review.
                        format!("backend={}", backend.map_or("none", live_backend_label)),
                        format!(
                            "streamed={} chunks={}",
                            completion.streamed,
                            completion.stream_chunks.len()
                        ),
                        detail,
                        "Proposal is registered only; app/editor/workspace own apply".to_string(),
                    ],
                    anchor,
                    replacement,
                    span,
                },
                Some(stream),
            )
        }
        // No completion. Which of the two reasons matters to whoever reads the
        // proposal.
        //
        // With no live backend selected, the deterministic proposal is the
        // product working as configured. With a backend selected and failing --
        // an unreachable Ollama, an Anthropic stream that broke after emitting
        // deltas -- the same fixture arrived labelled as though a provider had
        // produced it, and the partial text that had already reached the screen
        // was replaced by unrelated content with nothing saying why.
        None => (
            match backend {
                Some(backend) => failed_live_assisted_edit_proposal(backend),
                None => deterministic_assisted_edit_proposal(),
            },
            None,
        ),
    }
}

/// Where a live model's answer lands in the buffer, or why it does not.
enum AssistPlacement {
    /// The block resolved to a unique span.
    Resolved {
        span: (usize, usize),
        anchor: String,
        replacement: String,
        /// How the match was obtained, as a label rather than the resolver's
        /// enum: `legion-ai` is an optional dependency (`ai` or `offline`),
        /// and a type from it in this signature would make the whole Assist
        /// path fail to compile in a build with neither.
        outcome_label: String,
    },
    /// Nothing usable came back, and this says what.
    Unresolved { reason: String },
}

/// Read the model's search/replace block and locate it in the buffer.
///
/// Resolved against the excerpt, and the spans that come back are still
/// absolute buffer offsets: `bounded_by_bytes` returns a prefix, so the two
/// coordinate systems agree for every byte the model was shown. Resolving
/// against the excerpt is also the honest bound on uniqueness -- the model
/// cannot anchor on text it never saw, and a duplicate past the cut is not
/// something it could have disambiguated. It avoids copying a 100MB buffer
/// into the worker thread as a side benefit, not as the reason.
///
/// A block that cannot be found, or that matches twice, produces no edit at
/// all. That is the point of the change: the previous path took whatever came
/// back and prepended it, so a model that misread the file still produced a
/// confident-looking proposal that corrupted it. A proposal that changes
/// nothing and says why is a worse outcome for the model and a much better one
/// for the person reviewing it.
#[cfg(any(feature = "ai", feature = "offline"))]
fn resolve_assist_placement(
    buffer_excerpt: &str,
    file_path: &str,
    answer: &str,
) -> AssistPlacement {
    let blocks = legion_ai::patch::parse_edit_blocks_for_file(answer, file_path);
    let Some(block) = blocks.first() else {
        return AssistPlacement::Unresolved {
            reason: "no search/replace block in the reply".to_string(),
        };
    };
    if blocks.len() > 1 {
        // Deliberately not "apply the first". Assist proposes one reviewable
        // edit; silently dropping the rest would make the preview a partial
        // account of what the model intended.
        return AssistPlacement::Unresolved {
            reason: format!(
                "{} blocks in the reply; Assist proposes one edit at a time",
                blocks.len()
            ),
        };
    }
    match legion_ai::patch::resolve_edit_span(buffer_excerpt, &block.old_str, &block.new_str) {
        legion_ai::patch::PatchSpan::Resolved {
            start,
            end,
            replacement,
            outcome,
        } => AssistPlacement::Resolved {
            span: (start, end),
            anchor: block.old_str.clone(),
            replacement,
            // Named rather than `{outcome:?}`. `Fuzzy` reads as "the edit is
            // approximate", and the resolver's own doc says the opposite: the
            // bytes replaced are the file's, and only the *search* was
            // tolerant. A reviewer acts on this line.
            outcome_label: match outcome {
                legion_ai::patch::EditResolutionOutcome::Exact => "exact",
                legion_ai::patch::EditResolutionOutcome::Fuzzy => "whitespace-tolerant-anchor",
                legion_ai::patch::EditResolutionOutcome::WholeFileFallback => "whole-file",
            }
            .to_string(),
        },
        legion_ai::patch::PatchSpan::NoMatch(diagnostic) => AssistPlacement::Unresolved {
            reason: diagnostic.message,
        },
        legion_ai::patch::PatchSpan::Ambiguous { occurrences } => AssistPlacement::Unresolved {
            reason: format!("the quoted text appears {occurrences} times; it must be unique"),
        },
        legion_ai::patch::PatchSpan::ValidationError { reason } => {
            AssistPlacement::Unresolved { reason }
        }
    }
}

/// Without `legion-ai` there is no resolver, so there is no edit.
///
/// Unreachable in practice -- a build with neither `ai` nor `offline` has no
/// provider to answer with either -- but it must compile, and it must not
/// fall back to inserting the reply at the top of the file. That fallback is
/// the behaviour this whole change exists to remove; leaving it in one
/// configuration would leave it in the product.
#[cfg(not(any(feature = "ai", feature = "offline")))]
fn resolve_assist_placement(
    _buffer_excerpt: &str,
    _file_path: &str,
    _answer: &str,
) -> AssistPlacement {
    AssistPlacement::Unresolved {
        reason: "this build has no patch resolver".to_string(),
    }
}

/// The proposal to register when a selected live backend failed to answer.
///
/// Deterministic content, and honest about being it. The alternative -- what
/// this replaces -- is a fixture wearing a provider's name, which is worse than
/// a failure: a person reviewing it has no way to know the provider never
/// answered, and the details they would check say the opposite.
#[cfg(feature = "ai")]
pub(crate) fn failed_live_assisted_edit_proposal(
    backend: ProductAiLiveBackend,
) -> AssistedEditProposalSource {
    let label = live_backend_label(backend);
    let mut source = deterministic_assisted_edit_proposal();
    source.summary = format!("Offline fallback: the {label} provider did not answer");
    source.details.insert(
        0,
        format!("live_backend={label} outcome=failed; this text is the offline fallback"),
    );
    source
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
    match complete_product_chat(
        backend,
        system,
        &user,
        PRODUCT_COMPLETION_MAX_TOKENS,
        0.2,
        on_delta,
    ) {
        Some(completion) => {
            let stream = product_stream_from_completion(&completion, "delegate.chat");
            (bounded_label(completion.text, 1_200), Some(stream))
        }
        // No completion. Which of the two reasons matters to whoever reads the
        // reply, exactly as it does on the Assist path.
        //
        // Reporting an answer "ready" from the fixture told somebody their
        // question had been answered when the provider they had selected never
        // replied -- and then advised them to enable the very backend that was
        // already selected and had just failed.
        None => (
            match backend {
                Some(backend) => format!(
                    "Delegate provider {} did not answer; showing the offline reply instead. route={route_id} labels={}",
                    live_backend_label(backend),
                    route_labels.join(",")
                ),
                None => format!(
                    "Delegate provider answer ready via {citation_count} citation(s); route={route_id} labels={} (backend=none; fixture — enable Ollama loopback or Anthropic BYOK for a live reply)",
                    route_labels.join(",")
                ),
            },
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

#[cfg(all(test, any(feature = "ai", feature = "offline")))]
mod assist_placement_tests {
    use super::*;

    const FILE: &str = "fn main() {\n    println!(\"one\");\n    println!(\"two\");\n}\n";

    /// Destructure a placement the way the call site does.
    ///
    /// Returns `(span, anchor, replacement, detail)`. The accessors this
    /// replaces cloned out of one arm four times; the tests read the same shape
    /// the product does, so a change to that shape breaks them together.
    fn parts(placement: AssistPlacement) -> ((usize, usize), String, String, String) {
        match placement {
            AssistPlacement::Resolved {
                span,
                anchor,
                replacement,
                outcome_label,
            } => {
                let detail = format!("edit={outcome_label} bytes={}..{}", span.0, span.1);
                (span, anchor, replacement, detail)
            }
            AssistPlacement::Unresolved { reason } => (
                (0, 0),
                String::new(),
                String::new(),
                format!("edit=unresolved: {reason}"),
            ),
        }
    }

    fn block(old: &str, new: &str) -> String {
        format!("<<<<<<< SEARCH\n{old}\n=======\n{new}\n>>>>>>> REPLACE\n")
    }

    /// The edit lands where the model pointed, not at the top of the file.
    ///
    /// This is the whole point of the change. Assist asked for "the exact text
    /// to insert at the top of the file" and placed the answer at
    /// `TextRange::byte(0, 0)`, so the feature could prepend and nothing else --
    /// for the fixture, for Ollama and for Anthropic alike. A span that starts
    /// at 0 here means the regression is back.
    #[test]
    fn a_resolved_block_edits_where_the_model_pointed() {
        let answer = block("    println!(\"two\");", "    println!(\"three\");");
        let (span, _anchor, replacement, _detail) =
            parts(resolve_assist_placement(FILE, "src/main.rs", &answer));

        let (start, end) = span;
        assert!(
            start > 0,
            "the edit must land at the quoted text, not at the top of the file; got {start}..{end}"
        );
        // Exactly the quoted text, and not the newline after it: the span is
        // the match, so the file keeps its own line ending and the diff is one
        // line rather than two.
        assert_eq!(
            &FILE[start..end],
            "    println!(\"two\");",
            "the span must cover exactly the quoted text"
        );
        assert_eq!(replacement, "    println!(\"three\");");
    }

    /// Applying the resolved span to the buffer produces the intended file.
    ///
    /// The span and the replacement are handed to the proposal separately, so
    /// a test that only checks the span could pass while the two disagree.
    #[test]
    fn the_resolved_span_and_replacement_compose_into_the_intended_file() {
        let answer = block("    println!(\"one\");", "    println!(\"uno\");");
        let (span, _anchor, replacement, _detail) =
            parts(resolve_assist_placement(FILE, "src/main.rs", &answer));
        let (start, end) = span;

        let mut applied = String::new();
        applied.push_str(&FILE[..start]);
        applied.push_str(&replacement);
        applied.push_str(&FILE[end..]);

        assert_eq!(
            applied,
            "fn main() {\n    println!(\"uno\");\n    println!(\"two\");\n}\n"
        );
    }

    /// An anchor that is not in the file proposes nothing at all.
    ///
    /// The old path took whatever came back and prepended it, so a model that
    /// misread the file still produced a confident-looking proposal that
    /// corrupted it. Changing nothing and saying why is the better failure.
    #[test]
    fn an_anchor_that_is_not_in_the_file_proposes_no_edit() {
        let answer = block("    println!(\"nowhere\");", "    println!(\"x\");");
        let (span, _anchor, replacement, detail) =
            parts(resolve_assist_placement(FILE, "src/main.rs", &answer));

        assert_eq!(span, (0, 0));
        assert!(
            replacement.is_empty(),
            "an unresolved edit must replace nothing, or it corrupts the file"
        );
        assert!(
            detail.starts_with("edit=unresolved"),
            "the reviewer has to be told why; got {:?}",
            detail
        );
    }

    /// An anchor that matches twice is refused rather than guessed at.
    #[test]
    fn an_ambiguous_anchor_proposes_no_edit() {
        let repeated = "a();\nb();\na();\n";
        let answer = block("a();", "c();");
        let (span, _anchor, replacement, detail) =
            parts(resolve_assist_placement(repeated, "src/main.rs", &answer));

        assert_eq!(span, (0, 0));
        assert!(replacement.is_empty());
        assert!(
            detail.contains("2 times"),
            "the reason should name the ambiguity; got {:?}",
            detail
        );
    }

    /// A reply with no block at all is not silently treated as text to insert.
    #[test]
    fn prose_with_no_block_proposes_no_edit() {
        let (span, _anchor, replacement, detail) = parts(resolve_assist_placement(
            FILE,
            "src/main.rs",
            "Sure! You could rename the function.",
        ));

        assert_eq!(span, (0, 0));
        assert!(replacement.is_empty());
        assert!(
            detail.contains("no search/replace block"),
            "got {:?}",
            detail
        );
    }

    /// Several blocks are refused, not silently reduced to the first.
    ///
    /// Assist proposes one reviewable edit; applying block one and dropping the
    /// rest would make the preview a partial account of what the model meant.
    #[test]
    fn more_than_one_block_proposes_no_edit() {
        let answer = format!(
            "{}{}",
            block("    println!(\"one\");", "    println!(\"uno\");"),
            block("    println!(\"two\");", "    println!(\"dos\");")
        );
        let (span, _anchor, _replacement, detail) =
            parts(resolve_assist_placement(FILE, "src/main.rs", &answer));

        assert_eq!(span, (0, 0));
        assert!(detail.contains("one edit at a time"), "got {:?}", detail);
    }

    /// The resolved placement reports the anchor it matched.
    ///
    /// Carried so the completion path can re-check uniqueness against the whole
    /// file: resolution sees only the excerpt, and an anchor unique there can
    /// occur again past the cut. Without the anchor travelling with the source
    /// that check has nothing to count.
    #[test]
    fn a_resolved_placement_reports_the_anchor_it_matched() {
        let answer = block("    println!(\"two\");", "    println!(\"three\");");
        let (_span, anchor, _replacement, _detail) =
            parts(resolve_assist_placement(FILE, "src/main.rs", &answer));

        assert_eq!(anchor, "    println!(\"two\");");
    }

    /// An unresolved placement has no anchor to re-check.
    #[test]
    fn an_unresolved_placement_reports_no_anchor() {
        let (_span, anchor, _replacement, _detail) = parts(resolve_assist_placement(
            FILE,
            "src/main.rs",
            "no block here",
        ));

        assert!(anchor.is_empty());
    }

    /// The outcome label says what happened, not what the enum is called.
    ///
    /// `Fuzzy` reads as "the edit is approximate" and the resolver's own doc
    /// says the opposite: the bytes replaced are the file's, only the search was
    /// tolerant. This line ends up in a proposal a person reads before
    /// approving an edit.
    #[test]
    fn the_outcome_label_describes_the_match_not_the_enum() {
        // Indentation the model got wrong across two lines, so the exact
        // substring is absent and only the normalized search finds it. A
        // single mis-indented line would still match exactly, as a substring
        // of the indented one.
        let answer = block(
            "println!(\"one\");\n      println!(\"two\");",
            "println!(\"uno\");",
        );
        let (_span, _anchor, _replacement, detail) =
            parts(resolve_assist_placement(FILE, "src/main.rs", &answer));

        assert!(
            detail.contains("whitespace-tolerant-anchor"),
            "the label must say the anchor was matched tolerantly, not `Fuzzy`; got {detail:?}"
        );
        assert!(
            !detail.contains("Fuzzy"),
            "the enum's own name must not reach the reviewer; got {detail:?}"
        );
    }

    /// A block wrapped in a diff fence is accepted, because the parser accepts it.
    ///
    /// The prompt used to forbid markdown fences while `parse_diff_fences` read
    /// them anyway -- a rule the code did not enforce and the model was
    /// penalised for breaking, since a fenced block plus any other block became
    /// "two blocks" with no explanation a reader could act on.
    #[test]
    fn a_fenced_block_is_not_penalised_for_a_rule_the_parser_does_not_keep() {
        let answer = format!(
            "```diff\n{}```\n",
            block("    println!(\"two\");", "    println!(\"three\");")
        );
        let (span, _anchor, _replacement, detail) =
            parts(resolve_assist_placement(FILE, "src/main.rs", &answer));

        let (start, _) = span;
        assert!(
            start > 0,
            "a fenced search/replace block should still resolve; detail was {:?}",
            detail
        );
    }

    /// An unresolved edit is never registered as a proposal.
    ///
    /// It used to be, as an empty replacement over an empty span. Approving
    /// that is a real transaction: `EditorEngine::apply_edits` increments the
    /// buffer version, writes an undo entry, and marks the buffer dirty for
    /// text it did not change -- a button that looks like it worked and did
    /// nothing, which is worse than the prepend it replaced.
    ///
    /// Asserted at the source rather than through a run: an empty replacement
    /// is the condition `finish_assisted_edit_proposal_registration` declines
    /// on, so this pins the property that decision reads.
    #[test]
    fn an_unresolved_edit_carries_no_replacement_to_register() {
        let (_span, _anchor, replacement, _detail) = parts(resolve_assist_placement(
            FILE,
            "src/main.rs",
            "no block here",
        ));
        assert!(
            replacement.is_empty(),
            "an unresolved edit must carry nothing to apply, or a no-op reaches the              proposal lifecycle"
        );

        // And a resolved one does, so the guard cannot swallow real edits.
        let answer = block("    println!(\"two\");", "    println!(\"three\");");
        let (_span, _anchor, resolved, _detail) =
            parts(resolve_assist_placement(FILE, "src/main.rs", &answer));
        assert!(!resolved.is_empty());
    }

    /// A deletion is a real edit and must survive the no-op guard.
    ///
    /// A valid block with a non-empty SEARCH and an empty REPLACE resolves to a
    /// non-empty span with an empty replacement. Testing emptiness of the
    /// replacement alone classified that as unresolved, so Assist silently
    /// rejected every deletion it was asked for. "Changes nothing" is an empty
    /// span *and* an empty replacement.
    #[test]
    fn a_deletion_resolves_to_a_span_with_no_replacement() {
        let answer = block("    println!(\"two\");", "");
        let (span, _anchor, replacement, _detail) =
            parts(resolve_assist_placement(FILE, "src/main.rs", &answer));

        assert!(span.0 < span.1, "a deletion covers the text it removes");
        assert!(replacement.is_empty(), "and puts nothing in its place");
        assert_ne!(
            span,
            (0, 0),
            "so the registration guard, which tests both, keeps it"
        );
    }

    /// The deterministic fixture keeps prepending, and that stays honest.
    ///
    /// It is a canned comment that says it is a canned comment; inserting one
    /// at the top is exactly what it claims to do. Only a live model is held to
    /// producing an edit that resolves.
    #[test]
    fn the_fixture_proposal_still_inserts_at_the_top() {
        let fixture = deterministic_assisted_edit_proposal();
        assert_eq!(fixture.span, (0, 0));
        assert!(!fixture.replacement.is_empty());
    }
}
