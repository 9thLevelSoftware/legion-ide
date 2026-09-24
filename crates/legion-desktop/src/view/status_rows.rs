//! Projection status and panel rows extracted from the view chokepoint.
//!
//! These helpers only format snapshot facts for the desktop shell. Keeping them
//! here lets language, trust, and assistant surfaces change without growing
//! `view.rs` past the extract-before-modify slack.

use super::*;

pub(super) fn trust_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    let manifest = &snapshot.context_manifest_projection.manifest;
    if !manifest.items.is_empty() || !manifest.permissions.is_empty() {
        rows.push(format!(
            "context manifest {}: {} items, {} permissions, egress {:?}",
            manifest.manifest_id,
            manifest.items.len(),
            manifest.permissions.len(),
            manifest.egress
        ));
    }
    rows.extend(manifest.items.iter().take(10).map(|item| {
        format!(
            "context item {}: {:?} {:?} risk={:?} privacy={:?} egress={:?} file={:?} buffer={:?} path={} counts={} ranges={} labels={}",
            item.item_id,
            item.kind,
            item.inclusion,
            item.risk_label,
            item.privacy_label,
            item.egress,
            item.file_id.map(|file| file.0),
            item.buffer_id.map(|buffer| buffer.0),
            item.path
                .as_ref()
                .map(|path| path.0.as_str())
                .unwrap_or("<redacted>"),
            item.counts.len(),
            item.ranges.len(),
            bounded_join(&item.labels)
        )
    }));
    rows.extend(manifest.permissions.iter().take(10).map(|permission| {
        format!(
            "context permission {:?}: capability={} granted={} scope={:?} egress={:?} risk={:?}",
            permission.kind,
            permission.capability.0,
            permission.granted,
            permission.privacy_scope,
            permission.egress,
            permission.risk_label
        )
    }));

    let privacy = &snapshot.privacy_inspector_projection;
    if !privacy.records.is_empty() || privacy.refusal.is_some() {
        rows.push(format!(
            "privacy: {} records, {} denied, {} redacted, {} external, {} high-risk",
            privacy.records.len(),
            privacy.denied_record_count,
            privacy.redacted_record_count,
            privacy.external_egress_record_count,
            privacy.high_risk_record_count
        ));
    }
    const PRIVACY_RECORD_LIMIT: usize = 10;
    // Surface the most sensitive records first (denied / fully redacted /
    // external egress / high-risk) so they are not hidden by the row cap, and
    // report any omitted records explicitly.
    let mut ordered_records: Vec<&_> = privacy.records.iter().collect();
    ordered_records.sort_by_key(|record| {
        let prioritized = record.inclusion == ContextManifestInclusionState::Denied
            || record.redaction_state == PrivacyInspectorRedactionState::FullyRedacted
            || matches!(
                record.egress,
                ContextManifestEgressStatus::RemoteApprovalRequired
                    | ContextManifestEgressStatus::RemoteDenied
                    | ContextManifestEgressStatus::ExternalEgressMetadata
            )
            || matches!(
                record.risk_label,
                ProposalRiskLabel::High | ProposalRiskLabel::Unknown
            );
        // `false` (prioritized) sorts before `true`; sort_by_key is stable so
        // original ordering is preserved within each group.
        !prioritized
    });
    rows.extend(ordered_records.iter().take(PRIVACY_RECORD_LIMIT).map(|record| {
        format!(
            "privacy record {}: {:?} {:?} risk={:?} privacy={:?} egress={:?} permission={} reasons={}",
            record.exposure_id,
            record.source_kind,
            record.redaction_state,
            record.risk_label,
            record.privacy_label,
            record.egress,
            record
                .permission_label
                .as_ref()
                .map(|capability| capability.0.as_str())
                .unwrap_or("<none>"),
            bounded_join(&record.reasons)
        )
    }));
    if privacy.records.len() > PRIVACY_RECORD_LIMIT {
        rows.push(format!(
            "privacy records omitted from preview: {}",
            privacy.records.len() - PRIVACY_RECORD_LIMIT
        ));
    }
    if let Some(refusal) = &privacy.refusal {
        rows.push(format!(
            "privacy refusal {}: {} scope={:?} capability={} risk={:?} reasons={}",
            refusal.reason_code,
            refusal.label,
            refusal.privacy_scope,
            refusal
                .capability
                .as_ref()
                .map(|capability| capability.0.as_str())
                .unwrap_or("<none>"),
            refusal.risk_label,
            bounded_join(&refusal.reasons)
        ));
    }

    let budget = &snapshot.permission_budget_projection;
    if !budget.budgets.is_empty() || !budget.evaluations.is_empty() {
        rows.push(format!(
            "permission budget: {} budgets, {} evaluations, {} denied, {} depleted, {} refused",
            budget.budgets.len(),
            budget.evaluations.len(),
            budget.denied_budget_count,
            budget.depleted_budget_count,
            budget.refused_evaluation_count
        ));
    }
    rows.extend(budget.budgets.iter().take(10).map(|contract| {
        format!(
            "permission budget {}: {:?} state={:?} scope={:?} consent={:?} used={}/{} risk={:?} reasons={}",
            contract.budget_id,
            contract.action_class,
            contract.state,
            contract.privacy_scope,
            contract.consent_requirement_label,
            contract.usage.used,
            contract
                .usage
                .ceiling
                .map(|ceiling| ceiling.to_string())
                .unwrap_or_else(|| "uncapped".to_string()),
            contract.risk_label,
            bounded_join(&contract.reasons)
        )
    }));
    rows.extend(budget.evaluations.iter().take(10).map(|evaluation| {
        format!(
            "permission evaluation {}: budget={} disposition={:?} allowed={} action={:?} estimated={} reasons={}",
            evaluation.evaluation_id,
            evaluation.budget_id,
            evaluation.disposition,
            evaluation.allowed,
            evaluation.action.action_class,
            evaluation.action.estimated_units,
            bounded_join(&evaluation.reasons)
        )
    }));
    rows.extend(
        budget
            .evaluations
            .iter()
            .filter_map(|evaluation| {
                evaluation
                    .refusal
                    .as_ref()
                    .map(|refusal| (evaluation, refusal))
            })
            .take(6)
            .map(|(evaluation, refusal)| {
                format!(
                    "permission refusal {}: {} reason={} risk={:?}",
                    evaluation.evaluation_id,
                    refusal.label,
                    refusal.reason_code,
                    refusal.risk_label
                )
            }),
    );

    let checklist = &snapshot.approval_checklist_projection;
    if !checklist.gates.is_empty() || !checklist.blockers.is_empty() {
        rows.push(format!(
            "approval checklist: proposal {} lifecycle={:?} gates={} blockers={} ready={} denials={}",
            checklist.proposal_id.0,
            checklist.lifecycle_state,
            checklist.gates.len(),
            checklist.blockers.len(),
            checklist.ready_for_approval,
            checklist.explicit_denial_reasons.len()
        ));
    }
    rows.extend(checklist.gates.iter().take(12).map(|gate| {
        format!(
            "approval gate {:?}: {:?} risk={:?} privacy={:?} labels={} reasons={}",
            gate.gate,
            gate.status,
            gate.risk_label,
            gate.privacy_label,
            bounded_join(&gate.labels),
            gate.reasons.len()
        )
    }));
    rows.extend(checklist.blockers.iter().take(10).map(|blocker| {
        format!(
            "approval blocker {:?}: {} {} risk={:?} privacy={:?}",
            blocker.gate,
            blocker.reason_code,
            blocker.label,
            blocker.risk_label,
            blocker.privacy_label
        )
    }));
    if !checklist.explicit_denial_reasons.is_empty() {
        rows.push(format!(
            "approval explicit denials: {}",
            bounded_join(&checklist.explicit_denial_reasons)
        ));
    }

    let rollback = &snapshot.checkpoint_rollback_projection;
    if !rollback.targets.is_empty()
        || !rollback.rollback.limitations.is_empty()
        || !rollback.checkpoint.limitations.is_empty()
    {
        rows.push(format!(
            "checkpoint rollback: {} targets, rollback {:?}",
            rollback.targets.len(),
            rollback.rollback.availability
        ));
    }
    if !rollback.targets.is_empty() {
        rows.push(format!(
            "checkpoint: id={} available={} targets={} audit={:?} limitations={}",
            rollback.checkpoint.checkpoint_id,
            rollback.checkpoint.available,
            rollback.checkpoint.target_count,
            rollback.checkpoint.audit_status,
            rollback.checkpoint.limitations.len()
        ));
        rows.push(format!(
            "rollback: availability={:?} steps={} reversible={} irreversible={} audit={:?} limitations={}",
            rollback.rollback.availability,
            rollback.rollback.rollback_step_count,
            rollback.rollback.reversible_target_count,
            rollback.rollback.irreversible_target_count,
            rollback.rollback.audit_status,
            rollback.rollback.limitations.len()
        ));
    }
    rows.extend(rollback.targets.iter().take(10).map(|target| {
        format!(
            "rollback target {}: {:?} file={:?} buffer={:?} labels={}",
            target.target_id,
            target.kind,
            target.file_id.map(|file| file.0),
            target.buffer_id.map(|buffer| buffer.0),
            bounded_join(&target.labels)
        )
    }));
    rows.extend(
        rollback
            .checkpoint
            .limitations
            .iter()
            .take(6)
            .map(|limitation| {
                format!(
                    "checkpoint limitation {}: {} risk={:?}",
                    limitation.reason_code, limitation.label, limitation.risk_label
                )
            }),
    );
    rows.extend(
        rollback
            .rollback
            .limitations
            .iter()
            .take(6)
            .map(|limitation| {
                format!(
                    "rollback limitation {}: {} risk={:?}",
                    limitation.reason_code, limitation.label, limitation.risk_label
                )
            }),
    );

    rows
}

pub(super) fn assistant_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    rows.extend(legion_workflow_rows(snapshot));
    let inline = &snapshot.assist_inline_prediction_projection;
    if inline.has_activity() {
        rows.push(format!(
            "inline predictions: active={} rows={} in_flight={} stale={} generated_at={}",
            inline.active_prediction.is_some(),
            inline.rows.len(),
            inline.request_in_flight,
            inline.stale_prediction_count,
            inline.generated_at.0
        ));
    }
    if let Some(prediction) = &inline.active_prediction {
        rows.push(inline_prediction_row(prediction));
    }
    rows.extend(
        inline
            .rows
            .iter()
            .filter(|row| {
                inline
                    .active_prediction
                    .as_ref()
                    .is_none_or(|active| active.prediction_id != row.prediction_id)
            })
            .take(8)
            .map(inline_prediction_row),
    );
    let assisted = &snapshot.assisted_ai_projection;
    let budget_evaluation_count: u32 = assisted
        .requests
        .iter()
        .map(|request| request.permission_budget_evaluation_count)
        .sum();
    let refused_budget_evaluation_count: u32 = assisted
        .requests
        .iter()
        .map(|request| request.refused_permission_budget_evaluation_count)
        .sum();
    if assisted.provider_count > 0
        || assisted.request_count > 0
        || assisted.refusal_count > 0
        || budget_evaluation_count > 0
        || refused_budget_evaluation_count > 0
    {
        rows.push(format!(
            "assisted ai: {} providers, {} requests, {} refusals, {} previews, {} budget evals ({} refused)",
            assisted.provider_count,
            assisted.request_count,
            assisted.refusal_count,
            assisted.preview_ready_count,
            budget_evaluation_count,
            refused_budget_evaluation_count
        ));
    }
    rows.extend(assisted.providers.iter().take(8).map(|provider| {
        format!(
            "assisted provider {}: {} class={:?} availability={:?} ops={} cost={} risk_budget={} privacy={} risk={:?}",
            provider.provider_id,
            provider.provider_label,
            provider.provider_class,
            provider.availability,
            provider.supported_operation_count,
            provider.cost_budget_label,
            provider.risk_budget_label,
            provider.privacy_retention_label,
            provider.risk_label
        )
    }));
    rows.extend(
        assisted
            .providers
            .iter()
            .filter_map(|provider| provider.refusal.as_ref().map(|refusal| (provider, refusal)))
            .take(6)
            .map(|(provider, refusal)| {
                format!(
                    "assisted provider refusal {}: {} {} risk={:?}",
                    provider.provider_id, refusal.reason_code, refusal.label, refusal.risk_label
                )
            }),
    );
    rows.extend(assisted.routes.iter().take(8).map(|route| {
        format!(
            "assisted route {}: provider={} op={:?} disposition={:?} invocation={:?} refused_evals={} risk={:?} privacy={:?} reasons={}",
            route.request_id,
            route.provider_id,
            route.operation_class,
            route.disposition,
            route.provider_invocation,
            route.refused_permission_budget_evaluation_count,
            route.risk_label,
            route.privacy_label,
            bounded_join(&route.reasons)
        )
    }));
    rows.extend(
        assisted
            .routes
            .iter()
            .filter_map(|route| route.refusal.as_ref().map(|refusal| (route, refusal)))
            .take(6)
            .map(|(route, refusal)| {
                format!(
                    "assisted route refusal {}: {} {} risk={:?}",
                    route.request_id, refusal.reason_code, refusal.label, refusal.risk_label
                )
            }),
    );
    rows.extend(assisted.requests.iter().take(8).map(|request| {
        format!(
            "assisted request {}: op={:?} payload={:?} targets={} omitted={} capability={} cost={} budget_evals={}/{} route={:?} refs={}/{}/{} approval={} checkpoint={} labels={}",
            request.request_id,
            request.operation_class,
            request.proposal_payload_kind,
            request.proposal_target_count,
            request.omitted_target_count,
            request.required_capability.0,
            request.provider.cost_budget_label,
            request.permission_budget_evaluation_count,
            request.refused_permission_budget_evaluation_count,
            request.route_decision.disposition,
            request.context_manifest.reference_id,
            request.privacy_inspector.reference_id,
            request.permission_budget_projection.reference_id,
            request.approval_checklist.reference_id,
            request
                .checkpoint_rollback
                .as_ref()
                .map(|reference| reference.reference_id.as_str())
                .unwrap_or("<none>"),
            bounded_join(&request.labels)
        )
    }));
    rows.extend(assisted.proposal_previews.iter().take(8).map(|preview| {
        let request = assisted
            .requests
            .iter()
            .find(|request| request.request_id == preview.request_id);
        let request_cost = request
            .map(|request| request.provider.cost_budget_label.as_str())
            .unwrap_or("<unknown>");
        let request_budget_evals = request
            .map(|request| request.permission_budget_evaluation_count)
            .unwrap_or(0);
        let request_budget_refusals = request
            .map(|request| request.refused_permission_budget_evaluation_count)
            .unwrap_or(0);
        format!(
            "assisted preview {}: proposal={} readiness={:?} preview_ready={} approval_ready={} apply_ready={} ledger={} diff={:?} targets={} cost={} budget_evals={}/{} risk={:?} privacy={:?}",
            preview.preview_id,
            preview.proposal_id.0,
            preview.readiness,
            preview.ready_for_preview,
            preview.ready_for_approval,
            preview.ready_for_apply,
            preview.ledger_row_present,
            preview.diff_summary.kind,
            preview.target_coverage.targets.len(),
            request_cost,
            request_budget_evals,
            request_budget_refusals,
            preview.risk_label,
            preview.privacy_label
        )
    }));
    rows.extend(assisted.refusals.iter().take(8).map(|refusal| {
        format!(
            "assisted refusal {}: {} provider={} op={:?} capability={} risk={:?} reasons={}",
            refusal.reason_code,
            refusal.label,
            refusal.provider_id.as_deref().unwrap_or("<none>"),
            refusal.operation_class,
            refusal
                .capability
                .as_ref()
                .map(|capability| capability.0.as_str())
                .unwrap_or("<none>"),
            refusal.risk_label,
            bounded_join(&refusal.reasons)
        )
    }));
    let context_manifest = &snapshot.context_manifest_projection;
    if !context_manifest.manifest.items.is_empty() || context_manifest.selected_item_id.is_some() {
        rows.push(format!(
            "context manifest {}: {} items, selected={}",
            context_manifest.manifest.manifest_id,
            context_manifest.manifest.items.len(),
            context_manifest
                .selected_item_id
                .as_deref()
                .unwrap_or("<none>")
        ));
    }

    let delegated = &snapshot.delegated_task_projection;
    if delegated.plan_count == 0
        && delegated.plan_rows.is_empty()
        && delegated.step_summaries.is_empty()
        && delegated.blockers.is_empty()
        && delegated.refusals.is_empty()
        && delegated.required_approvals.is_empty()
        && delegated.proposal_preview_links.is_empty()
        && delegated.audit_readiness.is_empty()
        && delegated.chat_messages.is_empty()
        && delegated.context_citations.is_empty()
        && delegated.proposal_reviews.is_empty()
        && delegated.tool_permission_requests.is_empty()
    {
        return rows;
    }
    rows.push(format!(
        "delegated task command center: projection={} plans={} blocked={} refused={} chat={} citations={} reviews={} permissions={} runtime={:?} autonomous_apply=unsupported redaction={}",
        delegated.projection_id,
        delegated.plan_count,
        delegated.blocked_plan_count,
        delegated.refused_plan_count,
        delegated.chat_message_count,
        delegated.context_citation_count,
        delegated.proposal_review_count,
        delegated.tool_permission_request_count,
        delegated.runtime_activation,
        redaction_label(&delegated.redaction_hints)
    ));
    rows.extend(delegated.chat_messages.iter().take(12).map(|message| {
        format!(
            "delegate chat {}: role={:?} citations={} permissions={} label={}",
            message.message_id,
            message.role,
            message.citation_ids.len(),
            message.tool_permission_request_ids.len(),
            trim_middle(&message.content_label, 96)
        )
    }));
    rows.extend(delegated.context_citations.iter().take(12).map(|citation| {
        format!(
            "delegate citation {}: path={} bytes={:?} lines={:?} score={} hash={}",
            citation.citation_id,
            citation
                .path
                .as_ref()
                .map(|path| path.0.as_str())
                .unwrap_or("<none>"),
            citation.byte_range,
            citation.line_range,
            citation.score_basis_points,
            citation
                .chunk_hash
                .as_ref()
                .map(|hash| hash.value.as_str())
                .unwrap_or("<none>")
        )
    }));
    rows.extend(delegated.proposal_reviews.iter().take(8).map(|review| {
        format!(
            "delegate proposal review {}: proposal={} hunks={} accepted={} rejected={} pending={} ready={} filtered={}",
            review.review_id,
            review.proposal_id.0,
            review.hunks.len(),
            review.accepted_hunk_count,
            review.rejected_hunk_count,
            review.pending_hunk_count,
            review.ready_for_apply,
            review.filtered_apply_required
        )
    }));
    rows.extend(
        delegated
            .proposal_reviews
            .iter()
            .take(8)
            .flat_map(|review| {
                review.hunks.iter().take(8).map(move |hunk| {
                    format!(
                        "delegate proposal hunk {}: proposal={} target={} disposition={:?} payload={:?} changed={} +{} -{} risk={:?} privacy={:?}",
                        trim_middle(&hunk.hunk_id, 48),
                        review.proposal_id.0,
                        hunk.target_id.as_deref().unwrap_or("<none>"),
                        hunk.disposition,
                        hunk.payload_kind,
                        hunk.changed_line_count,
                        hunk.inserted_line_count,
                        hunk.deleted_line_count,
                        hunk.risk_label,
                        hunk.privacy_label
                    )
                })
            }),
    );
    rows.extend(delegated.tool_permission_requests.iter().take(12).map(|request| {
        format!(
            "delegate tool permission {}: profile={:?} action={:?} decision={:?} disposition={:?} approval_required={} approval_recorded={} runtime_allowed={} deny_overrides={}",
            request.request_id,
            request.profile,
            request.action_class,
            request.decision,
            request.disposition,
            request.human_approval_required,
            request.human_approval_recorded,
            request.runtime_allowed,
            request.deny_overrides
        )
    }));
    rows.extend(delegated.plan_only_disclaimers.iter().map(|disclaimer| {
        format!("delegated task disclaimer: {disclaimer} autonomous apply unsupported")
    }));
    rows.extend(delegated.plan_rows.iter().map(|plan| {
        format!(
            "delegated task plan {}: state={:?} readiness={:?} steps={} targets={} blockers={} refusals={} proposal_previews={} risk={:?} privacy={:?} runtime={:?} labels={}",
            plan.plan_id.0,
            plan.plan_state,
            plan.readiness,
            plan.step_count,
            plan.affected_target_count,
            plan.blocker_count,
            plan.refusal_count,
            plan.proposal_preview_link_count,
            plan.risk_label,
            plan.privacy_label,
            plan.runtime_activation,
            bounded_join(&plan.labels)
        )
    }));
    rows.extend(delegated.step_summaries.iter().map(|step| {
        format!(
            "delegated task step {} plan={} order={} op={:?} state={:?} deps={} targets={} proposal={:?} blockers={} risk={:?} privacy={:?}",
            step.step_id.0,
            step.plan_id.0,
            step.order,
            step.operation_class,
            step.state,
            step.dependency_count,
            step.target_count,
            step.proposal_id.map(|proposal| proposal.0),
            step.blocker_count,
            step.risk_label,
            step.privacy_label
        )
    }));
    rows.extend(delegated.required_approvals.iter().map(|gate| {
        format!(
            "delegated task trust gate {:?}: required={} satisfied={} risk={:?} privacy={:?} reasons={}",
            gate.kind,
            gate.required,
            gate.satisfied,
            gate.risk_label,
            gate.privacy_label,
            bounded_join(&gate.reasons)
        )
    }));
    rows.extend(delegated.blockers.iter().map(|blocker| {
        format!(
            "delegated task blocker {}: gate={:?} proposal={:?} label={} reasons={}",
            blocker.reason_code,
            blocker.gate,
            blocker.proposal_id.map(|proposal| proposal.0),
            blocker.label,
            bounded_join(&blocker.reasons)
        )
    }));
    rows.extend(delegated.refusals.iter().map(|refusal| {
        format!(
            "delegated task refusal {}: gate={:?} proposal={:?} label={} reasons={}",
            refusal.reason_code,
            refusal.gate,
            refusal.proposal_id.map(|proposal| proposal.0),
            refusal.label,
            bounded_join(&refusal.reasons)
        )
    }));
    rows.extend(delegated.proposal_preview_links.iter().map(|link| {
        format!(
            "delegated task proposal preview {}: proposal={} payload={:?} lifecycle={:?} targets={} hunks={} source_redacted={} proposal-mediated",
            link.link_id,
            link.proposal_id.0,
            link.payload_kind,
            link.lifecycle_state,
            link.target_count,
            link.hunk_count,
            link.full_source_redacted
        )
    }));
    rows.extend(delegated.audit_readiness.iter().map(|readiness| {
        format!(
            "delegated task audit readiness {}: readiness={:?} runtime={:?} core_ids={} blockers={} refusals={} proposal_previews={} labels={}",
            readiness.readiness_id,
            readiness.readiness,
            readiness.runtime_activation,
            readiness.correlation_causality_valid,
            readiness.blocker_count,
            readiness.refusal_count,
            readiness.proposal_preview_link_count,
            bounded_join(&readiness.labels)
        )
    }));
    rows
}

pub(super) fn inline_prediction_row(
    prediction: &legion_ui::AssistInlinePredictionRowProjection,
) -> String {
    format!(
        "inline prediction {}: provider={} status={:?} status_label={} latency={} stale={} fingerprint={} snapshot={:?} buffer_version={:?} range={} ghost={} replacement={} diagnostics={}",
        prediction.prediction_id,
        prediction.provider_label,
        prediction.status,
        prediction.status_label,
        prediction_latency_label(prediction),
        prediction.stale,
        prediction_fingerprint_label(prediction),
        prediction.snapshot_id.map(|snapshot| snapshot.0),
        prediction.buffer_version.map(|version| version.0),
        prediction.apply_range_label,
        prediction.ghost_text_label,
        prediction_replacement_label(prediction),
        prediction.diagnostics.len()
    )
}

pub(super) fn prediction_latency_label(
    prediction: &legion_ui::AssistInlinePredictionRowProjection,
) -> String {
    prediction
        .latency_ms
        .map(|latency| format!("{latency}ms"))
        .unwrap_or_else(|| "unknown".to_string())
}

pub(super) fn prediction_fingerprint_label(
    prediction: &legion_ui::AssistInlinePredictionRowProjection,
) -> String {
    prediction
        .file_fingerprint
        .as_ref()
        .map(|fingerprint| format!("{}:{}", fingerprint.algorithm, fingerprint.value))
        .unwrap_or_else(|| "<none>".to_string())
}

pub(super) fn prediction_replacement_label(
    prediction: &legion_ui::AssistInlinePredictionRowProjection,
) -> &str {
    prediction
        .replacement_preview_label
        .as_deref()
        .unwrap_or("<none>")
}

pub(super) fn legion_workflow_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let workflows = &snapshot.legion_workflow_projection;
    if workflows.rows.is_empty()
        && workflows.mcp_registries.is_empty()
        && workflows.decision_feed.is_empty()
        && workflows.risk_monitors.is_empty()
        && workflows.kill_switches.is_empty()
        && workflows.tool_permission_requests.is_empty()
        && snapshot.legion_workflow_comm_rows.is_empty()
        && snapshot.legion_workflow_budget_rows.is_empty()
    {
        return Vec::new();
    }
    let mut rows = vec![format!(
        "legion workflow command center: projection={} sessions={} mcp={} decisions={} risk_monitors={} kill_switches={} permissions={} omitted={} unattended merge unsupported until approval redaction={}",
        workflows.projection_id,
        workflows.total_session_count,
        workflows.mcp_registry_count,
        workflows.decision_feed_count,
        workflows.risk_monitor_count,
        workflows.kill_switch_count,
        workflows.tool_permission_request_count,
        workflows.omitted_row_count,
        redaction_label(&workflows.redaction_hints)
    )];
    rows.extend(workflows.rows.iter().map(|row| {
        format!(
            "workflow {}: state={:?} workers={} provider_routes={} dependencies={} conflicts={} verification={}/{} signoff={}/{} proposals={} directive_artifact={} spec_artifact={} task_graph_artifact={} merge={:?} labels={}",
            row.session_id.0,
            row.lifecycle_state,
            row.worker_count,
            row.provider_route_required_count,
            row.dependency_count,
            row.unresolved_conflict_count,
            row.passed_verification_count,
            row.verification_gate_count,
            row.signed_off_count,
            row.sign_off_count,
            row.linked_proposals.len(),
            row.directive_artifact_id.as_deref().unwrap_or("<none>"),
            row.spec_artifact_id.as_deref().unwrap_or("<none>"),
            row.task_graph_artifact_id.as_deref().unwrap_or("<none>"),
            row.merge_readiness.state,
            row.display_safe_labels.join("|")
        )
    }));
    rows.extend(workflows.rows.iter().flat_map(|row| {
        row.linked_proposals.iter().map(move |proposal_id| {
            format!(
                "legion workflow proposal link session={} proposal={} proposal-mediated",
                row.session_id.0, proposal_id.0
            )
        })
    }));
    rows.extend(workflows.rows.iter().flat_map(|row| {
        row.merge_readiness.labels.iter().map(move |label| {
            format!(
                "legion workflow merge readiness {}: state={:?} label={} approval-gated",
                row.session_id.0, row.merge_readiness.state, label
            )
        })
    }));
    rows.extend(workflows.mcp_registries.iter().map(|registry| {
        format!(
            "legion workflow mcp registry {}: server={} transport={:?} tools={} resources={} prompts={} version={} changed={:?}",
            registry.registry_id,
            registry.server.server_id.0,
            registry.server.transport_kind,
            registry.tools.len(),
            registry.resources.len(),
            registry.prompts.len(),
            registry.list_version,
            registry.last_notification_kind
        )
    }));
    rows.extend(workflows.decision_feed.iter().map(|entry| {
        format!(
            "legion workflow decision {}: session={} kind={:?} risk={:?} primitive={:?} permission={:?} summary={}",
            entry.decision_id.0,
            entry.session_id.0,
            entry.kind,
            entry.risk_label,
            entry.mcp_primitive_kind,
            entry.tool_permission_request_id,
            entry.summary_label
        )
    }));
    rows.extend(
        snapshot
            .legion_workflow_comm_rows
            .iter()
            .map(|row| format!("legion workflow comm row: {row}")),
    );
    rows.extend(snapshot.legion_workflow_budget_rows.iter().map(|row| {
        format!(
            "legion workflow budget session={} worker={} {} {} {} {} status={}",
            row.session_id.0,
            row.worker_id,
            row.model_turns_label,
            row.tool_calls_label,
            row.retry_label,
            row.output_bytes_label,
            row.status_label
        )
    }));
    rows.extend(workflows.risk_monitors.iter().map(|monitor| {
        format!(
            "legion workflow risk monitor {}: session={} state={:?} score={}/{} high_risk={} denied={} stale_mcp={} halt={:?}",
            monitor.monitor_id.0,
            monitor.session_id.0,
            monitor.state,
            monitor.risk_score,
            monitor.halt_threshold,
            monitor.high_risk_action_count,
            monitor.denied_tool_count,
            monitor.stale_mcp_registry_detected,
            monitor.halt_reason
        )
    }));
    rows.extend(workflows.kill_switches.iter().map(|switch| {
        format!(
            "legion workflow kill switch {}: session={} state={:?} reason={}",
            switch.kill_switch_id.0,
            switch.session_id.0,
            switch.state,
            switch.reason_label.as_deref().unwrap_or("<armed>")
        )
    }));
    rows.extend(workflows.tool_permission_requests.iter().map(|request| {
        format!(
            "legion workflow tool permission {}: profile={:?} action={:?} decision={:?} disposition={:?} runtime={} deny={}",
            request.request_id,
            request.profile,
            request.action_class,
            request.decision,
            request.disposition,
            request.runtime_allowed,
            request.deny_overrides
        )
    }));
    rows
}

pub(super) fn redaction_label(redaction_hints: &[legion_protocol::RedactionHint]) -> String {
    if redaction_hints.is_empty() {
        "none".to_string()
    } else {
        redaction_hints
            .iter()
            .take(4)
            .map(|hint| format!("{hint:?}"))
            .collect::<Vec<_>>()
            .join(",")
    }
}

pub(super) fn bounded_join(values: &[String]) -> String {
    if values.is_empty() {
        "<none>".to_string()
    } else {
        values
            .iter()
            .take(4)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }
}

pub(super) fn onboarding_rows(
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
) -> Vec<String> {
    if !state.first_run_onboarding_visible {
        return Vec::new();
    }

    let settings = DesktopSettingsViewModel::from_projection(&snapshot.settings_projection);
    DesktopSetupChecklistViewModel::from_snapshot(snapshot, &settings)
        .items
        .into_iter()
        .map(|item| format!("{} — {}", item.title, item.detail))
        .collect()
}

pub(super) fn manual_control_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let level = projected_product_mode(snapshot);
    let language = &snapshot.language_tooling_projection;
    let terminal = &snapshot.terminal_panel_projection;
    let search = DesktopSearchViewModel::from_projection(&snapshot.search_projection);
    let active = &snapshot.active_buffer_projection;
    let mut rows = Vec::new();

    if level != DesktopProductMode::Manual {
        rows.push(format!(
            "manual control center: inactive because active product mode is {}",
            level.label()
        ));
        return rows;
    }

    rows.push(
        "manual control center: AI Disabled; Local Tools Only; No Model Calls; No Agent Context"
            .to_string(),
    );
    rows.push(format!(
        "manual toolchain: language={:?} problems={} quick_fixes={} breadcrumbs={} sticky_scopes={} inlay_hints={} code_lenses={} completions={} terminal={:?} search={} structural_search={:?}/{} verification_runs={}",
        language.status,
        language.problems.len(),
        language.quick_fixes.len(),
        language.breadcrumbs.len(),
        language.sticky_scopes.len(),
        language.inlay_hints.len(),
        language.code_lenses.len(),
        language.completions.len(),
        terminal.status.kind,
        search.header,
        snapshot.structural_search_projection.status.kind,
        snapshot.structural_search_projection.matches.len(),
        snapshot.verification_run_projection.rows.len()
    ));
    rows.push(format!(
        "manual commands: save_all proposal-mediated; search/read/navigation intents only; no direct apply; statuses={}",
        snapshot.status_messages.len()
    ));
    rows.push(format!(
        "manual editor: dirty={} degraded={} active_buffer={:?} no autonomous writes",
        active.dirty,
        active.degraded,
        active.buffer_id.map(|buffer| buffer.0)
    ));
    rows.push(
        "manual trust boundary: no provider dispatch, no agent context, no terminal authority, no direct apply"
            .to_string(),
    );
    rows
}

pub(super) fn language_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let language = &snapshot.language_tooling_projection;
    let mut rows = Vec::new();
    if language.buffer_id.is_some()
        || !language.operations.is_empty()
        || !language.problems.is_empty()
        || !language.outline.is_empty()
        || !language.inlay_hints.is_empty()
        || !language.code_lenses.is_empty()
    {
        rows.push(format!(
            "language: {:?} problems={} quick_fixes={} breadcrumbs={} sticky_scopes={} inlay_hints={} code_lenses={} completions={} definitions={} references={} outline={} stale={} cancelled={}",
            language.status,
            language.problems.len(),
            language.quick_fixes.len(),
            language.breadcrumbs.len(),
            language.sticky_scopes.len(),
            language.inlay_hints.len(),
            language.code_lenses.len(),
            language.completions.len(),
            language.definitions.len(),
            language.references.len(),
            language.outline.len(),
            language.stale_result_count,
            language.cancellation_count
        ));
    }
    // Ahead of the ambient rows (problems, completions, references) because the
    // panel shows only the first dozen: call hierarchy is the answer to a
    // question the reader just asked, and an answer pushed off the end of the
    // list has not been rendered at all.
    rows.extend(call_hierarchy_rows(language));
    if let Some(hover) = &language.hover {
        rows.push(format!("hover {} {}", hover.hover_id, hover.label));
        rows.push(format!("hover docs {} {}", hover.label, hover.summary));
    }
    rows.extend(language.problems.iter().take(12).map(|problem| {
        let location = problem
            .path
            .as_ref()
            .map(|path| {
                if let Some(range) = &problem.range {
                    format!("{}:{}..{}", path.0, range.start.line, range.end.line)
                } else {
                    path.0.clone()
                }
            })
            .unwrap_or_else(|| "<unknown-path>".to_string());
        format!(
            "problem {} severity={:?} code={} source={} {}",
            location,
            problem.severity,
            problem.code_label.as_deref().unwrap_or("<none>"),
            problem.source_label.as_deref().unwrap_or("<none>"),
            problem.message
        )
    }));
    rows.extend(language.quick_fixes.iter().take(10).map(|quick_fix| {
        format!(
            "quick fix {} {} severity={:?} proposal={:?}",
            quick_fix.action_id,
            quick_fix.title,
            quick_fix.severity,
            quick_fix.proposal_id.map(|proposal| proposal.0)
        )
    }));
    rows.extend(language.breadcrumbs.iter().take(8).map(|breadcrumb| {
        format!(
            "breadcrumb {} {} kind={} depth={} source={}",
            breadcrumb.breadcrumb_id,
            breadcrumb.label,
            breadcrumb.kind_label,
            breadcrumb.depth,
            breadcrumb.source_label
        )
    }));
    rows.extend(language.sticky_scopes.iter().take(8).map(|scope| {
        format!(
            "sticky scope {} {} active={} kind={} depth={} source={}",
            scope.scope_id,
            scope.label,
            scope.active,
            scope.kind_label,
            scope.depth,
            scope.source_label
        )
    }));
    rows.extend(language.inlay_hints.iter().take(8).map(|hint| {
        format!(
            "inlay hint {} {} kind={} source={}",
            hint.hint_id, hint.label, hint.kind_label, hint.source_label
        )
    }));
    rows.extend(language.code_lenses.iter().take(8).map(|lens| {
        format!(
            "code lens {} {} command={} kind={} data={:?} source={}",
            lens.lens_id,
            lens.title,
            lens.command_label,
            lens.kind_label,
            lens.data_label,
            lens.source_label
        )
    }));
    rows.extend(language.completions.iter().take(20).map(|completion| {
        format!(
            "completion {} {} kind={} score={} detail={} degraded={}",
            completion.completion_id,
            completion.label,
            completion.kind_label,
            completion.score_basis_points,
            completion.detail_label.as_deref().unwrap_or("<none>"),
            completion.degraded
        )
    }));
    rows.extend(language.definitions.iter().take(12).map(|definition| {
        let location = definition
            .path
            .as_ref()
            .map(|path| {
                if let Some(range) = &definition.range {
                    format!("{}:{}", path.0, range.start.line)
                } else {
                    path.0.clone()
                }
            })
            .unwrap_or_else(|| "<unknown-path>".to_string());
        format!(
            "definition {} {} {} degraded={}",
            definition.location_id, location, definition.label, definition.degraded
        )
    }));
    rows.extend(language.references.iter().take(12).map(|reference| {
        let location = reference
            .path
            .as_ref()
            .map(|path| {
                if let Some(range) = &reference.range {
                    format!("{}:{}", path.0, range.start.line)
                } else {
                    path.0.clone()
                }
            })
            .unwrap_or_else(|| "<unknown-path>".to_string());
        format!(
            "reference {} {} {} degraded={}",
            reference.location_id, location, reference.label, reference.degraded
        )
    }));
    rows.extend(language.operations.iter().map(|operation| {
        format!(
            "language op {} {:?} {:?} proposal={:?}",
            operation.operation_id,
            operation.kind,
            operation.status,
            operation.proposal_id.map(|proposal| proposal.0)
        )
    }));
    rows
}

pub(super) fn symbol_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    snapshot
        .language_tooling_projection
        .outline
        .iter()
        .map(|symbol| {
            let mut row = format!(
                "{}{} · {}",
                "  ".repeat(usize::from(symbol.depth)),
                symbol.label,
                symbol.kind_label
            );
            if let Some(range) = &symbol.range {
                row.push_str(&format!(" · line {}", range.start.line.saturating_add(1)));
            }
            if symbol.children_omitted {
                row.push_str(" · more nested symbols");
            }
            row
        })
        .collect()
}

/// Projection-only LSP health rows derived from the snapshot (D2 wired).
///
/// Reads `LspServerHealthRecord` entries from
/// `snapshot.language_tooling_projection.lsp_health_records` — populated by
/// `AppComposition::shell_projection_snapshot()` via the background
/// `LspSessionHandle`.  Returns an empty vec when no health data is available.
/// No authority is claimed here; all rendering is projection-only read-only.
pub(super) fn lsp_health_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    use legion_ui::project_lsp_health;
    snapshot
        .language_tooling_projection
        .lsp_health_records
        .iter()
        .map(|record| {
            let proj = project_lsp_health(record, false);
            format!(
                "lsp server={} provenance={} version={} status={} restarts={}",
                proj.server_label,
                proj.provenance_label,
                proj.version_label,
                proj.status_label,
                proj.restart_count,
            )
        })
        .collect()
}

pub(super) fn structural_search_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let structural = &snapshot.structural_search_projection;
    let mut rows = Vec::new();

    if structural.query_id.is_some()
        || !structural.matches.is_empty()
        || !structural.diagnostics.is_empty()
        || structural.proposal_id.is_some()
    {
        rows.push(format!(
            "structural search: {:?} matches={} proposal={:?}",
            structural.status.kind,
            structural.matches.len(),
            structural.proposal_id.map(|proposal| proposal.0)
        ));
        rows.push(format!(
            "structural query: scope={:?} pattern={} rewrite={} limit={} omitted_matches={} omitted_files={} schema={}",
            structural.scope,
            structural.pattern_label,
            structural
                .rewrite_label
                .as_deref()
                .unwrap_or("<preview-only>"),
            structural.result_limit,
            structural.omitted_match_count,
            structural.omitted_file_count,
            structural.schema_version
        ));
    }

    for structural_match in structural.matches.iter().take(20) {
        rows.push(format!(
            "structural match {}:{} {} -> {}",
            structural_match.file_path.0,
            structural_match.range.start.line,
            structural_match.snippet,
            structural_match
                .replacement_preview
                .as_deref()
                .unwrap_or("<no rewrite>")
        ));
        for capture in structural_match.captures.iter().take(8) {
            rows.push(format!("capture {}={}", capture.name, capture.value));
        }
    }

    rows.extend(
        structural
            .diagnostics
            .iter()
            .take(8)
            .map(|diagnostic| format!("structural diagnostic {diagnostic}")),
    );
    rows
}

/// Projection-driven debug toolbar (B11): launch / step / continue / poll / stop.
///
/// Emits the same [`DesktopAction`]s keyboard and tests already use — no app
/// ownership in the renderer.
pub(super) fn render_debug_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let debug = &snapshot.debug_projection;
    ui.horizontal_wrapped(|ui| {
        if let Some(session_id) = debug.active_session_id.clone() {
            if ui.small_button("Continue").clicked() {
                actions.push(DesktopAction::DebugStep {
                    session_id: session_id.clone(),
                    kind: DebugStepKindProjection::Continue,
                });
            }
            if ui.small_button("Step Over").clicked() {
                actions.push(DesktopAction::DebugStep {
                    session_id: session_id.clone(),
                    kind: DebugStepKindProjection::Over,
                });
            }
            if ui.small_button("Step Into").clicked() {
                actions.push(DesktopAction::DebugStep {
                    session_id: session_id.clone(),
                    kind: DebugStepKindProjection::Into,
                });
            }
            if ui.small_button("Step Out").clicked() {
                actions.push(DesktopAction::DebugStep {
                    session_id: session_id.clone(),
                    kind: DebugStepKindProjection::Out,
                });
            }
            if ui.small_button("Poll").clicked() {
                actions.push(DesktopAction::PollDebugSession);
            }
            if ui.small_button("Stop").clicked() {
                actions.push(DesktopAction::StopDebugSession);
            }
        } else if let Some(configuration_id) = debug
            .configurations
            .first()
            .map(|config| config.configuration_id.clone())
        {
            if ui.small_button("Launch").clicked() {
                actions.push(DesktopAction::LaunchDebugSession { configuration_id });
            }
            if ui.small_button("Refresh configs").clicked() {
                actions.push(DesktopAction::RefreshDebugConfigurations);
            }
        } else if ui.small_button("Refresh configs").clicked() {
            actions.push(DesktopAction::RefreshDebugConfigurations);
        }
    });
}

pub(super) fn debug_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let debug = &snapshot.debug_projection;
    let mut rows = Vec::new();
    if debug.active_session_id.is_some()
        || !debug.configurations.is_empty()
        || !debug.breakpoints.is_empty()
        || !debug.stack_frames.is_empty()
        || !debug.variables.is_empty()
        || !debug.watches.is_empty()
        || !debug.console.is_empty()
        || !debug.inline_values.is_empty()
        || !debug.diagnostics.is_empty()
    {
        // Dual-mode honesty: live adapter vs simulated fixture (WS-A-D B3).
        //
        // Three states, not two. `live_adapter` is a property of the running
        // session, so with no session it answers a question nobody asked and
        // the old two-way branch turned its `false` into a claim about the
        // build. See `cut_lines::DEBUG_NO_SESSION_BANNER`.
        rows.push(format!(
            "debug: {}",
            if debug.active_session_id.is_none() {
                crate::cut_lines::DEBUG_NO_SESSION_BANNER
            } else if debug.live_adapter {
                crate::cut_lines::DEBUG_LIVE_BANNER
            } else {
                crate::cut_lines::DEBUG_SIMULATED_BANNER
            }
        ));
        if crate::debug_auto_poll::debug_needs_auto_poll(debug) {
            rows.push(
                "debug: auto-poll active (live continue; frame loop drains stop)".to_string(),
            );
        }
        rows.push(format!(
            "debug: status={:?} session={:?} state={:?} configs={} breakpoints={} frames={} variables={} watches={} console={} inline={} note={}",
            debug.status.kind,
            debug.active_session_id.as_ref().map(|session| session.0.as_str()),
            debug.session_state,
            debug.configurations.len(),
            debug.breakpoints.len(),
            debug.stack_frames.len(),
            debug.variables.len(),
            debug.watches.len(),
            debug.console.len(),
            debug.inline_values.len(),
            debug.status.message
        ));
    }
    rows.extend(debug.configurations.iter().take(8).map(|configuration| {
        format!(
            "debug config {} adapter={} program={} package={} target={} deterministic={}",
            configuration.configuration_id.0,
            configuration.adapter_type,
            configuration.program_label,
            configuration.cargo_package.as_deref().unwrap_or("<none>"),
            configuration.cargo_target.as_deref().unwrap_or("<none>"),
            configuration.deterministic
        )
    }));
    rows.extend(debug.breakpoints.iter().take(12).map(|breakpoint| {
        format!(
            "debug breakpoint {} {}:{} enabled={} verified={} condition={} hit={} log={}",
            breakpoint.breakpoint_id.0,
            breakpoint.path.0,
            breakpoint.line,
            breakpoint.enabled,
            breakpoint.verified,
            breakpoint.condition.as_deref().unwrap_or("<none>"),
            breakpoint.hit_condition.as_deref().unwrap_or("<none>"),
            breakpoint.log_message.as_deref().unwrap_or("<none>")
        )
    }));
    rows.extend(debug.stack_frames.iter().take(8).map(|frame| {
        let path = frame
            .path
            .as_ref()
            .map(|path| path.0.as_str())
            .unwrap_or("<unknown>");
        let line = frame
            .line
            .map(|line| line.to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        format!(
            "debug frame {}:{} {} {}:{}",
            frame.session_id.0, frame.frame_id, frame.name, path, line
        )
    }));
    rows.extend(debug.variables.iter().take(12).map(|variable| {
        format!(
            "debug variable {} {}={} type={} children={}",
            variable.session_id.0,
            variable.name,
            variable.value_label,
            variable.type_label.as_deref().unwrap_or("<none>"),
            variable.has_children
        )
    }));
    rows.extend(debug.watches.iter().take(8).map(|watch| {
        format!(
            "debug watch {} {} {}={} type={}",
            watch.session_id.0,
            watch.watch_id.0,
            watch.expression_label,
            watch.value_label,
            watch.type_label.as_deref().unwrap_or("<none>")
        )
    }));
    rows.extend(debug.console.iter().take(12).map(|entry| {
        format!(
            "debug console {} {}: {}",
            entry.session_id.0, entry.category_label, entry.message_label
        )
    }));
    rows.extend(debug.inline_values.iter().take(8).map(|inline_value| {
        format!(
            "debug inline {} {}:{} {}={}",
            inline_value.session_id.0,
            inline_value.path.0,
            inline_value.line,
            inline_value.expression_label,
            inline_value.value_label
        )
    }));
    rows.extend(
        debug
            .diagnostics
            .iter()
            .take(8)
            .map(|diagnostic| format!("debug diagnostic {diagnostic}")),
    );
    rows
}

pub(super) fn test_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let verification = &snapshot.verification_run_projection;
    let explorer = &snapshot.test_explorer_projection;
    let runnable_lenses = snapshot
        .language_tooling_projection
        .code_lenses
        .iter()
        .filter(|lens| lens.kind_label.contains("runnable"))
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    if explorer.status_label != "idle"
        || !explorer.items.is_empty()
        || !explorer.diagnostics.is_empty()
        || explorer.last_run_item_id.is_some()
    {
        let last_run = match (
            explorer.last_run_item_id.as_deref(),
            explorer.last_run_status.as_deref(),
        ) {
            (Some(id), Some(status)) => format!(
                "{id}:{status}:exit={}:{}ms",
                explorer
                    .last_run_exit_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "n/a".to_string()),
                explorer
                    .last_run_duration_ms
                    .map(|ms| ms.to_string())
                    .unwrap_or_else(|| "n/a".to_string())
            ),
            _ => "none".to_string(),
        };
        let groups = legion_ui::group_test_explorer_items_by_parent(&explorer.items);
        rows.push(format!(
            "test explorer: status={} controller={} items={} groups={} last_run={} diagnostics={}",
            explorer.status_label,
            explorer.controller_label,
            explorer.items.len(),
            groups.len(),
            last_run,
            if explorer.diagnostics.is_empty() {
                "none".to_string()
            } else {
                explorer.diagnostics.join(",")
            }
        ));
        rows.extend(legion_ui::format_test_explorer_tree_rows(
            &explorer.items,
            legion_ui::MAX_TEST_EXPLORER_TREE_DISPLAY_ROWS,
        ));
    }
    if !verification.rows.is_empty() || !runnable_lenses.is_empty() {
        rows.push(format!(
            "test explorer: verification_runs={} runnable_lenses={} omitted={} projection={}",
            verification.rows.len(),
            runnable_lenses.len(),
            verification.omitted_row_count,
            verification.projection_id
        ));
    }
    rows.extend(verification.rows.iter().take(12).map(|row| {
        let targets = if row.target_labels.is_empty() {
            "<none>".to_string()
        } else {
            row.target_labels.join(",")
        };
        let exit_code = row
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "pending".to_string());
        format!(
            "run {}: label={} state={:?} class={} targets={} exit={} evidence={} body_redacted={}",
            row.run_id,
            row.label,
            row.state,
            row.command_class_label,
            targets,
            exit_code,
            row.evidence_artifact_id.as_deref().unwrap_or("<none>"),
            row.command_body_redacted
        )
    }));
    rows.extend(runnable_lenses.into_iter().take(12).map(|lens| {
        let range_label = lens.range.as_ref().map_or_else(
            || "<none>".to_string(),
            |range| {
                format!(
                    "{}:{}..{}:{}",
                    range.start.line, range.start.character, range.end.line, range.end.character
                )
            },
        );
        format!(
            "runnable lens {}: title={} command={} source={} range={} data={}",
            lens.lens_id,
            lens.title,
            lens.command_label,
            lens.source_label,
            range_label,
            lens.data_label.as_deref().unwrap_or("<none>")
        )
    }));
    rows
}

pub(super) fn terminal_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let terminal = &snapshot.terminal_panel_projection;
    let mut rows = Vec::new();
    if terminal.active_session_id.is_some()
        || terminal.last_denial.is_some()
        || terminal.last_error.is_some()
        || !terminal.output_rows.is_empty()
    {
        rows.push(format!(
            "terminal: {:?} session={:?} rows={} omitted={} matches={}",
            terminal.status.kind,
            terminal.active_session_id.map(|session| session.0),
            terminal.output_rows.len(),
            terminal.scrollback.omitted_row_count,
            terminal.search.match_count
        ));
    }
    if let Some(policy) = &terminal.policy {
        rows.push(format!(
            "terminal policy: capability={} trust={:?} granted={} reason={}",
            policy.capability_id.0, policy.workspace_trust_state, policy.granted, policy.reason
        ));
    }
    if let Some(denial) = &terminal.last_denial {
        rows.push(format!("terminal denial: {denial}"));
    }
    if let Some(error) = &terminal.last_error {
        rows.push(format!("terminal error: {error}"));
    }
    rows.extend(terminal.output_rows.iter().take(5).map(|row| {
        format!(
            "terminal output {}: {}",
            row.sequence.0, row.redacted_payload
        )
    }));
    rows
}

pub(super) fn operational_health_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    DesktopOperationalHealthSnapshot::from_projection(snapshot).rows()
}

pub(super) fn plugin_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    for projection in &snapshot.plugin_contribution_projections {
        let commands = plugin_command_descriptors(projection);
        let other_contribution_count = projection
            .contributions
            .len()
            .saturating_sub(commands.len());
        rows.push(format!(
            "plugin management plugin {}: status={} contributions={} commands={} other={} sandbox=metadata-only {} audit=app-owned",
            projection.plugin_id.0,
            projection.status_label,
            projection.contributions.len(),
            commands.len(),
            other_contribution_count,
            crate::cut_lines::PLUGIN_EXECUTION_UNAVAILABLE
        ));
        if commands.is_empty() {
            rows.push(format!(
                "plugin management plugin {}: no projected commands",
                projection.plugin_id.0
            ));
        }
        rows.extend(commands.into_iter().map(|command| {
            format!(
                "plugin management plugin {} command {}: {} capability={} audit=dispatch-intent-only",
                projection.plugin_id.0,
                command.command_id,
                command.title,
                command.required_capability.0
            )
        }));
        // Surface the app-owned permission review rows shown before install
        // approval so the capability disclosure is visible in the plugin panel.
        rows.extend(projection.permission_review_rows.iter().map(|review| {
            format!(
                "plugin management plugin {} {}",
                projection.plugin_id.0, review
            )
        }));
    }
    rows
}

pub(super) fn plugin_command_descriptors(
    projection: &PluginContributionProjection,
) -> Vec<&PluginCommandDescriptor> {
    projection
        .contributions
        .iter()
        .filter_map(|contribution| match contribution {
            PluginContribution::Command(command) => Some(command),
            _ => None,
        })
        .collect()
}

pub(super) fn collaboration_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    let projection = &snapshot.collaboration_gui_projection;
    if !projection.runtime_enabled
        && !projection.presence_enabled
        && projection.session_rows.is_empty()
        && projection.shared_proposal_rows.is_empty()
        && snapshot.collaboration_presence_projections.is_empty()
    {
        return rows;
    }
    rows.push(format!(
        "collaboration: status={} runtime_enabled={} presence_enabled={} sessions={} reconnecting={} conflicts={} offline={} shared_proposals={} redaction=metadata-only",
        projection.status_label,
        projection.runtime_enabled,
        projection.presence_enabled,
        projection.session_rows.len(),
        projection.reconnecting_session_count,
        projection.conflict_session_count,
        projection.offline_session_count,
        projection.shared_proposal_rows.len()
    ));
    rows.extend(projection.session_rows.iter().map(|session| {
        format!(
            "collaboration session {}: state={:?} participants={} presence={} reconnecting={} conflicts={} operations={} acknowledgements={} gaps={} offline={} status={}",
            session.session_id.0,
            session.state,
            session.participant_count,
            session.presence_count,
            session.reconnecting_participant_count,
            session.conflict_count,
            session.operation_count,
            session.acknowledgement_count,
            session.causal_gap_count,
            session.offline,
            session.status_label
        )
    }));
    rows.extend(projection.shared_proposal_rows.iter().map(|review| {
        format!(
            "shared proposal session {} proposal {}: required={} authorized={} approvals={} denials={} pending={} operations={} stale={} status={} proposal-mediated",
            review.session_id.0,
            review.proposal_id.0,
            review.required_approver_count,
            review.authorized_approver_count,
            review.approval_count,
            review.denial_count,
            review.pending_count,
            review.applied_operation_count,
            review.stale,
            review.status_label
        )
    }));
    rows.extend(
        snapshot
            .collaboration_presence_projections
            .iter()
            .map(|presence| {
                format!(
                    "collaboration presence {} participant {} reconnecting={} activity={}",
                    presence.session_id.0,
                    presence.participant_id.0,
                    presence.reconnecting,
                    presence.activity_label.as_deref().unwrap_or("<none>")
                )
            }),
    );
    rows
}

pub(super) fn remote_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    let projection = &snapshot.remote_gui_projection;
    if !projection.runtime_enabled
        && projection.session_rows.is_empty()
        && projection.proposal_review_rows.is_empty()
    {
        return rows;
    }
    rows.push(format!(
        "remote workspace: status={} runtime_enabled={} sessions={} connected={} reconnecting={} offline={} proposal_reviews={} redaction=metadata-only",
        projection.status_label,
        projection.runtime_enabled,
        projection.session_rows.len(),
        projection.connected_session_count,
        projection.reconnecting_session_count,
        projection.offline_session_count,
        projection.proposal_review_rows.len()
    ));
    rows.extend(projection.session_rows.iter().map(|session| {
        format!(
            "remote workspace session {} authority={} agent={} state={:?} filesystem={} terminal={} lsp={} reconnect_supported={} reconnecting={} offline={} proposal_reviews={} status={}",
            session.session_id.0,
            session.authority_label,
            session.agent_version,
            session.state,
            session.filesystem_descriptor_status,
            session.terminal_descriptor_status,
            session.lsp_descriptor_status,
            session.reconnect_supported,
            session.reconnecting,
            session.offline,
            session.proposal_review_count,
            session.status_label
        )
    }));
    rows.extend(projection.proposal_review_rows.iter().map(|review| {
        format!(
            "remote proposal session {} proposal {} authority={} payload={:?} lifecycle={:?} status={} proposal-mediated={}",
            review.session_id.0,
            review.proposal_id.0,
            review.remote_authority_label,
            review.payload_kind,
            review.lifecycle_state,
            review.status_label,
            review.proposal_mediated
        )
    }));
    rows
}
