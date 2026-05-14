//! Application composition root for workspace/editor/ui orchestration.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use devil_editor::{EditorEngine, EditorError, SaveAcknowledgement, SaveRequestDto, TextEdit};
use devil_observability::{
    SharedEventSink, event_metadata_record, proposal_applied_event, proposal_approved_event,
    proposal_audit_record, proposal_created_event, proposal_failed_event, proposal_previewed_event,
    proposal_rejected_event, proposal_rolled_back_event, proposal_validated_event,
    save_denied_event, stale_proposal_rejected_event, transaction_event,
};
use devil_platform::{NativeFileSystem, NativeWatcherService};
use devil_project::{OpenedFileText, WorkspaceActor, WorkspaceError, WorkspaceSaveRequest};
use devil_protocol::{
    BufferId, CanonicalPath, CapabilityId, CapabilityNamespace, CausalityId, CorrelationId,
    EventEnvelope, EventSequence, EventSinkPort, EventSinkRequest, FileConflictContext,
    FileConflictLifecycleState, FileConflictReason, FileConflictState, FileContentVersion,
    FileFingerprint, FileId, FileIdentity, FileTreeNode, PreviewSummary, PrincipalId,
    ProposalDenialReason, ProposalFailureReason, ProposalId, ProposalLifecycleState,
    ProposalLifecycleTransition, ProposalPayload, ProposalPort, ProposalRequest, ProposalResponse,
    ProposalVersionPreconditions, ProtocolDiagnostic, ProtocolDiagnosticSeverity, ProtocolError,
    ProtocolResult, SaveConflictPolicy, SaveFileProposal, SaveIntent, StorageRepositoryPort,
    StorageRepositoryRequest, TextTransactionDescriptor, TimestampMillis, TransactionSource,
    TrustDecisionContext, WorkspaceGeneration, WorkspaceId, WorkspaceOpenRequest, WorkspaceOpened,
    WorkspacePort, WorkspaceProposal, WorkspaceRequest, WorkspaceResponse, WorkspaceTrustState,
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

impl EventContext {
    fn new(correlation_id: CorrelationId) -> Self {
        Self {
            correlation_id,
            causality_id: CausalityId(uuid::Uuid::now_v7()),
        }
    }
}

#[derive(Debug)]
struct SaveProposalCoordinator {
    next_proposal_id: u64,
    event_sink: SharedEventSink,
    next_event_sequence: u64,
    proposal_contexts: HashMap<ProposalId, EventContext>,
}

impl SaveProposalCoordinator {
    fn new(event_sink: SharedEventSink) -> Self {
        Self {
            next_proposal_id: 0,
            event_sink,
            next_event_sequence: 0,
            proposal_contexts: HashMap::new(),
        }
    }

    fn next_id(&mut self) -> devil_protocol::ProposalId {
        self.next_proposal_id = self.next_proposal_id.saturating_add(1);
        devil_protocol::ProposalId(self.next_proposal_id)
    }

    fn next_sequence(&mut self) -> EventSequence {
        self.next_event_sequence = self.next_event_sequence.saturating_add(1).max(1);
        EventSequence(self.next_event_sequence)
    }

    fn emit(&self, envelope: EventEnvelope) {
        let _ = self.event_sink.emit(EventSinkRequest { envelope });
    }

    fn build_save_proposal(
        &mut self,
        save: &SaveRequestDto,
        metadata: &ActiveFileMetadata,
        principal: PrincipalId,
        workspace_trust_state: WorkspaceTrustState,
        event_context: EventContext,
    ) -> WorkspaceProposal {
        let proposal_id = self.next_id();
        self.proposal_contexts.insert(proposal_id, event_context);
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
        proposal
    }

    fn created_response(&self, proposal: &WorkspaceProposal) -> ProposalResponse {
        ProposalResponse::Created(self.transition(
            proposal,
            ProposalLifecycleState::Created,
            Vec::new(),
        ))
    }

    fn transition(
        &self,
        proposal: &WorkspaceProposal,
        lifecycle_state: ProposalLifecycleState,
        diagnostics: Vec<ProtocolDiagnostic>,
    ) -> ProposalLifecycleTransition {
        let causality_id = self
            .proposal_contexts
            .get(&proposal.proposal_id)
            .map(|context| context.causality_id)
            .unwrap_or_else(|| CausalityId(uuid::Uuid::now_v7()));
        ProposalLifecycleTransition {
            proposal_id: proposal.proposal_id,
            lifecycle_state,
            timestamp: TimestampMillis::now(),
            principal: proposal.principal.clone(),
            capability: proposal.capability.clone(),
            correlation_id: proposal.correlation_id,
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

    fn validate_proposal(&self, proposal: &WorkspaceProposal) -> ProposalResponse {
        match &proposal.payload {
            ProposalPayload::SaveFile(save) => {
                let mut diagnostics = Vec::new();
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

                if diagnostics.is_empty() {
                    ProposalResponse::Validated(self.transition(
                        proposal,
                        ProposalLifecycleState::Validated,
                        Vec::new(),
                    ))
                } else {
                    ProposalResponse::Denied {
                        transition: self.transition(
                            proposal,
                            ProposalLifecycleState::Denied,
                            diagnostics,
                        ),
                        reason: ProposalDenialReason::PolicyDenied,
                    }
                }
            }
            _ => ProposalResponse::Failed {
                transition: self.transition(
                    proposal,
                    ProposalLifecycleState::Failed,
                    vec![Self::diagnostic(
                        "proposal.unsupported",
                        "only save proposals are supported by Track 2 coordinator",
                    )],
                ),
                reason: ProposalFailureReason::InternalError,
            },
        }
    }
}

impl ProposalPort for SaveProposalCoordinator {
    fn handle(&self, request: ProposalRequest) -> ProtocolResult<ProposalResponse> {
        match request {
            ProposalRequest::Validate(proposal) => Ok(self.validate_proposal(&proposal)),
            ProposalRequest::Preview(proposal) => Ok(ProposalResponse::Previewed {
                transition: self.transition(
                    &proposal,
                    ProposalLifecycleState::Previewed,
                    Vec::new(),
                ),
                proposal: Box::new(proposal),
            }),
            ProposalRequest::Apply(proposal) => Err(ProtocolError {
                code: "unsupported".to_string(),
                message: format!(
                    "proposal {} apply is owned by AppComposition save orchestration",
                    proposal.proposal_id.0
                ),
            }),
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
    active_principal_id: Option<PrincipalId>,
    active_workspace_trust: Option<WorkspaceTrustState>,
    active_file_id: Option<FileId>,
    active_file_path: Option<String>,
    active_buffer_id: Option<BufferId>,
    active_file_metadata: Option<ActiveFileMetadata>,
}

impl ActiveDocumentController {
    fn new() -> Self {
        Self {
            opened_workspace: None,
            active_principal_id: None,
            active_workspace_trust: None,
            active_file_id: None,
            active_file_path: None,
            active_buffer_id: None,
            active_file_metadata: None,
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
        principal: PrincipalId,
        trust: WorkspaceTrustState,
    ) {
        self.opened_workspace = Some(opened);
        self.active_principal_id = Some(principal);
        self.active_workspace_trust = Some(trust);
        self.clear_active_file();
    }

    fn clear_active_file(&mut self) {
        self.active_file_id = None;
        self.active_file_path = None;
        self.active_buffer_id = None;
        self.active_file_metadata = None;
    }

    fn bind_opened_file(&mut self, opened: &OpenedFileText, buffer_id: BufferId) {
        let identity = opened.identity.clone();
        self.active_file_id = Some(identity.file_id);
        self.active_file_path = Some(identity.canonical_path.0.clone());
        self.active_buffer_id = Some(buffer_id);
        self.active_file_metadata = Some(ActiveFileMetadata {
            identity,
            fingerprint: opened.fingerprint.clone(),
            file_content_version: opened.file_content_version,
            workspace_generation: opened.workspace_generation,
            modified_at: opened.modified_at,
            file_length: opened.file_length,
        });
    }

    fn bind_saved_file(&mut self, applied: devil_project::WorkspaceSaveApplied) {
        self.active_file_id = Some(applied.identity.file_id);
        self.active_file_path = Some(applied.identity.canonical_path.0.clone());
        self.active_file_metadata = Some(ActiveFileMetadata {
            identity: applied.identity,
            fingerprint: applied.fingerprint,
            file_content_version: applied.file_content_version,
            workspace_generation: applied.workspace_generation,
            modified_at: applied.modified_at,
            file_length: applied.file_length,
        });
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
                TextEdit::insert(at, text),
                correlation_id,
            ),
            CommandDispatchIntent::Delete { buffer_id, range } => {
                Self::edit_request(active, buffer_id, TextEdit::delete(range), correlation_id)
            }
            CommandDispatchIntent::Replace {
                buffer_id,
                range,
                replacement,
            } => Self::edit_request(
                active,
                buffer_id,
                TextEdit::new(range, replacement),
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
    ) -> Result<ActiveBufferProjection, AppCompositionError> {
        let Some(buffer_id) = active.active_buffer_id else {
            return Ok(ActiveBufferProjection::empty());
        };

        Ok(ActiveBufferProjection {
            workspace_id: active.workspace_id(),
            buffer_id: Some(buffer_id),
            file_id: active.active_file_id,
            file_path: active
                .active_file_path
                .as_ref()
                .map(|path| CanonicalPath(path.clone())),
            text: editor.text(buffer_id)?.to_string(),
            dirty: editor.is_dirty(buffer_id)?,
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
    fn save_active_buffer(
        editor: &mut EditorEngine,
        workspace: &WorkspaceActor,
        proposal_coordinator: &mut SaveProposalCoordinator,
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
        Self::observe_proposal_response(proposal_coordinator, storage, &proposal, &created, None);
        let validation = proposal_coordinator
            .handle(ProposalRequest::Validate(proposal.clone()))
            .unwrap_or_else(|err| {
                Self::failed_response_for_protocol_error(err, &proposal, event_context.causality_id)
            });
        Self::observe_proposal_response(
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
        Self::observe_proposal_response(proposal_coordinator, storage, &proposal, &preview, None);
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
                Self::observe_proposal_response(
                    proposal_coordinator,
                    storage,
                    &proposal,
                    &applied.response,
                    Some(&applied),
                );
                Ok(SaveWorkflowOutput { save, applied })
            }
            Err(response) => {
                Self::observe_proposal_response(
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

    fn observe_proposal_response(
        proposal_coordinator: &mut SaveProposalCoordinator,
        storage: &dyn StorageRepositoryPort,
        proposal: &WorkspaceProposal,
        response: &ProposalResponse,
        applied: Option<&devil_project::WorkspaceSaveApplied>,
    ) {
        for envelope in Self::events_for_response(proposal_coordinator, proposal, response) {
            let metadata = event_metadata_record(&envelope);
            proposal_coordinator.emit(envelope);
            let _ = storage.handle(StorageRepositoryRequest::SaveEventMetadata(metadata));
        }

        if let Some(transition) = Self::transition_for_response(response) {
            let audit = proposal_audit_record(proposal, transition);
            let _ = storage.handle(StorageRepositoryRequest::SaveProposalAuditRecord(audit));
        }

        if let Some(applied) = applied {
            let _ = applied.used_non_atomic_fallback;
        }
    }

    fn events_for_response(
        proposal_coordinator: &mut SaveProposalCoordinator,
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
                let file = proposal_file_identity(proposal);
                vec![
                    proposal_rejected_event(
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
                    ),
                    save_denied_event(
                        file.workspace_id,
                        file.file_id,
                        transition.correlation_id,
                        transition.causality_id,
                        proposal_coordinator.next_sequence(),
                        format!("{reason:?}"),
                    ),
                ]
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
                let file = proposal_file_identity(proposal);
                vec![stale_proposal_rejected_event(
                    file.workspace_id,
                    file.file_id,
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
            | ProposalResponse::Conflict { transition, .. } => Some(transition),
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

fn proposal_file_identity(proposal: &WorkspaceProposal) -> &FileIdentity {
    match &proposal.payload {
        ProposalPayload::SaveFile(payload) => &payload.file,
        ProposalPayload::DeleteFile(payload) => &payload.file,
        ProposalPayload::RenameFile(payload) => &payload.file,
        ProposalPayload::FormatFile(payload) => &payload.file,
        ProposalPayload::CodeAction(payload) => &payload.file,
        ProposalPayload::TextEdit(_)
        | ProposalPayload::CreateFile(_)
        | ProposalPayload::TerminalCommand(_) => {
            panic!("Track 5 save workflow only routes file-backed proposal payloads")
        }
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
    proposal_coordinator: SaveProposalCoordinator,
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
            proposal_coordinator: SaveProposalCoordinator::new(event_sink.clone()),
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
        let request = WorkspaceOpenRequest {
            correlation_id: self.correlation_generator.next(),
            principal_id: principal.clone(),
            root_path: CanonicalPath(root.as_ref().to_string_lossy().into_owned()),
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
            .bind_workspace(opened.clone(), principal, trust);
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
    pub fn active_buffer_projection(&self) -> Result<ActiveBufferProjection, AppCompositionError> {
        ProjectionBuilder::active_buffer_projection(&self.active_documents, &self.editor)
    }

    /// Build the complete projection snapshot consumed by the UI shell.
    pub fn shell_projection_snapshot(
        &self,
        title: impl Into<String>,
    ) -> Result<ShellProjectionSnapshot, AppCompositionError> {
        Ok(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain(title),
            explorer_projection: self.explorer_projection()?,
            active_buffer_projection: self.active_buffer_projection()?,
            status_messages: Vec::new(),
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

    /// Expose storage repository port for integration validation and future wiring.
    pub fn storage_port(&self) -> &dyn StorageRepositoryPort {
        &self.storage
    }

    /// Expose event publisher port placeholder for integration validation and future wiring.
    pub fn event_publisher(&self) -> &dyn EventSinkPort {
        &self.event_sink
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
