//! Delegate workflow state: the transcript, its citations and its routes.
//!
//! Extracted from `lib.rs` because the chokepoint gate asked for it, and it is
//! a coherent piece: everything a Delegate conversation accumulates in memory,
//! and the one function that copies it into a projection.

use super::*;

#[derive(Debug, Clone)]
pub(crate) struct DelegateWorkflowState {
    /// Where each Delegate turn's provider request went.
    ///
    /// Held here rather than in the command outcome, which the desktop reads
    /// for its citation count and then drops -- so the transcript kept no
    /// reviewer-visible destination for a turn that may have uploaded an
    /// excerpt. Newest last, bounded, metadata only.
    pub(crate) provider_routes: Vec<legion_protocol::DelegatedTaskProviderRoute>,
    pub(crate) chat_messages: Vec<DelegatedTaskChatMessage>,
    pub(crate) context_citations: Vec<DelegatedTaskContextCitation>,
    pub(crate) hunk_decisions: HashMap<(ProposalId, String), DelegatedTaskProposalHunkDisposition>,
    pub(crate) tool_permission_requests: HashMap<String, DelegatedTaskToolPermissionRequest>,
    pub(crate) runtime_activation: DelegatedTaskRuntimeActivationState,
    pub(crate) next_message_sequence: u64,
    /// Last live sandbox enforcement summary from tool-host spawns (display-safe).
    pub(crate) last_sandbox_enforcement_label: Option<String>,
}

impl Default for DelegateWorkflowState {
    fn default() -> Self {
        Self {
            chat_messages: Vec::new(),
            context_citations: Vec::new(),
            provider_routes: Vec::new(),
            hunk_decisions: HashMap::new(),
            tool_permission_requests: HashMap::new(),
            runtime_activation: DelegatedTaskRuntimeActivationState::NotEncoded,
            next_message_sequence: 0,
            last_sandbox_enforcement_label: None,
        }
    }
}

impl DelegateWorkflowState {
    pub(crate) fn next_message_id(&mut self, role: DelegatedTaskChatRole) -> String {
        self.next_message_sequence = self.next_message_sequence.saturating_add(1);
        format!("delegate:{role:?}:{}", self.next_message_sequence)
    }

    #[cfg_attr(not(feature = "ai"), allow(dead_code))]
    pub(crate) fn set_runtime_activation(
        &mut self,
        runtime_activation: DelegatedTaskRuntimeActivationState,
    ) {
        self.runtime_activation = runtime_activation;
    }

    #[cfg(any(test, feature = "test-helpers"))]
    pub(crate) fn record_system_message(
        &mut self,
        content_label: impl Into<String>,
        plan_id: Option<DelegatedTaskPlanId>,
        proposal_id: Option<ProposalId>,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
    ) -> String {
        let message_id = self.next_message_id(DelegatedTaskChatRole::System);
        self.chat_messages.push(DelegatedTaskChatMessage {
            message_id: message_id.clone(),
            role: DelegatedTaskChatRole::System,
            content_label: content_label.into(),
            plan_id,
            proposal_id,
            citation_ids: Vec::new(),
            tool_permission_request_ids: Vec::new(),
            correlation_id,
            causality_id,
            created_at: TimestampMillis::now(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        });
        message_id
    }

    pub(crate) fn record_tool_permission(&mut self, request: DelegatedTaskToolPermissionRequest) {
        self.tool_permission_requests
            .insert(request.request_id.clone(), request);
    }

    pub(crate) fn tool_permission(
        &self,
        request_id: &str,
    ) -> Option<&DelegatedTaskToolPermissionRequest> {
        self.tool_permission_requests.get(request_id)
    }

    pub(crate) fn record_tool_permission_decision(
        &mut self,
        mut input: DelegatedTaskToolPermissionRequestInput,
    ) -> DelegatedTaskToolPermissionRequest {
        let effective_decision = if self
            .tool_permission_requests
            .get(&input.request_id)
            .is_some_and(|request| request.deny_overrides)
        {
            DelegatedTaskToolPermissionDecision::Deny
        } else {
            input.decision
        };
        input.decision = effective_decision;
        let request = delegated_task_tool_permission_request(input);
        self.record_tool_permission(request.clone());
        request
    }

    pub(crate) fn apply_to_projection(
        &self,
        projection: &mut DelegatedTaskProjection,
        proposal_ledger: &ProposalLedgerProjection,
    ) {
        projection.chat_messages = self.chat_messages.clone();
        projection.context_citations = self.context_citations.clone();
        projection.provider_routes = self.provider_routes.clone();
        projection.proposal_reviews = proposal_ledger
            .rows
            .iter()
            .filter(|row| row.diff_summary.hunk_count > 0 || !row.diff_summary.chunks.is_empty())
            .map(|row| self.review_for_row(row))
            .collect();
        projection.tool_permission_requests = self
            .tool_permission_requests
            .values()
            .cloned()
            .collect::<Vec<_>>();
        projection.tool_permission_requests.sort_by(|left, right| {
            left.request_id
                .cmp(&right.request_id)
                .then_with(|| format!("{:?}", left.profile).cmp(&format!("{:?}", right.profile)))
        });
        projection.runtime_activation = self.runtime_activation;
        projection.chat_message_count = projection.chat_messages.len() as u32;
        projection.context_citation_count = projection.context_citations.len() as u32;
        projection.proposal_review_count = projection.proposal_reviews.len() as u32;
        projection.tool_permission_request_count = projection.tool_permission_requests.len() as u32;
        if let Some(label) = &self.last_sandbox_enforcement_label
            && !projection
                .plan_only_disclaimers
                .iter()
                .any(|existing| existing == label)
        {
            projection.plan_only_disclaimers.push(label.clone());
        }
    }

    pub(crate) fn review_for_row(&self, row: &ProposalLedgerRow) -> DelegatedTaskProposalReview {
        let chunks = proposal_review_chunks(row)
            .into_iter()
            .map(|chunk| {
                let hunk_id = delegate_hunk_id(row.proposal_id, &chunk);
                let disposition = self
                    .hunk_decisions
                    .get(&(row.proposal_id, hunk_id.clone()))
                    .copied()
                    .unwrap_or(DelegatedTaskProposalHunkDisposition::Pending);
                DelegatedTaskProposalHunkReview {
                    hunk_id,
                    proposal_id: row.proposal_id,
                    target_id: chunk.target_id.clone(),
                    payload_kind: row.payload_kind,
                    path: target_path_for_chunk(row, chunk.target_id.as_deref()),
                    byte_range: chunk.byte_range,
                    changed_line_count: chunk.changed_line_count,
                    inserted_line_count: chunk.inserted_line_count,
                    deleted_line_count: chunk.deleted_line_count,
                    content_hash: chunk.content_hash.clone(),
                    disposition,
                    risk_label: row.risk_label,
                    privacy_label: row.privacy_label,
                    labels: vec!["delegate.proposal_hunk.human_review".to_string()],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                }
            })
            .collect::<Vec<_>>();
        DelegatedTaskProposalReview::from_hunks(
            format!("delegate:review:{}", row.proposal_id.0),
            row.proposal_id,
            chunks,
            vec!["delegate.proposal_review.human_approval_queue".to_string()],
            1,
        )
    }
}
