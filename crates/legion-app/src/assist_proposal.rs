//! The Assist proposal path: authorize a route, get an edit, register a proposal.
//!
//! Extracted verbatim from `lib.rs`. Nothing here changed in the move — the
//! patch-first change lands in the commit after this one, which is the point:
//! ADR-0049 serialization rule 2 names this exact destination
//! (`assist proposal path -> assist_proposal.rs`) so that the feature diff is
//! readable instead of being an 865-line move with an edit hidden inside it.
//!
//! Every method is an inherent `AppComposition` method, so callers are
//! unaffected by the module existing.

use crate::*;

/// Why an anchor is not safe to edit on, or `None` when it is.
///
/// Two questions, because the resolver answers with two matching rules and a
/// uniqueness check that only knows one of them is not a check.
///
/// An exact duplicate is the obvious case. The subtle one: when the span was
/// found by whitespace-tolerant search, the anchor is *not present verbatim*
/// anywhere -- so counting exact occurrences over the whole file returns zero,
/// sails past `<= 1`, and the guard passes without having looked at anything.
/// A second whitespace-variant match below the excerpt would go unseen, which
/// is precisely the duplicate the model could not have disambiguated.
///
/// So the tolerant search runs too, and it runs for every outcome rather than
/// only for tolerant ones: normalized matching is a superset of exact, and two
/// sites differing only in indentation are two sites the model saw one of.
///
/// How far the ambiguity scans count before they stop.
///
/// The decision needs only "more than one", so any cap above one is correct.
/// Eight rather than two because the count reaches the reviewer -- "appears 3
/// times" is a useful thing to read and "at least 2" is not -- and eight
/// matches is still a bounded walk of a file that may be 100 MB.
#[cfg(any(feature = "ai", feature = "offline"))]
const ASSIST_MATCH_COUNT_CAP: usize = 8;

/// Counted rather than found, and the difference is the whole buffer.
///
/// `find_whitespace_insensitive` answers this too -- it returns `None` when a
/// second normalized match exists -- but it materialises three vectors and a
/// normalized `String` per line to do it. That is free against the 4 KB excerpt
/// the resolver runs on, and it is hundreds of megabytes of transient
/// allocation against a 100 MB buffer, on the app thread, with the UI waiting.
///
/// `count_whitespace_insensitive` streams the same matching rules and stops at
/// the second match, which is the only number this question needs.
///
/// The exact scan is capped for the same reason and was not, which made the
/// streaming one only half a fix: a short anchor repeated a million times in a
/// 100 MB buffer was counted a million times, on the app thread, to produce a
/// number the reviewer reads as "more than one". The cap is generous enough
/// that the number stays useful when it is small, and the message says "at
/// least" when the scan stopped early rather than reporting the cap as a total.
#[cfg(any(feature = "ai", feature = "offline"))]
fn assist_anchor_ambiguity(haystack: &str, needle: &str) -> Option<String> {
    let exact = legion_ai::patch::count_overlapping_up_to(haystack, needle, ASSIST_MATCH_COUNT_CAP);
    if exact > 1 {
        let count = if exact >= ASSIST_MATCH_COUNT_CAP {
            format!("at least {exact}")
        } else {
            exact.to_string()
        };
        return Some(format!(
            "the quoted text appears {count} times in the file, but only once in \
             the excerpt the model was shown"
        ));
    }
    if legion_ai::patch::count_whitespace_insensitive(haystack, needle, 2) > 1 {
        return Some(
            "the quoted text appears more than once in the file once whitespace is \
             ignored, but only once in the excerpt the model was shown"
                .to_string(),
        );
    }
    None
}

/// Without the resolver there is no anchor to check.
#[cfg(not(any(feature = "ai", feature = "offline")))]
fn assist_anchor_ambiguity(_haystack: &str, _needle: &str) -> Option<String> {
    None
}

/// Whether approving this edit would change no bytes.
///
/// A valid exact block whose SEARCH and REPLACE text are identical resolves to
/// a real span carrying a replacement equal to what is already there. Both are
/// non-empty, so the "changes nothing" guard downstream -- which tests an empty
/// span *and* an empty replacement -- registers it happily, and approving it
/// runs `EditorEngine::apply_edits`: version incremented, undo entry written,
/// buffer marked dirty, text identical. The same no-op button, reached from the
/// other side.
fn edit_replaces_text_with_itself(source: &AssistedEditProposalSource, full_text: &str) -> bool {
    let (start, end) = source.span;
    // An unresolved source is `(0, 0)` with an empty replacement, and the empty
    // slice at 0 does equal an empty replacement -- but that is a run that
    // produced no edit, not one that produced a pointless edit, and the two earn
    // different records. Left to the guard that names it.
    if start == end && source.replacement.is_empty() {
        return false;
    }
    // The span came from a resolver reading the excerpt, and the excerpt is a
    // prefix of this text, so it should always be in range. Should is not a
    // reason to index without checking.
    if start > end || end > full_text.len() {
        return false;
    }
    if !full_text.is_char_boundary(start) || !full_text.is_char_boundary(end) {
        return false;
    }
    full_text[start..end] == source.replacement
}

/// Turn a source into the same no-op an unresolvable block produces.
fn withdrawn_edit(
    source: AssistedEditProposalSource,
    headline: &str,
    reason: String,
) -> AssistedEditProposalSource {
    let mut source = source;
    source.summary = format!("{} ({headline})", source.summary);
    source.details.push(format!("edit=withdrawn: {reason}"));
    source.span = (0, 0);
    source.replacement = String::new();
    source.anchor = String::new();
    source
}

/// Withdraw an edit that must not be offered for approval.
///
/// Two ways to earn that. The anchor may not be unique in `full_text`, in which
/// case the resolved span is one of several sites the model never chose
/// between. Or the edit may be real, unique and pointless -- a replacement
/// identical to the text under it, which costs a dirty buffer and an undo entry
/// to accomplish nothing.
///
/// Pure, and separated from the method that reads the buffer for exactly one
/// reason: this is the safety net the change exists to add, and a net nothing
/// can test is a net nobody knows is there. The method below is now the two
/// lines that fetch the text.
fn withdraw_unapprovable_edit(
    source: AssistedEditProposalSource,
    full_text: &str,
) -> AssistedEditProposalSource {
    if !source.anchor.is_empty()
        && let Some(reason) = assist_anchor_ambiguity(full_text, &source.anchor)
    {
        return withdrawn_edit(source, "anchor not unique", reason);
    }
    if edit_replaces_text_with_itself(&source, full_text) {
        return withdrawn_edit(
            source,
            "changes nothing",
            "the replacement is identical to the text it would replace".to_string(),
        );
    }
    source
}

/// Why a completed Assist run registered no proposal.
///
/// The resolver and the withdrawal guard both write their reason into the
/// source's details as `edit=unresolved: ...` or `edit=withdrawn: ...`, and the
/// source is discarded once we know there is no proposal to attach it to. This
/// lifts the line out first, so the record says *why* rather than only that
/// nothing happened.
///
/// Metadata-only holds. Every reason on those two paths is a count, a line
/// number or a similarity score -- "appears 3 times", "closest line is 214 (71%
/// similar)", "2 blocks in the reply" -- and none quote the buffer or the
/// reply.
///
/// Read from the back, and only these two prefixes. A withdrawal is *appended*
/// to details the resolver already wrote, so a withdrawn identity edit carries
/// `edit=exact bytes=12..30` ahead of the line saying why it was withdrawn --
/// and taking the first `edit=` line reported the resolution as though it were
/// the reason nothing was registered.
fn assist_no_edit_diagnostic(source: &AssistedEditProposalSource) -> String {
    source
        .details
        .iter()
        .rev()
        .find(|detail| {
            detail.starts_with("edit=withdrawn:") || detail.starts_with("edit=unresolved:")
        })
        .cloned()
        .unwrap_or_else(|| "edit=unresolved: no reason recorded".to_string())
}

/// Restate a route intent's byte coverage as the span the proposal edits.
///
/// The intent is built at authorization time, before the model has answered,
/// so it declared `ByteRange::new(0, 0)` -- correct while every Assist edit was
/// an insertion at byte 0, and a false record the moment they stopped being.
/// The persisted request contract is what an audit reads to see what the run
/// targeted, so it has to name the range the registered proposal changes.
fn assist_intent_over_resolved_span(
    intent: &legion_protocol::AssistedAiProposalTargetIntent,
    span: (usize, usize),
) -> legion_protocol::AssistedAiProposalTargetIntent {
    let mut intent = intent.clone();
    for target in &mut intent.target_coverage.targets {
        target.byte_ranges = vec![legion_protocol::ByteRange::new(
            span.0 as u64,
            span.1 as u64,
        )];
    }
    intent
}

impl AppComposition {
    /// Withdraw an edit the whole file says must not be approved.
    ///
    /// Returns the source unchanged when there is nothing to withdraw -- no
    /// edit, an anchor that occurs exactly once, and a replacement that differs
    /// from the text under it. Otherwise it becomes the same no-op an
    /// unresolvable block produces: empty span, empty replacement, and a detail
    /// saying why, because a proposal that changes the wrong line confidently is
    /// the failure this whole path exists to remove.
    fn reject_unapprovable_assist_edit(
        &self,
        buffer_id: BufferId,
        source: AssistedEditProposalSource,
    ) -> AssistedEditProposalSource {
        let Ok(full_text) = self.editor.text(buffer_id) else {
            return source;
        };
        withdraw_unapprovable_edit(source, full_text)
    }
    /// Run one Assist operation, from authorization to proposal or refusal.
    ///
    /// The provider class is **not** a parameter. Both callers passed
    /// `LocalLoopback` because that is what the fixture used to be, and the
    /// value travelled all the way into the reviewer-facing capability -- so an
    /// Anthropic proposal was presented as local, free, metadata-only and
    /// air-gap safe while the excerpt went over the wire. Deriving `remote`
    /// from the class was only half a fix while the class itself came from a
    /// caller guessing. `product_ai_route_fields` resolves it below, from the
    /// backend that will actually receive the text, and nothing else may
    /// supply it.
    pub(crate) fn run_assisted_ai_operation(
        &mut self,
        operation_class: legion_protocol::AssistedAiOperationClass,
        instruction_label: impl Into<String>,
    ) -> Result<AppAiRunOutcome, AppCompositionError> {
        self.require_assist_mode()?;
        // Trimmed to the length declared to the broker below. An unbounded
        // instruction makes the declaration a number the request is free to
        // exceed, which is the same as declaring nothing.
        let instruction_label = bounded_assist_instruction(&instruction_label.into());
        let context = self.active_documents.require_active_save_context()?;
        let event_context = self.next_event_context();
        let generated_at = TimestampMillis::now();
        let snapshot = self.editor.current_snapshot(context.buffer_id)?.clone();
        let run_id =
            legion_protocol::AgentRunId(format!("phase4-run-{}", event_context.correlation_id.0));
        let route_id = format!("phase4-route-{}", event_context.correlation_id.0);
        let snapshot_hash = FileFingerprint {
            algorithm: "legion-text-snapshot".to_string(),
            value: snapshot.content_hash.clone(),
        };
        let instruction_bundle = instruction_prefix_bundle(
            context.workspace_id,
            generated_at,
            self.active_documents
                .workspace_root_path
                .as_ref()
                .map(|path| std::path::Path::new(path.as_str())),
            std::env::var_os("HOME")
                .as_deref()
                .map(std::path::Path::new),
        );
        // Resolved before the trust projections, not after them.
        //
        // These projections describe what the run will do, and they were built
        // from the loopback fixture's assumptions and then handed a route that
        // might be Anthropic. Ordering was the whole defect: the reviewer read
        // an accurate remote capability beside a manifest, an inspector and a
        // budget all still saying the buffer never leaves the machine.
        let live_backend = product_ai_selected_live_backend(self.preferred_ai_provider);
        let (
            route_provider_id,
            route_model,
            route_provider_class,
            route_network,
            route_health,
            route_cost,
            route_privacy,
        ) = product_ai_route_fields(live_backend);
        // Both halves, here where the projections are first built.
        //
        // `Explain` leaves through the metadata-only path without calling a
        // provider, so a remote class alone described an upload that never
        // happened. The proposal-registration path applies the same pair, and
        // Explain never reaches it -- which is why this is the copy that
        // mattered.
        let sends_the_buffer = provider_class_sends_the_buffer(route_provider_class)
            && operation_uploads_the_excerpt(operation_class);
        let mut context_manifest_projection =
            Phase4ContextAssemblyService::assemble_context_manifest(
                &context,
                &run_id,
                &route_id,
                snapshot.snapshot_id,
                snapshot.buffer_version,
                snapshot_hash,
                snapshot.byte_len as u64,
                snapshot.line_count.min(u32::MAX as usize) as u32,
                generated_at,
                instruction_bundle.manifest_items,
                sends_the_buffer,
            );
        let mut privacy_inspector_projection =
            legion_protocol::privacy_inspector_from_context_manifest_projection(
                &context_manifest_projection,
                format!("phase4:privacy:{}", run_id.0),
                generated_at,
                1,
            );
        let mut permission_budget_projection = phase4_permission_budget_projection(
            &context_manifest_projection,
            &run_id,
            generated_at,
            sends_the_buffer,
            // Not yet: the broker has not been asked at this point.
            false,
        );

        let mut agent = AgentRuntime::new(run_id.clone());
        agent
            .transition(
                legion_protocol::AgentRunState::Planning,
                "agent.planning.context_ready",
                event_context.correlation_id,
                event_context.causality_id,
                self.event_sequence_generator.next(),
            )
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;

        // The backend authorized here is the one resolved above, before the
        // trust projections were built from it.
        let provider_route_request = legion_protocol::AssistedAiProviderRouteRequest {
            route_id: route_id.clone(),
            provider_id: route_provider_id.clone(),
            model_label: route_model.clone(),
            provider_class: route_provider_class,
            operation_class,
            context_manifest: trust_reference(
                &context_manifest_projection.manifest.manifest_id,
                legion_protocol::AssistedAiTrustProjectionKind::ContextManifest,
            ),
            privacy_inspector: trust_reference(
                &privacy_inspector_projection.inspector_id,
                legion_protocol::AssistedAiTrustProjectionKind::PrivacyInspector,
            ),
            permission_budget: trust_reference(
                &permission_budget_projection.projection_id,
                legion_protocol::AssistedAiTrustProjectionKind::PermissionBudget,
            ),
            prompt_prefix: instruction_bundle.prompt_prefix,
            proposal_intent: legion_protocol::AssistedAiProposalTargetIntent {
                payload_kind: legion_protocol::ProposalPayloadKind::TextEdit,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![ProposalAffectedTarget {
                        target_id: format!("file:{}", context.metadata.identity.file_id.0),
                        kind: ProposalTargetKind::OpenBuffer,
                        workspace_id: Some(context.workspace_id),
                        file_id: Some(context.metadata.identity.file_id),
                        buffer_id: Some(context.buffer_id),
                        path: Some(context.metadata.identity.canonical_path.clone()),
                        terminal_session_id: None,
                        plugin_id: None,
                        remote_authority: None,
                        collaboration_session_id: None,
                        byte_ranges: vec![legion_protocol::ByteRange::new(0, 0)],
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                    }],
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                required_capability: CapabilityId("editor.write".to_string()),
                risk_label: legion_protocol::ProposalRiskLabel::Low,
                privacy_label: route_privacy,
                labels: vec![instruction_label.clone()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            policy_decision_id: None,
            required_capability: CapabilityId("ai.provider.invoke".to_string()),
            network_target: route_network,
            cancellation_token: legion_protocol::CancellationTokenId(uuid::Uuid::now_v7()),
            health_labels: route_health,
            cost_labels: route_cost,
            principal_id: context.principal.clone(),
            workspace_trust_state: context.trust.clone(),
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            event_sequence: self.event_sequence_generator.next(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let broker = DenyByDefaultBroker::new(
            // The installed bundle, not the product defaults. Assist was reached
            // through the rail commands this PR made reachable, and it asked
            // `product_ai_security_policy` -- so a bundle permitting Assist mode
            // while forbidding the selected provider still let the buffer
            // excerpt go out. Mode ceiling and provider ceiling are different
            // questions and passing the first is not passing the second.
            self.product_ai_policy_with_org_ceiling(live_backend),
            CapabilityNamespace("app.ai".to_string()),
        );
        // Capability/network decision only — product prose is filled by
        // complete_product_chat after authorization (registry may lack BYOK key).
        let route_response = {
            let decision = broker
                .handle(CapabilityRequest::Request {
                    principal_id: context.principal.clone(),
                    capability_id: CapabilityId("ai.provider.invoke".to_string()),
                    workspace_trust_state: context.trust.clone(),
                    target_path: None,
                    decision_id: None,
                    context: legion_protocol::CapabilityRequestContext {
                        network_target: provider_route_request.network_target.clone(),
                        // Declared, for the same reason the Delegate lane
                        // declares it: without an identity the broker sees a
                        // destination and nothing to match a provider
                        // restriction against, so an allowlist naming exactly
                        // which providers may run has no input to evaluate.
                        ai_provider_id: Some(route_provider_id.clone()),
                        // Same reasoning one field up: an undeclared token count
                        // is never compared against the org's cap, so a bundle
                        // capping tokens per request had nothing to cap.
                        budget_request_tokens: Some(declared_request_tokens(
                            ASSIST_PROMPT_MAX_BYTES,
                            crate::product_ai_completion::PRODUCT_COMPLETION_MAX_TOKENS,
                        )),
                        budget_request_cost_cents: live_backend
                            .and_then(declared_request_cost_cents),
                        ..Default::default()
                    },
                    correlation_id: event_context.correlation_id,
                })
                .map_err(|error| AppCompositionError::AiRuntime(error.message))?;
            let granted = matches!(
                decision,
                CapabilityResponse::Decision(ref d) if d.granted
            ) || matches!(decision, CapabilityResponse::Granted(_));
            // The manifest is built before the broker answers, so its provider
            // permission starts as ungranted with no decision behind it. That
            // is true right up until the broker grants, and then it is a record
            // of a permission that was never given -- which
            // `privacy_inspector_from_context_manifest_projection` reads as a
            // denial and turns into a refusal, so the approval checklist
            // reported blockers on every Assist proposal that had in fact been
            // authorized. A reviewer who sees blockers on a run nothing blocked
            // learns to click past them.
            if granted {
                let decision_id = match &decision {
                    CapabilityResponse::Decision(decision) => Some(decision.decision_id),
                    CapabilityResponse::Granted(grant) => Some(grant.decision_id),
                    CapabilityResponse::Denied(_) => None,
                };
                for permission in &mut context_manifest_projection.manifest.permissions {
                    if permission.capability.0 == "ai.provider.invoke" {
                        permission.granted = true;
                        permission.decision_id = decision_id;
                    }
                }
                // Rebuilt from the corrected manifest rather than patched: the
                // inspector is a projection of the manifest, and two ways to
                // change it is how they come apart.
                privacy_inspector_projection =
                    legion_protocol::privacy_inspector_from_context_manifest_projection(
                        &context_manifest_projection,
                        format!("phase4:privacy:{}", run_id.0),
                        generated_at,
                        1,
                    );
                permission_budget_projection = phase4_permission_budget_projection(
                    &context_manifest_projection,
                    &run_id,
                    generated_at,
                    sends_the_buffer,
                    // The person picked this destination; the broker only
                    // allowed it. `Auto` never routes remotely, so a remote
                    // route exists because somebody selected one.
                    matches!(
                        self.preferred_ai_provider,
                        ProductAiProviderPreference::Anthropic
                    ),
                );
            }
            if !granted {
                let event_sequence = provider_route_request.event_sequence;
                let refusal = legion_protocol::AssistedAiRefusalMetadata {
                    reason_code: "capability.denied".to_string(),
                    label: "provider capability denied by policy".to_string(),
                    provider_id: Some(route_provider_id.clone()),
                    operation_class: Some(operation_class),
                    privacy_scope: None,
                    capability: Some(CapabilityId("ai.provider.invoke".to_string())),
                    budget_id: None,
                    risk_label: legion_protocol::ProposalRiskLabel::High,
                    reasons: vec!["capability.denied".to_string()],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                };
                return self.finish_assisted_ai_metadata_only_run(
                    run_id,
                    route_id,
                    operation_class,
                    route_provider_class,
                    provider_route_request.clone(),
                    legion_protocol::AssistedAiProviderRouteResponse {
                        route_id: provider_route_request.route_id.clone(),
                        invocation_state:
                            legion_protocol::AssistedAiProviderInvocationState::Refused,
                        route_decision: legion_protocol::AssistedAiRouteDecision {
                            disposition: legion_protocol::AssistedAiRequestDisposition::Refused,
                            provider_invocation:
                                legion_protocol::AssistedAiProviderInvocationState::Refused,
                            refusal: Some(refusal.clone()),
                            reasons: vec!["capability.denied".to_string()],
                            redaction_hints: vec![RedactionHint::MetadataOnly],
                            schema_version: 1,
                        },
                        provider_id: route_provider_id.clone(),
                        model_label: route_model.clone(),
                        output_labels: vec!["output.not_encoded".to_string()],
                        refusal: Some(refusal),
                        correlation_id: event_context.correlation_id,
                        causality_id: event_context.causality_id,
                        event_sequence,
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                        schema_version: 1,
                    },
                    context_manifest_projection,
                    privacy_inspector_projection,
                    permission_budget_projection,
                    generated_at,
                    event_context,
                    // A policy refusal: the route never ran, so there is no
                    // resolution to explain. The refusal metadata is the reason.
                    None,
                    &mut agent,
                );
            }
            // Still exercise the deterministic router for offline fixture metadata
            // when no live backend is selected; live backends skip registry complete
            // (credentials live in the product keyring path, not the registry).
            if live_backend.is_none() {
                ProviderRouter::new(&self.ai_registry, &broker)
                    .route_completion(provider_route_request.clone())
                    .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?
            } else {
                legion_protocol::AssistedAiProviderRouteResponse {
                    route_id: provider_route_request.route_id.clone(),
                    invocation_state: legion_protocol::AssistedAiProviderInvocationState::Completed,
                    route_decision: legion_protocol::AssistedAiRouteDecision {
                        disposition:
                            legion_protocol::AssistedAiRequestDisposition::MetadataOnlyReady,
                        provider_invocation:
                            legion_protocol::AssistedAiProviderInvocationState::Completed,
                        refusal: None,
                        reasons: vec!["provider.authorized.product_edge".to_string()],
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                        schema_version: 1,
                    },
                    provider_id: route_provider_id,
                    model_label: route_model,
                    output_labels: vec!["route.authorized".to_string()],
                    refusal: None,
                    correlation_id: event_context.correlation_id,
                    causality_id: event_context.causality_id,
                    event_sequence: provider_route_request.event_sequence,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                }
            }
        };
        if route_response.invocation_state
            != legion_protocol::AssistedAiProviderInvocationState::Completed
            || operation_class == legion_protocol::AssistedAiOperationClass::Explain
        {
            return self.finish_assisted_ai_metadata_only_run(
                run_id,
                route_id,
                operation_class,
                route_provider_class,
                provider_route_request,
                route_response,
                context_manifest_projection,
                privacy_inspector_projection,
                permission_budget_projection,
                generated_at,
                event_context,
                // Explain never had a proposal to produce, and a route that did
                // not complete carries its own refusal.
                None,
                &mut agent,
            );
        }

        agent
            .transition(
                legion_protocol::AgentRunState::Proposing,
                "agent.proposing.provider_completed",
                event_context.correlation_id,
                event_context.causality_id,
                self.event_sequence_generator.next(),
            )
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;

        // Live completion only after the capability/network decision above.
        // Uses the authorized backend only (no silent Ollama→Anthropic fallback).
        // Bounded in bytes by the constant the broker is told about.
        //
        // A `4_000` here and an `ASSIST_EXCERPT_MAX_BYTES` in the declaration
        // are two numbers that happen to agree, and taking characters while
        // declaring bytes made them disagree for anything but ASCII: four
        // thousand emoji are four thousand `char`s and sixteen thousand bytes.
        let buffer_excerpt = assist_buffer_excerpt(
            self.editor.text(context.buffer_id).unwrap_or(""),
            ASSIST_EXCERPT_MAX_BYTES,
        );
        let preconditions = ProposalVersionPreconditions {
            file_version: Some(context.metadata.file_content_version),
            buffer_version: Some(snapshot.buffer_version),
            snapshot_id: Some(snapshot.snapshot_id),
            generation: Some(context.metadata.workspace_generation),
            file_content_version: Some(context.metadata.file_content_version),
            workspace_generation: Some(context.metadata.workspace_generation),
            expected_fingerprint: Some(context.metadata.fingerprint.clone()),
            expected_file_length: context.metadata.file_length,
            expected_modified_at: context.metadata.modified_at,
        };
        let pending_job = PendingAssistProposalJob {
            buffer_id: context.buffer_id,
            run_id: run_id.clone(),
            route_id: route_id.clone(),
            operation_class,
            // The class the route resolved, not one a caller supplied: this is
            // what the capability a reviewer reads is built from.
            provider_class: route_provider_class,
            // The decision as it stood when the excerpt was sent.
            consent_granted: matches!(
                self.preferred_ai_provider,
                ProductAiProviderPreference::Anthropic
            ),
            provider_route_request: provider_route_request.clone(),
            route_response: route_response.clone(),
            context_manifest_projection: context_manifest_projection.clone(),
            generated_at,
            event_context,
            principal: context.principal.clone(),
            file_id: context.metadata.identity.file_id,
            preconditions,
            agent,
        };

        // Live path: stream on a worker thread so the UI can poll progressive
        // deltas; proposal registration runs on poll_product_ai_stream when the
        // worker finishes (Delegate-chat parity). Offline/fixture stays sync so
        // tests keep receiving proposal_id in the same call.
        let inject_assist_spawn_failure = {
            #[cfg(any(test, feature = "test-helpers"))]
            {
                std::mem::take(&mut self.injected_assist_spawn_failure)
            }
            #[cfg(not(any(test, feature = "test-helpers")))]
            {
                false
            }
        };
        // Whether a worker runs follows the backend the broker approved, not a
        // second probe of the same question.
        //
        // `product_ai_will_attempt_live` re-asks whether a live route exists.
        // Between authorization and here that answer can change -- an Ollama
        // server that stopped, a network that went away -- and when it flipped
        // to `false` the fallback ran the already-authorized backend
        // *synchronously on this thread*, which is the blocking provider call
        // this lane exists to prevent. The authorized backend is a fact; a
        // re-probe is a guess about it.
        #[cfg(feature = "ai")]
        let use_background_live = live_backend.is_some() || inject_assist_spawn_failure;
        #[cfg(not(feature = "ai"))]
        let use_background_live = false;
        let lane_reservation = ProductAiLaneReservation::try_acquire(
            self.live_product_ai_stream.clone(),
            "assist.proposal",
            "pending",
            "",
        )
        .ok_or_else(|| {
            AppCompositionError::AiRuntime(
                "product AI provider lane is busy; poll the active result before dispatching another request"
                    .to_string(),
            )
        })?;

        #[cfg(feature = "ai")]
        if use_background_live {
            // No preference is captured: the worker uses the backend the
            // broker already approved, so there is nothing here that could
            // resolve to a different destination than the one authorized.
            // Bounded like the instruction, and for the same reason: the
            // prompt embeds this path and a canonical path has no length of its
            // own, so a declaration allowing for a nominal one understates any
            // request against a deeply nested file.
            let file_path = bounded_assist_path(&context.metadata.identity.canonical_path.0);
            let instruction_for_worker = instruction_label.clone();
            let excerpt_for_worker = buffer_excerpt.clone();
            let streaming_replay = legion_protocol::AgentReplayManifest {
                run_id: run_id.clone(),
                transitions: pending_job.agent.transitions().to_vec(),
                context_manifests: vec![trust_reference(
                    &context_manifest_projection.manifest.manifest_id,
                    legion_protocol::AssistedAiTrustProjectionKind::ContextManifest,
                )],
                provider_route_ids: vec![route_id.clone()],
                proposal_ids: Vec::new(),
                correlation_id: pending_job.event_context.correlation_id,
                causality_id: pending_job.event_context.causality_id,
                event_sequence: self.event_sequence_generator.next(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            };
            let sink_delta = lane_reservation.delta_writer();
            let worker = move || {
                let mut on_delta = move |delta: &str| sink_delta.push(delta);
                let (proposal_source, stream) = resolve_assisted_edit_proposal_text(
                    live_backend,
                    &instruction_for_worker,
                    &excerpt_for_worker,
                    &file_path,
                    Some(&mut on_delta),
                );
                let completion = stream.as_ref().map(|stream| ProductChatCompletion {
                    provider_id: stream.provider_id.clone(),
                    model: stream.model.clone(),
                    text: stream.text_preview.clone(),
                    stream_chunks: stream.chunks.clone(),
                    streamed: stream.streamed,
                });
                lane_reservation.finish_background(
                    ProductAiBackgroundResult {
                        assistant_message_id: String::new(),
                        content_label: String::new(),
                        live_failed: live_backend.is_some() && stream.is_none(),
                        stream,
                        assist_proposal: Some(proposal_source),
                        inline_prediction: None,
                        delegate_route: None,
                    },
                    completion.as_ref(),
                );
            };
            #[cfg(any(test, feature = "test-helpers"))]
            let spawn_result = if inject_assist_spawn_failure {
                Err(std::io::Error::other(
                    "injected Assist background worker spawn failure",
                ))
            } else {
                std::thread::Builder::new()
                    .name("legion-assist-proposal".to_string())
                    .spawn(worker)
            };
            #[cfg(not(any(test, feature = "test-helpers")))]
            let spawn_result = std::thread::Builder::new()
                .name("legion-assist-proposal".to_string())
                .spawn(worker);
            spawn_result.map_err(|error| {
                AppCompositionError::AiRuntime(format!(
                    "failed to spawn Assist background worker: {error}"
                ))
            })?;
            self.pending_assist_proposal = Some(pending_job);
            // Partial phase-4 projections are published only after the worker
            // exists, so a failed spawn cannot leave a phantom in-flight run.
            self.phase4_projection_state.context_manifest_projection =
                Some(context_manifest_projection.clone());
            self.phase4_projection_state.privacy_inspector_projection =
                Some(privacy_inspector_projection.clone());
            self.phase4_projection_state.permission_budget_projection =
                Some(permission_budget_projection.clone());
            // Streaming outcome: proposal_id arrives on the next poll cycle(s).
            return Ok(AppAiRunOutcome {
                run_id,
                proposal_id: None,
                proposal_created: None,
                route_response,
                context_manifest_projection,
                privacy_inspector_projection,
                permission_budget_projection,
                refusal: None,
                replay_manifest: streaming_replay,
            });
        }
        // Silence unused when `ai` feature is off (background path is feature-gated).
        #[cfg(not(feature = "ai"))]
        let _ = use_background_live;

        let injected_assist_reply = {
            #[cfg(all(
                any(test, feature = "test-helpers"),
                any(feature = "ai", feature = "offline")
            ))]
            {
                self.injected_assist_reply.take()
            }
            #[cfg(not(all(
                any(test, feature = "test-helpers"),
                any(feature = "ai", feature = "offline")
            )))]
            {
                Option::<String>::None
            }
        };
        let sink_delta = lane_reservation.delta_writer();
        let mut on_delta = move |delta: &str| sink_delta.push(delta);
        // An injected answer stands in for the provider, not for the resolver:
        // it takes the same placement path a live reply takes, so a test that
        // injects a duplicated anchor is testing the rule and not a copy of it.
        #[cfg(all(
            any(test, feature = "test-helpers"),
            any(feature = "ai", feature = "offline")
        ))]
        let (proposal_source, stream) = match injected_assist_reply {
            Some(answer) => (
                crate::product_ai_completion::assisted_edit_proposal_source_from_answer(
                    &buffer_excerpt,
                    &context.metadata.identity.canonical_path.0,
                    &answer,
                ),
                None,
            ),
            None => resolve_assisted_edit_proposal_text(
                live_backend,
                &instruction_label,
                &buffer_excerpt,
                &context.metadata.identity.canonical_path.0,
                Some(&mut on_delta),
            ),
        };
        #[cfg(not(all(
            any(test, feature = "test-helpers"),
            any(feature = "ai", feature = "offline")
        )))]
        let (proposal_source, stream) = {
            let _ = injected_assist_reply;
            resolve_assisted_edit_proposal_text(
                live_backend,
                &instruction_label,
                &buffer_excerpt,
                &context.metadata.identity.canonical_path.0,
                Some(&mut on_delta),
            )
        };
        let completion = stream.as_ref().map(|stream| ProductChatCompletion {
            provider_id: stream.provider_id.clone(),
            model: stream.model.clone(),
            text: stream.text_preview.clone(),
            stream_chunks: stream.chunks.clone(),
            streamed: stream.streamed,
        });
        lane_reservation.finish(completion.as_ref());
        if let Some(stream) = stream {
            self.last_product_ai_stream = Some(stream);
        }

        self.finish_assisted_edit_proposal_registration(pending_job, proposal_source)
    }

    /// Register the Assist edit proposal after live or fixture text is available.
    pub(crate) fn finish_assisted_edit_proposal_registration(
        &mut self,
        mut job: PendingAssistProposalJob,
        proposal_source: AssistedEditProposalSource,
    ) -> Result<AppAiRunOutcome, AppCompositionError> {
        let PendingAssistProposalJob {
            buffer_id,
            run_id,
            route_id,
            operation_class,
            provider_class,
            consent_granted,
            provider_route_request,
            route_response,
            context_manifest_projection,
            generated_at,
            event_context,
            principal,
            file_id,
            preconditions,
            ref mut agent,
        } = job;

        // The uniqueness the prompt asked for, checked against the file.
        //
        // Resolution ran against the excerpt, because that is the only text the
        // model saw and the only text it could have quoted. The span that comes
        // back is right. The *uniqueness* is not settled by it: an anchor that
        // appears once in the first 4,000 bytes can appear again further down,
        // and the prompt said "exactly once in the file". Editing the first
        // occurrence would be picking a site the model never chose between.
        //
        // Both the synchronous and background paths arrive here on the app
        // thread, which is the first point that can read the whole buffer
        // without copying it across a thread boundary.
        let proposal_source = self.reject_unapprovable_assist_edit(buffer_id, proposal_source);

        // An edit that changes nothing is not registered as a proposal.
        //
        // Registering one is not free: approving it runs
        // `EditorEngine::apply_edits`, which increments the buffer version,
        // writes an undo entry, and marks the buffer dirty for text it did not
        // change -- a button that looks like it worked and did nothing.
        //
        // "Changes nothing" is an empty span *and* an empty replacement, not
        // emptiness of either alone. A deletion is a real edit with a non-empty
        // span and an empty replacement, and testing the replacement by itself
        // silently rejected every one of them. The fixture is the mirror image:
        // an empty span with real text, which is an insertion. A replacement
        // identical to the text under it is the third case, and it is withdrawn
        // above rather than tested here.
        let registers_an_edit =
            proposal_source.span != (0, 0) || !proposal_source.replacement.is_empty();
        // Allocated only when there is something to allocate it for.
        //
        // Assigning the id first and returning metadata-only afterwards left
        // the outcome and the replay and tracker records reporting
        // `proposal_id: None` while the context manifest, the privacy inspector
        // and the permission budget derived from it all named a proposal that
        // was never registered. An audit reading those projections would go
        // looking for it.
        let proposal_id = registers_an_edit.then(|| self.proposal_coordinator.next_id());
        // The trust projections were built before this proposal existed, so
        // their actions named no proposal -- and `permission_budget_gate` only
        // considers refused evaluations whose action names the proposal being
        // approved. A remote route's unresolved consent was therefore
        // evaluated, recorded, and then omitted from the one gate it exists to
        // block. A refusal nobody is shown is the same as no refusal.
        //
        // Rebuilt rather than patched: the evaluation is derived from the
        // manifest's permission and the action built from it, so setting the id
        // in one place and leaving the derived record alone is how they come
        // apart.
        let mut context_manifest_projection = context_manifest_projection;
        context_manifest_projection.manifest.proposal_id = proposal_id;
        let permission_budget_projection = phase4_permission_budget_projection(
            &context_manifest_projection,
            &run_id,
            generated_at,
            provider_class_sends_the_buffer(provider_class)
                && operation_uploads_the_excerpt(operation_class),
            // The decision recorded when this run started, not whatever the
            // picker says now: the preference is still changeable while a
            // request streams, and by here the excerpt has already gone.
            consent_granted,
        );
        // Everything derived from the manifest, not just the budget.
        //
        // The inspector is a projection of the manifest and carries its
        // `proposal_id`; leaving the earlier one in place gave the checklist an
        // inspector naming no proposal, which `privacy_gate` reports as
        // `privacy_inspector.proposal_mismatch` -- a blocker on every proposal,
        // including the local ones this linking has nothing to do with.
        let privacy_inspector_projection =
            legion_protocol::privacy_inspector_from_context_manifest_projection(
                &context_manifest_projection,
                format!("phase4:privacy:{}", run_id.0),
                generated_at,
                1,
            );
        // The run still has to be recorded. It happened, it cost a provider
        // call, and the reason the edit did not resolve is the useful part --
        // so it finishes through the metadata-only path that already exists for
        // runs that produce no proposal, rather than returning early and
        // leaving no durable trail. That path performs the runtime and replay
        // persistence, the tracker-ledger append, the agent transition and the
        // projection updates, all of which an early return skipped.
        let Some(proposal_id) = proposal_id else {
            return self.finish_assisted_ai_metadata_only_run(
                run_id,
                route_id,
                operation_class,
                provider_class,
                provider_route_request,
                route_response,
                context_manifest_projection,
                privacy_inspector_projection,
                permission_budget_projection,
                generated_at,
                event_context,
                // The reason, carried into the durable records rather than
                // dropped with the source. Without it the audit says a
                // ProposeEdit run finished and produced nothing, and stops
                // there -- which is the half of the story nobody can act on.
                Some(assist_no_edit_diagnostic(&proposal_source)),
                agent,
            );
        };
        let output = legion_protocol::AssistedAiEditProposalOutput {
            output_id: format!("phase4-output-{}", event_context.correlation_id.0),
            request_id: format!("phase4-request-{}", event_context.correlation_id.0),
            provider_id: proposal_source.provider_id.clone(),
            proposal_id,
            principal: principal.clone(),
            capability: CapabilityId("editor.write".to_string()),
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            payload: ProposalPayload::TextEdit(legion_protocol::TextEditProposal {
                file_id,
                edits: legion_protocol::EditBatch {
                    edits: vec![legion_protocol::TextEdit {
                        // Where the model's edit actually goes.
                        //
                        // This was `byte(0, 0)` for every proposal Assist has
                        // ever made -- fixture, Ollama and Anthropic alike --
                        // so the feature could prepend text and nothing else.
                        // The span is resolved from a search/replace block
                        // against the excerpt the model was shown, and an
                        // unresolvable block yields `(0, 0)` with an empty
                        // replacement: a proposal that changes nothing and
                        // says why, rather than one that changes the wrong
                        // thing confidently.
                        range: legion_protocol::TextRange::byte(
                            proposal_source.span.0 as u64,
                            proposal_source.span.1 as u64,
                        ),
                        replacement: proposal_source.replacement.clone(),
                    }],
                },
            }),
            preconditions,
            preview: PreviewSummary {
                summary: proposal_source.summary.clone(),
                details: proposal_source.details.clone(),
            },
            expires_at: None,
            created_at: generated_at,
            context_manifest: trust_reference(
                &context_manifest_projection.manifest.manifest_id,
                legion_protocol::AssistedAiTrustProjectionKind::ContextManifest,
            ),
            approval_checklist: trust_reference(
                &format!("phase4:approval:{}", run_id.0),
                legion_protocol::AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let proposal = output
            .to_workspace_proposal()
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        let proposal_created = self.register_proposal_lifecycle(&proposal)?;
        let ledger_projection = self
            .proposal_coordinator
            .proposal_ledger_projection(generated_at);
        let checkpoint_rollback_projection =
            legion_protocol::checkpoint_rollback_projection_from_proposal(
                format!("phase4:checkpoint:{}", run_id.0),
                &proposal,
                ProposalLifecycleState::Created,
                Some(&ledger_projection),
                legion_protocol::CheckpointRollbackAuditStatus::Available,
                Some(event_context.causality_id),
                generated_at,
                1,
            );
        let approval_checklist_projection =
            legion_protocol::approval_checklist_from_trust_projections(
                format!("phase4:approval:{}", run_id.0),
                &proposal,
                ProposalLifecycleState::Created,
                Some(&ledger_projection),
                Some(&context_manifest_projection),
                Some(&privacy_inspector_projection),
                Some(&permission_budget_projection),
                Some(&checkpoint_rollback_projection),
                true,
                Some(event_context.causality_id),
                generated_at,
                1,
            );
        let provider_capability =
            phase4_provider_capability(provider_class, &provider_route_request.provider_id, None);
        let request_contract = assisted_ai_request_contract_from_metadata(
            output.request_id.clone(),
            &provider_capability,
            operation_class,
            &context_manifest_projection,
            &privacy_inspector_projection,
            &permission_budget_projection,
            &approval_checklist_projection,
            Some(&checkpoint_rollback_projection),
            event_context,
            assist_intent_over_resolved_span(
                &provider_route_request.proposal_intent,
                proposal_source.span,
            ),
            route_response.route_decision.clone(),
            generated_at,
        );
        let assisted_ai_projection = legion_protocol::assisted_ai_projection_from_metadata(
            format!("phase4:assisted:{}", run_id.0),
            vec![provider_capability],
            vec![request_contract],
            vec![output.clone()],
            Some(&ledger_projection),
            Some(&context_manifest_projection),
            Some(&privacy_inspector_projection),
            Some(&permission_budget_projection),
            Some(&approval_checklist_projection),
            Some(&checkpoint_rollback_projection),
            generated_at,
            1,
        );

        agent
            .transition(
                legion_protocol::AgentRunState::WaitingForApproval,
                "agent.waiting_for_approval.proposal_registered",
                event_context.correlation_id,
                event_context.causality_id,
                self.event_sequence_generator.next(),
            )
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        let replay_manifest = legion_protocol::AgentReplayManifest {
            run_id: run_id.clone(),
            transitions: agent.transitions().to_vec(),
            context_manifests: vec![trust_reference(
                &context_manifest_projection.manifest.manifest_id,
                legion_protocol::AssistedAiTrustProjectionKind::ContextManifest,
            )],
            provider_route_ids: vec![route_id.clone()],
            proposal_ids: vec![proposal_id],
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            event_sequence: self.event_sequence_generator.next(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        self.persist_phase4_runtime_records(
            &run_id,
            &route_id,
            route_response.invocation_state,
            // The label follows the state rather than restating the happy path.
            // Persisting `completed` beside a `Failed` invocation state is the
            // same contradiction the state was corrected to remove.
            if route_response.invocation_state
                == legion_protocol::AssistedAiProviderInvocationState::Failed
            {
                "phase4.provider.route.failed"
            } else {
                "phase4.provider.route.completed"
            },
            event_context,
            &replay_manifest,
            &[],
        )?;
        self.tracker_ledger
            .append(TrackerRunLedgerRecord {
                run_id: run_id.clone(),
                state: legion_protocol::AgentRunState::WaitingForApproval,
                proposal_id: Some(proposal_id),
                transitions: replay_manifest.transitions.clone(),
                correlation_id: event_context.correlation_id,
                causality_id: event_context.causality_id,
                event_sequence: self.event_sequence_generator.next(),
                labels: vec!["tracker.phase4.run.waiting_for_approval".to_string()],
            })
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        let _ = self
            .memory_service
            .propose_candidate(MemoryCandidateRecord {
                candidate_id: format!("phase4-memory-candidate-{}", run_id.0),
                run_id: Some(run_id.clone()),
                consent: MemoryConsentState::NotGranted,
                labels: vec!["memory.candidate.review_required".to_string()],
                correlation_id: event_context.correlation_id,
                causality_id: event_context.causality_id,
                event_sequence: self.event_sequence_generator.next(),
            })
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;

        self.phase4_projection_state.context_manifest_projection =
            Some(context_manifest_projection.clone());
        self.phase4_projection_state.privacy_inspector_projection =
            Some(privacy_inspector_projection.clone());
        self.phase4_projection_state.permission_budget_projection =
            Some(permission_budget_projection.clone());
        // The run's own projections, kept against the proposal they describe.
        //
        // The selected-proposal path reconstructs these from the proposal row,
        // which carries no route -- so it cannot tell a remote run from a local
        // one and assumes consent was never required.
        self.phase4_projection_state
            .phase4_trust_by_proposal
            .insert(
                proposal_id,
                Box::new(SelectedProposalTrustProjections {
                    context_manifest_projection: context_manifest_projection.clone(),
                    privacy_inspector_projection: privacy_inspector_projection.clone(),
                    permission_budget_projection: permission_budget_projection.clone(),
                    approval_checklist_projection: approval_checklist_projection.clone(),
                    checkpoint_rollback_projection: checkpoint_rollback_projection.clone(),
                }),
            );
        self.phase4_projection_state.approval_checklist_projection =
            Some(approval_checklist_projection);
        self.phase4_projection_state.checkpoint_rollback_projection =
            Some(checkpoint_rollback_projection);
        self.phase4_projection_state.assisted_ai_projection = Some(assisted_ai_projection.clone());
        self.phase4_projection_state
            .replay_manifests
            .insert(run_id.clone(), replay_manifest.clone());
        self.phase4_projection_state.inspection_snapshots.insert(
            run_id.clone(),
            AppAiInspectionSnapshot {
                run_id: run_id.clone(),
                context_manifest_projection: context_manifest_projection.clone(),
                privacy_inspector_projection: privacy_inspector_projection.clone(),
                permission_budget_projection: permission_budget_projection.clone(),
                assisted_ai_projection: assisted_ai_projection.clone(),
            },
        );

        Ok(AppAiRunOutcome {
            run_id,
            proposal_id: Some(proposal_id),
            proposal_created: Some(proposal_created),
            route_response,
            context_manifest_projection,
            privacy_inspector_projection,
            permission_budget_projection,
            refusal: None,
            replay_manifest,
        })
    }
}

#[cfg(all(test, any(feature = "ai", feature = "offline")))]
mod assist_guard_tests {
    use super::*;

    fn source(anchor: &str) -> AssistedEditProposalSource {
        AssistedEditProposalSource {
            provider_id: "ollama".to_string(),
            summary: "Assist edit proposal from ollama".to_string(),
            details: vec![
                "model=test".to_string(),
                // The resolver writes its own `edit=` line before any
                // withdrawal is appended. Leaving it out of the fixture is what
                // let a rule that reads the *first* one look correct.
                "edit=exact bytes=10..20".to_string(),
            ],
            anchor: anchor.to_string(),
            replacement: "changed();".to_string(),
            span: (10, 20),
        }
    }

    /// A unique anchor is left alone.
    #[test]
    fn a_unique_anchor_keeps_its_edit() {
        let file = "fn a() {}\nfn b() {}\n";
        let kept = withdraw_unapprovable_edit(source("fn a() {}"), file);

        assert_eq!(kept.span, (10, 20));
        assert_eq!(kept.replacement, "changed();");
    }

    /// An anchor appearing twice verbatim withdraws the edit.
    ///
    /// This is the net the whole change exists to add: resolution saw only the
    /// excerpt and reported a unique match, and the file disagrees.
    #[test]
    fn an_anchor_repeated_in_the_file_withdraws_the_edit() {
        let file = "fn a() {}\nfn b() {}\nfn a() {}\n";
        let withdrawn = withdraw_unapprovable_edit(source("fn a() {}"), file);

        assert_eq!(withdrawn.span, (0, 0), "a withdrawn edit spans nothing");
        assert!(withdrawn.replacement.is_empty());
        assert!(withdrawn.anchor.is_empty());
        assert!(
            withdrawn.summary.contains("anchor not unique"),
            "the summary must say so; got {:?}",
            withdrawn.summary
        );
        assert!(
            withdrawn
                .details
                .iter()
                .any(|detail| detail.contains("appears 2 times")),
            "the reviewer needs the count; got {:?}",
            withdrawn.details
        );
    }

    /// A count past the cap is reported as a bound, not as a total.
    ///
    /// The scans stop early so a 100 MB buffer with a million matches does not
    /// stall the app thread, and printing the cap as though it were the count
    /// would trade the stall for a false number in front of a reviewer.
    #[test]
    fn a_count_past_the_cap_says_at_least() {
        let file = "fn a() {}
"
        .repeat(40);
        let withdrawn = withdraw_unapprovable_edit(source("fn a() {}"), &file);

        assert!(
            withdrawn
                .details
                .iter()
                .any(|detail| detail.contains("at least 8 times")),
            "the reviewer must be told the number is a bound; got {:?}",
            withdrawn.details
        );
    }

    /// A duplicate that differs only in whitespace withdraws the edit too.
    ///
    /// The check that shipped first counted exact occurrences only. When the
    /// span was found by whitespace-tolerant search the anchor is not present
    /// verbatim anywhere, so that count returned zero, sailed past `<= 1`, and
    /// the guard passed without having looked at anything -- doing nothing in
    /// precisely the case it was added for.
    #[test]
    fn a_whitespace_variant_duplicate_withdraws_the_edit() {
        let file = "    call(a, b);\nother();\n        call(a,   b);\n";
        let withdrawn = withdraw_unapprovable_edit(source("call(a, b);"), file);

        assert_eq!(
            withdrawn.span,
            (0, 0),
            "two sites differing only in spacing are two sites the model saw one of"
        );
        assert!(
            withdrawn
                .details
                .iter()
                .any(|detail| detail.contains("whitespace is ignored")),
            "the reason must say which rule found the duplicate; got {:?}",
            withdrawn.details
        );
    }

    /// A source with no edit is passed through untouched.
    #[test]
    fn a_source_with_no_anchor_is_left_alone() {
        let mut empty = source("");
        empty.span = (0, 0);
        empty.replacement = "/* fixture */".to_string();
        let kept = withdraw_unapprovable_edit(empty, "anything at all");

        assert_eq!(kept.replacement, "/* fixture */");
    }

    /// An edit that replaces text with itself is withdrawn.
    ///
    /// A valid exact block whose SEARCH and REPLACE are identical resolves to a
    /// real span and a real replacement, so the "changes nothing" guard -- which
    /// tests an empty span *and* an empty replacement -- registered it. Approving
    /// it dirties the buffer, bumps the version and writes an undo entry for
    /// text it leaves exactly as it found it.
    #[test]
    fn a_replacement_identical_to_the_text_it_replaces_is_withdrawn() {
        let file = "fn a() {}\nchanged();\n";
        let mut identity = source("changed();");
        identity.span = (10, 20);

        let withdrawn = withdraw_unapprovable_edit(identity, file);

        assert_eq!(
            &file[10..20],
            "changed();",
            "the fixture must be an identity edit"
        );
        assert_eq!(withdrawn.span, (0, 0), "a withdrawn edit spans nothing");
        assert!(withdrawn.replacement.is_empty());
        assert!(
            withdrawn.summary.contains("changes nothing"),
            "the summary must say so; got {:?}",
            withdrawn.summary
        );
    }

    /// A deletion is not an identity edit, whatever the emptiness suggests.
    ///
    /// Its replacement is empty and the text under it is not, so it survives --
    /// the check that would have caught it is the one comparing the replacement
    /// against the buffer slice rather than against nothing.
    #[test]
    fn a_deletion_survives_the_identity_check() {
        let file = "fn a() {}\ndoomed();\n";
        let mut deletion = source("doomed();");
        deletion.span = (10, 20);
        deletion.replacement = String::new();

        let kept = withdraw_unapprovable_edit(deletion, file);

        assert_eq!(kept.span, (10, 20), "a deletion is a real edit");
        assert!(kept.replacement.is_empty());
    }

    /// A run that registers no proposal still says why.
    ///
    /// The source is discarded once there is no proposal to attach it to, and
    /// the reason went with it -- leaving an audit record saying a ProposeEdit
    /// run finished and produced nothing, with no way to find out what happened.
    #[test]
    fn the_reason_no_edit_resolved_survives_the_source() {
        let mut unresolved = source("");
        unresolved.span = (0, 0);
        unresolved.replacement = String::new();
        unresolved.details = vec![
            "model=test".to_string(),
            "edit=unresolved: no search/replace block in the reply".to_string(),
        ];

        assert_eq!(
            assist_no_edit_diagnostic(&unresolved),
            "edit=unresolved: no search/replace block in the reply"
        );
    }

    /// A withdrawal's reason wins over the resolution it overrode.
    ///
    /// The resolver writes `edit=exact bytes=..` and the withdrawal is appended
    /// after it, so reading the first `edit=` line reported a successful
    /// resolution as the reason nothing was registered -- which is the opposite
    /// of what happened.
    #[test]
    fn a_withdrawn_edit_reports_the_withdrawal_reason() {
        let file = "fn a() {}\nfn b() {}\nfn a() {}\n";
        let withdrawn = withdraw_unapprovable_edit(source("fn a() {}"), file);

        let diagnostic = assist_no_edit_diagnostic(&withdrawn);
        assert!(
            diagnostic.starts_with("edit=withdrawn:"),
            "the withdrawal reason must be the one recorded; got {diagnostic:?}"
        );
        assert!(
            diagnostic.contains("appears 2 times"),
            "and it must carry the count; got {diagnostic:?}"
        );
    }

    /// The persisted contract names the span the proposal edits.
    ///
    /// It declared `ByteRange::new(0, 0)` -- true while every Assist edit was
    /// an insertion at byte 0, and a false audit record the moment they stopped
    /// being. An audit reads this to see what the run targeted.
    #[test]
    fn the_route_intent_is_restated_over_the_resolved_span() {
        let intent = legion_protocol::AssistedAiProposalTargetIntent {
            payload_kind: legion_protocol::ProposalPayloadKind::TextEdit,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![ProposalAffectedTarget {
                    target_id: "file:1".to_string(),
                    kind: ProposalTargetKind::OpenBuffer,
                    workspace_id: None,
                    file_id: None,
                    buffer_id: None,
                    path: None,
                    terminal_session_id: None,
                    plugin_id: None,
                    remote_authority: None,
                    collaboration_session_id: None,
                    byte_ranges: vec![legion_protocol::ByteRange::new(0, 0)],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                }],
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            required_capability: CapabilityId("editor.write".to_string()),
            risk_label: legion_protocol::ProposalRiskLabel::Low,
            privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec![],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let restated = assist_intent_over_resolved_span(&intent, (42, 99));

        for target in &restated.target_coverage.targets {
            assert_eq!(
                target.byte_ranges,
                vec![legion_protocol::ByteRange::new(42, 99)],
                "every target must name the range the proposal changes"
            );
        }
    }
}
