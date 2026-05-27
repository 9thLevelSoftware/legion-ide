//! Application composition root for workspace/editor/ui orchestration.

#![warn(missing_docs)]

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use devil_agent::AgentRuntime;
use devil_ai::ProviderRouter;
use devil_ai_providers::{DETERMINISTIC_LOCAL_PROVIDER_ID, make_stub_registry};
use devil_collaboration::{CollaborationRuntimeConfig, CollaborationSessionRuntime};
use devil_editor::{
    BufferMode, Cursor, EditorEngine, EditorError, SaveAcknowledgement, SaveRequestDto, Selection,
    TextEdit, TextPosition, TextRange as EditorTextRange,
};
use devil_index::{
    DEFAULT_GRAMMAR_VERSION, DEFAULT_MODEL_VERSION, LexicalIndexer, SemanticIndex, SourceDocument,
};
use devil_memory::{MemoryCandidateRecord, MemoryConsentState, MemoryService};
use devil_observability::{
    SharedEventSink, agent_replay_manifest_recorded_event, collaboration_audit_recorded_event,
    event_metadata_record, phase4_runtime_audit_recorded_event, plugin_event_envelope,
    proposal_applied_event, proposal_approved_event, proposal_audit_record,
    proposal_audit_recorded_event, proposal_created_event, proposal_failed_event,
    proposal_previewed_event, proposal_rejected_event, proposal_rolled_back_event,
    proposal_validated_event, remote_audit_recorded_event, save_denied_event,
    stale_proposal_rejected_event, terminal_audit_recorded_event, transaction_event,
};
use devil_platform::{NativeFileSystem, NativeWatcherService};
use devil_plugin::PluginRuntimeHost;
use devil_project::{
    OpenedFileText, WorkspaceActor, WorkspaceCreateFileRequest, WorkspaceDeleteFileRequest,
    WorkspaceError, WorkspaceMutationRollbackCheckpoint,
    WorkspaceMutationRollbackCheckpointRequest, WorkspaceMutationRollbackRequest,
    WorkspaceMutationRollbackTarget, WorkspaceRenameFileRequest, WorkspaceSaveRequest,
};
use devil_protocol::{
    BatchProposalPayload, BufferId, ByteRange, CancellationTokenId, CanonicalPath,
    CapabilityBrokerPort, CapabilityDecision, CapabilityDecisionId, CapabilityId,
    CapabilityNamespace, CapabilityRequest, CapabilityRequestContext, CapabilityResponse,
    CausalityId, CollaborationAuditRecord, CollaborationDocumentBinding,
    CollaborationDocumentEpoch, CollaborationDocumentOperation, CollaborationDocumentOperationKind,
    CollaborationParticipant, CollaborationParticipantId, CollaborationParticipantRole,
    CollaborationPermission, CollaborationPresenceProjection, CollaborationSessionDescriptor,
    CollaborationSessionId, CollaborationSessionState, CollaborationSharedProposalApproval,
    CollaborationSharedProposalDisposition, CollaborationTransportEnvelope,
    CollaborationTransportPayload, CorrelationId, EditBatch, EditorApplyTransactionRequest,
    EventEnvelope, EventSequence, EventSinkPort, EventSinkRequest, FileConflictContext,
    FileConflictLifecycleState, FileConflictReason, FileConflictState, FileContentVersion,
    FileFingerprint, FileId, FileIdentity, FileKind, FileTreeNode, LanguageCompletionProjection,
    LanguageHoverProjection, LanguageId, LanguageLocationProjection,
    LanguageOutlineSymbolProjection, LanguageProblemProjection, LanguageToolingOperationKind,
    LanguageToolingOperationProjection, LanguageToolingProjection, LanguageToolingStatusKind,
    LspEditProposalConversionInput, LspRequestCorrelation, PluginContributionProjection,
    PluginHostCallKind, PluginHostCallRequest, PluginHostCallResponse, PluginId, PluginManifest,
    PreviewSummary, PrincipalId, ProposalAffectedTarget, ProposalBatchAtomicity, ProposalBatchItem,
    ProposalBatchRollbackPolicy, ProposalCancellationReason, ProposalDenialReason,
    ProposalFailureReason, ProposalId, ProposalLifecycleAction, ProposalLifecycleCommand,
    ProposalLifecycleCommandReason, ProposalLifecycleState, ProposalLifecycleTransition,
    ProposalPartialFailureDisposition, ProposalPartialFailureRecord, ProposalPayload, ProposalPort,
    ProposalPreviewWarning, ProposalPreviewWarningKind, ProposalRejectionReason, ProposalRequest,
    ProposalResponse, ProposalRollbackReason, ProposalStaleReason, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProposalTargetKind, ProposalVersionPreconditions,
    ProtocolDiagnostic, ProtocolDiagnosticSeverity, ProtocolError, ProtocolResult,
    ProtocolTextRange, RedactionHint, RemoteAgentDescriptor, RemoteAuditRecord,
    RemoteAuthorityDescriptor, RemoteTransportEnvelope, RemoteTransportPayload,
    RemoteWorkspaceLifecycleState, RemoteWorkspaceSessionDescriptor, RemoteWorkspaceSessionId,
    SaveConflictPolicy, SaveFileProposal, SaveIntent, SemanticGrammarVersion, SemanticModelVersion,
    SemanticPrivacyScope, SemanticQueryFreshnessPolicy, SemanticQueryId, SemanticQueryKind,
    SemanticQueryRequest, SemanticQueryScope, SessionDirtyIndicator, SessionPanelState, SessionTab,
    SessionTabGroup, StorageRepositoryPort, StorageRepositoryRequest, StorageRepositoryResponse,
    TerminalCloseRequest, TerminalInput, TerminalKillEscalation, TerminalKillRequest,
    TerminalOutputRowProjection, TerminalPanelProjection, TerminalPanelStatus,
    TerminalPanelStatusKind, TerminalPolicyProjection, TerminalResize, TerminalRuntimeState,
    TerminalScrollbackProjection, TerminalSearchProjection, TerminalSessionId, TextCoordinate,
    TextEdit as ProtocolWorkspaceTextEdit, TextRange as ProtocolEditTextRange,
    TextTransactionDescriptor, TimestampMillis, TransactionSource, TrustDecisionContext,
    VersionContext, ViewportScroll, WorkspaceCloseRequest, WorkspaceEditProposalPayload,
    WorkspaceEditSourceKind, WorkspaceGeneration, WorkspaceId, WorkspaceOpenRequest,
    WorkspaceOpened, WorkspacePort, WorkspaceProposal, WorkspaceRequest, WorkspaceResponse,
    WorkspaceSessionRecord, WorkspaceTextEdit, WorkspaceTrustState,
    validate_terminal_close_request, validate_terminal_input, validate_terminal_kill_request,
    validate_terminal_resize,
};
use devil_remote::{RemoteDevelopmentRuntime, RemoteOperationOutcome, RemoteRuntimeConfig};
use devil_security::{DenyByDefaultBroker, SecurityPolicy};
use devil_storage::InMemoryStorageRepositoryPort;
use devil_terminal::{TerminalFixtureConfig, TerminalFixtureRuntime};
use devil_tracker::{TrackerLedger, TrackerRunLedgerRecord};
use devil_ui::ui::{
    CloseDirtyPromptProjection, DailyEditingProjection, EditorTabProjection, EditorTabsProjection,
    EditorViewportStateProjection, SearchProjection, SearchResultProjection, SearchScopeProjection,
    SearchStatusKindProjection, SearchStatusProjection, WorkspaceSessionRecordProjection,
};
use devil_ui::{
    ActiveBufferProjection, CommandDispatchIntent, ExplorerNodeProjection, ExplorerProjection,
    ExplorerSelectionProjection, ShellLayoutProjection, ShellProjectionSnapshot,
};
use thiserror::Error;

const SEARCH_DEFAULT_RESULT_LIMIT: usize = 50;
const SEARCH_MAX_RESULT_LIMIT: usize = 100;
const SEARCH_SNIPPET_LIMIT_BYTES: usize = 160;
const WORKSPACE_SEARCH_MAX_FILE_BYTES: u64 = 256 * 1024;

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
    /// UI command targeted a buffer that is not open in the app tab set.
    #[error("buffer {0:?} is not open")]
    BufferNotOpen(BufferId),
    /// Dirty-close cancellation targeted a buffer with no active dirty-close prompt.
    #[error("dirty-close prompt is not active for buffer {0:?}")]
    DirtyClosePromptMissing(BufferId),
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
    /// Phase 4 AI runtime refused or failed a metadata-only step.
    #[error("phase 4 AI runtime failed: {0}")]
    AiRuntime(String),
    /// Requested Phase 4 AI run does not exist.
    #[error("phase 4 AI run not found: {run_id}")]
    AiRunMissing {
        /// Missing run identifier.
        run_id: String,
    },
    /// Collaboration runtime or app gate rejected a request.
    #[error("collaboration request failed: {0}")]
    Collaboration(String),
    /// Remote runtime or app gate rejected a request.
    #[error("remote request failed: {0}")]
    Remote(String),
    /// Language tooling workflow failed.
    #[error("language tooling request failed: {0}")]
    LanguageTooling(String),
    /// Terminal workflow failed.
    #[error("terminal request failed: {0}")]
    Terminal(String),
}

#[cfg(test)]
mod daily_editing_save_all_internal_tests {
    use super::*;

    #[test]
    fn daily_editing_save_all_missing_metadata_records_buffer_error() {
        let mut app = AppComposition::new();
        let orphaned_buffer = BufferId(9001);
        app.active_documents.open_tabs.push(orphaned_buffer);

        let outcome = app.save_all().expect("save-all records metadata error");
        assert_eq!(outcome.status, AppSaveAllStatus::Rejected);
        assert_eq!(outcome.saved_count, 0);
        assert_eq!(outcome.rejected_count, 1);
        assert_eq!(outcome.results.len(), 1);

        let item = &outcome.results[0];
        assert_eq!(item.buffer_id, orphaned_buffer);
        assert_eq!(item.status, AppSaveAllItemStatus::MetadataMissing);
        assert!(item.outcome.is_none());
        assert!(item.final_dirty);
        let metadata = item
            .rejection_metadata
            .as_ref()
            .expect("metadata failure summary");
        assert_eq!(metadata.response_kind, "MetadataMissing");
        assert_eq!(metadata.diagnostic_codes, vec!["save_all.metadata_missing"]);
    }
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

#[derive(Debug, Clone)]
struct CloseDirtyPromptState {
    buffer_id: BufferId,
    file_id: Option<FileId>,
    path: Option<CanonicalPath>,
    title: String,
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

/// Side-effect-free execution journal built from the batch execution contract.
///
/// Plan Phase 2: this is the bridge between preflight-only contracts and future runtime batch
/// mutation. It records deterministic per-item state and blocked execution stages without invoking
/// editor or workspace mutation helpers.
#[derive(Debug, Clone)]
pub struct BatchExecutionJournal {
    /// Proposal id that was journaled.
    pub proposal_id: ProposalId,
    /// Batch id when the proposal payload is a batch.
    pub batch_id: Option<uuid::Uuid>,
    /// True only after a future stage enables runtime mutation and all preconditions pass.
    pub mutation_allowed: bool,
    /// True when current code still blocks runtime mutation.
    pub runtime_apply_disabled: bool,
    /// True because success must remain impossible without audit proof.
    pub audit_before_success_required: bool,
    /// Ordered stage states derived from the execution contract.
    pub stages: Vec<BatchExecutionJournalStage>,
    /// Deterministic item states sorted by batch order, then item id.
    pub items: Vec<BatchExecutionJournalItem>,
    /// Planning and dependency partial failures recorded before mutation.
    pub partial_failures: Vec<ProposalPartialFailureRecord>,
    /// Journal-level diagnostics.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Journal-level preview warnings.
    pub preview_warnings: Vec<ProposalPreviewWarning>,
}

/// One batch execution stage recorded in the non-mutating journal.
#[derive(Debug, Clone)]
pub struct BatchExecutionJournalStage {
    /// Stage represented by this journal row.
    pub stage: BatchExecutionStage,
    /// Current state of the stage.
    pub state: BatchExecutionJournalStageState,
    /// Metadata-only diagnostics explaining blockers.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// State of a batch execution stage in the non-mutating journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchExecutionJournalStageState {
    /// Stage is structurally available for planning.
    Ready,
    /// Stage is required but blocked by current safety gates.
    Blocked,
}

/// One batch item recorded in the non-mutating execution journal.
#[derive(Debug, Clone)]
pub struct BatchExecutionJournalItem {
    /// Stable item id from the batch payload.
    pub item_id: String,
    /// Application order from the batch item.
    pub order: u32,
    /// App-level route selected for the item payload.
    pub route: BatchPreflightRoute,
    /// Target ids referenced by this item.
    pub target_ids: Vec<String>,
    /// Current item state.
    pub state: BatchExecutionJournalItemState,
    /// The item's planning/blocked disposition when present.
    pub partial_failure_disposition: Option<ProposalPartialFailureDisposition>,
    /// Diagnostics scoped to this journal item.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// State of one batch item in the non-mutating execution journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchExecutionJournalItemState {
    /// Item is prepared and could run in a future runtime-enabled stage.
    Prepared,
    /// Item failed preflight before any mutation.
    PreflightRejected,
    /// Item was not started because a prerequisite item failed.
    DependencyBlocked,
    /// Item preflight passed, but runtime mutation remains deliberately disabled.
    RuntimeMutationDisabled,
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
    SharedCollaboration,
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
                let mut has_collaboration = false;
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
                        ProposalTargetKind::CollaborationSession => has_collaboration = true,
                        ProposalTargetKind::RemoteWorkspace | ProposalTargetKind::Plugin => {
                            has_other = true;
                        }
                    }
                }

                let route_count = [
                    has_editor,
                    has_workspace,
                    has_terminal,
                    has_collaboration,
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
                    has_collaboration,
                    has_metadata,
                    has_other,
                ) {
                    (1, true, false, false, false, false, false) => Self::EditorBuffer,
                    (1, false, true, false, false, false, false) => Self::WorkspaceFile,
                    (1, false, false, true, false, false, false) => Self::Terminal,
                    (1, false, false, false, false, true, false) => Self::MetadataOnly,
                    (1, false, false, false, true, false, false) => Self::Unsupported,
                    (_, true, false, false, true, false, false)
                    | (_, false, true, false, true, false, false)
                    | (_, true, true, false, true, false, false) => Self::SharedCollaboration,
                    (_, _, _, _, _, _, true) => Self::Unsupported,
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
    selected_proposal_id: RefCell<Option<ProposalId>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ProposalLifecycleRecoveryRecord {
    proposal: WorkspaceProposal,
    state: ProposalLifecycleState,
    context: Option<EventContext>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct ProposalLifecycleRecoverySnapshot {
    records: Vec<ProposalLifecycleRecoveryRecord>,
    next_proposal_id: u64,
    next_event_sequence: u64,
    generated_at: TimestampMillis,
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
            selected_proposal_id: RefCell::new(None),
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

    fn proposal_for_id(&self, proposal_id: ProposalId) -> Option<WorkspaceProposal> {
        self.proposal(proposal_id)
    }

    fn select_proposal_for_details(&self, proposal_id: ProposalId) -> bool {
        if self.proposals.borrow().contains_key(&proposal_id) {
            self.selected_proposal_id.replace(Some(proposal_id));
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    fn proposal_lifecycle_recovery_snapshot(&self) -> ProposalLifecycleRecoverySnapshot {
        let proposals = self.proposals.borrow();
        let states = self.proposal_states.borrow();
        let contexts = self.proposal_contexts.borrow();
        let mut records = proposals
            .values()
            .map(|proposal| ProposalLifecycleRecoveryRecord {
                proposal: proposal.clone(),
                state: states
                    .get(&proposal.proposal_id)
                    .copied()
                    .unwrap_or(ProposalLifecycleState::Created),
                context: contexts.get(&proposal.proposal_id).copied(),
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|record| (record.proposal.proposal_id.0, record.proposal.created_at.0));

        ProposalLifecycleRecoverySnapshot {
            records,
            next_proposal_id: self.next_proposal_id.get(),
            next_event_sequence: self.next_event_sequence.get(),
            generated_at: TimestampMillis::now(),
        }
    }

    #[allow(dead_code)]
    fn recover_lifecycle_from_snapshot(&self, snapshot: ProposalLifecycleRecoverySnapshot) {
        let max_proposal_id = snapshot
            .records
            .iter()
            .map(|record| record.proposal.proposal_id.0)
            .max()
            .unwrap_or(0);

        {
            let mut proposals = self.proposals.borrow_mut();
            proposals.clear();
            proposals.extend(
                snapshot
                    .records
                    .iter()
                    .map(|record| (record.proposal.proposal_id, record.proposal.clone())),
            );
        }
        {
            let mut states = self.proposal_states.borrow_mut();
            states.clear();
            states.extend(
                snapshot
                    .records
                    .iter()
                    .map(|record| (record.proposal.proposal_id, record.state)),
            );
        }
        {
            let mut contexts = self.proposal_contexts.borrow_mut();
            contexts.clear();
            contexts.extend(snapshot.records.into_iter().filter_map(|record| {
                record
                    .context
                    .map(|context| (record.proposal.proposal_id, context))
            }));
        }

        self.next_proposal_id
            .set(snapshot.next_proposal_id.max(max_proposal_id));
        self.next_event_sequence.set(snapshot.next_event_sequence);
    }

    fn proposal_ledger_projection(
        &self,
        generated_at: TimestampMillis,
    ) -> devil_protocol::ProposalLedgerProjection {
        let proposals = self.proposals.borrow();
        let states = self.proposal_states.borrow();
        let mut rows = proposals
            .values()
            .map(|proposal| {
                let state = states
                    .get(&proposal.proposal_id)
                    .copied()
                    .unwrap_or(ProposalLifecycleState::Created);
                Self::proposal_ledger_row(proposal, state, generated_at)
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| (row.created_at.0, row.proposal_id.0));
        let selected_proposal_id = (*self.selected_proposal_id.borrow())
            .filter(|selected| rows.iter().any(|row| row.proposal_id == *selected))
            .or_else(|| rows.last().map(|row| row.proposal_id));

        devil_protocol::ProposalLedgerProjection {
            rows,
            selected_proposal_id,
            omitted_row_count: 0,
            generated_at,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn proposal_ledger_row(
        proposal: &WorkspaceProposal,
        state: ProposalLifecycleState,
        generated_at: TimestampMillis,
    ) -> devil_protocol::ProposalLedgerRow {
        let target_coverage = devil_protocol::proposal_metadata_target_coverage(proposal);
        let payload_kind = devil_protocol::proposal_payload_kind(&proposal.payload);
        let risk_label = Self::proposal_risk_label(&proposal.payload, &target_coverage);
        let privacy_label = if target_coverage.coverage_kind == ProposalTargetCoverageKind::Redacted
            || target_coverage.omitted_target_count > 0
        {
            devil_protocol::ProposalPrivacyLabel::RedactedSensitive
        } else {
            devil_protocol::ProposalPrivacyLabel::WorkspaceMetadata
        };

        devil_protocol::ProposalLedgerRow {
            proposal_id: proposal.proposal_id,
            workspace_id: Self::proposal_workspace_id(proposal, &target_coverage),
            title: Self::bounded_proposal_title(proposal, payload_kind),
            payload_kind,
            lifecycle: Self::lifecycle_state_display(state),
            principal: proposal.principal.clone(),
            capability: proposal.capability.clone(),
            created_at: proposal.created_at,
            updated_at: generated_at,
            expires_at: proposal.expires_at,
            risk_label,
            privacy_label,
            rollback: Self::rollback_availability(&proposal.payload),
            target_coverage: target_coverage.clone(),
            context_manifest: Self::context_manifest_summary(
                proposal.proposal_id,
                &target_coverage,
                privacy_label,
            ),
            diff_summary: Self::diff_summary(&proposal.payload, &target_coverage),
            preview_warnings: Self::preview_warnings(&proposal.payload),
            diagnostics: Self::payload_diagnostics(&proposal.payload),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn lifecycle_state_display(
        state: ProposalLifecycleState,
    ) -> devil_protocol::ProposalLifecycleStateDisplay {
        devil_protocol::ProposalLifecycleStateDisplay {
            state,
            label: format!("{state:?}"),
            description: format!("Proposal lifecycle state is {state:?}"),
        }
    }

    fn bounded_proposal_title(
        proposal: &WorkspaceProposal,
        payload_kind: devil_protocol::ProposalPayloadKind,
    ) -> String {
        let summary = proposal.preview.summary.trim();
        let title = if summary.is_empty() {
            format!("{payload_kind:?} proposal")
        } else {
            summary.to_string()
        };
        title.chars().take(120).collect()
    }

    fn proposal_workspace_id(
        proposal: &WorkspaceProposal,
        target_coverage: &ProposalTargetCoverage,
    ) -> Option<WorkspaceId> {
        target_coverage
            .targets
            .iter()
            .find_map(|target| target.workspace_id)
            .or(match &proposal.payload {
                ProposalPayload::SaveFile(payload) => Some(payload.file.workspace_id),
                ProposalPayload::DeleteFile(payload) => Some(payload.file.workspace_id),
                ProposalPayload::RenameFile(payload) => Some(payload.file.workspace_id),
                ProposalPayload::FormatFile(payload) => Some(payload.file.workspace_id),
                ProposalPayload::CodeAction(payload) => Some(payload.file.workspace_id),
                ProposalPayload::WorkspaceEdit(payload) => Some(payload.workspace_id),
                _ => None,
            })
    }

    fn proposal_risk_label(
        payload: &ProposalPayload,
        target_coverage: &ProposalTargetCoverage,
    ) -> devil_protocol::ProposalRiskLabel {
        if target_coverage.coverage_kind != ProposalTargetCoverageKind::Complete
            || target_coverage.omitted_target_count > 0
        {
            return devil_protocol::ProposalRiskLabel::Unknown;
        }

        match payload {
            ProposalPayload::TerminalCommand(_) | ProposalPayload::DeleteFile(_) => {
                devil_protocol::ProposalRiskLabel::High
            }
            ProposalPayload::Batch(_)
            | ProposalPayload::WorkspaceEdit(_)
            | ProposalPayload::RenameFile(_)
            | ProposalPayload::CodeAction(_) => devil_protocol::ProposalRiskLabel::Medium,
            ProposalPayload::TextEdit(_)
            | ProposalPayload::CreateFile(_)
            | ProposalPayload::SaveFile(_)
            | ProposalPayload::FormatFile(_) => devil_protocol::ProposalRiskLabel::Low,
        }
    }

    fn rollback_availability(
        payload: &ProposalPayload,
    ) -> devil_protocol::ProposalRollbackAvailability {
        match payload {
            ProposalPayload::Batch(batch) => match batch.rollback_policy {
                ProposalBatchRollbackPolicy::Required => {
                    if batch.rollback_steps.is_empty()
                        || batch.rollback_steps.iter().any(|step| {
                            step.action == devil_protocol::ProposalRollbackAction::Unsupported
                        })
                    {
                        devil_protocol::ProposalRollbackAvailability::Unavailable
                    } else {
                        devil_protocol::ProposalRollbackAvailability::Available
                    }
                }
                ProposalBatchRollbackPolicy::BestEffort => {
                    devil_protocol::ProposalRollbackAvailability::BestEffort
                }
                ProposalBatchRollbackPolicy::NotSupported => {
                    devil_protocol::ProposalRollbackAvailability::Unavailable
                }
                ProposalBatchRollbackPolicy::NotRequired => {
                    devil_protocol::ProposalRollbackAvailability::NotRequired
                }
            },
            ProposalPayload::WorkspaceEdit(_) => {
                devil_protocol::ProposalRollbackAvailability::BestEffort
            }
            ProposalPayload::TerminalCommand(_) => {
                devil_protocol::ProposalRollbackAvailability::Unavailable
            }
            ProposalPayload::TextEdit(_)
            | ProposalPayload::CreateFile(_)
            | ProposalPayload::DeleteFile(_)
            | ProposalPayload::RenameFile(_)
            | ProposalPayload::SaveFile(_)
            | ProposalPayload::FormatFile(_)
            | ProposalPayload::CodeAction(_) => {
                devil_protocol::ProposalRollbackAvailability::Unknown
            }
        }
    }

    fn context_manifest_summary(
        proposal_id: ProposalId,
        target_coverage: &ProposalTargetCoverage,
        privacy_label: devil_protocol::ProposalPrivacyLabel,
    ) -> devil_protocol::ProposalContextManifestSummary {
        let item_count = target_coverage.targets.len() as u32;
        let categories = if item_count == 0 && target_coverage.omitted_target_count == 0 {
            Vec::new()
        } else {
            vec![devil_protocol::ProposalContextManifestEntrySummary {
                category: "proposal_targets".to_string(),
                item_count,
                omitted_item_count: target_coverage.omitted_target_count,
                privacy_label,
                manifest_hash: None,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            }]
        };
        let category_count = categories.len() as u32;

        devil_protocol::ProposalContextManifestSummary {
            manifest_id: format!("proposal:{}:context", proposal_id.0),
            category_count,
            total_item_count: item_count,
            omitted_item_count: target_coverage.omitted_target_count,
            categories,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        }
    }

    fn diff_summary(
        payload: &ProposalPayload,
        target_coverage: &ProposalTargetCoverage,
    ) -> devil_protocol::ProposalDiffSummary {
        let (kind, hunk_count) = match payload {
            ProposalPayload::TextEdit(payload) => (
                devil_protocol::ProposalDiffSummaryKind::Text,
                payload.edits.edits.len() as u32,
            ),
            ProposalPayload::CreateFile(_)
            | ProposalPayload::DeleteFile(_)
            | ProposalPayload::RenameFile(_) => {
                (devil_protocol::ProposalDiffSummaryKind::FileOperation, 1)
            }
            ProposalPayload::WorkspaceEdit(payload) => (
                devil_protocol::ProposalDiffSummaryKind::WorkspaceEdit,
                payload
                    .file_edits
                    .iter()
                    .map(|file| file.edits.edits.len() as u32)
                    .sum::<u32>()
                    .saturating_add(payload.file_operations.len() as u32),
            ),
            ProposalPayload::Batch(payload) => (
                devil_protocol::ProposalDiffSummaryKind::WorkspaceEdit,
                payload.items.len() as u32,
            ),
            ProposalPayload::SaveFile(_)
            | ProposalPayload::FormatFile(_)
            | ProposalPayload::CodeAction(_) => (devil_protocol::ProposalDiffSummaryKind::Text, 0),
            ProposalPayload::TerminalCommand(_) => {
                (devil_protocol::ProposalDiffSummaryKind::TerminalMetadata, 0)
            }
        };

        devil_protocol::ProposalDiffSummary {
            kind,
            target_count: target_coverage.targets.len() as u32,
            hunk_count,
            inserted_line_count: 0,
            deleted_line_count: 0,
            omitted_hunk_count: 0,
            full_source_redacted: true,
            diff_hash: None,
            chunks: target_coverage
                .targets
                .iter()
                .enumerate()
                .map(
                    |(index, target)| devil_protocol::ProposalDiffChunkDescriptor {
                        chunk_id: format!("metadata-chunk:{index}"),
                        target_id: Some(target.target_id.clone()),
                        byte_range: target.byte_ranges.first().copied(),
                        changed_line_count: 0,
                        inserted_line_count: 0,
                        deleted_line_count: 0,
                        content_hash: None,
                    },
                )
                .collect(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
        }
    }

    fn preview_warnings(payload: &ProposalPayload) -> Vec<ProposalPreviewWarning> {
        match payload {
            ProposalPayload::Batch(payload) => payload.preview_warnings.clone(),
            _ => Vec::new(),
        }
    }

    fn payload_diagnostics(payload: &ProposalPayload) -> Vec<ProtocolDiagnostic> {
        match payload {
            ProposalPayload::SaveFile(payload) => payload.diagnostics.clone(),
            ProposalPayload::WorkspaceEdit(payload) => payload.diagnostics.clone(),
            _ => Vec::new(),
        }
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

    fn record_audit_failure_transition(&self, transition: &ProposalLifecycleTransition) {
        if self.has_lifecycle_context(transition.proposal_id) {
            self.proposal_states
                .borrow_mut()
                .insert(transition.proposal_id, transition.lifecycle_state);
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

        if (matches!(
            route,
            ProposalExecutionRoute::Terminal | ProposalExecutionRoute::Unsupported
        ) || (route == ProposalExecutionRoute::Mixed
            && !matches!(proposal.payload, ProposalPayload::WorkspaceEdit(_))))
            && diagnostics.is_empty()
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
    open_tabs: Vec<BufferId>,
    active_file_id: Option<FileId>,
    active_file_path: Option<String>,
    active_buffer_id: Option<BufferId>,
    active_file_metadata: Option<ActiveFileMetadata>,
    buffer_file_metadata: HashMap<BufferId, ActiveFileMetadata>,
    viewport_scrolls: HashMap<BufferId, ViewportScroll>,
    close_dirty_prompt: Option<CloseDirtyPromptState>,
}

impl ActiveDocumentController {
    fn new() -> Self {
        Self {
            opened_workspace: None,
            workspace_root_path: None,
            active_principal_id: None,
            active_workspace_trust: None,
            open_tabs: Vec::new(),
            active_file_id: None,
            active_file_path: None,
            active_buffer_id: None,
            active_file_metadata: None,
            buffer_file_metadata: HashMap::new(),
            viewport_scrolls: HashMap::new(),
            close_dirty_prompt: None,
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
        self.open_tabs.clear();
        self.active_file_id = None;
        self.active_file_path = None;
        self.active_buffer_id = None;
        self.active_file_metadata = None;
        self.buffer_file_metadata.clear();
        self.viewport_scrolls.clear();
        self.close_dirty_prompt = None;
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
        self.activate_metadata(buffer_id, &metadata);
        if !self.open_tabs.contains(&buffer_id) {
            self.open_tabs.push(buffer_id);
        }
        self.buffer_file_metadata.insert(buffer_id, metadata);
        self.close_dirty_prompt = None;
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
        let workspace_generation = metadata.workspace_generation;
        if self.active_buffer_id == Some(buffer_id) {
            self.activate_metadata(buffer_id, &metadata);
        }
        self.buffer_file_metadata.insert(buffer_id, metadata);
        self.refresh_workspace_generation(workspace_generation);
    }

    fn refresh_workspace_generation(&mut self, workspace_generation: WorkspaceGeneration) {
        for metadata in self.buffer_file_metadata.values_mut() {
            metadata.workspace_generation = workspace_generation;
        }
        if let Some(metadata) = self.active_file_metadata.as_mut() {
            metadata.workspace_generation = workspace_generation;
        }
    }

    fn clear_dirty_prompt_for(&mut self, buffer_id: BufferId) {
        if self
            .close_dirty_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.buffer_id == buffer_id)
        {
            self.close_dirty_prompt = None;
        }
    }

    fn activate_metadata(&mut self, buffer_id: BufferId, metadata: &ActiveFileMetadata) {
        self.active_file_id = Some(metadata.identity.file_id);
        self.active_file_path = Some(metadata.identity.canonical_path.0.clone());
        self.active_buffer_id = Some(buffer_id);
        self.active_file_metadata = Some(metadata.clone());
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

    fn require_open_buffer(&self, buffer_id: BufferId) -> Result<(), AppCompositionError> {
        if self.open_tabs.contains(&buffer_id) && self.metadata_for_buffer(buffer_id).is_some() {
            Ok(())
        } else {
            Err(AppCompositionError::BufferNotOpen(buffer_id))
        }
    }

    fn switch_to_buffer(&mut self, buffer_id: BufferId) -> Result<(), AppCompositionError> {
        let metadata = self
            .metadata_for_buffer(buffer_id)
            .cloned()
            .ok_or(AppCompositionError::BufferNotOpen(buffer_id))?;
        if !self.open_tabs.contains(&buffer_id) {
            return Err(AppCompositionError::BufferNotOpen(buffer_id));
        }
        self.activate_metadata(buffer_id, &metadata);
        self.close_dirty_prompt = None;
        Ok(())
    }

    fn remove_open_tab(&mut self, buffer_id: BufferId) {
        self.open_tabs.retain(|candidate| *candidate != buffer_id);
        self.buffer_file_metadata.remove(&buffer_id);
        self.viewport_scrolls.remove(&buffer_id);
        if self
            .close_dirty_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.buffer_id == buffer_id)
        {
            self.close_dirty_prompt = None;
        }
        if self.active_buffer_id == Some(buffer_id) {
            self.active_buffer_id = None;
            self.active_file_id = None;
            self.active_file_path = None;
            self.active_file_metadata = None;
        }
    }

    fn activate_first_available_tab(&mut self) {
        if self.active_buffer_id.is_some() {
            return;
        }
        if let Some(buffer_id) = self.open_tabs.first().copied()
            && let Some(metadata) = self.metadata_for_buffer(buffer_id).cloned()
        {
            self.activate_metadata(buffer_id, &metadata);
        }
    }

    fn set_viewport_scroll(
        &mut self,
        buffer_id: BufferId,
        scroll: ViewportScroll,
    ) -> Result<(), AppCompositionError> {
        self.require_open_buffer(buffer_id)?;
        self.viewport_scrolls.insert(buffer_id, scroll);
        Ok(())
    }

    fn viewport_scroll_for(&self, buffer_id: BufferId) -> ViewportScroll {
        self.viewport_scrolls
            .get(&buffer_id)
            .copied()
            .unwrap_or(ViewportScroll {
                top_line: 0,
                left_column: 0,
            })
    }

    fn prompt_dirty_close(&mut self, buffer_id: BufferId) -> Result<(), AppCompositionError> {
        let metadata = self
            .metadata_for_buffer(buffer_id)
            .cloned()
            .ok_or(AppCompositionError::BufferNotOpen(buffer_id))?;
        let title = tab_title(&metadata.identity.canonical_path);
        self.close_dirty_prompt = Some(CloseDirtyPromptState {
            buffer_id,
            file_id: Some(metadata.identity.file_id),
            path: Some(metadata.identity.canonical_path),
            title,
        });
        Ok(())
    }

    fn cancel_dirty_close(&mut self, buffer_id: BufferId) -> Result<(), AppCompositionError> {
        self.require_open_buffer(buffer_id)?;
        if self
            .close_dirty_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.buffer_id == buffer_id)
        {
            self.close_dirty_prompt = None;
            Ok(())
        } else {
            Err(AppCompositionError::DirtyClosePromptMissing(buffer_id))
        }
    }

    fn save_context_for_buffer(
        &self,
        buffer_id: BufferId,
    ) -> Result<ActiveSaveContext, AppCompositionError> {
        Ok(ActiveSaveContext {
            workspace_id: self.require_workspace_id()?,
            buffer_id,
            metadata: self
                .metadata_for_buffer(buffer_id)
                .cloned()
                .ok_or(AppCompositionError::BufferNotOpen(buffer_id))?,
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

    fn require_workspace_context(&self) -> Result<ActiveWorkspaceContext, AppCompositionError> {
        let opened = self
            .opened_workspace
            .clone()
            .ok_or(AppCompositionError::WorkspaceNotOpen)?;
        Ok(ActiveWorkspaceContext {
            workspace_id: opened.workspace_id,
            workspace_generation: opened.generation,
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
struct ActiveWorkspaceContext {
    workspace_id: WorkspaceId,
    workspace_generation: WorkspaceGeneration,
    principal: PrincipalId,
    trust: WorkspaceTrustState,
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
struct LanguageRequestInput {
    workspace_id: WorkspaceId,
    buffer_id: BufferId,
    metadata: ActiveFileMetadata,
    principal: PrincipalId,
    text: String,
    snapshot_id: devil_protocol::SnapshotId,
    buffer_version: devil_protocol::BufferVersion,
    event_context: EventContext,
}

#[derive(Debug, Clone, Copy)]
enum LanguageReadKind {
    Hover,
    Completion,
    Definition,
    References,
    Outline,
}

#[derive(Debug, Clone, Copy)]
enum LanguageProposalKind {
    Formatting,
    Rename,
    OrganizeImports,
    CodeAction,
}

#[derive(Debug, Clone)]
struct LanguageToolingWorkflow {
    projection: LanguageToolingProjection,
    semantic_index: SemanticIndex,
    next_operation_id: u64,
}

impl Default for LanguageToolingWorkflow {
    fn default() -> Self {
        Self {
            projection: LanguageToolingProjection::empty(),
            semantic_index: SemanticIndex::new(),
            next_operation_id: 0,
        }
    }
}

impl LanguageToolingWorkflow {
    fn projection(&self) -> LanguageToolingProjection {
        self.projection.clone()
    }

    fn cancel_operation(&mut self, operation_id: String, event_context: EventContext) {
        self.projection.status = LanguageToolingStatusKind::Cancelled;
        self.projection.status_message = format!("Language operation {operation_id} cancelled");
        self.projection.cancellation_count = self.projection.cancellation_count.saturating_add(1);
        self.projection.generated_at = TimestampMillis::now();
        self.push_operation(LanguageToolingOperationProjection {
            operation_id,
            kind: LanguageToolingOperationKind::Diagnostics,
            status: LanguageToolingStatusKind::Cancelled,
            request_id: None,
            proposal_id: None,
            message: "cancelled by app authority".to_string(),
            correlation_id: Some(event_context.correlation_id),
            causality_id: Some(event_context.causality_id),
            generated_at: TimestampMillis::now(),
            schema_version: 1,
        });
    }

    fn run_read(
        &mut self,
        input: LanguageRequestInput,
        kind: LanguageReadKind,
        position: TextCoordinate,
    ) -> LanguageToolingProjection {
        let operation_kind = match kind {
            LanguageReadKind::Hover => LanguageToolingOperationKind::Hover,
            LanguageReadKind::Completion => LanguageToolingOperationKind::Completion,
            LanguageReadKind::Definition => LanguageToolingOperationKind::Definition,
            LanguageReadKind::References => LanguageToolingOperationKind::References,
            LanguageReadKind::Outline => LanguageToolingOperationKind::Outline,
        };
        let operation_id = self.next_operation_id(operation_kind);
        let same_identity = self.projection.workspace_id == Some(input.workspace_id)
            && self.projection.buffer_id == Some(input.buffer_id)
            && self.projection.file_id == Some(input.metadata.identity.file_id);
        let previous_projection = if same_identity {
            self.projection.clone()
        } else {
            let mut projection = LanguageToolingProjection::empty();
            projection.operations = self.projection.operations.clone();
            projection.cancellation_count = self.projection.cancellation_count;
            projection.stale_result_count = if self.projection.buffer_id.is_some() {
                self.projection.stale_result_count.saturating_add(1)
            } else {
                self.projection.stale_result_count
            };
            projection
        };
        let language_id = language_id_for_path(&input.metadata.identity.canonical_path);
        let document = SourceDocument::with_versions(
            input.workspace_id,
            input.metadata.identity.file_id,
            input.metadata.identity.canonical_path.clone(),
            language_id.clone(),
            input.metadata.file_content_version,
            input.metadata.workspace_generation,
            Some(input.snapshot_id),
            SemanticPrivacyScope::Workspace,
            input.text,
        );
        let file_index = LexicalIndexer::new().index_document(
            &document,
            SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        );
        let diagnostics = file_index.diagnostics.clone();
        self.semantic_index.upsert(file_index);
        let query_kind = match kind {
            LanguageReadKind::Hover => SemanticQueryKind::HoverEnrichment,
            LanguageReadKind::Completion => SemanticQueryKind::CompletionRanking,
            LanguageReadKind::Definition => SemanticQueryKind::Definition,
            LanguageReadKind::References => SemanticQueryKind::References,
            LanguageReadKind::Outline => SemanticQueryKind::SymbolLookup,
        };
        let response = self.semantic_index.query(&SemanticQueryRequest {
            query_id: SemanticQueryId(uuid::Uuid::now_v7()),
            kind: query_kind,
            scope: SemanticQueryScope {
                workspace_id: input.workspace_id,
                file_ids: vec![input.metadata.identity.file_id],
                paths: vec![input.metadata.identity.canonical_path.clone()],
                language_ids: vec![language_id],
                privacy_scope: SemanticPrivacyScope::Workspace,
            },
            position: Some(position),
            text_query_hash: None,
            limit: 25,
            cancellation_token: CancellationTokenId(uuid::Uuid::now_v7()),
            freshness_policy: SemanticQueryFreshnessPolicy::BestEffort,
            correlation_id: input.event_context.correlation_id,
            causality_id: input.event_context.causality_id,
            schema_version: 1,
        });

        let problems = diagnostics
            .into_iter()
            .map(|diagnostic| LanguageProblemProjection {
                file_id: Some(input.metadata.identity.file_id),
                path: diagnostic
                    .path
                    .or_else(|| Some(input.metadata.identity.canonical_path.clone())),
                range: diagnostic.range,
                severity: diagnostic.severity,
                code_label: Some(diagnostic.code),
                message: bounded_label(diagnostic.message, 160),
                source_label: Some("devil-index".to_string()),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            })
            .collect::<Vec<_>>();
        let locations = response
            .results
            .iter()
            .enumerate()
            .map(|(index, result)| LanguageLocationProjection {
                location_id: result.result_id.0.clone(),
                file_id: result.file_id,
                path: result.path.clone(),
                range: result.range,
                label: bounded_label(
                    if result.label.is_empty() {
                        format!("result-{index}")
                    } else {
                        result.label.clone()
                    },
                    80,
                ),
                degraded: false,
                schema_version: 1,
            })
            .collect::<Vec<_>>();
        let completions = response
            .results
            .iter()
            .take(20)
            .map(|result| LanguageCompletionProjection {
                completion_id: result.result_id.0.clone(),
                label: bounded_label(result.label.clone(), 80),
                detail_label: Some(format!("{:?}", result.kind)),
                kind_label: format!("{:?}", result.kind),
                score_basis_points: result.score_basis_points,
                degraded: false,
                schema_version: 1,
            })
            .collect::<Vec<_>>();
        let outline = self
            .semantic_index
            .symbols()
            .iter()
            .filter(|symbol| symbol.file_id == input.metadata.identity.file_id)
            .take(50)
            .map(|symbol| LanguageOutlineSymbolProjection {
                symbol_id: symbol.symbol_id.0.clone(),
                label: bounded_label(
                    symbol
                        .display_name
                        .clone()
                        .unwrap_or_else(|| symbol.symbol_name_hash.value.clone()),
                    80,
                ),
                kind_label: bounded_label(symbol.kind.clone(), 48),
                range: symbol.declaration_range,
                depth: 0,
                children_omitted: false,
                schema_version: 1,
            })
            .collect::<Vec<_>>();

        let hover = response
            .results
            .first()
            .map(|result| LanguageHoverProjection {
                hover_id: format!("hover:{}", operation_id),
                file_id: result.file_id,
                range: result.range,
                label: bounded_label(result.label.clone(), 80),
                summary: format!("{:?} from semantic index", result.kind),
                degraded: false,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            });

        self.projection = LanguageToolingProjection {
            workspace_id: Some(input.workspace_id),
            buffer_id: Some(input.buffer_id),
            file_id: Some(input.metadata.identity.file_id),
            status: LanguageToolingStatusKind::Ready,
            status_message: format!("{operation_kind:?} ready"),
            problems,
            hover: if matches!(kind, LanguageReadKind::Hover) {
                hover
            } else {
                previous_projection.hover
            },
            completions: if matches!(kind, LanguageReadKind::Completion) {
                completions
            } else {
                previous_projection.completions
            },
            definitions: if matches!(kind, LanguageReadKind::Definition) {
                locations.clone()
            } else {
                previous_projection.definitions
            },
            references: if matches!(kind, LanguageReadKind::References) {
                locations
            } else {
                previous_projection.references
            },
            outline: if matches!(kind, LanguageReadKind::Outline) {
                outline
            } else {
                previous_projection.outline
            },
            operations: previous_projection.operations,
            stale_result_count: previous_projection.stale_result_count,
            cancellation_count: previous_projection.cancellation_count,
            generated_at: TimestampMillis::now(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        self.push_operation(LanguageToolingOperationProjection {
            operation_id,
            kind: operation_kind,
            status: LanguageToolingStatusKind::Ready,
            request_id: Some(devil_protocol::LspRequestId(uuid::Uuid::now_v7())),
            proposal_id: None,
            message: "semantic projection refreshed".to_string(),
            correlation_id: Some(input.event_context.correlation_id),
            causality_id: Some(input.event_context.causality_id),
            generated_at: TimestampMillis::now(),
            schema_version: 1,
        });
        self.projection()
    }

    fn record_proposal(
        &mut self,
        input: &LanguageRequestInput,
        kind: LanguageProposalKind,
        proposal_id: ProposalId,
        message: String,
    ) -> LanguageToolingProjection {
        let operation_kind = match kind {
            LanguageProposalKind::Formatting => LanguageToolingOperationKind::FormattingProposal,
            LanguageProposalKind::Rename => LanguageToolingOperationKind::RenameProposal,
            LanguageProposalKind::OrganizeImports => {
                LanguageToolingOperationKind::OrganizeImportsProposal
            }
            LanguageProposalKind::CodeAction => LanguageToolingOperationKind::CodeActionProposal,
        };
        self.projection.workspace_id = Some(input.workspace_id);
        self.projection.buffer_id = Some(input.buffer_id);
        self.projection.file_id = Some(input.metadata.identity.file_id);
        self.projection.status = LanguageToolingStatusKind::Ready;
        self.projection.status_message = message.clone();
        self.projection.generated_at = TimestampMillis::now();
        let operation_id = self.next_operation_id(operation_kind);
        self.push_operation(LanguageToolingOperationProjection {
            operation_id,
            kind: operation_kind,
            status: LanguageToolingStatusKind::Ready,
            request_id: Some(devil_protocol::LspRequestId(uuid::Uuid::now_v7())),
            proposal_id: Some(proposal_id),
            message,
            correlation_id: Some(input.event_context.correlation_id),
            causality_id: Some(input.event_context.causality_id),
            generated_at: TimestampMillis::now(),
            schema_version: 1,
        });
        self.projection()
    }

    fn record_proposal_failure(
        &mut self,
        input: &LanguageRequestInput,
        kind: LanguageProposalKind,
        message: String,
    ) -> LanguageToolingProjection {
        let operation_kind = match kind {
            LanguageProposalKind::Formatting => LanguageToolingOperationKind::FormattingProposal,
            LanguageProposalKind::Rename => LanguageToolingOperationKind::RenameProposal,
            LanguageProposalKind::OrganizeImports => {
                LanguageToolingOperationKind::OrganizeImportsProposal
            }
            LanguageProposalKind::CodeAction => LanguageToolingOperationKind::CodeActionProposal,
        };
        self.projection.workspace_id = Some(input.workspace_id);
        self.projection.buffer_id = Some(input.buffer_id);
        self.projection.file_id = Some(input.metadata.identity.file_id);
        self.projection.status = LanguageToolingStatusKind::Failed;
        self.projection.status_message = message.clone();
        self.projection.generated_at = TimestampMillis::now();
        let operation_id = self.next_operation_id(operation_kind);
        self.push_operation(LanguageToolingOperationProjection {
            operation_id,
            kind: operation_kind,
            status: LanguageToolingStatusKind::Failed,
            request_id: Some(devil_protocol::LspRequestId(uuid::Uuid::now_v7())),
            proposal_id: None,
            message,
            correlation_id: Some(input.event_context.correlation_id),
            causality_id: Some(input.event_context.causality_id),
            generated_at: TimestampMillis::now(),
            schema_version: 1,
        });
        self.projection()
    }

    fn next_operation_id(&mut self, kind: LanguageToolingOperationKind) -> String {
        self.next_operation_id = self.next_operation_id.saturating_add(1).max(1);
        format!("language:{kind:?}:{}", self.next_operation_id)
    }

    fn push_operation(&mut self, operation: LanguageToolingOperationProjection) {
        self.projection.operations.push(operation);
        if self.projection.operations.len() > 20 {
            let excess = self.projection.operations.len() - 20;
            self.projection.operations.drain(0..excess);
        }
    }
}

#[derive(Debug, Clone)]
struct TerminalWorkflow {
    projection: TerminalPanelProjection,
    fixture_enabled: bool,
    fixture: TerminalFixtureRuntime,
    security_broker: DenyByDefaultBroker,
    last_audit: Option<devil_protocol::TerminalAuditRecord>,
    next_sequence: u64,
}

#[derive(Debug, Clone)]
struct TerminalDenial {
    workspace_id: WorkspaceId,
    policy: TerminalPolicyProjection,
    reason: String,
    event_context: EventContext,
    session_id: Option<TerminalSessionId>,
    action: String,
    clear_active_session: bool,
}

impl Default for TerminalWorkflow {
    fn default() -> Self {
        Self {
            projection: TerminalPanelProjection::empty(),
            fixture_enabled: false,
            fixture: TerminalFixtureRuntime::new(TerminalFixtureConfig::default()),
            security_broker: DenyByDefaultBroker::new(
                SecurityPolicy::default(),
                CapabilityNamespace("app.terminal".to_string()),
            ),
            last_audit: None,
            next_sequence: 0,
        }
    }
}

impl TerminalWorkflow {
    fn projection(&self) -> TerminalPanelProjection {
        self.projection.clone()
    }

    fn enable_fixture(&mut self) {
        self.fixture_enabled = true;
        self.fixture = TerminalFixtureRuntime::new(TerminalFixtureConfig::enabled());
        let mut policy = SecurityPolicy::default();
        policy.terminal_policy.runtime_enabled = true;
        self.security_broker =
            DenyByDefaultBroker::new(policy, CapabilityNamespace("app.terminal".to_string()));
        self.projection.status = TerminalPanelStatus {
            kind: TerminalPanelStatusKind::Idle,
            message: "Terminal fixture enabled".to_string(),
        };
        self.projection.generated_at = TimestampMillis::now();
    }

    fn take_last_audit(&mut self) -> Option<devil_protocol::TerminalAuditRecord> {
        self.last_audit.take()
    }

    fn launch(
        &mut self,
        context: ActiveWorkspaceContext,
        command_label: String,
        event_context: EventContext,
    ) -> TerminalPanelProjection {
        let decision = self.capability_decision(
            &context,
            "terminal.launch",
            Some(command_label.as_str()),
            event_context,
        );
        let policy = Self::policy_projection(&context, "terminal.launch", &decision);
        self.projection.workspace_id = Some(context.workspace_id);
        self.projection.policy = Some(policy.clone());
        self.projection.generated_at = TimestampMillis::now();

        if !decision.granted {
            return self.deny(TerminalDenial {
                workspace_id: context.workspace_id,
                policy,
                reason: decision
                    .reason
                    .unwrap_or_else(|| "terminal launch denied by policy".to_string()),
                event_context,
                session_id: None,
                action: "launch".to_string(),
                clear_active_session: true,
            });
        }
        if !self.fixture_enabled {
            return self.deny(TerminalDenial {
                workspace_id: context.workspace_id,
                policy,
                reason: "Terminal fixture runtime is disabled".to_string(),
                event_context,
                session_id: None,
                action: "launch".to_string(),
                clear_active_session: true,
            });
        }

        let launch_policy = devil_protocol::TerminalLaunchPolicyContract {
            principal_id: context.principal.clone(),
            workspace_id: context.workspace_id,
            trust_state: context.trust.clone(),
            capability_id: CapabilityId("terminal.launch".to_string()),
            cwd_policy: "workspace-root".to_string(),
            output_byte_limit: 256 * 1024,
            timeout_seconds: 30,
            schema_version: 1,
        };
        let runtime_label = std::any::type_name::<
            devil_terminal::TerminalRuntime<devil_platform::NativePtyService>,
        >();
        match self.fixture.launch(launch_policy) {
            Ok(audit) => {
                let session_id = audit.session_id;
                let output = self.fixture.output_chunk(
                    &audit,
                    format!(
                        "fixture terminal ready: command_label_bytes={}; runtime={runtime_label}",
                        command_label.len()
                    ),
                    command_label.len() as u64,
                );
                self.projection.active_session_id = Some(session_id);
                self.projection.runtime_state = Some(TerminalRuntimeState::Running);
                self.projection.status = TerminalPanelStatus {
                    kind: TerminalPanelStatusKind::Running,
                    message: "Terminal fixture running".to_string(),
                };
                self.projection.last_denial = None;
                self.projection.last_error = None;
                self.record_audit(
                    session_id,
                    TerminalRuntimeState::Running,
                    event_context,
                    format!(
                        "action=launch state=running decision_id={} command_label_bytes={} output_limit=262144",
                        decision.decision_id.0,
                        command_label.len()
                    ),
                );
                if let Ok(output) = output {
                    self.push_terminal_output(output, false);
                }
                self.projection()
            }
            Err(error) => self.deny(TerminalDenial {
                workspace_id: context.workspace_id,
                policy,
                reason: format!("Terminal launch denied: {error}"),
                event_context,
                session_id: None,
                action: "launch".to_string(),
                clear_active_session: true,
            }),
        }
    }

    fn input(
        &mut self,
        context: ActiveWorkspaceContext,
        session_id: TerminalSessionId,
        payload: String,
        event_context: EventContext,
    ) -> TerminalPanelProjection {
        if !self.session_matches(session_id) {
            return self.fail(
                session_id,
                event_context,
                format!("terminal session {} is not active", session_id.0),
            );
        }
        let decision = self.capability_decision(&context, "terminal.input", None, event_context);
        let policy = Self::policy_projection(&context, "terminal.input", &decision);
        self.projection.policy = Some(policy.clone());
        if !decision.granted {
            return self.deny(TerminalDenial {
                workspace_id: context.workspace_id,
                policy,
                reason: decision
                    .reason
                    .unwrap_or_else(|| "terminal input denied by policy".to_string()),
                event_context,
                session_id: Some(session_id),
                action: "input".to_string(),
                clear_active_session: false,
            });
        }
        let input = TerminalInput {
            session_id,
            correlation_id: event_context.correlation_id,
            payload: payload.clone(),
        };
        if let Err(error) = validate_terminal_input(&input) {
            return self.fail(session_id, event_context, error.message);
        }
        self.push_row(
            session_id,
            format!("input accepted bytes={}", payload.len()),
            payload.len() as u64,
            false,
        );
        self.projection.status = TerminalPanelStatus {
            kind: TerminalPanelStatusKind::Running,
            message: "Terminal input accepted".to_string(),
        };
        self.projection.generated_at = TimestampMillis::now();
        self.record_audit(
            session_id,
            TerminalRuntimeState::Running,
            event_context,
            format!("action=input state=running input_bytes={}", payload.len()),
        );
        self.projection()
    }

    fn resize(
        &mut self,
        context: ActiveWorkspaceContext,
        session_id: TerminalSessionId,
        cols: u16,
        rows: u16,
        event_context: EventContext,
    ) -> TerminalPanelProjection {
        if !self.session_matches(session_id) {
            return self.fail(
                session_id,
                event_context,
                format!("terminal session {} is not active", session_id.0),
            );
        }
        let decision = self.capability_decision(&context, "terminal.resize", None, event_context);
        let policy = Self::policy_projection(&context, "terminal.resize", &decision);
        self.projection.policy = Some(policy.clone());
        if !decision.granted {
            return self.deny(TerminalDenial {
                workspace_id: context.workspace_id,
                policy,
                reason: decision
                    .reason
                    .unwrap_or_else(|| "terminal resize denied by policy".to_string()),
                event_context,
                session_id: Some(session_id),
                action: "resize".to_string(),
                clear_active_session: false,
            });
        }
        let resize = TerminalResize {
            session_id,
            cols,
            rows,
        };
        if let Err(error) = validate_terminal_resize(&resize) {
            return self.fail(session_id, event_context, error.message);
        }
        self.projection.status = TerminalPanelStatus {
            kind: TerminalPanelStatusKind::Running,
            message: format!("Terminal resized to {cols}x{rows}"),
        };
        self.projection.generated_at = TimestampMillis::now();
        self.record_audit(
            session_id,
            TerminalRuntimeState::Running,
            event_context,
            format!("action=resize state=running cols={cols} rows={rows}"),
        );
        self.projection()
    }

    fn poll(
        &mut self,
        session_id: TerminalSessionId,
        event_context: EventContext,
    ) -> TerminalPanelProjection {
        if !self.session_matches(session_id) {
            return self.fail(
                session_id,
                event_context,
                format!("terminal session {} is not active", session_id.0),
            );
        }
        self.push_row(session_id, "poll complete".to_string(), 13, false);
        self.projection.generated_at = TimestampMillis::now();
        self.record_audit(
            session_id,
            TerminalRuntimeState::Running,
            event_context,
            "action=poll state=running output_poll=bounded".to_string(),
        );
        self.projection()
    }

    fn search(
        &mut self,
        session_id: TerminalSessionId,
        query: String,
        event_context: EventContext,
    ) -> TerminalPanelProjection {
        if !self.session_matches(session_id) {
            return self.fail(
                session_id,
                event_context,
                format!("terminal session {} is not active", session_id.0),
            );
        }
        let query_label = bounded_label(query, 80);
        let query_bytes = query_label.len();
        let match_count = self
            .projection
            .output_rows
            .iter()
            .filter(|row| row.redacted_payload.contains(&query_label))
            .count() as u32;
        self.projection.search = TerminalSearchProjection {
            query_label: Some(query_label),
            match_count,
            active_match_index: (match_count > 0).then_some(0),
            truncated: false,
            schema_version: 1,
        };
        self.projection.generated_at = TimestampMillis::now();
        self.record_audit(
            session_id,
            TerminalRuntimeState::Running,
            event_context,
            format!(
                "action=search state=running query_bytes={} match_count={match_count}",
                query_bytes
            ),
        );
        self.projection()
    }

    fn close_or_kill(
        &mut self,
        context: ActiveWorkspaceContext,
        session_id: TerminalSessionId,
        killed: bool,
        event_context: EventContext,
    ) -> TerminalPanelProjection {
        if !self.session_matches(session_id) {
            return self.fail(
                session_id,
                event_context,
                format!("terminal session {} is not active", session_id.0),
            );
        }
        let capability = if killed {
            "terminal.kill"
        } else {
            "terminal.close"
        };
        let decision = self.capability_decision(&context, capability, None, event_context);
        let policy = Self::policy_projection(&context, capability, &decision);
        self.projection.policy = Some(policy.clone());
        if !decision.granted {
            return self.deny(TerminalDenial {
                workspace_id: context.workspace_id,
                policy,
                reason: decision.reason.unwrap_or_else(|| {
                    if killed {
                        "terminal kill denied by policy".to_string()
                    } else {
                        "terminal close denied by policy".to_string()
                    }
                }),
                event_context,
                session_id: Some(session_id),
                action: capability
                    .strip_prefix("terminal.")
                    .unwrap_or(capability)
                    .to_string(),
                clear_active_session: false,
            });
        }
        if killed {
            let request = TerminalKillRequest {
                session_id,
                principal_id: context.principal.clone(),
                capability_id: CapabilityId("terminal.kill".to_string()),
                escalation: TerminalKillEscalation::Terminate,
                kill_tree_authorized: false,
                escalation_timeout_ms: 5_000,
                event_sequence: self.next_event_sequence(),
                correlation_id: event_context.correlation_id,
                causality_id: event_context.causality_id,
                metadata_summary: "action=kill escalation=terminate".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            };
            if let Err(error) = validate_terminal_kill_request(&request) {
                return self.fail(session_id, event_context, error.message);
            }
        } else {
            let request = TerminalCloseRequest {
                session_id,
                principal_id: context.principal.clone(),
                capability_id: CapabilityId("terminal.close".to_string()),
                event_sequence: self.next_event_sequence(),
                correlation_id: event_context.correlation_id,
                causality_id: event_context.causality_id,
                metadata_summary: "action=close".to_string(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            };
            if let Err(error) = validate_terminal_close_request(&request) {
                return self.fail(session_id, event_context, error.message);
            }
        }
        self.projection.runtime_state = Some(TerminalRuntimeState::Exited);
        self.projection.status = TerminalPanelStatus {
            kind: TerminalPanelStatusKind::Exited,
            message: if killed {
                "Terminal killed".to_string()
            } else {
                "Terminal closed".to_string()
            },
        };
        self.projection.active_session_id = None;
        self.projection.generated_at = TimestampMillis::now();
        self.record_audit(
            session_id,
            TerminalRuntimeState::Exited,
            event_context,
            if killed {
                "action=kill state=exited escalation=terminate".to_string()
            } else {
                "action=close state=exited".to_string()
            },
        );
        self.projection()
    }

    fn deny(&mut self, denial: TerminalDenial) -> TerminalPanelProjection {
        self.projection.workspace_id = Some(denial.workspace_id);
        if denial.clear_active_session {
            self.projection.active_session_id = None;
        }
        self.projection.runtime_state = Some(TerminalRuntimeState::Denied);
        self.projection.status = TerminalPanelStatus {
            kind: TerminalPanelStatusKind::Denied,
            message: denial.reason.clone(),
        };
        self.projection.policy = Some(TerminalPolicyProjection {
            granted: false,
            reason: denial.reason.clone(),
            ..denial.policy
        });
        self.projection.last_denial = Some(denial.reason.clone());
        self.projection.generated_at = TimestampMillis::now();
        self.record_audit(
            denial
                .session_id
                .unwrap_or_else(|| self.audit_session_id(denial.workspace_id)),
            TerminalRuntimeState::Denied,
            denial.event_context,
            format!(
                "action={} state=denied reason={}",
                denial.action,
                bounded_label(denial.reason, 96)
            ),
        );
        self.projection()
    }

    fn fail(
        &mut self,
        session_id: TerminalSessionId,
        event_context: EventContext,
        reason: String,
    ) -> TerminalPanelProjection {
        self.projection.status = TerminalPanelStatus {
            kind: TerminalPanelStatusKind::Failed,
            message: reason.clone(),
        };
        self.projection.last_error = Some(reason);
        self.projection.runtime_state = Some(TerminalRuntimeState::Failed);
        self.projection.generated_at = TimestampMillis::now();
        self.record_audit(
            session_id,
            TerminalRuntimeState::Failed,
            event_context,
            format!(
                "action=terminal state=failed reason={}",
                bounded_label(
                    self.projection
                        .last_error
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    96
                )
            ),
        );
        self.projection()
    }

    fn capability_decision(
        &self,
        context: &ActiveWorkspaceContext,
        capability: &str,
        command_label: Option<&str>,
        event_context: EventContext,
    ) -> CapabilityDecision {
        let command_binary = command_label
            .and_then(|label| label.split_whitespace().next())
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        match self.security_broker.handle(CapabilityRequest::Request {
            principal_id: context.principal.clone(),
            capability_id: CapabilityId(capability.to_string()),
            workspace_trust_state: context.trust.clone(),
            target_path: None,
            decision_id: None,
            context: CapabilityRequestContext {
                command_binary,
                ..CapabilityRequestContext::default()
            },
            correlation_id: event_context.correlation_id,
        }) {
            Ok(CapabilityResponse::Decision(decision)) => decision,
            Ok(other) => CapabilityDecision {
                decision_id: CapabilityDecisionId(1),
                granted: false,
                capability: CapabilityId(capability.to_string()),
                reason: Some(format!(
                    "terminal policy returned unexpected response: {other:?}"
                )),
            },
            Err(error) => CapabilityDecision {
                decision_id: CapabilityDecisionId(1),
                granted: false,
                capability: CapabilityId(capability.to_string()),
                reason: Some(format!("terminal policy request failed: {error:?}")),
            },
        }
    }

    fn policy_projection(
        context: &ActiveWorkspaceContext,
        capability: &str,
        decision: &CapabilityDecision,
    ) -> TerminalPolicyProjection {
        TerminalPolicyProjection {
            capability_id: CapabilityId(capability.to_string()),
            workspace_trust_state: context.trust.clone(),
            granted: decision.granted,
            decision_id: Some(decision.decision_id),
            reason: decision
                .reason
                .clone()
                .unwrap_or_else(|| "policy decision recorded".to_string()),
            output_byte_limit: 256 * 1024,
            timeout_seconds: 30,
            schema_version: 1,
        }
    }

    fn record_audit(
        &mut self,
        session_id: TerminalSessionId,
        state: TerminalRuntimeState,
        event_context: EventContext,
        metadata_summary: String,
    ) {
        let event_sequence = self.next_event_sequence();
        self.last_audit = Some(devil_protocol::TerminalAuditRecord {
            session_id,
            state,
            event_sequence,
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            metadata_summary: bounded_label(metadata_summary, 160),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        });
    }

    fn next_event_sequence(&mut self) -> EventSequence {
        self.next_sequence = self.next_sequence.saturating_add(1).max(1);
        EventSequence(self.next_sequence)
    }

    fn audit_session_id(&self, workspace_id: WorkspaceId) -> TerminalSessionId {
        let fallback = u64::try_from(workspace_id.0).unwrap_or(u64::MAX).max(1);
        self.projection
            .active_session_id
            .unwrap_or(TerminalSessionId(fallback))
    }

    fn session_matches(&self, session_id: TerminalSessionId) -> bool {
        self.projection.active_session_id == Some(session_id)
    }

    fn push_terminal_output(
        &mut self,
        output: devil_protocol::TerminalOutputChunk,
        is_stderr: bool,
    ) {
        self.push_row(
            output.session_id,
            output.redacted_payload,
            output.byte_count,
            is_stderr,
        );
    }

    fn push_row(
        &mut self,
        session_id: TerminalSessionId,
        redacted_payload: String,
        byte_count: u64,
        is_stderr: bool,
    ) {
        let sequence = self.next_event_sequence();
        self.projection
            .output_rows
            .push(TerminalOutputRowProjection {
                session_id,
                sequence,
                redacted_payload: bounded_label(redacted_payload, 240),
                byte_count,
                is_stderr,
                truncated: byte_count > 240,
                redaction: RedactionHint::MetadataOnly,
                schema_version: 1,
            });
        let mut omitted = self.projection.scrollback.omitted_row_count;
        if self.projection.output_rows.len() > 100 {
            let excess = self.projection.output_rows.len() - 100;
            self.projection.output_rows.drain(0..excess);
            omitted = omitted.saturating_add(excess as u32);
        }
        self.projection.scrollback = TerminalScrollbackProjection {
            visible_row_count: self.projection.output_rows.len() as u32,
            omitted_row_count: omitted,
            byte_limit: 256 * 1024,
            truncated: omitted > 0,
            schema_version: 1,
        };
    }
}

fn language_id_for_path(path: &CanonicalPath) -> LanguageId {
    let lower = path.0.to_ascii_lowercase();
    let language = if lower.ends_with(".rs") {
        "rust"
    } else if lower.ends_with(".ts") || lower.ends_with(".tsx") {
        "typescript"
    } else if lower.ends_with(".js") || lower.ends_with(".jsx") {
        "javascript"
    } else if lower.ends_with(".md") {
        "markdown"
    } else if lower.ends_with(".json") {
        "json"
    } else {
        "text"
    };
    LanguageId(language.to_string())
}

fn bounded_label(value: impl Into<String>, limit: usize) -> String {
    value.into().chars().take(limit).collect()
}

fn identifier_byte_range_at(text: &str, requested_byte: u64) -> Option<ByteRange> {
    if text.is_empty() {
        return None;
    }

    let mut index = usize::try_from(requested_byte).unwrap_or(usize::MAX);
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    let bytes = text.as_bytes();
    if index == bytes.len() && index > 0 {
        index -= 1;
    }
    if index < bytes.len()
        && !is_identifier_byte(bytes[index])
        && index > 0
        && is_identifier_byte(bytes[index - 1])
    {
        index -= 1;
    }
    if index >= bytes.len() || !is_identifier_byte(bytes[index]) {
        return None;
    }

    let mut start = index;
    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = index + 1;
    while end < bytes.len() && is_identifier_byte(bytes[end]) {
        end += 1;
    }
    Some(ByteRange::new(start as u64, end as u64))
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

struct Phase4ContextAssemblyService;

impl Phase4ContextAssemblyService {
    #[allow(clippy::too_many_arguments)]
    fn assemble_context_manifest(
        context: &ActiveSaveContext,
        run_id: &devil_protocol::AgentRunId,
        provider_route_id: &str,
        snapshot_id: devil_protocol::SnapshotId,
        buffer_version: devil_protocol::BufferVersion,
        snapshot_hash: FileFingerprint,
        byte_len: u64,
        line_count: u32,
        generated_at: TimestampMillis,
    ) -> devil_protocol::ContextManifestProjection {
        let file_item = devil_protocol::ContextManifestItem {
            item_id: format!("phase4:{}:file", run_id.0),
            kind: devil_protocol::ContextManifestItemKind::File,
            inclusion: devil_protocol::ContextManifestInclusionState::Included,
            workspace_id: Some(context.workspace_id),
            file_id: Some(context.metadata.identity.file_id),
            buffer_id: Some(context.buffer_id),
            proposal_id: None,
            target_id: Some(context.metadata.identity.file_id.0.to_string()),
            path: Some(context.metadata.identity.canonical_path.clone()),
            ranges: Vec::new(),
            counts: context
                .metadata
                .file_length
                .map(|count| devil_protocol::ContextManifestItemCount {
                    label: "file_bytes".to_string(),
                    count: count.min(u32::MAX as u64) as u32,
                })
                .into_iter()
                .collect(),
            hashes: vec![context.metadata.fingerprint.clone()],
            privacy_scope: Some(devil_protocol::SemanticPrivacyScope::MetadataOnly),
            privacy_label: devil_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: devil_protocol::ProposalRiskLabel::Low,
            egress: devil_protocol::ContextManifestEgressStatus::LocalOnly,
            freshness: Some(devil_protocol::ContextManifestFreshnessSummary {
                state: devil_protocol::SemanticFreshnessState::Fresh,
                freshness_key_present: true,
                snapshot_id: Some(snapshot_id),
                file_content_version: Some(context.metadata.file_content_version),
                workspace_generation: Some(context.metadata.workspace_generation),
                content_hash: Some(context.metadata.fingerprint.clone()),
                privacy_scope: Some(devil_protocol::SemanticPrivacyScope::MetadataOnly),
                observed_at: Some(generated_at),
                risk_label: devil_protocol::ProposalRiskLabel::Low,
                risk_reasons: Vec::new(),
                schema_version: 1,
            }),
            preconditions: None,
            labels: vec!["phase4.context.file_metadata".to_string()],
            redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let buffer_item = devil_protocol::ContextManifestItem {
            item_id: format!("phase4:{}:buffer", run_id.0),
            kind: devil_protocol::ContextManifestItemKind::Buffer,
            inclusion: devil_protocol::ContextManifestInclusionState::Included,
            workspace_id: Some(context.workspace_id),
            file_id: Some(context.metadata.identity.file_id),
            buffer_id: Some(context.buffer_id),
            proposal_id: None,
            target_id: Some(context.buffer_id.0.to_string()),
            path: None,
            ranges: Vec::new(),
            counts: vec![
                devil_protocol::ContextManifestItemCount {
                    label: "snapshot_bytes".to_string(),
                    count: byte_len.min(u32::MAX as u64) as u32,
                },
                devil_protocol::ContextManifestItemCount {
                    label: "lines".to_string(),
                    count: line_count,
                },
            ],
            hashes: vec![snapshot_hash],
            privacy_scope: Some(devil_protocol::SemanticPrivacyScope::MetadataOnly),
            privacy_label: devil_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: devil_protocol::ProposalRiskLabel::Low,
            egress: devil_protocol::ContextManifestEgressStatus::LocalOnly,
            freshness: Some(devil_protocol::ContextManifestFreshnessSummary {
                state: devil_protocol::SemanticFreshnessState::Fresh,
                freshness_key_present: true,
                snapshot_id: Some(snapshot_id),
                file_content_version: Some(context.metadata.file_content_version),
                workspace_generation: Some(context.metadata.workspace_generation),
                content_hash: None,
                privacy_scope: Some(devil_protocol::SemanticPrivacyScope::MetadataOnly),
                observed_at: Some(generated_at),
                risk_label: devil_protocol::ProposalRiskLabel::Low,
                risk_reasons: Vec::new(),
                schema_version: 1,
            }),
            preconditions: Some(devil_protocol::ContextManifestPreconditionSummary {
                file_content_version: Some(context.metadata.file_content_version),
                buffer_version: Some(buffer_version),
                snapshot_id: Some(snapshot_id),
                workspace_generation: Some(context.metadata.workspace_generation),
                expected_fingerprint: Some(context.metadata.fingerprint.clone()),
                expected_file_length: context.metadata.file_length,
                expected_modified_at: context.metadata.modified_at,
                core_preconditions_present: true,
                risk_label: devil_protocol::ProposalRiskLabel::Low,
                risk_reasons: Vec::new(),
                schema_version: 1,
            }),
            labels: vec!["phase4.context.buffer_descriptor".to_string()],
            redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let route_item = Self::metadata_item(
            format!("phase4:{}:provider-route", run_id.0),
            devil_protocol::ContextManifestItemKind::ProviderRoute,
            context.workspace_id,
            provider_route_id,
            devil_protocol::ContextManifestEgressStatus::LocalProvider,
            vec!["phase4.provider.local_loopback".to_string()],
        );
        let agent_item = Self::metadata_item(
            format!("phase4:{}:agent-step", run_id.0),
            devil_protocol::ContextManifestItemKind::AgentStep,
            context.workspace_id,
            &run_id.0,
            devil_protocol::ContextManifestEgressStatus::LocalOnly,
            vec!["phase4.agent.proposal_only".to_string()],
        );
        let selection_item = Self::metadata_item(
            format!("phase4:{}:selection", run_id.0),
            devil_protocol::ContextManifestItemKind::UserSelection,
            context.workspace_id,
            "active-buffer",
            devil_protocol::ContextManifestEgressStatus::LocalOnly,
            vec!["phase4.selection.active_buffer".to_string()],
        );

        let permission = devil_protocol::ContextManifestPermissionSummary {
            kind: devil_protocol::ContextManifestPermissionKind::ModelProvider,
            capability: CapabilityId("ai.provider.invoke".to_string()),
            principal: Some(context.principal.clone()),
            decision_id: None,
            granted: false,
            privacy_scope: devil_protocol::SemanticPrivacyScope::MetadataOnly,
            egress: devil_protocol::ContextManifestEgressStatus::LocalProvider,
            risk_label: devil_protocol::ProposalRiskLabel::Low,
            redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let manifest = devil_protocol::ContextManifestRecord {
            manifest_id: format!("phase4:manifest:{}", run_id.0),
            workspace_id: Some(context.workspace_id),
            proposal_id: None,
            purpose: devil_protocol::ContextManifestPurpose::ProviderRequest,
            workspace_trust_state: Some(context.trust.clone()),
            privacy_label: devil_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: devil_protocol::ProposalRiskLabel::Low,
            egress: devil_protocol::ContextManifestEgressStatus::LocalProvider,
            items: vec![
                file_item,
                buffer_item,
                selection_item,
                route_item,
                agent_item,
            ],
            permissions: vec![permission],
            omitted_item_count: 0,
            stale_or_missing_metadata_risk_present: false,
            generated_at,
            redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        devil_protocol::ContextManifestProjection {
            manifest,
            selected_item_id: None,
            generated_at,
            redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn metadata_item(
        item_id: String,
        kind: devil_protocol::ContextManifestItemKind,
        workspace_id: WorkspaceId,
        target_id: &str,
        egress: devil_protocol::ContextManifestEgressStatus,
        labels: Vec<String>,
    ) -> devil_protocol::ContextManifestItem {
        devil_protocol::ContextManifestItem {
            item_id,
            kind,
            inclusion: devil_protocol::ContextManifestInclusionState::Included,
            workspace_id: Some(workspace_id),
            file_id: None,
            buffer_id: None,
            proposal_id: None,
            target_id: Some(target_id.to_string()),
            path: None,
            ranges: Vec::new(),
            counts: Vec::new(),
            hashes: Vec::new(),
            privacy_scope: Some(devil_protocol::SemanticPrivacyScope::MetadataOnly),
            privacy_label: devil_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: devil_protocol::ProposalRiskLabel::Low,
            egress,
            freshness: None,
            preconditions: None,
            labels,
            redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }
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
    TextEdit {
        workspace_id: WorkspaceId,
        file_id: FileId,
    },
    WorkspaceFile(WorkspaceMutationRollbackCheckpoint),
    Composite(Vec<ProposalMutationRollback>),
    Scoped {
        proposal: Box<WorkspaceProposal>,
        rollback: Box<ProposalMutationRollback>,
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
    /// Switch active tab.
    SwitchTab {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Request closing a tab.
    CloseTab {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Save every open buffer through app-owned save workflows.
    SaveAll,
    /// Set the primary cursor for a buffer.
    SetCursor {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor coordinate from UI projection space.
        cursor: TextCoordinate,
    },
    /// Set the primary selection for a buffer.
    SetSelection {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Selection range from UI projection space.
        range: devil_protocol::ProtocolTextRange,
    },
    /// Update viewport scroll state for a buffer.
    SetViewportScroll {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Scroll offsets.
        scroll: ViewportScroll,
    },
    /// Run bounded lexical search through app authority.
    RunSearch {
        /// App-generated query id.
        query_id: String,
        /// Search scope.
        scope: SearchScopeProjection,
        /// User query.
        query: String,
        /// Requested result limit; zero means app default.
        limit: usize,
    },
    /// Cancel the projected search by query id.
    CancelSearch {
        /// Query id to cancel.
        query_id: String,
    },
    /// Request hover data through app-owned language tooling.
    RequestHover {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position.
        position: TextCoordinate,
    },
    /// Request completion rows through app-owned language tooling.
    RequestCompletion {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position.
        position: TextCoordinate,
    },
    /// Request definition locations through app-owned language tooling.
    GoToDefinition {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position.
        position: TextCoordinate,
    },
    /// Request reference locations through app-owned language tooling.
    FindReferences {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position.
        position: TextCoordinate,
    },
    /// Refresh the active document outline through app-owned language tooling.
    RefreshOutline {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Request a formatting proposal preview through app-owned language tooling.
    RequestFormattingProposal {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Request a rename proposal preview through app-owned language tooling.
    RequestRenameProposal {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position.
        position: TextCoordinate,
        /// New symbol name label.
        new_name: String,
    },
    /// Request an organize-imports proposal preview through app-owned language tooling.
    RequestOrganizeImportsProposal {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Request a code-action proposal preview through app-owned language tooling.
    RequestCodeActionProposal {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Code action identifier.
        action_id: String,
    },
    /// Cancel an in-flight language operation.
    CancelLanguageOperation {
        /// Operation identifier.
        operation_id: String,
    },
    /// Launch a policy-gated terminal session.
    TerminalLaunch {
        /// Display-safe command label or fixture command.
        command_label: String,
    },
    /// Send input to an active terminal session.
    TerminalInput {
        /// Terminal session identifier.
        session_id: TerminalSessionId,
        /// Input payload.
        payload: String,
    },
    /// Resize an active terminal session.
    TerminalResize {
        /// Terminal session identifier.
        session_id: TerminalSessionId,
        /// Column count.
        cols: u16,
        /// Row count.
        rows: u16,
    },
    /// Kill an active terminal session.
    TerminalKill {
        /// Terminal session identifier.
        session_id: TerminalSessionId,
    },
    /// Close an active terminal session.
    TerminalClose {
        /// Terminal session identifier.
        session_id: TerminalSessionId,
    },
    /// Poll terminal output.
    TerminalOutputPoll {
        /// Terminal session identifier.
        session_id: TerminalSessionId,
    },
    /// Search terminal output projection.
    TerminalSearch {
        /// Terminal session identifier.
        session_id: TerminalSessionId,
        /// Query label.
        query: String,
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
    /// Start a Phase 4 AI run through app-owned composition.
    StartAiRun {
        /// Display-safe instruction label.
        instruction_label: String,
    },
    /// Start a metadata-only assisted-AI explain run through app-owned composition.
    StartAiExplain {
        /// Display-safe instruction label.
        instruction_label: String,
    },
    /// Start a proposal-only assisted-AI edit run through app-owned composition.
    StartAiProposal {
        /// Display-safe instruction label.
        instruction_label: String,
    },
    /// Cancel a Phase 4 AI run through app-owned composition.
    CancelAiRun {
        /// Run id to cancel.
        run_id: devil_protocol::AgentRunId,
    },
    /// Replay a Phase 4 AI run through metadata storage.
    ReplayAiRun {
        /// Run id to replay.
        run_id: devil_protocol::AgentRunId,
    },
    /// Inspect a Phase 4 AI run through projection metadata.
    InspectAiRun {
        /// Run id to inspect.
        run_id: devil_protocol::AgentRunId,
    },
    /// Invoke a Phase 5 plugin command through app-owned plugin composition.
    InvokePluginCommand {
        /// Plugin id selected from UI projection data.
        plugin_id: PluginId,
        /// Plugin command id selected from UI projection data.
        command_id: String,
        /// Metadata-only label for audit and bounded output.
        metadata_label: String,
    },
    /// Join a collaboration session through app-owned composition.
    JoinCollaborationSession {
        /// Session identifier selected from projection data.
        session_id: CollaborationSessionId,
    },
    /// Leave a collaboration session through app-owned composition.
    LeaveCollaborationSession {
        /// Session identifier selected from projection data.
        session_id: CollaborationSessionId,
    },
    /// Publish metadata-only collaboration presence through app-owned composition.
    PublishCollaborationPresence {
        /// Session identifier selected from projection data.
        session_id: CollaborationSessionId,
        /// Participant identifier selected from projection data.
        participant_id: CollaborationParticipantId,
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
            AppCommandRequest::Save { .. }
            | AppCommandRequest::SwitchTab { .. }
            | AppCommandRequest::CloseTab { .. }
            | AppCommandRequest::SaveAll
            | AppCommandRequest::SetCursor { .. }
            | AppCommandRequest::SetSelection { .. }
            | AppCommandRequest::SetViewportScroll { .. }
            | AppCommandRequest::RunSearch { .. }
            | AppCommandRequest::CancelSearch { .. }
            | AppCommandRequest::RequestHover { .. }
            | AppCommandRequest::RequestCompletion { .. }
            | AppCommandRequest::GoToDefinition { .. }
            | AppCommandRequest::FindReferences { .. }
            | AppCommandRequest::RefreshOutline { .. }
            | AppCommandRequest::RequestFormattingProposal { .. }
            | AppCommandRequest::RequestRenameProposal { .. }
            | AppCommandRequest::RequestOrganizeImportsProposal { .. }
            | AppCommandRequest::RequestCodeActionProposal { .. }
            | AppCommandRequest::CancelLanguageOperation { .. }
            | AppCommandRequest::TerminalLaunch { .. }
            | AppCommandRequest::TerminalInput { .. }
            | AppCommandRequest::TerminalResize { .. }
            | AppCommandRequest::TerminalKill { .. }
            | AppCommandRequest::TerminalClose { .. }
            | AppCommandRequest::TerminalOutputPoll { .. }
            | AppCommandRequest::TerminalSearch { .. }
            | AppCommandRequest::OpenPath { .. }
            | AppCommandRequest::StartAiRun { .. }
            | AppCommandRequest::StartAiExplain { .. }
            | AppCommandRequest::StartAiProposal { .. }
            | AppCommandRequest::CancelAiRun { .. }
            | AppCommandRequest::ReplayAiRun { .. }
            | AppCommandRequest::InspectAiRun { .. }
            | AppCommandRequest::InvokePluginCommand { .. }
            | AppCommandRequest::JoinCollaborationSession { .. }
            | AppCommandRequest::LeaveCollaborationSession { .. }
            | AppCommandRequest::PublishCollaborationPresence { .. } => Ok(None),
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
            CommandDispatchIntent::SwitchTab { buffer_id } => {
                Ok(AppCommandRequest::SwitchTab { buffer_id })
            }
            CommandDispatchIntent::CloseTab { buffer_id } => {
                Ok(AppCommandRequest::CloseTab { buffer_id })
            }
            CommandDispatchIntent::SaveAll => Ok(AppCommandRequest::SaveAll),
            CommandDispatchIntent::SetCursor { buffer_id, cursor } => {
                Ok(AppCommandRequest::SetCursor { buffer_id, cursor })
            }
            CommandDispatchIntent::SetSelection { buffer_id, range } => {
                Ok(AppCommandRequest::SetSelection { buffer_id, range })
            }
            CommandDispatchIntent::SetViewportScroll { buffer_id, scroll } => {
                Ok(AppCommandRequest::SetViewportScroll { buffer_id, scroll })
            }
            CommandDispatchIntent::RunSearch {
                scope,
                query,
                limit,
            } => Ok(AppCommandRequest::RunSearch {
                query_id: format!("search:{}", correlation_id.0),
                scope,
                query,
                limit,
            }),
            CommandDispatchIntent::CancelSearch { query_id } => {
                Ok(AppCommandRequest::CancelSearch { query_id })
            }
            CommandDispatchIntent::RequestHover {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestHover {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::RequestCompletion {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestCompletion {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::GoToDefinition {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::GoToDefinition {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::FindReferences {
                buffer_id,
                position,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::FindReferences {
                    buffer_id,
                    position,
                })
            }
            CommandDispatchIntent::RefreshOutline { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RefreshOutline { buffer_id })
            }
            CommandDispatchIntent::RequestFormattingProposal { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestFormattingProposal { buffer_id })
            }
            CommandDispatchIntent::RequestRenameProposal {
                buffer_id,
                position,
                new_name,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestRenameProposal {
                    buffer_id,
                    position,
                    new_name,
                })
            }
            CommandDispatchIntent::RequestOrganizeImportsProposal { buffer_id } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestOrganizeImportsProposal { buffer_id })
            }
            CommandDispatchIntent::RequestCodeActionProposal {
                buffer_id,
                action_id,
            } => {
                Self::ensure_active_buffer(active.buffer_id, buffer_id)?;
                Ok(AppCommandRequest::RequestCodeActionProposal {
                    buffer_id,
                    action_id,
                })
            }
            CommandDispatchIntent::CancelLanguageOperation { operation_id } => {
                Ok(AppCommandRequest::CancelLanguageOperation { operation_id })
            }
            CommandDispatchIntent::TerminalLaunch { command_label } => {
                Ok(AppCommandRequest::TerminalLaunch { command_label })
            }
            CommandDispatchIntent::TerminalInput {
                session_id,
                payload,
            } => Ok(AppCommandRequest::TerminalInput {
                session_id,
                payload,
            }),
            CommandDispatchIntent::TerminalResize {
                session_id,
                cols,
                rows,
            } => Ok(AppCommandRequest::TerminalResize {
                session_id,
                cols,
                rows,
            }),
            CommandDispatchIntent::TerminalKill { session_id } => {
                Ok(AppCommandRequest::TerminalKill { session_id })
            }
            CommandDispatchIntent::TerminalClose { session_id } => {
                Ok(AppCommandRequest::TerminalClose { session_id })
            }
            CommandDispatchIntent::TerminalOutputPoll { session_id } => {
                Ok(AppCommandRequest::TerminalOutputPoll { session_id })
            }
            CommandDispatchIntent::TerminalSearch { session_id, query } => {
                Ok(AppCommandRequest::TerminalSearch { session_id, query })
            }
            CommandDispatchIntent::OpenPath { path } => Ok(AppCommandRequest::OpenPath { path }),
            CommandDispatchIntent::RefreshExplorer => Ok(AppCommandRequest::RefreshExplorer),
            CommandDispatchIntent::RevealInExplorer { file_id } => {
                Ok(AppCommandRequest::RevealInExplorer { file_id })
            }
            CommandDispatchIntent::StartAiRun { instruction_label } => {
                Ok(AppCommandRequest::StartAiRun { instruction_label })
            }
            CommandDispatchIntent::StartAiExplain { instruction_label } => {
                Ok(AppCommandRequest::StartAiExplain { instruction_label })
            }
            CommandDispatchIntent::StartAiProposal { instruction_label } => {
                Ok(AppCommandRequest::StartAiProposal { instruction_label })
            }
            CommandDispatchIntent::CancelAiRun { run_id } => {
                Ok(AppCommandRequest::CancelAiRun { run_id })
            }
            CommandDispatchIntent::ReplayAiRun { run_id } => {
                Ok(AppCommandRequest::ReplayAiRun { run_id })
            }
            CommandDispatchIntent::InspectAiRun { run_id } => {
                Ok(AppCommandRequest::InspectAiRun { run_id })
            }
            CommandDispatchIntent::InvokePluginCommand {
                plugin_id,
                command_id,
                metadata_label,
            } => Ok(AppCommandRequest::InvokePluginCommand {
                plugin_id,
                command_id,
                metadata_label,
            }),
            CommandDispatchIntent::JoinCollaborationSession { session_id } => {
                Ok(AppCommandRequest::JoinCollaborationSession { session_id })
            }
            CommandDispatchIntent::LeaveCollaborationSession { session_id } => {
                Ok(AppCommandRequest::LeaveCollaborationSession { session_id })
            }
            CommandDispatchIntent::PublishCollaborationPresence {
                session_id,
                participant_id,
            } => Ok(AppCommandRequest::PublishCollaborationPresence {
                session_id,
                participant_id,
            }),
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

        let request = devil_protocol::EditorViewportRequest {
            buffer_id,
            scroll: active.viewport_scroll_for(buffer_id),
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

    fn daily_editing_projection(
        active: &ActiveDocumentController,
        editor: &EditorEngine,
    ) -> DailyEditingProjection {
        let tabs = active
            .open_tabs
            .iter()
            .filter_map(|buffer_id| {
                let metadata = active.metadata_for_buffer(*buffer_id)?;
                let dirty = editor
                    .is_dirty(*buffer_id)
                    .unwrap_or_else(|_| editor.buffer_metadata(*buffer_id).is_ok_and(|m| m.dirty));
                Some(EditorTabProjection {
                    buffer_id: *buffer_id,
                    file_id: Some(metadata.identity.file_id),
                    file_path: Some(metadata.identity.canonical_path.clone()),
                    title: tab_title(&metadata.identity.canonical_path),
                    active: active.active_buffer_id == Some(*buffer_id),
                    dirty,
                    pinned: false,
                    preview: false,
                })
            })
            .collect::<Vec<_>>();

        let viewport_states = active
            .open_tabs
            .iter()
            .map(|buffer_id| {
                let scroll = active.viewport_scroll_for(*buffer_id);
                let viewport = editor
                    .viewport_projection(devil_protocol::EditorViewportRequest {
                        buffer_id: *buffer_id,
                        scroll,
                        dimensions: devil_protocol::ViewportDimensions {
                            width_px: 800,
                            height_px: 384,
                        },
                    })
                    .ok();
                EditorViewportStateProjection {
                    buffer_id: *buffer_id,
                    scroll,
                    cursor: viewport.as_ref().map(|projection| projection.cursor),
                    selections: viewport
                        .as_ref()
                        .map(|projection| projection.selections.clone())
                        .unwrap_or_default(),
                }
            })
            .collect();

        DailyEditingProjection {
            tabs: EditorTabsProjection {
                tabs,
                active_buffer_id: active.active_buffer_id,
            },
            close_dirty_prompt: active.close_dirty_prompt.as_ref().map(|prompt| {
                CloseDirtyPromptProjection {
                    buffer_id: prompt.buffer_id,
                    file_id: prompt.file_id,
                    file_path: prompt.path.clone(),
                    title: prompt.title.clone(),
                    message: format!("Save changes to {} before closing?", prompt.title),
                }
            }),
            viewport_states,
            session_record: capture_workspace_session_record(active, editor)
                .ok()
                .map(|record| WorkspaceSessionRecordProjection {
                    session_id: record.session_id,
                    last_workspace: record.last_workspace,
                    open_tab_count: record.open_tabs.len(),
                    active_buffer: record.active_buffer,
                    saved_at: record.saved_at,
                    schema_version: record.schema_version,
                }),
        }
    }
}

fn tab_title(path: &CanonicalPath) -> String {
    Path::new(&path.0)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(path.0.as_str())
        .to_string()
}

fn tab_id_for_buffer(buffer_id: BufferId) -> String {
    format!("buffer:{}", buffer_id.0)
}

fn capture_workspace_session_record(
    active: &ActiveDocumentController,
    editor: &EditorEngine,
) -> Result<WorkspaceSessionRecord, AppCompositionError> {
    let open_tabs = active
        .open_tabs
        .iter()
        .filter_map(|buffer_id| {
            let metadata = active.metadata_for_buffer(*buffer_id)?;
            let dirty = editor.is_dirty(*buffer_id).unwrap_or(false);
            Some(SessionTab {
                tab_id: tab_id_for_buffer(*buffer_id),
                buffer_id: Some(*buffer_id),
                file_id: Some(metadata.identity.file_id),
                path: Some(metadata.identity.canonical_path.clone()),
                title: tab_title(&metadata.identity.canonical_path),
                pinned: false,
                preview: false,
                dirty,
            })
        })
        .collect::<Vec<_>>();
    let tab_ids = open_tabs
        .iter()
        .map(|tab| tab.tab_id.clone())
        .collect::<Vec<_>>();
    let active_tab = active.active_buffer_id.map(tab_id_for_buffer);
    let dirty_indicators = active
        .open_tabs
        .iter()
        .filter_map(|buffer_id| {
            let metadata = active.metadata_for_buffer(*buffer_id)?;
            let editor_metadata = editor.buffer_metadata(*buffer_id).ok()?;
            Some(SessionDirtyIndicator {
                buffer_id: *buffer_id,
                file_id: Some(metadata.identity.file_id),
                dirty: editor_metadata.dirty,
                buffer_version: editor_metadata.buffer_version,
            })
        })
        .collect();

    Ok(WorkspaceSessionRecord {
        session_id: active
            .workspace_id()
            .map(|workspace_id| format!("workspace-session:{}", workspace_id.0))
            .unwrap_or_else(|| "workspace-session:empty".to_string()),
        last_workspace: active.workspace_id(),
        last_workspace_path: active
            .workspace_root_path
            .as_ref()
            .map(|path| CanonicalPath(path.clone())),
        open_tabs,
        active_tab: active_tab.clone(),
        active_buffer: active.active_buffer_id,
        tab_groups: vec![SessionTabGroup {
            group_id: "main".to_string(),
            tab_ids,
            active_tab_id: active_tab,
        }],
        layout_splits: Vec::new(),
        explorer_expansion: Vec::new(),
        panel_state: SessionPanelState {
            bottom_visible: false,
            side_visible: true,
            active_panel: None,
            bottom_height_px: None,
            side_width_px: None,
        },
        dirty_indicators,
        saved_at: TimestampMillis::now(),
        schema_version: 1,
    })
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
                return Err(Self::record_audit_storage_failed_response(
                    proposal_coordinator,
                    proposal,
                    response,
                    error,
                ));
            }
            if let Err(error) =
                storage.handle(StorageRepositoryRequest::SaveEventMetadata(metadata))
                && audit_required
            {
                return Err(Self::record_audit_storage_failed_response(
                    proposal_coordinator,
                    proposal,
                    response,
                    error,
                ));
            }
        }

        if let Some(transition) = Self::transition_for_response(response) {
            let audit = proposal_audit_record(proposal, transition);
            if let Err(error) =
                storage.handle(StorageRepositoryRequest::SaveProposalAuditRecord(audit))
                && audit_required
            {
                return Err(Self::record_audit_storage_failed_response(
                    proposal_coordinator,
                    proposal,
                    response,
                    error,
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
                    return Err(Self::record_audit_storage_failed_response(
                        proposal_coordinator,
                        proposal,
                        response,
                        error,
                    ));
                }
                if let Err(error) =
                    storage.handle(StorageRepositoryRequest::SaveEventMetadata(metadata))
                {
                    return Err(Self::record_audit_storage_failed_response(
                        proposal_coordinator,
                        proposal,
                        response,
                        error,
                    ));
                }
            }
            proposal_coordinator.record_observed_transition(transition);
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

    fn record_audit_storage_failed_response(
        proposal_coordinator: &mut AppProposalCoordinator,
        proposal: &WorkspaceProposal,
        response: &ProposalResponse,
        error: ProtocolError,
    ) -> ProposalResponse {
        let failure = Self::audit_storage_failed_response(proposal, response, error);
        if let Some(transition) = Self::transition_for_response(&failure) {
            proposal_coordinator.record_audit_failure_transition(transition);
        }
        failure
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

fn empty_context_manifest_projection() -> devil_protocol::ContextManifestProjection {
    devil_protocol::ContextManifestProjection {
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
    }
}

fn empty_privacy_inspector_projection() -> devil_protocol::PrivacyInspectorProjection {
    devil_protocol::PrivacyInspectorProjection {
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
    }
}

fn empty_permission_budget_projection() -> devil_protocol::PermissionBudgetProjection {
    devil_protocol::PermissionBudgetProjection {
        projection_id: "permission-budgets:empty".to_string(),
        budgets: Vec::new(),
        evaluations: Vec::new(),
        denied_budget_count: 0,
        depleted_budget_count: 0,
        refused_evaluation_count: 0,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![devil_protocol::RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_approval_checklist_projection() -> devil_protocol::ProposalApprovalChecklistProjection {
    devil_protocol::ProposalApprovalChecklistProjection {
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
    }
}

fn empty_checkpoint_rollback_projection() -> devil_protocol::CheckpointRollbackProjection {
    devil_protocol::CheckpointRollbackProjection {
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
    }
}

fn empty_assisted_ai_projection() -> devil_protocol::AssistedAiProjection {
    devil_protocol::AssistedAiProjection {
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
    }
}

#[derive(Debug, Clone)]
struct SelectedProposalTrustProjections {
    context_manifest_projection: devil_protocol::ContextManifestProjection,
    privacy_inspector_projection: devil_protocol::PrivacyInspectorProjection,
    permission_budget_projection: devil_protocol::PermissionBudgetProjection,
    approval_checklist_projection: devil_protocol::ProposalApprovalChecklistProjection,
    checkpoint_rollback_projection: devil_protocol::CheckpointRollbackProjection,
}

fn metadata_fingerprint(algorithm: &str, value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: algorithm.to_string(),
        value: value.to_string(),
    }
}

fn trust_reference(
    reference_id: &str,
    kind: devil_protocol::AssistedAiTrustProjectionKind,
) -> devil_protocol::AssistedAiTrustProjectionReference {
    devil_protocol::AssistedAiTrustProjectionReference {
        reference_id: reference_id.to_string(),
        kind,
        projection_hash: metadata_fingerprint("projection-id", reference_id),
        schema_version: 1,
    }
}

fn phase4_provider_capability(
    provider_class: devil_protocol::AssistedAiProviderClass,
    refusal: Option<devil_protocol::AssistedAiRefusalMetadata>,
) -> devil_protocol::AssistedAiProviderCapability {
    devil_protocol::AssistedAiProviderCapability {
        provider_id: DETERMINISTIC_LOCAL_PROVIDER_ID.to_string(),
        provider_label: "Deterministic local provider".to_string(),
        provider_class,
        supported_operations: vec![
            devil_protocol::AssistedAiOperationClass::Explain,
            devil_protocol::AssistedAiOperationClass::ProposeEdit,
        ],
        model_capability_labels: vec!["deterministic".to_string()],
        tool_capability_labels: Vec::new(),
        context_window_label: "small".to_string(),
        cost_budget_label: "local.free".to_string(),
        risk_budget_label: "low".to_string(),
        privacy_retention_label: "metadata-only".to_string(),
        byok_support: devil_protocol::AssistedAiSupportLabel::Unsupported,
        local_execution_support: devil_protocol::AssistedAiSupportLabel::Supported,
        offline_support: devil_protocol::AssistedAiSupportLabel::Supported,
        air_gap_support: devil_protocol::AssistedAiSupportLabel::Supported,
        redaction_requirements: vec!["metadata-only".to_string()],
        consent_requirements: vec!["proposal-review".to_string()],
        availability: if refusal.is_some() {
            devil_protocol::AssistedAiProviderAvailabilityState::Refused
        } else {
            devil_protocol::AssistedAiProviderAvailabilityState::Available
        },
        refusal,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn phase4_permission_budget_projection(
    context_manifest: &devil_protocol::ContextManifestProjection,
    run_id: &devil_protocol::AgentRunId,
    generated_at: TimestampMillis,
) -> devil_protocol::PermissionBudgetProjection {
    let budget = devil_protocol::PermissionBudgetContract {
        budget_id: format!("phase4:budget:{}", run_id.0),
        action_class: devil_protocol::PermissionBudgetActionClass::InvokeProvider,
        capability: Some(CapabilityId("ai.provider.invoke".to_string())),
        state: devil_protocol::PermissionBudgetState::Allowed,
        privacy_scope: devil_protocol::SemanticPrivacyScope::MetadataOnly,
        usage: devil_protocol::PermissionBudgetUsageSummary {
            unit_label: "calls".to_string(),
            used: 0,
            ceiling: Some(1),
            remaining: Some(1),
            attempted: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        reset_policy_label: devil_protocol::PermissionBudgetResetPolicyLabel::Session,
        consent_requirement_label:
            devil_protocol::PermissionBudgetConsentRequirementLabel::NotRequired,
        risk_label: devil_protocol::ProposalRiskLabel::Low,
        reasons: vec!["phase4.local_provider.budget_allowed".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let action = devil_protocol::permission_budget_action_from_permission_summary(
        &context_manifest.manifest.permissions[0],
        format!("phase4:budget-action:{}", run_id.0),
        devil_protocol::PermissionBudgetActionClass::InvokeProvider,
        context_manifest.manifest.workspace_id,
        context_manifest.manifest.proposal_id,
        1,
    );
    let evaluation = devil_protocol::evaluate_permission_budget(
        &budget,
        action,
        format!("phase4:budget-eval:{}", run_id.0),
        1,
    );
    devil_protocol::permission_budget_projection_from_contracts(
        format!("phase4:permission-budget:{}", run_id.0),
        vec![budget],
        vec![evaluation],
        generated_at,
        1,
    )
}

#[allow(clippy::too_many_arguments)]
fn assisted_ai_request_contract_from_metadata(
    request_id: String,
    provider_capability: &devil_protocol::AssistedAiProviderCapability,
    operation_class: devil_protocol::AssistedAiOperationClass,
    context_manifest_projection: &devil_protocol::ContextManifestProjection,
    privacy_inspector_projection: &devil_protocol::PrivacyInspectorProjection,
    permission_budget_projection: &devil_protocol::PermissionBudgetProjection,
    approval_checklist_projection: &devil_protocol::ProposalApprovalChecklistProjection,
    checkpoint_rollback_projection: Option<&devil_protocol::CheckpointRollbackProjection>,
    event_context: EventContext,
    proposal_intent: devil_protocol::AssistedAiProposalTargetIntent,
    route_decision: devil_protocol::AssistedAiRouteDecision,
    generated_at: TimestampMillis,
) -> devil_protocol::AssistedAiRequestContract {
    devil_protocol::AssistedAiRequestContract {
        request_id,
        provider: provider_capability.clone(),
        operation_class,
        context_manifest: trust_reference(
            &context_manifest_projection.manifest.manifest_id,
            devil_protocol::AssistedAiTrustProjectionKind::ContextManifest,
        ),
        privacy_inspector: trust_reference(
            &privacy_inspector_projection.inspector_id,
            devil_protocol::AssistedAiTrustProjectionKind::PrivacyInspector,
        ),
        permission_budget_projection: trust_reference(
            &permission_budget_projection.projection_id,
            devil_protocol::AssistedAiTrustProjectionKind::PermissionBudget,
        ),
        permission_budget_evaluations: permission_budget_projection
            .evaluations
            .iter()
            .map(|evaluation| {
                devil_protocol::AssistedAiPermissionBudgetEvaluationReference::from_evaluation(
                    evaluation,
                    metadata_fingerprint("permission-budget-evaluation", &evaluation.evaluation_id),
                    1,
                )
            })
            .collect(),
        approval_checklist: trust_reference(
            &approval_checklist_projection.checklist_id,
            devil_protocol::AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
        ),
        checkpoint_rollback: checkpoint_rollback_projection.map(|projection| {
            trust_reference(
                &projection.projection_id,
                devil_protocol::AssistedAiTrustProjectionKind::CheckpointRollback,
            )
        }),
        correlation_id: event_context.correlation_id,
        causality_id: event_context.causality_id,
        proposal_intent,
        route_decision,
        created_at: generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
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
    /// All open buffers were saved or rejected through app authority.
    SaveAll(AppSaveAllOutcome),
    /// Active tab changed.
    TabSwitched(BufferId),
    /// Tab close request completed or produced a dirty prompt.
    TabClose(AppCloseTabOutcome),
    /// Cursor update completed.
    CursorSet(BufferId),
    /// Selection update completed.
    SelectionSet(BufferId),
    /// Viewport scroll update completed.
    ViewportScrollSet(BufferId),
    /// Search projection changed.
    SearchUpdated(SearchProjection),
    /// Language tooling projection changed.
    LanguageToolingUpdated(LanguageToolingProjection),
    /// Terminal panel projection changed.
    TerminalPanelUpdated(TerminalPanelProjection),
    /// Explorer projection was refreshed from workspace tree state.
    ExplorerRefreshed(ExplorerProjection),
    /// A workspace path was opened and bound to an editor buffer.
    Opened(FileId),
    /// Proposal lifecycle request completed through app-owned proposal authority.
    ProposalLifecycleUpdated(ProposalResponse),
    /// Proposal detail projection selection changed.
    ProposalDetailsOpened(ProposalId),
    /// Phase 4 AI run started and produced a proposal-only output.
    AiRunStarted(Box<AppAiRunOutcome>),
    /// Phase 4 AI run was cancelled through app-owned metadata.
    AiRunCancelled(devil_protocol::AgentRunId),
    /// Phase 4 AI run replay metadata was loaded.
    AiRunReplayed(Box<devil_protocol::AgentReplayManifest>),
    /// Phase 4 AI run inspectability snapshot was loaded.
    AiRunInspected(Box<AppAiInspectionSnapshot>),
    /// Phase 5 plugin command was invoked through app-owned plugin composition.
    PluginCommandInvoked(Box<PluginHostCallResponse>),
    /// Collaboration session was joined through app-owned composition.
    CollaborationSessionJoined(CollaborationSessionId),
    /// Collaboration session was left through app-owned composition.
    CollaborationSessionLeft(CollaborationSessionId),
    /// Metadata-only collaboration presence was published.
    CollaborationPresencePublished(CollaborationSessionId),
    /// Collaboration transport operation was accepted and applied through editor authority.
    CollaborationOperationApplied(TextTransactionDescriptor),
}

/// Per-buffer save-all result.
#[derive(Debug, Clone)]
pub struct AppSaveAllItemOutcome {
    /// Buffer that was saved or rejected.
    pub buffer_id: BufferId,
    /// File associated with the buffer, when known.
    pub file_id: Option<FileId>,
    /// Canonical file path associated with the buffer, when known.
    pub file_path: Option<CanonicalPath>,
    /// Explicit save-all item status.
    pub status: AppSaveAllItemStatus,
    /// Save outcome for this buffer when a proposal-mediated save was attempted.
    pub outcome: Option<AppSaveOutcome>,
    /// Proposal or metadata failure summary for rejected items.
    pub rejection_metadata: Option<AppSaveAllRejectionMetadata>,
    /// Dirty state after this save-all item completed.
    pub final_dirty: bool,
}

/// Explicit per-buffer save-all item status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSaveAllItemStatus {
    /// Buffer was saved through the proposal-mediated save workflow.
    Saved,
    /// Save was rejected and dirty text was preserved.
    Rejected,
    /// Buffer lacked the metadata required for a safe proposal-mediated save.
    MetadataMissing,
}

/// Proposal response summary attached to a save-all rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSaveAllRejectionMetadata {
    /// Proposal response or app metadata failure kind.
    pub response_kind: String,
    /// Proposal id from the response transition, when one exists.
    pub proposal_id: Option<ProposalId>,
    /// Diagnostic codes carried by the response.
    pub diagnostic_codes: Vec<String>,
    /// Diagnostic messages carried by the response or synthesized by app metadata validation.
    pub diagnostic_messages: Vec<String>,
}

/// Aggregate save-all result.
#[derive(Debug, Clone)]
pub struct AppSaveAllOutcome {
    /// Aggregate save-all status.
    pub status: AppSaveAllStatus,
    /// Per-buffer results in tab order.
    pub results: Vec<AppSaveAllItemOutcome>,
    /// Number of successfully saved buffers.
    pub saved_count: usize,
    /// Number of rejected saves that preserved dirty buffers.
    pub rejected_count: usize,
}

/// Aggregate save-all status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSaveAllStatus {
    /// No open buffers were available to save.
    Noop,
    /// Every attempted buffer saved successfully.
    Saved,
    /// Some buffers saved and some were rejected.
    Partial,
    /// No buffers saved successfully and at least one buffer was rejected.
    Rejected,
}

fn save_all_missing_metadata(buffer_id: BufferId) -> AppSaveAllRejectionMetadata {
    AppSaveAllRejectionMetadata {
        response_kind: "MetadataMissing".to_string(),
        proposal_id: None,
        diagnostic_codes: vec!["save_all.metadata_missing".to_string()],
        diagnostic_messages: vec![format!(
            "buffer {} is open without the metadata required for a safe save",
            buffer_id.0
        )],
    }
}

fn save_all_rejection_metadata(response: &ProposalResponse) -> AppSaveAllRejectionMetadata {
    let (response_kind, transition) = match response {
        ProposalResponse::Created(transition) => ("Created", transition),
        ProposalResponse::Validated(transition) => ("Validated", transition),
        ProposalResponse::Previewed { transition, .. } => ("Previewed", transition),
        ProposalResponse::Approved(transition) => ("Approved", transition),
        ProposalResponse::Rejected { transition, .. } => ("Rejected", transition),
        ProposalResponse::Applied(transition) => ("Applied", transition),
        ProposalResponse::Denied { transition, .. } => ("Denied", transition),
        ProposalResponse::Failed { transition, .. } => ("Failed", transition),
        ProposalResponse::RolledBack { transition, .. } => ("RolledBack", transition),
        ProposalResponse::Stale { transition, .. } => ("Stale", transition),
        ProposalResponse::Conflict { transition, .. } => ("Conflict", transition),
        ProposalResponse::Cancelled { transition, .. } => ("Cancelled", transition),
    };
    AppSaveAllRejectionMetadata {
        response_kind: response_kind.to_string(),
        proposal_id: Some(transition.proposal_id),
        diagnostic_codes: transition
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.clone())
            .collect(),
        diagnostic_messages: transition
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
    }
}

/// App-owned tab-close outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCloseTabOutcome {
    /// Clean tab was closed.
    Closed {
        /// Closed buffer.
        buffer_id: BufferId,
    },
    /// Dirty tab was kept open and projected as a prompt.
    CloseDirtyPrompt {
        /// Dirty buffer.
        buffer_id: BufferId,
        /// File associated with the dirty buffer.
        file_id: Option<FileId>,
        /// Display path associated with the dirty buffer.
        path: Option<CanonicalPath>,
    },
}

/// One tab skipped during metadata-only session restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSessionRestoreSkippedTab {
    /// Persisted session tab id.
    pub tab_id: String,
    /// Human-readable skip reason.
    pub reason: String,
}

/// Metadata-only session restore result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSessionRestoreOutcome {
    /// Restored workspace file ids.
    pub restored_file_ids: Vec<FileId>,
    /// Tabs skipped because their metadata could not be opened.
    pub skipped_tabs: Vec<AppSessionRestoreSkippedTab>,
}

/// App-owned result for a Phase 4 AI run.
#[derive(Debug, Clone)]
pub struct AppAiRunOutcome {
    /// Agent run identifier.
    pub run_id: devil_protocol::AgentRunId,
    /// Proposal id generated by mutation-capable runs.
    pub proposal_id: Option<ProposalId>,
    /// Created lifecycle response for the generated proposal, when present.
    pub proposal_created: Option<ProposalResponse>,
    /// Provider route response metadata.
    pub route_response: devil_protocol::AssistedAiProviderRouteResponse,
    /// Context manifest projection used before provider invocation.
    pub context_manifest_projection: devil_protocol::ContextManifestProjection,
    /// Privacy inspector projection derived from the manifest.
    pub privacy_inspector_projection: devil_protocol::PrivacyInspectorProjection,
    /// Permission budget projection evaluated before provider invocation.
    pub permission_budget_projection: devil_protocol::PermissionBudgetProjection,
    /// Visible refusal metadata for denied or unavailable routes.
    pub refusal: Option<devil_protocol::AssistedAiRefusalMetadata>,
    /// Replay manifest persisted for the run.
    pub replay_manifest: devil_protocol::AgentReplayManifest,
}

/// App-owned inspectability snapshot for a Phase 4 AI run.
#[derive(Debug, Clone)]
pub struct AppAiInspectionSnapshot {
    /// Agent run identifier.
    pub run_id: devil_protocol::AgentRunId,
    /// Run-specific context manifest projection.
    pub context_manifest_projection: devil_protocol::ContextManifestProjection,
    /// Run-specific privacy inspector projection.
    pub privacy_inspector_projection: devil_protocol::PrivacyInspectorProjection,
    /// Run-specific permission budget projection.
    pub permission_budget_projection: devil_protocol::PermissionBudgetProjection,
    /// Run-specific assisted-AI projection.
    pub assisted_ai_projection: devil_protocol::AssistedAiProjection,
}

#[derive(Debug, Clone, Default)]
struct Phase4ProjectionState {
    context_manifest_projection: Option<devil_protocol::ContextManifestProjection>,
    privacy_inspector_projection: Option<devil_protocol::PrivacyInspectorProjection>,
    permission_budget_projection: Option<devil_protocol::PermissionBudgetProjection>,
    approval_checklist_projection: Option<devil_protocol::ProposalApprovalChecklistProjection>,
    checkpoint_rollback_projection: Option<devil_protocol::CheckpointRollbackProjection>,
    assisted_ai_projection: Option<devil_protocol::AssistedAiProjection>,
    replay_manifests: HashMap<devil_protocol::AgentRunId, devil_protocol::AgentReplayManifest>,
    inspection_snapshots: HashMap<devil_protocol::AgentRunId, AppAiInspectionSnapshot>,
}

#[derive(Debug, Default)]
struct SearchBuildResult {
    results: Vec<SearchResultProjection>,
    omitted_result_count: usize,
    omitted_file_count: usize,
    diagnostics: Vec<String>,
    degraded_limited: bool,
}

struct SearchTextInput<'a> {
    query_id: &'a str,
    query: &'a str,
    scope: SearchScopeProjection,
    workspace_id: Option<WorkspaceId>,
    buffer_id: Option<BufferId>,
    file_id: Option<FileId>,
    file_path: Option<CanonicalPath>,
    text: &'a str,
    limit: usize,
    result: &'a mut SearchBuildResult,
}

struct SearchLineInput<'a> {
    query_id: &'a str,
    query: &'a str,
    scope: SearchScopeProjection,
    workspace_id: Option<WorkspaceId>,
    buffer_id: Option<BufferId>,
    file_id: Option<FileId>,
    file_path: Option<CanonicalPath>,
    line_number: u32,
    line_text: &'a str,
    absolute_line_start: u64,
    limit: usize,
    result: &'a mut SearchBuildResult,
}

fn normalize_search_limit(limit: usize) -> usize {
    if limit == 0 {
        SEARCH_DEFAULT_RESULT_LIMIT
    } else {
        limit.min(SEARCH_MAX_RESULT_LIMIT)
    }
}

fn search_status_for_result(
    scope: SearchScopeProjection,
    result: &SearchBuildResult,
) -> SearchStatusProjection {
    if result.degraded_limited {
        return SearchStatusProjection {
            kind: SearchStatusKindProjection::DegradedLimited,
            message: if result.results.is_empty() {
                "Search was limited to degraded viewport content; no visible matches".to_string()
            } else {
                format!(
                    "Search was limited to degraded viewport content; {} visible matches",
                    result.results.len()
                )
            },
        };
    }

    if result.results.is_empty() {
        SearchStatusProjection {
            kind: SearchStatusKindProjection::NoResults,
            message: "No search results".to_string(),
        }
    } else {
        let scope_label = match scope {
            SearchScopeProjection::ActiveFile => "active file",
            SearchScopeProjection::Workspace => "workspace",
        };
        SearchStatusProjection {
            kind: SearchStatusKindProjection::Completed,
            message: format!("Found {} results in {scope_label}", result.results.len()),
        }
    }
}

fn build_search_projection(
    query_id: Option<String>,
    scope: SearchScopeProjection,
    query_label: String,
    result_limit: usize,
    status: SearchStatusProjection,
    result: SearchBuildResult,
) -> SearchProjection {
    SearchProjection {
        query_id,
        scope,
        query_label,
        status,
        results: result.results,
        result_limit,
        omitted_result_count: result.omitted_result_count,
        omitted_file_count: result.omitted_file_count,
        diagnostics: result.diagnostics,
        generated_at: TimestampMillis::now(),
        schema_version: 1,
    }
}

fn workspace_node_is_regular_file(node: &FileTreeNode) -> bool {
    node.metadata
        .as_ref()
        .is_some_and(|metadata| matches!(metadata.kind, FileKind::File))
}

fn collect_search_results_for_text(input: SearchTextInput<'_>) {
    let mut absolute_line_start = 0_u64;
    for (line_number, line) in input.text.split_inclusive('\n').enumerate() {
        let line_text = line.trim_end_matches(&['\r', '\n'][..]);
        collect_search_results_for_line(SearchLineInput {
            query_id: input.query_id,
            query: input.query,
            scope: input.scope,
            workspace_id: input.workspace_id,
            buffer_id: input.buffer_id,
            file_id: input.file_id,
            file_path: input.file_path.clone(),
            line_number: line_number as u32,
            line_text,
            absolute_line_start,
            limit: input.limit,
            result: input.result,
        });
        absolute_line_start = absolute_line_start.saturating_add(line.len() as u64);
    }
}

fn collect_search_results_for_line(input: SearchLineInput<'_>) {
    if input.query.is_empty() {
        return;
    }

    let mut cursor = 0;
    while let Some(relative) = input.line_text[cursor..].find(input.query) {
        let byte_start = cursor + relative;
        let byte_end = byte_start + input.query.len();
        let character_start = input.line_text[..byte_start].chars().count() as u32;
        let character_end = input.line_text[..byte_end].chars().count() as u32;
        let (snippet, snippet_truncated) = bounded_search_snippet(input.line_text);
        let row = SearchResultProjection {
            query_id: input.query_id.to_string(),
            scope: input.scope,
            workspace_id: input.workspace_id,
            buffer_id: input.buffer_id,
            file_id: input.file_id,
            file_path: input.file_path.clone(),
            line_number: input.line_number,
            range: ProtocolTextRange {
                start: TextCoordinate {
                    line: input.line_number,
                    character: character_start,
                    byte_offset: Some(input.absolute_line_start + byte_start as u64),
                    utf16_offset: Some(character_start as u64),
                },
                end: TextCoordinate {
                    line: input.line_number,
                    character: character_end,
                    byte_offset: Some(input.absolute_line_start + byte_end as u64),
                    utf16_offset: Some(character_end as u64),
                },
            },
            snippet,
            snippet_truncated,
        };
        push_bounded_search_result(input.result, input.limit, row);
        cursor = byte_end;
    }
}

fn push_bounded_search_result(
    result: &mut SearchBuildResult,
    limit: usize,
    row: SearchResultProjection,
) {
    if result.results.len() < limit {
        result.results.push(row);
    } else {
        result.omitted_result_count = result.omitted_result_count.saturating_add(1);
    }
}

fn bounded_search_snippet(line: &str) -> (String, bool) {
    if line.len() <= SEARCH_SNIPPET_LIMIT_BYTES {
        return (line.to_string(), false);
    }

    let mut end = SEARCH_SNIPPET_LIMIT_BYTES;
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}...", &line[..end]), true)
}

#[derive(Debug, Clone)]
struct SharedProposalGate {
    required_approvers: HashSet<CollaborationParticipantId>,
    authorized_approvers: HashSet<CollaborationParticipantId>,
    approvals: HashMap<CollaborationParticipantId, CollaborationSharedProposalApproval>,
    denials: HashMap<CollaborationParticipantId, CollaborationSharedProposalApproval>,
    applied_operation_ids: Vec<devil_protocol::CollaborationOperationId>,
    stale: bool,
}

#[derive(Debug, Clone, Default)]
struct CollaborationComposition {
    runtime_sessions_enabled: bool,
    presence_enabled: bool,
    sessions: HashMap<CollaborationSessionId, CollaborationSessionRuntime>,
    shared_proposals: HashMap<(CollaborationSessionId, ProposalId), SharedProposalGate>,
}

impl CollaborationComposition {
    fn presence_projections(&self) -> Vec<CollaborationPresenceProjection> {
        let mut projections = self
            .sessions
            .values()
            .flat_map(CollaborationSessionRuntime::presence)
            .collect::<Vec<_>>();
        projections
            .sort_by_key(|projection| (projection.session_id.0, projection.participant_id.0));
        projections
    }
}

#[derive(Debug, Clone, Default)]
struct RemoteComposition {
    runtime_sessions_enabled: bool,
    runtime: RemoteDevelopmentRuntime,
}

impl RemoteComposition {
    fn enable(&mut self) {
        self.runtime_sessions_enabled = true;
        self.runtime = RemoteDevelopmentRuntime::new(RemoteRuntimeConfig::enabled());
    }

    fn session_descriptors(&self) -> Vec<RemoteWorkspaceSessionDescriptor> {
        self.runtime.session_descriptors()
    }
}

fn remote_error(error: impl ToString) -> AppCompositionError {
    AppCompositionError::Remote(error.to_string())
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
    ai_registry: devil_ai::ProviderRegistry,
    tracker_ledger: TrackerLedger,
    memory_service: MemoryService,
    phase4_projection_state: Phase4ProjectionState,
    plugin_runtime: PluginRuntimeHost,
    plugin_contribution_projections: Vec<PluginContributionProjection>,
    collaboration: CollaborationComposition,
    remote: RemoteComposition,
    search_projection: SearchProjection,
    language_tooling: LanguageToolingWorkflow,
    terminal_workflow: TerminalWorkflow,
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
            ai_registry: make_stub_registry(),
            tracker_ledger: TrackerLedger::new(),
            memory_service: MemoryService::new(),
            phase4_projection_state: Phase4ProjectionState::default(),
            plugin_runtime: PluginRuntimeHost::new(),
            plugin_contribution_projections: Vec::new(),
            collaboration: CollaborationComposition::default(),
            remote: RemoteComposition::default(),
            search_projection: SearchProjection::idle(),
            language_tooling: LanguageToolingWorkflow::default(),
            terminal_workflow: TerminalWorkflow::default(),
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

    fn persist_latest_terminal_audit(&mut self) -> Result<(), AppCompositionError> {
        let Some(record) = self.terminal_workflow.take_last_audit() else {
            return Ok(());
        };
        self.storage
            .handle(StorageRepositoryRequest::SaveTerminalAuditRecord(
                record.clone(),
            ))
            .map_err(AppCompositionError::Protocol)?;
        let envelope = terminal_audit_recorded_event(&record)
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        self.emit_event(envelope);
        Ok(())
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

        let buffer_id = self
            .editor
            .buffer_for_file(identity.workspace_id, identity.file_id)
            .or_else(|| {
                self.editor
                    .buffer_for_path(identity.workspace_id, &identity.canonical_path.0)
            })
            .map_or_else(
                || {
                    self.editor.open_buffer(
                        identity.workspace_id,
                        identity.file_id,
                        identity.canonical_path.0.clone(),
                        opened.text.clone(),
                    )
                },
                Ok,
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

    fn proposal_intent_id(intent: &CommandDispatchIntent) -> Option<ProposalId> {
        match intent {
            CommandDispatchIntent::PreviewProposal { proposal_id }
            | CommandDispatchIntent::ApproveProposal { proposal_id }
            | CommandDispatchIntent::RejectProposal { proposal_id, .. }
            | CommandDispatchIntent::ApplyProposal { proposal_id }
            | CommandDispatchIntent::RollbackProposal { proposal_id, .. }
            | CommandDispatchIntent::CancelProposal { proposal_id, .. }
            | CommandDispatchIntent::OpenProposalDetails { proposal_id } => Some(*proposal_id),
            _ => None,
        }
    }

    fn dispatch_proposal_ui_intent(
        &mut self,
        intent: CommandDispatchIntent,
        event_context: EventContext,
    ) -> Result<AppCommandOutcome, AppCompositionError> {
        let proposal_id = Self::proposal_intent_id(&intent)
            .ok_or(AppCompositionError::ProposalIntentMissingProposal)?;
        let proposal = self
            .proposal_coordinator
            .proposal_for_id(proposal_id)
            .ok_or(AppCompositionError::ProposalIntentMismatch {
                target: proposal_id,
                active: None,
            })?;
        let context = AppProposalIntentRouteContext {
            proposal: Some(proposal.clone()),
            principal: proposal.principal.clone(),
            capability: proposal.capability.clone(),
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            requested_at: TimestampMillis::now(),
        };
        let routed = CommandDispatcher::route_proposal_intent(intent, context)?;
        self.proposal_coordinator
            .select_proposal_for_details(proposal_id);

        if let Some(request) = routed {
            let response = self.handle_routed_proposal_request(request)?;
            Ok(AppCommandOutcome::ProposalLifecycleUpdated(response))
        } else {
            Ok(AppCommandOutcome::ProposalDetailsOpened(proposal_id))
        }
    }

    fn handle_routed_proposal_request(
        &mut self,
        request: ProposalRequest,
    ) -> Result<ProposalResponse, AppCompositionError> {
        if let ProposalRequest::Preview(proposal) = &request
            && self
                .proposal_coordinator
                .current_lifecycle_state(proposal.proposal_id)
                == Some(ProposalLifecycleState::Created)
        {
            let validated =
                self.handle_proposal_request(ProposalRequest::Validate(proposal.clone()))?;
            if !matches!(validated, ProposalResponse::Validated(_)) {
                return Ok(validated);
            }
        }
        self.handle_proposal_request(request)
    }

    /// Route a UI dispatch intent through editor and workspace authorities.
    pub fn dispatch_ui_intent(
        &mut self,
        intent: CommandDispatchIntent,
    ) -> Result<AppCommandOutcome, AppCompositionError> {
        let event_context = self.next_event_context();
        if Self::proposal_intent_id(&intent).is_some() {
            return self.dispatch_proposal_ui_intent(intent, event_context);
        }

        let request = CommandDispatcher::route_intent(
            intent,
            AppCommandRouteContext::from_active(&self.active_documents),
            event_context.correlation_id,
        )?;

        if let AppCommandRequest::ApplyEdit { buffer_id, edit } = &request {
            let descriptor = self.apply_edit_to_buffer_with_correlation(
                *buffer_id,
                edit.clone(),
                event_context.correlation_id,
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
            AppCommandRequest::SwitchTab { buffer_id } => {
                self.switch_tab(buffer_id)?;
                Ok(AppCommandOutcome::TabSwitched(buffer_id))
            }
            AppCommandRequest::CloseTab { buffer_id } => {
                Ok(AppCommandOutcome::TabClose(self.close_tab(buffer_id)?))
            }
            AppCommandRequest::SaveAll => Ok(AppCommandOutcome::SaveAll(self.save_all()?)),
            AppCommandRequest::SetCursor { buffer_id, cursor } => {
                self.set_buffer_cursor(buffer_id, cursor)?;
                Ok(AppCommandOutcome::CursorSet(buffer_id))
            }
            AppCommandRequest::SetSelection { buffer_id, range } => {
                self.set_buffer_selection(buffer_id, range)?;
                Ok(AppCommandOutcome::SelectionSet(buffer_id))
            }
            AppCommandRequest::SetViewportScroll { buffer_id, scroll } => {
                self.set_viewport_scroll(buffer_id, scroll)?;
                Ok(AppCommandOutcome::ViewportScrollSet(buffer_id))
            }
            AppCommandRequest::RunSearch {
                query_id,
                scope,
                query,
                limit,
            } => Ok(AppCommandOutcome::SearchUpdated(
                self.run_search(query_id, scope, query, limit)?,
            )),
            AppCommandRequest::CancelSearch { query_id } => Ok(AppCommandOutcome::SearchUpdated(
                self.cancel_search(query_id),
            )),
            AppCommandRequest::RequestHover {
                buffer_id,
                position,
            } => Ok(AppCommandOutcome::LanguageToolingUpdated(
                self.run_language_read(buffer_id, LanguageReadKind::Hover, position)?,
            )),
            AppCommandRequest::RequestCompletion {
                buffer_id,
                position,
            } => Ok(AppCommandOutcome::LanguageToolingUpdated(
                self.run_language_read(buffer_id, LanguageReadKind::Completion, position)?,
            )),
            AppCommandRequest::GoToDefinition {
                buffer_id,
                position,
            } => Ok(AppCommandOutcome::LanguageToolingUpdated(
                self.run_language_read(buffer_id, LanguageReadKind::Definition, position)?,
            )),
            AppCommandRequest::FindReferences {
                buffer_id,
                position,
            } => Ok(AppCommandOutcome::LanguageToolingUpdated(
                self.run_language_read(buffer_id, LanguageReadKind::References, position)?,
            )),
            AppCommandRequest::RefreshOutline { buffer_id } => Ok(
                AppCommandOutcome::LanguageToolingUpdated(self.run_language_read(
                    buffer_id,
                    LanguageReadKind::Outline,
                    TextCoordinate {
                        line: 0,
                        character: 0,
                        byte_offset: Some(0),
                        utf16_offset: Some(0),
                    },
                )?),
            ),
            AppCommandRequest::RequestFormattingProposal { buffer_id } => Ok(
                AppCommandOutcome::LanguageToolingUpdated(self.run_language_proposal(
                    buffer_id,
                    LanguageProposalKind::Formatting,
                    TextCoordinate {
                        line: 0,
                        character: 0,
                        byte_offset: Some(0),
                        utf16_offset: Some(0),
                    },
                    "format".to_string(),
                )?),
            ),
            AppCommandRequest::RequestRenameProposal {
                buffer_id,
                position,
                new_name,
            } => Ok(AppCommandOutcome::LanguageToolingUpdated(
                self.run_language_proposal(
                    buffer_id,
                    LanguageProposalKind::Rename,
                    position,
                    new_name,
                )?,
            )),
            AppCommandRequest::RequestOrganizeImportsProposal { buffer_id } => Ok(
                AppCommandOutcome::LanguageToolingUpdated(self.run_language_proposal(
                    buffer_id,
                    LanguageProposalKind::OrganizeImports,
                    TextCoordinate {
                        line: 0,
                        character: 0,
                        byte_offset: Some(0),
                        utf16_offset: Some(0),
                    },
                    "organize-imports".to_string(),
                )?),
            ),
            AppCommandRequest::RequestCodeActionProposal {
                buffer_id,
                action_id,
            } => Ok(AppCommandOutcome::LanguageToolingUpdated(
                self.run_language_proposal(
                    buffer_id,
                    LanguageProposalKind::CodeAction,
                    TextCoordinate {
                        line: 0,
                        character: 0,
                        byte_offset: Some(0),
                        utf16_offset: Some(0),
                    },
                    action_id,
                )?,
            )),
            AppCommandRequest::CancelLanguageOperation { operation_id } => {
                let event_context = self.next_event_context();
                self.language_tooling
                    .cancel_operation(operation_id, event_context);
                Ok(AppCommandOutcome::LanguageToolingUpdated(
                    self.language_tooling.projection(),
                ))
            }
            AppCommandRequest::TerminalLaunch { command_label } => {
                let context = self.active_documents.require_workspace_context()?;
                let event_context = self.next_event_context();
                let projection =
                    self.terminal_workflow
                        .launch(context, command_label, event_context);
                self.persist_latest_terminal_audit()?;
                Ok(AppCommandOutcome::TerminalPanelUpdated(projection))
            }
            AppCommandRequest::TerminalInput {
                session_id,
                payload,
            } => {
                let context = self.active_documents.require_workspace_context()?;
                let event_context = self.next_event_context();
                let projection =
                    self.terminal_workflow
                        .input(context, session_id, payload, event_context);
                self.persist_latest_terminal_audit()?;
                Ok(AppCommandOutcome::TerminalPanelUpdated(projection))
            }
            AppCommandRequest::TerminalResize {
                session_id,
                cols,
                rows,
            } => {
                let context = self.active_documents.require_workspace_context()?;
                let event_context = self.next_event_context();
                let projection =
                    self.terminal_workflow
                        .resize(context, session_id, cols, rows, event_context);
                self.persist_latest_terminal_audit()?;
                Ok(AppCommandOutcome::TerminalPanelUpdated(projection))
            }
            AppCommandRequest::TerminalKill { session_id } => {
                let context = self.active_documents.require_workspace_context()?;
                let event_context = self.next_event_context();
                let projection =
                    self.terminal_workflow
                        .close_or_kill(context, session_id, true, event_context);
                self.persist_latest_terminal_audit()?;
                Ok(AppCommandOutcome::TerminalPanelUpdated(projection))
            }
            AppCommandRequest::TerminalClose { session_id } => {
                let context = self.active_documents.require_workspace_context()?;
                let event_context = self.next_event_context();
                let projection =
                    self.terminal_workflow
                        .close_or_kill(context, session_id, false, event_context);
                self.persist_latest_terminal_audit()?;
                Ok(AppCommandOutcome::TerminalPanelUpdated(projection))
            }
            AppCommandRequest::TerminalOutputPoll { session_id } => {
                let event_context = self.next_event_context();
                let projection = self.terminal_workflow.poll(session_id, event_context);
                self.persist_latest_terminal_audit()?;
                Ok(AppCommandOutcome::TerminalPanelUpdated(projection))
            }
            AppCommandRequest::TerminalSearch { session_id, query } => {
                let event_context = self.next_event_context();
                let projection = self
                    .terminal_workflow
                    .search(session_id, query, event_context);
                self.persist_latest_terminal_audit()?;
                Ok(AppCommandOutcome::TerminalPanelUpdated(projection))
            }
            AppCommandRequest::OpenPath { path } => {
                Ok(AppCommandOutcome::Opened(self.open_file(path)?))
            }
            AppCommandRequest::StartAiRun { instruction_label } => Ok(
                AppCommandOutcome::AiRunStarted(Box::new(self.start_ai_run(instruction_label)?)),
            ),
            AppCommandRequest::StartAiExplain { instruction_label } => {
                Ok(AppCommandOutcome::AiRunStarted(Box::new(
                    self.start_ai_explain(instruction_label)?,
                )))
            }
            AppCommandRequest::StartAiProposal { instruction_label } => {
                Ok(AppCommandOutcome::AiRunStarted(Box::new(
                    self.start_ai_proposal(instruction_label)?,
                )))
            }
            AppCommandRequest::CancelAiRun { run_id } => {
                self.cancel_ai_run(run_id.clone())?;
                Ok(AppCommandOutcome::AiRunCancelled(run_id))
            }
            AppCommandRequest::ReplayAiRun { run_id } => Ok(AppCommandOutcome::AiRunReplayed(
                Box::new(self.replay_ai_run(run_id)?),
            )),
            AppCommandRequest::InspectAiRun { run_id } => Ok(AppCommandOutcome::AiRunInspected(
                Box::new(self.inspect_ai_run(run_id)?),
            )),
            AppCommandRequest::InvokePluginCommand {
                plugin_id,
                command_id,
                metadata_label,
            } => Ok(AppCommandOutcome::PluginCommandInvoked(Box::new(
                self.invoke_plugin_command(plugin_id, command_id, metadata_label)?,
            ))),
            AppCommandRequest::JoinCollaborationSession { session_id } => {
                self.join_collaboration_session(session_id)?;
                Ok(AppCommandOutcome::CollaborationSessionJoined(session_id))
            }
            AppCommandRequest::LeaveCollaborationSession { session_id } => {
                self.leave_collaboration_session(session_id)?;
                Ok(AppCommandOutcome::CollaborationSessionLeft(session_id))
            }
            AppCommandRequest::PublishCollaborationPresence {
                session_id,
                participant_id,
            } => {
                self.publish_collaboration_presence(session_id, participant_id)?;
                Ok(AppCommandOutcome::CollaborationPresencePublished(
                    session_id,
                ))
            }
            _ => unreachable!("command execution service handled non-workflow command"),
        }
    }

    /// Load a Phase 5 plugin manifest after app-level trust and manifest validation.
    pub fn load_plugin_manifest(
        &mut self,
        manifest: PluginManifest,
    ) -> Result<PluginId, AppCompositionError> {
        let projection = PluginContributionProjection {
            plugin_id: manifest.plugin_id,
            contributions: manifest.contributions.clone(),
            status_label: "loaded".to_string(),
        };
        let plugin_id = self
            .plugin_runtime
            .load_manifest(manifest)
            .map_err(AppCompositionError::Protocol)?;
        self.plugin_contribution_projections.push(projection);
        Ok(plugin_id)
    }

    /// Invoke a Phase 5 plugin command through protocol host-call envelopes only.
    pub fn invoke_plugin_command(
        &mut self,
        plugin_id: PluginId,
        command_id: impl Into<String>,
        metadata_label: impl Into<String>,
    ) -> Result<PluginHostCallResponse, AppCompositionError> {
        let command_id = command_id.into();
        let event_context = self.next_event_context();
        let sequence = self.event_sequence_generator.next();
        let response = self
            .plugin_runtime
            .dispatch_host_call(PluginHostCallRequest {
                plugin_id,
                kind: PluginHostCallKind::ReadOnlyContext,
                host_call_name: format!("command:{command_id}"),
                declared_capability: CapabilityId("plugin.command".to_string()),
                correlation_id: event_context.correlation_id,
                causality_id: event_context.causality_id,
                sequence,
                metadata_label: metadata_label.into(),
            })
            .map_err(AppCompositionError::Protocol)?;
        let envelope = plugin_event_envelope(
            devil_protocol::EventId(uuid::Uuid::now_v7()),
            plugin_id,
            "plugin.command_invoked",
            event_context.correlation_id,
            event_context.causality_id,
            sequence,
            TimestampMillis::now(),
        )
        .map_err(|err| {
            AppCompositionError::Protocol(ProtocolError {
                code: "plugin_event_invalid".to_string(),
                message: err.to_string(),
            })
        })?;
        self.emit_event(envelope);
        Ok(response)
    }

    /// Enable local deterministic collaboration runtime for trusted app-owned sessions.
    pub fn enable_local_collaboration_runtime(&mut self) {
        self.collaboration.runtime_sessions_enabled = true;
        self.collaboration.presence_enabled = true;
    }

    /// Create or join a local collaboration session bound to the active editor buffer.
    pub fn join_collaboration_session(
        &mut self,
        session_id: CollaborationSessionId,
    ) -> Result<(), AppCompositionError> {
        if !self.collaboration.runtime_sessions_enabled {
            return Err(AppCompositionError::Collaboration(
                "collaboration runtime sessions are disabled by policy".to_string(),
            ));
        }
        let context = self.active_documents.require_active_save_context()?;
        if context.trust != WorkspaceTrustState::Trusted {
            return Err(AppCompositionError::Collaboration(
                "untrusted workspaces cannot join collaboration sessions".to_string(),
            ));
        }
        if self.collaboration.sessions.contains_key(&session_id) {
            return Ok(());
        }
        let snapshot = self.editor.current_snapshot(context.buffer_id)?.clone();
        let descriptor = CollaborationSessionDescriptor {
            session_id,
            workspace_id: context.workspace_id,
            state: CollaborationSessionState::Active,
            created_by: context.principal.clone(),
            created_at: TimestampMillis::now(),
            document_bindings: vec![CollaborationDocumentBinding {
                workspace_id: context.workspace_id,
                file_id: context.metadata.identity.file_id,
                buffer_id: context.buffer_id,
                snapshot_id: snapshot.snapshot_id,
                buffer_version: snapshot.buffer_version,
                document_epoch: CollaborationDocumentEpoch(1),
                content_fingerprint: Some(FileFingerprint {
                    algorithm: "devil-text-snapshot".to_string(),
                    value: snapshot.content_hash,
                }),
                schema_version: 1,
            }],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let participant = CollaborationParticipant {
            session_id,
            participant_id: CollaborationParticipantId(1),
            principal_id: context.principal,
            role: CollaborationParticipantRole::Owner,
            permissions: vec![
                CollaborationPermission::CreateSession,
                CollaborationPermission::JoinSession,
                CollaborationPermission::PublishOperation,
                CollaborationPermission::PublishPresence,
                CollaborationPermission::ApproveSharedProposal,
                CollaborationPermission::ReplayMetadata,
                CollaborationPermission::ExportAudit,
            ],
            display_label: "local participant".to_string(),
            schema_version: 1,
        };
        let runtime = CollaborationSessionRuntime::new(
            descriptor,
            vec![participant],
            "",
            CollaborationRuntimeConfig::enabled(),
        )
        .map_err(|error| AppCompositionError::Collaboration(error.to_string()))?;
        self.collaboration.sessions.insert(session_id, runtime);
        Ok(())
    }

    /// Leave a local collaboration session without mutating editor text.
    pub fn leave_collaboration_session(
        &mut self,
        session_id: CollaborationSessionId,
    ) -> Result<(), AppCompositionError> {
        if let Some(runtime) = self.collaboration.sessions.get_mut(&session_id) {
            runtime.begin_shutdown();
            runtime.finish_shutdown();
        }
        self.collaboration.sessions.remove(&session_id);
        Ok(())
    }

    /// Publish metadata-only collaboration presence through app-owned runtime state.
    pub fn publish_collaboration_presence(
        &mut self,
        session_id: CollaborationSessionId,
        participant_id: CollaborationParticipantId,
    ) -> Result<(), AppCompositionError> {
        if !self.collaboration.presence_enabled {
            return Err(AppCompositionError::Collaboration(
                "collaboration presence is disabled by policy".to_string(),
            ));
        }
        let runtime = self
            .collaboration
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| {
                AppCompositionError::Collaboration("collaboration session is missing".to_string())
            })?;
        runtime
            .publish_presence(CollaborationPresenceProjection {
                session_id,
                participant_id,
                cursor: None,
                selections: Vec::new(),
                activity_label: Some("active".to_string()),
                reconnecting: false,
                schema_version: 1,
            })
            .map_err(|error| AppCompositionError::Collaboration(error.to_string()))?;
        Ok(())
    }

    /// Receive a deterministic local transport envelope and apply accepted operations through editor authority.
    pub fn receive_collaboration_transport_envelope(
        &mut self,
        envelope: CollaborationTransportEnvelope,
    ) -> Result<Option<AppCommandOutcome>, AppCompositionError> {
        let operation = match &envelope.payload {
            CollaborationTransportPayload::Operation(operation) => Some((**operation).clone()),
            _ => None,
        };
        let runtime = self
            .collaboration
            .sessions
            .get_mut(&envelope.session_id)
            .ok_or_else(|| {
                AppCompositionError::Collaboration("collaboration session is missing".to_string())
            })?;
        let outcome = runtime
            .handle_transport_envelope(envelope)
            .map_err(|error| AppCompositionError::Collaboration(error.to_string()))?;

        let Some(operation) = operation else {
            return Ok(None);
        };
        let Some(outcome) = outcome else {
            return Ok(None);
        };
        if outcome.acknowledgement.status
            != devil_protocol::CollaborationAcknowledgementStatus::Accepted
        {
            return Ok(None);
        }
        let descriptor = self.apply_collaboration_operation_through_editor(operation)?;
        self.emit_transaction_event(&descriptor);
        let audit = self.collaboration_audit_record(Some(descriptor.correlation_id), None, None)?;
        self.persist_collaboration_audit(audit)?;
        Ok(Some(AppCommandOutcome::CollaborationOperationApplied(
            descriptor,
        )))
    }

    fn apply_collaboration_operation_through_editor(
        &mut self,
        operation: CollaborationDocumentOperation,
    ) -> Result<TextTransactionDescriptor, AppCompositionError> {
        let replacement = match operation.kind {
            CollaborationDocumentOperationKind::Insert { text }
            | CollaborationDocumentOperationKind::Replace { text } => text,
            CollaborationDocumentOperationKind::Delete => String::new(),
            _ => {
                return Err(AppCompositionError::Collaboration(
                    "operation has no text mutation".to_string(),
                ));
            }
        };
        let range = operation.range.ok_or_else(|| {
            AppCompositionError::Collaboration("text operation requires a range".to_string())
        })?;
        let record = self
            .editor
            .apply_protocol_edits(EditorApplyTransactionRequest {
                workspace_id: operation.preconditions.workspace_id,
                buffer_id: operation.preconditions.buffer_id,
                file_id: operation.preconditions.file_id,
                edits: devil_protocol::EditBatch {
                    edits: vec![devil_protocol::TextEdit { range, replacement }],
                },
                source: TransactionSource::CollaborationParticipant {
                    session_id: operation.session_id,
                    participant_id: operation.author_participant_id,
                    operation_id: operation.operation_id,
                },
                undo_group_id: operation.undo_group.map(|group| group.group_id),
                correlation_id: operation.preconditions.correlation_id,
            })?;
        Ok(record.to_protocol_descriptor())
    }

    fn collaboration_audit_record(
        &mut self,
        correlation_id: Option<CorrelationId>,
        operation_id: Option<devil_protocol::CollaborationOperationId>,
        proposal_id: Option<ProposalId>,
    ) -> Result<CollaborationAuditRecord, AppCompositionError> {
        let session = self.collaboration.sessions.values().next().ok_or_else(|| {
            AppCompositionError::Collaboration("collaboration session is missing".to_string())
        })?;
        Ok(session.audit_record(
            operation_id,
            proposal_id,
            self.event_sequence_generator.next(),
            correlation_id.unwrap_or_else(|| self.correlation_generator.next()),
        ))
    }

    fn persist_collaboration_audit(
        &self,
        record: CollaborationAuditRecord,
    ) -> Result<(), AppCompositionError> {
        self.storage
            .handle(StorageRepositoryRequest::SaveCollaborationAuditRecord(
                record.clone(),
            ))
            .map_err(AppCompositionError::Protocol)?;
        let envelope = collaboration_audit_recorded_event(&record)
            .map_err(|error| AppCompositionError::Collaboration(error.to_string()))?;
        self.emit_event(envelope);
        Ok(())
    }

    /// Enable deterministic Phase 7 remote development runtime for app-owned sessions.
    pub fn enable_remote_development_runtime(&mut self) {
        self.remote.enable();
    }

    /// Connect a remote workspace session through app-owned composition.
    pub fn connect_remote_workspace_session(
        &mut self,
        session_id: RemoteWorkspaceSessionId,
        authority_label: impl Into<String>,
    ) -> Result<RemoteWorkspaceSessionDescriptor, AppCompositionError> {
        if !self.remote.runtime_sessions_enabled {
            return Err(AppCompositionError::Remote(
                "remote runtime sessions are disabled by policy".to_string(),
            ));
        }
        let context = self.active_documents.require_workspace_context()?;
        if context.trust != WorkspaceTrustState::Trusted {
            return Err(AppCompositionError::Remote(
                "untrusted workspaces cannot connect remote sessions".to_string(),
            ));
        }

        let authority_id = devil_protocol::RemoteAuthorityId(session_id.0.saturating_add(100));
        let descriptor = RemoteWorkspaceSessionDescriptor {
            session_id,
            authority: RemoteAuthorityDescriptor {
                authority_id,
                authority_label: authority_label.into(),
                workspace_id: context.workspace_id,
                trust_state: context.trust,
                principal_id: context.principal,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            agent: RemoteAgentDescriptor {
                agent_id: devil_protocol::RemoteAgentId(session_id.0.saturating_add(200)),
                authority_id,
                agent_version: "devil-remote-deterministic/1".to_string(),
                runtime_enabled: true,
                schema_version: 1,
            },
            state: RemoteWorkspaceLifecycleState::Active,
            granted_capabilities: vec![
                devil_protocol::RemoteCapabilityKind::Connect,
                devil_protocol::RemoteCapabilityKind::FilesystemRead,
                devil_protocol::RemoteCapabilityKind::FilesystemWrite,
                devil_protocol::RemoteCapabilityKind::ProcessLaunch,
                devil_protocol::RemoteCapabilityKind::PtyInput,
                devil_protocol::RemoteCapabilityKind::LspLaunch,
                devil_protocol::RemoteCapabilityKind::SemanticQuery,
                devil_protocol::RemoteCapabilityKind::OfflineResume,
                devil_protocol::RemoteCapabilityKind::AuditExport,
            ],
            created_at: TimestampMillis::now(),
            last_heartbeat_at: Some(TimestampMillis::now()),
            schema_version: 1,
        };
        self.remote
            .runtime
            .create_session(descriptor.clone(), context.workspace_generation)
            .map_err(|error| AppCompositionError::Remote(error.to_string()))?;
        Ok(descriptor)
    }

    /// Return projection-safe remote session descriptors.
    pub fn remote_session_projections(&self) -> Vec<RemoteWorkspaceSessionDescriptor> {
        self.remote.session_descriptors()
    }

    /// Seed an ephemeral deterministic remote fixture file for Phase 7 validation.
    pub fn seed_remote_fixture_file(
        &mut self,
        session_id: RemoteWorkspaceSessionId,
        path: CanonicalPath,
        file_id: FileId,
        content: impl Into<String>,
    ) -> Result<devil_protocol::RemoteFilesystemSnapshot, AppCompositionError> {
        self.remote
            .runtime
            .session_mut(session_id)
            .map_err(remote_error)?
            .seed_file(path, file_id, content)
            .map_err(remote_error)
    }

    /// Receive a remote transport envelope and persist metadata-only app-owned audit.
    pub fn receive_remote_transport_envelope(
        &mut self,
        envelope: RemoteTransportEnvelope,
    ) -> Result<RemoteOperationOutcome, AppCompositionError> {
        let session_id = envelope.session_id;
        let operation_id = envelope.operation_id;
        let correlation_id = envelope.correlation_id;
        let causality_id = envelope.causality_id;
        let proposal_id = match &envelope.payload {
            RemoteTransportPayload::FilesystemOperation(operation) => operation.proposal_id,
            RemoteTransportPayload::Audit(record) => record.proposal_id,
            _ => None,
        };

        let outcome = self
            .remote
            .runtime
            .handle_transport_envelope(envelope)
            .map_err(remote_error)?;
        let audit = self
            .remote
            .runtime
            .session(session_id)
            .map_err(remote_error)?
            .audit_record(
                Some(operation_id),
                proposal_id,
                self.event_sequence_generator.next(),
                correlation_id,
                causality_id,
            );
        self.persist_remote_audit(audit)?;
        Ok(outcome)
    }

    fn persist_remote_audit(&self, record: RemoteAuditRecord) -> Result<(), AppCompositionError> {
        self.storage
            .handle(StorageRepositoryRequest::SaveRemoteAuditRecord(
                record.clone(),
            ))
            .map_err(AppCompositionError::Protocol)?;
        let envelope = remote_audit_recorded_event(&record)
            .map_err(|error| AppCompositionError::Remote(error.to_string()))?;
        self.emit_event(envelope);
        Ok(())
    }

    /// Start a Phase 4 local-provider AI run and register its generated edit as a proposal.
    pub fn start_ai_run(
        &mut self,
        instruction_label: impl Into<String>,
    ) -> Result<AppAiRunOutcome, AppCompositionError> {
        self.start_ai_proposal(instruction_label)
    }

    /// Start a metadata-only assisted-AI explain run.
    pub fn start_ai_explain(
        &mut self,
        instruction_label: impl Into<String>,
    ) -> Result<AppAiRunOutcome, AppCompositionError> {
        self.run_assisted_ai_operation(
            devil_protocol::AssistedAiOperationClass::Explain,
            instruction_label,
            devil_protocol::AssistedAiProviderClass::LocalLoopback,
        )
    }

    /// Start a proposal-only assisted-AI edit run.
    pub fn start_ai_proposal(
        &mut self,
        instruction_label: impl Into<String>,
    ) -> Result<AppAiRunOutcome, AppCompositionError> {
        self.run_assisted_ai_operation(
            devil_protocol::AssistedAiOperationClass::ProposeEdit,
            instruction_label,
            devil_protocol::AssistedAiProviderClass::LocalLoopback,
        )
    }

    fn run_assisted_ai_operation(
        &mut self,
        operation_class: devil_protocol::AssistedAiOperationClass,
        instruction_label: impl Into<String>,
        provider_class: devil_protocol::AssistedAiProviderClass,
    ) -> Result<AppAiRunOutcome, AppCompositionError> {
        let instruction_label = instruction_label.into();
        let context = self.active_documents.require_active_save_context()?;
        let event_context = self.next_event_context();
        let generated_at = TimestampMillis::now();
        let snapshot = self.editor.current_snapshot(context.buffer_id)?.clone();
        let run_id =
            devil_protocol::AgentRunId(format!("phase4-run-{}", event_context.correlation_id.0));
        let route_id = format!("phase4-route-{}", event_context.correlation_id.0);
        let snapshot_hash = FileFingerprint {
            algorithm: "devil-text-snapshot".to_string(),
            value: snapshot.content_hash.clone(),
        };
        let context_manifest_projection = Phase4ContextAssemblyService::assemble_context_manifest(
            &context,
            &run_id,
            &route_id,
            snapshot.snapshot_id,
            snapshot.buffer_version,
            snapshot_hash,
            snapshot.byte_len as u64,
            snapshot.line_count.min(u32::MAX as usize) as u32,
            generated_at,
        );
        let privacy_inspector_projection =
            devil_protocol::privacy_inspector_from_context_manifest_projection(
                &context_manifest_projection,
                format!("phase4:privacy:{}", run_id.0),
                generated_at,
                1,
            );
        let permission_budget_projection = phase4_permission_budget_projection(
            &context_manifest_projection,
            &run_id,
            generated_at,
        );

        let mut agent = AgentRuntime::new(run_id.clone());
        agent
            .transition(
                devil_protocol::AgentRunState::Planning,
                "agent.planning.context_ready",
                event_context.correlation_id,
                event_context.causality_id,
                self.event_sequence_generator.next(),
            )
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;

        let provider_route_request = devil_protocol::AssistedAiProviderRouteRequest {
            route_id: route_id.clone(),
            provider_id: DETERMINISTIC_LOCAL_PROVIDER_ID.to_string(),
            model_label: "deterministic-local".to_string(),
            provider_class,
            operation_class,
            context_manifest: trust_reference(
                &context_manifest_projection.manifest.manifest_id,
                devil_protocol::AssistedAiTrustProjectionKind::ContextManifest,
            ),
            privacy_inspector: trust_reference(
                &privacy_inspector_projection.inspector_id,
                devil_protocol::AssistedAiTrustProjectionKind::PrivacyInspector,
            ),
            permission_budget: trust_reference(
                &permission_budget_projection.projection_id,
                devil_protocol::AssistedAiTrustProjectionKind::PermissionBudget,
            ),
            proposal_intent: devil_protocol::AssistedAiProposalTargetIntent {
                payload_kind: devil_protocol::ProposalPayloadKind::TextEdit,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![ProposalAffectedTarget {
                        target_id: format!("file:{}", context.metadata.identity.file_id.0),
                        kind: ProposalTargetKind::OpenBuffer,
                        workspace_id: Some(context.workspace_id),
                        file_id: Some(context.metadata.identity.file_id),
                        buffer_id: Some(context.buffer_id),
                        path: Some(context.metadata.identity.canonical_path.clone()),
                        terminal_session_id: None,
                        plugin_id: None,
                        remote_authority: None,
                        collaboration_session_id: None,
                        byte_ranges: vec![devil_protocol::ByteRange::new(0, 0)],
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                    }],
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                required_capability: CapabilityId("editor.write".to_string()),
                risk_label: devil_protocol::ProposalRiskLabel::Low,
                privacy_label: devil_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                labels: vec![instruction_label.clone()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            policy_decision_id: None,
            required_capability: CapabilityId("ai.provider.invoke".to_string()),
            network_target: Some(devil_protocol::NetworkTarget {
                scheme: "http".to_string(),
                host: "localhost".to_string(),
                port: Some(11434),
            }),
            cancellation_token: devil_protocol::CancellationTokenId(uuid::Uuid::now_v7()),
            health_labels: vec!["local.deterministic".to_string()],
            cost_labels: vec!["local.free".to_string()],
            principal_id: context.principal.clone(),
            workspace_trust_state: context.trust.clone(),
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            event_sequence: self.event_sequence_generator.next(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let broker = DenyByDefaultBroker::new(
            SecurityPolicy::default(),
            CapabilityNamespace("app.ai".to_string()),
        );
        let route_response = ProviderRouter::new(&self.ai_registry, &broker)
            .route_completion(provider_route_request.clone())
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        if route_response.invocation_state
            != devil_protocol::AssistedAiProviderInvocationState::Completed
            || operation_class == devil_protocol::AssistedAiOperationClass::Explain
        {
            return self.finish_assisted_ai_metadata_only_run(
                run_id,
                route_id,
                operation_class,
                provider_class,
                provider_route_request,
                route_response,
                context_manifest_projection,
                privacy_inspector_projection,
                permission_budget_projection,
                generated_at,
                event_context,
                &mut agent,
            );
        }

        agent
            .transition(
                devil_protocol::AgentRunState::Proposing,
                "agent.proposing.provider_completed",
                event_context.correlation_id,
                event_context.causality_id,
                self.event_sequence_generator.next(),
            )
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;

        let proposal_id = self.proposal_coordinator.next_id();
        let preconditions = ProposalVersionPreconditions {
            file_version: Some(context.metadata.file_content_version),
            buffer_version: Some(snapshot.buffer_version),
            snapshot_id: Some(snapshot.snapshot_id),
            generation: Some(context.metadata.workspace_generation),
            file_content_version: Some(context.metadata.file_content_version),
            workspace_generation: Some(context.metadata.workspace_generation),
            expected_fingerprint: Some(context.metadata.fingerprint.clone()),
            expected_file_length: context.metadata.file_length,
            expected_modified_at: context.metadata.modified_at,
        };
        let output = devil_protocol::AssistedAiEditProposalOutput {
            output_id: format!("phase4-output-{}", event_context.correlation_id.0),
            request_id: format!("phase4-request-{}", event_context.correlation_id.0),
            provider_id: DETERMINISTIC_LOCAL_PROVIDER_ID.to_string(),
            proposal_id,
            principal: context.principal.clone(),
            capability: CapabilityId("editor.write".to_string()),
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            payload: ProposalPayload::TextEdit(devil_protocol::TextEditProposal {
                file_id: context.metadata.identity.file_id,
                edits: devil_protocol::EditBatch {
                    edits: vec![devil_protocol::TextEdit {
                        range: devil_protocol::TextRange::byte(0, 0),
                        replacement: "/* phase4 local AI proposal */\n".to_string(),
                    }],
                },
            }),
            preconditions,
            preview: PreviewSummary {
                summary: "Phase 4 local AI edit proposal".to_string(),
                details: vec![
                    "Generated by deterministic local provider".to_string(),
                    "Proposal is registered only; app/editor/workspace own apply".to_string(),
                ],
            },
            expires_at: None,
            created_at: generated_at,
            context_manifest: trust_reference(
                &context_manifest_projection.manifest.manifest_id,
                devil_protocol::AssistedAiTrustProjectionKind::ContextManifest,
            ),
            approval_checklist: trust_reference(
                &format!("phase4:approval:{}", run_id.0),
                devil_protocol::AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let proposal = output
            .to_workspace_proposal()
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        let proposal_created = self.register_proposal_lifecycle(&proposal)?;
        let ledger_projection = self
            .proposal_coordinator
            .proposal_ledger_projection(generated_at);
        let checkpoint_rollback_projection =
            devil_protocol::checkpoint_rollback_projection_from_proposal(
                format!("phase4:checkpoint:{}", run_id.0),
                &proposal,
                ProposalLifecycleState::Created,
                Some(&ledger_projection),
                devil_protocol::CheckpointRollbackAuditStatus::Available,
                Some(event_context.causality_id),
                generated_at,
                1,
            );
        let approval_checklist_projection =
            devil_protocol::approval_checklist_from_trust_projections(
                format!("phase4:approval:{}", run_id.0),
                &proposal,
                ProposalLifecycleState::Created,
                Some(&ledger_projection),
                Some(&context_manifest_projection),
                Some(&privacy_inspector_projection),
                Some(&permission_budget_projection),
                Some(&checkpoint_rollback_projection),
                true,
                Some(event_context.causality_id),
                generated_at,
                1,
            );
        let provider_capability = phase4_provider_capability(provider_class, None);
        let request_contract = assisted_ai_request_contract_from_metadata(
            output.request_id.clone(),
            &provider_capability,
            operation_class,
            &context_manifest_projection,
            &privacy_inspector_projection,
            &permission_budget_projection,
            &approval_checklist_projection,
            Some(&checkpoint_rollback_projection),
            event_context,
            provider_route_request.proposal_intent.clone(),
            route_response.route_decision.clone(),
            generated_at,
        );
        let assisted_ai_projection = devil_protocol::assisted_ai_projection_from_metadata(
            format!("phase4:assisted:{}", run_id.0),
            vec![provider_capability],
            vec![request_contract],
            vec![output.clone()],
            Some(&ledger_projection),
            Some(&context_manifest_projection),
            Some(&privacy_inspector_projection),
            Some(&permission_budget_projection),
            Some(&approval_checklist_projection),
            Some(&checkpoint_rollback_projection),
            generated_at,
            1,
        );

        agent
            .transition(
                devil_protocol::AgentRunState::WaitingForApproval,
                "agent.waiting_for_approval.proposal_registered",
                event_context.correlation_id,
                event_context.causality_id,
                self.event_sequence_generator.next(),
            )
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        let replay_manifest = devil_protocol::AgentReplayManifest {
            run_id: run_id.clone(),
            transitions: agent.transitions().to_vec(),
            context_manifests: vec![trust_reference(
                &context_manifest_projection.manifest.manifest_id,
                devil_protocol::AssistedAiTrustProjectionKind::ContextManifest,
            )],
            provider_route_ids: vec![route_id.clone()],
            proposal_ids: vec![proposal_id],
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            event_sequence: self.event_sequence_generator.next(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        self.persist_phase4_runtime_records(
            &run_id,
            &route_id,
            route_response.invocation_state,
            "phase4.provider.route.completed",
            event_context,
            &replay_manifest,
        )?;
        self.tracker_ledger
            .append(TrackerRunLedgerRecord {
                run_id: run_id.clone(),
                state: devil_protocol::AgentRunState::WaitingForApproval,
                proposal_id: Some(proposal_id),
                transitions: replay_manifest.transitions.clone(),
                correlation_id: event_context.correlation_id,
                causality_id: event_context.causality_id,
                event_sequence: self.event_sequence_generator.next(),
                labels: vec!["tracker.phase4.run.waiting_for_approval".to_string()],
            })
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        let _ = self
            .memory_service
            .propose_candidate(MemoryCandidateRecord {
                candidate_id: format!("phase4-memory-candidate-{}", run_id.0),
                run_id: Some(run_id.clone()),
                consent: MemoryConsentState::NotGranted,
                labels: vec!["memory.candidate.review_required".to_string()],
                correlation_id: event_context.correlation_id,
                causality_id: event_context.causality_id,
                event_sequence: self.event_sequence_generator.next(),
            })
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;

        self.phase4_projection_state.context_manifest_projection =
            Some(context_manifest_projection.clone());
        self.phase4_projection_state.privacy_inspector_projection =
            Some(privacy_inspector_projection.clone());
        self.phase4_projection_state.permission_budget_projection =
            Some(permission_budget_projection.clone());
        self.phase4_projection_state.approval_checklist_projection =
            Some(approval_checklist_projection);
        self.phase4_projection_state.checkpoint_rollback_projection =
            Some(checkpoint_rollback_projection);
        self.phase4_projection_state.assisted_ai_projection = Some(assisted_ai_projection.clone());
        self.phase4_projection_state
            .replay_manifests
            .insert(run_id.clone(), replay_manifest.clone());
        self.phase4_projection_state.inspection_snapshots.insert(
            run_id.clone(),
            AppAiInspectionSnapshot {
                run_id: run_id.clone(),
                context_manifest_projection: context_manifest_projection.clone(),
                privacy_inspector_projection: privacy_inspector_projection.clone(),
                permission_budget_projection: permission_budget_projection.clone(),
                assisted_ai_projection: assisted_ai_projection.clone(),
            },
        );

        Ok(AppAiRunOutcome {
            run_id,
            proposal_id: Some(proposal_id),
            proposal_created: Some(proposal_created),
            route_response,
            context_manifest_projection,
            privacy_inspector_projection,
            permission_budget_projection,
            refusal: None,
            replay_manifest,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_assisted_ai_metadata_only_run(
        &mut self,
        run_id: devil_protocol::AgentRunId,
        route_id: String,
        operation_class: devil_protocol::AssistedAiOperationClass,
        provider_class: devil_protocol::AssistedAiProviderClass,
        provider_route_request: devil_protocol::AssistedAiProviderRouteRequest,
        route_response: devil_protocol::AssistedAiProviderRouteResponse,
        context_manifest_projection: devil_protocol::ContextManifestProjection,
        privacy_inspector_projection: devil_protocol::PrivacyInspectorProjection,
        permission_budget_projection: devil_protocol::PermissionBudgetProjection,
        generated_at: TimestampMillis,
        event_context: EventContext,
        agent: &mut AgentRuntime,
    ) -> Result<AppAiRunOutcome, AppCompositionError> {
        let refused = route_response.invocation_state
            != devil_protocol::AssistedAiProviderInvocationState::Completed;
        let agent_state = if refused {
            devil_protocol::AgentRunState::Blocked
        } else {
            devil_protocol::AgentRunState::Proposing
        };
        let outcome_label = if refused {
            "phase5.provider.route.refused"
        } else {
            "phase5.explain.metadata_ready"
        };
        agent
            .transition(
                agent_state,
                outcome_label,
                event_context.correlation_id,
                event_context.causality_id,
                self.event_sequence_generator.next(),
            )
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;

        let provider_capability =
            phase4_provider_capability(provider_class, route_response.refusal.clone());
        let approval_checklist_projection = empty_approval_checklist_projection();
        let checkpoint_rollback_projection = empty_checkpoint_rollback_projection();
        let request_contract = assisted_ai_request_contract_from_metadata(
            format!("phase5-request-{}", event_context.correlation_id.0),
            &provider_capability,
            operation_class,
            &context_manifest_projection,
            &privacy_inspector_projection,
            &permission_budget_projection,
            &approval_checklist_projection,
            Some(&checkpoint_rollback_projection),
            event_context,
            provider_route_request.proposal_intent.clone(),
            route_response.route_decision.clone(),
            generated_at,
        );
        let assisted_ai_projection = devil_protocol::assisted_ai_projection_from_metadata(
            format!("phase5:assisted:{}", run_id.0),
            vec![provider_capability],
            vec![request_contract],
            Vec::new(),
            None,
            Some(&context_manifest_projection),
            Some(&privacy_inspector_projection),
            Some(&permission_budget_projection),
            Some(&approval_checklist_projection),
            Some(&checkpoint_rollback_projection),
            generated_at,
            1,
        );
        let replay_manifest = devil_protocol::AgentReplayManifest {
            run_id: run_id.clone(),
            transitions: agent.transitions().to_vec(),
            context_manifests: vec![trust_reference(
                &context_manifest_projection.manifest.manifest_id,
                devil_protocol::AssistedAiTrustProjectionKind::ContextManifest,
            )],
            provider_route_ids: vec![route_id.clone()],
            proposal_ids: Vec::new(),
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            event_sequence: self.event_sequence_generator.next(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        self.persist_phase4_runtime_records(
            &run_id,
            &route_id,
            route_response.invocation_state,
            outcome_label,
            event_context,
            &replay_manifest,
        )?;
        self.tracker_ledger
            .append(TrackerRunLedgerRecord {
                run_id: run_id.clone(),
                state: agent_state,
                proposal_id: None,
                transitions: replay_manifest.transitions.clone(),
                correlation_id: event_context.correlation_id,
                causality_id: event_context.causality_id,
                event_sequence: self.event_sequence_generator.next(),
                labels: vec![outcome_label.to_string()],
            })
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;

        self.phase4_projection_state.context_manifest_projection =
            Some(context_manifest_projection.clone());
        self.phase4_projection_state.privacy_inspector_projection =
            Some(privacy_inspector_projection.clone());
        self.phase4_projection_state.permission_budget_projection =
            Some(permission_budget_projection.clone());
        self.phase4_projection_state.approval_checklist_projection =
            Some(approval_checklist_projection);
        self.phase4_projection_state.checkpoint_rollback_projection =
            Some(checkpoint_rollback_projection);
        self.phase4_projection_state.assisted_ai_projection = Some(assisted_ai_projection.clone());
        self.phase4_projection_state
            .replay_manifests
            .insert(run_id.clone(), replay_manifest.clone());
        self.phase4_projection_state.inspection_snapshots.insert(
            run_id.clone(),
            AppAiInspectionSnapshot {
                run_id: run_id.clone(),
                context_manifest_projection: context_manifest_projection.clone(),
                privacy_inspector_projection: privacy_inspector_projection.clone(),
                permission_budget_projection: permission_budget_projection.clone(),
                assisted_ai_projection: assisted_ai_projection.clone(),
            },
        );

        Ok(AppAiRunOutcome {
            run_id,
            proposal_id: None,
            proposal_created: None,
            refusal: route_response.refusal.clone(),
            route_response,
            context_manifest_projection,
            privacy_inspector_projection,
            permission_budget_projection,
            replay_manifest,
        })
    }

    /// Cancel a Phase 4 run by writing metadata-only cancellation audit.
    pub fn cancel_ai_run(
        &mut self,
        run_id: devil_protocol::AgentRunId,
    ) -> Result<(), AppCompositionError> {
        if !self
            .phase4_projection_state
            .replay_manifests
            .contains_key(&run_id)
        {
            return Err(AppCompositionError::AiRunMissing { run_id: run_id.0 });
        }
        let event_context = self.next_event_context();
        let record = devil_protocol::Phase4RuntimeAuditRecord {
            audit_id: format!("phase4-cancel:{}", run_id.0),
            run_id: Some(run_id),
            step_id: None,
            provider_route_id: None,
            invocation_state: devil_protocol::AssistedAiProviderInvocationState::Cancelled,
            outcome_label: "phase4.agent.cancelled".to_string(),
            labels: vec!["agent.cancelled.metadata_only".to_string()],
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            event_sequence: self.event_sequence_generator.next(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        self.storage
            .handle(StorageRepositoryRequest::SavePhase4RuntimeAuditRecord(
                record.clone(),
            ))
            .map_err(AppCompositionError::Protocol)?;
        let envelope = phase4_runtime_audit_recorded_event(&record)
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        self.emit_event(envelope);
        Ok(())
    }

    /// Replay a Phase 4 run from metadata-only storage.
    pub fn replay_ai_run(
        &self,
        run_id: devil_protocol::AgentRunId,
    ) -> Result<devil_protocol::AgentReplayManifest, AppCompositionError> {
        match self
            .storage
            .handle(StorageRepositoryRequest::ReadAgentReplayManifest(
                run_id.clone(),
            ))
            .map_err(AppCompositionError::Protocol)?
        {
            StorageRepositoryResponse::AgentReplayManifest(manifest) => manifest
                .as_ref()
                .clone()
                .ok_or(AppCompositionError::AiRunMissing { run_id: run_id.0 }),
            other => Err(AppCompositionError::Protocol(ProtocolError {
                code: "phase4_replay_unexpected_response".to_string(),
                message: format!("expected agent replay manifest, got {other:?}"),
            })),
        }
    }

    /// Inspect Phase 4 projections for a specific run's metadata.
    pub fn inspect_ai_run(
        &self,
        run_id: devil_protocol::AgentRunId,
    ) -> Result<AppAiInspectionSnapshot, AppCompositionError> {
        self.phase4_projection_state
            .inspection_snapshots
            .get(&run_id)
            .cloned()
            .ok_or(AppCompositionError::AiRunMissing { run_id: run_id.0 })
    }

    fn persist_phase4_runtime_records(
        &self,
        run_id: &devil_protocol::AgentRunId,
        route_id: &str,
        invocation_state: devil_protocol::AssistedAiProviderInvocationState,
        outcome_label: &str,
        event_context: EventContext,
        replay_manifest: &devil_protocol::AgentReplayManifest,
    ) -> Result<(), AppCompositionError> {
        let record = devil_protocol::Phase4RuntimeAuditRecord {
            audit_id: format!("phase4-runtime:{}:{}", run_id.0, route_id),
            run_id: Some(run_id.clone()),
            step_id: None,
            provider_route_id: Some(route_id.to_string()),
            invocation_state,
            outcome_label: outcome_label.to_string(),
            labels: vec!["phase4.runtime.metadata_only".to_string()],
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            event_sequence: replay_manifest.event_sequence,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        self.storage
            .handle(StorageRepositoryRequest::SavePhase4RuntimeAuditRecord(
                record.clone(),
            ))
            .map_err(AppCompositionError::Protocol)?;
        self.storage
            .handle(StorageRepositoryRequest::SaveAgentReplayManifest(
                replay_manifest.clone(),
            ))
            .map_err(AppCompositionError::Protocol)?;
        let runtime_event = phase4_runtime_audit_recorded_event(&record)
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        self.emit_event(runtime_event);
        let replay_event = agent_replay_manifest_recorded_event(replay_manifest)
            .map_err(|error| AppCompositionError::AiRuntime(error.to_string()))?;
        self.emit_event(replay_event);
        Ok(())
    }

    /// Save currently active buffer through editor save request and workspace write authority.
    pub fn save_active_buffer(&mut self) -> Result<AppSaveOutcome, AppCompositionError> {
        let buffer_id = self.active_documents.require_active_buffer()?;
        self.save_buffer(buffer_id)
    }

    fn save_buffer(&mut self, buffer_id: BufferId) -> Result<AppSaveOutcome, AppCompositionError> {
        let context = self.active_documents.save_context_for_buffer(buffer_id)?;
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
                self.active_documents
                    .bind_saved_buffer(output.save.buffer_id, output.applied);
                self.active_documents
                    .clear_dirty_prompt_for(output.save.buffer_id);
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

    /// Save all open tabs sequentially through the existing proposal-mediated save workflow.
    pub fn save_all(&mut self) -> Result<AppSaveAllOutcome, AppCompositionError> {
        let buffers = self.active_documents.open_tabs.clone();
        let mut results = Vec::new();
        let mut saved_count = 0;
        let mut rejected_count = 0;
        for buffer_id in buffers {
            let Some(metadata) = self
                .active_documents
                .metadata_for_buffer(buffer_id)
                .cloned()
            else {
                rejected_count += 1;
                results.push(AppSaveAllItemOutcome {
                    buffer_id,
                    file_id: None,
                    file_path: None,
                    status: AppSaveAllItemStatus::MetadataMissing,
                    outcome: None,
                    rejection_metadata: Some(save_all_missing_metadata(buffer_id)),
                    final_dirty: self.editor.is_dirty(buffer_id).unwrap_or(true),
                });
                continue;
            };
            if !self.active_documents.open_tabs.contains(&buffer_id) {
                rejected_count += 1;
                results.push(AppSaveAllItemOutcome {
                    buffer_id,
                    file_id: Some(metadata.identity.file_id),
                    file_path: Some(metadata.identity.canonical_path),
                    status: AppSaveAllItemStatus::MetadataMissing,
                    outcome: None,
                    rejection_metadata: Some(save_all_missing_metadata(buffer_id)),
                    final_dirty: self.editor.is_dirty(buffer_id).unwrap_or(true),
                });
                continue;
            }
            let file_id = Some(metadata.identity.file_id);
            let file_path = Some(metadata.identity.canonical_path);
            let outcome = self.save_buffer(buffer_id)?;
            let status = match &outcome {
                AppSaveOutcome::Saved(_) => AppSaveAllItemStatus::Saved,
                AppSaveOutcome::Rejected(_) => AppSaveAllItemStatus::Rejected,
            };
            let rejection_metadata = match &outcome {
                AppSaveOutcome::Saved(_) => None,
                AppSaveOutcome::Rejected(response) => Some(save_all_rejection_metadata(response)),
            };
            match &outcome {
                AppSaveOutcome::Saved(_) => saved_count += 1,
                AppSaveOutcome::Rejected(_) => rejected_count += 1,
            }
            results.push(AppSaveAllItemOutcome {
                buffer_id,
                file_id,
                file_path,
                status,
                outcome: Some(outcome),
                rejection_metadata,
                final_dirty: self.editor.is_dirty(buffer_id)?,
            });
        }

        let status = match (saved_count, rejected_count, results.is_empty()) {
            (_, _, true) => AppSaveAllStatus::Noop,
            (_, 0, false) => AppSaveAllStatus::Saved,
            (0, _, false) => AppSaveAllStatus::Rejected,
            _ => AppSaveAllStatus::Partial,
        };

        Ok(AppSaveAllOutcome {
            status,
            results,
            saved_count,
            rejected_count,
        })
    }

    /// Cancel the active dirty-close prompt without changing buffer text or tab state.
    pub fn cancel_dirty_close(&mut self, buffer_id: BufferId) -> Result<(), AppCompositionError> {
        self.active_documents.cancel_dirty_close(buffer_id)
    }

    /// Switch the active tab to an already-open buffer.
    pub fn switch_tab(&mut self, buffer_id: BufferId) -> Result<(), AppCompositionError> {
        self.active_documents.switch_to_buffer(buffer_id)
    }

    /// Close a tab when clean; dirty tabs produce a prompt projection and remain open.
    pub fn close_tab(
        &mut self,
        buffer_id: BufferId,
    ) -> Result<AppCloseTabOutcome, AppCompositionError> {
        self.active_documents.require_open_buffer(buffer_id)?;
        if self.editor.is_dirty(buffer_id)? {
            self.active_documents.prompt_dirty_close(buffer_id)?;
            let prompt = self
                .active_documents
                .close_dirty_prompt
                .as_ref()
                .ok_or(AppCompositionError::BufferNotOpen(buffer_id))?;
            return Ok(AppCloseTabOutcome::CloseDirtyPrompt {
                buffer_id,
                file_id: prompt.file_id,
                path: prompt.path.clone(),
            });
        }

        self.editor.close_buffer(buffer_id)?;
        self.active_documents.remove_open_tab(buffer_id);
        self.active_documents.activate_first_available_tab();
        Ok(AppCloseTabOutcome::Closed { buffer_id })
    }

    /// Set the active cursor for an open buffer through editor authority.
    pub fn set_buffer_cursor(
        &mut self,
        buffer_id: BufferId,
        cursor: TextCoordinate,
    ) -> Result<(), AppCompositionError> {
        self.active_documents.require_open_buffer(buffer_id)?;
        self.editor.set_cursors(
            buffer_id,
            vec![Cursor {
                position: CommandDispatcher::editor_position(cursor),
            }],
        )?;
        Ok(())
    }

    /// Set the active selection for an open buffer through editor authority.
    pub fn set_buffer_selection(
        &mut self,
        buffer_id: BufferId,
        range: devil_protocol::ProtocolTextRange,
    ) -> Result<(), AppCompositionError> {
        self.active_documents.require_open_buffer(buffer_id)?;
        self.editor.set_selections(
            buffer_id,
            vec![Selection {
                range: CommandDispatcher::editor_range(range),
            }],
        )?;
        Ok(())
    }

    /// Store viewport scroll for an open buffer.
    pub fn set_viewport_scroll(
        &mut self,
        buffer_id: BufferId,
        scroll: ViewportScroll,
    ) -> Result<(), AppCompositionError> {
        self.active_documents.set_viewport_scroll(buffer_id, scroll)
    }

    /// Run bounded lexical search through app-owned editor/workspace authority.
    pub fn run_search(
        &mut self,
        query_id: String,
        scope: SearchScopeProjection,
        query: String,
        limit: usize,
    ) -> Result<SearchProjection, AppCompositionError> {
        let result_limit = normalize_search_limit(limit);
        let query_label = query.trim().to_string();
        if query_label.is_empty() {
            self.search_projection = build_search_projection(
                Some(query_id),
                scope,
                query_label,
                result_limit,
                SearchStatusProjection {
                    kind: SearchStatusKindProjection::ValidationError,
                    message: "Search query is empty".to_string(),
                },
                SearchBuildResult::default(),
            );
            return Ok(self.search_projection.clone());
        }

        let result = match scope {
            SearchScopeProjection::ActiveFile => {
                self.run_active_file_search(&query_id, &query_label, result_limit)?
            }
            SearchScopeProjection::Workspace => {
                self.run_workspace_search(&query_id, &query_label, result_limit)?
            }
        };

        let status = search_status_for_result(scope, &result);
        self.search_projection = build_search_projection(
            Some(query_id),
            scope,
            query_label,
            result_limit,
            status,
            result,
        );
        Ok(self.search_projection.clone())
    }

    /// Cancel the projected search by query id.
    pub fn cancel_search(&mut self, query_id: String) -> SearchProjection {
        if self.search_projection.query_id.as_deref() == Some(query_id.as_str()) {
            self.search_projection.status = SearchStatusProjection {
                kind: SearchStatusKindProjection::Cancelled,
                message: "Search cancelled".to_string(),
            };
            self.search_projection.generated_at = TimestampMillis::now();
        }
        self.search_projection.clone()
    }

    /// Enable the deterministic terminal fixture for app integration tests.
    pub fn enable_terminal_fixture_for_tests(&mut self) {
        self.terminal_workflow.enable_fixture();
    }

    /// Return the current app-owned language tooling projection.
    pub fn language_tooling_projection(&self) -> LanguageToolingProjection {
        self.language_tooling.projection()
    }

    /// Return the current app-owned terminal panel projection.
    pub fn terminal_panel_projection(&self) -> TerminalPanelProjection {
        self.terminal_workflow.projection()
    }

    fn language_request_input(
        &mut self,
        buffer_id: BufferId,
        event_context: EventContext,
    ) -> Result<LanguageRequestInput, AppCompositionError> {
        self.active_documents.ensure_active_buffer(buffer_id)?;
        let metadata = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
            .ok_or(AppCompositionError::ActiveFileMissing)?;
        let snapshot_id = self.editor.current_snapshot(buffer_id)?.snapshot_id;
        let buffer_version = self.editor.buffer_version(buffer_id)?;
        let text = self.editor.text(buffer_id)?.to_string();
        let principal = self
            .active_documents
            .active_principal_id
            .clone()
            .ok_or(AppCompositionError::WorkspaceNotOpen)?;
        Ok(LanguageRequestInput {
            workspace_id: metadata.identity.workspace_id,
            buffer_id,
            metadata,
            principal,
            text,
            snapshot_id,
            buffer_version,
            event_context,
        })
    }

    fn run_language_read(
        &mut self,
        buffer_id: BufferId,
        kind: LanguageReadKind,
        position: TextCoordinate,
    ) -> Result<LanguageToolingProjection, AppCompositionError> {
        let event_context = self.next_event_context();
        let input = self.language_request_input(buffer_id, event_context)?;
        Ok(self.language_tooling.run_read(input, kind, position))
    }

    fn run_language_proposal(
        &mut self,
        buffer_id: BufferId,
        kind: LanguageProposalKind,
        position: TextCoordinate,
        label: String,
    ) -> Result<LanguageToolingProjection, AppCompositionError> {
        let event_context = self.next_event_context();
        let input = self.language_request_input(buffer_id, event_context)?;
        let proposal_id = self.proposal_coordinator.next_id();
        let capability = CapabilityId("fs.write".to_string());
        let preconditions = ProposalVersionPreconditions {
            file_version: Some(input.metadata.file_content_version),
            buffer_version: Some(input.buffer_version),
            snapshot_id: Some(input.snapshot_id),
            generation: Some(input.metadata.workspace_generation),
            file_content_version: Some(input.metadata.file_content_version),
            workspace_generation: Some(input.metadata.workspace_generation),
            expected_fingerprint: Some(input.metadata.fingerprint.clone()),
            expected_file_length: input.metadata.file_length,
            expected_modified_at: input.metadata.modified_at,
        };
        let byte = position.byte_offset.unwrap_or(0);
        let source = match kind {
            LanguageProposalKind::Formatting => WorkspaceEditSourceKind::LspFormatting,
            LanguageProposalKind::Rename => WorkspaceEditSourceKind::LspRename,
            LanguageProposalKind::OrganizeImports | LanguageProposalKind::CodeAction => {
                WorkspaceEditSourceKind::LspCodeAction
            }
        };
        let title = match kind {
            LanguageProposalKind::Formatting => "Format active buffer".to_string(),
            LanguageProposalKind::Rename => {
                format!("Rename symbol to {}", bounded_label(&label, 64))
            }
            LanguageProposalKind::OrganizeImports => "Organize imports".to_string(),
            LanguageProposalKind::CodeAction => {
                format!("Apply code action {}", bounded_label(&label, 64))
            }
        };
        let mut diagnostics = Vec::new();
        let (edit_range, replacement) = match kind {
            LanguageProposalKind::Rename => {
                let replacement = bounded_label(&label, 128);
                if replacement.trim().is_empty() {
                    return Ok(self.language_tooling.record_proposal_failure(
                        &input,
                        kind,
                        "Rename proposal requires a non-empty replacement label".to_string(),
                    ));
                }
                let Some(range) = identifier_byte_range_at(&input.text, byte) else {
                    return Ok(self.language_tooling.record_proposal_failure(
                        &input,
                        kind,
                        "Rename proposal requires an identifier at the requested position"
                            .to_string(),
                    ));
                };
                (range, replacement)
            }
            LanguageProposalKind::Formatting
            | LanguageProposalKind::OrganizeImports
            | LanguageProposalKind::CodeAction => {
                diagnostics.push(ProtocolDiagnostic {
                    code: "language_tooling.runtime_edit_unavailable".to_string(),
                    message: format!(
                        "{title} is represented as a safe no-op preview until live LSP edits are wired"
                    ),
                    severity: ProtocolDiagnosticSeverity::Warning,
                    path: Some(input.metadata.identity.canonical_path.clone()),
                    range: None,
                });
                (
                    ByteRange::new(0, input.text.len() as u64),
                    input.text.clone(),
                )
            }
        };
        let target = ProposalAffectedTarget {
            target_id: format!("file:{}", input.metadata.identity.file_id.0),
            kind: ProposalTargetKind::OpenBuffer,
            workspace_id: Some(input.workspace_id),
            file_id: Some(input.metadata.identity.file_id),
            buffer_id: Some(buffer_id),
            path: Some(input.metadata.identity.canonical_path.clone()),
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: vec![edit_range],
            redaction_hints: vec![RedactionHint::MetadataOnly],
        };
        let workspace_edit = WorkspaceEditProposalPayload {
            workspace_id: input.workspace_id,
            edit_id: uuid::Uuid::now_v7(),
            title: title.clone(),
            source,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![target],
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            file_edits: vec![WorkspaceTextEdit {
                file: input.metadata.identity.clone(),
                buffer_id: Some(buffer_id),
                edits: EditBatch {
                    edits: vec![ProtocolWorkspaceTextEdit {
                        range: ProtocolEditTextRange::byte(edit_range.start, edit_range.end),
                        replacement,
                    }],
                },
                preconditions: preconditions.clone(),
            }],
            file_operations: Vec::new(),
            required_capability: capability.clone(),
            diagnostics: diagnostics.clone(),
            schema_version: 1,
        };
        let request = LspRequestCorrelation {
            request_id: devil_protocol::LspRequestId(uuid::Uuid::now_v7()),
            server_id: devil_protocol::LanguageServerId(1),
            workspace_id: input.workspace_id,
            file_id: Some(input.metadata.identity.file_id),
            snapshot_id: Some(input.snapshot_id),
            buffer_version: Some(input.buffer_version),
            correlation_id: input.event_context.correlation_id,
            causality_id: input.event_context.causality_id,
            cancellation_token: Some(CancellationTokenId(uuid::Uuid::now_v7())),
            privacy_scope: SemanticPrivacyScope::Workspace,
            issued_at: TimestampMillis::now(),
            schema_version: 1,
        };
        let proposal = devil_protocol::convert_lsp_edit_to_workspace_proposal(
            LspEditProposalConversionInput {
                proposal_id,
                principal: input.principal.clone(),
                capability,
                request,
                workspace_edit,
                preconditions,
                lifecycle_state: ProposalLifecycleState::Created,
                privacy_label: devil_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                preview: PreviewSummary {
                    summary: title.clone(),
                    details: vec![
                        "language_tooling.proposal_preview".to_string(),
                        format!("buffer_version={}", input.buffer_version.0),
                        format!("snapshot_id={}", input.snapshot_id.0),
                    ],
                },
                expires_at: None,
                created_at: TimestampMillis::now(),
                diagnostics,
                schema_version: 1,
            },
        )
        .map_err(|error| AppCompositionError::LanguageTooling(format!("{error:?}")))?;
        self.proposal_coordinator
            .register_lifecycle_context(proposal.proposal_id, input.event_context);
        let created = self.proposal_coordinator.created_response(&proposal);
        if !matches!(created, ProposalResponse::Created(_)) {
            return Ok(self.language_tooling.record_proposal_failure(
                &input,
                kind,
                format!("{} proposal creation failed: {created:?}", title),
            ));
        }
        let validated = self
            .proposal_coordinator
            .handle(ProposalRequest::Validate(proposal.clone()));
        if !matches!(validated, Ok(ProposalResponse::Validated(_))) {
            return Ok(self.language_tooling.record_proposal_failure(
                &input,
                kind,
                format!("{} proposal validation failed: {validated:?}", title),
            ));
        }
        let previewed = self
            .proposal_coordinator
            .handle(ProposalRequest::Preview(proposal.clone()));
        if !matches!(previewed, Ok(ProposalResponse::Previewed { .. })) {
            return Ok(self.language_tooling.record_proposal_failure(
                &input,
                kind,
                format!("{} proposal preview failed: {previewed:?}", title),
            ));
        }
        Ok(self.language_tooling.record_proposal(
            &input,
            kind,
            proposal.proposal_id,
            format!("{} proposal preview created", title),
        ))
    }

    fn run_active_file_search(
        &self,
        query_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<SearchBuildResult, AppCompositionError> {
        let buffer_id = self.active_documents.require_active_buffer()?;
        let metadata = self
            .active_documents
            .metadata_for_buffer(buffer_id)
            .cloned()
            .ok_or(AppCompositionError::ActiveFileMissing)?;

        if matches!(self.editor.buffer_mode(buffer_id)?, BufferMode::Degraded) {
            return self
                .run_degraded_active_file_search(query_id, query, limit, buffer_id, metadata);
        }

        let text = self.editor.text(buffer_id)?;
        let mut result = SearchBuildResult::default();
        collect_search_results_for_text(SearchTextInput {
            query_id,
            query,
            scope: SearchScopeProjection::ActiveFile,
            workspace_id: Some(metadata.identity.workspace_id),
            buffer_id: Some(buffer_id),
            file_id: Some(metadata.identity.file_id),
            file_path: Some(metadata.identity.canonical_path),
            text,
            limit,
            result: &mut result,
        });
        Ok(result)
    }

    fn run_degraded_active_file_search(
        &self,
        query_id: &str,
        query: &str,
        limit: usize,
        buffer_id: BufferId,
        metadata: ActiveFileMetadata,
    ) -> Result<SearchBuildResult, AppCompositionError> {
        let scroll = self.active_documents.viewport_scroll_for(buffer_id);
        let viewport = self
            .editor
            .viewport_projection(devil_protocol::EditorViewportRequest {
                buffer_id,
                scroll,
                dimensions: devil_protocol::ViewportDimensions {
                    width_px: 800,
                    height_px: 384,
                },
            })?;
        let mut result = SearchBuildResult {
            degraded_limited: true,
            diagnostics: vec![
                "Active-file search is limited to the visible viewport in degraded mode"
                    .to_string(),
            ],
            ..SearchBuildResult::default()
        };

        for slice in &viewport.line_slices {
            collect_search_results_for_line(SearchLineInput {
                query_id,
                query,
                scope: SearchScopeProjection::ActiveFile,
                workspace_id: Some(metadata.identity.workspace_id),
                buffer_id: Some(buffer_id),
                file_id: Some(metadata.identity.file_id),
                file_path: Some(metadata.identity.canonical_path.clone()),
                line_number: slice.line_number,
                line_text: &slice.visible_text,
                absolute_line_start: slice.byte_range.start,
                limit,
                result: &mut result,
            });
        }

        Ok(result)
    }

    fn run_workspace_search(
        &self,
        query_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<SearchBuildResult, AppCompositionError> {
        let workspace_id = self.active_documents.require_workspace_id()?;
        let tree = self.workspace.tree_snapshot()?;
        let mut result = SearchBuildResult::default();

        for node in tree {
            if !workspace_node_is_regular_file(&node) {
                continue;
            }
            let Some(size_bytes) = node
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.size_bytes)
            else {
                result.omitted_file_count += 1;
                result.diagnostics.push(format!(
                    "Skipped {} because file size metadata is unavailable",
                    node.identity.canonical_path.0
                ));
                continue;
            };
            if size_bytes > WORKSPACE_SEARCH_MAX_FILE_BYTES {
                result.omitted_file_count += 1;
                result.diagnostics.push(format!(
                    "Skipped {} because {} bytes exceeds the workspace search bound",
                    node.identity.canonical_path.0, size_bytes
                ));
                continue;
            }

            let text = match self
                .workspace
                .read_file_text(workspace_id, &node.identity.canonical_path.0)
            {
                Ok(text) => text,
                Err(error) => {
                    result.omitted_file_count += 1;
                    result.diagnostics.push(format!(
                        "Skipped {}: {error}",
                        node.identity.canonical_path.0
                    ));
                    continue;
                }
            };

            collect_search_results_for_text(SearchTextInput {
                query_id,
                query,
                scope: SearchScopeProjection::Workspace,
                workspace_id: Some(workspace_id),
                buffer_id: self
                    .editor
                    .buffer_for_file(workspace_id, node.identity.file_id),
                file_id: Some(node.identity.file_id),
                file_path: Some(node.identity.canonical_path),
                text: &text,
                limit,
                result: &mut result,
            });
        }

        Ok(result)
    }

    /// Capture a metadata-only workspace session record.
    pub fn capture_workspace_session_record(
        &self,
    ) -> Result<WorkspaceSessionRecord, AppCompositionError> {
        capture_workspace_session_record(&self.active_documents, &self.editor)
    }

    /// Restore open tabs from a metadata-only session record.
    pub fn restore_workspace_session_record(
        &mut self,
        record: &WorkspaceSessionRecord,
    ) -> Result<AppSessionRestoreOutcome, AppCompositionError> {
        let mut restored_file_ids = Vec::new();
        let mut skipped_tabs = Vec::new();
        for tab in &record.open_tabs {
            let Some(path) = tab.path.as_ref() else {
                skipped_tabs.push(AppSessionRestoreSkippedTab {
                    tab_id: tab.tab_id.clone(),
                    reason: "missing path".to_string(),
                });
                continue;
            };
            if !Path::new(&path.0).exists() {
                skipped_tabs.push(AppSessionRestoreSkippedTab {
                    tab_id: tab.tab_id.clone(),
                    reason: "path missing".to_string(),
                });
                continue;
            }
            match self.open_file(&path.0) {
                Ok(file_id) => restored_file_ids.push(file_id),
                Err(error) => skipped_tabs.push(AppSessionRestoreSkippedTab {
                    tab_id: tab.tab_id.clone(),
                    reason: error.to_string(),
                }),
            }
        }

        if let Some(active_tab) = &record.active_tab
            && let Some(tab) = record
                .open_tabs
                .iter()
                .find(|candidate| &candidate.tab_id == active_tab)
            && let Some(path) = &tab.path
            && let Some(workspace_id) = self.active_documents.workspace_id()
            && let Some(buffer_id) = self.editor.buffer_for_path(workspace_id, &path.0)
        {
            let _ = self.switch_tab(buffer_id);
        }

        Ok(AppSessionRestoreOutcome {
            restored_file_ids,
            skipped_tabs,
        })
    }

    /// Build active-buffer projection from editor-engine state.
    pub fn active_buffer_projection(
        &self,
        layout: &ShellLayoutProjection,
    ) -> Result<ActiveBufferProjection, AppCompositionError> {
        ProjectionBuilder::active_buffer_projection(&self.active_documents, &self.editor, layout)
    }

    fn selected_proposal_trust_projections(
        &self,
        proposal_ledger_projection: &devil_protocol::ProposalLedgerProjection,
        generated_at: TimestampMillis,
    ) -> Option<SelectedProposalTrustProjections> {
        let selected_proposal_id = proposal_ledger_projection.selected_proposal_id?;
        let proposal = self
            .proposal_coordinator
            .proposal_for_id(selected_proposal_id)?;
        let row = proposal_ledger_projection
            .rows
            .iter()
            .find(|row| row.proposal_id == selected_proposal_id)?;
        let lifecycle_state = row.lifecycle.state;
        let context_manifest_projection = devil_protocol::ContextManifestProjection {
            manifest: devil_protocol::context_manifest_from_proposal(
                &proposal,
                format!("proposal:{}:context-details", selected_proposal_id.0),
                self.active_documents.active_workspace_trust.clone(),
                row.privacy_label,
                row.risk_label,
                generated_at,
                1,
            ),
            selected_item_id: None,
            generated_at,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let privacy_inspector_projection =
            devil_protocol::privacy_inspector_from_context_manifest_projection(
                &context_manifest_projection,
                format!("proposal:{}:privacy-details", selected_proposal_id.0),
                generated_at,
                1,
            );
        let permission_budget_projection = Self::selected_proposal_permission_budget_projection(
            &proposal,
            &context_manifest_projection,
            row.risk_label,
            generated_at,
        );
        let causality_id = self
            .proposal_coordinator
            .proposal_contexts
            .borrow()
            .get(&selected_proposal_id)
            .map(|context| context.causality_id);
        let audit_status =
            if row.rollback == devil_protocol::ProposalRollbackAvailability::NotRequired {
                devil_protocol::CheckpointRollbackAuditStatus::NotRequired
            } else {
                devil_protocol::CheckpointRollbackAuditStatus::Available
            };
        let checkpoint_rollback_projection =
            devil_protocol::checkpoint_rollback_projection_from_proposal(
                format!("proposal:{}:checkpoint-details", selected_proposal_id.0),
                &proposal,
                lifecycle_state,
                Some(proposal_ledger_projection),
                audit_status,
                causality_id,
                generated_at,
                1,
            );
        let approval_checklist_projection =
            devil_protocol::approval_checklist_from_trust_projections(
                format!("proposal:{}:approval-details", selected_proposal_id.0),
                &proposal,
                lifecycle_state,
                Some(proposal_ledger_projection),
                Some(&context_manifest_projection),
                Some(&privacy_inspector_projection),
                Some(&permission_budget_projection),
                Some(&checkpoint_rollback_projection),
                true,
                causality_id,
                generated_at,
                1,
            );

        Some(SelectedProposalTrustProjections {
            context_manifest_projection,
            privacy_inspector_projection,
            permission_budget_projection,
            approval_checklist_projection,
            checkpoint_rollback_projection,
        })
    }

    fn selected_proposal_permission_budget_projection(
        proposal: &WorkspaceProposal,
        context_manifest_projection: &devil_protocol::ContextManifestProjection,
        risk_label: devil_protocol::ProposalRiskLabel,
        generated_at: TimestampMillis,
    ) -> devil_protocol::PermissionBudgetProjection {
        let action_class = match &proposal.payload {
            ProposalPayload::TerminalCommand(_) => {
                devil_protocol::PermissionBudgetActionClass::AccessTerminal
            }
            _ => devil_protocol::PermissionBudgetActionClass::ApplyApprovedProposal,
        };
        let budget = devil_protocol::PermissionBudgetContract {
            budget_id: format!("proposal:{}:permission-budget", proposal.proposal_id.0),
            action_class,
            capability: Some(proposal.capability.clone()),
            state: devil_protocol::PermissionBudgetState::Allowed,
            privacy_scope: SemanticPrivacyScope::MetadataOnly,
            usage: devil_protocol::PermissionBudgetUsageSummary {
                unit_label: "actions".to_string(),
                used: 0,
                ceiling: Some(1),
                remaining: Some(1),
                attempted: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            reset_policy_label: devil_protocol::PermissionBudgetResetPolicyLabel::Session,
            consent_requirement_label:
                devil_protocol::PermissionBudgetConsentRequirementLabel::NotRequired,
            risk_label,
            reasons: vec!["proposal.permission_budget.metadata_only".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let action = context_manifest_projection
            .manifest
            .permissions
            .first()
            .map(|permission| {
                devil_protocol::permission_budget_action_from_permission_summary(
                    permission,
                    format!("proposal:{}:permission-action", proposal.proposal_id.0),
                    action_class,
                    context_manifest_projection.manifest.workspace_id,
                    Some(proposal.proposal_id),
                    1,
                )
            })
            .unwrap_or_else(|| devil_protocol::PermissionBudgetActionSummary {
                action_id: format!("proposal:{}:permission-action", proposal.proposal_id.0),
                action_class,
                capability: Some(proposal.capability.clone()),
                workspace_id: context_manifest_projection.manifest.workspace_id,
                proposal_id: Some(proposal.proposal_id),
                target_id: None,
                privacy_scope: SemanticPrivacyScope::MetadataOnly,
                egress: devil_protocol::ContextManifestEgressStatus::LocalOnly,
                estimated_units: 1,
                ranges: Vec::new(),
                counts: Vec::new(),
                hashes: Vec::new(),
                labels: vec!["proposal.permission.action".to_string()],
                risk_label,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            });
        let evaluation = devil_protocol::evaluate_permission_budget(
            &budget,
            action,
            format!("proposal:{}:permission-evaluation", proposal.proposal_id.0),
            1,
        );
        devil_protocol::permission_budget_projection_from_contracts(
            format!(
                "proposal:{}:permission-budget-projection",
                proposal.proposal_id.0
            ),
            vec![budget],
            vec![evaluation],
            generated_at,
            1,
        )
    }

    /// Build the complete projection snapshot consumed by the UI shell.
    pub fn shell_projection_snapshot(
        &self,
        title: impl Into<String>,
    ) -> Result<ShellProjectionSnapshot, AppCompositionError> {
        let layout_projection = ShellLayoutProjection::plain(title);
        let generated_at = TimestampMillis::now();
        let proposal_ledger_projection = self
            .proposal_coordinator
            .proposal_ledger_projection(generated_at);
        let selected_proposal_trust =
            self.selected_proposal_trust_projections(&proposal_ledger_projection, generated_at);
        Ok(ShellProjectionSnapshot {
            active_buffer_projection: self.active_buffer_projection(&layout_projection)?,
            layout_projection,
            explorer_projection: self.explorer_projection()?,
            status_messages: Vec::new(),
            proposal_ledger_projection,
            context_manifest_projection: selected_proposal_trust
                .as_ref()
                .map(|projections| projections.context_manifest_projection.clone())
                .or_else(|| {
                    self.phase4_projection_state
                        .context_manifest_projection
                        .clone()
                })
                .unwrap_or_else(empty_context_manifest_projection),
            privacy_inspector_projection: selected_proposal_trust
                .as_ref()
                .map(|projections| projections.privacy_inspector_projection.clone())
                .or_else(|| {
                    self.phase4_projection_state
                        .privacy_inspector_projection
                        .clone()
                })
                .unwrap_or_else(empty_privacy_inspector_projection),
            permission_budget_projection: selected_proposal_trust
                .as_ref()
                .map(|projections| projections.permission_budget_projection.clone())
                .or_else(|| {
                    self.phase4_projection_state
                        .permission_budget_projection
                        .clone()
                })
                .unwrap_or_else(empty_permission_budget_projection),
            approval_checklist_projection: selected_proposal_trust
                .as_ref()
                .map(|projections| projections.approval_checklist_projection.clone())
                .or_else(|| {
                    self.phase4_projection_state
                        .approval_checklist_projection
                        .clone()
                })
                .unwrap_or_else(empty_approval_checklist_projection),
            checkpoint_rollback_projection: selected_proposal_trust
                .as_ref()
                .map(|projections| projections.checkpoint_rollback_projection.clone())
                .or_else(|| {
                    self.phase4_projection_state
                        .checkpoint_rollback_projection
                        .clone()
                })
                .unwrap_or_else(empty_checkpoint_rollback_projection),
            assisted_ai_projection: self
                .phase4_projection_state
                .assisted_ai_projection
                .clone()
                .unwrap_or_else(empty_assisted_ai_projection),
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
            plugin_contribution_projections: self.plugin_contribution_projections.clone(),
            collaboration_presence_projections: self.collaboration.presence_projections(),
            daily_editing_projection: ProjectionBuilder::daily_editing_projection(
                &self.active_documents,
                &self.editor,
            ),
            search_projection: self.search_projection.clone(),
            language_tooling_projection: self.language_tooling.projection(),
            terminal_panel_projection: self.terminal_workflow.projection(),
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
            runtime_apply_disabled: false,
            atomicity: None,
            rollback_policy: None,
            planning_semantics: None,
            rollback_contract: None,
            items: Vec::new(),
            diagnostics: Vec::new(),
            preview_warnings: Vec::new(),
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

    /// Build a deterministic, non-mutating batch execution journal.
    ///
    /// Plan Phase 2: this exposes execution-ready state without permitting runtime batch mutation.
    /// It is intentionally derived from `plan_batch_execution_contract()` and does not call editor
    /// or workspace mutation helpers.
    pub fn plan_batch_execution_journal(
        &self,
        proposal: &WorkspaceProposal,
    ) -> BatchExecutionJournal {
        let contract = self.plan_batch_execution_contract(proposal);
        let mutation_allowed = contract.preflight.preflight_ok
            && !contract.runtime_apply_disabled
            && !contract.commit_blocked
            && !contract.finalize_blocked
            && contract.diagnostics.is_empty();

        let stages = contract
            .stages
            .iter()
            .map(|stage| BatchExecutionJournalStage {
                stage: stage.stage,
                state: if stage.blocked {
                    BatchExecutionJournalStageState::Blocked
                } else {
                    BatchExecutionJournalStageState::Ready
                },
                diagnostics: stage.diagnostics.clone(),
            })
            .collect::<Vec<_>>();

        let items = contract
            .items
            .iter()
            .map(|item| BatchExecutionJournalItem {
                item_id: item.item_id.clone(),
                order: item.order,
                route: item.route,
                target_ids: item.target_ids.clone(),
                state: Self::batch_journal_item_state(&contract, item),
                partial_failure_disposition: item.partial_failure_disposition,
                diagnostics: item.diagnostics.clone(),
            })
            .collect::<Vec<_>>();

        BatchExecutionJournal {
            proposal_id: contract.proposal_id,
            batch_id: contract.batch_id,
            mutation_allowed,
            runtime_apply_disabled: contract.runtime_apply_disabled,
            audit_before_success_required: contract.audit_before_success_required,
            stages,
            items,
            partial_failures: contract.partial_failures,
            diagnostics: contract.diagnostics,
            preview_warnings: contract.preview_warnings,
        }
    }

    fn batch_journal_item_state(
        contract: &BatchExecutionContract,
        item: &BatchExecutionItemContract,
    ) -> BatchExecutionJournalItemState {
        if item.partial_failure_disposition == Some(ProposalPartialFailureDisposition::NotStarted) {
            return BatchExecutionJournalItemState::DependencyBlocked;
        }
        if !item.preflight_ok || !item.diagnostics.is_empty() {
            return BatchExecutionJournalItemState::PreflightRejected;
        }
        if contract.runtime_apply_disabled {
            return BatchExecutionJournalItemState::RuntimeMutationDisabled;
        }
        BatchExecutionJournalItemState::Prepared
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

    /// Register a shared collaboration approval gate for an app-owned proposal.
    pub fn register_shared_collaboration_proposal(
        &mut self,
        session_id: CollaborationSessionId,
        proposal_id: ProposalId,
        required_approvers: Vec<CollaborationParticipantId>,
        authorized_approvers: Vec<CollaborationParticipantId>,
        applied_operation_ids: Vec<devil_protocol::CollaborationOperationId>,
    ) {
        self.collaboration.shared_proposals.insert(
            (session_id, proposal_id),
            SharedProposalGate {
                required_approvers: required_approvers.iter().copied().collect(),
                authorized_approvers: authorized_approvers.into_iter().collect(),
                approvals: HashMap::new(),
                denials: HashMap::new(),
                applied_operation_ids,
                stale: false,
            },
        );
    }

    /// Record an app-owned shared collaboration proposal approval or denial.
    pub fn record_shared_collaboration_approval(
        &mut self,
        approval: CollaborationSharedProposalApproval,
    ) -> Result<(), AppCompositionError> {
        if !approval.capability_decision.granted
            || approval.capability_decision.capability
                != CapabilityId("collaboration.proposal.approve".to_string())
        {
            return Err(AppCompositionError::Collaboration(
                "shared proposal approval lacks authorized capability".to_string(),
            ));
        }
        let gate = self
            .collaboration
            .shared_proposals
            .get_mut(&(approval.session_id, approval.proposal_id))
            .ok_or_else(|| {
                AppCompositionError::Collaboration("shared proposal gate is missing".to_string())
            })?;
        if !gate.authorized_approvers.contains(&approval.participant_id) {
            return Err(AppCompositionError::Collaboration(
                "participant is not authorized to approve shared proposal".to_string(),
            ));
        }
        match approval.disposition {
            CollaborationSharedProposalDisposition::Approved => {
                gate.approvals.insert(approval.participant_id, approval);
            }
            CollaborationSharedProposalDisposition::Denied => {
                gate.denials.insert(approval.participant_id, approval);
            }
            CollaborationSharedProposalDisposition::Superseded => {
                gate.stale = true;
            }
            CollaborationSharedProposalDisposition::Pending => {}
        }
        Ok(())
    }

    fn collaboration_session_for_proposal(
        proposal: &WorkspaceProposal,
    ) -> Option<CollaborationSessionId> {
        AppProposalCoordinator::affected_target_coverage(proposal)
            .targets
            .iter()
            .filter_map(|target| target.collaboration_session_id.as_deref())
            .find_map(|id| id.parse::<u128>().ok().map(CollaborationSessionId))
    }

    #[allow(clippy::result_large_err)]
    fn ensure_shared_collaboration_proposal_approved(
        &self,
        proposal: &WorkspaceProposal,
    ) -> Result<Option<CollaborationSessionId>, ProposalResponse> {
        let Some(session_id) = Self::collaboration_session_for_proposal(proposal) else {
            return Ok(None);
        };
        let Some(gate) = self
            .collaboration
            .shared_proposals
            .get(&(session_id, proposal.proposal_id))
        else {
            return Err(self.failed_apply_response(
                proposal,
                "proposal.shared_collaboration_approval_missing",
                "shared collaboration proposal requires app-owned approval evidence before apply",
            ));
        };
        if gate.stale {
            return Err(self.failed_apply_response(
                proposal,
                "proposal.shared_collaboration_approval_stale",
                "stale or superseded shared collaboration approval does not authorize apply",
            ));
        }
        if !gate.denials.is_empty() {
            return Err(self.failed_apply_response(
                proposal,
                "proposal.shared_collaboration_denied",
                "explicit shared collaboration denial blocks proposal apply",
            ));
        }
        if !gate
            .required_approvers
            .iter()
            .all(|participant| gate.approvals.contains_key(participant))
        {
            return Err(self.failed_apply_response(
                proposal,
                "proposal.shared_collaboration_quorum_missing",
                "shared collaboration proposal lacks required approvals",
            ));
        }
        Ok(Some(session_id))
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

        let shared_session_id = match self.ensure_shared_collaboration_proposal_approved(&proposal)
        {
            Ok(session_id) => session_id,
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
            ProposalPayload::Batch(payload) => self.apply_batch_proposal(&proposal, payload),
            ProposalPayload::CodeAction(payload) => {
                self.apply_code_action_proposal(&proposal, payload)
            }
            ProposalPayload::FormatFile(_) => self.failed_apply_response(
                &proposal,
                "proposal.format_requires_lowered_workspace_edit",
                "format-file apply requires a lowered TextEdit or WorkspaceEdit proposal payload",
            ),
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
            if let ProposalResponse::Applied(_) = &response
                && let Some(session_id) = shared_session_id
                && let Some(gate) = self
                    .collaboration
                    .shared_proposals
                    .get(&(session_id, proposal.proposal_id))
            {
                let operation_id = gate.applied_operation_ids.first().copied();
                if let Some(runtime) = self.collaboration.sessions.get(&session_id) {
                    let audit = runtime.audit_record(
                        operation_id,
                        Some(proposal.proposal_id),
                        self.event_sequence_generator.next(),
                        proposal.correlation_id,
                    );
                    self.persist_collaboration_audit(audit)?;
                }
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
            ProposalPayload::TextEdit(payload) => {
                let workspace_id =
                    self.active_documents
                        .require_workspace_id()
                        .map_err(|error| {
                            self.failed_apply_response(
                                proposal,
                                "proposal.workspace_missing",
                                format!(
                                    "apply requires an active workspace rollback authority: {error}"
                                ),
                            )
                        })?;
                Ok(ProposalMutationRollback::TextEdit {
                    workspace_id,
                    file_id: payload.file_id,
                })
            }
            ProposalPayload::CreateFile(payload) => self.workspace_rollback_checkpoint(
                proposal,
                WorkspaceMutationRollbackTarget::CreatedFile {
                    path: payload.path.clone(),
                },
            ),
            ProposalPayload::DeleteFile(payload) => self.workspace_rollback_checkpoint(
                proposal,
                WorkspaceMutationRollbackTarget::DeletedFile {
                    file: payload.file.clone(),
                },
            ),
            ProposalPayload::RenameFile(payload) => self.workspace_rollback_checkpoint(
                proposal,
                WorkspaceMutationRollbackTarget::RenamedFile {
                    file: payload.file.clone(),
                    destination: payload.destination.clone(),
                },
            ),
            ProposalPayload::SaveFile(payload) => self.workspace_rollback_checkpoint(
                proposal,
                WorkspaceMutationRollbackTarget::SavedFile {
                    file: payload.file.clone(),
                },
            ),
            ProposalPayload::WorkspaceEdit(payload) => {
                self.rollback_snapshot_for_workspace_edit(proposal, payload)
            }
            ProposalPayload::CodeAction(payload) => Ok(ProposalMutationRollback::TextEdit {
                workspace_id: payload.file.workspace_id,
                file_id: payload.file.file_id,
            }),
            ProposalPayload::Batch(payload) => self.rollback_snapshot_for_batch(proposal, payload),
            _ => Ok(ProposalMutationRollback::None),
        }
    }

    #[allow(clippy::result_large_err)]
    fn rollback_snapshot_for_workspace_edit(
        &self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::WorkspaceEditProposalPayload,
    ) -> Result<ProposalMutationRollback, ProposalResponse> {
        let mut rollbacks = Vec::new();
        for edit in &payload.file_edits {
            rollbacks.push(ProposalMutationRollback::TextEdit {
                workspace_id: edit.file.workspace_id,
                file_id: edit.file.file_id,
            });
        }
        for operation in &payload.file_operations {
            rollbacks
                .push(self.rollback_snapshot_for_workspace_file_operation(proposal, operation)?);
        }
        Ok(ProposalMutationRollback::Composite(rollbacks))
    }

    #[allow(clippy::result_large_err)]
    fn rollback_snapshot_for_workspace_file_operation(
        &self,
        proposal: &WorkspaceProposal,
        operation: &devil_protocol::WorkspaceFileOperation,
    ) -> Result<ProposalMutationRollback, ProposalResponse> {
        match operation {
            devil_protocol::WorkspaceFileOperation::Create { path, .. } => self
                .workspace_rollback_checkpoint(
                    proposal,
                    WorkspaceMutationRollbackTarget::CreatedFile { path: path.clone() },
                ),
            devil_protocol::WorkspaceFileOperation::Delete { file } => self
                .workspace_rollback_checkpoint(
                    proposal,
                    WorkspaceMutationRollbackTarget::DeletedFile { file: file.clone() },
                ),
            devil_protocol::WorkspaceFileOperation::Rename { file, destination } => self
                .workspace_rollback_checkpoint(
                    proposal,
                    WorkspaceMutationRollbackTarget::RenamedFile {
                        file: file.clone(),
                        destination: destination.clone(),
                    },
                ),
        }
    }

    #[allow(clippy::result_large_err)]
    fn rollback_snapshot_for_batch(
        &self,
        proposal: &WorkspaceProposal,
        payload: &BatchProposalPayload,
    ) -> Result<ProposalMutationRollback, ProposalResponse> {
        let mut rollbacks = Vec::new();
        for item in self.ordered_batch_items(payload) {
            let item_proposal = Self::batch_item_proposal(proposal, item);
            match item.payload.as_ref() {
                ProposalPayload::TextEdit(payload) => {
                    let workspace_id =
                        self.active_documents
                            .require_workspace_id()
                            .map_err(|error| {
                                self.failed_apply_response(
                                &item_proposal,
                                "proposal.workspace_missing",
                                format!(
                                    "batch text edit rollback requires an active workspace: {error}"
                                ),
                            )
                            })?;
                    rollbacks.push(ProposalMutationRollback::Scoped {
                        proposal: Box::new(item_proposal),
                        rollback: Box::new(ProposalMutationRollback::TextEdit {
                            workspace_id,
                            file_id: payload.file_id,
                        }),
                    });
                }
                ProposalPayload::CreateFile(payload) => {
                    let rollback = self.workspace_rollback_checkpoint(
                        &item_proposal,
                        WorkspaceMutationRollbackTarget::CreatedFile {
                            path: payload.path.clone(),
                        },
                    )?;
                    rollbacks.push(ProposalMutationRollback::Scoped {
                        proposal: Box::new(item_proposal),
                        rollback: Box::new(rollback),
                    });
                }
                ProposalPayload::DeleteFile(payload) => {
                    let rollback = self.workspace_rollback_checkpoint(
                        &item_proposal,
                        WorkspaceMutationRollbackTarget::DeletedFile {
                            file: payload.file.clone(),
                        },
                    )?;
                    rollbacks.push(ProposalMutationRollback::Scoped {
                        proposal: Box::new(item_proposal),
                        rollback: Box::new(rollback),
                    });
                }
                ProposalPayload::RenameFile(payload) => {
                    let rollback = self.workspace_rollback_checkpoint(
                        &item_proposal,
                        WorkspaceMutationRollbackTarget::RenamedFile {
                            file: payload.file.clone(),
                            destination: payload.destination.clone(),
                        },
                    )?;
                    rollbacks.push(ProposalMutationRollback::Scoped {
                        proposal: Box::new(item_proposal),
                        rollback: Box::new(rollback),
                    });
                }
                ProposalPayload::WorkspaceEdit(payload) => {
                    if let ProposalMutationRollback::Composite(items) =
                        self.rollback_snapshot_for_workspace_edit(&item_proposal, payload)?
                    {
                        rollbacks.push(ProposalMutationRollback::Scoped {
                            proposal: Box::new(item_proposal),
                            rollback: Box::new(ProposalMutationRollback::Composite(items)),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(ProposalMutationRollback::Composite(rollbacks))
    }

    #[allow(clippy::result_large_err)]
    fn workspace_rollback_checkpoint(
        &self,
        proposal: &WorkspaceProposal,
        target: WorkspaceMutationRollbackTarget,
    ) -> Result<ProposalMutationRollback, ProposalResponse> {
        let workspace_id = self
            .active_documents
            .require_workspace_id()
            .map_err(|error| {
                self.failed_apply_response(
                    proposal,
                    "proposal.workspace_missing",
                    format!("apply requires an active workspace rollback authority: {error}"),
                )
            })?;
        let request = WorkspaceMutationRollbackCheckpointRequest {
            workspace_id,
            proposal_id: proposal.proposal_id,
            principal: proposal.principal.clone(),
            required_capability: proposal.capability.clone(),
            target,
            correlation_id: proposal.correlation_id,
            causality_id: self.proposal_causality_id(proposal),
        };
        self.workspace
            .rollback_checkpoint_for_file_mutation(request)
            .map(ProposalMutationRollback::WorkspaceFile)
            .map_err(|response| match response {
                ProposalResponse::Failed { .. }
                | ProposalResponse::Denied { .. }
                | ProposalResponse::Stale { .. }
                | ProposalResponse::Conflict { .. } => response,
                other => self.failed_apply_response(
                    proposal,
                    "proposal.rollback_checkpoint_unavailable",
                    format!("apply requires a pre-mutation rollback checkpoint: {other:?}"),
                ),
            })
    }

    #[allow(clippy::result_large_err)]
    fn workspace_rollback_request(
        &self,
        proposal: &WorkspaceProposal,
        checkpoint: WorkspaceMutationRollbackCheckpoint,
    ) -> Result<WorkspaceMutationRollbackRequest, ProtocolDiagnostic> {
        let workspace_id = self
            .active_documents
            .require_workspace_id()
            .map_err(|error| {
                AppProposalCoordinator::diagnostic(
                    "proposal.audit_rollback_workspace_missing",
                    format!("audit failure rollback requires an active workspace: {error}"),
                )
            })?;
        Ok(WorkspaceMutationRollbackRequest {
            workspace_id,
            proposal_id: proposal.proposal_id,
            principal: proposal.principal.clone(),
            required_capability: proposal.capability.clone(),
            checkpoint,
            correlation_id: proposal.correlation_id,
            causality_id: self.proposal_causality_id(proposal),
        })
    }

    fn response_diagnostics(response: ProposalResponse) -> Vec<ProtocolDiagnostic> {
        match response {
            ProposalResponse::Created(transition)
            | ProposalResponse::Validated(transition)
            | ProposalResponse::Approved(transition)
            | ProposalResponse::Applied(transition) => transition.diagnostics,
            ProposalResponse::Previewed { transition, .. }
            | ProposalResponse::Rejected { transition, .. }
            | ProposalResponse::Denied { transition, .. }
            | ProposalResponse::Failed { transition, .. }
            | ProposalResponse::RolledBack { transition, .. }
            | ProposalResponse::Stale { transition, .. }
            | ProposalResponse::Conflict { transition, .. }
            | ProposalResponse::Cancelled { transition, .. } => transition.diagnostics,
        }
    }

    fn rollback_workspace_failed_diagnostic(response: ProposalResponse) -> Vec<ProtocolDiagnostic> {
        let diagnostics = Self::response_diagnostics(response);
        if diagnostics.is_empty() {
            return vec![AppProposalCoordinator::diagnostic(
                "proposal.audit_rollback_workspace_failed",
                "audit failure rollback did not restore workspace state",
            )];
        }
        diagnostics
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
            ProposalMutationRollback::TextEdit {
                workspace_id,
                file_id,
            } => self.rollback_audit_failed_text_edit(proposal, workspace_id, file_id),
            ProposalMutationRollback::WorkspaceFile(checkpoint) => {
                match self.workspace_rollback_request(proposal, checkpoint) {
                    Ok(request) => {
                        if let Err(response) = self
                            .workspace
                            .rollback_file_mutation_with_checkpoint(request)
                        {
                            diagnostics
                                .extend(Self::rollback_workspace_failed_diagnostic(response));
                        }
                    }
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
                self.refresh_workspace_after_audit_rollback(proposal);
            }
            ProposalMutationRollback::Composite(rollbacks) => {
                for rollback in rollbacks.into_iter().rev() {
                    diagnostics.extend(self.rollback_audit_failed_mutation(
                        proposal,
                        rollback,
                        deferred_save_success,
                    ));
                }
            }
            ProposalMutationRollback::Scoped { proposal, rollback } => {
                diagnostics.extend(self.rollback_audit_failed_mutation(
                    &proposal,
                    *rollback,
                    deferred_save_success,
                ));
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

    fn rollback_audit_failed_text_edit(
        &mut self,
        proposal: &WorkspaceProposal,
        workspace_id: WorkspaceId,
        file_id: FileId,
    ) {
        let Some(buffer_id) = self.editor.buffer_for_file(workspace_id, file_id) else {
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

    fn stale_text_edit_precondition_response_for(
        &self,
        proposal: &WorkspaceProposal,
        preconditions: &ProposalVersionPreconditions,
        actual: &VersionContext,
    ) -> Option<ProposalResponse> {
        if preconditions.buffer_version != Some(actual.buffer_version) {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::BufferVersionMismatch,
                Some(actual.clone()),
                "buffer version changed before text edit apply",
            ));
        }
        if preconditions.snapshot_id != Some(actual.snapshot_id) {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::SnapshotMismatch,
                Some(actual.clone()),
                "snapshot changed before text edit apply",
            ));
        }
        if let Some(expected) = preconditions
            .file_content_version
            .or(preconditions.file_version)
            && expected != actual.file_content_version
        {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::FileContentVersionMismatch,
                Some(actual.clone()),
                "file content version changed before text edit apply",
            ));
        }
        if let Some(expected) = preconditions
            .workspace_generation
            .or(preconditions.generation)
            && expected != actual.workspace_generation
        {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::WorkspaceGenerationMismatch,
                Some(actual.clone()),
                "workspace generation changed before text edit apply",
            ));
        }
        if let Some(expected) = &preconditions.expected_fingerprint
            && actual.fingerprint.as_ref() != Some(expected)
        {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::FingerprintMismatch,
                Some(actual.clone()),
                "file fingerprint changed before text edit apply",
            ));
        }
        if let Some(expected) = preconditions.expected_file_length
            && actual.file_length != Some(expected)
        {
            return Some(self.stale_apply_response(
                proposal,
                ProposalStaleReason::FileLengthMismatch,
                Some(actual.clone()),
                "file length changed before text edit apply",
            ));
        }
        if let Some(expected) = preconditions.expected_modified_at
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
        match self.apply_text_edit_mutation(
            proposal,
            payload.file_id,
            &payload.edits,
            &proposal.preconditions,
        ) {
            Ok(()) => self.applied_response(proposal),
            Err(response) => response,
        }
    }

    #[allow(clippy::result_large_err)]
    fn apply_text_edit_mutation(
        &mut self,
        proposal: &WorkspaceProposal,
        file_id: FileId,
        edits: &devil_protocol::EditBatch,
        preconditions: &ProposalVersionPreconditions,
    ) -> Result<(), ProposalResponse> {
        let workspace_id = match self.active_documents.require_workspace_id() {
            Ok(workspace_id) => workspace_id,
            Err(err) => {
                return Err(self.failed_apply_response(
                    proposal,
                    "proposal.workspace_missing",
                    err.to_string(),
                ));
            }
        };
        let Some(buffer_id) = self.editor.buffer_for_file(workspace_id, file_id) else {
            return Err(self.failed_apply_response(
                proposal,
                "proposal.closed_file_text_edit_denied",
                "text edit apply requires an open editor buffer in Stage 1C",
            ));
        };
        let actual = match self.active_file_version_context(buffer_id) {
            Ok(actual) => actual,
            Err(err) => {
                return Err(self.failed_apply_response(
                    proposal,
                    "proposal.editor_state_unavailable",
                    err.to_string(),
                ));
            }
        };
        if let Some(response) =
            self.stale_text_edit_precondition_response_for(proposal, preconditions, &actual)
        {
            return Err(response);
        }

        match self
            .editor
            .apply_protocol_edits(EditorApplyTransactionRequest {
                workspace_id,
                buffer_id,
                file_id,
                edits: edits.clone(),
                source: TransactionSource::System,
                undo_group_id: Some(uuid::Uuid::now_v7()),
                correlation_id: proposal.correlation_id,
            }) {
            Ok(record) => {
                let descriptor = record.to_protocol_descriptor();
                self.emit_transaction_event(&descriptor);
                Ok(())
            }
            Err(err) => Err(self.failed_apply_response(
                proposal,
                "proposal.editor_apply_failed",
                err.to_string(),
            )),
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

        for edit in &payload.file_edits {
            if edit.file.workspace_id != workspace_id {
                return self.failed_apply_response(
                    proposal,
                    "proposal.workspace_mismatch",
                    "workspace-edit file edit target does not match the active workspace",
                );
            }
            if let Err(response) = self.preflight_workspace_text_edit(proposal, edit) {
                return response;
            }
        }
        for operation in &payload.file_operations {
            if let Err(response) =
                self.preflight_workspace_file_operation(proposal, workspace_id, operation)
            {
                return response;
            }
        }

        let mut committed = Vec::new();
        for edit in &payload.file_edits {
            match self.apply_text_edit_mutation(
                proposal,
                edit.file.file_id,
                &edit.edits,
                &edit.preconditions,
            ) {
                Ok(()) => committed.push(ProposalMutationRollback::TextEdit {
                    workspace_id: edit.file.workspace_id,
                    file_id: edit.file.file_id,
                }),
                Err(response) => {
                    let diagnostics = self.rollback_committed_mutations(proposal, committed);
                    let mut response = response;
                    Self::append_response_diagnostics(&mut response, diagnostics);
                    return response;
                }
            }
        }
        for operation in &payload.file_operations {
            let rollback =
                match self.rollback_snapshot_for_workspace_file_operation(proposal, operation) {
                    Ok(rollback) => rollback,
                    Err(response) => {
                        let diagnostics = self.rollback_committed_mutations(proposal, committed);
                        let mut response = response;
                        Self::append_response_diagnostics(&mut response, diagnostics);
                        return response;
                    }
                };
            let response = self.apply_workspace_file_operation(proposal, operation);
            if Self::response_is_success(&response) {
                committed.push(rollback);
            } else {
                let diagnostics = self.rollback_committed_mutations(proposal, committed);
                let mut response = response;
                Self::append_response_diagnostics(&mut response, diagnostics);
                return response;
            }
        }

        self.applied_response(proposal)
    }

    #[allow(clippy::result_large_err)]
    fn preflight_workspace_text_edit(
        &self,
        proposal: &WorkspaceProposal,
        edit: &devil_protocol::WorkspaceTextEdit,
    ) -> Result<(), ProposalResponse> {
        let Some(buffer_id) = edit.buffer_id.or_else(|| {
            self.editor
                .buffer_for_file(edit.file.workspace_id, edit.file.file_id)
        }) else {
            return Err(self.failed_apply_response(
                proposal,
                "proposal.closed_file_text_edit_denied",
                "workspace-edit text edits require an open editor buffer authority",
            ));
        };
        let actual = self
            .active_file_version_context(buffer_id)
            .map_err(|error| {
                self.failed_apply_response(
                    proposal,
                    "proposal.editor_state_unavailable",
                    error.to_string(),
                )
            })?;
        if let Some(response) =
            self.stale_text_edit_precondition_response_for(proposal, &edit.preconditions, &actual)
        {
            return Err(response);
        }
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    fn preflight_workspace_file_operation(
        &self,
        proposal: &WorkspaceProposal,
        workspace_id: WorkspaceId,
        operation: &devil_protocol::WorkspaceFileOperation,
    ) -> Result<(), ProposalResponse> {
        match operation {
            devil_protocol::WorkspaceFileOperation::Create {
                path,
                initial_content_hash,
            } => {
                if initial_content_hash.is_some() {
                    return Err(self.failed_apply_response(
                        proposal,
                        "proposal.workspace_edit_hash_only_create_denied",
                        "workspace-edit create cannot materialize non-empty hash-only content",
                    ));
                }
                if let Some(response) = self.reject_open_path_mutation(proposal, workspace_id, path)
                {
                    return Err(response);
                }
                if proposal
                    .preconditions
                    .workspace_generation
                    .or(proposal.preconditions.generation)
                    .is_none()
                {
                    return Err(self.failed_apply_response(
                        proposal,
                        "proposal.missing_workspace_precondition",
                        "workspace-edit file operation requires workspace generation precondition",
                    ));
                }
            }
            devil_protocol::WorkspaceFileOperation::Delete { file }
            | devil_protocol::WorkspaceFileOperation::Rename { file, .. } => {
                if file.workspace_id != workspace_id {
                    return Err(self.failed_apply_response(
                        proposal,
                        "proposal.workspace_mismatch",
                        "workspace-edit file operation target does not match the active workspace",
                    ));
                }
                if let Some(response) = self.reject_open_file_mutation(proposal, file) {
                    return Err(response);
                }
                self.closed_file_preconditions(proposal)?;
            }
        }
        Ok(())
    }

    fn apply_workspace_file_operation(
        &mut self,
        proposal: &WorkspaceProposal,
        operation: &devil_protocol::WorkspaceFileOperation,
    ) -> ProposalResponse {
        match operation {
            devil_protocol::WorkspaceFileOperation::Create { path, .. } => {
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

    fn apply_code_action_proposal(
        &mut self,
        proposal: &WorkspaceProposal,
        payload: &devil_protocol::CodeActionProposal,
    ) -> ProposalResponse {
        if payload.edits.is_empty() {
            return self.failed_apply_response(
                proposal,
                "proposal.code_action_requires_edits",
                "code-action apply requires concrete text edits and no command execution",
            );
        }
        let edits = devil_protocol::EditBatch {
            edits: payload.edits.clone(),
        };
        match self.apply_text_edit_mutation(
            proposal,
            payload.file.file_id,
            &edits,
            &proposal.preconditions,
        ) {
            Ok(()) => self.applied_response(proposal),
            Err(response) => response,
        }
    }

    fn apply_batch_proposal(
        &mut self,
        proposal: &WorkspaceProposal,
        payload: &BatchProposalPayload,
    ) -> ProposalResponse {
        let plan = self.preflight_batch_proposal(proposal);
        if !plan.preflight_ok {
            return self.failed_apply_response(
                proposal,
                "proposal.batch_preflight_failed",
                "batch apply requires all item preflight checks to pass before mutation",
            );
        }
        if payload.atomicity == ProposalBatchAtomicity::OrderedNonAtomic {
            return self.failed_apply_response(
                proposal,
                "proposal.batch_ordered_non_atomic_requires_partial_failures",
                "ordered non-atomic batch apply requires exact partial-failure records before runtime mutation is enabled",
            );
        }

        let mut committed = Vec::new();
        for item in self.ordered_batch_items(payload) {
            let item_proposal = Self::batch_item_proposal(proposal, item);
            let rollback = match self.rollback_snapshot_for_batch_item(&item_proposal, item) {
                Ok(rollback) => rollback,
                Err(response) => {
                    let diagnostics = self.rollback_committed_mutations(proposal, committed);
                    let mut response = response;
                    Self::append_response_diagnostics(&mut response, diagnostics);
                    return response;
                }
            };
            let response = self.apply_batch_item(&item_proposal, item);
            if Self::response_is_success(&response) {
                committed.push(ProposalMutationRollback::Scoped {
                    proposal: Box::new(item_proposal),
                    rollback: Box::new(rollback),
                });
            } else {
                let diagnostics = self.rollback_committed_mutations(proposal, committed);
                let mut response = response;
                Self::append_response_diagnostics(&mut response, diagnostics);
                return response;
            }
        }

        self.applied_response(proposal)
    }

    fn batch_item_proposal(
        proposal: &WorkspaceProposal,
        item: &ProposalBatchItem,
    ) -> WorkspaceProposal {
        let mut item_proposal = proposal.clone();
        item_proposal.payload = (*item.payload).clone();
        item_proposal.capability = item.required_capability.clone();
        item_proposal
    }

    fn ordered_batch_items<'a>(
        &self,
        payload: &'a BatchProposalPayload,
    ) -> Vec<&'a ProposalBatchItem> {
        let mut items = payload.items.iter().collect::<Vec<_>>();
        items.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.item_id.cmp(&right.item_id))
        });
        items
    }

    #[allow(clippy::result_large_err)]
    fn rollback_snapshot_for_batch_item(
        &self,
        proposal: &WorkspaceProposal,
        item: &ProposalBatchItem,
    ) -> Result<ProposalMutationRollback, ProposalResponse> {
        match item.payload.as_ref() {
            ProposalPayload::TextEdit(payload) => {
                let workspace_id =
                    self.active_documents
                        .require_workspace_id()
                        .map_err(|error| {
                            self.failed_apply_response(
                                proposal,
                                "proposal.workspace_missing",
                                format!(
                                    "batch text edit rollback requires an active workspace: {error}"
                                ),
                            )
                        })?;
                Ok(ProposalMutationRollback::TextEdit {
                    workspace_id,
                    file_id: payload.file_id,
                })
            }
            ProposalPayload::CreateFile(payload) => self.workspace_rollback_checkpoint(
                proposal,
                WorkspaceMutationRollbackTarget::CreatedFile {
                    path: payload.path.clone(),
                },
            ),
            ProposalPayload::DeleteFile(payload) => self.workspace_rollback_checkpoint(
                proposal,
                WorkspaceMutationRollbackTarget::DeletedFile {
                    file: payload.file.clone(),
                },
            ),
            ProposalPayload::RenameFile(payload) => self.workspace_rollback_checkpoint(
                proposal,
                WorkspaceMutationRollbackTarget::RenamedFile {
                    file: payload.file.clone(),
                    destination: payload.destination.clone(),
                },
            ),
            ProposalPayload::WorkspaceEdit(payload) => {
                self.rollback_snapshot_for_workspace_edit(proposal, payload)
            }
            _ => Ok(ProposalMutationRollback::None),
        }
    }

    fn apply_batch_item(
        &mut self,
        proposal: &WorkspaceProposal,
        item: &ProposalBatchItem,
    ) -> ProposalResponse {
        match item.payload.as_ref() {
            ProposalPayload::TextEdit(payload) => match self.apply_text_edit_mutation(
                proposal,
                payload.file_id,
                &payload.edits,
                &proposal.preconditions,
            ) {
                Ok(()) => ProposalResponse::Applied(self.proposal_coordinator.transition(
                    proposal,
                    ProposalLifecycleState::Applied,
                    Vec::new(),
                )),
                Err(response) => response,
            },
            ProposalPayload::CreateFile(payload) => {
                self.apply_create_file_proposal(proposal, payload)
            }
            ProposalPayload::DeleteFile(payload) => {
                self.apply_delete_file_proposal(proposal, payload)
            }
            ProposalPayload::RenameFile(payload) => {
                self.apply_rename_file_proposal(proposal, payload)
            }
            ProposalPayload::WorkspaceEdit(payload) => {
                self.apply_workspace_edit_proposal(proposal, payload)
            }
            _ => self
                .proposal_coordinator
                .unsupported_response(proposal, "apply"),
        }
    }

    fn response_is_success(response: &ProposalResponse) -> bool {
        matches!(response, ProposalResponse::Applied(_))
    }

    fn rollback_committed_mutations(
        &mut self,
        proposal: &WorkspaceProposal,
        committed: Vec<ProposalMutationRollback>,
    ) -> Vec<ProtocolDiagnostic> {
        let mut diagnostics = Vec::new();
        for rollback in committed.into_iter().rev() {
            diagnostics.extend(self.rollback_audit_failed_mutation(proposal, rollback, None));
        }
        diagnostics
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
                blocked: false,
                diagnostics: Vec::new(),
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
                blocked: false,
                diagnostics: Vec::new(),
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

    fn plugin_manifest(plugin_id: PluginId) -> PluginManifest {
        PluginManifest {
            plugin_id,
            name: "phase5.test".to_string(),
            version: "0.1.0".to_string(),
            schema_version: 1,
            min_abi_version: 1,
            max_abi_version: 1,
            module_hash: "sha256:phase5".to_string(),
            manifest_id: "manifest:phase5".to_string(),
            trust: devil_protocol::PluginTrustMetadata {
                source: devil_protocol::PluginTrustSource::ExplicitLocalAllow,
                decision: devil_protocol::PluginTrustDecision::ExplicitlyAllowed,
                reason: "test allow".to_string(),
            },
            signature: None,
            activation_events: vec![devil_protocol::PluginActivationEvent::OnCommand {
                command: "phase5.run".to_string(),
            }],
            contributions: vec![devil_protocol::PluginContribution::Command(
                devil_protocol::PluginCommandDescriptor {
                    command_id: "phase5.run".to_string(),
                    title: "Phase 5 Run".to_string(),
                    required_capability: CapabilityId("plugin.command".to_string()),
                },
            )],
            requested_capabilities: vec![CapabilityId("plugin.command".to_string())],
            storage_namespace: devil_plugin::plugin_namespace(plugin_id, "state"),
            quotas: devil_protocol::PluginQuotaDeclaration {
                max_fuel: 1000,
                max_wall_time_ms: 50,
                max_memory_pages: 8,
                max_storage_bytes: 4096,
                max_host_calls: 4,
                max_events: 4,
                max_output_bytes: 128,
            },
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
        let diagnostic = ProtocolDiagnostic {
            code: "proposal.audit_rollback_workspace_failed".to_string(),
            message: "audit failure rollback did not restore workspace state: locked".to_string(),
            severity: ProtocolDiagnosticSeverity::Error,
            path: Some(path.clone()),
            range: None,
        };
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
        assert_transition_diagnostic(&response, "proposal.audit_rollback_workspace_failed");
        let ProposalResponse::Failed { transition, .. } = response else {
            panic!("expected failed response");
        };
        let rollback_diagnostic = transition
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "proposal.audit_rollback_workspace_failed")
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
    fn plugin_command_intent_routes_through_app_owned_plugin_runtime() {
        let mut app = AppComposition::new();
        let plugin_id = app
            .load_plugin_manifest(plugin_manifest(PluginId(7)))
            .expect("plugin manifest loads");

        let outcome = app
            .dispatch_ui_intent(CommandDispatchIntent::InvokePluginCommand {
                plugin_id,
                command_id: "phase5.run".to_string(),
                metadata_label: "metadata-only".to_string(),
            })
            .expect("plugin command routes through app");

        match outcome {
            AppCommandOutcome::PluginCommandInvoked(response) => {
                assert!(matches!(
                    response.as_ref(),
                    PluginHostCallResponse::Accepted { metadata_label }
                        if metadata_label == "metadata-only"
                ));
            }
            other => panic!("unexpected plugin command outcome: {other:?}"),
        }
    }

    #[test]
    fn command_dispatcher_routes_collaboration_intents_to_app_requests() {
        let active = AppCommandRouteContext {
            workspace_id: Some(WorkspaceId(1)),
            buffer_id: Some(BufferId(1)),
            file_id: Some(FileId(1)),
        };
        let join = CommandDispatcher::route_intent(
            CommandDispatchIntent::JoinCollaborationSession {
                session_id: CollaborationSessionId(7),
            },
            active,
            CorrelationId(1),
        )
        .expect("join routes");
        assert_eq!(
            join,
            AppCommandRequest::JoinCollaborationSession {
                session_id: CollaborationSessionId(7)
            }
        );

        let presence = CommandDispatcher::route_intent(
            CommandDispatchIntent::PublishCollaborationPresence {
                session_id: CollaborationSessionId(7),
                participant_id: CollaborationParticipantId(9),
            },
            active,
            CorrelationId(1),
        )
        .expect("presence routes");
        assert_eq!(
            presence,
            AppCommandRequest::PublishCollaborationPresence {
                session_id: CollaborationSessionId(7),
                participant_id: CollaborationParticipantId(9),
            }
        );
    }

    #[test]
    fn shared_collaboration_route_wraps_existing_safe_targets_only() {
        let editor_target = ProposalAffectedTarget {
            target_id: "editor".to_string(),
            kind: ProposalTargetKind::OpenBuffer,
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(1)),
            buffer_id: Some(BufferId(1)),
            path: None,
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
        };
        let collaboration_target = ProposalAffectedTarget {
            target_id: "collaboration".to_string(),
            kind: ProposalTargetKind::CollaborationSession,
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(1)),
            buffer_id: Some(BufferId(1)),
            path: None,
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: Some("7".to_string()),
            byte_ranges: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
        };
        let shared = ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![editor_target, collaboration_target.clone()],
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        };
        assert_eq!(
            ProposalExecutionRoute::for_payload(
                &text_edit_proposal(ProposalId(70)).payload,
                &shared
            ),
            ProposalExecutionRoute::SharedCollaboration
        );

        let pure_collaboration = ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![collaboration_target],
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        };
        assert_eq!(
            ProposalExecutionRoute::for_payload(
                &text_edit_proposal(ProposalId(71)).payload,
                &pure_collaboration
            ),
            ProposalExecutionRoute::Unsupported
        );
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
    fn proposal_coordinator_exports_and_recovers_lifecycle_snapshot() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let proposal = save_proposal(ProposalId(20));
        register_created(&coordinator, &proposal);
        assert!(matches!(
            coordinator.handle(ProposalRequest::Validate(proposal.clone())),
            Ok(ProposalResponse::Validated(_))
        ));
        assert!(matches!(
            coordinator.handle(ProposalRequest::Preview(proposal.clone())),
            Ok(ProposalResponse::Previewed { .. })
        ));

        let snapshot = coordinator.proposal_lifecycle_recovery_snapshot();
        assert_eq!(snapshot.records.len(), 1);
        assert!(snapshot.generated_at.0 > 0);

        let recovered = AppProposalCoordinator::new(SharedEventSink::default());
        recovered.recover_lifecycle_from_snapshot(snapshot);

        assert_eq!(
            recovered.current_lifecycle_state(proposal.proposal_id),
            Some(ProposalLifecycleState::Previewed)
        );
        assert!(recovered.has_lifecycle_context(proposal.proposal_id));
        assert_eq!(
            recovered
                .proposal(proposal.proposal_id)
                .map(|proposal| proposal.proposal_id),
            Some(proposal.proposal_id)
        );

        let ledger = recovered.proposal_ledger_projection(TimestampMillis(99));
        assert_eq!(ledger.rows.len(), 1);
        assert_eq!(ledger.selected_proposal_id, Some(proposal.proposal_id));
        assert_eq!(
            ledger.rows[0].lifecycle.state,
            ProposalLifecycleState::Previewed
        );
        assert_eq!(ledger.rows[0].updated_at, TimestampMillis(99));
        assert!(
            ledger.rows[0]
                .redaction_hints
                .contains(&RedactionHint::MetadataOnly)
        );
    }

    #[test]
    fn proposal_coordinator_builds_metadata_only_ledger_projection() {
        let coordinator = AppProposalCoordinator::new(SharedEventSink::default());
        let save = save_proposal(ProposalId(21));
        let terminal = proposal_with(ProposalId(22), "terminal.spawn", terminal_payload());
        register_created(&coordinator, &save);
        register_created(&coordinator, &terminal);

        let ledger = coordinator.proposal_ledger_projection(TimestampMillis(123));
        assert_eq!(ledger.rows.len(), 2);
        assert_eq!(ledger.selected_proposal_id, Some(ProposalId(22)));
        assert!(
            ledger
                .redaction_hints
                .contains(&RedactionHint::MetadataOnly)
        );

        let save_row = ledger
            .rows
            .iter()
            .find(|row| row.proposal_id == save.proposal_id)
            .expect("save row");
        assert_eq!(
            save_row.payload_kind,
            devil_protocol::ProposalPayloadKind::SaveFile
        );
        assert_eq!(save_row.workspace_id, Some(WorkspaceId(1)));
        assert!(save_row.diff_summary.full_source_redacted);
        assert_eq!(
            save_row.privacy_label,
            devil_protocol::ProposalPrivacyLabel::WorkspaceMetadata
        );

        let terminal_row = ledger
            .rows
            .iter()
            .find(|row| row.proposal_id == terminal.proposal_id)
            .expect("terminal row");
        assert_eq!(
            terminal_row.risk_label,
            devil_protocol::ProposalRiskLabel::High
        );
        assert_eq!(
            terminal_row.rollback,
            devil_protocol::ProposalRollbackAvailability::Unavailable
        );
        assert_eq!(
            terminal_row.diff_summary.kind,
            devil_protocol::ProposalDiffSummaryKind::TerminalMetadata
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
    fn proposal_coordinator_rejects_stateless_generic_save_apply() {
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
            panic!("stateless coordinator save apply should remain denied");
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
