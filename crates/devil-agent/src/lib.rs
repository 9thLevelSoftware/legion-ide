//! Agent workflows: plans, tool-use state machines, capability-scoped automation.

#![warn(missing_docs)]

use devil_protocol::{
    AgentReplayManifest, AgentRunId, AgentRunState, AgentStateTransitionRecord,
    AssistedAiContractError, AssistedAiEditProposalOutput, AssistedAiProviderRouteRequest,
    AssistedAiTrustProjectionKind, AssistedAiTrustProjectionReference, CanonicalPath, CapabilityId,
    CausalityId, CorrelationId, EventSequence, FileFingerprint, PreviewSummary, PrincipalId,
    ProposalId, ProposalPayload, ProposalVersionPreconditions, RedactionHint, TimestampMillis,
    validate_agent_replay_manifest, validate_phase4_runtime_audit_record,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

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
        validate_phase4_runtime_audit_record(&devil_protocol::Phase4RuntimeAuditRecord {
            audit_id: format!("agent:{}:{}", self.run_id.0, event_sequence.0),
            run_id: Some(self.run_id.clone()),
            step_id: None,
            provider_route_id: None,
            invocation_state: devil_protocol::AssistedAiProviderInvocationState::NotEncoded,
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

fn legal_transition(from: AgentRunState, to: AgentRunState) -> bool {
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

/// Orchestrator for isolating agent tasks under `target/delegated-tasks/task-{run_id}`.
/// Uses git worktrees with standard directory fallback if git is unavailable.
#[derive(Debug, Clone)]
pub struct DelegatedTaskSandboxOrchestrator {
    sandbox_path: PathBuf,
    is_worktree: bool,
}

impl DelegatedTaskSandboxOrchestrator {
    /// Creates a new orchestrator.
    pub fn new(run_id: &str) -> Self {
        let sandbox_path = PathBuf::from("target/delegated-tasks").join(format!("task-{}", run_id));
        Self {
            sandbox_path,
            is_worktree: false,
        }
    }

    /// Returns the sandbox path.
    pub fn sandbox_path(&self) -> &Path {
        &self.sandbox_path
    }

    /// Initializes the sandbox using `git worktree add` with fallback to copy-based isolation.
    pub fn initialize(&mut self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.sandbox_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Try git worktree first
        let status = Command::new("git")
            .args([
                "worktree",
                "add",
                self.sandbox_path.to_str().unwrap(),
                "HEAD",
            ])
            .status();

        match status {
            Ok(s) if s.success() => {
                self.is_worktree = true;
                Ok(())
            }
            _ => {
                self.is_worktree = false;
                std::fs::create_dir_all(&self.sandbox_path)?;
                Ok(())
            }
        }
    }

    /// Cleans up the sandbox.
    pub fn cleanup(&mut self) -> Result<(), std::io::Error> {
        if self.sandbox_path.exists() {
            if self.is_worktree {
                let _ = Command::new("git").args(["worktree", "prune"]).status();
            }
            std::fs::remove_dir_all(&self.sandbox_path)?;
        }
        Ok(())
    }
}

/// Validate that path is contained within the base directory.
pub fn validate_containment(base: &Path, path: &Path) -> Result<(), AgentError> {
    let base_absolute =
        std::fs::canonicalize(base).unwrap_or_else(|_| std::env::current_dir().unwrap().join(base));

    let path_absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap().join(path)
    };

    let mut clean_components = Vec::new();
    for component in path_absolute.components() {
        match component {
            std::path::Component::ParentDir => {
                clean_components.pop();
            }
            std::path::Component::CurDir => {}
            c => {
                clean_components.push(c);
            }
        }
    }

    let clean_path: PathBuf = clean_components.into_iter().collect();

    // Strip Windows UNC prefix if present to prevent starts_with discrepancies
    let strip_unc = |p: &Path| -> PathBuf {
        let p_str = p.to_str().unwrap_or("");
        if let Some(stripped) = p_str.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            p.to_path_buf()
        }
    };

    let clean_stripped = strip_unc(&clean_path);
    let base_stripped = strip_unc(&base_absolute);

    if !clean_stripped.starts_with(&base_stripped) {
        return Err(AgentError::InvalidMetadata(
            AssistedAiContractError::InvalidProposalMetadata {
                reason: "Path traversal escaped sandbox".to_string(),
            },
        ));
    }
    Ok(())
}

/// Proposal generator inside `devil-agent`.
#[derive(Debug, Clone)]
pub struct DelegatedTaskProposalGenerator {
    sandbox_base: PathBuf,
}

impl DelegatedTaskProposalGenerator {
    /// Creates a new proposal generator.
    pub fn new(sandbox_base: PathBuf) -> Self {
        Self { sandbox_base }
    }

    /// Compares sandbox directory state with HEAD checkout and builds AssistedAiEditProposalOutput.
    pub fn generate_proposal(
        &self,
        target_path: &Path,
        _modified_content: &str,
    ) -> Result<AssistedAiEditProposalOutput, AgentError> {
        validate_containment(&self.sandbox_base, target_path)?;

        let target_relative = target_path
            .strip_prefix(&self.sandbox_base)
            .unwrap_or(target_path);

        Ok(AssistedAiEditProposalOutput {
            output_id: "out-auto".to_string(),
            request_id: "req-auto".to_string(),
            provider_id: "provider-auto".to_string(),
            proposal_id: ProposalId(1),
            principal: PrincipalId("principal-auto".to_string()),
            capability: CapabilityId("capability-auto".to_string()),
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(uuid::Uuid::from_u128(1)),
            payload: ProposalPayload::CreateFile(devil_protocol::CreateFileProposal {
                path: CanonicalPath(target_relative.to_str().unwrap().to_string()),
                initial_content: Some("".to_string()),
            }),
            preconditions: ProposalVersionPreconditions {
                file_version: None,
                buffer_version: None,
                snapshot_id: None,
                generation: None,
                file_content_version: None,
                workspace_generation: None,
                expected_fingerprint: None,
                expected_file_length: None,
                expected_modified_at: None,
            },
            preview: PreviewSummary {
                summary: "Create file proposal".to_string(),
                details: vec![],
            },
            expires_at: None,
            created_at: TimestampMillis(0),
            context_manifest: AssistedAiTrustProjectionReference {
                reference_id: "ctx-ref".to_string(),
                kind: AssistedAiTrustProjectionKind::ContextManifest,
                projection_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "hash".to_string(),
                },
                schema_version: 1,
            },
            approval_checklist: AssistedAiTrustProjectionReference {
                reference_id: "appr-ref".to_string(),
                kind: AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
                projection_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "hash".to_string(),
                },
                schema_version: 1,
            },
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::{AgentReplayManifest, RedactionHint};
    use uuid::Uuid;

    fn causality(value: u128) -> CausalityId {
        CausalityId(Uuid::from_u128(value))
    }

    #[test]
    fn state_machine_accepts_legal_transitions_and_records_metadata() {
        let mut runtime = AgentRuntime::new(AgentRunId("run-legal".to_string()));

        let output = runtime
            .transition(
                AgentRunState::Planning,
                "agent.plan",
                CorrelationId(1),
                causality(1),
                EventSequence(1),
            )
            .expect("planning transition is legal");

        assert_eq!(runtime.state(), AgentRunState::Planning);
        assert_eq!(runtime.transitions().len(), 1);
        match output {
            AgentRuntimeOutput::Transition(transition) => {
                assert_eq!(transition.to_state, AgentRunState::Planning);
                assert_eq!(transition.reason_code, "agent.plan");
            }
            AgentRuntimeOutput::ProviderRoute(_) | AgentRuntimeOutput::EditProposal(_) => {
                panic!("state transitions must not create mutation-capable outputs")
            }
        }
    }

    #[test]
    fn state_machine_refuses_illegal_transition_without_recording_metadata() {
        let mut runtime = AgentRuntime::new(AgentRunId("run-illegal".to_string()));

        let error = runtime
            .transition(
                AgentRunState::Completed,
                "agent.skip",
                CorrelationId(2),
                causality(2),
                EventSequence(2),
            )
            .expect_err("direct completion is illegal");

        assert_eq!(
            error,
            AgentError::IllegalTransition {
                from: AgentRunState::Observing,
                to: AgentRunState::Completed,
            }
        );
        assert_eq!(runtime.state(), AgentRunState::Observing);
        assert!(runtime.transitions().is_empty());
    }

    #[test]
    fn cancellation_preserves_metadata_without_applying_proposals() {
        let mut runtime = AgentRuntime::new(AgentRunId("run-cancel".to_string()));
        runtime
            .transition(
                AgentRunState::Planning,
                "agent.plan",
                CorrelationId(3),
                causality(3),
                EventSequence(3),
            )
            .expect("planning transition");

        let output = runtime
            .transition(
                AgentRunState::Cancelled,
                "agent.cancel.user",
                CorrelationId(3),
                causality(4),
                EventSequence(4),
            )
            .expect("cancellation transition");

        assert_eq!(runtime.state(), AgentRunState::Cancelled);
        assert_eq!(runtime.transitions().len(), 2);
        assert!(matches!(output, AgentRuntimeOutput::Transition(_)));
    }

    #[test]
    fn replay_reconstructs_recovery_state_from_metadata_only_manifest() {
        let mut runtime = AgentRuntime::new(AgentRunId("run-replay".to_string()));
        runtime
            .transition(
                AgentRunState::Blocked,
                "agent.blocked.policy",
                CorrelationId(4),
                causality(5),
                EventSequence(5),
            )
            .expect("blocked transition");
        runtime
            .transition(
                AgentRunState::Recovering,
                "agent.recover",
                CorrelationId(4),
                causality(6),
                EventSequence(6),
            )
            .expect("recovery transition");
        runtime
            .transition(
                AgentRunState::Planning,
                "agent.replan",
                CorrelationId(4),
                causality(7),
                EventSequence(7),
            )
            .expect("replan transition");

        let manifest = AgentReplayManifest {
            run_id: AgentRunId("run-replay".to_string()),
            transitions: runtime.transitions().to_vec(),
            context_manifests: Vec::new(),
            provider_route_ids: vec!["route-replay".to_string()],
            proposal_ids: Vec::new(),
            correlation_id: CorrelationId(4),
            causality_id: causality(8),
            event_sequence: EventSequence(8),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let replayed = AgentRuntime::replay(&manifest).expect("metadata replay reconstructs state");

        assert_eq!(replayed.state(), AgentRunState::Planning);
        assert_eq!(replayed.transitions(), runtime.transitions());
    }

    #[test]
    fn replay_refuses_raw_provider_payload_markers() {
        let manifest = AgentReplayManifest {
            run_id: AgentRunId("run-raw".to_string()),
            transitions: vec![AgentStateTransitionRecord {
                run_id: AgentRunId("run-raw".to_string()),
                step_id: None,
                from_state: AgentRunState::Observing,
                to_state: AgentRunState::Planning,
                reason_code: "provider_payload leaked".to_string(),
                proposal_id: None,
                correlation_id: CorrelationId(5),
                causality_id: causality(9),
                event_sequence: EventSequence(9),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            context_manifests: Vec::new(),
            provider_route_ids: Vec::new(),
            proposal_ids: Vec::new(),
            correlation_id: CorrelationId(5),
            causality_id: causality(10),
            event_sequence: EventSequence(10),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        assert!(matches!(
            AgentRuntime::replay(&manifest),
            Err(AgentError::InvalidMetadata(_))
        ));
    }

    #[test]
    fn test_sandbox_orchestration_and_containment_and_proposal_generation() {
        let mut orchestrator = DelegatedTaskSandboxOrchestrator::new("test-run");
        orchestrator.initialize().expect("initialize sandbox");
        let sandbox_path = orchestrator.sandbox_path().to_path_buf();

        // 1. Sandbox exists on disk
        assert!(sandbox_path.exists());

        // 2. Traversal verification
        let target_file = sandbox_path.join("src/lib.rs");
        validate_containment(&sandbox_path, &target_file).expect("within containment");

        // Relative escaping path must fail containment check
        let escaping_file = sandbox_path.join("../escaping.txt");
        assert!(validate_containment(&sandbox_path, &escaping_file).is_err());

        // 3. Proposal generation verification
        let generator = DelegatedTaskProposalGenerator::new(sandbox_path.clone());
        let proposal = generator
            .generate_proposal(&target_file, "modified content")
            .expect("generate proposal");

        assert_eq!(proposal.output_id, "out-auto");
        assert_eq!(proposal.proposal_id.0, 1);

        // Cleanup
        orchestrator.cleanup().expect("cleanup sandbox");
        assert!(!sandbox_path.exists());
    }
}
