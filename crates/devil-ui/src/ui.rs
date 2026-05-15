//! Projection-only UI primitives for the native shell.

use devil_editor::{TextPosition, TextRange};
use devil_protocol::{BufferId, CanonicalPath, FileId, WorkspaceId};
use thiserror::Error;

/// Render mode for shell projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Basic projection listing.
    Plain,
}

/// Explorer tree projection consumed by shell-style UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerProjection {
    /// Flat node list from workspace tree snapshot.
    pub nodes: Vec<ExplorerNodeProjection>,
    /// Optional selected node in the explorer.
    pub selection: Option<ExplorerSelectionProjection>,
}

/// Projected explorer node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerNodeProjection {
    /// Stable file identifier.
    pub file_id: FileId,
    /// Canonical file path.
    pub canonical_path: CanonicalPath,
    /// Display name for UI list/tree rows.
    pub name: String,
    /// Child identifiers for directory rows.
    pub children: Vec<FileId>,
}

/// Projected explorer selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerSelectionProjection {
    /// Selected file identifier.
    pub file_id: FileId,
}

/// Minimal layout model used by the shell projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Window title for the shell.
    pub title: String,
    /// Width of the frame.
    pub width: u16,
    /// Height of the frame.
    pub height: u16,
}

impl Layout {
    /// Construct a layout.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: 80,
            height: 24,
        }
    }
}

/// Top-level layout projection consumed by the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellLayoutProjection {
    /// Window layout.
    pub layout: Layout,
    /// Current render mode.
    pub mode: RenderMode,
}

impl ShellLayoutProjection {
    /// Construct a plain layout projection.
    pub fn plain(title: impl Into<String>) -> Self {
        Self {
            layout: Layout::new(title),
            mode: RenderMode::Plain,
        }
    }
}

/// Active editor-buffer projection received by the UI from application state.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveBufferProjection {
    /// Owning workspace identifier if a workspace is open.
    pub workspace_id: Option<WorkspaceId>,
    /// Active editor buffer identifier.
    pub buffer_id: Option<BufferId>,
    /// Active workspace file identifier.
    pub file_id: Option<FileId>,
    /// Canonical path for display only.
    pub file_path: Option<CanonicalPath>,
    /// Bounded viewport projection instead of unbounded text.
    pub viewport: Option<devil_protocol::ViewportProjection>,
    /// Degraded status from the application layer.
    pub degraded: bool,
    /// Bounded small-buffer preview, requested explicitly.
    pub small_buffer_preview: Option<String>,
    /// Dirty indicator projected from the editor engine.
    pub dirty: bool,
}

impl ActiveBufferProjection {
    /// Construct an empty active-buffer projection.
    pub fn empty() -> Self {
        Self {
            workspace_id: None,
            buffer_id: None,
            file_id: None,
            file_path: None,
            viewport: None,
            degraded: false,
            small_buffer_preview: None,
            dirty: false,
        }
    }

    /// Return a bounded small-buffer preview if available.
    pub fn small_buffer_text(&self) -> Option<&str> {
        self.small_buffer_preview.as_deref()
    }
}

impl Default for ActiveBufferProjection {
    fn default() -> Self {
        Self::empty()
    }
}

/// UI status severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusSeverity {
    /// Informational status message.
    Info,
    /// Warning status message.
    Warning,
    /// Error status message.
    Error,
}

/// Projected status message shown by the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMessageProjection {
    /// Severity classification.
    pub severity: StatusSeverity,
    /// Human-readable message.
    pub message: String,
}

/// Typed command intent emitted by UI input handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDispatchIntent {
    /// No command was recognized.
    Noop,
    /// Quit the active shell loop.
    Quit,
    /// Undo through the editor engine for the target buffer.
    Undo {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Redo through the editor engine for the target buffer.
    Redo {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Insert text through the editor engine for the target buffer.
    Insert {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Insertion position in projected text coordinates.
        at: TextPosition,
        /// Replacement payload.
        text: String,
    },
    /// Delete a text range through the editor engine for the target buffer.
    Delete {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Range to delete.
        range: TextRange,
    },
    /// Replace a text range through the editor engine for the target buffer.
    Replace {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Range to replace.
        range: TextRange,
        /// Replacement payload.
        replacement: String,
    },
    /// Save through the editor save-request and workspace write path.
    Save {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Open a file by path through workspace authority.
    OpenPath {
        /// User-provided path text.
        path: String,
    },
    /// Refresh explorer state through workspace ports.
    RefreshExplorer,
    /// Reveal a workspace file in the explorer projection.
    RevealInExplorer {
        /// File identifier to reveal.
        file_id: FileId,
    },
}

/// Projection snapshot provided to the shell by the application layer.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellProjectionSnapshot {
    /// Layout projection.
    pub layout_projection: ShellLayoutProjection,
    /// Explorer projection.
    pub explorer_projection: ExplorerProjection,
    /// Active buffer projection.
    pub active_buffer_projection: ActiveBufferProjection,
    /// Status message projections.
    pub status_messages: Vec<StatusMessageProjection>,
}

/// Command parsing errors surfaced by projection-only shell input handling.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ShellCommandError {
    /// A command requires an active buffer projection, but none is present.
    #[error("active buffer projection is missing")]
    ActiveBufferMissing,
    /// A command supplied a range with start after end.
    #[error("command range start must be <= end")]
    InvalidRange,
}

/// Projection-only IDE shell state.
#[derive(Debug)]
pub struct Shell {
    /// Projection-only layout state.
    pub layout_projection: ShellLayoutProjection,
    /// Projection-only explorer state.
    pub explorer_projection: ExplorerProjection,
    /// Projection-only active buffer state.
    pub active_buffer_projection: ActiveBufferProjection,
    /// Projected status messages.
    pub status_messages: Vec<StatusMessageProjection>,
    /// Command dispatch intents emitted by input parsing.
    pub command_dispatch_intents: Vec<CommandDispatchIntent>,
}

impl Shell {
    /// Create a shell from a projection snapshot.
    pub fn new(snapshot: ShellProjectionSnapshot) -> Self {
        Self {
            layout_projection: snapshot.layout_projection,
            explorer_projection: snapshot.explorer_projection,
            active_buffer_projection: snapshot.active_buffer_projection,
            status_messages: snapshot.status_messages,
            command_dispatch_intents: Vec::new(),
        }
    }

    /// Create an empty projection-only shell.
    pub fn empty(title: impl Into<String>) -> Self {
        Self::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain(title),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
        })
    }

    /// Return a cloned shell projection snapshot.
    pub fn projection_snapshot(&self) -> ShellProjectionSnapshot {
        ShellProjectionSnapshot {
            layout_projection: self.layout_projection.clone(),
            explorer_projection: self.explorer_projection.clone(),
            active_buffer_projection: self.active_buffer_projection.clone(),
            status_messages: self.status_messages.clone(),
        }
    }

    /// Replace all render projections at once.
    pub fn replace_projection_snapshot(&mut self, snapshot: ShellProjectionSnapshot) {
        self.layout_projection = snapshot.layout_projection;
        self.explorer_projection = snapshot.explorer_projection;
        self.active_buffer_projection = snapshot.active_buffer_projection;
        self.status_messages = snapshot.status_messages;
    }

    /// Drain queued command-dispatch intents.
    pub fn drain_command_dispatch_intents(&mut self) -> Vec<CommandDispatchIntent> {
        self.command_dispatch_intents.drain(..).collect()
    }

    /// Render basic status and file content.
    pub fn render(&self) {
        print!("\x1b[2J\x1b[H");
        println!("{}", self.layout_projection.layout.title);
        println!(
            "Mode: {:?} | {}x{}",
            self.layout_projection.mode,
            self.layout_projection.layout.width,
            self.layout_projection.layout.height
        );
        println!(
            "{}",
            "-".repeat(self.layout_projection.layout.width as usize)
        );

        if self.active_buffer_projection.degraded {
            println!("<Degraded Mode: Large File>");
        }

        if let Some(text) = self.active_buffer_projection.small_buffer_text() {
            println!("{}", text);
        } else if let Some(viewport) = &self.active_buffer_projection.viewport {
            for slice in &viewport.line_slices {
                println!("{}", slice.visible_text);
            }
        } else {
            println!("<no active buffer>");
        }

        println!(
            "{}",
            "-".repeat(self.layout_projection.layout.width as usize)
        );
        let path = self
            .active_buffer_projection
            .file_path
            .as_ref()
            .map(|path| path.0.as_str())
            .unwrap_or("<no active file>");
        println!("Path: {}", path);
        println!("Commands: :i text | :d start,end | :r start,end,text | :w | :u | :redo | :q");
    }

    /// Parse a command and emit a typed dispatch intent without mutating editor or workspace state.
    pub fn handle_command(
        &mut self,
        input: &str,
    ) -> Result<Option<CommandDispatchIntent>, ShellCommandError> {
        let trimmed = input.trim();
        if trimmed == ":q" {
            return Ok(Some(self.push_intent(CommandDispatchIntent::Quit)));
        }
        if trimmed == ":u" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Undo { buffer_id }),
            ));
        }
        if trimmed == ":redo" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Redo { buffer_id }),
            ));
        }
        if trimmed == ":w" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Save { buffer_id }),
            ));
        }

        if let Some(payload) = trimmed.strip_prefix(":i ") {
            let buffer_id = self.active_buffer_id()?;
            let pos = TextPosition::new(0, 0);
            return Ok(Some(self.push_intent(CommandDispatchIntent::Insert {
                buffer_id,
                at: pos,
                text: payload.to_string(),
            })));
        }

        if let Some(payload) = trimmed.strip_prefix(":d ") {
            let buffer_id = self.active_buffer_id()?;
            let mut split = payload.split(',');
            let start = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let end = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            if start > end {
                return Err(ShellCommandError::InvalidRange);
            }
            let start = self.parse_pos(start);
            let end = self.parse_pos(end);
            return Ok(Some(self.push_intent(CommandDispatchIntent::Delete {
                buffer_id,
                range: TextRange::new(start, end),
            })));
        }

        if let Some(payload) = trimmed.strip_prefix(":r ") {
            let buffer_id = self.active_buffer_id()?;
            let mut split = payload.splitn(3, ',');
            let start = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let end = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let replacement = split.next().unwrap_or("");
            if start > end {
                return Err(ShellCommandError::InvalidRange);
            }
            let start = self.parse_pos(start);
            let end = self.parse_pos(end);
            return Ok(Some(self.push_intent(CommandDispatchIntent::Replace {
                buffer_id,
                range: TextRange::new(start, end),
                replacement: replacement.to_string(),
            })));
        }

        Ok(Some(self.push_intent(CommandDispatchIntent::Noop)))
    }

    fn active_buffer_id(&self) -> Result<BufferId, ShellCommandError> {
        self.active_buffer_projection
            .buffer_id
            .ok_or(ShellCommandError::ActiveBufferMissing)
    }

    fn push_intent(&mut self, intent: CommandDispatchIntent) -> CommandDispatchIntent {
        self.command_dispatch_intents.push(intent.clone());
        intent
    }

    fn parse_pos(&self, byte_offset: usize) -> TextPosition {
        if let Some(text) = self.active_buffer_projection.small_buffer_text() {
            return text
                .as_bytes()
                .get(..byte_offset)
                .map(|prefix| {
                    let line = prefix.iter().filter(|b| **b == b'\n').count();
                    let column = prefix.iter().rev().take_while(|b| **b != b'\n').count();
                    TextPosition::new(line, column)
                })
                .unwrap_or_else(|| TextPosition::new(0, 0));
        }

        if let Some(viewport) = &self.active_buffer_projection.viewport {
            let mut current_offset = 0;
            for (i, slice) in viewport.line_slices.iter().enumerate() {
                let slice_len = slice.visible_text.len() + 1; // +1 for newline
                if current_offset + slice_len > byte_offset {
                    let column = byte_offset - current_offset;
                    let line = viewport.scroll.top_line as usize + i;
                    return TextPosition::new(line, column);
                }
                current_offset += slice_len;
            }
        }

        TextPosition::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devil_protocol::{BufferId, CanonicalPath, FileId, WorkspaceId};

    #[test]
    fn shell_parses_commands_into_dispatch_intents_without_editor_ownership() {
        let mut shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("t"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection {
                workspace_id: Some(WorkspaceId(1)),
                buffer_id: Some(BufferId(2)),
                file_id: Some(FileId(9)),
                file_path: Some(CanonicalPath("a.md".to_string())),
                viewport: None,
                degraded: false,
                small_buffer_preview: Some("first".to_string()),
                dirty: false,
            },
            status_messages: Vec::new(),
        });

        let intent = shell
            .handle_command(":i \\n")
            .expect("insert command should parse")
            .expect("intent should be emitted");

        assert_eq!(
            intent,
            CommandDispatchIntent::Insert {
                buffer_id: BufferId(2),
                at: TextPosition::new(0, 0),
                text: "\\n".to_string(),
            }
        );
        assert_eq!(
            shell.active_buffer_projection.small_buffer_text(),
            Some("first")
        );
        assert_eq!(shell.command_dispatch_intents.len(), 1);
    }

    #[test]
    fn explorer_projection_holds_nodes_and_selection() {
        let projection = ExplorerProjection {
            nodes: vec![ExplorerNodeProjection {
                file_id: FileId(10),
                canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                name: "main.rs".to_string(),
                children: vec![],
            }],
            selection: Some(ExplorerSelectionProjection {
                file_id: FileId(10),
            }),
        };

        assert_eq!(projection.nodes.len(), 1);
        assert_eq!(projection.nodes[0].name, "main.rs");
        assert_eq!(
            projection.selection.map(|sel| sel.file_id),
            Some(FileId(10))
        );
    }
}
