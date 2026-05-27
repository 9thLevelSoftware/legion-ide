//! Desktop event to app-command bridge.

use std::path::PathBuf;

use devil_protocol::{
    AgentRunId, BufferId, FileId, ProposalCancellationReason, ProposalId, ProposalRejectionReason,
    ProposalRollbackReason, ProtocolTextRange, TerminalSessionId, TextCoordinate, ViewportScroll,
};
use devil_protocol::{PluginContribution, PluginId};
use devil_ui::{CommandDispatchIntent, SearchScopeProjection, ShellProjectionSnapshot};
use thiserror::Error;

/// Adapter-local renderer action before app routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopAction {
    /// Quit the desktop shell.
    Quit,
    /// Save the active buffer through app authority.
    SaveActive,
    /// Save every open tab through app authority.
    SaveAll,
    /// Switch to a projected tab.
    SwitchTab {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Request close for a projected tab.
    CloseTab {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Save the buffer currently represented by a dirty-close prompt.
    SaveDirtyClose {
        /// Prompt buffer identifier.
        buffer_id: BufferId,
    },
    /// Cancel the active dirty-close prompt without closing or discarding text.
    CancelDirtyClose {
        /// Prompt buffer identifier.
        buffer_id: BufferId,
    },
    /// Open a user-entered path through workspace authority.
    OpenPathText(String),
    /// Open a path selected by a native file dialog.
    OpenPathDialogSelected(String),
    /// Native file dialog was cancelled.
    OpenPathDialogCancelled,
    /// Ask the workflow layer to show an open-path prompt.
    ShowOpenPathPrompt,
    /// Ask the workflow layer to show a search prompt.
    ShowSearchPrompt {
        /// Search scope to preselect.
        scope: SearchScopeProjection,
    },
    /// Ask the workflow layer to open a workspace root.
    OpenWorkspace {
        /// Workspace root selected by the adapter.
        root: PathBuf,
    },
    /// Refresh the explorer projection through app authority.
    RefreshExplorer,
    /// Toggle adapter-local explorer expansion for a canonical path.
    ToggleExplorerPath {
        /// Canonical path represented by the explorer row.
        path: String,
    },
    /// Select/reveal an explorer file through app authority.
    SelectExplorerFile {
        /// Projected workspace file identifier.
        file_id: FileId,
    },
    /// Request a proposal preview through app authority.
    PreviewProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
    },
    /// Approve a projected proposal through app authority.
    ApproveProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
    },
    /// Reject a projected proposal through app authority.
    RejectProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
        /// Display-safe rejection reason.
        reason: ProposalRejectionReason,
    },
    /// Apply a projected proposal through app authority.
    ApplyProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
    },
    /// Roll back a projected proposal through app authority.
    RollbackProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
        /// Display-safe rollback reason.
        reason: ProposalRollbackReason,
    },
    /// Cancel a projected proposal through app authority.
    CancelProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
        /// Display-safe cancellation reason.
        reason: ProposalCancellationReason,
    },
    /// Open projected proposal details without taking proposal ownership.
    OpenProposalDetails {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
    },
    /// Start a metadata-only assisted-AI explain run through app authority.
    StartAiExplain {
        /// Display-safe instruction label.
        instruction_label: String,
    },
    /// Start a proposal-only assisted-AI edit run through app authority.
    StartAiProposal {
        /// Display-safe instruction label.
        instruction_label: String,
    },
    /// Cancel a projected assisted-AI run through app authority.
    CancelAiRun {
        /// Projected or user-visible run identifier.
        run_id: AgentRunId,
    },
    /// Replay a projected assisted-AI run from app-owned metadata.
    ReplayAiRun {
        /// Projected or user-visible run identifier.
        run_id: AgentRunId,
    },
    /// Inspect projected assisted-AI run metadata.
    InspectAiRun {
        /// Projected or user-visible run identifier.
        run_id: AgentRunId,
    },
    /// Invoke a projected plugin command through app-owned plugin authority.
    InvokePluginCommand {
        /// Plugin identifier selected from projection data.
        plugin_id: PluginId,
        /// Command identifier selected from projection data.
        command_id: String,
    },
    /// Insert text at a projected coordinate.
    InsertText {
        /// Text to insert.
        text: String,
        /// Projected insertion coordinate.
        at: TextCoordinate,
    },
    /// Replace a projected range.
    ReplaceRange {
        /// Projected range to replace.
        range: ProtocolTextRange,
        /// Replacement text.
        replacement: String,
    },
    /// Delete a projected range.
    DeleteRange {
        /// Projected range to delete.
        range: ProtocolTextRange,
    },
    /// Paste clipboard text at a projected coordinate.
    ClipboardPaste {
        /// Clipboard text to insert.
        text: String,
        /// Projected insertion coordinate.
        at: TextCoordinate,
    },
    /// Commit IME text at a projected coordinate.
    ImeCommit {
        /// IME text to insert.
        text: String,
        /// Projected insertion coordinate.
        at: TextCoordinate,
    },
    /// Undo the active buffer.
    Undo,
    /// Redo the active buffer.
    Redo,
    /// Set the primary cursor for a buffer or the active buffer.
    SetCursor {
        /// Optional target buffer; falls back to the active tab.
        buffer_id: Option<BufferId>,
        /// Cursor coordinate in projection space.
        cursor: TextCoordinate,
    },
    /// Set the primary selection for a buffer or the active buffer.
    SetSelection {
        /// Optional target buffer; falls back to the active tab.
        buffer_id: Option<BufferId>,
        /// Selection range in projection space.
        range: ProtocolTextRange,
    },
    /// Set viewport scroll for a buffer or the active buffer.
    SetViewportScroll {
        /// Optional target buffer; falls back to the active tab.
        buffer_id: Option<BufferId>,
        /// Projected viewport scroll state.
        scroll: ViewportScroll,
    },
    /// Run bounded lexical search through app authority.
    RunSearch {
        /// Search scope.
        scope: SearchScopeProjection,
        /// User-provided query text.
        query: String,
        /// Requested result limit; zero means app default.
        limit: usize,
    },
    /// Cancel projected search by query id.
    CancelSearch {
        /// Query id to cancel.
        query_id: String,
    },
    /// Request language hover for the active buffer.
    RequestHover {
        /// Projected cursor position.
        position: TextCoordinate,
    },
    /// Request language completions for the active buffer.
    RequestCompletion {
        /// Projected cursor position.
        position: TextCoordinate,
    },
    /// Request definition locations for the active buffer.
    GoToDefinition {
        /// Projected cursor position.
        position: TextCoordinate,
    },
    /// Request reference locations for the active buffer.
    FindReferences {
        /// Projected cursor position.
        position: TextCoordinate,
    },
    /// Refresh the active buffer outline.
    RefreshOutline,
    /// Request a formatting proposal preview.
    RequestFormattingProposal,
    /// Request a rename proposal preview.
    RequestRenameProposal {
        /// Projected cursor position.
        position: TextCoordinate,
        /// New symbol name.
        new_name: String,
    },
    /// Request an organize-imports proposal preview.
    RequestOrganizeImportsProposal,
    /// Request a code-action proposal preview.
    RequestCodeActionProposal {
        /// Code-action identifier.
        action_id: String,
    },
    /// Cancel a language operation.
    CancelLanguageOperation {
        /// Operation identifier.
        operation_id: String,
    },
    /// Launch a terminal session through app authority.
    TerminalLaunch {
        /// Command label.
        command_label: String,
    },
    /// Send input to the active terminal session.
    TerminalInput {
        /// Input payload.
        payload: String,
    },
    /// Resize the active terminal session.
    TerminalResize {
        /// Column count.
        cols: u16,
        /// Row count.
        rows: u16,
    },
    /// Kill the active terminal session.
    TerminalKill,
    /// Close the active terminal session.
    TerminalClose,
    /// Poll output for the active terminal session.
    TerminalOutputPoll,
    /// Search terminal output.
    TerminalSearch {
        /// Query label.
        query: String,
    },
}

/// App-owned request that is not a direct UI command intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopAppRequest {
    /// Open a workspace root through the app composition layer.
    OpenWorkspace {
        /// Workspace root path.
        root: PathBuf,
    },
    /// Ask workflow code to display an open-path prompt.
    ShowOpenPathPrompt,
    /// Ask workflow code to display a search prompt.
    ShowSearchPrompt {
        /// Search scope to preselect.
        scope: SearchScopeProjection,
    },
    /// Toggle adapter-local explorer expansion.
    ToggleExplorerPath {
        /// Canonical path represented by the explorer row.
        path: String,
    },
    /// Cancel an app-owned dirty-close prompt.
    CancelDirtyClose {
        /// Prompt buffer identifier.
        buffer_id: BufferId,
    },
}

/// Result of translating a desktop action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopBridgeOutput {
    /// Dispatch this intent to app authority.
    Intent(CommandDispatchIntent),
    /// Handle this request in the desktop workflow/app layer.
    AppRequest(DesktopAppRequest),
    /// No application work should happen.
    Noop,
    /// The action cannot be represented safely.
    Error(DesktopBridgeError),
}

/// Bridge mapping errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DesktopBridgeError {
    /// The current projection has no active buffer id.
    #[error("active buffer is required for this desktop action")]
    MissingActiveBuffer,
    /// Target buffer was not present in the projected tab list.
    #[error("unknown tab buffer: {buffer_id:?}")]
    UnknownTab {
        /// Unknown tab buffer.
        buffer_id: BufferId,
    },
    /// Target file was not present in the projected explorer tree.
    #[error("unknown explorer file: {file_id:?}")]
    UnknownExplorerFile {
        /// Unknown explorer file.
        file_id: FileId,
    },
    /// Target proposal was not present in the proposal ledger projection.
    #[error("unknown proposal: {proposal_id:?}")]
    UnknownProposal {
        /// Unknown proposal id.
        proposal_id: ProposalId,
    },
    /// Assisted-AI run id was empty or not present in current projections.
    #[error("unknown assisted-ai run: {run_id:?}")]
    UnknownAiRun {
        /// Unknown run id.
        run_id: AgentRunId,
    },
    /// Plugin id was not present in current contribution projections.
    #[error("unknown plugin: {plugin_id:?}")]
    UnknownPlugin {
        /// Unknown plugin id.
        plugin_id: PluginId,
    },
    /// Plugin command id was empty after normalization.
    #[error("plugin command id is empty for plugin {plugin_id:?}")]
    InvalidPluginCommand {
        /// Plugin id for the invalid command request.
        plugin_id: PluginId,
    },
    /// Command id was not present in the selected plugin projection.
    #[error("unknown plugin command: plugin {plugin_id:?} command {command_id}")]
    UnknownPluginCommand {
        /// Plugin id that was present.
        plugin_id: PluginId,
        /// Unknown command id.
        command_id: String,
    },
    /// Target buffer does not own the active dirty-close prompt.
    #[error("dirty-close prompt is not active for buffer {buffer_id:?}")]
    DirtyClosePromptMissing {
        /// Target buffer without an active prompt.
        buffer_id: BufferId,
    },
    /// Assisted-AI start action was missing a display-safe instruction label.
    #[error("assisted-ai instruction label is empty")]
    InvalidInstructionLabel,
    /// Path text was empty after trimming.
    #[error("path input is empty")]
    InvalidPathInput,
    /// The action is intentionally not supported by this phase.
    #[error("unsupported desktop action: {action}")]
    UnsupportedAction {
        /// Unsupported action label.
        action: &'static str,
    },
    /// A terminal action requires an active terminal session projection.
    #[error("active terminal session is required for this desktop action")]
    MissingActiveTerminalSession,
}

/// Adapter-local command bridge.
#[derive(Debug, Default)]
pub struct DesktopCommandBridge;

impl DesktopCommandBridge {
    /// Creates a bridge that owns no app/editor/workspace state.
    pub fn new() -> Self {
        Self
    }

    /// Translate a desktop action into a command intent, app request, no-op, or typed error.
    pub fn translate(
        &self,
        action: DesktopAction,
        snapshot: &ShellProjectionSnapshot,
    ) -> DesktopBridgeOutput {
        match action {
            DesktopAction::Quit => DesktopBridgeOutput::Intent(CommandDispatchIntent::Quit),
            DesktopAction::SaveActive => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::Save { buffer_id }
            }),
            DesktopAction::SaveAll => DesktopBridgeOutput::Intent(CommandDispatchIntent::SaveAll),
            DesktopAction::SwitchTab { buffer_id } => {
                self.with_known_tab(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::SwitchTab { buffer_id }
                })
            }
            DesktopAction::CloseTab { buffer_id } => {
                self.with_known_tab(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::CloseTab { buffer_id }
                })
            }
            DesktopAction::SaveDirtyClose { buffer_id } => {
                self.with_dirty_close_prompt(snapshot, buffer_id, |buffer_id| {
                    DesktopBridgeOutput::Intent(CommandDispatchIntent::Save { buffer_id })
                })
            }
            DesktopAction::CancelDirtyClose { buffer_id } => {
                self.with_dirty_close_prompt(snapshot, buffer_id, |buffer_id| {
                    DesktopBridgeOutput::AppRequest(DesktopAppRequest::CancelDirtyClose {
                        buffer_id,
                    })
                })
            }
            DesktopAction::OpenPathText(path) | DesktopAction::OpenPathDialogSelected(path) => {
                match normalized_path(path) {
                    Some(path) => {
                        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPath { path })
                    }
                    None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPathInput),
                }
            }
            DesktopAction::OpenPathDialogCancelled => DesktopBridgeOutput::Noop,
            DesktopAction::ShowOpenPathPrompt => {
                DesktopBridgeOutput::AppRequest(DesktopAppRequest::ShowOpenPathPrompt)
            }
            DesktopAction::ShowSearchPrompt { scope } => {
                DesktopBridgeOutput::AppRequest(DesktopAppRequest::ShowSearchPrompt { scope })
            }
            DesktopAction::OpenWorkspace { root } => {
                DesktopBridgeOutput::AppRequest(DesktopAppRequest::OpenWorkspace { root })
            }
            DesktopAction::RefreshExplorer => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshExplorer)
            }
            DesktopAction::ToggleExplorerPath { path } => match normalized_path(path) {
                Some(path) => {
                    DesktopBridgeOutput::AppRequest(DesktopAppRequest::ToggleExplorerPath { path })
                }
                None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPathInput),
            },
            DesktopAction::SelectExplorerFile { file_id } => {
                if explorer_contains_file(snapshot, file_id) {
                    DesktopBridgeOutput::Intent(CommandDispatchIntent::RevealInExplorer { file_id })
                } else {
                    DesktopBridgeOutput::Error(DesktopBridgeError::UnknownExplorerFile { file_id })
                }
            }
            DesktopAction::PreviewProposal { proposal_id } => {
                self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                    CommandDispatchIntent::PreviewProposal { proposal_id }
                })
            }
            DesktopAction::ApproveProposal { proposal_id } => {
                self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                    CommandDispatchIntent::ApproveProposal { proposal_id }
                })
            }
            DesktopAction::RejectProposal {
                proposal_id,
                reason,
            } => self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                CommandDispatchIntent::RejectProposal {
                    proposal_id,
                    reason,
                }
            }),
            DesktopAction::ApplyProposal { proposal_id } => {
                self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                    CommandDispatchIntent::ApplyProposal { proposal_id }
                })
            }
            DesktopAction::RollbackProposal {
                proposal_id,
                reason,
            } => self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                CommandDispatchIntent::RollbackProposal {
                    proposal_id,
                    reason,
                }
            }),
            DesktopAction::CancelProposal {
                proposal_id,
                reason,
            } => self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                CommandDispatchIntent::CancelProposal {
                    proposal_id,
                    reason,
                }
            }),
            DesktopAction::OpenProposalDetails { proposal_id } => {
                self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                    CommandDispatchIntent::OpenProposalDetails { proposal_id }
                })
            }
            DesktopAction::StartAiExplain { instruction_label } => {
                match normalized_instruction(instruction_label) {
                    Some(instruction_label) => {
                        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiExplain {
                            instruction_label,
                        })
                    }
                    None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidInstructionLabel),
                }
            }
            DesktopAction::StartAiProposal { instruction_label } => {
                match normalized_instruction(instruction_label) {
                    Some(instruction_label) => {
                        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiProposal {
                            instruction_label,
                        })
                    }
                    None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidInstructionLabel),
                }
            }
            DesktopAction::CancelAiRun { run_id } => {
                self.with_known_ai_run(snapshot, run_id, |run_id| {
                    CommandDispatchIntent::CancelAiRun { run_id }
                })
            }
            DesktopAction::ReplayAiRun { run_id } => {
                self.with_known_ai_run(snapshot, run_id, |run_id| {
                    CommandDispatchIntent::ReplayAiRun { run_id }
                })
            }
            DesktopAction::InspectAiRun { run_id } => {
                self.with_known_ai_run(snapshot, run_id, |run_id| {
                    CommandDispatchIntent::InspectAiRun { run_id }
                })
            }
            DesktopAction::InvokePluginCommand {
                plugin_id,
                command_id,
            } => self.with_known_plugin_command(snapshot, plugin_id, command_id),
            DesktopAction::InsertText { text, at }
            | DesktopAction::ClipboardPaste { text, at }
            | DesktopAction::ImeCommit { text, at } => {
                self.with_active_buffer(snapshot, |buffer_id| CommandDispatchIntent::Insert {
                    buffer_id,
                    at,
                    text,
                })
            }
            DesktopAction::ReplaceRange { range, replacement } => {
                self.with_active_buffer(snapshot, |buffer_id| CommandDispatchIntent::Replace {
                    buffer_id,
                    range,
                    replacement,
                })
            }
            DesktopAction::DeleteRange { range } => {
                self.with_active_buffer(snapshot, |buffer_id| CommandDispatchIntent::Delete {
                    buffer_id,
                    range,
                })
            }
            DesktopAction::Undo => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::Undo { buffer_id }
            }),
            DesktopAction::Redo => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::Redo { buffer_id }
            }),
            DesktopAction::SetCursor { buffer_id, cursor } => {
                self.with_resolved_buffer(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::SetCursor { buffer_id, cursor }
                })
            }
            DesktopAction::SetSelection { buffer_id, range } => {
                self.with_resolved_buffer(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::SetSelection { buffer_id, range }
                })
            }
            DesktopAction::SetViewportScroll { buffer_id, scroll } => {
                self.with_resolved_buffer(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::SetViewportScroll { buffer_id, scroll }
                })
            }
            DesktopAction::RunSearch {
                scope,
                query,
                limit,
            } => DesktopBridgeOutput::Intent(CommandDispatchIntent::RunSearch {
                scope,
                query,
                limit,
            }),
            DesktopAction::CancelSearch { query_id } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelSearch { query_id })
            }
            DesktopAction::RequestHover { position } => {
                self.with_active_buffer(snapshot, |buffer_id| CommandDispatchIntent::RequestHover {
                    buffer_id,
                    position,
                })
            }
            DesktopAction::RequestCompletion { position } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestCompletion {
                        buffer_id,
                        position,
                    }
                })
            }
            DesktopAction::GoToDefinition { position } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::GoToDefinition {
                        buffer_id,
                        position,
                    }
                })
            }
            DesktopAction::FindReferences { position } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::FindReferences {
                        buffer_id,
                        position,
                    }
                })
            }
            DesktopAction::RefreshOutline => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::RefreshOutline { buffer_id }
            }),
            DesktopAction::RequestFormattingProposal => self
                .with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestFormattingProposal { buffer_id }
                }),
            DesktopAction::RequestRenameProposal { position, new_name } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestRenameProposal {
                        buffer_id,
                        position,
                        new_name,
                    }
                })
            }
            DesktopAction::RequestOrganizeImportsProposal => self
                .with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestOrganizeImportsProposal { buffer_id }
                }),
            DesktopAction::RequestCodeActionProposal { action_id } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestCodeActionProposal {
                        buffer_id,
                        action_id,
                    }
                })
            }
            DesktopAction::CancelLanguageOperation { operation_id } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelLanguageOperation {
                    operation_id,
                })
            }
            DesktopAction::TerminalLaunch { command_label } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::TerminalLaunch { command_label })
            }
            DesktopAction::TerminalInput { payload } => {
                self.with_active_terminal(snapshot, |session_id| {
                    CommandDispatchIntent::TerminalInput {
                        session_id,
                        payload,
                    }
                })
            }
            DesktopAction::TerminalResize { cols, rows } => {
                self.with_active_terminal(snapshot, |session_id| {
                    CommandDispatchIntent::TerminalResize {
                        session_id,
                        cols,
                        rows,
                    }
                })
            }
            DesktopAction::TerminalKill => self.with_active_terminal(snapshot, |session_id| {
                CommandDispatchIntent::TerminalKill { session_id }
            }),
            DesktopAction::TerminalClose => self.with_active_terminal(snapshot, |session_id| {
                CommandDispatchIntent::TerminalClose { session_id }
            }),
            DesktopAction::TerminalOutputPoll => self
                .with_active_terminal(snapshot, |session_id| {
                    CommandDispatchIntent::TerminalOutputPoll { session_id }
                }),
            DesktopAction::TerminalSearch { query } => self
                .with_active_terminal(snapshot, |session_id| {
                    CommandDispatchIntent::TerminalSearch { session_id, query }
                }),
        }
    }

    fn with_active_buffer(
        &self,
        snapshot: &ShellProjectionSnapshot,
        build: impl FnOnce(BufferId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        match snapshot.active_buffer_projection.buffer_id {
            Some(buffer_id) => DesktopBridgeOutput::Intent(build(buffer_id)),
            None => DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveBuffer),
        }
    }

    fn with_active_terminal(
        &self,
        snapshot: &ShellProjectionSnapshot,
        build: impl FnOnce(TerminalSessionId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        match snapshot.terminal_panel_projection.active_session_id {
            Some(session_id) => DesktopBridgeOutput::Intent(build(session_id)),
            None => DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveTerminalSession),
        }
    }

    fn with_resolved_buffer(
        &self,
        snapshot: &ShellProjectionSnapshot,
        requested: Option<BufferId>,
        build: impl FnOnce(BufferId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        let Some(buffer_id) = requested
            .or(snapshot.daily_editing_projection.tabs.active_buffer_id)
            .or(snapshot.active_buffer_projection.buffer_id)
        else {
            return DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveBuffer);
        };

        self.with_known_tab(snapshot, buffer_id, build)
    }

    fn with_known_tab(
        &self,
        snapshot: &ShellProjectionSnapshot,
        buffer_id: BufferId,
        build: impl FnOnce(BufferId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if tab_is_known(snapshot, buffer_id) {
            DesktopBridgeOutput::Intent(build(buffer_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownTab { buffer_id })
        }
    }

    fn with_dirty_close_prompt(
        &self,
        snapshot: &ShellProjectionSnapshot,
        buffer_id: BufferId,
        build: impl FnOnce(BufferId) -> DesktopBridgeOutput,
    ) -> DesktopBridgeOutput {
        if !tab_is_known(snapshot, buffer_id) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownTab { buffer_id });
        }
        if snapshot
            .daily_editing_projection
            .close_dirty_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.buffer_id == buffer_id)
        {
            build(buffer_id)
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::DirtyClosePromptMissing { buffer_id })
        }
    }

    fn with_known_proposal(
        &self,
        snapshot: &ShellProjectionSnapshot,
        proposal_id: ProposalId,
        build: impl FnOnce(ProposalId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if proposal_is_known(snapshot, proposal_id) {
            DesktopBridgeOutput::Intent(build(proposal_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownProposal { proposal_id })
        }
    }

    fn with_known_ai_run(
        &self,
        snapshot: &ShellProjectionSnapshot,
        run_id: AgentRunId,
        build: impl FnOnce(AgentRunId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if assisted_ai_projection_references_run(snapshot, &run_id) {
            DesktopBridgeOutput::Intent(build(run_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownAiRun { run_id })
        }
    }

    fn with_known_plugin_command(
        &self,
        snapshot: &ShellProjectionSnapshot,
        plugin_id: PluginId,
        command_id: String,
    ) -> DesktopBridgeOutput {
        let Some(command_id) = normalized_plugin_command(command_id) else {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPluginCommand {
                plugin_id,
            });
        };
        let Some(projection) = snapshot
            .plugin_contribution_projections
            .iter()
            .find(|projection| projection.plugin_id == plugin_id)
        else {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownPlugin { plugin_id });
        };
        let Some(command) = projection
            .contributions
            .iter()
            .filter_map(|contribution| match contribution {
                PluginContribution::Command(command) => Some(command),
                _ => None,
            })
            .find(|command| command.command_id == command_id)
        else {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownPluginCommand {
                plugin_id,
                command_id,
            });
        };

        DesktopBridgeOutput::Intent(CommandDispatchIntent::InvokePluginCommand {
            plugin_id,
            command_id: command.command_id.clone(),
            metadata_label: plugin_command_metadata_label(
                plugin_id,
                &command.command_id,
                &command.title,
                &command.required_capability.0,
                &projection.status_label,
            ),
        })
    }
}

fn normalized_path(path: String) -> Option<String> {
    let path = path.trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

fn normalized_instruction(label: String) -> Option<String> {
    let label = label.trim();
    if label.is_empty() {
        None
    } else {
        Some(label.to_string())
    }
}

fn normalized_plugin_command(command_id: String) -> Option<String> {
    let command_id = command_id.trim();
    if command_id.is_empty() {
        None
    } else {
        Some(command_id.to_string())
    }
}

fn plugin_command_metadata_label(
    plugin_id: PluginId,
    command_id: &str,
    title: &str,
    capability: &str,
    status_label: &str,
) -> String {
    format!(
        "plugin {} command {command_id}: {title} (status={status_label} capability={capability})",
        plugin_id.0
    )
}

fn tab_is_known(snapshot: &ShellProjectionSnapshot, buffer_id: BufferId) -> bool {
    let tabs = &snapshot.daily_editing_projection.tabs.tabs;
    if tabs.is_empty() {
        return snapshot.active_buffer_projection.buffer_id == Some(buffer_id);
    }

    tabs.iter().any(|tab| tab.buffer_id == buffer_id)
}

fn explorer_contains_file(snapshot: &ShellProjectionSnapshot, file_id: FileId) -> bool {
    snapshot
        .explorer_projection
        .nodes
        .iter()
        .any(|node| node.file_id == file_id)
}

fn proposal_is_known(snapshot: &ShellProjectionSnapshot, proposal_id: ProposalId) -> bool {
    snapshot
        .proposal_ledger_projection
        .rows
        .iter()
        .any(|row| row.proposal_id == proposal_id)
}

fn assisted_ai_projection_references_run(
    snapshot: &ShellProjectionSnapshot,
    run_id: &AgentRunId,
) -> bool {
    let needle = run_id.0.trim();
    if needle.is_empty() {
        return false;
    }

    projected_assisted_run_id(snapshot).is_some_and(|projected| projected == needle)
}

fn projected_assisted_run_id(snapshot: &ShellProjectionSnapshot) -> Option<&str> {
    let projection_id = snapshot.assisted_ai_projection.projection_id.as_str();
    let run_index = projection_id.rfind("phase4-run-")?;
    Some(&projection_id[run_index..])
}
