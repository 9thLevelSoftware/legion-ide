//! Shared protocol types, event schemas, action schemas, and versioning for Devil IDE.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Workspace-authored semantic discovery decision for an individual path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceDiscoveryDecision {
    /// File content may be scheduled for semantic source processing.
    ContentAllowed,
    /// Only metadata may be retained or queried for this record.
    MetadataOnly,
    /// Record is explicitly excluded from semantic indexing.
    Excluded,
}

/// Workspace-authored reason a discovery record was skipped or downgraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceDiscoverySkipReason {
    /// Path matched workspace ignore rules.
    Ignored,
    /// Security or trust policy denied content use.
    PolicyDenied,
    /// Path is outside the workspace boundary.
    External,
    /// Path was deleted and should invalidate existing semantic records.
    Deleted,
    /// Path is hidden.
    Hidden,
    /// Path is generated output.
    Generated,
    /// Path is binary or non-text.
    Binary,
    /// Path belongs to vendored dependencies or build artifacts.
    Vendored,
    /// Path exceeds the workspace discovery size policy.
    Oversized,
    /// Path could not be read as workspace metadata.
    Unreadable,
    /// Privacy policy permits only metadata retention.
    PrivacyRestricted,
}

/// Workspace-authored path policy result for semantic discovery metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceDiscoveryPathPolicyResult {
    /// Path stayed inside the workspace boundary and policy permitted metadata discovery.
    WorkspaceAllowed,
    /// Path stayed inside the workspace boundary but policy denied content use.
    WorkspaceDenied,
    /// Path was outside the workspace boundary.
    External,
    /// Path could not be resolved by the workspace authority.
    Unresolved,
}

/// Workspace trust result attached to discovery metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceDiscoveryTrustResult {
    /// Workspace was trusted when discovery metadata was authored.
    Trusted,
    /// Workspace was explicitly untrusted when discovery metadata was authored.
    Untrusted,
    /// Workspace trust was unknown when discovery metadata was authored.
    Unknown,
}

/// Workspace-authored operation represented by a discovery record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceDiscoveryChangeKind {
    /// Record was added by workspace discovery.
    Added,
    /// Record metadata or policy changed.
    Changed,
    /// Record was deleted and invalidates prior semantic records.
    Deleted,
    /// Record represents a policy-only state change.
    PolicyChanged,
}

/// Workspace-authored policy decision and safety flags for semantic discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDiscoveryPolicyDecision {
    /// Content, metadata-only, or excluded decision.
    pub decision: WorkspaceDiscoveryDecision,
    /// Optional skip or downgrade reason.
    pub skip_reason: Option<WorkspaceDiscoverySkipReason>,
    /// Workspace path policy result.
    pub path_policy: WorkspaceDiscoveryPathPolicyResult,
    /// Workspace trust result.
    pub trust: WorkspaceDiscoveryTrustResult,
    /// True when the path was classified as generated.
    pub generated: bool,
    /// True when the path was classified as binary.
    pub binary: bool,
    /// True when the path was classified as vendored.
    pub vendored: bool,
    /// True when the path exceeded workspace discovery size policy.
    pub oversized: bool,
    /// True when consumers must retain/query metadata only and never source bodies.
    pub metadata_only: bool,
}

/// Workspace-authored semantic discovery record crossing into the index boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDiscoveryRecord {
    /// Discovery record schema version.
    pub schema_version: u16,
    /// Workspace identifier when available from the authoring authority.
    pub workspace_id: Option<WorkspaceId>,
    /// Workspace root identifier when available.
    pub workspace_root_id: Option<WorkspaceRootId>,
    /// Workspace generation that authored the record.
    pub workspace_generation: WorkspaceGeneration,
    /// Optional stable file identity for accepted or previously known records.
    pub identity: Option<FileIdentity>,
    /// Optional canonical path metadata.
    pub path: Option<CanonicalPath>,
    /// Bounded display path or root-relative path metadata.
    pub display_path: Option<String>,
    /// Optional safe file metadata.
    pub metadata: Option<FileMetadata>,
    /// Workspace-authored policy result and safety flags.
    pub policy: WorkspaceDiscoveryPolicyDecision,
    /// Language hint produced by workspace metadata or extension policy.
    pub language_hint: Option<LanguageId>,
    /// Privacy scope for semantic consumers.
    pub privacy_scope: SemanticPrivacyScope,
    /// Workspace disk/content fingerprint when available.
    pub content_fingerprint: Option<FileFingerprint>,
    /// Semantic content hash when content use is allowed or safely known.
    pub content_hash: Option<FileFingerprint>,
    /// Optional watcher/tree change kind represented by this record.
    pub change_kind: Option<WorkspaceDiscoveryChangeKind>,
}

/// Workspace-authored discovery snapshot for semantic-index import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDiscoverySnapshot {
    /// Discovery snapshot schema version.
    pub schema_version: u16,
    /// Workspace identifier that authored the snapshot.
    pub workspace_id: WorkspaceId,
    /// Workspace root identifier when available.
    pub workspace_root_id: Option<WorkspaceRootId>,
    /// Workspace generation that authored the snapshot.
    pub workspace_generation: WorkspaceGeneration,
    /// Snapshot capture timestamp.
    pub captured_at: TimestampMillis,
    /// Workspace-authored discovery records.
    pub records: Vec<WorkspaceDiscoveryRecord>,
    /// Metadata-only diagnostics emitted during discovery.
    pub diagnostics: Vec<ProtocolDiagnostic>,
}

/// Workspace-authored discovery delta for semantic-index import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDiscoveryDelta {
    /// Discovery delta schema version.
    pub schema_version: u16,
    /// Workspace identifier that authored the delta.
    pub workspace_id: WorkspaceId,
    /// Workspace generation that authored the delta.
    pub workspace_generation: WorkspaceGeneration,
    /// Monotonic workspace event sequence for this delta.
    pub sequence: EventSequence,
    /// Delta records in workspace-authored order.
    pub records: Vec<WorkspaceDiscoveryRecord>,
    /// Metadata-only diagnostics emitted during delta construction.
    pub diagnostics: Vec<ProtocolDiagnostic>,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Buffer identifier whose snapshot is leased.
    pub buffer_id: BufferId,
    /// Snapshot identifier guarded by the lease.
    pub snapshot_id: SnapshotId,
    /// Buffer version guarded by the lease.
    pub buffer_version: BufferVersion,
    /// Consumer category holding the lease.
    pub consumer_kind: SnapshotConsumerKind,
    /// Lease expiration time.
    pub expires_at: TimestampMillis,
    /// Number of chunks pinned by the lease.
    pub chunk_count: u32,
    /// Lease descriptor schema version.
    pub schema_version: u16,
}

/// Bounded chunk payload read through a snapshot lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotLeaseChunk {
    /// Descriptor for the lease that authorized this bounded read.
    pub lease: SnapshotLeaseDescriptor,
    /// Descriptor for the chunk that was read.
    pub chunk: SnapshotChunkDescriptor,
    /// Bounded chunk text. Consumers must not persist or treat this as whole-buffer ownership.
    pub text: String,
    /// Chunk read DTO schema version.
    pub schema_version: u16,
}

/// Collaboration session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CollaborationSessionId(pub u128);

/// Collaboration participant identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CollaborationParticipantId(pub u128);

/// Collaboration operation identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CollaborationOperationId(pub u128);

/// Collaboration document epoch used to reject stale session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CollaborationDocumentEpoch(pub u64);

/// Participant role inside a collaboration session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CollaborationParticipantRole {
    /// Session owner with admission authority.
    Owner,
    /// Participant allowed to publish document operations.
    Editor,
    /// Participant allowed to review and approve shared proposals.
    Reviewer,
    /// Read-only participant allowed to publish presence only.
    Observer,
    /// Bot or agent activity projected into the session.
    Agent,
}

/// Collaboration session lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CollaborationSessionState {
    /// Session has been created but is not active.
    Created,
    /// Participant admission is in progress.
    Joining,
    /// Session accepts validated operations.
    Active,
    /// Session is active with degraded transport or replay state.
    Degraded,
    /// Session is attempting to resume from a disconnected state.
    Reconnecting,
    /// Session is blocked on explicit conflict or resync handling.
    Conflict,
    /// Session is closing.
    Closing,
    /// Session is closed.
    Closed,
    /// Session or participant admission was denied.
    Denied,
}

/// Collaboration permission class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CollaborationPermission {
    /// Create a collaboration session.
    CreateSession,
    /// Join a collaboration session.
    JoinSession,
    /// Invite another participant.
    InviteParticipant,
    /// Publish a document operation.
    PublishOperation,
    /// Publish cursor, selection, or activity presence.
    PublishPresence,
    /// Approve a shared proposal.
    ApproveSharedProposal,
    /// Read replay metadata.
    ReplayMetadata,
    /// Export metadata-only audit records.
    ExportAudit,
}

/// Collaboration document binding for an editor-owned buffer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationDocumentBinding {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Buffer identifier.
    pub buffer_id: BufferId,
    /// Base snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Base buffer version.
    pub buffer_version: BufferVersion,
    /// Document epoch.
    pub document_epoch: CollaborationDocumentEpoch,
    /// Optional metadata-only content fingerprint.
    pub content_fingerprint: Option<FileFingerprint>,
    /// Binding schema version.
    pub schema_version: u16,
}

/// Collaboration session descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationSessionDescriptor {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Session lifecycle state.
    pub state: CollaborationSessionState,
    /// Creator principal.
    pub created_by: PrincipalId,
    /// Creation timestamp.
    pub created_at: TimestampMillis,
    /// Bound documents.
    pub document_bindings: Vec<CollaborationDocumentBinding>,
    /// Redaction hints for displayed session metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Descriptor schema version.
    pub schema_version: u16,
}

/// Collaboration participant descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationParticipant {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Participant identifier.
    pub participant_id: CollaborationParticipantId,
    /// Principal identifier.
    pub principal_id: PrincipalId,
    /// Participant role.
    pub role: CollaborationParticipantRole,
    /// Granted collaboration permissions.
    pub permissions: Vec<CollaborationPermission>,
    /// Redacted label for UI projections and audit.
    pub display_label: String,
    /// Participant schema version.
    pub schema_version: u16,
}

/// Version vector entry for one collaboration participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CollaborationVersionVectorEntry {
    /// Participant identifier.
    pub participant_id: CollaborationParticipantId,
    /// Last observed operation sequence from this participant.
    pub sequence: u64,
}

/// Collaboration version vector in deterministic entry order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationVersionVector {
    /// Vector entries.
    pub entries: Vec<CollaborationVersionVectorEntry>,
}

/// Collaboration operation kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationDocumentOperationKind {
    /// Insert bounded text at a range start.
    Insert {
        /// Bounded inserted text.
        text: String,
    },
    /// Delete a range.
    Delete,
    /// Replace a range with bounded text.
    Replace {
        /// Bounded replacement text.
        text: String,
    },
    /// Move cursor without changing text.
    CursorMove,
    /// Update selection without changing text.
    SelectionUpdate,
    /// Explicit undo compensation operation.
    UndoCompensation,
    /// Acknowledgement with no text effect.
    NoopAcknowledgement,
    /// Request resynchronization after a causal gap or conflict.
    ResyncRequest,
}

/// Collaboration operation preconditions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationOperationPreconditions {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Buffer identifier.
    pub buffer_id: BufferId,
    /// Snapshot identifier used as the operation base.
    pub snapshot_id: SnapshotId,
    /// Buffer version used as the operation base.
    pub buffer_version: BufferVersion,
    /// Document epoch used as the operation base.
    pub document_epoch: CollaborationDocumentEpoch,
    /// Base version vector.
    pub base_vector: CollaborationVersionVector,
    /// Author principal.
    pub author_principal: PrincipalId,
    /// Capability decision for the operation.
    pub capability_decision: CapabilityDecision,
    /// Non-zero correlation id.
    pub correlation_id: CorrelationId,
    /// Non-nil causality id.
    pub causality_id: CausalityId,
    /// Redaction hints for operation metadata.
    pub redaction_hints: Vec<RedactionHint>,
}

impl CollaborationOperationPreconditions {
    /// Returns true when required identity and capability metadata is non-empty.
    pub fn has_valid_identity_metadata(&self) -> bool {
        self.correlation_id.0 != 0
            && !self.causality_id.0.is_nil()
            && self.capability_decision.decision_id.0 != 0
            && self.capability_decision.granted
            && !self.author_principal.0.is_empty()
    }
}

/// Collaboration document operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationDocumentOperation {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Operation identifier.
    pub operation_id: CollaborationOperationId,
    /// Author participant identifier.
    pub author_participant_id: CollaborationParticipantId,
    /// Participant-local sequence number.
    pub participant_sequence: u64,
    /// Operation kind.
    pub kind: CollaborationDocumentOperationKind,
    /// Affected text range, if any.
    pub range: Option<TextRange>,
    /// Operation preconditions.
    pub preconditions: CollaborationOperationPreconditions,
    /// Undo group metadata, if any.
    pub undo_group: Option<UndoGroup>,
    /// Operation timestamp.
    pub occurred_at: TimestampMillis,
    /// Operation schema version.
    pub schema_version: u16,
}

/// Collaboration acknowledgement status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CollaborationAcknowledgementStatus {
    /// Operation was accepted.
    Accepted,
    /// Operation was a duplicate and was ignored.
    Duplicate,
    /// Operation was rejected as stale.
    Stale,
    /// Operation exposed a causal gap.
    GapDetected,
    /// Operation requires resynchronization.
    ResyncRequired,
    /// Operation was denied by capability or trust policy.
    Denied,
}

/// Collaboration acknowledgement DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationAcknowledgement {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Operation identifier.
    pub operation_id: CollaborationOperationId,
    /// Acknowledging participant.
    pub participant_id: CollaborationParticipantId,
    /// Acknowledgement status.
    pub status: CollaborationAcknowledgementStatus,
    /// Observed version vector after processing.
    pub observed_vector: CollaborationVersionVector,
    /// Reason code for denied, stale, or gap outcomes.
    pub reason_code: Option<String>,
    /// Acknowledgement schema version.
    pub schema_version: u16,
}

/// Collaboration causal gap descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationCausalGap {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Participant whose sequence has a gap.
    pub participant_id: CollaborationParticipantId,
    /// Expected next sequence.
    pub expected_sequence: u64,
    /// Actual observed sequence.
    pub observed_sequence: u64,
    /// Gap reason code.
    pub reason_code: String,
}

/// Collaboration presence update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationPresenceProjection {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Participant identifier.
    pub participant_id: CollaborationParticipantId,
    /// Optional cursor position.
    pub cursor: Option<TextCoordinate>,
    /// Visible selections.
    pub selections: Vec<ProtocolTextRange>,
    /// Redacted activity label.
    pub activity_label: Option<String>,
    /// Whether the participant is reconnecting.
    pub reconnecting: bool,
    /// Projection schema version.
    pub schema_version: u16,
}

/// Metadata-only collaboration GUI projection for session and review surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationGuiProjection {
    /// Whether app policy currently enables local collaboration runtime sessions.
    pub runtime_enabled: bool,
    /// Whether app policy currently enables metadata-only presence publication.
    pub presence_enabled: bool,
    /// Session summary rows.
    pub session_rows: Vec<CollaborationSessionGuiRow>,
    /// Shared proposal review summary rows.
    pub shared_proposal_rows: Vec<CollaborationSharedProposalGuiRow>,
    /// Total sessions with reconnecting participants.
    pub reconnecting_session_count: usize,
    /// Total sessions with conflicts, causal gaps, or resync-required acknowledgements.
    pub conflict_session_count: usize,
    /// Total sessions not currently active for collaboration work.
    pub offline_session_count: usize,
    /// Metadata-only status label.
    pub status_label: String,
    /// Redaction hints for displayed collaboration metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

impl CollaborationGuiProjection {
    /// Empty projection for the default disabled policy state.
    pub fn disabled() -> Self {
        Self {
            runtime_enabled: false,
            presence_enabled: false,
            session_rows: Vec::new(),
            shared_proposal_rows: Vec::new(),
            reconnecting_session_count: 0,
            conflict_session_count: 0,
            offline_session_count: 0,
            status_label: "collaboration runtime disabled by policy".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }
}

/// Metadata-only row for one collaboration session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationSessionGuiRow {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Session lifecycle state.
    pub state: CollaborationSessionState,
    /// Number of projected participants known to the GUI layer.
    pub participant_count: usize,
    /// Number of projected participant presence records.
    pub presence_count: usize,
    /// Number of reconnecting participants.
    pub reconnecting_participant_count: usize,
    /// Accepted operation count.
    pub operation_count: usize,
    /// Acknowledgement count.
    pub acknowledgement_count: usize,
    /// Causal gap count.
    pub causal_gap_count: usize,
    /// Conflict/resync count derived from acknowledgements and causal gaps.
    pub conflict_count: usize,
    /// Whether the session is closed, denied, or otherwise offline for GUI work.
    pub offline: bool,
    /// Metadata-only session status label.
    pub status_label: String,
}

/// Metadata-only row for a shared collaboration proposal review gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationSharedProposalGuiRow {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Proposal identifier reviewed through app-owned proposal lifecycle actions.
    pub proposal_id: ProposalId,
    /// Required approver count.
    pub required_approver_count: usize,
    /// Authorized approver count.
    pub authorized_approver_count: usize,
    /// Recorded approval count.
    pub approval_count: usize,
    /// Recorded denial count.
    pub denial_count: usize,
    /// Pending required approval count.
    pub pending_count: usize,
    /// Linked collaboration operation count.
    pub applied_operation_count: usize,
    /// Whether the approval gate is stale or superseded.
    pub stale: bool,
    /// Metadata-only review status label.
    pub status_label: String,
}

/// Shared proposal approval disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CollaborationSharedProposalDisposition {
    /// Participant approval is pending.
    Pending,
    /// Participant approved the proposal.
    Approved,
    /// Participant denied the proposal.
    Denied,
    /// Participant approval was superseded by a newer proposal state.
    Superseded,
}

/// Shared proposal approval record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationSharedProposalApproval {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Proposal identifier.
    pub proposal_id: ProposalId,
    /// Participant identifier.
    pub participant_id: CollaborationParticipantId,
    /// Approval disposition.
    pub disposition: CollaborationSharedProposalDisposition,
    /// Capability decision for approval or denial.
    pub capability_decision: CapabilityDecision,
    /// Applied operation identifiers linked to this proposal.
    pub applied_operation_ids: Vec<CollaborationOperationId>,
    /// Denial reason when disposition is denied.
    pub denial_reason: Option<String>,
    /// Approval schema version.
    pub schema_version: u16,
}

/// Metadata-only collaboration audit record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationAuditRecord {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Operation identifier when the audit record is operation-scoped.
    pub operation_id: Option<CollaborationOperationId>,
    /// Proposal identifier when the audit record is proposal-scoped.
    pub proposal_id: Option<ProposalId>,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Retention label.
    pub retention_label: RetentionLabel,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Metadata summary without raw source text.
    pub metadata_summary: String,
    /// Audit schema version.
    pub schema_version: u16,
}

/// Metadata-only collaboration replay manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationReplayManifest {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Deterministic operation identifiers in replay order.
    pub operation_ids: Vec<CollaborationOperationId>,
    /// Participant count.
    pub participant_count: u32,
    /// Acknowledgement count.
    pub acknowledgement_count: u32,
    /// Causal gap count.
    pub causal_gap_count: u32,
    /// Final document byte count without source text.
    pub final_byte_count: u64,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Retention label.
    pub retention_label: RetentionLabel,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Replay manifest schema version.
    pub schema_version: u16,
}

/// Collaboration transport envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollaborationTransportEnvelope {
    /// Session identifier.
    pub session_id: CollaborationSessionId,
    /// Sender participant identifier.
    pub sender_participant_id: CollaborationParticipantId,
    /// Envelope correlation identifier.
    pub correlation_id: CorrelationId,
    /// Envelope causality identifier.
    pub causality_id: CausalityId,
    /// Envelope payload.
    pub payload: CollaborationTransportPayload,
    /// Envelope schema version.
    pub schema_version: u16,
}

/// Collaboration transport payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationTransportPayload {
    /// Session descriptor payload.
    Session(CollaborationSessionDescriptor),
    /// Participant descriptor payload.
    Participant(CollaborationParticipant),
    /// Document operation payload.
    Operation(Box<CollaborationDocumentOperation>),
    /// Acknowledgement payload.
    Acknowledgement(CollaborationAcknowledgement),
    /// Causal gap payload.
    CausalGap(CollaborationCausalGap),
    /// Presence projection payload.
    Presence(CollaborationPresenceProjection),
    /// Shared proposal approval payload.
    SharedProposalApproval(CollaborationSharedProposalApproval),
    /// Metadata-only audit payload.
    Audit(CollaborationAuditRecord),
}

/// Remote workspace authority identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoteAuthorityId(pub u128);

/// Remote edge agent identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoteAgentId(pub u128);

/// Remote workspace session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoteWorkspaceSessionId(pub u128);

/// Remote operation identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoteOperationId(pub u128);

/// Remote operation-log checkpoint identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RemoteOperationLogCheckpointId(pub u128);

/// Remote workspace session lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteWorkspaceLifecycleState {
    /// Session has been requested but not connected.
    Created,
    /// Session is connecting transport and identity.
    Connecting,
    /// Session is authenticating the principal and authority.
    Authenticating,
    /// Session is authorizing requested capabilities.
    Authorizing,
    /// Session is active and accepts policy-validated requests.
    Active,
    /// Session is active with degraded transport or remote health.
    Degraded,
    /// Session is reconnecting after transport loss.
    Reconnecting,
    /// Session is offline and can only resume from accepted manifests.
    Offline,
    /// Session is closing.
    Closing,
    /// Session is closed.
    Closed,
    /// Session was denied by trust, identity, capability, or policy checks.
    Denied,
}

/// Remote capability class requested from policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteCapabilityKind {
    /// Connect to a remote authority.
    Connect,
    /// Read remote filesystem metadata or content through approved ports.
    FilesystemRead,
    /// Write remote filesystem state through proposal-mediated ports.
    FilesystemWrite,
    /// Launch a remote process.
    ProcessLaunch,
    /// Send PTY input.
    PtyInput,
    /// Access a remote terminal session.
    TerminalAccess,
    /// Launch or attach to a remote language server.
    LspLaunch,
    /// Execute a remote semantic query.
    SemanticQuery,
    /// Read remote cache metadata.
    CacheAccess,
    /// Use remote egress.
    Egress,
    /// Export metadata-only audit records.
    AuditExport,
    /// Resume from an offline manifest.
    OfflineResume,
}

/// Remote workspace authority descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAuthorityDescriptor {
    /// Remote authority identifier.
    pub authority_id: RemoteAuthorityId,
    /// Redacted authority label or stable hash.
    pub authority_label: String,
    /// Workspace identifier projected locally for this authority.
    pub workspace_id: WorkspaceId,
    /// Workspace trust state observed before activation.
    pub trust_state: WorkspaceTrustState,
    /// Principal requesting the authority.
    pub principal_id: PrincipalId,
    /// Redaction hints for authority metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Descriptor schema version.
    pub schema_version: u16,
}

/// Remote edge agent descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAgentDescriptor {
    /// Remote agent identifier.
    pub agent_id: RemoteAgentId,
    /// Owning remote authority.
    pub authority_id: RemoteAuthorityId,
    /// Agent version or build label.
    pub agent_version: String,
    /// Whether runtime behavior is explicitly enabled by app-owned composition.
    pub runtime_enabled: bool,
    /// Descriptor schema version.
    pub schema_version: u16,
}

/// Remote workspace session descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteWorkspaceSessionDescriptor {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Remote authority descriptor.
    pub authority: RemoteAuthorityDescriptor,
    /// Remote agent descriptor.
    pub agent: RemoteAgentDescriptor,
    /// Lifecycle state.
    pub state: RemoteWorkspaceLifecycleState,
    /// Granted remote capability kinds.
    pub granted_capabilities: Vec<RemoteCapabilityKind>,
    /// Creation timestamp.
    pub created_at: TimestampMillis,
    /// Last heartbeat timestamp.
    pub last_heartbeat_at: Option<TimestampMillis>,
    /// Descriptor schema version.
    pub schema_version: u16,
}

impl RemoteWorkspaceSessionDescriptor {
    /// Returns true when the session is explicitly enabled and trusted.
    pub fn activation_is_policy_ready(&self) -> bool {
        self.agent.runtime_enabled
            && self.authority.trust_state == WorkspaceTrustState::Trusted
            && !self.authority.principal_id.0.is_empty()
            && matches!(self.state, RemoteWorkspaceLifecycleState::Active)
    }
}

/// Metadata-only remote workspace GUI projection for session/status/review surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteGuiProjection {
    /// Whether app policy currently enables remote runtime sessions.
    pub runtime_enabled: bool,
    /// Remote workspace session summary rows.
    pub session_rows: Vec<RemoteWorkspaceSessionGuiRow>,
    /// Remote proposal review summary rows.
    pub proposal_review_rows: Vec<RemoteProposalReviewGuiRow>,
    /// Total connected or degraded sessions.
    pub connected_session_count: usize,
    /// Total sessions currently reconnecting.
    pub reconnecting_session_count: usize,
    /// Total sessions closed, denied, or offline.
    pub offline_session_count: usize,
    /// Metadata-only status label.
    pub status_label: String,
    /// Redaction hints for displayed remote metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

impl RemoteGuiProjection {
    /// Empty projection for the default disabled policy state.
    pub fn disabled() -> Self {
        Self {
            runtime_enabled: false,
            session_rows: Vec::new(),
            proposal_review_rows: Vec::new(),
            connected_session_count: 0,
            reconnecting_session_count: 0,
            offline_session_count: 0,
            status_label: "remote workspace runtime disabled by policy".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }
}

/// Metadata-only row for one remote workspace session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteWorkspaceSessionGuiRow {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Redacted authority label or stable hash.
    pub authority_label: String,
    /// Remote agent version or build label.
    pub agent_version: String,
    /// Session lifecycle state.
    pub state: RemoteWorkspaceLifecycleState,
    /// Filesystem descriptor status derived from granted capabilities.
    pub filesystem_descriptor_status: String,
    /// Terminal descriptor status derived from granted capabilities.
    pub terminal_descriptor_status: String,
    /// LSP descriptor status derived from granted capabilities.
    pub lsp_descriptor_status: String,
    /// Whether offline resume is projected as supported.
    pub reconnect_supported: bool,
    /// Whether the session is currently reconnecting.
    pub reconnecting: bool,
    /// Whether the session is closed, denied, or offline for GUI work.
    pub offline: bool,
    /// Number of proposal-mediated remote review rows for this session.
    pub proposal_review_count: usize,
    /// Metadata-only session status label.
    pub status_label: String,
}

/// Metadata-only row for a remote workspace proposal review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteProposalReviewGuiRow {
    /// Session identifier associated with the remote authority.
    pub session_id: RemoteWorkspaceSessionId,
    /// Proposal identifier reviewed through app-owned proposal lifecycle actions.
    pub proposal_id: ProposalId,
    /// Redacted authority label or stable hash.
    pub remote_authority_label: String,
    /// Proposal payload kind summarized for display.
    pub payload_kind: ProposalPayloadKind,
    /// Proposal lifecycle state summarized for display.
    pub lifecycle_state: ProposalLifecycleState,
    /// Metadata-only review status label.
    pub status_label: String,
    /// True when remote mutation must pass through proposal lifecycle authority.
    pub proposal_mediated: bool,
}

/// Remote transport envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportEnvelope {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Remote operation identifier.
    pub operation_id: RemoteOperationId,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Sender principal.
    pub principal_id: PrincipalId,
    /// Envelope payload.
    pub payload: RemoteTransportPayload,
    /// Redaction hints for envelope metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Envelope schema version.
    pub schema_version: u16,
}

impl RemoteTransportEnvelope {
    /// Returns true when required audit identifiers are non-zero and non-nil.
    pub fn has_valid_event_identity(&self) -> bool {
        self.correlation_id.0 != 0
            && !self.causality_id.0.is_nil()
            && self.event_sequence.0 != 0
            && !self.principal_id.0.is_empty()
    }
}

/// Remote transport payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteTransportPayload {
    /// Session descriptor payload.
    Session(RemoteWorkspaceSessionDescriptor),
    /// Remote filesystem snapshot payload.
    FilesystemSnapshot(RemoteFilesystemSnapshot),
    /// Remote filesystem operation payload.
    FilesystemOperation(RemoteFilesystemOperation),
    /// Remote process descriptor payload.
    Process(RemoteProcessDescriptor),
    /// Remote PTY descriptor payload.
    Pty(RemotePtyDescriptor),
    /// Remote LSP descriptor payload.
    Lsp(RemoteLspDescriptor),
    /// Remote semantic query descriptor payload.
    SemanticQuery(RemoteSemanticQueryDescriptor),
    /// Remote operation checkpoint payload.
    OperationLogCheckpoint(RemoteOperationLogCheckpoint),
    /// Offline resume manifest payload.
    OfflineResume(RemoteOfflineResumeManifest),
    /// Metadata-only audit record payload.
    Audit(RemoteAuditRecord),
}

/// Remote filesystem operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteFilesystemOperationKind {
    /// Read file metadata or content through an approved port.
    Read,
    /// List a directory.
    List,
    /// Stat a path.
    Stat,
    /// Write file content through a proposal-mediated path.
    Write,
    /// Create a file through a proposal-mediated path.
    Create,
    /// Delete a file through a proposal-mediated path.
    Delete,
    /// Rename a file through a proposal-mediated path.
    Rename,
}

/// Remote filesystem snapshot metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFilesystemSnapshot {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Workspace generation.
    pub workspace_generation: WorkspaceGeneration,
    /// Snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// File identifier when snapshot is file-scoped.
    pub file_id: Option<FileId>,
    /// File content version when known.
    pub file_content_version: Option<FileContentVersion>,
    /// Metadata fingerprint.
    pub fingerprint: Option<FileFingerprint>,
    /// Byte length without source payload.
    pub byte_len: Option<u64>,
    /// Redaction hints for snapshot metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Snapshot schema version.
    pub schema_version: u16,
}

/// Remote write and filesystem-operation preconditions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteWritePreconditions {
    /// Required capability decision.
    pub capability_decision: CapabilityDecision,
    /// Principal requesting the operation.
    pub principal_id: PrincipalId,
    /// Expected disk or remote fingerprint.
    pub expected_fingerprint: Option<FileFingerprint>,
    /// Expected file content version.
    pub file_content_version: FileContentVersion,
    /// Expected workspace generation.
    pub workspace_generation: WorkspaceGeneration,
    /// Expected buffer version when operation originates from a buffer.
    pub buffer_version: Option<BufferVersion>,
    /// Expected snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
}

impl RemoteWritePreconditions {
    /// Returns true when mandatory proposal-style write preconditions are present.
    pub fn has_required_write_guards(&self) -> bool {
        self.capability_decision.decision_id.0 != 0
            && self.capability_decision.granted
            && !self.principal_id.0.is_empty()
            && self.file_content_version.0 != 0
            && self.workspace_generation.0 != 0
            && self.snapshot_id.0 != 0
            && self.correlation_id.0 != 0
            && !self.causality_id.0.is_nil()
    }
}

/// Remote filesystem operation metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFilesystemOperation {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Operation identifier.
    pub operation_id: RemoteOperationId,
    /// Operation kind.
    pub kind: RemoteFilesystemOperationKind,
    /// Target path metadata.
    pub path: CanonicalPath,
    /// Destination path for rename operations.
    pub destination: Option<CanonicalPath>,
    /// Preconditions for mutating operations.
    pub write_preconditions: Option<RemoteWritePreconditions>,
    /// Proposal linked to the operation when mutation is requested.
    pub proposal_id: Option<ProposalId>,
    /// Operation schema version.
    pub schema_version: u16,
}

/// Remote process descriptor without raw output bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteProcessDescriptor {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Operation identifier.
    pub operation_id: RemoteOperationId,
    /// Redacted command label or hash.
    pub command_label: String,
    /// Working directory metadata.
    pub cwd: Option<CanonicalPath>,
    /// Capability decision for process launch.
    pub capability_decision: CapabilityDecision,
    /// Cancellation token.
    pub cancellation_token_id: CancellationTokenId,
    /// Output byte limit.
    pub output_byte_limit: u64,
    /// Descriptor schema version.
    pub schema_version: u16,
}

/// Remote PTY descriptor without raw transcript bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemotePtyDescriptor {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Terminal session identifier.
    pub terminal_session_id: TerminalSessionId,
    /// Terminal size in columns.
    pub columns: u16,
    /// Terminal size in rows.
    pub rows: u16,
    /// Bounded transcript byte limit.
    pub transcript_byte_limit: u64,
    /// Capability decision for terminal access.
    pub capability_decision: CapabilityDecision,
    /// Descriptor schema version.
    pub schema_version: u16,
}

/// Remote LSP descriptor without raw source payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteLspDescriptor {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Language server identifier.
    pub language_server_id: LanguageServerId,
    /// Request identifier.
    pub request_id: LspRequestId,
    /// Language identifier.
    pub language_id: LanguageId,
    /// Capability decision for LSP launch or request execution.
    pub capability_decision: CapabilityDecision,
    /// Cancellation token.
    pub cancellation_token_id: CancellationTokenId,
    /// Descriptor schema version.
    pub schema_version: u16,
}

/// Remote semantic-query descriptor without vector or raw-source activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteSemanticQueryDescriptor {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Semantic query identifier.
    pub query_id: SemanticQueryId,
    /// Query purpose label.
    pub purpose: String,
    /// Maximum result count.
    pub max_results: u32,
    /// Capability decision for semantic query execution.
    pub capability_decision: CapabilityDecision,
    /// Redaction hints for query metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Descriptor schema version.
    pub schema_version: u16,
}

/// Remote network health state for latency and reconnect projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteNetworkHealthState {
    /// Remote transport is healthy.
    Healthy,
    /// Transport latency is elevated.
    Latent,
    /// Transport is degraded by loss, duplication, or reordering.
    Degraded,
    /// Transport is disconnected.
    Disconnected,
    /// Remote state is offline.
    Offline,
}

/// Remote operation-log checkpoint metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteOperationLogCheckpoint {
    /// Checkpoint identifier.
    pub checkpoint_id: RemoteOperationLogCheckpointId,
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Last operation included in the checkpoint.
    pub last_operation_id: RemoteOperationId,
    /// Collaboration-compatible version vector entries.
    pub version_vector: CollaborationVersionVector,
    /// Network health at checkpoint time.
    pub network_health: RemoteNetworkHealthState,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Checkpoint schema version.
    pub schema_version: u16,
}

/// Offline resume manifest for remote sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteOfflineResumeManifest {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Checkpoints available for resume.
    pub checkpoints: Vec<RemoteOperationLogCheckpointId>,
    /// Last known workspace generation.
    pub workspace_generation: WorkspaceGeneration,
    /// Last known filesystem snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Manifest schema version.
    pub schema_version: u16,
}

/// Metadata-only remote audit record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAuditRecord {
    /// Session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Operation identifier when scoped to an operation.
    pub operation_id: Option<RemoteOperationId>,
    /// Proposal identifier when scoped to a proposal-mediated mutation.
    pub proposal_id: Option<ProposalId>,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Retention label.
    pub retention_label: RetentionLabel,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Metadata summary without raw source, transcript, process output, or secrets.
    pub metadata_summary: String,
    /// Audit schema version.
    pub schema_version: u16,
}

impl RemoteAuditRecord {
    /// Returns true when the record is metadata-only and has valid event identifiers.
    pub fn is_metadata_only_valid(&self) -> bool {
        self.event_sequence.0 != 0
            && self.correlation_id.0 != 0
            && !self.causality_id.0.is_nil()
            && self.redaction_hints.contains(&RedactionHint::MetadataOnly)
            && !self.redaction_hints.contains(&RedactionHint::None)
    }
}

// -----------------------------------------------------------------------------
// Phase 8 future-surface contracts
// -----------------------------------------------------------------------------

/// Production remote transport endpoint metadata without raw socket ownership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportEndpointDescriptor {
    /// Stable endpoint identifier or redacted endpoint label.
    pub endpoint_id: String,
    /// Endpoint scheme, such as `https`.
    pub scheme: String,
    /// Redacted host label used for policy matching.
    pub host: String,
    /// Optional network port.
    pub port: Option<u16>,
    /// Whether this endpoint is loopback-only.
    pub loopback_only: bool,
    /// Endpoint descriptor schema version.
    pub schema_version: u16,
}

/// Remote transport peer identity bound to authority, agent, and principal metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportPeerIdentity {
    /// Remote authority identifier.
    pub authority_id: RemoteAuthorityId,
    /// Remote agent identifier.
    pub agent_id: RemoteAgentId,
    /// Principal bound to the transport handshake.
    pub principal_id: PrincipalId,
    /// Redacted certificate or key reference.
    pub credential_reference: String,
    /// Peer identity schema version.
    pub schema_version: u16,
}

/// Metadata-only credential reference for remote transport TLS/mTLS material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportCredentialReference {
    /// Stable credential reference identifier.
    pub reference_id: String,
    /// Credential kind label, such as `root-store`, `pin`, or `client-cert`.
    pub kind: String,
    /// Digest of the referenced credential material, never the credential bytes.
    pub digest: FileFingerprint,
    /// Credential reference schema version.
    pub schema_version: u16,
}

/// Mutual TLS mode required by a remote transport carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteTransportMutualTlsMode {
    /// Client authentication is disabled.
    Disabled,
    /// Client authentication may be used when configured.
    Optional,
    /// Client authentication is mandatory before production carrier activation.
    Required,
}

/// TLS policy metadata for a production remote transport connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportTlsPolicy {
    /// Whether TLS is required for this carrier.
    pub require_tls: bool,
    /// Server identity expected by policy.
    pub server_identity: String,
    /// Optional root store reference.
    pub root_store_reference: Option<RemoteTransportCredentialReference>,
    /// Optional certificate pin reference.
    pub certificate_pin_reference: Option<RemoteTransportCredentialReference>,
    /// Mutual TLS policy.
    pub mtls_mode: RemoteTransportMutualTlsMode,
    /// Client credential reference required for mTLS.
    pub client_credential_reference: Option<RemoteTransportCredentialReference>,
    /// Accepted ALPN protocol identifiers.
    pub alpn_protocols: Vec<String>,
    /// Minimum accepted transport schema version.
    pub min_schema_version: u16,
    /// Maximum accepted transport schema version.
    pub max_schema_version: u16,
    /// TLS policy schema version.
    pub schema_version: u16,
}

/// Endpoint policy metadata for production remote transport activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportEndpointPolicy {
    /// Endpoint descriptor under policy.
    pub endpoint: RemoteTransportEndpointDescriptor,
    /// Allowed endpoint schemes; production policy must include only TLS schemes.
    pub allowed_schemes: Vec<String>,
    /// Whether redirect/downgrade behavior is allowed.
    pub redirects_allowed: bool,
    /// Endpoint policy schema version.
    pub schema_version: u16,
}

/// Remote transport connection attempt metadata before network side effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportConnectionAttempt {
    /// Endpoint policy applied to the attempt.
    pub endpoint_policy: RemoteTransportEndpointPolicy,
    /// TLS policy applied to the attempt.
    pub tls_policy: RemoteTransportTlsPolicy,
    /// Selected ALPN protocol after negotiation.
    pub selected_alpn: String,
    /// Selected transport schema version after negotiation.
    pub selected_schema_version: u16,
    /// Connection timeout in milliseconds.
    pub timeout_ms: u64,
    /// Whether cancellation was requested before activation.
    pub cancellation_requested: bool,
    /// Capability decision for `remote.transport.connect`.
    pub capability_decision: CapabilityDecision,
    /// Audit event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Metadata-only attempt summary.
    pub metadata_summary: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Attempt schema version.
    pub schema_version: u16,
}

/// Metadata-only remote transport carrier diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportCarrierDiagnostic {
    /// Workspace session identifier when known.
    pub session_id: Option<RemoteWorkspaceSessionId>,
    /// Carrier lifecycle state.
    pub state: RemoteTransportLifecycleState,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Metadata-only diagnostic summary.
    pub metadata_summary: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Diagnostic schema version.
    pub schema_version: u16,
}

/// Schema negotiation mode for Phase 8 transport handshakes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteTransportSchemaCompatibility {
    /// Exact schema match is required.
    Exact,
    /// Backward-compatible schema range was negotiated.
    BackwardCompatible,
    /// Schema negotiation failed and transport must be denied.
    Incompatible,
}

/// Production remote transport handshake metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportHandshake {
    /// Workspace session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Endpoint descriptor.
    pub endpoint: RemoteTransportEndpointDescriptor,
    /// Peer identity.
    pub peer_identity: RemoteTransportPeerIdentity,
    /// Workspace trust state observed before transport activation.
    pub trust_state: WorkspaceTrustState,
    /// Schema compatibility decision.
    pub schema_compatibility: RemoteTransportSchemaCompatibility,
    /// Required remote capability decision.
    pub capability_decision: CapabilityDecision,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Handshake schema version.
    pub schema_version: u16,
}

/// Remote transport frame metadata; raw frame payloads are intentionally excluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportFrameMetadata {
    /// Workspace session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Remote operation identifier carried by the frame.
    pub operation_id: RemoteOperationId,
    /// Frame sequence number.
    pub frame_sequence: EventSequence,
    /// Declared envelope byte length.
    pub envelope_byte_len: u64,
    /// Maximum accepted frame size.
    pub max_frame_bytes: u64,
    /// Whether the frame carried compressed typed-envelope bytes.
    pub compressed: bool,
    /// Frame metadata schema version.
    pub schema_version: u16,
}

/// Opaque remote transport resume token metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportResumeToken {
    /// Workspace session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Redacted token digest, never the raw bearer token.
    pub token_digest: String,
    /// Last accepted checkpoint.
    pub checkpoint_id: RemoteOperationLogCheckpointId,
    /// Token expiry timestamp.
    pub expires_at: TimestampMillis,
    /// Token schema version.
    pub schema_version: u16,
}

/// Transport lifecycle state for the production Phase 8 transport core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteTransportLifecycleState {
    /// Transport was created but no handshake has started.
    Created,
    /// Handshake is in progress.
    Handshaking,
    /// Transport accepts typed envelope frames.
    Active,
    /// Transport is applying backpressure until frames are acknowledged.
    Backpressured,
    /// Transport is reconnecting.
    Reconnecting,
    /// Transport is validating resume metadata.
    Resuming,
    /// Transport is draining in-flight frames before close.
    Draining,
    /// Transport is closed.
    Closed,
    /// Transport activation was denied.
    Denied,
}

/// Metadata-only flow-control window for remote transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportFlowControlWindow {
    /// Workspace session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Maximum in-flight frames allowed.
    pub max_inflight_frames: u32,
    /// Remaining available frame credit.
    pub available_credit: u32,
    /// Maximum accepted typed-envelope frame size.
    pub max_frame_bytes: u64,
    /// Bounded queued frame count.
    pub queued_frame_count: u32,
    /// Last accepted frame sequence.
    pub last_accepted_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Window contract schema version.
    pub schema_version: u16,
}

/// Metadata-only replay window for remote transport duplicate/replay defense.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportReplayWindow {
    /// Workspace session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Lowest retained accepted frame sequence.
    pub lowest_accepted_sequence: EventSequence,
    /// Highest accepted frame sequence.
    pub highest_accepted_sequence: EventSequence,
    /// Number of accepted unique operations retained.
    pub accepted_operation_count: u32,
    /// Number of duplicate operations observed.
    pub duplicate_operation_count: u32,
    /// Last checkpoint represented by this replay window.
    pub checkpoint_id: Option<RemoteOperationLogCheckpointId>,
    /// Replay window contract schema version.
    pub schema_version: u16,
}

/// Remote agent package metadata required before package activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAgentPackageDescriptor {
    /// Remote agent identifier.
    pub agent_id: RemoteAgentId,
    /// Remote authority identifier.
    pub authority_id: RemoteAuthorityId,
    /// Stable package identifier.
    pub package_id: String,
    /// Package semantic version or redacted version label.
    pub version: String,
    /// Package integrity digest.
    pub package_digest: FileFingerprint,
    /// Redacted signature or certificate reference.
    pub signature_reference: String,
    /// Declared package capabilities.
    pub declared_capabilities: Vec<CapabilityId>,
    /// Granted activation capability decision.
    pub capability_decision: CapabilityDecision,
    /// Redaction hints for package metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Package descriptor contract schema version.
    pub schema_version: u16,
}

/// Remote agent package lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteAgentPackageLifecycleState {
    /// Package was staged but not yet activated.
    Staged,
    /// Package integrity/signature metadata was verified.
    Verified,
    /// Package health check passed.
    Healthy,
    /// Package was activated.
    Activated,
    /// Package was shut down.
    Shutdown,
    /// Package activation or upgrade failed.
    Failed,
    /// Package was rolled back.
    RolledBack,
}

/// Metadata-only remote agent package lifecycle record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteAgentPackageLifecycleRecord {
    /// Remote agent identifier.
    pub agent_id: RemoteAgentId,
    /// Remote authority identifier.
    pub authority_id: RemoteAuthorityId,
    /// Stable package identifier.
    pub package_id: String,
    /// Package integrity digest.
    pub package_digest: FileFingerprint,
    /// Current lifecycle state.
    pub state: RemoteAgentPackageLifecycleState,
    /// Previous lifecycle state when known.
    pub previous_state: Option<RemoteAgentPackageLifecycleState>,
    /// Optional rollback reference, never a payload body.
    pub rollback_reference: Option<String>,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Metadata-only lifecycle summary.
    pub metadata_summary: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Lifecycle record schema version.
    pub schema_version: u16,
}

/// Metadata-only production remote transport health summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportHealthSummary {
    /// Workspace session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Current health state.
    pub health: RemoteNetworkHealthState,
    /// Last accepted operation when known.
    pub last_operation_id: Option<RemoteOperationId>,
    /// Bounded queued frame count.
    pub queued_frame_count: u32,
    /// Reconnect attempt count.
    pub reconnect_attempts: u32,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Health summary schema version.
    pub schema_version: u16,
}

/// Metadata-only production remote transport audit summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTransportAuditSummary {
    /// Workspace session identifier.
    pub session_id: RemoteWorkspaceSessionId,
    /// Audit event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Metadata-only summary without raw transport payloads.
    pub metadata_summary: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Audit summary schema version.
    pub schema_version: u16,
}

/// Extended local terminal runtime state for Phase 8 activation gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminalRuntimeState {
    /// Session is starting.
    Starting,
    /// Session is running.
    Running,
    /// Session is exiting.
    Exiting,
    /// Session exited normally or by signal.
    Exited,
    /// Session launch or lifecycle was denied by policy.
    Denied,
    /// Backend failed.
    Failed,
    /// Runtime is degraded but still projected.
    Degraded,
}

/// Terminal launch policy metadata required before local PTY activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalLaunchPolicyContract {
    /// Principal requesting launch.
    pub principal_id: PrincipalId,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Workspace trust state.
    pub trust_state: WorkspaceTrustState,
    /// Required terminal capability.
    pub capability_id: CapabilityId,
    /// Working-directory policy label.
    pub cwd_policy: String,
    /// Bounded output byte ceiling.
    pub output_byte_limit: u64,
    /// Timeout in seconds.
    pub timeout_seconds: u64,
    /// Contract schema version.
    pub schema_version: u16,
}

/// Bounded terminal output metadata chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalOutputChunk {
    /// Terminal session identifier.
    pub session_id: TerminalSessionId,
    /// Output sequence.
    pub sequence: EventSequence,
    /// Bounded redacted text payload for projection only.
    pub redacted_payload: String,
    /// Byte count before redaction or truncation.
    pub byte_count: u64,
    /// Whether output was truncated.
    pub truncated: bool,
    /// Redaction hint.
    pub redaction: RedactionHint,
    /// Chunk schema version.
    pub schema_version: u16,
}

/// Terminal kill escalation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminalKillEscalation {
    /// Request graceful interrupt.
    Interrupt,
    /// Request process termination.
    Terminate,
    /// Request process-tree kill; requires explicit policy approval.
    KillTree,
}

/// Typed terminal close request metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCloseRequest {
    /// Terminal session identifier.
    pub session_id: TerminalSessionId,
    /// Principal requesting close.
    pub principal_id: PrincipalId,
    /// Required terminal capability.
    pub capability_id: CapabilityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Metadata-only close summary.
    pub metadata_summary: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Close request schema version.
    pub schema_version: u16,
}

/// Typed terminal kill request metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalKillRequest {
    /// Terminal session identifier.
    pub session_id: TerminalSessionId,
    /// Principal requesting kill.
    pub principal_id: PrincipalId,
    /// Required terminal capability.
    pub capability_id: CapabilityId,
    /// Requested escalation mode.
    pub escalation: TerminalKillEscalation,
    /// Whether kill-tree escalation was explicitly authorized.
    pub kill_tree_authorized: bool,
    /// Maximum wait time before escalation in milliseconds.
    pub escalation_timeout_ms: u64,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Metadata-only kill summary.
    pub metadata_summary: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Kill request schema version.
    pub schema_version: u16,
}

/// Metadata-only terminal audit summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalAuditRecord {
    /// Terminal session identifier.
    pub session_id: TerminalSessionId,
    /// Terminal runtime state.
    pub state: TerminalRuntimeState,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Metadata-only summary without raw command bodies or transcripts.
    pub metadata_summary: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Audit schema version.
    pub schema_version: u16,
}

/// Hosted telemetry category with explicit consent semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostedTelemetryCategory {
    /// Local diagnostics metadata.
    Diagnostics,
    /// Performance metrics metadata.
    Performance,
    /// Security audit metadata.
    SecurityAudit,
    /// Crash or failure summary metadata.
    CrashSummary,
    /// Remote transport health metadata.
    RemoteTransportHealth,
    /// Terminal health metadata.
    TerminalHealth,
}

/// Hosted telemetry endpoint policy descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetryEndpointDescriptor {
    /// Endpoint identifier.
    pub endpoint_id: String,
    /// Endpoint URL or redacted URL label.
    pub endpoint_label: String,
    /// Region policy label.
    pub region: String,
    /// Whether endpoint is explicitly allowlisted.
    pub allowlisted: bool,
    /// Endpoint schema version.
    pub schema_version: u16,
}

/// Hosted telemetry consent grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetryConsentGrant {
    /// Principal granting consent.
    pub principal_id: PrincipalId,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Consented telemetry categories.
    pub categories: Vec<HostedTelemetryCategory>,
    /// Consented endpoint descriptor.
    pub endpoint: HostedTelemetryEndpointDescriptor,
    /// Consent expiry timestamp.
    pub expires_at: Option<TimestampMillis>,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Consent schema version.
    pub schema_version: u16,
}

/// Hosted telemetry consent state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostedTelemetryConsentState {
    /// Consent is current and may be used for policy-bound export.
    Current,
    /// Consent was revoked.
    Revoked,
    /// Consent expired.
    Expired,
}

/// Hosted telemetry consent binding metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetryConsentBinding {
    /// Stable consent grant identifier.
    pub grant_id: String,
    /// Principal bound to the grant.
    pub principal_id: PrincipalId,
    /// Workspace bound to the grant.
    pub workspace_id: WorkspaceId,
    /// Endpoint identifier bound to the grant.
    pub endpoint_id: String,
    /// Region bound to the grant.
    pub region: String,
    /// Categories covered by the grant.
    pub categories: Vec<HostedTelemetryCategory>,
    /// Grant issue timestamp.
    pub issued_at: TimestampMillis,
    /// Revocation generation observed when issued.
    pub revocation_generation: u64,
    /// Current consent state.
    pub state: HostedTelemetryConsentState,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Consent binding schema version.
    pub schema_version: u16,
}

/// Hosted telemetry TLS policy metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetryTlsPolicy {
    /// Whether HTTPS is mandatory for production export.
    pub https_required: bool,
    /// Minimum TLS version label.
    pub min_tls_version: String,
    /// Whether certificate validation is required.
    pub certificate_validation_required: bool,
    /// TLS policy schema version.
    pub schema_version: u16,
}

/// Hosted telemetry proxy policy metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetryProxyPolicy {
    /// Whether proxy routing is allowed.
    pub proxy_allowed: bool,
    /// Optional proxy endpoint reference.
    pub proxy_endpoint_id: Option<String>,
    /// Whether proxy bypass is allowed.
    pub bypass_allowed: bool,
    /// Proxy policy schema version.
    pub schema_version: u16,
}

/// Hosted telemetry retry policy metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetryRetryPolicy {
    /// Maximum upload attempts.
    pub max_attempts: u8,
    /// Initial backoff in milliseconds.
    pub initial_backoff_ms: u64,
    /// Maximum backoff in milliseconds.
    pub max_backoff_ms: u64,
    /// Whether `Retry-After` metadata may be honored.
    pub retry_after_allowed: bool,
    /// Retry policy schema version.
    pub schema_version: u16,
}

/// Hosted telemetry endpoint policy metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetryEndpointPolicy {
    /// Endpoint descriptor.
    pub endpoint: HostedTelemetryEndpointDescriptor,
    /// TLS policy.
    pub tls_policy: HostedTelemetryTlsPolicy,
    /// Proxy policy.
    pub proxy_policy: HostedTelemetryProxyPolicy,
    /// Retry policy.
    pub retry_policy: HostedTelemetryRetryPolicy,
    /// Categories allowed at this endpoint.
    pub allowed_categories: Vec<HostedTelemetryCategory>,
    /// Maximum JSON body size in bytes.
    pub max_body_bytes: u64,
    /// Endpoint policy schema version.
    pub schema_version: u16,
}

/// Hosted telemetry diagnostics snapshot metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetryDiagnosticsSnapshot {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Number of pending records.
    pub pending_record_count: u64,
    /// Number of dropped records.
    pub dropped_record_count: u64,
    /// Age of oldest pending record in milliseconds.
    pub oldest_record_age_ms: Option<u64>,
    /// Metadata-only upload status label.
    pub last_upload_status: String,
    /// Next retry delay in milliseconds.
    pub next_retry_after_ms: Option<u64>,
    /// Air-gap denial count.
    pub air_gap_denial_count: u64,
    /// Redaction rejection count.
    pub redaction_rejection_count: u64,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Metadata-only diagnostic summary.
    pub metadata_summary: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Diagnostics schema version.
    pub schema_version: u16,
}

/// Privacy classification for hosted telemetry export fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrivacyClassification {
    /// Field is safe bounded metadata.
    Metadata,
    /// Field was bucketed or hashed.
    Derived,
    /// Field is sensitive and must be dropped before export.
    Sensitive,
    /// Field is raw source or transcript and must never be exported by telemetry.
    RawContent,
}

/// Hosted telemetry spool record containing classified metadata only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetrySpoolRecord {
    /// Spool record identifier.
    pub record_id: String,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Telemetry category.
    pub category: HostedTelemetryCategory,
    /// Privacy classification after structured inspection.
    pub classification: PrivacyClassification,
    /// Metadata-only summary.
    pub metadata_summary: String,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Spool schema version.
    pub schema_version: u16,
}

/// Hosted telemetry export batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetryExportBatch {
    /// Batch identifier.
    pub batch_id: String,
    /// Export endpoint descriptor.
    pub endpoint: HostedTelemetryEndpointDescriptor,
    /// Consent grant used for export.
    pub consent: HostedTelemetryConsentGrant,
    /// Metadata-only spool records.
    pub records: Vec<HostedTelemetrySpoolRecord>,
    /// Batch schema version.
    pub schema_version: u16,
}

/// Hosted telemetry upload outcome metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedTelemetryUploadOutcome {
    /// Batch identifier.
    pub batch_id: String,
    /// Whether upload was accepted by the endpoint.
    pub accepted: bool,
    /// Retry-after milliseconds when provided.
    pub retry_after_ms: Option<u64>,
    /// Metadata-only status label.
    pub status: String,
    /// Outcome schema version.
    pub schema_version: u16,
}

/// Purpose-bound raw-source retention purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RawSourceRetentionPurpose {
    /// Local crash or support bundle.
    SupportBundle,
    /// Local replay or reproduction bundle.
    Replay,
    /// Storage migration debug bundle.
    MigrationDebug,
    /// Break-glass enterprise support flow.
    BreakGlass,
}

/// Default-deny raw-source retention policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceRetentionPolicy {
    /// Whether raw-source capture is enabled.
    pub capture_enabled: bool,
    /// Allowed purposes under this policy.
    pub allowed_purposes: Vec<RawSourceRetentionPurpose>,
    /// Maximum retained bytes per bundle.
    pub max_bundle_bytes: u64,
    /// Default TTL in milliseconds.
    pub ttl_ms: u64,
    /// Policy schema version.
    pub schema_version: u16,
}

/// Raw-source retention consent grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceRetentionConsentGrant {
    /// Principal granting consent.
    pub principal_id: PrincipalId,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Purpose bound to this grant.
    pub purpose: RawSourceRetentionPurpose,
    /// Canonical path scope.
    pub path_scope: Vec<CanonicalPath>,
    /// Grant expiry timestamp.
    pub expires_at: TimestampMillis,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Grant schema version.
    pub schema_version: u16,
}

/// Raw-source capture request metadata; raw content is intentionally excluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceCaptureRequest {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Principal requesting capture.
    pub principal_id: PrincipalId,
    /// Capture purpose.
    pub purpose: RawSourceRetentionPurpose,
    /// Paths requested for capture.
    pub paths: Vec<CanonicalPath>,
    /// Maximum requested bytes.
    pub max_bytes: u64,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Request schema version.
    pub schema_version: u16,
}

/// Raw-source retention lease metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceRetentionLease {
    /// Lease identifier.
    pub lease_id: String,
    /// Consent grant backing this lease.
    pub consent: RawSourceRetentionConsentGrant,
    /// Expiry timestamp.
    pub expires_at: TimestampMillis,
    /// Lease schema version.
    pub schema_version: u16,
}

/// Raw-source retention bundle descriptor without raw content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceRetentionBundleDescriptor {
    /// Bundle identifier.
    pub bundle_id: String,
    /// Lease identifier.
    pub lease_id: String,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Purpose bound to this bundle.
    pub purpose: RawSourceRetentionPurpose,
    /// Encrypted byte length.
    pub encrypted_byte_len: u64,
    /// Integrity fingerprint.
    pub integrity: FileFingerprint,
    /// Bundle schema version.
    pub schema_version: u16,
}

/// Metadata-only raw-source retention access audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceRetentionAccessAudit {
    /// Bundle identifier.
    pub bundle_id: String,
    /// Principal accessing the bundle.
    pub principal_id: PrincipalId,
    /// Metadata-only action label.
    pub action: String,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Audit schema version.
    pub schema_version: u16,
}

/// Raw-source retention deletion tombstone metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceRetentionTombstone {
    /// Bundle identifier.
    pub bundle_id: String,
    /// Deletion reason label.
    pub reason: String,
    /// Deletion timestamp.
    pub deleted_at: TimestampMillis,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Tombstone schema version.
    pub schema_version: u16,
}

/// Hosted telemetry linkage to a raw-source bundle descriptor by reference only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedRetentionExportLinkage {
    /// Hosted telemetry batch identifier.
    pub telemetry_batch_id: String,
    /// Retention bundle identifier.
    pub bundle_id: String,
    /// Whether separate raw-source export consent was verified.
    pub raw_source_consent_verified: bool,
    /// Linkage schema version.
    pub schema_version: u16,
}

/// Production raw-source vault algorithm identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RawSourceVaultAlgorithm {
    /// AES-256-GCM authenticated encryption.
    Aes256Gcm,
    /// ChaCha20-Poly1305 authenticated encryption.
    ChaCha20Poly1305,
}

/// Metadata-only raw-source key reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceKeyReference {
    /// Stable key reference identifier.
    pub key_id: String,
    /// Key version label.
    pub key_version: String,
    /// Provider label, never provider credentials or key bytes.
    pub provider_label: String,
    /// Rotation generation.
    pub rotation_generation: u64,
    /// Key reference schema version.
    pub schema_version: u16,
}

/// Metadata-only raw-source vault envelope descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceVaultEnvelope {
    /// Bundle identifier.
    pub bundle_id: String,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Purpose bound to the envelope.
    pub purpose: RawSourceRetentionPurpose,
    /// Authenticated encryption algorithm.
    pub algorithm: RawSourceVaultAlgorithm,
    /// Key reference.
    pub key_reference: RawSourceKeyReference,
    /// Nonce digest or nonce reference, never raw nonce bytes.
    pub nonce_digest: String,
    /// Ciphertext digest.
    pub ciphertext_digest: FileFingerprint,
    /// Authentication tag digest or tag reference, never raw tag bytes.
    pub tag_digest: String,
    /// Additional authenticated data digest.
    pub aad_digest: String,
    /// Encrypted byte length.
    pub encrypted_byte_len: u64,
    /// Envelope schema version.
    pub schema_version: u16,
}

/// Metadata-only raw-source key rotation record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceKeyRotationRecord {
    /// Bundle identifier rotated.
    pub bundle_id: String,
    /// Previous key reference.
    pub previous_key_reference: RawSourceKeyReference,
    /// New key reference.
    pub new_key_reference: RawSourceKeyReference,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Metadata-only rotation summary.
    pub metadata_summary: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Rotation record schema version.
    pub schema_version: u16,
}

/// Raw-source vault recovery state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RawSourceVaultRecoveryState {
    /// Recovery was denied and data remained inaccessible.
    FailedClosed,
    /// Corrupt or suspect metadata was quarantined.
    Quarantined,
    /// Metadata was recovered without exposing raw source.
    Recovered,
}

/// Metadata-only raw-source vault recovery report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceVaultRecoveryReport {
    /// Stable recovery identifier.
    pub recovery_id: String,
    /// Bundle identifier when scoped to one bundle.
    pub bundle_id: Option<String>,
    /// Recovery state.
    pub state: RawSourceVaultRecoveryState,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Metadata-only recovery summary.
    pub metadata_summary: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Recovery report schema version.
    pub schema_version: u16,
}

/// Separate hosted raw-source export consent metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceHostedExportConsent {
    /// Hosted raw-source export grant identifier.
    pub grant_id: String,
    /// Principal granting hosted raw-source export.
    pub principal_id: PrincipalId,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Endpoint identifier bound to this grant.
    pub endpoint_id: String,
    /// Purpose bound to this grant.
    pub purpose: RawSourceRetentionPurpose,
    /// Grant issue timestamp.
    pub issued_at: TimestampMillis,
    /// Grant expiry timestamp.
    pub expires_at: TimestampMillis,
    /// Whether the grant has been revoked.
    pub revoked: bool,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Hosted export consent schema version.
    pub schema_version: u16,
}

/// Metadata-only schema manifest for a persisted Phase 8 store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageSchemaManifest {
    /// Stable subsystem identifier, such as `telemetry-spool`.
    pub subsystem_id: String,
    /// Stable logical store identifier.
    pub store_id: String,
    /// Currently active schema version.
    pub active_schema_version: u16,
    /// Oldest schema version supported by the migration registry.
    pub min_supported_schema_version: u16,
    /// Newest schema version supported by the migration registry.
    pub max_supported_schema_version: u16,
    /// Metadata-only summary; never source, transcript, payload, prompt, or secret content.
    pub metadata_summary: String,
    /// Redaction hints for the manifest summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Manifest contract schema version.
    pub schema_version: u16,
}

/// Explicit storage migration registry step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageMigrationStep {
    /// Stable migration identifier.
    pub migration_id: String,
    /// Subsystem affected by the step.
    pub subsystem_id: String,
    /// Source schema version.
    pub from_schema_version: u16,
    /// Target schema version.
    pub to_schema_version: u16,
    /// Whether the step may remove or compact data.
    pub destructive: bool,
    /// Whether a backup marker is required before apply.
    pub requires_backup: bool,
    /// Step contract schema version.
    pub schema_version: u16,
}

/// Dry-run migration report; metadata only and safe to archive as evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageMigrationDryRunReport {
    /// Migration step evaluated by the dry-run.
    pub step: StorageMigrationStep,
    /// Whether the step is compatible with the active registry.
    pub compatible: bool,
    /// Estimated number of metadata records affected.
    pub estimated_record_count: u64,
    /// Metadata-only dry-run summary.
    pub metadata_summary: String,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Redaction hints for the dry-run summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Report contract schema version.
    pub schema_version: u16,
}

/// Storage checksum descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageChecksum {
    /// Checksum algorithm label.
    pub algorithm: String,
    /// Checksum digest value.
    pub value: String,
    /// Checksum contract schema version.
    pub schema_version: u16,
}

/// Backup marker emitted before mutation-capable migration work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageBackupMarker {
    /// Stable backup identifier.
    pub backup_id: String,
    /// Subsystem covered by the backup.
    pub subsystem_id: String,
    /// Redacted path or storage location label.
    pub location_label: String,
    /// Backup checksum descriptor.
    pub checksum: StorageChecksum,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Marker contract schema version.
    pub schema_version: u16,
}

/// Metadata-only recovery outcome for storage or vault migration drills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageRecoveryOutcome {
    /// Stable recovery identifier.
    pub recovery_id: String,
    /// Subsystem recovered or quarantined.
    pub subsystem_id: String,
    /// Whether recovery succeeded.
    pub recovered: bool,
    /// Whether corrupt data was quarantined.
    pub quarantined: bool,
    /// Backup marker used for recovery when available.
    pub backup_id: Option<String>,
    /// Metadata-only recovery summary.
    pub metadata_summary: String,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Redaction hints for the recovery summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Outcome contract schema version.
    pub schema_version: u16,
}

/// Explicit request to perform migration repair work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageRepairRequest {
    /// Subsystem targeted by repair.
    pub subsystem_id: String,
    /// Principal requesting repair.
    pub principal_id: PrincipalId,
    /// Capability decision for `storage.migration.repair`.
    pub capability_decision: CapabilityDecision,
    /// Explicit operator repair flag; repair must fail closed when false.
    pub explicit_repair_flag: bool,
    /// Metadata-only request summary.
    pub metadata_summary: String,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Request contract schema version.
    pub schema_version: u16,
}

/// Metadata-only replay manifest used for Phase 8 evidence drills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageReplayManifest {
    /// Stable replay identifier.
    pub replay_id: String,
    /// Subsystem covered by replay.
    pub subsystem_id: String,
    /// Number of metadata events in the replay.
    pub event_count: u64,
    /// First event sequence included.
    pub first_event_sequence: EventSequence,
    /// Last event sequence included.
    pub last_event_sequence: EventSequence,
    /// Metadata-only replay summary.
    pub metadata_summary: String,
    /// Redaction hints for replay evidence.
    pub redaction_hints: Vec<RedactionHint>,
    /// Replay contract schema version.
    pub schema_version: u16,
}

/// Metadata-only subsystem health summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageSubsystemHealthSummary {
    /// Subsystem being reported.
    pub subsystem_id: String,
    /// Whether the subsystem is healthy.
    pub healthy: bool,
    /// Whether the subsystem is degraded but available.
    pub degraded: bool,
    /// Metadata-only health summary.
    pub metadata_summary: String,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Redaction hints for health evidence.
    pub redaction_hints: Vec<RedactionHint>,
    /// Health summary contract schema version.
    pub schema_version: u16,
}

/// Metadata-only evidence generation summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageEvidenceSummary {
    /// Evidence artifact identifier.
    pub artifact_id: String,
    /// Command or drill label.
    pub command_label: String,
    /// Whether the evidence command or drill passed.
    pub passed: bool,
    /// Metadata-only evidence summary.
    pub metadata_summary: String,
    /// Redaction hints for evidence records.
    pub redaction_hints: Vec<RedactionHint>,
    /// Evidence summary contract schema version.
    pub schema_version: u16,
}

/// Explicit request to apply a storage migration after dry-run and backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageMigrationApplyRequest {
    /// Migration step to apply.
    pub step: StorageMigrationStep,
    /// Principal requesting apply.
    pub principal_id: PrincipalId,
    /// Capability decision for `storage.migration.apply`.
    pub capability_decision: CapabilityDecision,
    /// Preflight dry-run report.
    pub preflight_report: StorageMigrationDryRunReport,
    /// Backup marker required before apply.
    pub backup_marker: StorageBackupMarker,
    /// Durable migration journal identifier.
    pub journal_id: String,
    /// Explicit operator apply flag.
    pub explicit_apply_flag: bool,
    /// Metadata-only apply summary.
    pub metadata_summary: String,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Apply request schema version.
    pub schema_version: u16,
}

/// Metadata-only storage migration apply outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageMigrationApplyOutcome {
    /// Migration identifier.
    pub migration_id: String,
    /// Subsystem identifier.
    pub subsystem_id: String,
    /// Whether the migration was applied.
    pub applied: bool,
    /// Backup identifier used for apply.
    pub backup_id: String,
    /// Durable migration journal identifier.
    pub journal_id: String,
    /// Optional recovery reference if apply failed closed.
    pub recovery_id: Option<String>,
    /// Post-apply checksum.
    pub checksum: StorageChecksum,
    /// Metadata-only outcome summary.
    pub metadata_summary: String,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Apply outcome schema version.
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
    /// Validated collaboration participant edit.
    CollaborationParticipant {
        /// Session identifier.
        session_id: CollaborationSessionId,
        /// Participant identifier.
        participant_id: CollaborationParticipantId,
        /// Operation identifier.
        operation_id: CollaborationOperationId,
    },
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Assisted-AI-produced proposal after provider output was translated into reviewable DTOs.
    AiAssisted,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Display-safe lifecycle state label for proposal ledger projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalLifecycleStateDisplay {
    /// Machine-readable lifecycle state.
    pub state: ProposalLifecycleState,
    /// Short display label.
    pub label: String,
    /// Bounded display description that contains no raw source or prompts.
    pub description: String,
}

/// Proposal risk label used by projection-only UI surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalRiskLabel {
    /// Metadata-only or read-only risk.
    Informational,
    /// Low-risk bounded local change.
    Low,
    /// Medium-risk multi-target or policy-sensitive change.
    Medium,
    /// High-risk destructive, irreversible, privileged, or broad change.
    High,
    /// Risk was redacted or unavailable and must be treated conservatively.
    Unknown,
}

/// Proposal privacy label used by metadata-only projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalPrivacyLabel {
    /// Public metadata only.
    PublicMetadata,
    /// Workspace-private metadata only.
    WorkspaceMetadata,
    /// Sensitive metadata was redacted.
    RedactedSensitive,
    /// Provider or network egress metadata is involved.
    ExternalEgressMetadata,
    /// Privacy classification is unknown and must be treated conservatively.
    Unknown,
}

/// Rollback availability shown in proposal ledger projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalRollbackAvailability {
    /// Rollback is available and expected to be exact.
    Available,
    /// Rollback is available but may be best-effort.
    BestEffort,
    /// Rollback is not required for this proposal.
    NotRequired,
    /// Rollback is unavailable or unsupported.
    Unavailable,
    /// Rollback status is unknown.
    Unknown,
}

/// Kind of redacted diff metadata represented in a projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalDiffSummaryKind {
    /// Text edit summary.
    Text,
    /// File create/delete/rename summary.
    FileOperation,
    /// Workspace edit summary.
    WorkspaceEdit,
    /// Terminal or process-side metadata summary.
    TerminalMetadata,
    /// Metadata-only proposal summary.
    MetadataOnly,
}

/// Chunk or range descriptor for large proposal diffs without raw source text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalDiffChunkDescriptor {
    /// Stable chunk identifier within the diff summary.
    pub chunk_id: String,
    /// Optional target id from target coverage.
    pub target_id: Option<String>,
    /// Affected byte range when safely discloseable.
    pub byte_range: Option<ByteRange>,
    /// Number of changed lines represented by this chunk.
    pub changed_line_count: u32,
    /// Number of inserted lines represented by this chunk.
    pub inserted_line_count: u32,
    /// Number of deleted lines represented by this chunk.
    pub deleted_line_count: u32,
    /// Optional metadata hash of this chunk instead of raw text.
    pub content_hash: Option<FileFingerprint>,
}

/// Metadata-only diff summary for proposal ledger projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalDiffSummary {
    /// Diff summary kind.
    pub kind: ProposalDiffSummaryKind,
    /// Number of files or targets represented.
    pub target_count: u32,
    /// Number of edit hunks or operation groups.
    pub hunk_count: u32,
    /// Number of inserted lines, if known.
    pub inserted_line_count: u32,
    /// Number of deleted lines, if known.
    pub deleted_line_count: u32,
    /// Number of omitted hunks or chunks due to projection bounds.
    pub omitted_hunk_count: u32,
    /// True when this summary intentionally excludes raw source text.
    pub full_source_redacted: bool,
    /// Optional hash of the complete diff payload or provider-side patch.
    pub diff_hash: Option<FileFingerprint>,
    /// Bounded chunk descriptors for large diffs.
    pub chunks: Vec<ProposalDiffChunkDescriptor>,
    /// Redaction hints that apply to the diff summary.
    pub redaction_hints: Vec<RedactionHint>,
}

/// One context-manifest category summary for proposal projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalContextManifestEntrySummary {
    /// Context category, such as files, symbols, diagnostics, tests, provider route, or memory.
    pub category: String,
    /// Number of metadata items in this category.
    pub item_count: u32,
    /// Number of items omitted or redacted in this category.
    pub omitted_item_count: u32,
    /// Privacy label for this category summary.
    pub privacy_label: ProposalPrivacyLabel,
    /// Optional metadata hash for the category manifest.
    pub manifest_hash: Option<FileFingerprint>,
    /// Redaction hints that apply to this category.
    pub redaction_hints: Vec<RedactionHint>,
}

/// Metadata-only context manifest summary for a proposal ledger row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalContextManifestSummary {
    /// Stable manifest identifier or hash label.
    pub manifest_id: String,
    /// Number of context categories represented.
    pub category_count: u32,
    /// Number of total metadata items represented.
    pub total_item_count: u32,
    /// Number of redacted or omitted items.
    pub omitted_item_count: u32,
    /// Deterministic category summaries.
    pub categories: Vec<ProposalContextManifestEntrySummary>,
    /// Redaction hints that apply to the manifest summary.
    pub redaction_hints: Vec<RedactionHint>,
}

/// Trust-surface purpose for a metadata-only context manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextManifestPurpose {
    /// Proposal review or approval context.
    ProposalReview,
    /// Trust-layer approval gateway context.
    TrustReview,
    /// Future provider request context; this DTO does not authorize provider calls.
    ProviderRequest,
    /// Explain-only context that does not imply mutation.
    Explanation,
}

/// Metadata-only category for a context manifest item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextManifestItemKind {
    /// Workspace metadata.
    Workspace,
    /// File identity, hash, path, range, or count metadata.
    File,
    /// Buffer and snapshot identity metadata.
    Buffer,
    /// Proposal target metadata.
    ProposalTarget,
    /// Proposal precondition metadata.
    ProposalPreconditions,
    /// Semantic metadata record summary.
    SemanticRecord,
    /// Semantic fabric job request metadata.
    SemanticFabricJob,
    /// Semantic fabric schedule-plan metadata.
    SemanticFabricSchedulePlan,
    /// LSP diagnostic summary metadata.
    LspDiagnosticSummary,
    /// LSP supervision metadata.
    LspSupervisionSummary,
    /// Privacy-scope metadata.
    PrivacyScope,
    /// Model/provider permission metadata.
    ModelPermission,
    /// Tool or capability permission metadata.
    ToolPermission,
    /// Risk label metadata.
    RiskLabel,
    /// Tracker task metadata.
    TrackerTask,
    /// Memory retention or candidate metadata.
    MemoryRecord,
    /// Retrieval result metadata without vector payloads or source bodies.
    RetrievedChunk,
    /// Terminal summary metadata without terminal output bodies.
    TerminalSummary,
    /// User selection metadata.
    UserSelection,
    /// Provider route metadata.
    ProviderRoute,
    /// Agent step metadata.
    AgentStep,
}

/// Inclusion disposition for a manifest item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextManifestInclusionState {
    /// Item is included as metadata only.
    Included,
    /// Item is excluded by policy, freshness, or selection.
    Excluded,
    /// Item is represented only by redacted metadata.
    Redacted,
    /// Item was denied by policy.
    Denied,
    /// Item is counted but omitted due to projection bounds.
    Omitted,
}

/// Metadata-only egress state represented by a context manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextManifestEgressStatus {
    /// Context remains local only.
    LocalOnly,
    /// Local provider metadata is represented without network egress.
    LocalProvider,
    /// Remote egress would require explicit approval elsewhere.
    RemoteApprovalRequired,
    /// Remote egress is denied.
    RemoteDenied,
    /// External egress metadata is represented, without payload bodies.
    ExternalEgressMetadata,
    /// Egress posture is unknown and must be treated conservatively.
    Unknown,
}

/// Permission kind summarized in a context manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextManifestPermissionKind {
    /// Filesystem capability metadata.
    Filesystem,
    /// LSP capability metadata.
    Lsp,
    /// Semantic fabric capability metadata.
    Semantic,
    /// Model/provider capability metadata.
    ModelProvider,
    /// Tool capability metadata.
    Tool,
    /// Network or egress capability metadata.
    Network,
}

/// Metadata-only permission summary for context and trust projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifestPermissionSummary {
    /// Permission class.
    pub kind: ContextManifestPermissionKind,
    /// Capability associated with the permission.
    pub capability: CapabilityId,
    /// Principal associated with the permission, when known.
    pub principal: Option<PrincipalId>,
    /// Existing capability decision, when known.
    pub decision_id: Option<CapabilityDecisionId>,
    /// Whether policy metadata says this permission is granted.
    pub granted: bool,
    /// Privacy scope associated with the permission.
    pub privacy_scope: SemanticPrivacyScope,
    /// Egress posture associated with the permission.
    pub egress: ContextManifestEgressStatus,
    /// Risk label for the permission summary.
    pub risk_label: ProposalRiskLabel,
    /// Redaction hints for the permission summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Permission summary schema version.
    pub schema_version: u16,
}

/// Count or bounded metric attached to a metadata-only manifest item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifestItemCount {
    /// Metric label.
    pub label: String,
    /// Metric value.
    pub count: u32,
}

/// Metadata-only freshness summary for a manifest item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifestFreshnessSummary {
    /// Freshness state.
    pub state: SemanticFreshnessState,
    /// Whether a freshness key or equivalent precondition metadata was present.
    pub freshness_key_present: bool,
    /// Snapshot identifier represented by the freshness metadata.
    pub snapshot_id: Option<SnapshotId>,
    /// File content version represented by the freshness metadata.
    pub file_content_version: Option<FileContentVersion>,
    /// Workspace generation represented by the freshness metadata.
    pub workspace_generation: Option<WorkspaceGeneration>,
    /// Content hash represented by the freshness metadata.
    pub content_hash: Option<FileFingerprint>,
    /// Privacy scope represented by the freshness metadata.
    pub privacy_scope: Option<SemanticPrivacyScope>,
    /// Observation timestamp, when known.
    pub observed_at: Option<TimestampMillis>,
    /// Risk label caused by freshness posture.
    pub risk_label: ProposalRiskLabel,
    /// Metadata-only reason codes explaining freshness risk.
    pub risk_reasons: Vec<String>,
    /// Freshness summary schema version.
    pub schema_version: u16,
}

/// Metadata-only proposal precondition summary for a context manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifestPreconditionSummary {
    /// Expected file content version.
    pub file_content_version: Option<FileContentVersion>,
    /// Expected buffer version.
    pub buffer_version: Option<BufferVersion>,
    /// Expected snapshot id.
    pub snapshot_id: Option<SnapshotId>,
    /// Expected workspace generation.
    pub workspace_generation: Option<WorkspaceGeneration>,
    /// Expected disk fingerprint hash.
    pub expected_fingerprint: Option<FileFingerprint>,
    /// Expected file length.
    pub expected_file_length: Option<u64>,
    /// Expected modified timestamp.
    pub expected_modified_at: Option<TimestampMillis>,
    /// Whether all core proposal preconditions are present.
    pub core_preconditions_present: bool,
    /// Risk label caused by precondition posture.
    pub risk_label: ProposalRiskLabel,
    /// Metadata-only reason codes explaining precondition risk.
    pub risk_reasons: Vec<String>,
    /// Precondition summary schema version.
    pub schema_version: u16,
}

/// One metadata-only item in a context manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifestItem {
    /// Stable item identifier within the manifest.
    pub item_id: String,
    /// Manifest category for this item.
    pub kind: ContextManifestItemKind,
    /// Inclusion disposition.
    pub inclusion: ContextManifestInclusionState,
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Optional file identifier.
    pub file_id: Option<FileId>,
    /// Optional buffer identifier.
    pub buffer_id: Option<BufferId>,
    /// Optional proposal identifier.
    pub proposal_id: Option<ProposalId>,
    /// Optional target identifier.
    pub target_id: Option<String>,
    /// Canonical path only when policy allows path disclosure.
    pub path: Option<CanonicalPath>,
    /// Ranges represented without source bodies.
    pub ranges: Vec<ByteRange>,
    /// Metadata-only counts represented by this item.
    pub counts: Vec<ContextManifestItemCount>,
    /// Hashes represented by this item.
    pub hashes: Vec<FileFingerprint>,
    /// Privacy scope associated with this item.
    pub privacy_scope: Option<SemanticPrivacyScope>,
    /// Privacy label for projection and audit.
    pub privacy_label: ProposalPrivacyLabel,
    /// Risk label for projection and audit.
    pub risk_label: ProposalRiskLabel,
    /// Egress posture for this item.
    pub egress: ContextManifestEgressStatus,
    /// Freshness metadata when relevant.
    pub freshness: Option<ContextManifestFreshnessSummary>,
    /// Proposal precondition metadata when relevant.
    pub preconditions: Option<ContextManifestPreconditionSummary>,
    /// Bounded non-source labels and reason codes.
    pub labels: Vec<String>,
    /// Redaction hints that apply to this item.
    pub redaction_hints: Vec<RedactionHint>,
    /// Item schema version.
    pub schema_version: u16,
}

/// Metadata-only context manifest for trust and proposal projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifestRecord {
    /// Stable manifest identifier or hash label.
    pub manifest_id: String,
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Optional proposal identifier associated with the manifest.
    pub proposal_id: Option<ProposalId>,
    /// Manifest purpose.
    pub purpose: ContextManifestPurpose,
    /// Workspace trust state represented in the manifest.
    pub workspace_trust_state: Option<WorkspaceTrustState>,
    /// Overall privacy label.
    pub privacy_label: ProposalPrivacyLabel,
    /// Overall risk label.
    pub risk_label: ProposalRiskLabel,
    /// Overall egress posture.
    pub egress: ContextManifestEgressStatus,
    /// Manifest items.
    pub items: Vec<ContextManifestItem>,
    /// Permission summaries represented by this manifest.
    pub permissions: Vec<ContextManifestPermissionSummary>,
    /// Number of omitted items.
    pub omitted_item_count: u32,
    /// True when stale or missing freshness/precondition metadata is visible in the manifest.
    pub stale_or_missing_metadata_risk_present: bool,
    /// Manifest generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints that apply to the manifest.
    pub redaction_hints: Vec<RedactionHint>,
    /// Manifest schema version.
    pub schema_version: u16,
}

/// Static context-manifest projection consumed by projection-only UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifestProjection {
    /// Manifest snapshot.
    pub manifest: ContextManifestRecord,
    /// Selected item id when details are open.
    pub selected_item_id: Option<String>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints that apply to the projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

/// Metadata-only source category for a privacy inspector exposure record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyInspectorSourceKind {
    /// Context manifest item metadata.
    ContextManifestItem,
    /// Proposal target metadata.
    ProposalTarget,
    /// Semantic fabric metadata.
    SemanticMetadata,
    /// LSP diagnostic or supervision metadata.
    LspMetadata,
    /// Provider permission label metadata.
    ProviderPermission,
    /// Local tool permission label metadata.
    ToolPermission,
    /// Permission budget metadata.
    PermissionBudget,
}

/// Metadata-only redaction state displayed by the privacy inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyInspectorRedactionState {
    /// Only display-safe metadata is represented.
    MetadataOnly,
    /// Sensitive details are fully redacted.
    FullyRedacted,
}

/// Explicit refusal metadata for privacy or permission-gated actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyInspectorRefusal {
    /// Stable refusal reason code.
    pub reason_code: String,
    /// Display-safe refusal label.
    pub label: String,
    /// Privacy scope associated with the refusal.
    pub privacy_scope: Option<SemanticPrivacyScope>,
    /// Capability associated with the refusal, when known.
    pub capability: Option<CapabilityId>,
    /// Permission budget associated with the refusal, when known.
    pub budget_id: Option<String>,
    /// Risk label for the refusal.
    pub risk_label: ProposalRiskLabel,
    /// Metadata-only reason labels.
    pub reasons: Vec<String>,
    /// Redaction hints for the refusal record.
    pub redaction_hints: Vec<RedactionHint>,
    /// Refusal schema version.
    pub schema_version: u16,
}

/// One metadata-only exposure row in the privacy inspector projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyInspectorExposureRecord {
    /// Stable exposure identifier within the projection.
    pub exposure_id: String,
    /// Source category for this exposure row.
    pub source_kind: PrivacyInspectorSourceKind,
    /// Context manifest item identifier, when derived from an item.
    pub context_item_id: Option<String>,
    /// Proposal identifier, when proposal metadata is represented.
    pub proposal_id: Option<ProposalId>,
    /// Proposal target identifier, when target metadata is represented.
    pub target_id: Option<String>,
    /// Workspace identifier, when known.
    pub workspace_id: Option<WorkspaceId>,
    /// File identifier, when known.
    pub file_id: Option<FileId>,
    /// Buffer identifier, when known.
    pub buffer_id: Option<BufferId>,
    /// Privacy scope associated with the exposure.
    pub privacy_scope: Option<SemanticPrivacyScope>,
    /// Privacy label associated with the exposure.
    pub privacy_label: ProposalPrivacyLabel,
    /// Redaction state associated with the exposure.
    pub redaction_state: PrivacyInspectorRedactionState,
    /// Context inclusion state associated with the exposure.
    pub inclusion: ContextManifestInclusionState,
    /// Egress posture associated with the exposure.
    pub egress: ContextManifestEgressStatus,
    /// Risk label associated with the exposure.
    pub risk_label: ProposalRiskLabel,
    /// Permission or provider/tool capability label, when represented.
    pub permission_label: Option<CapabilityId>,
    /// Bounded ranges represented without source bodies.
    pub ranges: Vec<ByteRange>,
    /// Counts represented without raw payloads.
    pub counts: Vec<ContextManifestItemCount>,
    /// Hashes represented without raw payloads.
    pub hashes: Vec<FileFingerprint>,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Display-safe reason codes.
    pub reasons: Vec<String>,
    /// Redaction hints for the exposure record.
    pub redaction_hints: Vec<RedactionHint>,
    /// Exposure record schema version.
    pub schema_version: u16,
}

/// Static privacy inspector projection consumed by projection-only UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyInspectorProjection {
    /// Stable inspector projection identifier.
    pub inspector_id: String,
    /// Source context manifest identifier.
    pub manifest_id: Option<String>,
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Optional proposal identifier.
    pub proposal_id: Option<ProposalId>,
    /// Metadata-only exposure records.
    pub records: Vec<PrivacyInspectorExposureRecord>,
    /// Number of denied exposure records.
    pub denied_record_count: u32,
    /// Number of redacted exposure records.
    pub redacted_record_count: u32,
    /// Number of records with external or remote egress metadata.
    pub external_egress_record_count: u32,
    /// Number of high or unknown risk records.
    pub high_risk_record_count: u32,
    /// Explicit refusal metadata when privacy inspection blocks an action.
    pub refusal: Option<PrivacyInspectorRefusal>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints that apply to the projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

/// Metadata-only proposal context used to derive a privacy inspector from a proposal envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyInspectorProposalContext {
    /// Workspace trust state represented by the derived context manifest.
    pub workspace_trust_state: Option<WorkspaceTrustState>,
    /// Privacy label represented by the derived context manifest.
    pub privacy_label: ProposalPrivacyLabel,
    /// Risk label represented by the derived context manifest.
    pub risk_label: ProposalRiskLabel,
}

/// Permission-budget action classes used by trust-surface contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionBudgetActionClass {
    /// Read context manifest metadata.
    ReadContext,
    /// Read semantic metadata.
    ReadSemanticMetadata,
    /// Invoke a local tool.
    InvokeLocalTool,
    /// Invoke a model/provider route.
    InvokeProvider,
    /// Propose edits through proposal DTOs.
    ProposeEdits,
    /// Apply an already approved proposal through authority-owned execution.
    ApplyApprovedProposal,
    /// Access network or remote egress.
    AccessNetwork,
    /// Access terminal capabilities.
    AccessTerminal,
    /// Access workspace file metadata or file bodies through approved authority.
    AccessWorkspaceFiles,
    /// Retain memory metadata.
    RetainMemory,
}

/// Permission-budget state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionBudgetState {
    /// Budget is available for metadata-only contract evaluation.
    Allowed,
    /// Budget is denied by policy or consent posture.
    Denied,
    /// Budget is depleted.
    Depleted,
}

/// Reset policy label for a permission budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionBudgetResetPolicyLabel {
    /// No automatic reset.
    None,
    /// Resets per IDE session.
    Session,
    /// Resets daily.
    Daily,
    /// Resets per workspace.
    Workspace,
    /// Resets only after manual approval.
    ManualApproval,
    /// Reset posture is unknown.
    Unknown,
}

/// Consent requirement label for a permission budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionBudgetConsentRequirementLabel {
    /// Consent is not required for this metadata-only action class.
    NotRequired,
    /// Consent is required before action may proceed.
    Required,
    /// Consent must be renewed before action may proceed.
    RenewalRequired,
    /// Policy denies consent for this action class.
    DeniedByPolicy,
}

/// Permission-budget usage summary without raw sensitive payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionBudgetUsageSummary {
    /// Usage unit label, such as items, calls, bytes, or tokens.
    pub unit_label: String,
    /// Amount already used.
    pub used: u64,
    /// Optional ceiling for the usage window.
    pub ceiling: Option<u64>,
    /// Remaining amount when a ceiling is known.
    pub remaining: Option<u64>,
    /// Amount requested by the evaluated action.
    pub attempted: u64,
    /// Redaction hints for the usage summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Usage summary schema version.
    pub schema_version: u16,
}

/// Metadata-only permission budget contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionBudgetContract {
    /// Stable budget identifier.
    pub budget_id: String,
    /// Budgeted action class.
    pub action_class: PermissionBudgetActionClass,
    /// Capability associated with the budget, when known.
    pub capability: Option<CapabilityId>,
    /// Current budget state.
    pub state: PermissionBudgetState,
    /// Privacy scope governed by the budget.
    pub privacy_scope: SemanticPrivacyScope,
    /// Usage summary.
    pub usage: PermissionBudgetUsageSummary,
    /// Reset policy label.
    pub reset_policy_label: PermissionBudgetResetPolicyLabel,
    /// Consent requirement label.
    pub consent_requirement_label: PermissionBudgetConsentRequirementLabel,
    /// Risk label for the budget.
    pub risk_label: ProposalRiskLabel,
    /// Metadata-only reason labels.
    pub reasons: Vec<String>,
    /// Redaction hints for the budget.
    pub redaction_hints: Vec<RedactionHint>,
    /// Budget schema version.
    pub schema_version: u16,
}

/// Metadata-only action summary evaluated against a permission budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionBudgetActionSummary {
    /// Stable action identifier.
    pub action_id: String,
    /// Action class requested by the caller.
    pub action_class: PermissionBudgetActionClass,
    /// Capability requested by the action, when known.
    pub capability: Option<CapabilityId>,
    /// Workspace identifier, when known.
    pub workspace_id: Option<WorkspaceId>,
    /// Proposal identifier, when known.
    pub proposal_id: Option<ProposalId>,
    /// Target identifier, when known.
    pub target_id: Option<String>,
    /// Privacy scope requested by the action.
    pub privacy_scope: SemanticPrivacyScope,
    /// Egress posture requested by the action.
    pub egress: ContextManifestEgressStatus,
    /// Estimated usage units for this action.
    pub estimated_units: u64,
    /// Ranges represented without source bodies.
    pub ranges: Vec<ByteRange>,
    /// Counts represented without raw payloads.
    pub counts: Vec<ContextManifestItemCount>,
    /// Hashes represented without raw payloads.
    pub hashes: Vec<FileFingerprint>,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Risk label for the action.
    pub risk_label: ProposalRiskLabel,
    /// Redaction hints for the action summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Action summary schema version.
    pub schema_version: u16,
}

/// Permission-budget evaluation disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionBudgetEvaluationDisposition {
    /// Metadata-only contract evaluation allows the action class.
    Allowed,
    /// Budget state denied the action class.
    RefusedDenied,
    /// Budget usage is depleted or would exceed its ceiling.
    RefusedDepleted,
    /// Privacy scope or egress posture refused the action class.
    RefusedPrivacyScope,
    /// Consent is required, expired, or denied by policy.
    RefusedConsentRequired,
    /// The action class does not match the budget contract.
    RefusedActionClassMismatch,
}

/// Result of evaluating an action summary against a permission budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionBudgetEvaluation {
    /// Stable evaluation identifier.
    pub evaluation_id: String,
    /// Budget identifier used for the evaluation.
    pub budget_id: String,
    /// Action summary evaluated against the budget.
    pub action: PermissionBudgetActionSummary,
    /// Evaluation disposition.
    pub disposition: PermissionBudgetEvaluationDisposition,
    /// Budget state observed during evaluation.
    pub state: PermissionBudgetState,
    /// Whether the contract allowed the action class.
    pub allowed: bool,
    /// Usage summary after an allowed action, or observed usage for refusals.
    pub usage_after: PermissionBudgetUsageSummary,
    /// Explicit refusal metadata for denied or depleted evaluations.
    pub refusal: Option<PrivacyInspectorRefusal>,
    /// Metadata-only reason labels.
    pub reasons: Vec<String>,
    /// Redaction hints for the evaluation.
    pub redaction_hints: Vec<RedactionHint>,
    /// Evaluation schema version.
    pub schema_version: u16,
}

/// Static permission-budget projection consumed by projection-only UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionBudgetProjection {
    /// Stable permission-budget projection identifier.
    pub projection_id: String,
    /// Budget contracts displayed by the trust surface.
    pub budgets: Vec<PermissionBudgetContract>,
    /// Recent metadata-only evaluations displayed by the trust surface.
    pub evaluations: Vec<PermissionBudgetEvaluation>,
    /// Number of denied budgets.
    pub denied_budget_count: u32,
    /// Number of depleted budgets.
    pub depleted_budget_count: u32,
    /// Number of refused evaluations.
    pub refused_evaluation_count: u32,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints that apply to the projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

/// Trust-layer approval checklist gate represented as metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalChecklistGateKind {
    /// Context manifest completeness and proposal linkage.
    ContextManifestCompleteness,
    /// Privacy-inspector refusal and redaction posture.
    PrivacyInspection,
    /// Permission-budget evaluation and consent posture.
    PermissionBudget,
    /// Proposal lifecycle readiness for approval.
    ProposalLifecycle,
    /// Affected-target coverage and target identity validation.
    TargetValidation,
    /// Freshness and proposal precondition availability.
    FreshnessPreconditions,
    /// Audit-before-success metadata availability.
    AuditBeforeSuccess,
    /// Checkpoint and rollback availability.
    RollbackCheckpoint,
    /// Aggregate risk labels.
    RiskLabels,
    /// Explicit refusal, denial, or rejection reasons.
    ExplicitDenialReasons,
}

/// Trust-layer approval checklist gate status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalChecklistGateStatus {
    /// Gate is satisfied by metadata-only evidence.
    Satisfied,
    /// Gate blocks approval readiness.
    Blocked,
    /// Gate is non-blocking but carries visible risk metadata.
    Risk,
    /// Gate is not required for this metadata-only action.
    NotRequired,
    /// Gate evidence is missing or unknown.
    Unknown,
}

/// Metadata-only blocker or risk reason shown in an approval checklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalChecklistReason {
    /// Gate that produced the reason.
    pub gate: ApprovalChecklistGateKind,
    /// Stable reason code.
    pub reason_code: String,
    /// Display-safe reason label.
    pub label: String,
    /// Target identifier associated with the reason, when known.
    pub target_id: Option<String>,
    /// Budget identifier associated with the reason, when known.
    pub budget_id: Option<String>,
    /// Capability associated with the reason, when known.
    pub capability: Option<CapabilityId>,
    /// Risk label associated with the reason.
    pub risk_label: ProposalRiskLabel,
    /// Privacy label associated with the reason.
    pub privacy_label: ProposalPrivacyLabel,
    /// Redaction hints for the reason.
    pub redaction_hints: Vec<RedactionHint>,
    /// Reason DTO schema version.
    pub schema_version: u16,
}

/// Metadata-only summary for one approval checklist gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalChecklistGateSummary {
    /// Gate represented by this summary.
    pub gate: ApprovalChecklistGateKind,
    /// Gate status.
    pub status: ApprovalChecklistGateStatus,
    /// Highest risk observed for this gate.
    pub risk_label: ProposalRiskLabel,
    /// Highest privacy label observed for this gate.
    pub privacy_label: ProposalPrivacyLabel,
    /// Display-safe labels associated with the gate.
    pub labels: Vec<String>,
    /// Metadata-only blocker or risk reasons associated with this gate.
    pub reasons: Vec<ApprovalChecklistReason>,
    /// Redaction hints for the gate summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Gate summary schema version.
    pub schema_version: u16,
}

/// Metadata-only proposal approval checklist projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalApprovalChecklistProjection {
    /// Stable checklist projection identifier.
    pub checklist_id: String,
    /// Proposal represented by this checklist.
    pub proposal_id: ProposalId,
    /// Workspace represented by this checklist, when known.
    pub workspace_id: Option<WorkspaceId>,
    /// Payload kind represented without payload bodies.
    pub payload_kind: ProposalPayloadKind,
    /// Lifecycle state observed by the approval surface.
    pub lifecycle_state: ProposalLifecycleState,
    /// Proposal correlation id.
    pub correlation_id: CorrelationId,
    /// Optional causality id when audit/projection metadata has one.
    pub causality_id: Option<CausalityId>,
    /// True when every blocking gate is satisfied and no approval/apply side effect has occurred.
    pub ready_for_approval: bool,
    /// Gate summaries in deterministic order.
    pub gates: Vec<ApprovalChecklistGateSummary>,
    /// Flattened blocking reasons in deterministic order.
    pub blockers: Vec<ApprovalChecklistReason>,
    /// Aggregate risk labels represented by this checklist.
    pub risk_labels: Vec<ProposalRiskLabel>,
    /// Aggregate privacy labels represented by this checklist.
    pub privacy_labels: Vec<ProposalPrivacyLabel>,
    /// Explicit denial, refusal, or rejection reason codes.
    pub explicit_denial_reasons: Vec<String>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints that apply to the checklist.
    pub redaction_hints: Vec<RedactionHint>,
    /// Checklist projection schema version.
    pub schema_version: u16,
}

/// Metadata-only audit availability for checkpoint and rollback projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointRollbackAuditStatus {
    /// Audit metadata is available before success can be reported.
    Available,
    /// Audit metadata is expected but not yet available.
    Pending,
    /// Audit metadata is missing and should block approval or rollback claims.
    Missing,
    /// Audit metadata is not required for this metadata-only projection.
    NotRequired,
}

/// Metadata-only limitation attached to checkpoint or rollback availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRollbackLimitation {
    /// Stable limitation reason code.
    pub reason_code: String,
    /// Display-safe limitation label.
    pub label: String,
    /// Target identifier associated with the limitation, when known.
    pub target_id: Option<String>,
    /// Risk label associated with the limitation.
    pub risk_label: ProposalRiskLabel,
    /// Redaction hints for the limitation.
    pub redaction_hints: Vec<RedactionHint>,
    /// Limitation schema version.
    pub schema_version: u16,
}

/// Metadata-only affected target summary for checkpoint and rollback projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRollbackTargetSummary {
    /// Stable target identifier.
    pub target_id: String,
    /// Target class.
    pub kind: ProposalTargetKind,
    /// Workspace identifier when known.
    pub workspace_id: Option<WorkspaceId>,
    /// File identifier when known.
    pub file_id: Option<FileId>,
    /// Buffer identifier when known.
    pub buffer_id: Option<BufferId>,
    /// Terminal session identifier when known.
    pub terminal_session_id: Option<TerminalSessionId>,
    /// Plugin identifier when known.
    pub plugin_id: Option<PluginId>,
    /// Affected ranges represented without source bodies.
    pub ranges: Vec<ByteRange>,
    /// Hashes represented without source bodies.
    pub hashes: Vec<FileFingerprint>,
    /// Expected file content version represented by proposal preconditions.
    pub expected_file_content_version: Option<FileContentVersion>,
    /// Expected buffer version represented by proposal preconditions.
    pub expected_buffer_version: Option<BufferVersion>,
    /// Expected snapshot id represented by proposal preconditions.
    pub expected_snapshot_id: Option<SnapshotId>,
    /// Expected workspace generation represented by proposal preconditions.
    pub expected_workspace_generation: Option<WorkspaceGeneration>,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Redaction hints for the target summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Target summary schema version.
    pub schema_version: u16,
}

/// Metadata-only checkpoint projection for a proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalCheckpointProjection {
    /// Stable checkpoint identifier or deterministic metadata label.
    pub checkpoint_id: String,
    /// Whether checkpoint metadata says rollback can rely on this checkpoint.
    pub available: bool,
    /// Number of affected targets covered by the checkpoint metadata.
    pub target_count: u32,
    /// Proposal precondition summary represented without source bodies.
    pub expected_preconditions: ContextManifestPreconditionSummary,
    /// Hashes represented without source bodies.
    pub hashes: Vec<FileFingerprint>,
    /// Audit status for the checkpoint metadata.
    pub audit_status: CheckpointRollbackAuditStatus,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Metadata-only checkpoint limitations.
    pub limitations: Vec<CheckpointRollbackLimitation>,
    /// Redaction hints for the checkpoint projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Checkpoint projection schema version.
    pub schema_version: u16,
}

/// Metadata-only rollback projection for a proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalRollbackProjection {
    /// Rollback availability.
    pub availability: ProposalRollbackAvailability,
    /// Number of rollback step descriptors represented.
    pub rollback_step_count: u32,
    /// Number of targets that appear reversible from metadata.
    pub reversible_target_count: u32,
    /// Number of targets that appear irreversible or unsupported from metadata.
    pub irreversible_target_count: u32,
    /// Audit status for rollback metadata.
    pub audit_status: CheckpointRollbackAuditStatus,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Metadata-only rollback limitations.
    pub limitations: Vec<CheckpointRollbackLimitation>,
    /// Redaction hints for the rollback projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Rollback projection schema version.
    pub schema_version: u16,
}

/// Static checkpoint and rollback projection consumed by projection-only UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRollbackProjection {
    /// Stable checkpoint/rollback projection identifier.
    pub projection_id: String,
    /// Proposal represented by this projection.
    pub proposal_id: ProposalId,
    /// Workspace represented by this projection, when known.
    pub workspace_id: Option<WorkspaceId>,
    /// Payload kind represented without payload bodies.
    pub payload_kind: ProposalPayloadKind,
    /// Lifecycle state observed by the projection.
    pub lifecycle_state: ProposalLifecycleState,
    /// Proposal correlation id.
    pub correlation_id: CorrelationId,
    /// Optional causality id when audit/projection metadata has one.
    pub causality_id: Option<CausalityId>,
    /// Metadata-only checkpoint summary.
    pub checkpoint: ProposalCheckpointProjection,
    /// Metadata-only rollback summary.
    pub rollback: ProposalRollbackProjection,
    /// Affected targets represented without path bodies or source bodies.
    pub targets: Vec<CheckpointRollbackTargetSummary>,
    /// Aggregate risk labels represented by this projection.
    pub risk_labels: Vec<ProposalRiskLabel>,
    /// Aggregate privacy labels represented by this projection.
    pub privacy_labels: Vec<ProposalPrivacyLabel>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints that apply to this projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

// -----------------------------------------------------------------------------
// Assisted-AI boundary contracts
// -----------------------------------------------------------------------------

/// Provider execution class represented as metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiProviderClass {
    /// Provider runs inside the local process or local machine boundary.
    Local,
    /// Provider is reached through a local loopback endpoint for a local model runtime.
    LocalLoopback,
    /// Remote provider reached with user-managed credentials.
    ByokRemote,
    /// Hosted remote provider or gateway.
    HostedRemote,
    /// Future managed gateway; this contract does not authorize network egress.
    Gateway,
    /// Provider class is unknown and must be treated conservatively.
    Unknown,
}

/// Assisted-AI operation class labels used for policy and consent checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssistedAiOperationClass {
    /// Explain code or metadata with citations and no mutation-capable output.
    Explain,
    /// Produce a reviewable edit proposal payload.
    ProposeEdit,
    /// Produce structured metadata output without provider-specific payloads.
    StructuredMetadata,
    /// Produce embedding metadata labels only; no vector generation is activated by this DTO.
    EmbeddingMetadata,
    /// Produce reranking metadata labels only.
    RerankMetadata,
    /// Describe tool-planning support labels only; no tool execution is activated by this DTO.
    ToolPlanningMetadata,
}

/// Label-level support state for provider posture declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiSupportLabel {
    /// Feature is supported by metadata declaration.
    Supported,
    /// Feature is not supported by metadata declaration.
    Unsupported,
    /// Feature requires explicit approval or configuration before use.
    ApprovalRequired,
    /// Feature support is unknown and must be treated conservatively.
    Unknown,
}

/// Metadata-only provider availability state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiProviderAvailabilityState {
    /// Provider metadata says routing is available when all consent gates pass.
    Available,
    /// Provider is intentionally disabled.
    Disabled,
    /// Provider is refused by policy or consent posture.
    Refused,
    /// Provider is unavailable without fallback to unsafe routes.
    Unavailable,
}

/// Consent posture for an assisted-AI boundary evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiConsentState {
    /// Consent is granted for the requested metadata-only route boundary.
    Granted,
    /// Consent is not required for this metadata-only operation.
    NotRequired,
    /// Consent is missing and must block provider routing.
    Missing,
    /// Consent was denied.
    Denied,
    /// Consent exists but must be renewed before provider routing.
    RenewalRequired,
}

/// Request disposition after consent, privacy, trust, budget, and route checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiRequestDisposition {
    /// Metadata-only contract is ready for a future router; no call is encoded here.
    MetadataOnlyReady,
    /// Request is refused by an explicit consent, privacy, budget, trust, or egress gate.
    Refused,
    /// Provider or operation is disabled before routing.
    Disabled,
}

/// Provider invocation encoding state for this boundary slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiProviderInvocationState {
    /// No provider call, network request, tool execution, prompt payload, or runtime work is encoded.
    NotEncoded,
    /// Provider route is planned but has not been approved for invocation.
    Planned,
    /// Provider route passed policy checks and may be invoked by the owning runtime.
    PolicyApproved,
    /// Local provider invocation is in progress.
    InvokingLocalProvider,
    /// Provider invocation is producing bounded stream metadata.
    Streaming,
    /// Provider invocation completed; raw response payload is not stored here.
    Completed,
    /// Provider invocation was cancelled.
    Cancelled,
    /// Provider invocation failed with redacted metadata.
    Failed,
    /// Provider invocation was refused by policy, consent, privacy, budget, or availability.
    Refused,
}

/// Stable agent run identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentRunId(pub String);

/// Stable agent step identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentStepId(pub String);

/// Phase 4 agent run state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRunState {
    /// Runtime is collecting metadata-only context.
    Observing,
    /// Runtime is planning provider and proposal work.
    Planning,
    /// Runtime is preparing proposal-only output.
    Proposing,
    /// Runtime is waiting for explicit user approval.
    WaitingForApproval,
    /// Existing app/workspace authorities are applying an approved proposal.
    Applying,
    /// Runtime is verifying metadata-only outcome evidence.
    Verifying,
    /// Runtime is recovering from a failed or stale step.
    Recovering,
    /// Runtime is blocked by policy or missing preconditions.
    Blocked,
    /// Runtime was cancelled.
    Cancelled,
    /// Runtime completed.
    Completed,
    /// Runtime failed.
    Failed,
}

/// Phase 4 agent step state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStepState {
    /// Step is planned.
    Planned,
    /// Step is ready to execute through an owning runtime.
    Ready,
    /// Step is running.
    Running,
    /// Step is waiting for approval.
    WaitingForApproval,
    /// Step is blocked.
    Blocked,
    /// Step was cancelled.
    Cancelled,
    /// Step completed.
    Completed,
    /// Step failed.
    Failed,
}

/// Redacted provider route request used by Phase 4 runtime composition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiProviderRouteRequest {
    /// Stable route identifier.
    pub route_id: String,
    /// Provider identifier.
    pub provider_id: String,
    /// Display-safe model label.
    pub model_label: String,
    /// Provider class.
    pub provider_class: AssistedAiProviderClass,
    /// Operation class requested from the provider.
    pub operation_class: AssistedAiOperationClass,
    /// Context manifest reference required before invocation.
    pub context_manifest: AssistedAiTrustProjectionReference,
    /// Privacy inspector reference required before invocation.
    pub privacy_inspector: AssistedAiTrustProjectionReference,
    /// Permission-budget reference required before invocation.
    pub permission_budget: AssistedAiTrustProjectionReference,
    /// Proposal intent required to keep output proposal-only.
    pub proposal_intent: AssistedAiProposalTargetIntent,
    /// Capability decision id, if already decided.
    pub policy_decision_id: Option<CapabilityDecisionId>,
    /// Required provider capability.
    pub required_capability: CapabilityId,
    /// Optional network target metadata for loopback/egress policy.
    pub network_target: Option<NetworkTarget>,
    /// Cancellation token for in-flight work.
    pub cancellation_token: CancellationTokenId,
    /// Health metadata labels.
    pub health_labels: Vec<String>,
    /// Cost estimate metadata labels.
    pub cost_labels: Vec<String>,
    /// Principal requesting the route.
    pub principal_id: PrincipalId,
    /// Workspace trust state observed for the route.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence associated with the route.
    pub event_sequence: EventSequence,
    /// Redaction hints for route metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Route schema version.
    pub schema_version: u16,
}

/// Redacted provider route response used by Phase 4 runtime composition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiProviderRouteResponse {
    /// Stable route identifier.
    pub route_id: String,
    /// Provider invocation state after routing.
    pub invocation_state: AssistedAiProviderInvocationState,
    /// Route decision.
    pub route_decision: AssistedAiRouteDecision,
    /// Provider identifier.
    pub provider_id: String,
    /// Display-safe model label.
    pub model_label: String,
    /// Bounded output labels, never raw response text.
    pub output_labels: Vec<String>,
    /// Optional refusal metadata.
    pub refusal: Option<AssistedAiRefusalMetadata>,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence associated with the response.
    pub event_sequence: EventSequence,
    /// Redaction hints for response metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Response schema version.
    pub schema_version: u16,
}

/// Runtime-safe provider capability metadata for Phase 4 routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiRuntimeProviderCapability {
    /// Provider identifier.
    pub provider_id: String,
    /// Provider class.
    pub provider_class: AssistedAiProviderClass,
    /// Supports stream metadata.
    pub supports_streaming: bool,
    /// Supports structured proposal-producing output.
    pub supports_structured_output: bool,
    /// Embedding support label; vector persistence remains deferred.
    pub embeddings_label: String,
    /// Reranking support label; retrieval remains metadata-only.
    pub reranking_label: String,
    /// Tool-planning support label; tool execution is not authorized here.
    pub tool_planning_label: String,
    /// Context-window display label.
    pub context_window_label: String,
    /// Cost display label.
    pub cost_label: String,
    /// Cancellation support.
    pub supports_cancellation: bool,
    /// Health state label.
    pub health_state_label: String,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Schema version.
    pub schema_version: u16,
}

/// Structured output schema metadata without schema bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiStructuredOutputSchemaMetadata {
    /// Stable schema identifier.
    pub schema_id: String,
    /// Display-safe schema label.
    pub schema_label: String,
    /// Hash of the schema body stored or shipped elsewhere.
    pub schema_hash: FileFingerprint,
    /// Expected proposal payload kind.
    pub proposal_payload_kind: ProposalPayloadKind,
    /// Whether proposal preconditions are required.
    pub requires_proposal_preconditions: bool,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Schema version.
    pub schema_version: u16,
}

/// Structured output validation result metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiStructuredOutputValidationResult {
    /// Stable validation identifier.
    pub validation_id: String,
    /// Referenced schema identifier.
    pub schema_id: String,
    /// Whether validation passed.
    pub valid: bool,
    /// Display-safe reason labels.
    pub reason_labels: Vec<String>,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Schema version.
    pub schema_version: u16,
}

/// Agent runtime state transition record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStateTransitionRecord {
    /// Agent run identifier.
    pub run_id: AgentRunId,
    /// Optional step identifier.
    pub step_id: Option<AgentStepId>,
    /// Previous run state.
    pub from_state: AgentRunState,
    /// Next run state.
    pub to_state: AgentRunState,
    /// Metadata-only reason code.
    pub reason_code: String,
    /// Optional proposal link.
    pub proposal_id: Option<ProposalId>,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Schema version.
    pub schema_version: u16,
}

/// Agent replay manifest built from metadata-only records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentReplayManifest {
    /// Agent run identifier.
    pub run_id: AgentRunId,
    /// Transition records needed for replay.
    pub transitions: Vec<AgentStateTransitionRecord>,
    /// Context manifest references used by the run.
    pub context_manifests: Vec<AssistedAiTrustProjectionReference>,
    /// Provider route identifiers used by the run.
    pub provider_route_ids: Vec<String>,
    /// Proposal identifiers linked to the run.
    pub proposal_ids: Vec<ProposalId>,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Schema version.
    pub schema_version: u16,
}

/// Metadata-only Phase 4 runtime audit record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phase4RuntimeAuditRecord {
    /// Stable audit identifier.
    pub audit_id: String,
    /// Optional agent run identifier.
    pub run_id: Option<AgentRunId>,
    /// Optional agent step identifier.
    pub step_id: Option<AgentStepId>,
    /// Optional provider route identifier.
    pub provider_route_id: Option<String>,
    /// Runtime invocation state.
    pub invocation_state: AssistedAiProviderInvocationState,
    /// Outcome label.
    pub outcome_label: String,
    /// Display-safe metadata labels.
    pub labels: Vec<String>,
    /// Correlation identifier.
    pub correlation_id: CorrelationId,
    /// Causality identifier.
    pub causality_id: CausalityId,
    /// Event sequence.
    pub event_sequence: EventSequence,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Schema version.
    pub schema_version: u16,
}

/// Stable reference to a trust-layer projection by identifier and hash only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiTrustProjectionReference {
    /// Stable projection or manifest identifier.
    pub reference_id: String,
    /// Projection kind label.
    pub kind: AssistedAiTrustProjectionKind,
    /// Metadata hash for the referenced projection.
    pub projection_hash: FileFingerprint,
    /// Projection schema version observed by the caller.
    pub schema_version: u16,
}

/// Trust-layer projection kinds referenced by assisted-AI request contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiTrustProjectionKind {
    /// Context manifest projection reference.
    ContextManifest,
    /// Privacy inspector projection reference.
    PrivacyInspector,
    /// Permission budget projection reference.
    PermissionBudget,
    /// Proposal approval checklist projection reference.
    ProposalApprovalChecklist,
    /// Checkpoint and rollback projection reference.
    CheckpointRollback,
    /// Assisted-AI projection reference used by later delegated-task plan boundaries.
    AssistedAiProjection,
}

/// Metadata-only permission-budget evaluation reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiPermissionBudgetEvaluationReference {
    /// Stable evaluation identifier.
    pub evaluation_id: String,
    /// Stable budget identifier.
    pub budget_id: String,
    /// Evaluation disposition.
    pub disposition: PermissionBudgetEvaluationDisposition,
    /// Whether the referenced evaluation allowed the action.
    pub allowed: bool,
    /// Metadata hash for the referenced evaluation.
    pub evaluation_hash: FileFingerprint,
    /// Redaction hints for the evaluation reference.
    pub redaction_hints: Vec<RedactionHint>,
    /// Evaluation reference schema version.
    pub schema_version: u16,
}

/// Explicit refusal or disabled metadata for assisted-AI boundary checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiRefusalMetadata {
    /// Stable refusal reason code.
    pub reason_code: String,
    /// Display-safe refusal label.
    pub label: String,
    /// Provider identifier associated with the refusal.
    pub provider_id: Option<String>,
    /// Operation class associated with the refusal.
    pub operation_class: Option<AssistedAiOperationClass>,
    /// Privacy scope associated with the refusal.
    pub privacy_scope: Option<SemanticPrivacyScope>,
    /// Capability associated with the refusal, when known.
    pub capability: Option<CapabilityId>,
    /// Budget identifier associated with the refusal, when known.
    pub budget_id: Option<String>,
    /// Risk label for the refusal.
    pub risk_label: ProposalRiskLabel,
    /// Metadata-only reason labels.
    pub reasons: Vec<String>,
    /// Redaction hints for the refusal.
    pub redaction_hints: Vec<RedactionHint>,
    /// Refusal schema version.
    pub schema_version: u16,
}

/// Metadata-only provider capability and consent posture labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiProviderCapability {
    /// Stable provider identifier or redacted provider label.
    pub provider_id: String,
    /// Display-safe provider label.
    pub provider_label: String,
    /// Local, loopback, BYOK, remote, gateway, or unknown provider class.
    pub provider_class: AssistedAiProviderClass,
    /// Operation classes declared by this provider metadata.
    pub supported_operations: Vec<AssistedAiOperationClass>,
    /// Model capability labels without model prompts or provider payloads.
    pub model_capability_labels: Vec<String>,
    /// Tool capability labels without tool-call payloads or execution authority.
    pub tool_capability_labels: Vec<String>,
    /// Context-window label, such as small, medium, large, or redacted.
    pub context_window_label: String,
    /// Cost budget label, such as free, capped, paid, or unknown.
    pub cost_budget_label: String,
    /// Risk budget label, such as low, medium, high, or unknown.
    pub risk_budget_label: String,
    /// Privacy and retention posture label.
    pub privacy_retention_label: String,
    /// Bring-your-own-key support label.
    pub byok_support: AssistedAiSupportLabel,
    /// Local execution support label.
    pub local_execution_support: AssistedAiSupportLabel,
    /// Offline mode support label.
    pub offline_support: AssistedAiSupportLabel,
    /// Air-gap support label.
    pub air_gap_support: AssistedAiSupportLabel,
    /// Redaction requirements as display-safe labels.
    pub redaction_requirements: Vec<String>,
    /// Consent requirements as display-safe labels.
    pub consent_requirements: Vec<String>,
    /// Current availability state.
    pub availability: AssistedAiProviderAvailabilityState,
    /// Explicit disabled or refused metadata, when availability is not usable.
    pub refusal: Option<AssistedAiRefusalMetadata>,
    /// Redaction hints for the capability record.
    pub redaction_hints: Vec<RedactionHint>,
    /// Capability schema version.
    pub schema_version: u16,
}

/// Metadata-only consent boundary evaluated before assisted-AI routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiConsentBoundary {
    /// Stable consent boundary identifier.
    pub boundary_id: String,
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Observed workspace trust state.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Requested privacy scope.
    pub requested_privacy_scope: SemanticPrivacyScope,
    /// Whether privacy policy permits the requested scope.
    pub privacy_scope_allowed: bool,
    /// Consent state for this boundary.
    pub consent_state: AssistedAiConsentState,
    /// Permission-budget evaluations represented as metadata only.
    pub budget_evaluations: Vec<PermissionBudgetEvaluation>,
    /// Air-gap policy state.
    pub air_gap_mode: bool,
    /// Offline policy state.
    pub offline_mode: bool,
    /// Local-provider-only policy state.
    pub local_only_mode: bool,
    /// Required capability for the future provider route, if known.
    pub required_capability: Option<CapabilityId>,
    /// Metadata-only reason labels.
    pub reasons: Vec<String>,
    /// Redaction hints for the boundary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Boundary schema version.
    pub schema_version: u16,
}

/// Metadata-only provider route decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiRouteDecision {
    /// Request disposition after contract-level gates.
    pub disposition: AssistedAiRequestDisposition,
    /// Provider invocation state; this P6.1 slice only allows `NotEncoded`.
    pub provider_invocation: AssistedAiProviderInvocationState,
    /// Explicit refusal or disabled metadata when routing is blocked.
    pub refusal: Option<AssistedAiRefusalMetadata>,
    /// Metadata-only reason labels.
    pub reasons: Vec<String>,
    /// Redaction hints for the route decision.
    pub redaction_hints: Vec<RedactionHint>,
    /// Route-decision schema version.
    pub schema_version: u16,
}

/// Proposal-target intent metadata for assisted-AI request contracts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiProposalTargetIntent {
    /// Intended proposal payload kind.
    pub payload_kind: ProposalPayloadKind,
    /// Target coverage expected for any resulting proposal.
    pub target_coverage: ProposalTargetCoverage,
    /// Required capability for proposal creation or review.
    pub required_capability: CapabilityId,
    /// Risk label for the proposal intent.
    pub risk_label: ProposalRiskLabel,
    /// Privacy label for the proposal intent.
    pub privacy_label: ProposalPrivacyLabel,
    /// Display-safe intent labels.
    pub labels: Vec<String>,
    /// Redaction hints for the proposal intent.
    pub redaction_hints: Vec<RedactionHint>,
    /// Proposal intent schema version.
    pub schema_version: u16,
}

/// Assisted-AI request contract that references P5 trust projections by ids and hashes only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiRequestContract {
    /// Stable request identifier.
    pub request_id: String,
    /// Provider capability metadata.
    pub provider: AssistedAiProviderCapability,
    /// Requested operation class.
    pub operation_class: AssistedAiOperationClass,
    /// Context manifest projection reference.
    pub context_manifest: AssistedAiTrustProjectionReference,
    /// Privacy inspector projection reference.
    pub privacy_inspector: AssistedAiTrustProjectionReference,
    /// Permission budget projection reference.
    pub permission_budget_projection: AssistedAiTrustProjectionReference,
    /// Permission-budget evaluation references.
    pub permission_budget_evaluations: Vec<AssistedAiPermissionBudgetEvaluationReference>,
    /// Proposal approval checklist projection reference.
    pub approval_checklist: AssistedAiTrustProjectionReference,
    /// Optional checkpoint and rollback projection reference.
    pub checkpoint_rollback: Option<AssistedAiTrustProjectionReference>,
    /// Correlation identifier for audit continuity.
    pub correlation_id: CorrelationId,
    /// Causality identifier for audit continuity.
    pub causality_id: CausalityId,
    /// Proposal target intent metadata.
    pub proposal_intent: AssistedAiProposalTargetIntent,
    /// Route decision after contract-level gates.
    pub route_decision: AssistedAiRouteDecision,
    /// Request creation timestamp.
    pub created_at: TimestampMillis,
    /// Redaction hints for the request contract.
    pub redaction_hints: Vec<RedactionHint>,
    /// Request contract schema version.
    pub schema_version: u16,
}

/// Assisted-AI edit output constrained to proposal-only conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistedAiEditProposalOutput {
    /// Stable output identifier.
    pub output_id: String,
    /// Request contract identifier that produced the output metadata.
    pub request_id: String,
    /// Provider identifier associated with the output metadata.
    pub provider_id: String,
    /// Proposal identifier to use for the converted proposal.
    pub proposal_id: ProposalId,
    /// Principal that requested proposal creation.
    pub principal: PrincipalId,
    /// Capability required before mutation authority may apply the proposal.
    pub capability: CapabilityId,
    /// Correlation identifier preserved on the resulting proposal.
    pub correlation_id: CorrelationId,
    /// Causality identifier preserved in this output contract for audit continuity.
    pub causality_id: CausalityId,
    /// Proposal payload; bounded replacement text may appear only inside existing edit DTOs.
    pub payload: ProposalPayload,
    /// Version and fingerprint preconditions required before proposal use.
    pub preconditions: ProposalVersionPreconditions,
    /// Preview metadata.
    pub preview: PreviewSummary,
    /// Optional expiration timestamp.
    pub expires_at: Option<TimestampMillis>,
    /// Creation timestamp.
    pub created_at: TimestampMillis,
    /// Context manifest projection reference.
    pub context_manifest: AssistedAiTrustProjectionReference,
    /// Approval checklist projection reference.
    pub approval_checklist: AssistedAiTrustProjectionReference,
    /// Redaction hints for the output contract.
    pub redaction_hints: Vec<RedactionHint>,
    /// Output contract schema version.
    pub schema_version: u16,
}

/// Projection readiness for an assisted-AI proposal preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiProposalPreviewReadiness {
    /// Output metadata can be shown as a reviewable proposal preview.
    PreviewReady,
    /// Route decision refused or disabled the assisted-AI request.
    RouteRefused,
    /// Output preconditions or proposal metadata were invalid.
    InvalidOutput,
    /// Proposal ledger metadata was not present for the output proposal id.
    MissingProposalLedger,
}

/// Metadata-only visible provider capability summary for projection surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiProviderCapabilitySummary {
    /// Stable provider identifier or redacted provider label.
    pub provider_id: String,
    /// Display-safe provider label.
    pub provider_label: String,
    /// Provider execution class label.
    pub provider_class: AssistedAiProviderClass,
    /// Supported operation labels.
    pub supported_operations: Vec<AssistedAiOperationClass>,
    /// Number of supported operations.
    pub supported_operation_count: u32,
    /// Number of model capability labels.
    pub model_capability_label_count: u32,
    /// Number of tool capability labels.
    pub tool_capability_label_count: u32,
    /// Context-window label.
    pub context_window_label: String,
    /// Cost-budget label.
    pub cost_budget_label: String,
    /// Risk-budget label.
    pub risk_budget_label: String,
    /// Privacy and retention posture label.
    pub privacy_retention_label: String,
    /// Current availability state.
    pub availability: AssistedAiProviderAvailabilityState,
    /// Explicit refusal metadata when availability is not usable.
    pub refusal: Option<AssistedAiRefusalMetadata>,
    /// Risk label derived from availability and provider class metadata.
    pub risk_label: ProposalRiskLabel,
    /// Privacy label derived from provider class metadata.
    pub privacy_label: ProposalPrivacyLabel,
    /// Redaction hints for the provider summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Provider summary schema version.
    pub schema_version: u16,
}

/// Metadata-only route and consent decision summary for projection surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiRouteDecisionSummary {
    /// Assisted-AI request identifier.
    pub request_id: String,
    /// Provider identifier associated with the decision.
    pub provider_id: String,
    /// Requested operation class.
    pub operation_class: AssistedAiOperationClass,
    /// Route disposition.
    pub disposition: AssistedAiRequestDisposition,
    /// Provider invocation state; P6.2 remains `NotEncoded`.
    pub provider_invocation: AssistedAiProviderInvocationState,
    /// Correlation identifier for audit continuity.
    pub correlation_id: CorrelationId,
    /// Causality identifier for audit continuity.
    pub causality_id: CausalityId,
    /// Explicit refusal metadata when routing is blocked.
    pub refusal: Option<AssistedAiRefusalMetadata>,
    /// Number of permission-budget evaluation references represented by the request.
    pub permission_budget_evaluation_count: u32,
    /// Number of refused permission-budget evaluation references.
    pub refused_permission_budget_evaluation_count: u32,
    /// Risk label derived from route disposition and refusal metadata.
    pub risk_label: ProposalRiskLabel,
    /// Privacy label derived from provider class and proposal intent metadata.
    pub privacy_label: ProposalPrivacyLabel,
    /// Metadata-only reason labels.
    pub reasons: Vec<String>,
    /// Redaction hints for the route summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Route summary schema version.
    pub schema_version: u16,
}

/// Metadata-only request contract summary for projection surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiRequestContractSummary {
    /// Assisted-AI request identifier.
    pub request_id: String,
    /// Provider summary represented by labels and counts only.
    pub provider: AssistedAiProviderCapabilitySummary,
    /// Requested operation class.
    pub operation_class: AssistedAiOperationClass,
    /// Context manifest projection reference.
    pub context_manifest: AssistedAiTrustProjectionReference,
    /// Privacy inspector projection reference.
    pub privacy_inspector: AssistedAiTrustProjectionReference,
    /// Permission-budget projection reference.
    pub permission_budget_projection: AssistedAiTrustProjectionReference,
    /// Proposal approval checklist projection reference.
    pub approval_checklist: AssistedAiTrustProjectionReference,
    /// Optional checkpoint and rollback projection reference.
    pub checkpoint_rollback: Option<AssistedAiTrustProjectionReference>,
    /// Number of permission-budget evaluations referenced by this request.
    pub permission_budget_evaluation_count: u32,
    /// Number of refused permission-budget evaluations referenced by this request.
    pub refused_permission_budget_evaluation_count: u32,
    /// Intended proposal payload kind.
    pub proposal_payload_kind: ProposalPayloadKind,
    /// Intended target count.
    pub proposal_target_count: u32,
    /// Omitted target count.
    pub omitted_target_count: u32,
    /// Required capability for resulting proposal review or mutation authority.
    pub required_capability: CapabilityId,
    /// Route decision summary.
    pub route_decision: AssistedAiRouteDecisionSummary,
    /// Correlation identifier for audit continuity.
    pub correlation_id: CorrelationId,
    /// Causality identifier for audit continuity.
    pub causality_id: CausalityId,
    /// Request creation timestamp.
    pub created_at: TimestampMillis,
    /// Risk label from route and proposal intent metadata.
    pub risk_label: ProposalRiskLabel,
    /// Privacy label from route and proposal intent metadata.
    pub privacy_label: ProposalPrivacyLabel,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Redaction hints for the request summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Request summary schema version.
    pub schema_version: u16,
}

/// Metadata-only proposal preview summary for assisted-AI edit outputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiProposalPreviewSummary {
    /// Stable preview identifier.
    pub preview_id: String,
    /// Assisted-AI output identifier.
    pub output_id: String,
    /// Assisted-AI request identifier.
    pub request_id: String,
    /// Provider identifier associated with output metadata.
    pub provider_id: String,
    /// Proposal identifier for the reviewable proposal.
    pub proposal_id: ProposalId,
    /// Payload kind represented without payload bodies.
    pub payload_kind: ProposalPayloadKind,
    /// Lifecycle state observed in proposal ledger metadata, or `Created` when absent.
    pub lifecycle_state: ProposalLifecycleState,
    /// Preview readiness classification.
    pub readiness: AssistedAiProposalPreviewReadiness,
    /// True when this projection can show proposal preview metadata.
    pub ready_for_preview: bool,
    /// True when trust checklist metadata says the proposal is ready for user approval.
    pub ready_for_approval: bool,
    /// Always false in this projection-only slice; apply remains authority-owned elsewhere.
    pub ready_for_apply: bool,
    /// Correlation identifier preserved on the proposal.
    pub correlation_id: CorrelationId,
    /// Causality identifier preserved for audit continuity.
    pub causality_id: CausalityId,
    /// Context manifest projection reference.
    pub context_manifest: AssistedAiTrustProjectionReference,
    /// Approval checklist projection reference.
    pub approval_checklist: AssistedAiTrustProjectionReference,
    /// Optional checkpoint and rollback projection reference from the request.
    pub checkpoint_rollback: Option<AssistedAiTrustProjectionReference>,
    /// Proposal precondition summary represented without source bodies.
    pub preconditions: ContextManifestPreconditionSummary,
    /// Affected target coverage represented without source bodies.
    pub target_coverage: ProposalTargetCoverage,
    /// Diff summary represented without raw source bodies.
    pub diff_summary: ProposalDiffSummary,
    /// Trust projection references that contributed to this preview.
    pub trust_projection_references: Vec<AssistedAiTrustProjectionReference>,
    /// Whether a matching proposal ledger row was present.
    pub ledger_row_present: bool,
    /// Number of preview warnings represented by ledger/proposal metadata.
    pub preview_warning_count: u32,
    /// Refusal or invalid-output metadata when preview is not ready.
    pub refusal: Option<AssistedAiRefusalMetadata>,
    /// Risk label for the preview summary.
    pub risk_label: ProposalRiskLabel,
    /// Privacy label for the preview summary.
    pub privacy_label: ProposalPrivacyLabel,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Redaction hints for the proposal preview summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Preview summary schema version.
    pub schema_version: u16,
}

/// Static assisted-AI projection consumed by projection-only UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiProjection {
    /// Stable projection identifier.
    pub projection_id: String,
    /// Visible provider capability summaries.
    pub providers: Vec<AssistedAiProviderCapabilitySummary>,
    /// Route and consent summaries.
    pub routes: Vec<AssistedAiRouteDecisionSummary>,
    /// Request contract summaries.
    pub requests: Vec<AssistedAiRequestContractSummary>,
    /// Refusal states visible to users.
    pub refusals: Vec<AssistedAiRefusalMetadata>,
    /// Proposal preview summaries for assisted-AI edit outputs.
    pub proposal_previews: Vec<AssistedAiProposalPreviewSummary>,
    /// Number of providers represented.
    pub provider_count: u32,
    /// Number of requests represented.
    pub request_count: u32,
    /// Number of refused route or provider states represented.
    pub refusal_count: u32,
    /// Number of proposal previews that are reviewable.
    pub preview_ready_count: u32,
    /// Provider invocation state for this projection; P6.2 remains `NotEncoded`.
    pub provider_invocation: AssistedAiProviderInvocationState,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints that apply to the projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

// -----------------------------------------------------------------------------
// Delegated task planning boundary contracts
// -----------------------------------------------------------------------------

/// Stable delegated-task plan identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DelegatedTaskPlanId(pub String);

/// Stable delegated-task plan-step identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DelegatedTaskStepId(pub String);

/// Plan-only delegated-task operation class; this does not authorize runtime execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DelegatedTaskOperationClass {
    /// Read context-manifest metadata only.
    ReadContextMetadata,
    /// Query or cite semantic metadata only.
    QuerySemanticMetadata,
    /// Reference assisted-AI route or request metadata without provider invocation.
    ReferenceAssistedAiMetadata,
    /// Draft a proposal payload for later authority-owned validation.
    DraftProposalMetadata,
    /// Link an already-created proposal preview.
    LinkProposalPreview,
    /// Request human review or approval metadata.
    RequestHumanApproval,
    /// Check checkpoint metadata.
    CheckCheckpointMetadata,
    /// Check rollback readiness metadata.
    CheckRollbackReadiness,
    /// Summarize verification readiness metadata without running tools.
    SummarizeVerificationReadiness,
}

/// Trust gate required by delegated-task planning contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DelegatedTaskTrustGateKind {
    /// Workspace trust must be explicit.
    WorkspaceTrust,
    /// Context-manifest projection metadata must be present.
    ContextManifest,
    /// Privacy-inspector projection must allow the plan boundary.
    PrivacyInspector,
    /// Permission-budget projection must be available and non-depleted.
    PermissionBudget,
    /// Proposal approval checklist metadata must be present and valid.
    ApprovalChecklist,
    /// Checkpoint metadata must be present when required.
    Checkpoint,
    /// Rollback metadata must be present when required.
    Rollback,
    /// Assisted-AI projection metadata must be present when referenced.
    AssistedAiProjection,
    /// Non-zero correlation and non-nil causality metadata must be present.
    CorrelationCausality,
}

/// Plan-only delegated-task state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegatedTaskPlanState {
    /// Plan metadata has been drafted but not accepted by gates.
    Draft,
    /// Plan metadata is ready for review without execution.
    Planned,
    /// Plan awaits explicit approval metadata.
    AwaitingApproval,
    /// Plan is blocked by one or more required gates.
    Blocked,
    /// Plan is refused by privacy, permission, trust, or correlation gates.
    Refused,
    /// Plan was cancelled before any runtime activation.
    Cancelled,
}

/// Plan-step state for projection-only delegated-task summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegatedTaskStepState {
    /// Step is planned only.
    Planned,
    /// Step waits on approval or trust metadata.
    AwaitingApproval,
    /// Step is blocked by required gate metadata.
    Blocked,
    /// Step is refused by a fail-closed gate.
    Refused,
    /// Step is represented only as proposal-preview metadata.
    ProposalPreviewLinked,
}

/// Delegated-task runtime activation state; P7.1 only allows `NotEncoded`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegatedTaskRuntimeActivationState {
    /// No agent runtime, provider call, tool execution, terminal command, network request, or mutation is encoded.
    NotEncoded,
}

/// Projection readiness for delegated-task plan summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegatedTaskPlanReadinessStatus {
    /// Plan can be displayed as reviewable metadata only.
    PlanReady,
    /// Plan is blocked by missing or incomplete gate metadata.
    Blocked,
    /// Plan is refused by privacy, budget, trust, or correlation metadata.
    Refused,
}

/// Fine-grained delegated-task readiness classification for P7.2 audit linkage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegatedTaskReadinessClassification {
    /// Plan-only metadata is ready and encodes no runtime activation.
    PlanOnlyReady,
    /// Plan is blocked by missing or incomplete gate metadata.
    Blocked,
    /// Plan is refused by trust, privacy, budget, or core-id metadata.
    Refused,
    /// Plan is waiting for proposal-preview metadata to be linked.
    WaitingForProposalPreview,
    /// Plan is waiting for assisted-AI audit metadata to be linked.
    WaitingForAudit,
    /// Plan is waiting for user approval metadata; no approval or apply is encoded.
    WaitingForApproval,
    /// Plan or linkage metadata is invalid and must not progress.
    InvalidMetadata,
}

/// Metadata-only required gate status for a delegated-task plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskRequiredTrustGate {
    /// Gate kind.
    pub kind: DelegatedTaskTrustGateKind,
    /// Whether the gate is required for this plan.
    pub required: bool,
    /// Whether metadata proves the gate is currently satisfied.
    pub satisfied: bool,
    /// Optional projection reference that satisfied or failed this gate.
    pub projection_reference: Option<AssistedAiTrustProjectionReference>,
    /// Display-safe labels for this gate.
    pub labels: Vec<String>,
    /// Metadata-only reason labels.
    pub reasons: Vec<String>,
    /// Gate risk label.
    pub risk_label: ProposalRiskLabel,
    /// Gate privacy label.
    pub privacy_label: ProposalPrivacyLabel,
    /// Redaction hints for the gate.
    pub redaction_hints: Vec<RedactionHint>,
    /// Gate DTO schema version.
    pub schema_version: u16,
}

/// Metadata-only delegated-task blocker or refusal reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskPlanBlocker {
    /// Stable reason code.
    pub reason_code: String,
    /// Display-safe blocker label.
    pub label: String,
    /// Gate that produced the blocker.
    pub gate: DelegatedTaskTrustGateKind,
    /// Optional step associated with this blocker.
    pub step_id: Option<DelegatedTaskStepId>,
    /// Optional target identifier associated with this blocker.
    pub target_id: Option<String>,
    /// Optional proposal associated with this blocker.
    pub proposal_id: Option<ProposalId>,
    /// Optional capability associated with this blocker.
    pub capability: Option<CapabilityId>,
    /// Optional budget identifier associated with this blocker.
    pub budget_id: Option<String>,
    /// Optional trust projection reference associated with this blocker.
    pub projection_reference: Option<AssistedAiTrustProjectionReference>,
    /// Blocker risk label.
    pub risk_label: ProposalRiskLabel,
    /// Blocker privacy label.
    pub privacy_label: ProposalPrivacyLabel,
    /// Metadata-only reason labels.
    pub reasons: Vec<String>,
    /// Redaction hints for the blocker.
    pub redaction_hints: Vec<RedactionHint>,
    /// Blocker DTO schema version.
    pub schema_version: u16,
}

/// Metadata-only delegated-task target summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskAffectedTargetSummary {
    /// Stable target identifier.
    pub target_id: String,
    /// Target kind represented without source bodies.
    pub kind: ProposalTargetKind,
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Optional file identifier.
    pub file_id: Option<FileId>,
    /// Optional buffer identifier.
    pub buffer_id: Option<BufferId>,
    /// Bounded ranges represented without source bodies.
    pub ranges: Vec<ByteRange>,
    /// Hashes represented without raw payloads.
    pub hashes: Vec<FileFingerprint>,
    /// Metadata-only counts for the target.
    pub counts: Vec<ContextManifestItemCount>,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Target risk label.
    pub risk_label: ProposalRiskLabel,
    /// Target privacy label.
    pub privacy_label: ProposalPrivacyLabel,
    /// Redaction hints for the target.
    pub redaction_hints: Vec<RedactionHint>,
    /// Target DTO schema version.
    pub schema_version: u16,
}

/// Metadata-only proposal-preview link emitted by a delegated-task plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskProposalPreviewLink {
    /// Stable link identifier.
    pub link_id: String,
    /// Proposal identifier for preview only.
    pub proposal_id: ProposalId,
    /// Payload kind represented without payload bodies.
    pub payload_kind: ProposalPayloadKind,
    /// Proposal lifecycle state represented by projection metadata.
    pub lifecycle_state: ProposalLifecycleState,
    /// Optional approval-checklist reference.
    pub approval_checklist: Option<AssistedAiTrustProjectionReference>,
    /// Optional checkpoint/rollback reference.
    pub checkpoint_rollback: Option<AssistedAiTrustProjectionReference>,
    /// Target count represented by this link.
    pub target_count: u32,
    /// Hunk or change count represented by this link.
    pub hunk_count: u32,
    /// Whether full source is redacted in the preview.
    pub full_source_redacted: bool,
    /// Link risk label.
    pub risk_label: ProposalRiskLabel,
    /// Link privacy label.
    pub privacy_label: ProposalPrivacyLabel,
    /// Redaction hints for the link.
    pub redaction_hints: Vec<RedactionHint>,
    /// Link DTO schema version.
    pub schema_version: u16,
}

/// One metadata-only step in a delegated-task plan graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskPlanStep {
    /// Stable step identifier.
    pub step_id: DelegatedTaskStepId,
    /// Stable display order.
    pub order: u32,
    /// Summary hash for the step objective; no raw objective body is stored.
    pub objective_summary_hash: FileFingerprint,
    /// Step operation class.
    pub operation_class: DelegatedTaskOperationClass,
    /// Predecessor step identifiers.
    pub depends_on: Vec<DelegatedTaskStepId>,
    /// Required gate kinds for this step.
    pub required_gates: Vec<DelegatedTaskTrustGateKind>,
    /// Affected target identifiers.
    pub target_ids: Vec<String>,
    /// Optional proposal-preview link.
    pub proposal_preview: Option<DelegatedTaskProposalPreviewLink>,
    /// Step state.
    pub state: DelegatedTaskStepState,
    /// Step blockers.
    pub blockers: Vec<DelegatedTaskPlanBlocker>,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Metadata-only counts represented by this step.
    pub counts: Vec<ContextManifestItemCount>,
    /// Step risk label.
    pub risk_label: ProposalRiskLabel,
    /// Step privacy label.
    pub privacy_label: ProposalPrivacyLabel,
    /// Redaction hints for the step.
    pub redaction_hints: Vec<RedactionHint>,
    /// Step DTO schema version.
    pub schema_version: u16,
}

/// Metadata-only audit and readiness summary for a delegated-task plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskAuditReadinessStatus {
    /// Stable audit/readiness identifier.
    pub readiness_id: String,
    /// Plan readiness status.
    pub readiness: DelegatedTaskPlanReadinessStatus,
    /// Runtime activation state; P7.1 only allows `NotEncoded`.
    pub runtime_activation: DelegatedTaskRuntimeActivationState,
    /// Whether correlation and causality metadata are valid.
    pub correlation_causality_valid: bool,
    /// Number of blockers represented.
    pub blocker_count: u32,
    /// Number of refusals represented.
    pub refusal_count: u32,
    /// Number of proposal-preview links represented.
    pub proposal_preview_link_count: u32,
    /// Display-safe readiness labels.
    pub labels: Vec<String>,
    /// Redaction hints for the readiness status.
    pub redaction_hints: Vec<RedactionHint>,
    /// Readiness DTO schema version.
    pub schema_version: u16,
}

/// Metadata-only reference to an assisted-AI audit record used by delegated-task readiness linkage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskAssistedAiAuditReference {
    /// Stable assisted-AI audit identifier.
    pub audit_id: String,
    /// Hash of the assisted-AI audit metadata.
    pub audit_hash: FileFingerprint,
    /// Assisted-AI request contract identifier.
    pub request_contract_id: String,
    /// Hash of the request contract metadata.
    pub request_contract_hash: FileFingerprint,
    /// Optional assisted-AI projection identifier.
    pub projection_id: Option<String>,
    /// Optional assisted-AI projection hash.
    pub projection_hash: Option<FileFingerprint>,
    /// Optional proposal-preview identifier.
    pub preview_id: Option<String>,
    /// Optional proposal-preview metadata hash.
    pub preview_hash: Option<FileFingerprint>,
    /// Optional proposal identifier linked by preview metadata only.
    pub proposal_id: Option<ProposalId>,
    /// Assisted-AI outcome category.
    pub outcome_category: AssistedAiAuditOutcomeCategory,
    /// Assisted-AI audit event sequence.
    pub event_sequence: EventSequence,
    /// Audit redaction state.
    pub redaction_state: AssistedAiAuditRedactionState,
    /// Provider invocation state; P7.2 linkage only permits `NotEncoded`.
    pub runtime_invocation_state: AssistedAiProviderInvocationState,
    /// Reference schema version.
    pub schema_version: u16,
}

/// Metadata-only delegated-task readiness/audit linkage record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskAuditLinkageRecord {
    /// Stable linkage record identifier.
    pub linkage_id: String,
    /// Linked delegated-task plan identifier.
    pub plan_id: DelegatedTaskPlanId,
    /// Hash of the delegated-task plan metadata.
    pub plan_hash: FileFingerprint,
    /// Step identifiers represented by this linkage.
    pub step_ids: Vec<DelegatedTaskStepId>,
    /// Proposal-preview links represented without payload bodies.
    pub proposal_preview_links: Vec<DelegatedTaskProposalPreviewLink>,
    /// Trust projection references used by readiness classification.
    pub trust_projection_references: Vec<AssistedAiTrustProjectionReference>,
    /// Assisted-AI audit references used by readiness classification.
    pub assisted_ai_audit_references: Vec<DelegatedTaskAssistedAiAuditReference>,
    /// Proposal identifiers linked by preview or audit metadata only.
    pub proposal_ids: Vec<ProposalId>,
    /// Blockers visible to users.
    pub blockers: Vec<DelegatedTaskPlanBlocker>,
    /// Refusals visible to users.
    pub refusals: Vec<DelegatedTaskPlanBlocker>,
    /// Fine-grained readiness classification.
    pub readiness_classification: DelegatedTaskReadinessClassification,
    /// Correlation identifier for audit continuity.
    pub correlation_id: CorrelationId,
    /// Causality identifier for audit continuity.
    pub causality_id: CausalityId,
    /// Event sequence for this linkage record.
    pub event_sequence: EventSequence,
    /// Risk labels represented by this linkage.
    pub risk_labels: Vec<ProposalRiskLabel>,
    /// Privacy labels represented by this linkage.
    pub privacy_labels: Vec<ProposalPrivacyLabel>,
    /// Runtime activation state; P7.2 only permits `NotEncoded`.
    pub runtime_activation: DelegatedTaskRuntimeActivationState,
    /// Runtime activation labels proving no provider, agent, tool, terminal, or mutation runtime started.
    pub runtime_activation_labels: Vec<String>,
    /// Redaction hints for persisted fields.
    pub redaction_hints: Vec<RedactionHint>,
    /// Linkage record schema version.
    pub schema_version: u16,
}

impl DelegatedTaskAuditLinkageRecord {
    /// Validates this delegated-task linkage as metadata-only and core-ID safe.
    pub fn validate(&self) -> Result<(), AssistedAiContractError> {
        validate_delegated_task_audit_linkage_record(self)
    }
}

/// Metadata-only delegated-task planning contract; this is plan-only and encodes no execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskPlanContract {
    /// Stable plan identifier.
    pub plan_id: DelegatedTaskPlanId,
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Objective summary hash; no raw objective text is stored.
    pub objective_summary_hash: FileFingerprint,
    /// Allowed plan-only operation classes.
    pub allowed_operation_classes: Vec<DelegatedTaskOperationClass>,
    /// Required trust gates with projection references where available.
    pub required_trust_gates: Vec<DelegatedTaskRequiredTrustGate>,
    /// Context manifest projection reference.
    pub context_manifest: Option<AssistedAiTrustProjectionReference>,
    /// Privacy inspector projection reference.
    pub privacy_inspector: Option<AssistedAiTrustProjectionReference>,
    /// Permission budget projection reference.
    pub permission_budget_projection: Option<AssistedAiTrustProjectionReference>,
    /// Proposal approval checklist projection reference.
    pub approval_checklist: Option<AssistedAiTrustProjectionReference>,
    /// Checkpoint/rollback projection reference.
    pub checkpoint_rollback: Option<AssistedAiTrustProjectionReference>,
    /// Assisted-AI projection reference.
    pub assisted_ai_projection: Option<AssistedAiTrustProjectionReference>,
    /// Metadata-only affected target summaries.
    pub affected_targets: Vec<DelegatedTaskAffectedTargetSummary>,
    /// Proposed step graph represented as metadata only.
    pub steps: Vec<DelegatedTaskPlanStep>,
    /// Proposal-preview links referenced by this plan.
    pub proposal_preview_links: Vec<DelegatedTaskProposalPreviewLink>,
    /// Aggregate risk labels.
    pub risk_labels: Vec<ProposalRiskLabel>,
    /// Aggregate privacy labels.
    pub privacy_labels: Vec<ProposalPrivacyLabel>,
    /// Whether checkpoint metadata is required.
    pub checkpoint_required: bool,
    /// Whether rollback metadata is required.
    pub rollback_required: bool,
    /// Plan state after contract-level gate evaluation.
    pub plan_state: DelegatedTaskPlanState,
    /// Plan blockers.
    pub blockers: Vec<DelegatedTaskPlanBlocker>,
    /// Refusal metadata.
    pub refusals: Vec<DelegatedTaskPlanBlocker>,
    /// Audit/readiness status.
    pub audit_readiness: DelegatedTaskAuditReadinessStatus,
    /// Correlation identifier for audit continuity.
    pub correlation_id: CorrelationId,
    /// Causality identifier for audit continuity.
    pub causality_id: CausalityId,
    /// Plan creation timestamp.
    pub created_at: TimestampMillis,
    /// Redaction hints for the plan contract.
    pub redaction_hints: Vec<RedactionHint>,
    /// Contract schema version.
    pub schema_version: u16,
}

/// Metadata-only inputs for delegated-task plan gate evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskPlanningBoundaryInput {
    /// Stable plan identifier.
    pub plan_id: DelegatedTaskPlanId,
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Objective summary hash; no raw objective text is stored.
    pub objective_summary_hash: FileFingerprint,
    /// Allowed plan-only operation classes.
    pub allowed_operation_classes: Vec<DelegatedTaskOperationClass>,
    /// Context manifest projection reference.
    pub context_manifest: Option<AssistedAiTrustProjectionReference>,
    /// Privacy inspector projection reference.
    pub privacy_inspector: Option<AssistedAiTrustProjectionReference>,
    /// Permission budget projection reference.
    pub permission_budget_projection: Option<AssistedAiTrustProjectionReference>,
    /// Proposal approval checklist projection reference.
    pub approval_checklist: Option<AssistedAiTrustProjectionReference>,
    /// Checkpoint/rollback projection reference.
    pub checkpoint_rollback: Option<AssistedAiTrustProjectionReference>,
    /// Assisted-AI projection reference.
    pub assisted_ai_projection: Option<AssistedAiTrustProjectionReference>,
    /// Whether assisted-AI projection metadata is required by this plan.
    pub assisted_ai_required: bool,
    /// Metadata-only affected target summaries.
    pub affected_targets: Vec<DelegatedTaskAffectedTargetSummary>,
    /// Proposed step graph represented as metadata only.
    pub steps: Vec<DelegatedTaskPlanStep>,
    /// Proposal-preview links referenced by this plan.
    pub proposal_preview_links: Vec<DelegatedTaskProposalPreviewLink>,
    /// Workspace trust state observed by the boundary.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Whether privacy inspection refused the plan.
    pub privacy_denied: bool,
    /// Whether any permission budget is denied.
    pub permission_budget_denied: bool,
    /// Whether any required permission budget is depleted.
    pub permission_budget_depleted: bool,
    /// Whether approval-checklist metadata is valid and present.
    pub approval_checklist_valid: bool,
    /// Whether checkpoint metadata is required.
    pub checkpoint_required: bool,
    /// Whether checkpoint metadata is available.
    pub checkpoint_available: bool,
    /// Whether rollback metadata is required.
    pub rollback_required: bool,
    /// Whether rollback metadata is available.
    pub rollback_available: bool,
    /// Correlation identifier for audit continuity.
    pub correlation_id: CorrelationId,
    /// Causality identifier for audit continuity.
    pub causality_id: CausalityId,
    /// Plan creation timestamp.
    pub created_at: TimestampMillis,
    /// Input schema version.
    pub schema_version: u16,
}

/// Metadata-only delegated-task plan row for projection-only UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskPlanRow {
    /// Stable plan identifier.
    pub plan_id: DelegatedTaskPlanId,
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Objective summary hash.
    pub objective_summary_hash: FileFingerprint,
    /// Plan state.
    pub plan_state: DelegatedTaskPlanState,
    /// Readiness status.
    pub readiness: DelegatedTaskPlanReadinessStatus,
    /// Number of steps in the plan graph.
    pub step_count: u32,
    /// Number of affected targets represented.
    pub affected_target_count: u32,
    /// Number of blockers represented.
    pub blocker_count: u32,
    /// Number of refusals represented.
    pub refusal_count: u32,
    /// Number of proposal-preview links represented.
    pub proposal_preview_link_count: u32,
    /// Highest plan risk label.
    pub risk_label: ProposalRiskLabel,
    /// Highest plan privacy label.
    pub privacy_label: ProposalPrivacyLabel,
    /// Correlation identifier for audit continuity.
    pub correlation_id: CorrelationId,
    /// Causality identifier for audit continuity.
    pub causality_id: CausalityId,
    /// Runtime activation state.
    pub runtime_activation: DelegatedTaskRuntimeActivationState,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Redaction hints for the row.
    pub redaction_hints: Vec<RedactionHint>,
    /// Row schema version.
    pub schema_version: u16,
}

/// Metadata-only delegated-task step summary for projection-only UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskStepSummary {
    /// Stable step identifier.
    pub step_id: DelegatedTaskStepId,
    /// Owning plan identifier.
    pub plan_id: DelegatedTaskPlanId,
    /// Stable display order.
    pub order: u32,
    /// Objective summary hash.
    pub objective_summary_hash: FileFingerprint,
    /// Step operation class.
    pub operation_class: DelegatedTaskOperationClass,
    /// Step state.
    pub state: DelegatedTaskStepState,
    /// Number of dependencies.
    pub dependency_count: u32,
    /// Number of affected targets.
    pub target_count: u32,
    /// Optional proposal id linked by preview metadata.
    pub proposal_id: Option<ProposalId>,
    /// Number of blockers represented on this step.
    pub blocker_count: u32,
    /// Step risk label.
    pub risk_label: ProposalRiskLabel,
    /// Step privacy label.
    pub privacy_label: ProposalPrivacyLabel,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Redaction hints for the summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Summary schema version.
    pub schema_version: u16,
}

/// Projection-only delegated-task plan summaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTaskProjection {
    /// Stable projection identifier.
    pub projection_id: String,
    /// Plan rows.
    pub plan_rows: Vec<DelegatedTaskPlanRow>,
    /// Step summaries.
    pub step_summaries: Vec<DelegatedTaskStepSummary>,
    /// Blockers visible to users.
    pub blockers: Vec<DelegatedTaskPlanBlocker>,
    /// Refusals visible to users.
    pub refusals: Vec<DelegatedTaskPlanBlocker>,
    /// Required approval gates visible to users.
    pub required_approvals: Vec<DelegatedTaskRequiredTrustGate>,
    /// Proposal-preview links visible to users.
    pub proposal_preview_links: Vec<DelegatedTaskProposalPreviewLink>,
    /// Audit/readiness statuses.
    pub audit_readiness: Vec<DelegatedTaskAuditReadinessStatus>,
    /// Plan-only disclaimers as display-safe labels.
    pub plan_only_disclaimers: Vec<String>,
    /// Number of plans represented.
    pub plan_count: u32,
    /// Number of blocked plans represented.
    pub blocked_plan_count: u32,
    /// Number of refused plans represented.
    pub refused_plan_count: u32,
    /// Runtime activation state.
    pub runtime_activation: DelegatedTaskRuntimeActivationState,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints that apply to this projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

// -----------------------------------------------------------------------------
// P8.1 future-surface planning gates
// -----------------------------------------------------------------------------

/// Stable future-surface planning-gate identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FutureSurfaceGateId(pub String);

/// Future runtime surface class represented by P8.1 planning gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FutureSurfaceClass {
    /// Integrated terminal planning surface.
    Terminal,
    /// Sandboxed plugin host planning surface.
    Plugin,
    /// Collaboration and multi-user operation-log planning surface.
    Collaboration,
    /// Remote workspace authority and transport planning surface.
    RemoteWorkspace,
    /// Opt-in autonomous automation planning surface.
    Autonomy,
}

/// Operation classes visible to future-surface planning gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FutureSurfaceOperationClass {
    /// Metadata-only projection display.
    MetadataProjection,
    /// Future output is represented as a proposal payload or proposal-preview link only.
    ProposalOnlyEditOutput,
    /// Terminal command DTO seed represented without command text or process launch.
    TerminalCommandProposal,
    /// Plugin intent or host-call request represented as metadata only.
    PluginIntentProposal,
    /// Collaboration edit or operation represented as attributable proposal metadata.
    CollaborationEditProposal,
    /// Remote file operation represented as proposal metadata with preconditions.
    RemoteWorkspaceProposal,
    /// Autonomous plan output represented as reviewable proposal metadata only.
    AutonomousPlanProposal,
    /// Runtime process or PTY launch; P8.1 never authorizes this.
    RuntimeProcessLaunch,
    /// Runtime plugin host activation; P8.1 never authorizes this.
    RuntimePluginHost,
    /// Runtime collaboration session activation; P8.1 never authorizes this.
    RuntimeCollaborationSession,
    /// Runtime remote session activation; P8.1 never authorizes this.
    RuntimeRemoteSession,
    /// Runtime autonomous execution; P8.1 never authorizes this.
    RuntimeAutonomousExecution,
    /// Provider invocation; P8.1 never authorizes this.
    ProviderInvocation,
    /// Network egress; P8.1 never authorizes this.
    NetworkEgress,
    /// Direct editor mutation; all future outputs must instead become proposals.
    EditorMutation,
    /// Direct workspace or storage mutation; all future outputs must instead become proposals.
    WorkspaceOrStorageMutation,
}

impl FutureSurfaceOperationClass {
    /// Returns true when this operation class would require a runtime surface that P8.1 does not encode.
    pub fn requires_runtime_activation(self) -> bool {
        matches!(
            self,
            Self::RuntimeProcessLaunch
                | Self::RuntimePluginHost
                | Self::RuntimeCollaborationSession
                | Self::RuntimeRemoteSession
                | Self::RuntimeAutonomousExecution
                | Self::ProviderInvocation
                | Self::NetworkEgress
        )
    }

    /// Returns true when this operation class would bypass proposal-mediated mutation.
    pub fn is_direct_mutation(self) -> bool {
        matches!(
            self,
            Self::EditorMutation | Self::WorkspaceOrStorageMutation
        )
    }
}

/// Required planning-gate artifact status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FutureSurfaceRequirementStatus {
    /// Required artifact is missing.
    Missing,
    /// Artifact exists only as a draft and does not satisfy activation gates.
    Draft,
    /// Artifact is present but not accepted.
    Present,
    /// Artifact is accepted by the planning gate.
    Accepted,
    /// Artifact is not required for this metadata-only gate.
    NotRequired,
}

impl FutureSurfaceRequirementStatus {
    /// Returns true when the artifact satisfies P8.1 planning readiness.
    pub fn satisfies_gate(self) -> bool {
        matches!(self, Self::Accepted | Self::NotRequired)
    }
}

/// Future-surface runtime activation state; P8.1 only permits `NotEncoded`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FutureSurfaceRuntimeActivationState {
    /// No terminal process, plugin host, collaboration session, remote session, autonomous agent, provider call, network call, tool execution, or mutation runtime is encoded.
    NotEncoded,
}

/// Fine-grained planning-gate classification for future surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FutureSurfaceGateClassification {
    /// Planning metadata is blocked by missing non-policy/non-contract gate material.
    Blocked,
    /// Planning metadata is refused by privacy, budget, denied operation, or invalid core-id metadata.
    Refused,
    /// ADR, dependency policy, threat model, or phase-status policy metadata is required.
    PolicyRequired,
    /// Contract tests are required before readiness can be claimed.
    ContractTestsRequired,
    /// Workspace trust or trust-layer projection metadata is required.
    TrustRequired,
    /// Proposal-only future edit/output metadata is ready for display; no runtime is encoded.
    ProposalOnlyReady,
    /// Requested operation would require runtime activation, which remains intentionally unencoded.
    RuntimeNotEncoded,
}

/// Planning-gate blocker/refusal category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FutureSurfaceBlockerCategory {
    /// ADR, policy, threat model, or phase ledger metadata is missing or not accepted.
    PolicyRequirement,
    /// Contract tests are missing or not accepted.
    ContractTestRequirement,
    /// Workspace trust or trust-projection metadata is missing.
    TrustRequirement,
    /// Privacy gates denied the planning boundary.
    PrivacyRefusal,
    /// Permission, cost, or risk budget denied or depleted the boundary.
    BudgetRefusal,
    /// Approval checklist metadata is required but not available.
    ApprovalRequirement,
    /// Checkpoint metadata is required but not available.
    CheckpointRequirement,
    /// Rollback metadata is required but not available.
    RollbackRequirement,
    /// Proposal-only mutation route is required but unavailable.
    ProposalOnlyRequirement,
    /// Requested operation is denied, not allowed, or directly mutating.
    OperationRefusal,
    /// Runtime activation is intentionally not encoded.
    RuntimeNotEncoded,
    /// Core id, schema, or metadata-only validation failed.
    MetadataInvalid,
}

/// Metadata-only blocker or refusal reason for a future-surface planning gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FutureSurfaceGateReason {
    /// Stable reason code.
    pub reason_code: String,
    /// Display-safe label.
    pub label: String,
    /// Reason category.
    pub category: FutureSurfaceBlockerCategory,
    /// Operation class associated with this reason, when known.
    pub operation_class: Option<FutureSurfaceOperationClass>,
    /// Capability associated with this reason, when known.
    pub capability: Option<CapabilityId>,
    /// Budget identifier associated with this reason, when known.
    pub budget_id: Option<String>,
    /// Risk label for this reason.
    pub risk_label: ProposalRiskLabel,
    /// Privacy label for this reason.
    pub privacy_label: ProposalPrivacyLabel,
    /// Metadata-only reason labels.
    pub reasons: Vec<String>,
    /// Redaction hints for this reason.
    pub redaction_hints: Vec<RedactionHint>,
    /// Reason schema version.
    pub schema_version: u16,
}

/// Metadata-only input for evaluating one future-surface planning gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FutureSurfacePlanningGateInput {
    /// Stable gate identifier.
    pub gate_id: FutureSurfaceGateId,
    /// Future surface class.
    pub surface_class: FutureSurfaceClass,
    /// Operation classes allowed by this planning gate.
    pub allowed_operation_classes: Vec<FutureSurfaceOperationClass>,
    /// Operation classes denied by this planning gate.
    pub denied_operation_classes: Vec<FutureSurfaceOperationClass>,
    /// Requested operation classes for this metadata-only evaluation.
    pub requested_operation_classes: Vec<FutureSurfaceOperationClass>,
    /// ADR acceptance status.
    pub adr_status: FutureSurfaceRequirementStatus,
    /// Dependency-policy status.
    pub dependency_policy_status: FutureSurfaceRequirementStatus,
    /// Contract-test status.
    pub contract_test_status: FutureSurfaceRequirementStatus,
    /// Threat-model status.
    pub threat_model_status: FutureSurfaceRequirementStatus,
    /// Phase-status ledger status.
    pub phase_status_entry_status: FutureSurfaceRequirementStatus,
    /// Workspace trust state observed by the planning gate.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Whether privacy inspection refused the planning boundary.
    pub privacy_denied: bool,
    /// Whether permission budget metadata denied the planning boundary.
    pub permission_budget_denied: bool,
    /// Whether permission budget metadata is depleted.
    pub permission_budget_depleted: bool,
    /// Whether approval checklist metadata is required.
    pub approval_required: bool,
    /// Whether approval checklist metadata is available.
    pub approval_available: bool,
    /// Whether checkpoint metadata is required.
    pub checkpoint_required: bool,
    /// Whether checkpoint metadata is available.
    pub checkpoint_available: bool,
    /// Whether rollback metadata is required.
    pub rollback_required: bool,
    /// Whether rollback metadata is available.
    pub rollback_available: bool,
    /// Whether proposal-only mutation is required.
    pub proposal_only_mutation_required: bool,
    /// Whether proposal-only mutation metadata is available.
    pub proposal_only_mutation_available: bool,
    /// Runtime activation state; P8.1 only permits `NotEncoded`.
    pub runtime_activation: FutureSurfaceRuntimeActivationState,
    /// Correlation identifier for audit continuity.
    pub correlation_id: CorrelationId,
    /// Causality identifier for audit continuity.
    pub causality_id: CausalityId,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Risk labels represented by this input.
    pub risk_labels: Vec<ProposalRiskLabel>,
    /// Privacy labels represented by this input.
    pub privacy_labels: Vec<ProposalPrivacyLabel>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Input schema version.
    pub schema_version: u16,
}

/// Metadata-only evaluated planning gate for a future surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FutureSurfacePlanningGate {
    /// Stable gate identifier.
    pub gate_id: FutureSurfaceGateId,
    /// Future surface class.
    pub surface_class: FutureSurfaceClass,
    /// Operation classes allowed by this planning gate.
    pub allowed_operation_classes: Vec<FutureSurfaceOperationClass>,
    /// Operation classes denied by this planning gate.
    pub denied_operation_classes: Vec<FutureSurfaceOperationClass>,
    /// Requested operation classes for this metadata-only evaluation.
    pub requested_operation_classes: Vec<FutureSurfaceOperationClass>,
    /// ADR acceptance status.
    pub adr_status: FutureSurfaceRequirementStatus,
    /// Dependency-policy status.
    pub dependency_policy_status: FutureSurfaceRequirementStatus,
    /// Contract-test status.
    pub contract_test_status: FutureSurfaceRequirementStatus,
    /// Threat-model status.
    pub threat_model_status: FutureSurfaceRequirementStatus,
    /// Phase-status ledger status.
    pub phase_status_entry_status: FutureSurfaceRequirementStatus,
    /// Workspace trust state observed by the planning gate.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Proposal-only mutation requirement.
    pub proposal_only_mutation_required: bool,
    /// Proposal-only mutation metadata availability.
    pub proposal_only_mutation_available: bool,
    /// Runtime activation state; P8.1 only permits `NotEncoded`.
    pub runtime_activation: FutureSurfaceRuntimeActivationState,
    /// Fine-grained classification.
    pub classification: FutureSurfaceGateClassification,
    /// True when proposal-only future edit/output metadata is ready without mutation.
    pub proposal_only_ready: bool,
    /// Blockers visible to users.
    pub blockers: Vec<FutureSurfaceGateReason>,
    /// Refusals visible to users.
    pub refusals: Vec<FutureSurfaceGateReason>,
    /// Correlation identifier for audit continuity.
    pub correlation_id: CorrelationId,
    /// Causality identifier for audit continuity.
    pub causality_id: CausalityId,
    /// Runtime activation labels proving no runtime started.
    pub runtime_activation_labels: Vec<String>,
    /// Display-safe labels.
    pub labels: Vec<String>,
    /// Risk labels represented by this gate.
    pub risk_labels: Vec<ProposalRiskLabel>,
    /// Privacy labels represented by this gate.
    pub privacy_labels: Vec<ProposalPrivacyLabel>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints for this gate.
    pub redaction_hints: Vec<RedactionHint>,
    /// Gate schema version.
    pub schema_version: u16,
}

impl FutureSurfacePlanningGate {
    /// Validates this planning gate as metadata-only and runtime-unencoded.
    pub fn validate(&self) -> Result<(), AssistedAiContractError> {
        validate_future_surface_planning_gate(self)
    }
}

/// Static future-surface gate projection consumed by projection-only UI snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FutureSurfaceGateProjection {
    /// Stable projection identifier.
    pub projection_id: String,
    /// Evaluated gate rows.
    pub gates: Vec<FutureSurfacePlanningGate>,
    /// Number of gates represented.
    pub gate_count: u32,
    /// Number of blocked gates represented.
    pub blocked_gate_count: u32,
    /// Number of refused gates represented.
    pub refused_gate_count: u32,
    /// Number of proposal-only ready gates represented.
    pub proposal_only_ready_gate_count: u32,
    /// Number of gates whose requested runtime remains unencoded.
    pub runtime_not_encoded_gate_count: u32,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints for this projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

/// Assisted-AI audit privacy disposition represented without source, prompt, or provider payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiAuditPrivacyDisposition {
    /// Privacy gates allowed the metadata-only route posture.
    Allowed,
    /// Privacy gates denied the route or preview.
    Denied,
    /// Sensitive metadata was redacted before audit storage.
    Redacted,
    /// Privacy disposition is unknown and must be treated conservatively.
    Unknown,
}

/// Assisted-AI audit outcome category represented as metadata-only reason codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiAuditOutcomeCategory {
    /// Route metadata is ready but provider invocation remains unencoded.
    RouteReadyNoInvocation,
    /// Route was refused before any provider invocation.
    RouteRefused,
    /// Consent denied, missing, or renewal-required blocked routing.
    ConsentDenied,
    /// Privacy policy denied the requested scope or redacted the metadata.
    PrivacyDenied,
    /// Permission, cost, or risk budget blocked routing.
    BudgetDenied,
    /// Output or proposal preconditions were invalid.
    InvalidPreconditions,
    /// Proposal preview metadata is ready for review without apply authority.
    ProposalPreviewReady,
    /// Proposal preview metadata is blocked or missing.
    ProposalPreviewBlocked,
    /// Provider is disabled or unavailable before invocation.
    ProviderDisabled,
    /// Provider error category metadata was captured without provider payloads.
    ProviderErrorMetadataOnly,
}

/// Assisted-AI audit redaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssistedAiAuditRedactionState {
    /// Audit record stores metadata, ids, hashes, labels, counts, and dispositions only.
    MetadataOnly,
    /// Audit record stores only fully redacted markers.
    FullyRedacted,
}

/// Metadata-only assisted-AI audit record for route, consent, preview, and proposal linkage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedAiAuditRecord {
    /// Stable audit record identifier.
    pub audit_id: String,
    /// Provider capability identifier or redacted provider label.
    pub provider_capability_id: String,
    /// Hash of provider capability metadata.
    pub provider_capability_hash: FileFingerprint,
    /// Stable route-decision metadata identifier.
    pub route_decision_id: String,
    /// Hash of route-decision metadata.
    pub route_decision_hash: FileFingerprint,
    /// Consent disposition when a consent boundary was available.
    pub consent_disposition: Option<AssistedAiConsentState>,
    /// Permission-budget dispositions referenced by this audit record.
    pub budget_dispositions: Vec<PermissionBudgetEvaluationDisposition>,
    /// Privacy disposition represented without source, prompt, or provider payloads.
    pub privacy_disposition: AssistedAiAuditPrivacyDisposition,
    /// Assisted-AI request contract identifier.
    pub request_contract_id: String,
    /// Hash of request contract metadata.
    pub request_contract_hash: FileFingerprint,
    /// Assisted-AI projection identifier when a projection contributed to the audit.
    pub projection_id: Option<String>,
    /// Hash of projection metadata when a projection contributed to the audit.
    pub projection_hash: Option<FileFingerprint>,
    /// Proposal-preview identifier when preview metadata contributed to the audit.
    pub preview_id: Option<String>,
    /// Hash of proposal-preview metadata when present.
    pub preview_hash: Option<FileFingerprint>,
    /// Proposal identifier when an assisted-AI output was converted to a reviewable proposal.
    pub proposal_id: Option<ProposalId>,
    /// Metadata-only outcome category for refusal, preview, or route readiness.
    pub outcome_category: AssistedAiAuditOutcomeCategory,
    /// Stable refusal or error category code when present.
    pub refusal_error_category: Option<String>,
    /// Correlation identifier for audit continuity.
    pub correlation_id: CorrelationId,
    /// Causality identifier for audit continuity.
    pub causality_id: CausalityId,
    /// Event sequence associated with this audit record.
    pub event_sequence: EventSequence,
    /// Risk labels represented by this audit record.
    pub risk_labels: Vec<ProposalRiskLabel>,
    /// Privacy labels represented by this audit record.
    pub privacy_labels: Vec<ProposalPrivacyLabel>,
    /// Redaction state for the audit record.
    pub redaction_state: AssistedAiAuditRedactionState,
    /// Runtime invocation state; P6.3 only permits `NotEncoded`.
    pub runtime_invocation_state: AssistedAiProviderInvocationState,
    /// Runtime activation labels proving no provider, network, tool, agent, or terminal runtime started.
    pub runtime_activation_labels: Vec<String>,
    /// Redaction hints for persisted fields.
    pub redaction_hints: Vec<RedactionHint>,
    /// Audit record schema version.
    pub schema_version: u16,
}

impl AssistedAiAuditRecord {
    /// Validates this audit record as metadata-only and core-ID safe.
    pub fn validate(&self) -> Result<(), AssistedAiContractError> {
        validate_assisted_ai_audit_record(self)
    }
}

/// Error raised by assisted-AI metadata-only contract helpers.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AssistedAiContractError {
    /// Correlation id was zero.
    #[error("assisted AI contract requires non-zero correlation id")]
    ZeroCorrelationId,
    /// Causality id was nil.
    #[error("assisted AI contract requires non-nil causality id")]
    NilCausalityId,
    /// Event sequence was zero.
    #[error("assisted AI audit record requires non-zero event sequence")]
    ZeroEventSequence,
    /// Required precondition metadata was missing.
    #[error("assisted AI proposal preconditions are incomplete: {reason}")]
    MissingPrecondition {
        /// Stable missing-precondition reason code.
        reason: String,
    },
    /// Proposal metadata was empty or invalid.
    #[error("assisted AI proposal metadata is invalid: {reason}")]
    InvalidProposalMetadata {
        /// Stable invalid-metadata reason code.
        reason: String,
    },
    /// Route decision was not allowed for proposal conversion.
    #[error("assisted AI route decision refused proposal conversion: {reason}")]
    RefusedRouteDecision {
        /// Stable refusal reason code.
        reason: String,
    },
    /// Assisted-AI audit metadata attempted to store forbidden raw or payload-like material.
    #[error("assisted AI audit metadata is not metadata-only: {field}: {reason}")]
    NonMetadataOnlyAuditRecord {
        /// Field or source that contained forbidden material.
        field: String,
        /// Stable validation reason code.
        reason: String,
    },
}

impl AssistedAiProviderCapability {
    /// Returns true when the provider class implies remote or external egress.
    pub fn is_remote_class(&self) -> bool {
        matches!(
            self.provider_class,
            AssistedAiProviderClass::ByokRemote
                | AssistedAiProviderClass::HostedRemote
                | AssistedAiProviderClass::Gateway
                | AssistedAiProviderClass::Unknown
        )
    }

    /// Returns true when the provider declares support for the requested operation class.
    pub fn supports_operation(&self, operation: AssistedAiOperationClass) -> bool {
        self.supported_operations.contains(&operation)
    }
}

impl AssistedAiPermissionBudgetEvaluationReference {
    /// Builds an evaluation reference from an existing metadata-only budget evaluation.
    pub fn from_evaluation(
        evaluation: &PermissionBudgetEvaluation,
        evaluation_hash: FileFingerprint,
        schema_version: u16,
    ) -> Self {
        Self {
            evaluation_id: evaluation.evaluation_id.clone(),
            budget_id: evaluation.budget_id.clone(),
            disposition: evaluation.disposition,
            allowed: evaluation.allowed,
            evaluation_hash,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        }
    }
}

impl AssistedAiRequestContract {
    /// Builds a metadata-only request contract after validating correlation and causality metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request_id: impl Into<String>,
        provider: AssistedAiProviderCapability,
        operation_class: AssistedAiOperationClass,
        context_manifest: AssistedAiTrustProjectionReference,
        privacy_inspector: AssistedAiTrustProjectionReference,
        permission_budget_projection: AssistedAiTrustProjectionReference,
        permission_budget_evaluations: Vec<AssistedAiPermissionBudgetEvaluationReference>,
        approval_checklist: AssistedAiTrustProjectionReference,
        checkpoint_rollback: Option<AssistedAiTrustProjectionReference>,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        proposal_intent: AssistedAiProposalTargetIntent,
        route_decision: AssistedAiRouteDecision,
        created_at: TimestampMillis,
        schema_version: u16,
    ) -> Result<Self, AssistedAiContractError> {
        validate_assisted_ai_correlation(correlation_id, causality_id)?;
        Ok(Self {
            request_id: request_id.into(),
            provider,
            operation_class,
            context_manifest,
            privacy_inspector,
            permission_budget_projection,
            permission_budget_evaluations,
            approval_checklist,
            checkpoint_rollback,
            correlation_id,
            causality_id,
            proposal_intent,
            route_decision,
            created_at,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        })
    }
}

impl AssistedAiEditProposalOutput {
    /// Converts this output into a workspace proposal without applying or approving it.
    pub fn to_workspace_proposal(&self) -> Result<WorkspaceProposal, AssistedAiContractError> {
        validate_assisted_ai_correlation(self.correlation_id, self.causality_id)?;
        validate_assisted_ai_proposal_preconditions(&self.preconditions)?;
        if self.principal.0.is_empty() {
            return Err(AssistedAiContractError::InvalidProposalMetadata {
                reason: "missing.principal".to_string(),
            });
        }
        if self.capability.0.is_empty() {
            return Err(AssistedAiContractError::InvalidProposalMetadata {
                reason: "missing.capability".to_string(),
            });
        }

        Ok(WorkspaceProposal {
            proposal_id: self.proposal_id,
            principal: self.principal.clone(),
            capability: self.capability.clone(),
            correlation_id: self.correlation_id,
            payload: self.payload.clone(),
            preconditions: self.preconditions.clone(),
            preview: self.preview.clone(),
            expires_at: self.expires_at,
            created_at: self.created_at,
        })
    }
}

impl AssistedAiProviderCapabilitySummary {
    /// Builds a metadata-only provider summary from a provider capability record.
    pub fn from_capability(provider: &AssistedAiProviderCapability, schema_version: u16) -> Self {
        let risk_label = if provider.availability == AssistedAiProviderAvailabilityState::Available
        {
            if provider.is_remote_class() {
                ProposalRiskLabel::Medium
            } else {
                ProposalRiskLabel::Low
            }
        } else {
            ProposalRiskLabel::High
        };
        let privacy_label = if provider.is_remote_class() {
            ProposalPrivacyLabel::ExternalEgressMetadata
        } else {
            ProposalPrivacyLabel::WorkspaceMetadata
        };
        Self {
            provider_id: provider.provider_id.clone(),
            provider_label: provider.provider_label.clone(),
            provider_class: provider.provider_class,
            supported_operations: provider.supported_operations.clone(),
            supported_operation_count: provider.supported_operations.len() as u32,
            model_capability_label_count: provider.model_capability_labels.len() as u32,
            tool_capability_label_count: provider.tool_capability_labels.len() as u32,
            context_window_label: provider.context_window_label.clone(),
            cost_budget_label: provider.cost_budget_label.clone(),
            risk_budget_label: provider.risk_budget_label.clone(),
            privacy_retention_label: provider.privacy_retention_label.clone(),
            availability: provider.availability,
            refusal: provider.refusal.clone(),
            risk_label,
            privacy_label,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        }
    }
}

impl AssistedAiRouteDecisionSummary {
    /// Builds a metadata-only route summary from a request contract.
    pub fn from_request(request: &AssistedAiRequestContract, schema_version: u16) -> Self {
        let refused_permission_budget_evaluation_count = request
            .permission_budget_evaluations
            .iter()
            .filter(|evaluation| !evaluation.allowed)
            .count() as u32;
        let risk_label = if request.route_decision.disposition
            == AssistedAiRequestDisposition::MetadataOnlyReady
        {
            max_risk_label(
                request.proposal_intent.risk_label,
                AssistedAiProviderCapabilitySummary::from_capability(
                    &request.provider,
                    schema_version,
                )
                .risk_label,
            )
        } else {
            ProposalRiskLabel::High
        };
        let privacy_label = max_privacy_label(
            request.proposal_intent.privacy_label,
            AssistedAiProviderCapabilitySummary::from_capability(&request.provider, schema_version)
                .privacy_label,
        );
        Self {
            request_id: request.request_id.clone(),
            provider_id: request.provider.provider_id.clone(),
            operation_class: request.operation_class,
            disposition: request.route_decision.disposition,
            provider_invocation: request.route_decision.provider_invocation,
            correlation_id: request.correlation_id,
            causality_id: request.causality_id,
            refusal: request.route_decision.refusal.clone(),
            permission_budget_evaluation_count: request.permission_budget_evaluations.len() as u32,
            refused_permission_budget_evaluation_count,
            risk_label,
            privacy_label,
            reasons: request.route_decision.reasons.clone(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        }
    }
}

impl AssistedAiRequestContractSummary {
    /// Builds a metadata-only request summary from an assisted-AI request contract.
    pub fn from_request(request: &AssistedAiRequestContract, schema_version: u16) -> Self {
        let provider =
            AssistedAiProviderCapabilitySummary::from_capability(&request.provider, schema_version);
        let route_decision = AssistedAiRouteDecisionSummary::from_request(request, schema_version);
        let risk_label = max_risk_label(
            request.proposal_intent.risk_label,
            route_decision.risk_label,
        );
        let privacy_label = max_privacy_label(
            request.proposal_intent.privacy_label,
            route_decision.privacy_label,
        );
        Self {
            request_id: request.request_id.clone(),
            provider,
            operation_class: request.operation_class,
            context_manifest: request.context_manifest.clone(),
            privacy_inspector: request.privacy_inspector.clone(),
            permission_budget_projection: request.permission_budget_projection.clone(),
            approval_checklist: request.approval_checklist.clone(),
            checkpoint_rollback: request.checkpoint_rollback.clone(),
            permission_budget_evaluation_count: request.permission_budget_evaluations.len() as u32,
            refused_permission_budget_evaluation_count: request
                .permission_budget_evaluations
                .iter()
                .filter(|evaluation| !evaluation.allowed)
                .count() as u32,
            proposal_payload_kind: request.proposal_intent.payload_kind,
            proposal_target_count: request.proposal_intent.target_coverage.targets.len() as u32,
            omitted_target_count: request.proposal_intent.target_coverage.omitted_target_count,
            required_capability: request.proposal_intent.required_capability.clone(),
            route_decision,
            correlation_id: request.correlation_id,
            causality_id: request.causality_id,
            created_at: request.created_at,
            risk_label,
            privacy_label,
            labels: request.proposal_intent.labels.clone(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        }
    }
}

/// Builds an assisted-AI proposal preview summary without applying, approving, or mutating state.
pub fn assisted_ai_proposal_preview_from_output(
    output: &AssistedAiEditProposalOutput,
    request: Option<&AssistedAiRequestContract>,
    ledger_projection: Option<&ProposalLedgerProjection>,
    approval_checklist_projection: Option<&ProposalApprovalChecklistProjection>,
    checkpoint_rollback_projection: Option<&CheckpointRollbackProjection>,
    schema_version: u16,
) -> AssistedAiProposalPreviewSummary {
    let proposal_result = output.to_workspace_proposal();
    let ledger_row = ledger_projection
        .and_then(|projection| proposal_ledger_row(projection, output.proposal_id));
    let request_route_ready = request.is_none_or(|request| {
        request.route_decision.disposition == AssistedAiRequestDisposition::MetadataOnlyReady
            && assisted_ai_provider_state_allows_proposal_preview(
                request.route_decision.provider_invocation,
            )
    });
    let readiness = if !request_route_ready {
        AssistedAiProposalPreviewReadiness::RouteRefused
    } else if proposal_result.is_err() {
        AssistedAiProposalPreviewReadiness::InvalidOutput
    } else if ledger_row.is_none() {
        AssistedAiProposalPreviewReadiness::MissingProposalLedger
    } else {
        AssistedAiProposalPreviewReadiness::PreviewReady
    };
    let ready_for_preview = readiness == AssistedAiProposalPreviewReadiness::PreviewReady;
    let lifecycle_state = ledger_row
        .map(|row| row.lifecycle.state)
        .unwrap_or(ProposalLifecycleState::Created);
    let payload_kind = proposal_result
        .as_ref()
        .map(|proposal| proposal_payload_kind(&proposal.payload))
        .unwrap_or_else(|_| proposal_payload_kind(&output.payload));
    let target_coverage = ledger_row
        .map(|row| row.target_coverage.clone())
        .or_else(|| {
            proposal_result
                .as_ref()
                .ok()
                .map(proposal_metadata_target_coverage)
        })
        .unwrap_or_else(empty_target_coverage);
    let diff_summary = ledger_row
        .map(|row| row.diff_summary.clone())
        .unwrap_or_else(|| diff_summary_from_payload_metadata(&output.payload, &target_coverage));
    let preconditions = ContextManifestPreconditionSummary::from_preconditions(
        &output.preconditions,
        schema_version,
    );
    let ready_for_approval = ready_for_preview
        && approval_checklist_projection.is_some_and(|checklist| {
            checklist.proposal_id == output.proposal_id && checklist.ready_for_approval
        });
    let mut trust_projection_references = vec![
        output.context_manifest.clone(),
        output.approval_checklist.clone(),
    ];
    if let Some(request) = request {
        trust_projection_references.push(request.privacy_inspector.clone());
        trust_projection_references.push(request.permission_budget_projection.clone());
        if let Some(checkpoint) = &request.checkpoint_rollback {
            trust_projection_references.push(checkpoint.clone());
        }
    }
    let checkpoint_rollback = request
        .and_then(|request| request.checkpoint_rollback.clone())
        .or_else(|| {
            checkpoint_rollback_projection.map(|projection| AssistedAiTrustProjectionReference {
                reference_id: projection.projection_id.clone(),
                kind: AssistedAiTrustProjectionKind::CheckpointRollback,
                projection_hash: FileFingerprint {
                    algorithm: "projection-id".to_string(),
                    value: projection.projection_id.clone(),
                },
                schema_version: projection.schema_version,
            })
        });
    let refusal = assisted_ai_preview_refusal(
        output,
        request,
        readiness,
        proposal_result.as_ref().err(),
        schema_version,
    );
    let risk_label = if ready_for_preview {
        ledger_row
            .map(|row| row.risk_label)
            .unwrap_or(preconditions.risk_label)
    } else {
        ProposalRiskLabel::High
    };
    let privacy_label = ledger_row
        .map(|row| row.privacy_label)
        .or_else(|| request.map(|request| request.proposal_intent.privacy_label))
        .unwrap_or(ProposalPrivacyLabel::WorkspaceMetadata);
    AssistedAiProposalPreviewSummary {
        preview_id: format!("assist:preview:{}", output.proposal_id.0),
        output_id: output.output_id.clone(),
        request_id: output.request_id.clone(),
        provider_id: output.provider_id.clone(),
        proposal_id: output.proposal_id,
        payload_kind,
        lifecycle_state,
        readiness,
        ready_for_preview,
        ready_for_approval,
        ready_for_apply: false,
        correlation_id: output.correlation_id,
        causality_id: output.causality_id,
        context_manifest: output.context_manifest.clone(),
        approval_checklist: output.approval_checklist.clone(),
        checkpoint_rollback,
        preconditions,
        target_coverage,
        diff_summary,
        trust_projection_references,
        ledger_row_present: ledger_row.is_some(),
        preview_warning_count: ledger_row
            .map(|row| row.preview_warnings.len() as u32)
            .unwrap_or(0),
        refusal,
        risk_label,
        privacy_label,
        labels: vec![
            "assisted_ai.proposal_preview.metadata_only".to_string(),
            "proposal.apply.not_encoded".to_string(),
        ],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Builds a static assisted-AI projection from metadata-only contracts and trust projections.
#[allow(clippy::too_many_arguments)]
pub fn assisted_ai_projection_from_metadata(
    projection_id: impl Into<String>,
    providers: Vec<AssistedAiProviderCapability>,
    requests: Vec<AssistedAiRequestContract>,
    edit_outputs: Vec<AssistedAiEditProposalOutput>,
    proposal_ledger_projection: Option<&ProposalLedgerProjection>,
    _context_manifest_projection: Option<&ContextManifestProjection>,
    _privacy_inspector_projection: Option<&PrivacyInspectorProjection>,
    _permission_budget_projection: Option<&PermissionBudgetProjection>,
    approval_checklist_projection: Option<&ProposalApprovalChecklistProjection>,
    checkpoint_rollback_projection: Option<&CheckpointRollbackProjection>,
    generated_at: TimestampMillis,
    schema_version: u16,
) -> AssistedAiProjection {
    let request_by_id = requests
        .iter()
        .map(|request| (request.request_id.as_str(), request))
        .collect::<HashMap<_, _>>();
    let providers = providers
        .iter()
        .map(|provider| {
            AssistedAiProviderCapabilitySummary::from_capability(provider, schema_version)
        })
        .collect::<Vec<_>>();
    let routes = requests
        .iter()
        .map(|request| AssistedAiRouteDecisionSummary::from_request(request, schema_version))
        .collect::<Vec<_>>();
    let request_summaries = requests
        .iter()
        .map(|request| AssistedAiRequestContractSummary::from_request(request, schema_version))
        .collect::<Vec<_>>();
    let proposal_previews = edit_outputs
        .iter()
        .map(|output| {
            assisted_ai_proposal_preview_from_output(
                output,
                request_by_id.get(output.request_id.as_str()).copied(),
                proposal_ledger_projection,
                approval_checklist_projection,
                checkpoint_rollback_projection,
                schema_version,
            )
        })
        .collect::<Vec<_>>();
    let refusals = assisted_ai_projection_refusals(&providers, &routes, &proposal_previews);
    let preview_ready_count = proposal_previews
        .iter()
        .filter(|preview| preview.ready_for_preview)
        .count() as u32;
    AssistedAiProjection {
        projection_id: projection_id.into(),
        provider_count: providers.len() as u32,
        request_count: request_summaries.len() as u32,
        refusal_count: refusals.len() as u32,
        preview_ready_count,
        providers,
        routes,
        requests: request_summaries,
        refusals,
        proposal_previews,
        provider_invocation: requests
            .iter()
            .map(|request| request.route_decision.provider_invocation)
            .find(|state| *state != AssistedAiProviderInvocationState::NotEncoded)
            .unwrap_or(AssistedAiProviderInvocationState::NotEncoded),
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn assisted_ai_provider_state_allows_proposal_preview(
    state: AssistedAiProviderInvocationState,
) -> bool {
    matches!(
        state,
        AssistedAiProviderInvocationState::NotEncoded
            | AssistedAiProviderInvocationState::PolicyApproved
            | AssistedAiProviderInvocationState::Completed
    )
}

/// Validates assisted-AI audit metadata without allowing runtime invocation or raw payload storage.
pub fn validate_assisted_ai_audit_record(
    record: &AssistedAiAuditRecord,
) -> Result<(), AssistedAiContractError> {
    validate_assisted_ai_correlation(record.correlation_id, record.causality_id)?;
    if record.event_sequence.0 == 0 {
        return Err(AssistedAiContractError::ZeroEventSequence);
    }
    if record.schema_version == 0 {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "schema_version".to_string(),
            reason: "schema.zero".to_string(),
        });
    }
    if record.runtime_invocation_state != AssistedAiProviderInvocationState::NotEncoded {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "runtime_invocation_state".to_string(),
            reason: "runtime.invocation_encoded".to_string(),
        });
    }
    if record.redaction_hints.contains(&RedactionHint::None) {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "redaction_hints".to_string(),
            reason: "redaction.none".to_string(),
        });
    }

    validate_assisted_ai_audit_string("audit_id", &record.audit_id)?;
    validate_assisted_ai_audit_string("provider_capability_id", &record.provider_capability_id)?;
    validate_assisted_ai_audit_string("route_decision_id", &record.route_decision_id)?;
    validate_assisted_ai_audit_string("request_contract_id", &record.request_contract_id)?;
    validate_assisted_ai_audit_fingerprint(
        "provider_capability_hash",
        &record.provider_capability_hash,
    )?;
    validate_assisted_ai_audit_fingerprint("route_decision_hash", &record.route_decision_hash)?;
    validate_assisted_ai_audit_fingerprint("request_contract_hash", &record.request_contract_hash)?;
    if let Some(projection_id) = &record.projection_id {
        validate_assisted_ai_audit_string("projection_id", projection_id)?;
    }
    if let Some(projection_hash) = &record.projection_hash {
        validate_assisted_ai_audit_fingerprint("projection_hash", projection_hash)?;
    }
    if let Some(preview_id) = &record.preview_id {
        validate_assisted_ai_audit_string("preview_id", preview_id)?;
    }
    if let Some(preview_hash) = &record.preview_hash {
        validate_assisted_ai_audit_fingerprint("preview_hash", preview_hash)?;
    }
    if let Some(category) = &record.refusal_error_category {
        validate_assisted_ai_audit_string("refusal_error_category", category)?;
    }
    for label in &record.runtime_activation_labels {
        validate_assisted_ai_audit_string("runtime_activation_labels", label)?;
    }
    Ok(())
}

/// Validates Phase 4 runtime audit metadata while preserving raw-payload redaction rules.
pub fn validate_phase4_runtime_audit_record(
    record: &Phase4RuntimeAuditRecord,
) -> Result<(), AssistedAiContractError> {
    validate_assisted_ai_correlation(record.correlation_id, record.causality_id)?;
    if record.event_sequence.0 == 0 {
        return Err(AssistedAiContractError::ZeroEventSequence);
    }
    if record.schema_version == 0 {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "schema_version".to_string(),
            reason: "schema.zero".to_string(),
        });
    }
    if record.redaction_hints.contains(&RedactionHint::None) {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "redaction_hints".to_string(),
            reason: "redaction.none".to_string(),
        });
    }

    validate_assisted_ai_audit_string("audit_id", &record.audit_id)?;
    validate_assisted_ai_audit_string("outcome_label", &record.outcome_label)?;
    if let Some(route_id) = &record.provider_route_id {
        validate_assisted_ai_audit_string("provider_route_id", route_id)?;
    }
    if let Some(run_id) = &record.run_id {
        validate_assisted_ai_audit_string("run_id", &run_id.0)?;
    }
    if let Some(step_id) = &record.step_id {
        validate_assisted_ai_audit_string("step_id", &step_id.0)?;
    }
    for label in &record.labels {
        validate_assisted_ai_audit_string("labels", label)?;
    }
    Ok(())
}

/// Validates an agent replay manifest without allowing raw provider, prompt, or source payloads.
pub fn validate_agent_replay_manifest(
    manifest: &AgentReplayManifest,
) -> Result<(), AssistedAiContractError> {
    validate_assisted_ai_correlation(manifest.correlation_id, manifest.causality_id)?;
    if manifest.event_sequence.0 == 0 {
        return Err(AssistedAiContractError::ZeroEventSequence);
    }
    if manifest.schema_version == 0 {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "schema_version".to_string(),
            reason: "schema.zero".to_string(),
        });
    }
    if manifest.redaction_hints.contains(&RedactionHint::None) {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "redaction_hints".to_string(),
            reason: "redaction.none".to_string(),
        });
    }
    validate_assisted_ai_audit_string("run_id", &manifest.run_id.0)?;
    for transition in &manifest.transitions {
        validate_assisted_ai_audit_string("transition.reason_code", &transition.reason_code)?;
        validate_assisted_ai_correlation(transition.correlation_id, transition.causality_id)?;
        if transition.event_sequence.0 == 0 {
            return Err(AssistedAiContractError::ZeroEventSequence);
        }
    }
    for reference in &manifest.context_manifests {
        validate_assisted_ai_audit_string(
            "context_manifest.reference_id",
            &reference.reference_id,
        )?;
        validate_assisted_ai_audit_fingerprint(
            "context_manifest.projection_hash",
            &reference.projection_hash,
        )?;
    }
    for route_id in &manifest.provider_route_ids {
        validate_assisted_ai_audit_string("provider_route_ids", route_id)?;
    }
    Ok(())
}

/// Validates a provider route request before a provider router may invoke runtime behavior.
pub fn validate_assisted_ai_provider_route_request(
    request: &AssistedAiProviderRouteRequest,
) -> Result<(), AssistedAiContractError> {
    validate_assisted_ai_correlation(request.correlation_id, request.causality_id)?;
    if request.event_sequence.0 == 0 {
        return Err(AssistedAiContractError::ZeroEventSequence);
    }
    if request.schema_version == 0 {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "schema_version".to_string(),
            reason: "schema.zero".to_string(),
        });
    }
    if request.redaction_hints.contains(&RedactionHint::None) {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "redaction_hints".to_string(),
            reason: "redaction.none".to_string(),
        });
    }
    if request.cancellation_token.0 == Uuid::nil() {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "cancellation_token".to_string(),
            reason: "cancellation.nil".to_string(),
        });
    }
    validate_assisted_ai_audit_string("route_id", &request.route_id)?;
    validate_assisted_ai_audit_string("provider_id", &request.provider_id)?;
    validate_assisted_ai_audit_string("model_label", &request.model_label)?;
    validate_assisted_ai_audit_string("required_capability", &request.required_capability.0)?;
    for label in &request.health_labels {
        validate_assisted_ai_audit_string("health_labels", label)?;
    }
    for label in &request.cost_labels {
        validate_assisted_ai_audit_string("cost_labels", label)?;
    }
    Ok(())
}

/// Validates delegated-task readiness/audit linkage metadata without allowing runtime activation.
pub fn validate_delegated_task_audit_linkage_record(
    record: &DelegatedTaskAuditLinkageRecord,
) -> Result<(), AssistedAiContractError> {
    validate_assisted_ai_correlation(record.correlation_id, record.causality_id)?;
    if record.event_sequence.0 == 0 {
        return Err(AssistedAiContractError::ZeroEventSequence);
    }
    if record.schema_version == 0 {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "schema_version".to_string(),
            reason: "schema.zero".to_string(),
        });
    }
    if record.runtime_activation != DelegatedTaskRuntimeActivationState::NotEncoded {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "runtime_activation".to_string(),
            reason: "runtime.activation_encoded".to_string(),
        });
    }
    if record.redaction_hints.contains(&RedactionHint::None) {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "redaction_hints".to_string(),
            reason: "redaction.none".to_string(),
        });
    }

    validate_assisted_ai_audit_string("linkage_id", &record.linkage_id)?;
    validate_assisted_ai_audit_string("plan_id", &record.plan_id.0)?;
    validate_assisted_ai_audit_fingerprint("plan_hash", &record.plan_hash)?;
    for step_id in &record.step_ids {
        validate_assisted_ai_audit_string("step_ids", &step_id.0)?;
    }
    for trust_ref in &record.trust_projection_references {
        validate_assisted_ai_audit_string(
            "trust_projection.reference_id",
            &trust_ref.reference_id,
        )?;
        validate_assisted_ai_audit_fingerprint(
            "trust_projection.projection_hash",
            &trust_ref.projection_hash,
        )?;
        if trust_ref.schema_version == 0 {
            return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
                field: "trust_projection.schema_version".to_string(),
                reason: "schema.zero".to_string(),
            });
        }
    }
    for preview in &record.proposal_preview_links {
        validate_delegated_task_preview_link(preview)?;
    }
    for audit_ref in &record.assisted_ai_audit_references {
        validate_delegated_task_audit_reference(audit_ref)?;
    }
    for blocker in record.blockers.iter().chain(record.refusals.iter()) {
        validate_delegated_task_blocker_metadata(blocker)?;
    }
    for label in &record.runtime_activation_labels {
        validate_assisted_ai_audit_string("runtime_activation_labels", label)?;
    }
    Ok(())
}

/// Classifies a delegated-task plan using metadata-only readiness and audit references.
pub fn classify_delegated_task_readiness(
    plan: &DelegatedTaskPlanContract,
    audit_references: &[DelegatedTaskAssistedAiAuditReference],
) -> DelegatedTaskReadinessClassification {
    if plan.schema_version == 0
        || plan.audit_readiness.schema_version == 0
        || plan.correlation_id.0 == 0
        || plan.causality_id.0 == Uuid::nil()
        || plan.audit_readiness.runtime_activation
            != DelegatedTaskRuntimeActivationState::NotEncoded
        || audit_references.iter().any(|audit| {
            audit.schema_version == 0
                || audit.event_sequence.0 == 0
                || audit.redaction_state != AssistedAiAuditRedactionState::MetadataOnly
                || audit.runtime_invocation_state != AssistedAiProviderInvocationState::NotEncoded
        })
    {
        return DelegatedTaskReadinessClassification::InvalidMetadata;
    }
    if !plan.refusals.is_empty() || plan.plan_state == DelegatedTaskPlanState::Refused {
        return DelegatedTaskReadinessClassification::Refused;
    }
    if !plan.blockers.is_empty() || plan.plan_state == DelegatedTaskPlanState::Blocked {
        return DelegatedTaskReadinessClassification::Blocked;
    }
    let proposal_preview_required = plan.steps.iter().any(|step| {
        matches!(
            step.operation_class,
            DelegatedTaskOperationClass::DraftProposalMetadata
                | DelegatedTaskOperationClass::LinkProposalPreview
        )
    });
    if proposal_preview_required && plan.proposal_preview_links.is_empty() {
        return DelegatedTaskReadinessClassification::WaitingForProposalPreview;
    }
    let assisted_ai_required =
        plan.required_trust_gates.iter().any(|gate| {
            gate.kind == DelegatedTaskTrustGateKind::AssistedAiProjection && gate.required
        }) || plan.assisted_ai_projection.is_some()
            || plan.steps.iter().any(|step| {
                step.operation_class == DelegatedTaskOperationClass::ReferenceAssistedAiMetadata
            });
    if assisted_ai_required && audit_references.is_empty() {
        return DelegatedTaskReadinessClassification::WaitingForAudit;
    }
    if plan.plan_state == DelegatedTaskPlanState::AwaitingApproval
        || plan.proposal_preview_links.iter().any(|link| {
            !matches!(
                link.lifecycle_state,
                ProposalLifecycleState::Approved | ProposalLifecycleState::Applied
            )
        })
    {
        return DelegatedTaskReadinessClassification::WaitingForApproval;
    }
    DelegatedTaskReadinessClassification::PlanOnlyReady
}

/// Builds a metadata-only delegated-task readiness/audit linkage record.
pub fn delegated_task_audit_linkage_record(
    linkage_id: impl Into<String>,
    plan: &DelegatedTaskPlanContract,
    plan_hash: FileFingerprint,
    audit_references: Vec<DelegatedTaskAssistedAiAuditReference>,
    event_sequence: EventSequence,
    schema_version: u16,
) -> Result<DelegatedTaskAuditLinkageRecord, AssistedAiContractError> {
    let trust_projection_references = delegated_task_trust_projection_references(plan);
    let proposal_ids = delegated_task_linked_proposal_ids(plan, &audit_references);
    let record = DelegatedTaskAuditLinkageRecord {
        linkage_id: linkage_id.into(),
        plan_id: plan.plan_id.clone(),
        plan_hash,
        step_ids: plan.steps.iter().map(|step| step.step_id.clone()).collect(),
        proposal_preview_links: plan.proposal_preview_links.clone(),
        trust_projection_references,
        assisted_ai_audit_references: audit_references.clone(),
        proposal_ids,
        blockers: plan.blockers.clone(),
        refusals: plan.refusals.clone(),
        readiness_classification: classify_delegated_task_readiness(plan, &audit_references),
        correlation_id: plan.correlation_id,
        causality_id: plan.causality_id,
        event_sequence,
        risk_labels: plan.risk_labels.clone(),
        privacy_labels: plan.privacy_labels.clone(),
        runtime_activation: DelegatedTaskRuntimeActivationState::NotEncoded,
        runtime_activation_labels: vec![
            "agent.runtime.not_encoded".to_string(),
            "provider.invocation.not_encoded".to_string(),
            "network.not_encoded".to_string(),
            "tool.execution.not_encoded".to_string(),
            "terminal.execution.not_encoded".to_string(),
            "proposal.apply.not_encoded".to_string(),
            "workspace.mutation.not_encoded".to_string(),
        ],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    };
    validate_delegated_task_audit_linkage_record(&record)?;
    Ok(record)
}

/// Evaluates an assisted-AI provider route at the metadata-only consent boundary.
pub fn assisted_ai_evaluate_route_decision(
    provider: &AssistedAiProviderCapability,
    boundary: &AssistedAiConsentBoundary,
    operation_class: AssistedAiOperationClass,
    schema_version: u16,
) -> AssistedAiRouteDecision {
    let mut reasons = boundary.reasons.clone();
    if provider.availability != AssistedAiProviderAvailabilityState::Available {
        let reason_code = match provider.availability {
            AssistedAiProviderAvailabilityState::Available => "provider.available",
            AssistedAiProviderAvailabilityState::Disabled => "provider.disabled",
            AssistedAiProviderAvailabilityState::Refused => "provider.refused",
            AssistedAiProviderAvailabilityState::Unavailable => "provider.unavailable",
        };
        reasons.push(reason_code.to_string());
        return route_decision_refused(
            AssistedAiRequestDisposition::Disabled,
            provider,
            boundary,
            operation_class,
            reason_code,
            "Provider is not available for assisted AI routing",
            None,
            reasons,
            schema_version,
        );
    }

    if !provider.supports_operation(operation_class) {
        reasons.push("provider.operation_unsupported".to_string());
        return route_decision_refused(
            AssistedAiRequestDisposition::Disabled,
            provider,
            boundary,
            operation_class,
            "provider.operation_unsupported",
            "Provider does not support the requested assisted AI operation",
            None,
            reasons,
            schema_version,
        );
    }

    if boundary.workspace_trust_state != WorkspaceTrustState::Trusted {
        reasons.push("workspace.untrusted".to_string());
        return route_decision_refused(
            AssistedAiRequestDisposition::Refused,
            provider,
            boundary,
            operation_class,
            "workspace.untrusted",
            "Workspace trust is required before provider routing",
            None,
            reasons,
            schema_version,
        );
    }

    if !matches!(
        boundary.consent_state,
        AssistedAiConsentState::Granted | AssistedAiConsentState::NotRequired
    ) {
        let reason_code = match boundary.consent_state {
            AssistedAiConsentState::Granted | AssistedAiConsentState::NotRequired => {
                "consent.granted"
            }
            AssistedAiConsentState::Missing => "consent.missing",
            AssistedAiConsentState::Denied => "consent.denied",
            AssistedAiConsentState::RenewalRequired => "consent.renewal_required",
        };
        reasons.push(reason_code.to_string());
        return route_decision_refused(
            AssistedAiRequestDisposition::Refused,
            provider,
            boundary,
            operation_class,
            reason_code,
            "Consent is required before provider routing",
            None,
            reasons,
            schema_version,
        );
    }

    if !boundary.privacy_scope_allowed {
        reasons.push("privacy.scope_denied".to_string());
        return route_decision_refused(
            AssistedAiRequestDisposition::Refused,
            provider,
            boundary,
            operation_class,
            "privacy.scope_denied",
            "Privacy scope denied assisted AI routing",
            None,
            reasons,
            schema_version,
        );
    }

    if let Some(evaluation) = boundary.budget_evaluations.iter().find(|evaluation| {
        !evaluation.allowed
            || evaluation.state == PermissionBudgetState::Denied
            || evaluation.state == PermissionBudgetState::Depleted
    }) {
        let reason_code = if evaluation.state == PermissionBudgetState::Depleted
            || evaluation.disposition == PermissionBudgetEvaluationDisposition::RefusedDepleted
        {
            "budget.depleted"
        } else {
            "budget.denied"
        };
        reasons.push(reason_code.to_string());
        return route_decision_refused(
            AssistedAiRequestDisposition::Refused,
            provider,
            boundary,
            operation_class,
            reason_code,
            "Permission budget refused assisted AI routing",
            Some(evaluation.budget_id.clone()),
            reasons,
            schema_version,
        );
    }

    if provider.is_remote_class()
        && (boundary.air_gap_mode || boundary.offline_mode || boundary.local_only_mode)
    {
        reasons.push("egress.remote_denied".to_string());
        return route_decision_refused(
            AssistedAiRequestDisposition::Refused,
            provider,
            boundary,
            operation_class,
            "egress.remote_denied",
            "Remote provider routing is denied by air-gap, offline, or local-only policy",
            None,
            reasons,
            schema_version,
        );
    }

    reasons.push("route.metadata_only_ready".to_string());
    AssistedAiRouteDecision {
        disposition: AssistedAiRequestDisposition::MetadataOnlyReady,
        provider_invocation: AssistedAiProviderInvocationState::NotEncoded,
        refusal: None,
        reasons,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Validates required assisted-AI proposal preconditions before proposal conversion.
pub fn validate_assisted_ai_proposal_preconditions(
    preconditions: &ProposalVersionPreconditions,
) -> Result<(), AssistedAiContractError> {
    let summary = ContextManifestPreconditionSummary::from_preconditions(preconditions, 1);
    if !summary.core_preconditions_present {
        return Err(AssistedAiContractError::MissingPrecondition {
            reason: summary
                .risk_reasons
                .first()
                .cloned()
                .unwrap_or_else(|| "missing.core_preconditions".to_string()),
        });
    }
    if preconditions.expected_fingerprint.is_none() {
        return Err(AssistedAiContractError::MissingPrecondition {
            reason: "missing.expected_fingerprint".to_string(),
        });
    }
    Ok(())
}

fn validate_assisted_ai_correlation(
    correlation_id: CorrelationId,
    causality_id: CausalityId,
) -> Result<(), AssistedAiContractError> {
    if correlation_id.0 == 0 {
        return Err(AssistedAiContractError::ZeroCorrelationId);
    }
    if causality_id.0 == Uuid::nil() {
        return Err(AssistedAiContractError::NilCausalityId);
    }
    Ok(())
}

fn validate_assisted_ai_audit_fingerprint(
    field: &str,
    fingerprint: &FileFingerprint,
) -> Result<(), AssistedAiContractError> {
    validate_assisted_ai_audit_string(field, &fingerprint.algorithm)?;
    validate_assisted_ai_audit_string(field, &fingerprint.value)
}

fn validate_assisted_ai_audit_string(
    field: &str,
    value: &str,
) -> Result<(), AssistedAiContractError> {
    if assisted_ai_forbidden_audit_marker(value) {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: field.to_string(),
            reason: "forbidden.raw_or_payload_marker".to_string(),
        });
    }
    Ok(())
}

fn assisted_ai_forbidden_audit_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "raw prompt",
        "source_body",
        "source body",
        "provider_payload",
        "provider request payload",
        "provider response payload",
        "chatcompletionrequest",
        "terminal output",
        "full diff",
        "reconstructed file",
        "model-generated prose",
        "network_request",
        "tool_call",
        "agent_runtime",
        "runtime_started",
        "fn main",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn validate_delegated_task_preview_link(
    link: &DelegatedTaskProposalPreviewLink,
) -> Result<(), AssistedAiContractError> {
    validate_assisted_ai_audit_string("proposal_preview.link_id", &link.link_id)?;
    if link.schema_version == 0 {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "proposal_preview.schema_version".to_string(),
            reason: "schema.zero".to_string(),
        });
    }
    if link.redaction_hints.contains(&RedactionHint::None) || !link.full_source_redacted {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "proposal_preview.redaction".to_string(),
            reason: "proposal_preview.source_not_redacted".to_string(),
        });
    }
    if let Some(reference) = &link.approval_checklist {
        validate_assisted_ai_audit_string(
            "proposal_preview.approval_checklist",
            &reference.reference_id,
        )?;
        validate_assisted_ai_audit_fingerprint(
            "proposal_preview.approval_checklist.hash",
            &reference.projection_hash,
        )?;
    }
    if let Some(reference) = &link.checkpoint_rollback {
        validate_assisted_ai_audit_string(
            "proposal_preview.checkpoint_rollback",
            &reference.reference_id,
        )?;
        validate_assisted_ai_audit_fingerprint(
            "proposal_preview.checkpoint_rollback.hash",
            &reference.projection_hash,
        )?;
    }
    Ok(())
}

fn validate_delegated_task_audit_reference(
    reference: &DelegatedTaskAssistedAiAuditReference,
) -> Result<(), AssistedAiContractError> {
    if reference.schema_version == 0 {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "assisted_ai_audit_reference.schema_version".to_string(),
            reason: "schema.zero".to_string(),
        });
    }
    if reference.event_sequence.0 == 0 {
        return Err(AssistedAiContractError::ZeroEventSequence);
    }
    if reference.redaction_state != AssistedAiAuditRedactionState::MetadataOnly
        || reference.runtime_invocation_state != AssistedAiProviderInvocationState::NotEncoded
    {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "assisted_ai_audit_reference.runtime".to_string(),
            reason: "runtime.invocation_encoded".to_string(),
        });
    }
    validate_assisted_ai_audit_string("assisted_ai_audit_reference.audit_id", &reference.audit_id)?;
    validate_assisted_ai_audit_string(
        "assisted_ai_audit_reference.request_contract_id",
        &reference.request_contract_id,
    )?;
    validate_assisted_ai_audit_fingerprint(
        "assisted_ai_audit_reference.audit_hash",
        &reference.audit_hash,
    )?;
    validate_assisted_ai_audit_fingerprint(
        "assisted_ai_audit_reference.request_contract_hash",
        &reference.request_contract_hash,
    )?;
    if let Some(projection_id) = &reference.projection_id {
        validate_assisted_ai_audit_string(
            "assisted_ai_audit_reference.projection_id",
            projection_id,
        )?;
    }
    if let Some(projection_hash) = &reference.projection_hash {
        validate_assisted_ai_audit_fingerprint(
            "assisted_ai_audit_reference.projection_hash",
            projection_hash,
        )?;
    }
    if let Some(preview_id) = &reference.preview_id {
        validate_assisted_ai_audit_string("assisted_ai_audit_reference.preview_id", preview_id)?;
    }
    if let Some(preview_hash) = &reference.preview_hash {
        validate_assisted_ai_audit_fingerprint(
            "assisted_ai_audit_reference.preview_hash",
            preview_hash,
        )?;
    }
    Ok(())
}

fn validate_delegated_task_blocker_metadata(
    blocker: &DelegatedTaskPlanBlocker,
) -> Result<(), AssistedAiContractError> {
    validate_assisted_ai_audit_string("delegated_task_blocker.reason_code", &blocker.reason_code)?;
    validate_assisted_ai_audit_string("delegated_task_blocker.label", &blocker.label)?;
    if let Some(step_id) = &blocker.step_id {
        validate_assisted_ai_audit_string("delegated_task_blocker.step_id", &step_id.0)?;
    }
    if let Some(target_id) = &blocker.target_id {
        validate_assisted_ai_audit_string("delegated_task_blocker.target_id", target_id)?;
    }
    if let Some(capability) = &blocker.capability {
        validate_assisted_ai_audit_string("delegated_task_blocker.capability", &capability.0)?;
    }
    if let Some(budget_id) = &blocker.budget_id {
        validate_assisted_ai_audit_string("delegated_task_blocker.budget_id", budget_id)?;
    }
    for reason in &blocker.reasons {
        validate_assisted_ai_audit_string("delegated_task_blocker.reasons", reason)?;
    }
    if blocker.schema_version == 0 || blocker.redaction_hints.contains(&RedactionHint::None) {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "delegated_task_blocker.schema_or_redaction".to_string(),
            reason: "metadata.invalid".to_string(),
        });
    }
    Ok(())
}

fn delegated_task_trust_projection_references(
    plan: &DelegatedTaskPlanContract,
) -> Vec<AssistedAiTrustProjectionReference> {
    let mut refs = Vec::new();
    refs.extend(plan.context_manifest.clone());
    refs.extend(plan.privacy_inspector.clone());
    refs.extend(plan.permission_budget_projection.clone());
    refs.extend(plan.approval_checklist.clone());
    refs.extend(plan.checkpoint_rollback.clone());
    refs.extend(plan.assisted_ai_projection.clone());
    refs.extend(
        plan.required_trust_gates
            .iter()
            .filter_map(|gate| gate.projection_reference.clone()),
    );
    refs
}

fn delegated_task_linked_proposal_ids(
    plan: &DelegatedTaskPlanContract,
    audit_references: &[DelegatedTaskAssistedAiAuditReference],
) -> Vec<ProposalId> {
    let mut proposal_ids = Vec::new();
    for link in &plan.proposal_preview_links {
        if !proposal_ids.contains(&link.proposal_id) {
            proposal_ids.push(link.proposal_id);
        }
    }
    for audit in audit_references
        .iter()
        .filter_map(|audit| audit.proposal_id)
    {
        if !proposal_ids.contains(&audit) {
            proposal_ids.push(audit);
        }
    }
    proposal_ids
}

#[allow(clippy::too_many_arguments)]
fn route_decision_refused(
    disposition: AssistedAiRequestDisposition,
    provider: &AssistedAiProviderCapability,
    boundary: &AssistedAiConsentBoundary,
    operation_class: AssistedAiOperationClass,
    reason_code: &str,
    label: &str,
    budget_id: Option<String>,
    reasons: Vec<String>,
    schema_version: u16,
) -> AssistedAiRouteDecision {
    AssistedAiRouteDecision {
        disposition,
        provider_invocation: AssistedAiProviderInvocationState::NotEncoded,
        refusal: Some(AssistedAiRefusalMetadata {
            reason_code: reason_code.to_string(),
            label: label.to_string(),
            provider_id: Some(provider.provider_id.clone()),
            operation_class: Some(operation_class),
            privacy_scope: Some(boundary.requested_privacy_scope),
            capability: boundary.required_capability.clone(),
            budget_id,
            risk_label: ProposalRiskLabel::High,
            reasons: reasons.clone(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        }),
        reasons,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn empty_target_coverage() -> ProposalTargetCoverage {
    ProposalTargetCoverage {
        coverage_kind: ProposalTargetCoverageKind::Redacted,
        targets: Vec::new(),
        omitted_target_count: 1,
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
}

fn diff_summary_from_payload_metadata(
    payload: &ProposalPayload,
    target_coverage: &ProposalTargetCoverage,
) -> ProposalDiffSummary {
    let mut hunk_count = 0;
    let kind = match payload {
        ProposalPayload::TextEdit(text) => {
            hunk_count = text.edits.edits.len() as u32;
            ProposalDiffSummaryKind::Text
        }
        ProposalPayload::CreateFile(_)
        | ProposalPayload::DeleteFile(_)
        | ProposalPayload::RenameFile(_) => {
            hunk_count = 1;
            ProposalDiffSummaryKind::FileOperation
        }
        ProposalPayload::WorkspaceEdit(edit) => {
            hunk_count = edit
                .file_edits
                .iter()
                .map(|file| file.edits.edits.len() as u32)
                .sum::<u32>()
                .saturating_add(edit.file_operations.len() as u32);
            ProposalDiffSummaryKind::WorkspaceEdit
        }
        ProposalPayload::Batch(batch) => {
            hunk_count = batch.items.len() as u32;
            ProposalDiffSummaryKind::WorkspaceEdit
        }
        ProposalPayload::SaveFile(_)
        | ProposalPayload::FormatFile(_)
        | ProposalPayload::CodeAction(_) => ProposalDiffSummaryKind::Text,
        ProposalPayload::TerminalCommand(_) => ProposalDiffSummaryKind::TerminalMetadata,
    };
    ProposalDiffSummary {
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
            .map(|(index, target)| ProposalDiffChunkDescriptor {
                chunk_id: format!("metadata-chunk:{index}"),
                target_id: Some(target.target_id.clone()),
                byte_range: target.byte_ranges.first().copied(),
                changed_line_count: 0,
                inserted_line_count: 0,
                deleted_line_count: 0,
                content_hash: None,
            })
            .collect(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
}

fn assisted_ai_preview_refusal(
    output: &AssistedAiEditProposalOutput,
    request: Option<&AssistedAiRequestContract>,
    readiness: AssistedAiProposalPreviewReadiness,
    output_error: Option<&AssistedAiContractError>,
    schema_version: u16,
) -> Option<AssistedAiRefusalMetadata> {
    match readiness {
        AssistedAiProposalPreviewReadiness::PreviewReady => None,
        AssistedAiProposalPreviewReadiness::RouteRefused => request
            .and_then(|request| request.route_decision.refusal.clone())
            .or_else(|| {
                Some(AssistedAiRefusalMetadata {
                    reason_code: "route.refused".to_string(),
                    label: "Assisted AI route refused preview readiness".to_string(),
                    provider_id: Some(output.provider_id.clone()),
                    operation_class: request.map(|request| request.operation_class),
                    privacy_scope: None,
                    capability: Some(output.capability.clone()),
                    budget_id: None,
                    risk_label: ProposalRiskLabel::High,
                    reasons: vec!["route.refused".to_string()],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version,
                })
            }),
        AssistedAiProposalPreviewReadiness::InvalidOutput => Some(AssistedAiRefusalMetadata {
            reason_code: assisted_ai_output_error_reason(output_error),
            label: "Assisted AI output cannot become a proposal preview".to_string(),
            provider_id: Some(output.provider_id.clone()),
            operation_class: request.map(|request| request.operation_class),
            privacy_scope: None,
            capability: Some(output.capability.clone()),
            budget_id: None,
            risk_label: ProposalRiskLabel::High,
            reasons: vec!["output.invalid".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        }),
        AssistedAiProposalPreviewReadiness::MissingProposalLedger => {
            Some(AssistedAiRefusalMetadata {
                reason_code: "proposal.ledger_missing".to_string(),
                label: "Proposal ledger row is required before assisted AI preview readiness"
                    .to_string(),
                provider_id: Some(output.provider_id.clone()),
                operation_class: request.map(|request| request.operation_class),
                privacy_scope: None,
                capability: Some(output.capability.clone()),
                budget_id: None,
                risk_label: ProposalRiskLabel::High,
                reasons: vec!["proposal.ledger_missing".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version,
            })
        }
    }
}

fn assisted_ai_output_error_reason(error: Option<&AssistedAiContractError>) -> String {
    match error {
        Some(AssistedAiContractError::ZeroCorrelationId) => "correlation.zero".to_string(),
        Some(AssistedAiContractError::NilCausalityId) => "causality.nil".to_string(),
        Some(AssistedAiContractError::MissingPrecondition { reason }) => reason.clone(),
        Some(AssistedAiContractError::InvalidProposalMetadata { reason }) => reason.clone(),
        Some(AssistedAiContractError::RefusedRouteDecision { reason }) => reason.clone(),
        Some(AssistedAiContractError::ZeroEventSequence) => "event_sequence.zero".to_string(),
        Some(AssistedAiContractError::NonMetadataOnlyAuditRecord { reason, .. }) => reason.clone(),
        None => "output.invalid".to_string(),
    }
}

fn assisted_ai_projection_refusals(
    providers: &[AssistedAiProviderCapabilitySummary],
    routes: &[AssistedAiRouteDecisionSummary],
    previews: &[AssistedAiProposalPreviewSummary],
) -> Vec<AssistedAiRefusalMetadata> {
    let mut refusals = Vec::new();
    refusals.extend(
        providers
            .iter()
            .filter_map(|provider| provider.refusal.clone()),
    );
    refusals.extend(routes.iter().filter_map(|route| route.refusal.clone()));
    refusals.extend(
        previews
            .iter()
            .filter_map(|preview| preview.refusal.clone()),
    );
    refusals
}

/// Builds a delegated-task plan contract from metadata-only boundary inputs.
pub fn delegated_task_plan_from_boundary_input(
    input: DelegatedTaskPlanningBoundaryInput,
) -> DelegatedTaskPlanContract {
    let mut required_trust_gates = Vec::new();
    let mut blockers = Vec::new();
    let mut refusals = Vec::new();
    let schema_version = input.schema_version;

    push_delegated_task_gate(
        &mut required_trust_gates,
        DelegatedTaskTrustGateKind::WorkspaceTrust,
        true,
        input.workspace_trust_state == WorkspaceTrustState::Trusted,
        None,
        schema_version,
    );
    if input.workspace_trust_state != WorkspaceTrustState::Trusted {
        push_delegated_task_refusal(
            &mut refusals,
            "workspace.untrusted",
            "Workspace trust is required before delegated task planning can proceed",
            DelegatedTaskTrustGateKind::WorkspaceTrust,
            None,
            None,
            None,
            None,
            None,
            schema_version,
        );
    }

    push_delegated_task_gate(
        &mut required_trust_gates,
        DelegatedTaskTrustGateKind::ContextManifest,
        true,
        input.context_manifest.is_some(),
        input.context_manifest.clone(),
        schema_version,
    );
    if input.context_manifest.is_none() {
        push_delegated_task_blocker(
            &mut blockers,
            "context_manifest.missing",
            "Context manifest projection reference is required",
            DelegatedTaskTrustGateKind::ContextManifest,
            None,
            None,
            None,
            None,
            None,
            schema_version,
        );
    }

    push_delegated_task_gate(
        &mut required_trust_gates,
        DelegatedTaskTrustGateKind::PrivacyInspector,
        true,
        input.privacy_inspector.is_some() && !input.privacy_denied,
        input.privacy_inspector.clone(),
        schema_version,
    );
    if input.privacy_inspector.is_none() {
        push_delegated_task_blocker(
            &mut blockers,
            "privacy_inspector.missing",
            "Privacy inspector projection reference is required",
            DelegatedTaskTrustGateKind::PrivacyInspector,
            None,
            None,
            None,
            None,
            None,
            schema_version,
        );
    } else if input.privacy_denied {
        push_delegated_task_refusal(
            &mut refusals,
            "privacy.denied",
            "Privacy inspector denied delegated task planning",
            DelegatedTaskTrustGateKind::PrivacyInspector,
            None,
            None,
            None,
            None,
            input.privacy_inspector.clone(),
            schema_version,
        );
    }

    push_delegated_task_gate(
        &mut required_trust_gates,
        DelegatedTaskTrustGateKind::PermissionBudget,
        true,
        input.permission_budget_projection.is_some()
            && !input.permission_budget_denied
            && !input.permission_budget_depleted,
        input.permission_budget_projection.clone(),
        schema_version,
    );
    if input.permission_budget_projection.is_none() {
        push_delegated_task_blocker(
            &mut blockers,
            "permission_budget.missing",
            "Permission budget projection reference is required",
            DelegatedTaskTrustGateKind::PermissionBudget,
            None,
            None,
            None,
            None,
            None,
            schema_version,
        );
    } else if input.permission_budget_depleted {
        push_delegated_task_refusal(
            &mut refusals,
            "budget.depleted",
            "Delegated task permission budget is depleted",
            DelegatedTaskTrustGateKind::PermissionBudget,
            None,
            None,
            None,
            Some("permission-budget".to_string()),
            input.permission_budget_projection.clone(),
            schema_version,
        );
    } else if input.permission_budget_denied {
        push_delegated_task_refusal(
            &mut refusals,
            "budget.denied",
            "Delegated task permission budget is denied",
            DelegatedTaskTrustGateKind::PermissionBudget,
            None,
            None,
            None,
            Some("permission-budget".to_string()),
            input.permission_budget_projection.clone(),
            schema_version,
        );
    }

    push_delegated_task_gate(
        &mut required_trust_gates,
        DelegatedTaskTrustGateKind::ApprovalChecklist,
        true,
        input.approval_checklist.is_some() && input.approval_checklist_valid,
        input.approval_checklist.clone(),
        schema_version,
    );
    if input.approval_checklist.is_none() {
        push_delegated_task_blocker(
            &mut blockers,
            "approval_checklist.missing",
            "Approval checklist projection reference is required",
            DelegatedTaskTrustGateKind::ApprovalChecklist,
            None,
            None,
            None,
            None,
            None,
            schema_version,
        );
    } else if !input.approval_checklist_valid {
        push_delegated_task_blocker(
            &mut blockers,
            "approval_checklist.invalid",
            "Approval checklist projection is not ready",
            DelegatedTaskTrustGateKind::ApprovalChecklist,
            None,
            None,
            None,
            None,
            input.approval_checklist.clone(),
            schema_version,
        );
    }

    push_delegated_task_gate(
        &mut required_trust_gates,
        DelegatedTaskTrustGateKind::Checkpoint,
        input.checkpoint_required,
        !input.checkpoint_required || input.checkpoint_available,
        input.checkpoint_rollback.clone(),
        schema_version,
    );
    if input.checkpoint_required && !input.checkpoint_available {
        push_delegated_task_blocker(
            &mut blockers,
            "checkpoint.missing",
            "Checkpoint metadata is required before delegated task planning can proceed",
            DelegatedTaskTrustGateKind::Checkpoint,
            None,
            None,
            None,
            None,
            input.checkpoint_rollback.clone(),
            schema_version,
        );
    }

    push_delegated_task_gate(
        &mut required_trust_gates,
        DelegatedTaskTrustGateKind::Rollback,
        input.rollback_required,
        !input.rollback_required || input.rollback_available,
        input.checkpoint_rollback.clone(),
        schema_version,
    );
    if input.rollback_required && !input.rollback_available {
        push_delegated_task_blocker(
            &mut blockers,
            "rollback.missing",
            "Rollback metadata is required before delegated task planning can proceed",
            DelegatedTaskTrustGateKind::Rollback,
            None,
            None,
            None,
            None,
            input.checkpoint_rollback.clone(),
            schema_version,
        );
    }

    push_delegated_task_gate(
        &mut required_trust_gates,
        DelegatedTaskTrustGateKind::AssistedAiProjection,
        input.assisted_ai_required,
        !input.assisted_ai_required || input.assisted_ai_projection.is_some(),
        input.assisted_ai_projection.clone(),
        schema_version,
    );
    if input.assisted_ai_required && input.assisted_ai_projection.is_none() {
        push_delegated_task_blocker(
            &mut blockers,
            "assisted_ai_projection.missing",
            "Assisted-AI projection reference is required before delegated task planning can proceed",
            DelegatedTaskTrustGateKind::AssistedAiProjection,
            None,
            None,
            None,
            None,
            None,
            schema_version,
        );
    }

    let correlation_causality_valid =
        input.correlation_id.0 != 0 && input.causality_id.0 != Uuid::nil();
    push_delegated_task_gate(
        &mut required_trust_gates,
        DelegatedTaskTrustGateKind::CorrelationCausality,
        true,
        correlation_causality_valid,
        None,
        schema_version,
    );
    if input.correlation_id.0 == 0 {
        push_delegated_task_refusal(
            &mut refusals,
            "correlation.zero",
            "Delegated task plan requires non-zero correlation metadata",
            DelegatedTaskTrustGateKind::CorrelationCausality,
            None,
            None,
            None,
            None,
            None,
            schema_version,
        );
    }
    if input.causality_id.0 == Uuid::nil() {
        push_delegated_task_refusal(
            &mut refusals,
            "causality.nil",
            "Delegated task plan requires non-nil causality metadata",
            DelegatedTaskTrustGateKind::CorrelationCausality,
            None,
            None,
            None,
            None,
            None,
            schema_version,
        );
    }

    let plan_state = if !refusals.is_empty() {
        DelegatedTaskPlanState::Refused
    } else if !blockers.is_empty() {
        DelegatedTaskPlanState::Blocked
    } else if input.approval_checklist_valid && !input.proposal_preview_links.is_empty() {
        DelegatedTaskPlanState::AwaitingApproval
    } else {
        DelegatedTaskPlanState::Planned
    };
    let readiness = match plan_state {
        DelegatedTaskPlanState::Refused => DelegatedTaskPlanReadinessStatus::Refused,
        DelegatedTaskPlanState::Blocked => DelegatedTaskPlanReadinessStatus::Blocked,
        DelegatedTaskPlanState::Draft
        | DelegatedTaskPlanState::Planned
        | DelegatedTaskPlanState::AwaitingApproval
        | DelegatedTaskPlanState::Cancelled => DelegatedTaskPlanReadinessStatus::PlanReady,
    };
    let risk_labels = delegated_task_risk_labels(&input, &blockers, &refusals);
    let privacy_labels = delegated_task_privacy_labels(&input, &blockers, &refusals);
    let audit_readiness = DelegatedTaskAuditReadinessStatus {
        readiness_id: format!("delegated-task:readiness:{}", input.plan_id.0),
        readiness,
        runtime_activation: DelegatedTaskRuntimeActivationState::NotEncoded,
        correlation_causality_valid,
        blocker_count: blockers.len() as u32,
        refusal_count: refusals.len() as u32,
        proposal_preview_link_count: input.proposal_preview_links.len() as u32,
        labels: vec![
            "delegated_task.plan_only".to_string(),
            "runtime.not_encoded".to_string(),
            "mutation.proposal_only".to_string(),
        ],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    };

    DelegatedTaskPlanContract {
        plan_id: input.plan_id,
        workspace_id: input.workspace_id,
        objective_summary_hash: input.objective_summary_hash,
        allowed_operation_classes: input.allowed_operation_classes,
        required_trust_gates,
        context_manifest: input.context_manifest,
        privacy_inspector: input.privacy_inspector,
        permission_budget_projection: input.permission_budget_projection,
        approval_checklist: input.approval_checklist,
        checkpoint_rollback: input.checkpoint_rollback,
        assisted_ai_projection: input.assisted_ai_projection,
        affected_targets: input.affected_targets,
        steps: input.steps,
        proposal_preview_links: input.proposal_preview_links,
        risk_labels,
        privacy_labels,
        checkpoint_required: input.checkpoint_required,
        rollback_required: input.rollback_required,
        plan_state,
        blockers,
        refusals,
        audit_readiness,
        correlation_id: input.correlation_id,
        causality_id: input.causality_id,
        created_at: input.created_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Builds a static delegated-task projection from plan-only metadata contracts.
pub fn delegated_task_projection_from_plan_contracts(
    projection_id: impl Into<String>,
    plans: Vec<DelegatedTaskPlanContract>,
    generated_at: TimestampMillis,
    schema_version: u16,
) -> DelegatedTaskProjection {
    let plan_rows = plans
        .iter()
        .map(|plan| delegated_task_plan_row(plan, schema_version))
        .collect::<Vec<_>>();
    let step_summaries = plans
        .iter()
        .flat_map(|plan| delegated_task_step_summaries(plan, schema_version))
        .collect::<Vec<_>>();
    let blockers = plans
        .iter()
        .flat_map(|plan| plan.blockers.clone())
        .collect::<Vec<_>>();
    let refusals = plans
        .iter()
        .flat_map(|plan| plan.refusals.clone())
        .collect::<Vec<_>>();
    let required_approvals = plans
        .iter()
        .flat_map(|plan| {
            plan.required_trust_gates
                .iter()
                .filter(|gate| gate.required)
                .cloned()
        })
        .collect::<Vec<_>>();
    let proposal_preview_links = plans
        .iter()
        .flat_map(|plan| plan.proposal_preview_links.clone())
        .collect::<Vec<_>>();
    let audit_readiness = plans
        .iter()
        .map(|plan| plan.audit_readiness.clone())
        .collect::<Vec<_>>();
    let blocked_plan_count = plan_rows
        .iter()
        .filter(|row| row.readiness == DelegatedTaskPlanReadinessStatus::Blocked)
        .count() as u32;
    let refused_plan_count = plan_rows
        .iter()
        .filter(|row| row.readiness == DelegatedTaskPlanReadinessStatus::Refused)
        .count() as u32;

    DelegatedTaskProjection {
        projection_id: projection_id.into(),
        plan_count: plan_rows.len() as u32,
        blocked_plan_count,
        refused_plan_count,
        plan_rows,
        step_summaries,
        blockers,
        refusals,
        required_approvals,
        proposal_preview_links,
        audit_readiness,
        plan_only_disclaimers: vec![
            "delegated_task.plan_only_no_runtime".to_string(),
            "agent.provider.tool.terminal.not_encoded".to_string(),
            "outputs.must_be_proposals_only".to_string(),
        ],
        runtime_activation: DelegatedTaskRuntimeActivationState::NotEncoded,
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Evaluates a P8.1 future-surface planning gate as metadata only.
pub fn evaluate_future_surface_planning_gate(
    input: FutureSurfacePlanningGateInput,
) -> FutureSurfacePlanningGate {
    let mut blockers = Vec::new();
    let mut refusals = Vec::new();
    let schema_version = input.schema_version;

    if schema_version == 0 {
        push_future_surface_blocker(
            &mut blockers,
            "metadata.schema_zero",
            "Future-surface gate schema version must be non-zero",
            FutureSurfaceBlockerCategory::MetadataInvalid,
            None,
            schema_version,
        );
    }
    if input.correlation_id.0 == 0 {
        push_future_surface_refusal(
            &mut refusals,
            "correlation.zero",
            "Future-surface gate requires non-zero correlation metadata",
            FutureSurfaceBlockerCategory::MetadataInvalid,
            None,
            schema_version,
        );
    }
    if input.causality_id.0 == Uuid::nil() {
        push_future_surface_refusal(
            &mut refusals,
            "causality.nil",
            "Future-surface gate requires non-nil causality metadata",
            FutureSurfaceBlockerCategory::MetadataInvalid,
            None,
            schema_version,
        );
    }

    push_future_surface_policy_requirement(
        &mut blockers,
        input.adr_status,
        "adr.required",
        "Accepted ADR metadata is required before future-surface readiness",
        schema_version,
    );
    push_future_surface_policy_requirement(
        &mut blockers,
        input.dependency_policy_status,
        "dependency_policy.required",
        "Dependency-policy metadata is required before future-surface readiness",
        schema_version,
    );
    push_future_surface_policy_requirement(
        &mut blockers,
        input.threat_model_status,
        "threat_model.required",
        "Threat-model metadata is required before future-surface readiness",
        schema_version,
    );
    push_future_surface_policy_requirement(
        &mut blockers,
        input.phase_status_entry_status,
        "phase_status.required",
        "Phase-status metadata is required before future-surface readiness",
        schema_version,
    );
    if !input.contract_test_status.satisfies_gate() {
        push_future_surface_blocker(
            &mut blockers,
            "contract_tests.required",
            "Contract tests are required before future-surface readiness",
            FutureSurfaceBlockerCategory::ContractTestRequirement,
            None,
            schema_version,
        );
    }

    if input.workspace_trust_state != WorkspaceTrustState::Trusted {
        push_future_surface_blocker(
            &mut blockers,
            "workspace_trust.required",
            "Workspace trust is required before future-surface readiness",
            FutureSurfaceBlockerCategory::TrustRequirement,
            None,
            schema_version,
        );
    }
    if input.privacy_denied {
        push_future_surface_refusal(
            &mut refusals,
            "privacy.denied",
            "Privacy inspection denied the future-surface planning boundary",
            FutureSurfaceBlockerCategory::PrivacyRefusal,
            None,
            schema_version,
        );
    }
    if input.permission_budget_denied {
        push_future_surface_refusal(
            &mut refusals,
            "budget.denied",
            "Permission budget denied the future-surface planning boundary",
            FutureSurfaceBlockerCategory::BudgetRefusal,
            None,
            schema_version,
        );
    }
    if input.permission_budget_depleted {
        push_future_surface_refusal(
            &mut refusals,
            "budget.depleted",
            "Permission budget is depleted for the future-surface planning boundary",
            FutureSurfaceBlockerCategory::BudgetRefusal,
            None,
            schema_version,
        );
    }

    if input.approval_required && !input.approval_available {
        push_future_surface_blocker(
            &mut blockers,
            "approval.required",
            "Approval checklist metadata is required before future-surface readiness",
            FutureSurfaceBlockerCategory::ApprovalRequirement,
            None,
            schema_version,
        );
    }
    if input.checkpoint_required && !input.checkpoint_available {
        push_future_surface_blocker(
            &mut blockers,
            "checkpoint.required",
            "Checkpoint metadata is required before future-surface readiness",
            FutureSurfaceBlockerCategory::CheckpointRequirement,
            None,
            schema_version,
        );
    }
    if input.rollback_required && !input.rollback_available {
        push_future_surface_blocker(
            &mut blockers,
            "rollback.required",
            "Rollback metadata is required before future-surface readiness",
            FutureSurfaceBlockerCategory::RollbackRequirement,
            None,
            schema_version,
        );
    }
    if input.proposal_only_mutation_required && !input.proposal_only_mutation_available {
        push_future_surface_blocker(
            &mut blockers,
            "proposal_only.required",
            "Proposal-only mutation metadata is required before future-surface readiness",
            FutureSurfaceBlockerCategory::ProposalOnlyRequirement,
            None,
            schema_version,
        );
    }

    for operation in &input.requested_operation_classes {
        if !input.allowed_operation_classes.contains(operation) {
            push_future_surface_blocker(
                &mut blockers,
                "operation.not_allowed",
                "Requested operation class is not allowed by the future-surface planning gate",
                FutureSurfaceBlockerCategory::OperationRefusal,
                Some(*operation),
                schema_version,
            );
        }
        if input.denied_operation_classes.contains(operation) || operation.is_direct_mutation() {
            push_future_surface_refusal(
                &mut refusals,
                "operation.denied",
                "Requested operation class is denied by the future-surface planning gate",
                FutureSurfaceBlockerCategory::OperationRefusal,
                Some(*operation),
                schema_version,
            );
        }
        if operation.requires_runtime_activation() {
            push_future_surface_blocker(
                &mut blockers,
                "runtime.not_encoded",
                "Requested future-surface runtime activation remains intentionally unencoded",
                FutureSurfaceBlockerCategory::RuntimeNotEncoded,
                Some(*operation),
                schema_version,
            );
        }
    }

    let classification = classify_future_surface_gate(&blockers, &refusals);
    let proposal_only_ready = classification == FutureSurfaceGateClassification::ProposalOnlyReady;

    FutureSurfacePlanningGate {
        gate_id: input.gate_id,
        surface_class: input.surface_class,
        allowed_operation_classes: input.allowed_operation_classes,
        denied_operation_classes: input.denied_operation_classes,
        requested_operation_classes: input.requested_operation_classes,
        adr_status: input.adr_status,
        dependency_policy_status: input.dependency_policy_status,
        contract_test_status: input.contract_test_status,
        threat_model_status: input.threat_model_status,
        phase_status_entry_status: input.phase_status_entry_status,
        workspace_trust_state: input.workspace_trust_state,
        proposal_only_mutation_required: input.proposal_only_mutation_required,
        proposal_only_mutation_available: input.proposal_only_mutation_available,
        runtime_activation: FutureSurfaceRuntimeActivationState::NotEncoded,
        classification,
        proposal_only_ready,
        blockers,
        refusals,
        correlation_id: input.correlation_id,
        causality_id: input.causality_id,
        runtime_activation_labels: vec![
            "terminal.runtime.not_encoded".to_string(),
            "plugin.host.not_encoded".to_string(),
            "collaboration.session.not_encoded".to_string(),
            "remote.session.not_encoded".to_string(),
            "autonomy.runtime.not_encoded".to_string(),
            "provider.invocation.not_encoded".to_string(),
            "network.not_encoded".to_string(),
            "workspace.mutation.proposal_only".to_string(),
        ],
        labels: input.labels,
        risk_labels: input.risk_labels,
        privacy_labels: input.privacy_labels,
        generated_at: input.generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Classifies an evaluated future-surface gate from metadata-only blockers and refusals.
pub fn classify_future_surface_gate(
    blockers: &[FutureSurfaceGateReason],
    refusals: &[FutureSurfaceGateReason],
) -> FutureSurfaceGateClassification {
    if !refusals.is_empty() {
        return FutureSurfaceGateClassification::Refused;
    }
    if blockers
        .iter()
        .any(|reason| reason.category == FutureSurfaceBlockerCategory::PolicyRequirement)
    {
        return FutureSurfaceGateClassification::PolicyRequired;
    }
    if blockers
        .iter()
        .any(|reason| reason.category == FutureSurfaceBlockerCategory::ContractTestRequirement)
    {
        return FutureSurfaceGateClassification::ContractTestsRequired;
    }
    if blockers
        .iter()
        .any(|reason| reason.category == FutureSurfaceBlockerCategory::TrustRequirement)
    {
        return FutureSurfaceGateClassification::TrustRequired;
    }
    if blockers
        .iter()
        .any(|reason| reason.category == FutureSurfaceBlockerCategory::RuntimeNotEncoded)
    {
        return FutureSurfaceGateClassification::RuntimeNotEncoded;
    }
    if !blockers.is_empty() {
        return FutureSurfaceGateClassification::Blocked;
    }
    FutureSurfaceGateClassification::ProposalOnlyReady
}

/// Builds a static projection from future-surface planning gates.
pub fn future_surface_gate_projection_from_gates(
    projection_id: impl Into<String>,
    gates: Vec<FutureSurfacePlanningGate>,
    generated_at: TimestampMillis,
    schema_version: u16,
) -> FutureSurfaceGateProjection {
    let blocked_gate_count = gates
        .iter()
        .filter(|gate| {
            matches!(
                gate.classification,
                FutureSurfaceGateClassification::Blocked
                    | FutureSurfaceGateClassification::PolicyRequired
                    | FutureSurfaceGateClassification::ContractTestsRequired
                    | FutureSurfaceGateClassification::TrustRequired
            )
        })
        .count() as u32;
    let refused_gate_count = gates
        .iter()
        .filter(|gate| gate.classification == FutureSurfaceGateClassification::Refused)
        .count() as u32;
    let proposal_only_ready_gate_count = gates
        .iter()
        .filter(|gate| gate.classification == FutureSurfaceGateClassification::ProposalOnlyReady)
        .count() as u32;
    let runtime_not_encoded_gate_count = gates
        .iter()
        .filter(|gate| gate.classification == FutureSurfaceGateClassification::RuntimeNotEncoded)
        .count() as u32;

    FutureSurfaceGateProjection {
        projection_id: projection_id.into(),
        gate_count: gates.len() as u32,
        gates,
        blocked_gate_count,
        refused_gate_count,
        proposal_only_ready_gate_count,
        runtime_not_encoded_gate_count,
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Validates a future-surface planning gate as metadata-only and runtime-unencoded.
pub fn validate_future_surface_planning_gate(
    gate: &FutureSurfacePlanningGate,
) -> Result<(), AssistedAiContractError> {
    validate_assisted_ai_correlation(gate.correlation_id, gate.causality_id)?;
    if gate.schema_version == 0 {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "schema_version".to_string(),
            reason: "schema.zero".to_string(),
        });
    }
    if gate.redaction_hints.contains(&RedactionHint::None) {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "redaction_hints".to_string(),
            reason: "redaction.none".to_string(),
        });
    }
    validate_assisted_ai_audit_string("future_surface.gate_id", &gate.gate_id.0)?;
    for label in gate
        .labels
        .iter()
        .chain(gate.runtime_activation_labels.iter())
    {
        validate_assisted_ai_audit_string("future_surface.labels", label)?;
    }
    for reason in gate.blockers.iter().chain(gate.refusals.iter()) {
        validate_future_surface_reason(reason)?;
    }
    Ok(())
}

fn push_future_surface_policy_requirement(
    blockers: &mut Vec<FutureSurfaceGateReason>,
    status: FutureSurfaceRequirementStatus,
    reason_code: &str,
    label: &str,
    schema_version: u16,
) {
    if !status.satisfies_gate() {
        push_future_surface_blocker(
            blockers,
            reason_code,
            label,
            FutureSurfaceBlockerCategory::PolicyRequirement,
            None,
            schema_version,
        );
    }
}

fn push_future_surface_blocker(
    blockers: &mut Vec<FutureSurfaceGateReason>,
    reason_code: &str,
    label: &str,
    category: FutureSurfaceBlockerCategory,
    operation_class: Option<FutureSurfaceOperationClass>,
    schema_version: u16,
) {
    blockers.push(future_surface_reason(
        reason_code,
        label,
        category,
        operation_class,
        schema_version,
    ));
}

fn push_future_surface_refusal(
    refusals: &mut Vec<FutureSurfaceGateReason>,
    reason_code: &str,
    label: &str,
    category: FutureSurfaceBlockerCategory,
    operation_class: Option<FutureSurfaceOperationClass>,
    schema_version: u16,
) {
    refusals.push(future_surface_reason(
        reason_code,
        label,
        category,
        operation_class,
        schema_version,
    ));
}

fn future_surface_reason(
    reason_code: &str,
    label: &str,
    category: FutureSurfaceBlockerCategory,
    operation_class: Option<FutureSurfaceOperationClass>,
    schema_version: u16,
) -> FutureSurfaceGateReason {
    FutureSurfaceGateReason {
        reason_code: reason_code.to_string(),
        label: label.to_string(),
        category,
        operation_class,
        capability: None,
        budget_id: if category == FutureSurfaceBlockerCategory::BudgetRefusal {
            Some("future-surface-budget".to_string())
        } else {
            None
        },
        risk_label: ProposalRiskLabel::High,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        reasons: vec![reason_code.to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn validate_future_surface_reason(
    reason: &FutureSurfaceGateReason,
) -> Result<(), AssistedAiContractError> {
    validate_assisted_ai_audit_string("future_surface.reason_code", &reason.reason_code)?;
    validate_assisted_ai_audit_string("future_surface.reason_label", &reason.label)?;
    if let Some(budget_id) = &reason.budget_id {
        validate_assisted_ai_audit_string("future_surface.budget_id", budget_id)?;
    }
    if reason.schema_version == 0 || reason.redaction_hints.contains(&RedactionHint::None) {
        return Err(AssistedAiContractError::NonMetadataOnlyAuditRecord {
            field: "future_surface.reason.schema_or_redaction".to_string(),
            reason: "schema.zero_or_redaction.none".to_string(),
        });
    }
    for label in &reason.reasons {
        validate_assisted_ai_audit_string("future_surface.reason.reasons", label)?;
    }
    Ok(())
}

fn push_delegated_task_gate(
    gates: &mut Vec<DelegatedTaskRequiredTrustGate>,
    kind: DelegatedTaskTrustGateKind,
    required: bool,
    satisfied: bool,
    projection_reference: Option<AssistedAiTrustProjectionReference>,
    schema_version: u16,
) {
    gates.push(DelegatedTaskRequiredTrustGate {
        kind,
        required,
        satisfied,
        projection_reference,
        labels: vec!["delegated_task.gate.metadata_only".to_string()],
        reasons: if satisfied {
            Vec::new()
        } else {
            vec![format!("delegated_task.gate.{kind:?}.unsatisfied")]
        },
        risk_label: if satisfied {
            ProposalRiskLabel::Low
        } else {
            ProposalRiskLabel::High
        },
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    });
}

#[allow(clippy::too_many_arguments)]
fn push_delegated_task_blocker(
    blockers: &mut Vec<DelegatedTaskPlanBlocker>,
    reason_code: &str,
    label: &str,
    gate: DelegatedTaskTrustGateKind,
    step_id: Option<DelegatedTaskStepId>,
    target_id: Option<String>,
    proposal_id: Option<ProposalId>,
    budget_id: Option<String>,
    projection_reference: Option<AssistedAiTrustProjectionReference>,
    schema_version: u16,
) {
    blockers.push(delegated_task_blocker(
        reason_code,
        label,
        gate,
        step_id,
        target_id,
        proposal_id,
        None,
        budget_id,
        projection_reference,
        schema_version,
    ));
}

#[allow(clippy::too_many_arguments)]
fn push_delegated_task_refusal(
    refusals: &mut Vec<DelegatedTaskPlanBlocker>,
    reason_code: &str,
    label: &str,
    gate: DelegatedTaskTrustGateKind,
    step_id: Option<DelegatedTaskStepId>,
    target_id: Option<String>,
    proposal_id: Option<ProposalId>,
    budget_id: Option<String>,
    projection_reference: Option<AssistedAiTrustProjectionReference>,
    schema_version: u16,
) {
    refusals.push(delegated_task_blocker(
        reason_code,
        label,
        gate,
        step_id,
        target_id,
        proposal_id,
        None,
        budget_id,
        projection_reference,
        schema_version,
    ));
}

#[allow(clippy::too_many_arguments)]
fn delegated_task_blocker(
    reason_code: &str,
    label: &str,
    gate: DelegatedTaskTrustGateKind,
    step_id: Option<DelegatedTaskStepId>,
    target_id: Option<String>,
    proposal_id: Option<ProposalId>,
    capability: Option<CapabilityId>,
    budget_id: Option<String>,
    projection_reference: Option<AssistedAiTrustProjectionReference>,
    schema_version: u16,
) -> DelegatedTaskPlanBlocker {
    DelegatedTaskPlanBlocker {
        reason_code: reason_code.to_string(),
        label: label.to_string(),
        gate,
        step_id,
        target_id,
        proposal_id,
        capability,
        budget_id,
        projection_reference,
        risk_label: ProposalRiskLabel::High,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        reasons: vec![reason_code.to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn delegated_task_plan_row(
    plan: &DelegatedTaskPlanContract,
    schema_version: u16,
) -> DelegatedTaskPlanRow {
    DelegatedTaskPlanRow {
        plan_id: plan.plan_id.clone(),
        workspace_id: plan.workspace_id,
        objective_summary_hash: plan.objective_summary_hash.clone(),
        plan_state: plan.plan_state,
        readiness: plan.audit_readiness.readiness,
        step_count: plan.steps.len() as u32,
        affected_target_count: plan.affected_targets.len() as u32,
        blocker_count: plan.blockers.len() as u32,
        refusal_count: plan.refusals.len() as u32,
        proposal_preview_link_count: plan.proposal_preview_links.len() as u32,
        risk_label: plan
            .risk_labels
            .iter()
            .copied()
            .fold(ProposalRiskLabel::Informational, max_risk_label),
        privacy_label: plan
            .privacy_labels
            .iter()
            .copied()
            .fold(ProposalPrivacyLabel::PublicMetadata, max_privacy_label),
        correlation_id: plan.correlation_id,
        causality_id: plan.causality_id,
        runtime_activation: DelegatedTaskRuntimeActivationState::NotEncoded,
        labels: vec!["delegated_task.plan_row.metadata_only".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn delegated_task_step_summaries(
    plan: &DelegatedTaskPlanContract,
    schema_version: u16,
) -> Vec<DelegatedTaskStepSummary> {
    plan.steps
        .iter()
        .map(|step| DelegatedTaskStepSummary {
            step_id: step.step_id.clone(),
            plan_id: plan.plan_id.clone(),
            order: step.order,
            objective_summary_hash: step.objective_summary_hash.clone(),
            operation_class: step.operation_class,
            state: step.state,
            dependency_count: step.depends_on.len() as u32,
            target_count: step.target_ids.len() as u32,
            proposal_id: step
                .proposal_preview
                .as_ref()
                .map(|preview| preview.proposal_id),
            blocker_count: step.blockers.len() as u32,
            risk_label: step.risk_label,
            privacy_label: step.privacy_label,
            labels: step.labels.clone(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        })
        .collect()
}

fn delegated_task_risk_labels(
    input: &DelegatedTaskPlanningBoundaryInput,
    blockers: &[DelegatedTaskPlanBlocker],
    refusals: &[DelegatedTaskPlanBlocker],
) -> Vec<ProposalRiskLabel> {
    let mut labels = Vec::new();
    labels.extend(
        input
            .affected_targets
            .iter()
            .map(|target| target.risk_label),
    );
    labels.extend(input.steps.iter().map(|step| step.risk_label));
    labels.extend(blockers.iter().map(|blocker| blocker.risk_label));
    labels.extend(refusals.iter().map(|refusal| refusal.risk_label));
    if labels.is_empty() {
        labels.push(ProposalRiskLabel::Informational);
    }
    labels
}

fn delegated_task_privacy_labels(
    input: &DelegatedTaskPlanningBoundaryInput,
    blockers: &[DelegatedTaskPlanBlocker],
    refusals: &[DelegatedTaskPlanBlocker],
) -> Vec<ProposalPrivacyLabel> {
    let mut labels = Vec::new();
    labels.extend(
        input
            .affected_targets
            .iter()
            .map(|target| target.privacy_label),
    );
    labels.extend(input.steps.iter().map(|step| step.privacy_label));
    labels.extend(blockers.iter().map(|blocker| blocker.privacy_label));
    labels.extend(refusals.iter().map(|refusal| refusal.privacy_label));
    if labels.is_empty() {
        labels.push(ProposalPrivacyLabel::WorkspaceMetadata);
    }
    labels
}

impl ContextManifestPreconditionSummary {
    /// Builds metadata-only precondition visibility and risk labels.
    pub fn from_preconditions(
        preconditions: &ProposalVersionPreconditions,
        schema_version: u16,
    ) -> Self {
        let file_content_version = preconditions
            .file_content_version
            .or(preconditions.file_version);
        let workspace_generation = preconditions
            .workspace_generation
            .or(preconditions.generation);
        let core_preconditions_present = file_content_version.is_some()
            && workspace_generation.is_some()
            && preconditions.buffer_version.is_some()
            && preconditions.snapshot_id.is_some();
        let mut risk_reasons = Vec::new();
        if file_content_version.is_none() {
            risk_reasons.push("missing.file_content_version".to_string());
        }
        if workspace_generation.is_none() {
            risk_reasons.push("missing.workspace_generation".to_string());
        }
        if preconditions.buffer_version.is_none() {
            risk_reasons.push("missing.buffer_version".to_string());
        }
        if preconditions.snapshot_id.is_none() {
            risk_reasons.push("missing.snapshot_id".to_string());
        }
        if preconditions.expected_fingerprint.is_none() {
            risk_reasons.push("missing.expected_fingerprint".to_string());
        }
        let risk_label =
            if core_preconditions_present && preconditions.expected_fingerprint.is_some() {
                ProposalRiskLabel::Low
            } else if core_preconditions_present {
                ProposalRiskLabel::Medium
            } else {
                ProposalRiskLabel::High
            };

        Self {
            file_content_version,
            buffer_version: preconditions.buffer_version,
            snapshot_id: preconditions.snapshot_id,
            workspace_generation,
            expected_fingerprint: preconditions.expected_fingerprint.clone(),
            expected_file_length: preconditions.expected_file_length,
            expected_modified_at: preconditions.expected_modified_at,
            core_preconditions_present,
            risk_label,
            risk_reasons,
            schema_version,
        }
    }
}

impl ContextManifestFreshnessSummary {
    /// Builds a metadata-only summary from semantic freshness-key metadata.
    pub fn from_semantic_freshness_key(
        key: &SemanticMetadataFreshnessKey,
        state: SemanticFreshnessState,
        observed_at: Option<TimestampMillis>,
        schema_version: u16,
    ) -> Self {
        let mut risk_reasons = Vec::new();
        if !matches!(state, SemanticFreshnessState::Fresh) {
            risk_reasons.push("semantic.freshness_not_fresh".to_string());
        }
        let risk_label = freshness_risk_label(state, true);
        Self {
            state,
            freshness_key_present: true,
            snapshot_id: key.snapshot_id,
            file_content_version: Some(key.file_content_version),
            workspace_generation: Some(key.workspace_generation),
            content_hash: Some(key.content_hash.clone()),
            privacy_scope: Some(key.privacy_scope),
            observed_at,
            risk_label,
            risk_reasons,
            schema_version,
        }
    }

    /// Builds a freshness-risk marker when freshness metadata is missing.
    pub fn missing(schema_version: u16) -> Self {
        Self {
            state: SemanticFreshnessState::Unavailable,
            freshness_key_present: false,
            snapshot_id: None,
            file_content_version: None,
            workspace_generation: None,
            content_hash: None,
            privacy_scope: None,
            observed_at: None,
            risk_label: ProposalRiskLabel::High,
            risk_reasons: vec!["missing.freshness_key".to_string()],
            schema_version,
        }
    }
}

impl ContextManifestRecord {
    /// Returns a metadata-only category summary for proposal ledger rows.
    pub fn to_summary(&self) -> ProposalContextManifestSummary {
        context_manifest_summary_from_items(
            self.manifest_id.clone(),
            &self.items,
            self.omitted_item_count,
            self.redaction_hints.clone(),
        )
    }
}

/// Builds a metadata-only privacy inspector projection from context manifest data.
pub fn privacy_inspector_from_context_manifest_projection(
    projection: &ContextManifestProjection,
    inspector_id: impl Into<String>,
    generated_at: TimestampMillis,
    schema_version: u16,
) -> PrivacyInspectorProjection {
    let manifest = &projection.manifest;
    let mut records = Vec::new();
    records.extend(
        manifest
            .items
            .iter()
            .map(|item| privacy_inspector_record_from_context_item(item, schema_version)),
    );
    records.extend(
        manifest
            .permissions
            .iter()
            .enumerate()
            .map(|(index, permission)| {
                privacy_inspector_record_from_permission_summary(
                    permission,
                    manifest.workspace_id,
                    manifest.proposal_id,
                    index,
                    schema_version,
                )
            }),
    );

    let denied_record_count = records
        .iter()
        .filter(|record| record.inclusion == ContextManifestInclusionState::Denied)
        .count() as u32;
    let redacted_record_count = records
        .iter()
        .filter(|record| record.redaction_state == PrivacyInspectorRedactionState::FullyRedacted)
        .count() as u32;
    let external_egress_record_count = records
        .iter()
        .filter(|record| {
            matches!(
                record.egress,
                ContextManifestEgressStatus::RemoteApprovalRequired
                    | ContextManifestEgressStatus::RemoteDenied
                    | ContextManifestEgressStatus::ExternalEgressMetadata
            )
        })
        .count() as u32;
    let high_risk_record_count = records
        .iter()
        .filter(|record| {
            matches!(
                record.risk_label,
                ProposalRiskLabel::High | ProposalRiskLabel::Unknown
            )
        })
        .count() as u32;
    let refusal = records
        .iter()
        .find(|record| record.inclusion == ContextManifestInclusionState::Denied)
        .map(|record| PrivacyInspectorRefusal {
            reason_code: "privacy.scope.denied".to_string(),
            label: "Privacy scope denied".to_string(),
            privacy_scope: record.privacy_scope,
            capability: record.permission_label.clone(),
            budget_id: None,
            risk_label: max_risk_label(record.risk_label, ProposalRiskLabel::High),
            reasons: record.reasons.clone(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        });

    PrivacyInspectorProjection {
        inspector_id: inspector_id.into(),
        manifest_id: Some(manifest.manifest_id.clone()),
        workspace_id: manifest.workspace_id,
        proposal_id: manifest.proposal_id,
        records,
        denied_record_count,
        redacted_record_count,
        external_egress_record_count,
        high_risk_record_count,
        refusal,
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Builds a metadata-only privacy inspector projection from a workspace proposal.
pub fn privacy_inspector_from_proposal(
    proposal: &WorkspaceProposal,
    inspector_id: impl Into<String>,
    manifest_id: impl Into<String>,
    context: PrivacyInspectorProposalContext,
    generated_at: TimestampMillis,
    schema_version: u16,
) -> PrivacyInspectorProjection {
    let manifest = context_manifest_from_proposal(
        proposal,
        manifest_id,
        context.workspace_trust_state,
        context.privacy_label,
        context.risk_label,
        generated_at,
        schema_version,
    );
    let projection = ContextManifestProjection {
        manifest,
        selected_item_id: None,
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    };
    privacy_inspector_from_context_manifest_projection(
        &projection,
        inspector_id,
        generated_at,
        schema_version,
    )
}

/// Builds a metadata-only permission-budget action summary from permission metadata.
pub fn permission_budget_action_from_permission_summary(
    permission: &ContextManifestPermissionSummary,
    action_id: impl Into<String>,
    action_class: PermissionBudgetActionClass,
    workspace_id: Option<WorkspaceId>,
    proposal_id: Option<ProposalId>,
    schema_version: u16,
) -> PermissionBudgetActionSummary {
    PermissionBudgetActionSummary {
        action_id: action_id.into(),
        action_class,
        capability: Some(permission.capability.clone()),
        workspace_id,
        proposal_id,
        target_id: None,
        privacy_scope: permission.privacy_scope,
        egress: permission.egress,
        estimated_units: 1,
        ranges: Vec::new(),
        counts: Vec::new(),
        hashes: Vec::new(),
        labels: vec![format!("permission.kind.{:?}", permission.kind)],
        risk_label: permission.risk_label,
        redaction_hints: permission.redaction_hints.clone(),
        schema_version,
    }
}

/// Evaluates a metadata-only action summary against a permission budget contract.
pub fn evaluate_permission_budget(
    budget: &PermissionBudgetContract,
    action: PermissionBudgetActionSummary,
    evaluation_id: impl Into<String>,
    schema_version: u16,
) -> PermissionBudgetEvaluation {
    let (disposition, reason_code, reason_label) = permission_budget_disposition(budget, &action);
    let allowed = disposition == PermissionBudgetEvaluationDisposition::Allowed;
    let attempted = action.estimated_units;
    let used = if allowed {
        budget.usage.used.saturating_add(attempted)
    } else {
        budget.usage.used
    };
    let usage_after = PermissionBudgetUsageSummary {
        unit_label: budget.usage.unit_label.clone(),
        used,
        ceiling: budget.usage.ceiling,
        remaining: budget
            .usage
            .ceiling
            .map(|ceiling| ceiling.saturating_sub(used)),
        attempted,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    };
    let refusal = (!allowed).then(|| PrivacyInspectorRefusal {
        reason_code: reason_code.clone(),
        label: reason_label,
        privacy_scope: Some(action.privacy_scope),
        capability: action
            .capability
            .clone()
            .or_else(|| budget.capability.clone()),
        budget_id: Some(budget.budget_id.clone()),
        risk_label: max_risk_label(budget.risk_label, action.risk_label),
        reasons: vec![reason_code.clone()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    });

    PermissionBudgetEvaluation {
        evaluation_id: evaluation_id.into(),
        budget_id: budget.budget_id.clone(),
        action,
        disposition,
        state: if allowed {
            PermissionBudgetState::Allowed
        } else if disposition == PermissionBudgetEvaluationDisposition::RefusedDepleted {
            PermissionBudgetState::Depleted
        } else {
            budget.state
        },
        allowed,
        usage_after,
        refusal,
        reasons: if allowed {
            vec!["budget.allowed.metadata_only".to_string()]
        } else {
            vec![reason_code]
        },
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Builds a static permission-budget projection from budgets and evaluations.
pub fn permission_budget_projection_from_contracts(
    projection_id: impl Into<String>,
    budgets: Vec<PermissionBudgetContract>,
    evaluations: Vec<PermissionBudgetEvaluation>,
    generated_at: TimestampMillis,
    schema_version: u16,
) -> PermissionBudgetProjection {
    let denied_budget_count = budgets
        .iter()
        .filter(|budget| budget.state == PermissionBudgetState::Denied)
        .count() as u32;
    let depleted_budget_count = budgets
        .iter()
        .filter(|budget| budget.state == PermissionBudgetState::Depleted)
        .count() as u32;
    let refused_evaluation_count = evaluations
        .iter()
        .filter(|evaluation| !evaluation.allowed)
        .count() as u32;
    PermissionBudgetProjection {
        projection_id: projection_id.into(),
        budgets,
        evaluations,
        denied_budget_count,
        depleted_budget_count,
        refused_evaluation_count,
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Returns the metadata-only payload kind for a proposal payload.
pub fn proposal_payload_kind(payload: &ProposalPayload) -> ProposalPayloadKind {
    match payload {
        ProposalPayload::TextEdit(_) => ProposalPayloadKind::TextEdit,
        ProposalPayload::CreateFile(_) => ProposalPayloadKind::CreateFile,
        ProposalPayload::DeleteFile(_) => ProposalPayloadKind::DeleteFile,
        ProposalPayload::RenameFile(_) => ProposalPayloadKind::RenameFile,
        ProposalPayload::SaveFile(_) => ProposalPayloadKind::SaveFile,
        ProposalPayload::FormatFile(_) => ProposalPayloadKind::FormatFile,
        ProposalPayload::CodeAction(_) => ProposalPayloadKind::CodeAction,
        ProposalPayload::WorkspaceEdit(_) => ProposalPayloadKind::WorkspaceEdit,
        ProposalPayload::TerminalCommand(_) => ProposalPayloadKind::TerminalCommand,
        ProposalPayload::Batch(_) => ProposalPayloadKind::Batch,
    }
}

/// Builds deterministic affected-target coverage from proposal metadata without mutation.
pub fn proposal_metadata_target_coverage(proposal: &WorkspaceProposal) -> ProposalTargetCoverage {
    if let Some(coverage) = proposal_declared_target_coverage(proposal)
        && coverage_is_declared(coverage)
    {
        return coverage.clone();
    }

    let mut targets = Vec::new();
    visit_proposal_payload_targets(&proposal.payload, &mut |target| targets.push(target));
    ProposalTargetCoverage {
        coverage_kind: ProposalTargetCoverageKind::Complete,
        targets,
        omitted_target_count: 0,
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
}

/// Builds a metadata-only checkpoint and rollback projection from proposal metadata.
#[allow(clippy::too_many_arguments)]
pub fn checkpoint_rollback_projection_from_proposal(
    projection_id: impl Into<String>,
    proposal: &WorkspaceProposal,
    lifecycle_state: ProposalLifecycleState,
    ledger_projection: Option<&ProposalLedgerProjection>,
    audit_status: CheckpointRollbackAuditStatus,
    causality_id: Option<CausalityId>,
    generated_at: TimestampMillis,
    schema_version: u16,
) -> CheckpointRollbackProjection {
    let coverage = proposal_metadata_target_coverage(proposal);
    let expected_preconditions = ContextManifestPreconditionSummary::from_preconditions(
        &proposal.preconditions,
        schema_version,
    );
    let ledger_rollback = ledger_projection
        .and_then(|projection| proposal_ledger_row(projection, proposal.proposal_id))
        .map(|row| row.rollback);
    let availability =
        ledger_rollback.unwrap_or_else(|| rollback_availability_from_payload(&proposal.payload));
    let rollback_step_count = rollback_step_count(&proposal.payload);
    let unsupported_target_count = coverage
        .targets
        .iter()
        .filter(|target| {
            matches!(
                target.kind,
                ProposalTargetKind::RemoteWorkspace
                    | ProposalTargetKind::CollaborationSession
                    | ProposalTargetKind::Plugin
                    | ProposalTargetKind::TerminalSession
            )
        })
        .count() as u32;
    let reversible_target_count = if matches!(
        availability,
        ProposalRollbackAvailability::Available
            | ProposalRollbackAvailability::BestEffort
            | ProposalRollbackAvailability::NotRequired
    ) {
        coverage
            .targets
            .len()
            .saturating_sub(unsupported_target_count as usize) as u32
    } else {
        0
    };
    let irreversible_target_count = if matches!(
        availability,
        ProposalRollbackAvailability::Unavailable | ProposalRollbackAvailability::Unknown
    ) {
        coverage.targets.len() as u32
    } else {
        unsupported_target_count
    };
    let mut limitations = rollback_limitations(
        availability,
        rollback_step_count,
        audit_status,
        &coverage,
        schema_version,
    );
    let checkpoint_available = matches!(
        availability,
        ProposalRollbackAvailability::Available | ProposalRollbackAvailability::BestEffort
    ) && audit_status == CheckpointRollbackAuditStatus::Available;
    let mut checkpoint_limitations = Vec::new();
    if !checkpoint_available {
        checkpoint_limitations.push(checkpoint_limitation(
            "checkpoint.unavailable",
            "Checkpoint metadata is not available for exact rollback",
            None,
            ProposalRiskLabel::High,
            schema_version,
        ));
    }
    checkpoint_limitations.extend(limitations.clone());

    let targets = coverage
        .targets
        .iter()
        .map(|target| checkpoint_target_summary(target, &proposal.preconditions, schema_version))
        .collect::<Vec<_>>();
    let mut risk_labels = vec![expected_preconditions.risk_label];
    if !limitations.is_empty() || !checkpoint_limitations.is_empty() {
        risk_labels.push(ProposalRiskLabel::High);
    }
    risk_labels.sort_by_key(|label| risk_label_rank(*label));
    risk_labels.dedup();
    limitations.sort_by(|left, right| left.reason_code.cmp(&right.reason_code));

    CheckpointRollbackProjection {
        projection_id: projection_id.into(),
        proposal_id: proposal.proposal_id,
        workspace_id: proposal_workspace_id_from_coverage(&coverage)
            .or_else(|| proposal_workspace_id(proposal)),
        payload_kind: proposal_payload_kind(&proposal.payload),
        lifecycle_state,
        correlation_id: proposal.correlation_id,
        causality_id,
        checkpoint: ProposalCheckpointProjection {
            checkpoint_id: format!("checkpoint:proposal:{}:metadata", proposal.proposal_id.0),
            available: checkpoint_available,
            target_count: coverage.targets.len() as u32,
            expected_preconditions,
            hashes: proposal
                .preconditions
                .expected_fingerprint
                .clone()
                .into_iter()
                .collect(),
            audit_status,
            labels: vec!["checkpoint.metadata_only".to_string()],
            limitations: checkpoint_limitations,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        },
        rollback: ProposalRollbackProjection {
            availability,
            rollback_step_count,
            reversible_target_count,
            irreversible_target_count,
            audit_status,
            labels: vec![format!("rollback.availability.{availability:?}")],
            limitations,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        },
        targets,
        risk_labels,
        privacy_labels: vec![ProposalPrivacyLabel::WorkspaceMetadata],
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Builds a metadata-only approval checklist from trust-layer projections.
#[allow(clippy::too_many_arguments)]
pub fn approval_checklist_from_trust_projections(
    checklist_id: impl Into<String>,
    proposal: &WorkspaceProposal,
    lifecycle_state: ProposalLifecycleState,
    ledger_projection: Option<&ProposalLedgerProjection>,
    context_manifest_projection: Option<&ContextManifestProjection>,
    privacy_inspector_projection: Option<&PrivacyInspectorProjection>,
    permission_budget_projection: Option<&PermissionBudgetProjection>,
    checkpoint_rollback_projection: Option<&CheckpointRollbackProjection>,
    audit_before_success_available: bool,
    causality_id: Option<CausalityId>,
    generated_at: TimestampMillis,
    schema_version: u16,
) -> ProposalApprovalChecklistProjection {
    let coverage = proposal_metadata_target_coverage(proposal);
    let gates = vec![
        context_manifest_gate(proposal, context_manifest_projection, schema_version),
        privacy_gate(
            privacy_inspector_projection,
            proposal.proposal_id,
            schema_version,
        ),
        permission_budget_gate(
            permission_budget_projection,
            proposal.proposal_id,
            schema_version,
        ),
        lifecycle_gate(lifecycle_state, schema_version),
        target_validation_gate(&coverage, schema_version),
        freshness_precondition_gate(
            proposal,
            context_manifest_projection,
            generated_at,
            schema_version,
        ),
        audit_gate(audit_before_success_available, schema_version),
        rollback_checkpoint_gate(
            checkpoint_rollback_projection,
            ledger_projection,
            proposal.proposal_id,
            schema_version,
        ),
        risk_label_gate(
            context_manifest_projection,
            privacy_inspector_projection,
            permission_budget_projection,
            checkpoint_rollback_projection,
            schema_version,
        ),
        explicit_denial_gate(
            lifecycle_state,
            privacy_inspector_projection,
            permission_budget_projection,
            checkpoint_rollback_projection,
            schema_version,
        ),
    ];

    let blockers = gates
        .iter()
        .filter(|gate| {
            matches!(
                gate.status,
                ApprovalChecklistGateStatus::Blocked | ApprovalChecklistGateStatus::Unknown
            )
        })
        .flat_map(|gate| gate.reasons.clone())
        .collect::<Vec<_>>();
    let mut risk_labels = gates.iter().map(|gate| gate.risk_label).collect::<Vec<_>>();
    risk_labels.sort_by_key(|label| risk_label_rank(*label));
    risk_labels.dedup();
    let mut privacy_labels = gates
        .iter()
        .map(|gate| gate.privacy_label)
        .collect::<Vec<_>>();
    privacy_labels.sort_by_key(|label| privacy_label_rank(*label));
    privacy_labels.dedup();
    let explicit_denial_reasons = gates
        .iter()
        .flat_map(|gate| gate.reasons.iter())
        .filter(|reason| {
            matches!(
                reason.gate,
                ApprovalChecklistGateKind::ExplicitDenialReasons
                    | ApprovalChecklistGateKind::PrivacyInspection
                    | ApprovalChecklistGateKind::PermissionBudget
            )
        })
        .map(|reason| reason.reason_code.clone())
        .collect::<Vec<_>>();
    let ready_for_approval = blockers.is_empty()
        && gates.iter().all(|gate| {
            matches!(
                gate.status,
                ApprovalChecklistGateStatus::Satisfied
                    | ApprovalChecklistGateStatus::Risk
                    | ApprovalChecklistGateStatus::NotRequired
            )
        });

    ProposalApprovalChecklistProjection {
        checklist_id: checklist_id.into(),
        proposal_id: proposal.proposal_id,
        workspace_id: proposal_workspace_id_from_coverage(&coverage)
            .or_else(|| proposal_workspace_id(proposal)),
        payload_kind: proposal_payload_kind(&proposal.payload),
        lifecycle_state,
        correlation_id: proposal.correlation_id,
        causality_id,
        ready_for_approval,
        gates,
        blockers,
        risk_labels,
        privacy_labels,
        explicit_denial_reasons,
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Builds a metadata-only context manifest from an existing proposal envelope.
pub fn context_manifest_from_proposal(
    proposal: &WorkspaceProposal,
    manifest_id: impl Into<String>,
    workspace_trust_state: Option<WorkspaceTrustState>,
    privacy_label: ProposalPrivacyLabel,
    risk_label: ProposalRiskLabel,
    generated_at: TimestampMillis,
    schema_version: u16,
) -> ContextManifestRecord {
    let precondition_summary = ContextManifestPreconditionSummary::from_preconditions(
        &proposal.preconditions,
        schema_version,
    );
    let precondition_risk = precondition_summary.risk_label;
    let mut items = vec![ContextManifestItem {
        item_id: format!("proposal:{}:preconditions", proposal.proposal_id.0),
        kind: ContextManifestItemKind::ProposalPreconditions,
        inclusion: ContextManifestInclusionState::Included,
        workspace_id: proposal_workspace_id(proposal),
        file_id: None,
        buffer_id: None,
        proposal_id: Some(proposal.proposal_id),
        target_id: None,
        path: None,
        ranges: Vec::new(),
        counts: Vec::new(),
        hashes: precondition_summary
            .expected_fingerprint
            .clone()
            .into_iter()
            .collect(),
        privacy_scope: None,
        privacy_label,
        risk_label: precondition_risk,
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: None,
        preconditions: Some(precondition_summary),
        labels: vec!["proposal.preconditions".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }];

    items.extend(
        proposal_target_coverage(proposal)
            .into_iter()
            .flat_map(|coverage| {
                coverage
                    .targets
                    .iter()
                    .map(move |target| (coverage, target))
            })
            .enumerate()
            .map(|(index, (_, target))| {
                context_manifest_item_from_proposal_target(
                    proposal.proposal_id,
                    target,
                    index,
                    privacy_label,
                    risk_label,
                    schema_version,
                )
            }),
    );

    let permission = ContextManifestPermissionSummary {
        kind: permission_kind_for_capability(&proposal.capability),
        capability: proposal.capability.clone(),
        principal: Some(proposal.principal.clone()),
        decision_id: None,
        granted: false,
        privacy_scope: SemanticPrivacyScope::MetadataOnly,
        egress: ContextManifestEgressStatus::LocalOnly,
        risk_label,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    };
    let stale_or_missing_metadata_risk_present = items.iter().any(|item| {
        matches!(
            item.risk_label,
            ProposalRiskLabel::Medium | ProposalRiskLabel::High | ProposalRiskLabel::Unknown
        )
    });

    ContextManifestRecord {
        manifest_id: manifest_id.into(),
        workspace_id: proposal_workspace_id(proposal),
        proposal_id: Some(proposal.proposal_id),
        purpose: ContextManifestPurpose::ProposalReview,
        workspace_trust_state,
        privacy_label,
        risk_label: max_risk_label(risk_label, precondition_risk),
        egress: ContextManifestEgressStatus::LocalOnly,
        items,
        permissions: vec![permission],
        omitted_item_count: proposal_target_coverage(proposal)
            .map_or(0, |coverage| coverage.omitted_target_count),
        stale_or_missing_metadata_risk_present,
        generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Builds a metadata-only item from semantic fabric job metadata.
pub fn context_manifest_item_from_semantic_fabric_job(
    request: &SemanticFabricJobRequest,
    index: usize,
    schema_version: u16,
) -> ContextManifestItem {
    ContextManifestItem {
        item_id: format!("semantic-job:{}:{}", index, request.job_id),
        kind: ContextManifestItemKind::SemanticFabricJob,
        inclusion: ContextManifestInclusionState::Included,
        workspace_id: Some(request.workspace_id),
        file_id: Some(request.file_id),
        buffer_id: None,
        proposal_id: None,
        target_id: None,
        path: Some(request.file_identity.canonical_path.clone()),
        ranges: request.descriptor.ranges.clone(),
        counts: vec![
            ContextManifestItemCount {
                label: "dependency_hints".to_string(),
                count: request.dependency_hints.len() as u32,
            },
            ContextManifestItemCount {
                label: "chunks".to_string(),
                count: request.descriptor.chunks.len() as u32,
            },
        ],
        hashes: vec![
            request.file_identity.content_hash.clone(),
            request.descriptor.content_hash.clone(),
        ],
        privacy_scope: Some(request.privacy.privacy_scope),
        privacy_label: privacy_label_from_scope(request.privacy.privacy_scope),
        risk_label: freshness_risk_label(
            if request.persisted_freshness_key.as_ref() == Some(&request.expected_freshness_key) {
                SemanticFreshnessState::Fresh
            } else {
                SemanticFreshnessState::Stale
            },
            request.persisted_freshness_key.is_some(),
        ),
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: Some(
            ContextManifestFreshnessSummary::from_semantic_freshness_key(
                &request.expected_freshness_key,
                if request.persisted_freshness_key.as_ref() == Some(&request.expected_freshness_key)
                {
                    SemanticFreshnessState::Fresh
                } else {
                    SemanticFreshnessState::Stale
                },
                None,
                schema_version,
            ),
        ),
        preconditions: None,
        labels: vec![
            format!("semantic.source.{:?}", request.source_kind),
            format!("semantic.trigger.{:?}", request.trigger),
        ],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Builds a metadata-only item from a semantic fabric schedule plan.
pub fn context_manifest_item_from_semantic_fabric_schedule_plan(
    plan: &SemanticFabricSchedulePlan,
    schema_version: u16,
) -> ContextManifestItem {
    let stale_or_missing = plan.decisions.iter().any(|decision| {
        !matches!(decision.freshness_state, SemanticFreshnessState::Fresh)
            || !decision.invalidation_causes.is_empty()
    });
    ContextManifestItem {
        item_id: format!("semantic-plan:{}", plan.correlation_id.0),
        kind: ContextManifestItemKind::SemanticFabricSchedulePlan,
        inclusion: ContextManifestInclusionState::Included,
        workspace_id: None,
        file_id: None,
        buffer_id: None,
        proposal_id: None,
        target_id: None,
        path: None,
        ranges: Vec::new(),
        counts: vec![
            ContextManifestItemCount {
                label: "decisions".to_string(),
                count: plan.decisions.len() as u32,
            },
            ContextManifestItemCount {
                label: "admitted".to_string(),
                count: plan.admitted_count,
            },
            ContextManifestItemCount {
                label: "capacity".to_string(),
                count: plan.capacity,
            },
        ],
        hashes: Vec::new(),
        privacy_scope: Some(SemanticPrivacyScope::MetadataOnly),
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: if stale_or_missing {
            ProposalRiskLabel::Medium
        } else {
            ProposalRiskLabel::Informational
        },
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: if stale_or_missing {
            Some(ContextManifestFreshnessSummary::missing(schema_version))
        } else {
            None
        },
        preconditions: None,
        labels: vec!["semantic.fabric.schedule_plan".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Builds a metadata-only item from an LSP diagnostic summary.
pub fn context_manifest_item_from_lsp_diagnostic_summary(
    summary: &LspDiagnosticSummary,
    index: usize,
    schema_version: u16,
) -> ContextManifestItem {
    ContextManifestItem {
        item_id: format!("lsp-diagnostics:{}:{}", summary.file_id.0, index),
        kind: ContextManifestItemKind::LspDiagnosticSummary,
        inclusion: ContextManifestInclusionState::Included,
        workspace_id: Some(summary.workspace_id),
        file_id: Some(summary.file_id),
        buffer_id: None,
        proposal_id: None,
        target_id: None,
        path: None,
        ranges: summary
            .ranges
            .iter()
            .filter_map(protocol_text_range_to_byte_range)
            .collect(),
        counts: vec![
            ContextManifestItemCount {
                label: "diagnostics".to_string(),
                count: summary.diagnostic_count,
            },
            ContextManifestItemCount {
                label: "errors".to_string(),
                count: summary.error_count,
            },
            ContextManifestItemCount {
                label: "warnings".to_string(),
                count: summary.warning_count,
            },
        ],
        hashes: summary
            .content_hash
            .clone()
            .into_iter()
            .chain(summary.diagnostic_hashes.clone())
            .chain(summary.source_hashes.clone())
            .collect(),
        privacy_scope: Some(summary.privacy_scope),
        privacy_label: privacy_label_from_scope(summary.privacy_scope),
        risk_label: freshness_risk_label(summary.freshness, summary.content_hash.is_some()),
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: Some(ContextManifestFreshnessSummary {
            state: summary.freshness,
            freshness_key_present: summary.content_hash.is_some(),
            snapshot_id: Some(summary.snapshot_id),
            file_content_version: None,
            workspace_generation: None,
            content_hash: summary.content_hash.clone(),
            privacy_scope: Some(summary.privacy_scope),
            observed_at: None,
            risk_label: freshness_risk_label(summary.freshness, summary.content_hash.is_some()),
            risk_reasons: freshness_risk_reasons(summary.freshness, summary.content_hash.is_some()),
            schema_version,
        }),
        preconditions: None,
        labels: vec!["lsp.diagnostics.summary".to_string()],
        redaction_hints: summary.redaction_hints.clone(),
        schema_version,
    }
}

/// Builds metadata-only items from semantic metadata records.
pub fn context_manifest_item_from_semantic_metadata_record(
    record: &SemanticMetadataRecord,
    index: usize,
    schema_version: u16,
) -> ContextManifestItem {
    ContextManifestItem {
        item_id: format!("semantic-record:{}:{}", index, record.record_id.0),
        kind: ContextManifestItemKind::SemanticRecord,
        inclusion: ContextManifestInclusionState::Included,
        workspace_id: Some(record.workspace_id),
        file_id: Some(record.file_id),
        buffer_id: None,
        proposal_id: None,
        target_id: None,
        path: Some(record.file_identity.canonical_path.clone()),
        ranges: record
            .symbols
            .iter()
            .filter_map(|symbol| symbol.declaration_range.as_ref())
            .filter_map(protocol_text_range_to_byte_range)
            .collect(),
        counts: vec![
            ContextManifestItemCount {
                label: "symbols".to_string(),
                count: record.symbols.len() as u32,
            },
            ContextManifestItemCount {
                label: "graph_records".to_string(),
                count: record.graph_records.len() as u32,
            },
            ContextManifestItemCount {
                label: "diagnostics".to_string(),
                count: record
                    .diagnostic_summaries
                    .iter()
                    .map(|summary| summary.count)
                    .sum(),
            },
        ],
        hashes: vec![record.file_identity.content_hash.clone()],
        privacy_scope: Some(record.freshness_key.privacy_scope),
        privacy_label: privacy_label_from_scope(record.freshness_key.privacy_scope),
        risk_label: freshness_risk_label(record.freshness_state, true),
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: Some(
            ContextManifestFreshnessSummary::from_semantic_freshness_key(
                &record.freshness_key,
                record.freshness_state,
                Some(record.persisted_at),
                schema_version,
            ),
        ),
        preconditions: None,
        labels: vec![format!(
            "semantic.provenance.{:?}",
            record.provenance.source
        )],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

/// Builds a metadata-only item from an LSP supervision event.
pub fn context_manifest_item_from_lsp_supervision_event(
    event: &LspSupervisionEvent,
    schema_version: u16,
) -> ContextManifestItem {
    ContextManifestItem {
        item_id: format!("lsp-supervision:{}", event.sequence.0),
        kind: ContextManifestItemKind::LspSupervisionSummary,
        inclusion: ContextManifestInclusionState::Included,
        workspace_id: Some(event.identity.workspace_id),
        file_id: event.request.as_ref().and_then(|request| request.file_id),
        buffer_id: None,
        proposal_id: None,
        target_id: None,
        path: None,
        ranges: Vec::new(),
        counts: vec![
            ContextManifestItemCount {
                label: "capabilities".to_string(),
                count: event.capabilities.len() as u32,
            },
            ContextManifestItemCount {
                label: "diagnostic_summaries".to_string(),
                count: event.diagnostic_summaries.len() as u32,
            },
        ],
        hashes: vec![event.identity.command_hash.clone()],
        privacy_scope: event.request.as_ref().map(|request| request.privacy_scope),
        privacy_label: event
            .request
            .as_ref()
            .map(|request| privacy_label_from_scope(request.privacy_scope))
            .unwrap_or(ProposalPrivacyLabel::WorkspaceMetadata),
        risk_label: if matches!(
            event.lifecycle_state,
            LspSupervisionLifecycleState::Failed | LspSupervisionLifecycleState::CircuitOpen
        ) {
            ProposalRiskLabel::High
        } else {
            ProposalRiskLabel::Informational
        },
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: None,
        preconditions: None,
        labels: vec![
            format!("lsp.lifecycle.{:?}", event.lifecycle_state),
            format!("lsp.health.{:?}", event.health_state),
        ],
        redaction_hints: event.redaction_hints.clone(),
        schema_version,
    }
}

/// Builds a proposal-ledger-compatible summary from context manifest items.
pub fn context_manifest_summary_from_items(
    manifest_id: String,
    items: &[ContextManifestItem],
    omitted_item_count: u32,
    redaction_hints: Vec<RedactionHint>,
) -> ProposalContextManifestSummary {
    let mut categories: Vec<ProposalContextManifestEntrySummary> = Vec::new();
    for item in items {
        let category = format!("{:?}", item.kind);
        if let Some(existing) = categories
            .iter_mut()
            .find(|entry| entry.category == category)
        {
            existing.item_count = existing.item_count.saturating_add(1);
            if matches!(
                item.inclusion,
                ContextManifestInclusionState::Omitted | ContextManifestInclusionState::Redacted
            ) {
                existing.omitted_item_count = existing.omitted_item_count.saturating_add(1);
            }
            existing.privacy_label = max_privacy_label(existing.privacy_label, item.privacy_label);
            continue;
        }
        categories.push(ProposalContextManifestEntrySummary {
            category,
            item_count: 1,
            omitted_item_count: if matches!(
                item.inclusion,
                ContextManifestInclusionState::Omitted | ContextManifestInclusionState::Redacted
            ) {
                1
            } else {
                0
            },
            privacy_label: item.privacy_label,
            manifest_hash: item.hashes.first().cloned(),
            redaction_hints: item.redaction_hints.clone(),
        });
    }
    let redacted_count = items
        .iter()
        .filter(|item| {
            matches!(
                item.inclusion,
                ContextManifestInclusionState::Omitted | ContextManifestInclusionState::Redacted
            )
        })
        .count() as u32;
    ProposalContextManifestSummary {
        manifest_id,
        category_count: categories.len() as u32,
        total_item_count: items.len() as u32,
        omitted_item_count: omitted_item_count.saturating_add(redacted_count),
        categories,
        redaction_hints,
    }
}

fn context_manifest_item_from_proposal_target(
    proposal_id: ProposalId,
    target: &ProposalAffectedTarget,
    index: usize,
    privacy_label: ProposalPrivacyLabel,
    risk_label: ProposalRiskLabel,
    schema_version: u16,
) -> ContextManifestItem {
    ContextManifestItem {
        item_id: format!("proposal:{}:target:{}", proposal_id.0, index),
        kind: ContextManifestItemKind::ProposalTarget,
        inclusion: ContextManifestInclusionState::Included,
        workspace_id: target.workspace_id,
        file_id: target.file_id,
        buffer_id: target.buffer_id,
        proposal_id: Some(proposal_id),
        target_id: Some(target.target_id.clone()),
        path: target.path.clone(),
        ranges: target.byte_ranges.clone(),
        counts: vec![ContextManifestItemCount {
            label: "ranges".to_string(),
            count: target.byte_ranges.len() as u32,
        }],
        hashes: Vec::new(),
        privacy_scope: Some(SemanticPrivacyScope::MetadataOnly),
        privacy_label,
        risk_label,
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: None,
        preconditions: None,
        labels: vec![format!("proposal.target.{:?}", target.kind)],
        redaction_hints: target.redaction_hints.clone(),
        schema_version,
    }
}

fn proposal_workspace_id(proposal: &WorkspaceProposal) -> Option<WorkspaceId> {
    match &proposal.payload {
        ProposalPayload::TextEdit(_) => None,
        ProposalPayload::CreateFile(_) => None,
        ProposalPayload::DeleteFile(payload) => Some(payload.file.workspace_id),
        ProposalPayload::RenameFile(payload) => Some(payload.file.workspace_id),
        ProposalPayload::SaveFile(payload) => Some(payload.file.workspace_id),
        ProposalPayload::FormatFile(payload) => Some(payload.file.workspace_id),
        ProposalPayload::CodeAction(payload) => Some(payload.file.workspace_id),
        ProposalPayload::WorkspaceEdit(payload) => Some(payload.workspace_id),
        ProposalPayload::TerminalCommand(_) => None,
        ProposalPayload::Batch(payload) => payload
            .target_coverage
            .targets
            .iter()
            .find_map(|target| target.workspace_id),
    }
}

fn proposal_target_coverage(proposal: &WorkspaceProposal) -> Option<&ProposalTargetCoverage> {
    match &proposal.payload {
        ProposalPayload::WorkspaceEdit(payload) => Some(&payload.target_coverage),
        ProposalPayload::Batch(payload) => Some(&payload.target_coverage),
        _ => None,
    }
}

fn privacy_label_from_scope(scope: SemanticPrivacyScope) -> ProposalPrivacyLabel {
    match scope {
        SemanticPrivacyScope::Public => ProposalPrivacyLabel::PublicMetadata,
        SemanticPrivacyScope::Workspace
        | SemanticPrivacyScope::Project
        | SemanticPrivacyScope::File => ProposalPrivacyLabel::WorkspaceMetadata,
        SemanticPrivacyScope::MetadataOnly => ProposalPrivacyLabel::WorkspaceMetadata,
        SemanticPrivacyScope::Redacted => ProposalPrivacyLabel::RedactedSensitive,
    }
}

fn max_privacy_label(a: ProposalPrivacyLabel, b: ProposalPrivacyLabel) -> ProposalPrivacyLabel {
    if privacy_label_rank(b) > privacy_label_rank(a) {
        b
    } else {
        a
    }
}

fn privacy_label_rank(label: ProposalPrivacyLabel) -> u8 {
    match label {
        ProposalPrivacyLabel::PublicMetadata => 0,
        ProposalPrivacyLabel::WorkspaceMetadata => 1,
        ProposalPrivacyLabel::RedactedSensitive => 2,
        ProposalPrivacyLabel::ExternalEgressMetadata => 3,
        ProposalPrivacyLabel::Unknown => 4,
    }
}

fn max_risk_label(a: ProposalRiskLabel, b: ProposalRiskLabel) -> ProposalRiskLabel {
    if risk_label_rank(b) > risk_label_rank(a) {
        b
    } else {
        a
    }
}

fn risk_label_rank(label: ProposalRiskLabel) -> u8 {
    match label {
        ProposalRiskLabel::Informational => 0,
        ProposalRiskLabel::Low => 1,
        ProposalRiskLabel::Medium => 2,
        ProposalRiskLabel::High => 3,
        ProposalRiskLabel::Unknown => 4,
    }
}

fn freshness_risk_label(
    state: SemanticFreshnessState,
    freshness_key_present: bool,
) -> ProposalRiskLabel {
    if !freshness_key_present {
        return ProposalRiskLabel::High;
    }
    match state {
        SemanticFreshnessState::Fresh => ProposalRiskLabel::Informational,
        SemanticFreshnessState::Partial => ProposalRiskLabel::Medium,
        SemanticFreshnessState::Stale | SemanticFreshnessState::Unavailable => {
            ProposalRiskLabel::High
        }
    }
}

fn freshness_risk_reasons(
    state: SemanticFreshnessState,
    freshness_key_present: bool,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if !freshness_key_present {
        reasons.push("missing.freshness_key".to_string());
    }
    match state {
        SemanticFreshnessState::Fresh => {}
        SemanticFreshnessState::Partial => reasons.push("freshness.partial".to_string()),
        SemanticFreshnessState::Stale => reasons.push("freshness.stale".to_string()),
        SemanticFreshnessState::Unavailable => reasons.push("freshness.unavailable".to_string()),
    }
    reasons
}

fn permission_kind_for_capability(capability: &CapabilityId) -> ContextManifestPermissionKind {
    if capability.0.starts_with("fs.") {
        ContextManifestPermissionKind::Filesystem
    } else if capability.0.starts_with("lsp.") {
        ContextManifestPermissionKind::Lsp
    } else if capability.0.starts_with("semantic.") {
        ContextManifestPermissionKind::Semantic
    } else if capability.0.starts_with("model.") || capability.0.starts_with("provider.") {
        ContextManifestPermissionKind::ModelProvider
    } else if capability.0.starts_with("network.") || capability.0.starts_with("remote.") {
        ContextManifestPermissionKind::Network
    } else {
        ContextManifestPermissionKind::Tool
    }
}

fn privacy_inspector_record_from_context_item(
    item: &ContextManifestItem,
    schema_version: u16,
) -> PrivacyInspectorExposureRecord {
    let source_kind = match item.kind {
        ContextManifestItemKind::ProposalTarget
        | ContextManifestItemKind::ProposalPreconditions => {
            PrivacyInspectorSourceKind::ProposalTarget
        }
        ContextManifestItemKind::SemanticRecord
        | ContextManifestItemKind::SemanticFabricJob
        | ContextManifestItemKind::SemanticFabricSchedulePlan => {
            PrivacyInspectorSourceKind::SemanticMetadata
        }
        ContextManifestItemKind::LspDiagnosticSummary
        | ContextManifestItemKind::LspSupervisionSummary => PrivacyInspectorSourceKind::LspMetadata,
        ContextManifestItemKind::ModelPermission => PrivacyInspectorSourceKind::ProviderPermission,
        ContextManifestItemKind::ToolPermission => PrivacyInspectorSourceKind::ToolPermission,
        _ => PrivacyInspectorSourceKind::ContextManifestItem,
    };
    let mut reasons = item.labels.clone();
    reasons.extend(
        item.freshness
            .iter()
            .flat_map(|freshness| freshness.risk_reasons.clone()),
    );
    reasons.extend(
        item.preconditions
            .iter()
            .flat_map(|preconditions| preconditions.risk_reasons.clone()),
    );
    if item.inclusion == ContextManifestInclusionState::Denied {
        reasons.push("privacy.scope.denied".to_string());
    }

    PrivacyInspectorExposureRecord {
        exposure_id: format!("exposure:{}", item.item_id),
        source_kind,
        context_item_id: Some(item.item_id.clone()),
        proposal_id: item.proposal_id,
        target_id: item.target_id.clone(),
        workspace_id: item.workspace_id,
        file_id: item.file_id,
        buffer_id: item.buffer_id,
        privacy_scope: item.privacy_scope,
        privacy_label: item.privacy_label,
        redaction_state: redaction_state_for(item.inclusion, &item.redaction_hints),
        inclusion: item.inclusion,
        egress: item.egress,
        risk_label: item.risk_label,
        permission_label: None,
        ranges: item.ranges.clone(),
        counts: item.counts.clone(),
        hashes: item.hashes.clone(),
        labels: item.labels.clone(),
        reasons,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn privacy_inspector_record_from_permission_summary(
    permission: &ContextManifestPermissionSummary,
    workspace_id: Option<WorkspaceId>,
    proposal_id: Option<ProposalId>,
    index: usize,
    schema_version: u16,
) -> PrivacyInspectorExposureRecord {
    let source_kind = match permission.kind {
        ContextManifestPermissionKind::ModelProvider | ContextManifestPermissionKind::Network => {
            PrivacyInspectorSourceKind::ProviderPermission
        }
        ContextManifestPermissionKind::Tool => PrivacyInspectorSourceKind::ToolPermission,
        ContextManifestPermissionKind::Lsp => PrivacyInspectorSourceKind::LspMetadata,
        ContextManifestPermissionKind::Semantic => PrivacyInspectorSourceKind::SemanticMetadata,
        ContextManifestPermissionKind::Filesystem => {
            PrivacyInspectorSourceKind::ContextManifestItem
        }
    };
    let inclusion = if permission.granted {
        ContextManifestInclusionState::Included
    } else {
        ContextManifestInclusionState::Denied
    };
    let mut reasons = vec![format!("permission.kind.{:?}", permission.kind)];
    if !permission.granted {
        reasons.push("permission.not_granted".to_string());
    }

    PrivacyInspectorExposureRecord {
        exposure_id: format!("permission:{}:{}", permission.capability.0, index),
        source_kind,
        context_item_id: None,
        proposal_id,
        target_id: None,
        workspace_id,
        file_id: None,
        buffer_id: None,
        privacy_scope: Some(permission.privacy_scope),
        privacy_label: privacy_label_from_scope(permission.privacy_scope),
        redaction_state: redaction_state_for(inclusion, &permission.redaction_hints),
        inclusion,
        egress: permission.egress,
        risk_label: permission.risk_label,
        permission_label: Some(permission.capability.clone()),
        ranges: Vec::new(),
        counts: Vec::new(),
        hashes: Vec::new(),
        labels: vec![format!("permission.kind.{:?}", permission.kind)],
        reasons,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn redaction_state_for(
    inclusion: ContextManifestInclusionState,
    hints: &[RedactionHint],
) -> PrivacyInspectorRedactionState {
    if inclusion == ContextManifestInclusionState::Denied || hints.contains(&RedactionHint::Full) {
        PrivacyInspectorRedactionState::FullyRedacted
    } else {
        PrivacyInspectorRedactionState::MetadataOnly
    }
}

fn permission_budget_disposition(
    budget: &PermissionBudgetContract,
    action: &PermissionBudgetActionSummary,
) -> (PermissionBudgetEvaluationDisposition, String, String) {
    if budget.action_class != action.action_class {
        return (
            PermissionBudgetEvaluationDisposition::RefusedActionClassMismatch,
            "budget.action_class_mismatch".to_string(),
            "Permission budget action class mismatch".to_string(),
        );
    }
    if matches!(action.privacy_scope, SemanticPrivacyScope::Redacted)
        || matches!(budget.privacy_scope, SemanticPrivacyScope::Redacted)
        || matches!(action.egress, ContextManifestEgressStatus::RemoteDenied)
    {
        return (
            PermissionBudgetEvaluationDisposition::RefusedPrivacyScope,
            "privacy.scope.denied".to_string(),
            "Privacy scope denied".to_string(),
        );
    }
    if budget.state == PermissionBudgetState::Denied {
        return (
            PermissionBudgetEvaluationDisposition::RefusedDenied,
            "budget.denied".to_string(),
            "Permission budget denied".to_string(),
        );
    }
    if matches!(
        budget.consent_requirement_label,
        PermissionBudgetConsentRequirementLabel::Required
            | PermissionBudgetConsentRequirementLabel::RenewalRequired
            | PermissionBudgetConsentRequirementLabel::DeniedByPolicy
    ) {
        return (
            PermissionBudgetEvaluationDisposition::RefusedConsentRequired,
            "consent.required".to_string(),
            "Permission budget requires consent".to_string(),
        );
    }
    if budget.state == PermissionBudgetState::Depleted
        || budget.usage.ceiling.is_some_and(|ceiling| {
            budget.usage.used.saturating_add(action.estimated_units) > ceiling
        })
    {
        return (
            PermissionBudgetEvaluationDisposition::RefusedDepleted,
            "budget.depleted".to_string(),
            "Permission budget depleted".to_string(),
        );
    }

    (
        PermissionBudgetEvaluationDisposition::Allowed,
        "budget.allowed.metadata_only".to_string(),
        "Permission budget allows metadata-only action class".to_string(),
    )
}

fn approval_reason(
    gate: ApprovalChecklistGateKind,
    reason_code: impl Into<String>,
    label: impl Into<String>,
    risk_label: ProposalRiskLabel,
    privacy_label: ProposalPrivacyLabel,
    schema_version: u16,
) -> ApprovalChecklistReason {
    ApprovalChecklistReason {
        gate,
        reason_code: reason_code.into(),
        label: label.into(),
        target_id: None,
        budget_id: None,
        capability: None,
        risk_label,
        privacy_label,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn approval_gate_summary(
    gate: ApprovalChecklistGateKind,
    status: ApprovalChecklistGateStatus,
    risk_label: ProposalRiskLabel,
    privacy_label: ProposalPrivacyLabel,
    labels: Vec<String>,
    reasons: Vec<ApprovalChecklistReason>,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    ApprovalChecklistGateSummary {
        gate,
        status,
        risk_label,
        privacy_label,
        labels,
        reasons,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn context_manifest_gate(
    proposal: &WorkspaceProposal,
    projection: Option<&ContextManifestProjection>,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    let gate = ApprovalChecklistGateKind::ContextManifestCompleteness;
    let Some(projection) = projection else {
        return approval_gate_summary(
            gate,
            ApprovalChecklistGateStatus::Blocked,
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::Unknown,
            vec!["context_manifest.missing".to_string()],
            vec![approval_reason(
                gate,
                "context_manifest.missing",
                "Context manifest projection is missing",
                ProposalRiskLabel::High,
                ProposalPrivacyLabel::Unknown,
                schema_version,
            )],
            schema_version,
        );
    };
    let manifest = &projection.manifest;
    let mut reasons = Vec::new();
    if manifest.proposal_id != Some(proposal.proposal_id) {
        reasons.push(approval_reason(
            gate,
            "context_manifest.proposal_mismatch",
            "Context manifest is not linked to this proposal",
            ProposalRiskLabel::High,
            manifest.privacy_label,
            schema_version,
        ));
    }
    if manifest.items.is_empty() {
        reasons.push(approval_reason(
            gate,
            "context_manifest.empty",
            "Context manifest has no metadata items",
            ProposalRiskLabel::High,
            manifest.privacy_label,
            schema_version,
        ));
    }
    if manifest.omitted_item_count > 0 {
        reasons.push(approval_reason(
            gate,
            "context_manifest.omitted_items",
            "Context manifest has omitted metadata items",
            ProposalRiskLabel::Medium,
            manifest.privacy_label,
            schema_version,
        ));
    }
    let status = if reasons.iter().any(|reason| {
        matches!(
            reason.risk_label,
            ProposalRiskLabel::High | ProposalRiskLabel::Unknown
        )
    }) {
        ApprovalChecklistGateStatus::Blocked
    } else if reasons.is_empty() {
        ApprovalChecklistGateStatus::Satisfied
    } else {
        ApprovalChecklistGateStatus::Risk
    };
    approval_gate_summary(
        gate,
        status,
        max_risk_label(
            manifest.risk_label,
            reasons
                .iter()
                .fold(ProposalRiskLabel::Informational, |acc, reason| {
                    max_risk_label(acc, reason.risk_label)
                }),
        ),
        manifest.privacy_label,
        vec![format!("context_manifest.items:{}", manifest.items.len())],
        reasons,
        schema_version,
    )
}

fn privacy_gate(
    projection: Option<&PrivacyInspectorProjection>,
    proposal_id: ProposalId,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    let gate = ApprovalChecklistGateKind::PrivacyInspection;
    let Some(projection) = projection else {
        return approval_gate_summary(
            gate,
            ApprovalChecklistGateStatus::Blocked,
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::Unknown,
            vec!["privacy_inspector.missing".to_string()],
            vec![approval_reason(
                gate,
                "privacy_inspector.missing",
                "Privacy inspector projection is missing",
                ProposalRiskLabel::High,
                ProposalPrivacyLabel::Unknown,
                schema_version,
            )],
            schema_version,
        );
    };
    let mut reasons = Vec::new();
    if projection.proposal_id != Some(proposal_id) {
        reasons.push(approval_reason(
            gate,
            "privacy_inspector.proposal_mismatch",
            "Privacy inspector is not linked to this proposal",
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::Unknown,
            schema_version,
        ));
    }
    if let Some(refusal) = &projection.refusal {
        let mut reason = approval_reason(
            gate,
            refusal.reason_code.clone(),
            refusal.label.clone(),
            max_risk_label(refusal.risk_label, ProposalRiskLabel::High),
            refusal
                .privacy_scope
                .map(privacy_label_from_scope)
                .unwrap_or(ProposalPrivacyLabel::Unknown),
            schema_version,
        );
        reason.capability = refusal.capability.clone();
        reason.budget_id = refusal.budget_id.clone();
        reasons.push(reason);
    }
    if projection.denied_record_count > 0 {
        reasons.push(approval_reason(
            gate,
            "privacy_inspector.denied_records",
            "Privacy inspector has denied records",
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::RedactedSensitive,
            schema_version,
        ));
    }
    let status = if reasons.is_empty() {
        ApprovalChecklistGateStatus::Satisfied
    } else {
        ApprovalChecklistGateStatus::Blocked
    };
    approval_gate_summary(
        gate,
        status,
        if projection.high_risk_record_count > 0 || !reasons.is_empty() {
            ProposalRiskLabel::High
        } else {
            ProposalRiskLabel::Low
        },
        if projection.external_egress_record_count > 0 {
            ProposalPrivacyLabel::ExternalEgressMetadata
        } else if projection.redacted_record_count > 0 {
            ProposalPrivacyLabel::RedactedSensitive
        } else {
            ProposalPrivacyLabel::WorkspaceMetadata
        },
        vec![format!("privacy.records:{}", projection.records.len())],
        reasons,
        schema_version,
    )
}

fn permission_budget_gate(
    projection: Option<&PermissionBudgetProjection>,
    proposal_id: ProposalId,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    let gate = ApprovalChecklistGateKind::PermissionBudget;
    let Some(projection) = projection else {
        return approval_gate_summary(
            gate,
            ApprovalChecklistGateStatus::Blocked,
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::Unknown,
            vec!["permission_budget.missing".to_string()],
            vec![approval_reason(
                gate,
                "permission_budget.missing",
                "Permission budget projection is missing",
                ProposalRiskLabel::High,
                ProposalPrivacyLabel::Unknown,
                schema_version,
            )],
            schema_version,
        );
    };
    let mut reasons = Vec::new();
    for budget in &projection.budgets {
        if budget.state == PermissionBudgetState::Denied
            || budget.state == PermissionBudgetState::Depleted
        {
            let mut reason = approval_reason(
                gate,
                match budget.state {
                    PermissionBudgetState::Denied => "budget.denied",
                    PermissionBudgetState::Depleted => "budget.depleted",
                    PermissionBudgetState::Allowed => "budget.allowed",
                },
                format!(
                    "Permission budget {} is {:?}",
                    budget.budget_id, budget.state
                ),
                max_risk_label(budget.risk_label, ProposalRiskLabel::High),
                privacy_label_from_scope(budget.privacy_scope),
                schema_version,
            );
            reason.budget_id = Some(budget.budget_id.clone());
            reason.capability = budget.capability.clone();
            reasons.push(reason);
        }
    }
    for evaluation in projection.evaluations.iter().filter(|evaluation| {
        evaluation.action.proposal_id == Some(proposal_id) && !evaluation.allowed
    }) {
        let mut reason = approval_reason(
            gate,
            evaluation
                .refusal
                .as_ref()
                .map(|refusal| refusal.reason_code.clone())
                .unwrap_or_else(|| format!("budget.evaluation.{:?}", evaluation.disposition)),
            format!(
                "Permission budget evaluation {} refused",
                evaluation.evaluation_id
            ),
            max_risk_label(evaluation.action.risk_label, ProposalRiskLabel::High),
            privacy_label_from_scope(evaluation.action.privacy_scope),
            schema_version,
        );
        reason.budget_id = Some(evaluation.budget_id.clone());
        reason.capability = evaluation.action.capability.clone();
        reasons.push(reason);
    }
    let status = if reasons.is_empty() {
        ApprovalChecklistGateStatus::Satisfied
    } else {
        ApprovalChecklistGateStatus::Blocked
    };
    approval_gate_summary(
        gate,
        status,
        if reasons.is_empty() {
            ProposalRiskLabel::Low
        } else {
            ProposalRiskLabel::High
        },
        ProposalPrivacyLabel::WorkspaceMetadata,
        vec![format!(
            "permission_budget.evaluations:{}",
            projection.evaluations.len()
        )],
        reasons,
        schema_version,
    )
}

fn lifecycle_gate(
    lifecycle_state: ProposalLifecycleState,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    let gate = ApprovalChecklistGateKind::ProposalLifecycle;
    let ready = lifecycle_state == ProposalLifecycleState::Previewed;
    approval_gate_summary(
        gate,
        if ready {
            ApprovalChecklistGateStatus::Satisfied
        } else {
            ApprovalChecklistGateStatus::Blocked
        },
        if ready {
            ProposalRiskLabel::Low
        } else {
            ProposalRiskLabel::High
        },
        ProposalPrivacyLabel::WorkspaceMetadata,
        vec![format!("proposal.lifecycle.{lifecycle_state:?}")],
        if ready {
            Vec::new()
        } else {
            vec![approval_reason(
                gate,
                "proposal.lifecycle_not_previewed",
                "Proposal must be previewed before approval readiness",
                ProposalRiskLabel::High,
                ProposalPrivacyLabel::WorkspaceMetadata,
                schema_version,
            )]
        },
        schema_version,
    )
}

fn target_validation_gate(
    coverage: &ProposalTargetCoverage,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    let gate = ApprovalChecklistGateKind::TargetValidation;
    let mut reasons = Vec::new();
    if coverage.coverage_kind != ProposalTargetCoverageKind::Complete {
        reasons.push(approval_reason(
            gate,
            "proposal.incomplete_target_coverage",
            "Proposal target coverage is not complete",
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::WorkspaceMetadata,
            schema_version,
        ));
    }
    if coverage.targets.is_empty() {
        reasons.push(approval_reason(
            gate,
            "proposal.missing_targets",
            "Proposal has no affected-target metadata",
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::WorkspaceMetadata,
            schema_version,
        ));
    }
    for target in &coverage.targets {
        if target.target_id.trim().is_empty() {
            let mut reason = approval_reason(
                gate,
                "proposal.unknown_target",
                "Affected target has no stable identifier",
                ProposalRiskLabel::High,
                ProposalPrivacyLabel::WorkspaceMetadata,
                schema_version,
            );
            reason.target_id = Some(target.target_id.clone());
            reasons.push(reason);
        }
        if target
            .path
            .as_ref()
            .is_some_and(|path| path.0.trim().is_empty())
        {
            let mut reason = approval_reason(
                gate,
                "proposal.unknown_path_target",
                "Affected target has an empty path label",
                ProposalRiskLabel::High,
                ProposalPrivacyLabel::WorkspaceMetadata,
                schema_version,
            );
            reason.target_id = Some(target.target_id.clone());
            reasons.push(reason);
        }
        if matches!(target.kind, ProposalTargetKind::ClosedFile)
            && (target.workspace_id.is_none() || target.file_id.is_none())
        {
            let mut reason = approval_reason(
                gate,
                "proposal.missing_file_identity_target",
                "Closed-file target lacks file identity metadata",
                ProposalRiskLabel::High,
                ProposalPrivacyLabel::WorkspaceMetadata,
                schema_version,
            );
            reason.target_id = Some(target.target_id.clone());
            reasons.push(reason);
        }
    }
    approval_gate_summary(
        gate,
        if reasons.is_empty() {
            ApprovalChecklistGateStatus::Satisfied
        } else {
            ApprovalChecklistGateStatus::Blocked
        },
        if reasons.is_empty() {
            ProposalRiskLabel::Low
        } else {
            ProposalRiskLabel::High
        },
        ProposalPrivacyLabel::WorkspaceMetadata,
        vec![format!("proposal.targets:{}", coverage.targets.len())],
        reasons,
        schema_version,
    )
}

fn freshness_precondition_gate(
    proposal: &WorkspaceProposal,
    context_projection: Option<&ContextManifestProjection>,
    generated_at: TimestampMillis,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    let gate = ApprovalChecklistGateKind::FreshnessPreconditions;
    let preconditions = ContextManifestPreconditionSummary::from_preconditions(
        &proposal.preconditions,
        schema_version,
    );
    let mut reasons = preconditions
        .risk_reasons
        .iter()
        .map(|reason| {
            approval_reason(
                gate,
                reason.clone(),
                format!("Proposal precondition risk: {reason}"),
                preconditions.risk_label,
                ProposalPrivacyLabel::WorkspaceMetadata,
                schema_version,
            )
        })
        .collect::<Vec<_>>();
    if proposal.is_expired(generated_at) {
        reasons.push(approval_reason(
            gate,
            "proposal.expired",
            "Proposal is expired",
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::WorkspaceMetadata,
            schema_version,
        ));
    }
    if context_projection
        .is_some_and(|projection| projection.manifest.stale_or_missing_metadata_risk_present)
    {
        reasons.push(approval_reason(
            gate,
            "context_manifest.stale_or_missing_metadata",
            "Context manifest reports stale or missing freshness metadata",
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::WorkspaceMetadata,
            schema_version,
        ));
    }
    approval_gate_summary(
        gate,
        if reasons.is_empty() {
            ApprovalChecklistGateStatus::Satisfied
        } else {
            ApprovalChecklistGateStatus::Blocked
        },
        if reasons.is_empty() {
            ProposalRiskLabel::Low
        } else {
            ProposalRiskLabel::High
        },
        ProposalPrivacyLabel::WorkspaceMetadata,
        vec!["proposal.preconditions.metadata_only".to_string()],
        reasons,
        schema_version,
    )
}

fn audit_gate(
    audit_before_success_available: bool,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    let gate = ApprovalChecklistGateKind::AuditBeforeSuccess;
    approval_gate_summary(
        gate,
        if audit_before_success_available {
            ApprovalChecklistGateStatus::Satisfied
        } else {
            ApprovalChecklistGateStatus::Blocked
        },
        if audit_before_success_available {
            ProposalRiskLabel::Low
        } else {
            ProposalRiskLabel::High
        },
        ProposalPrivacyLabel::WorkspaceMetadata,
        vec!["audit_before_success.metadata_only".to_string()],
        if audit_before_success_available {
            Vec::new()
        } else {
            vec![approval_reason(
                gate,
                "audit_before_success.missing",
                "Audit-before-success metadata is missing",
                ProposalRiskLabel::High,
                ProposalPrivacyLabel::WorkspaceMetadata,
                schema_version,
            )]
        },
        schema_version,
    )
}

fn rollback_checkpoint_gate(
    checkpoint_projection: Option<&CheckpointRollbackProjection>,
    ledger_projection: Option<&ProposalLedgerProjection>,
    proposal_id: ProposalId,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    let gate = ApprovalChecklistGateKind::RollbackCheckpoint;
    let availability = checkpoint_projection
        .map(|projection| projection.rollback.availability)
        .or_else(|| {
            ledger_projection
                .and_then(|projection| proposal_ledger_row(projection, proposal_id))
                .map(|row| row.rollback)
        });
    let Some(availability) = availability else {
        return approval_gate_summary(
            gate,
            ApprovalChecklistGateStatus::Blocked,
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::WorkspaceMetadata,
            vec!["rollback_checkpoint.missing".to_string()],
            vec![approval_reason(
                gate,
                "rollback_checkpoint.missing",
                "Rollback/checkpoint projection is missing",
                ProposalRiskLabel::High,
                ProposalPrivacyLabel::WorkspaceMetadata,
                schema_version,
            )],
            schema_version,
        );
    };
    let mut reasons = checkpoint_projection
        .into_iter()
        .flat_map(|projection| projection.rollback.limitations.iter())
        .map(|limitation| {
            approval_reason(
                gate,
                limitation.reason_code.clone(),
                limitation.label.clone(),
                limitation.risk_label,
                ProposalPrivacyLabel::WorkspaceMetadata,
                schema_version,
            )
        })
        .collect::<Vec<_>>();
    if matches!(
        availability,
        ProposalRollbackAvailability::Unavailable | ProposalRollbackAvailability::Unknown
    ) {
        reasons.push(approval_reason(
            gate,
            "rollback.unavailable",
            "Rollback is unavailable or unknown",
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::WorkspaceMetadata,
            schema_version,
        ));
    }
    let status = if reasons.is_empty() {
        ApprovalChecklistGateStatus::Satisfied
    } else if matches!(availability, ProposalRollbackAvailability::BestEffort) {
        ApprovalChecklistGateStatus::Risk
    } else {
        ApprovalChecklistGateStatus::Blocked
    };
    approval_gate_summary(
        gate,
        status,
        if reasons.is_empty() {
            ProposalRiskLabel::Low
        } else {
            ProposalRiskLabel::High
        },
        ProposalPrivacyLabel::WorkspaceMetadata,
        vec![format!("rollback.availability.{availability:?}")],
        reasons,
        schema_version,
    )
}

fn risk_label_gate(
    context: Option<&ContextManifestProjection>,
    privacy: Option<&PrivacyInspectorProjection>,
    budget: Option<&PermissionBudgetProjection>,
    rollback: Option<&CheckpointRollbackProjection>,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    let gate = ApprovalChecklistGateKind::RiskLabels;
    let mut risk = ProposalRiskLabel::Informational;
    if let Some(context) = context {
        risk = max_risk_label(risk, context.manifest.risk_label);
    }
    if let Some(privacy) = privacy
        && privacy.high_risk_record_count > 0
    {
        risk = max_risk_label(risk, ProposalRiskLabel::High);
    }
    if let Some(budget) = budget
        && (budget.denied_budget_count > 0 || budget.depleted_budget_count > 0)
    {
        risk = max_risk_label(risk, ProposalRiskLabel::High);
    }
    if let Some(rollback) = rollback {
        risk = rollback
            .risk_labels
            .iter()
            .copied()
            .fold(risk, max_risk_label);
    }
    let blocks = risk == ProposalRiskLabel::Unknown;
    approval_gate_summary(
        gate,
        if blocks {
            ApprovalChecklistGateStatus::Blocked
        } else if matches!(risk, ProposalRiskLabel::High | ProposalRiskLabel::Medium) {
            ApprovalChecklistGateStatus::Risk
        } else {
            ApprovalChecklistGateStatus::Satisfied
        },
        risk,
        ProposalPrivacyLabel::WorkspaceMetadata,
        vec![format!("risk.aggregate.{risk:?}")],
        if blocks {
            vec![approval_reason(
                gate,
                "risk.unknown",
                "Risk label is unknown",
                ProposalRiskLabel::Unknown,
                ProposalPrivacyLabel::Unknown,
                schema_version,
            )]
        } else {
            Vec::new()
        },
        schema_version,
    )
}

fn explicit_denial_gate(
    lifecycle_state: ProposalLifecycleState,
    privacy: Option<&PrivacyInspectorProjection>,
    budget: Option<&PermissionBudgetProjection>,
    rollback: Option<&CheckpointRollbackProjection>,
    schema_version: u16,
) -> ApprovalChecklistGateSummary {
    let gate = ApprovalChecklistGateKind::ExplicitDenialReasons;
    let mut reasons = Vec::new();
    if matches!(
        lifecycle_state,
        ProposalLifecycleState::Denied
            | ProposalLifecycleState::Rejected
            | ProposalLifecycleState::Failed
            | ProposalLifecycleState::Stale
            | ProposalLifecycleState::Conflict
            | ProposalLifecycleState::Cancelled
    ) {
        reasons.push(approval_reason(
            gate,
            format!("proposal.lifecycle.{lifecycle_state:?}"),
            "Proposal lifecycle carries an explicit terminal or denial state",
            ProposalRiskLabel::High,
            ProposalPrivacyLabel::WorkspaceMetadata,
            schema_version,
        ));
    }
    if let Some(refusal) = privacy.and_then(|projection| projection.refusal.as_ref()) {
        reasons.push(approval_reason(
            gate,
            refusal.reason_code.clone(),
            refusal.label.clone(),
            ProposalRiskLabel::High,
            refusal
                .privacy_scope
                .map(privacy_label_from_scope)
                .unwrap_or(ProposalPrivacyLabel::Unknown),
            schema_version,
        ));
    }
    if let Some(budget) = budget {
        for evaluation in budget
            .evaluations
            .iter()
            .filter(|evaluation| !evaluation.allowed)
        {
            reasons.push(approval_reason(
                gate,
                format!("budget.evaluation.{:?}", evaluation.disposition),
                "Permission budget evaluation refused",
                ProposalRiskLabel::High,
                privacy_label_from_scope(evaluation.action.privacy_scope),
                schema_version,
            ));
        }
    }
    if let Some(rollback) = rollback {
        for limitation in &rollback.rollback.limitations {
            reasons.push(approval_reason(
                gate,
                limitation.reason_code.clone(),
                limitation.label.clone(),
                limitation.risk_label,
                ProposalPrivacyLabel::WorkspaceMetadata,
                schema_version,
            ));
        }
    }
    approval_gate_summary(
        gate,
        if reasons.is_empty() {
            ApprovalChecklistGateStatus::Satisfied
        } else {
            ApprovalChecklistGateStatus::Blocked
        },
        if reasons.is_empty() {
            ProposalRiskLabel::Low
        } else {
            ProposalRiskLabel::High
        },
        ProposalPrivacyLabel::WorkspaceMetadata,
        vec!["explicit_denial.metadata_only".to_string()],
        reasons,
        schema_version,
    )
}

fn proposal_ledger_row(
    projection: &ProposalLedgerProjection,
    proposal_id: ProposalId,
) -> Option<&ProposalLedgerRow> {
    projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal_id)
}

fn coverage_is_declared(coverage: &ProposalTargetCoverage) -> bool {
    !coverage.targets.is_empty()
        || coverage.omitted_target_count > 0
        || coverage.coverage_kind != ProposalTargetCoverageKind::Complete
}

fn proposal_declared_target_coverage(
    proposal: &WorkspaceProposal,
) -> Option<&ProposalTargetCoverage> {
    match &proposal.payload {
        ProposalPayload::WorkspaceEdit(payload) => Some(&payload.target_coverage),
        ProposalPayload::Batch(payload) => Some(&payload.target_coverage),
        _ => None,
    }
}

fn proposal_workspace_id_from_coverage(coverage: &ProposalTargetCoverage) -> Option<WorkspaceId> {
    coverage
        .targets
        .iter()
        .find_map(|target| target.workspace_id)
}

fn rollback_availability_from_payload(payload: &ProposalPayload) -> ProposalRollbackAvailability {
    match payload {
        ProposalPayload::Batch(batch) => match batch.rollback_policy {
            ProposalBatchRollbackPolicy::Required => {
                if batch.rollback_steps.is_empty()
                    || batch
                        .rollback_steps
                        .iter()
                        .any(|step| step.action == ProposalRollbackAction::Unsupported)
                {
                    ProposalRollbackAvailability::Unavailable
                } else {
                    ProposalRollbackAvailability::Available
                }
            }
            ProposalBatchRollbackPolicy::BestEffort => ProposalRollbackAvailability::BestEffort,
            ProposalBatchRollbackPolicy::NotSupported => ProposalRollbackAvailability::Unavailable,
            ProposalBatchRollbackPolicy::NotRequired => ProposalRollbackAvailability::NotRequired,
        },
        ProposalPayload::WorkspaceEdit(_) => ProposalRollbackAvailability::BestEffort,
        ProposalPayload::TerminalCommand(_) => ProposalRollbackAvailability::Unavailable,
        ProposalPayload::TextEdit(_)
        | ProposalPayload::CreateFile(_)
        | ProposalPayload::DeleteFile(_)
        | ProposalPayload::RenameFile(_)
        | ProposalPayload::SaveFile(_)
        | ProposalPayload::FormatFile(_)
        | ProposalPayload::CodeAction(_) => ProposalRollbackAvailability::Unknown,
    }
}

fn rollback_step_count(payload: &ProposalPayload) -> u32 {
    match payload {
        ProposalPayload::Batch(batch) => batch.rollback_steps.len() as u32,
        _ => 0,
    }
}

fn checkpoint_limitation(
    reason_code: impl Into<String>,
    label: impl Into<String>,
    target_id: Option<String>,
    risk_label: ProposalRiskLabel,
    schema_version: u16,
) -> CheckpointRollbackLimitation {
    CheckpointRollbackLimitation {
        reason_code: reason_code.into(),
        label: label.into(),
        target_id,
        risk_label,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn rollback_limitations(
    availability: ProposalRollbackAvailability,
    rollback_step_count: u32,
    audit_status: CheckpointRollbackAuditStatus,
    coverage: &ProposalTargetCoverage,
    schema_version: u16,
) -> Vec<CheckpointRollbackLimitation> {
    let mut limitations = Vec::new();
    if matches!(
        availability,
        ProposalRollbackAvailability::Unavailable | ProposalRollbackAvailability::Unknown
    ) {
        limitations.push(checkpoint_limitation(
            "rollback.unavailable",
            "Rollback is unavailable or unknown from metadata",
            None,
            ProposalRiskLabel::High,
            schema_version,
        ));
    }
    if availability == ProposalRollbackAvailability::BestEffort {
        limitations.push(checkpoint_limitation(
            "rollback.best_effort",
            "Rollback is best-effort",
            None,
            ProposalRiskLabel::Medium,
            schema_version,
        ));
    }
    if availability == ProposalRollbackAvailability::Available && rollback_step_count == 0 {
        limitations.push(checkpoint_limitation(
            "rollback.steps_missing",
            "Rollback availability is declared but no rollback step descriptors are present",
            None,
            ProposalRiskLabel::High,
            schema_version,
        ));
    }
    if matches!(
        audit_status,
        CheckpointRollbackAuditStatus::Missing | CheckpointRollbackAuditStatus::Pending
    ) {
        limitations.push(checkpoint_limitation(
            "audit_before_success.missing",
            "Rollback audit metadata is missing or pending",
            None,
            ProposalRiskLabel::High,
            schema_version,
        ));
    }
    for target in &coverage.targets {
        if matches!(
            target.kind,
            ProposalTargetKind::TerminalSession
                | ProposalTargetKind::RemoteWorkspace
                | ProposalTargetKind::CollaborationSession
                | ProposalTargetKind::Plugin
        ) {
            limitations.push(checkpoint_limitation(
                "rollback.unsupported_target_kind",
                format!("Rollback target kind {:?} is unsupported", target.kind),
                Some(target.target_id.clone()),
                ProposalRiskLabel::High,
                schema_version,
            ));
        }
    }
    limitations
}

fn checkpoint_target_summary(
    target: &ProposalAffectedTarget,
    preconditions: &ProposalVersionPreconditions,
    schema_version: u16,
) -> CheckpointRollbackTargetSummary {
    let expected_file_content_version = preconditions
        .file_content_version
        .or(preconditions.file_version);
    let expected_workspace_generation = preconditions
        .workspace_generation
        .or(preconditions.generation);
    CheckpointRollbackTargetSummary {
        target_id: target.target_id.clone(),
        kind: target.kind,
        workspace_id: target.workspace_id,
        file_id: target.file_id,
        buffer_id: target.buffer_id,
        terminal_session_id: target.terminal_session_id,
        plugin_id: target.plugin_id,
        ranges: target.byte_ranges.clone(),
        hashes: preconditions
            .expected_fingerprint
            .clone()
            .into_iter()
            .collect(),
        expected_file_content_version,
        expected_buffer_version: preconditions.buffer_version,
        expected_snapshot_id: preconditions.snapshot_id,
        expected_workspace_generation,
        labels: vec![
            format!("target.kind.{:?}", target.kind),
            "path.redacted".to_string(),
        ],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version,
    }
}

fn visit_proposal_payload_targets(
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
            visitor(proposal_file_target(
                format!("text-edit:file:{}", payload.file_id.0),
                ProposalTargetKind::OpenBuffer,
                None,
                Some(payload.file_id),
                None,
                None,
                byte_ranges,
            ));
        }
        ProposalPayload::CreateFile(payload) => visitor(proposal_file_target(
            "create-file:path".to_string(),
            ProposalTargetKind::PathOnly,
            None,
            None,
            None,
            Some(payload.path.clone()),
            Vec::new(),
        )),
        ProposalPayload::DeleteFile(payload) => visitor(proposal_identity_target(
            format!("delete-file:file:{}", payload.file.file_id.0),
            ProposalTargetKind::ClosedFile,
            &payload.file,
            None,
            Vec::new(),
        )),
        ProposalPayload::RenameFile(payload) => {
            visitor(proposal_identity_target(
                format!("rename-file:source:{}", payload.file.file_id.0),
                ProposalTargetKind::ClosedFile,
                &payload.file,
                None,
                Vec::new(),
            ));
            visitor(proposal_file_target(
                "rename-file:destination".to_string(),
                ProposalTargetKind::PathOnly,
                None,
                None,
                None,
                Some(payload.destination.clone()),
                Vec::new(),
            ));
        }
        ProposalPayload::SaveFile(payload) => visitor(proposal_identity_target(
            format!(
                "save-file:file:{}:buffer:{}",
                payload.file.file_id.0, payload.buffer_id.0
            ),
            ProposalTargetKind::OpenBuffer,
            &payload.file,
            Some(payload.buffer_id),
            Vec::new(),
        )),
        ProposalPayload::FormatFile(payload) => visitor(proposal_identity_target(
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
            visitor(proposal_identity_target(
                format!("code-action:file:{}", payload.file.file_id.0),
                ProposalTargetKind::ClosedFile,
                &payload.file,
                None,
                byte_ranges,
            ));
        }
        ProposalPayload::WorkspaceEdit(payload) => {
            for edit in &payload.file_edits {
                let byte_ranges = edit
                    .edits
                    .edits
                    .iter()
                    .filter_map(|edit| edit.range.as_byte_range())
                    .collect();
                visitor(proposal_identity_target(
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
                    WorkspaceFileOperation::Create { path, .. } => visitor(proposal_file_target(
                        "workspace-edit:create:path".to_string(),
                        ProposalTargetKind::PathOnly,
                        None,
                        None,
                        None,
                        Some(path.clone()),
                        Vec::new(),
                    )),
                    WorkspaceFileOperation::Delete { file } => visitor(proposal_identity_target(
                        format!("workspace-edit:delete:file:{}", file.file_id.0),
                        ProposalTargetKind::ClosedFile,
                        file,
                        None,
                        Vec::new(),
                    )),
                    WorkspaceFileOperation::Rename { file, destination } => {
                        visitor(proposal_identity_target(
                            format!("workspace-edit:rename:source:{}", file.file_id.0),
                            ProposalTargetKind::ClosedFile,
                            file,
                            None,
                            Vec::new(),
                        ));
                        visitor(proposal_file_target(
                            "workspace-edit:rename:destination".to_string(),
                            ProposalTargetKind::PathOnly,
                            None,
                            None,
                            None,
                            Some(destination.clone()),
                            Vec::new(),
                        ));
                    }
                }
            }
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
        ProposalPayload::Batch(payload) => {
            let mut items = payload.items.iter().collect::<Vec<_>>();
            items.sort_by(|left, right| {
                left.order
                    .cmp(&right.order)
                    .then_with(|| left.item_id.cmp(&right.item_id))
            });
            for item in items {
                visit_proposal_payload_targets(item.payload.as_ref(), visitor);
            }
        }
    }
}

fn proposal_identity_target(
    target_id: String,
    kind: ProposalTargetKind,
    file: &FileIdentity,
    buffer_id: Option<BufferId>,
    byte_ranges: Vec<ByteRange>,
) -> ProposalAffectedTarget {
    proposal_file_target(
        target_id,
        kind,
        Some(file.workspace_id),
        Some(file.file_id),
        buffer_id,
        Some(file.canonical_path.clone()),
        byte_ranges,
    )
}

fn proposal_file_target(
    target_id: String,
    kind: ProposalTargetKind,
    workspace_id: Option<WorkspaceId>,
    file_id: Option<FileId>,
    buffer_id: Option<BufferId>,
    path: Option<CanonicalPath>,
    byte_ranges: Vec<ByteRange>,
) -> ProposalAffectedTarget {
    ProposalAffectedTarget {
        target_id,
        kind,
        workspace_id,
        file_id,
        buffer_id,
        path,
        terminal_session_id: None,
        plugin_id: None,
        remote_authority: None,
        collaboration_session_id: None,
        byte_ranges,
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
}

fn protocol_text_range_to_byte_range(range: &ProtocolTextRange) -> Option<ByteRange> {
    Some(ByteRange::new(
        range.start.byte_offset?,
        range.end.byte_offset?,
    ))
}

/// One metadata-only row in the product-visible proposal ledger projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalLedgerRow {
    /// Proposal identifier.
    pub proposal_id: ProposalId,
    /// Optional workspace identifier.
    pub workspace_id: Option<WorkspaceId>,
    /// Bounded display title.
    pub title: String,
    /// Payload kind summarized for display.
    pub payload_kind: ProposalPayloadKind,
    /// Lifecycle state display metadata.
    pub lifecycle: ProposalLifecycleStateDisplay,
    /// Principal label or redacted principal identifier.
    pub principal: PrincipalId,
    /// Capability associated with the proposal.
    pub capability: CapabilityId,
    /// Creation timestamp.
    pub created_at: TimestampMillis,
    /// Last update timestamp.
    pub updated_at: TimestampMillis,
    /// Expiration timestamp when known.
    pub expires_at: Option<TimestampMillis>,
    /// Risk label.
    pub risk_label: ProposalRiskLabel,
    /// Privacy label.
    pub privacy_label: ProposalPrivacyLabel,
    /// Rollback availability summary.
    pub rollback: ProposalRollbackAvailability,
    /// Target coverage metadata.
    pub target_coverage: ProposalTargetCoverage,
    /// Context manifest summary.
    pub context_manifest: ProposalContextManifestSummary,
    /// Diff summary without raw source text.
    pub diff_summary: ProposalDiffSummary,
    /// Bounded preview warnings.
    pub preview_warnings: Vec<ProposalPreviewWarning>,
    /// Diagnostics safe for projection.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Redaction hints that apply to the row.
    pub redaction_hints: Vec<RedactionHint>,
    /// Row DTO schema version.
    pub schema_version: u16,
}

/// Static proposal ledger projection consumed by projection-only UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalLedgerProjection {
    /// Rows in deterministic display order.
    pub rows: Vec<ProposalLedgerRow>,
    /// Selected proposal id when details are open.
    pub selected_proposal_id: Option<ProposalId>,
    /// Number of rows omitted by paging or redaction.
    pub omitted_row_count: u32,
    /// Projection timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints that apply to the whole projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection DTO schema version.
    pub schema_version: u16,
}

// -----------------------------------------------------------------------------
// Language and terminal IDE loop projections
// -----------------------------------------------------------------------------

/// High-level status for the language tooling panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageToolingStatusKind {
    /// No language tooling request has run.
    Idle,
    /// Language tooling data is ready for the active buffer.
    Ready,
    /// A language tooling request is running.
    Running,
    /// The latest result is stale for the current buffer identity.
    Stale,
    /// The latest request was cancelled.
    Cancelled,
    /// Language tooling is unavailable for the current buffer.
    Unavailable,
    /// Language tooling failed before producing projection data.
    Failed,
}

/// Operation kind represented in the language tooling projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageToolingOperationKind {
    /// Diagnostics refresh.
    Diagnostics,
    /// Hover lookup.
    Hover,
    /// Completion lookup.
    Completion,
    /// Definition lookup.
    Definition,
    /// References lookup.
    References,
    /// Outline refresh.
    Outline,
    /// Formatting proposal conversion.
    FormattingProposal,
    /// Rename proposal conversion.
    RenameProposal,
    /// Organize imports proposal conversion.
    OrganizeImportsProposal,
    /// Code action proposal conversion.
    CodeActionProposal,
}

/// One language tooling operation row with request/proposal correlation metadata only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageToolingOperationProjection {
    /// Stable operation identifier.
    pub operation_id: String,
    /// Operation kind.
    pub kind: LanguageToolingOperationKind,
    /// Current operation status.
    pub status: LanguageToolingStatusKind,
    /// LSP request identifier when the operation crossed an LSP boundary.
    pub request_id: Option<LspRequestId>,
    /// Proposal identifier when an edit-producing operation created a proposal preview.
    pub proposal_id: Option<ProposalId>,
    /// Bounded metadata-only status message.
    pub message: String,
    /// Cross-domain correlation id when issued by app authority.
    pub correlation_id: Option<CorrelationId>,
    /// Cross-domain causality id when issued by app authority.
    pub causality_id: Option<CausalityId>,
    /// Projection row generation timestamp.
    pub generated_at: TimestampMillis,
    /// Operation row schema version.
    pub schema_version: u16,
}

/// Metadata-only language problem row for diagnostics and parser/index feedback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageProblemProjection {
    /// File containing the problem when known.
    pub file_id: Option<FileId>,
    /// Canonical path for display when disclosure is allowed.
    pub path: Option<CanonicalPath>,
    /// Problem range when disclosure is allowed.
    pub range: Option<ProtocolTextRange>,
    /// Diagnostic severity.
    pub severity: ProtocolDiagnosticSeverity,
    /// Optional metadata-only code or code hash.
    pub code_label: Option<String>,
    /// Bounded display message.
    pub message: String,
    /// Optional source label, such as an LSP server or lexical indexer.
    pub source_label: Option<String>,
    /// Redaction hints for the row.
    pub redaction_hints: Vec<RedactionHint>,
    /// Problem row schema version.
    pub schema_version: u16,
}

/// Bounded hover projection for the active symbol or cursor position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageHoverProjection {
    /// Metadata-only hover identifier.
    pub hover_id: String,
    /// File containing the hover target.
    pub file_id: Option<FileId>,
    /// Range associated with the hover result when known.
    pub range: Option<ProtocolTextRange>,
    /// Bounded display label for the hovered symbol.
    pub label: String,
    /// Bounded documentation or type summary.
    pub summary: String,
    /// Whether the hover was produced from degraded semantic data.
    pub degraded: bool,
    /// Redaction hints for the row.
    pub redaction_hints: Vec<RedactionHint>,
    /// Hover row schema version.
    pub schema_version: u16,
}

/// Bounded completion row for projection-only UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageCompletionProjection {
    /// Metadata-only completion identifier.
    pub completion_id: String,
    /// Display label.
    pub label: String,
    /// Optional detail label.
    pub detail_label: Option<String>,
    /// Completion kind label.
    pub kind_label: String,
    /// Rank in basis points.
    pub score_basis_points: u16,
    /// Whether this completion was produced from degraded semantic data.
    pub degraded: bool,
    /// Completion row schema version.
    pub schema_version: u16,
}

/// Location row used for definition and reference projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageLocationProjection {
    /// Metadata-only location identifier.
    pub location_id: String,
    /// File containing the target location.
    pub file_id: Option<FileId>,
    /// Canonical path for display when disclosure is allowed.
    pub path: Option<CanonicalPath>,
    /// Target range when disclosure is allowed.
    pub range: Option<ProtocolTextRange>,
    /// Display label for the symbol or target.
    pub label: String,
    /// Whether this location was produced from degraded semantic data.
    pub degraded: bool,
    /// Location row schema version.
    pub schema_version: u16,
}

/// Outline row for the active document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageOutlineSymbolProjection {
    /// Metadata-only outline row identifier.
    pub symbol_id: String,
    /// Display label.
    pub label: String,
    /// Symbol kind label.
    pub kind_label: String,
    /// Range associated with the symbol when known.
    pub range: Option<ProtocolTextRange>,
    /// Outline depth.
    pub depth: u16,
    /// Whether child rows were omitted by projection bounds.
    pub children_omitted: bool,
    /// Outline row schema version.
    pub schema_version: u16,
}

/// Projection-only language tooling panel state for the active editor buffer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageToolingProjection {
    /// Workspace represented by the projection, when one is open.
    pub workspace_id: Option<WorkspaceId>,
    /// Active editor buffer represented by the projection, when one is open.
    pub buffer_id: Option<BufferId>,
    /// Active file represented by the projection, when one is open.
    pub file_id: Option<FileId>,
    /// High-level projection status.
    pub status: LanguageToolingStatusKind,
    /// Bounded metadata-only status message.
    pub status_message: String,
    /// Diagnostic/problem rows.
    pub problems: Vec<LanguageProblemProjection>,
    /// Current hover result.
    pub hover: Option<LanguageHoverProjection>,
    /// Current completion rows.
    pub completions: Vec<LanguageCompletionProjection>,
    /// Current definition locations.
    pub definitions: Vec<LanguageLocationProjection>,
    /// Current reference locations.
    pub references: Vec<LanguageLocationProjection>,
    /// Current outline rows.
    pub outline: Vec<LanguageOutlineSymbolProjection>,
    /// Recent operation status rows.
    pub operations: Vec<LanguageToolingOperationProjection>,
    /// Count of stale results discarded before projection.
    pub stale_result_count: u32,
    /// Count of cancellation acknowledgements projected.
    pub cancellation_count: u32,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints for the whole projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

impl LanguageToolingProjection {
    /// Construct an empty language tooling projection.
    pub fn empty() -> Self {
        Self {
            workspace_id: None,
            buffer_id: None,
            file_id: None,
            status: LanguageToolingStatusKind::Idle,
            status_message: "Language tooling idle".to_string(),
            problems: Vec::new(),
            hover: None,
            completions: Vec::new(),
            definitions: Vec::new(),
            references: Vec::new(),
            outline: Vec::new(),
            operations: Vec::new(),
            stale_result_count: 0,
            cancellation_count: 0,
            generated_at: TimestampMillis(0),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }
}

impl Default for LanguageToolingProjection {
    fn default() -> Self {
        Self::empty()
    }
}

/// High-level terminal panel status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalPanelStatusKind {
    /// Terminal workflow is disabled.
    Disabled,
    /// Terminal workflow was denied by policy.
    Denied,
    /// No terminal session is active.
    Idle,
    /// Terminal launch is in progress.
    Starting,
    /// Terminal session is running.
    Running,
    /// Terminal session exited.
    Exited,
    /// Terminal workflow failed.
    Failed,
    /// Terminal output is degraded or bounded.
    Degraded,
}

/// Terminal panel status row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalPanelStatus {
    /// Status kind.
    pub kind: TerminalPanelStatusKind,
    /// Bounded metadata-only status message.
    pub message: String,
}

/// Metadata-only policy state for a terminal workflow request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalPolicyProjection {
    /// Capability required by terminal launch or lifecycle actions.
    pub capability_id: CapabilityId,
    /// Workspace trust posture observed by app authority.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Whether policy granted the latest request.
    pub granted: bool,
    /// Capability decision identifier when known.
    pub decision_id: Option<CapabilityDecisionId>,
    /// Bounded metadata-only reason.
    pub reason: String,
    /// Output byte limit selected by policy.
    pub output_byte_limit: u64,
    /// Timeout selected by policy.
    pub timeout_seconds: u64,
    /// Policy projection schema version.
    pub schema_version: u16,
}

/// Bounded redacted terminal output row for projection-only UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalOutputRowProjection {
    /// Terminal session identifier.
    pub session_id: TerminalSessionId,
    /// Output sequence.
    pub sequence: EventSequence,
    /// Bounded redacted payload.
    pub redacted_payload: String,
    /// Byte count before redaction or truncation.
    pub byte_count: u64,
    /// Whether the row came from stderr.
    pub is_stderr: bool,
    /// Whether output was truncated.
    pub truncated: bool,
    /// Redaction applied to this row.
    pub redaction: RedactionHint,
    /// Output row schema version.
    pub schema_version: u16,
}

/// Scrollback bounds projected for terminal output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalScrollbackProjection {
    /// Number of visible output rows.
    pub visible_row_count: u32,
    /// Number of rows omitted by retention bounds.
    pub omitted_row_count: u32,
    /// Byte ceiling for projected output.
    pub byte_limit: u64,
    /// Whether scrollback was truncated.
    pub truncated: bool,
    /// Scrollback projection schema version.
    pub schema_version: u16,
}

/// Search summary for projected terminal output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSearchProjection {
    /// Bounded display label for the search query.
    pub query_label: Option<String>,
    /// Number of matches in projected rows.
    pub match_count: u32,
    /// Active match index when one is selected.
    pub active_match_index: Option<u32>,
    /// Whether search results were truncated by projection bounds.
    pub truncated: bool,
    /// Search projection schema version.
    pub schema_version: u16,
}

/// Projection-only terminal panel state for app-owned terminal workflows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalPanelProjection {
    /// Workspace represented by the terminal panel, when one is open.
    pub workspace_id: Option<WorkspaceId>,
    /// Active terminal session identifier, when one exists.
    pub active_session_id: Option<TerminalSessionId>,
    /// Runtime state reported by the terminal boundary, when one exists.
    pub runtime_state: Option<TerminalRuntimeState>,
    /// High-level panel status.
    pub status: TerminalPanelStatus,
    /// Latest terminal policy projection.
    pub policy: Option<TerminalPolicyProjection>,
    /// Bounded output rows.
    pub output_rows: Vec<TerminalOutputRowProjection>,
    /// Scrollback bound summary.
    pub scrollback: TerminalScrollbackProjection,
    /// Current terminal search summary.
    pub search: TerminalSearchProjection,
    /// Bounded metadata-only error message.
    pub last_error: Option<String>,
    /// Bounded metadata-only denial reason.
    pub last_denial: Option<String>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Redaction hints for the whole projection.
    pub redaction_hints: Vec<RedactionHint>,
    /// Projection schema version.
    pub schema_version: u16,
}

impl TerminalPanelProjection {
    /// Construct an empty terminal panel projection.
    pub fn empty() -> Self {
        Self {
            workspace_id: None,
            active_session_id: None,
            runtime_state: None,
            status: TerminalPanelStatus {
                kind: TerminalPanelStatusKind::Disabled,
                message: "Terminal workflow disabled".to_string(),
            },
            policy: None,
            output_rows: Vec::new(),
            scrollback: TerminalScrollbackProjection {
                visible_row_count: 0,
                omitted_row_count: 0,
                byte_limit: 0,
                truncated: false,
                schema_version: 1,
            },
            search: TerminalSearchProjection {
                query_label: None,
                match_count: 0,
                active_match_index: None,
                truncated: false,
                schema_version: 1,
            },
            last_error: None,
            last_denial: None,
            generated_at: TimestampMillis(0),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }
}

impl Default for TerminalPanelProjection {
    fn default() -> Self {
        Self::empty()
    }
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

/// Metadata-only configured identity for a language server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspConfiguredServerIdentity {
    /// Stable server identifier.
    pub server_id: LanguageServerId,
    /// Workspace that owns the configuration metadata.
    pub workspace_id: WorkspaceId,
    /// Workspace root associated with the server, when known.
    pub root_id: Option<WorkspaceRootId>,
    /// Language served by this configuration.
    pub language_id: LanguageId,
    /// Bounded display name for the server.
    pub display_name: String,
    /// Hash of the configured executable or command label.
    pub command_hash: FileFingerprint,
    /// Hash of configured argument metadata.
    pub args_hash: Option<FileFingerprint>,
    /// Hash of configured environment metadata without secret values.
    pub env_hash: Option<FileFingerprint>,
    /// Hash of working-directory metadata, when disclosure is allowed.
    pub cwd_hash: Option<FileFingerprint>,
    /// Hash of server-specific settings without raw settings payloads.
    pub settings_hash: Option<FileFingerprint>,
    /// Redaction hints for the identity record.
    pub redaction_hints: Vec<RedactionHint>,
    /// Identity DTO schema version.
    pub schema_version: u16,
}

/// Trust and privacy posture used before LSP supervision may be enabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspWorkspaceTrustPosture {
    /// Workspace being evaluated.
    pub workspace_id: WorkspaceId,
    /// Workspace trust state.
    pub workspace_trust_state: WorkspaceTrustState,
    /// Privacy scope requested by LSP supervision.
    pub privacy_scope: SemanticPrivacyScope,
    /// Whether the privacy scope allows metadata-only LSP supervision.
    pub privacy_scope_allowed: bool,
    /// Capability required before launch may be considered.
    pub required_capability: CapabilityId,
    /// Capability decision identifier when already evaluated.
    pub decision_id: Option<CapabilityDecisionId>,
    /// Metadata-only diagnostics explaining trust or privacy posture.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Posture DTO schema version.
    pub schema_version: u16,
}

/// Contract-level launch disposition for LSP supervision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspLaunchDisposition {
    /// Launch is permitted by this metadata contract, subject to a future runtime gate.
    Eligible,
    /// Launch is disabled because the workspace is not trusted.
    DisabledUntrustedWorkspace,
    /// Launch is disabled because privacy scope denied supervision.
    DisabledPrivacyDenied,
    /// Launch is disabled because the required capability was denied or is absent.
    DisabledCapabilityDenied,
    /// Launch is refused because runtime activation is intentionally deferred.
    RuntimeActivationDeferred,
}

/// Metadata-only launch policy decision for one configured server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspLaunchPolicyDecision {
    /// Server identity evaluated by the policy.
    pub identity: LspConfiguredServerIdentity,
    /// Trust and privacy posture observed by the policy.
    pub posture: LspWorkspaceTrustPosture,
    /// Launch disposition.
    pub disposition: LspLaunchDisposition,
    /// Whether this contract authorizes an actual process launch.
    pub process_launch_allowed: bool,
    /// Whether runtime activation has been accepted by phase gates.
    pub runtime_activation_accepted: bool,
    /// Metadata-only reason code.
    pub reason_code: String,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Metadata-only diagnostics.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Redaction hints for the decision record.
    pub redaction_hints: Vec<RedactionHint>,
    /// Launch policy DTO schema version.
    pub schema_version: u16,
}

impl LspLaunchPolicyDecision {
    /// Builds a fail-closed launch policy decision without starting any runtime.
    pub fn evaluate(
        identity: LspConfiguredServerIdentity,
        posture: LspWorkspaceTrustPosture,
        runtime_activation_accepted: bool,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
        observed_diagnostics: Vec<ProtocolDiagnostic>,
        schema_version: u16,
    ) -> Self {
        let (disposition, reason_code) =
            if posture.workspace_trust_state != WorkspaceTrustState::Trusted {
                (
                    LspLaunchDisposition::DisabledUntrustedWorkspace,
                    "lsp.supervision.disabled.untrusted_workspace",
                )
            } else if !posture.privacy_scope_allowed
                || matches!(posture.privacy_scope, SemanticPrivacyScope::Redacted)
            {
                (
                    LspLaunchDisposition::DisabledPrivacyDenied,
                    "lsp.supervision.disabled.privacy_denied",
                )
            } else if !runtime_activation_accepted {
                (
                    LspLaunchDisposition::RuntimeActivationDeferred,
                    "lsp.supervision.runtime_deferred",
                )
            } else {
                (LspLaunchDisposition::Eligible, "lsp.supervision.eligible")
            };

        let process_launch_allowed =
            matches!(disposition, LspLaunchDisposition::Eligible) && runtime_activation_accepted;

        Self {
            identity,
            posture,
            disposition,
            process_launch_allowed,
            runtime_activation_accepted,
            reason_code: reason_code.to_string(),
            correlation_id,
            causality_id,
            diagnostics: observed_diagnostics,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version,
        }
    }
}

/// LSP supervised lifecycle state represented without runtime ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspSupervisionLifecycleState {
    /// Server is configured but not launched.
    Configured,
    /// Supervision is disabled by trust, privacy, capability, or runtime gate.
    Disabled,
    /// Launch is deferred until a later runtime activation gate.
    LaunchDeferred,
    /// Future runtime would be starting.
    Starting,
    /// Future runtime would be running.
    Running,
    /// Future runtime would be stopping.
    Stopping,
    /// Runtime is stopped.
    Stopped,
    /// Runtime failed in a supervised boundary.
    Failed,
    /// Circuit breaker is open after repeated failures.
    CircuitOpen,
}

/// Metadata-only health summary for a supervised LSP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspHealthState {
    /// Health has not been observed.
    Unknown,
    /// Server is healthy.
    Healthy,
    /// Server is degraded but some features may work.
    Degraded,
    /// Server is unavailable.
    Unavailable,
}

/// Restart and backoff metadata for supervised LSP boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRestartBackoffMetadata {
    /// Restart attempts observed in the current window.
    pub restart_attempts: u32,
    /// Maximum restart attempts allowed by policy.
    pub max_restart_attempts: u32,
    /// Backoff delay before another restart may be considered.
    pub next_backoff_ms: u64,
    /// Whether the restart circuit breaker is open.
    pub circuit_breaker_open: bool,
    /// Metadata-only last failure code.
    pub last_failure_code: Option<String>,
    /// Hash of the last failure detail, without raw logs or output.
    pub last_failure_hash: Option<FileFingerprint>,
    /// Restart metadata schema version.
    pub schema_version: u16,
}

/// Metadata-only capability summary advertised or inferred for a server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspCapabilitySummary {
    /// Capability label, such as diagnostics, formatting, rename, or code_action.
    pub capability: String,
    /// Whether the server reports support for this capability.
    pub supported: bool,
    /// Whether dynamic registration was reported.
    pub dynamic_registration: bool,
    /// Optional metadata-only option hash.
    pub option_hash: Option<FileFingerprint>,
    /// Redaction hints for capability metadata.
    pub redaction_hints: Vec<RedactionHint>,
    /// Capability summary schema version.
    pub schema_version: u16,
}

/// Metadata-only summary of LSP diagnostics without source bodies or messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDiagnosticSummary {
    /// Workspace containing the diagnostics.
    pub workspace_id: WorkspaceId,
    /// File containing the diagnostics.
    pub file_id: FileId,
    /// Snapshot described by the diagnostics.
    pub snapshot_id: SnapshotId,
    /// Buffer version described by the diagnostics.
    pub buffer_version: BufferVersion,
    /// Content hash used for freshness checks.
    pub content_hash: Option<FileFingerprint>,
    /// Total diagnostic count.
    pub diagnostic_count: u32,
    /// Number of error diagnostics.
    pub error_count: u32,
    /// Number of warning diagnostics.
    pub warning_count: u32,
    /// Number of informational diagnostics.
    pub information_count: u32,
    /// Number of hint diagnostics.
    pub hint_count: u32,
    /// Diagnostic ranges that may be disclosed.
    pub ranges: Vec<ProtocolTextRange>,
    /// Hashes of diagnostic codes or messages supplied by the boundary.
    pub diagnostic_hashes: Vec<FileFingerprint>,
    /// Hashes of diagnostic source labels supplied by the boundary.
    pub source_hashes: Vec<FileFingerprint>,
    /// Freshness state of the diagnostic summary.
    pub freshness: SemanticFreshnessState,
    /// Privacy scope of the summary.
    pub privacy_scope: SemanticPrivacyScope,
    /// Redaction hints for the summary.
    pub redaction_hints: Vec<RedactionHint>,
    /// Diagnostic summary schema version.
    pub schema_version: u16,
}

/// Correlation metadata for a supervised LSP request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRequestCorrelation {
    /// LSP request identifier.
    pub request_id: LspRequestId,
    /// Server handling the request.
    pub server_id: LanguageServerId,
    /// Workspace scope.
    pub workspace_id: WorkspaceId,
    /// Optional file scope.
    pub file_id: Option<FileId>,
    /// Optional snapshot scope.
    pub snapshot_id: Option<SnapshotId>,
    /// Optional buffer version scope.
    pub buffer_version: Option<BufferVersion>,
    /// Cross-domain correlation id.
    pub correlation_id: CorrelationId,
    /// Cross-domain causality id.
    pub causality_id: CausalityId,
    /// Optional cancellation token for the request.
    pub cancellation_token: Option<CancellationTokenId>,
    /// Privacy scope of request metadata.
    pub privacy_scope: SemanticPrivacyScope,
    /// Request issue timestamp.
    pub issued_at: TimestampMillis,
    /// Request correlation schema version.
    pub schema_version: u16,
}

/// Redacted supervision event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspSupervisionEventKind {
    /// Server configuration was observed.
    Configured,
    /// Launch was disabled or refused by contract policy.
    LaunchRefused,
    /// Lifecycle state changed.
    LifecycleChanged,
    /// Health state changed.
    HealthChanged,
    /// Restart backoff state changed.
    RestartBackoffUpdated,
    /// Capability summary changed.
    CapabilitiesSummarized,
    /// Diagnostic summary was published.
    DiagnosticsSummarized,
    /// Request correlation metadata was recorded.
    RequestCorrelated,
    /// Edit-producing response was converted to a proposal.
    EditConvertedToProposal,
    /// Edit-producing response was rejected before mutation.
    EditRejected,
}

/// Redacted metadata-only event for LSP supervision boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspSupervisionEvent {
    /// Event identifier.
    pub event_id: EventId,
    /// Event sequence.
    pub sequence: EventSequence,
    /// Event kind.
    pub kind: LspSupervisionEventKind,
    /// Configured server identity.
    pub identity: LspConfiguredServerIdentity,
    /// Lifecycle state represented by this event.
    pub lifecycle_state: LspSupervisionLifecycleState,
    /// Health state represented by this event.
    pub health_state: LspHealthState,
    /// Optional request correlation metadata.
    pub request: Option<LspRequestCorrelation>,
    /// Restart/backoff metadata.
    pub restart_backoff: Option<LspRestartBackoffMetadata>,
    /// Capability summaries.
    pub capabilities: Vec<LspCapabilitySummary>,
    /// Diagnostic summaries.
    pub diagnostic_summaries: Vec<LspDiagnosticSummary>,
    /// Metadata-only diagnostics about this event.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Redaction hints for the event payload.
    pub redaction_hints: Vec<RedactionHint>,
    /// Supervision event schema version.
    pub schema_version: u16,
}

/// Contract validation failures for LSP supervision and proposal conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspContractValidationError {
    /// Correlation id must be non-zero.
    ZeroCorrelationId,
    /// Causality id must not be nil.
    NilCausalityId,
    /// Privacy scope denied edit or supervision output.
    PrivacyDenied,
    /// Proposal lifecycle state is incompatible with conversion.
    IncompatibleProposalLifecycle,
    /// Workspace edit required capability did not match the proposal capability.
    CapabilityMismatch,
    /// Required proposal precondition was absent.
    MissingPrecondition,
    /// Workspace edit did not include complete target coverage.
    IncompleteTargetCoverage,
    /// Workspace edit source was not an LSP edit-producing source.
    UnsupportedEditSource,
}

/// Input used to convert an LSP-produced edit into a proposal envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspEditProposalConversionInput {
    /// Proposal identifier to assign.
    pub proposal_id: ProposalId,
    /// Principal responsible for creating the proposal.
    pub principal: PrincipalId,
    /// Capability requested by the proposal.
    pub capability: CapabilityId,
    /// Request correlation metadata.
    pub request: LspRequestCorrelation,
    /// Workspace edit payload to wrap in a proposal.
    pub workspace_edit: WorkspaceEditProposalPayload,
    /// Envelope preconditions copied into the proposal.
    pub preconditions: ProposalVersionPreconditions,
    /// Current lifecycle state that allows proposal creation.
    pub lifecycle_state: ProposalLifecycleState,
    /// Privacy label for the proposal conversion.
    pub privacy_label: ProposalPrivacyLabel,
    /// Preview summary without raw diff bodies.
    pub preview: PreviewSummary,
    /// Proposal expiration timestamp.
    pub expires_at: Option<TimestampMillis>,
    /// Proposal creation timestamp.
    pub created_at: TimestampMillis,
    /// Conversion diagnostics.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Conversion input schema version.
    pub schema_version: u16,
}

/// Converts an LSP-produced edit into a workspace proposal without mutation.
pub fn convert_lsp_edit_to_workspace_proposal(
    input: LspEditProposalConversionInput,
) -> Result<WorkspaceProposal, LspContractValidationError> {
    validate_lsp_edit_proposal_contract(&input)?;

    let LspEditProposalConversionInput {
        proposal_id,
        principal,
        capability,
        request,
        workspace_edit,
        preconditions,
        preview,
        expires_at,
        created_at,
        ..
    } = input;

    Ok(WorkspaceProposal {
        proposal_id,
        principal,
        capability,
        correlation_id: request.correlation_id,
        payload: ProposalPayload::WorkspaceEdit(workspace_edit),
        preconditions,
        preview,
        expires_at,
        created_at,
    })
}

/// Validates the contract for LSP edit-to-proposal conversion.
pub fn validate_lsp_edit_proposal_contract(
    input: &LspEditProposalConversionInput,
) -> Result<(), LspContractValidationError> {
    if input.request.correlation_id.0 == 0 {
        return Err(LspContractValidationError::ZeroCorrelationId);
    }
    if input.request.causality_id.0 == Uuid::nil() {
        return Err(LspContractValidationError::NilCausalityId);
    }
    if matches!(
        input.request.privacy_scope,
        SemanticPrivacyScope::Redacted | SemanticPrivacyScope::MetadataOnly
    ) || matches!(
        input.privacy_label,
        ProposalPrivacyLabel::RedactedSensitive | ProposalPrivacyLabel::Unknown
    ) {
        return Err(LspContractValidationError::PrivacyDenied);
    }
    if !matches!(
        input.lifecycle_state,
        ProposalLifecycleState::Created
            | ProposalLifecycleState::Validated
            | ProposalLifecycleState::Previewed
    ) {
        return Err(LspContractValidationError::IncompatibleProposalLifecycle);
    }
    if input.workspace_edit.required_capability != input.capability {
        return Err(LspContractValidationError::CapabilityMismatch);
    }
    if !matches!(
        input.workspace_edit.source,
        WorkspaceEditSourceKind::LspRename
            | WorkspaceEditSourceKind::LspFormatting
            | WorkspaceEditSourceKind::LspCodeAction
    ) {
        return Err(LspContractValidationError::UnsupportedEditSource);
    }
    if !matches!(
        input.workspace_edit.target_coverage.coverage_kind,
        ProposalTargetCoverageKind::Complete
    ) {
        return Err(LspContractValidationError::IncompleteTargetCoverage);
    }
    if !proposal_preconditions_complete_for_lsp(&input.preconditions) {
        return Err(LspContractValidationError::MissingPrecondition);
    }
    if input.workspace_edit.file_edits.iter().any(|edit| {
        edit.preconditions.file_content_version.is_none()
            || edit.preconditions.workspace_generation.is_none()
            || edit.preconditions.buffer_version.is_none()
            || edit.preconditions.snapshot_id.is_none()
            || edit.preconditions.expected_fingerprint.is_none()
    }) {
        return Err(LspContractValidationError::MissingPrecondition);
    }

    Ok(())
}

fn proposal_preconditions_complete_for_lsp(preconditions: &ProposalVersionPreconditions) -> bool {
    preconditions.file_content_version.is_some()
        && preconditions.workspace_generation.is_some()
        && preconditions.buffer_version.is_some()
        && preconditions.snapshot_id.is_some()
        && preconditions.expected_fingerprint.is_some()
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Metadata-only source class for predictive semantic fabric scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticFabricWorkSourceKind {
    /// Workspace-authored discovery metadata or discovery delta.
    WorkspaceDiscovery,
    /// Descriptor-first snapshot, range, or chunk metadata.
    SourceDescriptor,
    /// Snapshot lease metadata; source bodies remain transient outside scheduling state.
    SnapshotLeaseMetadata,
    /// Existing metadata-only semantic persistence record.
    SemanticPersistence,
    /// LSP DTO metadata prepared for future fusion; this does not authorize process startup.
    LspDtoMetadata,
}

/// User or actor trigger used to assign deterministic semantic scheduling priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticFabricSchedulingTrigger {
    /// Visible viewport or direct navigation support.
    ForegroundViewport,
    /// Recent editor transaction or live snapshot supersession.
    RecentEdit,
    /// Save-adjacent invalidation or refresh.
    SaveAdjacent,
    /// Metadata-only LSP enrichment input; no runtime process is implied.
    LspEnrichment,
    /// Workspace discovery delta or watcher-like metadata.
    WorkspaceDiscovery,
    /// Dependency hint affecting related files.
    DependencyHint,
    /// Background crawl or broad repository refresh.
    BackgroundCrawl,
    /// Cache or persistence maintenance.
    Maintenance,
}

/// Semantic scheduling priority class exposed in metadata-only decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticFabricPriority {
    /// Foreground viewport or direct user request.
    ForegroundViewport,
    /// Recent edit or live snapshot work.
    RecentEdit,
    /// Save-adjacent invalidation work.
    SaveAdjacent,
    /// Dependency-hint work that should outrank background crawls.
    DependencyHint,
    /// LSP DTO metadata fusion work.
    LspEnrichment,
    /// Workspace discovery or watcher-delta work.
    WorkspaceDiscovery,
    /// Background crawl work.
    BackgroundCrawl,
    /// Maintenance work.
    Maintenance,
}

/// Metadata-only cause that changes semantic scheduling admission or freshness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticFabricInvalidationCause {
    /// No persisted metadata matched the requested freshness key.
    MetadataMissing,
    /// Privacy scope changed or was reduced.
    PrivacyScopeChanged,
    /// Workspace generation changed.
    WorkspaceGenerationChanged,
    /// Persistence or descriptor schema version changed.
    SchemaVersionChanged,
    /// Parser or extraction version changed.
    ParserVersionChanged,
    /// Grammar version changed.
    GrammarVersionChanged,
    /// Model metadata version changed.
    ModelVersionChanged,
    /// Language identifier changed.
    LanguageChanged,
    /// Descriptor identity changed.
    DescriptorIdentityChanged,
    /// Content hash changed.
    ContentHashChanged,
    /// Workspace discovery indicated deletion.
    DiscoveryDeleted,
    /// Queue pressure rejected, downgraded, or delayed lower-priority work.
    QueuePressure,
    /// Snapshot or content identity was superseded.
    SnapshotSuperseded,
}

/// Metadata-only action selected by the semantic fabric scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticFabricSchedulingAction {
    /// Admit a new metadata-described indexing job.
    Schedule,
    /// Refresh cache or persistence metadata without claiming fresh parser output.
    Refresh,
    /// Reindex because identity, descriptor, or semantic inputs changed.
    Reindex,
    /// Coalesce with an already fresh or equivalent job.
    Coalesce,
    /// Reject or drop work before it can affect interactive workflows.
    Reject,
}

/// Privacy label carried by semantic scheduling jobs and decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticFabricPrivacyLabel {
    /// Semantic privacy scope.
    pub privacy_scope: SemanticPrivacyScope,
    /// Whether the job must remain metadata-only.
    pub metadata_only: bool,
    /// Redaction hint that should be used for scheduling events.
    pub redaction: RedactionHint,
    /// Privacy-label schema version.
    pub schema_version: u16,
}

/// Metadata-only dependency hint for deterministic semantic job priority.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemanticFabricDependencyHint {
    /// Related file identity.
    pub file_id: FileId,
    /// Hash of the dependency label or edge kind.
    pub label_hash: FileFingerprint,
    /// Hint confidence in basis points.
    pub confidence_basis_points: u16,
    /// Hint schema version.
    pub schema_version: u16,
}

/// Metadata-only descriptor reference for semantic scheduling.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemanticFabricDescriptorReference {
    /// Source input kind represented by the descriptor.
    pub source_kind: SemanticMetadataSourceKind,
    /// Snapshot identifier when known.
    pub snapshot_id: Option<SnapshotId>,
    /// Snapshot or document content hash used for invalidation.
    pub content_hash: FileFingerprint,
    /// Snapshot byte length when known.
    pub byte_len: Option<u64>,
    /// Byte ranges represented by the descriptor.
    pub ranges: Vec<ByteRange>,
    /// Chunk references represented by the descriptor.
    pub chunks: Vec<SemanticMetadataChunkReference>,
    /// Highest schema version observed in descriptor components.
    pub schema_version: u16,
}

/// Metadata-only request admitted to the predictive semantic fabric scheduler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticFabricJobRequest {
    /// Deterministic caller-provided job identifier.
    pub job_id: String,
    /// Source class proving where scheduling metadata originated.
    pub source_kind: SemanticFabricWorkSourceKind,
    /// Scheduling trigger.
    pub trigger: SemanticFabricSchedulingTrigger,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Language identifier.
    pub language_id: LanguageId,
    /// File identity without source text.
    pub file_identity: SemanticFileFingerprintIdentity,
    /// Freshness key requested for the next semantic result.
    pub expected_freshness_key: SemanticMetadataFreshnessKey,
    /// Persisted freshness key, when an existing metadata record is being evaluated.
    pub persisted_freshness_key: Option<SemanticMetadataFreshnessKey>,
    /// Descriptor reference for scheduling and invalidation.
    pub descriptor: SemanticFabricDescriptorReference,
    /// Privacy label for the request.
    pub privacy: SemanticFabricPrivacyLabel,
    /// Metadata-only dependency hints.
    pub dependency_hints: Vec<SemanticFabricDependencyHint>,
    /// Cancellation token bound to this scheduled work.
    pub cancellation: SemanticCancellationToken,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Request schema version.
    pub schema_version: u16,
}

/// Metadata-only scheduling decision for one semantic fabric job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticFabricSchedulingDecision {
    /// Job identifier copied from the request.
    pub job_id: String,
    /// Selected scheduling action.
    pub action: SemanticFabricSchedulingAction,
    /// Priority class.
    pub priority: SemanticFabricPriority,
    /// Deterministic priority score; higher starts earlier.
    pub priority_score: u16,
    /// Freshness state represented by this decision.
    pub freshness_state: SemanticFreshnessState,
    /// Invalidation causes that led to refresh, reindex, or rejection.
    pub invalidation_causes: Vec<SemanticFabricInvalidationCause>,
    /// Cancellation reason when work is not admitted as-is.
    pub cancellation_reason: Option<SemanticCancellationReason>,
    /// Whether scheduling state remains metadata-only.
    pub metadata_only: bool,
    /// Queue depth observed by the actor-owned planner after this decision.
    pub queue_depth: u32,
    /// Metadata-only diagnostics.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Decision schema version.
    pub schema_version: u16,
}

/// Deterministic metadata-only scheduling plan for a batch of semantic jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticFabricSchedulePlan {
    /// Decisions in actor start order for admitted work, followed by coalesced or rejected work.
    pub decisions: Vec<SemanticFabricSchedulingDecision>,
    /// Capacity used by scheduled, refresh, or reindex decisions.
    pub admitted_count: u32,
    /// Queue capacity applied by the actor-owned planner.
    pub capacity: u32,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Plan schema version.
    pub schema_version: u16,
}

/// File content and fingerprint identity for semantic cache keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Metadata-only source input category for persisted semantic records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticMetadataSourceKind {
    /// Descriptor metadata only; no source payload is persisted.
    DescriptorOnly,
    /// Records derived from bounded lease chunks, storing only chunk metadata.
    LeaseChunks,
    /// Records derived from changed ranges and descriptor metadata.
    ChangedRanges,
    /// Records derived from an explicit bounded full-text optimization, without persisting text.
    BoundedFullText,
}

/// Metadata-only persisted chunk reference for semantic record provenance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemanticMetadataChunkReference {
    /// Snapshot identifier owning the chunk.
    pub snapshot_id: SnapshotId,
    /// Chunk ordinal in the snapshot.
    pub chunk_index: u32,
    /// Byte range covered by this chunk.
    pub byte_range: ByteRange,
    /// Line range covered by this chunk.
    pub line_range: LineIndexRange,
    /// Chunk byte length.
    pub byte_len: u64,
    /// Hash of the chunk contents.
    pub chunk_hash: FileFingerprint,
    /// Whether a transient lease existed; the lease id and chunk text are not persisted.
    pub lease_present: bool,
    /// Chunk descriptor schema version.
    pub schema_version: u16,
}

/// Metadata-only descriptor identity for freshness-gated semantic persistence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemanticMetadataDescriptorIdentity {
    /// Source input kind represented by the persisted metadata.
    pub source_kind: SemanticMetadataSourceKind,
    /// Snapshot identifier when known.
    pub snapshot_id: Option<SnapshotId>,
    /// Snapshot or document content hash used for invalidation.
    pub content_hash: FileFingerprint,
    /// Snapshot byte length when known.
    pub byte_len: Option<u64>,
    /// Byte ranges represented by the semantic metadata.
    pub ranges: Vec<ByteRange>,
    /// Chunk references represented by the semantic metadata.
    pub chunks: Vec<SemanticMetadataChunkReference>,
    /// Highest schema version observed in descriptor components.
    pub schema_version: u16,
}

/// Freshness key required before persisted semantic metadata can be query-authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemanticMetadataFreshnessKey {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Language identifier.
    pub language_id: LanguageId,
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
    /// Model metadata version for ranking or enrichment invalidation.
    pub model_version: Option<SemanticModelVersion>,
    /// Deterministic parser or extraction version.
    pub parser_version: String,
    /// Privacy scope for storage and query exposure.
    pub privacy_scope: SemanticPrivacyScope,
    /// Metadata descriptor identity.
    pub descriptor: SemanticMetadataDescriptorIdentity,
    /// Persistence schema version.
    pub schema_version: u16,
}

/// Metadata-only persisted symbol record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMetadataSymbolRecord {
    /// Stable symbol identifier.
    pub symbol_id: SemanticSymbolId,
    /// Hash of the symbol name.
    pub symbol_name_hash: FileFingerprint,
    /// Hash of the symbol kind label.
    pub kind_hash: FileFingerprint,
    /// Declaration range when known.
    pub declaration_range: Option<ProtocolTextRange>,
    /// Reference ranges known for the symbol.
    pub reference_ranges: Vec<ProtocolTextRange>,
    /// Symbol record schema version.
    pub schema_version: u16,
}

/// Metadata-only persisted graph record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMetadataGraphRecord {
    /// Stable graph record identifier.
    pub record_id: SemanticRecordId,
    /// Graph record kind.
    pub kind: SemanticGraphRecordKind,
    /// Source endpoint.
    pub source: SemanticGraphEndpoint,
    /// Optional target endpoint.
    pub target: Option<SemanticGraphEndpoint>,
    /// Hash of the relationship label.
    pub label_hash: FileFingerprint,
    /// Hashes of graph property keys and values.
    pub property_hashes: Vec<FileFingerprint>,
    /// Freshness state.
    pub freshness: SemanticFreshnessState,
    /// Graph metadata schema version.
    pub schema_version: u16,
}

/// Metadata-only persisted diagnostic summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMetadataDiagnosticSummary {
    /// Hash of the diagnostic code.
    pub code_hash: FileFingerprint,
    /// Diagnostic severity.
    pub severity: ProtocolDiagnosticSeverity,
    /// Diagnostic range when known.
    pub range: Option<ProtocolTextRange>,
    /// Number of diagnostics represented by this summary.
    pub count: u32,
}

/// Metadata-only persisted semantic record set for a single file/freshness key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMetadataRecord {
    /// Stable metadata record identifier.
    pub record_id: SemanticRecordId,
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier.
    pub file_id: FileId,
    /// Language identifier.
    pub language_id: LanguageId,
    /// Freshness key that must exactly match before reuse.
    pub freshness_key: SemanticMetadataFreshnessKey,
    /// File content fingerprint identity without source text.
    pub file_identity: SemanticFileFingerprintIdentity,
    /// Record provenance.
    pub provenance: SemanticRecordProvenance,
    /// Persisted symbol metadata.
    pub symbols: Vec<SemanticMetadataSymbolRecord>,
    /// Persisted graph metadata.
    pub graph_records: Vec<SemanticMetadataGraphRecord>,
    /// Persisted diagnostic summaries.
    pub diagnostic_summaries: Vec<SemanticMetadataDiagnosticSummary>,
    /// Record freshness state.
    pub freshness_state: SemanticFreshnessState,
    /// Timestamp when metadata was persisted.
    pub persisted_at: TimestampMillis,
    /// Metadata record schema version.
    pub schema_version: u16,
}

/// Reason persisted semantic metadata was tombstoned or rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticMetadataTombstoneReason {
    /// File was deleted.
    FileDeleted,
    /// Path policy denied indexing or storage.
    PathDenied,
    /// Workspace generation changed.
    WorkspaceGenerationChanged,
    /// Privacy scope was revoked or reduced.
    PrivacyScopeRevoked,
    /// Content hash changed.
    ContentHashMismatch,
    /// Cache or persistence schema changed.
    SchemaVersionChanged,
    /// Parser or extraction version changed.
    ParserVersionChanged,
    /// Grammar version changed.
    GrammarVersionChanged,
    /// Model metadata version changed.
    ModelVersionChanged,
    /// Language identifier changed.
    LanguageChanged,
    /// Descriptor identity changed.
    DescriptorIdentityChanged,
}

/// Metadata tombstone used to remove or quarantine stale semantic records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMetadataTombstone {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// File identifier when the tombstone is file-scoped.
    pub file_id: Option<FileId>,
    /// Freshness key that replaces or invalidates prior records when known.
    pub freshness_key: Option<SemanticMetadataFreshnessKey>,
    /// Tombstone reason.
    pub reason: SemanticMetadataTombstoneReason,
    /// Timestamp when the tombstone was observed.
    pub observed_at: TimestampMillis,
    /// Tombstone schema version.
    pub schema_version: u16,
}

/// Batch of semantic metadata records and tombstones for persistence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMetadataBatch {
    /// Records to upsert.
    pub records: Vec<SemanticMetadataRecord>,
    /// Tombstones to apply before records become query-authoritative.
    pub tombstones: Vec<SemanticMetadataTombstone>,
    /// Correlation id for the metadata write.
    pub correlation_id: CorrelationId,
    /// Causality id for the metadata write.
    pub causality_id: CausalityId,
    /// Batch schema version.
    pub schema_version: u16,
}

/// Query for freshness-gated persisted semantic metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMetadataQuery {
    /// Workspace identifier.
    pub workspace_id: WorkspaceId,
    /// Optional file identifiers to include.
    pub file_ids: Vec<FileId>,
    /// Optional language identifiers to include.
    pub language_ids: Vec<LanguageId>,
    /// Required privacy scope for query exposure.
    pub privacy_scope: SemanticPrivacyScope,
    /// Required freshness key; mismatched records are rejected from the result.
    pub freshness_key: Option<SemanticMetadataFreshnessKey>,
    /// Whether stale records may be returned as non-authoritative records.
    pub include_stale: bool,
    /// Query schema version.
    pub schema_version: u16,
}

/// Result of a freshness-gated semantic metadata query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMetadataReadResult {
    /// Fresh or explicitly allowed stale records returned to the caller.
    pub records: Vec<SemanticMetadataRecord>,
    /// Tombstones explaining records rejected or invalidated during query.
    pub rejected: Vec<SemanticMetadataTombstone>,
    /// Read-result schema version.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Runtime contract schema version.
    pub schema_version: u16,
    /// Minimum supported ABI version.
    pub min_abi_version: u16,
    /// Maximum supported ABI version.
    pub max_abi_version: u16,
    /// Module hash used for deterministic trust decisions.
    pub module_hash: String,
    /// Manifest identifier or signed manifest digest.
    pub manifest_id: String,
    /// Source/trust metadata.
    pub trust: PluginTrustMetadata,
    /// Optional signature metadata.
    pub signature: Option<PluginSignatureMetadata>,
    /// Activation events.
    pub activation_events: Vec<PluginActivationEvent>,
    /// Declarative contributions.
    pub contributions: Vec<PluginContribution>,
    /// Requested capabilities.
    pub requested_capabilities: Vec<CapabilityId>,
    /// Plugin-owned storage namespace.
    pub storage_namespace: PluginStateNamespace,
    /// Runtime quota declaration.
    pub quotas: PluginQuotaDeclaration,
}

/// Plugin trust metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTrustMetadata {
    /// Source classification.
    pub source: PluginTrustSource,
    /// Trust decision.
    pub decision: PluginTrustDecision,
    /// Human-readable decision reason.
    pub reason: String,
}

/// Plugin source classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginTrustSource {
    /// First-party bundled plugin.
    FirstParty,
    /// Explicitly allowed local plugin.
    ExplicitLocalAllow,
    /// Unknown local plugin.
    UnknownLocal,
    /// Revoked source.
    Revoked,
}

/// Plugin trust decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginTrustDecision {
    /// Manifest is trusted for activation.
    Trusted,
    /// Manifest is explicitly allowed locally.
    ExplicitlyAllowed,
    /// Manifest was revoked.
    Revoked,
    /// Module checksum mismatched.
    ChecksumMismatch,
    /// Signer is unknown.
    UnknownSigner,
    /// ABI is incompatible.
    IncompatibleAbi,
    /// Manifest is denied by default.
    DeniedByDefault,
}

/// Plugin signature metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSignatureMetadata {
    /// Signing key identifier.
    pub signer: String,
    /// Signature algorithm.
    pub algorithm: String,
    /// Detached signature digest or label, never raw key material.
    pub signature_digest: String,
}

/// Plugin quota declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginQuotaDeclaration {
    /// Maximum fuel units per invocation.
    pub max_fuel: u64,
    /// Maximum wall-clock milliseconds per invocation.
    pub max_wall_time_ms: u64,
    /// Maximum WebAssembly memory pages.
    pub max_memory_pages: u32,
    /// Maximum storage bytes for this plugin.
    pub max_storage_bytes: u64,
    /// Maximum host calls per invocation.
    pub max_host_calls: u32,
    /// Maximum emitted events per invocation.
    pub max_events: u32,
    /// Maximum bounded output bytes.
    pub max_output_bytes: u64,
}

/// Declarative plugin contribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginContribution {
    /// Command palette command.
    Command(PluginCommandDescriptor),
    /// Menu entry.
    Menu(PluginMenuContribution),
    /// Bounded panel projection.
    Panel(PluginPanelContribution),
    /// Status item projection.
    StatusItem(PluginStatusItemContribution),
    /// Editor decoration label.
    EditorDecoration(PluginEditorDecorationContribution),
    /// Snippet contribution.
    Snippet(PluginSnippetContribution),
    /// Language-provider availability.
    LanguageProvider(PluginLanguageProviderContribution),
    /// Formatter availability.
    Formatter(PluginFormatterContribution),
    /// LSP registration metadata only.
    LspRegistration(PluginLspRegistrationContribution),
    /// Workspace scanner metadata only.
    WorkspaceScanner(PluginWorkspaceScannerContribution),
    /// Passive AI context provider metadata only.
    PassiveAiContextProvider(ContextProviderDescriptor),
}

/// Menu contribution projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMenuContribution {
    /// Menu path label.
    pub menu_path: String,
    /// Command id to dispatch through app authority.
    pub command_id: String,
    /// Display label.
    pub title: String,
}

/// Panel contribution projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPanelContribution {
    /// Panel id.
    pub panel_id: String,
    /// Display title.
    pub title: String,
    /// Bounded metadata-only label.
    pub metadata_label: String,
}

/// Status item contribution projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginStatusItemContribution {
    /// Status item id.
    pub status_item_id: String,
    /// Display label.
    pub label: String,
}

/// Editor decoration contribution projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginEditorDecorationContribution {
    /// Decoration id.
    pub decoration_id: String,
    /// Display label.
    pub label: String,
}

/// Snippet contribution projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSnippetContribution {
    /// Language id.
    pub language_id: LanguageId,
    /// Snippet label.
    pub label: String,
}

/// Language provider contribution projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLanguageProviderContribution {
    /// Language id.
    pub language_id: LanguageId,
    /// Provider kind label.
    pub provider_kind: String,
}

/// Formatter contribution projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginFormatterContribution {
    /// Language id.
    pub language_id: LanguageId,
    /// Command id to invoke for proposal-producing format.
    pub command_id: String,
}

/// LSP registration metadata projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLspRegistrationContribution {
    /// Language id.
    pub language_id: LanguageId,
    /// Server label only; process launch remains separately gated.
    pub server_label: String,
}

/// Workspace scanner metadata projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginWorkspaceScannerContribution {
    /// Scanner id.
    pub scanner_id: String,
    /// Metadata-only scanner label.
    pub label: String,
}

/// Runtime host call class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginHostCallKind {
    /// Read-only context lookup.
    ReadOnlyContext,
    /// Semantic metadata query.
    SemanticMetadataQuery,
    /// Contribution registration.
    RegisterContribution,
    /// Proposal creation.
    CreateProposal,
    /// Metadata-only event emission.
    EmitEvent,
    /// Cancellation check.
    CheckCancellation,
    /// Plugin storage operation.
    Storage,
}

/// Runtime quota class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginQuotaClass {
    /// CPU/fuel budget.
    Fuel,
    /// Wall-clock timeout budget.
    WallTime,
    /// Memory-pages budget.
    Memory,
    /// Storage-byte budget.
    Storage,
    /// Host-call count budget.
    HostCall,
    /// Event count budget.
    Event,
    /// Output byte budget.
    Output,
}

/// Sandbox operation class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginSandboxOperationClass {
    /// WebAssembly module instantiation.
    Instantiate,
    /// Plugin activation.
    Activate,
    /// Command invocation.
    InvokeCommand,
    /// Host call dispatch.
    HostCall,
    /// Storage operation.
    Storage,
    /// Cancellation or teardown.
    Teardown,
}

/// Plugin host-call request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHostCallRequest {
    /// Plugin id.
    pub plugin_id: PluginId,
    /// Host-call kind.
    pub kind: PluginHostCallKind,
    /// Host-call name.
    pub host_call_name: String,
    /// Required declared capability.
    pub declared_capability: CapabilityId,
    /// Correlation id.
    pub correlation_id: CorrelationId,
    /// Causality id.
    pub causality_id: CausalityId,
    /// Event sequence for audit ordering.
    pub sequence: EventSequence,
    /// Metadata-only payload label.
    pub metadata_label: String,
}

/// Plugin denial reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginDenialReason {
    /// Host call is unsupported.
    UnsupportedHostCall,
    /// Capability was not declared or granted.
    MissingCapability,
    /// Workspace is untrusted.
    UntrustedWorkspace,
    /// ABI mismatch.
    AbiMismatch,
    /// Quota exceeded.
    QuotaExceeded,
    /// Invocation was cancelled.
    CancelledInvocation,
    /// Sandbox trapped or crashed.
    SandboxCrash,
    /// Metadata validation failed.
    InvalidMetadata,
}

/// Plugin host-call response.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginHostCallResponse {
    /// Host call accepted with metadata-only label.
    Accepted {
        /// Accepted metadata-only response label.
        metadata_label: String,
    },
    /// Proposal creation output.
    ProposalCreated(PluginActionProposal),
    /// Denied response.
    Denied {
        /// Denial reason.
        reason: PluginDenialReason,
        /// Human-readable denial message.
        message: String,
    },
}

/// Plugin storage operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginStorageOperation {
    /// Put a metadata record.
    Put,
    /// Get a metadata record.
    Get,
    /// Delete a metadata record.
    Delete,
    /// List metadata keys.
    List,
    /// Query quota usage.
    QuotaUsage,
}

/// Plugin storage record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStorageRecord {
    /// Workspace id.
    pub workspace_id: WorkspaceId,
    /// Plugin id.
    pub plugin_id: PluginId,
    /// Plugin namespace.
    pub namespace: PluginStateNamespace,
    /// Record key.
    pub key: String,
    /// Metadata-only value.
    pub value: String,
    /// Schema version.
    pub schema_version: u16,
    /// Retention label.
    pub retention: RetentionLabel,
    /// Redaction hint.
    pub redaction: RedactionHint,
    /// Byte count charged to quota.
    pub byte_count: u64,
}

/// Plugin storage request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStorageRequest {
    /// Operation.
    pub operation: PluginStorageOperation,
    /// Workspace id.
    pub workspace_id: WorkspaceId,
    /// Calling plugin id.
    pub plugin_id: PluginId,
    /// Target namespace.
    pub namespace: PluginStateNamespace,
    /// Optional record key.
    pub key: Option<String>,
    /// Optional metadata-only record.
    pub record: Option<PluginStorageRecord>,
    /// Maximum allowed bytes for this plugin.
    pub quota_bytes: u64,
    /// Correlation id.
    pub correlation_id: CorrelationId,
}

/// Plugin storage response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginStorageResponse {
    /// Record was stored.
    Stored {
        /// Stored record key.
        key: String,
        /// Used bytes after storing this record.
        used_bytes: u64,
    },
    /// Optional record result.
    Record(Option<PluginStorageRecord>),
    /// Listed keys.
    Keys(Vec<String>),
    /// Quota usage.
    QuotaUsage {
        /// Used bytes for the plugin.
        used_bytes: u64,
        /// Configured quota bytes.
        quota_bytes: u64,
    },
    /// Denied storage request.
    Denied {
        /// Denial reason.
        reason: PluginDenialReason,
        /// Human-readable denial message.
        message: String,
    },
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
    /// Activate on language identifier.
    OnLanguage {
        /// Language identifier.
        language_id: LanguageId,
    },
    /// Activate on workspace scanner trigger.
    OnWorkspaceScanner,
    /// Activate on passive context provider trigger.
    OnPassiveContextProvider,
}

/// Capability request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Plugin contribution projection for UI shells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginContributionProjection {
    /// Plugin id.
    pub plugin_id: PluginId,
    /// Contributions accepted for projection-only rendering.
    pub contributions: Vec<PluginContribution>,
    /// Metadata-only status label.
    pub status_label: String,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Plugin id for plugin-scoped policy decisions.
    pub plugin_id: Option<PluginId>,
    /// Plugin host-call name.
    pub plugin_host_call_name: Option<String>,
    /// Plugin module hash.
    pub plugin_module_hash: Option<String>,
    /// Plugin manifest id.
    pub plugin_manifest_id: Option<String>,
    /// Declared plugin capability id.
    pub plugin_declared_capability_id: Option<CapabilityId>,
    /// Plugin quota class for budget decisions.
    pub plugin_quota_class: Option<PluginQuotaClass>,
    /// Sandbox operation class.
    pub plugin_sandbox_operation_class: Option<PluginSandboxOperationClass>,
    /// Language-server binary for LSP-scoped policy decisions.
    pub lsp_server_binary: Option<String>,
    /// Explicit operator repair flag for storage migration repair requests.
    #[serde(default)]
    pub storage_explicit_repair: bool,
    /// Explicit operator apply flag for storage migration apply requests.
    #[serde(default)]
    pub storage_explicit_apply: bool,
    /// Whether current hosted telemetry consent has been verified for this request.
    #[serde(default)]
    pub hosted_telemetry_consent_current: bool,
    /// Whether current raw-source retention consent has been verified for this request.
    #[serde(default)]
    pub raw_source_retention_consent_current: bool,
    /// Whether separate hosted raw-source export consent has been verified.
    #[serde(default)]
    pub raw_source_hosted_export_consent_current: bool,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Read workspace-authored semantic discovery snapshot.
    ReadSemanticDiscoverySnapshot(WorkspaceId),
    /// Build a workspace-authored semantic discovery delta from watcher metadata.
    BuildSemanticDiscoveryDelta {
        /// Workspace id to resolve events against.
        workspace_id: WorkspaceId,
        /// Watcher events supplied by workspace authority.
        events: Vec<WatcherEvent>,
    },
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
    /// Workspace-authored semantic discovery snapshot.
    SemanticDiscoverySnapshot(WorkspaceDiscoverySnapshot),
    /// Workspace-authored semantic discovery delta.
    SemanticDiscoveryDelta(WorkspaceDiscoveryDelta),
    /// Conflict.
    Conflict(FileConflictState),
}

/// Semantic fabric request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SemanticRequest {
    /// Plan semantic fabric jobs through actor-owned scheduling policy.
    PlanJobs {
        /// Metadata-only job requests to plan.
        requests: Vec<SemanticFabricJobRequest>,
        /// Correlation id.
        correlation_id: CorrelationId,
        /// Causality id.
        causality_id: CausalityId,
    },
    /// Execute a semantic query against actor-owned index state.
    Query(SemanticQueryRequest),
    /// Cancel semantic work by token metadata.
    Cancel(SemanticCancellationToken),
}

/// Semantic fabric response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SemanticResponse {
    /// Metadata-only scheduling plan.
    SchedulePlan(SemanticFabricSchedulePlan),
    /// Semantic query response.
    Query(SemanticQueryResponse),
    /// Cancellation was accepted or observed by the semantic service boundary.
    Cancelled(SemanticCancellationToken),
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
    /// Kill terminal process/session.
    Kill(TerminalKillRequest),
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
    /// Register metadata-only supervision boundary without launching a process.
    RegisterSupervision(Box<LspLaunchPolicyDecision>),
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
    /// Supervision launch policy decision.
    SupervisionPlanned(Box<LspLaunchPolicyDecision>),
    /// Redacted supervision event.
    SupervisionEvent(Box<LspSupervisionEvent>),
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
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginRequest {
    /// Manifest.
    Manifest(PluginManifest),
    /// Command descriptor.
    CommandDescriptor(PluginCommandDescriptor),
    /// Contribution descriptor.
    Contribution(ContributionDescriptor),
    /// Host-call request.
    HostCall(PluginHostCallRequest),
}

/// Plugin response envelope.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginResponse {
    /// Manifest loaded.
    Loaded(PluginId),
    /// Command registered.
    CommandRegistered(String),
    /// Contribution registered.
    ContributionRegistered(String),
    /// Host-call response.
    HostCall(PluginHostCallResponse),
}

/// Capability request envelope.
#[allow(clippy::large_enum_variant)]
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
    /// Save metadata-only assisted-AI audit record.
    SaveAssistedAiAuditRecord(AssistedAiAuditRecord),
    /// Save metadata-only delegated-task readiness/audit linkage record.
    SaveDelegatedTaskAuditLinkageRecord(DelegatedTaskAuditLinkageRecord),
    /// Save metadata-only Phase 4 runtime audit record.
    SavePhase4RuntimeAuditRecord(Phase4RuntimeAuditRecord),
    /// Save metadata-only agent replay manifest.
    SaveAgentReplayManifest(AgentReplayManifest),
    /// Save metadata-only collaboration audit record.
    SaveCollaborationAuditRecord(CollaborationAuditRecord),
    /// Save metadata-only remote-development audit record.
    SaveRemoteAuditRecord(RemoteAuditRecord),
    /// Save metadata-only Phase 8 remote transport audit summary.
    SaveRemoteTransportAuditSummary(RemoteTransportAuditSummary),
    /// Save metadata-only Phase 8 terminal audit record.
    SaveTerminalAuditRecord(TerminalAuditRecord),
    /// Save metadata-only hosted telemetry spool record.
    SaveHostedTelemetrySpoolRecord(HostedTelemetrySpoolRecord),
    /// Save metadata-only raw-source retention access audit.
    SaveRawSourceRetentionAccessAudit(RawSourceRetentionAccessAudit),
    /// Save durable event metadata.
    SaveEventMetadata(EventMetadataRecord),
    /// Save metadata-only semantic records and tombstones.
    SaveSemanticMetadata(SemanticMetadataBatch),
    /// Tombstone metadata-only semantic records.
    TombstoneSemanticMetadata(SemanticMetadataTombstone),
    /// Execute plugin-scoped metadata storage operation.
    PluginStorage(PluginStorageRequest),
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
    /// Read metadata-only assisted-AI audit record.
    ReadAssistedAiAuditRecord(String),
    /// Read metadata-only delegated-task readiness/audit linkage record.
    ReadDelegatedTaskAuditLinkageRecord(String),
    /// Read metadata-only Phase 4 runtime audit record.
    ReadPhase4RuntimeAuditRecord(String),
    /// Read metadata-only agent replay manifest.
    ReadAgentReplayManifest(AgentRunId),
    /// Read metadata-only collaboration audit record.
    ReadCollaborationAuditRecord {
        /// Collaboration session identifier.
        session_id: CollaborationSessionId,
        /// Event sequence for the audit record.
        event_sequence: EventSequence,
    },
    /// Read metadata-only remote-development audit record.
    ReadRemoteAuditRecord {
        /// Remote workspace session identifier.
        session_id: RemoteWorkspaceSessionId,
        /// Event sequence for the audit record.
        event_sequence: EventSequence,
    },
    /// Read metadata-only Phase 8 remote transport audit summary.
    ReadRemoteTransportAuditSummary {
        /// Remote workspace session identifier.
        session_id: RemoteWorkspaceSessionId,
        /// Event sequence for the audit summary.
        event_sequence: EventSequence,
    },
    /// Read metadata-only Phase 8 terminal audit record.
    ReadTerminalAuditRecord {
        /// Terminal session identifier.
        session_id: TerminalSessionId,
        /// Event sequence for the audit record.
        event_sequence: EventSequence,
    },
    /// Read metadata-only hosted telemetry spool record by id.
    ReadHostedTelemetrySpoolRecord(String),
    /// Read metadata-only raw-source retention access audit.
    ReadRawSourceRetentionAccessAudit {
        /// Retention bundle identifier.
        bundle_id: String,
        /// Event sequence for the access audit.
        event_sequence: EventSequence,
    },
    /// Read durable event metadata.
    ReadEventMetadata(EventId),
    /// Read freshness-gated metadata-only semantic records.
    ReadSemanticMetadata(SemanticMetadataQuery),
    /// Read metadata-only semantic tombstones for a workspace and optional file.
    ReadSemanticMetadataTombstones {
        /// Workspace identifier.
        workspace_id: WorkspaceId,
        /// Optional file identifier.
        file_id: Option<FileId>,
    },
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
    /// Metadata-only assisted-AI audit record.
    AssistedAiAuditRecord(Box<Option<AssistedAiAuditRecord>>),
    /// Metadata-only delegated-task readiness/audit linkage record.
    DelegatedTaskAuditLinkageRecord(Box<Option<DelegatedTaskAuditLinkageRecord>>),
    /// Metadata-only Phase 4 runtime audit record.
    Phase4RuntimeAuditRecord(Box<Option<Phase4RuntimeAuditRecord>>),
    /// Metadata-only agent replay manifest.
    AgentReplayManifest(Box<Option<AgentReplayManifest>>),
    /// Metadata-only collaboration audit record.
    CollaborationAuditRecord(Box<Option<CollaborationAuditRecord>>),
    /// Metadata-only remote-development audit record.
    RemoteAuditRecord(Box<Option<RemoteAuditRecord>>),
    /// Metadata-only Phase 8 remote transport audit summary.
    RemoteTransportAuditSummary(Box<Option<RemoteTransportAuditSummary>>),
    /// Metadata-only Phase 8 terminal audit record.
    TerminalAuditRecord(Box<Option<TerminalAuditRecord>>),
    /// Metadata-only hosted telemetry spool record.
    HostedTelemetrySpoolRecord(Box<Option<HostedTelemetrySpoolRecord>>),
    /// Metadata-only raw-source retention access audit.
    RawSourceRetentionAccessAudit(Box<Option<RawSourceRetentionAccessAudit>>),
    /// Event metadata.
    EventMetadata(Option<EventMetadataRecord>),
    /// Freshness-gated semantic metadata read result.
    SemanticMetadata(SemanticMetadataReadResult),
    /// Semantic metadata tombstones.
    SemanticMetadataTombstones(Vec<SemanticMetadataTombstone>),
    /// Plugin storage response.
    PluginStorage(PluginStorageResponse),
    /// Missing.
    Missing,
}

/// Validate Phase 5 plugin manifest metadata before runtime activation.
pub fn validate_plugin_manifest(
    manifest: &PluginManifest,
    host_abi_version: u16,
) -> ProtocolResult<()> {
    if manifest.plugin_id.0 == 0 {
        return Err(ProtocolError {
            code: "plugin_manifest_invalid".to_string(),
            message: "plugin id must be non-zero".to_string(),
        });
    }
    if manifest.name.trim().is_empty() {
        return Err(ProtocolError {
            code: "plugin_manifest_invalid".to_string(),
            message: "plugin name must be non-empty".to_string(),
        });
    }
    if manifest.schema_version == 0
        || manifest.min_abi_version == 0
        || manifest.max_abi_version == 0
        || manifest.min_abi_version > manifest.max_abi_version
    {
        return Err(ProtocolError {
            code: "plugin_manifest_invalid".to_string(),
            message: "plugin ABI/schema version is invalid".to_string(),
        });
    }
    if host_abi_version < manifest.min_abi_version || host_abi_version > manifest.max_abi_version {
        return Err(ProtocolError {
            code: "plugin_abi_mismatch".to_string(),
            message: "plugin ABI range does not include host ABI".to_string(),
        });
    }
    if manifest.module_hash.trim().is_empty() || manifest.manifest_id.trim().is_empty() {
        return Err(ProtocolError {
            code: "plugin_manifest_invalid".to_string(),
            message: "plugin module hash and manifest id are required".to_string(),
        });
    }
    if manifest.storage_namespace.plugin_id != manifest.plugin_id
        || manifest.storage_namespace.namespace.trim().is_empty()
    {
        return Err(ProtocolError {
            code: "plugin_namespace_invalid".to_string(),
            message: "plugin storage namespace must match plugin id and be non-empty".to_string(),
        });
    }
    if manifest
        .requested_capabilities
        .iter()
        .any(|capability| contains_forbidden_plugin_payload(&capability.0))
    {
        return Err(ProtocolError {
            code: "plugin_manifest_invalid".to_string(),
            message: "plugin capability contains forbidden payload marker".to_string(),
        });
    }
    Ok(())
}

/// Validate plugin host-call audit metadata.
pub fn validate_plugin_host_call_request(request: &PluginHostCallRequest) -> ProtocolResult<()> {
    if request.plugin_id.0 == 0 {
        return Err(ProtocolError {
            code: "plugin_host_call_invalid".to_string(),
            message: "plugin id must be non-zero".to_string(),
        });
    }
    if request.host_call_name.trim().is_empty() || request.metadata_label.trim().is_empty() {
        return Err(ProtocolError {
            code: "plugin_host_call_invalid".to_string(),
            message: "host call name and metadata label are required".to_string(),
        });
    }
    if request.correlation_id.0 == 0
        || request.causality_id.0 == Uuid::nil()
        || request.sequence.0 == 0
    {
        return Err(ProtocolError {
            code: "plugin_host_call_invalid".to_string(),
            message: "correlation, causality, and sequence metadata must be non-zero".to_string(),
        });
    }
    if contains_forbidden_plugin_payload(&request.metadata_label) {
        return Err(ProtocolError {
            code: "plugin_host_call_invalid".to_string(),
            message: "host-call metadata contains forbidden payload marker".to_string(),
        });
    }
    Ok(())
}

/// Validate plugin storage metadata before persistence.
pub fn validate_plugin_storage_record(record: &PluginStorageRecord) -> ProtocolResult<()> {
    if record.plugin_id.0 == 0
        || record.namespace.plugin_id != record.plugin_id
        || record.namespace.namespace.trim().is_empty()
    {
        return Err(ProtocolError {
            code: "plugin_storage_invalid".to_string(),
            message: "plugin storage namespace is invalid".to_string(),
        });
    }
    if record.key.trim().is_empty() || record.schema_version == 0 || record.byte_count == 0 {
        return Err(ProtocolError {
            code: "plugin_storage_invalid".to_string(),
            message: "plugin storage key, schema version, and byte count are required".to_string(),
        });
    }
    if contains_forbidden_plugin_payload(&record.key)
        || contains_forbidden_plugin_payload(&record.value)
    {
        return Err(ProtocolError {
            code: "plugin_storage_invalid".to_string(),
            message: "plugin storage contains forbidden payload marker".to_string(),
        });
    }
    Ok(())
}

/// Validate metadata-only collaboration audit records before persistence.
pub fn validate_collaboration_audit_record(
    record: &CollaborationAuditRecord,
) -> ProtocolResult<()> {
    if record.session_id.0 == 0
        || record.event_sequence.0 == 0
        || record.correlation_id.0 == 0
        || record.causality_id.0 == Uuid::nil()
        || record.schema_version == 0
    {
        return Err(ProtocolError {
            code: "collaboration_audit_invalid".to_string(),
            message: "session, event sequence, correlation, causality, and schema metadata must be non-zero".to_string(),
        });
    }
    if !record
        .redaction_hints
        .contains(&RedactionHint::MetadataOnly)
        && record.retention_label == RetentionLabel::Audit
    {
        return Err(ProtocolError {
            code: "collaboration_audit_invalid".to_string(),
            message: "audit-retained collaboration records must be metadata-only".to_string(),
        });
    }
    if contains_forbidden_collaboration_payload(&record.metadata_summary) {
        return Err(ProtocolError {
            code: "collaboration_audit_invalid".to_string(),
            message: "collaboration audit metadata contains forbidden source or secret marker"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate metadata-only remote-development audit records before persistence.
pub fn validate_remote_audit_record(record: &RemoteAuditRecord) -> ProtocolResult<()> {
    if record.session_id.0 == 0
        || record.event_sequence.0 == 0
        || record.correlation_id.0 == 0
        || record.causality_id.0 == Uuid::nil()
        || record.schema_version == 0
    {
        return Err(ProtocolError {
            code: "remote_audit_invalid".to_string(),
            message: "session, event sequence, correlation, causality, and schema metadata must be non-zero".to_string(),
        });
    }
    if !record.is_metadata_only_valid() && record.retention_label == RetentionLabel::Audit {
        return Err(ProtocolError {
            code: "remote_audit_invalid".to_string(),
            message: "audit-retained remote records must be metadata-only".to_string(),
        });
    }
    if contains_forbidden_remote_payload(&record.metadata_summary) {
        return Err(ProtocolError {
            code: "remote_audit_invalid".to_string(),
            message: "remote audit metadata contains forbidden source, transcript, output, transport, or secret marker".to_string(),
        });
    }
    Ok(())
}

/// Validate inert Phase 8 remote transport handshake metadata before runtime activation.
pub fn validate_remote_transport_handshake(
    handshake: &RemoteTransportHandshake,
) -> ProtocolResult<()> {
    if handshake.session_id.0 == 0
        || handshake.endpoint.endpoint_id.trim().is_empty()
        || handshake.endpoint.host.trim().is_empty()
        || handshake.endpoint.schema_version == 0
        || handshake.peer_identity.authority_id.0 == 0
        || handshake.peer_identity.agent_id.0 == 0
        || handshake.peer_identity.principal_id.0.trim().is_empty()
        || handshake
            .peer_identity
            .credential_reference
            .trim()
            .is_empty()
        || handshake.peer_identity.schema_version == 0
        || handshake.capability_decision.decision_id.0 == 0
        || !handshake.capability_decision.granted
        || handshake.correlation_id.0 == 0
        || handshake.causality_id.0 == Uuid::nil()
        || handshake.event_sequence.0 == 0
        || handshake.schema_version == 0
    {
        return Err(ProtocolError {
            code: "remote_transport_handshake_invalid".to_string(),
            message: "handshake identity, endpoint, capability, and event metadata must be present"
                .to_string(),
        });
    }
    if handshake.trust_state != WorkspaceTrustState::Trusted
        || handshake.schema_compatibility == RemoteTransportSchemaCompatibility::Incompatible
    {
        return Err(ProtocolError {
            code: "remote_transport_handshake_invalid".to_string(),
            message: "remote transport handshake must be trusted and schema-compatible".to_string(),
        });
    }
    Ok(())
}

/// Validate remote transport credential references do not contain inline TLS material.
pub fn validate_remote_transport_credential_reference(
    reference: &RemoteTransportCredentialReference,
) -> ProtocolResult<()> {
    if reference.reference_id.trim().is_empty()
        || reference.kind.trim().is_empty()
        || reference.digest.algorithm.trim().is_empty()
        || reference.digest.value.trim().is_empty()
        || reference.schema_version == 0
        || contains_forbidden_phase8_payload(&reference.reference_id)
        || contains_forbidden_phase8_payload(&reference.kind)
        || contains_forbidden_phase8_payload(&reference.digest.value)
    {
        return Err(ProtocolError {
            code: "remote_transport_credential_invalid".to_string(),
            message:
                "remote transport credential references must be metadata-only digest references"
                    .to_string(),
        });
    }
    Ok(())
}

/// Validate remote transport TLS/mTLS policy before carrier connection.
pub fn validate_remote_transport_tls_policy(
    policy: &RemoteTransportTlsPolicy,
) -> ProtocolResult<()> {
    if !policy.require_tls
        || policy.server_identity.trim().is_empty()
        || policy.alpn_protocols.is_empty()
        || policy.min_schema_version == 0
        || policy.max_schema_version < policy.min_schema_version
        || policy.schema_version == 0
        || contains_forbidden_phase8_payload(&policy.server_identity)
        || policy
            .alpn_protocols
            .iter()
            .any(|alpn| alpn.trim().is_empty() || contains_forbidden_phase8_payload(alpn))
    {
        return Err(ProtocolError {
            code: "remote_transport_tls_policy_invalid".to_string(),
            message:
                "remote transport TLS policy must require TLS with bounded ALPN/schema metadata"
                    .to_string(),
        });
    }
    for reference in [
        policy.root_store_reference.as_ref(),
        policy.certificate_pin_reference.as_ref(),
        policy.client_credential_reference.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_remote_transport_credential_reference(reference)?;
    }
    if policy.mtls_mode == RemoteTransportMutualTlsMode::Required
        && policy.client_credential_reference.is_none()
    {
        return Err(ProtocolError {
            code: "remote_transport_tls_policy_invalid".to_string(),
            message: "required mTLS must include a client credential reference".to_string(),
        });
    }
    Ok(())
}

/// Validate remote transport endpoint policy rejects plaintext/downgrade metadata.
pub fn validate_remote_transport_endpoint_policy(
    policy: &RemoteTransportEndpointPolicy,
) -> ProtocolResult<()> {
    if policy.endpoint.endpoint_id.trim().is_empty()
        || policy.endpoint.host.trim().is_empty()
        || policy.endpoint.schema_version == 0
        || policy.allowed_schemes.is_empty()
        || policy.redirects_allowed
        || policy.schema_version == 0
        || policy.endpoint.scheme != "https"
        || !policy
            .allowed_schemes
            .iter()
            .any(|scheme| scheme == "https")
        || policy.allowed_schemes.iter().any(|scheme| scheme == "http")
        || contains_forbidden_phase8_payload(&policy.endpoint.endpoint_id)
        || contains_forbidden_phase8_payload(&policy.endpoint.host)
    {
        return Err(ProtocolError {
            code: "remote_transport_endpoint_policy_invalid".to_string(),
            message: "remote transport endpoint policy must be https-only and downgrade-safe"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate a remote transport connection attempt before any network side effects.
pub fn validate_remote_transport_connection_attempt(
    attempt: &RemoteTransportConnectionAttempt,
) -> ProtocolResult<()> {
    validate_remote_transport_endpoint_policy(&attempt.endpoint_policy)?;
    validate_remote_transport_tls_policy(&attempt.tls_policy)?;
    if attempt.selected_alpn.trim().is_empty()
        || !attempt
            .tls_policy
            .alpn_protocols
            .contains(&attempt.selected_alpn)
        || attempt.selected_schema_version < attempt.tls_policy.min_schema_version
        || attempt.selected_schema_version > attempt.tls_policy.max_schema_version
        || attempt.timeout_ms == 0
        || attempt.timeout_ms > 120_000
        || attempt.capability_decision.decision_id.0 == 0
        || attempt.capability_decision.capability.0 != "remote.transport.connect"
        || !attempt.capability_decision.granted
        || attempt.event_sequence.0 == 0
        || attempt.correlation_id.0 == 0
        || attempt.causality_id.0 == Uuid::nil()
        || attempt.schema_version == 0
        || !phase8_metadata_only_redaction(&attempt.redaction_hints)
        || contains_forbidden_phase8_payload(&attempt.selected_alpn)
        || contains_forbidden_phase8_payload(&attempt.metadata_summary)
    {
        return Err(ProtocolError {
            code: "remote_transport_connection_attempt_invalid".to_string(),
            message: "remote transport connection attempts require granted capability, TLS negotiation, bounded timeout, and metadata-only audit".to_string(),
        });
    }
    Ok(())
}

/// Validate remote transport carrier diagnostics remain metadata-only.
pub fn validate_remote_transport_carrier_diagnostic(
    diagnostic: &RemoteTransportCarrierDiagnostic,
) -> ProtocolResult<()> {
    if diagnostic.session_id.is_some_and(|session| session.0 == 0)
        || diagnostic.event_sequence.0 == 0
        || diagnostic.correlation_id.0 == 0
        || diagnostic.causality_id.0 == Uuid::nil()
        || diagnostic.schema_version == 0
        || !phase8_metadata_only_redaction(&diagnostic.redaction_hints)
        || contains_forbidden_phase8_payload(&diagnostic.metadata_summary)
    {
        return Err(ProtocolError {
            code: "remote_transport_diagnostic_invalid".to_string(),
            message: "remote transport diagnostics must be metadata-only with valid event identity"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate metadata-only remote transport flow-control state.
pub fn validate_remote_transport_flow_control_window(
    window: &RemoteTransportFlowControlWindow,
) -> ProtocolResult<()> {
    if window.session_id.0 == 0
        || window.max_inflight_frames == 0
        || window.available_credit > window.max_inflight_frames
        || window.max_frame_bytes == 0
        || window.queued_frame_count > window.max_inflight_frames
        || window.correlation_id.0 == 0
        || window.causality_id.0 == Uuid::nil()
        || window.schema_version == 0
    {
        return Err(ProtocolError {
            code: "remote_transport_flow_control_invalid".to_string(),
            message: "remote transport flow-control window must be bounded and event-identified"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate metadata-only remote transport replay window state.
pub fn validate_remote_transport_replay_window(
    window: &RemoteTransportReplayWindow,
) -> ProtocolResult<()> {
    if window.session_id.0 == 0
        || window.lowest_accepted_sequence.0 == 0
        || window.highest_accepted_sequence.0 < window.lowest_accepted_sequence.0
        || window.accepted_operation_count == 0
        || window.schema_version == 0
    {
        return Err(ProtocolError {
            code: "remote_transport_replay_window_invalid".to_string(),
            message: "remote transport replay window must be ordered and non-empty".to_string(),
        });
    }
    Ok(())
}

/// Validate remote agent package metadata before activation.
pub fn validate_remote_agent_package_descriptor(
    descriptor: &RemoteAgentPackageDescriptor,
) -> ProtocolResult<()> {
    if descriptor.agent_id.0 == 0
        || descriptor.authority_id.0 == 0
        || descriptor.package_id.trim().is_empty()
        || descriptor.version.trim().is_empty()
        || descriptor.package_digest.algorithm.trim().is_empty()
        || descriptor.package_digest.value.trim().is_empty()
        || descriptor.signature_reference.trim().is_empty()
        || descriptor.declared_capabilities.is_empty()
        || descriptor.capability_decision.decision_id.0 == 0
        || descriptor.capability_decision.capability.0 != "remote.agent.package.activate"
        || !descriptor.capability_decision.granted
        || descriptor.schema_version == 0
        || !descriptor
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
        || descriptor.redaction_hints.contains(&RedactionHint::None)
        || contains_forbidden_phase8_payload(&descriptor.package_id)
        || contains_forbidden_phase8_payload(&descriptor.version)
        || contains_forbidden_phase8_payload(&descriptor.signature_reference)
    {
        return Err(ProtocolError {
            code: "remote_agent_package_invalid".to_string(),
            message: "remote agent package activation requires metadata-only identity, integrity, signature, and granted capability".to_string(),
        });
    }
    Ok(())
}

/// Validate remote agent package lifecycle records remain metadata-only and ordered.
pub fn validate_remote_agent_package_lifecycle_record(
    record: &RemoteAgentPackageLifecycleRecord,
) -> ProtocolResult<()> {
    if record.agent_id.0 == 0
        || record.authority_id.0 == 0
        || record.package_id.trim().is_empty()
        || record.package_digest.algorithm.trim().is_empty()
        || record.package_digest.value.trim().is_empty()
        || record.event_sequence.0 == 0
        || record.correlation_id.0 == 0
        || record.causality_id.0 == Uuid::nil()
        || record.schema_version == 0
        || !phase8_metadata_only_redaction(&record.redaction_hints)
        || contains_forbidden_phase8_payload(&record.package_id)
        || contains_forbidden_phase8_payload(&record.package_digest.value)
        || contains_forbidden_phase8_payload(&record.metadata_summary)
        || record
            .rollback_reference
            .as_deref()
            .is_some_and(contains_forbidden_phase8_payload)
    {
        return Err(ProtocolError {
            code: "remote_agent_package_lifecycle_invalid".to_string(),
            message: "remote agent package lifecycle records must be metadata-only with package integrity and event identity".to_string(),
        });
    }
    Ok(())
}

/// Validate Phase 8 remote transport metadata summaries remain metadata-only.
pub fn validate_remote_transport_audit_summary(
    summary: &RemoteTransportAuditSummary,
) -> ProtocolResult<()> {
    if summary.session_id.0 == 0
        || summary.event_sequence.0 == 0
        || summary.correlation_id.0 == 0
        || summary.causality_id.0 == Uuid::nil()
        || summary.schema_version == 0
        || !summary
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
        || summary.redaction_hints.contains(&RedactionHint::None)
    {
        return Err(ProtocolError {
            code: "remote_transport_audit_invalid".to_string(),
            message: "remote transport audit must be metadata-only with valid event identity"
                .to_string(),
        });
    }
    if contains_forbidden_phase8_payload(&summary.metadata_summary) {
        return Err(ProtocolError {
            code: "remote_transport_audit_invalid".to_string(),
            message: "remote transport audit metadata contains forbidden raw payload marker"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate Phase 8 terminal audit summaries remain metadata-only.
pub fn validate_terminal_audit_record(record: &TerminalAuditRecord) -> ProtocolResult<()> {
    if record.session_id.0 == 0
        || record.event_sequence.0 == 0
        || record.correlation_id.0 == 0
        || record.causality_id.0 == Uuid::nil()
        || record.schema_version == 0
        || !record
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
        || record.redaction_hints.contains(&RedactionHint::None)
    {
        return Err(ProtocolError {
            code: "terminal_audit_invalid".to_string(),
            message: "terminal audit must be metadata-only with valid event identity".to_string(),
        });
    }
    if contains_forbidden_phase8_payload(&record.metadata_summary) {
        return Err(ProtocolError {
            code: "terminal_audit_invalid".to_string(),
            message: "terminal audit metadata contains forbidden raw output marker".to_string(),
        });
    }
    Ok(())
}

/// Validate terminal launch policy metadata before PTY activation.
pub fn validate_terminal_launch_policy_contract(
    contract: &TerminalLaunchPolicyContract,
) -> ProtocolResult<()> {
    if contract.principal_id.0.trim().is_empty()
        || contract.workspace_id.0 == 0
        || contract.trust_state != WorkspaceTrustState::Trusted
        || contract.capability_id.0 != "terminal.launch"
        || contract.cwd_policy.trim().is_empty()
        || contract.output_byte_limit == 0
        || contract.timeout_seconds == 0
        || contract.schema_version == 0
        || contains_forbidden_phase8_payload(&contract.cwd_policy)
    {
        return Err(ProtocolError {
            code: "terminal_launch_policy_invalid".to_string(),
            message: "terminal launch requires trusted workspace, terminal.launch capability, and bounded metadata".to_string(),
        });
    }
    Ok(())
}

/// Validate terminal input remains bounded before runtime dispatch.
pub fn validate_terminal_input(input: &TerminalInput) -> ProtocolResult<()> {
    if input.session_id.0 == 0
        || input.correlation_id.0 == 0
        || input.payload.is_empty()
        || input.payload.len() > 64 * 1024
        || contains_forbidden_phase8_payload(&input.payload)
    {
        return Err(ProtocolError {
            code: "terminal_input_invalid".to_string(),
            message:
                "terminal input must be session-scoped, bounded, and free of raw/secret markers"
                    .to_string(),
        });
    }
    Ok(())
}

/// Validate terminal resize requests are bounded.
pub fn validate_terminal_resize(resize: &TerminalResize) -> ProtocolResult<()> {
    if resize.session_id.0 == 0
        || resize.cols == 0
        || resize.rows == 0
        || resize.cols > 1_000
        || resize.rows > 1_000
    {
        return Err(ProtocolError {
            code: "terminal_resize_invalid".to_string(),
            message:
                "terminal resize must be session-scoped with non-zero bounded rows and columns"
                    .to_string(),
        });
    }
    Ok(())
}

/// Validate typed terminal close requests.
pub fn validate_terminal_close_request(request: &TerminalCloseRequest) -> ProtocolResult<()> {
    if request.session_id.0 == 0
        || request.principal_id.0.trim().is_empty()
        || request.capability_id.0 != "terminal.close"
        || request.event_sequence.0 == 0
        || request.correlation_id.0 == 0
        || request.causality_id.0 == Uuid::nil()
        || request.schema_version == 0
        || !phase8_metadata_only_redaction(&request.redaction_hints)
        || contains_forbidden_phase8_payload(&request.metadata_summary)
    {
        return Err(ProtocolError {
            code: "terminal_close_invalid".to_string(),
            message: "terminal close requires principal, terminal.close capability, and metadata-only event identity".to_string(),
        });
    }
    Ok(())
}

/// Validate typed terminal kill requests.
pub fn validate_terminal_kill_request(request: &TerminalKillRequest) -> ProtocolResult<()> {
    if request.session_id.0 == 0
        || request.principal_id.0.trim().is_empty()
        || request.capability_id.0 != "terminal.kill"
        || request.escalation_timeout_ms == 0
        || request.escalation_timeout_ms > 30_000
        || request.event_sequence.0 == 0
        || request.correlation_id.0 == 0
        || request.causality_id.0 == Uuid::nil()
        || request.schema_version == 0
        || !phase8_metadata_only_redaction(&request.redaction_hints)
        || contains_forbidden_phase8_payload(&request.metadata_summary)
    {
        return Err(ProtocolError {
            code: "terminal_kill_invalid".to_string(),
            message: "terminal kill requires principal, terminal.kill capability, bounded escalation, and metadata-only event identity".to_string(),
        });
    }
    if request.escalation == TerminalKillEscalation::KillTree && !request.kill_tree_authorized {
        return Err(ProtocolError {
            code: "terminal_kill_invalid".to_string(),
            message: "terminal kill-tree escalation requires explicit authorization".to_string(),
        });
    }
    Ok(())
}

/// Validate hosted telemetry export batches before hosted egress is considered.
pub fn validate_hosted_telemetry_export_batch(
    batch: &HostedTelemetryExportBatch,
) -> ProtocolResult<()> {
    if batch.batch_id.trim().is_empty()
        || batch.schema_version == 0
        || !batch.endpoint.allowlisted
        || batch.endpoint.endpoint_id.trim().is_empty()
        || batch.endpoint.schema_version == 0
        || batch.consent.schema_version == 0
        || batch.consent.principal_id.0.trim().is_empty()
        || batch.consent.workspace_id.0 == 0
        || batch.consent.correlation_id.0 == 0
        || batch.records.is_empty()
    {
        return Err(ProtocolError {
            code: "hosted_telemetry_export_invalid".to_string(),
            message: "hosted telemetry export requires endpoint allowlist, consent, and records"
                .to_string(),
        });
    }
    for record in &batch.records {
        if record.record_id.trim().is_empty()
            || record.workspace_id != batch.consent.workspace_id
            || record.event_sequence.0 == 0
            || record.correlation_id.0 == 0
            || record.causality_id.0 == Uuid::nil()
            || record.schema_version == 0
            || !record
                .redaction_hints
                .contains(&RedactionHint::MetadataOnly)
            || matches!(
                record.classification,
                PrivacyClassification::Sensitive | PrivacyClassification::RawContent
            )
            || contains_forbidden_phase8_payload(&record.metadata_summary)
        {
            return Err(ProtocolError {
                code: "hosted_telemetry_export_invalid".to_string(),
                message: "hosted telemetry records must be classified metadata-only".to_string(),
            });
        }
        if !batch.consent.categories.contains(&record.category) {
            return Err(ProtocolError {
                code: "hosted_telemetry_export_invalid".to_string(),
                message: "hosted telemetry record category is not covered by consent".to_string(),
            });
        }
    }
    Ok(())
}

/// Validate hosted telemetry consent binding is current and scope-bound.
pub fn validate_hosted_telemetry_consent_binding(
    binding: &HostedTelemetryConsentBinding,
) -> ProtocolResult<()> {
    if binding.grant_id.trim().is_empty()
        || binding.principal_id.0.trim().is_empty()
        || binding.workspace_id.0 == 0
        || binding.endpoint_id.trim().is_empty()
        || binding.region.trim().is_empty()
        || binding.categories.is_empty()
        || binding.issued_at.0 == 0
        || binding.correlation_id.0 == 0
        || binding.schema_version == 0
        || binding.state != HostedTelemetryConsentState::Current
        || contains_forbidden_phase8_payload(&binding.grant_id)
        || contains_forbidden_phase8_payload(&binding.endpoint_id)
        || contains_forbidden_phase8_payload(&binding.region)
    {
        return Err(ProtocolError {
            code: "hosted_telemetry_consent_invalid".to_string(),
            message: "hosted telemetry consent must be current and bound to principal, workspace, endpoint, region, and categories".to_string(),
        });
    }
    Ok(())
}

/// Validate hosted telemetry endpoint, TLS, proxy, and retry policy metadata.
pub fn validate_hosted_telemetry_endpoint_policy(
    policy: &HostedTelemetryEndpointPolicy,
) -> ProtocolResult<()> {
    if policy.endpoint.endpoint_id.trim().is_empty()
        || policy.endpoint.endpoint_label.trim().is_empty()
        || policy.endpoint.region.trim().is_empty()
        || !policy.endpoint.allowlisted
        || policy.endpoint.schema_version == 0
        || !policy.tls_policy.https_required
        || policy.tls_policy.min_tls_version.trim().is_empty()
        || !policy.tls_policy.certificate_validation_required
        || policy.tls_policy.schema_version == 0
        || policy.proxy_policy.bypass_allowed
        || policy.proxy_policy.schema_version == 0
        || policy.retry_policy.max_attempts == 0
        || policy.retry_policy.initial_backoff_ms == 0
        || policy.retry_policy.max_backoff_ms < policy.retry_policy.initial_backoff_ms
        || policy.retry_policy.schema_version == 0
        || policy.allowed_categories.is_empty()
        || policy.max_body_bytes == 0
        || policy.schema_version == 0
        || !policy.endpoint.endpoint_label.starts_with("https://")
        || contains_forbidden_phase8_payload(&policy.endpoint.endpoint_id)
        || contains_forbidden_phase8_payload(&policy.endpoint.endpoint_label)
        || contains_forbidden_phase8_payload(&policy.endpoint.region)
        || policy
            .proxy_policy
            .proxy_endpoint_id
            .as_deref()
            .is_some_and(contains_forbidden_phase8_payload)
    {
        return Err(ProtocolError {
            code: "hosted_telemetry_endpoint_policy_invalid".to_string(),
            message: "hosted telemetry endpoint policy must be allowlisted HTTPS metadata with fail-closed proxy/retry bounds".to_string(),
        });
    }
    Ok(())
}

/// Validate hosted telemetry diagnostics snapshots are metadata-only.
pub fn validate_hosted_telemetry_diagnostics_snapshot(
    snapshot: &HostedTelemetryDiagnosticsSnapshot,
) -> ProtocolResult<()> {
    if snapshot.workspace_id.0 == 0
        || snapshot.last_upload_status.trim().is_empty()
        || snapshot.event_sequence.0 == 0
        || snapshot.correlation_id.0 == 0
        || snapshot.causality_id.0 == Uuid::nil()
        || snapshot.schema_version == 0
        || !phase8_metadata_only_redaction(&snapshot.redaction_hints)
        || contains_forbidden_phase8_payload(&snapshot.last_upload_status)
        || contains_forbidden_phase8_payload(&snapshot.metadata_summary)
    {
        return Err(ProtocolError {
            code: "hosted_telemetry_diagnostics_invalid".to_string(),
            message: "hosted telemetry diagnostics must be metadata-only with valid event identity"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate a hosted telemetry spool record before metadata-only persistence.
pub fn validate_hosted_telemetry_spool_record(
    record: &HostedTelemetrySpoolRecord,
) -> ProtocolResult<()> {
    if record.record_id.trim().is_empty()
        || record.workspace_id.0 == 0
        || record.event_sequence.0 == 0
        || record.correlation_id.0 == 0
        || record.causality_id.0 == Uuid::nil()
        || record.schema_version == 0
        || !record
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
        || record.redaction_hints.contains(&RedactionHint::None)
        || matches!(
            record.classification,
            PrivacyClassification::Sensitive | PrivacyClassification::RawContent
        )
        || contains_forbidden_phase8_payload(&record.metadata_summary)
    {
        return Err(ProtocolError {
            code: "hosted_telemetry_spool_invalid".to_string(),
            message: "hosted telemetry spool records must be classified metadata-only".to_string(),
        });
    }
    Ok(())
}

/// Validate a raw-source retention capture request against explicit policy and consent metadata.
pub fn validate_raw_source_capture_request(
    policy: &RawSourceRetentionPolicy,
    grant: &RawSourceRetentionConsentGrant,
    request: &RawSourceCaptureRequest,
) -> ProtocolResult<()> {
    if !policy.capture_enabled
        || policy.schema_version == 0
        || grant.schema_version == 0
        || request.schema_version == 0
        || request.workspace_id.0 == 0
        || request.principal_id.0.trim().is_empty()
        || request.correlation_id.0 == 0
        || request.causality_id.0 == Uuid::nil()
        || request.max_bytes == 0
        || request.max_bytes > policy.max_bundle_bytes
        || request.paths.is_empty()
    {
        return Err(ProtocolError {
            code: "raw_source_retention_invalid".to_string(),
            message:
                "raw-source capture requires enabled policy, scoped grant, and bounded request"
                    .to_string(),
        });
    }
    if request.workspace_id != grant.workspace_id
        || request.principal_id != grant.principal_id
        || request.purpose != grant.purpose
        || !policy.allowed_purposes.contains(&request.purpose)
    {
        return Err(ProtocolError {
            code: "raw_source_retention_invalid".to_string(),
            message: "raw-source capture request is outside consent or policy scope".to_string(),
        });
    }
    for path in &request.paths {
        if !grant.path_scope.contains(path) {
            return Err(ProtocolError {
                code: "raw_source_retention_invalid".to_string(),
                message: "raw-source capture path is outside consent scope".to_string(),
            });
        }
    }
    Ok(())
}

/// Validate raw-source retention access audit metadata.
pub fn validate_raw_source_retention_access_audit(
    audit: &RawSourceRetentionAccessAudit,
) -> ProtocolResult<()> {
    if audit.bundle_id.trim().is_empty()
        || audit.principal_id.0.trim().is_empty()
        || audit.action.trim().is_empty()
        || audit.event_sequence.0 == 0
        || audit.correlation_id.0 == 0
        || audit.causality_id.0 == Uuid::nil()
        || audit.schema_version == 0
        || !audit.redaction_hints.contains(&RedactionHint::MetadataOnly)
        || audit.redaction_hints.contains(&RedactionHint::None)
        || contains_forbidden_phase8_payload(&audit.action)
    {
        return Err(ProtocolError {
            code: "raw_source_retention_audit_invalid".to_string(),
            message: "raw-source retention audit must be metadata-only with valid event identity"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate hosted retention export linkage requires separate raw-source consent.
pub fn validate_hosted_retention_export_linkage(
    linkage: &HostedRetentionExportLinkage,
) -> ProtocolResult<()> {
    if linkage.telemetry_batch_id.trim().is_empty()
        || linkage.bundle_id.trim().is_empty()
        || !linkage.raw_source_consent_verified
        || linkage.schema_version == 0
        || contains_forbidden_phase8_payload(&linkage.telemetry_batch_id)
        || contains_forbidden_phase8_payload(&linkage.bundle_id)
    {
        return Err(ProtocolError {
            code: "hosted_retention_export_linkage_invalid".to_string(),
            message: "hosted raw-source export linkage requires separate verified consent and metadata-only identifiers".to_string(),
        });
    }
    Ok(())
}

/// Validate raw-source key references never serialize key material.
pub fn validate_raw_source_key_reference(reference: &RawSourceKeyReference) -> ProtocolResult<()> {
    if reference.key_id.trim().is_empty()
        || reference.key_version.trim().is_empty()
        || reference.provider_label.trim().is_empty()
        || reference.schema_version == 0
        || contains_forbidden_phase8_payload(&reference.key_id)
        || contains_forbidden_phase8_payload(&reference.key_version)
        || contains_forbidden_phase8_payload(&reference.provider_label)
    {
        return Err(ProtocolError {
            code: "raw_source_key_reference_invalid".to_string(),
            message: "raw-source key references must be metadata-only and versioned".to_string(),
        });
    }
    Ok(())
}

/// Validate raw-source vault envelopes are AEAD metadata descriptors only.
pub fn validate_raw_source_vault_envelope(envelope: &RawSourceVaultEnvelope) -> ProtocolResult<()> {
    validate_raw_source_key_reference(&envelope.key_reference)?;
    if envelope.bundle_id.trim().is_empty()
        || envelope.workspace_id.0 == 0
        || envelope.nonce_digest.trim().is_empty()
        || envelope.ciphertext_digest.algorithm.trim().is_empty()
        || envelope.ciphertext_digest.value.trim().is_empty()
        || envelope.tag_digest.trim().is_empty()
        || envelope.aad_digest.trim().is_empty()
        || envelope.encrypted_byte_len == 0
        || envelope.schema_version == 0
        || contains_forbidden_phase8_payload(&envelope.bundle_id)
        || contains_forbidden_phase8_payload(&envelope.nonce_digest)
        || contains_forbidden_phase8_payload(&envelope.ciphertext_digest.value)
        || contains_forbidden_phase8_payload(&envelope.tag_digest)
        || contains_forbidden_phase8_payload(&envelope.aad_digest)
        || envelope.ciphertext_digest.algorithm == "devil-vault-stable-sum-v1"
    {
        return Err(ProtocolError {
            code: "raw_source_vault_envelope_invalid".to_string(),
            message: "raw-source vault envelopes require AEAD metadata, key reference, cryptographic digest, and no raw content".to_string(),
        });
    }
    Ok(())
}

/// Validate raw-source key rotation records are metadata-only.
pub fn validate_raw_source_key_rotation_record(
    record: &RawSourceKeyRotationRecord,
) -> ProtocolResult<()> {
    validate_raw_source_key_reference(&record.previous_key_reference)?;
    validate_raw_source_key_reference(&record.new_key_reference)?;
    if record.bundle_id.trim().is_empty()
        || record.previous_key_reference.key_id == record.new_key_reference.key_id
            && record.previous_key_reference.key_version == record.new_key_reference.key_version
        || record.event_sequence.0 == 0
        || record.correlation_id.0 == 0
        || record.causality_id.0 == Uuid::nil()
        || record.schema_version == 0
        || !phase8_metadata_only_redaction(&record.redaction_hints)
        || contains_forbidden_phase8_payload(&record.bundle_id)
        || contains_forbidden_phase8_payload(&record.metadata_summary)
    {
        return Err(ProtocolError {
            code: "raw_source_key_rotation_invalid".to_string(),
            message: "raw-source key rotation records require changed key references and metadata-only event identity".to_string(),
        });
    }
    Ok(())
}

/// Validate raw-source vault recovery reports are metadata-only.
pub fn validate_raw_source_vault_recovery_report(
    report: &RawSourceVaultRecoveryReport,
) -> ProtocolResult<()> {
    if report.recovery_id.trim().is_empty()
        || report.event_sequence.0 == 0
        || report.correlation_id.0 == 0
        || report.causality_id.0 == Uuid::nil()
        || report.schema_version == 0
        || !phase8_metadata_only_redaction(&report.redaction_hints)
        || contains_forbidden_phase8_payload(&report.recovery_id)
        || report
            .bundle_id
            .as_deref()
            .is_some_and(contains_forbidden_phase8_payload)
        || contains_forbidden_phase8_payload(&report.metadata_summary)
    {
        return Err(ProtocolError {
            code: "raw_source_vault_recovery_invalid".to_string(),
            message: "raw-source vault recovery reports must be fail-closed metadata-only evidence"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate separate hosted raw-source export consent.
pub fn validate_raw_source_hosted_export_consent(
    consent: &RawSourceHostedExportConsent,
) -> ProtocolResult<()> {
    if consent.grant_id.trim().is_empty()
        || consent.principal_id.0.trim().is_empty()
        || consent.workspace_id.0 == 0
        || consent.endpoint_id.trim().is_empty()
        || consent.issued_at.0 == 0
        || consent.expires_at.0 <= consent.issued_at.0
        || consent.revoked
        || consent.correlation_id.0 == 0
        || consent.schema_version == 0
        || contains_forbidden_phase8_payload(&consent.grant_id)
        || contains_forbidden_phase8_payload(&consent.endpoint_id)
    {
        return Err(ProtocolError {
            code: "raw_source_hosted_export_consent_invalid".to_string(),
            message: "hosted raw-source export requires current separate consent bound to endpoint and purpose".to_string(),
        });
    }
    Ok(())
}

/// Validate a Phase 8 storage schema manifest before migration planning.
pub fn validate_storage_schema_manifest(manifest: &StorageSchemaManifest) -> ProtocolResult<()> {
    if manifest.subsystem_id.trim().is_empty()
        || manifest.store_id.trim().is_empty()
        || manifest.active_schema_version == 0
        || manifest.min_supported_schema_version == 0
        || manifest.max_supported_schema_version == 0
        || manifest.schema_version == 0
        || manifest.active_schema_version < manifest.min_supported_schema_version
        || manifest.active_schema_version > manifest.max_supported_schema_version
        || !manifest
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
        || manifest.redaction_hints.contains(&RedactionHint::None)
        || contains_forbidden_phase8_payload(&manifest.metadata_summary)
    {
        return Err(ProtocolError {
            code: "storage_schema_manifest_invalid".to_string(),
            message:
                "storage schema manifests must be bounded metadata with compatible schema versions"
                    .to_string(),
        });
    }
    Ok(())
}

fn validate_storage_checksum(checksum: &StorageChecksum) -> ProtocolResult<()> {
    if checksum.algorithm.trim().is_empty()
        || checksum.value.trim().is_empty()
        || checksum.schema_version == 0
        || checksum.algorithm == "stable_storage_sum"
        || checksum.algorithm == "devil-vault-stable-sum-v1"
        || contains_forbidden_phase8_payload(&checksum.algorithm)
        || contains_forbidden_phase8_payload(&checksum.value)
    {
        return Err(ProtocolError {
            code: "storage_checksum_invalid".to_string(),
            message: "storage checksums must use cryptographic metadata descriptors".to_string(),
        });
    }
    Ok(())
}

/// Validate storage backup markers before mutation-capable apply.
pub fn validate_storage_backup_marker(marker: &StorageBackupMarker) -> ProtocolResult<()> {
    validate_storage_checksum(&marker.checksum)?;
    if marker.backup_id.trim().is_empty()
        || marker.subsystem_id.trim().is_empty()
        || marker.location_label.trim().is_empty()
        || marker.event_sequence.0 == 0
        || marker.correlation_id.0 == 0
        || marker.causality_id.0 == Uuid::nil()
        || marker.schema_version == 0
        || contains_forbidden_phase8_payload(&marker.backup_id)
        || contains_forbidden_phase8_payload(&marker.subsystem_id)
        || contains_forbidden_phase8_payload(&marker.location_label)
    {
        return Err(ProtocolError {
            code: "storage_backup_marker_invalid".to_string(),
            message: "storage backup markers require checksum, subsystem, location label, and event identity".to_string(),
        });
    }
    Ok(())
}

/// Validate storage recovery outcomes are metadata-only.
pub fn validate_storage_recovery_outcome(outcome: &StorageRecoveryOutcome) -> ProtocolResult<()> {
    if outcome.recovery_id.trim().is_empty()
        || outcome.subsystem_id.trim().is_empty()
        || (!outcome.recovered && !outcome.quarantined)
        || outcome.event_sequence.0 == 0
        || outcome.correlation_id.0 == 0
        || outcome.causality_id.0 == Uuid::nil()
        || outcome.schema_version == 0
        || !phase8_metadata_only_redaction(&outcome.redaction_hints)
        || contains_forbidden_phase8_payload(&outcome.recovery_id)
        || contains_forbidden_phase8_payload(&outcome.subsystem_id)
        || outcome
            .backup_id
            .as_deref()
            .is_some_and(contains_forbidden_phase8_payload)
        || contains_forbidden_phase8_payload(&outcome.metadata_summary)
    {
        return Err(ProtocolError {
            code: "storage_recovery_outcome_invalid".to_string(),
            message: "storage recovery outcomes must be metadata-only and fail-closed/quarantined when not recovered".to_string(),
        });
    }
    Ok(())
}

/// Validate storage subsystem health summaries are metadata-only.
pub fn validate_storage_subsystem_health_summary(
    summary: &StorageSubsystemHealthSummary,
) -> ProtocolResult<()> {
    if summary.subsystem_id.trim().is_empty()
        || summary.event_sequence.0 == 0
        || summary.correlation_id.0 == 0
        || summary.causality_id.0 == Uuid::nil()
        || summary.schema_version == 0
        || !phase8_metadata_only_redaction(&summary.redaction_hints)
        || contains_forbidden_phase8_payload(&summary.subsystem_id)
        || contains_forbidden_phase8_payload(&summary.metadata_summary)
    {
        return Err(ProtocolError {
            code: "storage_health_summary_invalid".to_string(),
            message: "storage health summaries must be metadata-only with valid event identity"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate storage evidence summaries are metadata-only.
pub fn validate_storage_evidence_summary(summary: &StorageEvidenceSummary) -> ProtocolResult<()> {
    if summary.artifact_id.trim().is_empty()
        || summary.command_label.trim().is_empty()
        || summary.schema_version == 0
        || !phase8_metadata_only_redaction(&summary.redaction_hints)
        || contains_forbidden_phase8_payload(&summary.artifact_id)
        || contains_forbidden_phase8_payload(&summary.command_label)
        || contains_forbidden_phase8_payload(&summary.metadata_summary)
    {
        return Err(ProtocolError {
            code: "storage_evidence_summary_invalid".to_string(),
            message: "storage evidence summaries must be metadata-only command evidence"
                .to_string(),
        });
    }
    Ok(())
}

/// Validate a Phase 8 storage migration dry-run report.
pub fn validate_storage_migration_dry_run_report(
    report: &StorageMigrationDryRunReport,
) -> ProtocolResult<()> {
    if report.step.migration_id.trim().is_empty()
        || report.step.subsystem_id.trim().is_empty()
        || report.step.from_schema_version == 0
        || report.step.to_schema_version == 0
        || report.step.to_schema_version <= report.step.from_schema_version
        || report.step.schema_version == 0
        || report.event_sequence.0 == 0
        || report.correlation_id.0 == 0
        || report.causality_id.0 == Uuid::nil()
        || report.schema_version == 0
        || !report
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
        || report.redaction_hints.contains(&RedactionHint::None)
        || contains_forbidden_phase8_payload(&report.metadata_summary)
    {
        return Err(ProtocolError {
            code: "storage_migration_dry_run_invalid".to_string(),
            message:
                "storage migration dry-runs must be forward-only metadata with valid event identity"
                    .to_string(),
        });
    }
    if report.step.destructive && !report.step.requires_backup {
        return Err(ProtocolError {
            code: "storage_migration_dry_run_invalid".to_string(),
            message: "destructive storage migrations require a backup marker".to_string(),
        });
    }
    Ok(())
}

/// Validate explicit storage migration apply requests.
pub fn validate_storage_migration_apply_request(
    request: &StorageMigrationApplyRequest,
) -> ProtocolResult<()> {
    validate_storage_migration_dry_run_report(&request.preflight_report)?;
    validate_storage_backup_marker(&request.backup_marker)?;
    if request.step != request.preflight_report.step
        || request.step.subsystem_id != request.backup_marker.subsystem_id
        || request.principal_id.0.trim().is_empty()
        || request.capability_decision.decision_id.0 == 0
        || request.capability_decision.capability.0 != "storage.migration.apply"
        || !request.capability_decision.granted
        || request.journal_id.trim().is_empty()
        || !request.explicit_apply_flag
        || request.event_sequence.0 == 0
        || request.correlation_id.0 == 0
        || request.causality_id.0 == Uuid::nil()
        || request.schema_version == 0
        || !phase8_metadata_only_redaction(&request.redaction_hints)
        || contains_forbidden_phase8_payload(&request.journal_id)
        || contains_forbidden_phase8_payload(&request.metadata_summary)
    {
        return Err(ProtocolError {
            code: "storage_migration_apply_request_invalid".to_string(),
            message: "storage migration apply requires matching preflight, backup, journal, granted capability, and explicit operator intent".to_string(),
        });
    }
    Ok(())
}

/// Validate storage migration apply outcomes are metadata-only.
pub fn validate_storage_migration_apply_outcome(
    outcome: &StorageMigrationApplyOutcome,
) -> ProtocolResult<()> {
    validate_storage_checksum(&outcome.checksum)?;
    if outcome.migration_id.trim().is_empty()
        || outcome.subsystem_id.trim().is_empty()
        || outcome.backup_id.trim().is_empty()
        || outcome.journal_id.trim().is_empty()
        || (!outcome.applied && outcome.recovery_id.is_none())
        || outcome.event_sequence.0 == 0
        || outcome.correlation_id.0 == 0
        || outcome.causality_id.0 == Uuid::nil()
        || outcome.schema_version == 0
        || !phase8_metadata_only_redaction(&outcome.redaction_hints)
        || contains_forbidden_phase8_payload(&outcome.migration_id)
        || contains_forbidden_phase8_payload(&outcome.subsystem_id)
        || contains_forbidden_phase8_payload(&outcome.backup_id)
        || contains_forbidden_phase8_payload(&outcome.journal_id)
        || outcome
            .recovery_id
            .as_deref()
            .is_some_and(contains_forbidden_phase8_payload)
        || contains_forbidden_phase8_payload(&outcome.metadata_summary)
    {
        return Err(ProtocolError {
            code: "storage_migration_apply_outcome_invalid".to_string(),
            message: "storage migration apply outcomes must be metadata-only and include recovery reference when apply fails".to_string(),
        });
    }
    Ok(())
}

/// Validate an explicit Phase 8 storage repair request.
pub fn validate_storage_repair_request(request: &StorageRepairRequest) -> ProtocolResult<()> {
    if request.subsystem_id.trim().is_empty()
        || request.principal_id.0.trim().is_empty()
        || request.capability_decision.decision_id.0 == 0
        || request.capability_decision.capability.0 != "storage.migration.repair"
        || !request.capability_decision.granted
        || !request.explicit_repair_flag
        || request.event_sequence.0 == 0
        || request.correlation_id.0 == 0
        || request.causality_id.0 == Uuid::nil()
        || request.schema_version == 0
        || contains_forbidden_phase8_payload(&request.metadata_summary)
    {
        return Err(ProtocolError {
            code: "storage_repair_request_invalid".to_string(),
            message: "storage repair requires explicit operator intent, granted capability, and metadata-only identity".to_string(),
        });
    }
    Ok(())
}

/// Validate a Phase 8 metadata-only replay manifest.
pub fn validate_storage_replay_manifest(manifest: &StorageReplayManifest) -> ProtocolResult<()> {
    if manifest.replay_id.trim().is_empty()
        || manifest.subsystem_id.trim().is_empty()
        || manifest.event_count == 0
        || manifest.first_event_sequence.0 == 0
        || manifest.last_event_sequence.0 < manifest.first_event_sequence.0
        || manifest.schema_version == 0
        || !manifest
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly)
        || manifest.redaction_hints.contains(&RedactionHint::None)
        || contains_forbidden_phase8_payload(&manifest.metadata_summary)
    {
        return Err(ProtocolError {
            code: "storage_replay_manifest_invalid".to_string(),
            message: "storage replay manifests must be ordered metadata-only evidence".to_string(),
        });
    }
    Ok(())
}

fn contains_forbidden_collaboration_payload(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "source_text",
        "source_body",
        "raw_source",
        "raw_transcript",
        "full_snapshot",
        "secret",
        "api_key",
        "unbounded_payload",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn phase8_metadata_only_redaction(redaction_hints: &[RedactionHint]) -> bool {
    redaction_hints.contains(&RedactionHint::MetadataOnly)
        && !redaction_hints.contains(&RedactionHint::None)
}

fn contains_forbidden_phase8_payload(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "-----begin",
        "-----end",
        "private_key",
        "private key",
        "key_bytes",
        "raw_key",
        "raw_cert",
        "pem_body",
        "source_text",
        "source_body",
        "raw_source",
        "raw_transcript",
        "terminal_output",
        "process_output",
        "transport_payload",
        "raw_prompt",
        "provider_response",
        "full_snapshot",
        "secret",
        "token",
        "password",
        "api_key",
        "unbounded_payload",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn contains_forbidden_remote_payload(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "source_text",
        "source_body",
        "raw_source",
        "raw_transcript",
        "terminal_output",
        "process_output",
        "transport_payload",
        "full_snapshot",
        "secret",
        "token",
        "password",
        "api_key",
        "unbounded_payload",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn contains_forbidden_plugin_payload(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "source_body",
        "fn main",
        "raw_source",
        "raw_prompt",
        "provider_response",
        "secret",
        "api_key",
        "unbounded_output",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
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

/// Service-port for semantic fabric interactions.
pub trait SemanticPort {
    /// Handle semantic fabric request.
    fn handle(&self, request: SemanticRequest) -> ProtocolResult<SemanticResponse>;
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

/// Service-port for plugin runtime interactions.
pub trait PluginPort {
    /// Handle a plugin request.
    fn handle(&self, request: PluginRequest) -> ProtocolResult<PluginResponse>;
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
            schema_version: 1,
            min_abi_version: 1,
            max_abi_version: 1,
            module_hash: "sha256:module".to_string(),
            manifest_id: "manifest-1".to_string(),
            trust: PluginTrustMetadata {
                source: PluginTrustSource::ExplicitLocalAllow,
                decision: PluginTrustDecision::ExplicitlyAllowed,
                reason: "test allow".to_string(),
            },
            signature: None,
            activation_events: vec![PluginActivationEvent::OnCommand {
                command: "plugin.command".to_string(),
            }],
            contributions: vec![PluginContribution::Command(PluginCommandDescriptor {
                command_id: "plugin.command".to_string(),
                title: "Plugin Command".to_string(),
                required_capability: CapabilityId("plugin.command".to_string()),
            })],
            requested_capabilities: vec![CapabilityId("plugin.command".to_string())],
            storage_namespace: PluginStateNamespace {
                plugin_id: PluginId(3),
                namespace: "state".to_string(),
            },
            quotas: PluginQuotaDeclaration {
                max_fuel: 1000,
                max_wall_time_ms: 100,
                max_memory_pages: 16,
                max_storage_bytes: 4096,
                max_host_calls: 16,
                max_events: 8,
                max_output_bytes: 1024,
            },
        };

        let as_json = serde_json::to_string(&manifest).unwrap();
        let back: PluginManifest = serde_json::from_str(&as_json).unwrap();
        assert_eq!(back.plugin_id, manifest.plugin_id);

        let invalid = r#"{"plugin_id":1, "name":"x", "version":"0.1.0"}"#;
        assert!(serde_json::from_str::<PluginManifest>(invalid).is_err());
        assert!(validate_plugin_manifest(&manifest, 1).is_ok());
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
        struct MockSemanticPort;
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
        impl SemanticPort for MockSemanticPort {
            fn handle(&self, _request: SemanticRequest) -> ProtocolResult<SemanticResponse> {
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

        struct AllPorts<W, E, P, T, L, SEM, C, ES, S> {
            w: W,
            e: E,
            p: P,
            t: T,
            l: L,
            sem: SEM,
            c: C,
            es: ES,
            s: S,
        }

        fn use_all_ports<W, E, P, T, L, SEM, C, ES, S>(
            ports: AllPorts<W, E, P, T, L, SEM, C, ES, S>,
        ) where
            W: WorkspacePort,
            E: EditorPort,
            P: ProposalPort,
            T: TerminalPort,
            L: LspPort,
            SEM: SemanticPort,
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
                sem,
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
                sem.handle(SemanticRequest::Cancel(SemanticCancellationToken {
                    token_id: CancellationTokenId(Uuid::now_v7()),
                    workspace_id: WorkspaceId(1),
                    file_id: None,
                    snapshot_id: None,
                    content_hash: None,
                    workspace_generation: Some(WorkspaceGeneration(1)),
                    privacy_scope: SemanticPrivacyScope::Workspace,
                    reason: Some(SemanticCancellationReason::UserCancelled),
                    issued_at: TimestampMillis(1),
                    expires_at: None,
                    schema_version: 1,
                })),
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
            sem: MockSemanticPort,
            c: MockCapabilityBrokerPort,
            es: MockEventSinkPort,
            s: MockStorageRepositoryPort,
        });
    }
}
