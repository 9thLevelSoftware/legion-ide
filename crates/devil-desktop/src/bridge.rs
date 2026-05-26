//! Desktop event to app-command bridge.

use std::path::PathBuf;

use devil_protocol::{BufferId, ProtocolTextRange, TextCoordinate};
use devil_ui::{CommandDispatchIntent, ShellProjectionSnapshot};
use thiserror::Error;

/// Adapter-local renderer action before app routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopAction {
    /// Quit the desktop shell.
    Quit,
    /// Save the active buffer through app authority.
    SaveActive,
    /// Open a user-entered path through workspace authority.
    OpenPathText(String),
    /// Open a path selected by a native file dialog.
    OpenPathDialogSelected(String),
    /// Native file dialog was cancelled.
    OpenPathDialogCancelled,
    /// Ask the workflow layer to show an open-path prompt.
    ShowOpenPathPrompt,
    /// Ask the workflow layer to open a workspace root.
    OpenWorkspace {
        /// Workspace root selected by the adapter.
        root: PathBuf,
    },
    /// Refresh the explorer projection through app authority.
    RefreshExplorer,
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
            DesktopAction::OpenWorkspace { root } => {
                DesktopBridgeOutput::AppRequest(DesktopAppRequest::OpenWorkspace { root })
            }
            DesktopAction::RefreshExplorer => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshExplorer)
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
}

fn normalized_path(path: String) -> Option<String> {
    let path = path.trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}
