//! Local Tracker: tasks, plans, links, approvals, run records.

#![warn(missing_docs)]

use devil_protocol::{
    AgentRunId, AgentRunState, AgentStateTransitionRecord, AssistedAiContractError, CausalityId,
    CorrelationId, EventSequence, ProposalId, RedactionHint, validate_phase4_runtime_audit_record,
};
use thiserror::Error;

/// Tracker errors for Phase 4 metadata ledger operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TrackerError {
    /// A metadata record failed protocol validation.
    #[error("invalid tracker metadata: {0}")]
    InvalidMetadata(#[from] AssistedAiContractError),
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
        let audit = devil_protocol::Phase4RuntimeAuditRecord {
            audit_id: format!("tracker:{}", self.run_id.0),
            run_id: Some(self.run_id.clone()),
            step_id: None,
            provider_route_id: None,
            invocation_state: devil_protocol::AssistedAiProviderInvocationState::NotEncoded,
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

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::{AssistedAiContractError, RedactionHint};
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
}
