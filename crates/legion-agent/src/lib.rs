//! Agent workflows: plans, tool-use state machines, capability-scoped automation.

#![warn(missing_docs)]

/// Parsed communication log lines for agent workflows.
pub mod comm;
/// Approved-plan DAG helpers for Legion workflow coordination.
pub mod dag;
/// Merge-readiness reports with verification-evidence citations.
pub mod merge_readiness;
/// Editable plan construction helpers for agent workflows.
pub mod plan;
/// Task scheduling and lane management.
pub mod scheduler;
/// Scope enforcement for delegated task tool calls.
pub mod scope;
/// Tool definitions and re-exports.
pub mod tools;

pub use scope::{tool_call_feedback_for_scope_denial, validate_delegated_task_tool_call};

use legion_protocol::{
    AgentReplayManifest, AgentRunId, AgentRunState, AgentStateTransitionRecord,
    AssistedAiContractError, AssistedAiEditProposalOutput, AssistedAiOperationClass,
    AssistedAiProposalTargetIntent, AssistedAiProviderClass, AssistedAiProviderRouteRequest,
    AssistedAiTrustProjectionReference, CancellationTokenId, CanonicalPath, CapabilityId,
    CausalityId, CorrelationId, DelegatedTaskToolPermissionProfile,
    DelegatedTaskToolPermissionRequest, EventSequence, FileFingerprint, LegionEvidenceKind,
    LegionEvidencePrivacyScope, LegionEvidenceRecord, LegionEvidenceSource, LegionModelCapability,
    LegionProviderLocalityPreference, LegionProviderPrivacyPolicy, LegionProviderRouteHealth,
    LegionProviderRouteMetadata, LegionTaskFileScope, LegionTaskOutputContract, LegionTaskPacket,
    LegionTaskPacketId, LegionTaskPolicy, LegionTaskValidationPlan, LegionToolKind,
    LegionWorkerResult, LegionWorkerResultKind, LegionWorkflowConflict, LegionWorkflowConflictId,
    LegionWorkflowConflictKind, LegionWorkflowConflictState, LegionWorkflowDependencyState,
    LegionWorkflowMergeReadiness, LegionWorkflowModelBackend, LegionWorkflowSession,
    LegionWorkflowWorkerAssignment, LegionWorkflowWorkerId, LegionWorkflowWorkerState,
    PermissionBudgetActionClass, PreviewSummary, PrincipalId, ProposalAffectedTarget, ProposalId,
    ProposalPayload, ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel,
    ProposalTargetCoverage, ProposalTargetCoverageKind, ProposalVersionPreconditions,
    RedactionHint, TimestampMillis, WorkspaceTrustState, evaluate_legion_workflow_merge_readiness,
    validate_agent_replay_manifest, validate_legion_evidence_record,
    validate_legion_provider_route_metadata, validate_legion_task_packet,
    validate_legion_worker_result, validate_legion_workflow_session,
    validate_phase4_runtime_audit_record,
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
        validate_phase4_runtime_audit_record(&legion_protocol::Phase4RuntimeAuditRecord {
            audit_id: format!("agent:{}:{}", self.run_id.0, event_sequence.0),
            run_id: Some(self.run_id.clone()),
            step_id: None,
            provider_route_id: None,
            invocation_state: legion_protocol::AssistedAiProviderInvocationState::NotEncoded,
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
    source_root: Option<PathBuf>,
    is_worktree: bool,
}

impl DelegatedTaskSandboxOrchestrator {
    /// Creates a new orchestrator.
    pub fn new(run_id: &str) -> Self {
        let sandbox_path = PathBuf::from("target/delegated-tasks").join(format!("task-{}", run_id));
        Self {
            sandbox_path,
            source_root: None,
            is_worktree: false,
        }
    }

    /// Creates a new orchestrator that isolates a specific workspace root.
    pub fn with_workspace_root(source_root: &Path, run_id: &str) -> Self {
        let sandbox_path = PathBuf::from("target/delegated-tasks").join(format!("task-{}", run_id));
        Self {
            sandbox_path,
            source_root: Some(source_root.to_path_buf()),
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

        // Try git worktree first. Pass the path as an OsStr so a non-UTF-8
        // PathBuf cannot panic the process.
        let mut command = Command::new("git");
        if let Some(source_root) = &self.source_root {
            command.arg("-C").arg(source_root);
        }
        let output = command
            .arg("worktree")
            .arg("add")
            .arg(&self.sandbox_path)
            .arg("HEAD")
            .output();

        match output {
            Ok(output) if output.status.success() => {
                self.is_worktree = true;
                Ok(())
            }
            _ => {
                self.is_worktree = false;
                std::fs::create_dir_all(&self.sandbox_path)?;
                if let Some(source_root) = &self.source_root {
                    copy_workspace_tree(source_root, &self.sandbox_path)?;
                }
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

/// NOTE: `crates/legion-app/src/offline_ai.rs::reap_orphaned_sandboxes`
/// mirrors this logic for offline builds — apply any change to both.
///
/// Removes orphaned sandbox directories under `delegated_tasks_root`.
///
/// A directory is an orphan when its name starts with `task-` and its
/// run-id suffix is not in `active_run_ids`. Attempts `git worktree
/// remove --force` first (mirroring `initialize`'s worktree-first
/// strategy) and falls back to plain directory removal. Returns the
/// paths that were removed. A missing root is a successful no-op.
pub fn reap_orphaned_sandboxes(
    delegated_tasks_root: &Path,
    active_run_ids: &[String],
) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut removed = Vec::new();
    if !delegated_tasks_root.exists() {
        return Ok(removed);
    }
    for entry in std::fs::read_dir(delegated_tasks_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(run_id) = name.strip_prefix("task-") else {
            continue;
        };
        if active_run_ids.iter().any(|active| active == run_id) {
            continue;
        }
        let path = entry.path();
        let worktree_removed = Command::new("git")
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&path)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if !worktree_removed {
            std::fs::remove_dir_all(&path)?;
        }
        removed.push(path);
    }
    Ok(removed)
}

fn copy_workspace_tree(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path == destination {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            std::fs::create_dir_all(&destination_path)?;
            copy_workspace_tree(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = destination_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
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

/// Validate that `path` is contained within the base directory.
///
/// Returns the lexically-normalized path *relative to* `base` (with any `.`/`..`
/// segments already collapsed) so callers emit a genuinely canonical path rather
/// than one that still embeds traversal segments. Any path that escapes the base
/// or retains residual traversal/root/prefix components after normalization is
/// rejected.
pub fn validate_containment(base: &Path, path: &Path) -> Result<PathBuf, AgentError> {
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

    let relative = clean_stripped.strip_prefix(&base_stripped).map_err(|_| {
        AgentError::InvalidMetadata(AssistedAiContractError::InvalidProposalMetadata {
            reason: "Path traversal escaped sandbox".to_string(),
        })
    })?;

    // Defense in depth: a normalized, contained path must not retain any
    // traversal, root, or prefix components.
    if relative.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) {
        return Err(AgentError::InvalidMetadata(
            AssistedAiContractError::InvalidProposalMetadata {
                reason: "Normalized sandbox path retained traversal components".to_string(),
            },
        ));
    }

    Ok(relative.to_path_buf())
}

/// Proposal generator inside `legion-agent`.
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

    /// Builds an `AssistedAiEditProposalOutput` for the sandbox target.
    ///
    /// Stats and reads the base target (the HEAD checkout / sandbox-base copy at
    /// `target_path`) and compares it with the provider-produced
    /// `modified_content` to decide the payload shape:
    ///
    /// * When the base target does **not** exist this is a genuine creation and a
    ///   [`ProposalPayload::CreateFile`] is emitted with no preconditions.
    /// * When the base target **already exists** a path-based
    ///   [`ProposalPayload::CreateFile`] (create/overwrite) is emitted carrying
    ///   the full modified content plus a content-level concurrency guard. The
    ///   generator has no workspace `FileId` or open buffer, so it cannot emit a
    ///   buffer-addressable `TextEdit` (the apply path resolves edits via
    ///   `buffer_for_file`); a path-based proposal with preconditions is the
    ///   appliable representation.
    ///
    /// For an existing base the size, last-modified timestamp, content
    /// fingerprint, and a content-derived [`legion_protocol::FileContentVersion`]
    /// are recorded in [`ProposalVersionPreconditions`] so an apply step can
    /// detect concurrent modification. The proposal target path is the
    /// normalized, sandbox-relative path returned by [`validate_containment`].
    pub fn generate_proposal(
        &self,
        input: DelegatedTaskProposalInput<'_>,
    ) -> Result<AssistedAiEditProposalOutput, AgentError> {
        let target_relative = validate_containment(&self.sandbox_base, input.target_path)?;
        // Emit a canonical, forward-slash relative path regardless of host OS so
        // the `CanonicalPath` payload is portable and free of `..` segments.
        let target_relative = target_relative
            .components()
            .map(|component| {
                component.as_os_str().to_str().ok_or_else(|| {
                    AgentError::InvalidMetadata(AssistedAiContractError::InvalidProposalMetadata {
                        reason: "Proposal target path is not valid UTF-8".to_string(),
                    })
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/");

        // Stat/read the base target so we can both populate a concurrency guard
        // and decide whether this is a create (no base) or an edit (base exists).
        let base_state = BaseTargetState::read(input.target_path);

        let create_payload = || {
            ProposalPayload::CreateFile(legion_protocol::CreateFileProposal {
                path: CanonicalPath(target_relative.clone()),
                initial_content: Some(input.modified_content.to_string()),
            })
        };

        // The delegated generator works from the sandbox output and has no
        // workspace `FileId` or open editor buffer. A `TextEdit` proposal would
        // need a real buffer-addressable file id (the apply path resolves edits
        // via `buffer_for_file`), so a synthetic id would make existing-file
        // proposals reject at apply time. Always emit a path-based create/
        // overwrite proposal; when a base already exists, attach a content-level
        // concurrency guard derived from its snapshot.
        let (payload, preconditions) = match &base_state {
            Some(base) => (create_payload(), base.preconditions()),
            None => (create_payload(), empty_preconditions()),
        };
        let preview_summary = "Create file proposal".to_string();

        Ok(AssistedAiEditProposalOutput {
            output_id: input.output_id,
            request_id: input.request_id,
            provider_id: input.provider_id,
            proposal_id: input.proposal_id,
            principal: input.principal,
            capability: input.capability,
            correlation_id: input.correlation_id,
            causality_id: input.causality_id,
            payload,
            preconditions,
            preview: PreviewSummary {
                summary: preview_summary,
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

/// On-disk state of a base proposal target captured once so that both the
/// concurrency guard and the create-vs-edit decision use a consistent snapshot.
struct BaseTargetState {
    /// Length of the base file in bytes.
    len: u64,
    /// Last-modified timestamp when the platform exposes one.
    modified_at: Option<TimestampMillis>,
    /// Stable FNV-1a digest of the base bytes when readable.
    content_hash: Option<u64>,
}

impl BaseTargetState {
    /// Reads the base target at `path`. Returns `None` when the target does not
    /// exist or is not a regular file (a genuine create). Any read error after a
    /// successful stat still yields a state (with `content_hash == None`) so the
    /// generator treats an existing file as an overwrite rather than
    /// misclassifying it as a create.
    fn read(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file() {
            return None;
        }
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|elapsed| TimestampMillis(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)));
        let content_hash = std::fs::read(path)
            .ok()
            .map(|bytes| stable_hash_128(&bytes) as u64);
        Some(Self {
            len: metadata.len(),
            modified_at,
            content_hash,
        })
    }

    /// Derives a concurrency guard from this base snapshot. The length and
    /// modified timestamp always populate; the fingerprint and content version
    /// populate only when the base bytes were readable.
    fn preconditions(&self) -> ProposalVersionPreconditions {
        ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: self.content_hash.map(legion_protocol::FileContentVersion),
            workspace_generation: None,
            expected_fingerprint: self.content_hash.map(|hash| FileFingerprint {
                algorithm: "fnv1a-64-v1".to_string(),
                value: format!("{hash:016x}"),
            }),
            expected_file_length: Some(self.len),
            expected_modified_at: self.modified_at,
        }
    }
}

/// Preconditions for a genuine create: nothing on disk to guard against.
fn empty_preconditions() -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        file_version: None,
        buffer_version: None,
        snapshot_id: None,
        generation: None,
        file_content_version: None,
        workspace_generation: None,
        expected_fingerprint: None,
        expected_file_length: None,
        expected_modified_at: None,
    }
}

/// Deterministic, cross-version-stable 128-bit FNV-1a hash.
///
/// Unlike `std::collections::hash_map::DefaultHasher`, FNV-1a is a fixed
/// published specification, so identifiers and fingerprints derived from it stay
/// stable across compiler versions, platforms, and runs. The unit-separator
/// (`\u{1f}`) between domain prefix and payload prevents prefix-collision
/// ambiguity between distinct id namespaces.
fn stable_hash_128(bytes: &[u8]) -> u128 {
    // FNV-1a (128-bit) constants.
    const FNV_OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= byte as u128;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
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
    /// Canonical task packet metadata is ready for a worker.
    TaskPacketReady(Box<LegionTaskPacket>),
    /// Canonical provider route metadata is ready for a provider-backed worker.
    ProviderRouteMetadataReady(Box<LegionProviderRouteMetadata>),
    /// Canonical worker result metadata is ready.
    WorkerResultReady(Box<LegionWorkerResult>),
    /// Canonical evidence record is ready.
    EvidenceReady(Box<LegionEvidenceRecord>),
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
    task_packets: Vec<LegionTaskPacket>,
    worker_results: Vec<LegionWorkerResult>,
    evidence_records: Vec<LegionEvidenceRecord>,
    provider_route_metadata: Vec<LegionProviderRouteMetadata>,
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
            task_packets: Vec::new(),
            worker_results: Vec::new(),
            evidence_records: Vec::new(),
            provider_route_metadata: Vec::new(),
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

    /// Returns canonical task packets emitted by this coordinator.
    pub fn task_packets(&self) -> &[LegionTaskPacket] {
        &self.task_packets
    }

    /// Returns canonical worker results emitted by this coordinator.
    pub fn worker_results(&self) -> &[LegionWorkerResult] {
        &self.worker_results
    }

    /// Returns canonical evidence records emitted by this coordinator.
    pub fn evidence_records(&self) -> &[LegionEvidenceRecord] {
        &self.evidence_records
    }

    /// Returns canonical provider route metadata emitted by this coordinator.
    pub fn provider_route_metadata(&self) -> &[LegionProviderRouteMetadata] {
        &self.provider_route_metadata
    }

    /// Returns the worker result for a specific worker, if any.
    pub fn worker_result_for_worker(
        &self,
        worker_id: &LegionWorkflowWorkerId,
    ) -> Option<&LegionWorkerResult> {
        let expected_id = format!("legion-result:{}", worker_id.0);
        self.worker_results
            .iter()
            .find(|r| r.result_id == expected_id)
    }

    /// Returns evidence records for a specific worker.
    pub fn evidence_records_for_worker(
        &self,
        worker_id: &LegionWorkflowWorkerId,
    ) -> Vec<&LegionEvidenceRecord> {
        let expected_id = format!("legion-evidence:{}", worker_id.0);
        self.evidence_records
            .iter()
            .filter(|e| e.evidence_id == expected_id)
            .collect()
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

    /// Emits a `ProviderRouteRequired` for a provider-backed worker without invocation.
    /// Records provider-route metadata internally; repeated calls return the stored request.
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
        let route_id = format!("legion-route:{}", worker.worker_id.0);
        if let Some(existing) = self
            .provider_route_requests
            .iter()
            .find(|r| r.route_id == route_id)
        {
            return Ok(LegionWorkflowCoordinatorOutput::ProviderRouteRequired(
                Box::new(existing.clone()),
            ));
        }
        let route_ref = worker.assisted_ai_route.clone().ok_or_else(|| {
            AgentError::InvalidLegionWorkflow(
                "provider-backed worker missing route metadata".to_string(),
            )
        })?;
        let route_request = provider_route_request_from_worker(&worker, route_ref);
        let metadata = legion_provider_route_metadata_from_worker(&worker, &route_request);
        validate_legion_provider_route_metadata(&metadata).map_err(|e| {
            AgentError::InvalidLegionWorkflow(format!("provider route metadata invalid: {e:?}"))
        })?;
        self.provider_route_requests.push(route_request.clone());
        if self
            .provider_route_metadata
            .iter()
            .all(|m| m.route_id != route_id)
        {
            self.provider_route_metadata.push(metadata.clone());
        }
        Ok(LegionWorkflowCoordinatorOutput::ProviderRouteRequired(
            Box::new(route_request),
        ))
    }

    /// Returns canonical provider route metadata for a provider-backed worker.
    pub fn provider_route_metadata_for_worker(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
    ) -> Result<LegionWorkflowCoordinatorOutput, AgentError> {
        let worker = self.find_worker(worker_id)?.clone();
        if worker.model_backend != LegionWorkflowModelBackend::ProviderBacked {
            return Err(AgentError::InvalidLegionWorkflow(
                "provider route metadata requested for non-provider worker".to_string(),
            ));
        }
        let route_id = format!("legion-route:{}", worker.worker_id.0);
        if let Some(metadata) = self
            .provider_route_metadata
            .iter()
            .find(|m| m.route_id == route_id)
        {
            return Ok(LegionWorkflowCoordinatorOutput::ProviderRouteMetadataReady(
                Box::new(metadata.clone()),
            ));
        }
        let route_ref = worker.assisted_ai_route.clone().ok_or_else(|| {
            AgentError::InvalidLegionWorkflow(
                "provider-backed worker missing route metadata".to_string(),
            )
        })?;
        let route_request = provider_route_request_from_worker(&worker, route_ref);
        let metadata = legion_provider_route_metadata_from_worker(&worker, &route_request);
        validate_legion_provider_route_metadata(&metadata).map_err(|e| {
            AgentError::InvalidLegionWorkflow(format!("provider route metadata invalid: {e:?}"))
        })?;
        self.provider_route_metadata.push(metadata.clone());
        Ok(LegionWorkflowCoordinatorOutput::ProviderRouteMetadataReady(
            Box::new(metadata),
        ))
    }

    /// Builds and validates a canonical task packet for a worker.
    /// Idempotent: repeated calls for the same worker return the stored packet.
    pub fn task_packet_for_worker(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
    ) -> Result<LegionWorkflowCoordinatorOutput, AgentError> {
        let worker = self.find_worker(worker_id)?.clone();
        let packet_id = self.packet_id_for_worker(worker_id);
        if let Some(existing) = self.task_packets.iter().find(|p| p.packet_id == packet_id) {
            return Ok(LegionWorkflowCoordinatorOutput::TaskPacketReady(Box::new(
                existing.clone(),
            )));
        }
        let workspace_id = worker
            .affected_targets
            .iter()
            .find_map(|t| t.workspace_id)
            .ok_or(AgentError::InvalidLegionWorkflow(
                "task packet requires workspace-scoped target".to_string(),
            ))?;
        let objective_summary_hash = FileFingerprint {
            algorithm: "sha256".to_string(),
            value: format!("legion-objective-hash:{}", worker.worker_id.0),
        };
        let allowed_files: Vec<LegionTaskFileScope> = worker
            .affected_targets
            .iter()
            .map(|target| {
                let path = target
                    .workspace_id
                    .map(|_| format!("workspace://{}", target.target_id))
                    .unwrap_or_else(|| format!("metadata://{}", target.target_id));
                LegionTaskFileScope {
                    scope_id: format!("scope:{}", target.target_id),
                    path: CanonicalPath(path),
                    fingerprint: Some(FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: format!("legion-fingerprint:{}", target.target_id),
                    }),
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                }
            })
            .collect();
        let forbidden_files = Vec::new();
        let context_snippet_refs = Vec::new();
        let full_file_refs = Vec::new();
        let command_output_refs = Vec::new();
        let output_contract = LegionTaskOutputContract {
            expected_result_kind: LegionWorkerResultKind::PatchProposal,
            proposal_only: true,
            direct_mutation_allowed: false,
            required_evidence_kinds: vec![
                LegionEvidenceKind::CommandRun,
                LegionEvidenceKind::Review,
            ],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let validation_plan = LegionTaskValidationPlan {
            required_commands: vec!["legion.validate.proposal_only".to_string()],
            success_criteria: vec!["legion.validate.proposal_ready".to_string()],
            stop_conditions: vec!["legion.validate.stop_on_conflict".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let policy = LegionTaskPolicy {
            locality_preference: match worker.model_backend {
                LegionWorkflowModelBackend::Local => {
                    LegionProviderLocalityPreference::LocalPreferred
                }
                LegionWorkflowModelBackend::ProviderBacked => {
                    LegionProviderLocalityPreference::RemoteAllowed
                }
                LegionWorkflowModelBackend::Unavailable => {
                    LegionProviderLocalityPreference::LocalPreferred
                }
            },
            privacy_policy: LegionProviderPrivacyPolicy::MetadataOnly,
            cost_budget_cents: Some(0),
            latency_budget_ms: Some(1000),
            allow_network: false,
            allow_direct_workspace_mutation: false,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let packet = LegionTaskPacket {
            packet_id: packet_id.clone(),
            workspace_id,
            objective_summary_hash,
            allowed_files,
            forbidden_files,
            context_snippet_refs,
            full_file_refs,
            command_output_refs,
            output_contract,
            validation_plan,
            policy,
            correlation_id: worker.correlation_id,
            causality_id: worker.causality_id,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_legion_task_packet(&packet).map_err(|e| {
            AgentError::InvalidLegionWorkflow(format!("task packet invalid: {e:?}"))
        })?;
        self.task_packets.push(packet.clone());
        Ok(LegionWorkflowCoordinatorOutput::TaskPacketReady(Box::new(
            packet,
        )))
    }

    fn packet_id_for_worker(&self, worker_id: &LegionWorkflowWorkerId) -> LegionTaskPacketId {
        LegionTaskPacketId(format!("legion-packet:{}", worker_id.0))
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

        // Fail closed on duplicate recording: the evidence/result ids are derived
        // solely from the worker id, so a second call would mint colliding ids
        // and duplicate worker results.
        let evidence_id = format!("legion-evidence:{}", worker_id.0);
        let result_id = format!("legion-result:{}", worker_id.0);
        if self
            .worker_results
            .iter()
            .any(|result| result.result_id == result_id)
            || self
                .evidence_records
                .iter()
                .any(|record| record.evidence_id == evidence_id)
        {
            return Err(AgentError::InvalidLegionWorkflow(format!(
                "proposal output already recorded for worker {}",
                worker_id.0
            )));
        }

        self.proposal_outputs.push(output.clone());

        let packet_id = self.packet_id_for_worker(worker_id);
        let evidence = LegionEvidenceRecord {
            evidence_id,
            kind: LegionEvidenceKind::CommandRun,
            source: LegionEvidenceSource::LocalCommand,
            payload_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: format!("legion-evidence-hash:{}", worker_id.0),
            },
            redacted_payload_summary: format!("legion evidence for worker {}", worker_id.0),
            command_label: Some("legion.proposal_record".to_string()),
            exit_status: Some(0),
            privacy_scope: LegionEvidencePrivacyScope::WorkspaceMetadata,
            generated_at: output.created_at,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_legion_evidence_record(&evidence).map_err(|e| {
            AgentError::InvalidLegionWorkflow(format!("evidence record invalid: {e:?}"))
        })?;
        self.evidence_records.push(evidence.clone());

        let result = LegionWorkerResult {
            result_id,
            packet_id,
            result_kind: LegionWorkerResultKind::PatchProposal,
            patch_proposal: Some(output.proposal_id),
            documentation_proposal: None,
            analysis_summary: None,
            test_plan_summary: None,
            blocked_reason: None,
            invalid_reason: None,
            evidence_records: vec![evidence],
            provider_route: None,
            correlation_id: output.correlation_id,
            causality_id: output.causality_id,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_legion_worker_result(&result).map_err(|e| {
            AgentError::InvalidLegionWorkflow(format!("worker result invalid: {e:?}"))
        })?;
        self.worker_results.push(result.clone());

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
    has_dependency_path(session, left, right) || has_dependency_path(session, right, left)
}

fn has_dependency_path(
    session: &LegionWorkflowSession,
    start: &LegionWorkflowWorkerId,
    end: &LegionWorkflowWorkerId,
) -> bool {
    if start == end {
        return true;
    }

    let mut stack = vec![start.0.as_str()];
    let mut visited = HashSet::new();
    while let Some(worker_id) = stack.pop() {
        if !visited.insert(worker_id) {
            continue;
        }

        for dependency in &session.dependency_edges {
            if dependency.predecessor_worker_id.0.as_str() != worker_id {
                continue;
            }
            if dependency.successor_worker_id == *end {
                return true;
            }
            stack.push(dependency.successor_worker_id.0.as_str());
        }
    }

    false
}

/// Derives a per-worker cancellation token so provider routes do not share a
/// single audit/cancellation identity. Deterministic for a given
/// `(worker_id, correlation_id)` pair so replays stay stable.
fn derive_cancellation_token(worker_id: &str, correlation_id: u64) -> CancellationTokenId {
    CancellationTokenId(uuid::Uuid::from_u128(stable_hash_128(
        format!("legion.workflow.cancellation\u{1f}{worker_id}\u{1f}{correlation_id}").as_bytes(),
    )))
}

/// Derives a per-worker event sequence so provider-route audit records do not
/// all collapse onto a single sequence value. Deterministic for a given
/// `(worker_id, correlation_id)` pair.
fn derive_event_sequence(worker_id: &str, correlation_id: u64) -> EventSequence {
    EventSequence(stable_hash_128(
        format!("legion.workflow.event_sequence\u{1f}{worker_id}\u{1f}{correlation_id}").as_bytes(),
    ) as u64)
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
        prompt_prefix: String::new(),
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
        cancellation_token: derive_cancellation_token(&worker.worker_id.0, worker.correlation_id.0),
        health_labels: vec!["provider_route.not_invoked".to_string()],
        cost_labels: vec!["cost.metadata_only".to_string()],
        principal_id: PrincipalId("legion.workflow.coordinator".to_string()),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        correlation_id: worker.correlation_id,
        causality_id: worker.causality_id,
        event_sequence: derive_event_sequence(&worker.worker_id.0, worker.correlation_id.0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn legion_provider_route_metadata_from_worker(
    worker: &LegionWorkflowWorkerAssignment,
    route_request: &AssistedAiProviderRouteRequest,
) -> LegionProviderRouteMetadata {
    let locality_preference = match worker.model_backend {
        LegionWorkflowModelBackend::ProviderBacked => {
            LegionProviderLocalityPreference::RemoteAllowed
        }
        _ => LegionProviderLocalityPreference::LocalPreferred,
    };
    let cost_budget_cents = match worker.model_backend {
        LegionWorkflowModelBackend::ProviderBacked => Some(100),
        _ => Some(0),
    };
    let route_health = if route_request
        .health_labels
        .iter()
        .any(|l| l.contains("unavailable"))
    {
        LegionProviderRouteHealth::Unavailable
    } else if route_request
        .health_labels
        .iter()
        .any(|l| l.contains("degraded"))
    {
        LegionProviderRouteHealth::Degraded
    } else {
        LegionProviderRouteHealth::Healthy
    };
    let mut labels = route_request.health_labels.clone();
    labels.extend(route_request.cost_labels.clone());
    labels.push("legion.provider_route.metadata".to_string());
    LegionProviderRouteMetadata {
        route_id: route_request.route_id.clone(),
        locality_preference,
        cost_budget_cents,
        latency_budget_ms: Some(1000),
        privacy_policy: LegionProviderPrivacyPolicy::MetadataOnly,
        model_capability: LegionModelCapability::CodePatch,
        provider_class: route_request.provider_class,
        route_health,
        labels,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        AgentReplayManifest, AssistedAiTrustProjectionKind, CommandRiskLabel,
        ContextManifestItemCount, DelegatedTaskAffectedTargetSummary, DelegatedTaskOperationClass,
        DelegatedTaskPlanId, FileFingerprint, LegionWorkflowDependency, LegionWorkflowDependencyId,
        LegionWorkflowDependencyState, LegionWorkflowMergeApproval,
        LegionWorkflowMergeReadinessBlocker, LegionWorkflowSessionId, LegionWorkflowSignOff,
        LegionWorkflowSignOffId, LegionWorkflowSignOffState, LegionWorkflowState,
        LegionWorkflowVerificationGate, LegionWorkflowVerificationGateId,
        LegionWorkflowVerificationGateState, LegionWorkflowWorkerRole, LegionWorkflowWorkerState,
        PrivacyClassification, ProposalPrivacyLabel, ProposalRiskLabel, ProposalTargetKind,
        RedactionHint, WorkspaceId, validate_legion_workflow_session,
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
        let denied = legion_protocol::delegated_task_tool_permission_request(
            legion_protocol::DelegatedTaskToolPermissionRequestInput {
                request_id: "sandbox:denied".to_string(),
                profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::AccessWorkspaceFiles,
                capability: Some(CapabilityId("delegated.runtime.allocate".to_string())),
                target_id: Some("target/delegated-tasks".to_string()),
                decision: legion_protocol::DelegatedTaskToolPermissionDecision::Deny,
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
    ) -> legion_protocol::DelegatedTaskToolPermissionRequest {
        legion_protocol::delegated_task_tool_permission_request(
            legion_protocol::DelegatedTaskToolPermissionRequestInput {
                request_id: request_id.to_string(),
                profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::AccessWorkspaceFiles,
                capability: Some(CapabilityId("delegated.runtime.allocate".to_string())),
                target_id: Some("target/delegated-tasks".to_string()),
                decision: legion_protocol::DelegatedTaskToolPermissionDecision::Allow,
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
            workspace_id: Some(WorkspaceId(1)),
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
            product_mode: legion_protocol::ProductMode::LegionWorkflows,
            worker_assignments: vec![
                workflow_worker(
                    "worker:local",
                    LegionWorkflowModelBackend::Local,
                    "crates/legion-agent/src/lib.rs",
                ),
                workflow_worker(
                    "worker:provider",
                    LegionWorkflowModelBackend::ProviderBacked,
                    "crates/legion-agent/tests/review.rs",
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
    fn legion_workflow_three_task_dag_with_one_dependency_schedules_two_parallel_workers() {
        let mut session = workflow_session();
        session.worker_assignments = vec![
            workflow_worker(
                "worker:root",
                LegionWorkflowModelBackend::Local,
                "crates/legion-agent/src/root.rs",
            ),
            workflow_worker(
                "worker:left",
                LegionWorkflowModelBackend::Local,
                "crates/legion-agent/src/left.rs",
            ),
            workflow_worker(
                "worker:right",
                LegionWorkflowModelBackend::Local,
                "crates/legion-agent/src/right.rs",
            ),
        ];
        session.dependency_edges = vec![LegionWorkflowDependency {
            dependency_id: LegionWorkflowDependencyId("dependency:root-left".to_string()),
            predecessor_worker_id: LegionWorkflowWorkerId("worker:root".to_string()),
            successor_worker_id: LegionWorkflowWorkerId("worker:left".to_string()),
            state: LegionWorkflowDependencyState::Pending,
            label: "root before left".to_string(),
            schema_version: 1,
        }];

        let coordinator = LegionWorkflowCoordinator::new(session).expect("valid workflow");
        let ready = coordinator.next_ready_workers();

        assert_eq!(ready.len(), 2);
        let ready_ids = ready
            .into_iter()
            .map(|worker| worker.worker_id.0)
            .collect::<Vec<_>>();
        assert!(ready_ids.contains(&"worker:root".to_string()));
        assert!(ready_ids.contains(&"worker:right".to_string()));
    }

    #[test]
    fn legion_workflow_resume_uses_persisted_worker_and_dependency_state() {
        let mut session = workflow_session();
        session.worker_assignments = vec![
            workflow_worker(
                "worker:root",
                LegionWorkflowModelBackend::Local,
                "crates/legion-agent/src/root.rs",
            ),
            workflow_worker(
                "worker:child",
                LegionWorkflowModelBackend::Local,
                "crates/legion-agent/src/child.rs",
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
    fn legion_workflow_provider_worker_route_is_idempotent() {
        let mut coordinator =
            LegionWorkflowCoordinator::new(workflow_session()).expect("valid workflow");
        let worker_id = LegionWorkflowWorkerId("worker:provider".to_string());

        let first = coordinator
            .provider_route_for_worker(&worker_id)
            .expect("first provider route metadata");
        let second = coordinator
            .provider_route_for_worker(&worker_id)
            .expect("second provider route metadata");

        assert!(matches!(
            first,
            LegionWorkflowCoordinatorOutput::ProviderRouteRequired(_)
        ));
        assert!(matches!(
            second,
            LegionWorkflowCoordinatorOutput::ProviderRouteRequired(_)
        ));
        assert_eq!(coordinator.provider_route_requests().len(), 1);
        assert_eq!(coordinator.provider_route_metadata().len(), 1);

        let first_meta = coordinator
            .provider_route_metadata_for_worker(&worker_id)
            .expect("first metadata");
        let second_meta = coordinator
            .provider_route_metadata_for_worker(&worker_id)
            .expect("second metadata");
        assert!(matches!(
            first_meta,
            LegionWorkflowCoordinatorOutput::ProviderRouteMetadataReady(_)
        ));
        assert!(matches!(
            second_meta,
            LegionWorkflowCoordinatorOutput::ProviderRouteMetadataReady(_)
        ));
        assert_eq!(coordinator.provider_route_metadata().len(), 1);
    }

    #[test]
    fn legion_workflow_task_packet_is_idempotent() {
        let mut coordinator =
            LegionWorkflowCoordinator::new(workflow_session()).expect("valid workflow");
        let worker_id = LegionWorkflowWorkerId("worker:local".to_string());

        let first = coordinator
            .task_packet_for_worker(&worker_id)
            .expect("first task packet");
        let second = coordinator
            .task_packet_for_worker(&worker_id)
            .expect("second task packet");

        assert!(matches!(
            first,
            LegionWorkflowCoordinatorOutput::TaskPacketReady(ref packet)
            if packet.packet_id.0 == "legion-packet:worker:local"
        ));
        assert!(matches!(
            second,
            LegionWorkflowCoordinatorOutput::TaskPacketReady(ref packet)
            if packet.packet_id.0 == "legion-packet:worker:local"
        ));
        assert_eq!(coordinator.task_packets().len(), 1);
    }

    #[test]
    fn legion_workflow_task_packet_without_workspace_fails_closed() {
        let mut session = workflow_session();
        session.worker_assignments[0].affected_targets[0].workspace_id = None;

        let mut coordinator = LegionWorkflowCoordinator::new(session).expect("valid workflow");
        let worker_id = LegionWorkflowWorkerId("worker:local".to_string());

        let error = coordinator
            .task_packet_for_worker(&worker_id)
            .expect_err("must fail without workspace-scoped target");

        assert_eq!(
            error,
            AgentError::InvalidLegionWorkflow(
                "task packet requires workspace-scoped target".to_string()
            )
        );
        assert!(coordinator.task_packets().is_empty());
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
    fn legion_workflow_same_target_transitive_dependency_is_ordered() {
        let mut session = workflow_session();
        session.worker_assignments = vec![
            workflow_worker(
                "worker:root",
                LegionWorkflowModelBackend::Local,
                "crates/legion-agent/src/shared.rs",
            ),
            workflow_worker(
                "worker:middle",
                LegionWorkflowModelBackend::Local,
                "crates/legion-agent/src/intermediate.rs",
            ),
            workflow_worker(
                "worker:leaf",
                LegionWorkflowModelBackend::Local,
                "crates/legion-agent/src/shared.rs",
            ),
        ];
        session.dependency_edges = vec![
            LegionWorkflowDependency {
                dependency_id: LegionWorkflowDependencyId("dependency:root-middle".to_string()),
                predecessor_worker_id: LegionWorkflowWorkerId("worker:root".to_string()),
                successor_worker_id: LegionWorkflowWorkerId("worker:middle".to_string()),
                state: LegionWorkflowDependencyState::Pending,
                label: "root before middle".to_string(),
                schema_version: 1,
            },
            LegionWorkflowDependency {
                dependency_id: LegionWorkflowDependencyId("dependency:middle-leaf".to_string()),
                predecessor_worker_id: LegionWorkflowWorkerId("worker:middle".to_string()),
                successor_worker_id: LegionWorkflowWorkerId("worker:leaf".to_string()),
                state: LegionWorkflowDependencyState::Pending,
                label: "middle before leaf".to_string(),
                schema_version: 1,
            },
        ];

        validate_legion_workflow_session(&session).expect("session shape valid");
        let coordinator = LegionWorkflowCoordinator::new(session).expect("coordinator starts");

        assert_eq!(coordinator.conflicts(), &[]);
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
    fn proposal_populates_concurrency_guard_when_target_exists() {
        let sandbox = PathBuf::from("target/legion-agent-precondition-test");
        std::fs::create_dir_all(&sandbox).expect("create sandbox");
        let target = sandbox.join("existing.txt");
        std::fs::write(&target, b"existing base content").expect("write base file");

        let generator = DelegatedTaskProposalGenerator::new(sandbox.clone());
        let proposal = generator
            .generate_proposal(proposal_input(&target, "updated content"))
            .expect("proposal output");

        assert_eq!(proposal.preconditions.expected_file_length, Some(21));
        assert!(proposal.preconditions.expected_fingerprint.is_some());
        assert!(proposal.preconditions.expected_modified_at.is_some());

        // A non-existent target is a genuine create and carries no guard.
        let missing = sandbox.join("missing.txt");
        let create = generator
            .generate_proposal(proposal_input(&missing, "new content"))
            .expect("create proposal");
        assert_eq!(create.preconditions.expected_file_length, None);
        assert_eq!(create.preconditions.expected_fingerprint, None);

        std::fs::remove_dir_all(sandbox).expect("cleanup sandbox");
    }

    #[test]
    fn proposal_emits_create_file_when_base_absent() {
        let sandbox = PathBuf::from("target/legion-agent-create-when-absent-test");
        std::fs::create_dir_all(&sandbox).expect("create sandbox");
        let missing = sandbox.join("brand-new.rs");

        let generator = DelegatedTaskProposalGenerator::new(sandbox.clone());
        let proposal = generator
            .generate_proposal(proposal_input(&missing, "fn fresh() {}\n"))
            .expect("create proposal");

        match &proposal.payload {
            ProposalPayload::CreateFile(create_file) => {
                assert_eq!(create_file.path.0, "brand-new.rs");
                assert_eq!(
                    create_file.initial_content.as_deref(),
                    Some("fn fresh() {}\n")
                );
            }
            other => panic!("expected create-file proposal, got {other:?}"),
        }
        // A genuine create carries no concurrency guard.
        assert_eq!(proposal.preconditions.expected_file_length, None);
        assert_eq!(proposal.preconditions.expected_fingerprint, None);
        assert_eq!(proposal.preconditions.expected_modified_at, None);
        assert_eq!(proposal.preconditions.file_content_version, None);
        assert_eq!(proposal.preview.summary, "Create file proposal");

        std::fs::remove_dir_all(sandbox).expect("cleanup sandbox");
    }

    #[test]
    fn proposal_emits_guarded_create_when_base_exists() {
        let sandbox = PathBuf::from("target/legion-agent-edit-when-present-test");
        std::fs::create_dir_all(&sandbox).expect("create sandbox");
        let target = sandbox.join("existing.rs");
        let base = "hello world\n";
        std::fs::write(&target, base.as_bytes()).expect("write base file");

        let generator = DelegatedTaskProposalGenerator::new(sandbox.clone());
        let modified = "hello brave world\n";
        let proposal = generator
            .generate_proposal(proposal_input(&target, modified))
            .expect("create proposal");

        // An existing base produces a path-based create/overwrite carrying the
        // full modified content (the generator has no workspace FileId/open
        // buffer, so an appliable TextEdit cannot be addressed here).
        match &proposal.payload {
            ProposalPayload::CreateFile(create_file) => {
                assert_eq!(create_file.path.0, "existing.rs");
                assert_eq!(create_file.initial_content.as_deref(), Some(modified));
            }
            other => panic!("expected create-file proposal, got {other:?}"),
        }

        // Content-level concurrency guard populated from the base snapshot.
        assert_eq!(
            proposal.preconditions.expected_file_length,
            Some(base.len() as u64)
        );
        assert!(proposal.preconditions.expected_fingerprint.is_some());
        assert!(proposal.preconditions.expected_modified_at.is_some());
        assert!(proposal.preconditions.file_content_version.is_some());
        assert_eq!(proposal.preview.summary, "Create file proposal");

        std::fs::remove_dir_all(sandbox).expect("cleanup sandbox");
    }

    #[test]
    fn legion_workflow_duplicate_proposal_record_fails_closed() {
        let mut coordinator =
            LegionWorkflowCoordinator::new(workflow_session()).expect("valid workflow");
        let sandbox = PathBuf::from("target/legion-agent-duplicate-record-test");
        std::fs::create_dir_all(&sandbox).expect("create sandbox");
        let generator = DelegatedTaskProposalGenerator::new(sandbox.clone());
        let target = sandbox.join("dup.txt");
        let worker = LegionWorkflowWorkerId("worker:local".to_string());

        let first = generator
            .generate_proposal(proposal_input(&target, "first"))
            .expect("first proposal");
        coordinator
            .record_proposal_output(&worker, first)
            .expect("first record");

        let second = generator
            .generate_proposal(proposal_input(&target, "second"))
            .expect("second proposal");
        let error = coordinator
            .record_proposal_output(&worker, second)
            .expect_err("duplicate recording must fail closed");

        assert!(matches!(error, AgentError::InvalidLegionWorkflow(_)));
        assert_eq!(coordinator.worker_results().len(), 1);
        assert_eq!(coordinator.evidence_records().len(), 1);

        std::fs::remove_dir_all(sandbox).expect("cleanup sandbox");
    }

    #[test]
    fn legion_workflow_provider_routes_have_distinct_audit_identity() {
        let mut session = workflow_session();
        session.dependency_edges.clear();
        session.worker_assignments = vec![
            workflow_worker(
                "worker:provider-a",
                LegionWorkflowModelBackend::ProviderBacked,
                "crates/legion-agent/src/a.rs",
            ),
            workflow_worker(
                "worker:provider-b",
                LegionWorkflowModelBackend::ProviderBacked,
                "crates/legion-agent/src/b.rs",
            ),
        ];

        let mut coordinator = LegionWorkflowCoordinator::new(session).expect("valid workflow");

        let route_a = match coordinator
            .provider_route_for_worker(&LegionWorkflowWorkerId("worker:provider-a".to_string()))
            .expect("route a")
        {
            LegionWorkflowCoordinatorOutput::ProviderRouteRequired(route) => *route,
            _ => panic!("expected provider route"),
        };
        let route_b = match coordinator
            .provider_route_for_worker(&LegionWorkflowWorkerId("worker:provider-b".to_string()))
            .expect("route b")
        {
            LegionWorkflowCoordinatorOutput::ProviderRouteRequired(route) => *route,
            _ => panic!("expected provider route"),
        };

        assert_ne!(route_a.cancellation_token, route_b.cancellation_token);
        assert_ne!(route_a.event_sequence, route_b.event_sequence);
        assert_ne!(route_a.cancellation_token.0, uuid::Uuid::from_u128(13));
        assert_ne!(route_a.event_sequence.0, 13);
    }

    #[test]
    fn legion_workflow_agent_crate_does_not_import_app_ui_or_desktop() {
        let source = include_str!("lib.rs");
        let forbidden_imports = [
            ["legion", "_app"].concat(),
            ["legion", "_ui"].concat(),
            ["legion", "_desktop"].concat(),
            ["legion", "_editor"].concat(),
            ["legion", "_project"].concat(),
        ];

        for forbidden in forbidden_imports {
            assert!(
                !source.contains(&forbidden),
                "unexpected forbidden import {forbidden}"
            );
        }
    }
}
