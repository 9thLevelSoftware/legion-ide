//! Shared protocol types, event schemas, action schemas, and versioning for Devil IDE.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// -----------------------------------------------------------------------------
// Core identifiers and shared primitives
// -----------------------------------------------------------------------------

/// Canonical project identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub u128);

/// Canonical workspace identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceId(pub u128);

/// Canonical workspace root identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceRootId(pub u128);

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

/// Canonical buffer version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BufferVersion(pub u64);

/// Canonical file content version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FileContentVersion(pub u64);

/// Canonical workspace generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceGeneration(pub u64);

/// Canonical terminal session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TerminalSessionId(pub u64);

/// Canonical proposal identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProposalId(pub u64);

/// Cross-domain correlation identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(pub u64);

/// Language-server identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LanguageServerId(pub u64);

/// Plugin identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginId(pub u64);

/// Capability-decision identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityDecisionId(pub u64);

/// Event sequence counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventSequence(pub u64);

/// Opaque principal identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrincipalId(pub String);

/// Capability identifier used by broker policies.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityId(pub String);

/// Capability namespace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityNamespace(pub String);

/// Canonicalized OS path used for identity-sensitive contracts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanonicalPath(pub String);

/// Unix-style language identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LanguageId(pub String);

/// Timestamp in milliseconds since unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimestampMillis(pub u64);

impl TimestampMillis {
    /// Returns current timestamp in milliseconds.
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        Self(millis)
    }
}

/// Opaque byte range used by editor and project contracts.
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

    /// Returns `true` if range has non-negative order.
    pub const fn is_valid(self) -> bool {
        self.start <= self.end
    }
}

/// UTF-8 byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ByteOffset {
    /// Byte offset.
    pub value: u64,
}

/// UTF-16 code unit offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Utf16Offset {
    /// UTF-16 code-unit offset.
    pub value: u64,
}

/// Coordinate encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextCoordinateEncoding {
    /// UTF-8 byte offsets.
    Byte,
    /// UTF-16 code-unit offsets.
    Utf16,
}

/// Text offset with explicit encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextOffset {
    /// Offset value.
    pub value: u64,
    /// Encoding kind.
    pub encoding: TextCoordinateEncoding,
}

impl TextOffset {
    /// Creates a UTF-8 offset.
    pub const fn byte(value: u64) -> Self {
        Self {
            value,
            encoding: TextCoordinateEncoding::Byte,
        }
    }

    /// Creates a UTF-16 offset.
    pub const fn utf16(value: u64) -> Self {
        Self {
            value,
            encoding: TextCoordinateEncoding::Utf16,
        }
    }

    /// Converts to byte offset when possible.
    pub const fn as_byte(self) -> Option<ByteOffset> {
        match self.encoding {
            TextCoordinateEncoding::Byte => Some(ByteOffset { value: self.value }),
            TextCoordinateEncoding::Utf16 => None,
        }
    }

    /// Converts to UTF-16 offset when possible.
    pub const fn as_utf16(self) -> Option<Utf16Offset> {
        match self.encoding {
            TextCoordinateEncoding::Utf16 => Some(Utf16Offset { value: self.value }),
            TextCoordinateEncoding::Byte => None,
        }
    }
}

/// Text range in typed coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextRange {
    /// Inclusive start.
    pub start: TextOffset,
    /// Exclusive end.
    pub end: TextOffset,
}

impl TextRange {
    /// Creates a typed text range.
    pub const fn new(start: TextOffset, end: TextOffset) -> Self {
        Self { start, end }
    }

    /// Constructs a UTF-8 byte range.
    pub const fn byte(start: u64, end: u64) -> Self {
        Self {
            start: TextOffset::byte(start),
            end: TextOffset::byte(end),
        }
    }

    /// Returns `true` when coordinates are ordered and encoded consistently.
    pub fn is_valid(self) -> bool {
        self.start.encoding == self.end.encoding && self.start.value <= self.end.value
    }

    /// Converts to byte range when encoded as bytes.
    pub fn as_byte_range(self) -> Option<ByteRange> {
        if self.start.encoding == TextCoordinateEncoding::Byte
            && self.end.encoding == TextCoordinateEncoding::Byte
        {
            Some(ByteRange::new(self.start.value, self.end.value))
        } else {
            None
        }
    }
}

// -----------------------------------------------------------------------------
// Workspace contracts
// -----------------------------------------------------------------------------

/// Workspace trust state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceTrustState {
    /// User explicitly trusted workspace.
    Trusted,
    /// User explicitly declined trust.
    Untrusted,
    /// Trust undecided.
    Unknown,
}

/// Workspace and root open/close DTOs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceOpenRequest {
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Requesting principal.
    pub principal_id: PrincipalId,
    /// Canonical root path.
    pub root_path: CanonicalPath,
    /// Optional trust override.
    pub trust: Option<WorkspaceTrustState>,
}

/// Workspace opened confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceOpened {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Root identifier.
    pub root_id: WorkspaceRootId,
    /// Stable generation.
    pub generation: WorkspaceGeneration,
    /// Open snapshot.
    pub snapshot_id: SnapshotId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
}

/// Workspace close request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCloseRequest {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Request principal.
    pub principal_id: PrincipalId,
}

/// Workspace closed response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceClosed {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Whether close completed.
    pub success: bool,
}

/// Workspace identity record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIdentity {
    /// File identifier.
    pub file_id: FileId,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Canonical path.
    pub canonical_path: CanonicalPath,
    /// Known content version.
    pub content_version: FileContentVersion,
    /// Optional deterministic hash.
    pub content_hash: Option<String>,
}

/// File kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileKind {
    /// Regular file.
    File,
    /// Directory.
    Directory,
    /// Symlink.
    Symlink,
    /// Unknown/other.
    Other(String),
}

/// File metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Canonical path.
    pub canonical_path: CanonicalPath,
    /// File kind.
    pub kind: FileKind,
    /// Size in bytes.
    pub size_bytes: Option<u64>,
    /// Modified timestamp.
    pub modified_at: Option<TimestampMillis>,
    /// Read-only marker.
    pub read_only: bool,
    /// Permission text.
    pub permissions: Option<String>,
    /// Stable hash if available.
    pub hash: Option<String>,
}

/// Tree node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeNode {
    /// File identity.
    pub identity: FileIdentity,
    /// Node name.
    pub name: String,
    /// Child ids for directory nodes.
    pub children: Vec<FileId>,
    /// Optional metadata.
    pub metadata: Option<FileMetadata>,
}

/// Tree delta operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileTreeDeltaOp {
    /// Node added.
    Add,
    /// Node removed.
    Remove,
    /// Node moved.
    Rename,
    /// Node updated.
    Update,
}

/// Tree delta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeDelta {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Event sequence.
    pub sequence: EventSequence,
    /// Operation.
    pub op: FileTreeDeltaOp,
    /// Changed identity.
    pub identity: FileIdentity,
    /// Optional canonical target path.
    pub target_path: Option<CanonicalPath>,
    /// Correlation id.
    pub correlation_id: CorrelationId,
}

/// Watcher event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatcherEventKind {
    /// File was modified.
    Modified,
    /// File was created.
    Created,
    /// File was deleted.
    Deleted,
    /// File was renamed.
    Renamed,
    /// Overflow condition.
    Overflow,
}

/// Watcher event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherEvent {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Event kind.
    pub kind: WatcherEventKind,
    /// Affected path.
    pub path: CanonicalPath,
    /// Old path for rename.
    pub old_path: Option<CanonicalPath>,
    /// Event sequence.
    pub sequence: EventSequence,
}

/// Snapshot of workspace config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfigSnapshot {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Canonical root path.
    pub root_path: CanonicalPath,
    /// Merge of all config levels.
    pub merged: HashMap<String, String>,
    /// Trust state.
    pub trust_state: WorkspaceTrustState,
    /// Timestamp captured.
    pub captured_at: TimestampMillis,
    /// Schema version.
    pub schema_version: String,
}

/// Conflict state between disk and buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConflictState {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identity.
    pub file_identity: FileIdentity,
    /// Buffer version.
    pub buffer_version: BufferVersion,
    /// File content version.
    pub file_content_version: FileContentVersion,
    /// Snapshot id.
    pub snapshot_id: SnapshotId,
    /// Conflict message.
    pub reason: String,
}

// -----------------------------------------------------------------------------
// Editor contracts
// -----------------------------------------------------------------------------

/// Buffer lifecycle kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BufferLifecycleKind {
    /// Opened.
    Opened,
    /// Activated.
    Activated,
    /// Reloaded from disk.
    Reloaded,
    /// Closed.
    Closed,
}

/// Buffer lifecycle payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferLifecycle {
    /// Buffer identifier.
    pub buffer_id: BufferId,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File id when file-backed.
    pub file_id: Option<FileId>,
    /// Lifecycle event.
    pub kind: BufferLifecycleKind,
    /// Correlation id.
    pub correlation_id: CorrelationId,
}

/// Snapshot descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDescriptor {
    /// Snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Optional file id.
    pub file_id: Option<FileId>,
    /// Version.
    pub buffer_version: BufferVersion,
    /// Byte length.
    pub byte_len: u64,
    /// Content hash.
    pub content_hash: Option<String>,
    /// Creation time.
    pub created_at: TimestampMillis,
}

/// Transaction source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionSource {
    /// User edits.
    User,
    /// Code action.
    CodeAction,
    /// Formatter.
    Formatter,
    /// Plugin.
    Plugin,
    /// Restore/redo command.
    Restore,
    /// System.
    System,
}

/// Text edit for editor domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    /// Target range.
    pub range: TextRange,
    /// Replacement text.
    pub replacement: String,
}

/// Deterministic edit batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditBatch {
    /// Edits in sequence.
    pub edits: Vec<TextEdit>,
}

/// Descriptor for full editor transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextTransactionDescriptor {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Buffer identifier.
    pub buffer_id: BufferId,
    /// File identifier.
    pub file_id: FileId,
    /// Transaction sequence.
    pub transaction_id: Uuid,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Source.
    pub source: TransactionSource,
    /// Pre-snapshot.
    pub pre_snapshot_id: SnapshotId,
    /// Post-snapshot.
    pub post_snapshot_id: SnapshotId,
    /// Pre-version.
    pub pre_buffer_version: BufferVersion,
    /// Post-version.
    pub post_buffer_version: BufferVersion,
    /// Changed ranges.
    pub changed_ranges: Vec<TextRange>,
    /// Undo grouping id.
    pub undo_group_id: Option<Uuid>,
    /// Timestamp.
    pub occurred_at: TimestampMillis,
}

/// Undo group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoGroup {
    /// Group id.
    pub group_id: Uuid,
    /// Transaction ids.
    pub transaction_ids: Vec<Uuid>,
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlaySeverity {
    /// Error.
    Error,
    /// Warning.
    Warning,
    /// Info.
    Info,
    /// Hint.
    Hint,
}

/// Editor overlay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticOverlay {
    /// Overlay identifier.
    pub overlay_id: Uuid,
    /// File identifier.
    pub file_id: FileId,
    /// Message.
    pub message: String,
    /// Severity.
    pub severity: OverlaySeverity,
    /// Covering range.
    pub range: TextRange,
}

/// Completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Snapshot used by request.
    pub snapshot_id: SnapshotId,
    /// Cursor position.
    pub position: TextOffset,
    /// Correlation id.
    pub correlation_id: CorrelationId,
}

/// Completion item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    /// Display label.
    pub label: String,
    /// Detail label.
    pub detail: Option<String>,
    /// Insert text.
    pub insert_text: String,
    /// Kind.
    pub kind: String,
    /// Score.
    pub score: Option<u32>,
    /// Extra text.
    pub documentation: Option<String>,
}

/// Workspace edit proposal payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEditProposal {
    /// Batch identifier.
    pub batch_id: Uuid,
    /// Edits.
    pub edits: EditBatch,
}

// -----------------------------------------------------------------------------
// Proposal contracts
// -----------------------------------------------------------------------------

/// Proposal preconditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalVersionPreconditions {
    /// File version.
    pub file_version: Option<FileContentVersion>,
    /// Buffer version.
    pub buffer_version: Option<BufferVersion>,
    /// Snapshot id.
    pub snapshot_id: Option<SnapshotId>,
    /// Workspace generation.
    pub generation: Option<WorkspaceGeneration>,
}

/// Proposal versioning context.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VersionContext {
    /// File version.
    pub file_version: FileContentVersion,
    /// Buffer version.
    pub buffer_version: BufferVersion,
    /// Snapshot id.
    pub snapshot_id: SnapshotId,
    /// Workspace generation.
    pub generation: WorkspaceGeneration,
}

impl ProposalVersionPreconditions {
    /// Returns true when the preconditions are not met.
    pub fn is_stale(&self, context: VersionContext) -> bool {
        if let Some(expected) = self.file_version
            && expected != context.file_version
        {
            return true;
        }
        if let Some(expected) = self.buffer_version
            && expected != context.buffer_version
        {
            return true;
        }
        if let Some(expected) = self.snapshot_id
            && expected != context.snapshot_id
        {
            return true;
        }
        if let Some(expected) = self.generation
            && expected != context.generation
        {
            return true;
        }

        false
    }
}

/// Preview summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewSummary {
    /// One-line summary.
    pub summary: String,
    /// Detail lines.
    pub details: Vec<String>,
}

/// Workspace proposal envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceProposal {
    /// Proposal identifier.
    pub proposal_id: ProposalId,
    /// Principal.
    pub principal: PrincipalId,
    /// Requested capability.
    pub capability: CapabilityId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Proposed content.
    pub payload: ProposalPayload,
    /// Version preconditions.
    pub preconditions: ProposalVersionPreconditions,
    /// Preview details.
    pub preview: PreviewSummary,
    /// Expiration.
    pub expires_at: Option<TimestampMillis>,
    /// Created.
    pub created_at: TimestampMillis,
}

impl WorkspaceProposal {
    /// Returns true when this proposal is stale for the given context.
    pub fn is_stale(&self, context: VersionContext) -> bool {
        self.preconditions.is_stale(context)
    }

    /// Returns true when proposal expired.
    pub fn is_expired(&self, now: TimestampMillis) -> bool {
        self.expires_at.is_some_and(|expiry| expiry.0 <= now.0)
    }
}

/// Payload for proposals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalPayload {
    /// Text edit proposal.
    TextEdit(TextEditProposal),
    /// Create file.
    CreateFile(CreateFileProposal),
    /// Delete file.
    DeleteFile(DeleteFileProposal),
    /// Rename file.
    RenameFile(RenameFileProposal),
    /// Save.
    SaveFile(SaveFileProposal),
    /// Format.
    FormatFile(FormatFileProposal),
    /// Code action.
    CodeAction(CodeActionProposal),
    /// Terminal command.
    TerminalCommand(TerminalCommandProposal),
}

/// Text edit proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEditProposal {
    /// Target file.
    pub file_id: FileId,
    /// Edits.
    pub edits: EditBatch,
}

/// File creation proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFileProposal {
    /// Destination path.
    pub path: CanonicalPath,
    /// Optional initial payload.
    pub initial_content: Option<String>,
}

/// File deletion proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFileProposal {
    /// File to delete.
    pub file: FileIdentity,
}

/// File rename proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameFileProposal {
    /// Original file.
    pub file: FileIdentity,
    /// New path.
    pub destination: CanonicalPath,
}

/// Save proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveFileProposal {
    /// File identity.
    pub file: FileIdentity,
    /// Snapshot id.
    pub snapshot_id: SnapshotId,
}

/// Format proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatFileProposal {
    /// File identity.
    pub file: FileIdentity,
    /// Snapshot id.
    pub snapshot_id: SnapshotId,
    /// Formatting options.
    pub options: HashMap<String, String>,
}

/// Code action proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeActionProposal {
    /// File identity.
    pub file: FileIdentity,
    /// Action title.
    pub title: String,
    /// Edits.
    pub edits: Vec<TextEdit>,
}

/// Terminal command proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalCommandProposal {
    /// Target session id.
    pub session_id: Option<TerminalSessionId>,
    /// Command to execute.
    pub command: String,
    /// Working directory.
    pub cwd: Option<CanonicalPath>,
    /// Env vars.
    pub env: HashMap<String, String>,
}

// -----------------------------------------------------------------------------
// LSP contracts
// -----------------------------------------------------------------------------

/// Language server status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LspServerStatus {
    /// Stopped state.
    Stopped,
    /// Starting state.
    Starting,
    /// Running state.
    Running,
    /// Failed state.
    Failed {
        /// Failure reason.
        reason: String,
    },
}

/// Language server config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageServerConfig {
    /// Server identifier.
    pub server_id: LanguageServerId,
    /// Workspace owner.
    pub workspace_id: WorkspaceId,
    /// Language id.
    pub language_id: LanguageId,
    /// Launch command.
    pub command: String,
    /// Arguments.
    pub args: Vec<String>,
    /// Environment map.
    pub env: HashMap<String, String>,
    /// Working directory.
    pub cwd: Option<CanonicalPath>,
}

/// Sync strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LspSyncKind {
    /// Full synchronization.
    Full,
    /// Incremental synchronization.
    Incremental,
}

/// Document synchronization state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSyncState {
    /// Document uri/path.
    pub path: CanonicalPath,
    /// Associated file id.
    pub file_id: FileId,
    /// Workspace owner.
    pub workspace_id: WorkspaceId,
    /// Server id.
    pub server_id: LanguageServerId,
    /// Current snapshot id.
    pub snapshot_id: SnapshotId,
    /// Buffer version.
    pub buffer_version: BufferVersion,
    /// Sync kind.
    pub sync_kind: LspSyncKind,
}

/// LSP severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspDiagnosticSeverity {
    /// Error.
    Error,
    /// Warning.
    Warning,
    /// Info.
    Information,
    /// Hint.
    Hint,
}

/// LSP diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspDiagnostic {
    /// Target file.
    pub file_id: FileId,
    /// Range.
    pub range: TextRange,
    /// Severity.
    pub severity: LspDiagnosticSeverity,
    /// Message.
    pub message: String,
    /// Source.
    pub source: Option<String>,
}

/// Diagnostic set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSet {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Snapshot.
    pub snapshot_id: SnapshotId,
    /// Diagnostics.
    pub diagnostics: Vec<LspDiagnostic>,
}

/// Hover descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    /// Target range.
    pub range: TextRange,
    /// Hover content.
    pub contents: String,
}

/// Completion request through LSP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCompletionRequest {
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Request body.
    pub editor_request: CompletionRequest,
    /// Language server.
    pub server_id: LanguageServerId,
}

/// Completion response through LSP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCompletionResponse {
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Items.
    pub items: Vec<CompletionItem>,
}

/// Formatting request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspFormattingRequest {
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// File id.
    pub file_id: FileId,
    /// Server id.
    pub server_id: LanguageServerId,
    /// Snapshot id.
    pub snapshot_id: SnapshotId,
}

/// Formatting response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspFormattingResponse {
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Edit batch.
    pub edits: EditBatch,
}

/// Semantic token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticToken {
    /// Line.
    pub line: u32,
    /// Character.
    pub start: u32,
    /// Length in UTF-16 units.
    pub length: u32,
    /// Token kind.
    pub token_type: String,
}

/// Semantic token set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTokenSet {
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Server id.
    pub server_id: LanguageServerId,
    /// File id.
    pub file_id: FileId,
    /// Snapshot id.
    pub snapshot_id: SnapshotId,
    /// Tokens.
    pub tokens: Vec<SemanticToken>,
}

/// Symbol location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolLocation {
    /// Symbol name.
    pub name: String,
    /// Path.
    pub uri: CanonicalPath,
    /// Range.
    pub range: TextRange,
    /// Symbol kind.
    pub kind: String,
}

/// Code action request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCodeActionRequest {
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// File id.
    pub file_id: FileId,
    /// Server id.
    pub server_id: LanguageServerId,
    /// Snapshot id.
    pub snapshot_id: SnapshotId,
    /// Diagnostic context.
    pub diagnostics: Vec<LspDiagnostic>,
}

/// Code action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCodeAction {
    /// Title.
    pub title: String,
    /// Edits.
    pub edits: Vec<TextEdit>,
}

/// Code action response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCodeActionResponse {
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Actions.
    pub actions: Vec<LspCodeAction>,
}

// -----------------------------------------------------------------------------
// Terminal contracts
// -----------------------------------------------------------------------------

/// Terminal session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalSessionState {
    /// Starting process.
    Starting,
    /// Running process.
    Running,
    /// Exited process.
    Exited,
}

/// Launch request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalLaunchRequest {
    /// Session id.
    pub session_id: TerminalSessionId,
    /// Principal.
    pub principal: PrincipalId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Command.
    pub command: String,
    /// Arguments.
    pub args: Vec<String>,
    /// Current directory.
    pub cwd: Option<CanonicalPath>,
    /// Env map.
    pub env: HashMap<String, String>,
    /// Required capability.
    pub required_capability: CapabilityId,
    /// Decision id.
    pub decision_id: Option<CapabilityDecisionId>,
}

/// Output chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalOutput {
    /// Session id.
    pub session_id: TerminalSessionId,
    /// Monotonic sequence.
    pub sequence: EventSequence,
    /// Output text.
    pub payload: String,
    /// Is error stream.
    pub is_stderr: bool,
    /// Timestamp.
    pub timestamp: TimestampMillis,
}

/// Input request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInput {
    /// Session id.
    pub session_id: TerminalSessionId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Input payload.
    pub payload: String,
}

/// Resize request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalResize {
    /// Session id.
    pub session_id: TerminalSessionId,
    /// Column count.
    pub cols: u16,
    /// Row count.
    pub rows: u16,
}

/// Exit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalExit {
    /// Session id.
    pub session_id: TerminalSessionId,
    /// Exit code.
    pub exit_code: Option<i32>,
    /// Exit reason.
    pub reason: Option<String>,
    /// Timestamp.
    pub timestamp: TimestampMillis,
    /// Session state.
    pub state: TerminalSessionState,
}

/// Terminal capability contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalCapability {
    /// Session id.
    pub session_id: TerminalSessionId,
    /// Supports resize.
    pub supports_resize: bool,
    /// Supports kill.
    pub supports_kill: bool,
}

// -----------------------------------------------------------------------------
// Plugin and capability contracts
// -----------------------------------------------------------------------------

/// Plugin manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin id.
    pub plugin_id: PluginId,
    /// Human name.
    pub name: String,
    /// Version.
    pub version: String,
    /// API range.
    pub api_version: String,
    /// Source hash.
    pub checksum: Option<String>,
    /// Requested capabilities.
    pub requested_capabilities: Vec<CapabilityId>,
}

/// Activation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginActivationEvent {
    /// Activate on startup.
    Startup,
    /// Activate on file extension.
    OnFileOpen {
        /// File extension to match.
        extension: String,
    },
    /// Activate on command.
    OnCommand {
        /// Command id to match.
        command: String,
    },
}

/// Capability request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommandDescriptor {
    /// Command id.
    pub command_id: String,
    /// Human title.
    pub title: String,
    /// Required capability.
    pub required_capability: CapabilityId,
}

/// Contribution descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionDescriptor {
    /// Plugin id.
    pub plugin_id: PluginId,
    /// Contribution name.
    pub name: String,
    /// Contribution type.
    pub kind: String,
    /// Arbitrary contribution payload.
    pub payload: String,
}

/// Plugin state namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateNamespace {
    /// Plugin id.
    pub plugin_id: PluginId,
    /// Namespace.
    pub namespace: String,
}

/// Capability grant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGrant {
    /// Decision id.
    pub decision_id: CapabilityDecisionId,
    /// Principal.
    pub principal_id: PrincipalId,
    /// Capability.
    pub capability_id: CapabilityId,
    /// Namespace.
    pub namespace: CapabilityNamespace,
    /// Expiration.
    pub expires_at: Option<TimestampMillis>,
}

/// Capability denial.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDenial {
    /// Decision id.
    pub decision_id: CapabilityDecisionId,
    /// Principal.
    pub principal_id: PrincipalId,
    /// Capability.
    pub capability_id: CapabilityId,
    /// Reason.
    pub reason: String,
}

/// Capability decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDecision {
    /// Decision id.
    pub decision_id: CapabilityDecisionId,
    /// Decision status.
    pub granted: bool,
    /// Capability.
    pub capability: CapabilityId,
    /// Reason.
    pub reason: Option<String>,
}

/// Plugin action proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginActionProposal {
    /// Plugin identifier.
    pub plugin_id: PluginId,
    /// Underlying workspace proposal.
    pub proposal: WorkspaceProposal,
}

/// Context provider descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextProviderDescriptor {
    /// Plugin id.
    pub plugin_id: PluginId,
    /// Context key.
    pub key: String,
    /// Human description.
    pub description: String,
}

// -----------------------------------------------------------------------------
// Legacy/minimal spike contracts retained for compatibility.
// -----------------------------------------------------------------------------

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

/// Protocol-facing error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolError {
    /// Error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl ProtocolError {
    /// Creates an "unsupported" protocol error with the provided human-readable message.
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            code: "unsupported".to_string(),
            message: message.into(),
        }
    }
}

/// Shared protocol result.
pub type ProtocolResult<T> = Result<T, ProtocolError>;

// -----------------------------------------------------------------------------
// Protocol request/response envelopes
// -----------------------------------------------------------------------------

/// Workspace request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceRequest {
    /// Open request.
    Open(WorkspaceOpenRequest),
    /// Close request.
    Close(WorkspaceCloseRequest),
    /// Resolve path.
    ResolveFile {
        /// Workspace id to resolve against.
        workspace_id: WorkspaceId,
        /// Canonical path of the file.
        path: CanonicalPath,
    },
    /// Read config.
    ReadConfig(WorkspaceId),
    /// Apply tree delta.
    ApplyTreeDelta(FileTreeDelta),
}

/// Workspace response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceResponse {
    /// Opened.
    Opened(WorkspaceOpened),
    /// Closed.
    Closed(WorkspaceClosed),
    /// Identity.
    ResolvedFile(FileIdentity),
    /// Snapshot.
    Config(WorkspaceConfigSnapshot),
    /// Tree.
    Tree(Vec<FileTreeNode>),
    /// Conflict.
    Conflict(FileConflictState),
}

/// Editor request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorRequest {
    /// Open.
    OpenBuffer {
        /// Workspace id.
        workspace_id: WorkspaceId,
        /// Path.
        path: CanonicalPath,
    },
    /// Apply transaction.
    ApplyTransaction(TextTransactionDescriptor),
    /// Completion.
    Completion(CompletionRequest),
    /// Snapshot descriptor.
    Snapshot(SnapshotDescriptor),
    /// Overlay.
    Overlay(DiagnosticOverlay),
}

/// Editor response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorResponse {
    /// Opened.
    BufferOpened(BufferOpened),
    /// Closed.
    BufferClosed(CorrelationId),
    /// Transaction.
    Transaction(TextTransactionDescriptor),
    /// Completion.
    Completion(LspCompletionResponse),
    /// Snapshot.
    Snapshot(SnapshotDescriptor),
    /// Overlay.
    OverlayApplied(Uuid),
}

/// Proposal request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalRequest {
    /// Validate proposal.
    Validate(WorkspaceProposal),
    /// Preview proposal.
    Preview(WorkspaceProposal),
    /// Apply proposal.
    Apply(WorkspaceProposal),
}

/// Proposal response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalResponse {
    /// Validation result.
    Valid(ProposalId),
    /// Preview result.
    Preview(WorkspaceProposal),
    /// Applied result.
    Applied(ProposalId),
    /// Denied.
    Denied {
        /// Proposal id.
        proposal_id: ProposalId,
        /// Reason.
        reason: String,
    },
}

/// Terminal request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalRequest {
    /// Launch terminal.
    Launch(TerminalLaunchRequest),
    /// Write input.
    Input(TerminalInput),
    /// Resize.
    Resize(TerminalResize),
    /// Close.
    Close {
        /// Session id to close.
        session_id: TerminalSessionId,
    },
}

/// Terminal response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalResponse {
    /// Launched.
    Launched(TerminalSessionId),
    /// Output.
    Output(TerminalOutput),
    /// Exit.
    Exited(TerminalExit),
}

/// LSP request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LspRequest {
    /// Register.
    RegisterServer(LanguageServerConfig),
    /// Open document.
    OpenDocument(DocumentSyncState),
    /// Update document.
    UpdateDocument(DocumentSyncState),
    /// Completion.
    Completion(LspCompletionRequest),
    /// Hover.
    Hover {
        /// Language server to resolve hover from.
        server_id: LanguageServerId,
        /// File id for the hover query.
        file_id: FileId,
    },
    /// Formatting.
    Formatting(LspFormattingRequest),
    /// Symbol.
    Symbol {
        /// File id for which to request symbols.
        file_id: FileId,
    },
    /// Code action.
    CodeAction(LspCodeActionRequest),
}

/// LSP response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LspResponse {
    /// Registered.
    Registered {
        /// Server id.
        server_id: LanguageServerId,
    },
    /// Completion.
    Completion(LspCompletionResponse),
    /// Hover.
    Hover(Hover),
    /// Formatting.
    Formatting(LspFormattingResponse),
    /// Diagnostics.
    Diagnostics(DiagnosticSet),
    /// Semantic tokens.
    SemanticTokens(SemanticTokenSet),
    /// Symbols.
    Symbols(Vec<SymbolLocation>),
    /// Actions.
    CodeActions(LspCodeActionResponse),
    /// Status.
    Status(LspServerStatus),
}

/// Plugin/manifest request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginRequest {
    /// Manifest.
    Manifest(PluginManifest),
    /// Command descriptor.
    CommandDescriptor(PluginCommandDescriptor),
    /// Contribution descriptor.
    Contribution(ContributionDescriptor),
}

/// Plugin response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginResponse {
    /// Manifest loaded.
    Loaded(PluginId),
    /// Command registered.
    CommandRegistered(String),
    /// Contribution registered.
    ContributionRegistered(String),
}

/// Capability request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CapabilityRequest {
    /// New capability request.
    Request {
        /// Principal.
        principal_id: PrincipalId,
        /// Capability.
        capability_id: CapabilityId,
        /// Correlation id.
        correlation_id: CorrelationId,
    },
    /// Decision grant.
    Grant(CapabilityGrant),
    /// Decision denial.
    Deny(CapabilityDenial),
}

/// Capability response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CapabilityResponse {
    /// Decision.
    Decision(CapabilityDecision),
    /// Grant ack.
    Granted(CapabilityGrant),
    /// Deny ack.
    Denied(CapabilityDenial),
}

/// Event envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Event name.
    pub event: String,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Workspace id.
    pub workspace_id: Option<WorkspaceId>,
    /// Sequence.
    pub sequence: EventSequence,
    /// Actor principal.
    pub principal_id: Option<PrincipalId>,
    /// Payload body.
    pub payload: serde_json::Value,
}

/// Event sink request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSinkRequest {
    /// Envelope.
    pub envelope: EventEnvelope,
}

/// Storage repository request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageRepositoryRequest {
    /// Save workspace config.
    SaveWorkspaceConfig(WorkspaceConfigSnapshot),
    /// Save file metadata.
    SaveFileMetadata(FileMetadata),
    /// Read workspace config.
    ReadWorkspaceConfig(WorkspaceId),
    /// Read file metadata.
    ReadFileMetadata(FileId),
}

/// Storage repository response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageRepositoryResponse {
    /// Persisted marker.
    Saved {
        /// Opaque key.
        key: String,
    },
    /// Config.
    WorkspaceConfig(Option<WorkspaceConfigSnapshot>),
    /// Metadata.
    FileMetadata(Option<FileMetadata>),
    /// Missing.
    Missing,
}

// -----------------------------------------------------------------------------
// Service ports
// -----------------------------------------------------------------------------

/// Service-port for workspace interactions.
pub trait WorkspacePort {
    /// Handle a workspace request.
    fn handle(&self, request: WorkspaceRequest) -> ProtocolResult<WorkspaceResponse>;
}

/// Service-port for editor interactions.
pub trait EditorPort {
    /// Handle an editor request.
    fn handle(&self, request: EditorRequest) -> ProtocolResult<EditorResponse>;
}

/// Service-port for proposal interactions.
pub trait ProposalPort {
    /// Handle a proposal request.
    fn handle(&self, request: ProposalRequest) -> ProtocolResult<ProposalResponse>;
}

/// Service-port for terminal interactions.
pub trait TerminalPort {
    /// Handle a terminal request.
    fn handle(&self, request: TerminalRequest) -> ProtocolResult<TerminalResponse>;
}

/// Service-port for LSP interactions.
pub trait LspPort {
    /// Handle LSP request.
    fn handle(&self, request: LspRequest) -> ProtocolResult<LspResponse>;
}

/// Service-port for capability broker interactions.
pub trait CapabilityBrokerPort {
    /// Handle capability request.
    fn handle(&self, request: CapabilityRequest) -> ProtocolResult<CapabilityResponse>;
}

/// Service-port for event publishing.
pub trait EventSinkPort {
    /// Emit event.
    fn emit(&self, request: EventSinkRequest) -> ProtocolResult<()>;
}

/// Service-port for storage repos.
pub trait StorageRepositoryPort {
    /// Handle storage request.
    fn handle(
        &self,
        request: StorageRepositoryRequest,
    ) -> ProtocolResult<StorageRepositoryResponse>;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn workspace_open_request_serde_round_trip() {
        let request = WorkspaceOpenRequest {
            correlation_id: CorrelationId(12),
            principal_id: PrincipalId("user-a".to_string()),
            root_path: CanonicalPath("/repo/root".to_string()),
            trust: Some(WorkspaceTrustState::Trusted),
        };

        let text = serde_json::to_string(&request).expect("serialize");
        let parsed: WorkspaceOpenRequest = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(request.correlation_id.0, parsed.correlation_id.0);
        assert_eq!(request.principal_id, parsed.principal_id);
    }

    #[test]
    fn workspace_open_request_golden_schema() {
        let expected = serde_json::from_str::<Value>(
            r#"{
                "correlation_id": 12,
                "principal_id": "user-a",
                "root_path": "/repo/root",
                "trust": "Trusted"
            }"#,
        )
        .unwrap();

        let value = serde_json::to_value(WorkspaceOpenRequest {
            correlation_id: CorrelationId(12),
            principal_id: PrincipalId("user-a".to_string()),
            root_path: CanonicalPath("/repo/root".to_string()),
            trust: Some(WorkspaceTrustState::Trusted),
        })
        .unwrap();

        assert_eq!(expected, value);
    }

    #[test]
    fn workspace_open_request_required_fields() {
        let bad = r#"{"correlation_id": 12, "principal_id": "user-a"}"#;
        assert!(serde_json::from_str::<WorkspaceOpenRequest>(bad).is_err());
    }

    #[test]
    fn text_range_coordinate_golden_schema() {
        let range = TextRange::byte(1, 4);
        let value = serde_json::to_value(range).unwrap();
        let expected = serde_json::json!({
            "start": {"value": 1, "encoding": "Byte"},
            "end": {"value": 4, "encoding": "Byte"},
        });
        assert_eq!(value, expected);
        assert!(TextRange::byte(1, 4).is_valid());
        assert!(!TextRange::new(TextOffset::byte(4), TextOffset::utf16(3)).is_valid());
    }

    #[test]
    fn text_edit_batch_roundtrip() {
        let edits = EditBatch {
            edits: vec![TextEdit {
                range: TextRange::byte(0, 3),
                replacement: "abc".to_string(),
            }],
        };
        let proposal = WorkspaceProposal {
            proposal_id: ProposalId(2),
            principal: PrincipalId("user-a".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(2),
            payload: ProposalPayload::TextEdit(TextEditProposal {
                file_id: FileId(9),
                edits: edits.clone(),
            }),
            preconditions: ProposalVersionPreconditions {
                file_version: None,
                buffer_version: None,
                snapshot_id: None,
                generation: None,
            },
            preview: PreviewSummary {
                summary: "insert abc".to_string(),
                details: vec!["single edit".to_string()],
            },
            expires_at: Some(TimestampMillis(1000)),
            created_at: TimestampMillis(1),
        };

        let raw = serde_json::to_string_pretty(&proposal).unwrap();
        let decoded: WorkspaceProposal = serde_json::from_str(&raw).unwrap();
        match decoded.payload {
            ProposalPayload::TextEdit(inner) => {
                assert_eq!(inner.edits.edits[0].replacement, "abc");
            }
            _ => panic!("unexpected payload"),
        }
    }

    #[test]
    fn stale_proposal_check() {
        let proposal = WorkspaceProposal {
            proposal_id: ProposalId(1),
            principal: PrincipalId("user-a".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(3),
            payload: ProposalPayload::DeleteFile(DeleteFileProposal {
                file: FileIdentity {
                    file_id: FileId(5),
                    workspace_id: WorkspaceId(1),
                    canonical_path: CanonicalPath("/a.txt".to_string()),
                    content_version: FileContentVersion(2),
                    content_hash: None,
                },
            }),
            preconditions: ProposalVersionPreconditions {
                file_version: Some(FileContentVersion(1)),
                buffer_version: Some(BufferVersion(9)),
                snapshot_id: Some(SnapshotId(3)),
                generation: Some(WorkspaceGeneration(1)),
            },
            preview: PreviewSummary {
                summary: "delete".to_string(),
                details: vec![],
            },
            expires_at: None,
            created_at: TimestampMillis(1),
        };

        let up_to_date = VersionContext {
            file_version: FileContentVersion(1),
            buffer_version: BufferVersion(9),
            snapshot_id: SnapshotId(3),
            generation: WorkspaceGeneration(1),
        };

        let stale = VersionContext {
            file_version: FileContentVersion(2),
            buffer_version: BufferVersion(9),
            snapshot_id: SnapshotId(3),
            generation: WorkspaceGeneration(1),
        };

        assert!(!proposal.is_stale(up_to_date));
        assert!(proposal.is_stale(stale));
    }

    #[test]
    fn proposal_expiry() {
        let proposal = WorkspaceProposal {
            proposal_id: ProposalId(2),
            principal: PrincipalId("user-a".to_string()),
            capability: CapabilityId("format".to_string()),
            correlation_id: CorrelationId(4),
            payload: ProposalPayload::FormatFile(FormatFileProposal {
                file: FileIdentity {
                    file_id: FileId(1),
                    workspace_id: WorkspaceId(1),
                    canonical_path: CanonicalPath("/a.rs".to_string()),
                    content_version: FileContentVersion(1),
                    content_hash: None,
                },
                snapshot_id: SnapshotId(2),
                options: HashMap::new(),
            }),
            preconditions: ProposalVersionPreconditions {
                file_version: None,
                buffer_version: None,
                snapshot_id: None,
                generation: None,
            },
            preview: PreviewSummary {
                summary: "format".to_string(),
                details: vec![],
            },
            expires_at: Some(TimestampMillis(1)),
            created_at: TimestampMillis(1),
        };

        assert!(proposal.is_expired(TimestampMillis(2)));
        assert!(!proposal.is_expired(TimestampMillis(0)));
    }

    #[test]
    fn lsp_round_trip() {
        let response = LspResponse::Completion(LspCompletionResponse {
            correlation_id: CorrelationId(11),
            items: vec![CompletionItem {
                label: "println!".to_string(),
                detail: Some("macro".to_string()),
                insert_text: "println!(\"$1\")".to_string(),
                kind: "Function".to_string(),
                score: Some(1),
                documentation: None,
            }],
        });

        let text = serde_json::to_string(&response).unwrap();
        let round: LspResponse = serde_json::from_str(&text).unwrap();
        match round {
            LspResponse::Completion(inner) => {
                assert_eq!(inner.items[0].label, "println!");
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn terminal_launch_roundtrip() {
        let launch = TerminalLaunchRequest {
            session_id: TerminalSessionId(7),
            principal: PrincipalId("shell".to_string()),
            correlation_id: CorrelationId(8),
            command: "bash".to_string(),
            args: vec!["-lc".to_string(), "echo hi".to_string()],
            cwd: Some(CanonicalPath("/tmp".to_string())),
            env: HashMap::from([("TERM".to_string(), "xterm-256color".to_string())]),
            required_capability: CapabilityId("terminal.launch".to_string()),
            decision_id: None,
        };

        let text = serde_json::to_string_pretty(&launch).unwrap();
        let value: TerminalLaunchRequest = serde_json::from_str(&text).unwrap();
        assert_eq!(value.command, "bash");
        assert_eq!(value.args[0], "-lc");
    }

    #[test]
    fn plugin_manifest_required_fields_and_roundtrip() {
        let manifest = PluginManifest {
            plugin_id: PluginId(3),
            name: "my-plugin".to_string(),
            version: "0.1.0".to_string(),
            api_version: "1.0".to_string(),
            checksum: Some("sha256".to_string()),
            requested_capabilities: vec![CapabilityId("cmd.exec".to_string())],
        };

        let as_json = serde_json::to_string(&manifest).unwrap();
        let back: PluginManifest = serde_json::from_str(&as_json).unwrap();
        assert_eq!(back.plugin_id, manifest.plugin_id);

        let invalid = r#"{"plugin_id":1, "name":"x", "version":"0.1.0"}"#;
        assert!(serde_json::from_str::<PluginManifest>(invalid).is_err());
    }

    #[test]
    fn file_tree_delta_roundtrip_and_schema() {
        let identity = FileIdentity {
            file_id: FileId(21),
            workspace_id: WorkspaceId(8),
            canonical_path: CanonicalPath("/project/src/lib.rs".to_string()),
            content_version: FileContentVersion(3),
            content_hash: Some("h1".to_string()),
        };

        let delta = FileTreeDelta {
            workspace_id: WorkspaceId(8),
            sequence: EventSequence(12),
            op: FileTreeDeltaOp::Rename,
            identity,
            target_path: Some(CanonicalPath("/project/src/main.rs".to_string())),
            correlation_id: CorrelationId(99),
        };

        let value = serde_json::to_value(&delta).unwrap();
        let expected = serde_json::json!({
            "workspace_id": 8,
            "sequence": 12,
            "op": "Rename",
            "identity": {
                "file_id": 21,
                "workspace_id": 8,
                "canonical_path": "/project/src/lib.rs",
                "content_version": 3,
                "content_hash": "h1",
            },
            "target_path": "/project/src/main.rs",
            "correlation_id": 99,
        });

        assert_eq!(value, expected);

        let decoded = serde_json::from_value::<FileTreeDelta>(value).unwrap();
        assert_eq!(decoded.sequence, EventSequence(12));
    }

    #[test]
    fn workspace_config_snapshot_required_fields_and_roundtrip() {
        let config = WorkspaceConfigSnapshot {
            workspace_id: WorkspaceId(3),
            root_path: CanonicalPath("/project".to_string()),
            merged: HashMap::from([
                ("editor.tab_size".to_string(), "4".to_string()),
                ("theme".to_string(), "dark".to_string()),
            ]),
            trust_state: WorkspaceTrustState::Trusted,
            captured_at: TimestampMillis(1_234),
            schema_version: "v1".to_string(),
        };

        let text = serde_json::to_string_pretty(&config).unwrap();
        let parsed: WorkspaceConfigSnapshot = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.workspace_id, WorkspaceId(3));

        let invalid = r#"{"workspace_id":3, "root_path":"/project", "merged":{}, "trust_state":"Trusted", "captured_at":1234}"#;
        assert!(serde_json::from_str::<WorkspaceConfigSnapshot>(invalid).is_err());
    }

    #[test]
    fn capability_struct_roundtrip() {
        let grant = CapabilityGrant {
            decision_id: CapabilityDecisionId(4),
            principal_id: PrincipalId("plugin-loader".to_string()),
            capability_id: CapabilityId("plugin.load".to_string()),
            namespace: CapabilityNamespace("plugins".to_string()),
            expires_at: Some(TimestampMillis(9000)),
        };

        let decision = CapabilityDecision {
            decision_id: CapabilityDecisionId(4),
            granted: true,
            capability: CapabilityId("plugin.load".to_string()),
            reason: Some("policy approved".to_string()),
        };

        let denial = CapabilityDenial {
            decision_id: CapabilityDecisionId(5),
            principal_id: PrincipalId("plugin-loader".to_string()),
            capability_id: CapabilityId("exec.shell".to_string()),
            reason: "not allowed in air-gap mode".to_string(),
        };

        let grant_text = serde_json::to_string(&grant).unwrap();
        let grant_round: CapabilityGrant = serde_json::from_str(&grant_text).unwrap();
        assert_eq!(grant_round.capability_id, grant.capability_id);

        let decision_text = serde_json::to_string(&decision).unwrap();
        let decision_round: CapabilityDecision = serde_json::from_str(&decision_text).unwrap();
        assert!(decision_round.granted);

        let denial_value = serde_json::to_value(&denial).unwrap();
        let denial_round: CapabilityDenial = serde_json::from_value(denial_value).unwrap();
        assert_eq!(denial_round.reason, denial.reason);
    }

    #[test]
    fn storage_repository_request_and_response_roundtrip() {
        let req = StorageRepositoryRequest::ReadWorkspaceConfig(WorkspaceId(4));
        let req_text = serde_json::to_string(&req).unwrap();
        let decoded_req: StorageRepositoryRequest = serde_json::from_str(&req_text).unwrap();

        match decoded_req {
            StorageRepositoryRequest::ReadWorkspaceConfig(id) => assert_eq!(id, WorkspaceId(4)),
            _ => panic!("unexpected request variant"),
        }

        let response = StorageRepositoryResponse::WorkspaceConfig(Some(WorkspaceConfigSnapshot {
            workspace_id: WorkspaceId(4),
            root_path: CanonicalPath("/project".to_string()),
            merged: HashMap::from([(
                "editor.trim_trailing_whitespace".to_string(),
                "true".to_string(),
            )]),
            trust_state: WorkspaceTrustState::Trusted,
            captured_at: TimestampMillis(77),
            schema_version: "v1".to_string(),
        }));

        let response_text = serde_json::to_string_pretty(&response).unwrap();
        let decoded_response: StorageRepositoryResponse =
            serde_json::from_str(&response_text).unwrap();

        match decoded_response {
            StorageRepositoryResponse::WorkspaceConfig(Some(value)) => {
                assert_eq!(value.workspace_id, WorkspaceId(4));
            }
            _ => panic!("unexpected response variant"),
        }
    }

    #[test]
    fn mock_ports_compile() {
        struct MockWorkspacePort;
        struct MockEditorPort;
        struct MockProposalPort;
        struct MockTerminalPort;
        struct MockLspPort;
        struct MockCapabilityBrokerPort;
        struct MockEventSinkPort;
        struct MockStorageRepositoryPort;

        impl WorkspacePort for MockWorkspacePort {
            fn handle(&self, _request: WorkspaceRequest) -> ProtocolResult<WorkspaceResponse> {
                Err(ProtocolError::unsupported("not implemented"))
            }
        }
        impl EditorPort for MockEditorPort {
            fn handle(&self, _request: EditorRequest) -> ProtocolResult<EditorResponse> {
                Err(ProtocolError::unsupported("not implemented"))
            }
        }
        impl ProposalPort for MockProposalPort {
            fn handle(&self, _request: ProposalRequest) -> ProtocolResult<ProposalResponse> {
                Err(ProtocolError::unsupported("not implemented"))
            }
        }
        impl TerminalPort for MockTerminalPort {
            fn handle(&self, _request: TerminalRequest) -> ProtocolResult<TerminalResponse> {
                Err(ProtocolError::unsupported("not implemented"))
            }
        }
        impl LspPort for MockLspPort {
            fn handle(&self, _request: LspRequest) -> ProtocolResult<LspResponse> {
                Err(ProtocolError::unsupported("not implemented"))
            }
        }
        impl CapabilityBrokerPort for MockCapabilityBrokerPort {
            fn handle(&self, _request: CapabilityRequest) -> ProtocolResult<CapabilityResponse> {
                Err(ProtocolError::unsupported("not implemented"))
            }
        }
        impl EventSinkPort for MockEventSinkPort {
            fn emit(&self, _request: EventSinkRequest) -> ProtocolResult<()> {
                Err(ProtocolError::unsupported("not implemented"))
            }
        }
        impl StorageRepositoryPort for MockStorageRepositoryPort {
            fn handle(
                &self,
                _request: StorageRepositoryRequest,
            ) -> ProtocolResult<StorageRepositoryResponse> {
                Err(ProtocolError::unsupported("not implemented"))
            }
        }

        fn use_all_ports(
            w: impl WorkspacePort,
            e: impl EditorPort,
            p: impl ProposalPort,
            t: impl TerminalPort,
            l: impl LspPort,
            c: impl CapabilityBrokerPort,
            es: impl EventSinkPort,
            s: impl StorageRepositoryPort,
        ) {
            let _ = (
                w.handle(WorkspaceRequest::ReadConfig(WorkspaceId(1))),
                e.handle(EditorRequest::Snapshot(SnapshotDescriptor {
                    snapshot_id: SnapshotId(1),
                    file_id: None,
                    buffer_version: BufferVersion(1),
                    byte_len: 0,
                    content_hash: None,
                    created_at: TimestampMillis(1),
                })),
                p.handle(ProposalRequest::Validate(WorkspaceProposal {
                    proposal_id: ProposalId(1),
                    principal: PrincipalId("x".to_string()),
                    capability: CapabilityId("fs.read".to_string()),
                    correlation_id: CorrelationId(1),
                    payload: ProposalPayload::SaveFile(SaveFileProposal {
                        file: FileIdentity {
                            file_id: FileId(1),
                            workspace_id: WorkspaceId(1),
                            canonical_path: CanonicalPath("/x".to_string()),
                            content_version: FileContentVersion(1),
                            content_hash: None,
                        },
                        snapshot_id: SnapshotId(1),
                    }),
                    preconditions: ProposalVersionPreconditions {
                        file_version: None,
                        buffer_version: None,
                        snapshot_id: None,
                        generation: None,
                    },
                    preview: PreviewSummary {
                        summary: "save".to_string(),
                        details: vec![],
                    },
                    expires_at: None,
                    created_at: TimestampMillis(1),
                })),
                t.handle(TerminalRequest::Close {
                    session_id: TerminalSessionId(1),
                }),
                l.handle(LspRequest::Hover {
                    server_id: LanguageServerId(1),
                    file_id: FileId(1),
                }),
                c.handle(CapabilityRequest::Request {
                    principal_id: PrincipalId("x".to_string()),
                    capability_id: CapabilityId("k".to_string()),
                    correlation_id: CorrelationId(1),
                }),
                es.emit(EventSinkRequest {
                    envelope: EventEnvelope {
                        event: "init".to_string(),
                        correlation_id: CorrelationId(1),
                        workspace_id: None,
                        sequence: EventSequence(1),
                        principal_id: None,
                        payload: serde_json::json!({"ok": true}),
                    },
                }),
                s.handle(StorageRepositoryRequest::ReadWorkspaceConfig(WorkspaceId(
                    1,
                ))),
            );
        }

        use_all_ports(
            MockWorkspacePort,
            MockEditorPort,
            MockProposalPort,
            MockTerminalPort,
            MockLspPort,
            MockCapabilityBrokerPort,
            MockEventSinkPort,
            MockStorageRepositoryPort,
        );
    }
}
