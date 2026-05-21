//! Application composition root for workspace/editor/ui orchestration.

#![warn(missing_docs)]

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use devil_editor::{
    EditorEngine, EditorError, SaveAcknowledgement, SaveRequestDto, TextEdit, TextPosition,
    TextRange as EditorTextRange,
};
use devil_observability::{
    SharedEventSink, event_metadata_record, proposal_applied_event, proposal_approved_event,
    proposal_audit_record, proposal_audit_recorded_event, proposal_created_event,
    proposal_failed_event, proposal_previewed_event, proposal_rejected_event,
    proposal_rolled_back_event, proposal_validated_event, save_denied_event,
    stale_proposal_rejected_event, transaction_event,
};
use devil_platform::{NativeFileSystem, NativeWatcherService};
use devil_project::{
    OpenedFileText, WorkspaceActor, WorkspaceCreateFileRequest, WorkspaceDeleteFileRequest,
    WorkspaceError, WorkspaceRenameFileRequest, WorkspaceSaveRequest,
};
use devil_protocol::{
    BatchProposalPayload, BufferId, CanonicalPath, CapabilityId, CapabilityNamespace, CausalityId,
    CorrelationId, EditorApplyTransactionRequest, EventEnvelope, EventSequence, EventSinkPort,
    EventSinkRequest, FileConflictContext, FileConflictLifecycleState, FileConflictReason,
    FileConflictState, FileContentVersion, FileFingerprint, FileId, FileIdentity, FileTreeNode,
    PreviewSummary, PrincipalId, ProposalAffectedTarget, ProposalBatchAtomicity, ProposalBatchItem,
    ProposalBatchRollbackPolicy, ProposalCancellationReason, ProposalDenialReason,
    ProposalFailureReason, ProposalId, ProposalLifecycleAction, ProposalLifecycleCommand,
    ProposalLifecycleCommandReason, ProposalLifecycleState, ProposalLifecycleTransition,
    ProposalPartialFailureDisposition, ProposalPartialFailureRecord, ProposalPayload, ProposalPort,
    ProposalPreviewWarning, ProposalPreviewWarningKind, ProposalRejectionReason, ProposalRequest,
    ProposalResponse, ProposalRollbackReason, ProposalStaleReason, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProposalTargetKind, ProposalVersionPreconditions,
    ProtocolDiagnostic, ProtocolDiagnosticSeverity, ProtocolError, ProtocolResult, RedactionHint,
    SaveConflictPolicy, SaveFileProposal, SaveIntent, StorageRepositoryPort,
    StorageRepositoryRequest, TextCoordinate, TextTransactionDescriptor, TimestampMillis,
    TransactionSource, TrustDecisionContext, VersionContext, WorkspaceCloseRequest,
    WorkspaceGeneration, WorkspaceId, WorkspaceOpenRequest, WorkspaceOpened, WorkspacePort,
    WorkspaceProposal, WorkspaceRequest, WorkspaceResponse, WorkspaceTrustState,
};
use devil_security::{DenyByDefaultBroker, SecurityPolicy};
use devil_storage::InMemoryStorageRepositoryPort;
use devil_ui::{
    ActiveBufferProjection, CommandDispatchIntent, ExplorerNodeProjection, ExplorerProjection,
    ExplorerSelectionProjection, ShellLayoutProjection, ShellProjectionSnapshot,
};
use thiserror::Error;

/// App-level composition errors.
#[derive(Debug, Error)]
pub enum AppCompositionError {
    /// Workspace operation failed.
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    /// Editor operation failed.
    #[error(transparent)]
    Editor(#[from] EditorError),
    /// Protocol-port operation failed.
    #[error("protocol error: {0:?}")]
    Protocol(ProtocolError),
    /// No active workspace.
    #[error("workspace is not open")]
    WorkspaceNotOpen,
    /// No active file in composition.
    #[error("active file is not selected")]
    ActiveFileMissing,
    /// Active buffer not initialized for selected file.
    #[error("active buffer is not open")]
    ActiveBufferMissing,
    /// UI command targeted a buffer other than the active application buffer.
    #[error("command targeted buffer {target:?}, but active buffer is {active:?}")]
    BufferMismatch {
        /// Targeted buffer id.
        target: BufferId,
        /// Active buffer id.
        active: Option<BufferId>,
    },
    /// UI proposal intent targeted a proposal other than the app-owned proposal being routed.
    #[error("proposal intent targeted {target:?}, but routed proposal is {active:?}")]
    ProposalIntentMismatch {
        /// Targeted proposal id.
        target: ProposalId,
        /// App-owned proposal id available for routing.
        active: Option<ProposalId>,
    },
    /// UI proposal intent requires an app-owned proposal object for routing.
    #[error("proposal intent requires an app-owned proposal")]
    ProposalIntentMissingProposal,
    /// Proposal-mediated save did not apply.
    #[error("save proposal did not apply: {0:?}")]
    SaveProposalRejected(Box<ProposalResponse>),
}

/// Typed save result returned by application save routing.
#[derive(Debug, Clone)]
pub enum AppSaveOutcome {
    /// Save applied successfully.
    Saved(SaveRequestDto),
    /// Save proposal was rejected, denied, stale, conflicting, or failed without mutating disk.
    Rejected(Box<ProposalResponse>),
}

/// Intent used when opening a path into an editor buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenFileIntent {
    /// Open must load an existing text file and surface read failures.
    Existing,
    /// Open may create an empty editor buffer only under explicit safe-new-file preconditions.
    CreateNew,
}

#[derive(Debug, Clone)]
struct ActiveFileMetadata {
    identity: FileIdentity,
    fingerprint: FileFingerprint,
    file_content_version: FileContentVersion,
    workspace_generation: WorkspaceGeneration,
    modified_at: Option<TimestampMillis>,
    file_length: Option<u64>,
}

/// Non-zero observability identifiers assigned to one app-routed workflow.
#[derive(Debug, Clone, Copy)]
pub struct EventContext {
    /// Non-zero correlation identifier for this routed workflow.
    pub correlation_id: CorrelationId,
    /// Non-nil causality identifier linking emitted audit events.
    pub causality_id: CausalityId,
}

/// Side-effect-free app-level preflight result for a batch proposal.
#[derive(Debug, Clone)]
pub struct BatchPreflightPlan {
    /// Proposal id that was planned.
    pub proposal_id: ProposalId,
    /// Batch id when the proposal payload is a batch.
    pub batch_id: Option<uuid::Uuid>,
    /// True when every structural, route, precondition, and rollback-boundary check passed.
    pub preflight_ok: bool,
    /// True because Stage 1D intentionally keeps runtime batch mutation fail-closed.
    pub runtime_apply_disabled: bool,
    /// Batch atomicity promise, when available.
    pub atomicity: Option<ProposalBatchAtomicity>,
    /// Batch rollback policy, when available.
    pub rollback_policy: Option<ProposalBatchRollbackPolicy>,
    /// Planning semantics selected from the batch DTO before any mutation is possible.
    pub planning_semantics: Option<BatchPlanningSemantics>,
    /// Rollback proof/acceptance contract built before any mutation is possible.
    pub rollback_contract: Option<BatchRollbackContract>,
    /// Deterministic item planning records sorted by `order`, then `item_id`.
    pub items: Vec<BatchPreflightItemPlan>,
    /// Proposal-level diagnostics collected without mutating editor or workspace state.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Preview warnings collected during planning.
    pub preview_warnings: Vec<ProposalPreviewWarning>,
    /// Planning/preflight failure records. These never imply mutation happened.
    pub partial_failures: Vec<ProposalPartialFailureRecord>,
}

/// Side-effect-free preflight record for one batch item.
#[derive(Debug, Clone)]
pub struct BatchPreflightItemPlan {
    /// Stable item id from the batch payload.
    pub item_id: String,
    /// Application order from the batch item.
    pub order: u32,
    /// App-level route selected for the item payload.
    pub route: BatchPreflightRoute,
    /// Whether Stage 1D can preflight this route.
    pub supported: bool,
    /// Whether this item passed preflight checks.
    pub preflight_ok: bool,
    /// Target ids referenced by this item.
    pub target_ids: Vec<String>,
    /// Diagnostics scoped to this item.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Inspectable Stage 1E batch execution contract assembled without mutating app state.
#[derive(Debug, Clone)]
pub struct BatchExecutionContract {
    /// Proposal id that was planned.
    pub proposal_id: ProposalId,
    /// Batch id when the proposal payload is a batch.
    pub batch_id: Option<uuid::Uuid>,
    /// The Stage 1D preflight plan reused as the contract's prepare/preflight basis.
    pub preflight: BatchPreflightPlan,
    /// Ordered execution stages and their current safety gates.
    pub stages: Vec<BatchExecutionStageContract>,
    /// True because Stage 1E still denies runtime batch mutation.
    pub runtime_apply_disabled: bool,
    /// True until a future mutator can prove safe commit semantics.
    pub commit_blocked: bool,
    /// True until mutation, commit, and audit proof are available.
    pub finalize_blocked: bool,
    /// True because future success responses must be preceded by durable audit proof.
    pub audit_before_success_required: bool,
    /// Planning semantics selected from atomicity and rollback policy.
    pub planning_semantics: Option<BatchPlanningSemantics>,
    /// Rollback proof/acceptance contract required before mutation.
    pub rollback_contract: Option<BatchRollbackContract>,
    /// Per-item execution contracts derived from deterministic preflight item order.
    pub items: Vec<BatchExecutionItemContract>,
    /// Contract-level diagnostics that prevent interpreting planning as execution.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Contract-level preview warnings that prevent interpreting planning as execution.
    pub preview_warnings: Vec<ProposalPreviewWarning>,
    /// Deterministic partial-failure records for planning failures and blocked dependents.
    pub partial_failures: Vec<ProposalPartialFailureRecord>,
}

/// One ordered Stage 1E execution stage gate.
#[derive(Debug, Clone)]
pub struct BatchExecutionStageContract {
    /// Stage name in deterministic execution order.
    pub stage: BatchExecutionStage,
    /// Whether this stage is required before any future batch success response.
    pub required: bool,
    /// Whether the current implementation blocks this stage.
    pub blocked: bool,
    /// Diagnostics explaining why the stage is blocked or required.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Stage 1E batch execution stages, ordered by contract evaluation sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchExecutionStage {
    /// Build a deterministic execution plan from immutable proposal data.
    Prepare,
    /// Validate structure, routes, preconditions, dependencies, and rollback proof.
    Preflight,
    /// Mutate editor/workspace state. Disabled in Stage 1E.
    Mutate,
    /// Commit successful mutation results. Blocked while mutation is disabled.
    Commit,
    /// Persist audit evidence before success is observable.
    Audit,
    /// Return a future success/final state only after audit proof exists.
    Finalize,
    /// Roll back committed mutations exactly when required. Disabled in Stage 1E.
    Rollback,
}

/// App-level batch planning semantics derived from protocol atomicity and rollback policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchPlanningSemantics {
    /// All items must preflight successfully and exact rollback proof is required before mutation.
    Atomic,
    /// Items are ordered and rollback is best-effort or explicitly unsupported by policy.
    BestEffort,
    /// The batch is planned only; mutation remains a dry-run/preflight contract.
    DryRun,
}

/// Rollback proof/acceptance status for a planned batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchRollbackContractStatus {
    /// Every reversible item has an exact, route-compatible rollback step.
    Exact,
    /// Rollback is explicitly best-effort and may produce failure records.
    BestEffort,
    /// Rollback is not required because the plan is dry-run or metadata/preflight-only.
    NotRequired,
    /// Rollback is unsupported but explicitly accepted by ordered non-atomic policy.
    IrreversibleAccepted,
    /// Rollback proof is insufficient and mutation must be denied.
    Denied,
}

/// Side-effect-free rollback contract assembled before batch mutation.
#[derive(Debug, Clone)]
pub struct BatchRollbackContract {
    /// Rollback policy declared by the batch DTO.
    pub policy: ProposalBatchRollbackPolicy,
    /// Atomicity declared by the batch DTO.
    pub atomicity: ProposalBatchAtomicity,
    /// Planning semantics selected by app coordination.
    pub semantics: BatchPlanningSemantics,
    /// Overall rollback proof/acceptance status.
    pub status: BatchRollbackContractStatus,
    /// Whether irreversible execution is explicitly accepted by the DTO combination.
    pub irreversible_execution_accepted: bool,
    /// Number of reversible mutation items in the planned batch.
    pub reversible_item_count: usize,
    /// Deterministic rollback step contracts resolved before mutation.
    pub steps: Vec<BatchRollbackStepContract>,
    /// Metadata-only diagnostics that must block mutation when status is denied.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Side-effect-free rollback-step proof for one batch item/target.
#[derive(Debug, Clone)]
pub struct BatchRollbackStepContract {
    /// Stable rollback step identifier.
    pub step_id: String,
    /// Owning batch item identifier.
    pub item_id: String,
    /// Target identifier covered by this rollback step.
    pub target_id: String,
    /// Rollback action declared by protocol DTO.
    pub action: devil_protocol::ProposalRollbackAction,
    /// Whether the step exactly matches owning item, target, route, and has no diagnostics.
    pub exact: bool,
    /// Metadata-only diagnostics scoped to this rollback step.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Per-item Stage 1E execution safety contract.
#[derive(Debug, Clone)]
pub struct BatchExecutionItemContract {
    /// Stable item id from the batch payload.
    pub item_id: String,
    /// Application order from the batch item.
    pub order: u32,
    /// App-level route selected for the item payload.
    pub route: BatchPreflightRoute,
    /// Target ids referenced by this item.
    pub target_ids: Vec<String>,
    /// Whether preflight accepts the item before any mutation.
    pub preflight_ok: bool,
    /// Whether required rollback proof resolves exactly and is route-compatible.
    pub exact_rollback_proof: bool,
    /// The item's planning/blocked disposition when it cannot execute.
    pub partial_failure_disposition: Option<ProposalPartialFailureDisposition>,
    /// Diagnostics scoped to this item contract.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Route classification used by the Stage 1D batch preflight planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchPreflightRoute {
    /// Open-buffer text edit.
    TextEdit,
    /// Closed workspace create-file operation.
    CreateFile,
    /// Closed workspace delete-file operation.
    DeleteFile,
    /// Closed workspace rename-file operation.
    RenameFile,
    /// Nested batch, intentionally denied.
    Batch,
    /// Terminal command, intentionally denied.
    Terminal,
    /// Save proposal, intentionally denied inside batch planning.
    Save,
    /// Format proposal, intentionally denied in Stage 1D batch planning.
    Format,
    /// Code action proposal, intentionally denied in Stage 1D batch planning.
    CodeAction,
    /// Workspace edit proposal, intentionally denied in Stage 1D batch planning.
    WorkspaceEdit,
    /// Plugin, remote, collaboration, metadata-only, mixed, or otherwise unsupported route.
    Unsupported,
}

impl EventContext {
    fn new(correlation_id: CorrelationId) -> Self {
        Self {
            correlation_id: Self::non_zero_correlation_id(correlation_id),
            causality_id: CausalityId(uuid::Uuid::now_v7()),
        }
    }

    fn non_zero_correlation_id(correlation_id: CorrelationId) -> CorrelationId {
        CorrelationId(correlation_id.0.max(1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProposalExecutionRoute {
    SaveFile,
    EditorBuffer,
    WorkspaceFile,
    Terminal,
    Batch,
    MetadataOnly,
    Mixed,
    Unsupported,
}

impl ProposalExecutionRoute {
    fn for_payload(payload: &ProposalPayload, coverage: &ProposalTargetCoverage) -> Self {
        match payload {
            ProposalPayload::SaveFile(_) => Self::SaveFile,
            ProposalPayload::Batch(_) => Self::Batch,
            _ if coverage.targets.is_empty() => Self::Unsupported,
            _ => {
                let mut has_editor = false;
                let mut has_workspace = false;
                let mut has_terminal = false;
                let mut has_metadata = false;
                let mut has_other = false;

                for target in &coverage.targets {
                    match target.kind {
                        ProposalTargetKind::OpenBuffer => has_editor = true,
                        ProposalTargetKind::ClosedFile | ProposalTargetKind::PathOnly => {
                            has_workspace = true;
                        }
                        ProposalTargetKind::TerminalSession => has_terminal = true,
                        ProposalTargetKind::MetadataOnly => has_metadata = true,
                        ProposalTargetKind::RemoteWorkspace
                        | ProposalTargetKind::CollaborationSession
                        | ProposalTargetKind::Plugin => has_other = true,
                    }
                }

                let route_count = [
                    has_editor,
                    has_workspace,
                    has_terminal,
                    has_metadata,
                    has_other,
                ]
                .into_iter()
                .filter(|present| *present)
                .count();

                match (
                    route_count,
                    has_editor,
                    has_workspace,
                    has_terminal,
                    has_metadata,
                    has_other,
                ) {
                    (1, true, false, false, false, false) => Self::EditorBuffer,
                    (1, false, true, false, false, false) => Self::WorkspaceFile,
                    (1, false, false, true, false, false) => Self::Terminal,
                    (1, false, false, false, true, false) => Self::MetadataOnly,
                    (_, _, _, _, _, true) => Self::Unsupported,
                    _ => Self::Mixed,
                }
            }
        }
    }
}

#[derive(Debug)]
struct AppProposalCoordinator {
    next_proposal_id: Cell<u64>,
    event_sink: SharedEventSink,
    next_event_sequence: Cell<u64>,
    proposal_contexts: RefCell<HashMap<ProposalId, EventContext>>,
    proposal_states: RefCell<HashMap<ProposalId, ProposalLifecycleState>>,
    proposals: RefCell<HashMap<ProposalId, WorkspaceProposal>>,
}

impl AppProposalCoordinator {
    fn new(event_sink: SharedEventSink) -> Self {
        Self {
            next_proposal_id: Cell::new(0),
            event_sink,
            next_event_sequence: Cell::new(0),
            proposal_contexts: RefCell::new(HashMap::new()),
            proposal_states: RefCell::new(HashMap::new()),
            proposals: RefCell::new(HashMap::new()),
        }
    }

    fn next_id(&self) -> devil_protocol::ProposalId {
        let next = self.next_proposal_id.get().saturating_add(1).max(1);
        self.next_proposal_id.set(next);
        devil_protocol::ProposalId(next)
    }

    fn next_sequence(&self) -> EventSequence {
        let next = self.next_event_sequence.get().saturating_add(1).max(1);
        self.next_event_sequence.set(next);
        EventSequence(next)
    }

    fn emit(&self, envelope: EventEnvelope) -> ProtocolResult<()> {
        self.event_sink.emit(EventSinkRequest { envelope })
    }

    fn build_save_proposal(
        &self,
        save: &SaveRequestDto,
        metadata: &ActiveFileMetadata,
        principal: PrincipalId,
        workspace_trust_state: WorkspaceTrustState,
        event_context: EventContext,
    ) -> WorkspaceProposal {
        let proposal_id = self.next_id();
        self.register_lifecycle_context(proposal_id, event_context);
        let _ = self.record_lifecycle_state(proposal_id, ProposalLifecycleState::Created);
        let capability = CapabilityId("fs.write".to_string());
        let preconditions = ProposalVersionPreconditions {
            file_version: Some(metadata.file_content_version),
            buffer_version: Some(save.buffer_version),
            snapshot_id: Some(save.snapshot_id),
            generation: Some(metadata.workspace_generation),
            file_content_version: Some(metadata.file_content_version),
            workspace_generation: Some(metadata.workspace_generation),
            expected_fingerprint: Some(metadata.fingerprint.clone()),
            expected_file_length: None,
            expected_modified_at: None,
        };
        let preview = PreviewSummary {
            summary: format!(
                "Save {} bytes to {}",
                save.payload_byte_len, metadata.identity.canonical_path.0
            ),
            details: vec![
                format!("buffer_version={}", save.buffer_version.0),
                format!("snapshot_id={}", save.snapshot_id.0),
                format!("file_content_version={}", metadata.file_content_version.0),
                format!("workspace_generation={}", metadata.workspace_generation.0),
                format!("expected_fingerprint={}", metadata.fingerprint.value),
                format!(
                    "modified_at={}",
                    metadata
                        .modified_at
                        .map(|value| value.0.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
                format!(
                    "file_length={}",
                    metadata
                        .file_length
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
            ],
        };
        let payload = SaveFileProposal {
            file: metadata.identity.clone(),
            buffer_id: save.buffer_id,
            file_id: save.file_id,
            snapshot_id: save.snapshot_id,
            buffer_version: save.buffer_version,
            file_content_version: metadata.file_content_version,
            workspace_generation: metadata.workspace_generation,
            expected_fingerprint: Some(metadata.fingerprint.clone()),
            save_intent: SaveIntent::Manual,
            conflict_policy: SaveConflictPolicy::RejectIfChanged,
            trust_decision: TrustDecisionContext {
                workspace_trust_state,
                decision_id: None,
                decided_at: Some(TimestampMillis::now()),
            },
            required_capability: capability.clone(),
            principal: principal.clone(),
            correlation_id: save.correlation_id,
            diagnostics: Vec::new(),
        };

        let proposal = WorkspaceProposal {
            proposal_id,
            principal,
            capability,
            correlation_id: save.correlation_id,
            payload: ProposalPayload::SaveFile(payload),
            preconditions,
            preview,
            expires_at: None,
            created_at: TimestampMillis::now(),
        };
        self.remember_proposal(&proposal);
        proposal
    }

    fn created_response(&self, proposal: &WorkspaceProposal) -> ProposalResponse {
        self.remember_proposal(proposal);
        match self.record_transition(proposal, ProposalLifecycleState::Created, "create") {
            Ok(transition) => ProposalResponse::Created(transition),
            Err(response) => response,
        }
    }

    fn register_lifecycle_context(&self, proposal_id: ProposalId, event_context: EventContext) {
        self.proposal_contexts
            .borrow_mut()
            .insert(proposal_id, event_context);
    }

    fn remember_proposal(&self, proposal: &WorkspaceProposal) {
        self.proposals
            .borrow_mut()
            .insert(proposal.proposal_id, proposal.clone());
    }

    fn proposal(&self, proposal_id: ProposalId) -> Option<WorkspaceProposal> {
        self.proposals.borrow().get(&proposal_id).cloned()
    }

    fn has_lifecycle_context(&self, proposal_id: ProposalId) -> bool {
        self.proposal_contexts.borrow().contains_key(&proposal_id)
    }

    fn current_lifecycle_state(&self, proposal_id: ProposalId) -> Option<ProposalLifecycleState> {
        self.proposal_states.borrow().get(&proposal_id).copied()
    }

    fn record_lifecycle_state(
        &self,
        proposal_id: ProposalId,
        next_state: ProposalLifecycleState,
    ) -> Result<(), Option<ProposalLifecycleState>> {
        let mut states = self.proposal_states.borrow_mut();
        let current = states.get(&proposal_id).copied();

        if Self::lifecycle_transition_allowed(current, next_state) {
            states.insert(proposal_id, next_state);
            Ok(())
        } else {
            Err(current)
        }
    }

    fn lifecycle_transition_allowed(
        current: Option<ProposalLifecycleState>,
        next: ProposalLifecycleState,
    ) -> bool {
        let Some(current) = current else {
            return next == ProposalLifecycleState::Created;
        };

        if current == next {
            return true;
        }

        matches!(
            (current, next),
            (
                ProposalLifecycleState::Created,
                ProposalLifecycleState::Validated
                    | ProposalLifecycleState::Denied
                    | ProposalLifecycleState::Rejected
                    | ProposalLifecycleState::Cancelled
                    | ProposalLifecycleState::Failed
            ) | (
                ProposalLifecycleState::Validated,
                ProposalLifecycleState::Previewed
                    | ProposalLifecycleState::Denied
                    | ProposalLifecycleState::Rejected
                    | ProposalLifecycleState::Cancelled
                    | ProposalLifecycleState::Failed
            ) | (
                ProposalLifecycleState::Previewed,
                ProposalLifecycleState::Approved
                    | ProposalLifecycleState::Applied
                    | ProposalLifecycleState::Denied
                    | ProposalLifecycleState::Rejected
                    | ProposalLifecycleState::Stale
                    | ProposalLifecycleState::Conflict
                    | ProposalLifecycleState::Cancelled
                    | ProposalLifecycleState::Failed
            ) | (
                ProposalLifecycleState::Approved,
                ProposalLifecycleState::Applied
                    | ProposalLifecycleState::Denied
                    | ProposalLifecycleState::Rejected
                    | ProposalLifecycleState::Stale
                    | ProposalLifecycleState::Conflict
                    | ProposalLifecycleState::Cancelled
                    | ProposalLifecycleState::Failed
            ) | (
                ProposalLifecycleState::Applied,
                ProposalLifecycleState::RolledBack
            ) | (
                ProposalLifecycleState::Failed,
                ProposalLifecycleState::RolledBack
            )
        )
    }

    #[allow(clippy::result_large_err)]
    fn record_transition(
        &self,
        proposal: &WorkspaceProposal,
        lifecycle_state: ProposalLifecycleState,
        action: &str,
    ) -> Result<ProposalLifecycleTransition, ProposalResponse> {
        self.record_transition_with_diagnostics(proposal, lifecycle_state, action, Vec::new())
    }

    #[allow(clippy::result_large_err)]
    fn record_transition_with_diagnostics(
        &self,
        proposal: &WorkspaceProposal,
        lifecycle_state: ProposalLifecycleState,
        action: &str,
        diagnostics: Vec<ProtocolDiagnostic>,
    ) -> Result<ProposalLifecycleTransition, ProposalResponse> {
        if !self.has_lifecycle_context(proposal.proposal_id) {
            return Err(self.missing_lifecycle_context_response(proposal, action));
        }

        let context_diagnostics = self.lifecycle_context_diagnostics(proposal);
        if !context_diagnostics.is_empty() {
            return Err(self.invalid_lifecycle_context_response(
                proposal,
                action,
                context_diagnostics,
            ));
        }

        if proposal.is_expired(TimestampMillis::now())
            && !Self::allows_expired_transition(lifecycle_state)
        {
            return Err(self.expired_lifecycle_response(proposal, action, diagnostics));
        }

        match self.record_lifecycle_state(proposal.proposal_id, lifecycle_state) {
            Ok(()) => Ok(self.transition(proposal, lifecycle_state, diagnostics)),
            Err(current) => Err(self.invalid_lifecycle_transition_response(
                proposal,
                action,
                current,
                lifecycle_state,
            )),
        }
    }

    fn allows_expired_transition(lifecycle_state: ProposalLifecycleState) -> bool {
        matches!(
            lifecycle_state,
            ProposalLifecycleState::Created
                | ProposalLifecycleState::Rejected
                | ProposalLifecycleState::Denied
                | ProposalLifecycleState::Failed
                | ProposalLifecycleState::RolledBack
                | ProposalLifecycleState::Stale
                | ProposalLifecycleState::Conflict
                | ProposalLifecycleState::Cancelled
        )
    }

    fn lifecycle_context_diagnostics(
        &self,
        proposal: &WorkspaceProposal,
    ) -> Vec<ProtocolDiagnostic> {
        let mut diagnostics = Vec::new();
        if proposal.correlation_id.0 == 0 {
            diagnostics.push(Self::diagnostic(
                "proposal.zero_correlation_id",
                "proposal lifecycle transition requires a non-zero proposal correlation id",
            ));
        }
        if let Some(context) = self.proposal_contexts.borrow().get(&proposal.proposal_id) {
            if context.correlation_id.0 == 0 {
                diagnostics.push(Self::diagnostic(
                    "proposal.lifecycle_context_zero_correlation_id",
                    "proposal lifecycle context requires a non-zero correlation id",
                ));
            }
            if context.causality_id.0.is_nil() {
                diagnostics.push(Self::diagnostic(
                    "proposal.lifecycle_context_nil_causality_id",
                    "proposal lifecycle context requires a non-nil causality id",
                ));
            }
        }
        diagnostics
    }

    fn record_observed_transition(&self, transition: &ProposalLifecycleTransition) {
        if self.has_lifecycle_context(transition.proposal_id) {
            let _ = self.record_lifecycle_state(transition.proposal_id, transition.lifecycle_state);
        }
    }

    fn transition(
        &self,
        proposal: &WorkspaceProposal,
        lifecycle_state: ProposalLifecycleState,
        diagnostics: Vec<ProtocolDiagnostic>,
    ) -> ProposalLifecycleTransition {
        let context = self
            .proposal_contexts
            .borrow()
            .get(&proposal.proposal_id)
            .copied();
        let correlation_id = context
            .map(|context| context.correlation_id)
            .filter(|correlation_id| correlation_id.0 != 0)
            .or_else(|| (proposal.correlation_id.0 != 0).then_some(proposal.correlation_id))
            .unwrap_or(CorrelationId(1));
        let causality_id = context
            .map(|context| context.causality_id)
            .filter(|causality_id| !causality_id.0.is_nil())
            .unwrap_or_else(|| CausalityId(uuid::Uuid::now_v7()));
        ProposalLifecycleTransition {
            proposal_id: proposal.proposal_id,
            lifecycle_state,
            timestamp: TimestampMillis::now(),
            principal: proposal.principal.clone(),
            capability: proposal.capability.clone(),
            correlation_id,
            causality_id,
            diagnostics,
        }
    }

    fn diagnostic(code: impl Into<String>, message: impl Into<String>) -> ProtocolDiagnostic {
        ProtocolDiagnostic {
            code: code.into(),
            message: message.into(),
            severity: ProtocolDiagnosticSeverity::Error,
            path: None,
            range: None,
        }
    }

    fn missing_lifecycle_context_response(
        &self,
        proposal: &WorkspaceProposal,
        action: &str,
    ) -> ProposalResponse {
        ProposalResponse::Rejected {
            transition: self.transition(
                proposal,
                ProposalLifecycleState::Rejected,
                vec![Self::diagnostic(
                    "proposal.missing_lifecycle_context",
                    format!(
                        "proposal {action} requires app-created lifecycle context before it can proceed"
                    ),
                )],
            ),
            reason: ProposalRejectionReason::ValidationFailed,
        }
    }

    fn invalid_lifecycle_transition_response(
        &self,
        proposal: &WorkspaceProposal,
        action: &str,
        current: Option<ProposalLifecycleState>,
        next: ProposalLifecycleState,
    ) -> ProposalResponse {
        ProposalResponse::Rejected {
            transition: self.transition(
                proposal,
                ProposalLifecycleState::Rejected,
                vec![Self::diagnostic(
                    "proposal.invalid_lifecycle_transition",
                    format!("proposal {action} cannot transition from {current:?} to {next:?}"),
                )],
            ),
            reason: ProposalRejectionReason::ValidationFailed,
        }
    }

    fn invalid_lifecycle_context_response(
        &self,
        proposal: &WorkspaceProposal,
        action: &str,
        mut diagnostics: Vec<ProtocolDiagnostic>,
    ) -> ProposalResponse {
        diagnostics.insert(
            0,
            Self::diagnostic(
                "proposal.invalid_lifecycle_context",
                format!(
                    "proposal {action} requires non-zero correlation and non-nil causality lifecycle context"
                ),
            ),
        );
        ProposalResponse::Rejected {
            transition: self.transition(proposal, ProposalLifecycleState::Rejected, diagnostics),
            reason: ProposalRejectionReason::ValidationFailed,
        }
    }

    fn expired_lifecycle_response(
        &self,
        proposal: &WorkspaceProposal,
        action: &str,
        mut diagnostics: Vec<ProtocolDiagnostic>,
    ) -> ProposalResponse {
        diagnostics.insert(
            0,
            Self::diagnostic(
                "proposal.expired",
                format!("proposal {action} cannot proceed because the proposal is expired"),
            ),
        );
        match self.record_lifecycle_state(proposal.proposal_id, ProposalLifecycleState::Rejected) {
            Ok(()) => ProposalResponse::Rejected {
                transition: self.transition(
                    proposal,
                    ProposalLifecycleState::Rejected,
                    diagnostics,
                ),
                reason: ProposalRejectionReason::Expired,
            },
            Err(current) => self.invalid_lifecycle_transition_response(
                proposal,
                action,
                current,
                ProposalLifecycleState::Rejected,
            ),
        }
    }

    fn missing_lifecycle_command_response(
        &self,
        command: &ProposalLifecycleCommand,
        action: &str,
    ) -> ProposalResponse {
        ProposalResponse::Rejected {
            transition: self.transition_for_command(
                command,
                ProposalLifecycleState::Rejected,
                vec![Self::diagnostic(
                    "proposal.missing_lifecycle_context",
                    format!(
                        "proposal {action} requires app-created lifecycle context before it can proceed"
                    ),
                )],
            ),
            reason: ProposalRejectionReason::ValidationFailed,
        }
    }

    fn invalid_lifecycle_command_response(
        &self,
        command: &ProposalLifecycleCommand,
        action: &str,
        current: Option<ProposalLifecycleState>,
        next: ProposalLifecycleState,
    ) -> ProposalResponse {
        ProposalResponse::Rejected {
            transition: self.transition_for_command(
                command,
                ProposalLifecycleState::Rejected,
                vec![Self::diagnostic(
                    "proposal.invalid_lifecycle_transition",
                    format!("proposal {action} cannot transition from {current:?} to {next:?}"),
                )],
            ),
            reason: ProposalRejectionReason::ValidationFailed,
        }
    }

    fn affected_target_coverage(proposal: &WorkspaceProposal) -> ProposalTargetCoverage {
        Self::affected_target_coverage_for_payload(&proposal.payload)
    }

    fn affected_target_coverage_for_payload(payload: &ProposalPayload) -> ProposalTargetCoverage {
        if let ProposalPayload::Batch(batch) = payload
            && Self::coverage_is_declared(&batch.target_coverage)
        {
            return batch.target_coverage.clone();
        }
        if let ProposalPayload::WorkspaceEdit(workspace_edit) = payload
            && Self::coverage_is_declared(&workspace_edit.target_coverage)
        {
            return workspace_edit.target_coverage.clone();
        }

        let mut targets = Vec::new();
        Self::visit_inferred_payload_targets(payload, &mut |target| targets.push(target));
        ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets,
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        }
    }

    fn coverage_is_declared(coverage: &ProposalTargetCoverage) -> bool {
        !coverage.targets.is_empty()
            || coverage.omitted_target_count > 0
            || coverage.coverage_kind != ProposalTargetCoverageKind::Complete
    }

    fn inferred_targets(payload: &ProposalPayload) -> Vec<ProposalAffectedTarget> {
        let mut targets = Vec::new();
        Self::visit_inferred_payload_targets(payload, &mut |target| targets.push(target));
        targets
    }

    fn visit_inferred_payload_targets(
        payload: &ProposalPayload,
        visitor: &mut impl FnMut(ProposalAffectedTarget),
    ) {
        match payload {
            ProposalPayload::TextEdit(payload) => {
                let byte_ranges = payload
                    .edits
                    .edits
                    .iter()
                    .filter_map(|edit| edit.range.as_byte_range())
                    .collect();
                visitor(ProposalAffectedTarget {
                    target_id: format!("text-edit:file:{}", payload.file_id.0),
                    kind: ProposalTargetKind::OpenBuffer,
                    workspace_id: None,
                    file_id: Some(payload.file_id),
                    buffer_id: None,
                    path: None,
                    terminal_session_id: None,
                    plugin_id: None,
                    remote_authority: None,
                    collaboration_session_id: None,
                    byte_ranges,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                });
            }
            ProposalPayload::CreateFile(payload) => visitor(Self::path_target(
                format!("create-file:path:{}", payload.path.0),
                ProposalTargetKind::PathOnly,
                payload.path.clone(),
                Vec::new(),
            )),
            ProposalPayload::DeleteFile(payload) => visitor(Self::file_identity_target(
                format!("delete-file:file:{}", payload.file.file_id.0),
                ProposalTargetKind::ClosedFile,
                &payload.file,
                None,
                Vec::new(),
            )),
            ProposalPayload::RenameFile(payload) => {
                visitor(Self::file_identity_target(
                    format!("rename-file:source:{}", payload.file.file_id.0),
                    ProposalTargetKind::ClosedFile,
                    &payload.file,
                    None,
                    Vec::new(),
                ));
                visitor(Self::path_target(
                    format!("rename-file:destination:{}", payload.destination.0),
                    ProposalTargetKind::PathOnly,
                    payload.destination.clone(),
                    Vec::new(),
                ));
            }
            ProposalPayload::SaveFile(payload) => visitor(Self::file_identity_target(
                format!(
                    "save-file:file:{}:buffer:{}",
                    payload.file.file_id.0, payload.buffer_id.0
                ),
                ProposalTargetKind::OpenBuffer,
                &payload.file,
                Some(payload.buffer_id),
                Vec::new(),
            )),
            ProposalPayload::FormatFile(payload) => visitor(Self::file_identity_target(
                format!("format-file:file:{}", payload.file.file_id.0),
                ProposalTargetKind::ClosedFile,
                &payload.file,
                None,
                Vec::new(),
            )),
            ProposalPayload::CodeAction(payload) => {
                let byte_ranges = payload
                    .edits
                    .iter()
                    .filter_map(|edit| edit.range.as_byte_range())
                    .collect();
                visitor(Self::file_identity_target(
                    format!("code-action:file:{}", payload.file.file_id.0),
                    ProposalTargetKind::ClosedFile,
                    &payload.file,
                    None,
                    byte_ranges,
                ));
            }
            ProposalPayload::TerminalCommand(payload) => visitor(ProposalAffectedTarget {
                target_id: payload
                    .session_id
                    .map(|session_id| format!("terminal:{}", session_id.0))
                    .unwrap_or_else(|| "terminal:new".to_string()),
                kind: ProposalTargetKind::TerminalSession,
                workspace_id: None,
                file_id: None,
                buffer_id: None,
                path: payload.cwd.clone(),
                terminal_session_id: payload.session_id,
                plugin_id: None,
                remote_authority: None,
                collaboration_session_id: None,
                byte_ranges: Vec::new(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
            }),
            ProposalPayload::WorkspaceEdit(payload) => {
                for edit in &payload.file_edits {
                    let byte_ranges = edit
                        .edits
                        .edits
                        .iter()
                        .filter_map(|edit| edit.range.as_byte_range())
                        .collect();
                    visitor(Self::file_identity_target(
                        format!("workspace-edit:text:file:{}", edit.file.file_id.0),
                        if edit.buffer_id.is_some() {
                            ProposalTargetKind::OpenBuffer
                        } else {
                            ProposalTargetKind::ClosedFile
                        },
                        &edit.file,
                        edit.buffer_id,
                        byte_ranges,
                    ));
                }
                for operation in &payload.file_operations {
                    match operation {
                        devil_protocol::WorkspaceFileOperation::Create { path, .. } => {
                            visitor(Self::path_target(
                                format!("workspace-edit:create:path:{}", path.0),
                                ProposalTargetKind::PathOnly,
                                path.clone(),
                                Vec::new(),
                            ))
                        }
                        devil_protocol::WorkspaceFileOperation::Delete { file } => {
                            visitor(Self::file_identity_target(
                                format!("workspace-edit:delete:file:{}", file.file_id.0),
                                ProposalTargetKind::ClosedFile,
                                file,
                                None,
                                Vec::new(),
                            ))
                        }
                        devil_protocol::WorkspaceFileOperation::Rename { file, destination } => {
                            visitor(Self::file_identity_target(
                                format!("workspace-edit:rename:source:{}", file.file_id.0),
                                ProposalTargetKind::ClosedFile,
                                file,
                                None,
                                Vec::new(),
                            ));
                            visitor(Self::path_target(
                                format!("workspace-edit:rename:destination:{}", destination.0),
                                ProposalTargetKind::PathOnly,
                                destination.clone(),
                                Vec::new(),
                            ));
                        }
                    }
                }
                if payload.file_edits.is_empty() && payload.file_operations.is_empty() {
                    for target in payload
                        .target_coverage
                        .targets
                        .iter()
                        .filter(|target| target.kind == ProposalTargetKind::MetadataOnly)
                    {
                        visitor(target.clone());
                    }
                }
            }
            ProposalPayload::Batch(payload) => {
                let mut items = payload.items.iter().collect::<Vec<_>>();
                items.sort_by(|left, right| {
                    left.order
                        .cmp(&right.order)
                        .then_with(|| left.item_id.cmp(&right.item_id))
                });
                for item in items {
                    Self::visit_inferred_payload_targets(item.payload.as_ref(), visitor);
                }
                if payload.items.is_empty() {
                    for target in payload
                        .target_coverage
                        .targets
                        .iter()
                        .filter(|target| target.kind == ProposalTargetKind::MetadataOnly)
                    {
                        visitor(target.clone());
                    }
                }
            }
        }
    }

    fn file_identity_target(
        target_id: String,
        kind: ProposalTargetKind,
        file: &FileIdentity,
        buffer_id: Option<BufferId>,
        byte_ranges: Vec<devil_protocol::ByteRange>,
    ) -> ProposalAffectedTarget {
        ProposalAffectedTarget {
            target_id,
            kind,
            workspace_id: Some(file.workspace_id),
            file_id: Some(file.file_id),
            buffer_id,
            path: Some(file.canonical_path.clone()),
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        }
    }

    fn path_target(
        target_id: String,
        kind: ProposalTargetKind,
        path: CanonicalPath,
        byte_ranges: Vec<devil_protocol::ByteRange>,
    ) -> ProposalAffectedTarget {
        ProposalAffectedTarget {
            target_id,
            kind,
            workspace_id: None,
            file_id: None,
            buffer_id: None,
            path: Some(path),
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        }
    }

    fn unsupported_response(&self, proposal: &WorkspaceProposal, action: &str) -> ProposalResponse {
        let coverage = Self::affected_target_coverage(proposal);
        let route = ProposalExecutionRoute::for_payload(&proposal.payload, &coverage);
        let first_path = coverage
            .targets
            .iter()
            .find_map(|target| target.path.clone());
        let unsupported_message = if matches!(proposal.payload, ProposalPayload::SaveFile(_))
            && action == "apply"
        {
            "generic save apply is denied until it can reuse the app/editor/workspace save workflow context; use AppComposition::save_active_buffer for proposal-mediated saves".to_string()
        } else {
            format!(
                "proposal {action} for route {route:?} over {} affected target(s) is denied until generalized execution is implemented",
                coverage.targets.len()
            )
        };
        let mut diagnostics = vec![Self::diagnostic(
            "proposal.unsupported_execution",
            unsupported_message,
        )];
        diagnostics[0].path = first_path;
        if !self.has_lifecycle_context(proposal.proposal_id) {
            diagnostics.push(Self::diagnostic(
                "proposal.missing_lifecycle_context",
                format!(
                    "proposal {action} has no app-created lifecycle context; unsupported route remains fail-closed"
                ),
            ));
        }

        let transition = if self.has_lifecycle_context(proposal.proposal_id) {
            match self.record_transition_with_diagnostics(
                proposal,
                ProposalLifecycleState::Rejected,
                action,
                diagnostics,
            ) {
                Ok(transition) => transition,
                Err(response) => return response,
            }
        } else {
            self.transition(proposal, ProposalLifecycleState::Rejected, diagnostics)
        };

        ProposalResponse::Rejected {
            transition,
            reason: ProposalRejectionReason::Unsupported,
        }
    }

    fn push_common_validation_diagnostics(
        proposal: &WorkspaceProposal,
        coverage: &ProposalTargetCoverage,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        if proposal.principal.0.trim().is_empty() {
            diagnostics.push(Self::diagnostic(
                "proposal.missing_principal",
                "proposal requires a non-empty principal",
            ));
        }
        if proposal.capability.0.trim().is_empty() {
            diagnostics.push(Self::diagnostic(
                "proposal.missing_capability",
                "proposal requires a non-empty capability",
            ));
        }
        if proposal.correlation_id.0 == 0 {
            diagnostics.push(Self::diagnostic(
                "proposal.zero_correlation_id",
                "proposal requires a non-zero correlation id",
            ));
        }
        if proposal.preview.summary.trim().is_empty() {
            diagnostics.push(Self::diagnostic(
                "proposal.missing_preview",
                "proposal requires a metadata-only preview summary",
            ));
        }
        if coverage.coverage_kind != ProposalTargetCoverageKind::Complete {
            diagnostics.push(Self::diagnostic(
                "proposal.incomplete_target_coverage",
                "proposal validation requires complete affected-target coverage before apply can be considered",
            ));
        }
        if coverage.omitted_target_count != 0 {
            diagnostics.push(Self::diagnostic(
                "proposal.omitted_target_coverage",
                "proposal validation requires zero omitted affected targets",
            ));
        }
        if coverage.targets.is_empty() {
            diagnostics.push(Self::diagnostic(
                "proposal.missing_targets",
                "proposal validation requires at least one affected target",
            ));
        }
    }

    fn push_target_validation_diagnostics(
        proposal: &WorkspaceProposal,
        coverage: &ProposalTargetCoverage,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        let mut target_ids = HashSet::new();
        let mut resource_keys = HashSet::new();
        for target in &coverage.targets {
            Self::push_single_target_validation_diagnostics(target, diagnostics);
            if !target.target_id.trim().is_empty() && !target_ids.insert(target.target_id.as_str())
            {
                diagnostics.push(Self::diagnostic(
                    "proposal.duplicate_target",
                    format!("affected target id '{}' is duplicated", target.target_id),
                ));
            }
            if let Some(resource_key) = Self::target_resource_key(target)
                && !resource_keys.insert(resource_key.clone())
            {
                diagnostics.push(Self::diagnostic(
                    "proposal.duplicate_target",
                    format!("affected target resource '{resource_key}' is duplicated"),
                ));
            }
        }

        match &proposal.payload {
            ProposalPayload::WorkspaceEdit(payload) => {
                Self::push_declared_coverage_matches_payload(
                    &payload.target_coverage,
                    &Self::inferred_targets(&proposal.payload),
                    diagnostics,
                    "workspace-edit proposal",
                );
            }
            ProposalPayload::Batch(payload) => {
                Self::push_batch_coverage_validation_diagnostics(payload, coverage, diagnostics);
            }
            _ => {}
        }
    }

    fn push_single_target_validation_diagnostics(
        target: &ProposalAffectedTarget,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        if target.target_id.trim().is_empty() {
            diagnostics.push(Self::diagnostic(
                "proposal.unknown_target",
                "affected target requires a stable non-empty target id",
            ));
        }
        if target.workspace_id == Some(WorkspaceId(0)) {
            diagnostics.push(Self::diagnostic(
                "proposal.unknown_workspace_target",
                format!("target {} has an unknown workspace id", target.target_id),
            ));
        }
        if target.file_id == Some(FileId(0)) {
            diagnostics.push(Self::diagnostic(
                "proposal.unknown_file_target",
                format!("target {} has an unknown file id", target.target_id),
            ));
        }
        if target.buffer_id == Some(BufferId(0)) {
            diagnostics.push(Self::diagnostic(
                "proposal.unknown_buffer_target",
                format!("target {} has an unknown buffer id", target.target_id),
            ));
        }
        if target.terminal_session_id == Some(devil_protocol::TerminalSessionId(0)) {
            diagnostics.push(Self::diagnostic(
                "proposal.unknown_terminal_target",
                format!(
                    "target {} has an unknown terminal session id",
                    target.target_id
                ),
            ));
        }
        if target
            .path
            .as_ref()
            .is_some_and(|path| path.0.trim().is_empty())
        {
            diagnostics.push(Self::diagnostic(
                "proposal.unknown_path_target",
                format!("target {} has an empty path", target.target_id),
            ));
        }

        let mut category_fields = 0;
        category_fields += usize::from(target.file_id.is_some() || target.buffer_id.is_some());
        category_fields += usize::from(target.terminal_session_id.is_some());
        category_fields += usize::from(target.plugin_id.is_some());
        category_fields += usize::from(target.remote_authority.is_some());
        category_fields += usize::from(target.collaboration_session_id.is_some());
        if category_fields > 1 {
            diagnostics.push(Self::diagnostic(
                "proposal.ambiguous_target",
                format!(
                    "target {} mixes multiple target authority categories",
                    target.target_id
                ),
            ));
        }

        match target.kind {
            ProposalTargetKind::OpenBuffer => {
                if target.file_id.is_none() {
                    diagnostics.push(Self::diagnostic(
                        "proposal.unknown_file_target",
                        format!("open-buffer target {} requires a file id", target.target_id),
                    ));
                }
            }
            ProposalTargetKind::ClosedFile => {
                if target.workspace_id.is_none()
                    || target.file_id.is_none()
                    || target.path.is_none()
                {
                    diagnostics.push(Self::diagnostic(
                        "proposal.missing_file_identity_target",
                        format!(
                            "closed-file target {} requires workspace id, file id, and canonical path from file identity",
                            target.target_id
                        ),
                    ));
                }
            }
            ProposalTargetKind::PathOnly => {
                if target.path.is_none() {
                    diagnostics.push(Self::diagnostic(
                        "proposal.unknown_path_target",
                        format!(
                            "path-only target {} requires a canonical path",
                            target.target_id
                        ),
                    ));
                }
            }
            ProposalTargetKind::TerminalSession => {
                if target.target_id == "terminal:new" && target.path.is_none() {
                    diagnostics.push(Self::diagnostic(
                        "proposal.unknown_terminal_target",
                        "new terminal target requires a working-directory path for deterministic validation",
                    ));
                }
            }
            ProposalTargetKind::MetadataOnly => {
                if target.file_id.is_some()
                    || target.buffer_id.is_some()
                    || target.path.is_some()
                    || target.terminal_session_id.is_some()
                    || target.plugin_id.is_some()
                    || target.remote_authority.is_some()
                    || target.collaboration_session_id.is_some()
                {
                    diagnostics.push(Self::diagnostic(
                        "proposal.ambiguous_target",
                        format!(
                            "metadata-only target {} must not carry mutable authority fields",
                            target.target_id
                        ),
                    ));
                }
            }
            ProposalTargetKind::RemoteWorkspace
            | ProposalTargetKind::CollaborationSession
            | ProposalTargetKind::Plugin => {
                diagnostics.push(Self::diagnostic(
                    "proposal.unsupported_target_kind",
                    format!(
                        "target {} kind {:?} is unsupported by app proposal validation",
                        target.target_id, target.kind
                    ),
                ));
            }
        }
    }

    fn target_resource_key(target: &ProposalAffectedTarget) -> Option<String> {
        match target.kind {
            ProposalTargetKind::OpenBuffer | ProposalTargetKind::ClosedFile => {
                target.file_id.map(|file_id| {
                    format!(
                        "file:{}:{}",
                        target.workspace_id.map_or(0, |id| id.0),
                        file_id.0
                    )
                })
            }
            ProposalTargetKind::PathOnly => {
                target.path.as_ref().map(|path| format!("path:{}", path.0))
            }
            ProposalTargetKind::TerminalSession => Some(format!(
                "terminal:{}:{}",
                target.terminal_session_id.map_or(0, |id| id.0),
                target.path.as_ref().map_or("", |path| path.0.as_str())
            )),
            ProposalTargetKind::MetadataOnly => Some(format!("metadata:{}", target.target_id)),
            ProposalTargetKind::RemoteWorkspace => target
                .remote_authority
                .as_ref()
                .map(|authority| format!("remote:{authority}")),
            ProposalTargetKind::CollaborationSession => target
                .collaboration_session_id
                .as_ref()
                .map(|session| format!("collaboration:{session}")),
            ProposalTargetKind::Plugin => target
                .plugin_id
                .map(|plugin_id| format!("plugin:{}", plugin_id.0)),
        }
    }

    fn target_resource_keys(targets: &[ProposalAffectedTarget]) -> HashSet<String> {
        targets
            .iter()
            .filter_map(Self::target_resource_key)
            .collect()
    }

    fn push_declared_coverage_matches_payload(
        declared: &ProposalTargetCoverage,
        inferred: &[ProposalAffectedTarget],
        diagnostics: &mut Vec<ProtocolDiagnostic>,
        context: &str,
    ) {
        if !Self::coverage_is_declared(declared) {
            return;
        }

        let declared_keys = Self::target_resource_keys(&declared.targets);
        let inferred_keys = Self::target_resource_keys(inferred);
        if declared_keys != inferred_keys
            && !Self::target_sets_equivalent(&declared.targets, inferred)
        {
            diagnostics.push(Self::diagnostic(
                "proposal.target_coverage_mismatch",
                format!(
                    "{context} declared target coverage does not exactly match discovered payload targets"
                ),
            ));
        }
    }

    fn push_batch_coverage_validation_diagnostics(
        payload: &BatchProposalPayload,
        coverage: &ProposalTargetCoverage,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        let inferred = Self::inferred_targets(&ProposalPayload::Batch(payload.clone()));
        Self::push_declared_coverage_matches_payload(
            coverage,
            &inferred,
            diagnostics,
            "batch proposal",
        );

        let coverage_by_id = coverage
            .targets
            .iter()
            .map(|target| (target.target_id.as_str(), target))
            .collect::<HashMap<_, _>>();
        let mut item_ids = HashSet::new();
        for item in &payload.items {
            if !item.item_id.trim().is_empty() && !item_ids.insert(item.item_id.as_str()) {
                diagnostics.push(Self::diagnostic(
                    "proposal.duplicate_batch_item",
                    format!("batch item id '{}' is duplicated", item.item_id),
                ));
            }

            let route = Self::batch_item_validation_route(item.payload.as_ref());
            if !matches!(
                route,
                BatchPreflightRoute::TextEdit
                    | BatchPreflightRoute::CreateFile
                    | BatchPreflightRoute::DeleteFile
                    | BatchPreflightRoute::RenameFile
            ) {
                diagnostics.push(Self::diagnostic(
                    "proposal.unsupported_batch_item_route",
                    format!(
                        "batch item {} route {:?} is unsupported and validation fails closed",
                        item.item_id, route
                    ),
                ));
            }

            let mut item_target_ids = HashSet::new();
            for target_id in &item.target_ids {
                if target_id.trim().is_empty() || !item_target_ids.insert(target_id.as_str()) {
                    diagnostics.push(Self::diagnostic(
                        "proposal.duplicate_batch_item_target",
                        format!(
                            "batch item {} has an empty or duplicated target id",
                            item.item_id
                        ),
                    ));
                }
                if !coverage_by_id.contains_key(target_id.as_str()) {
                    diagnostics.push(Self::diagnostic(
                        "proposal.unknown_batch_target",
                        format!(
                            "batch item {} references unknown target id {}",
                            item.item_id, target_id
                        ),
                    ));
                }
            }

            let declared_targets = item
                .target_ids
                .iter()
                .filter_map(|target_id| coverage_by_id.get(target_id.as_str()).copied())
                .cloned()
                .collect::<Vec<_>>();
            let inferred_item_targets = Self::inferred_targets(item.payload.as_ref());
            if Self::target_resource_keys(&declared_targets)
                != Self::target_resource_keys(&inferred_item_targets)
                && !Self::target_sets_equivalent(&declared_targets, &inferred_item_targets)
            {
                diagnostics.push(Self::diagnostic(
                    "proposal.batch_item_target_coverage_mismatch",
                    format!(
                        "batch item {} target ids do not exactly cover its discovered payload targets",
                        item.item_id
                    ),
                ));
            }
        }
    }

    fn target_sets_equivalent(
        declared: &[ProposalAffectedTarget],
        inferred: &[ProposalAffectedTarget],
    ) -> bool {
        declared.len() == inferred.len()
            && inferred.iter().all(|inferred_target| {
                declared.iter().any(|declared_target| {
                    Self::targets_equivalent(declared_target, inferred_target)
                })
            })
            && declared.iter().all(|declared_target| {
                inferred.iter().any(|inferred_target| {
                    Self::targets_equivalent(declared_target, inferred_target)
                })
            })
    }

    fn targets_equivalent(left: &ProposalAffectedTarget, right: &ProposalAffectedTarget) -> bool {
        match (left.kind, right.kind) {
            (
                ProposalTargetKind::OpenBuffer | ProposalTargetKind::ClosedFile,
                ProposalTargetKind::OpenBuffer | ProposalTargetKind::ClosedFile,
            ) => {
                left.file_id == right.file_id
                    && (left.workspace_id.is_none()
                        || right.workspace_id.is_none()
                        || left.workspace_id == right.workspace_id)
            }
            (ProposalTargetKind::PathOnly, ProposalTargetKind::PathOnly) => left.path == right.path,
            (ProposalTargetKind::TerminalSession, ProposalTargetKind::TerminalSession) => {
                left.terminal_session_id == right.terminal_session_id && left.path == right.path
            }
            (ProposalTargetKind::MetadataOnly, ProposalTargetKind::MetadataOnly) => {
                left.target_id == right.target_id
            }
            (ProposalTargetKind::RemoteWorkspace, ProposalTargetKind::RemoteWorkspace) => {
                left.remote_authority == right.remote_authority
            }
            (
                ProposalTargetKind::CollaborationSession,
                ProposalTargetKind::CollaborationSession,
            ) => left.collaboration_session_id == right.collaboration_session_id,
            (ProposalTargetKind::Plugin, ProposalTargetKind::Plugin) => {
                left.plugin_id == right.plugin_id
            }
            _ => false,
        }
    }

    fn batch_item_validation_route(payload: &ProposalPayload) -> BatchPreflightRoute {
        match payload {
            ProposalPayload::TextEdit(_) => BatchPreflightRoute::TextEdit,
            ProposalPayload::CreateFile(_) => BatchPreflightRoute::CreateFile,
            ProposalPayload::DeleteFile(_) => BatchPreflightRoute::DeleteFile,
            ProposalPayload::RenameFile(_) => BatchPreflightRoute::RenameFile,
            ProposalPayload::Batch(_) => BatchPreflightRoute::Batch,
            ProposalPayload::TerminalCommand(_) => BatchPreflightRoute::Terminal,
            ProposalPayload::SaveFile(_) => BatchPreflightRoute::Save,
            ProposalPayload::FormatFile(_) => BatchPreflightRoute::Format,
            ProposalPayload::CodeAction(_) => BatchPreflightRoute::CodeAction,
            ProposalPayload::WorkspaceEdit(_) => BatchPreflightRoute::WorkspaceEdit,
        }
    }

    fn missing_file_version(preconditions: &ProposalVersionPreconditions) -> bool {
        preconditions.file_content_version.is_none() && preconditions.file_version.is_none()
    }

    fn missing_workspace_generation(preconditions: &ProposalVersionPreconditions) -> bool {
        preconditions.workspace_generation.is_none() && preconditions.generation.is_none()
    }

    fn push_file_precondition_diagnostics(
        preconditions: &ProposalVersionPreconditions,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
        context: &str,
        require_buffer_snapshot: bool,
        require_fingerprint: bool,
    ) {
        if Self::missing_file_version(preconditions)
            || Self::missing_workspace_generation(preconditions)
            || (require_fingerprint && preconditions.expected_fingerprint.is_none())
        {
            diagnostics.push(Self::diagnostic(
                "proposal.missing_file_precondition",
                format!(
                    "{context} requires file content version, workspace generation, and expected fingerprint preconditions"
                ),
            ));
        }
        if require_buffer_snapshot
            && (preconditions.buffer_version.is_none() || preconditions.snapshot_id.is_none())
        {
            diagnostics.push(Self::diagnostic(
                "proposal.missing_buffer_precondition",
                format!("{context} requires buffer version and snapshot preconditions"),
            ));
        }
    }

    fn push_capability_diagnostic(
        actual: &CapabilityId,
        expected: &str,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
        context: &str,
    ) {
        if actual.0 != expected {
            diagnostics.push(Self::diagnostic(
                "proposal.invalid_capability",
                format!("{context} requires {expected} capability"),
            ));
        }
    }

    fn push_payload_validation_diagnostics(
        proposal: &WorkspaceProposal,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        match &proposal.payload {
            ProposalPayload::TextEdit(payload) => {
                Self::push_capability_diagnostic(
                    &proposal.capability,
                    "editor.write",
                    diagnostics,
                    "text edit proposal",
                );
                if payload.edits.edits.is_empty() {
                    diagnostics.push(Self::diagnostic(
                        "proposal.empty_edit_batch",
                        "text edit proposal requires at least one edit",
                    ));
                }
                Self::push_file_precondition_diagnostics(
                    &proposal.preconditions,
                    diagnostics,
                    "text edit proposal",
                    true,
                    false,
                );
            }
            ProposalPayload::CreateFile(payload) => {
                Self::push_capability_diagnostic(
                    &proposal.capability,
                    "fs.write",
                    diagnostics,
                    "create-file proposal",
                );
                if payload.path.0.trim().is_empty() {
                    diagnostics.push(Self::diagnostic(
                        "proposal.empty_path",
                        "create-file proposal requires a destination path",
                    ));
                }
                if Self::missing_workspace_generation(&proposal.preconditions) {
                    diagnostics.push(Self::diagnostic(
                        "proposal.missing_workspace_precondition",
                        "create-file proposal requires workspace generation preconditions",
                    ));
                }
            }
            ProposalPayload::DeleteFile(payload) => {
                Self::push_capability_diagnostic(
                    &proposal.capability,
                    "fs.write",
                    diagnostics,
                    "delete-file proposal",
                );
                if payload.file.canonical_path.0.trim().is_empty() {
                    diagnostics.push(Self::diagnostic(
                        "proposal.empty_path",
                        "delete-file proposal requires a file path",
                    ));
                }
                Self::push_file_precondition_diagnostics(
                    &proposal.preconditions,
                    diagnostics,
                    "delete-file proposal",
                    false,
                    true,
                );
            }
            ProposalPayload::RenameFile(payload) => {
                Self::push_capability_diagnostic(
                    &proposal.capability,
                    "fs.write",
                    diagnostics,
                    "rename-file proposal",
                );
                if payload.file.canonical_path.0.trim().is_empty()
                    || payload.destination.0.trim().is_empty()
                {
                    diagnostics.push(Self::diagnostic(
                        "proposal.empty_path",
                        "rename-file proposal requires source and destination paths",
                    ));
                }
                Self::push_file_precondition_diagnostics(
                    &proposal.preconditions,
                    diagnostics,
                    "rename-file proposal",
                    false,
                    true,
                );
            }
            ProposalPayload::SaveFile(save) => {
                if save.expected_fingerprint.is_none()
                    || proposal.preconditions.expected_fingerprint.is_none()
                {
                    diagnostics.push(Self::diagnostic(
                        "proposal.missing_fingerprint",
                        "save proposal requires expected disk fingerprint",
                    ));
                }
                if proposal.preconditions.file_content_version.is_none()
                    || proposal.preconditions.workspace_generation.is_none()
                    || proposal.preconditions.buffer_version.is_none()
                    || proposal.preconditions.snapshot_id.is_none()
                {
                    diagnostics.push(Self::diagnostic(
                        "proposal.missing_precondition",
                        "save proposal requires file, workspace, buffer, and snapshot preconditions",
                    ));
                }
                if save.required_capability.0 != "fs.write" || proposal.capability.0 != "fs.write" {
                    diagnostics.push(Self::diagnostic(
                        "proposal.invalid_capability",
                        "save proposal requires fs.write capability",
                    ));
                }
                if save.principal != proposal.principal
                    || save.correlation_id != proposal.correlation_id
                {
                    diagnostics.push(Self::diagnostic(
                        "proposal.context_mismatch",
                        "save proposal payload context must match proposal envelope",
                    ));
                }
            }
            ProposalPayload::FormatFile(payload) => {
                Self::push_capability_diagnostic(
                    &proposal.capability,
                    "editor.write",
                    diagnostics,
                    "format-file proposal",
                );
                if payload.snapshot_id.0 == 0 {
                    diagnostics.push(Self::diagnostic(
                        "proposal.zero_snapshot_id",
                        "format-file proposal requires a non-zero snapshot id",
                    ));
                }
                Self::push_file_precondition_diagnostics(
                    &proposal.preconditions,
                    diagnostics,
                    "format-file proposal",
                    true,
                    false,
                );
            }
            ProposalPayload::CodeAction(payload) => {
                Self::push_capability_diagnostic(
                    &proposal.capability,
                    "editor.write",
                    diagnostics,
                    "code-action proposal",
                );
                if payload.title.trim().is_empty() || payload.edits.is_empty() {
                    diagnostics.push(Self::diagnostic(
                        "proposal.empty_code_action",
                        "code-action proposal requires a title and at least one edit",
                    ));
                }
                Self::push_file_precondition_diagnostics(
                    &proposal.preconditions,
                    diagnostics,
                    "code-action proposal",
                    true,
                    false,
                );
            }
            ProposalPayload::WorkspaceEdit(payload) => {
                if payload.schema_version == 0 {
                    diagnostics.push(Self::diagnostic(
                        "proposal.invalid_schema_version",
                        "workspace-edit proposal requires a non-zero schema version",
                    ));
                }
                if payload.required_capability != proposal.capability {
                    diagnostics.push(Self::diagnostic(
                        "proposal.context_mismatch",
                        "workspace-edit required capability must match the proposal envelope",
                    ));
                }
                if payload.target_coverage.coverage_kind != ProposalTargetCoverageKind::Complete
                    || payload.target_coverage.targets.is_empty()
                {
                    diagnostics.push(Self::diagnostic(
                        "proposal.incomplete_target_coverage",
                        "workspace-edit proposal requires complete target coverage",
                    ));
                }
                if payload.file_edits.is_empty() && payload.file_operations.is_empty() {
                    diagnostics.push(Self::diagnostic(
                        "proposal.empty_workspace_edit",
                        "workspace-edit proposal requires at least one text edit or file operation",
                    ));
                }
                for edit in &payload.file_edits {
                    Self::push_file_precondition_diagnostics(
                        &edit.preconditions,
                        diagnostics,
                        "workspace text edit",
                        edit.buffer_id.is_some(),
                        true,
                    );
                }
                for operation in &payload.file_operations {
                    match operation {
                        devil_protocol::WorkspaceFileOperation::Create { .. } => {
                            if Self::missing_workspace_generation(&proposal.preconditions) {
                                diagnostics.push(Self::diagnostic(
                                    "proposal.missing_workspace_precondition",
                                    "workspace-edit create operation requires workspace generation precondition",
                                ));
                            }
                        }
                        devil_protocol::WorkspaceFileOperation::Delete { .. }
                        | devil_protocol::WorkspaceFileOperation::Rename { .. } => {
                            Self::push_file_precondition_diagnostics(
                                &proposal.preconditions,
                                diagnostics,
                                "workspace-edit file operation",
                                false,
                                true,
                            );
                        }
                    }
                }
                if payload.source == devil_protocol::WorkspaceEditSourceKind::Plugin {
                    diagnostics.push(Self::diagnostic(
                        "proposal.plugin_source_denied",
                        "plugin-produced workspace edits remain denied until plugin activation gates exist",
                    ));
                }
            }
            ProposalPayload::TerminalCommand(_) => {}
            ProposalPayload::Batch(payload) => {
                if payload.schema_version == 0 {
                    diagnostics.push(Self::diagnostic(
                        "proposal.invalid_schema_version",
                        "batch proposal requires a non-zero schema version",
                    ));
                }
                if payload.items.is_empty() {
                    diagnostics.push(Self::diagnostic(
                        "proposal.empty_batch",
                        "batch proposal requires at least one item",
                    ));
                }
                if payload.target_coverage.coverage_kind != ProposalTargetCoverageKind::Complete
                    || payload.target_coverage.targets.is_empty()
                {
                    diagnostics.push(Self::diagnostic(
                        "proposal.incomplete_target_coverage",
                        "batch proposal requires complete target coverage",
                    ));
                }
                if payload.rollback_policy == devil_protocol::ProposalBatchRollbackPolicy::Required
                    && payload.rollback_steps.is_empty()
                {
                    diagnostics.push(Self::diagnostic(
                        "proposal.missing_rollback_plan",
                        "batch proposal with required rollback policy needs rollback steps before apply can be considered",
                    ));
                }
                if payload.rollback_policy
                    == devil_protocol::ProposalBatchRollbackPolicy::NotSupported
                    && payload.atomicity != devil_protocol::ProposalBatchAtomicity::OrderedNonAtomic
                {
                    diagnostics.push(Self::diagnostic(
                        "proposal.unsupported_rollback_policy",
                        "batch proposal cannot promise atomic execution while rollback is unsupported",
                    ));
                }
                for item in &payload.items {
                    if item.item_id.trim().is_empty()
                        || item.required_capability.0.trim().is_empty()
                    {
                        diagnostics.push(Self::diagnostic(
                            "proposal.invalid_batch_item",
                            "batch items require stable identifiers and capabilities",
                        ));
                    }
                    if payload.rollback_policy
                        == devil_protocol::ProposalBatchRollbackPolicy::Required
                        && item.rollback_step_ids.is_empty()
                    {
                        diagnostics.push(Self::diagnostic(
                            "proposal.missing_rollback_plan",
                            format!(
                                "batch item {} requires rollback step references",
                                item.item_id
                            ),
                        ));
                    }
                }
            }
        }
    }

    fn validate_proposal(&self, proposal: &WorkspaceProposal) -> ProposalResponse {
        let coverage = Self::affected_target_coverage(proposal);
        let route = ProposalExecutionRoute::for_payload(&proposal.payload, &coverage);

        let mut diagnostics = Vec::new();
        Self::push_common_validation_diagnostics(proposal, &coverage, &mut diagnostics);
        Self::push_target_validation_diagnostics(proposal, &coverage, &mut diagnostics);
        Self::push_payload_validation_diagnostics(proposal, &mut diagnostics);

        if matches!(
            route,
            ProposalExecutionRoute::Terminal
                | ProposalExecutionRoute::Unsupported
                | ProposalExecutionRoute::Mixed
        ) && diagnostics.is_empty()
        {
            return self.unsupported_response(proposal, "validate");
        }

        if diagnostics.is_empty() {
            match self.record_transition(proposal, ProposalLifecycleState::Validated, "validate") {
                Ok(transition) => ProposalResponse::Validated(transition),
                Err(response) => response,
            }
        } else {
            match self.record_transition_with_diagnostics(
                proposal,
                ProposalLifecycleState::Denied,
                "validate",
                diagnostics,
            ) {
                Ok(transition) => ProposalResponse::Denied {
                    transition,
                    reason: ProposalDenialReason::PolicyDenied,
                },
                Err(response) => response,
            }
        }
    }

    fn transition_for_command(
        &self,
        command: &ProposalLifecycleCommand,
        lifecycle_state: ProposalLifecycleState,
        diagnostics: Vec<ProtocolDiagnostic>,
    ) -> ProposalLifecycleTransition {
        let context = self
            .proposal_contexts
            .borrow()
            .get(&command.proposal_id)
            .copied();
        let correlation_id = (command.correlation_id.0 != 0)
            .then_some(command.correlation_id)
            .or_else(|| {
                context
                    .map(|context| context.correlation_id)
                    .filter(|correlation_id| correlation_id.0 != 0)
            })
            .unwrap_or(CorrelationId(1));
        let causality_id = (!command.causality_id.0.is_nil())
            .then_some(command.causality_id)
            .or_else(|| {
                context
                    .map(|context| context.causality_id)
                    .filter(|causality_id| !causality_id.0.is_nil())
            })
            .unwrap_or_else(|| CausalityId(uuid::Uuid::now_v7()));
        ProposalLifecycleTransition {
            proposal_id: command.proposal_id,
            lifecycle_state,
            timestamp: TimestampMillis::now(),
            principal: command.principal.clone(),
            capability: command.capability.clone(),
            correlation_id,
            causality_id,
            diagnostics,
        }
    }

    #[allow(clippy::result_large_err)]
    fn record_command_transition(
        &self,
        command: &ProposalLifecycleCommand,
        lifecycle_state: ProposalLifecycleState,
        action: &str,
    ) -> Result<ProposalLifecycleTransition, ProposalResponse> {
        let current = self.current_lifecycle_state(command.proposal_id);
        if current.is_none() {
            return Err(self.missing_lifecycle_command_response(command, action));
        }

        let context_diagnostics = self.lifecycle_command_context_diagnostics(command, action);
        if !context_diagnostics.is_empty() {
            return Err(self.invalid_lifecycle_command_context_response(
                command,
                action,
                context_diagnostics,
            ));
        }

        match self.record_lifecycle_state(command.proposal_id, lifecycle_state) {
            Ok(()) => Ok(self.transition_for_command(
                command,
                lifecycle_state,
                command.diagnostics.clone(),
            )),
            Err(current) => Err(self.invalid_lifecycle_command_response(
                command,
                action,
                current,
                lifecycle_state,
            )),
        }
    }

    fn lifecycle_command_context_diagnostics(
        &self,
        command: &ProposalLifecycleCommand,
        action: &str,
    ) -> Vec<ProtocolDiagnostic> {
        let mut diagnostics = Vec::new();
        if command.correlation_id.0 == 0 {
            diagnostics.push(Self::diagnostic(
                "proposal.command_zero_correlation_id",
                "proposal lifecycle command requires a non-zero correlation id",
            ));
        }
        if command.causality_id.0.is_nil() {
            diagnostics.push(Self::diagnostic(
                "proposal.command_nil_causality_id",
                "proposal lifecycle command requires a non-nil causality id",
            ));
        }
        if !Self::command_action_matches_request(command, action) {
            diagnostics.push(Self::diagnostic(
                "proposal.lifecycle_command_action_mismatch",
                format!(
                    "proposal command action {:?} does not match request action {action}",
                    command.action
                ),
            ));
        }
        if let Some(context) = self.proposal_contexts.borrow().get(&command.proposal_id) {
            if context.correlation_id.0 == 0 {
                diagnostics.push(Self::diagnostic(
                    "proposal.lifecycle_context_zero_correlation_id",
                    "proposal lifecycle context requires a non-zero correlation id",
                ));
            }
            if context.causality_id.0.is_nil() {
                diagnostics.push(Self::diagnostic(
                    "proposal.lifecycle_context_nil_causality_id",
                    "proposal lifecycle context requires a non-nil causality id",
                ));
            }
        }
        diagnostics
    }

    fn command_action_matches_request(command: &ProposalLifecycleCommand, action: &str) -> bool {
        matches!(
            (command.action, action),
            (devil_protocol::ProposalLifecycleAction::Approve, "approve")
                | (devil_protocol::ProposalLifecycleAction::Reject, "reject")
                | (devil_protocol::ProposalLifecycleAction::Cancel, "cancel")
                | (
                    devil_protocol::ProposalLifecycleAction::Rollback,
                    "rollback"
                )
        )
    }

    fn invalid_lifecycle_command_context_response(
        &self,
        command: &ProposalLifecycleCommand,
        action: &str,
        mut diagnostics: Vec<ProtocolDiagnostic>,
    ) -> ProposalResponse {
        diagnostics.insert(
            0,
            Self::diagnostic(
                "proposal.invalid_lifecycle_context",
                format!(
                    "proposal {action} requires non-zero correlation and non-nil causality lifecycle context"
                ),
            ),
        );
        ProposalResponse::Rejected {
            transition: self.transition_for_command(
                command,
                ProposalLifecycleState::Rejected,
                diagnostics,
            ),
            reason: ProposalRejectionReason::ValidationFailed,
        }
    }

    fn rejection_reason(command: &ProposalLifecycleCommand) -> ProposalRejectionReason {
        match command.reason.as_ref() {
            Some(ProposalLifecycleCommandReason::Rejection(reason)) => *reason,
            _ => ProposalRejectionReason::UserRejected,
        }
    }

    fn cancellation_reason(command: &ProposalLifecycleCommand) -> ProposalCancellationReason {
        match command.reason.as_ref() {
            Some(ProposalLifecycleCommandReason::Cancellation(reason)) => *reason,
            _ => ProposalCancellationReason::UserCancelled,
        }
    }

    fn rollback_reason(command: &ProposalLifecycleCommand) -> ProposalRollbackReason {
        match command.reason.as_ref() {
            Some(ProposalLifecycleCommandReason::Rollback(reason)) => *reason,
            _ => ProposalRollbackReason::UserRequested,
        }
    }
}

impl ProposalPort for AppProposalCoordinator {
    fn handle(&self, request: ProposalRequest) -> ProtocolResult<ProposalResponse> {
        match request {
            ProposalRequest::Validate(proposal) => Ok(self.validate_proposal(&proposal)),
            ProposalRequest::Preview(proposal) => {
                match self.record_transition(
                    &proposal,
                    ProposalLifecycleState::Previewed,
                    "preview",
                ) {
                    Ok(transition) => Ok(ProposalResponse::Previewed {
                        transition,
                        proposal: Box::new(proposal),
                    }),
                    Err(response) => Ok(response),
                }
            }
            ProposalRequest::Apply(proposal) => Ok(self.unsupported_response(&proposal, "apply")),
            ProposalRequest::Approve(command) => {
                match self.record_command_transition(
                    &command,
                    ProposalLifecycleState::Approved,
                    "approve",
                ) {
                    Ok(transition) => Ok(ProposalResponse::Approved(transition)),
                    Err(response) => Ok(response),
                }
            }
            ProposalRequest::Reject(command) => match self.record_command_transition(
                &command,
                ProposalLifecycleState::Rejected,
                "reject",
            ) {
                Ok(transition) => Ok(ProposalResponse::Rejected {
                    transition,
                    reason: Self::rejection_reason(&command),
                }),
                Err(response) => Ok(response),
            },
            ProposalRequest::Cancel(command) => match self.record_command_transition(
                &command,
                ProposalLifecycleState::Cancelled,
                "cancel",
            ) {
                Ok(transition) => Ok(ProposalResponse::Cancelled {
                    transition,
                    reason: Self::cancellation_reason(&command),
                }),
                Err(response) => Ok(response),
            },
            ProposalRequest::Rollback(command) => match self.record_command_transition(
                &command,
                ProposalLifecycleState::RolledBack,
                "rollback",
            ) {
                Ok(transition) => Ok(ProposalResponse::RolledBack {
                    transition,
                    reason: Self::rollback_reason(&command),
                }),
                Err(response) => Ok(response),
            },
        }
    }
}

#[derive(Debug, Default)]
struct CorrelationGenerator {
    next: u64,
}

impl CorrelationGenerator {
    fn next(&mut self) -> CorrelationId {
        self.next = self.next.saturating_add(1).max(1);
        CorrelationId(self.next)
    }
}

#[derive(Debug, Default)]
struct EventSequenceGenerator {
    next: u64,
}

impl EventSequenceGenerator {
    fn next(&mut self) -> EventSequence {
        self.next = self.next.saturating_add(1).max(1);
        EventSequence(self.next)
    }
}

#[derive(Debug)]
struct ActiveDocumentController {
    opened_workspace: Option<WorkspaceOpened>,
    workspace_root_path: Option<String>,
    active_principal_id: Option<PrincipalId>,
    active_workspace_trust: Option<WorkspaceTrustState>,
    active_file_id: Option<FileId>,
    active_file_path: Option<String>,
    active_buffer_id: Option<BufferId>,
    active_file_metadata: Option<ActiveFileMetadata>,
    buffer_file_metadata: HashMap<BufferId, ActiveFileMetadata>,
}

impl ActiveDocumentController {
    fn new() -> Self {
        Self {
            opened_workspace: None,
            workspace_root_path: None,
            active_principal_id: None,
            active_workspace_trust: None,
            active_file_id: None,
            active_file_path: None,
            active_buffer_id: None,
            active_file_metadata: None,
            buffer_file_metadata: HashMap::new(),
        }
    }

    fn workspace_id(&self) -> Option<WorkspaceId> {
        self.opened_workspace
            .as_ref()
            .map(|opened| opened.workspace_id)
    }

    fn bind_workspace(
        &mut self,
        opened: WorkspaceOpened,
        root_path: CanonicalPath,
        principal: PrincipalId,
        trust: WorkspaceTrustState,
    ) {
        self.opened_workspace = Some(opened);
        self.workspace_root_path = Some(root_path.0);
        self.active_principal_id = Some(principal);
        self.active_workspace_trust = Some(trust);
        self.clear_active_file();
    }

    fn clear_active_file(&mut self) {
        self.active_file_id = None;
        self.active_file_path = None;
        self.active_buffer_id = None;
        self.active_file_metadata = None;
        self.buffer_file_metadata.clear();
    }

    fn bind_opened_file(&mut self, opened: &OpenedFileText, buffer_id: BufferId) {
        let identity = opened.identity.clone();
        let metadata = ActiveFileMetadata {
            identity: identity.clone(),
            fingerprint: opened.fingerprint.clone(),
            file_content_version: opened.file_content_version,
            workspace_generation: opened.workspace_generation,
            modified_at: opened.modified_at,
            file_length: opened.file_length,
        };
        self.active_file_id = Some(identity.file_id);
        self.active_file_path = Some(identity.canonical_path.0.clone());
        self.active_buffer_id = Some(buffer_id);
        self.active_file_metadata = Some(metadata.clone());
        self.buffer_file_metadata.insert(buffer_id, metadata);
    }

    fn bind_saved_file(&mut self, applied: devil_project::WorkspaceSaveApplied) {
        let metadata = ActiveFileMetadata {
            identity: applied.identity.clone(),
            fingerprint: applied.fingerprint,
            file_content_version: applied.file_content_version,
            workspace_generation: applied.workspace_generation,
            modified_at: applied.modified_at,
            file_length: applied.file_length,
        };
        self.active_file_id = Some(applied.identity.file_id);
        self.active_file_path = Some(applied.identity.canonical_path.0.clone());
        self.active_file_metadata = Some(metadata.clone());
        if let Some(buffer_id) = self.active_buffer_id {
            self.buffer_file_metadata.insert(buffer_id, metadata);
        }
    }

    fn bind_saved_buffer(
        &mut self,
        buffer_id: BufferId,
        applied: devil_project::WorkspaceSaveApplied,
    ) {
        let metadata = ActiveFileMetadata {
            identity: applied.identity.clone(),
            fingerprint: applied.fingerprint,
            file_content_version: applied.file_content_version,
            workspace_generation: applied.workspace_generation,
            modified_at: applied.modified_at,
            file_length: applied.file_length,
        };
        if self.active_buffer_id == Some(buffer_id) {
            self.active_file_id = Some(applied.identity.file_id);
            self.active_file_path = Some(applied.identity.canonical_path.0.clone());
            self.active_file_metadata = Some(metadata.clone());
        }
        self.buffer_file_metadata.insert(buffer_id, metadata);
    }

    fn metadata_for_buffer(&self, buffer_id: BufferId) -> Option<&ActiveFileMetadata> {
        self.buffer_file_metadata.get(&buffer_id).or_else(|| {
            if self.active_buffer_id == Some(buffer_id) {
                self.active_file_metadata.as_ref()
            } else {
                None
            }
        })
    }

    fn ensure_active_buffer(&self, target: BufferId) -> Result<(), AppCompositionError> {
        if self.active_buffer_id == Some(target) {
            Ok(())
        } else {
            Err(AppCompositionError::BufferMismatch {
                target,
                active: self.active_buffer_id,
            })
        }
    }

    fn require_workspace_id(&self) -> Result<WorkspaceId, AppCompositionError> {
        self.workspace_id()
            .ok_or(AppCompositionError::WorkspaceNotOpen)
    }

    fn require_active_buffer(&self) -> Result<BufferId, AppCompositionError> {
        self.active_buffer_id
            .ok_or(AppCompositionError::ActiveBufferMissing)
    }

    fn require_active_save_context(&self) -> Result<ActiveSaveContext, AppCompositionError> {
        Ok(ActiveSaveContext {
            workspace_id: self.require_workspace_id()?,
            buffer_id: self.require_active_buffer()?,
            metadata: self
                .active_file_metadata
                .clone()
                .ok_or(AppCompositionError::ActiveFileMissing)?,
            principal: self
                .active_principal_id
                .clone()
                .ok_or(AppCompositionError::WorkspaceNotOpen)?,
            trust: self
                .active_workspace_trust
                .clone()
                .ok_or(AppCompositionError::WorkspaceNotOpen)?,
        })
    }
}

#[derive(Debug, Clone)]
struct ActiveSaveContext {
    workspace_id: WorkspaceId,
    buffer_id: BufferId,
    metadata: ActiveFileMetadata,
    principal: PrincipalId,
    trust: WorkspaceTrustState,
}

#[derive(Debug, Clone)]
struct DeferredSaveSuccess {
    request_id: uuid::Uuid,
    buffer_id: BufferId,
    applied: devil_project::WorkspaceSaveApplied,
}

#[derive(Debug, Clone)]
enum ProposalMutationRollback {
    None,
    TextEdit,
    CreatedFile {
        path: CanonicalPath,
    },
    DeletedFile {
        path: CanonicalPath,
        text: String,
    },
    RenamedFile {
        source: CanonicalPath,
        destination: CanonicalPath,
    },
    SavedFile {
        path: CanonicalPath,
        text: String,
    },
}

/// Port-shaped request emitted by the application command dispatcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCommandRequest {
    /// Command had no effect.
    Noop,
    /// Command requested shell termination.
    Quit,
    /// Undo the active buffer through editor authority.
    Undo {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Redo the active buffer through editor authority.
    Redo {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Apply a text edit through editor authority.
    ApplyEdit {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Editor edit in UI-projected text coordinates.
        edit: TextEdit,
    },
    /// Save the active buffer through editor/proposal/workspace authorities.
    Save {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Open a workspace path through workspace authority.
    OpenPath {
        /// User-provided path text.
        path: String,
    },
    /// Refresh explorer projection through workspace authority.
    RefreshExplorer,
    /// Reveal a workspace file in the explorer projection.
    RevealInExplorer {
        /// File identifier to reveal.
        file_id: FileId,
    },
}

/// Minimal editor command port used by app command routing.
pub trait AppEditorCommandPort {
    /// Apply a text edit through editor authority.
    fn apply_edit(
        &mut self,
        buffer_id: BufferId,
        edit: TextEdit,
    ) -> Result<TextTransactionDescriptor, AppCompositionError>;

    /// Undo a buffer through editor authority.
    fn undo(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<TextTransactionDescriptor, AppCompositionError>;

    /// Redo a buffer through editor authority.
    fn redo(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<TextTransactionDescriptor, AppCompositionError>;
}

impl AppEditorCommandPort for EditorEngine {
    fn apply_edit(
        &mut self,
        buffer_id: BufferId,
        edit: TextEdit,
    ) -> Result<TextTransactionDescriptor, AppCompositionError> {
        let record =
            EditorEngine::apply_edit(self, buffer_id, edit, TransactionSource::User, None, None)?;
        Ok(record.to_protocol_descriptor())
    }

    fn undo(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<TextTransactionDescriptor, AppCompositionError> {
        let record = EditorEngine::undo(self, buffer_id, None)?;
        Ok(record.to_protocol_descriptor())
    }

    fn redo(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<TextTransactionDescriptor, AppCompositionError> {
        let record = EditorEngine::redo(self, buffer_id, None)?;
        Ok(record.to_protocol_descriptor())
    }
}

/// Minimal workspace command port used by app command routing.
pub trait AppWorkspaceCommandPort {
    /// Open a file path through workspace authority and return workspace-opened text metadata.
    fn open_file_text(
        &self,
        workspace_id: WorkspaceId,
        path: &str,
        intent: OpenFileIntent,
        event_context: Option<EventContext>,
    ) -> Result<OpenedFileText, AppCompositionError>;

    /// Read a workspace tree snapshot through workspace authority.
    fn tree_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<FileTreeNode>, AppCompositionError>;
}

impl AppWorkspaceCommandPort for WorkspaceActor {
    fn open_file_text(
        &self,
        workspace_id: WorkspaceId,
        path: &str,
        intent: OpenFileIntent,
        event_context: Option<EventContext>,
    ) -> Result<OpenedFileText, AppCompositionError> {
        match intent {
            OpenFileIntent::Existing => Ok(self.open_existing_file_text_with_causality(
                workspace_id,
                path,
                event_context.map(|context| context.correlation_id),
                event_context.map(|context| context.causality_id),
            )?),
            OpenFileIntent::CreateNew => Ok(self.open_new_file_text(workspace_id, path)?),
        }
    }

    fn tree_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<FileTreeNode>, AppCompositionError> {
        match self
            .handle(WorkspaceRequest::ReadTree(workspace_id))
            .map_err(AppCompositionError::Protocol)?
        {
            WorkspaceResponse::Tree(tree) => Ok(tree),
            other => Err(AppCompositionError::Protocol(ProtocolError {
                code: "workspace_tree_unexpected_response".to_string(),
                message: format!("expected tree response, got {other:?}"),
            })),
        }
    }
}

/// Mutable application command state exposed to command-service tests without concrete app internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppCommandExecutionState {
    /// Active workspace identifier when a workspace is open.
    pub workspace_id: Option<WorkspaceId>,
    /// Active buffer identifier when a buffer is selected.
    pub active_buffer_id: Option<BufferId>,
    /// Active file identifier when a file is selected.
    pub active_file_id: Option<FileId>,
}

impl AppCommandExecutionState {
    fn from_active(active: &ActiveDocumentController) -> Self {
        Self {
            workspace_id: active.workspace_id(),
            active_buffer_id: active.active_buffer_id,
            active_file_id: active.active_file_id,
        }
    }

    fn apply_to_active(self, active: &mut ActiveDocumentController) {
        active.active_file_id = self.active_file_id;
    }

    fn ensure_active_buffer(&self, target: BufferId) -> Result<(), AppCompositionError> {
        if self.active_buffer_id == Some(target) {
            Ok(())
        } else {
            Err(AppCompositionError::BufferMismatch {
                target,
                active: self.active_buffer_id,
            })
        }
    }

    fn require_workspace_id(&self) -> Result<WorkspaceId, AppCompositionError> {
        self.workspace_id
            .ok_or(AppCompositionError::WorkspaceNotOpen)
    }
}

/// Service that executes routed commands against app-owned ports and domain services.
pub struct CommandExecutionService;

impl CommandExecutionService {
    /// Execute a previously routed command request when no app-owned save/open workflow is needed.
    pub fn execute(
        request: &AppCommandRequest,
        editor: &mut dyn AppEditorCommandPort,
        workspace: &dyn AppWorkspaceCommandPort,
        state: &mut AppCommandExecutionState,
    ) -> Result<Option<AppCommandOutcome>, AppCompositionError> {
        match request {
            AppCommandRequest::Noop => Ok(Some(AppCommandOutcome::Noop)),
            AppCommandRequest::Quit => Ok(Some(AppCommandOutcome::Quit)),
            AppCommandRequest::Undo { buffer_id } => {
                state.ensure_active_buffer(*buffer_id)?;
                Ok(Some(AppCommandOutcome::Edited(editor.undo(*buffer_id)?)))
            }
            AppCommandRequest::Redo { buffer_id } => {
                state.ensure_active_buffer(*buffer_id)?;
                Ok(Some(AppCommandOutcome::Edited(editor.redo(*buffer_id)?)))
            }
            AppCommandRequest::ApplyEdit { buffer_id, edit } => {
                state.ensure_active_buffer(*buffer_id)?;
                Ok(Some(AppCommandOutcome::Edited(
                    editor.apply_edit(*buffer_id, edit.clone())?,
                )))
            }
            AppCommandRequest::Save { .. } | AppCommandRequest::OpenPath { .. } => Ok(None),
            AppCommandRequest::RefreshExplorer => {
                let workspace_id = state.require_workspace_id()?;
                let tree = workspace.tree_snapshot(workspace_id)?;
                Ok(Some(AppCommandOutcome::ExplorerRefreshed(
                    ProjectionBuilder::explorer_projection_for_selection(
                        state.active_file_id,
                        tree,
                    ),
                )))
            }
            AppCommandRequest::RevealInExplorer { file_id } => {
                state.active_file_id = Some(*file_id);
                let workspace_id = state.require_workspace_id()?;
                let tree = workspace.tree_snapshot(workspace_id)?;
                Ok(Some(AppCommandOutcome::ExplorerRefreshed(
                    ProjectionBuilder::explorer_projection_for_selection(
                        state.active_file_id,
                        tree,
                    ),
                )))
            }
        }
    }
}

/// Service that maps UI intents into application command requests without invoking concrete adapters.
#[derive(Debug)]
pub struct CommandDispatcher;

/// App-owned metadata used to turn projection-only proposal UI intents into protocol requests.
#[derive(Debug, Clone)]
pub struct AppProposalIntentRouteContext {
    /// App-owned proposal for preview/apply intents when required.
    pub proposal: Option<WorkspaceProposal>,
    /// Principal selected by app/session policy, not by UI state.
    pub principal: PrincipalId,
    /// Capability selected by app/proposal policy, not by UI state.
    pub capability: CapabilityId,
    /// Non-zero app-routed correlation id.
    pub correlation_id: CorrelationId,
    /// App-routed causality id.
    pub causality_id: CausalityId,
    /// App-routed request timestamp.
    pub requested_at: TimestampMillis,
}

impl CommandDispatcher {
    /// Convert a UI command intent into a port-shaped application command request.
    pub fn route_intent(
        intent: CommandDispatchIntent,
        active: AppCommandRouteContext,
        correlation_id: CorrelationId,
    ) -> Result<AppCommandRequest, AppCompositionError> {
        match intent {
            CommandDispatchIntent::Noop => Ok(AppCommandRequest::Noop),
            CommandDispatchIntent::Quit => Ok(AppCommandRequest::Quit),
            CommandDispatchIntent::Undo { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::Undo { buffer_id })
            }
            CommandDispatchIntent::Redo { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::Redo { buffer_id })
            }
            CommandDispatchIntent::Insert {
                buffer_id,
                at,
                text,
            } => Self::edit_request(
                active,
                buffer_id,
                TextEdit::insert(Self::editor_position(at), text),
                correlation_id,
            ),
            CommandDispatchIntent::Delete { buffer_id, range } => Self::edit_request(
                active,
                buffer_id,
                TextEdit::delete(Self::editor_range(range)),
                correlation_id,
            ),
            CommandDispatchIntent::Replace {
                buffer_id,
                range,
                replacement,
            } => Self::edit_request(
                active,
                buffer_id,
                TextEdit::new(Self::editor_range(range), replacement),
                correlation_id,
            ),
            CommandDispatchIntent::Save { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::Save { buffer_id })
            }
            CommandDispatchIntent::OpenPath { path } => Ok(AppCommandRequest::OpenPath { path }),
            CommandDispatchIntent::RefreshExplorer => Ok(AppCommandRequest::RefreshExplorer),
            CommandDispatchIntent::RevealInExplorer { file_id } => {
                Ok(AppCommandRequest::RevealInExplorer { file_id })
            }
            CommandDispatchIntent::PreviewProposal { .. }
            | CommandDispatchIntent::ApproveProposal { .. }
            | CommandDispatchIntent::RejectProposal { .. }
            | CommandDispatchIntent::ApplyProposal { .. }
            | CommandDispatchIntent::RollbackProposal { .. }
            | CommandDispatchIntent::CancelProposal { .. }
            | CommandDispatchIntent::OpenProposalDetails { .. } => Ok(AppCommandRequest::Noop),
        }
    }

    /// Convert a projection-only proposal UI intent into a protocol proposal request.
    pub fn route_proposal_intent(
        intent: CommandDispatchIntent,
        context: AppProposalIntentRouteContext,
    ) -> Result<Option<ProposalRequest>, AppCompositionError> {
        match intent {
            CommandDispatchIntent::PreviewProposal { proposal_id } => {
                let proposal = Self::owned_proposal_for_intent(proposal_id, context.proposal)?;
                Ok(Some(ProposalRequest::Preview(proposal)))
            }
            CommandDispatchIntent::ApplyProposal { proposal_id } => {
                let proposal = Self::owned_proposal_for_intent(proposal_id, context.proposal)?;
                Ok(Some(ProposalRequest::Apply(proposal)))
            }
            CommandDispatchIntent::ApproveProposal { proposal_id } => Ok(Some(
                ProposalRequest::Approve(Self::proposal_lifecycle_command(
                    proposal_id,
                    ProposalLifecycleAction::Approve,
                    None,
                    context,
                )),
            )),
            CommandDispatchIntent::RejectProposal {
                proposal_id,
                reason,
            } => Ok(Some(ProposalRequest::Reject(
                Self::proposal_lifecycle_command(
                    proposal_id,
                    ProposalLifecycleAction::Reject,
                    Some(ProposalLifecycleCommandReason::Rejection(reason)),
                    context,
                ),
            ))),
            CommandDispatchIntent::RollbackProposal {
                proposal_id,
                reason,
            } => Ok(Some(ProposalRequest::Rollback(
                Self::proposal_lifecycle_command(
                    proposal_id,
                    ProposalLifecycleAction::Rollback,
                    Some(ProposalLifecycleCommandReason::Rollback(reason)),
                    context,
                ),
            ))),
            CommandDispatchIntent::CancelProposal {
                proposal_id,
                reason,
            } => Ok(Some(ProposalRequest::Cancel(
                Self::proposal_lifecycle_command(
                    proposal_id,
                    ProposalLifecycleAction::Cancel,
                    Some(ProposalLifecycleCommandReason::Cancellation(reason)),
                    context,
                ),
            ))),
            CommandDispatchIntent::OpenProposalDetails { .. } => Ok(None),
            _ => Ok(None),
        }
    }

    fn owned_proposal_for_intent(
        proposal_id: ProposalId,
        proposal: Option<WorkspaceProposal>,
    ) -> Result<WorkspaceProposal, AppCompositionError> {
        let proposal = proposal.ok_or(AppCompositionError::ProposalIntentMissingProposal)?;
        if proposal.proposal_id == proposal_id {
            Ok(proposal)
        } else {
            Err(AppCompositionError::ProposalIntentMismatch {
                target: proposal_id,
                active: Some(proposal.proposal_id),
            })
        }
    }

    fn proposal_lifecycle_command(
        proposal_id: ProposalId,
        action: ProposalLifecycleAction,
        reason: Option<ProposalLifecycleCommandReason>,
        context: AppProposalIntentRouteContext,
    ) -> ProposalLifecycleCommand {
        ProposalLifecycleCommand {
            proposal_id,
            action,
            principal: context.principal,
            capability: context.capability,
            correlation_id: context.correlation_id,
            causality_id: context.causality_id,
            reason,
            diagnostics: Vec::new(),
            requested_at: context.requested_at,
            schema_version: 1,
        }
    }

    fn edit_request(
        active: AppCommandRouteContext,
        buffer_id: BufferId,
        edit: TextEdit,
        _correlation_id: CorrelationId,
    ) -> Result<AppCommandRequest, AppCompositionError> {
        Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
        let _ = active
            .workspace_id
            .ok_or(AppCompositionError::WorkspaceNotOpen)?;
        let _ = active
            .file_id
            .ok_or(AppCompositionError::ActiveFileMissing)?;

        Ok(AppCommandRequest::ApplyEdit { buffer_id, edit })
    }

    fn editor_position(position: TextCoordinate) -> TextPosition {
        TextPosition::new(position.line as usize, position.character as usize)
    }

    fn editor_range(range: devil_protocol::ProtocolTextRange) -> EditorTextRange {
        EditorTextRange::new(
            Self::editor_position(range.start),
            Self::editor_position(range.end),
        )
    }

    fn ensure_active_buffer(
        active: Option<BufferId>,
        target: BufferId,
    ) -> Result<(), AppCompositionError> {
        if active == Some(target) {
            Ok(())
        } else {
            Err(AppCompositionError::BufferMismatch { target, active })
        }
    }
}

/// Minimal active-document context used by command routing tests and dispatcher calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppCommandRouteContext {
    /// Active workspace identifier when a workspace is open.
    pub workspace_id: Option<WorkspaceId>,
    /// Active buffer identifier.
    pub buffer_id: Option<BufferId>,
    /// Active file identifier.
    pub file_id: Option<FileId>,
}

impl AppCommandRouteContext {
    fn from_active(active: &ActiveDocumentController) -> Self {
        Self {
            workspace_id: active.workspace_id(),
            buffer_id: active.active_buffer_id,
            file_id: active.active_file_id,
        }
    }
}

#[derive(Debug)]
struct ProjectionBuilder;

impl ProjectionBuilder {
    fn active_buffer_projection(
        active: &ActiveDocumentController,
        editor: &EditorEngine,
        layout: &ShellLayoutProjection,
    ) -> Result<ActiveBufferProjection, AppCompositionError> {
        let Some(buffer_id) = active.active_buffer_id else {
            return Ok(ActiveBufferProjection::empty());
        };

        // Construct default viewport request
        let request = devil_protocol::EditorViewportRequest {
            buffer_id,
            scroll: devil_protocol::ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: devil_protocol::ViewportDimensions {
                width_px: layout.layout.width as u32 * 8, // Approximate
                height_px: layout.layout.height as u32 * 16,
            },
        };

        let viewport = editor.viewport_projection(request).ok();
        let degraded = viewport
            .as_ref()
            .is_some_and(|vp| vp.large_file_status.is_some());

        let dirty = editor.is_dirty(buffer_id)?;

        Ok(ActiveBufferProjection {
            workspace_id: active.workspace_id(),
            buffer_id: Some(buffer_id),
            file_id: active.active_file_id,
            file_path: active
                .active_file_path
                .as_ref()
                .map(|path| CanonicalPath(path.clone())),
            viewport,
            degraded,
            small_buffer_preview: if degraded {
                None
            } else {
                editor.text(buffer_id).ok().map(|s| s.to_string())
            },
            dirty,
        })
    }

    fn explorer_projection(
        active: &ActiveDocumentController,
        tree: Vec<FileTreeNode>,
    ) -> ExplorerProjection {
        Self::explorer_projection_for_selection(active.active_file_id, tree)
    }

    fn explorer_projection_for_selection(
        active_file_id: Option<FileId>,
        tree: Vec<FileTreeNode>,
    ) -> ExplorerProjection {
        let nodes = tree
            .into_iter()
            .map(|node| ExplorerNodeProjection {
                file_id: node.identity.file_id,
                canonical_path: node.identity.canonical_path,
                name: node.name,
                children: node.children,
            })
            .collect();

        ExplorerProjection {
            nodes,
            selection: active_file_id.map(|file_id| ExplorerSelectionProjection { file_id }),
        }
    }
}

#[derive(Debug, Clone)]
struct SaveWorkflowOutput {
    save: SaveRequestDto,
    applied: devil_project::WorkspaceSaveApplied,
}

#[derive(Debug, Clone)]
struct SaveWorkflowFailure {
    request_id: uuid::Uuid,
    response: ProposalResponse,
}

#[derive(Debug)]
struct SaveWorkflowService;

impl SaveWorkflowService {
    #[allow(clippy::result_large_err)]
    fn save_active_buffer(
        editor: &mut EditorEngine,
        workspace: &WorkspaceActor,
        proposal_coordinator: &mut AppProposalCoordinator,
        storage: &dyn StorageRepositoryPort,
        context: ActiveSaveContext,
        event_context: EventContext,
    ) -> Result<SaveWorkflowOutput, SaveWorkflowFailure> {
        let save = editor
            .request_save(context.buffer_id, Some(event_context.correlation_id))
            .map_err(|error| SaveWorkflowFailure {
                request_id: uuid::Uuid::nil(),
                response: Self::failed_response_for_editor_error(
                    error,
                    &context,
                    event_context.correlation_id,
                    event_context.causality_id,
                ),
            })?;
        let proposal = proposal_coordinator.build_save_proposal(
            &save,
            &context.metadata,
            context.principal.clone(),
            context.trust,
            event_context,
        );
        let created = proposal_coordinator.created_response(&proposal);
        let _ = Self::observe_proposal_response(
            proposal_coordinator,
            storage,
            &proposal,
            &created,
            None,
        );
        let validation = proposal_coordinator
            .handle(ProposalRequest::Validate(proposal.clone()))
            .unwrap_or_else(|err| {
                Self::failed_response_for_protocol_error(err, &proposal, event_context.causality_id)
            });
        let _ = Self::observe_proposal_response(
            proposal_coordinator,
            storage,
            &proposal,
            &validation,
            None,
        );
        if !matches!(validation, ProposalResponse::Validated(_)) {
            return Err(SaveWorkflowFailure {
                request_id: save.request_id,
                response: validation,
            });
        }
        let preview = proposal_coordinator
            .handle(ProposalRequest::Preview(proposal.clone()))
            .unwrap_or_else(|err| {
                Self::failed_response_for_protocol_error(err, &proposal, event_context.causality_id)
            });
        let _ = Self::observe_proposal_response(
            proposal_coordinator,
            storage,
            &proposal,
            &preview,
            None,
        );
        if !matches!(preview, ProposalResponse::Previewed { .. }) {
            return Err(SaveWorkflowFailure {
                request_id: save.request_id,
                response: preview,
            });
        }

        let Some(expected_fingerprint) = proposal.preconditions.expected_fingerprint.clone() else {
            return Err(SaveWorkflowFailure {
                request_id: save.request_id,
                response: validation,
            });
        };
        let workspace_save = WorkspaceSaveRequest {
            workspace_id: context.workspace_id,
            proposal_id: proposal.proposal_id,
            principal: context.principal,
            required_capability: proposal.capability.clone(),
            file_id: save.file_id,
            path: context.metadata.identity.canonical_path.clone(),
            expected_fingerprint,
            expected_file_content_version: context.metadata.file_content_version,
            expected_workspace_generation: context.metadata.workspace_generation,
            buffer_version: save.buffer_version,
            snapshot_id: save.snapshot_id,
            payload_byte_len: save.payload_byte_len,
            correlation_id: save.correlation_id,
            causality_id: event_context.causality_id,
            text: save.text.clone(),
        };

        match workspace.save_file_with_proposal(workspace_save) {
            Ok(applied) => {
                if let Err(response) = SaveWorkflowService::observe_proposal_response(
                    proposal_coordinator,
                    storage,
                    &proposal,
                    &applied.response,
                    Some(&applied),
                ) {
                    return Err(SaveWorkflowFailure {
                        request_id: save.request_id,
                        response,
                    });
                }
                Ok(SaveWorkflowOutput { save, applied })
            }
            Err(response) => {
                let _ = SaveWorkflowService::observe_proposal_response(
                    proposal_coordinator,
                    storage,
                    &proposal,
                    &response,
                    None,
                );
                Err(SaveWorkflowFailure {
                    request_id: save.request_id,
                    response,
                })
            }
        }
    }

    #[allow(clippy::result_large_err)]
    fn observe_proposal_response(
        proposal_coordinator: &mut AppProposalCoordinator,
        storage: &dyn StorageRepositoryPort,
        proposal: &WorkspaceProposal,
        response: &ProposalResponse,
        applied: Option<&devil_project::WorkspaceSaveApplied>,
    ) -> Result<(), ProposalResponse> {
        let audit_required = Self::audit_before_success_required(response);
        for envelope in Self::events_for_response(proposal_coordinator, proposal, response) {
            let metadata = event_metadata_record(&envelope);
            if let Err(error) = proposal_coordinator.emit(envelope)
                && audit_required
            {
                return Err(Self::audit_storage_failed_response(
                    proposal, response, error,
                ));
            }
            if let Err(error) =
                storage.handle(StorageRepositoryRequest::SaveEventMetadata(metadata))
                && audit_required
            {
                return Err(Self::audit_storage_failed_response(
                    proposal, response, error,
                ));
            }
        }

        if let Some(transition) = Self::transition_for_response(response) {
            proposal_coordinator.record_observed_transition(transition);
            let audit = proposal_audit_record(proposal, transition);
            if let Err(error) =
                storage.handle(StorageRepositoryRequest::SaveProposalAuditRecord(audit))
                && audit_required
            {
                return Err(Self::audit_storage_failed_response(
                    proposal, response, error,
                ));
            }
            if audit_required {
                let envelope = proposal_audit_recorded_event(
                    proposal,
                    transition,
                    proposal_coordinator.next_sequence(),
                );
                let metadata = event_metadata_record(&envelope);
                if let Err(error) = proposal_coordinator.emit(envelope) {
                    return Err(Self::audit_storage_failed_response(
                        proposal, response, error,
                    ));
                }
                if let Err(error) =
                    storage.handle(StorageRepositoryRequest::SaveEventMetadata(metadata))
                {
                    return Err(Self::audit_storage_failed_response(
                        proposal, response, error,
                    ));
                }
            }
        }

        if let Some(applied) = applied {
            let _ = applied.used_non_atomic_fallback;
        }
        Ok(())
    }

    fn audit_before_success_required(response: &ProposalResponse) -> bool {
        matches!(
            response,
            ProposalResponse::Applied(_) | ProposalResponse::RolledBack { .. }
        )
    }

    fn audit_storage_failed_response(
        proposal: &WorkspaceProposal,
        response: &ProposalResponse,
        error: ProtocolError,
    ) -> ProposalResponse {
        let (correlation_id, causality_id) = Self::transition_for_response(response)
            .map(|transition| (transition.correlation_id, transition.causality_id))
            .unwrap_or((proposal.correlation_id, CausalityId(uuid::Uuid::now_v7())));
        ProposalResponse::Failed {
            transition: ProposalLifecycleTransition {
                proposal_id: proposal.proposal_id,
                lifecycle_state: ProposalLifecycleState::Failed,
                timestamp: TimestampMillis::now(),
                principal: proposal.principal.clone(),
                capability: proposal.capability.clone(),
                correlation_id,
                causality_id,
                diagnostics: vec![ProtocolDiagnostic {
                    code: "proposal.audit_storage_failed".to_string(),
                    message: format!(
                        "proposal success blocked because audit storage failed: {}",
                        error.code
                    ),
                    severity: ProtocolDiagnosticSeverity::Error,
                    path: None,
                    range: None,
                }],
            },
            reason: ProposalFailureReason::StorageFailed,
        }
    }

    fn events_for_response(
        proposal_coordinator: &mut AppProposalCoordinator,
        proposal: &WorkspaceProposal,
        response: &ProposalResponse,
    ) -> Vec<EventEnvelope> {
        match response {
            ProposalResponse::Created(transition) => vec![proposal_created_event(
                proposal,
                transition.causality_id,
                proposal_coordinator.next_sequence(),
            )],
            ProposalResponse::Validated(transition) => vec![proposal_validated_event(
                proposal,
                transition,
                proposal_coordinator.next_sequence(),
            )],
            ProposalResponse::Previewed { transition, .. } => vec![proposal_previewed_event(
                proposal,
                transition,
                proposal_coordinator.next_sequence(),
            )],
            ProposalResponse::Applied(transition) => vec![proposal_applied_event(
                proposal,
                transition,
                proposal_coordinator.next_sequence(),
            )],
            ProposalResponse::Denied { transition, reason } => {
                let save_target = save_event_target(proposal);
                let generic_event = proposal_rejected_event(
                    proposal,
                    transition,
                    match reason {
                        ProposalDenialReason::CapabilityDenied
                        | ProposalDenialReason::WorkspaceUntrusted
                        | ProposalDenialReason::PrincipalUnauthorized
                        | ProposalDenialReason::PolicyDenied => {
                            devil_protocol::ProposalRejectionReason::ValidationFailed
                        }
                    },
                    proposal_coordinator.next_sequence(),
                );

                let mut events = vec![generic_event];
                if let Some(save_target) = save_target {
                    events.push(save_denied_event(
                        save_target.workspace_id,
                        save_target.file_id,
                        transition.correlation_id,
                        transition.causality_id,
                        proposal_coordinator.next_sequence(),
                        format!("{reason:?}"),
                    ));
                }
                events
            }
            ProposalResponse::Failed { transition, reason } => vec![proposal_failed_event(
                proposal,
                transition,
                *reason,
                proposal_coordinator.next_sequence(),
            )],
            ProposalResponse::RolledBack { transition, reason } => {
                vec![proposal_rolled_back_event(
                    proposal,
                    transition,
                    *reason,
                    proposal_coordinator.next_sequence(),
                )]
            }
            ProposalResponse::Stale { transition, stale } => {
                let Some(save_target) = save_event_target(proposal) else {
                    return vec![proposal_rejected_event(
                        proposal,
                        transition,
                        devil_protocol::ProposalRejectionReason::ValidationFailed,
                        proposal_coordinator.next_sequence(),
                    )];
                };
                vec![stale_proposal_rejected_event(
                    save_target.workspace_id,
                    save_target.file_id,
                    transition.correlation_id,
                    transition.causality_id,
                    proposal_coordinator.next_sequence(),
                    transition.proposal_id,
                    stale.reason,
                )]
            }
            ProposalResponse::Conflict { transition, .. } => vec![proposal_rejected_event(
                proposal,
                transition,
                devil_protocol::ProposalRejectionReason::ValidationFailed,
                proposal_coordinator.next_sequence(),
            )],
            ProposalResponse::Rejected { transition, reason } => vec![proposal_rejected_event(
                proposal,
                transition,
                *reason,
                proposal_coordinator.next_sequence(),
            )],
            ProposalResponse::Approved(transition) => vec![proposal_approved_event(
                proposal,
                transition,
                proposal_coordinator.next_sequence(),
            )],
            ProposalResponse::Cancelled { transition, .. } => vec![proposal_rejected_event(
                proposal,
                transition,
                devil_protocol::ProposalRejectionReason::Cancelled,
                proposal_coordinator.next_sequence(),
            )],
        }
    }

    fn transition_for_response(
        response: &ProposalResponse,
    ) -> Option<&ProposalLifecycleTransition> {
        match response {
            ProposalResponse::Created(transition)
            | ProposalResponse::Validated(transition)
            | ProposalResponse::Approved(transition)
            | ProposalResponse::Applied(transition) => Some(transition),
            ProposalResponse::Previewed { transition, .. }
            | ProposalResponse::Rejected { transition, .. }
            | ProposalResponse::Denied { transition, .. }
            | ProposalResponse::Failed { transition, .. }
            | ProposalResponse::RolledBack { transition, .. }
            | ProposalResponse::Stale { transition, .. }
            | ProposalResponse::Conflict { transition, .. }
            | ProposalResponse::Cancelled { transition, .. } => Some(transition),
        }
    }

    fn failed_response_for_editor_error(
        error: EditorError,
        context: &ActiveSaveContext,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
    ) -> ProposalResponse {
        let proposal = WorkspaceProposal {
            proposal_id: devil_protocol::ProposalId(0),
            principal: context.principal.clone(),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id,
            payload: ProposalPayload::SaveFile(SaveFileProposal {
                file: context.metadata.identity.clone(),
                buffer_id: context.buffer_id,
                file_id: context.metadata.identity.file_id,
                snapshot_id: devil_protocol::SnapshotId(0),
                buffer_version: devil_protocol::BufferVersion(0),
                file_content_version: context.metadata.file_content_version,
                workspace_generation: context.metadata.workspace_generation,
                expected_fingerprint: Some(context.metadata.fingerprint.clone()),
                save_intent: SaveIntent::Manual,
                conflict_policy: SaveConflictPolicy::RejectIfChanged,
                trust_decision: TrustDecisionContext {
                    workspace_trust_state: context.trust.clone(),
                    decision_id: None,
                    decided_at: Some(TimestampMillis::now()),
                },
                required_capability: CapabilityId("fs.write".to_string()),
                principal: context.principal.clone(),
                correlation_id,
                diagnostics: Vec::new(),
            }),
            preconditions: ProposalVersionPreconditions {
                file_version: Some(context.metadata.file_content_version),
                buffer_version: None,
                snapshot_id: None,
                generation: Some(context.metadata.workspace_generation),
                file_content_version: Some(context.metadata.file_content_version),
                workspace_generation: Some(context.metadata.workspace_generation),
                expected_fingerprint: Some(context.metadata.fingerprint.clone()),
                expected_file_length: context.metadata.file_length,
                expected_modified_at: context.metadata.modified_at,
            },
            preview: PreviewSummary {
                summary: "save failed before proposal creation".to_string(),
                details: vec![error.to_string()],
            },
            expires_at: None,
            created_at: TimestampMillis::now(),
        };
        Self::failed_response_for_protocol_error(
            ProtocolError {
                code: "editor_error".to_string(),
                message: error.to_string(),
            },
            &proposal,
            causality_id,
        )
    }

    fn failed_response_for_protocol_error(
        error: ProtocolError,
        proposal: &WorkspaceProposal,
        causality_id: CausalityId,
    ) -> ProposalResponse {
        ProposalResponse::Failed {
            transition: ProposalLifecycleTransition {
                proposal_id: proposal.proposal_id,
                lifecycle_state: ProposalLifecycleState::Failed,
                timestamp: TimestampMillis::now(),
                principal: proposal.principal.clone(),
                capability: proposal.capability.clone(),
                correlation_id: proposal.correlation_id,
                causality_id,
                diagnostics: vec![ProtocolDiagnostic {
                    code: error.code,
                    message: error.message,
                    severity: ProtocolDiagnosticSeverity::Error,
                    path: None,
                    range: None,
                }],
            },
            reason: ProposalFailureReason::InternalError,
        }
    }
}

fn acknowledgement_for_response(response: &ProposalResponse) -> SaveAcknowledgement {
    match response {
        ProposalResponse::Applied(_) => SaveAcknowledgement::Saved,
        ProposalResponse::Stale { transition, stale } => SaveAcknowledgement::Stale {
            conflict: stale.actual.as_ref().map(|actual| FileConflictState {
                state: FileConflictLifecycleState::ConflictDirty,
                context: FileConflictContext {
                    workspace_id: WorkspaceId(0),
                    file_identity: FileIdentity {
                        file_id: FileId(0),
                        workspace_id: WorkspaceId(0),
                        canonical_path: CanonicalPath("unknown".to_string()),
                        content_version: actual.file_content_version,
                        content_hash: None,
                    },
                    buffer_version: actual.buffer_version,
                    file_content_version: actual.file_content_version,
                    snapshot_id: actual.snapshot_id,
                    disk_fingerprint: actual.fingerprint.clone(),
                    expected_fingerprint: stale.expected.expected_fingerprint.clone(),
                    reason: FileConflictReason::DiskFingerprintChanged,
                    diagnostics: transition.diagnostics.clone(),
                },
                diagnostics: transition.diagnostics.clone(),
                schema_version: 1,
            }),
            diagnostics: transition.diagnostics.clone(),
        },
        ProposalResponse::Conflict { conflict, .. } => SaveAcknowledgement::Conflict {
            conflict: conflict.clone(),
        },
        ProposalResponse::Denied { transition, .. } => SaveAcknowledgement::Denied {
            diagnostics: transition.diagnostics.clone(),
        },
        ProposalResponse::Failed { transition, .. } => SaveAcknowledgement::Failed {
            diagnostics: transition.diagnostics.clone(),
        },
        ProposalResponse::Rejected { transition, .. } => SaveAcknowledgement::Failed {
            diagnostics: transition.diagnostics.clone(),
        },
        ProposalResponse::RolledBack { transition, .. } => SaveAcknowledgement::Failed {
            diagnostics: transition.diagnostics.clone(),
        },
        ProposalResponse::Cancelled { transition, .. } => SaveAcknowledgement::Failed {
            diagnostics: transition.diagnostics.clone(),
        },
        ProposalResponse::Created(_)
        | ProposalResponse::Validated(_)
        | ProposalResponse::Previewed { .. }
        | ProposalResponse::Approved(_) => SaveAcknowledgement::Failed {
            diagnostics: vec![ProtocolDiagnostic {
                code: "proposal.incomplete".to_string(),
                message: "proposal did not reach applied lifecycle".to_string(),
                severity: ProtocolDiagnosticSeverity::Error,
                path: None,
                range: None,
            }],
        },
    }
}

#[derive(Debug, Clone, Copy)]
struct SaveEventTarget {
    workspace_id: WorkspaceId,
    file_id: FileId,
}

fn save_event_target(proposal: &WorkspaceProposal) -> Option<SaveEventTarget> {
    match &proposal.payload {
        ProposalPayload::SaveFile(payload) => Some(SaveEventTarget {
            workspace_id: payload.file.workspace_id,
            file_id: payload.file.file_id,
        }),
        _ => AppProposalCoordinator::affected_target_coverage(proposal)
            .targets
            .into_iter()
            .find_map(|target| {
                Some(SaveEventTarget {
                    workspace_id: target.workspace_id?,
                    file_id: target.file_id?,
                })
            }),
    }
}

/// Result of routing a UI command intent through application-owned services.
#[derive(Debug, Clone)]
pub enum AppCommandOutcome {
    /// Command had no effect.
    Noop,
    /// Command requested shell termination.
    Quit,
    /// Editor transaction was applied.
    Edited(TextTransactionDescriptor),
    /// Buffer save completed through workspace authority.
    Save(AppSaveOutcome),
    /// Explorer projection was refreshed from workspace tree state.
    ExplorerRefreshed(ExplorerProjection),
    /// A workspace path was opened and bound to an editor buffer.
    Opened(FileId),
}

/// Root application composition.
pub struct AppComposition {
    workspace: WorkspaceActor,
    editor: EditorEngine,
    proposal_coordinator: AppProposalCoordinator,
    active_documents: ActiveDocumentController,
    correlation_generator: CorrelationGenerator,
    event_sequence_generator: EventSequenceGenerator,
    storage: InMemoryStorageRepositoryPort,
    event_sink: SharedEventSink,
}

impl AppComposition {
    /// Build composition with native platform adapters and default-deny security broker.
    pub fn new() -> Self {
        Self::with_event_sink(SharedEventSink::default())
    }

    /// Build composition with native platform adapters and an injected event sink.
    pub fn with_event_sink(event_sink: SharedEventSink) -> Self {
        let fs = Arc::new(NativeFileSystem);
        let watcher = Arc::new(NativeWatcherService);
        let security = DenyByDefaultBroker::new(
            SecurityPolicy::default(),
            CapabilityNamespace("app".to_string()),
        );

        Self {
            workspace: WorkspaceActor::with_event_sink(
                fs,
                watcher,
                security,
                Box::new(event_sink.clone()),
            ),
            editor: EditorEngine::new(),
            proposal_coordinator: AppProposalCoordinator::new(event_sink.clone()),
            active_documents: ActiveDocumentController::new(),
            correlation_generator: CorrelationGenerator::default(),
            event_sequence_generator: EventSequenceGenerator::default(),
            storage: InMemoryStorageRepositoryPort::with_event_sink(event_sink.clone()),
            event_sink,
        }
    }

    fn next_event_context(&mut self) -> EventContext {
        EventContext::new(self.correlation_generator.next())
    }

    fn emit_event(&self, envelope: EventEnvelope) {
        let _ = self.storage.record_event(envelope);
    }

    fn emit_transaction_event(&mut self, descriptor: &TextTransactionDescriptor) {
        let envelope =
            transaction_event(descriptor, true, None, self.event_sequence_generator.next());
        self.emit_event(envelope);
    }

    /// Open a workspace.
    pub fn open_workspace(
        &mut self,
        root: impl AsRef<Path>,
        trust: WorkspaceTrustState,
        principal: PrincipalId,
    ) -> Result<WorkspaceOpened, AppCompositionError> {
        let root_path = CanonicalPath(root.as_ref().to_string_lossy().into_owned());
        let request = WorkspaceOpenRequest {
            correlation_id: self.correlation_generator.next(),
            principal_id: principal.clone(),
            root_path: root_path.clone(),
            trust: Some(trust.clone()),
        };

        let opened = match self
            .workspace
            .handle(WorkspaceRequest::Open(request))
            .map_err(AppCompositionError::Protocol)?
        {
            WorkspaceResponse::Opened(opened) => opened,
            other => {
                return Err(AppCompositionError::Protocol(ProtocolError {
                    code: "workspace_open_unexpected_response".to_string(),
                    message: format!("expected opened response, got {other:?}"),
                }));
            }
        };
        self.active_documents
            .bind_workspace(opened.clone(), root_path, principal, trust);
        Ok(opened)
    }

    /// Open a file through workspace authority and bind it into editor engine.
    pub fn open_file(&mut self, path: impl AsRef<str>) -> Result<FileId, AppCompositionError> {
        self.open_file_with_intent(path, OpenFileIntent::Existing)
    }

    /// Open a new-file buffer only when the caller explicitly requested create intent.
    pub fn open_new_file(&mut self, path: impl AsRef<str>) -> Result<FileId, AppCompositionError> {
        self.open_file_with_intent(path, OpenFileIntent::CreateNew)
    }

    /// Open a file using an explicit open intent.
    pub fn open_file_with_intent(
        &mut self,
        path: impl AsRef<str>,
        intent: OpenFileIntent,
    ) -> Result<FileId, AppCompositionError> {
        let workspace_id = self.active_documents.require_workspace_id()?;
        let event_context = self.next_event_context();
        let opened = self.workspace.open_file_text(
            workspace_id,
            path.as_ref(),
            intent,
            Some(event_context),
        )?;
        self.bind_opened_file(opened)
    }

    fn bind_opened_file(&mut self, opened: OpenedFileText) -> Result<FileId, AppCompositionError> {
        let identity = opened.identity.clone();

        let buffer_id = self.editor.open_buffer(
            identity.workspace_id,
            identity.file_id,
            identity.canonical_path.0.clone(),
            opened.text.clone(),
        )?;

        self.active_documents.bind_opened_file(&opened, buffer_id);
        Ok(identity.file_id)
    }

    /// Apply an edit command directly to the active editor-engine buffer.
    pub fn edit_active_buffer(
        &mut self,
        edit: TextEdit,
    ) -> Result<TextTransactionDescriptor, AppCompositionError> {
        let buffer_id = self.active_documents.require_active_buffer()?;
        let correlation_id = self.correlation_generator.next();
        let descriptor =
            self.apply_edit_to_buffer_with_correlation(buffer_id, edit, correlation_id)?;
        self.emit_transaction_event(&descriptor);
        Ok(descriptor)
    }

    /// Route a UI dispatch intent through editor and workspace authorities.
    pub fn dispatch_ui_intent(
        &mut self,
        intent: CommandDispatchIntent,
    ) -> Result<AppCommandOutcome, AppCompositionError> {
        let correlation_id = self.correlation_generator.next();
        let request = CommandDispatcher::route_intent(
            intent,
            AppCommandRouteContext::from_active(&self.active_documents),
            correlation_id,
        )?;

        if let AppCommandRequest::ApplyEdit { buffer_id, edit } = &request {
            let descriptor = self.apply_edit_to_buffer_with_correlation(
                *buffer_id,
                edit.clone(),
                correlation_id,
            )?;
            self.emit_transaction_event(&descriptor);
            return Ok(AppCommandOutcome::Edited(descriptor));
        }

        let mut state = AppCommandExecutionState::from_active(&self.active_documents);
        if let Some(outcome) = CommandExecutionService::execute(
            &request,
            &mut self.editor,
            &self.workspace,
            &mut state,
        )? {
            state.apply_to_active(&mut self.active_documents);
            return Ok(outcome);
        }

        match request {
            AppCommandRequest::Save { buffer_id } => {
                self.active_documents.ensure_active_buffer(buffer_id)?;
                Ok(AppCommandOutcome::Save(self.save_active_buffer()?))
            }
            AppCommandRequest::OpenPath { path } => {
                Ok(AppCommandOutcome::Opened(self.open_file(path)?))
            }
            _ => unreachable!("command execution service handled non-workflow command"),
        }
    }

    /// Save currently active buffer through editor save request and workspace write authority.
    pub fn save_active_buffer(&mut self) -> Result<AppSaveOutcome, AppCompositionError> {
        let context = self.active_documents.require_active_save_context()?;
        let event_context = self.next_event_context();
        match SaveWorkflowService::save_active_buffer(
            &mut self.editor,
            &self.workspace,
            &mut self.proposal_coordinator,
            &self.storage,
            context,
            event_context,
        ) {
            Ok(output) => {
                self.editor
                    .acknowledge_save_outcome(output.save.request_id, SaveAcknowledgement::Saved);
                self.active_documents.bind_saved_file(output.applied);
                Ok(AppSaveOutcome::Saved(output.save))
            }
            Err(failure) => {
                if failure.request_id != uuid::Uuid::nil() {
                    self.editor.acknowledge_save_outcome(
                        failure.request_id,
                        acknowledgement_for_response(&failure.response),
                    );
                }
                Ok(AppSaveOutcome::Rejected(Box::new(failure.response)))
            }
        }
    }

    /// Build active-buffer projection from editor-engine state.
    pub fn active_buffer_projection(
        &self,
        layout: &ShellLayoutProjection,
    ) -> Result<ActiveBufferProjection, AppCompositionError> {
        ProjectionBuilder::active_buffer_projection(&self.active_documents, &self.editor, layout)
    }

    /// Build the complete projection snapshot consumed by the UI shell.
    pub fn shell_projection_snapshot(
        &self,
        title: impl Into<String>,
    ) -> Result<ShellProjectionSnapshot, AppCompositionError> {
        let layout_projection = ShellLayoutProjection::plain(title);
        Ok(ShellProjectionSnapshot {
            active_buffer_projection: self.active_buffer_projection(&layout_projection)?,
            layout_projection,
            explorer_projection: self.explorer_projection()?,
            status_messages: Vec::new(),
            proposal_ledger_projection: devil_protocol::ProposalLedgerProjection {
                rows: Vec::new(),
                selected_proposal_id: None,
                omitted_row_count: 0,
                generated_at: TimestampMillis(0),
                redaction_hints: Vec::new(),
                schema_version: 1,
            },
            context_manifest_projection: devil_protocol::ContextManifestProjection {
                manifest: devil_protocol::ContextManifestRecord {
                    manifest_id: "manifest:empty".to_string(),
                    workspace_id: None,
                    proposal_id: None,
                    purpose: devil_protocol::ContextManifestPurpose::TrustReview,
                    workspace_trust_state: None,
                    privacy_label: devil_protocol::ProposalPrivacyLabel::PublicMetadata,
                    risk_label: devil_protocol::ProposalRiskLabel::Informational,
                    egress: devil_protocol::ContextManifestEgressStatus::LocalOnly,
                    items: Vec::new(),
                    permissions: Vec::new(),
                    omitted_item_count: 0,
                    stale_or_missing_metadata_risk_present: false,
                    generated_at: TimestampMillis(0),
                    redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
                    schema_version: 1,
                },
                selected_item_id: None,
                generated_at: TimestampMillis(0),
                redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            privacy_inspector_projection: devil_protocol::PrivacyInspectorProjection {
                inspector_id: "privacy:empty".to_string(),
                manifest_id: None,
                workspace_id: None,
                proposal_id: None,
                records: Vec::new(),
                denied_record_count: 0,
                redacted_record_count: 0,
                external_egress_record_count: 0,
                high_risk_record_count: 0,
                refusal: None,
                generated_at: TimestampMillis(0),
                redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            permission_budget_projection: devil_protocol::PermissionBudgetProjection {
                projection_id: "permission-budgets:empty".to_string(),
                budgets: Vec::new(),
                evaluations: Vec::new(),
                denied_budget_count: 0,
                depleted_budget_count: 0,
                refused_evaluation_count: 0,
                generated_at: TimestampMillis(0),
                redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            approval_checklist_projection: devil_protocol::ProposalApprovalChecklistProjection {
                checklist_id: "approval-checklist:empty".to_string(),
                proposal_id: ProposalId(0),
                workspace_id: None,
                payload_kind: devil_protocol::ProposalPayloadKind::SaveFile,
                lifecycle_state: devil_protocol::ProposalLifecycleState::Created,
                correlation_id: CorrelationId(0),
                causality_id: None,
                ready_for_approval: false,
                gates: Vec::new(),
                blockers: Vec::new(),
                risk_labels: Vec::new(),
                privacy_labels: Vec::new(),
                explicit_denial_reasons: Vec::new(),
                generated_at: TimestampMillis(0),
                redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            checkpoint_rollback_projection: devil_protocol::CheckpointRollbackProjection {
                projection_id: "checkpoint-rollback:empty".to_string(),
                proposal_id: ProposalId(0),
                workspace_id: None,
                payload_kind: devil_protocol::ProposalPayloadKind::SaveFile,
                lifecycle_state: devil_protocol::ProposalLifecycleState::Created,
                correlation_id: CorrelationId(0),
                causality_id: None,
                checkpoint: devil_protocol::ProposalCheckpointProjection {
                    checkpoint_id: "checkpoint:empty".to_string(),
                    available: false,
                    target_count: 0,
                    expected_preconditions:
                        devil_protocol::ContextManifestPreconditionSummary::from_preconditions(
                            &ProposalVersionPreconditions {
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
                            1,
                        ),
                    hashes: Vec::new(),
                    audit_status: devil_protocol::CheckpointRollbackAuditStatus::NotRequired,
                    labels: Vec::new(),
                    limitations: Vec::new(),
                    redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
                    schema_version: 1,
                },
                rollback: devil_protocol::ProposalRollbackProjection {
                    availability: devil_protocol::ProposalRollbackAvailability::NotRequired,
                    rollback_step_count: 0,
                    reversible_target_count: 0,
                    irreversible_target_count: 0,
                    audit_status: devil_protocol::CheckpointRollbackAuditStatus::NotRequired,
                    labels: Vec::new(),
                    limitations: Vec::new(),
                    redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
                    schema_version: 1,
                },
                targets: Vec::new(),
                risk_labels: Vec::new(),
                privacy_labels: Vec::new(),
                generated_at: TimestampMillis(0),
                redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            assisted_ai_projection: devil_protocol::AssistedAiProjection {
                projection_id: "assisted-ai:empty".to_string(),
                providers: Vec::new(),
                routes: Vec::new(),
                requests: Vec::new(),
                refusals: Vec::new(),
                proposal_previews: Vec::new(),
                provider_count: 0,
                request_count: 0,
                refusal_count: 0,
                preview_ready_count: 0,
                provider_invocation: devil_protocol::AssistedAiProviderInvocationState::NotEncoded,
                generated_at: TimestampMillis(0),
                redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            delegated_task_projection: devil_protocol::DelegatedTaskProjection {
                projection_id: "delegated-task:empty".to_string(),
                plan_rows: Vec::new(),
                step_summaries: Vec::new(),
                blockers: Vec::new(),
                refusals: Vec::new(),
                required_approvals: Vec::new(),
                proposal_preview_links: Vec::new(),
                audit_readiness: Vec::new(),
                plan_only_disclaimers: vec!["delegated_task.plan_only.no_runtime".to_string()],
                plan_count: 0,
                blocked_plan_count: 0,
                refused_plan_count: 0,
                runtime_activation: devil_protocol::DelegatedTaskRuntimeActivationState::NotEncoded,
                generated_at: TimestampMillis(0),
                redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
                schema_version: 1,
            },
        })
    }

    /// Build explorer projection from workspace tree snapshot.
    pub fn explorer_projection(&self) -> Result<ExplorerProjection, AppCompositionError> {
        let workspace_id = self.active_documents.require_workspace_id()?;
        let nodes = AppWorkspaceCommandPort::tree_snapshot(&self.workspace, workspace_id)?;
        Ok(ProjectionBuilder::explorer_projection(
            &self.active_documents,
            nodes,
        ))
    }

    /// Expose workspace for integration validation.
    pub fn workspace(&self) -> &WorkspaceActor {
        &self.workspace
    }

    /// Expose editor for integration validation.
    pub fn editor(&self) -> &EditorEngine {
        &self.editor
    }

    /// Expose active buffer id for integration validation.
    pub fn active_buffer_id(&self) -> Option<BufferId> {
        self.active_documents.active_buffer_id
    }

    /// Expose active file id for integration validation.
    pub fn active_file_id(&self) -> Option<FileId> {
        self.active_documents.active_file_id
    }

    /// Expose the active open-file fingerprint for integration validation.
    pub fn active_file_fingerprint(&self) -> Option<&FileFingerprint> {
        self.active_documents
            .active_file_metadata
            .as_ref()
            .map(|metadata| &metadata.fingerprint)
    }

    /// Expose active workspace id.
    pub fn workspace_id(&self) -> Option<WorkspaceId> {
        self.active_documents.workspace_id()
    }

    /// Plan and preflight a batch proposal without mutating editor buffers or workspace state.
    ///
    /// Stage 1D deliberately separates planning from runtime mutation: this API validates batch
    /// shape, target coverage, dependency graph, rollback/atomicity boundaries, route support, and
    /// current editor/workspace preconditions. It does not call apply helpers or workspace mutation
    /// methods, and `runtime_apply_disabled` remains true until batch commit/rollback contracts are
    /// proven by a later stage.
    pub fn preflight_batch_proposal(&self, proposal: &WorkspaceProposal) -> BatchPreflightPlan {
        let mut plan = BatchPreflightPlan {
            proposal_id: proposal.proposal_id,
            batch_id: None,
            preflight_ok: false,
            runtime_apply_disabled: true,
            atomicity: None,
            rollback_policy: None,
            planning_semantics: None,
            rollback_contract: None,
            items: Vec::new(),
            diagnostics: Vec::new(),
            preview_warnings: vec![Self::batch_warning(
                "proposal.batch_runtime_apply_disabled",
                ProposalPreviewWarningKind::UnsupportedRuntime,
                "batch runtime mutation remains fail-closed in Stage 1D",
                None,
            )],
            partial_failures: Vec::new(),
        };

        let ProposalPayload::Batch(batch) = &proposal.payload else {
            plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.batch_preflight_non_batch",
                "batch preflight requires ProposalPayload::Batch",
            ));
            return plan;
        };

        plan.batch_id = Some(batch.batch_id);
        plan.atomicity = Some(batch.atomicity);
        plan.rollback_policy = Some(batch.rollback_policy);
        plan.planning_semantics = Some(Self::batch_planning_semantics(batch));
        plan.preview_warnings
            .extend(batch.preview_warnings.iter().cloned());
        plan.partial_failures
            .extend(batch.partial_failures.iter().cloned());

        let coverage = AppProposalCoordinator::affected_target_coverage(proposal);
        AppProposalCoordinator::push_common_validation_diagnostics(
            proposal,
            &coverage,
            &mut plan.diagnostics,
        );
        AppProposalCoordinator::push_payload_validation_diagnostics(
            proposal,
            &mut plan.diagnostics,
        );

        let workspace_tree = self.read_workspace_tree_for_preflight(&mut plan.diagnostics);
        self.preflight_batch_structure(batch, &mut plan);

        let coverage_ids = batch
            .target_coverage
            .targets
            .iter()
            .map(|target| target.target_id.as_str())
            .collect::<HashSet<_>>();
        let mut items = batch.items.iter().collect::<Vec<_>>();
        items.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.item_id.cmp(&right.item_id))
        });

        for item in items {
            let item_plan = self.preflight_batch_item(
                proposal,
                batch,
                item,
                &coverage_ids,
                workspace_tree.as_deref(),
            );
            if !item_plan.preflight_ok {
                for diagnostic in &item_plan.diagnostics {
                    plan.partial_failures.push(Self::planning_failure(
                        &item.item_id,
                        item.target_ids.first().map_or("", String::as_str),
                        diagnostic.clone(),
                    ));
                }
            }
            plan.items.push(item_plan);
        }

        Self::append_dependency_blocked_failures(batch, &mut plan);

        let rollback_contract = Self::batch_rollback_contract(batch);
        plan.diagnostics
            .extend(rollback_contract.diagnostics.iter().cloned());
        if matches!(
            rollback_contract.status,
            BatchRollbackContractStatus::BestEffort
        ) {
            plan.preview_warnings.push(Self::batch_warning(
                "proposal.batch_rollback_best_effort",
                ProposalPreviewWarningKind::RollbackBestEffort,
                "batch rollback is planned as best-effort and may emit rollback-failure records",
                None,
            ));
        }
        if matches!(
            rollback_contract.status,
            BatchRollbackContractStatus::IrreversibleAccepted
        ) {
            plan.preview_warnings.push(Self::batch_warning(
                "proposal.batch_irreversible_execution_accepted",
                ProposalPreviewWarningKind::AtomicityUnavailable,
                "ordered non-atomic batch explicitly accepts irreversible execution without rollback support",
                None,
            ));
        }
        plan.rollback_contract = Some(rollback_contract);

        plan.preflight_ok = plan.diagnostics.is_empty()
            && plan.items.iter().all(|item| item.preflight_ok)
            && plan
                .partial_failures
                .iter()
                .all(|failure| failure.diagnostics.is_empty());
        plan
    }

    /// Build the Stage 1E batch execution safety contract without executing the batch.
    ///
    /// The contract deliberately reuses `preflight_batch_proposal()` and records the missing
    /// mutation, commit, audit, finalize, and rollback proofs as data. It must not call editor or
    /// workspace mutation helpers, and it must not be interpreted as permission to apply a batch.
    pub fn plan_batch_execution_contract(
        &self,
        proposal: &WorkspaceProposal,
    ) -> BatchExecutionContract {
        let preflight = self.preflight_batch_proposal(proposal);
        let batch = match &proposal.payload {
            ProposalPayload::Batch(batch) => Some(batch),
            _ => None,
        };
        let mut diagnostics = preflight.diagnostics.clone();
        diagnostics.push(AppProposalCoordinator::diagnostic(
            "proposal.batch_contract_runtime_apply_disabled",
            "Stage 1E batch execution contract is preflight-only; runtime batch mutation remains disabled",
        ));
        diagnostics.push(AppProposalCoordinator::diagnostic(
            "proposal.batch_contract_audit_before_success_required",
            "future batch success requires audit proof before finalize or success response",
        ));

        let mut preview_warnings = preflight.preview_warnings.clone();
        preview_warnings.push(Self::batch_warning(
            "proposal.batch_contract_not_runtime_execution",
            ProposalPreviewWarningKind::UnsupportedRuntime,
            "Stage 1E contract planning is not runtime execution and cannot mutate editor or disk state",
            None,
        ));

        let items = preflight
            .items
            .iter()
            .map(|item| {
                let exact_rollback_proof =
                    batch.is_some_and(|batch| Self::item_has_exact_rollback_proof(batch, item));
                let partial_failure_disposition = preflight
                    .partial_failures
                    .iter()
                    .find(|failure| failure.item_id == item.item_id)
                    .map(|failure| failure.disposition);
                BatchExecutionItemContract {
                    item_id: item.item_id.clone(),
                    order: item.order,
                    route: item.route,
                    target_ids: item.target_ids.clone(),
                    preflight_ok: item.preflight_ok,
                    exact_rollback_proof,
                    partial_failure_disposition,
                    diagnostics: item.diagnostics.clone(),
                }
            })
            .collect::<Vec<_>>();

        BatchExecutionContract {
            proposal_id: preflight.proposal_id,
            batch_id: preflight.batch_id,
            stages: Self::batch_execution_stages(preflight.preflight_ok),
            runtime_apply_disabled: preflight.runtime_apply_disabled,
            commit_blocked: true,
            finalize_blocked: true,
            audit_before_success_required: true,
            planning_semantics: preflight.planning_semantics,
            rollback_contract: preflight.rollback_contract.clone(),
            items,
            partial_failures: preflight.partial_failures.clone(),
            diagnostics,
            preview_warnings,
            preflight,
        }
    }

    /// Expose storage repository port for integration validation and future wiring.
    pub fn storage_port(&self) -> &dyn StorageRepositoryPort {
        &self.storage
    }

    /// Inject a one-shot proposal audit write failure for integration validation.
    pub fn fail_next_proposal_audit_write_for_test(&self) {
        self.storage.fail_next_proposal_audit_write();
    }

    /// Inject a one-shot event metadata write failure for integration validation.
    pub fn fail_next_event_metadata_write_for_test(&self) {
        self.storage.fail_next_event_metadata_write();
    }

    /// Expose event publisher port placeholder for integration validation and future wiring.
    pub fn event_publisher(&self) -> &dyn EventSinkPort {
        &self.event_sink
    }

    /// Route an explicit proposal request through the app-level proposal coordinator.
    pub fn handle_proposal_request(
        &mut self,
        request: ProposalRequest,
    ) -> Result<ProposalResponse, AppCompositionError> {
        match request {
            ProposalRequest::Validate(proposal) => {
                let response = self
                    .proposal_coordinator
                    .handle(ProposalRequest::Validate(proposal.clone()))
                    .map_err(AppCompositionError::Protocol)?;
                let _ = SaveWorkflowService::observe_proposal_response(
                    &mut self.proposal_coordinator,
                    &self.storage,
                    &proposal,
                    &response,
                    None,
                );
                Ok(response)
            }
            ProposalRequest::Preview(proposal) => {
                let response = self
                    .proposal_coordinator
                    .handle(ProposalRequest::Preview(proposal.clone()))
                    .map_err(AppCompositionError::Protocol)?;
                let _ = SaveWorkflowService::observe_proposal_response(
                    &mut self.proposal_coordinator,
                    &self.storage,
                    &proposal,
                    &response,
                    None,
                );
                Ok(response)
            }
            ProposalRequest::Apply(proposal) => self.apply_workspace_proposal(proposal),
            ProposalRequest::Approve(command) => {
                self.handle_lifecycle_command_request(ProposalRequest::Approve(command))
            }
            ProposalRequest::Reject(command) => {
                self.handle_lifecycle_command_request(ProposalRequest::Reject(command))
            }
            ProposalRequest::Cancel(command) => {
                self.handle_lifecycle_command_request(ProposalRequest::Cancel(command))
            }
            ProposalRequest::Rollback(command) => {
                self.handle_lifecycle_command_request(ProposalRequest::Rollback(command))
            }
        }
    }

    fn handle_lifecycle_command_request(
        &mut self,
        request: ProposalRequest,
    ) -> Result<ProposalResponse, AppCompositionError> {
        let proposal_id = match &request {
            ProposalRequest::Approve(command)
            | ProposalRequest::Reject(command)
            | ProposalRequest::Cancel(command)
            | ProposalRequest::Rollback(command) => command.proposal_id,
            ProposalRequest::Validate(_)
            | ProposalRequest::Preview(_)
            | ProposalRequest::Apply(_) => {
                unreachable!("non-command proposal request routed as lifecycle command")
            }
        };
        let response = self
            .proposal_coordinator
            .handle(request)
            .map_err(AppCompositionError::Protocol)?;
        if let Some(proposal) = self.proposal_coordinator.proposal(proposal_id)
            && let Err(failure) = SaveWorkflowService::observe_proposal_response(
                &mut self.proposal_coordinator,
                &self.storage,
                &proposal,
                &response,
                None,
            )
        {
            return Ok(failure);
        }
        Ok(response)
    }

    /// Register an externally constructed proposal into the app lifecycle before validation.
    pub fn register_proposal_lifecycle(
        &mut self,
        proposal: &WorkspaceProposal,
    ) -> Result<ProposalResponse, AppCompositionError> {
        self.proposal_coordinator.register_lifecycle_context(
            proposal.proposal_id,
            EventContext::new(proposal.correlation_id),
        );
        let response = self.proposal_coordinator.created_response(proposal);
        let _ = SaveWorkflowService::observe_proposal_response(
            &mut self.proposal_coordinator,
            &self.storage,
            proposal,
            &response,
            None,
        );
        Ok(response)
    }

    fn apply_workspace_proposal(
        &mut self,
        proposal: WorkspaceProposal,
    ) -> Result<ProposalResponse, AppCompositionError> {
        self.proposal_coordinator.remember_proposal(&proposal);
        if !self
            .proposal_coordinator
            .has_lifecycle_context(proposal.proposal_id)
        {
            let response = self
                .proposal_coordinator
                .missing_lifecycle_context_response(&proposal, "apply");
            let _ = SaveWorkflowService::observe_proposal_response(
                &mut self.proposal_coordinator,
                &self.storage,
                &proposal,
                &response,
                None,
            );
            return Ok(response);
        }

        if !matches!(
            self.proposal_coordinator
                .current_lifecycle_state(proposal.proposal_id),
            Some(ProposalLifecycleState::Previewed | ProposalLifecycleState::Approved)
        ) {
            let response = self
                .proposal_coordinator
                .invalid_lifecycle_transition_response(
                    &proposal,
                    "apply",
                    self.proposal_coordinator
                        .current_lifecycle_state(proposal.proposal_id),
                    ProposalLifecycleState::Applied,
                );
            let _ = SaveWorkflowService::observe_proposal_response(
                &mut self.proposal_coordinator,
                &self.storage,
                &proposal,
                &response,
                None,
            );
            return Ok(response);
        }

        let rollback = match self.rollback_snapshot_for_proposal(&proposal) {
            Ok(rollback) => rollback,
            Err(response) => {
                let _ = SaveWorkflowService::observe_proposal_response(
                    &mut self.proposal_coordinator,
                    &self.storage,
                    &proposal,
                    &response,
                    None,
                );
                return Ok(response);
            }
        };
        let mut deferred_save_success = None;
        let response = match &proposal.payload {
            ProposalPayload::TextEdit(payload) => self.apply_text_edit_proposal(&proposal, payload),
            ProposalPayload::CreateFile(payload) => {
                self.apply_create_file_proposal(&proposal, payload)
            }
            ProposalPayload::DeleteFile(payload) => {
                self.apply_delete_file_proposal(&proposal, payload)
            }
            ProposalPayload::RenameFile(payload) => {
                self.apply_rename_file_proposal(&proposal, payload)
            }
            ProposalPayload::SaveFile(payload) => {
                let (response, save_success) = self.apply_save_file_proposal(&proposal, payload);
                deferred_save_success = save_success;
                response
            }
            ProposalPayload::WorkspaceEdit(payload) => {
                self.apply_workspace_edit_proposal(&proposal, payload)
            }
            ProposalPayload::Batch(_) => self
                .proposal_coordinator
                .unsupported_response(&proposal, "apply"),
            _ => self
                .proposal_coordinator
                .unsupported_response(&proposal, "apply"),
        };

        if let Err(mut failure) = SaveWorkflowService::observe_proposal_response(
            &mut self.proposal_coordinator,
            &self.storage,
            &proposal,
            &response,
            None,
        ) {
            let rollback_diagnostics = self.rollback_audit_failed_mutation(
                &proposal,
                rollback,
                deferred_save_success.as_ref(),
            );
            Self::append_response_diagnostics(&mut failure, rollback_diagnostics);
            Ok(failure)
        } else {
            if let Some(save_success) = deferred_save_success {
                self.commit_deferred_save_success(save_success);
            }
            Ok(response)
        }
    }

    #[allow(clippy::result_large_err)]
    fn rollback_snapshot_for_proposal(
        &self,
        proposal: &WorkspaceProposal,
    ) -> Result<ProposalMutationRollback, ProposalResponse> {
        match &proposal.payload {
            ProposalPayload::TextEdit(_) => Ok(ProposalMutationRollback::TextEdit),
            ProposalPayload::CreateFile(payload) => Ok(ProposalMutationRollback::CreatedFile {
                path: payload.path.clone(),
            }),
            ProposalPayload::DeleteFile(payload) => self
                .rollback_text_snapshot(proposal, &payload.file.canonical_path)
                .map(|text| ProposalMutationRollback::DeletedFile {
                    path: payload.file.canonical_path.clone(),
                    text,
                }),
            ProposalPayload::RenameFile(payload) => Ok(ProposalMutationRollback::RenamedFile {
                source: payload.file.canonical_path.clone(),
                destination: payload.destination.clone(),
            }),
            ProposalPayload::SaveFile(payload) => self
                .rollback_text_snapshot(proposal, &payload.file.canonical_path)
                .map(|text| ProposalMutationRollback::SavedFile {
                    path: payload.file.canonical_path.clone(),
                    text,
                }),
            ProposalPayload::WorkspaceEdit(payload) => {
                self.rollback_snapshot_for_workspace_edit(proposal, payload)
            }
            _ => Ok(ProposalMutationRollback::None),
        }
    }

    #[allow(clippy::result_large_err)]
    fn rollback_snapshot_for_workspace_edit(
        &self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::WorkspaceEditProposalPayload,
    ) -> Result<ProposalMutationRollback, ProposalResponse> {
        if !payload.file_edits.is_empty() || payload.file_operations.len() != 1 {
            return Ok(ProposalMutationRollback::None);
        }

        match &payload.file_operations[0] {
            devil_protocol::WorkspaceFileOperation::Create { path, .. } => {
                Ok(ProposalMutationRollback::CreatedFile { path: path.clone() })
            }
            devil_protocol::WorkspaceFileOperation::Delete { file } => self
                .rollback_text_snapshot(proposal, &file.canonical_path)
                .map(|text| ProposalMutationRollback::DeletedFile {
                    path: file.canonical_path.clone(),
                    text,
                }),
            devil_protocol::WorkspaceFileOperation::Rename { file, destination } => {
                Ok(ProposalMutationRollback::RenamedFile {
                    source: file.canonical_path.clone(),
                    destination: destination.clone(),
                })
            }
        }
    }

    #[allow(clippy::result_large_err)]
    fn rollback_text_snapshot(
        &self,
        proposal: &WorkspaceProposal,
        path: &CanonicalPath,
    ) -> Result<String, ProposalResponse> {
        std::fs::read_to_string(&path.0).map_err(|error| {
            self.failed_apply_response(
                proposal,
                "proposal.rollback_snapshot_unavailable",
                format!("apply requires a pre-mutation rollback snapshot: {error}"),
            )
        })
    }

    fn rollback_audit_failed_mutation(
        &mut self,
        proposal: &WorkspaceProposal,
        rollback: ProposalMutationRollback,
        deferred_save_success: Option<&DeferredSaveSuccess>,
    ) -> Vec<ProtocolDiagnostic> {
        let mut diagnostics = Vec::new();
        match rollback {
            ProposalMutationRollback::None => {}
            ProposalMutationRollback::TextEdit => self.rollback_audit_failed_text_edit(proposal),
            ProposalMutationRollback::CreatedFile { path } => {
                if let Err(error) = std::fs::remove_file(&path.0) {
                    diagnostics.push(Self::rollback_failed_diagnostic(
                        "proposal.audit_rollback_remove_failed",
                        &path,
                        error,
                    ));
                }
                self.refresh_workspace_after_audit_rollback(proposal);
            }
            ProposalMutationRollback::DeletedFile { path, text }
            | ProposalMutationRollback::SavedFile { path, text } => {
                if let Err(error) = std::fs::write(&path.0, text) {
                    diagnostics.push(Self::rollback_failed_diagnostic(
                        "proposal.audit_rollback_write_failed",
                        &path,
                        error,
                    ));
                }
                self.refresh_workspace_after_audit_rollback(proposal);
            }
            ProposalMutationRollback::RenamedFile {
                source,
                destination,
            } => {
                if Path::new(&destination.0).exists() {
                    if let Err(error) = std::fs::rename(&destination.0, &source.0) {
                        diagnostics.push(Self::rollback_failed_diagnostic(
                            "proposal.audit_rollback_rename_failed",
                            &destination,
                            error,
                        ));
                    }
                } else {
                    diagnostics.push(ProtocolDiagnostic {
                        code: "proposal.audit_rollback_rename_missing_destination".to_string(),
                        message: format!(
                            "audit failure rollback could not restore rename because destination '{}' is missing",
                            &destination.0
                        ),
                        severity: ProtocolDiagnosticSeverity::Error,
                        path: Some(destination),
                        range: None,
                    });
                }
                self.refresh_workspace_after_audit_rollback(proposal);
            }
        }

        if let Some(save_success) = deferred_save_success {
            self.editor.acknowledge_save_outcome(
                save_success.request_id,
                SaveAcknowledgement::Failed {
                    diagnostics: Vec::new(),
                },
            );
        }

        diagnostics
    }

    fn rollback_failed_diagnostic(
        code: &str,
        path: &CanonicalPath,
        error: std::io::Error,
    ) -> ProtocolDiagnostic {
        ProtocolDiagnostic {
            code: code.to_string(),
            message: format!(
                "audit failure rollback did not restore '{}': {error}",
                path.0
            ),
            severity: ProtocolDiagnosticSeverity::Error,
            path: Some(path.clone()),
            range: None,
        }
    }

    fn append_response_diagnostics(
        response: &mut ProposalResponse,
        diagnostics: Vec<ProtocolDiagnostic>,
    ) {
        if diagnostics.is_empty() {
            return;
        }

        match response {
            ProposalResponse::Created(transition)
            | ProposalResponse::Validated(transition)
            | ProposalResponse::Approved(transition)
            | ProposalResponse::Applied(transition) => transition.diagnostics.extend(diagnostics),
            ProposalResponse::Previewed { transition, .. }
            | ProposalResponse::Rejected { transition, .. }
            | ProposalResponse::Denied { transition, .. }
            | ProposalResponse::Failed { transition, .. }
            | ProposalResponse::RolledBack { transition, .. }
            | ProposalResponse::Stale { transition, .. }
            | ProposalResponse::Conflict { transition, .. }
            | ProposalResponse::Cancelled { transition, .. } => {
                transition.diagnostics.extend(diagnostics);
            }
        }
    }

    fn rollback_audit_failed_text_edit(&mut self, proposal: &WorkspaceProposal) {
        let ProposalPayload::TextEdit(payload) = &proposal.payload else {
            return;
        };
        let Some(workspace_id) = self.active_documents.workspace_id() else {
            return;
        };
        let Some(buffer_id) = self.editor.buffer_for_file(workspace_id, payload.file_id) else {
            return;
        };
        if let Ok(record) = self.editor.undo(buffer_id, Some(proposal.correlation_id)) {
            let descriptor = record.to_protocol_descriptor();
            self.emit_transaction_event(&descriptor);
        }
    }

    fn refresh_workspace_after_audit_rollback(&mut self, proposal: &WorkspaceProposal) {
        let Some(opened) = self.active_documents.opened_workspace.clone() else {
            return;
        };
        let Some(root_path) = self.active_documents.workspace_root_path.clone() else {
            return;
        };
        let principal = self
            .active_documents
            .active_principal_id
            .clone()
            .unwrap_or_else(|| proposal.principal.clone());
        let trust = self
            .active_documents
            .active_workspace_trust
            .clone()
            .unwrap_or(WorkspaceTrustState::Unknown);

        let _ = self
            .workspace
            .handle(WorkspaceRequest::Close(WorkspaceCloseRequest {
                workspace_id: opened.workspace_id,
                correlation_id: proposal.correlation_id,
                principal_id: principal.clone(),
            }));
        if let Ok(WorkspaceResponse::Opened(reopened)) =
            self.workspace
                .handle(WorkspaceRequest::Open(WorkspaceOpenRequest {
                    correlation_id: proposal.correlation_id,
                    principal_id: principal.clone(),
                    root_path: CanonicalPath(root_path.clone()),
                    trust: Some(trust.clone()),
                }))
        {
            self.active_documents.bind_workspace(
                reopened,
                CanonicalPath(root_path),
                principal,
                trust,
            );
        }
    }

    fn commit_deferred_save_success(&mut self, save_success: DeferredSaveSuccess) {
        self.editor
            .acknowledge_save_outcome(save_success.request_id, SaveAcknowledgement::Saved);
        self.active_documents
            .bind_saved_buffer(save_success.buffer_id, save_success.applied);
    }

    fn proposal_causality_id(&self, proposal: &WorkspaceProposal) -> CausalityId {
        self.proposal_coordinator
            .transition(proposal, ProposalLifecycleState::Applied, Vec::new())
            .causality_id
    }

    fn applied_response(&self, proposal: &WorkspaceProposal) -> ProposalResponse {
        match self.proposal_coordinator.record_transition(
            proposal,
            ProposalLifecycleState::Applied,
            "apply",
        ) {
            Ok(transition) => ProposalResponse::Applied(transition),
            Err(response) => response,
        }
    }

    fn failed_apply_response(
        &self,
        proposal: &WorkspaceProposal,
        code: &str,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = AppProposalCoordinator::diagnostic(code, message);
        match self
            .proposal_coordinator
            .record_transition_with_diagnostics(
                proposal,
                ProposalLifecycleState::Failed,
                "apply",
                vec![diagnostic],
            ) {
            Ok(transition) => ProposalResponse::Failed {
                transition,
                reason: ProposalFailureReason::ApplyFailed,
            },
            Err(response) => response,
        }
    }

    fn denied_apply_response(
        &self,
        proposal: &WorkspaceProposal,
        code: &str,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = AppProposalCoordinator::diagnostic(code, message);
        match self
            .proposal_coordinator
            .record_transition_with_diagnostics(
                proposal,
                ProposalLifecycleState::Denied,
                "apply",
                vec![diagnostic],
            ) {
            Ok(transition) => ProposalResponse::Denied {
                transition,
                reason: ProposalDenialReason::PolicyDenied,
            },
            Err(response) => response,
        }
    }

    fn stale_apply_response(
        &self,
        proposal: &WorkspaceProposal,
        reason: ProposalStaleReason,
        actual: Option<VersionContext>,
        message: impl Into<String>,
    ) -> ProposalResponse {
        let diagnostic = AppProposalCoordinator::diagnostic("proposal.stale", message);
        match self
            .proposal_coordinator
            .record_transition_with_diagnostics(
                proposal,
                ProposalLifecycleState::Stale,
                "apply",
                vec![diagnostic],
            ) {
            Ok(transition) => ProposalResponse::Stale {
                transition,
                stale: devil_protocol::ProposalStaleContext {
                    reason,
                    expected: proposal.preconditions.clone(),
                    actual,
                },
            },
            Err(response) => response,
        }
    }

    fn active_file_version_context(
        &self,
        buffer_id: BufferId,
    ) -> Result<VersionContext, AppCompositionError> {
        let buffer_version = self.editor.buffer_version(buffer_id)?;
        let snapshot_id = self.editor.current_snapshot(buffer_id)?.snapshot_id;
        let metadata = self.active_documents.metadata_for_buffer(buffer_id);
        Ok(VersionContext {
            file_version: metadata
                .map(|metadata| metadata.file_content_version)
                .unwrap_or(FileContentVersion(0)),
            buffer_version,
            snapshot_id,
            generation: metadata
                .map(|metadata| metadata.workspace_generation)
                .unwrap_or(WorkspaceGeneration(0)),
            file_content_version: metadata
                .map(|metadata| metadata.file_content_version)
                .unwrap_or(FileContentVersion(0)),
            workspace_generation: metadata
                .map(|metadata| metadata.workspace_generation)
                .unwrap_or(WorkspaceGeneration(0)),
            fingerprint: metadata.map(|metadata| metadata.fingerprint.clone()),
            file_length: metadata.and_then(|metadata| metadata.file_length),
            modified_at: metadata.and_then(|metadata| metadata.modified_at),
        })
    }

    #[allow(clippy::result_large_err)]
    fn closed_file_preconditions(
        &self,
        proposal: &WorkspaceProposal,
    ) -> Result<(FileContentVersion, WorkspaceGeneration, FileFingerprint), ProposalResponse> {
        let Some(file_content_version) = proposal
            .preconditions
            .file_content_version
            .or(proposal.preconditions.file_version)
        else {
            return Err(self.failed_apply_response(
                proposal,
                "proposal.missing_file_precondition",
                "apply requires file content version precondition",
            ));
        };
        let Some(workspace_generation) = proposal
            .preconditions
            .workspace_generation
            .or(proposal.preconditions.generation)
        else {
            return Err(self.failed_apply_response(
                proposal,
                "proposal.missing_workspace_precondition",
                "apply requires workspace generation precondition",
            ));
        };
        let Some(fingerprint) = proposal.preconditions.expected_fingerprint.clone() else {
            return Err(self.failed_apply_response(
                proposal,
                "proposal.missing_fingerprint",
                "apply requires expected fingerprint precondition",
            ));
        };
        Ok((file_content_version, workspace_generation, fingerprint))
    }

    fn reject_open_file_mutation(
        &self,
        proposal: &WorkspaceProposal,
        file: &FileIdentity,
    ) -> Option<ProposalResponse> {
        let open_buffer = self
            .editor
            .buffer_for_file(file.workspace_id, file.file_id)
            .or_else(|| {
                self.editor
                    .buffer_for_path(file.workspace_id, &file.canonical_path.0)
            })
            .or_else(|| {
                let active_path = self.active_documents.active_file_path.as_deref()?;
                if self.active_documents.workspace_id() == Some(file.workspace_id)
                    && Self::paths_equivalent(active_path, &file.canonical_path.0)
                {
                    self.active_documents.active_buffer_id
                } else {
                    None
                }
            });

        open_buffer.map(|_| {
                self.denied_apply_response(
                    proposal,
                    "proposal.open_file_workspace_mutation_denied",
                    "closed-file workspace mutation is denied while the target file is open in the editor",
                )
            })
    }

    fn reject_open_path_mutation(
        &self,
        proposal: &WorkspaceProposal,
        workspace_id: WorkspaceId,
        path: &CanonicalPath,
    ) -> Option<ProposalResponse> {
        let open_buffer = self
            .editor
            .buffer_for_path(workspace_id, &path.0)
            .or_else(|| {
                let active_path = self.active_documents.active_file_path.as_deref()?;
                if self.active_documents.workspace_id() == Some(workspace_id)
                    && Self::paths_equivalent(active_path, &path.0)
                {
                    self.active_documents.active_buffer_id
                } else {
                    None
                }
            });

        open_buffer.map(|_| {
            self.denied_apply_response(
                proposal,
                "proposal.open_file_workspace_mutation_denied",
                "closed-file workspace mutation is denied while the target path is open in the editor",
            )
        })
    }

    fn paths_equivalent(left: &str, right: &str) -> bool {
        if left == right || Path::new(left) == Path::new(right) {
            return true;
        }

        match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
            (Ok(left), Ok(right)) => left == right,
            _ => false,
        }
    }

    fn stale_text_edit_precondition_response(
        &self,
        proposal: &WorkspaceProposal,
        actual: &VersionContext,
    ) -> Option<ProposalResponse> {
        if proposal.preconditions.buffer_version != Some(actual.buffer_version) {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::BufferVersionMismatch,
                Some(actual.clone()),
                "buffer version changed before text edit apply",
            ));
        }
        if proposal.preconditions.snapshot_id != Some(actual.snapshot_id) {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::SnapshotMismatch,
                Some(actual.clone()),
                "snapshot changed before text edit apply",
            ));
        }
        if let Some(expected) = proposal
            .preconditions
            .file_content_version
            .or(proposal.preconditions.file_version)
            && expected != actual.file_content_version
        {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::FileContentVersionMismatch,
                Some(actual.clone()),
                "file content version changed before text edit apply",
            ));
        }
        if let Some(expected) = proposal
            .preconditions
            .workspace_generation
            .or(proposal.preconditions.generation)
            && expected != actual.workspace_generation
        {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::WorkspaceGenerationMismatch,
                Some(actual.clone()),
                "workspace generation changed before text edit apply",
            ));
        }
        if let Some(expected) = &proposal.preconditions.expected_fingerprint
            && actual.fingerprint.as_ref() != Some(expected)
        {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::FingerprintMismatch,
                Some(actual.clone()),
                "file fingerprint changed before text edit apply",
            ));
        }
        if let Some(expected) = proposal.preconditions.expected_file_length
            && actual.file_length != Some(expected)
        {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::FileLengthMismatch,
                Some(actual.clone()),
                "file length changed before text edit apply",
            ));
        }
        if let Some(expected) = proposal.preconditions.expected_modified_at
            && actual.modified_at != Some(expected)
        {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::ModifiedTimestampMismatch,
                Some(actual.clone()),
                "modified timestamp changed before text edit apply",
            ));
        }

        None
    }

    fn apply_text_edit_proposal(
        &mut self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::TextEditProposal,
    ) -> ProposalResponse {
        let workspace_id = match self.active_documents.require_workspace_id() {
            Ok(workspace_id) => workspace_id,
            Err(err) => {
                return self.failed_apply_response(
                    proposal,
                    "proposal.workspace_missing",
                    err.to_string(),
                );
            }
        };
        let Some(buffer_id) = self.editor.buffer_for_file(workspace_id, payload.file_id) else {
            return self.failed_apply_response(
                proposal,
                "proposal.closed_file_text_edit_denied",
                "text edit apply requires an open editor buffer in Stage 1C",
            );
        };
        let actual = match self.active_file_version_context(buffer_id) {
            Ok(actual) => actual,
            Err(err) => {
                return self.failed_apply_response(
                    proposal,
                    "proposal.editor_state_unavailable",
                    err.to_string(),
                );
            }
        };
        if let Some(response) = self.stale_text_edit_precondition_response(proposal, &actual) {
            return response;
        }

        match self
            .editor
            .apply_protocol_edits(EditorApplyTransactionRequest {
                workspace_id,
                buffer_id,
                file_id: payload.file_id,
                edits: payload.edits.clone(),
                source: TransactionSource::System,
                undo_group_id: Some(uuid::Uuid::now_v7()),
                correlation_id: proposal.correlation_id,
            }) {
            Ok(record) => {
                let descriptor = record.to_protocol_descriptor();
                self.emit_transaction_event(&descriptor);
                self.applied_response(proposal)
            }
            Err(err) => self.failed_apply_response(
                proposal,
                "proposal.editor_apply_failed",
                err.to_string(),
            ),
        }
    }

    fn apply_create_file_proposal(
        &mut self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::CreateFileProposal,
    ) -> ProposalResponse {
        let workspace_id = match self.active_documents.require_workspace_id() {
            Ok(workspace_id) => workspace_id,
            Err(err) => {
                return self.failed_apply_response(
                    proposal,
                    "proposal.workspace_missing",
                    err.to_string(),
                );
            }
        };
        let Some(expected_workspace_generation) = proposal
            .preconditions
            .workspace_generation
            .or(proposal.preconditions.generation)
        else {
            return self.failed_apply_response(
                proposal,
                "proposal.missing_workspace_precondition",
                "create-file apply requires workspace generation precondition",
            );
        };
        if let Some(response) =
            self.reject_open_path_mutation(proposal, workspace_id, &payload.path)
        {
            return response;
        }
        let request = WorkspaceCreateFileRequest {
            workspace_id,
            proposal_id: proposal.proposal_id,
            principal: proposal.principal.clone(),
            required_capability: proposal.capability.clone(),
            path: payload.path.clone(),
            expected_workspace_generation,
            initial_content: payload.initial_content.clone().unwrap_or_default(),
            correlation_id: proposal.correlation_id,
            causality_id: self.proposal_causality_id(proposal),
        };
        match self.workspace.create_file_with_proposal(request) {
            Ok(applied) => applied.response,
            Err(response) => response,
        }
    }

    fn apply_delete_file_proposal(
        &mut self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::DeleteFileProposal,
    ) -> ProposalResponse {
        if let Some(response) = self.reject_open_file_mutation(proposal, &payload.file) {
            return response;
        }
        let (file_content_version, workspace_generation, fingerprint) =
            match self.closed_file_preconditions(proposal) {
                Ok(values) => values,
                Err(response) => return response,
            };
        let request = WorkspaceDeleteFileRequest {
            workspace_id: payload.file.workspace_id,
            proposal_id: proposal.proposal_id,
            principal: proposal.principal.clone(),
            required_capability: proposal.capability.clone(),
            file: payload.file.clone(),
            expected_fingerprint: fingerprint,
            expected_file_content_version: file_content_version,
            expected_workspace_generation: workspace_generation,
            correlation_id: proposal.correlation_id,
            causality_id: self.proposal_causality_id(proposal),
        };
        match self.workspace.delete_file_with_proposal(request) {
            Ok(applied) => applied.response,
            Err(response) => response,
        }
    }

    fn apply_rename_file_proposal(
        &mut self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::RenameFileProposal,
    ) -> ProposalResponse {
        if let Some(response) = self.reject_open_file_mutation(proposal, &payload.file) {
            return response;
        }
        let (file_content_version, workspace_generation, fingerprint) =
            match self.closed_file_preconditions(proposal) {
                Ok(values) => values,
                Err(response) => return response,
            };
        let request = WorkspaceRenameFileRequest {
            workspace_id: payload.file.workspace_id,
            proposal_id: proposal.proposal_id,
            principal: proposal.principal.clone(),
            required_capability: proposal.capability.clone(),
            file: payload.file.clone(),
            destination: payload.destination.clone(),
            expected_fingerprint: fingerprint,
            expected_file_content_version: file_content_version,
            expected_workspace_generation: workspace_generation,
            correlation_id: proposal.correlation_id,
            causality_id: self.proposal_causality_id(proposal),
        };
        match self.workspace.rename_file_with_proposal(request) {
            Ok(applied) => applied.response,
            Err(response) => response,
        }
    }

    fn apply_save_file_proposal(
        &mut self,
        proposal: &WorkspaceProposal,
        payload: &SaveFileProposal,
    ) -> (ProposalResponse, Option<DeferredSaveSuccess>) {
        let workspace_id = match self.active_documents.require_workspace_id() {
            Ok(workspace_id) => workspace_id,
            Err(err) => {
                return (
                    self.failed_apply_response(
                        proposal,
                        "proposal.workspace_missing",
                        err.to_string(),
                    ),
                    None,
                );
            }
        };
        if workspace_id != payload.file.workspace_id {
            return (
                self.failed_apply_response(
                    proposal,
                    "proposal.workspace_mismatch",
                    "save-file proposal workspace does not match the active workspace",
                ),
                None,
            );
        }
        let Some(buffer_id) = self
            .editor
            .buffer_for_file(workspace_id, payload.file_id)
            .or_else(|| {
                self.editor
                    .buffer_for_path(workspace_id, &payload.file.canonical_path.0)
            })
        else {
            return (
                self.denied_apply_response(
                    proposal,
                    "proposal.closed_file_save_denied",
                    "save-file apply requires an open editor buffer as the text authority",
                ),
                None,
            );
        };
        if buffer_id != payload.buffer_id {
            return (
                self.failed_apply_response(
                    proposal,
                    "proposal.buffer_mismatch",
                    "save-file payload buffer id does not match the open editor buffer",
                ),
                None,
            );
        }
        let actual = match self.active_file_version_context(buffer_id) {
            Ok(actual) => actual,
            Err(err) => {
                return (
                    self.failed_apply_response(
                        proposal,
                        "proposal.editor_state_unavailable",
                        err.to_string(),
                    ),
                    None,
                );
            }
        };
        if payload.buffer_version != actual.buffer_version
            || proposal.preconditions.buffer_version != Some(actual.buffer_version)
        {
            return (
                self.stale_apply_response(
                    proposal,
                    ProposalStaleReason::BufferVersionMismatch,
                    Some(actual),
                    "buffer version changed before save apply",
                ),
                None,
            );
        }
        if payload.snapshot_id != actual.snapshot_id
            || proposal.preconditions.snapshot_id != Some(actual.snapshot_id)
        {
            return (
                self.stale_apply_response(
                    proposal,
                    ProposalStaleReason::SnapshotMismatch,
                    Some(actual),
                    "snapshot changed before save apply",
                ),
                None,
            );
        }
        if payload.file_content_version != actual.file_content_version
            || proposal
                .preconditions
                .file_content_version
                .or(proposal.preconditions.file_version)
                != Some(actual.file_content_version)
        {
            return (
                self.stale_apply_response(
                    proposal,
                    ProposalStaleReason::FileContentVersionMismatch,
                    Some(actual),
                    "file content version changed before save apply",
                ),
                None,
            );
        }
        if payload.workspace_generation != actual.workspace_generation
            || proposal
                .preconditions
                .workspace_generation
                .or(proposal.preconditions.generation)
                != Some(actual.workspace_generation)
        {
            return (
                self.stale_apply_response(
                    proposal,
                    ProposalStaleReason::WorkspaceGenerationMismatch,
                    Some(actual),
                    "workspace generation changed before save apply",
                ),
                None,
            );
        }
        let Some(expected_fingerprint) = proposal
            .preconditions
            .expected_fingerprint
            .clone()
            .or_else(|| payload.expected_fingerprint.clone())
        else {
            return (
                self.failed_apply_response(
                    proposal,
                    "proposal.missing_fingerprint",
                    "save-file apply requires expected fingerprint precondition",
                ),
                None,
            );
        };

        let save = match self
            .editor
            .request_save(buffer_id, Some(proposal.correlation_id))
        {
            Ok(save) => save,
            Err(err) => {
                return (
                    self.failed_apply_response(
                        proposal,
                        "proposal.editor_save_payload_unavailable",
                        err.to_string(),
                    ),
                    None,
                );
            }
        };
        let request = WorkspaceSaveRequest {
            workspace_id,
            proposal_id: proposal.proposal_id,
            principal: proposal.principal.clone(),
            required_capability: proposal.capability.clone(),
            file_id: payload.file.file_id,
            path: payload.file.canonical_path.clone(),
            expected_fingerprint,
            expected_file_content_version: payload.file_content_version,
            expected_workspace_generation: payload.workspace_generation,
            buffer_version: payload.buffer_version,
            snapshot_id: payload.snapshot_id,
            payload_byte_len: save.payload_byte_len,
            correlation_id: proposal.correlation_id,
            causality_id: self.proposal_causality_id(proposal),
            text: save.text.clone(),
        };

        match self.workspace.save_file_with_proposal(request) {
            Ok(applied) => (
                applied.response.clone(),
                Some(DeferredSaveSuccess {
                    request_id: save.request_id,
                    buffer_id,
                    applied,
                }),
            ),
            Err(response) => {
                self.editor.acknowledge_save_outcome(
                    save.request_id,
                    acknowledgement_for_response(&response),
                );
                (response, None)
            }
        }
    }

    fn apply_workspace_edit_proposal(
        &mut self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::WorkspaceEditProposalPayload,
    ) -> ProposalResponse {
        if !payload.file_edits.is_empty() || payload.file_operations.len() != 1 {
            return self
                .proposal_coordinator
                .unsupported_response(proposal, "apply");
        }
        let workspace_id = match self.active_documents.require_workspace_id() {
            Ok(workspace_id) => workspace_id,
            Err(err) => {
                return self.failed_apply_response(
                    proposal,
                    "proposal.workspace_missing",
                    err.to_string(),
                );
            }
        };
        if payload.workspace_id != workspace_id {
            return self.failed_apply_response(
                proposal,
                "proposal.workspace_mismatch",
                "workspace-edit payload workspace does not match the active workspace",
            );
        }

        match &payload.file_operations[0] {
            devil_protocol::WorkspaceFileOperation::Create {
                path,
                initial_content_hash,
            } => {
                if initial_content_hash.is_some() {
                    return self
                        .proposal_coordinator
                        .unsupported_response(proposal, "apply");
                }
                let create = devil_protocol::CreateFileProposal {
                    path: path.clone(),
                    initial_content: Some(String::new()),
                };
                self.apply_create_file_proposal(proposal, &create)
            }
            devil_protocol::WorkspaceFileOperation::Delete { file } => {
                let delete = devil_protocol::DeleteFileProposal { file: file.clone() };
                self.apply_delete_file_proposal(proposal, &delete)
            }
            devil_protocol::WorkspaceFileOperation::Rename { file, destination } => {
                let rename = devil_protocol::RenameFileProposal {
                    file: file.clone(),
                    destination: destination.clone(),
                };
                self.apply_rename_file_proposal(proposal, &rename)
            }
        }
    }

    /// Build deterministic affected-target coverage for a proposal without executing it.
    pub fn proposal_target_coverage(&self, proposal: &WorkspaceProposal) -> ProposalTargetCoverage {
        AppProposalCoordinator::affected_target_coverage(proposal)
    }

    fn preflight_batch_structure(
        &self,
        batch: &BatchProposalPayload,
        plan: &mut BatchPreflightPlan,
    ) {
        if batch.items.is_empty() {
            plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.empty_batch",
                "batch preflight requires at least one item",
            ));
        }
        if batch.target_coverage.coverage_kind != ProposalTargetCoverageKind::Complete
            || batch.target_coverage.targets.is_empty()
            || batch.target_coverage.omitted_target_count != 0
        {
            plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.incomplete_target_coverage",
                "batch preflight requires complete non-empty target coverage with no omissions",
            ));
        }

        let mut item_ids = HashSet::new();
        for item in &batch.items {
            if item.item_id.trim().is_empty() || !item_ids.insert(item.item_id.as_str()) {
                plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.invalid_batch_item_id",
                    format!("batch item id '{}' is empty or duplicated", item.item_id),
                ));
            }
        }

        let mut target_ids = HashSet::new();
        let mut target_resources = HashSet::new();
        for target in &batch.target_coverage.targets {
            if target.target_id.trim().is_empty() || !target_ids.insert(target.target_id.as_str()) {
                plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.invalid_batch_target_id",
                    format!(
                        "batch target id '{}' is empty or duplicated",
                        target.target_id
                    ),
                ));
            }
            if let Some(resource_key) = AppProposalCoordinator::target_resource_key(target)
                && !target_resources.insert(resource_key.clone())
            {
                plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.duplicate_target",
                    format!(
                        "batch target resource '{resource_key}' is duplicated across nested targets"
                    ),
                ));
            }
            if !matches!(
                target.kind,
                ProposalTargetKind::OpenBuffer
                    | ProposalTargetKind::ClosedFile
                    | ProposalTargetKind::PathOnly
            ) {
                plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.unsupported_batch_target_kind",
                    format!(
                        "batch target {} kind {:?} is not executable in Stage 1D preflight",
                        target.target_id, target.kind
                    ),
                ));
            }
        }

        for edge in &batch.dependency_edges {
            if !item_ids.contains(edge.prerequisite_item_id.as_str())
                || !item_ids.contains(edge.dependent_item_id.as_str())
            {
                plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.unknown_batch_dependency",
                    format!(
                        "dependency edge {} -> {} references an unknown item id",
                        edge.prerequisite_item_id, edge.dependent_item_id
                    ),
                ));
            }
        }
        if Self::batch_has_dependency_cycle(batch, &item_ids) {
            plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.batch_dependency_cycle",
                "batch dependency graph contains a cycle",
            ));
        }

        if batch.atomicity == ProposalBatchAtomicity::AllOrNothing
            || batch.rollback_policy == ProposalBatchRollbackPolicy::Required
        {
            let rollback_steps = batch
                .rollback_steps
                .iter()
                .map(|step| (step.step_id.as_str(), step))
                .collect::<HashMap<_, _>>();
            for item in &batch.items {
                if item.rollback_step_ids.is_empty()
                    || item
                        .rollback_step_ids
                        .iter()
                        .any(|step_id| !rollback_steps.contains_key(step_id.as_str()))
                {
                    plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                        "proposal.missing_rollback_proof",
                        format!(
                            "batch item {} lacks resolvable rollback step ids required by atomicity/rollback policy",
                            item.item_id
                        ),
                    ));
                }
                for step_id in &item.rollback_step_ids {
                    if let Some(step) = rollback_steps.get(step_id.as_str())
                        && (step.item_id != item.item_id
                            || !item
                                .target_ids
                                .iter()
                                .any(|target_id| target_id == &step.target_id)
                            || !Self::rollback_action_matches_route(
                                Self::batch_item_route(item.payload.as_ref()),
                                step.action,
                            )
                            || !step.diagnostics.is_empty())
                    {
                        plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                            "proposal.unresolved_rollback_step",
                            format!(
                                "rollback step {} does not exactly resolve for batch item {}",
                                step.step_id, item.item_id
                            ),
                        ));
                    }
                }
            }
        }

        if batch.rollback_policy == ProposalBatchRollbackPolicy::NotSupported
            && batch.atomicity != ProposalBatchAtomicity::OrderedNonAtomic
        {
            plan.diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.unsupported_rollback_policy",
                "rollback NotSupported cannot be combined with stronger-than OrderedNonAtomic atomicity",
            ));
        }
    }

    fn batch_execution_stages(preflight_ok: bool) -> Vec<BatchExecutionStageContract> {
        vec![
            BatchExecutionStageContract {
                stage: BatchExecutionStage::Prepare,
                required: true,
                blocked: false,
                diagnostics: Vec::new(),
            },
            BatchExecutionStageContract {
                stage: BatchExecutionStage::Preflight,
                required: true,
                blocked: !preflight_ok,
                diagnostics: if preflight_ok {
                    Vec::new()
                } else {
                    vec![AppProposalCoordinator::diagnostic(
                        "proposal.batch_contract_preflight_failed",
                        "batch execution cannot proceed because preflight did not pass",
                    )]
                },
            },
            BatchExecutionStageContract {
                stage: BatchExecutionStage::Mutate,
                required: true,
                blocked: true,
                diagnostics: vec![AppProposalCoordinator::diagnostic(
                    "proposal.batch_runtime_apply_disabled",
                    "runtime batch mutation remains disabled in Stage 1E",
                )],
            },
            BatchExecutionStageContract {
                stage: BatchExecutionStage::Commit,
                required: true,
                blocked: true,
                diagnostics: vec![AppProposalCoordinator::diagnostic(
                    "proposal.batch_commit_blocked",
                    "batch commit is blocked until mutation results and rollback boundaries are proven",
                )],
            },
            BatchExecutionStageContract {
                stage: BatchExecutionStage::Audit,
                required: true,
                blocked: true,
                diagnostics: vec![AppProposalCoordinator::diagnostic(
                    "proposal.batch_audit_before_success_required",
                    "batch success must be blocked until durable audit proof exists",
                )],
            },
            BatchExecutionStageContract {
                stage: BatchExecutionStage::Finalize,
                required: true,
                blocked: true,
                diagnostics: vec![AppProposalCoordinator::diagnostic(
                    "proposal.batch_finalize_blocked",
                    "batch finalize is blocked until mutation, commit, and audit proof exist",
                )],
            },
            BatchExecutionStageContract {
                stage: BatchExecutionStage::Rollback,
                required: true,
                blocked: true,
                diagnostics: vec![AppProposalCoordinator::diagnostic(
                    "proposal.batch_rollback_runtime_disabled",
                    "runtime batch rollback remains disabled while exact rollback is only contract-validated",
                )],
            },
        ]
    }

    fn batch_planning_semantics(batch: &BatchProposalPayload) -> BatchPlanningSemantics {
        if batch.atomicity == ProposalBatchAtomicity::AllOrNothing
            || batch.rollback_policy == ProposalBatchRollbackPolicy::Required
        {
            BatchPlanningSemantics::Atomic
        } else if batch.rollback_policy == ProposalBatchRollbackPolicy::NotRequired {
            BatchPlanningSemantics::DryRun
        } else {
            BatchPlanningSemantics::BestEffort
        }
    }

    fn batch_rollback_contract(batch: &BatchProposalPayload) -> BatchRollbackContract {
        let semantics = Self::batch_planning_semantics(batch);
        let steps_by_id = batch
            .rollback_steps
            .iter()
            .map(|step| (step.step_id.as_str(), step))
            .collect::<HashMap<_, _>>();
        let reversible_items = batch
            .items
            .iter()
            .filter(|item| Self::route_is_reversible(Self::batch_item_route(item.payload.as_ref())))
            .collect::<Vec<_>>();
        let mut step_contracts = Vec::new();
        let mut diagnostics = Vec::new();

        for item in &reversible_items {
            if matches!(semantics, BatchPlanningSemantics::Atomic)
                && item.rollback_step_ids.is_empty()
            {
                diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.missing_rollback_proof",
                    format!(
                        "atomic batch item {} requires rollback steps before mutation",
                        item.item_id
                    ),
                ));
            }

            for step_id in &item.rollback_step_ids {
                let Some(step) = steps_by_id.get(step_id.as_str()) else {
                    diagnostics.push(AppProposalCoordinator::diagnostic(
                        "proposal.missing_rollback_proof",
                        format!(
                            "batch item {} references unknown rollback step {}",
                            item.item_id, step_id
                        ),
                    ));
                    continue;
                };
                let route = Self::batch_item_route(item.payload.as_ref());
                let exact = step.item_id == item.item_id
                    && item
                        .target_ids
                        .iter()
                        .any(|target_id| target_id == &step.target_id)
                    && Self::rollback_action_matches_route(route, step.action)
                    && step.diagnostics.is_empty();
                let mut step_diagnostics = step.diagnostics.clone();
                if !exact {
                    step_diagnostics.push(AppProposalCoordinator::diagnostic(
                        "proposal.unresolved_rollback_step",
                        format!(
                            "rollback step {} does not exactly resolve for batch item {}",
                            step.step_id, item.item_id
                        ),
                    ));
                }
                step_contracts.push(BatchRollbackStepContract {
                    step_id: step.step_id.clone(),
                    item_id: step.item_id.clone(),
                    target_id: step.target_id.clone(),
                    action: step.action,
                    exact,
                    diagnostics: step_diagnostics,
                });
            }
        }

        let all_reversible_items_proven = reversible_items.iter().all(|item| {
            !item.rollback_step_ids.is_empty()
                && item.rollback_step_ids.iter().all(|step_id| {
                    step_contracts.iter().any(|step| {
                        step.step_id == *step_id && step.item_id == item.item_id && step.exact
                    })
                })
        });
        let status = match semantics {
            BatchPlanningSemantics::Atomic => {
                if all_reversible_items_proven {
                    BatchRollbackContractStatus::Exact
                } else {
                    if diagnostics.is_empty() {
                        diagnostics.push(AppProposalCoordinator::diagnostic(
                            "proposal.missing_rollback_proof",
                            "atomic batch requires exact rollback proof for every reversible item",
                        ));
                    }
                    BatchRollbackContractStatus::Denied
                }
            }
            BatchPlanningSemantics::DryRun => BatchRollbackContractStatus::NotRequired,
            BatchPlanningSemantics::BestEffort => {
                if batch.rollback_policy == ProposalBatchRollbackPolicy::NotSupported {
                    if batch.atomicity == ProposalBatchAtomicity::OrderedNonAtomic {
                        BatchRollbackContractStatus::IrreversibleAccepted
                    } else {
                        diagnostics.push(AppProposalCoordinator::diagnostic(
                            "proposal.unsupported_rollback_policy",
                            "irreversible batch execution is denied unless ordered non-atomic policy accepts it",
                        ));
                        BatchRollbackContractStatus::Denied
                    }
                } else {
                    BatchRollbackContractStatus::BestEffort
                }
            }
        };

        BatchRollbackContract {
            policy: batch.rollback_policy,
            atomicity: batch.atomicity,
            semantics,
            status,
            irreversible_execution_accepted: matches!(
                status,
                BatchRollbackContractStatus::IrreversibleAccepted
            ),
            reversible_item_count: reversible_items.len(),
            steps: step_contracts,
            diagnostics,
        }
    }

    fn route_is_reversible(route: BatchPreflightRoute) -> bool {
        matches!(
            route,
            BatchPreflightRoute::TextEdit
                | BatchPreflightRoute::CreateFile
                | BatchPreflightRoute::DeleteFile
                | BatchPreflightRoute::RenameFile
        )
    }

    fn append_dependency_blocked_failures(
        batch: &BatchProposalPayload,
        plan: &mut BatchPreflightPlan,
    ) {
        let item_order = plan
            .items
            .iter()
            .map(|item| (item.item_id.as_str(), item.order))
            .collect::<HashMap<_, _>>();
        let item_targets = plan
            .items
            .iter()
            .map(|item| {
                (
                    item.item_id.as_str(),
                    item.target_ids.first().cloned().unwrap_or_default(),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut blocked = plan
            .items
            .iter()
            .filter(|item| !item.preflight_ok)
            .map(|item| item.item_id.clone())
            .collect::<HashSet<_>>();
        let mut records = Vec::new();

        loop {
            let mut added = Vec::new();
            for edge in &batch.dependency_edges {
                if !matches!(
                    edge.kind,
                    devil_protocol::ProposalBatchDependencyKind::RequiresValidation
                        | devil_protocol::ProposalBatchDependencyKind::RequiresApply
                ) || !blocked.contains(&edge.prerequisite_item_id)
                    || blocked.contains(&edge.dependent_item_id)
                {
                    continue;
                }
                added.push(edge.dependent_item_id.clone());
                records.push(ProposalPartialFailureRecord {
                    item_id: edge.dependent_item_id.clone(),
                    target_id: item_targets
                        .get(edge.dependent_item_id.as_str())
                        .cloned()
                        .unwrap_or_default(),
                    reason: ProposalFailureReason::ApplyFailed,
                    disposition: ProposalPartialFailureDisposition::NotStarted,
                    diagnostics: vec![AppProposalCoordinator::diagnostic(
                        "proposal.batch_dependency_blocked",
                        format!(
                            "batch item {} was not started because prerequisite item {} failed preflight",
                            edge.dependent_item_id, edge.prerequisite_item_id
                        ),
                    )],
                });
            }
            if added.is_empty() {
                break;
            }
            blocked.extend(added);
        }

        records.sort_by(|left, right| {
            item_order
                .get(left.item_id.as_str())
                .copied()
                .unwrap_or(u32::MAX)
                .cmp(
                    &item_order
                        .get(right.item_id.as_str())
                        .copied()
                        .unwrap_or(u32::MAX),
                )
                .then_with(|| left.item_id.cmp(&right.item_id))
                .then_with(|| left.target_id.cmp(&right.target_id))
        });
        plan.partial_failures.extend(records);
    }

    fn item_has_exact_rollback_proof(
        batch: &BatchProposalPayload,
        item_plan: &BatchPreflightItemPlan,
    ) -> bool {
        let Some(item) = batch
            .items
            .iter()
            .find(|item| item.item_id == item_plan.item_id)
        else {
            return false;
        };
        if item.rollback_step_ids.is_empty() {
            return false;
        }
        let rollback_steps = batch
            .rollback_steps
            .iter()
            .map(|step| (step.step_id.as_str(), step))
            .collect::<HashMap<_, _>>();
        item.rollback_step_ids.iter().all(|step_id| {
            rollback_steps.get(step_id.as_str()).is_some_and(|step| {
                step.item_id == item.item_id
                    && item
                        .target_ids
                        .iter()
                        .any(|target_id| target_id == &step.target_id)
                    && Self::rollback_action_matches_route(item_plan.route, step.action)
                    && step.diagnostics.is_empty()
            })
        })
    }

    fn rollback_action_matches_route(
        route: BatchPreflightRoute,
        action: devil_protocol::ProposalRollbackAction,
    ) -> bool {
        matches!(
            (route, action),
            (
                BatchPreflightRoute::TextEdit,
                devil_protocol::ProposalRollbackAction::EditorUndoGroup
            ) | (
                BatchPreflightRoute::CreateFile,
                devil_protocol::ProposalRollbackAction::DeleteCreatedFile
            ) | (
                BatchPreflightRoute::DeleteFile,
                devil_protocol::ProposalRollbackAction::RecreateDeletedFile
            ) | (
                BatchPreflightRoute::RenameFile,
                devil_protocol::ProposalRollbackAction::RenamePathBack
            )
        )
    }

    fn batch_has_dependency_cycle(batch: &BatchProposalPayload, item_ids: &HashSet<&str>) -> bool {
        fn visit<'a>(
            node: &'a str,
            edges: &HashMap<&'a str, Vec<&'a str>>,
            visiting: &mut HashSet<&'a str>,
            visited: &mut HashSet<&'a str>,
        ) -> bool {
            if visited.contains(node) {
                return false;
            }
            if !visiting.insert(node) {
                return true;
            }
            if let Some(next) = edges.get(node) {
                for child in next {
                    if visit(child, edges, visiting, visited) {
                        return true;
                    }
                }
            }
            visiting.remove(node);
            visited.insert(node);
            false
        }

        let mut edges: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &batch.dependency_edges {
            if item_ids.contains(edge.prerequisite_item_id.as_str())
                && item_ids.contains(edge.dependent_item_id.as_str())
            {
                edges
                    .entry(edge.prerequisite_item_id.as_str())
                    .or_default()
                    .push(edge.dependent_item_id.as_str());
            }
        }
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();
        item_ids
            .iter()
            .any(|item_id| visit(item_id, &edges, &mut visiting, &mut visited))
    }

    fn preflight_batch_item(
        &self,
        proposal: &WorkspaceProposal,
        batch: &BatchProposalPayload,
        item: &ProposalBatchItem,
        coverage_ids: &HashSet<&str>,
        workspace_tree: Option<&[FileTreeNode]>,
    ) -> BatchPreflightItemPlan {
        let route = Self::batch_item_route(item.payload.as_ref());
        let supported = matches!(
            route,
            BatchPreflightRoute::TextEdit
                | BatchPreflightRoute::CreateFile
                | BatchPreflightRoute::DeleteFile
                | BatchPreflightRoute::RenameFile
        );
        let mut diagnostics = Vec::new();

        if item.target_ids.is_empty() {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.missing_batch_item_targets",
                format!(
                    "batch item {} requires at least one target id",
                    item.item_id
                ),
            ));
        }
        for target_id in &item.target_ids {
            if !coverage_ids.contains(target_id.as_str()) {
                diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.unknown_batch_target",
                    format!(
                        "batch item {} references unknown target id {}",
                        item.item_id, target_id
                    ),
                ));
            }
        }
        if !supported {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.unsupported_batch_item_route",
                format!(
                    "batch item {} route {:?} is not executable in Stage 1D preflight",
                    item.item_id, route
                ),
            ));
        }
        if item.required_capability.0.trim().is_empty() {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.missing_batch_item_capability",
                format!(
                    "batch item {} requires a non-empty capability",
                    item.item_id
                ),
            ));
        }
        if let Some(expected) = Self::batch_item_required_capability(route)
            && item.required_capability.0 != expected
        {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.invalid_batch_item_capability",
                format!(
                    "batch item {} route {:?} requires {expected} capability",
                    item.item_id, route
                ),
            ));
        }

        match item.payload.as_ref() {
            ProposalPayload::TextEdit(payload) => {
                self.preflight_text_edit_item(proposal, payload, &mut diagnostics)
            }
            ProposalPayload::CreateFile(payload) => {
                self.preflight_create_file_item(proposal, payload, workspace_tree, &mut diagnostics)
            }
            ProposalPayload::DeleteFile(payload) => {
                self.preflight_delete_file_item(proposal, payload, workspace_tree, &mut diagnostics)
            }
            ProposalPayload::RenameFile(payload) => {
                self.preflight_rename_file_item(proposal, payload, workspace_tree, &mut diagnostics)
            }
            ProposalPayload::Batch(_)
            | ProposalPayload::TerminalCommand(_)
            | ProposalPayload::SaveFile(_)
            | ProposalPayload::FormatFile(_)
            | ProposalPayload::CodeAction(_)
            | ProposalPayload::WorkspaceEdit(_) => {}
        }

        if batch.atomicity == ProposalBatchAtomicity::AllOrNothing
            || batch.rollback_policy == ProposalBatchRollbackPolicy::Required
        {
            let rollback_steps = batch
                .rollback_steps
                .iter()
                .map(|step| (step.step_id.as_str(), step))
                .collect::<HashMap<_, _>>();
            if item.rollback_step_ids.is_empty()
                || item
                    .rollback_step_ids
                    .iter()
                    .any(|step_id| !rollback_steps.contains_key(step_id.as_str()))
            {
                diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.missing_rollback_proof",
                    format!("batch item {} has no exact rollback proof", item.item_id),
                ));
            }
            for step_id in &item.rollback_step_ids {
                if let Some(step) = rollback_steps.get(step_id.as_str())
                    && (step.item_id != item.item_id
                        || !item
                            .target_ids
                            .iter()
                            .any(|target_id| target_id == &step.target_id)
                        || !Self::rollback_action_matches_route(route, step.action)
                        || !step.diagnostics.is_empty())
                {
                    diagnostics.push(AppProposalCoordinator::diagnostic(
                        "proposal.unresolved_rollback_step",
                        format!(
                            "rollback step {} does not exactly resolve for batch item {}",
                            step.step_id, item.item_id
                        ),
                    ));
                }
            }
        }

        BatchPreflightItemPlan {
            item_id: item.item_id.clone(),
            order: item.order,
            route,
            supported,
            preflight_ok: diagnostics.is_empty(),
            target_ids: item.target_ids.clone(),
            diagnostics,
        }
    }

    fn batch_item_route(payload: &ProposalPayload) -> BatchPreflightRoute {
        match payload {
            ProposalPayload::TextEdit(_) => BatchPreflightRoute::TextEdit,
            ProposalPayload::CreateFile(_) => BatchPreflightRoute::CreateFile,
            ProposalPayload::DeleteFile(_) => BatchPreflightRoute::DeleteFile,
            ProposalPayload::RenameFile(_) => BatchPreflightRoute::RenameFile,
            ProposalPayload::Batch(_) => BatchPreflightRoute::Batch,
            ProposalPayload::TerminalCommand(_) => BatchPreflightRoute::Terminal,
            ProposalPayload::SaveFile(_) => BatchPreflightRoute::Save,
            ProposalPayload::FormatFile(_) => BatchPreflightRoute::Format,
            ProposalPayload::CodeAction(_) => BatchPreflightRoute::CodeAction,
            ProposalPayload::WorkspaceEdit(_) => BatchPreflightRoute::WorkspaceEdit,
        }
    }

    fn batch_item_required_capability(route: BatchPreflightRoute) -> Option<&'static str> {
        match route {
            BatchPreflightRoute::TextEdit => Some("editor.write"),
            BatchPreflightRoute::CreateFile
            | BatchPreflightRoute::DeleteFile
            | BatchPreflightRoute::RenameFile => Some("fs.write"),
            BatchPreflightRoute::Batch
            | BatchPreflightRoute::Terminal
            | BatchPreflightRoute::Save
            | BatchPreflightRoute::Format
            | BatchPreflightRoute::CodeAction
            | BatchPreflightRoute::WorkspaceEdit
            | BatchPreflightRoute::Unsupported => None,
        }
    }

    fn read_workspace_tree_for_preflight(
        &self,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) -> Option<Vec<FileTreeNode>> {
        let workspace_id = self.active_documents.workspace_id()?;
        match self
            .workspace
            .handle(WorkspaceRequest::ReadTree(workspace_id))
        {
            Ok(WorkspaceResponse::Tree(tree)) => Some(tree),
            Ok(other) => {
                diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.workspace_tree_unavailable",
                    format!("expected workspace tree during preflight, got {other:?}"),
                ));
                None
            }
            Err(err) => {
                diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.workspace_tree_unavailable",
                    format!("workspace tree unavailable during preflight: {err:?}"),
                ));
                None
            }
        }
    }

    fn preflight_text_edit_item(
        &self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::TextEditProposal,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        let Some(workspace_id) = self.active_documents.workspace_id() else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.workspace_missing",
                "text-edit preflight requires an open workspace",
            ));
            return;
        };
        let Some(buffer_id) = self.editor.buffer_for_file(workspace_id, payload.file_id) else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.closed_file_text_edit_denied",
                "text-edit preflight requires an open editor buffer",
            ));
            return;
        };
        let Ok(actual) = self.active_file_version_context(buffer_id) else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.editor_state_unavailable",
                "text-edit preflight could not read editor version context",
            ));
            return;
        };
        if proposal.preconditions.buffer_version != Some(actual.buffer_version) {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.stale_buffer_version",
                "text-edit preflight buffer version does not match current editor state",
            ));
        }
        if proposal.preconditions.snapshot_id != Some(actual.snapshot_id) {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.stale_snapshot",
                "text-edit preflight snapshot id does not match current editor state",
            ));
        }
        self.preflight_optional_file_preconditions(proposal, &actual, diagnostics);

        let Ok(snapshot) = self.editor.current_snapshot(buffer_id) else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.editor_state_unavailable",
                "text-edit preflight could not read current snapshot descriptor",
            ));
            return;
        };
        for edit in &payload.edits.edits {
            let Some(range) = edit.range.as_byte_range() else {
                diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.invalid_text_edit_range",
                    "text-edit preflight requires byte-coordinate ranges",
                ));
                continue;
            };
            if !range.is_valid() || range.end as usize > snapshot.byte_len {
                diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.invalid_text_edit_range",
                    "text-edit preflight range is outside the current snapshot byte length",
                ));
            }
        }
    }

    fn preflight_optional_file_preconditions(
        &self,
        proposal: &WorkspaceProposal,
        actual: &VersionContext,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        let preconditions = &proposal.preconditions;
        if let Some(expected) = preconditions
            .file_content_version
            .or(preconditions.file_version)
            && expected != actual.file_content_version
        {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.stale_file_content_version",
                "proposal file content version does not match current app context",
            ));
        }
        if let Some(expected) = preconditions
            .workspace_generation
            .or(preconditions.generation)
            && expected != actual.workspace_generation
        {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.stale_workspace_generation",
                "proposal workspace generation does not match current app context",
            ));
        }
        if let Some(expected) = &preconditions.expected_fingerprint
            && actual.fingerprint.as_ref() != Some(expected)
        {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.stale_fingerprint",
                "proposal expected fingerprint does not match current app context",
            ));
        }
    }

    fn preflight_create_file_item(
        &self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::CreateFileProposal,
        workspace_tree: Option<&[FileTreeNode]>,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        self.preflight_workspace_generation(proposal, diagnostics);
        self.preflight_path_inside_workspace(&payload.path, "create-file", diagnostics);
        if payload.path.0.trim().is_empty() {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.missing_path_target",
                "create-file preflight requires a destination path",
            ));
        }
        if let Some(tree) = workspace_tree
            && Self::tree_contains_path(tree, &payload.path)
        {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.destination_exists",
                "create-file preflight destination already exists in the workspace tree",
            ));
        }
        if Path::new(&payload.path.0).exists() {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.destination_exists",
                "create-file preflight destination already exists on disk",
            ));
        }
    }

    fn preflight_delete_file_item(
        &self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::DeleteFileProposal,
        workspace_tree: Option<&[FileTreeNode]>,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        self.preflight_closed_file_item(proposal, &payload.file, workspace_tree, diagnostics);
    }

    fn preflight_rename_file_item(
        &self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::RenameFileProposal,
        workspace_tree: Option<&[FileTreeNode]>,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        self.preflight_closed_file_item(proposal, &payload.file, workspace_tree, diagnostics);
        self.preflight_path_inside_workspace(&payload.destination, "rename-file", diagnostics);
        if payload.destination.0.trim().is_empty() {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.missing_path_target",
                "rename-file preflight requires a destination path",
            ));
        }
        if let Some(tree) = workspace_tree
            && Self::tree_contains_path(tree, &payload.destination)
        {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.destination_exists",
                "rename-file preflight destination already exists in the workspace tree",
            ));
        }
        if Path::new(&payload.destination.0).exists() {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.destination_exists",
                "rename-file preflight destination already exists on disk",
            ));
        }
    }

    fn preflight_closed_file_item(
        &self,
        proposal: &WorkspaceProposal,
        file: &FileIdentity,
        workspace_tree: Option<&[FileTreeNode]>,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        if self.reject_open_file_preflight(file) {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.open_file_workspace_mutation_denied",
                "closed-file preflight denies mutation while the target file is open in the editor",
            ));
        }
        let Some(node) = workspace_tree.and_then(|tree| Self::tree_node_for_file(tree, file))
        else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.file_metadata_missing",
                "closed-file preflight requires current workspace tree metadata for the target",
            ));
            return;
        };
        self.preflight_required_closed_file_preconditions(proposal, node, diagnostics);
    }

    fn preflight_workspace_generation(
        &self,
        proposal: &WorkspaceProposal,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        let Some(expected) = proposal
            .preconditions
            .workspace_generation
            .or(proposal.preconditions.generation)
        else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.missing_workspace_precondition",
                "preflight requires workspace generation precondition",
            ));
            return;
        };
        let Some(opened) = &self.active_documents.opened_workspace else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.workspace_missing",
                "preflight requires an open workspace",
            ));
            return;
        };
        if expected != opened.generation {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.stale_workspace_generation",
                "workspace generation precondition does not match current workspace",
            ));
        }
    }

    fn preflight_path_inside_workspace(
        &self,
        path: &CanonicalPath,
        operation: &str,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        if path.0.trim().is_empty() {
            return;
        }
        let Some(root) = self.active_documents.workspace_root_path.as_deref() else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.workspace_missing",
                format!("{operation} preflight requires an open workspace root"),
            ));
            return;
        };
        if !Self::path_is_inside_root(&path.0, root) {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.path_outside_workspace",
                format!("{operation} preflight path is outside the active workspace"),
            ));
        }
    }

    fn path_is_inside_root(path: &str, root: &str) -> bool {
        let path = Path::new(path);
        let root = Path::new(root);
        if path == root {
            return true;
        }

        if let (Ok(path), Ok(root)) = (std::fs::canonicalize(path), std::fs::canonicalize(root)) {
            return path.starts_with(root);
        }

        let path = Self::normalize_path_components(path);
        let root = Self::normalize_path_components(root);
        path.starts_with(root)
    }

    fn normalize_path_components(path: &Path) -> PathBuf {
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
                Component::RootDir => normalized.push(component.as_os_str()),
                Component::CurDir => {}
                Component::Normal(part) => normalized.push(part),
                Component::ParentDir => {
                    if !normalized.pop() {
                        normalized.push(Component::ParentDir.as_os_str());
                    }
                }
            }
        }
        normalized
    }

    fn preflight_required_closed_file_preconditions(
        &self,
        proposal: &WorkspaceProposal,
        node: &FileTreeNode,
        diagnostics: &mut Vec<ProtocolDiagnostic>,
    ) {
        let Some(expected_file_version) = proposal
            .preconditions
            .file_content_version
            .or(proposal.preconditions.file_version)
        else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.missing_file_precondition",
                "closed-file preflight requires file content version precondition",
            ));
            return;
        };
        let Some(expected_generation) = proposal
            .preconditions
            .workspace_generation
            .or(proposal.preconditions.generation)
        else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.missing_workspace_precondition",
                "closed-file preflight requires workspace generation precondition",
            ));
            return;
        };
        let Some(expected_fingerprint) = &proposal.preconditions.expected_fingerprint else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.missing_fingerprint",
                "closed-file preflight requires expected fingerprint precondition",
            ));
            return;
        };

        if expected_file_version != node.identity.content_version {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.stale_file_content_version",
                "closed-file preflight file content version does not match workspace tree",
            ));
        }
        if let Some(metadata) = &node.metadata {
            if metadata.workspace_generation != Some(expected_generation) {
                diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.stale_workspace_generation",
                    "closed-file preflight workspace generation does not match workspace tree",
                ));
            }
            if metadata.fingerprint.as_ref() != Some(expected_fingerprint) {
                diagnostics.push(AppProposalCoordinator::diagnostic(
                    "proposal.stale_fingerprint",
                    "closed-file preflight fingerprint does not match workspace tree",
                ));
            }
        } else {
            diagnostics.push(AppProposalCoordinator::diagnostic(
                "proposal.file_metadata_missing",
                "closed-file preflight requires fingerprint metadata",
            ));
        }
    }

    fn reject_open_file_preflight(&self, file: &FileIdentity) -> bool {
        self.editor
            .buffer_for_file(file.workspace_id, file.file_id)
            .or_else(|| {
                self.editor
                    .buffer_for_path(file.workspace_id, &file.canonical_path.0)
            })
            .is_some()
    }

    fn tree_node_for_file<'a>(
        tree: &'a [FileTreeNode],
        file: &FileIdentity,
    ) -> Option<&'a FileTreeNode> {
        tree.iter().find(|node| {
            node.identity.file_id == file.file_id
                || Self::paths_equivalent(&node.identity.canonical_path.0, &file.canonical_path.0)
        })
    }

    fn tree_contains_path(tree: &[FileTreeNode], path: &CanonicalPath) -> bool {
        tree.iter()
            .any(|node| Self::paths_equivalent(&node.identity.canonical_path.0, &path.0))
    }

    fn batch_warning(
        code: &str,
        kind: ProposalPreviewWarningKind,
        message: &str,
        target_id: Option<String>,
    ) -> ProposalPreviewWarning {
        ProposalPreviewWarning {
            code: code.to_string(),
            kind,
            message: message.to_string(),
            target_id,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        }
    }

    fn planning_failure(
        item_id: &str,
        target_id: &str,
        diagnostic: ProtocolDiagnostic,
    ) -> ProposalPartialFailureRecord {
        ProposalPartialFailureRecord {
            item_id: item_id.to_string(),
            target_id: target_id.to_string(),
            reason: ProposalFailureReason::ApplyFailed,
            disposition: ProposalPartialFailureDisposition::FailedBeforeMutation,
            diagnostics: vec![diagnostic],
        }
    }

    fn apply_edit_to_buffer_with_correlation(
        &mut self,
        buffer_id: BufferId,
        edit: TextEdit,
        correlation_id: CorrelationId,
    ) -> Result<TextTransactionDescriptor, AppCompositionError> {
        self.active_documents.ensure_active_buffer(buffer_id)?;
        let record = EditorEngine::apply_edit(
            &mut self.editor,
            buffer_id,
            edit,
            TransactionSource::User,
            None,
            Some(correlation_id),
        )?;
        Ok(record.to_protocol_descriptor())
    }
}

impl Default for AppComposition {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the workspace root fallback used by CLI shell.
pub fn default_workspace_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save_proposal(proposal_id: ProposalId) -> WorkspaceProposal {
        let file = FileIdentity {
            file_id: FileId(1),
            workspace_id: WorkspaceId(1),
            canonical_path: CanonicalPath("C:/repo/file.txt".to_string()),
            content_version: FileContentVersion(1),
            content_hash: None,
        };
        let fingerprint = FileFingerprint {
            algorithm: "test".to_string(),
            value: "hash:test".to_string(),
        };
        WorkspaceProposal {
            proposal_id,
            principal: PrincipalId("trusted".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(1),
            payload: ProposalPayload::SaveFile(SaveFileProposal {
                file: file.clone(),
                buffer_id: BufferId(1),
                file_id: file.file_id,
                snapshot_id: devil_protocol::SnapshotId(1),
                buffer_version: devil_protocol::BufferVersion(1),
                file_content_version: FileContentVersion(1),
                workspace_generation: WorkspaceGeneration(1),
                expected_fingerprint: Some(fingerprint.clone()),
                save_intent: SaveIntent::Manual,
                conflict_policy: SaveConflictPolicy::RejectIfChanged,
                trust_decision: TrustDecisionContext {
                    workspace_trust_state: WorkspaceTrustState::Trusted,
                    decision_id: None,
                    decided_at: Some(TimestampMillis(1)),
                },
                required_capability: CapabilityId("fs.write".to_string()),
                principal: PrincipalId("trusted".to_string()),
                correlation_id: CorrelationId(1),
                diagnostics: Vec::new(),
            }),
            preconditions: ProposalVersionPreconditions {
                file_version: Some(FileContentVersion(1)),
                buffer_version: Some(devil_protocol::BufferVersion(1)),
                snapshot_id: Some(devil_protocol::SnapshotId(1)),
                generation: Some(WorkspaceGeneration(1)),
                file_content_version: Some(FileContentVersion(1)),
                workspace_generation: Some(WorkspaceGeneration(1)),
                expected_fingerprint: Some(fingerprint),
                expected_file_length: None,
                expected_modified_at: None,
            },
            preview: PreviewSummary {
                summary: "test save".to_string(),
                details: Vec::new(),
            },
            expires_at: None,
            created_at: TimestampMillis(1),
        }
    }

    fn command(
        proposal_id: ProposalId,
        action: devil_protocol::ProposalLifecycleAction,
    ) -> ProposalLifecycleCommand {
        ProposalLifecycleCommand {
            proposal_id,
            action,
            principal: PrincipalId("trusted".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(1),
            causality_id: CausalityId(uuid::Uuid::now_v7()),
            reason: None,
            diagnostics: Vec::new(),
            requested_at: TimestampMillis(1),
            schema_version: 1,
        }
    }

    fn register_created(coordinator: &AppProposalCoordinator, proposal: &WorkspaceProposal) {
        coordinator
            .register_lifecycle_context(proposal.proposal_id, EventContext::new(CorrelationId(1)));
        assert!(matches!(
            coordinator.created_response(proposal),
            ProposalResponse::Created(_)
        ));
    }

    fn proposal_intent_route_context(
        proposal: Option<WorkspaceProposal>,
    ) -> AppProposalIntentRouteContext {
        AppProposalIntentRouteContext {
            proposal,
            principal: PrincipalId("trusted".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(99),
            causality_id: CausalityId(
                uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            ),
            requested_at: TimestampMillis(123),
        }
    }

    fn assert_transition_diagnostic(response: &ProposalResponse, expected_code: &str) {
        let diagnostics = match response {
            ProposalResponse::Created(transition)
            | ProposalResponse::Validated(transition)
            | ProposalResponse::Approved(transition)
            | ProposalResponse::Applied(transition) => &transition.diagnostics,
            ProposalResponse::Previewed { transition, .. } => &transition.diagnostics,
            ProposalResponse::Rejected { transition, .. }
            | ProposalResponse::Denied { transition, .. }
            | ProposalResponse::Failed { transition, .. }
            | ProposalResponse::RolledBack { transition, .. }
            | ProposalResponse::Stale { transition, .. }
            | ProposalResponse::Conflict { transition, .. }
            | ProposalResponse::Cancelled { transition, .. } => &transition.diagnostics,
        };

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected_code),
            "expected diagnostic {expected_code}, got {diagnostics:?}"
        );
    }

    #[test]
    fn audit_rollback_failure_diagnostics_are_preserved_on_failed_response() {
        let path = CanonicalPath("C:/repo/locked-file.txt".to_string());
        let diagnostic = AppComposition::rollback_failed_diagnostic(
            "proposal.audit_rollback_write_failed",
            &path,
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "locked"),
        );
        let mut response = ProposalResponse::Failed {
            transition: ProposalLifecycleTransition {
                proposal_id: ProposalId(99),
                lifecycle_state: ProposalLifecycleState::Failed,
                timestamp: TimestampMillis(1),
                principal: PrincipalId("trusted".to_string()),
                capability: CapabilityId("fs.write".to_string()),
                correlation_id: CorrelationId(1),
                causality_id: CausalityId(uuid::Uuid::now_v7()),
                diagnostics: vec![AppProposalCoordinator::diagnostic(
                    "proposal.audit_storage_failed",
                    "audit storage failed",
                )],
            },
            reason: ProposalFailureReason::StorageFailed,
        };

        AppComposition::append_response_diagnostics(&mut response, vec![diagnostic]);

        assert_transition_diagnostic(&response, "proposal.audit_storage_failed");
        assert_transition_diagnostic(&response, "proposal.audit_rollback_write_failed");
        let ProposalResponse::Failed { transition, .. } = response else {
            panic!("expected failed response");
        };
        let rollback_diagnostic = transition
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "proposal.audit_rollback_write_failed")
            .expect("rollback diagnostic");
        assert_eq!(rollback_diagnostic.path.as_ref(), Some(&path));
        assert!(rollback_diagnostic.message.contains("locked"));
    }

    fn text_edit_proposal(proposal_id: ProposalId) -> WorkspaceProposal {
        WorkspaceProposal {
            proposal_id,
            principal: PrincipalId("trusted".to_string()),
            capability: CapabilityId("editor.write".to_string()),
            correlation_id: CorrelationId(1),
            payload: ProposalPayload::TextEdit(devil_protocol::TextEditProposal {
                file_id: FileId(1),
                edits: devil_protocol::EditBatch {
                    edits: vec![devil_protocol::TextEdit {
                        range: devil_protocol::TextRange::new(
                            devil_protocol::TextOffset::byte(0),
                            devil_protocol::TextOffset::byte(0),
                        ),
                        replacement: "replacement".to_string(),
                    }],
                },
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
                summary: "test text edit".to_string(),
                details: Vec::new(),
            },
            expires_at: None,
            created_at: TimestampMillis(1),
        }
    }

    fn test_file(file_id: u128, path: &str) -> FileIdentity {
        FileIdentity {
            file_id: FileId(file_id),
            workspace_id: WorkspaceId(1),
            canonical_path: CanonicalPath(path.to_string()),
            content_version: FileContentVersion(1),
            content_hash: None,
        }
    }

    fn complete_file_preconditions() -> ProposalVersionPreconditions {
        ProposalVersionPreconditions {
            file_version: Some(FileContentVersion(1)),
            buffer_version: Some(devil_protocol::BufferVersion(1)),
            snapshot_id: Some(devil_protocol::SnapshotId(1)),
            generation: Some(WorkspaceGeneration(1)),
            file_content_version: Some(FileContentVersion(1)),
            workspace_generation: Some(WorkspaceGeneration(1)),
            expected_fingerprint: Some(FileFingerprint {
                algorithm: "test".to_string(),
                value: "hash:test".to_string(),
            }),
            expected_file_length: None,
            expected_modified_at: None,
        }
    }

    fn proposal_with(
        proposal_id: ProposalId,
        capability: &str,
        payload: ProposalPayload,
    ) -> WorkspaceProposal {
        WorkspaceProposal {
            proposal_id,
            principal: PrincipalId("trusted".to_string()),
            capability: CapabilityId(capability.to_string()),
            correlation_id: CorrelationId(1),
            payload,
            preconditions: complete_file_preconditions(),
            preview: PreviewSummary {
                summary: "test proposal".to_string(),
                details: Vec::new(),
            },
            expires_at: None,
            created_at: TimestampMillis(1),
        }
    }

    fn workspace_edit_payload() -> ProposalPayload {
        let path = CanonicalPath("C:/repo/workspace-created.rs".to_string());
        ProposalPayload::WorkspaceEdit(devil_protocol::WorkspaceEditProposalPayload {
            workspace_id: WorkspaceId(1),
            edit_id: uuid::Uuid::now_v7(),
            title: "workspace create".to_string(),
            source: devil_protocol::WorkspaceEditSourceKind::User,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![AppProposalCoordinator::path_target(
                    "workspace-create".to_string(),
                    ProposalTargetKind::PathOnly,
                    path.clone(),
                    Vec::new(),
                )],
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            file_edits: Vec::new(),
            file_operations: vec![devil_protocol::WorkspaceFileOperation::Create {
                path,
                initial_content_hash: None,
            }],
            required_capability: CapabilityId("fs.write".to_string()),
            diagnostics: Vec::new(),
            schema_version: 1,
        })
    }

    fn terminal_payload() -> ProposalPayload {
        ProposalPayload::TerminalCommand(devil_protocol::TerminalCommandProposal {
            session_id: Some(devil_protocol::TerminalSessionId(7)),
            command: "cargo test".to_string(),
            cwd: Some(CanonicalPath("C:/repo".to_string())),
            env: HashMap::new(),
        })
    }

    #[test]
    fn proposal_coordinator_enforces_preview_after_validation() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let proposal = save_proposal(ProposalId(1));
        register_created(&coordinator, &proposal);

        let preview = coordinator
            .handle(ProposalRequest::Preview(proposal.clone()))
            .expect("preview response");
        let ProposalResponse::Rejected { transition, reason } = preview else {
            panic!("preview before validation should reject");
        };
        assert_eq!(reason, ProposalRejectionReason::ValidationFailed);
        assert!(
            transition
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "proposal.invalid_lifecycle_transition")
        );

        let validation = coordinator
            .handle(ProposalRequest::Validate(proposal.clone()))
            .expect("validate response");
        assert!(matches!(validation, ProposalResponse::Validated(_)));
        let preview = coordinator
            .handle(ProposalRequest::Preview(proposal))
            .expect("preview response");
        assert!(matches!(preview, ProposalResponse::Previewed { .. }));
    }

    #[test]
    fn command_dispatcher_maps_projection_only_proposal_intents_to_protocol_requests() {
        let proposal = save_proposal(ProposalId(42));
        let preview = CommandDispatcher::route_proposal_intent(
            CommandDispatchIntent::PreviewProposal {
                proposal_id: ProposalId(42),
            },
            proposal_intent_route_context(Some(proposal.clone())),
        )
        .expect("preview intent maps")
        .expect("preview request");
        assert!(
            matches!(preview, ProposalRequest::Preview(mapped) if mapped.proposal_id == ProposalId(42))
        );

        let approve = CommandDispatcher::route_proposal_intent(
            CommandDispatchIntent::ApproveProposal {
                proposal_id: ProposalId(42),
            },
            proposal_intent_route_context(None),
        )
        .expect("approve intent maps")
        .expect("approve request");
        let ProposalRequest::Approve(command) = approve else {
            panic!("expected approve request");
        };
        assert_eq!(command.proposal_id, ProposalId(42));
        assert_eq!(command.action, ProposalLifecycleAction::Approve);
        assert_eq!(command.principal, PrincipalId("trusted".to_string()));

        let reject = CommandDispatcher::route_proposal_intent(
            CommandDispatchIntent::RejectProposal {
                proposal_id: ProposalId(42),
                reason: ProposalRejectionReason::UserRejected,
            },
            proposal_intent_route_context(None),
        )
        .expect("reject intent maps")
        .expect("reject request");
        let ProposalRequest::Reject(command) = reject else {
            panic!("expected reject request");
        };
        assert!(matches!(
            command.reason,
            Some(ProposalLifecycleCommandReason::Rejection(
                ProposalRejectionReason::UserRejected
            ))
        ));

        let details = CommandDispatcher::route_proposal_intent(
            CommandDispatchIntent::OpenProposalDetails {
                proposal_id: ProposalId(42),
            },
            proposal_intent_route_context(Some(proposal)),
        )
        .expect("details intent maps");
        assert!(details.is_none());
    }

    #[test]
    fn command_dispatcher_rejects_apply_intent_without_app_owned_matching_proposal() {
        let missing = CommandDispatcher::route_proposal_intent(
            CommandDispatchIntent::ApplyProposal {
                proposal_id: ProposalId(42),
            },
            proposal_intent_route_context(None),
        );
        assert!(matches!(
            missing,
            Err(AppCompositionError::ProposalIntentMissingProposal)
        ));

        let mismatch = CommandDispatcher::route_proposal_intent(
            CommandDispatchIntent::ApplyProposal {
                proposal_id: ProposalId(42),
            },
            proposal_intent_route_context(Some(save_proposal(ProposalId(7)))),
        );
        assert!(matches!(
            mismatch,
            Err(AppCompositionError::ProposalIntentMismatch {
                target: ProposalId(42),
                active: Some(ProposalId(7))
            })
        ));
    }

    #[test]
    fn proposal_coordinator_allows_created_validated_previewed_approved_applied_path() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let proposal = save_proposal(ProposalId(10));
        register_created(&coordinator, &proposal);

        assert!(matches!(
            coordinator.handle(ProposalRequest::Validate(proposal.clone())),
            Ok(ProposalResponse::Validated(_))
        ));
        assert!(matches!(
            coordinator.handle(ProposalRequest::Preview(proposal.clone())),
            Ok(ProposalResponse::Previewed { .. })
        ));
        assert!(matches!(
            coordinator.handle(ProposalRequest::Approve(command(
                proposal.proposal_id,
                devil_protocol::ProposalLifecycleAction::Approve,
            ))),
            Ok(ProposalResponse::Approved(_))
        ));

        let transition = coordinator
            .record_transition(&proposal, ProposalLifecycleState::Applied, "apply")
            .expect("approved proposal can apply");
        assert_eq!(transition.lifecycle_state, ProposalLifecycleState::Applied);
        assert_eq!(
            coordinator.current_lifecycle_state(proposal.proposal_id),
            Some(ProposalLifecycleState::Applied)
        );
    }

    #[test]
    fn proposal_coordinator_allows_validated_denied_path() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let proposal = save_proposal(ProposalId(11));
        register_created(&coordinator, &proposal);

        assert!(matches!(
            coordinator.handle(ProposalRequest::Validate(proposal.clone())),
            Ok(ProposalResponse::Validated(_))
        ));
        let transition = coordinator
            .record_transition_with_diagnostics(
                &proposal,
                ProposalLifecycleState::Denied,
                "validate",
                vec![AppProposalCoordinator::diagnostic(
                    "proposal.validation_denied",
                    "test validation denial",
                )],
            )
            .expect("validated proposal can deny");
        assert_eq!(transition.lifecycle_state, ProposalLifecycleState::Denied);
        assert!(
            transition
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "proposal.validation_denied")
        );
    }

    #[test]
    fn proposal_coordinator_allows_approved_stale_conflict_and_failed_paths() {
        for (proposal_id, terminal_state) in [
            (ProposalId(12), ProposalLifecycleState::Stale),
            (ProposalId(13), ProposalLifecycleState::Conflict),
            (ProposalId(14), ProposalLifecycleState::Failed),
        ] {
            let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
            let proposal = save_proposal(proposal_id);
            register_created(&coordinator, &proposal);
            assert!(matches!(
                coordinator.handle(ProposalRequest::Validate(proposal.clone())),
                Ok(ProposalResponse::Validated(_))
            ));
            assert!(matches!(
                coordinator.handle(ProposalRequest::Preview(proposal.clone())),
                Ok(ProposalResponse::Previewed { .. })
            ));
            assert!(matches!(
                coordinator.handle(ProposalRequest::Approve(command(
                    proposal.proposal_id,
                    devil_protocol::ProposalLifecycleAction::Approve,
                ))),
                Ok(ProposalResponse::Approved(_))
            ));

            let transition = coordinator
                .record_transition_with_diagnostics(
                    &proposal,
                    terminal_state,
                    "apply",
                    vec![AppProposalCoordinator::diagnostic(
                        "proposal.apply_terminal",
                        format!("test {terminal_state:?} terminal transition"),
                    )],
                )
                .expect("approved proposal can enter terminal apply state");
            assert_eq!(transition.lifecycle_state, terminal_state);
            assert!(
                transition
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "proposal.apply_terminal")
            );
        }
    }

    #[test]
    fn proposal_coordinator_rejects_created_to_applied_without_state_mutation() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let proposal = save_proposal(ProposalId(15));
        register_created(&coordinator, &proposal);

        let response = coordinator
            .record_transition(&proposal, ProposalLifecycleState::Applied, "apply")
            .expect_err("created proposal cannot apply directly");
        assert_transition_diagnostic(&response, "proposal.invalid_lifecycle_transition");
        assert_eq!(
            coordinator.current_lifecycle_state(proposal.proposal_id),
            Some(ProposalLifecycleState::Created)
        );
    }

    #[test]
    fn proposal_coordinator_rejects_expired_lifecycle_before_validation() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let mut proposal = save_proposal(ProposalId(16));
        proposal.expires_at = Some(TimestampMillis(1));
        register_created(&coordinator, &proposal);

        let response = coordinator
            .handle(ProposalRequest::Validate(proposal.clone()))
            .expect("expired validate response");
        let ProposalResponse::Rejected { reason, .. } = &response else {
            panic!("expired proposal should reject, got {response:?}");
        };
        assert_eq!(*reason, ProposalRejectionReason::Expired);
        assert_transition_diagnostic(&response, "proposal.expired");
        assert_eq!(
            coordinator.current_lifecycle_state(proposal.proposal_id),
            Some(ProposalLifecycleState::Rejected)
        );
    }

    #[test]
    fn proposal_coordinator_rejects_zero_correlation_or_nil_causality_context() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let mut proposal = save_proposal(ProposalId(17));
        proposal.correlation_id = CorrelationId(0);
        coordinator.register_lifecycle_context(
            proposal.proposal_id,
            EventContext {
                correlation_id: CorrelationId(0),
                causality_id: CausalityId(uuid::Uuid::nil()),
            },
        );

        let response = coordinator.created_response(&proposal);
        assert_transition_diagnostic(&response, "proposal.invalid_lifecycle_context");
        assert_transition_diagnostic(&response, "proposal.zero_correlation_id");
        assert_transition_diagnostic(&response, "proposal.lifecycle_context_nil_causality_id");
        assert_eq!(
            coordinator.current_lifecycle_state(proposal.proposal_id),
            None
        );

        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let proposal = save_proposal(ProposalId(18));
        register_created(&coordinator, &proposal);
        assert!(matches!(
            coordinator.handle(ProposalRequest::Validate(proposal.clone())),
            Ok(ProposalResponse::Validated(_))
        ));
        assert!(matches!(
            coordinator.handle(ProposalRequest::Preview(proposal.clone())),
            Ok(ProposalResponse::Previewed { .. })
        ));
        let mut approve = command(
            proposal.proposal_id,
            devil_protocol::ProposalLifecycleAction::Approve,
        );
        approve.correlation_id = CorrelationId(0);
        approve.causality_id = CausalityId(uuid::Uuid::nil());

        let response = coordinator
            .handle(ProposalRequest::Approve(approve))
            .expect("invalid command context response");
        assert_transition_diagnostic(&response, "proposal.command_zero_correlation_id");
        assert_transition_diagnostic(&response, "proposal.command_nil_causality_id");
        assert_eq!(
            coordinator.current_lifecycle_state(proposal.proposal_id),
            Some(ProposalLifecycleState::Previewed)
        );
    }

    #[test]
    fn proposal_coordinator_rejects_command_without_lifecycle_context() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let response = coordinator
            .handle(ProposalRequest::Approve(command(
                ProposalId(99),
                devil_protocol::ProposalLifecycleAction::Approve,
            )))
            .expect("approve response");

        let ProposalResponse::Rejected { transition, reason } = response else {
            panic!("unknown lifecycle command should reject");
        };
        assert_eq!(reason, ProposalRejectionReason::ValidationFailed);
        assert!(
            transition
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "proposal.missing_lifecycle_context")
        );
    }

    #[test]
    fn proposal_coordinator_denies_registered_text_edit_missing_preconditions() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let proposal = text_edit_proposal(ProposalId(3));
        coordinator
            .register_lifecycle_context(proposal.proposal_id, EventContext::new(CorrelationId(1)));
        assert!(matches!(
            coordinator.created_response(&proposal),
            ProposalResponse::Created(_)
        ));

        let response = coordinator
            .handle(ProposalRequest::Validate(proposal))
            .expect("validate response");
        let ProposalResponse::Denied { transition, reason } = response else {
            panic!("registered text edit with missing preconditions should deny");
        };
        assert_eq!(reason, ProposalDenialReason::PolicyDenied);
        assert!(transition.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "proposal.missing_buffer_precondition"
                || diagnostic.code == "proposal.missing_file_precondition"
        }));
    }

    #[test]
    fn proposal_coordinator_documents_generic_save_apply_denial() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let proposal = save_proposal(ProposalId(2));
        coordinator
            .register_lifecycle_context(proposal.proposal_id, EventContext::new(CorrelationId(1)));
        assert!(matches!(
            coordinator.created_response(&proposal),
            ProposalResponse::Created(_)
        ));
        assert!(matches!(
            coordinator.handle(ProposalRequest::Validate(proposal.clone())),
            Ok(ProposalResponse::Validated(_))
        ));
        assert!(matches!(
            coordinator.handle(ProposalRequest::Preview(proposal.clone())),
            Ok(ProposalResponse::Previewed { .. })
        ));

        let response = coordinator
            .handle(ProposalRequest::Apply(proposal))
            .expect("apply response");
        let ProposalResponse::Rejected { transition, reason } = response else {
            panic!("generic save apply should remain denied");
        };
        assert_eq!(reason, ProposalRejectionReason::Unsupported);
        assert!(transition.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("use AppComposition::save_active_buffer")
        }));
    }

    #[test]
    fn proposal_coordinator_discovers_targets_for_every_payload_variant() {
        let file = test_file(10, "C:/repo/file.rs");
        let rename_destination = CanonicalPath("C:/repo/renamed.rs".to_string());
        let cases = vec![
            (
                ProposalPayload::TextEdit(devil_protocol::TextEditProposal {
                    file_id: FileId(10),
                    edits: devil_protocol::EditBatch {
                        edits: vec![devil_protocol::TextEdit {
                            range: devil_protocol::TextRange::new(
                                devil_protocol::TextOffset::byte(0),
                                devil_protocol::TextOffset::byte(4),
                            ),
                            replacement: "edit".to_string(),
                        }],
                    },
                }),
                vec![ProposalTargetKind::OpenBuffer],
            ),
            (
                ProposalPayload::CreateFile(devil_protocol::CreateFileProposal {
                    path: CanonicalPath("C:/repo/new.rs".to_string()),
                    initial_content: None,
                }),
                vec![ProposalTargetKind::PathOnly],
            ),
            (
                ProposalPayload::DeleteFile(devil_protocol::DeleteFileProposal {
                    file: file.clone(),
                }),
                vec![ProposalTargetKind::ClosedFile],
            ),
            (
                ProposalPayload::RenameFile(devil_protocol::RenameFileProposal {
                    file: file.clone(),
                    destination: rename_destination,
                }),
                vec![ProposalTargetKind::ClosedFile, ProposalTargetKind::PathOnly],
            ),
            (
                save_proposal(ProposalId(40)).payload,
                vec![ProposalTargetKind::OpenBuffer],
            ),
            (
                ProposalPayload::FormatFile(devil_protocol::FormatFileProposal {
                    file: file.clone(),
                    snapshot_id: devil_protocol::SnapshotId(1),
                    options: HashMap::new(),
                }),
                vec![ProposalTargetKind::ClosedFile],
            ),
            (
                ProposalPayload::CodeAction(devil_protocol::CodeActionProposal {
                    file: file.clone(),
                    title: "fix".to_string(),
                    edits: vec![devil_protocol::TextEdit {
                        range: devil_protocol::TextRange::new(
                            devil_protocol::TextOffset::byte(1),
                            devil_protocol::TextOffset::byte(2),
                        ),
                        replacement: "x".to_string(),
                    }],
                }),
                vec![ProposalTargetKind::ClosedFile],
            ),
            (workspace_edit_payload(), vec![ProposalTargetKind::PathOnly]),
            (
                terminal_payload(),
                vec![ProposalTargetKind::TerminalSession],
            ),
            (
                ProposalPayload::Batch(BatchProposalPayload {
                    batch_id: uuid::Uuid::now_v7(),
                    atomicity: ProposalBatchAtomicity::OrderedNonAtomic,
                    rollback_policy: ProposalBatchRollbackPolicy::NotSupported,
                    target_coverage: ProposalTargetCoverage {
                        coverage_kind: ProposalTargetCoverageKind::Complete,
                        targets: Vec::new(),
                        omitted_target_count: 0,
                        redaction_hints: Vec::new(),
                    },
                    items: vec![ProposalBatchItem {
                        order: 0,
                        item_id: "create".to_string(),
                        payload: Box::new(ProposalPayload::CreateFile(
                            devil_protocol::CreateFileProposal {
                                path: CanonicalPath("C:/repo/batch.rs".to_string()),
                                initial_content: None,
                            },
                        )),
                        target_ids: Vec::new(),
                        required_capability: CapabilityId("fs.write".to_string()),
                        rollback_step_ids: Vec::new(),
                    }],
                    dependency_edges: Vec::new(),
                    rollback_steps: Vec::new(),
                    partial_failures: Vec::new(),
                    preview_warnings: Vec::new(),
                    schema_version: 1,
                }),
                vec![ProposalTargetKind::PathOnly],
            ),
        ];

        for (payload, expected_kinds) in cases {
            let coverage = AppProposalCoordinator::affected_target_coverage_for_payload(&payload);
            let actual_kinds = coverage
                .targets
                .iter()
                .map(|target| target.kind)
                .collect::<Vec<_>>();
            assert_eq!(coverage.coverage_kind, ProposalTargetCoverageKind::Complete);
            assert_eq!(coverage.omitted_target_count, 0);
            assert_eq!(actual_kinds, expected_kinds, "payload {payload:?}");
        }
    }

    #[test]
    fn proposal_coordinator_denies_duplicate_ambiguous_and_unsupported_targets() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let file = test_file(20, "C:/repo/dup.rs");
        let mut proposal = proposal_with(
            ProposalId(41),
            "fs.write",
            ProposalPayload::WorkspaceEdit(devil_protocol::WorkspaceEditProposalPayload {
                workspace_id: WorkspaceId(1),
                edit_id: uuid::Uuid::now_v7(),
                title: "duplicate targets".to_string(),
                source: devil_protocol::WorkspaceEditSourceKind::User,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![
                        AppProposalCoordinator::file_identity_target(
                            "dup".to_string(),
                            ProposalTargetKind::ClosedFile,
                            &file,
                            None,
                            Vec::new(),
                        ),
                        AppProposalCoordinator::file_identity_target(
                            "dup".to_string(),
                            ProposalTargetKind::ClosedFile,
                            &file,
                            None,
                            Vec::new(),
                        ),
                    ],
                    omitted_target_count: 0,
                    redaction_hints: Vec::new(),
                },
                file_edits: vec![devil_protocol::WorkspaceTextEdit {
                    file,
                    buffer_id: None,
                    edits: devil_protocol::EditBatch { edits: Vec::new() },
                    preconditions: complete_file_preconditions(),
                }],
                file_operations: Vec::new(),
                required_capability: CapabilityId("fs.write".to_string()),
                diagnostics: Vec::new(),
                schema_version: 1,
            }),
        );
        register_created(&coordinator, &proposal);

        let response = coordinator
            .handle(ProposalRequest::Validate(proposal.clone()))
            .expect("validate duplicate targets");
        let ProposalResponse::Denied { transition, reason } = response else {
            panic!("duplicate targets should deny, got {response:?}");
        };
        assert_eq!(reason, ProposalDenialReason::PolicyDenied);
        assert!(
            transition
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "proposal.duplicate_target")
        );

        proposal.proposal_id = ProposalId(42);
        let ProposalPayload::WorkspaceEdit(payload) = &mut proposal.payload else {
            panic!("expected workspace-edit payload");
        };
        payload.target_coverage.targets = vec![ProposalAffectedTarget {
            target_id: "ambiguous".to_string(),
            kind: ProposalTargetKind::Plugin,
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(20)),
            buffer_id: None,
            path: None,
            terminal_session_id: None,
            plugin_id: Some(devil_protocol::PluginId(7)),
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: Vec::new(),
            redaction_hints: Vec::new(),
        }];
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        register_created(&coordinator, &proposal);
        let response = coordinator
            .handle(ProposalRequest::Validate(proposal))
            .expect("validate ambiguous target");
        assert_transition_diagnostic(&response, "proposal.ambiguous_target");
        assert_transition_diagnostic(&response, "proposal.unsupported_target_kind");
    }

    #[test]
    fn proposal_coordinator_denies_nested_batch_duplicates_and_unsupported_items() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let create_path = CanonicalPath("C:/repo/batch-create.rs".to_string());
        let duplicate_target = AppProposalCoordinator::path_target(
            "target-create".to_string(),
            ProposalTargetKind::PathOnly,
            create_path.clone(),
            Vec::new(),
        );
        let proposal = proposal_with(
            ProposalId(43),
            "fs.write",
            ProposalPayload::Batch(BatchProposalPayload {
                batch_id: uuid::Uuid::now_v7(),
                atomicity: ProposalBatchAtomicity::OrderedNonAtomic,
                rollback_policy: ProposalBatchRollbackPolicy::NotSupported,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![duplicate_target.clone(), duplicate_target],
                    omitted_target_count: 0,
                    redaction_hints: Vec::new(),
                },
                items: vec![
                    ProposalBatchItem {
                        order: 0,
                        item_id: "create".to_string(),
                        payload: Box::new(ProposalPayload::CreateFile(
                            devil_protocol::CreateFileProposal {
                                path: create_path,
                                initial_content: None,
                            },
                        )),
                        target_ids: vec!["target-create".to_string(), "target-create".to_string()],
                        required_capability: CapabilityId("fs.write".to_string()),
                        rollback_step_ids: Vec::new(),
                    },
                    ProposalBatchItem {
                        order: 1,
                        item_id: "terminal".to_string(),
                        payload: Box::new(terminal_payload()),
                        target_ids: vec!["target-missing".to_string()],
                        required_capability: CapabilityId("terminal.execute".to_string()),
                        rollback_step_ids: Vec::new(),
                    },
                ],
                dependency_edges: Vec::new(),
                rollback_steps: Vec::new(),
                partial_failures: Vec::new(),
                preview_warnings: Vec::new(),
                schema_version: 1,
            }),
        );
        register_created(&coordinator, &proposal);

        let response = coordinator
            .handle(ProposalRequest::Validate(proposal))
            .expect("validate batch");
        let ProposalResponse::Denied { transition, reason } = response else {
            panic!("invalid batch should deny, got {response:?}");
        };
        assert_eq!(reason, ProposalDenialReason::PolicyDenied);
        for expected in [
            "proposal.duplicate_target",
            "proposal.duplicate_batch_item_target",
            "proposal.unknown_batch_target",
            "proposal.unsupported_batch_item_route",
        ] {
            assert!(
                transition
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == expected),
                "missing {expected}: {:?}",
                transition.diagnostics
            );
        }
    }
}
