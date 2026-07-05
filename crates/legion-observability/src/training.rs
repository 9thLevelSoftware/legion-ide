//! Training-candidate helpers for consented acceptance/rejection metadata.

use crate::ObservabilityError;
use legion_protocol::{
    AssistedAiAuditOutcomeCategory, AssistedAiAuditRecord, AssistedAiAuditRedactionState,
    AssistedAiConsentState, AssistedAiProviderInvocationState, CausalityId, CorrelationId,
    EventSequence, FileFingerprint, PermissionBudgetEvaluationDisposition, ProposalAuditRecord,
    ProposalId, ProposalLifecycleState, ProposalPayloadSummary, ProposalPrivacyLabel,
    ProposalRiskLabel, RedactionHint, validate_assisted_ai_audit_record,
};
use serde::{Deserialize, Serialize};

/// Training label derived from consented proposal lifecycle metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainingCandidateLabel {
    /// The trace corresponds to an accepted proposal.
    Accepted,
    /// The trace corresponds to a rejected proposal.
    Rejected,
}

/// Metadata-only training candidate derived from a consented acceptance or rejection trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingCandidate {
    /// Stable training-candidate identifier.
    pub candidate_id: String,
    /// Source assisted-AI audit record identifier.
    pub audit_id: String,
    /// Proposal identifier tied to the acceptance/rejection outcome.
    pub proposal_id: ProposalId,
    /// Acceptance/rejection label.
    pub label: TrainingCandidateLabel,
    /// Consent posture for the trace.
    pub consent_state: AssistedAiConsentState,
    /// Proposal lifecycle state recorded for the trace.
    pub proposal_lifecycle_state: ProposalLifecycleState,
    /// Audit outcome category captured for the trace.
    pub outcome_category: AssistedAiAuditOutcomeCategory,
    /// Proposal payload summary for eval comparison.
    pub proposal_payload_summary: ProposalPayloadSummary,
    /// Request-contract hash that anchors the trace.
    pub request_contract_hash: FileFingerprint,
    /// Route-decision hash that anchors the trace.
    pub route_decision_hash: FileFingerprint,
    /// Preview hash when the trace produced preview metadata.
    pub preview_hash: Option<FileFingerprint>,
    /// Correlation identifier for replay stitching.
    pub correlation_id: CorrelationId,
    /// Causality identifier for replay stitching.
    pub causality_id: CausalityId,
    /// Event sequence for deterministic ordering.
    pub event_sequence: EventSequence,
    /// Redaction hints preserved for the training artifact.
    pub redaction_hints: Vec<RedactionHint>,
    /// Audit redaction state.
    pub redaction_state: AssistedAiAuditRedactionState,
    /// Runtime invocation state; always metadata-only here.
    pub runtime_invocation_state: AssistedAiProviderInvocationState,
    /// Budget dispositions copied into the candidate for eval reproducibility.
    pub budget_dispositions: Vec<PermissionBudgetEvaluationDisposition>,
    /// Proposal risk labels preserved in the training artifact.
    pub risk_labels: Vec<ProposalRiskLabel>,
    /// Proposal privacy labels preserved in the training artifact.
    pub privacy_labels: Vec<ProposalPrivacyLabel>,
    /// Schema version for the training candidate DTO.
    pub schema_version: u16,
}

/// Build a metadata-only training candidate from a consented audit/proposal pair.
///
/// Returns `Ok(None)` when the trace is not consented or does not correspond to an
/// acceptance/rejection lifecycle state.
pub fn consented_training_candidate_from_records(
    audit: &AssistedAiAuditRecord,
    proposal: &ProposalAuditRecord,
) -> Result<Option<TrainingCandidate>, ObservabilityError> {
    let Some(consent_state) = audit.consent_disposition else {
        return Ok(None);
    };
    if !matches!(
        consent_state,
        AssistedAiConsentState::Granted | AssistedAiConsentState::NotRequired
    ) {
        return Ok(None);
    }

    validate_assisted_ai_audit_record(audit).map_err(|_| ObservabilityError::InvalidPayload)?;

    let label = match proposal.lifecycle_state {
        ProposalLifecycleState::Approved => TrainingCandidateLabel::Accepted,
        ProposalLifecycleState::Rejected => TrainingCandidateLabel::Rejected,
        _ => return Ok(None),
    };

    let Some(proposal_id) = audit.proposal_id else {
        return Ok(None);
    };
    if proposal_id != proposal.proposal_id {
        return Err(ObservabilityError::InvalidPayload);
    }

    let candidate = TrainingCandidate {
        candidate_id: format!(
            "training-candidate:{}:{}",
            audit.audit_id,
            lifecycle_state_slug(proposal.lifecycle_state)
        ),
        audit_id: audit.audit_id.clone(),
        proposal_id,
        label,
        consent_state,
        proposal_lifecycle_state: proposal.lifecycle_state,
        outcome_category: audit.outcome_category,
        proposal_payload_summary: proposal.payload_summary.clone(),
        request_contract_hash: audit.request_contract_hash.clone(),
        route_decision_hash: audit.route_decision_hash.clone(),
        preview_hash: audit.preview_hash.clone(),
        correlation_id: audit.correlation_id,
        causality_id: audit.causality_id,
        event_sequence: audit.event_sequence,
        redaction_hints: audit.redaction_hints.clone(),
        redaction_state: audit.redaction_state,
        runtime_invocation_state: audit.runtime_invocation_state,
        budget_dispositions: audit.budget_dispositions.clone(),
        risk_labels: audit.risk_labels.clone(),
        privacy_labels: audit.privacy_labels.clone(),
        schema_version: audit.schema_version,
    };

    Ok(Some(candidate))
}

fn lifecycle_state_slug(state: ProposalLifecycleState) -> &'static str {
    match state {
        ProposalLifecycleState::Approved => "accepted",
        ProposalLifecycleState::Rejected => "rejected",
        ProposalLifecycleState::Created => "created",
        ProposalLifecycleState::Validated => "validated",
        ProposalLifecycleState::Previewed => "previewed",
        ProposalLifecycleState::Applied => "applied",
        ProposalLifecycleState::Denied => "denied",
        ProposalLifecycleState::Failed => "failed",
        ProposalLifecycleState::RolledBack => "rolled_back",
        ProposalLifecycleState::Stale => "stale",
        ProposalLifecycleState::Conflict => "conflict",
        ProposalLifecycleState::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        AssistedAiAuditPrivacyDisposition, CausalityId, CorrelationId, EventSequence,
        FileFingerprint, ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel,
        RedactionHint, TimestampMillis,
    };
    use serde_json;
    use uuid::Uuid;

    fn audit_record(consent: AssistedAiConsentState) -> AssistedAiAuditRecord {
        AssistedAiAuditRecord {
            audit_id: "assist:audit:req-1:77".to_string(),
            provider_capability_id: "provider:local-redacted".to_string(),
            provider_capability_hash: FileFingerprint {
                algorithm: "hash".to_string(),
                value: "provider-hash".to_string(),
            },
            route_decision_id: "assist:route:req-1".to_string(),
            route_decision_hash: FileFingerprint {
                algorithm: "hash".to_string(),
                value: "route-hash".to_string(),
            },
            consent_disposition: Some(consent),
            budget_dispositions: vec![PermissionBudgetEvaluationDisposition::Allowed],
            privacy_disposition: AssistedAiAuditPrivacyDisposition::Allowed,
            request_contract_id: "assist:req:1".to_string(),
            request_contract_hash: FileFingerprint {
                algorithm: "hash".to_string(),
                value: "request-hash".to_string(),
            },
            projection_id: Some("assisted-ai:p6-3".to_string()),
            projection_hash: Some(FileFingerprint {
                algorithm: "hash".to_string(),
                value: "projection-hash".to_string(),
            }),
            preview_id: Some("assist:preview:701".to_string()),
            preview_hash: Some(FileFingerprint {
                algorithm: "hash".to_string(),
                value: "preview-hash".to_string(),
            }),
            proposal_id: Some(ProposalId(701)),
            outcome_category: AssistedAiAuditOutcomeCategory::ProposalPreviewReady,
            refusal_error_category: None,
            correlation_id: CorrelationId(901),
            causality_id: CausalityId(
                Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            ),
            event_sequence: EventSequence(77),
            risk_labels: vec![ProposalRiskLabel::Medium],
            privacy_labels: vec![ProposalPrivacyLabel::WorkspaceMetadata],
            redaction_state: AssistedAiAuditRedactionState::MetadataOnly,
            runtime_invocation_state: AssistedAiProviderInvocationState::NotEncoded,
            runtime_activation_labels: vec![
                "provider.invocation.not_encoded".to_string(),
                "network.not_encoded".to_string(),
                "tool.disabled".to_string(),
                "agent.disabled".to_string(),
                "terminal.disabled".to_string(),
            ],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn proposal_record(state: ProposalLifecycleState) -> ProposalAuditRecord {
        ProposalAuditRecord {
            proposal_id: ProposalId(701),
            lifecycle_state: state,
            timestamp: TimestampMillis(1_717_171_717),
            principal: legion_protocol::PrincipalId("principal-1".to_string()),
            capability: legion_protocol::CapabilityId("cap:proposal".to_string()),
            correlation_id: CorrelationId(901),
            causality_id: CausalityId(
                Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            ),
            payload_summary: ProposalPayloadSummary {
                kind: ProposalPayloadKind::TextEdit,
                affected_files: vec![],
                title: Some("Fix acceptance edge case".to_string()),
                byte_count: Some(144),
            },
            checkpoint_rollback_projection: None,
            risk_rule_ids: vec!["risk.rule.accepted".to_string()],
            diagnostics: vec![],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn consented_approved_trace_becomes_training_candidate() {
        let candidate = consented_training_candidate_from_records(
            &audit_record(AssistedAiConsentState::Granted),
            &proposal_record(ProposalLifecycleState::Approved),
        )
        .expect("candidate conversion")
        .expect("consented approved trace should be retained");

        assert_eq!(
            candidate.candidate_id,
            "training-candidate:assist:audit:req-1:77:accepted"
        );
        assert_eq!(candidate.label, TrainingCandidateLabel::Accepted);
        assert_eq!(candidate.consent_state, AssistedAiConsentState::Granted);
        assert_eq!(
            candidate.proposal_lifecycle_state,
            ProposalLifecycleState::Approved
        );
        assert_eq!(candidate.proposal_id, ProposalId(701));
        assert_eq!(
            candidate.outcome_category,
            AssistedAiAuditOutcomeCategory::ProposalPreviewReady
        );
        assert_eq!(
            candidate.proposal_payload_summary.title.as_deref(),
            Some("Fix acceptance edge case")
        );
    }

    #[test]
    fn denied_consents_are_filtered_from_training_candidates() {
        let candidate = consented_training_candidate_from_records(
            &audit_record(AssistedAiConsentState::Denied),
            &proposal_record(ProposalLifecycleState::Rejected),
        )
        .expect("candidate conversion");

        assert!(
            candidate.is_none(),
            "non-consented traces must not land in the candidate set"
        );
    }

    #[test]
    fn non_metadata_only_audit_traces_are_rejected_before_training_candidate_creation() {
        let mut audit = audit_record(AssistedAiConsentState::Granted);
        audit.redaction_state = AssistedAiAuditRedactionState::FullyRedacted;
        audit.runtime_invocation_state = AssistedAiProviderInvocationState::Completed;

        let err = consented_training_candidate_from_records(
            &audit,
            &proposal_record(ProposalLifecycleState::Approved),
        )
        .expect_err("fully redacted traces must be rejected at the boundary");

        assert_eq!(err, ObservabilityError::InvalidPayload);
    }

    #[test]
    fn training_candidate_fixture_round_trips() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../evals/training-candidates/consented_accept_reject.jsonl"
        ));
        let candidates = fixture
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str::<TrainingCandidate>(line).expect("valid candidate json")
            })
            .collect::<Vec<_>>();

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].label, TrainingCandidateLabel::Accepted);
        assert_eq!(candidates[1].label, TrainingCandidateLabel::Rejected);
        assert_eq!(
            candidates[0].candidate_id,
            "training-candidate:assist:audit:req-1:77:accepted"
        );
        assert_eq!(
            candidates[1].candidate_id,
            "training-candidate:assist:audit:req-1:77:rejected"
        );
    }
}
