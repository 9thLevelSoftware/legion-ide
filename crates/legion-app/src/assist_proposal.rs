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

impl AppComposition {
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
        let buffer_excerpt = bounded_by_bytes(
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

        let sink_delta = lane_reservation.delta_writer();
        let mut on_delta = move |delta: &str| sink_delta.push(delta);
        let (proposal_source, stream) = resolve_assisted_edit_proposal_text(
            live_backend,
            &instruction_label,
            &buffer_excerpt,
            &context.metadata.identity.canonical_path.0,
            Some(&mut on_delta),
        );
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

        let proposal_id = self.proposal_coordinator.next_id();
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
        context_manifest_projection.manifest.proposal_id = Some(proposal_id);
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
                        range: legion_protocol::TextRange::byte(0, 0),
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
            provider_route_request.proposal_intent.clone(),
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
