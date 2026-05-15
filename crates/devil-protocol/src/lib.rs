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

/// Language-server request identifier used for supervised operation tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LspRequestId(pub Uuid);

/// Cross-domain cancellation token identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CancellationTokenId(pub Uuid);

/// Semantic query identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SemanticQueryId(pub Uuid);

/// Semantic graph or cache record identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SemanticRecordId(pub String);

/// Semantic symbol identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SemanticSymbolId(pub String);

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

/// UTF-16 line/character position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Utf16Position {
    /// Zero-based UTF-16 line index.
    pub line: u32,
    /// Zero-based UTF-16 code-unit character offset on line.
    pub character: u32,
}

/// UTF-16 range in line/character coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Utf16Range {
    /// Inclusive start UTF-16 position.
    pub start: Utf16Position,
    /// Exclusive end UTF-16 position.
    pub end: Utf16Position,
}

/// Byte + UTF-16 changed range descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChangedTextRange {
    /// Changed byte range in post-edit coordinates.
    pub byte_range: ByteRange,
    /// Changed UTF-16 range in post-edit coordinates.
    pub utf16_range: Utf16Range,
}

/// Causality chain identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CausalityId(pub Uuid);

/// Event identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(pub Uuid);

/// Stable fingerprint for disk-backed file content or metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileFingerprint {
    /// Fingerprint algorithm or provenance label.
    pub algorithm: String,
    /// Fingerprint value emitted by the producing subsystem.
    pub value: String,
}

/// Grammar version used to invalidate parser-derived semantic records.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SemanticGrammarVersion(pub String);

/// Model version used to invalidate learned ranking or enrichment records.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SemanticModelVersion(pub String);

/// Privacy scope attached to semantic and LSP-derived records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticPrivacyScope {
    /// Public or intentionally shareable metadata.
    Public,
    /// Workspace-private data.
    Workspace,
    /// Project-private data.
    Project,
    /// Single-file scoped data.
    File,
    /// Metadata-only data that must not carry source excerpts.
    MetadataOnly,
    /// Redacted data whose details cannot be disclosed to the caller.
    Redacted,
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

/// Protocol-level text coordinate independent of editor internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextCoordinate {
    /// Zero-based line index.
    pub line: u32,
    /// Zero-based character offset within the line.
    pub character: u32,
    /// Optional UTF-8 byte offset in the snapshot.
    pub byte_offset: Option<u64>,
    /// Optional UTF-16 code-unit offset in the snapshot.
    pub utf16_offset: Option<u64>,
}

/// Protocol-level text range independent of editor internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolTextRange {
    /// Inclusive start coordinate.
    pub start: TextCoordinate,
    /// Exclusive end coordinate.
    pub end: TextCoordinate,
}

/// Pixel dimensions for viewport projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ViewportDimensions {
    /// Viewport width in physical pixels.
    pub width_px: u32,
    /// Viewport height in physical pixels.
    pub height_px: u32,
}

/// Scroll offsets for viewport projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ViewportScroll {
    /// Top visible line index.
    pub top_line: u32,
    /// Leftmost visible column.
    pub left_column: u32,
}

/// Viewport projection operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ViewportProjectionMode {
    /// Standard projection mode for normal buffers.
    #[default]
    Normal,
    /// Compatibility mode where explicitly bounded small-buffer payloads are allowed.
    BoundedSmallBuffer,
    /// Degraded projection mode for large files where overlays may be deferred.
    DegradedLargeFile,
}

/// Truncation state for a visible viewport line slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViewportLineTruncationState {
    /// Slice contains the full logical line.
    None,
    /// Slice omits leading content from the logical line.
    Leading,
    /// Slice omits trailing content from the logical line.
    Trailing,
    /// Slice omits both leading and trailing content from the logical line.
    Both,
}

/// Visible viewport slice for a logical line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewportLineSlice {
    /// Zero-based logical line number represented by the slice.
    pub line_number: u32,
    /// Visible text rendered for this line slice.
    pub visible_text: String,
    /// Visible byte range in snapshot coordinates.
    pub byte_range: ByteRange,
    /// Visible UTF-16 range in snapshot coordinates.
    pub utf16_range: Utf16Range,
    /// Hash for the chunk backing this slice.
    pub chunk_hash: FileFingerprint,
    /// Whether the slice omits leading and/or trailing content.
    pub truncation_state: ViewportLineTruncationState,
}

/// Line metric aligned with [`ViewportProjection::line_slices`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewportLineMetric {
    /// Total byte length for the logical line.
    pub byte_length: u64,
    /// Total UTF-16 code-unit length for the logical line.
    pub utf16_length: u64,
    /// Width of the line ending in bytes.
    pub line_ending_width: u8,
    /// Whether the metric is exact rather than estimated.
    pub exact: bool,
}

/// Placeholder decoration span for future overlay phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ViewportDecorationSpan {}

/// Placeholder fold range for future overlay phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ViewportFoldRange {}

/// Placeholder semantic token overlay for future overlay phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ViewportSemanticTokenOverlay {}

/// Large-file projection status for degraded viewport rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LargeFileStatus {
    /// Large-file threshold in bytes that triggered compatibility behavior.
    pub threshold_bytes: u64,
    /// Current snapshot byte length.
    pub byte_len: u64,
    /// User-visible reasons why overlays are disabled or deferred.
    pub disabled_overlay_reasons: Vec<String>,
    /// Whether bounded search remains available in the current mode.
    pub bounded_search_enabled: bool,
    /// User-visible large-file status message.
    pub message: String,
}

/// Protocol-level viewport projection for later UI rendering contracts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewportProjection {
    /// Owning workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Projected buffer identifier.
    pub buffer_id: BufferId,
    /// Optional file identifier when the buffer is file-backed.
    pub file_id: Option<FileId>,
    /// Snapshot used to produce this projection.
    pub snapshot_id: SnapshotId,
    /// Buffer version used to produce this projection.
    pub buffer_version: BufferVersion,
    /// Visible text range in snapshot coordinates.
    pub visible_range: ProtocolTextRange,
    /// Selection ranges in snapshot coordinates.
    pub selections: Vec<ProtocolTextRange>,
    /// Primary cursor coordinate.
    pub cursor: TextCoordinate,
    /// Scroll offsets.
    pub scroll: ViewportScroll,
    /// Viewport dimensions.
    pub dimensions: ViewportDimensions,
    /// Projection mode used to produce the viewport payload.
    #[serde(default)]
    pub mode: ViewportProjectionMode,
    /// Visible line slices in render order.
    #[serde(default)]
    pub line_slices: Vec<ViewportLineSlice>,
    /// Per-line metrics aligned with [`ViewportProjection::line_slices`].
    #[serde(default)]
    pub line_metrics: Vec<ViewportLineMetric>,
    /// Decoration placeholders reserved for later phases.
    #[serde(default)]
    pub decoration_spans: Vec<ViewportDecorationSpan>,
    /// Fold placeholders reserved for later phases.
    #[serde(default)]
    pub fold_ranges: Vec<ViewportFoldRange>,
    /// Semantic overlay placeholders reserved for later phases.
    #[serde(default)]
    pub semantic_token_overlays: Vec<ViewportSemanticTokenOverlay>,
    /// Large-file compatibility status when projection behavior is constrained.
    #[serde(default)]
    pub large_file_status: Option<LargeFileStatus>,
    /// Viewport projection schema version.
    pub schema_version: u16,
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
    /// Stable file identifier when metadata is persisted by a workspace authority.
    #[serde(default)]
    pub file_id: Option<FileId>,
    /// Workspace owner when metadata is scoped to an open workspace.
    #[serde(default)]
    pub workspace_id: Option<WorkspaceId>,
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
    /// Stable fingerprint if available.
    #[serde(default)]
    pub fingerprint: Option<FileFingerprint>,
    /// File content version associated with this metadata, when known.
    #[serde(default)]
    pub content_version: Option<FileContentVersion>,
    /// Workspace generation associated with this metadata, when known.
    #[serde(default)]
    pub workspace_generation: Option<WorkspaceGeneration>,
    /// Metadata DTO schema version.
    #[serde(default)]
    pub schema_version: u16,
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

/// Protocol diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolDiagnosticSeverity {
    /// Error diagnostic.
    Error,
    /// Warning diagnostic.
    Warning,
    /// Informational diagnostic.
    Info,
    /// Hint diagnostic.
    Hint,
}

/// Structured protocol diagnostic shared by proposal, conflict, and audit DTOs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolDiagnostic {
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Diagnostic severity.
    pub severity: ProtocolDiagnosticSeverity,
    /// Optional related path.
    pub path: Option<CanonicalPath>,
    /// Optional related text range.
    pub range: Option<ProtocolTextRange>,
}

/// Typed conflict/save state for file-backed buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileConflictLifecycleState {
    /// Buffer and disk are clean relative to the last acknowledged fingerprint.
    Clean,
    /// Buffer has unpersisted changes.
    Dirty,
    /// Save is currently in progress.
    Saving,
    /// A save attempt failed.
    SaveFailed,
    /// Disk changed while the buffer is clean.
    DiskChangedClean,
    /// Disk and buffer both changed and require conflict handling.
    ConflictDirty,
    /// Reload is available for a disk-changed buffer.
    ReloadAvailable,
    /// Keep-both resolution is pending.
    KeepBothPending,
    /// Compare resolution is pending.
    ComparePending,
}

/// Typed reason for a conflict or save-state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileConflictReason {
    /// No conflict reason applies.
    None,
    /// Buffer contains unsaved changes.
    BufferDirty,
    /// Save is already in progress.
    SaveInProgress,
    /// Previous save attempt failed.
    SaveFailed,
    /// Disk fingerprint changed from the expected fingerprint.
    DiskFingerprintChanged,
    /// File disappeared from disk.
    FileDeletedOnDisk,
    /// File appeared on disk while a save was pending.
    FileCreatedOnDisk,
    /// Required metadata was unavailable.
    MetadataUnavailable,
    /// User requested reload resolution.
    UserRequestedReload,
    /// User requested keep-both resolution.
    UserRequestedKeepBoth,
    /// User requested compare resolution.
    UserRequestedCompare,
}

/// Structured context for file conflict and save-state DTOs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConflictContext {
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
    /// Fingerprint observed on disk.
    pub disk_fingerprint: Option<FileFingerprint>,
    /// Fingerprint expected by the caller or proposal.
    pub expected_fingerprint: Option<FileFingerprint>,
    /// Typed conflict reason.
    pub reason: FileConflictReason,
    /// Diagnostics associated with this context.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Conflict state between disk and buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConflictState {
    /// Typed lifecycle state.
    pub state: FileConflictLifecycleState,
    /// Structured conflict context.
    pub context: FileConflictContext,
    /// State-level diagnostics.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Conflict DTO schema version.
    pub schema_version: u16,
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

/// Request to open a fully resolved text buffer in the editor authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorOpenBufferRequest {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier supplied by workspace authority.
    pub file_id: FileId,
    /// Canonical file path for display and file-to-buffer binding.
    pub path: CanonicalPath,
    /// UTF-8 text used to initialize the buffer.
    pub initial_text: String,
    /// Correlation id for the open command.
    pub correlation_id: CorrelationId,
}

/// Editor-emitted save request DTO used by proposal/workspace save orchestration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSaveRequest {
    /// Request id.
    pub request_id: Uuid,
    /// Workspace id.
    pub workspace_id: WorkspaceId,
    /// Buffer id.
    pub buffer_id: BufferId,
    /// File id.
    pub file_id: FileId,
    /// Snapshot id to persist.
    pub snapshot_id: SnapshotId,
    /// Buffer version associated with snapshot.
    pub buffer_version: BufferVersion,
    /// Content hash for compare-and-save preconditions.
    pub content_hash: String,
    /// UTF-8 payload byte length for proposal capability checks.
    pub payload_byte_len: u64,
    /// UTF-8 text payload to persist through workspace/proposal ports.
    pub text: String,
    /// Emission timestamp.
    pub requested_at: TimestampMillis,
    /// Caller or generated correlation id.
    pub correlation_id: CorrelationId,
}

/// Typed editor acknowledgement for a save request routed through the editor port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorSaveOutcome {
    /// Save applied successfully.
    Saved,
    /// Proposal became stale before apply.
    Stale {
        /// Optional conflict state projected from the stale response.
        conflict: Option<FileConflictState>,
        /// Diagnostics recorded for later UI projection.
        diagnostics: Vec<ProtocolDiagnostic>,
    },
    /// Proposal encountered a disk/buffer conflict.
    Conflict {
        /// Queryable conflict state.
        conflict: FileConflictState,
    },
    /// Save was denied by policy.
    Denied {
        /// Diagnostics recorded for later UI projection.
        diagnostics: Vec<ProtocolDiagnostic>,
    },
    /// Save failed while applying or validating.
    Failed {
        /// Diagnostics recorded for later UI projection.
        diagnostics: Vec<ProtocolDiagnostic>,
    },
}

/// Save acknowledgement request routed back into the editor authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSaveAcknowledgement {
    /// Save request being acknowledged.
    pub request_id: Uuid,
    /// Typed save outcome.
    pub outcome: EditorSaveOutcome,
}

/// Request to apply a text edit batch to an existing editor buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorApplyTransactionRequest {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Buffer identifier.
    pub buffer_id: BufferId,
    /// File identifier.
    pub file_id: FileId,
    /// Ordered edit batch.
    pub edits: EditBatch,
    /// Source of the transaction.
    pub source: TransactionSource,
    /// Optional undo group.
    pub undo_group_id: Option<Uuid>,
    /// Correlation id.
    pub correlation_id: CorrelationId,
}

/// Request to build a viewport projection for a buffer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EditorViewportRequest {
    /// Buffer identifier.
    pub buffer_id: BufferId,
    /// Scroll offsets.
    pub scroll: ViewportScroll,
    /// Viewport dimensions.
    pub dimensions: ViewportDimensions,
}

/// Protocol buffer metadata projected from the editor authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorBufferMetadata {
    /// Owning workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Buffer identifier.
    pub buffer_id: BufferId,
    /// File identifier.
    pub file_id: FileId,
    /// Canonical path for display.
    pub path: CanonicalPath,
    /// Current snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Current buffer version.
    pub buffer_version: BufferVersion,
    /// Current byte length.
    pub byte_len: u64,
    /// Current content hash.
    pub content_hash: Option<String>,
    /// Whether the buffer has unsaved changes.
    pub dirty: bool,
    /// Current save/conflict lifecycle state.
    pub save_state: FileConflictLifecycleState,
    /// Latest conflict state when one is active.
    pub conflict: Option<FileConflictState>,
    /// Number of undo entries retained for the buffer.
    pub undo_len: usize,
    /// Number of redo entries retained for the buffer.
    pub redo_len: usize,
    /// Metadata DTO schema version.
    pub schema_version: u16,
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

/// Zero-based line index range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LineIndexRange {
    /// Inclusive start line index.
    pub start: u32,
    /// Exclusive end line index.
    pub end: u32,
}

/// Snapshot chunk descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotChunkDescriptor {
    /// Snapshot identifier owning the chunk.
    pub snapshot_id: SnapshotId,
    /// Zero-based chunk ordinal within the snapshot.
    pub chunk_index: u32,
    /// Absolute byte range covered by the chunk.
    pub byte_range: ByteRange,
    /// Zero-based logical line range covered by the chunk.
    pub line_range: LineIndexRange,
    /// Chunk byte length.
    pub byte_len: u64,
    /// Algorithm-tagged hash for the chunk contents.
    pub chunk_hash: FileFingerprint,
    /// Chunk descriptor schema version.
    pub schema_version: u16,
}

/// Snapshot lease consumer kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SnapshotConsumerKind {
    /// Core editor runtime.
    Editor,
    /// Projection-only UI renderer.
    #[serde(rename = "UI")]
    Ui,
    /// Language-server consumer.
    #[serde(rename = "LSP")]
    Lsp,
    /// Future indexing consumer.
    Index,
    /// Future plugin consumer.
    Plugin,
    /// Future AI consumer.
    #[serde(rename = "AI")]
    Ai,
    /// Future collaboration consumer.
    Collaboration,
    /// Storage or persistence observer.
    Storage,
    /// Observability and audit sink.
    Observability,
}

/// Snapshot lease descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotLeaseDescriptor {
    /// Stable lease identifier.
    pub lease_id: Uuid,
    /// Snapshot identifier guarded by the lease.
    pub snapshot_id: SnapshotId,
    /// Consumer category holding the lease.
    pub consumer_kind: SnapshotConsumerKind,
    /// Lease expiration time.
    pub expires_at: TimestampMillis,
    /// Number of chunks pinned by the lease.
    pub chunk_count: u32,
    /// Lease descriptor schema version.
    pub schema_version: u16,
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
    /// Changed ranges with byte + UTF-16 metadata.
    pub changed_ranges: Vec<ChangedTextRange>,
    /// Causality chain identifier.
    pub causality_id: CausalityId,
    /// Optional parent transaction id when transaction is causally linked.
    pub parent_transaction_id: Option<Uuid>,
    /// Transaction DTO schema version.
    pub schema_version: u16,
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

/// Source feature that produced a proposal-ready workspace edit payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceEditSourceKind {
    /// LSP rename operation.
    LspRename,
    /// LSP formatting operation.
    LspFormatting,
    /// LSP code action operation.
    LspCodeAction,
    /// Semantic refactoring preview.
    SemanticRefactor,
    /// Plugin-produced proposal.
    Plugin,
    /// User-authored proposal.
    User,
}

/// File-scoped text edits inside a proposal-ready workspace edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTextEdit {
    /// Target file identity.
    pub file: FileIdentity,
    /// Open buffer affected by the edit, when known.
    pub buffer_id: Option<BufferId>,
    /// Ordered edit batch for this file.
    pub edits: EditBatch,
    /// Version preconditions required before this file edit may apply.
    pub preconditions: ProposalVersionPreconditions,
}

/// File operation inside a proposal-ready workspace edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceFileOperation {
    /// Create a file without embedding full source content in the operation descriptor.
    Create {
        /// Destination path.
        path: CanonicalPath,
        /// Optional hash of the initial content supplied out-of-band.
        initial_content_hash: Option<FileFingerprint>,
    },
    /// Delete an existing file.
    Delete {
        /// File to delete.
        file: FileIdentity,
    },
    /// Rename or move an existing file.
    Rename {
        /// File to rename.
        file: FileIdentity,
        /// Destination path.
        destination: CanonicalPath,
    },
}

/// Proposal-ready workspace edit payload for LSP and semantic mutation producers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEditProposalPayload {
    /// Workspace receiving the proposal.
    pub workspace_id: WorkspaceId,
    /// Stable edit identifier for this payload.
    pub edit_id: Uuid,
    /// User-visible title.
    pub title: String,
    /// Feature source for audit and preview routing.
    pub source: WorkspaceEditSourceKind,
    /// Deterministic affected-target coverage.
    pub target_coverage: ProposalTargetCoverage,
    /// File text edits grouped by target file.
    pub file_edits: Vec<WorkspaceTextEdit>,
    /// File create/delete/rename operations.
    pub file_operations: Vec<WorkspaceFileOperation>,
    /// Capability required before any mutation may apply.
    pub required_capability: CapabilityId,
    /// Diagnostics explaining proposal translation decisions.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Workspace edit payload schema version.
    pub schema_version: u16,
}

// -----------------------------------------------------------------------------
// Proposal contracts
// -----------------------------------------------------------------------------

/// Proposal preconditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalVersionPreconditions {
    /// Legacy file content version alias.
    pub file_version: Option<FileContentVersion>,
    /// Buffer version.
    pub buffer_version: Option<BufferVersion>,
    /// Snapshot id.
    pub snapshot_id: Option<SnapshotId>,
    /// Legacy workspace generation alias.
    pub generation: Option<WorkspaceGeneration>,
    /// Expected file content version.
    #[serde(default)]
    pub file_content_version: Option<FileContentVersion>,
    /// Expected workspace generation.
    #[serde(default)]
    pub workspace_generation: Option<WorkspaceGeneration>,
    /// Expected disk fingerprint.
    #[serde(default)]
    pub expected_fingerprint: Option<FileFingerprint>,
    /// Expected file length when available.
    #[serde(default)]
    pub expected_file_length: Option<u64>,
    /// Expected modified timestamp when available.
    #[serde(default)]
    pub expected_modified_at: Option<TimestampMillis>,
}

/// Proposal versioning context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionContext {
    /// Legacy file content version alias.
    pub file_version: FileContentVersion,
    /// Buffer version.
    pub buffer_version: BufferVersion,
    /// Snapshot id.
    pub snapshot_id: SnapshotId,
    /// Legacy workspace generation alias.
    pub generation: WorkspaceGeneration,
    /// Current file content version.
    pub file_content_version: FileContentVersion,
    /// Current workspace generation.
    pub workspace_generation: WorkspaceGeneration,
    /// Current disk fingerprint when available.
    pub fingerprint: Option<FileFingerprint>,
    /// Current file length when available.
    pub file_length: Option<u64>,
    /// Current modified timestamp when available.
    pub modified_at: Option<TimestampMillis>,
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
        if let Some(expected) = self.file_content_version
            && expected != context.file_content_version
        {
            return true;
        }
        if let Some(expected) = self.workspace_generation
            && expected != context.workspace_generation
        {
            return true;
        }
        if let Some(expected) = &self.expected_fingerprint
            && context.fingerprint.as_ref() != Some(expected)
        {
            return true;
        }
        if let Some(expected) = self.expected_file_length
            && context.file_length != Some(expected)
        {
            return true;
        }
        if let Some(expected) = self.expected_modified_at
            && context.modified_at != Some(expected)
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
    /// Proposal-ready workspace edit produced by LSP or semantic tooling.
    WorkspaceEdit(WorkspaceEditProposalPayload),
    /// Terminal command.
    TerminalCommand(TerminalCommandProposal),
    /// Ordered batch or multi-target proposal.
    Batch(BatchProposalPayload),
}

/// Atomicity boundary promised by a batch proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalBatchAtomicity {
    /// Every item must prepare and apply as one logical unit or none may commit.
    AllOrNothing,
    /// Every item must prepare successfully before any mutation starts.
    PrepareAllBeforeMutate,
    /// Items apply in deterministic order and partial-failure records are mandatory on failure.
    OrderedNonAtomic,
}

/// Rollback expectation for a batch proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalBatchRollbackPolicy {
    /// Rollback is required for every mutation item that can commit.
    Required,
    /// Rollback is attempted but may produce explicit failure records.
    BestEffort,
    /// Rollback is not supported and apply must fail closed unless explicitly allowed.
    NotSupported,
    /// No rollback is required because the batch is metadata-only or preflight-only.
    NotRequired,
}

/// Target class affected by a proposal payload or batch item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalTargetKind {
    /// Open editor buffer target.
    OpenBuffer,
    /// Closed or disk-backed file target.
    ClosedFile,
    /// Path-only target without a resolved file identity.
    PathOnly,
    /// Terminal session target.
    TerminalSession,
    /// Remote workspace target.
    RemoteWorkspace,
    /// Collaboration session target.
    CollaborationSession,
    /// Plugin-owned target.
    Plugin,
    /// Metadata-only target with no durable state mutation.
    MetadataOnly,
}

/// Deterministic affected-target descriptor for proposal previews and audit records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalAffectedTarget {
    /// Stable target identifier within the proposal or batch.
    pub target_id: String,
    /// Target class.
    pub kind: ProposalTargetKind,
    /// Workspace identifier when known.
    pub workspace_id: Option<WorkspaceId>,
    /// File identifier when known.
    pub file_id: Option<FileId>,
    /// Buffer identifier when known.
    pub buffer_id: Option<BufferId>,
    /// Canonical path when a path can be disclosed.
    pub path: Option<CanonicalPath>,
    /// Terminal session identifier when the target is terminal-scoped.
    pub terminal_session_id: Option<TerminalSessionId>,
    /// Plugin identifier when the target is plugin-scoped.
    pub plugin_id: Option<PluginId>,
    /// Redacted remote authority label or hash.
    pub remote_authority: Option<String>,
    /// Collaboration session label or hash.
    pub collaboration_session_id: Option<String>,
    /// Affected byte ranges when metadata can disclose ranges.
    pub byte_ranges: Vec<ByteRange>,
    /// Redaction hints that apply to this target descriptor.
    pub redaction_hints: Vec<RedactionHint>,
}

/// Completeness of affected-target coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalTargetCoverageKind {
    /// All affected targets are represented.
    Complete,
    /// Some affected targets are represented and omissions are counted.
    Partial,
    /// Target metadata exists but was redacted for policy or privacy reasons.
    Redacted,
}

/// Deterministic affected-target coverage for a proposal or batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalTargetCoverage {
    /// Coverage completeness.
    pub coverage_kind: ProposalTargetCoverageKind,
    /// Targets in deterministic display and audit order.
    pub targets: Vec<ProposalAffectedTarget>,
    /// Number of omitted targets when coverage is partial or redacted.
    pub omitted_target_count: u32,
    /// Redaction hints that apply to the coverage record.
    pub redaction_hints: Vec<RedactionHint>,
}

/// Ordered batch proposal payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProposalPayload {
    /// Stable batch identifier.
    pub batch_id: Uuid,
    /// Batch atomicity boundary.
    pub atomicity: ProposalBatchAtomicity,
    /// Rollback policy for committed steps.
    pub rollback_policy: ProposalBatchRollbackPolicy,
    /// Affected target coverage for the whole batch.
    pub target_coverage: ProposalTargetCoverage,
    /// Ordered proposal items. The `order` field is the authoritative application order.
    pub items: Vec<ProposalBatchItem>,
    /// Deterministic dependency edges between item identifiers.
    pub dependency_edges: Vec<ProposalBatchDependency>,
    /// Deterministic rollback plan records.
    pub rollback_steps: Vec<ProposalRollbackStep>,
    /// Partial-failure records captured during planning or apply.
    pub partial_failures: Vec<ProposalPartialFailureRecord>,
    /// Bounded preview warnings for the batch.
    pub preview_warnings: Vec<ProposalPreviewWarning>,
    /// Batch DTO schema version.
    pub schema_version: u16,
}

/// One ordered item inside a batch proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalBatchItem {
    /// Zero-based deterministic application order.
    pub order: u32,
    /// Stable item identifier unique within the batch.
    pub item_id: String,
    /// Item payload.
    pub payload: Box<ProposalPayload>,
    /// Target identifiers from the batch coverage record affected by this item.
    pub target_ids: Vec<String>,
    /// Capability required by this item.
    pub required_capability: CapabilityId,
    /// Rollback step identifiers associated with this item.
    pub rollback_step_ids: Vec<String>,
}

/// Dependency semantics between batch items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalBatchDependencyKind {
    /// The dependent item requires the prerequisite item to validate successfully.
    RequiresValidation,
    /// The dependent item requires the prerequisite item to apply successfully.
    RequiresApply,
    /// The two items conflict and cannot both be applied.
    ConflictsWith,
}

/// Deterministic dependency edge between batch items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalBatchDependency {
    /// Prerequisite item identifier.
    pub prerequisite_item_id: String,
    /// Dependent item identifier.
    pub dependent_item_id: String,
    /// Dependency semantics.
    pub kind: ProposalBatchDependencyKind,
}

/// Rollback action kind for a committed mutation item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalRollbackAction {
    /// Undo through an editor undo group.
    EditorUndoGroup,
    /// Restore a file snapshot or backup.
    RestoreFileSnapshot,
    /// Delete a file created by the proposal.
    DeleteCreatedFile,
    /// Recreate a file deleted by the proposal.
    RecreateDeletedFile,
    /// Rename a path back to its prior identity.
    RenamePathBack,
    /// Cancel or compensate terminal-side work.
    CancelTerminalCommand,
    /// Emit metadata only; no state rollback is needed.
    MetadataOnlyRecord,
    /// Rollback is unsupported and must be recorded explicitly.
    Unsupported,
}

/// Deterministic rollback step descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalRollbackStep {
    /// Zero-based rollback order.
    pub order: u32,
    /// Stable rollback step identifier.
    pub step_id: String,
    /// Batch item identifier that owns this rollback step.
    pub item_id: String,
    /// Target identifier covered by this rollback step.
    pub target_id: String,
    /// Rollback action.
    pub action: ProposalRollbackAction,
    /// Preconditions expected before rollback when known.
    pub expected_preconditions: ProposalVersionPreconditions,
    /// Diagnostics associated with the rollback step.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Disposition of an item after a batch partial failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalPartialFailureDisposition {
    /// Item was not started.
    NotStarted,
    /// Item failed before mutating state.
    FailedBeforeMutation,
    /// Item mutation committed and remains present.
    MutationCommitted,
    /// Item mutation committed and was rolled back.
    RolledBack,
    /// Rollback failed for the item.
    RollbackFailed,
    /// Dirty editor buffer was preserved rather than discarded.
    PreservedDirtyBuffer,
}

/// Partial-failure record for a batch item or target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalPartialFailureRecord {
    /// Batch item identifier.
    pub item_id: String,
    /// Target identifier.
    pub target_id: String,
    /// Failure reason.
    pub reason: ProposalFailureReason,
    /// Item disposition after the failure.
    pub disposition: ProposalPartialFailureDisposition,
    /// Diagnostics associated with the failure.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Preview warning kind for generalized proposals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalPreviewWarningKind {
    /// Atomicity cannot be guaranteed by the current target set.
    AtomicityUnavailable,
    /// Rollback is best-effort rather than exact.
    RollbackBestEffort,
    /// Target coverage is partial or redacted.
    TargetCoveragePartial,
    /// Policy is expected to deny apply.
    PolicyWillDenyApply,
    /// Raw source content was redacted from the preview.
    RawSourceRedacted,
    /// Runtime implementation is intentionally unsupported in this phase.
    UnsupportedRuntime,
}

/// Bounded warning emitted during proposal preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalPreviewWarning {
    /// Stable warning code.
    pub code: String,
    /// Warning kind.
    pub kind: ProposalPreviewWarningKind,
    /// Human-readable warning message.
    pub message: String,
    /// Optional target identifier.
    pub target_id: Option<String>,
    /// Redaction hints for this warning.
    pub redaction_hints: Vec<RedactionHint>,
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

/// User or system intent behind a save proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveIntent {
    /// Explicit user-invoked save.
    Manual,
    /// Automatic save.
    AutoSave,
    /// Save requested after format-on-save.
    FormatOnSave,
    /// Save requested during shutdown or workspace close.
    Shutdown,
    /// Save requested by an extension, command, or automation.
    ExternalCommand,
}

/// Policy for handling conflicts encountered by a save proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveConflictPolicy {
    /// Reject the save when disk state differs from the expected fingerprint.
    RejectIfChanged,
    /// Prompt the user before resolving the conflict.
    PromptUser,
    /// Reload from disk before attempting a save.
    ReloadThenSave,
    /// Preserve both buffer and disk content.
    KeepBoth,
    /// Open a compare flow before deciding.
    CompareBeforeSaving,
}

/// Trust and decision context associated with privileged proposal creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustDecisionContext {
    /// Workspace trust state observed for this proposal.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Capability decision id when a broker decision exists.
    pub decision_id: Option<CapabilityDecisionId>,
    /// Decision timestamp when available.
    pub decided_at: Option<TimestampMillis>,
}

/// Save proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveFileProposal {
    /// File identity.
    pub file: FileIdentity,
    /// Buffer identifier being saved.
    pub buffer_id: BufferId,
    /// File identifier being saved.
    pub file_id: FileId,
    /// Snapshot id.
    pub snapshot_id: SnapshotId,
    /// Buffer version being saved.
    pub buffer_version: BufferVersion,
    /// File content version expected by the save.
    pub file_content_version: FileContentVersion,
    /// Workspace generation expected by the save.
    pub workspace_generation: WorkspaceGeneration,
    /// Expected disk fingerprint before writing.
    pub expected_fingerprint: Option<FileFingerprint>,
    /// Save intent.
    pub save_intent: SaveIntent,
    /// Conflict handling policy.
    pub conflict_policy: SaveConflictPolicy,
    /// Trust decision context.
    pub trust_decision: TrustDecisionContext,
    /// Required capability for this save.
    pub required_capability: CapabilityId,
    /// Principal requesting the save.
    pub principal: PrincipalId,
    /// Correlation id for this save proposal.
    pub correlation_id: CorrelationId,
    /// Proposal diagnostics.
    pub diagnostics: Vec<ProtocolDiagnostic>,
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

/// Discriminant for proposal payload summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalPayloadKind {
    /// Text edit payload.
    TextEdit,
    /// Create-file payload.
    CreateFile,
    /// Delete-file payload.
    DeleteFile,
    /// Rename-file payload.
    RenameFile,
    /// Save-file payload.
    SaveFile,
    /// Format-file payload.
    FormatFile,
    /// Code-action payload.
    CodeAction,
    /// Workspace-edit payload.
    WorkspaceEdit,
    /// Terminal-command payload.
    TerminalCommand,
    /// Batch payload.
    Batch,
}

/// Proposal lifecycle state suitable for response and audit records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalLifecycleState {
    /// Proposal was created.
    Created,
    /// Proposal was validated.
    Validated,
    /// Proposal preview was produced.
    Previewed,
    /// Proposal was approved.
    Approved,
    /// Proposal was rejected by a user or validation flow.
    Rejected,
    /// Proposal was applied.
    Applied,
    /// Proposal was denied by policy.
    Denied,
    /// Proposal failed while processing.
    Failed,
    /// Proposal changes were rolled back.
    RolledBack,
    /// Proposal became stale before application.
    Stale,
    /// Proposal encountered a file conflict.
    Conflict,
    /// Proposal was cancelled before apply completed.
    Cancelled,
}

/// Typed reason for proposal rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalRejectionReason {
    /// User rejected the proposal.
    UserRejected,
    /// Validation rejected the proposal.
    ValidationFailed,
    /// Proposal expired.
    Expired,
    /// Proposal kind is unsupported.
    Unsupported,
    /// Proposal was cancelled.
    Cancelled,
}

/// Typed reason for proposal denial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalDenialReason {
    /// Required capability was denied.
    CapabilityDenied,
    /// Workspace was not trusted.
    WorkspaceUntrusted,
    /// Principal was unauthorized.
    PrincipalUnauthorized,
    /// Policy denied the proposal.
    PolicyDenied,
}

/// Typed reason for proposal failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalFailureReason {
    /// Apply operation failed.
    ApplyFailed,
    /// Rollback operation failed.
    RollbackFailed,
    /// Audit or metadata storage failed.
    StorageFailed,
    /// Internal proposal engine failure.
    InternalError,
}

/// Typed reason for proposal rollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalRollbackReason {
    /// Rollback followed an apply failure.
    ApplyFailed,
    /// Rollback was requested by a user.
    UserRequested,
    /// Rollback was requested by policy.
    PolicyRequested,
    /// Rollback was requested by the system.
    SystemRequested,
}

/// Typed reason for proposal cancellation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalCancellationReason {
    /// User cancelled the proposal.
    UserCancelled,
    /// Proposal was superseded by a newer proposal.
    Superseded,
    /// Proposal expired before completion.
    Expired,
    /// Proposal was cancelled during shutdown or workspace close.
    SystemShutdown,
    /// Proposal was cancelled by policy.
    PolicyCancelled,
}

/// Typed reason for proposal staleness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStaleReason {
    /// File content version mismatch.
    FileContentVersionMismatch,
    /// Buffer version mismatch.
    BufferVersionMismatch,
    /// Snapshot id mismatch.
    SnapshotMismatch,
    /// Workspace generation mismatch.
    WorkspaceGenerationMismatch,
    /// Disk fingerprint mismatch.
    FingerprintMismatch,
    /// File length mismatch.
    FileLengthMismatch,
    /// Modified timestamp mismatch.
    ModifiedTimestampMismatch,
}

/// Common lifecycle transition metadata for proposal responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalLifecycleTransition {
    /// Proposal identifier.
    pub proposal_id: ProposalId,
    /// Lifecycle state.
    pub lifecycle_state: ProposalLifecycleState,
    /// Transition timestamp.
    pub timestamp: TimestampMillis,
    /// Principal responsible for the transition.
    pub principal: PrincipalId,
    /// Capability associated with the proposal.
    pub capability: CapabilityId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Transition diagnostics.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Lifecycle action represented by proposal request commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalLifecycleAction {
    /// Validate action.
    Validate,
    /// Preview action.
    Preview,
    /// Approve action.
    Approve,
    /// Reject action.
    Reject,
    /// Apply action.
    Apply,
    /// Cancel action.
    Cancel,
    /// Rollback action.
    Rollback,
}

/// Typed reason attached to a proposal lifecycle command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalLifecycleCommandReason {
    /// Rejection reason.
    Rejection(ProposalRejectionReason),
    /// Cancellation reason.
    Cancellation(ProposalCancellationReason),
    /// Rollback reason.
    Rollback(ProposalRollbackReason),
    /// Failure reason.
    Failure(ProposalFailureReason),
    /// Metadata-only free-form note.
    Note(String),
}

/// Explicit proposal lifecycle command for approve, reject, cancel, and rollback intents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalLifecycleCommand {
    /// Proposal identifier.
    pub proposal_id: ProposalId,
    /// Lifecycle action requested.
    pub action: ProposalLifecycleAction,
    /// Principal requesting the lifecycle command.
    pub principal: PrincipalId,
    /// Capability associated with the proposal lifecycle command.
    pub capability: CapabilityId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Optional typed reason for the command.
    pub reason: Option<ProposalLifecycleCommandReason>,
    /// Command diagnostics.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Request timestamp.
    pub requested_at: TimestampMillis,
    /// Command DTO schema version.
    pub schema_version: u16,
}

/// Structured context for stale proposal responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalStaleContext {
    /// Stale reason.
    pub reason: ProposalStaleReason,
    /// Expected preconditions.
    pub expected: ProposalVersionPreconditions,
    /// Actual version context when available.
    pub actual: Option<VersionContext>,
}

/// Compact summary of a proposal payload for audit persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalPayloadSummary {
    /// Payload kind.
    pub kind: ProposalPayloadKind,
    /// Affected file identifiers.
    pub affected_files: Vec<FileId>,
    /// Optional display title.
    pub title: Option<String>,
    /// Optional payload byte count.
    pub byte_count: Option<u64>,
}

/// Proposal audit record suitable for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalAuditRecord {
    /// Proposal identifier.
    pub proposal_id: ProposalId,
    /// Lifecycle state.
    pub lifecycle_state: ProposalLifecycleState,
    /// Audit timestamp.
    pub timestamp: TimestampMillis,
    /// Principal associated with the transition.
    pub principal: PrincipalId,
    /// Capability associated with the proposal.
    pub capability: CapabilityId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Payload summary.
    pub payload_summary: ProposalPayloadSummary,
    /// Diagnostics captured for the transition.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Redaction hints for persisted fields.
    pub redaction_hints: Vec<RedactionHint>,
    /// Audit DTO schema version.
    pub schema_version: u16,
}

// -----------------------------------------------------------------------------
// LSP contracts
// -----------------------------------------------------------------------------

/// Supervised LSP operation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspResultStatus {
    /// Result is fresh for the requested snapshot and content identity.
    Fresh,
    /// Result is stale and must not overwrite newer state.
    Stale,
    /// Result is partial but usable for degraded UI or semantic enrichment.
    Partial,
    /// Operation was cancelled before completion.
    Cancelled,
    /// Operation timed out.
    Timeout,
    /// Server or feature was unavailable.
    Unavailable,
    /// Result was produced under degraded runtime conditions.
    Degraded,
}

/// Shared request context for cancellable supervised LSP operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspOperationContext {
    /// Stable request identifier.
    pub request_id: LspRequestId,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Buffer identifier for the synced document.
    pub buffer_id: BufferId,
    /// Snapshot used by the request.
    pub snapshot_id: SnapshotId,
    /// Buffer version used by the request.
    pub buffer_version: BufferVersion,
    /// Language identifier.
    pub language_id: LanguageId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Timeout budget in milliseconds.
    pub timeout_ms: u64,
    /// Cancellation token for this operation.
    pub cancellation_token: CancellationTokenId,
    /// Content hash used to discard stale responses.
    pub content_hash: Option<FileFingerprint>,
    /// Privacy scope of request metadata and response payloads.
    pub privacy_scope: SemanticPrivacyScope,
    /// LSP operation context schema version.
    pub schema_version: u16,
}

/// Shared metadata for supervised LSP responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspResultMetadata {
    /// Request identifier this response resolves.
    pub request_id: LspRequestId,
    /// Language server that produced the response.
    pub server_id: LanguageServerId,
    /// Snapshot the result claims to describe.
    pub snapshot_id: SnapshotId,
    /// Buffer version the result claims to describe.
    pub buffer_version: BufferVersion,
    /// Content hash the result claims to describe.
    pub content_hash: Option<FileFingerprint>,
    /// Supervised result status.
    pub status: LspResultStatus,
    /// Response generation timestamp.
    pub generated_at: TimestampMillis,
    /// Metadata-only diagnostics produced while handling the response.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// LSP result metadata schema version.
    pub schema_version: u16,
}

/// Normalized LSP target location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspLocation {
    /// Workspace containing the location.
    pub workspace_id: WorkspaceId,
    /// File containing the location.
    pub file_id: FileId,
    /// Canonical path when disclosure is allowed.
    pub path: CanonicalPath,
    /// Full location range.
    pub range: ProtocolTextRange,
    /// Narrow target selection range when supplied by the server.
    pub target_selection_range: Option<ProtocolTextRange>,
    /// Bounded symbol display name when available.
    pub symbol_name: Option<String>,
    /// Server-provided symbol kind when available.
    pub symbol_kind: Option<String>,
}

/// Definition lookup request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspDefinitionRequest {
    /// Shared supervised operation context.
    pub context: LspOperationContext,
    /// Lookup position.
    pub position: TextCoordinate,
    /// Whether declaration-like fallback locations may be returned.
    pub include_declaration: bool,
}

/// Definition lookup response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspDefinitionResponse {
    /// Response metadata.
    pub metadata: LspResultMetadata,
    /// Definition target locations.
    pub locations: Vec<LspLocation>,
}

/// Scope for an LSP reference lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspReferenceScope {
    /// Current document only.
    Document,
    /// Current workspace.
    Workspace,
    /// Repository-wide lookup when supported by policy and server capability.
    Repository,
}

/// Reference lookup request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspReferenceRequest {
    /// Shared supervised operation context.
    pub context: LspOperationContext,
    /// Lookup position.
    pub position: TextCoordinate,
    /// Whether declaration locations should be included with references.
    pub include_declaration: bool,
    /// Requested lookup scope.
    pub scope: LspReferenceScope,
}

/// Reference lookup response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspReferenceResponse {
    /// Response metadata.
    pub metadata: LspResultMetadata,
    /// Reference locations.
    pub references: Vec<LspLocation>,
}

/// Command descriptor for command-only or mixed LSP code actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCommandDescriptor {
    /// Server command identifier.
    pub command_id: String,
    /// User-visible command title.
    pub title: String,
    /// Redacted command argument descriptors.
    pub argument_hints: Vec<String>,
}

/// Proposal routing result for mutation-producing LSP operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LspMutationProposalResult {
    /// A complete workspace proposal is ready for validation and preview.
    Proposal(Box<WorkspaceProposal>),
    /// The operation completed without producing any mutation.
    NoChanges {
        /// Diagnostics explaining why no mutation is needed.
        diagnostics: Vec<ProtocolDiagnostic>,
    },
    /// The operation could not be safely represented as a proposal.
    Rejected {
        /// Metadata-only rejection reason.
        reason: String,
        /// Diagnostics explaining the rejection.
        diagnostics: Vec<ProtocolDiagnostic>,
    },
    /// A command-only action was denied or deferred because it is not an edit proposal.
    CommandOnlyDenied {
        /// Command descriptor that was not executed.
        command: LspCommandDescriptor,
        /// Diagnostics explaining the denial or deferral.
        diagnostics: Vec<ProtocolDiagnostic>,
    },
}

/// Rename request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspRenameRequest {
    /// Shared supervised operation context.
    pub context: LspOperationContext,
    /// Rename position.
    pub position: TextCoordinate,
    /// Requested replacement symbol name.
    pub new_name: String,
    /// Whether this request only prepares rename metadata.
    pub prepare_only: bool,
}

/// Prepare-rename response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspPrepareRenameResult {
    /// Range that may be renamed.
    pub range: ProtocolTextRange,
    /// Placeholder displayed to the user.
    pub placeholder: Option<String>,
    /// Whether rename is allowed at the requested position.
    pub allowed: bool,
}

/// Rename response represented as proposal-ready output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspRenameResponse {
    /// Response metadata.
    pub metadata: LspResultMetadata,
    /// Prepare-rename metadata when requested or available.
    pub prepare: Option<LspPrepareRenameResult>,
    /// Proposal routing result for edit-producing rename output.
    pub proposal: Option<LspMutationProposalResult>,
}

/// Formatting mode requested from an LSP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspFormattingMode {
    /// Whole-document formatting.
    Document,
    /// Range formatting.
    Range,
    /// On-type formatting.
    OnType,
}

/// Deterministic formatting options for LSP requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspFormattingOptions {
    /// Tab size in columns.
    pub tab_size: u16,
    /// Whether spaces should be inserted instead of tab characters.
    pub insert_spaces: bool,
    /// Whether trailing whitespace should be trimmed by the formatter.
    pub trim_trailing_whitespace: bool,
    /// Whether a final newline should be inserted.
    pub insert_final_newline: bool,
    /// Deterministic custom option key-value pairs.
    pub custom_options: Vec<(String, String)>,
}

/// Formatting request that requires proposal-mediated output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspFormattingProposalRequest {
    /// Shared supervised operation context.
    pub context: LspOperationContext,
    /// Formatting mode.
    pub mode: LspFormattingMode,
    /// Optional range for range or on-type formatting.
    pub range: Option<ProtocolTextRange>,
    /// Formatting options.
    pub options: LspFormattingOptions,
}

/// Formatting response represented as proposal-ready output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspFormattingProposalResponse {
    /// Response metadata.
    pub metadata: LspResultMetadata,
    /// Proposal routing result for edit-producing formatting output.
    pub proposal: LspMutationProposalResult,
}

/// Code-action request that separates edit and command payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCodeActionProposalRequest {
    /// Shared supervised operation context.
    pub context: LspOperationContext,
    /// Requested range.
    pub range: ProtocolTextRange,
    /// Diagnostics supplied as context for quick fixes.
    pub diagnostics: Vec<LspDiagnostic>,
    /// Requested code-action kinds.
    pub only: Vec<String>,
}

/// Normalized code-action payload category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LspCodeActionPayload {
    /// Command-only actions are not direct mutations and require separate policy routing.
    CommandOnly {
        /// Command descriptor.
        command: LspCommandDescriptor,
    },
    /// Edit-only actions are represented as proposal-ready workspace edits.
    EditOnly {
        /// Workspace edit payload.
        workspace_edit: WorkspaceEditProposalPayload,
    },
    /// Mixed actions carry an edit proposal and a command descriptor for policy routing.
    EditAndCommand {
        /// Workspace edit payload.
        workspace_edit: WorkspaceEditProposalPayload,
        /// Command descriptor.
        command: LspCommandDescriptor,
    },
    /// Disabled action with metadata-only reason.
    Disabled {
        /// Disabled reason.
        reason: String,
    },
}

/// Normalized LSP code-action candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCodeActionCandidate {
    /// User-visible title.
    pub title: String,
    /// Server-provided action kind.
    pub kind: Option<String>,
    /// Preferred action marker.
    pub is_preferred: bool,
    /// Normalized action payload.
    pub payload: LspCodeActionPayload,
    /// Diagnostics associated with action translation.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Code-action response with proposal-ready edit payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCodeActionProposalResponse {
    /// Response metadata.
    pub metadata: LspResultMetadata,
    /// Code-action candidates in deterministic display order.
    pub actions: Vec<LspCodeActionCandidate>,
}

/// LSP cancellation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCancellationRequest {
    /// Request being cancelled.
    pub request_id: LspRequestId,
    /// Cancellation token to propagate.
    pub cancellation_token: CancellationTokenId,
    /// Metadata-only cancellation reason.
    pub reason: String,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
}

/// LSP cancellation acknowledgement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCancellationAck {
    /// Request that was marked cancelled locally.
    pub request_id: LspRequestId,
    /// Cancellation token acknowledged locally.
    pub cancellation_token: CancellationTokenId,
    /// Whether cancellation was propagated to the server.
    pub propagated_to_server: bool,
    /// Acknowledgement timestamp.
    pub acknowledged_at: TimestampMillis,
}

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
    /// Legacy edit batch; callers must route edits through proposal mediation before mutation.
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
    /// Legacy edits; callers must convert edit-producing actions into proposal payloads.
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
// Semantic fabric contracts
// -----------------------------------------------------------------------------

/// Reason semantic work was cancelled or marked obsolete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticCancellationReason {
    /// User requested cancellation.
    UserCancelled,
    /// A newer snapshot superseded the work.
    SnapshotSuperseded,
    /// Content hash no longer matches.
    ContentHashMismatch,
    /// Grammar version changed.
    GrammarVersionChanged,
    /// Model version changed.
    ModelVersionChanged,
    /// Privacy scope was reduced.
    PrivacyScopeReduced,
    /// Queue pressure cancelled or downgraded work.
    QueuePressure,
    /// Workspace or runtime shutdown began.
    Shutdown,
    /// Timeout budget expired.
    Timeout,
}

/// Cancellable semantic work token descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCancellationToken {
    /// Token identifier.
    pub token_id: CancellationTokenId,
    /// Workspace scope.
    pub workspace_id: WorkspaceId,
    /// Optional file scope.
    pub file_id: Option<FileId>,
    /// Optional snapshot scope.
    pub snapshot_id: Option<SnapshotId>,
    /// Optional content hash scope.
    pub content_hash: Option<FileFingerprint>,
    /// Workspace generation scope.
    pub workspace_generation: Option<WorkspaceGeneration>,
    /// Privacy scope guarded by this token.
    pub privacy_scope: SemanticPrivacyScope,
    /// Cancellation reason when known.
    pub reason: Option<SemanticCancellationReason>,
    /// Token issue timestamp.
    pub issued_at: TimestampMillis,
    /// Optional expiration timestamp.
    pub expires_at: Option<TimestampMillis>,
    /// Cancellation token schema version.
    pub schema_version: u16,
}

/// File content and fingerprint identity for semantic cache keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFileFingerprintIdentity {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Canonical file path.
    pub canonical_path: CanonicalPath,
    /// File content version observed by workspace authority.
    pub file_content_version: FileContentVersion,
    /// Workspace generation associated with the identity.
    pub workspace_generation: WorkspaceGeneration,
    /// Content hash used for semantic invalidation.
    pub content_hash: FileFingerprint,
    /// Disk fingerprint used by workspace persistence authority, when known.
    pub disk_fingerprint: Option<FileFingerprint>,
    /// File byte length when known.
    pub byte_len: Option<u64>,
    /// Modified timestamp when known.
    pub modified_at: Option<TimestampMillis>,
    /// Privacy scope for this identity.
    pub privacy_scope: SemanticPrivacyScope,
    /// Fingerprint identity schema version.
    pub schema_version: u16,
}

/// Complete invalidation key for semantic cache records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticInvalidationKey {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Snapshot identifier when record is snapshot-bound.
    pub snapshot_id: Option<SnapshotId>,
    /// File content version.
    pub file_content_version: FileContentVersion,
    /// Workspace generation.
    pub workspace_generation: WorkspaceGeneration,
    /// Content hash.
    pub content_hash: FileFingerprint,
    /// Grammar version for parser-derived records.
    pub grammar_version: Option<SemanticGrammarVersion>,
    /// Model version for learned or ranked records.
    pub model_version: Option<SemanticModelVersion>,
    /// Privacy scope for storage and query exposure.
    pub privacy_scope: SemanticPrivacyScope,
    /// Invalidation key schema version.
    pub schema_version: u16,
}

/// Freshness state for semantic records and query results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticFreshnessState {
    /// Record is fresh for its invalidation key.
    Fresh,
    /// Record is stale and must be labelled as such.
    Stale,
    /// Record is partial due to bounded or degraded processing.
    Partial,
    /// Record is unavailable.
    Unavailable,
}

/// Semantic freshness metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFreshness {
    /// Freshness state.
    pub state: SemanticFreshnessState,
    /// Invalidation key used to compute freshness.
    pub key: SemanticInvalidationKey,
    /// User-visible degraded reasons.
    pub degraded_reasons: Vec<String>,
    /// Freshness observation timestamp.
    pub observed_at: TimestampMillis,
}

/// Source that produced a semantic record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticRecordSource {
    /// Shallow lexical extraction.
    Lexical,
    /// Tree-sitter parser extraction.
    TreeSitter,
    /// Language-server enrichment.
    Lsp,
    /// Workspace metadata.
    WorkspaceMetadata,
    /// Model metadata without vector activation.
    ModelMetadata,
    /// User-provided metadata.
    User,
}

/// Provenance for semantic records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticRecordProvenance {
    /// Record source.
    pub source: SemanticRecordSource,
    /// Language server that enriched the record, when applicable.
    pub server_id: Option<LanguageServerId>,
    /// Extraction contract version.
    pub extraction_version: String,
    /// Confidence in basis points, from 0 to 10_000.
    pub confidence_basis_points: u16,
}

/// Lexical or semantic symbol-to-file map record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolFileMapRecord {
    /// Stable symbol identifier.
    pub symbol_id: SemanticSymbolId,
    /// Hash of the symbol name for metadata-only lookup.
    pub symbol_name_hash: FileFingerprint,
    /// Optional bounded display name.
    pub display_name: Option<String>,
    /// Symbol kind label.
    pub kind: String,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Canonical path.
    pub path: CanonicalPath,
    /// Language identifier.
    pub language_id: LanguageId,
    /// Declaration range when known.
    pub declaration_range: Option<ProtocolTextRange>,
    /// Reference ranges known from the shallow map.
    pub reference_ranges: Vec<ProtocolTextRange>,
    /// Invalidation key.
    pub invalidation_key: SemanticInvalidationKey,
    /// Record provenance.
    pub provenance: SemanticRecordProvenance,
    /// Symbol map schema version.
    pub schema_version: u16,
}

/// Normalized semantic graph record kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticGraphRecordKind {
    /// Symbol declaration or definition record.
    Symbol,
    /// Symbol reference record.
    Reference,
    /// Import relationship.
    Import,
    /// Export relationship.
    Export,
    /// Call edge relationship.
    CallEdge,
    /// Type relationship.
    TypeRelation,
    /// Test-to-target relationship.
    TestLink,
    /// Diagnostic-to-symbol or diagnostic-to-location relationship.
    DiagnosticLink,
    /// Ownership or code-owner metadata.
    OwnershipMetadata,
}

/// Endpoint for normalized semantic graph records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticGraphEndpoint {
    /// Related record identifier when known.
    pub record_id: Option<SemanticRecordId>,
    /// Related symbol identifier when known.
    pub symbol_id: Option<SemanticSymbolId>,
    /// Related file identifier when known.
    pub file_id: Option<FileId>,
    /// Related text range when known.
    pub range: Option<ProtocolTextRange>,
}

/// Metadata property for normalized graph records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticProperty {
    /// Property key.
    pub key: String,
    /// Metadata-only property value or bounded excerpt.
    pub value: String,
    /// Redaction hint for the property.
    pub redaction: RedactionHint,
}

/// Normalized semantic graph record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticGraphRecord {
    /// Stable record identifier.
    pub record_id: SemanticRecordId,
    /// Record kind.
    pub kind: SemanticGraphRecordKind,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Source endpoint.
    pub source: SemanticGraphEndpoint,
    /// Optional target endpoint.
    pub target: Option<SemanticGraphEndpoint>,
    /// Relationship label.
    pub label: String,
    /// Deterministic metadata properties.
    pub properties: Vec<SemanticProperty>,
    /// Invalidation key.
    pub invalidation_key: SemanticInvalidationKey,
    /// Record provenance.
    pub provenance: SemanticRecordProvenance,
    /// Freshness state.
    pub freshness: SemanticFreshnessState,
    /// Graph record schema version.
    pub schema_version: u16,
}

/// Semantic query kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticQueryKind {
    /// Symbol lookup query.
    SymbolLookup,
    /// Definition lookup query.
    Definition,
    /// Reference lookup query.
    References,
    /// Hover enrichment query.
    HoverEnrichment,
    /// Completion ranking query.
    CompletionRanking,
    /// AI context selection query.
    AiContextSelection,
    /// Agent planning query.
    AgentPlanning,
    /// Test impact query.
    TestImpact,
    /// Refactoring preview query.
    RefactoringPreview,
}

/// Scope for a semantic query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQueryScope {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifiers to include.
    pub file_ids: Vec<FileId>,
    /// Paths to include when file ids are unavailable.
    pub paths: Vec<CanonicalPath>,
    /// Language identifiers to include.
    pub language_ids: Vec<LanguageId>,
    /// Privacy scope requested by the caller.
    pub privacy_scope: SemanticPrivacyScope,
}

/// Freshness policy for semantic queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticQueryFreshnessPolicy {
    /// Fresh results are required.
    RequireFresh,
    /// Stale results are allowed when labelled.
    AllowStale,
    /// Metadata-only results are allowed.
    MetadataOnly,
    /// Best-effort result quality is allowed under pressure.
    BestEffort,
}

/// Semantic query request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQueryRequest {
    /// Query identifier.
    pub query_id: SemanticQueryId,
    /// Query kind.
    pub kind: SemanticQueryKind,
    /// Query scope.
    pub scope: SemanticQueryScope,
    /// Optional position for navigation-like queries.
    pub position: Option<TextCoordinate>,
    /// Optional metadata-only hash of the text query.
    pub text_query_hash: Option<FileFingerprint>,
    /// Maximum number of results to return.
    pub limit: u32,
    /// Cancellation token identity.
    pub cancellation_token: CancellationTokenId,
    /// Freshness policy.
    pub freshness_policy: SemanticQueryFreshnessPolicy,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Query request schema version.
    pub schema_version: u16,
}

/// Semantic query response status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticQueryStatus {
    /// Query returned fresh results.
    Fresh,
    /// Query returned stale results.
    Stale,
    /// Query returned partial results.
    Partial,
    /// Query was cancelled.
    Cancelled,
    /// Query timed out.
    Timeout,
    /// Query was unavailable.
    Unavailable,
    /// Query returned degraded results.
    Degraded,
}

/// Semantic query result kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticQueryResultKind {
    /// Symbol result.
    Symbol,
    /// Location result.
    Location,
    /// Graph-record result.
    GraphRecord,
    /// Proposal-preview result.
    ProposalPreview,
    /// Diagnostic result.
    Diagnostic,
    /// Metadata result.
    Metadata,
}

/// Semantic query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQueryResult {
    /// Result identifier.
    pub result_id: SemanticRecordId,
    /// Result kind.
    pub kind: SemanticQueryResultKind,
    /// Display label or metadata-only title.
    pub label: String,
    /// File identifier when applicable.
    pub file_id: Option<FileId>,
    /// Canonical path when disclosure is allowed.
    pub path: Option<CanonicalPath>,
    /// Text range when applicable.
    pub range: Option<ProtocolTextRange>,
    /// Ranking score in basis points.
    pub score_basis_points: u16,
    /// Freshness metadata.
    pub freshness: SemanticFreshness,
    /// Result provenance.
    pub provenance: SemanticRecordProvenance,
    /// Related semantic record identifiers.
    pub related_record_ids: Vec<SemanticRecordId>,
    /// Optional proposal preview summary for mutation-producing suggestions.
    pub proposal_preview: Option<ProposalPayloadSummary>,
}

/// Semantic query response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQueryResponse {
    /// Query identifier.
    pub query_id: SemanticQueryId,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Query response status.
    pub status: SemanticQueryStatus,
    /// Results in deterministic ranking order.
    pub results: Vec<SemanticQueryResult>,
    /// Diagnostics emitted while serving the query.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Optional token for retrieving a subsequent page.
    pub next_page_token: Option<String>,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Query response schema version.
    pub schema_version: u16,
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

/// Command class supplied to capability policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityCommandClass {
    /// Read-only command.
    Read,
    /// File or workspace mutation command.
    Write,
    /// Terminal command.
    Terminal,
    /// Network-capable command.
    Network,
    /// Language-server command.
    LanguageServer,
    /// Plugin command.
    Plugin,
    /// Other command class.
    Other,
}

/// Network target supplied to capability policies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkTarget {
    /// Network scheme or protocol.
    pub scheme: String,
    /// Target host.
    pub host: String,
    /// Target port when known.
    pub port: Option<u16>,
}

/// Additional context supplied with capability requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityRequestContext {
    /// Number of bytes a write request intends to write.
    pub write_byte_count: Option<u64>,
    /// Command binary being launched or inspected.
    pub command_binary: Option<String>,
    /// Command class for policy decisions.
    pub command_class: Option<CapabilityCommandClass>,
    /// Network target for network-scoped policy decisions.
    pub network_target: Option<NetworkTarget>,
    /// Plugin namespace for plugin-scoped policy decisions.
    pub plugin_namespace: Option<CapabilityNamespace>,
    /// Language-server binary for LSP-scoped policy decisions.
    pub lsp_server_binary: Option<String>,
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
    /// Read current workspace tree snapshot.
    ReadTree(WorkspaceId),
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
    /// Open a buffer with workspace-resolved identity and text.
    OpenBufferText(EditorOpenBufferRequest),
    /// Apply transaction.
    ApplyTransaction(TextTransactionDescriptor),
    /// Apply a concrete edit batch as an editor transaction.
    ApplyEdit(EditorApplyTransactionRequest),
    /// Request a save DTO for a buffer.
    RequestSave {
        /// Buffer identifier.
        buffer_id: BufferId,
        /// Correlation id.
        correlation_id: CorrelationId,
    },
    /// Acknowledge a pending save request.
    AcknowledgeSave(EditorSaveAcknowledgement),
    /// Build a viewport projection.
    Viewport(EditorViewportRequest),
    /// Query buffer metadata.
    BufferMetadata(BufferId),
    /// Query dirty/conflict/undo state for a buffer.
    BufferState(BufferId),
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
    /// Save request emitted by editor authority.
    SaveRequested(EditorSaveRequest),
    /// Save acknowledgement was accepted for the buffer.
    SaveAcknowledged {
        /// Buffer identifier for the acknowledged save, when still open.
        buffer_id: Option<BufferId>,
    },
    /// Viewport projection.
    Viewport(ViewportProjection),
    /// Buffer metadata.
    BufferMetadata(EditorBufferMetadata),
    /// Buffer dirty/conflict/undo state.
    BufferState(EditorBufferMetadata),
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
    /// Approve proposal.
    Approve(ProposalLifecycleCommand),
    /// Reject proposal.
    Reject(ProposalLifecycleCommand),
    /// Apply proposal.
    Apply(WorkspaceProposal),
    /// Cancel proposal.
    Cancel(ProposalLifecycleCommand),
    /// Roll back an already applied or partially applied proposal.
    Rollback(ProposalLifecycleCommand),
}

/// Proposal response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalResponse {
    /// Proposal-created result.
    Created(ProposalLifecycleTransition),
    /// Proposal-validated result.
    Validated(ProposalLifecycleTransition),
    /// Proposal-previewed result.
    Previewed {
        /// Lifecycle transition metadata.
        transition: ProposalLifecycleTransition,
        /// Previewed proposal.
        proposal: Box<WorkspaceProposal>,
    },
    /// Proposal-approved result.
    Approved(ProposalLifecycleTransition),
    /// Proposal-rejected result.
    Rejected {
        /// Lifecycle transition metadata.
        transition: ProposalLifecycleTransition,
        /// Typed rejection reason.
        reason: ProposalRejectionReason,
    },
    /// Proposal-applied result.
    Applied(ProposalLifecycleTransition),
    /// Proposal-denied result.
    Denied {
        /// Lifecycle transition metadata.
        transition: ProposalLifecycleTransition,
        /// Typed denial reason.
        reason: ProposalDenialReason,
    },
    /// Proposal-failed result.
    Failed {
        /// Lifecycle transition metadata.
        transition: ProposalLifecycleTransition,
        /// Typed failure reason.
        reason: ProposalFailureReason,
    },
    /// Proposal-rolled-back result.
    RolledBack {
        /// Lifecycle transition metadata.
        transition: ProposalLifecycleTransition,
        /// Typed rollback reason.
        reason: ProposalRollbackReason,
    },
    /// Proposal-stale result.
    Stale {
        /// Lifecycle transition metadata.
        transition: ProposalLifecycleTransition,
        /// Stale context.
        stale: ProposalStaleContext,
    },
    /// Proposal-conflict result.
    Conflict {
        /// Lifecycle transition metadata.
        transition: ProposalLifecycleTransition,
        /// Conflict context.
        conflict: FileConflictState,
    },
    /// Proposal-cancelled result.
    Cancelled {
        /// Lifecycle transition metadata.
        transition: ProposalLifecycleTransition,
        /// Typed cancellation reason.
        reason: ProposalCancellationReason,
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
    /// Definition lookup.
    Definition(LspDefinitionRequest),
    /// Reference lookup.
    References(LspReferenceRequest),
    /// Rename operation.
    Rename(LspRenameRequest),
    /// Hover.
    Hover {
        /// Language server to resolve hover from.
        server_id: LanguageServerId,
        /// File id for the hover query.
        file_id: FileId,
    },
    /// Formatting.
    Formatting(LspFormattingRequest),
    /// Formatting operation that must return proposal-mediated output.
    FormattingProposal(LspFormattingProposalRequest),
    /// Symbol.
    Symbol {
        /// File id for which to request symbols.
        file_id: FileId,
    },
    /// Code action.
    CodeAction(LspCodeActionRequest),
    /// Code action operation with separated edit and command payloads.
    CodeActionProposal(LspCodeActionProposalRequest),
    /// Cancel an in-flight LSP operation.
    Cancel(LspCancellationRequest),
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
    /// Definition lookup response.
    Definition(LspDefinitionResponse),
    /// Reference lookup response.
    References(LspReferenceResponse),
    /// Rename response.
    Rename(LspRenameResponse),
    /// Hover.
    Hover(Hover),
    /// Formatting.
    Formatting(LspFormattingResponse),
    /// Formatting response with proposal-mediated output.
    FormattingProposal(LspFormattingProposalResponse),
    /// Diagnostics.
    Diagnostics(DiagnosticSet),
    /// Semantic tokens.
    SemanticTokens(SemanticTokenSet),
    /// Symbols.
    Symbols(Vec<SymbolLocation>),
    /// Actions.
    CodeActions(LspCodeActionResponse),
    /// Code-action response with separated edit and command payloads.
    CodeActionProposals(LspCodeActionProposalResponse),
    /// Cancellation acknowledgement.
    Cancelled(LspCancellationAck),
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
        /// Explicit workspace trust state for policy decisions.
        workspace_trust_state: WorkspaceTrustState,
        /// Optional target path for path-scoped capability checks.
        target_path: Option<CanonicalPath>,
        /// Optional prior decision id for continuation or replay contexts.
        decision_id: Option<CapabilityDecisionId>,
        /// Additional policy context.
        #[serde(default)]
        context: CapabilityRequestContext,
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
    /// Event envelope schema version.
    pub schema_version: u16,
    /// Stable event identifier.
    pub event_id: EventId,
    /// Optional parent event identifier for event lineage.
    pub parent_event_id: Option<EventId>,
    /// Causality chain identifier.
    pub causality_id: CausalityId,
    /// Event name.
    pub event: String,
    /// Event severity classification.
    pub severity: EventSeverity,
    /// Event retention label.
    pub retention: RetentionLabel,
    /// Event redaction hint.
    pub redaction: RedactionHint,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Workspace id.
    pub workspace_id: Option<WorkspaceId>,
    /// Sequence.
    pub sequence: EventSequence,
    /// Actor principal.
    pub principal_id: Option<PrincipalId>,
    /// Occurrence timestamp.
    pub occurred_at: TimestampMillis,
    /// Payload body.
    pub payload: serde_json::Value,
}

/// Event severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSeverity {
    /// Debug-level event.
    Debug,
    /// Informational event.
    Info,
    /// Warning-level event.
    Warning,
    /// Error-level event.
    Error,
    /// Critical-level event.
    Critical,
}

/// Event retention label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionLabel {
    /// Keep only short-lived hot-window data.
    Hot,
    /// Keep medium-term warm-window data.
    Warm,
    /// Keep long-term audit data.
    Audit,
}

/// Event payload redaction hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedactionHint {
    /// Payload is safe to keep in full.
    None,
    /// Keep metadata and remove source text or sensitive values.
    MetadataOnly,
    /// Remove payload content completely.
    Full,
}

/// Event sink request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSinkRequest {
    /// Envelope.
    pub envelope: EventEnvelope,
}

/// Persisted workspace trust record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustRecord {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Principal identifier.
    pub principal_id: PrincipalId,
    /// Trust state.
    pub trust_state: WorkspaceTrustState,
    /// Optional decision id that established the trust state.
    pub decision_id: Option<CapabilityDecisionId>,
    /// Correlation id associated with the trust decision.
    pub correlation_id: CorrelationId,
    /// Record timestamp.
    pub recorded_at: TimestampMillis,
    /// Trust record schema version.
    pub schema_version: u16,
}

/// Persisted session tab record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTab {
    /// Stable tab identifier.
    pub tab_id: String,
    /// Buffer identifier when the tab is backed by an open buffer.
    pub buffer_id: Option<BufferId>,
    /// File identifier when the tab is file-backed.
    pub file_id: Option<FileId>,
    /// Canonical path when available.
    pub path: Option<CanonicalPath>,
    /// Display title.
    pub title: String,
    /// Whether the tab is pinned.
    pub pinned: bool,
    /// Whether the tab is a preview tab.
    pub preview: bool,
    /// Whether the tab has unsaved changes.
    pub dirty: bool,
}

/// Persisted session tab group record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTabGroup {
    /// Stable group identifier.
    pub group_id: String,
    /// Tab identifiers in display order.
    pub tab_ids: Vec<String>,
    /// Active tab in this group.
    pub active_tab_id: Option<String>,
}

/// Orientation for persisted layout splits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionSplitOrientation {
    /// Horizontal split.
    Horizontal,
    /// Vertical split.
    Vertical,
}

/// Persisted layout split record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLayoutSplit {
    /// Stable split identifier.
    pub split_id: String,
    /// Split orientation.
    pub orientation: SessionSplitOrientation,
    /// First child group or split identifier.
    pub first: String,
    /// Second child group or split identifier.
    pub second: String,
    /// Ratio assigned to the first child.
    pub ratio: f32,
}

/// Persisted panel visibility and sizing state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPanelState {
    /// Whether the bottom panel is visible.
    pub bottom_visible: bool,
    /// Whether the side panel is visible.
    pub side_visible: bool,
    /// Active panel identifier.
    pub active_panel: Option<String>,
    /// Bottom panel height in pixels.
    pub bottom_height_px: Option<u32>,
    /// Side panel width in pixels.
    pub side_width_px: Option<u32>,
}

/// Persisted dirty indicator record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDirtyIndicator {
    /// Buffer identifier.
    pub buffer_id: BufferId,
    /// File identifier when available.
    pub file_id: Option<FileId>,
    /// Dirty flag.
    pub dirty: bool,
    /// Buffer version associated with the dirty flag.
    pub buffer_version: BufferVersion,
}

/// Session metadata persisted for workspace restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSessionRecord {
    /// Session identifier.
    pub session_id: String,
    /// Last workspace identifier.
    pub last_workspace: Option<WorkspaceId>,
    /// Last workspace path.
    pub last_workspace_path: Option<CanonicalPath>,
    /// Open tabs.
    pub open_tabs: Vec<SessionTab>,
    /// Active tab identifier.
    pub active_tab: Option<String>,
    /// Active buffer identifier.
    pub active_buffer: Option<BufferId>,
    /// Tab groups.
    pub tab_groups: Vec<SessionTabGroup>,
    /// Layout splits.
    pub layout_splits: Vec<SessionLayoutSplit>,
    /// Expanded explorer paths.
    pub explorer_expansion: Vec<CanonicalPath>,
    /// Panel state.
    pub panel_state: SessionPanelState,
    /// Dirty indicators.
    pub dirty_indicators: Vec<SessionDirtyIndicator>,
    /// Last saved timestamp.
    pub saved_at: TimestampMillis,
    /// Session DTO schema version.
    pub schema_version: u16,
}

/// Durable event metadata record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadataRecord {
    /// Event identifier.
    pub event_id: EventId,
    /// Optional parent event identifier.
    pub parent_event_id: Option<EventId>,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Event name.
    pub event: String,
    /// Workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Event sequence.
    pub sequence: EventSequence,
    /// Principal identifier.
    pub principal_id: Option<PrincipalId>,
    /// Retention label.
    pub retention: RetentionLabel,
    /// Redaction hint.
    pub redaction: RedactionHint,
    /// Event timestamp.
    pub occurred_at: TimestampMillis,
    /// Event metadata schema version.
    pub schema_version: u16,
}

/// Storage repository request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageRepositoryRequest {
    /// Save workspace config.
    SaveWorkspaceConfig(WorkspaceConfigSnapshot),
    /// Save file metadata.
    SaveFileMetadata(FileMetadata),
    /// Save workspace session record.
    SaveSessionRecord(WorkspaceSessionRecord),
    /// Save trust record.
    SaveTrustRecord(TrustRecord),
    /// Save proposal audit record.
    SaveProposalAuditRecord(ProposalAuditRecord),
    /// Save durable event metadata.
    SaveEventMetadata(EventMetadataRecord),
    /// Read workspace config.
    ReadWorkspaceConfig(WorkspaceId),
    /// Read file metadata.
    ReadFileMetadata(FileId),
    /// Read workspace session record.
    ReadSessionRecord {
        /// Session identifier.
        session_id: String,
    },
    /// Read trust record.
    ReadTrustRecord {
        /// Workspace identifier.
        workspace_id: WorkspaceId,
        /// Principal identifier.
        principal_id: PrincipalId,
    },
    /// Read proposal audit record.
    ReadProposalAuditRecord(ProposalId),
    /// Read durable event metadata.
    ReadEventMetadata(EventId),
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
    /// Session record.
    SessionRecord(Option<WorkspaceSessionRecord>),
    /// Trust record.
    TrustRecord(Option<TrustRecord>),
    /// Proposal audit record.
    ProposalAuditRecord(Option<ProposalAuditRecord>),
    /// Event metadata.
    EventMetadata(Option<EventMetadataRecord>),
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
                file_content_version: None,
                workspace_generation: None,
                expected_fingerprint: None,
                expected_file_length: None,
                expected_modified_at: None,
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
                file_content_version: Some(FileContentVersion(1)),
                workspace_generation: Some(WorkspaceGeneration(1)),
                expected_fingerprint: None,
                expected_file_length: None,
                expected_modified_at: None,
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
            file_content_version: FileContentVersion(1),
            workspace_generation: WorkspaceGeneration(1),
            fingerprint: None,
            file_length: None,
            modified_at: None,
        };

        let stale = VersionContext {
            file_version: FileContentVersion(2),
            buffer_version: BufferVersion(9),
            snapshot_id: SnapshotId(3),
            generation: WorkspaceGeneration(1),
            file_content_version: FileContentVersion(2),
            workspace_generation: WorkspaceGeneration(1),
            fingerprint: None,
            file_length: None,
            modified_at: None,
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
                file_content_version: None,
                workspace_generation: None,
                expected_fingerprint: None,
                expected_file_length: None,
                expected_modified_at: None,
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

        struct AllPorts<W, E, P, T, L, C, ES, S> {
            w: W,
            e: E,
            p: P,
            t: T,
            l: L,
            c: C,
            es: ES,
            s: S,
        }

        fn use_all_ports<W, E, P, T, L, C, ES, S>(ports: AllPorts<W, E, P, T, L, C, ES, S>)
        where
            W: WorkspacePort,
            E: EditorPort,
            P: ProposalPort,
            T: TerminalPort,
            L: LspPort,
            C: CapabilityBrokerPort,
            ES: EventSinkPort,
            S: StorageRepositoryPort,
        {
            let AllPorts {
                w,
                e,
                p,
                t,
                l,
                c,
                es,
                s,
            } = ports;
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
                        buffer_id: BufferId(1),
                        file_id: FileId(1),
                        snapshot_id: SnapshotId(1),
                        buffer_version: BufferVersion(1),
                        file_content_version: FileContentVersion(1),
                        workspace_generation: WorkspaceGeneration(1),
                        expected_fingerprint: None,
                        save_intent: SaveIntent::Manual,
                        conflict_policy: SaveConflictPolicy::RejectIfChanged,
                        trust_decision: TrustDecisionContext {
                            workspace_trust_state: WorkspaceTrustState::Trusted,
                            decision_id: None,
                            decided_at: None,
                        },
                        required_capability: CapabilityId("fs.write".to_string()),
                        principal: PrincipalId("x".to_string()),
                        correlation_id: CorrelationId(1),
                        diagnostics: vec![],
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
                        summary: "save".to_string(),
                        details: vec![],
                    },
                    expires_at: None,
                    created_at: TimestampMillis(1),
                })),
                p.handle(ProposalRequest::Approve(ProposalLifecycleCommand {
                    proposal_id: ProposalId(1),
                    action: ProposalLifecycleAction::Approve,
                    principal: PrincipalId("x".to_string()),
                    capability: CapabilityId("fs.write".to_string()),
                    correlation_id: CorrelationId(1),
                    causality_id: CausalityId(Uuid::now_v7()),
                    reason: None,
                    diagnostics: vec![],
                    requested_at: TimestampMillis(1),
                    schema_version: 1,
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
                    workspace_trust_state: WorkspaceTrustState::Trusted,
                    target_path: None,
                    decision_id: None,
                    context: CapabilityRequestContext::default(),
                    correlation_id: CorrelationId(1),
                }),
                es.emit(EventSinkRequest {
                    envelope: EventEnvelope {
                        schema_version: 1,
                        event_id: EventId(Uuid::nil()),
                        parent_event_id: None,
                        causality_id: CausalityId(Uuid::nil()),
                        event: "init".to_string(),
                        severity: EventSeverity::Info,
                        retention: RetentionLabel::Hot,
                        redaction: RedactionHint::None,
                        correlation_id: CorrelationId(1),
                        workspace_id: None,
                        sequence: EventSequence(1),
                        principal_id: None,
                        occurred_at: TimestampMillis(1),
                        payload: serde_json::json!({"ok": true}),
                    },
                }),
                s.handle(StorageRepositoryRequest::ReadWorkspaceConfig(WorkspaceId(
                    1,
                ))),
            );
        }

        use_all_ports(AllPorts {
            w: MockWorkspacePort,
            e: MockEditorPort,
            p: MockProposalPort,
            t: MockTerminalPort,
            l: MockLspPort,
            c: MockCapabilityBrokerPort,
            es: MockEventSinkPort,
            s: MockStorageRepositoryPort,
        });
    }
}
