//! Opt-in long-term memory: embedding references, retention policies, consent.

#![warn(missing_docs)]

use devil_protocol::{
    AgentRunId, AssistedAiContractError, CausalityId, CorrelationId, EventSequence,
    Phase4RuntimeAuditRecord, RedactionHint, validate_phase4_runtime_audit_record,
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

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::AssistedAiProviderInvocationState;
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
}
