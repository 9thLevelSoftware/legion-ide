//! Application composition root for workspace/editor/ui orchestration.

#![warn(missing_docs)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use devil_editor::{EditorEngine, EditorError, SaveRequestDto, TextEdit};
use devil_platform::{NativeFileSystem, NativeWatcherService};
use devil_project::{WorkspaceActor, WorkspaceError};
use devil_protocol::{
    BufferId, CanonicalPath, CapabilityNamespace, FileId, FileTreeNode, PrincipalId,
    TextTransactionDescriptor, TransactionSource, WorkspaceId, WorkspaceOpenRequest,
    WorkspaceOpened, WorkspaceTrustState,
};
use devil_security::{DenyByDefaultBroker, SecurityPolicy};
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
    Saved(SaveRequestDto),
    /// Explorer projection was refreshed from workspace tree state.
    ExplorerRefreshed(ExplorerProjection),
    /// A workspace path was opened and bound to an editor buffer.
    Opened(FileId),
}

/// Root application composition.
pub struct AppComposition {
    workspace: WorkspaceActor,
    editor: EditorEngine,
    opened_workspace: Option<WorkspaceOpened>,
    active_file_id: Option<FileId>,
    active_file_path: Option<String>,
    active_buffer_id: Option<devil_protocol::BufferId>,
}

impl AppComposition {
    /// Build composition with native platform adapters and default-deny security broker.
    pub fn new() -> Self {
        let fs = Arc::new(NativeFileSystem);
        let watcher = Arc::new(NativeWatcherService);
        let security = DenyByDefaultBroker::new(
            SecurityPolicy::default(),
            CapabilityNamespace("app".to_string()),
        );

        Self {
            workspace: WorkspaceActor::new(fs, watcher, security),
            editor: EditorEngine::new(),
            opened_workspace: None,
            active_file_id: None,
            active_file_path: None,
            active_buffer_id: None,
        }
    }

    /// Open a workspace.
    pub fn open_workspace(
        &mut self,
        root: impl AsRef<Path>,
        trust: WorkspaceTrustState,
        principal: PrincipalId,
    ) -> Result<WorkspaceOpened, AppCompositionError> {
        let request = WorkspaceOpenRequest {
            correlation_id: devil_protocol::CorrelationId(1),
            principal_id: principal,
            root_path: CanonicalPath(root.as_ref().to_string_lossy().into_owned()),
            trust: Some(trust),
        };

        let opened = self.workspace.open_workspace(request)?;
        self.opened_workspace = Some(opened.clone());
        self.active_file_id = None;
        self.active_file_path = None;
        self.active_buffer_id = None;
        Ok(opened)
    }

    /// Open a file through workspace authority and bind it into editor engine.
    pub fn open_file(&mut self, path: impl AsRef<str>) -> Result<FileId, AppCompositionError> {
        let workspace_id = self
            .opened_workspace
            .as_ref()
            .map(|opened| opened.workspace_id)
            .ok_or(AppCompositionError::WorkspaceNotOpen)?;

        let identity = self.workspace.resolve_file(workspace_id, path.as_ref())?;
        let text = self
            .workspace
            .read_file_text(workspace_id, identity.canonical_path.0.as_str())
            .unwrap_or_default();

        let buffer_id = self.editor.open_buffer(
            workspace_id,
            identity.file_id,
            identity.canonical_path.0.clone(),
            text,
        )?;

        self.active_file_id = Some(identity.file_id);
        self.active_file_path = Some(identity.canonical_path.0);
        self.active_buffer_id = Some(buffer_id);
        Ok(identity.file_id)
    }

    /// Apply an edit command directly to the active editor-engine buffer.
    pub fn edit_active_buffer(
        &mut self,
        edit: TextEdit,
    ) -> Result<TextTransactionDescriptor, AppCompositionError> {
        let buffer_id = self
            .active_buffer_id
            .ok_or(AppCompositionError::ActiveBufferMissing)?;
        self.apply_edit_to_buffer(buffer_id, edit)
    }

    /// Route a UI dispatch intent through editor and workspace authorities.
    pub fn dispatch_ui_intent(
        &mut self,
        intent: CommandDispatchIntent,
    ) -> Result<AppCommandOutcome, AppCompositionError> {
        match intent {
            CommandDispatchIntent::Noop => Ok(AppCommandOutcome::Noop),
            CommandDispatchIntent::Quit => Ok(AppCommandOutcome::Quit),
            CommandDispatchIntent::Undo { buffer_id } => {
                self.ensure_active_buffer(buffer_id)?;
                let record = self.editor.undo(buffer_id, None)?;
                Ok(AppCommandOutcome::Edited(record.to_protocol_descriptor()))
            }
            CommandDispatchIntent::Redo { buffer_id } => {
                self.ensure_active_buffer(buffer_id)?;
                let record = self.editor.redo(buffer_id, None)?;
                Ok(AppCommandOutcome::Edited(record.to_protocol_descriptor()))
            }
            CommandDispatchIntent::Insert {
                buffer_id,
                at,
                text,
            } => Ok(AppCommandOutcome::Edited(
                self.apply_edit_to_buffer(buffer_id, TextEdit::insert(at, text))?,
            )),
            CommandDispatchIntent::Delete { buffer_id, range } => Ok(AppCommandOutcome::Edited(
                self.apply_edit_to_buffer(buffer_id, TextEdit::delete(range))?,
            )),
            CommandDispatchIntent::Replace {
                buffer_id,
                range,
                replacement,
            } => Ok(AppCommandOutcome::Edited(self.apply_edit_to_buffer(
                buffer_id,
                TextEdit::new(range, replacement),
            )?)),
            CommandDispatchIntent::Save { buffer_id } => {
                self.ensure_active_buffer(buffer_id)?;
                Ok(AppCommandOutcome::Saved(self.save_active_buffer()?))
            }
            CommandDispatchIntent::OpenPath { path } => {
                Ok(AppCommandOutcome::Opened(self.open_file(path)?))
            }
            CommandDispatchIntent::RefreshExplorer => Ok(AppCommandOutcome::ExplorerRefreshed(
                self.explorer_projection()?,
            )),
            CommandDispatchIntent::RevealInExplorer { file_id } => {
                self.active_file_id = Some(file_id);
                Ok(AppCommandOutcome::ExplorerRefreshed(
                    self.explorer_projection()?,
                ))
            }
        }
    }

    /// Save currently active buffer through editor save request and workspace write authority.
    pub fn save_active_buffer(&mut self) -> Result<SaveRequestDto, AppCompositionError> {
        let workspace_id = self
            .opened_workspace
            .as_ref()
            .map(|opened| opened.workspace_id)
            .ok_or(AppCompositionError::WorkspaceNotOpen)?;
        let buffer_id = self
            .active_buffer_id
            .ok_or(AppCompositionError::ActiveBufferMissing)?;
        let file_path = self
            .active_file_path
            .as_ref()
            .ok_or(AppCompositionError::ActiveFileMissing)?
            .clone();
        let workspace_relative_path = self
            .workspace
            .current_workspace_root()
            .ok()
            .and_then(|root| {
                std::path::Path::new(&file_path)
                    .strip_prefix(root)
                    .ok()
                    .map(|path| path.to_string_lossy().into_owned())
            })
            .unwrap_or(file_path);

        let save = self.editor.request_save(buffer_id, None)?;
        self.workspace
            .write_file_text(workspace_id, workspace_relative_path, &save.text)?;
        self.editor.acknowledge_save(save.request_id, true);
        Ok(save)
    }

    /// Build active-buffer projection from editor-engine state.
    pub fn active_buffer_projection(&self) -> Result<ActiveBufferProjection, AppCompositionError> {
        let Some(buffer_id) = self.active_buffer_id else {
            return Ok(ActiveBufferProjection::empty());
        };

        Ok(ActiveBufferProjection {
            workspace_id: self.workspace_id(),
            buffer_id: Some(buffer_id),
            file_id: self.active_file_id,
            file_path: self
                .active_file_path
                .as_ref()
                .map(|path| CanonicalPath(path.clone())),
            text: self.editor.text(buffer_id)?.to_string(),
            dirty: self.editor.is_dirty(buffer_id)?,
        })
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
        let nodes = self.workspace.tree_snapshot()?;
        Ok(self.project_tree(nodes))
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
        self.active_buffer_id
    }

    /// Expose active file id for integration validation.
    pub fn active_file_id(&self) -> Option<FileId> {
        self.active_file_id
    }

    /// Expose active workspace id.
    pub fn workspace_id(&self) -> Option<WorkspaceId> {
        self.opened_workspace
            .as_ref()
            .map(|opened| opened.workspace_id)
    }

    fn project_tree(&self, tree: Vec<FileTreeNode>) -> ExplorerProjection {
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
            selection: self
                .active_file_id
                .map(|file_id| ExplorerSelectionProjection { file_id }),
        }
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

    fn apply_edit_to_buffer(
        &mut self,
        buffer_id: BufferId,
        edit: TextEdit,
    ) -> Result<TextTransactionDescriptor, AppCompositionError> {
        self.ensure_active_buffer(buffer_id)?;
        let record =
            self.editor
                .apply_edit(buffer_id, edit, TransactionSource::User, None, None)?;
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
