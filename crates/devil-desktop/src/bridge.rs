//! Desktop event to app-command bridge.

use std::path::PathBuf;

use devil_protocol::{BufferId, FileId, ProtocolTextRange, TextCoordinate, ViewportScroll};
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
    /// Path text was empty after trimming.
    #[error("path input is empty")]
    InvalidPathInput,
    /// The action is intentionally not supported by this phase.
    #[error("unsupported desktop action: {action}")]
    UnsupportedAction {
        /// Unsupported action label.
        action: &'static str,
    },
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
}

fn normalized_path(path: String) -> Option<String> {
    let path = path.trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
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
