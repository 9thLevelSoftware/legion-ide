//! Agent workflows: plans, tool-use state machines, capability-scoped automation.

#![warn(missing_docs)]

use devil_protocol::{
    AgentReplayManifest, AgentRunId, AgentRunState, AgentStateTransitionRecord,
    AssistedAiContractError, AssistedAiEditProposalOutput, AssistedAiOperationClass,
    AssistedAiProposalTargetIntent, AssistedAiProviderClass, AssistedAiProviderRouteRequest,
    AssistedAiTrustProjectionReference, CancellationTokenId, CanonicalPath, CapabilityId,
    CausalityId, CorrelationId, DelegatedTaskToolPermissionProfile,
    DelegatedTaskToolPermissionRequest, EventSequence, LegionWorkflowConflict,
    LegionWorkflowConflictId, LegionWorkflowConflictKind, LegionWorkflowConflictState,
    LegionWorkflowDependencyState, LegionWorkflowMergeReadiness, LegionWorkflowModelBackend,
    LegionWorkflowSession, LegionWorkflowWorkerAssignment, LegionWorkflowWorkerId,
    LegionWorkflowWorkerState, PermissionBudgetActionClass, PreviewSummary, PrincipalId,
    ProposalAffectedTarget, ProposalId, ProposalPayload, ProposalPayloadKind, ProposalPrivacyLabel,
    ProposalRiskLabel, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProposalVersionPreconditions, RedactionHint, TimestampMillis, WorkspaceTrustState,
    evaluate_legion_workflow_merge_readiness, validate_agent_replay_manifest,
    validate_legion_workflow_session, validate_phase4_runtime_audit_record,
};
use std::collections::{HashMap, HashSet};
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
    pub fn initialize(
        &mut self,
        permission: &DelegatedTaskToolPermissionRequest,
    ) -> Result<(), std::io::Error> {
        validate_sandbox_permission(permission, "initialize")?;
        if let Some(parent) = self.sandbox_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Try git worktree first
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                self.sandbox_path.to_str().unwrap(),
                "HEAD",
            ])
            .output();

        match output {
            Ok(output) if output.status.success() => {
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
    pub fn cleanup(
        &mut self,
        permission: &DelegatedTaskToolPermissionRequest,
    ) -> Result<(), std::io::Error> {
        validate_sandbox_permission(permission, "cleanup")?;
        if self.sandbox_path.exists() {
            if self.is_worktree {
                let output = Command::new("git")
                    .arg("worktree")
                    .arg("remove")
                    .arg("--force")
                    .arg(&self.sandbox_path)
                    .output()?;
                if !output.status.success() {
                    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(std::io::Error::other(format!(
                        "git worktree remove failed for {}: {}",
                        self.sandbox_path.display(),
                        message
                    )));
                }
            } else {
                std::fs::remove_dir_all(&self.sandbox_path)?;
            }
        }
        Ok(())
    }
}

fn validate_sandbox_permission(
    permission: &DelegatedTaskToolPermissionRequest,
    operation: &str,
) -> Result<(), std::io::Error> {
    let write_profile = permission.profile == DelegatedTaskToolPermissionProfile::Write;
    let sandbox_action = matches!(
        permission.action_class,
        PermissionBudgetActionClass::AccessWorkspaceFiles
            | PermissionBudgetActionClass::InvokeLocalTool
    );
    let delegated_runtime_capability = permission
        .capability
        .as_ref()
        .is_some_and(|capability| capability.0 == "delegated.runtime.allocate");
    if write_profile
        && sandbox_action
        && delegated_runtime_capability
        && permission.runtime_allowed
        && permission.human_approval_recorded
        && !permission.deny_overrides
    {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!("delegated sandbox {operation} requires approved Write tool permission"),
    ))
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

/// Request-scoped inputs for delegated task proposal generation.
#[derive(Debug, Clone)]
pub struct DelegatedTaskProposalInput<'a> {
    /// Target path inside the delegated task sandbox.
    pub target_path: &'a Path,
    /// Provider-produced file content for create-file proposals.
    pub modified_content: &'a str,
    /// Output identifier assigned by the caller.
    pub output_id: String,
    /// Provider request identifier associated with the proposal.
    pub request_id: String,
    /// Provider identifier that produced the proposed content.
    pub provider_id: String,
    /// Proposal identifier assigned by the caller.
    pub proposal_id: ProposalId,
    /// Principal on whose behalf the proposal was generated.
    pub principal: PrincipalId,
    /// Capability authorizing proposal generation.
    pub capability: CapabilityId,
    /// Correlation identifier for observability.
    pub correlation_id: CorrelationId,
    /// Causality identifier for observability.
    pub causality_id: CausalityId,
    /// Creation timestamp assigned by the caller.
    pub created_at: TimestampMillis,
    /// Metadata-only context manifest reference used to generate the proposal.
    pub context_manifest: AssistedAiTrustProjectionReference,
    /// Metadata-only approval checklist reference gating the proposal.
    pub approval_checklist: AssistedAiTrustProjectionReference,
}

impl DelegatedTaskProposalGenerator {
    /// Creates a new proposal generator.
    pub fn new(sandbox_base: PathBuf) -> Self {
        Self { sandbox_base }
    }

    /// Compares sandbox directory state with HEAD checkout and builds AssistedAiEditProposalOutput.
    pub fn generate_proposal(
        &self,
        input: DelegatedTaskProposalInput<'_>,
    ) -> Result<AssistedAiEditProposalOutput, AgentError> {
        validate_containment(&self.sandbox_base, input.target_path)?;

        let target_relative = input
            .target_path
            .strip_prefix(&self.sandbox_base)
            .unwrap_or(input.target_path);
        let target_relative = target_relative.to_str().ok_or_else(|| {
            AgentError::InvalidMetadata(AssistedAiContractError::InvalidProposalMetadata {
                reason: "Proposal target path is not valid UTF-8".to_string(),
            })
        })?;

        Ok(AssistedAiEditProposalOutput {
            output_id: input.output_id,
            request_id: input.request_id,
            provider_id: input.provider_id,
            proposal_id: input.proposal_id,
            principal: input.principal,
            capability: input.capability,
            correlation_id: input.correlation_id,
            causality_id: input.causality_id,
            payload: ProposalPayload::CreateFile(devil_protocol::CreateFileProposal {
                path: CanonicalPath(target_relative.to_string()),
                initial_content: Some(input.modified_content.to_string()),
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
            created_at: input.created_at,
            context_manifest: input.context_manifest,
            approval_checklist: input.approval_checklist,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        })
    }
}

/// Metadata-only output from a Legion workflow coordinator action.
#[derive(Debug, Clone)]
pub enum LegionWorkflowCoordinatorOutput {
    /// Provider route metadata is required; no provider invocation was performed.
    ProviderRouteRequired(Box<AssistedAiProviderRouteRequest>),
    /// Proposal-only output metadata is ready; no proposal was applied.
    ProposalReady(Box<AssistedAiEditProposalOutput>),
    /// Conflict metadata blocks merge readiness until app-owned resolution.
    Conflict(Box<LegionWorkflowConflict>),
    /// Workflow merge readiness decision.
    MergeReadiness(LegionWorkflowMergeReadiness),
    /// Worker was blocked with display-safe reasons.
    Blocked {
        /// Worker id.
        worker_id: LegionWorkflowWorkerId,
        /// Display-safe reason labels.
        reasons: Vec<String>,
    },
}

/// Bounded Legion workflow coordinator over existing delegated-task primitives.
#[derive(Debug, Clone)]
pub struct LegionWorkflowCoordinator {
    session: LegionWorkflowSession,
    completed_worker_ids: Vec<LegionWorkflowWorkerId>,
    blocked_worker_ids: Vec<LegionWorkflowWorkerId>,
    provider_route_requests: Vec<AssistedAiProviderRouteRequest>,
    proposal_outputs: Vec<AssistedAiEditProposalOutput>,
    conflicts: Vec<LegionWorkflowConflict>,
}

impl LegionWorkflowCoordinator {
    /// Creates a coordinator from validated metadata-only workflow session data.
    pub fn new(session: LegionWorkflowSession) -> Result<Self, AgentError> {
        validate_legion_workflow_session(&session)
            .map_err(|error| AgentError::InvalidLegionWorkflow(error.message))?;
        if has_dependency_cycle(&session) {
            return Err(AgentError::LegionWorkflowDependencyCycle);
        }
        let completed_worker_ids = session
            .worker_assignments
            .iter()
            .filter(|worker| worker.state == LegionWorkflowWorkerState::Completed)
            .map(|worker| worker.worker_id.clone())
            .collect::<Vec<_>>();
        let blocked_worker_ids = session
            .worker_assignments
            .iter()
            .filter(|worker| {
                matches!(
                    worker.state,
                    LegionWorkflowWorkerState::Blocked
                        | LegionWorkflowWorkerState::Failed
                        | LegionWorkflowWorkerState::Cancelled
                )
            })
            .map(|worker| worker.worker_id.clone())
            .collect::<Vec<_>>();
        let conflicts = detect_initial_target_conflicts(&session);
        Ok(Self {
            conflicts,
            session,
            completed_worker_ids,
            blocked_worker_ids,
            provider_route_requests: Vec::new(),
            proposal_outputs: Vec::new(),
        })
    }

    /// Returns the workflow session metadata.
    pub fn session(&self) -> &LegionWorkflowSession {
        &self.session
    }

    /// Returns unresolved conflict metadata detected by the coordinator.
    pub fn conflicts(&self) -> &[LegionWorkflowConflict] {
        &self.conflicts
    }

    /// Returns provider route requests emitted by this coordinator.
    pub fn provider_route_requests(&self) -> &[AssistedAiProviderRouteRequest] {
        &self.provider_route_requests
    }

    /// Returns proposal-only outputs emitted by this coordinator.
    pub fn proposal_outputs(&self) -> &[AssistedAiEditProposalOutput] {
        &self.proposal_outputs
    }

    /// Returns workers whose dependencies are satisfied and that have not already ended.
    pub fn next_ready_workers(&self) -> Vec<LegionWorkflowWorkerAssignment> {
        self.session
            .worker_assignments
            .iter()
            .filter(|worker| {
                !self.completed_worker_ids.contains(&worker.worker_id)
                    && !self.blocked_worker_ids.contains(&worker.worker_id)
                    && worker_can_be_scheduled(worker.state)
                    && self.dependencies_satisfied_for(&worker.worker_id)
            })
            .cloned()
            .collect()
    }

    /// Marks a worker complete after its metadata-only output has been collected.
    pub fn mark_worker_completed(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
    ) -> Result<(), AgentError> {
        self.find_worker(worker_id)?;
        if self.completed_worker_ids.contains(worker_id) {
            return Err(AgentError::LegionWorkflowWorkerAlreadyCompleted(
                worker_id.0.clone(),
            ));
        }
        self.completed_worker_ids.push(worker_id.clone());
        Ok(())
    }

    /// Marks a worker blocked with display-safe reason labels.
    pub fn mark_worker_blocked(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
        reasons: Vec<String>,
    ) -> Result<LegionWorkflowCoordinatorOutput, AgentError> {
        self.find_worker(worker_id)?;
        if !self.blocked_worker_ids.contains(worker_id) {
            self.blocked_worker_ids.push(worker_id.clone());
        }
        Ok(LegionWorkflowCoordinatorOutput::Blocked {
            worker_id: worker_id.clone(),
            reasons,
        })
    }

    /// Emits provider route metadata for a provider-backed worker without invocation.
    pub fn provider_route_for_worker(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
    ) -> Result<LegionWorkflowCoordinatorOutput, AgentError> {
        let worker = self.find_worker(worker_id)?.clone();
        if worker.model_backend != LegionWorkflowModelBackend::ProviderBacked {
            return Err(AgentError::InvalidLegionWorkflow(
                "provider route requested for non-provider worker".to_string(),
            ));
        }
        let route_ref = worker.assisted_ai_route.clone().ok_or_else(|| {
            AgentError::InvalidLegionWorkflow(
                "provider-backed worker missing route metadata".to_string(),
            )
        })?;
        let route_request = provider_route_request_from_worker(&worker, route_ref);
        self.provider_route_requests.push(route_request.clone());
        Ok(LegionWorkflowCoordinatorOutput::ProviderRouteRequired(
            Box::new(route_request),
        ))
    }

    /// Records proposal-only worker output without applying it.
    pub fn record_proposal_output(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
        output: AssistedAiEditProposalOutput,
    ) -> Result<LegionWorkflowCoordinatorOutput, AgentError> {
        self.find_worker(worker_id)?;
        if output.correlation_id.0 == 0 || output.causality_id.0.is_nil() {
            return Err(AgentError::InvalidLegionWorkflow(
                "proposal output requires non-zero correlation and non-nil causality".to_string(),
            ));
        }
        if output.redaction_hints.contains(&RedactionHint::None) {
            return Err(AgentError::InvalidLegionWorkflow(
                "proposal output must remain metadata-redacted".to_string(),
            ));
        }
        self.proposal_outputs.push(output.clone());
        Ok(LegionWorkflowCoordinatorOutput::ProposalReady(Box::new(
            output,
        )))
    }

    /// Evaluates merge readiness from session and coordinator conflict metadata.
    pub fn merge_readiness(&self) -> LegionWorkflowMergeReadiness {
        let mut session = self.session.clone();
        session.conflict_summaries.extend(self.conflicts.clone());
        evaluate_legion_workflow_merge_readiness(&session)
    }

    /// Emits current merge readiness as a coordinator output.
    pub fn merge_readiness_output(&self) -> LegionWorkflowCoordinatorOutput {
        LegionWorkflowCoordinatorOutput::MergeReadiness(self.merge_readiness())
    }

    fn dependencies_satisfied_for(&self, worker_id: &LegionWorkflowWorkerId) -> bool {
        self.session
            .dependency_edges
            .iter()
            .filter(|dependency| &dependency.successor_worker_id == worker_id)
            .all(|dependency| {
                dependency.state == LegionWorkflowDependencyState::Satisfied
                    || self
                        .completed_worker_ids
                        .contains(&dependency.predecessor_worker_id)
            })
    }

    fn find_worker(
        &self,
        worker_id: &LegionWorkflowWorkerId,
    ) -> Result<&LegionWorkflowWorkerAssignment, AgentError> {
        self.session
            .worker_assignments
            .iter()
            .find(|worker| &worker.worker_id == worker_id)
            .ok_or_else(|| AgentError::UnknownLegionWorkflowWorker(worker_id.0.clone()))
    }
}

fn worker_can_be_scheduled(state: LegionWorkflowWorkerState) -> bool {
    matches!(
        state,
        LegionWorkflowWorkerState::Pending
            | LegionWorkflowWorkerState::Ready
            | LegionWorkflowWorkerState::WaitingForDependency
            | LegionWorkflowWorkerState::ProviderRouteRequired
    )
}

fn has_dependency_cycle(session: &LegionWorkflowSession) -> bool {
    let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
    for dependency in &session.dependency_edges {
        outgoing
            .entry(dependency.predecessor_worker_id.0.as_str())
            .or_default()
            .push(dependency.successor_worker_id.0.as_str());
    }

    fn visit<'a>(
        node: &'a str,
        outgoing: &HashMap<&'a str, Vec<&'a str>>,
        visiting: &mut HashSet<&'a str>,
        visited: &mut HashSet<&'a str>,
    ) -> bool {
        if visited.contains(node) {
            return false;
        }
        if !visiting.insert(node) {
            return true;
        }
        if let Some(next_nodes) = outgoing.get(node) {
            for next in next_nodes {
                if visit(next, outgoing, visiting, visited) {
                    return true;
                }
            }
        }
        visiting.remove(node);
        visited.insert(node);
        false
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    session.worker_assignments.iter().any(|worker| {
        visit(
            worker.worker_id.0.as_str(),
            &outgoing,
            &mut visiting,
            &mut visited,
        )
    })
}

fn detect_initial_target_conflicts(session: &LegionWorkflowSession) -> Vec<LegionWorkflowConflict> {
    let mut target_owner: HashMap<String, LegionWorkflowWorkerId> = HashMap::new();
    let mut conflicts = Vec::new();
    for worker in &session.worker_assignments {
        for target in &worker.affected_targets {
            let target_label = target
                .labels
                .first()
                .cloned()
                .unwrap_or_else(|| target.target_id.clone());
            if let Some(existing_worker_id) = target_owner.get(&target_label) {
                if !has_dependency_between(session, existing_worker_id, &worker.worker_id) {
                    conflicts.push(LegionWorkflowConflict {
                        conflict_id: LegionWorkflowConflictId(format!(
                            "legion-conflict:{}:{}",
                            existing_worker_id.0, worker.worker_id.0
                        )),
                        kind: LegionWorkflowConflictKind::SameTarget,
                        state: LegionWorkflowConflictState::Unresolved,
                        worker_ids: vec![existing_worker_id.clone(), worker.worker_id.clone()],
                        target_label: target_label.clone(),
                        target_hash: target.hashes.first().cloned(),
                        labels: vec!["legion_workflow.same_target_conflict".to_string()],
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                        schema_version: 1,
                    });
                }
            } else {
                target_owner.insert(target_label, worker.worker_id.clone());
            }
        }
    }
    conflicts
}

fn has_dependency_between(
    session: &LegionWorkflowSession,
    left: &LegionWorkflowWorkerId,
    right: &LegionWorkflowWorkerId,
) -> bool {
    session.dependency_edges.iter().any(|dependency| {
        (&dependency.predecessor_worker_id == left && &dependency.successor_worker_id == right)
            || (&dependency.predecessor_worker_id == right
                && &dependency.successor_worker_id == left)
    })
}

fn provider_route_request_from_worker(
    worker: &LegionWorkflowWorkerAssignment,
    route_ref: AssistedAiTrustProjectionReference,
) -> AssistedAiProviderRouteRequest {
    let targets = worker
        .affected_targets
        .iter()
        .map(|target| ProposalAffectedTarget {
            target_id: target.target_id.clone(),
            kind: target.kind,
            workspace_id: target.workspace_id,
            file_id: target.file_id,
            buffer_id: target.buffer_id,
            path: None,
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: target.ranges.clone(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
        })
        .collect::<Vec<_>>();

    AssistedAiProviderRouteRequest {
        route_id: format!("legion-route:{}", worker.worker_id.0),
        provider_id: route_ref.reference_id.clone(),
        model_label: worker.display_safe_model_label.clone(),
        provider_class: AssistedAiProviderClass::HostedRemote,
        operation_class: AssistedAiOperationClass::ProposeEdit,
        context_manifest: route_ref.clone(),
        privacy_inspector: route_ref.clone(),
        permission_budget: route_ref,
        proposal_intent: AssistedAiProposalTargetIntent {
            payload_kind: ProposalPayloadKind::CreateFile,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets,
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            required_capability: CapabilityId("legion.workflow.propose".to_string()),
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["legion_workflow.provider_route_metadata".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        policy_decision_id: None,
        required_capability: CapabilityId("legion.workflow.provider_route".to_string()),
        network_target: None,
        cancellation_token: CancellationTokenId(uuid::Uuid::from_u128(13)),
        health_labels: vec!["provider_route.not_invoked".to_string()],
        cost_labels: vec!["cost.metadata_only".to_string()],
        principal_id: PrincipalId("legion.workflow.coordinator".to_string()),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        correlation_id: worker.correlation_id,
        causality_id: worker.causality_id,
        event_sequence: EventSequence(13),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::{
        AgentReplayManifest, AssistedAiTrustProjectionKind, CommandRiskLabel,
        ContextManifestItemCount, DelegatedTaskAffectedTargetSummary, DelegatedTaskOperationClass,
        DelegatedTaskPlanId, FileFingerprint, LegionWorkflowDependency, LegionWorkflowDependencyId,
        LegionWorkflowDependencyState, LegionWorkflowMergeApproval,
        LegionWorkflowMergeReadinessBlocker, LegionWorkflowSessionId, LegionWorkflowSignOff,
        LegionWorkflowSignOffId, LegionWorkflowSignOffState, LegionWorkflowState,
        LegionWorkflowVerificationGate, LegionWorkflowVerificationGateId,
        LegionWorkflowVerificationGateState, LegionWorkflowWorkerRole, LegionWorkflowWorkerState,
        PrivacyClassification, ProposalPrivacyLabel, ProposalRiskLabel, ProposalTargetKind,
        RedactionHint, validate_legion_workflow_session,
    };
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
        let permission = approved_sandbox_permission("sandbox:init");
        orchestrator
            .initialize(&permission)
            .expect("initialize sandbox");
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
        let modified_content = "fn generated_from_request() {}\n";
        let proposal = generator
            .generate_proposal(proposal_input(&target_file, modified_content))
            .expect("generate proposal");

        assert_eq!(proposal.output_id, "output:request-derived");
        assert_eq!(proposal.request_id, "request:delegate");
        assert_eq!(proposal.provider_id, "provider:local-deterministic");
        assert_eq!(proposal.proposal_id, ProposalId(4242));
        assert_eq!(
            proposal.principal,
            PrincipalId("principal:user".to_string())
        );
        assert_eq!(
            proposal.capability,
            CapabilityId("capability:write-proposal".to_string())
        );
        assert_eq!(proposal.correlation_id, CorrelationId(42));
        assert_eq!(proposal.causality_id, causality(42));
        assert_eq!(proposal.created_at, TimestampMillis(424242));
        assert_eq!(proposal.context_manifest.reference_id, "context:manifest");
        assert_eq!(
            proposal.context_manifest.kind,
            AssistedAiTrustProjectionKind::ContextManifest
        );
        assert_eq!(
            proposal.approval_checklist.reference_id,
            "approval:checklist"
        );
        assert_eq!(
            proposal.approval_checklist.kind,
            AssistedAiTrustProjectionKind::ProposalApprovalChecklist
        );
        assert_eq!(proposal.redaction_hints, vec![RedactionHint::MetadataOnly]);
        match &proposal.payload {
            ProposalPayload::CreateFile(create_file) => {
                assert_eq!(create_file.path.0, "src/lib.rs");
                assert_eq!(
                    create_file.initial_content.as_deref(),
                    Some(modified_content)
                );
            }
            _ => panic!("expected create-file proposal"),
        }

        // Cleanup
        orchestrator.cleanup(&permission).expect("cleanup sandbox");
        assert!(!sandbox_path.exists());
    }

    #[test]
    fn delegated_sandbox_requires_approved_write_tool_permission() {
        let mut orchestrator = DelegatedTaskSandboxOrchestrator::new("permission-test-run");
        let denied = devil_protocol::delegated_task_tool_permission_request(
            devil_protocol::DelegatedTaskToolPermissionRequestInput {
                request_id: "sandbox:denied".to_string(),
                profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::AccessWorkspaceFiles,
                capability: Some(CapabilityId("delegated.runtime.allocate".to_string())),
                target_id: Some("target/delegated-tasks".to_string()),
                decision: devil_protocol::DelegatedTaskToolPermissionDecision::Deny,
                labels: vec!["test".to_string()],
                schema_version: 1,
            },
        );

        let error = orchestrator
            .initialize(&denied)
            .expect_err("denied permission blocks sandbox init");
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(!orchestrator.sandbox_path().exists());
    }

    fn approved_sandbox_permission(
        request_id: &str,
    ) -> devil_protocol::DelegatedTaskToolPermissionRequest {
        devil_protocol::delegated_task_tool_permission_request(
            devil_protocol::DelegatedTaskToolPermissionRequestInput {
                request_id: request_id.to_string(),
                profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::AccessWorkspaceFiles,
                capability: Some(CapabilityId("delegated.runtime.allocate".to_string())),
                target_id: Some("target/delegated-tasks".to_string()),
                decision: devil_protocol::DelegatedTaskToolPermissionDecision::Allow,
                labels: vec!["test".to_string()],
                schema_version: 1,
            },
        )
    }

    fn workflow_hash(value: &str) -> FileFingerprint {
        FileFingerprint {
            algorithm: "sha256".to_string(),
            value: value.to_string(),
        }
    }

    fn workflow_ref(id: &str) -> AssistedAiTrustProjectionReference {
        AssistedAiTrustProjectionReference {
            reference_id: id.to_string(),
            kind: AssistedAiTrustProjectionKind::AssistedAiProjection,
            projection_hash: workflow_hash(id),
            schema_version: 1,
        }
    }

    fn proposal_ref(
        id: &str,
        kind: AssistedAiTrustProjectionKind,
    ) -> AssistedAiTrustProjectionReference {
        AssistedAiTrustProjectionReference {
            reference_id: id.to_string(),
            kind,
            projection_hash: workflow_hash(id),
            schema_version: 1,
        }
    }

    fn proposal_input<'a>(
        target_path: &'a Path,
        modified_content: &'a str,
    ) -> DelegatedTaskProposalInput<'a> {
        DelegatedTaskProposalInput {
            target_path,
            modified_content,
            output_id: "output:request-derived".to_string(),
            request_id: "request:delegate".to_string(),
            provider_id: "provider:local-deterministic".to_string(),
            proposal_id: ProposalId(4242),
            principal: PrincipalId("principal:user".to_string()),
            capability: CapabilityId("capability:write-proposal".to_string()),
            correlation_id: CorrelationId(42),
            causality_id: causality(42),
            created_at: TimestampMillis(424242),
            context_manifest: proposal_ref(
                "context:manifest",
                AssistedAiTrustProjectionKind::ContextManifest,
            ),
            approval_checklist: proposal_ref(
                "approval:checklist",
                AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
            ),
        }
    }

    fn workflow_target(label: &str) -> DelegatedTaskAffectedTargetSummary {
        DelegatedTaskAffectedTargetSummary {
            target_id: format!("target:{label}"),
            kind: ProposalTargetKind::MetadataOnly,
            workspace_id: None,
            file_id: None,
            buffer_id: None,
            ranges: Vec::new(),
            hashes: vec![workflow_hash(label)],
            counts: vec![ContextManifestItemCount {
                label: "target-count".to_string(),
                count: 1,
            }],
            labels: vec![label.to_string()],
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn workflow_worker(
        id: &str,
        backend: LegionWorkflowModelBackend,
        target_label: &str,
    ) -> LegionWorkflowWorkerAssignment {
        LegionWorkflowWorkerAssignment {
            worker_id: LegionWorkflowWorkerId(id.to_string()),
            role: LegionWorkflowWorkerRole::Implementer,
            state: if backend == LegionWorkflowModelBackend::ProviderBacked {
                LegionWorkflowWorkerState::ProviderRouteRequired
            } else {
                LegionWorkflowWorkerState::Ready
            },
            model_backend: backend,
            display_safe_model_label: format!("{id}:metadata"),
            allowed_command_classes: vec![DelegatedTaskOperationClass::DraftProposalMetadata],
            linked_delegated_plan_id: Some(DelegatedTaskPlanId(format!("plan:{id}"))),
            assisted_ai_route: (backend == LegionWorkflowModelBackend::ProviderBacked)
                .then(|| workflow_ref(&format!("route:{id}"))),
            affected_targets: vec![workflow_target(target_label)],
            risk_labels: vec![CommandRiskLabel::Review],
            privacy_labels: vec![PrivacyClassification::Metadata],
            correlation_id: CorrelationId(901),
            causality_id: causality(901),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn workflow_session() -> LegionWorkflowSession {
        LegionWorkflowSession {
            session_id: LegionWorkflowSessionId("session:legion:agent".to_string()),
            directive_artifact_id: Some("artifact:directive:agent".to_string()),
            spec_artifact_id: Some("artifact:spec:agent".to_string()),
            task_graph_artifact_id: Some("artifact:task-graph:agent".to_string()),
            product_mode: devil_protocol::ProductMode::LegionWorkflows,
            worker_assignments: vec![
                workflow_worker(
                    "worker:local",
                    LegionWorkflowModelBackend::Local,
                    "crates/devil-agent/src/lib.rs",
                ),
                workflow_worker(
                    "worker:provider",
                    LegionWorkflowModelBackend::ProviderBacked,
                    "crates/devil-agent/tests/review.rs",
                ),
            ],
            dependency_edges: vec![LegionWorkflowDependency {
                dependency_id: LegionWorkflowDependencyId("dependency:local-provider".to_string()),
                predecessor_worker_id: LegionWorkflowWorkerId("worker:local".to_string()),
                successor_worker_id: LegionWorkflowWorkerId("worker:provider".to_string()),
                state: LegionWorkflowDependencyState::Pending,
                label: "local before provider".to_string(),
                schema_version: 1,
            }],
            conflict_summaries: Vec::new(),
            verification_gates: vec![LegionWorkflowVerificationGate {
                gate_id: LegionWorkflowVerificationGateId("verification:agent".to_string()),
                state: LegionWorkflowVerificationGateState::Passed,
                label: "agent tests".to_string(),
                evidence_artifact_id: Some("artifact:evidence:agent".to_string()),
                command_class_label: "cargo-test".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            sign_off_records: vec![LegionWorkflowSignOff {
                sign_off_id: LegionWorkflowSignOffId("signoff:agent".to_string()),
                state: LegionWorkflowSignOffState::SignedOff,
                required_role: LegionWorkflowWorkerRole::Reviewer,
                reviewer_principal_id: Some(PrincipalId("principal:reviewer".to_string())),
                label: "review sign-off".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            proposal_ids: vec![ProposalId(1303)],
            merge_approval: Some(LegionWorkflowMergeApproval {
                approval_artifact_id: Some("artifact:approval:agent".to_string()),
                approval_granted: true,
                rollback_available: true,
                audit_persisted_before_success: true,
                main_workspace_dirty_conflict: false,
                proposal_preconditions_stale: false,
                labels: vec!["approval-gated".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }),
            lifecycle_state: LegionWorkflowState::Executing,
            generated_at: TimestampMillis(1303),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
            correlation_id: CorrelationId(901),
            causality_id: causality(902),
        }
    }

    #[test]
    fn legion_workflow_ready_worker_order_follows_dependencies() {
        let mut coordinator =
            LegionWorkflowCoordinator::new(workflow_session()).expect("valid workflow");

        let ready = coordinator.next_ready_workers();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].worker_id.0, "worker:local");

        coordinator
            .mark_worker_completed(&LegionWorkflowWorkerId("worker:local".to_string()))
            .expect("worker completes");
        let ready = coordinator.next_ready_workers();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].worker_id.0, "worker:provider");
    }

    #[test]
    fn legion_workflow_resume_uses_persisted_worker_and_dependency_state() {
        let mut session = workflow_session();
        session.worker_assignments = vec![
            workflow_worker(
                "worker:root",
                LegionWorkflowModelBackend::Local,
                "crates/devil-agent/src/root.rs",
            ),
            workflow_worker(
                "worker:child",
                LegionWorkflowModelBackend::Local,
                "crates/devil-agent/src/child.rs",
            ),
        ];
        session.worker_assignments[0].state = LegionWorkflowWorkerState::Completed;
        session.worker_assignments[1].state = LegionWorkflowWorkerState::Ready;
        session.dependency_edges = vec![LegionWorkflowDependency {
            dependency_id: LegionWorkflowDependencyId("dependency:root-child".to_string()),
            predecessor_worker_id: LegionWorkflowWorkerId("worker:root".to_string()),
            successor_worker_id: LegionWorkflowWorkerId("worker:child".to_string()),
            state: LegionWorkflowDependencyState::Satisfied,
            label: "root before child".to_string(),
            schema_version: 1,
        }];

        let coordinator = LegionWorkflowCoordinator::new(session).expect("valid resumed workflow");
        let ready = coordinator.next_ready_workers();

        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].worker_id.0, "worker:child");
    }

    #[test]
    fn legion_workflow_dependency_cycle_is_blocked() {
        let mut session = workflow_session();
        session.dependency_edges.push(LegionWorkflowDependency {
            dependency_id: LegionWorkflowDependencyId("dependency:provider-local".to_string()),
            predecessor_worker_id: LegionWorkflowWorkerId("worker:provider".to_string()),
            successor_worker_id: LegionWorkflowWorkerId("worker:local".to_string()),
            state: LegionWorkflowDependencyState::Pending,
            label: "provider before local".to_string(),
            schema_version: 1,
        });

        let error = LegionWorkflowCoordinator::new(session).expect_err("cycle must block");
        assert_eq!(error, AgentError::LegionWorkflowDependencyCycle);
    }

    #[test]
    fn legion_workflow_provider_worker_emits_route_metadata_without_invocation() {
        let mut coordinator =
            LegionWorkflowCoordinator::new(workflow_session()).expect("valid workflow");

        let output = coordinator
            .provider_route_for_worker(&LegionWorkflowWorkerId("worker:provider".to_string()))
            .expect("provider route metadata");

        match output {
            LegionWorkflowCoordinatorOutput::ProviderRouteRequired(route) => {
                assert_eq!(route.provider_id, "route:worker:provider");
                assert_eq!(route.operation_class, AssistedAiOperationClass::ProposeEdit);
                assert!(
                    route
                        .health_labels
                        .contains(&"provider_route.not_invoked".to_string())
                );
                assert_eq!(route.redaction_hints, vec![RedactionHint::MetadataOnly]);
            }
            _ => panic!("expected provider route metadata"),
        }
        assert_eq!(coordinator.provider_route_requests().len(), 1);
    }

    #[test]
    fn legion_workflow_same_target_conflict_blocks_merge_readiness() {
        let mut session = workflow_session();
        session.dependency_edges.clear();
        session.worker_assignments[1].affected_targets =
            session.worker_assignments[0].affected_targets.clone();

        validate_legion_workflow_session(&session).expect("session shape valid");
        let coordinator = LegionWorkflowCoordinator::new(session).expect("coordinator starts");

        assert_eq!(coordinator.conflicts().len(), 1);
        assert!(
            coordinator
                .merge_readiness()
                .blockers
                .contains(&LegionWorkflowMergeReadinessBlocker::UnresolvedConflict)
        );
    }

    #[test]
    fn legion_workflow_local_proposal_output_remains_proposal_only() {
        let mut coordinator =
            LegionWorkflowCoordinator::new(workflow_session()).expect("valid workflow");
        let sandbox = PathBuf::from("target/legion-workflow-agent-test");
        std::fs::create_dir_all(&sandbox).expect("create sandbox");
        let generator = DelegatedTaskProposalGenerator::new(sandbox.clone());
        let target_path = sandbox.join("generated.txt");
        let proposal = generator
            .generate_proposal(proposal_input(&target_path, "proposal metadata"))
            .expect("proposal output");

        let output = coordinator
            .record_proposal_output(
                &LegionWorkflowWorkerId("worker:local".to_string()),
                proposal,
            )
            .expect("record proposal output");

        match output {
            LegionWorkflowCoordinatorOutput::ProposalReady(proposal) => {
                assert_eq!(proposal.proposal_id, ProposalId(4242));
                assert_eq!(proposal.redaction_hints, vec![RedactionHint::MetadataOnly]);
                match &proposal.payload {
                    ProposalPayload::CreateFile(create_file) => {
                        assert_eq!(
                            create_file.initial_content.as_deref(),
                            Some("proposal metadata")
                        );
                    }
                    _ => panic!("expected create-file proposal"),
                }
            }
            _ => panic!("expected proposal-ready metadata"),
        }
        assert_eq!(coordinator.proposal_outputs().len(), 1);
        std::fs::remove_dir_all(sandbox).expect("cleanup sandbox");
    }

    #[test]
    fn legion_workflow_blocked_worker_is_not_rescheduled() {
        let mut coordinator =
            LegionWorkflowCoordinator::new(workflow_session()).expect("valid workflow");
        let output = coordinator
            .mark_worker_blocked(
                &LegionWorkflowWorkerId("worker:local".to_string()),
                vec!["policy.blocked".to_string()],
            )
            .expect("mark blocked");

        assert!(matches!(
            output,
            LegionWorkflowCoordinatorOutput::Blocked { .. }
        ));
        assert!(coordinator.next_ready_workers().is_empty());
    }

    #[test]
    fn legion_workflow_agent_crate_does_not_import_app_ui_or_desktop() {
        let source = include_str!("lib.rs");
        let forbidden_imports = [
            ["devil", "_app"].concat(),
            ["devil", "_ui"].concat(),
            ["devil", "_desktop"].concat(),
            ["devil", "_editor"].concat(),
            ["devil", "_project"].concat(),
        ];

        for forbidden in forbidden_imports {
            assert!(
                !source.contains(&forbidden),
                "unexpected forbidden import {forbidden}"
            );
        }
    }
}
