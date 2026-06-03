//! Local Tracker: tasks, plans, links, approvals, run records.

#![warn(missing_docs)]

use legion_protocol::{
    AgentRunId, AgentRunState, AgentStateTransitionRecord, AssistedAiContractError, CausalityId,
    CorrelationId, EventSequence, FileFingerprint, LegionWorkflowConflictId,
    LegionWorkflowMergeReadinessState, LegionWorkflowSessionId, LegionWorkflowVerificationGateId,
    LegionWorkflowWorkerId, PrivacyClassification, ProposalId, RedactionHint,
    validate_phase4_runtime_audit_record,
};
use thiserror::Error;

/// Tracker errors for Phase 4 metadata ledger operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TrackerError {
    /// A metadata record failed protocol validation.
    #[error("invalid tracker metadata: {0}")]
    InvalidMetadata(#[from] AssistedAiContractError),
    /// A Legion workflow tracker record failed validation.
    #[error("invalid legion workflow tracker metadata: {0}")]
    InvalidLegionWorkflowMetadata(String),
}

/// Metadata-only ledger record for an agent run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackerRunLedgerRecord {
    /// Agent run identifier.
    pub run_id: AgentRunId,
    /// Current run state.
    pub state: AgentRunState,
    /// Optional linked proposal id.
    pub proposal_id: Option<ProposalId>,
    /// State transitions observed for this run.
    pub transitions: Vec<AgentStateTransitionRecord>,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Display-safe labels.
    pub labels: Vec<String>,
}

impl TrackerRunLedgerRecord {
    /// Validates that the ledger record contains metadata only.
    pub fn validate(&self) -> Result<(), TrackerError> {
        let audit = legion_protocol::Phase4RuntimeAuditRecord {
            audit_id: format!("tracker:{}", self.run_id.0),
            run_id: Some(self.run_id.clone()),
            step_id: None,
            provider_route_id: None,
            invocation_state: legion_protocol::AssistedAiProviderInvocationState::NotEncoded,
            outcome_label: format!("tracker.state.{:?}", self.state),
            labels: self.labels.clone(),
            correlation_id: self.correlation_id,
            causality_id: self.causality_id,
            event_sequence: self.event_sequence,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_phase4_runtime_audit_record(&audit)?;
        Ok(())
    }
}

/// In-memory tracker ledger for deterministic tests and app-owned composition.
#[derive(Debug, Default)]
pub struct TrackerLedger {
    records: Vec<TrackerRunLedgerRecord>,
}

impl TrackerLedger {
    /// Creates an empty tracker ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a validated metadata-only run record.
    pub fn append(&mut self, record: TrackerRunLedgerRecord) -> Result<(), TrackerError> {
        record.validate()?;
        self.records.push(record);
        Ok(())
    }

    /// Looks up records by run id.
    pub fn by_run_id(&self, run_id: &AgentRunId) -> Vec<&TrackerRunLedgerRecord> {
        self.records
            .iter()
            .filter(|record| &record.run_id == run_id)
            .collect()
    }

    /// Looks up records by proposal id.
    pub fn by_proposal_id(&self, proposal_id: ProposalId) -> Vec<&TrackerRunLedgerRecord> {
        self.records
            .iter()
            .filter(|record| record.proposal_id == Some(proposal_id))
            .collect()
    }
}

/// Metadata-only tracker record for Legion workflow orchestration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegionWorkflowTrackerRecord {
    /// Stable tracker record identifier.
    pub record_id: String,
    /// Workflow session id.
    pub workflow_session_id: LegionWorkflowSessionId,
    /// Optional worker id when the record is worker-scoped.
    pub worker_id: Option<LegionWorkflowWorkerId>,
    /// Linked proposal ids.
    pub linked_proposal_ids: Vec<ProposalId>,
    /// Verification gate ids represented by this record.
    pub verification_gate_ids: Vec<LegionWorkflowVerificationGateId>,
    /// Conflict ids represented by this record.
    pub conflict_ids: Vec<LegionWorkflowConflictId>,
    /// Number of unresolved conflicts.
    pub unresolved_conflict_count: u32,
    /// Number of failed or blocked verification gates.
    pub failed_verification_count: u32,
    /// Number of required sign-off records.
    pub required_sign_off_count: u32,
    /// Number of signed-off records.
    pub signed_off_count: u32,
    /// Merge readiness state represented by this record.
    pub merge_readiness_state: LegionWorkflowMergeReadinessState,
    /// Display-safe risk labels.
    pub risk_labels: Vec<String>,
    /// Privacy labels.
    pub privacy_labels: Vec<PrivacyClassification>,
    /// Metadata summary hash.
    pub summary_hash: FileFingerprint,
    /// Audit correlation id.
    pub correlation_id: CorrelationId,
    /// Audit causality id.
    pub causality_id: CausalityId,
    /// Audit event sequence.
    pub event_sequence: EventSequence,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Record schema version.
    pub schema_version: u16,
}

impl LegionWorkflowTrackerRecord {
    /// Validates this record as metadata-only and fail-closed.
    pub fn validate(&self) -> Result<(), TrackerError> {
        if self.record_id.trim().is_empty() {
            return Err(TrackerError::InvalidLegionWorkflowMetadata(
                "record id is required".to_string(),
            ));
        }
        if self.workflow_session_id.0.trim().is_empty() {
            return Err(TrackerError::InvalidLegionWorkflowMetadata(
                "workflow session id is required".to_string(),
            ));
        }
        if self.schema_version == 0 {
            return Err(TrackerError::InvalidLegionWorkflowMetadata(
                "schema version must be non-zero".to_string(),
            ));
        }
        if self.correlation_id.0 == 0 {
            return Err(TrackerError::InvalidLegionWorkflowMetadata(
                "correlation id must be non-zero".to_string(),
            ));
        }
        if self.causality_id.0.is_nil() {
            return Err(TrackerError::InvalidLegionWorkflowMetadata(
                "causality id must be non-nil".to_string(),
            ));
        }
        if self.event_sequence.0 == 0 {
            return Err(TrackerError::InvalidLegionWorkflowMetadata(
                "event sequence must be non-zero".to_string(),
            ));
        }
        if self.redaction_hints.is_empty() || self.redaction_hints.contains(&RedactionHint::None) {
            return Err(TrackerError::InvalidLegionWorkflowMetadata(
                "redaction must be metadata-only".to_string(),
            ));
        }
        if self.summary_hash.algorithm.trim().is_empty()
            || self.summary_hash.value.trim().is_empty()
        {
            return Err(TrackerError::InvalidLegionWorkflowMetadata(
                "summary hash is required".to_string(),
            ));
        }
        if self.merge_readiness_state == LegionWorkflowMergeReadinessState::Ready
            && self.unresolved_conflict_count > 0
        {
            return Err(TrackerError::InvalidLegionWorkflowMetadata(
                "merge-ready record cannot contain unresolved conflicts".to_string(),
            ));
        }
        if self.merge_readiness_state == LegionWorkflowMergeReadinessState::Ready
            && (self.verification_gate_ids.is_empty()
                || self.required_sign_off_count == 0
                || self.signed_off_count < self.required_sign_off_count)
        {
            return Err(TrackerError::InvalidLegionWorkflowMetadata(
                "merge-ready record requires verification and sign-off metadata".to_string(),
            ));
        }
        validate_phase4_runtime_audit_record(&legion_protocol::Phase4RuntimeAuditRecord {
            audit_id: format!("tracker:legion:{}", self.record_id),
            run_id: None,
            step_id: None,
            provider_route_id: None,
            invocation_state: legion_protocol::AssistedAiProviderInvocationState::NotEncoded,
            outcome_label: format!("legion_workflow.{:?}", self.merge_readiness_state),
            labels: self.labels.clone(),
            correlation_id: self.correlation_id,
            causality_id: self.causality_id,
            event_sequence: self.event_sequence,
            redaction_hints: self.redaction_hints.clone(),
            schema_version: self.schema_version,
        })?;
        Ok(())
    }
}

/// In-memory tracker ledger for Legion workflow metadata.
#[derive(Debug, Default)]
pub struct LegionWorkflowTrackerLedger {
    records: Vec<LegionWorkflowTrackerRecord>,
}

impl LegionWorkflowTrackerLedger {
    /// Creates an empty Legion workflow tracker ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a validated workflow record.
    pub fn append(&mut self, record: LegionWorkflowTrackerRecord) -> Result<(), TrackerError> {
        record.validate()?;
        self.records.push(record);
        Ok(())
    }

    /// Returns all records.
    pub fn records(&self) -> &[LegionWorkflowTrackerRecord] {
        &self.records
    }

    /// Looks up records by workflow session id.
    pub fn by_workflow_session_id(
        &self,
        session_id: &LegionWorkflowSessionId,
    ) -> Vec<&LegionWorkflowTrackerRecord> {
        self.records
            .iter()
            .filter(|record| &record.workflow_session_id == session_id)
            .collect()
    }

    /// Looks up records by worker id.
    pub fn by_worker_id(
        &self,
        worker_id: &LegionWorkflowWorkerId,
    ) -> Vec<&LegionWorkflowTrackerRecord> {
        self.records
            .iter()
            .filter(|record| record.worker_id.as_ref() == Some(worker_id))
            .collect()
    }

    /// Looks up records by proposal id.
    pub fn by_proposal_id(&self, proposal_id: ProposalId) -> Vec<&LegionWorkflowTrackerRecord> {
        self.records
            .iter()
            .filter(|record| record.linked_proposal_ids.contains(&proposal_id))
            .collect()
    }

    /// Returns records with conflicts for a session.
    pub fn conflicts_for_session(
        &self,
        session_id: &LegionWorkflowSessionId,
    ) -> Vec<&LegionWorkflowTrackerRecord> {
        self.by_workflow_session_id(session_id)
            .into_iter()
            .filter(|record| {
                !record.conflict_ids.is_empty() || record.unresolved_conflict_count > 0
            })
            .collect()
    }

    /// Returns records with verification gates for a session.
    pub fn verification_records_for_session(
        &self,
        session_id: &LegionWorkflowSessionId,
    ) -> Vec<&LegionWorkflowTrackerRecord> {
        self.by_workflow_session_id(session_id)
            .into_iter()
            .filter(|record| !record.verification_gate_ids.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        AssistedAiContractError, LegionWorkflowConflictId, LegionWorkflowMergeReadinessState,
        LegionWorkflowSessionId, LegionWorkflowVerificationGateId, LegionWorkflowWorkerId,
        PrivacyClassification, RedactionHint,
    };
    use uuid::Uuid;

    fn causality(value: u128) -> CausalityId {
        CausalityId(Uuid::from_u128(value))
    }

    fn transition(run_id: AgentRunId) -> AgentStateTransitionRecord {
        AgentStateTransitionRecord {
            run_id,
            step_id: None,
            from_state: AgentRunState::Observing,
            to_state: AgentRunState::Planning,
            reason_code: "tracker.plan".to_string(),
            proposal_id: Some(ProposalId(42)),
            correlation_id: CorrelationId(1),
            causality_id: causality(1),
            event_sequence: EventSequence(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn record(run_id: AgentRunId) -> TrackerRunLedgerRecord {
        TrackerRunLedgerRecord {
            run_id: run_id.clone(),
            state: AgentRunState::Planning,
            proposal_id: Some(ProposalId(42)),
            transitions: vec![transition(run_id)],
            correlation_id: CorrelationId(1),
            causality_id: causality(2),
            event_sequence: EventSequence(2),
            labels: vec!["tracker.metadata_only".to_string()],
        }
    }

    #[test]
    fn tracker_ledger_roundtrips_run_and_proposal_metadata() {
        let run_id = AgentRunId("tracker-run".to_string());
        let mut ledger = TrackerLedger::new();

        ledger
            .append(record(run_id.clone()))
            .expect("tracker metadata appends");

        let by_run = ledger.by_run_id(&run_id);
        assert_eq!(by_run.len(), 1);
        assert_eq!(by_run[0].state, AgentRunState::Planning);
        assert_eq!(by_run[0].transitions.len(), 1);

        let by_proposal = ledger.by_proposal_id(ProposalId(42));
        assert_eq!(by_proposal.len(), 1);
        assert_eq!(by_proposal[0].run_id, run_id);
    }

    #[test]
    fn tracker_ledger_rejects_raw_prompt_markers() {
        let run_id = AgentRunId("tracker-raw".to_string());
        let mut raw_record = record(run_id);
        raw_record.labels.push("raw prompt contents".to_string());
        let mut ledger = TrackerLedger::new();

        let error = ledger
            .append(raw_record)
            .expect_err("raw labels are rejected");

        assert!(matches!(
            error,
            TrackerError::InvalidMetadata(
                AssistedAiContractError::NonMetadataOnlyAuditRecord { .. }
            )
        ));
    }

    #[test]
    fn tracker_ledger_rejects_zero_event_sequence() {
        let run_id = AgentRunId("tracker-zero-event".to_string());
        let mut invalid_record = record(run_id);
        invalid_record.event_sequence = EventSequence(0);
        let mut ledger = TrackerLedger::new();

        let error = ledger
            .append(invalid_record)
            .expect_err("zero event sequence is rejected");

        assert!(matches!(
            error,
            TrackerError::InvalidMetadata(AssistedAiContractError::ZeroEventSequence)
        ));
    }

    fn workflow_hash(value: &str) -> FileFingerprint {
        FileFingerprint {
            algorithm: "sha256".to_string(),
            value: value.to_string(),
        }
    }

    fn legion_workflow_record() -> LegionWorkflowTrackerRecord {
        LegionWorkflowTrackerRecord {
            record_id: "legion-tracker-record".to_string(),
            workflow_session_id: LegionWorkflowSessionId("session:legion:tracker".to_string()),
            worker_id: Some(LegionWorkflowWorkerId("worker:tracker".to_string())),
            linked_proposal_ids: vec![ProposalId(1304)],
            verification_gate_ids: vec![LegionWorkflowVerificationGateId(
                "verification:tracker".to_string(),
            )],
            conflict_ids: Vec::new(),
            unresolved_conflict_count: 0,
            failed_verification_count: 0,
            required_sign_off_count: 1,
            signed_off_count: 1,
            merge_readiness_state: LegionWorkflowMergeReadinessState::Ready,
            risk_labels: vec!["review".to_string()],
            privacy_labels: vec![PrivacyClassification::Metadata],
            summary_hash: workflow_hash("tracker-summary"),
            correlation_id: CorrelationId(1),
            causality_id: causality(11),
            event_sequence: EventSequence(11),
            labels: vec!["legion_workflow.metadata_only".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn legion_workflow_tracker_appends_and_indexes_metadata() {
        let mut ledger = LegionWorkflowTrackerLedger::new();
        let record = legion_workflow_record();
        let session_id = record.workflow_session_id.clone();
        let worker_id = record.worker_id.clone().expect("worker id");

        ledger.append(record).expect("append workflow metadata");

        assert_eq!(ledger.records().len(), 1);
        assert_eq!(ledger.by_workflow_session_id(&session_id).len(), 1);
        assert_eq!(ledger.by_worker_id(&worker_id).len(), 1);
        assert_eq!(ledger.by_proposal_id(ProposalId(1304)).len(), 1);
        assert_eq!(
            ledger.verification_records_for_session(&session_id).len(),
            1
        );
        assert!(ledger.conflicts_for_session(&session_id).is_empty());
    }

    #[test]
    fn legion_workflow_tracker_rejects_invalid_zero_event_and_nil_causality() {
        let mut zero_event = legion_workflow_record();
        zero_event.event_sequence = EventSequence(0);
        assert!(matches!(
            zero_event.validate(),
            Err(TrackerError::InvalidLegionWorkflowMetadata(_))
        ));

        let mut nil_causality = legion_workflow_record();
        nil_causality.causality_id = CausalityId(Uuid::nil());
        assert!(matches!(
            nil_causality.validate(),
            Err(TrackerError::InvalidLegionWorkflowMetadata(_))
        ));
    }

    #[test]
    fn legion_workflow_tracker_rejects_unresolved_conflict_ready_state() {
        let mut record = legion_workflow_record();
        record.conflict_ids = vec![LegionWorkflowConflictId("conflict:tracker".to_string())];
        record.unresolved_conflict_count = 1;

        assert!(matches!(
            record.validate(),
            Err(TrackerError::InvalidLegionWorkflowMetadata(_))
        ));
    }

    #[test]
    fn legion_workflow_tracker_conflict_and_proposal_lookup_are_metadata_only() {
        let mut ledger = LegionWorkflowTrackerLedger::new();
        let mut record = legion_workflow_record();
        record.merge_readiness_state = LegionWorkflowMergeReadinessState::Blocked;
        record.conflict_ids = vec![LegionWorkflowConflictId("conflict:tracker".to_string())];
        record.unresolved_conflict_count = 1;
        let session_id = record.workflow_session_id.clone();

        ledger
            .append(record)
            .expect("blocked conflict metadata appends");

        let conflicts = ledger.conflicts_for_session(&session_id);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].linked_proposal_ids, vec![ProposalId(1304)]);
        assert_eq!(
            conflicts[0].redaction_hints,
            vec![RedactionHint::MetadataOnly]
        );
    }
}
