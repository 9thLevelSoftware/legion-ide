//! Offline replacements for app-local AI and agent runtime edges.
//!
//! This module is compiled when `legion-app` is built without the `ai` feature.
//! It keeps protocol projections and proposal-mediated workflows available while
//! avoiding any dependency on `legion-ai`, `legion-ai-providers`, or `legion-agent`.

use legion_protocol::{
    AgentRunId, AgentRunState, AgentStateTransitionRecord, AssistedAiEditProposalOutput,
    AssistedAiOperationClass, AssistedAiProposalTargetIntent, AssistedAiProviderClass,
    AssistedAiProviderInvocationState, AssistedAiProviderRouteRequest,
    AssistedAiProviderRouteResponse, AssistedAiRefusalMetadata, AssistedAiRequestDisposition,
    AssistedAiRouteDecision, AssistedAiTrustProjectionReference, CancellationTokenId,
    CanonicalPath, CapabilityBrokerPort, CapabilityId, CausalityId, CorrelationId,
    DelegatedTaskToolPermissionProfile, DelegatedTaskToolPermissionRequest, EventSequence,
    FileFingerprint, LegionEvidenceKind, LegionEvidencePrivacyScope, LegionEvidenceRecord,
    LegionEvidenceSource, LegionModelCapability, LegionProviderLocalityPreference,
    LegionProviderPrivacyPolicy, LegionProviderRouteHealth, LegionProviderRouteMetadata,
    LegionTaskFileScope, LegionTaskOutputContract, LegionTaskPacket, LegionTaskPacketId,
    LegionTaskPolicy, LegionTaskValidationPlan, LegionWorkerResult, LegionWorkerResultKind,
    LegionWorkflowConflict, LegionWorkflowConflictId, LegionWorkflowConflictKind,
    LegionWorkflowConflictState, LegionWorkflowDependencyState, LegionWorkflowMergeReadiness,
    LegionWorkflowModelBackend, LegionWorkflowSession, LegionWorkflowWorkerAssignment,
    LegionWorkflowWorkerId, LegionWorkflowWorkerState, PermissionBudgetActionClass, PreviewSummary,
    PrincipalId, ProposalAffectedTarget, ProposalId, ProposalPayload, ProposalPayloadKind,
    ProposalPrivacyLabel, ProposalRiskLabel, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProposalVersionPreconditions, RedactionHint, TimestampMillis, WorkspaceTrustState,
    evaluate_legion_workflow_merge_readiness, validate_legion_evidence_record,
    validate_legion_provider_route_metadata, validate_legion_task_packet,
    validate_legion_worker_result, validate_legion_workflow_session,
    validate_phase4_runtime_audit_record,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// Deterministic provider id used by existing app projections in offline builds.
pub const DETERMINISTIC_LOCAL_PROVIDER_ID: &str = "offline-ai-disabled";

/// Offline provider registry placeholder.
#[derive(Debug, Default)]
pub struct ProviderRegistry;

/// Create an empty offline provider registry.
pub fn make_stub_registry() -> ProviderRegistry {
    ProviderRegistry
}

/// Error type for offline AI replacements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfflineAiError {
    /// Requested agent transition is not legal.
    IllegalTransition {
        /// Current state.
        from: AgentRunState,
        /// Requested state.
        to: AgentRunState,
    },
    /// Metadata validation failed.
    InvalidMetadata(String),
    /// Legion workflow metadata was invalid.
    InvalidLegionWorkflow(String),
    /// Legion workflow worker was unknown.
    UnknownLegionWorkflowWorker(String),
    /// Legion workflow worker was completed more than once.
    LegionWorkflowWorkerAlreadyCompleted(String),
    /// Legion workflow dependency graph contains a cycle.
    LegionWorkflowDependencyCycle,
    /// Offline provider routing is disabled.
    ProviderDisabled(String),
}

impl std::fmt::Display for OfflineAiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IllegalTransition { from, to } => {
                write!(
                    formatter,
                    "illegal agent transition from {from:?} to {to:?}"
                )
            }
            Self::InvalidMetadata(message) => {
                write!(formatter, "invalid agent metadata: {message}")
            }
            Self::InvalidLegionWorkflow(message) => {
                write!(formatter, "invalid legion workflow metadata: {message}")
            }
            Self::UnknownLegionWorkflowWorker(worker_id) => {
                write!(formatter, "unknown legion workflow worker: {worker_id}")
            }
            Self::LegionWorkflowWorkerAlreadyCompleted(worker_id) => {
                write!(
                    formatter,
                    "legion workflow worker already completed: {worker_id}"
                )
            }
            Self::LegionWorkflowDependencyCycle => {
                write!(formatter, "legion workflow dependency cycle detected")
            }
            Self::ProviderDisabled(reason) => {
                write!(formatter, "offline provider disabled: {reason}")
            }
        }
    }
}

impl std::error::Error for OfflineAiError {}

/// Metadata-only output produced by offline agent state transitions.
#[derive(Debug, Clone)]
pub enum AgentRuntimeOutput {
    /// State transition metadata for tracker/storage owned by composition.
    Transition(AgentStateTransitionRecord),
}

/// Deterministic agent state machine used by the offline app build.
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

    /// Returns recorded metadata-only transitions.
    pub fn transitions(&self) -> &[AgentStateTransitionRecord] {
        &self.transitions
    }

    /// Applies a legal transition and records metadata for replay.
    pub fn transition(
        &mut self,
        to_state: AgentRunState,
        reason_code: impl Into<String>,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        event_sequence: EventSequence,
    ) -> Result<AgentRuntimeOutput, OfflineAiError> {
        if !legal_transition(self.state, to_state) {
            return Err(OfflineAiError::IllegalTransition {
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
            audit_id: format!("offline-agent:{}:{}", self.run_id.0, event_sequence.0),
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
        })
        .map_err(|error| OfflineAiError::InvalidMetadata(error.to_string()))?;
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

/// Orchestrator for deterministic delegated-task sandboxes in offline builds.
#[derive(Debug, Clone)]
pub struct DelegatedTaskSandboxOrchestrator {
    sandbox_path: PathBuf,
    is_worktree: bool,
    /// Exclusive lock handle over the sandbox's `.lock` lease file. `Arc`
    /// keeps this orchestrator `Clone`: every clone shares the same lease,
    /// so the lease outlives any single clone and is only released when the
    /// last clone drops it (or `cleanup` clears it explicitly). Best-effort:
    /// `None` if the lease could not be acquired (see `initialize`).
    lease: Option<Arc<std::fs::File>>,
}

impl DelegatedTaskSandboxOrchestrator {
    /// Creates a new orchestrator under `target/delegated-tasks`.
    pub fn new(run_id: &str) -> Self {
        let sandbox_path = PathBuf::from("target/delegated-tasks").join(format!("task-{run_id}"));
        Self {
            sandbox_path,
            is_worktree: false,
            lease: None,
        }
    }

    /// Returns the sandbox path.
    pub fn sandbox_path(&self) -> &Path {
        &self.sandbox_path
    }

    /// Returns the sibling lease file path for this sandbox
    /// (`task-<run_id>.lock` next to `task-<run_id>/`).
    fn lease_path(&self) -> PathBuf {
        lease_path_for_sandbox(&self.sandbox_path)
    }

    /// Initializes the sandbox, preferring a git worktree and falling back to a directory.
    pub fn initialize(
        &mut self,
        permission: &DelegatedTaskToolPermissionRequest,
    ) -> Result<(), std::io::Error> {
        validate_sandbox_permission(permission, "initialize")?;
        if let Some(parent) = self.sandbox_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                self.sandbox_path.to_str().unwrap_or_default(),
                "HEAD",
            ])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                self.is_worktree = true;
            }
            _ => {
                self.is_worktree = false;
                std::fs::create_dir_all(&self.sandbox_path)?;
            }
        }

        // Acquire a best-effort exclusive lease on the sandbox's sibling
        // `.lock` file so a cross-process `reap_orphaned_sandboxes` call
        // can distinguish this live sandbox from an abandoned one. Lease
        // acquisition is not a correctness gate: if it fails for any
        // reason (permissions, unsupported filesystem, contention on an
        // extremely unlikely stale lock), the sandbox still initializes
        // successfully without protection.
        if let Ok(lock_file) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.lease_path())
            && lock_file.try_lock().is_ok()
        {
            self.lease = Some(Arc::new(lock_file));
        }

        Ok(())
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
        // Drop the lease before attempting to remove the lock file: on
        // Windows an open (even unlocked) file handle can prevent deletion.
        // Best-effort — another clone of this orchestrator may still hold
        // the lease, in which case the removal simply fails and is ignored.
        self.lease = None;
        let _ = std::fs::remove_file(self.lease_path());
        Ok(())
    }
}

/// Returns the sibling lease file path for a `task-<run_id>` sandbox
/// directory: `task-<run_id>.lock` next to it.
fn lease_path_for_sandbox(sandbox_path: &Path) -> PathBuf {
    let mut lease_path = sandbox_path.to_path_buf();
    let file_name = sandbox_path
        .file_name()
        .map(|name| {
            let mut name = name.to_os_string();
            name.push(".lock");
            name
        })
        .unwrap_or_else(|| std::ffi::OsString::from("task.lock"));
    lease_path.set_file_name(file_name);
    lease_path
}

/// NOTE: `crates/legion-agent/src/lib.rs::reap_orphaned_sandboxes` mirrors
/// this logic for `ai`-feature builds — apply any change to both.
///
/// Removes orphaned sandbox directories under `delegated_tasks_root`.
///
/// Mirrors `legion_agent::reap_orphaned_sandboxes` for offline builds, which
/// have no dependency on `legion-agent`. A directory is an orphan when its
/// name starts with `task-` and its run-id suffix is not in
/// `active_run_ids`. Attempts `git worktree remove --force` first and falls
/// back to plain directory removal. Returns the paths that were removed. A
/// missing root is a successful no-op.
///
/// Lock-file lease protocol: each sandbox may have a sibling
/// `task-<run_id>.lock` file (see `DelegatedTaskSandboxOrchestrator`). A
/// sandbox is only reaped if its lease file is absent (legacy sandbox from
/// before this protocol existed, or one whose owner already released the
/// lease) or its lease can be acquired here (owner is gone). If the lease
/// is currently held elsewhere, `try_lock` fails and the reaper treats the
/// owner as alive and skips that sandbox entirely — fail-closed toward NOT
/// deleting. This makes the empty `active_run_ids` list used at startup
/// safe even when another process instance has live sandboxes under the
/// same relative root. Stale `.lock` files whose sandbox directory no
/// longer exists are removed as housekeeping when they can be locked.
pub fn reap_orphaned_sandboxes(
    delegated_tasks_root: &Path,
    active_run_ids: &[&str],
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
        if active_run_ids.contains(&run_id) {
            continue;
        }
        let path = entry.path();
        let lease_path = lease_path_for_sandbox(&path);
        if !sandbox_lease_is_reapable(&lease_path) {
            // Owner process is alive and holding the lease: skip, do not delete.
            continue;
        }
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
        let _ = std::fs::remove_file(&lease_path);
        removed.push(path);
    }
    remove_stale_lease_files(delegated_tasks_root)?;
    Ok(removed)
}

/// Returns `true` when a sandbox's sibling `.lock` file does not currently
/// protect a live owner: either the file does not exist (legacy sandbox, or
/// no lease was ever acquired), or it exists and this call can acquire an
/// exclusive lock on it (the previous owner released it or is gone). Any
/// `try_lock` error is treated as "still held elsewhere" and returns
/// `false`, fail-closed toward not deleting the sandbox.
fn sandbox_lease_is_reapable(lease_path: &Path) -> bool {
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lease_path)
    {
        Ok(lock_file) => lock_file.try_lock().is_ok(),
        Err(_) => true,
    }
}

/// Housekeeping: removes `task-<id>.lock` files whose corresponding
/// `task-<id>` sandbox directory no longer exists, when the lock can be
/// acquired (i.e. is not held by a live process). This can happen if a
/// sandbox directory was removed by means other than `cleanup`/`reap`.
fn remove_stale_lease_files(delegated_tasks_root: &Path) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(delegated_tasks_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".lock") else {
            continue;
        };
        if !stem.starts_with("task-") {
            continue;
        }
        if delegated_tasks_root.join(stem).exists() {
            continue;
        }
        if sandbox_lease_is_reapable(&path) {
            let _ = std::fs::remove_file(&path);
        }
    }
    Ok(())
}

fn validate_sandbox_permission(
    permission: &DelegatedTaskToolPermissionRequest,
    operation: &str,
) -> Result<(), std::io::Error> {
    let write_profile = permission.profile == DelegatedTaskToolPermissionProfile::Write;
    let runtime_action = matches!(
        permission.action_class,
        PermissionBudgetActionClass::AccessWorkspaceFiles
            | PermissionBudgetActionClass::InvokeLocalTool
    );
    let capability_ok = permission
        .capability
        .as_ref()
        .is_some_and(|capability| capability.0 == "delegated.runtime.allocate");
    if write_profile
        && runtime_action
        && capability_ok
        && permission.runtime_allowed
        && permission.human_approval_recorded
        && !permission.deny_overrides
    {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!("delegated sandbox {operation} requires approved Write permission"),
    ))
}

/// Proposal generator inside the offline app build.
#[derive(Debug, Clone)]
pub struct DelegatedTaskProposalGenerator {
    sandbox_base: PathBuf,
}

/// Request-scoped inputs for offline delegated-task proposal generation.
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

    /// Builds a proposal metadata record after sandbox containment validation.
    pub fn generate_proposal(
        &self,
        input: DelegatedTaskProposalInput<'_>,
    ) -> Result<AssistedAiEditProposalOutput, OfflineAiError> {
        validate_containment(&self.sandbox_base, input.target_path)?;

        let target_relative = input
            .target_path
            .strip_prefix(&self.sandbox_base)
            .unwrap_or(input.target_path);

        Ok(AssistedAiEditProposalOutput {
            output_id: input.output_id,
            request_id: input.request_id,
            provider_id: input.provider_id,
            proposal_id: input.proposal_id,
            principal: input.principal,
            capability: input.capability,
            correlation_id: input.correlation_id,
            causality_id: input.causality_id,
            payload: ProposalPayload::CreateFile(legion_protocol::CreateFileProposal {
                path: CanonicalPath(target_relative.to_str().unwrap_or_default().to_string()),
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
                summary: "Offline delegated task proposal".to_string(),
                details: vec!["AI crates are excluded from this build".to_string()],
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

fn validate_containment(base: &Path, path: &Path) -> Result<(), OfflineAiError> {
    let current_dir = || {
        std::env::current_dir().map_err(|error| {
            OfflineAiError::InvalidMetadata(format!(
                "current directory unavailable for containment check: {error}"
            ))
        })
    };

    let base_absolute = match std::fs::canonicalize(base) {
        Ok(canonical) => canonical,
        Err(_) => current_dir()?.join(base),
    };

    let path_absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_dir()?.join(path)
    };

    let mut clean_components = Vec::new();
    for component in path_absolute.components() {
        match component {
            std::path::Component::ParentDir => {
                clean_components.pop();
            }
            std::path::Component::CurDir => {}
            component => clean_components.push(component),
        }
    }

    let clean_path: PathBuf = clean_components.into_iter().collect();

    // Resolve symlinks by canonicalizing the longest existing ancestor and
    // re-appending the not-yet-existent tail (which cannot contain symlinks).
    // This prevents a symlink placed inside the sandbox from escaping it,
    // which a purely lexical normalization would miss.
    let resolved_path = canonicalize_existing_prefix(&clean_path).unwrap_or(clean_path);

    let clean_stripped = strip_windows_unc_prefix(&resolved_path);
    let base_stripped = strip_windows_unc_prefix(&base_absolute);

    if !clean_stripped.starts_with(&base_stripped) {
        return Err(OfflineAiError::InvalidMetadata(
            "path traversal escaped sandbox".to_string(),
        ));
    }
    Ok(())
}

/// Canonicalize the longest existing ancestor of `path` and re-append the
/// remaining (non-existent) components. Returns `None` only when no ancestor
/// can be canonicalized. The re-appended tail cannot contain symlinks because
/// those path elements do not exist on disk yet.
fn canonicalize_existing_prefix(path: &Path) -> Option<PathBuf> {
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = path;
    loop {
        if let Ok(canonical) = std::fs::canonicalize(cursor) {
            let mut resolved = canonical;
            for component in tail.iter().rev() {
                resolved.push(component);
            }
            return Some(resolved);
        }
        match (cursor.parent(), cursor.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                cursor = parent;
            }
            _ => return None,
        }
    }
}

fn strip_windows_unc_prefix(path: &Path) -> PathBuf {
    let path_string = path.to_str().unwrap_or_default();
    if let Some(stripped) = path_string.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
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
    pub fn new(session: LegionWorkflowSession) -> Result<Self, OfflineAiError> {
        validate_legion_workflow_session(&session)
            .map_err(|error| OfflineAiError::InvalidLegionWorkflow(error.message))?;
        if has_dependency_cycle(&session) {
            return Err(OfflineAiError::LegionWorkflowDependencyCycle);
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
    ) -> Result<(), OfflineAiError> {
        self.find_worker(worker_id)?;
        if self.completed_worker_ids.contains(worker_id) {
            return Err(OfflineAiError::LegionWorkflowWorkerAlreadyCompleted(
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
    ) -> Result<LegionWorkflowCoordinatorOutput, OfflineAiError> {
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
    ) -> Result<LegionWorkflowCoordinatorOutput, OfflineAiError> {
        let worker = self.find_worker(worker_id)?.clone();
        if worker.model_backend != LegionWorkflowModelBackend::ProviderBacked {
            return Err(OfflineAiError::InvalidLegionWorkflow(
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
            OfflineAiError::InvalidLegionWorkflow(
                "provider-backed worker missing route metadata".to_string(),
            )
        })?;
        let route_request = provider_route_request_from_worker(&worker, route_ref);
        let metadata = legion_provider_route_metadata_from_worker(&worker, &route_request);
        validate_legion_provider_route_metadata(&metadata).map_err(|e| {
            OfflineAiError::InvalidLegionWorkflow(format!("provider route metadata invalid: {e:?}"))
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
    ) -> Result<LegionWorkflowCoordinatorOutput, OfflineAiError> {
        let worker = self.find_worker(worker_id)?.clone();
        if worker.model_backend != LegionWorkflowModelBackend::ProviderBacked {
            return Err(OfflineAiError::InvalidLegionWorkflow(
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
            OfflineAiError::InvalidLegionWorkflow(
                "provider-backed worker missing route metadata".to_string(),
            )
        })?;
        let route_request = provider_route_request_from_worker(&worker, route_ref);
        let metadata = legion_provider_route_metadata_from_worker(&worker, &route_request);
        validate_legion_provider_route_metadata(&metadata).map_err(|e| {
            OfflineAiError::InvalidLegionWorkflow(format!("provider route metadata invalid: {e:?}"))
        })?;
        self.provider_route_metadata.push(metadata.clone());
        Ok(LegionWorkflowCoordinatorOutput::ProviderRouteMetadataReady(
            Box::new(metadata),
        ))
    }

    /// Derives the canonical packet ID for a given worker.
    pub fn packet_id_for_worker(&self, worker_id: &LegionWorkflowWorkerId) -> LegionTaskPacketId {
        LegionTaskPacketId(format!("legion-packet:{}", worker_id.0))
    }

    /// Builds and validates a canonical task packet for a worker.
    /// Idempotent: repeated calls for the same worker return the stored packet.
    pub fn task_packet_for_worker(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
    ) -> Result<LegionWorkflowCoordinatorOutput, OfflineAiError> {
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
            .ok_or(OfflineAiError::InvalidLegionWorkflow(
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
            OfflineAiError::InvalidLegionWorkflow(format!("task packet invalid: {e:?}"))
        })?;
        self.task_packets.push(packet.clone());
        Ok(LegionWorkflowCoordinatorOutput::TaskPacketReady(Box::new(
            packet,
        )))
    }

    /// Records proposal-only worker output without applying it.
    pub fn record_proposal_output(
        &mut self,
        worker_id: &LegionWorkflowWorkerId,
        output: AssistedAiEditProposalOutput,
    ) -> Result<LegionWorkflowCoordinatorOutput, OfflineAiError> {
        self.find_worker(worker_id)?;
        if output.correlation_id.0 == 0 || output.causality_id.0.is_nil() {
            return Err(OfflineAiError::InvalidLegionWorkflow(
                "proposal output requires non-zero correlation and non-nil causality".to_string(),
            ));
        }
        if output.redaction_hints.contains(&RedactionHint::None) {
            return Err(OfflineAiError::InvalidLegionWorkflow(
                "proposal output must remain metadata-redacted".to_string(),
            ));
        }
        self.proposal_outputs.push(output.clone());

        let evidence = LegionEvidenceRecord {
            evidence_id: format!("legion-evidence:{}", worker_id.0),
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
            OfflineAiError::InvalidLegionWorkflow(format!("evidence record invalid: {e:?}"))
        })?;
        self.evidence_records.push(evidence.clone());

        let packet_id = self.packet_id_for_worker(worker_id);
        let result = LegionWorkerResult {
            result_id: format!("legion-result:{}", worker_id.0),
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
            OfflineAiError::InvalidLegionWorkflow(format!("worker result invalid: {e:?}"))
        })?;
        self.worker_results.push(result);

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
    ) -> Result<&LegionWorkflowWorkerAssignment, OfflineAiError> {
        self.session
            .worker_assignments
            .iter()
            .find(|worker| &worker.worker_id == worker_id)
            .ok_or_else(|| OfflineAiError::UnknownLegionWorkflowWorker(worker_id.0.clone()))
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

/// Offline provider router that always refuses provider invocation metadata.
pub struct ProviderRouter<'a> {
    _registry: &'a ProviderRegistry,
    _capability_broker: &'a dyn CapabilityBrokerPort,
}

impl<'a> ProviderRouter<'a> {
    /// Creates a provider router over the offline registry and capability broker.
    pub fn new(
        registry: &'a ProviderRegistry,
        capability_broker: &'a dyn CapabilityBrokerPort,
    ) -> Self {
        Self {
            _registry: registry,
            _capability_broker: capability_broker,
        }
    }

    /// Refuses completion routing because the offline build excludes AI providers.
    pub fn route_completion(
        &self,
        request: AssistedAiProviderRouteRequest,
    ) -> Result<AssistedAiProviderRouteResponse, OfflineAiError> {
        Ok(refused_response(
            &request,
            "offline.ai_feature_disabled",
            "AI provider routing is excluded from this build",
        ))
    }
}

fn refused_response(
    request: &AssistedAiProviderRouteRequest,
    reason_code: &str,
    label: &str,
) -> AssistedAiProviderRouteResponse {
    let refusal = AssistedAiRefusalMetadata {
        reason_code: reason_code.to_string(),
        label: label.to_string(),
        provider_id: Some(request.provider_id.clone()),
        operation_class: Some(request.operation_class),
        privacy_scope: None,
        capability: Some(request.required_capability.clone()),
        budget_id: None,
        risk_label: ProposalRiskLabel::High,
        reasons: vec![reason_code.to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: request.schema_version,
    };
    AssistedAiProviderRouteResponse {
        route_id: request.route_id.clone(),
        invocation_state: AssistedAiProviderInvocationState::Refused,
        route_decision: AssistedAiRouteDecision {
            disposition: AssistedAiRequestDisposition::Refused,
            provider_invocation: AssistedAiProviderInvocationState::Refused,
            refusal: Some(refusal.clone()),
            reasons: vec![reason_code.to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: request.schema_version,
        },
        provider_id: request.provider_id.clone(),
        model_label: request.model_label.clone(),
        output_labels: vec!["output.not_encoded".to_string()],
        refusal: Some(refusal),
        correlation_id: request.correlation_id,
        causality_id: request.causality_id,
        event_sequence: request.event_sequence,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: request.schema_version,
    }
}

#[cfg(test)]
fn trust_reference(
    reference_id: &str,
    kind: legion_protocol::AssistedAiTrustProjectionKind,
) -> AssistedAiTrustProjectionReference {
    AssistedAiTrustProjectionReference {
        reference_id: reference_id.to_string(),
        kind,
        projection_hash: legion_protocol::FileFingerprint {
            algorithm: "sha256".to_string(),
            value: reference_id.to_string(),
        },
        schema_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{AssistedAiTrustProjectionKind, WorkspaceId};
    use legion_security::DenyByDefaultBroker;

    fn route_request() -> AssistedAiProviderRouteRequest {
        let trust = trust_reference(
            "offline-test",
            AssistedAiTrustProjectionKind::ContextManifest,
        );
        AssistedAiProviderRouteRequest {
            route_id: "offline-route".to_string(),
            prompt_prefix: String::new(),
            provider_id: DETERMINISTIC_LOCAL_PROVIDER_ID.to_string(),
            model_label: "offline".to_string(),
            provider_class: AssistedAiProviderClass::LocalLoopback,
            operation_class: AssistedAiOperationClass::ProposeEdit,
            context_manifest: trust.clone(),
            privacy_inspector: trust.clone(),
            permission_budget: trust,
            proposal_intent: AssistedAiProposalTargetIntent {
                payload_kind: ProposalPayloadKind::TextEdit,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: Vec::new(),
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                required_capability: CapabilityId("editor.write".to_string()),
                risk_label: ProposalRiskLabel::Low,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                labels: vec!["offline-test".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            policy_decision_id: None,
            required_capability: CapabilityId("ai.provider.invoke".to_string()),
            network_target: None,
            cancellation_token: CancellationTokenId(uuid::Uuid::from_u128(42)),
            health_labels: vec!["offline".to_string()],
            cost_labels: vec!["disabled".to_string()],
            principal_id: PrincipalId("principal".to_string()),
            workspace_trust_state: WorkspaceTrustState::Trusted,
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(uuid::Uuid::from_u128(43)),
            event_sequence: EventSequence(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn offline_provider_router_refuses_without_invocation() {
        let registry = make_stub_registry();
        let broker = DenyByDefaultBroker::default();
        let response = ProviderRouter::new(&registry, &broker)
            .route_completion(route_request())
            .expect("offline route returns metadata refusal");

        assert_eq!(
            response.invocation_state,
            AssistedAiProviderInvocationState::Refused
        );
        assert_eq!(
            response.refusal.as_ref().unwrap().reason_code,
            "offline.ai_feature_disabled"
        );
        assert_eq!(
            response.output_labels,
            vec!["output.not_encoded".to_string()]
        );
    }

    #[test]
    fn offline_agent_runtime_records_metadata_only_transition() {
        let mut runtime = AgentRuntime::new(AgentRunId("offline-run".to_string()));

        runtime
            .transition(
                AgentRunState::Planning,
                "offline.plan",
                CorrelationId(1),
                CausalityId(uuid::Uuid::from_u128(44)),
                EventSequence(1),
            )
            .expect("transition is legal");

        assert_eq!(runtime.transitions().len(), 1);
        assert_eq!(runtime.transitions()[0].reason_code, "offline.plan");
        assert_eq!(
            runtime.transitions()[0].redaction_hints,
            vec![RedactionHint::MetadataOnly]
        );
    }

    #[test]
    fn offline_task_packet_without_workspace_fails_closed() {
        let mut session = minimal_session();
        session.worker_assignments[0].affected_targets[0].workspace_id = None;

        let mut coordinator = LegionWorkflowCoordinator::new(session).expect("valid workflow");
        let worker_id = LegionWorkflowWorkerId("worker:offline".to_string());

        let error = coordinator
            .task_packet_for_worker(&worker_id)
            .expect_err("must fail without workspace-scoped target");

        assert_eq!(
            error,
            OfflineAiError::InvalidLegionWorkflow(
                "task packet requires workspace-scoped target".to_string()
            )
        );
        assert!(coordinator.task_packets().is_empty());
    }

    fn minimal_session() -> LegionWorkflowSession {
        use legion_protocol::{
            LegionWorkflowSessionId, LegionWorkflowState, LegionWorkflowWorkerId,
            LegionWorkflowWorkerRole, LegionWorkflowWorkerState,
        };
        LegionWorkflowSession {
            session_id: LegionWorkflowSessionId("session:offline".to_string()),
            directive_artifact_id: None,
            spec_artifact_id: None,
            task_graph_artifact_id: None,
            product_mode: legion_protocol::ProductMode::LegionWorkflows,
            worker_assignments: vec![LegionWorkflowWorkerAssignment {
                worker_id: LegionWorkflowWorkerId("worker:offline".to_string()),
                role: LegionWorkflowWorkerRole::Implementer,
                state: LegionWorkflowWorkerState::Ready,
                model_backend: LegionWorkflowModelBackend::Local,
                display_safe_model_label: "offline:metadata".to_string(),
                allowed_command_classes: vec![],
                linked_delegated_plan_id: None,
                assisted_ai_route: None,
                affected_targets: vec![legion_protocol::DelegatedTaskAffectedTargetSummary {
                    target_id: "target:offline".to_string(),
                    kind: legion_protocol::ProposalTargetKind::MetadataOnly,
                    workspace_id: Some(WorkspaceId(1)),
                    file_id: None,
                    buffer_id: None,
                    ranges: vec![],
                    hashes: vec![],
                    counts: vec![],
                    labels: vec!["offline".to_string()],
                    risk_label: legion_protocol::ProposalRiskLabel::Low,
                    privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                }],
                risk_labels: vec![],
                privacy_labels: vec![],
                correlation_id: CorrelationId(1),
                causality_id: CausalityId(uuid::Uuid::from_u128(1)),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            dependency_edges: vec![],
            conflict_summaries: vec![],
            verification_gates: vec![],
            sign_off_records: vec![],
            proposal_ids: vec![],
            merge_approval: None,
            lifecycle_state: LegionWorkflowState::Executing,
            generated_at: TimestampMillis(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(uuid::Uuid::from_u128(1)),
        }
    }

    fn reap_temp_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "legion-app-offline-reap-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn reap_orphaned_sandboxes_removes_orphans_and_preserves_active_and_unrelated() {
        let root = reap_temp_root("basic");
        std::fs::create_dir_all(root.join("task-orphan-1")).unwrap();
        std::fs::write(root.join("task-orphan-1/marker.txt"), "stale").unwrap();
        std::fs::create_dir_all(root.join("task-active-1")).unwrap();
        std::fs::create_dir_all(root.join("not-a-task-dir")).unwrap();

        let removed = reap_orphaned_sandboxes(&root, &["active-1"]).expect("reap succeeds");

        assert_eq!(removed.len(), 1);
        assert!(removed[0].ends_with("task-orphan-1"));
        assert!(!root.join("task-orphan-1").exists(), "orphan removed");
        assert!(root.join("task-active-1").exists(), "active lane preserved");
        assert!(
            root.join("not-a-task-dir").exists(),
            "non-task dirs untouched"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reap_orphaned_sandboxes_on_missing_root_is_a_noop() {
        let root = reap_temp_root("missing").join("does-not-exist");
        let removed = reap_orphaned_sandboxes(&root, &[]).expect("noop on missing root");
        assert!(removed.is_empty());
    }

    #[test]
    fn reap_orphaned_sandboxes_skips_locked_lease_and_removes_once_released() {
        let root = reap_temp_root("leased");
        std::fs::create_dir_all(root.join("task-live-1")).unwrap();
        std::fs::write(root.join("task-live-1/marker.txt"), "live").unwrap();
        let lease_path = root.join("task-live-1.lock");

        // Simulate the owning process/orchestrator holding an exclusive
        // lease. A same-process second `File::open` yields a distinct file
        // description, so the reaper's `try_lock` on its own handle
        // genuinely contends with this one (mirroring real cross-process
        // contention).
        let holder = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lease_path)
            .expect("create lease file");
        holder.try_lock().expect("test holds the lease");

        let removed = reap_orphaned_sandboxes(&root, &[]).expect("reap succeeds");
        assert!(
            removed.is_empty(),
            "locked lease must protect its sandbox from reaping"
        );
        assert!(
            root.join("task-live-1").exists(),
            "live sandbox must survive while its lease is held"
        );
        assert!(lease_path.exists(), "lock file must survive too");

        drop(holder);
        let removed = reap_orphaned_sandboxes(&root, &[]).expect("reap succeeds after release");
        assert_eq!(removed.len(), 1);
        assert!(removed[0].ends_with("task-live-1"));
        assert!(
            !root.join("task-live-1").exists(),
            "sandbox removed once lease is released"
        );
        assert!(
            !lease_path.exists(),
            "lock file removed alongside its sandbox"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reap_orphaned_sandboxes_still_removes_legacy_sandboxes_without_a_lease_file() {
        let root = reap_temp_root("legacy");
        std::fs::create_dir_all(root.join("task-legacy-1")).unwrap();
        std::fs::write(root.join("task-legacy-1/marker.txt"), "no lease").unwrap();

        let removed = reap_orphaned_sandboxes(&root, &[]).expect("reap succeeds");

        assert_eq!(removed.len(), 1);
        assert!(removed[0].ends_with("task-legacy-1"));
        assert!(
            !root.join("task-legacy-1").exists(),
            "legacy sandbox without a lease file must still be reaped"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
