//! Agent runtime state machine: states, transitions, and error types.

use thiserror::Error;

use legion_protocol::{
    AgentReplayManifest, AgentRunId, AgentRunState, AgentStateTransitionRecord,
    AssistedAiContractError, AssistedAiEditProposalOutput, AssistedAiProviderInvocationState,
    AssistedAiProviderRouteRequest, CausalityId, CorrelationId, EventSequence, LegionToolKind,
    Phase4RuntimeAuditRecord, RedactionHint, validate_agent_replay_manifest,
    validate_phase4_runtime_audit_record,
};

/// Agent runtime errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentError {
    /// Transition is not legal from the current state.
    #[error("illegal agent transition from {from:?} to {to:?}")]
    IllegalTransition {
        /// Current state.
        from: AgentRunState,
        /// Requested state.
        to: AgentRunState,
    },
    /// Metadata validation failed.
    #[error("invalid agent metadata: {0}")]
    InvalidMetadata(#[from] AssistedAiContractError),
    /// Replay metadata referenced a different run id.
    #[error("agent replay run id mismatch: expected {expected:?}, actual {actual:?}")]
    ReplayRunMismatch {
        /// Expected run identifier.
        expected: AgentRunId,
        /// Actual run identifier found in a transition.
        actual: AgentRunId,
    },
    /// Legion workflow metadata was invalid or blocked.
    #[error("invalid legion workflow metadata: {0}")]
    InvalidLegionWorkflow(String),
    /// Legion workflow worker was unknown.
    #[error("unknown legion workflow worker: {0}")]
    UnknownLegionWorkflowWorker(String),
    /// Legion workflow worker was completed more than once.
    #[error("legion workflow worker already completed: {0}")]
    LegionWorkflowWorkerAlreadyCompleted(String),
    /// Legion workflow dependency graph contains a cycle.
    #[error("legion workflow dependency cycle detected")]
    LegionWorkflowDependencyCycle,
    /// Delegated task tool call was denied by scope policy.
    #[error("delegated task scope denied: {tool:?} ({reason})")]
    DelegatedTaskScopeDenied {
        /// Tool that was denied.
        tool: LegionToolKind,
        /// Target path that was denied, if any.
        target_path: Option<String>,
        /// Reason for denial.
        reason: String,
    },
}

/// Mutation-safe output produced by the agent state machine.
#[derive(Debug, Clone)]
pub enum AgentRuntimeOutput {
    /// Provider route request; the agent does not invoke providers directly.
    ProviderRoute(Box<AssistedAiProviderRouteRequest>),
    /// Proposal-only edit output; the agent does not apply it.
    EditProposal(Box<AssistedAiEditProposalOutput>),
    /// State transition metadata for tracker/storage owned by composition.
    Transition(AgentStateTransitionRecord),
}

/// Deterministic Phase 4 agent state machine.
#[derive(Debug, Clone)]
pub struct AgentRuntime {
    run_id: AgentRunId,
    state: AgentRunState,
    transitions: Vec<AgentStateTransitionRecord>,
}

impl AgentRuntime {
    /// Creates an agent runtime in the observing state.
    pub fn new(run_id: AgentRunId) -> Self {
        Self {
            run_id,
            state: AgentRunState::Observing,
            transitions: Vec::new(),
        }
    }

    /// Returns the current run state.
    pub fn state(&self) -> AgentRunState {
        self.state
    }

    /// Returns recorded metadata-only transitions.
    pub fn transitions(&self) -> &[AgentStateTransitionRecord] {
        &self.transitions
    }

    /// Reconstructs runtime state from metadata-only replay records.
    pub fn replay(manifest: &AgentReplayManifest) -> Result<Self, AgentError> {
        validate_agent_replay_manifest(manifest)?;
        let mut runtime = Self::new(manifest.run_id.clone());
        for transition in &manifest.transitions {
            if transition.run_id != manifest.run_id {
                return Err(AgentError::ReplayRunMismatch {
                    expected: manifest.run_id.clone(),
                    actual: transition.run_id.clone(),
                });
            }
            if transition.from_state != runtime.state
                || !legal_transition(runtime.state, transition.to_state)
            {
                return Err(AgentError::IllegalTransition {
                    from: runtime.state,
                    to: transition.to_state,
                });
            }
            runtime.state = transition.to_state;
            runtime.transitions.push(transition.clone());
        }
        Ok(runtime)
    }

    /// Applies a legal transition and records metadata for replay.
    pub fn transition(
        &mut self,
        to_state: AgentRunState,
        reason_code: impl Into<String>,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        event_sequence: EventSequence,
    ) -> Result<AgentRuntimeOutput, AgentError> {
        if !legal_transition(self.state, to_state) {
            return Err(AgentError::IllegalTransition {
                from: self.state,
                to: to_state,
            });
        }
        let transition = AgentStateTransitionRecord {
            run_id: self.run_id.clone(),
            step_id: None,
            from_state: self.state,
            to_state,
            reason_code: reason_code.into(),
            proposal_id: None,
            correlation_id,
            causality_id,
            event_sequence,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_phase4_runtime_audit_record(&Phase4RuntimeAuditRecord {
            audit_id: format!("agent:{}:{}", self.run_id.0, event_sequence.0),
            run_id: Some(self.run_id.clone()),
            step_id: None,
            provider_route_id: None,
            invocation_state: AssistedAiProviderInvocationState::NotEncoded,
            outcome_label: transition.reason_code.clone(),
            labels: vec![format!("agent.state.{to_state:?}")],
            correlation_id,
            causality_id,
            event_sequence,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })?;
        self.state = to_state;
        self.transitions.push(transition.clone());
        Ok(AgentRuntimeOutput::Transition(transition))
    }
}

pub(crate) fn legal_transition(from: AgentRunState, to: AgentRunState) -> bool {
    matches!(
        (from, to),
        (AgentRunState::Observing, AgentRunState::Planning)
            | (AgentRunState::Planning, AgentRunState::Proposing)
            | (AgentRunState::Proposing, AgentRunState::WaitingForApproval)
            | (AgentRunState::WaitingForApproval, AgentRunState::Applying)
            | (AgentRunState::Applying, AgentRunState::Verifying)
            | (AgentRunState::Verifying, AgentRunState::Completed)
            | (_, AgentRunState::Cancelled)
            | (_, AgentRunState::Blocked)
            | (_, AgentRunState::Failed)
            | (AgentRunState::Blocked, AgentRunState::Recovering)
            | (AgentRunState::Recovering, AgentRunState::Planning)
    )
}
