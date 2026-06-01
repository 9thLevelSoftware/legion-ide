//! Opt-in long-term memory: embedding references, retention policies, consent.

#![warn(missing_docs)]

use devil_protocol::{
    AgentRunId, AssistedAiContractError, CausalityId, CorrelationId, EventSequence,
    FileFingerprint, LegionWorkflowModelBackend, LegionWorkflowSession, Phase4RuntimeAuditRecord,
    PrivacyClassification, RedactionHint, validate_phase4_runtime_audit_record,
};
use thiserror::Error;

/// Memory service errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoryError {
    /// Retention was requested without explicit consent.
    #[error("memory retention requires explicit consent")]
    ConsentRequired,
    /// A metadata record failed protocol validation.
    #[error("invalid memory metadata: {0}")]
    InvalidMetadata(#[from] AssistedAiContractError),
}

/// Consent state for a memory candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryConsentState {
    /// No consent has been granted.
    NotGranted,
    /// Session-scoped retention has been approved.
    SessionOnly,
    /// Project-scoped long-term retention has been approved.
    ProjectLongTerm,
}

/// Metadata-only memory candidate proposed by AI and reviewed by a user/app authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCandidateRecord {
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Optional agent run link.
    pub run_id: Option<AgentRunId>,
    /// Consent state.
    pub consent: MemoryConsentState,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
}

/// Metadata-only memory service.
#[derive(Debug, Default)]
pub struct MemoryService {
    retained: Vec<MemoryCandidateRecord>,
    workflow_retained: Vec<LegionWorkflowOutcomeCandidate>,
}

impl MemoryService {
    /// Creates an empty memory service.
    pub fn new() -> Self {
        Self::default()
    }

    /// Proposes a candidate without retaining it.
    pub fn propose_candidate(
        &self,
        candidate: MemoryCandidateRecord,
    ) -> Result<MemoryCandidateRecord, MemoryError> {
        validate_memory_candidate(&candidate)?;
        Ok(candidate)
    }

    /// Retains a candidate only when explicit consent is present.
    pub fn retain(&mut self, candidate: MemoryCandidateRecord) -> Result<(), MemoryError> {
        validate_memory_candidate(&candidate)?;
        if candidate.consent == MemoryConsentState::NotGranted {
            return Err(MemoryError::ConsentRequired);
        }
        self.retained.push(candidate);
        Ok(())
    }

    /// Deletes a retained memory candidate by id.
    pub fn delete(&mut self, candidate_id: &str) -> bool {
        let before = self.retained.len();
        self.retained
            .retain(|candidate| candidate.candidate_id != candidate_id);
        before != self.retained.len()
    }

    /// Returns retained metadata records.
    pub fn retained(&self) -> &[MemoryCandidateRecord] {
        &self.retained
    }

    /// Proposes a Legion workflow outcome candidate without retaining it.
    pub fn propose_legion_workflow_candidate(
        &self,
        candidate: LegionWorkflowOutcomeCandidate,
    ) -> Result<LegionWorkflowOutcomeCandidate, MemoryError> {
        validate_legion_workflow_candidate(&candidate)?;
        Ok(candidate)
    }

    /// Retains a Legion workflow outcome candidate only with explicit consent.
    pub fn retain_legion_workflow_candidate(
        &mut self,
        candidate: LegionWorkflowOutcomeCandidate,
    ) -> Result<(), MemoryError> {
        validate_legion_workflow_candidate(&candidate)?;
        if candidate.consent == MemoryConsentState::NotGranted {
            return Err(MemoryError::ConsentRequired);
        }
        self.workflow_retained.push(candidate);
        Ok(())
    }

    /// Deletes a retained Legion workflow outcome candidate by id.
    pub fn delete_legion_workflow_candidate(&mut self, candidate_id: &str) -> bool {
        let before = self.workflow_retained.len();
        self.workflow_retained
            .retain(|candidate| candidate.candidate_id != candidate_id);
        before != self.workflow_retained.len()
    }

    /// Returns retained Legion workflow outcome candidates.
    pub fn retained_legion_workflow_candidates(&self) -> &[LegionWorkflowOutcomeCandidate] {
        &self.workflow_retained
    }

    /// Looks up retained Legion workflow candidates by workflow session id.
    pub fn legion_workflow_candidates_by_session_id(
        &self,
        workflow_session_id: &str,
    ) -> Vec<&LegionWorkflowOutcomeCandidate> {
        self.workflow_retained
            .iter()
            .filter(|candidate| candidate.workflow_session_id == workflow_session_id)
            .collect()
    }

    /// Looks up retained Legion workflow candidates by worker role/backend label.
    pub fn legion_workflow_candidates_by_worker_role_backend(
        &self,
        worker_role_backend_label: &str,
    ) -> Vec<&LegionWorkflowOutcomeCandidate> {
        self.workflow_retained
            .iter()
            .filter(|candidate| candidate.worker_role_backend_label == worker_role_backend_label)
            .collect()
    }
}

fn validate_memory_candidate(candidate: &MemoryCandidateRecord) -> Result<(), MemoryError> {
    validate_phase4_runtime_audit_record(&Phase4RuntimeAuditRecord {
        audit_id: format!("memory:{}", candidate.candidate_id),
        run_id: candidate.run_id.clone(),
        step_id: None,
        provider_route_id: None,
        invocation_state: devil_protocol::AssistedAiProviderInvocationState::NotEncoded,
        outcome_label: "memory.candidate.metadata_only".to_string(),
        labels: candidate.labels.clone(),
        correlation_id: candidate.correlation_id,
        causality_id: candidate.causality_id,
        event_sequence: candidate.event_sequence,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    })?;
    Ok(())
}

/// Consent-aware metadata-only candidate for Legion workflow outcome learning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegionWorkflowOutcomeCandidate {
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Workflow session identifier.
    pub workflow_session_id: String,
    /// Display-safe worker role/backend label.
    pub worker_role_backend_label: String,
    /// Display-safe outcome label.
    pub outcome_label: String,
    /// Verification state label.
    pub verification_state_label: String,
    /// Sign-off state label.
    pub sign_off_state_label: String,
    /// Conflict count represented by this candidate.
    pub conflict_count: u32,
    /// Proposal count represented by this candidate.
    pub proposal_count: u32,
    /// Metadata summary hash.
    pub summary_hash: FileFingerprint,
    /// Consent state.
    pub consent: MemoryConsentState,
    /// Privacy labels represented by this candidate.
    pub privacy_labels: Vec<PrivacyClassification>,
    /// Whether raw payloads were retained. Must remain false by default.
    pub raw_payload_retained: bool,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Candidate schema version.
    pub schema_version: u16,
}

impl LegionWorkflowOutcomeCandidate {
    /// Builds an outcome candidate from protocol session metadata only.
    pub fn from_session_metadata(
        session: &LegionWorkflowSession,
        consent: MemoryConsentState,
        summary_hash: FileFingerprint,
    ) -> Result<Self, MemoryError> {
        let provider_count = session
            .worker_assignments
            .iter()
            .filter(|worker| worker.model_backend == LegionWorkflowModelBackend::ProviderBacked)
            .count();
        let local_count = session
            .worker_assignments
            .len()
            .saturating_sub(provider_count);
        let signed_off_count = session
            .sign_off_records
            .iter()
            .filter(|signoff| {
                signoff.state == devil_protocol::LegionWorkflowSignOffState::SignedOff
            })
            .count();
        let passed_verification_count = session
            .verification_gates
            .iter()
            .filter(|gate| {
                gate.state == devil_protocol::LegionWorkflowVerificationGateState::Passed
            })
            .count();
        let candidate = Self {
            candidate_id: format!("legion-memory:{}", session.session_id.0),
            workflow_session_id: session.session_id.0.clone(),
            worker_role_backend_label: format!("local:{local_count};provider:{provider_count}"),
            outcome_label: format!("workflow.{:?}", session.lifecycle_state),
            verification_state_label: format!(
                "verification:{passed_verification_count}/{}",
                session.verification_gates.len()
            ),
            sign_off_state_label: format!(
                "signoff:{signed_off_count}/{}",
                session.sign_off_records.len()
            ),
            conflict_count: session.conflict_summaries.len() as u32,
            proposal_count: session.proposal_ids.len() as u32,
            summary_hash,
            consent,
            privacy_labels: vec![PrivacyClassification::Metadata],
            raw_payload_retained: false,
            correlation_id: session.correlation_id,
            causality_id: session.causality_id,
            event_sequence: EventSequence(13),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: session.schema_version.max(1),
        };
        validate_legion_workflow_candidate(&candidate)?;
        Ok(candidate)
    }
}

fn validate_legion_workflow_candidate(
    candidate: &LegionWorkflowOutcomeCandidate,
) -> Result<(), MemoryError> {
    if candidate.candidate_id.trim().is_empty()
        || candidate.workflow_session_id.trim().is_empty()
        || candidate.summary_hash.algorithm.trim().is_empty()
        || candidate.summary_hash.value.trim().is_empty()
    {
        return Err(MemoryError::InvalidMetadata(
            AssistedAiContractError::InvalidProposalMetadata {
                reason: "legion workflow memory candidate requires ids and summary hash"
                    .to_string(),
            },
        ));
    }
    if candidate.raw_payload_retained {
        return Err(MemoryError::InvalidMetadata(
            AssistedAiContractError::NonMetadataOnlyAuditRecord {
                field: "raw_payload_retained".to_string(),
                reason: "raw workflow payload retention is not permitted".to_string(),
            },
        ));
    }
    if candidate.schema_version == 0
        || candidate.redaction_hints.is_empty()
        || candidate.redaction_hints.contains(&RedactionHint::None)
    {
        return Err(MemoryError::InvalidMetadata(
            AssistedAiContractError::NonMetadataOnlyAuditRecord {
                field: "legion_workflow_memory".to_string(),
                reason: "metadata-only redaction and non-zero schema are required".to_string(),
            },
        ));
    }
    validate_phase4_runtime_audit_record(&Phase4RuntimeAuditRecord {
        audit_id: format!("memory:{}", candidate.candidate_id),
        run_id: None,
        step_id: None,
        provider_route_id: None,
        invocation_state: devil_protocol::AssistedAiProviderInvocationState::NotEncoded,
        outcome_label: candidate.outcome_label.clone(),
        labels: vec![
            candidate.worker_role_backend_label.clone(),
            candidate.verification_state_label.clone(),
            candidate.sign_off_state_label.clone(),
        ],
        correlation_id: candidate.correlation_id,
        causality_id: candidate.causality_id,
        event_sequence: candidate.event_sequence,
        redaction_hints: candidate.redaction_hints.clone(),
        schema_version: candidate.schema_version,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::{
        AssistedAiProviderInvocationState, CommandRiskLabel, DelegatedTaskOperationClass,
        LegionWorkflowMergeApproval, LegionWorkflowModelBackend, LegionWorkflowSession,
        LegionWorkflowSessionId, LegionWorkflowSignOff, LegionWorkflowSignOffId,
        LegionWorkflowSignOffState, LegionWorkflowState, LegionWorkflowVerificationGate,
        LegionWorkflowVerificationGateId, LegionWorkflowVerificationGateState,
        LegionWorkflowWorkerAssignment, LegionWorkflowWorkerId, LegionWorkflowWorkerRole,
        LegionWorkflowWorkerState, PrincipalId, PrivacyClassification, ProductMode, ProposalId,
    };
    use uuid::Uuid;

    fn causality(value: u128) -> CausalityId {
        CausalityId(Uuid::from_u128(value))
    }

    fn candidate(consent: MemoryConsentState) -> MemoryCandidateRecord {
        MemoryCandidateRecord {
            candidate_id: "memory-candidate".to_string(),
            run_id: Some(AgentRunId("memory-run".to_string())),
            consent,
            labels: vec![
                "memory.metadata_only".to_string(),
                "vector.deferred".to_string(),
            ],
            correlation_id: CorrelationId(1),
            causality_id: causality(1),
            event_sequence: EventSequence(1),
        }
    }

    #[test]
    fn candidate_review_does_not_retain_without_authority() {
        let service = MemoryService::new();
        let proposed = service
            .propose_candidate(candidate(MemoryConsentState::NotGranted))
            .expect("candidate review is metadata-only");

        assert_eq!(proposed.candidate_id, "memory-candidate");
        assert!(service.retained().is_empty());
    }

    #[test]
    fn long_term_retention_requires_explicit_consent_and_can_be_deleted() {
        let mut service = MemoryService::new();

        let error = service
            .retain(candidate(MemoryConsentState::NotGranted))
            .expect_err("retention without consent is denied");
        assert_eq!(error, MemoryError::ConsentRequired);

        service
            .retain(candidate(MemoryConsentState::ProjectLongTerm))
            .expect("explicit project consent allows retention");
        assert_eq!(service.retained().len(), 1);

        assert!(service.delete("memory-candidate"));
        assert!(service.retained().is_empty());
        assert!(!service.delete("memory-candidate"));
    }

    #[test]
    fn memory_candidates_reject_raw_source_and_provider_payload_markers() {
        let service = MemoryService::new();
        let mut raw_candidate = candidate(MemoryConsentState::SessionOnly);
        raw_candidate.labels.push("source_body leaked".to_string());

        assert!(matches!(
            service.propose_candidate(raw_candidate),
            Err(MemoryError::InvalidMetadata(_))
        ));
    }

    #[test]
    fn memory_validation_uses_not_encoded_invocation_metadata() {
        let candidate = candidate(MemoryConsentState::SessionOnly);
        validate_phase4_runtime_audit_record(&Phase4RuntimeAuditRecord {
            audit_id: format!("memory:{}", candidate.candidate_id),
            run_id: candidate.run_id.clone(),
            step_id: None,
            provider_route_id: None,
            invocation_state: AssistedAiProviderInvocationState::NotEncoded,
            outcome_label: "memory.candidate.metadata_only".to_string(),
            labels: candidate.labels,
            correlation_id: candidate.correlation_id,
            causality_id: candidate.causality_id,
            event_sequence: candidate.event_sequence,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
        .expect("memory audit metadata is valid without encoded runtime payloads");
    }

    fn workflow_hash(value: &str) -> FileFingerprint {
        FileFingerprint {
            algorithm: "sha256".to_string(),
            value: value.to_string(),
        }
    }

    fn legion_workflow_session() -> LegionWorkflowSession {
        LegionWorkflowSession {
            session_id: LegionWorkflowSessionId("session:legion:memory".to_string()),
            directive_artifact_id: Some("artifact:directive:memory".to_string()),
            spec_artifact_id: Some("artifact:spec:memory".to_string()),
            task_graph_artifact_id: Some("artifact:task-graph:memory".to_string()),
            product_mode: ProductMode::LegionWorkflows,
            worker_assignments: vec![LegionWorkflowWorkerAssignment {
                worker_id: LegionWorkflowWorkerId("worker:memory".to_string()),
                role: LegionWorkflowWorkerRole::Verifier,
                state: LegionWorkflowWorkerState::Completed,
                model_backend: LegionWorkflowModelBackend::Local,
                display_safe_model_label: "local:metadata".to_string(),
                allowed_command_classes: vec![
                    DelegatedTaskOperationClass::SummarizeVerificationReadiness,
                ],
                linked_delegated_plan_id: None,
                assisted_ai_route: None,
                affected_targets: Vec::new(),
                risk_labels: vec![CommandRiskLabel::Review],
                privacy_labels: vec![PrivacyClassification::Metadata],
                correlation_id: CorrelationId(1),
                causality_id: causality(21),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            dependency_edges: Vec::new(),
            conflict_summaries: Vec::new(),
            verification_gates: vec![LegionWorkflowVerificationGate {
                gate_id: LegionWorkflowVerificationGateId("verification:memory".to_string()),
                state: LegionWorkflowVerificationGateState::Passed,
                label: "memory tests".to_string(),
                evidence_artifact_id: Some("artifact:evidence:memory".to_string()),
                command_class_label: "cargo-test".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            sign_off_records: vec![LegionWorkflowSignOff {
                sign_off_id: LegionWorkflowSignOffId("signoff:memory".to_string()),
                state: LegionWorkflowSignOffState::SignedOff,
                required_role: LegionWorkflowWorkerRole::Reviewer,
                reviewer_principal_id: Some(PrincipalId("principal:memory-reviewer".to_string())),
                label: "memory sign-off".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            proposal_ids: vec![ProposalId(1304)],
            merge_approval: Some(LegionWorkflowMergeApproval {
                approval_artifact_id: Some("artifact:approval:memory".to_string()),
                approval_granted: true,
                rollback_available: true,
                audit_persisted_before_success: true,
                main_workspace_dirty_conflict: false,
                proposal_preconditions_stale: false,
                labels: vec!["approval-gated".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }),
            lifecycle_state: LegionWorkflowState::Completed,
            generated_at: devil_protocol::TimestampMillis(1304),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
            correlation_id: CorrelationId(1),
            causality_id: causality(22),
        }
    }

    #[test]
    fn legion_workflow_memory_candidate_creation_is_metadata_only() {
        let session = legion_workflow_session();
        let candidate = LegionWorkflowOutcomeCandidate::from_session_metadata(
            &session,
            MemoryConsentState::NotGranted,
            workflow_hash("legion-memory-summary"),
        )
        .expect("candidate builds from metadata");

        assert_eq!(candidate.workflow_session_id, "session:legion:memory");
        assert_eq!(candidate.worker_role_backend_label, "local:1;provider:0");
        assert!(!candidate.raw_payload_retained);
        assert_eq!(candidate.redaction_hints, vec![RedactionHint::MetadataOnly]);
    }

    #[test]
    fn legion_workflow_memory_consent_denied_prevents_retention() {
        let mut service = MemoryService::new();
        let candidate = LegionWorkflowOutcomeCandidate::from_session_metadata(
            &legion_workflow_session(),
            MemoryConsentState::NotGranted,
            workflow_hash("legion-memory-summary"),
        )
        .expect("candidate builds");

        let proposed = service
            .propose_legion_workflow_candidate(candidate.clone())
            .expect("proposal does not retain");
        assert_eq!(proposed.workflow_session_id, candidate.workflow_session_id);
        assert!(service.retained_legion_workflow_candidates().is_empty());
        assert_eq!(
            service.retain_legion_workflow_candidate(candidate),
            Err(MemoryError::ConsentRequired)
        );
    }

    #[test]
    fn legion_workflow_memory_retains_with_consent_and_deletes() {
        let mut service = MemoryService::new();
        let candidate = LegionWorkflowOutcomeCandidate::from_session_metadata(
            &legion_workflow_session(),
            MemoryConsentState::ProjectLongTerm,
            workflow_hash("legion-memory-summary"),
        )
        .expect("candidate builds");
        let candidate_id = candidate.candidate_id.clone();

        service
            .retain_legion_workflow_candidate(candidate)
            .expect("explicit consent retains");

        assert_eq!(service.retained_legion_workflow_candidates().len(), 1);
        assert_eq!(
            service
                .legion_workflow_candidates_by_session_id("session:legion:memory")
                .len(),
            1
        );
        assert_eq!(
            service
                .legion_workflow_candidates_by_worker_role_backend("local:1;provider:0")
                .len(),
            1
        );
        assert!(service.delete_legion_workflow_candidate(&candidate_id));
        assert!(service.retained_legion_workflow_candidates().is_empty());
    }

    #[test]
    fn legion_workflow_memory_rejects_raw_payload_retention() {
        let mut candidate = LegionWorkflowOutcomeCandidate::from_session_metadata(
            &legion_workflow_session(),
            MemoryConsentState::SessionOnly,
            workflow_hash("legion-memory-summary"),
        )
        .expect("candidate builds");
        candidate.raw_payload_retained = true;

        assert!(matches!(
            validate_legion_workflow_candidate(&candidate),
            Err(MemoryError::InvalidMetadata(_))
        ));
    }
}
