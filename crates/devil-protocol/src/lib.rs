//! Shared protocol types, event schemas, action schemas, and versioning for Devil IDE.

#![warn(missing_docs)]

use std::ops::Range;

use serde::{Deserialize, Serialize};

/// Canonical project identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub u128);

/// Canonical text snapshot identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SnapshotId(pub u128);

/// Canonical editor buffer identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BufferId(pub u128);

/// Canonical file identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FileId(pub u128);

/// Opaque byte-range used by editor and project contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ByteRange {
    /// Start byte index.
    pub start: u64,
    /// End byte index (exclusive).
    pub end: u64,
}

impl ByteRange {
    /// Creates a byte range.
    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    /// Converts to `std::ops::Range<usize>`.
    pub const fn as_range(self) -> Range<usize> {
        Range {
            start: self.start as usize,
            end: self.end as usize,
        }
    }
}

/// Minimal project metadata surfaced to editor clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectView {
    /// Project id.
    pub project_id: ProjectId,
    /// Workspace root path.
    pub root_path: String,
    /// Workspace name.
    pub name: String,
    /// Tracked extension list.
    pub allowed_extensions: Vec<String>,
}

/// Event emitted by editor shell when a new buffer is opened.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferOpened {
    /// Optional project context identifier if already known.
    pub project_id: Option<ProjectId>,
    /// Optional project file identifier if already known.
    pub file_id: Option<FileId>,
    /// Mandatory buffer identifier for the shell instance.
    pub buffer_id: BufferId,
}

/// Query sent from editor services to project services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfoQuery {
    /// Buffer identifier for the request.
    pub buffer_id: BufferId,
    /// Path of the requested file.
    pub file_path: String,
}

/// Canonical result type for project metadata lookups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// Resolved project id.
    pub project_id: ProjectId,
    /// Workspace root.
    pub root_path: String,
    /// Language id if known.
    pub language_id: Option<String>,
    /// File id.
    pub file_id: FileId,
}

/// Error returned by project service APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectServiceError {
    /// Classification code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

/// Editor transaction metadata event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorTransactionEvent {
    /// Source buffer id.
    pub buffer_id: BufferId,
    /// Source project id.
    pub project_id: ProjectId,
    /// Human-readable file path.
    pub file_path: String,
    /// Changed range.
    pub changed_range: Option<ByteRange>,
    /// Correlation id for indexing / tracking.
    pub transaction_id: String,
}

/// Minimal protocol abstraction for editor/project interactions.
pub trait ProjectInfoPort {
    /// Resolve project context for the editor-provided path.
    fn resolve_project_for_file(
        &self,
        query: ProjectInfoQuery,
    ) -> Result<ProjectInfo, ProjectServiceError>;

    /// Emit editor mutation metadata for indexers and trackers.
    fn notify_editor_transaction(
        &self,
        event: EditorTransactionEvent,
    ) -> Result<(), ProjectServiceError>;
}
