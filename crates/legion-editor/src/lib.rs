//! Editor core with multi-buffer transactions, undo/redo grouping, and save-request DTO emission.

#![warn(missing_docs)]

pub mod diff;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use legion_observability::{NoopEventSink, transaction_event};
use legion_protocol::{
    BufferId, BufferOpened, BufferVersion, ByteRange, CanonicalPath, CausalityId, ChangedTextRange,
    CompletionItem, CompletionRequest, CorrelationId, EditorApplyTransactionRequest,
    EditorBufferMetadata, EditorOpenBufferRequest, EditorPort, EditorRequest, EditorResponse,
    EditorSaveAcknowledgement, EditorSaveOutcome, EditorSaveRequest, EditorViewportRequest,
    EventSequence, EventSinkPort, EventSinkRequest, FileConflictLifecycleState, FileConflictState,
    FileFingerprint, FileId, LargeFileStatus, LineIndexRange, LspCompletionResponse,
    ProtocolDiagnostic, ProtocolError, ProtocolResult, ProtocolTextRange, SnapshotChunkDescriptor,
    SnapshotConsumerKind, SnapshotId, SnapshotLeaseChunk, SnapshotLeaseDescriptor, TextCoordinate,
    TextOffset, TextTransactionDescriptor, TimestampMillis, TransactionSource,
    Utf16Position as ProtocolUtf16Position, Utf16Range as ProtocolUtf16Range,
    ViewportDecorationSpan, ViewportFoldRange, ViewportLineMetric, ViewportLineSlice,
    ViewportLineTruncationState, ViewportProjection, ViewportProjectionMode,
    ViewportSemanticTokenOverlay, WorkspaceId,
};
use legion_text::{
    DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES, RetentionPinReason, TextBuffer, TextError,
    TextSnapshotDescriptor, Utf16Position, Utf16Range,
};
use regex::RegexBuilder;
use thiserror::Error;
use uuid::Uuid;

pub use legion_protocol::CaretAffinity;
pub use legion_text::{TextEdit, TextPosition, TextRange};

/// Multiple cursors: creating them, and editing at all of them at once.
pub mod multi_cursor;

/// Editor operation errors.
#[derive(Debug, Error)]
pub enum EditorError {
    /// Buffer not found.
    #[error("buffer {0:?} does not exist")]
    BufferNotFound(BufferId),
    /// Open buffer for a workspace file was not found.
    #[error("workspace {workspace_id:?} file {file_id:?} is not open in the editor")]
    CompletionBufferNotFound {
        /// Workspace identifier.
        workspace_id: WorkspaceId,
        /// File identifier.
        file_id: FileId,
    },
    /// Completion request targeted an older snapshot.
    #[error("completion snapshot {requested:?} is stale; current snapshot is {current:?}")]
    StaleCompletionSnapshot {
        /// Requested snapshot identifier.
        requested: SnapshotId,
        /// Current snapshot identifier.
        current: SnapshotId,
    },
    /// Completion request used an offset that could not be resolved safely.
    #[error("invalid completion position: {0}")]
    InvalidCompletionPosition(&'static str),
    /// Visual caret placement targeted an older snapshot or buffer version.
    #[error(
        "visual caret placement is stale: expected snapshot {expected_snapshot_id:?}/version {expected_buffer_version:?}, current snapshot {actual_snapshot_id:?}/version {actual_buffer_version:?}"
    )]
    StaleVisualCaretPlacement {
        /// Snapshot identifier supplied by the visual input producer.
        expected_snapshot_id: SnapshotId,
        /// Current snapshot identifier owned by the editor.
        actual_snapshot_id: SnapshotId,
        /// Buffer version supplied by the visual input producer.
        expected_buffer_version: BufferVersion,
        /// Current buffer version owned by the editor.
        actual_buffer_version: BufferVersion,
    },
    /// File is already open in another buffer.
    #[error("file {0:?} is already open")]
    FileAlreadyOpen(FileId),
    /// Invalid edit request.
    #[error("invalid edit: {0}")]
    InvalidEdit(&'static str),
    /// Text model error.
    #[error(transparent)]
    Text(#[from] TextError),
    /// Undo stack empty.
    #[error("nothing to undo")]
    NothingToUndo,
    /// Redo stack empty.
    #[error("nothing to redo")]
    NothingToRedo,
    /// Save requires a full-source payload that the current editor policy declined to assemble.
    #[error(
        "buffer {0:?} is in degraded mode; save requests fail closed when chunked save assembly is disabled"
    )]
    DegradedSaveUnavailable(BufferId),
    /// Snapshot lease was not found.
    #[error("snapshot lease {0} does not exist")]
    SnapshotLeaseNotFound(Uuid),
    /// Snapshot lease was revoked while a worker still held a clone.
    #[error("snapshot lease {0} was revoked")]
    SnapshotLeaseRevoked(Uuid),
    /// Snapshot lease has expired and consumers must resynchronize.
    #[error("snapshot lease {lease_id} expired at {expired_at:?} before {now:?}; resynchronize")]
    SnapshotLeaseExpired {
        /// Lease identifier.
        lease_id: Uuid,
        /// Lease expiry timestamp.
        expired_at: TimestampMillis,
        /// Read attempt timestamp.
        now: TimestampMillis,
    },
    /// Snapshot lease identity does not match the consumer expectation.
    #[error(
        "snapshot lease {lease_id} is stale for the requested buffer/snapshot/version; resynchronize"
    )]
    SnapshotLeaseStale {
        /// Lease identifier.
        lease_id: Uuid,
        /// Expected buffer identifier.
        expected_buffer_id: BufferId,
        /// Actual leased buffer identifier.
        actual_buffer_id: BufferId,
        /// Expected snapshot identifier.
        expected_snapshot_id: SnapshotId,
        /// Actual leased snapshot identifier.
        actual_snapshot_id: SnapshotId,
        /// Expected buffer version.
        expected_buffer_version: BufferVersion,
        /// Actual leased buffer version.
        actual_buffer_version: BufferVersion,
    },
    /// File was detected as binary and cannot be opened as a text buffer.
    #[error("file {path:?} detected as binary (NUL byte at offset {nul_offset}); preview refused")]
    BinaryFileRefused {
        /// Path of the binary file.
        path: String,
        /// Offset of the first NUL byte found.
        nul_offset: usize,
    },
}

/// Cursor state for a single caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Cursor position.
    pub position: TextPosition,
}

/// Semantic document boundary used by native editor navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryKind {
    /// Start of the current logical line.
    LineStart,
    /// End of the current logical line, excluding its line ending.
    LineEnd,
    /// Start of the document.
    DocumentStart,
    /// End of the document, at the end of the final logical line.
    DocumentEnd,
}

/// Direction for native deletion from collapsed directed carets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteDirection {
    /// Delete the grapheme immediately before each collapsed caret.
    Backward,
    /// Delete the grapheme immediately after each collapsed caret.
    Forward,
}

/// A finite, non-negative row-local rendered X coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreferredX(u32);

impl PreferredX {
    /// Validate and construct a preferred rendered X from rendering points.
    pub fn new(value: f32) -> Result<Self, EditorError> {
        if value.is_finite() && value >= 0.0 {
            Ok(Self(value.to_bits()))
        } else {
            Err(EditorError::InvalidEdit(
                "preferred X must be finite and non-negative",
            ))
        }
    }

    /// Return the rendering-point value.
    pub fn get(self) -> f32 {
        f32::from_bits(self.0)
    }
}

/// Opaque identity for the shaped layout facts used by vertical movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VerticalLayoutId(u128);

impl VerticalLayoutId {
    /// Construct a nonzero layout identity.
    pub const fn new(value: u128) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    /// Return the opaque identity value.
    pub const fn get(self) -> u128 {
        self.0
    }
}

/// Direction for semantic visual-row movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalDirection {
    /// Move to the preceding visual row.
    Up,
    /// Move to the following visual row.
    Down,
}

/// One renderer-shaped caret stop on the adjacent visual row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerticalCaretStop {
    /// Valid UTF-8 source position represented by the stop.
    pub position: TextPosition,
    /// Row-local rendered X in the request's layout coordinate space.
    pub x: PreferredX,
    /// Wrap-side affinity at this stop.
    pub affinity: CaretAffinity,
}

/// Renderer-shaped visual row and its bounded caret stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapedVisualRow {
    /// Logical text line containing this visual row.
    pub logical_line: u32,
    /// Zero-based visual-row ordinal within the logical line.
    pub row_index: Option<u32>,
    /// Number of visual rows in the logical line.
    pub row_count: Option<u32>,
    /// Inclusive row start position.
    pub start: TextPosition,
    /// Inclusive row end position.
    pub end: TextPosition,
    /// Ordered valid caret stops on the target row.
    pub stops: Vec<VerticalCaretStop>,
}

/// Renderer-shaped source row facts for one ordered source caret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerticalSourceRow {
    /// Source visual row containing the source caret.
    pub row: ShapedVisualRow,
    /// Source row-local rendered X measured by the renderer's galley.
    pub source_x: PreferredX,
}

/// Atomic request for moving every directed caret across one visual row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerticalMovementRequest {
    /// Snapshot expected by the renderer that shaped the facts.
    pub expected_snapshot_id: SnapshotId,
    /// Buffer version expected by the renderer that shaped the facts.
    pub expected_buffer_version: BufferVersion,
    /// Exact ordered source caret vector used to shape the facts.
    pub expected_carets: Vec<DirectedCaret>,
    /// Opaque identity for the renderer layout facts.
    pub layout_id: VerticalLayoutId,
    /// Requested visual-row direction.
    pub direction: VerticalDirection,
    /// Whether to preserve or initialize directed anchors.
    pub extend: bool,
    /// Bounded source-row facts for each ordered source caret.
    pub source_rows: Vec<VerticalSourceRow>,
    /// Bounded adjacent target-row facts for each source caret.
    pub target_rows: Vec<ShapedVisualRow>,
}

/// Direction for semantic horizontal caret movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalDirection {
    /// Move to the strictly previous extended grapheme boundary.
    Left,
    /// Move to the strictly next extended grapheme boundary.
    Right,
}

/// A caret with a UTF-8 head and an optional directed anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectedCaret {
    /// Current caret head.
    pub head: TextPosition,
    /// Optional anchor retained while extending a selection.
    pub anchor: Option<TextPosition>,
    /// Visual row affinity at a wrapped-row boundary.
    pub affinity: CaretAffinity,
    /// Editor-owned preferred row-local rendered X for vertical movement.
    pub preferred_x: Option<PreferredX>,
}

impl DirectedCaret {
    /// Construct a directed caret.
    pub const fn new(head: TextPosition, anchor: Option<TextPosition>) -> Self {
        Self {
            head,
            anchor,
            affinity: CaretAffinity::Upstream,
            preferred_x: None,
        }
    }

    /// Return this caret with an explicitly selected visual row affinity.
    pub const fn with_affinity(mut self, affinity: CaretAffinity) -> Self {
        self.affinity = affinity;
        self
    }

    /// Return this caret with an editor-owned preferred rendered X.
    pub const fn with_preferred_x(mut self, preferred_x: PreferredX) -> Self {
        self.preferred_x = Some(preferred_x);
        self
    }
}

/// Selection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Selection range.
    pub range: TextRange,
}

/// Transient overlay owned by the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiOverlay {
    /// Overlay id.
    pub overlay_id: Uuid,
    /// Human-readable category.
    pub kind: String,
    /// Text range covered by overlay.
    pub range: TextRange,
    /// Optional payload.
    pub payload: Option<String>,
}

/// Change delta with both byte and UTF-16 range projections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedDelta {
    /// Changed byte range in post-edit coordinates.
    pub byte_range: ByteRange,
    /// Changed UTF-16 range in post-edit coordinates.
    pub utf16_range: Utf16Range,
}

/// Local transaction record with deterministic metadata.
#[derive(Debug, Clone)]
pub struct TransactionRecord {
    /// Transaction id.
    pub transaction_id: Uuid,
    /// Causality trace id for distributed debugging.
    pub causality_trace_id: Uuid,
    /// Workspace id.
    pub workspace_id: WorkspaceId,
    /// Buffer id.
    pub buffer_id: BufferId,
    /// File id.
    pub file_id: FileId,
    /// Source of mutation.
    pub source: TransactionSource,
    /// Pre-change snapshot descriptor.
    pub pre_snapshot: TextSnapshotDescriptor,
    /// Post-change snapshot descriptor.
    pub post_snapshot: TextSnapshotDescriptor,
    /// Changed ranges in byte + UTF-16 coordinates.
    pub deltas: Vec<ChangedDelta>,
    /// Optional undo group identifier.
    pub undo_group_id: Option<Uuid>,
    /// High-resolution timestamp (ms in current protocol contract).
    pub occurred_at: TimestampMillis,
    /// Optional correlation id from caller context.
    pub correlation_id: Option<CorrelationId>,
}

impl TransactionRecord {
    /// Convert local transaction record into the protocol descriptor.
    pub fn to_protocol_descriptor(&self) -> TextTransactionDescriptor {
        let correlation_id = self.correlation_id.unwrap_or(CorrelationId(1));
        TextTransactionDescriptor {
            workspace_id: self.workspace_id,
            buffer_id: self.buffer_id,
            file_id: self.file_id,
            transaction_id: self.transaction_id,
            correlation_id,
            source: self.source.clone(),
            pre_snapshot_id: self.pre_snapshot.snapshot_id,
            post_snapshot_id: self.post_snapshot.snapshot_id,
            pre_buffer_version: self.pre_snapshot.buffer_version,
            post_buffer_version: self.post_snapshot.buffer_version,
            changed_ranges: self
                .deltas
                .iter()
                .map(|delta| ChangedTextRange {
                    byte_range: delta.byte_range,
                    utf16_range: ProtocolUtf16Range {
                        start: ProtocolUtf16Position {
                            line: delta.utf16_range.start.line as u32,
                            character: delta.utf16_range.start.character as u32,
                        },
                        end: ProtocolUtf16Position {
                            line: delta.utf16_range.end.line as u32,
                            character: delta.utf16_range.end.character as u32,
                        },
                    },
                })
                .collect(),
            causality_id: CausalityId(self.causality_trace_id),
            parent_transaction_id: None,
            schema_version: 1,
            undo_group_id: self.undo_group_id,
            occurred_at: self.occurred_at,
        }
    }
}

/// Save-request DTO for decoupled persistence.
#[derive(Debug, Clone)]
pub struct SaveRequestDto {
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
    /// UTF-8 text payload to persist asynchronously through workspace/proposal ports.
    pub text: String,
    /// Emission timestamp.
    pub requested_at: TimestampMillis,
    /// Caller or generated correlation id.
    pub correlation_id: CorrelationId,
}

/// Typed editor acknowledgement for a pending save request.
#[derive(Debug, Clone)]
pub enum SaveAcknowledgement {
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

#[derive(Debug, Clone)]
struct UndoEntry {
    snapshot: legion_text::TextSnapshot,
    carets: Vec<DirectedCaret>,
    vertical_layout_id: Option<VerticalLayoutId>,
    undo_group_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
struct PreparedBatchEdit {
    start: usize,
    end: usize,
    new_text: String,
}

#[derive(Debug, Clone)]
struct BatchEditPlan {
    pre_snapshot: legion_text::TextSnapshot,
    pre_descriptor: TextSnapshotDescriptor,
    pre_version: BufferVersion,
    pre_carets: Vec<DirectedCaret>,
    edits: Vec<PreparedBatchEdit>,
}

fn carets_match_vertical_source(actual: &[DirectedCaret], expected: &[DirectedCaret]) -> bool {
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(left, right)| {
            left.head == right.head
                && left.anchor == right.anchor
                && left.affinity == right.affinity
        })
}

/// Map one caret/anchor offset through a batch that is ordered **descending**
/// by start. Later (lower) insertions still have to shift already-mapped
/// higher carets, so a zero-width insert must advance `offset > start`.
fn map_edit_offset(mut offset: usize, head_affinity: bool, edits: &[PreparedBatchEdit]) -> usize {
    for edit in edits {
        if offset < edit.start {
            continue;
        }
        if edit.start == edit.end {
            if offset == edit.start {
                if head_affinity {
                    offset += edit.new_text.len();
                }
            } else if offset > edit.start {
                offset += edit.new_text.len();
            }
            continue;
        }
        if offset < edit.end {
            offset = if head_affinity {
                edit.start + edit.new_text.len()
            } else {
                edit.start
            };
        } else {
            // `offset >= edit.end`, so subtracting the removed span cannot
            // underflow. Avoid i64 casts that wrap on a negative delta.
            let removed = edit.end - edit.start;
            offset = offset
                .saturating_sub(removed)
                .saturating_add(edit.new_text.len());
        }
    }
    offset
}

#[derive(Debug, Clone)]
struct SaveSnapshotPayload {
    snapshot: legion_text::TextSnapshot,
    dto: SaveRequestDto,
}

#[derive(Debug, Clone)]
struct SnapshotLeaseRecord {
    snapshot: legion_text::TextSnapshot,
    descriptor: SnapshotLeaseDescriptor,
    owned_state: Option<Arc<OwnedSnapshotCell>>,
}

/// Maximum UTF-8 bytes returned by one worker-owned snapshot line read.
pub const MAX_OWNED_SNAPSHOT_LINE_CHUNK_BYTES: usize = 96 * 1024;

/// An immutable, revocable snapshot lease that can cross to a background worker.
///
/// The snapshot is intentionally private: callers can request only bounded line chunks. All
/// clones share revocation state, so releasing the originating editor lease invalidates every
/// worker clone without retaining an unbounded text representation.
#[derive(Debug, Clone)]
pub struct OwnedSnapshotLease {
    descriptor: SnapshotLeaseDescriptor,
    cell: Arc<OwnedSnapshotCell>,
}

#[derive(Debug)]
struct OwnedSnapshotCell {
    snapshot: Mutex<Option<legion_text::TextSnapshot>>,
    revoked: AtomicBool,
}

impl OwnedSnapshotCell {
    fn new(snapshot: legion_text::TextSnapshot) -> Arc<Self> {
        Arc::new(Self {
            snapshot: Mutex::new(Some(snapshot)),
            revoked: AtomicBool::new(false),
        })
    }

    fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
        if let Ok(mut snapshot) = self.snapshot.try_lock() {
            *snapshot = None;
        }
    }

    fn is_revoked(&self) -> bool {
        self.revoked.load(Ordering::Acquire)
    }
}

impl OwnedSnapshotLease {
    /// Return the descriptor bound to this immutable snapshot.
    pub fn descriptor(&self) -> &SnapshotLeaseDescriptor {
        &self.descriptor
    }

    /// Read one bounded logical-line chunk without borrowing the editor or app owner.
    pub fn read_line_chunk(
        &self,
        line: usize,
        start_byte: usize,
        max_bytes: usize,
    ) -> Result<SnapshotLeaseLineChunk, EditorError> {
        let now = TimestampMillis::now();
        if self.cell.is_revoked() {
            return Err(EditorError::SnapshotLeaseRevoked(self.descriptor.lease_id));
        }
        if now.0 > self.descriptor.expires_at.0 {
            self.cell.revoke();
            return Err(EditorError::SnapshotLeaseExpired {
                lease_id: self.descriptor.lease_id,
                expired_at: self.descriptor.expires_at,
                now,
            });
        }
        if max_bytes == 0 || max_bytes > MAX_OWNED_SNAPSHOT_LINE_CHUNK_BYTES {
            return Err(EditorError::Text(TextError::InvalidWindowBudget {
                requested: max_bytes,
                maximum: MAX_OWNED_SNAPSHOT_LINE_CHUNK_BYTES,
            }));
        }
        // Hold the cell only for the bounded read. Revoke is a non-blocking
        // try_lock so EditorEngine drop never waits on a worker.
        let mut guard = self
            .cell
            .snapshot
            .lock()
            .map_err(|_| EditorError::InvalidEdit("snapshot lease state lock poisoned"))?;
        if self.cell.is_revoked() {
            *guard = None;
            return Err(EditorError::SnapshotLeaseRevoked(self.descriptor.lease_id));
        }
        let snapshot = guard
            .as_ref()
            .ok_or(EditorError::SnapshotLeaseNotFound(self.descriptor.lease_id))?;
        let line = snapshot.line_chunk_from_byte(line, start_byte, max_bytes)?;
        if self.cell.is_revoked() {
            *guard = None;
            return Err(EditorError::SnapshotLeaseRevoked(self.descriptor.lease_id));
        }
        drop(guard);
        Ok(SnapshotLeaseLineChunk {
            lease: self.descriptor.clone(),
            line,
            schema_version: 1,
        })
    }
}

/// Bounded logical-line data read through a validated snapshot lease.
///
/// The lease descriptor binds the chunk to one immutable snapshot and buffer
/// version. `line` is a bounded source range and may be iterated by passing
/// its `end_byte` back to [`EditorEngine::read_snapshot_lease_line_chunk`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotLeaseLineChunk {
    /// Descriptor for the lease that authorized this read.
    pub lease: SnapshotLeaseDescriptor,
    /// Bounded logical-line chunk.
    pub line: legion_text::TextLineChunk,
    /// Read DTO schema version.
    pub schema_version: u16,
}

/// Drained metadata-only transaction events captured by the editor.
#[derive(Debug, Clone, Default)]
pub struct DrainedTransactionEvents {
    /// Transaction descriptors produced since the previous drain.
    pub descriptors: Vec<TextTransactionDescriptor>,
    /// Number of older descriptors dropped before this drain because the queue was full.
    pub dropped_before_drain: u64,
}

#[derive(Debug, Clone)]
struct RetainedSnapshotDescriptor {
    buffer_id: BufferId,
    reason: RetentionPinReason,
    descriptor: TextSnapshotDescriptor,
}

#[derive(Debug, Clone)]
struct EditorBufferState {
    workspace_id: WorkspaceId,
    buffer_id: BufferId,
    file_id: FileId,
    file_path: String,
    buffer: TextBuffer,
    mode: BufferMode,
    dirty: bool,
    carets: Vec<DirectedCaret>,
    vertical_layout_id: Option<VerticalLayoutId>,
    overlays: Vec<UiOverlay>,
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    active_undo_group: Option<Uuid>,
    active_group_evicted: bool,
    current_snapshot: legion_text::TextSnapshot,
    /// Whether the text was streamed from disk rather than handed over whole.
    ///
    /// Not derivable after the fact: a streamed buffer and one built from a
    /// `String` of the same size look identical once loaded, and only the
    /// former never had the whole file in memory.
    streamed: bool,
    save_state: FileConflictLifecycleState,
    save_diagnostics: Vec<ProtocolDiagnostic>,
    conflict_state: Option<FileConflictState>,
}

impl EditorBufferState {
    fn build(
        workspace_id: WorkspaceId,
        buffer_id: BufferId,
        file_id: FileId,
        file_path: impl Into<String>,
        mut buffer: TextBuffer,
        mode: BufferMode,
        streamed: bool,
    ) -> Result<Self, EditorError> {
        buffer.set_version(BufferVersion(0));
        let current_snapshot =
            buffer.try_snapshot_with_retention(RetentionPinReason::CurrentBuffer)?;

        Ok(Self {
            workspace_id,
            buffer_id,
            file_id,
            file_path: file_path.into(),
            buffer,
            mode,
            streamed,
            dirty: false,
            carets: vec![DirectedCaret::new(TextPosition::zero(), None)],
            vertical_layout_id: None,
            overlays: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            active_undo_group: None,
            active_group_evicted: false,
            current_snapshot,
            save_state: FileConflictLifecycleState::Clean,
            save_diagnostics: Vec::new(),
            conflict_state: None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotStackKind {
    Undo,
    Redo,
}

/// Production multi-buffer editor engine.
#[derive(Debug)]
pub struct EditorEngine {
    next_buffer_id: u128,
    buffers: HashMap<BufferId, EditorBufferState>,
    file_to_buffer: HashMap<(WorkspaceId, FileId), BufferId>,
    transaction_log: Vec<TransactionRecord>,
    transaction_events: VecDeque<TextTransactionDescriptor>,
    transaction_event_queue_capacity: usize,
    dropped_transaction_event_count: u64,
    pending_save_requests: Vec<SaveRequestDto>,
    snapshot_leases: HashMap<Uuid, SnapshotLeaseRecord>,
    pinned_snapshot_ids: HashSet<SnapshotId>,
    thresholds: EditorThresholds,
    snapshot_retention_policy: SnapshotRetentionPolicy,
    retained_snapshots: VecDeque<RetainedSnapshotDescriptor>,
}

impl Drop for EditorEngine {
    fn drop(&mut self) {
        // Worker-owned leases must not keep immutable source readable after the authoritative
        // editor owner disappears. Clear the shared cells before the editor's snapshot tables
        // are dropped; clones then fail closed without retaining the snapshot until expiry.
        for lease in self.snapshot_leases.values() {
            if let Some(state) = &lease.owned_state {
                state.revoke();
            }
        }
    }
}

struct EditorEventContext<'a> {
    event_sink: &'a dyn EventSinkPort,
    next_sequence: &'a mut u64,
}

impl<'a> EditorEventContext<'a> {
    fn sequence(&mut self) -> EventSequence {
        *self.next_sequence = self.next_sequence.saturating_add(1).max(1);
        EventSequence(*self.next_sequence)
    }

    fn emit_transaction(
        &mut self,
        record: &TransactionRecord,
        applied: bool,
        reason: Option<&str>,
    ) {
        // Building the envelope only fails when the transaction's core ids are
        // invalid; drop the event in that case rather than aborting the edit.
        if let Ok(envelope) = transaction_event(
            &record.to_protocol_descriptor(),
            applied,
            reason,
            self.sequence(),
        ) {
            let _ = self.event_sink.emit(EventSinkRequest { envelope });
        }
    }
}

/// Mutex-backed adapter exposing [`EditorEngine`] through the protocol [`EditorPort`].
pub struct EditorEnginePort {
    engine: Mutex<EditorEngine>,
    event_sink: Box<dyn EventSinkPort + Send + Sync>,
    next_event_sequence: Mutex<u64>,
}

impl EditorEnginePort {
    /// Construct a new editor port adapter from an editor engine.
    pub fn new(engine: EditorEngine) -> Self {
        Self::with_event_sink(engine, Box::new(NoopEventSink))
    }

    /// Construct a new editor port adapter from an editor engine and event sink.
    pub fn with_event_sink(
        engine: EditorEngine,
        event_sink: Box<dyn EventSinkPort + Send + Sync>,
    ) -> Self {
        Self {
            engine: Mutex::new(engine),
            event_sink,
            next_event_sequence: Mutex::new(0),
        }
    }

    /// Consume the adapter and return the wrapped editor engine.
    pub fn into_inner(self) -> Result<EditorEngine, EditorError> {
        self.engine
            .into_inner()
            .map_err(|_| EditorError::InvalidEdit("editor engine lock poisoned"))
    }
}

impl Default for EditorEnginePort {
    fn default() -> Self {
        Self::new(EditorEngine::default())
    }
}

const DEFAULT_RETENTION_BUDGET_SNAPSHOTS: usize = 256;
const DEFAULT_RETENTION_BUDGET_BYTES: usize = DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES * 4;
const DEFAULT_TRANSACTION_EVENT_QUEUE_CAPACITY: usize = 256;
const DEFAULT_SNAPSHOT_LEASE_TTL_MILLIS: u64 = 60_000;

/// Buffer operating mode selected by size-based degradation gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferMode {
    /// Full-featured editing mode.
    Normal,
    /// Degraded mode for large buffers to protect interactive latency.
    Degraded,
}

/// Editor runtime thresholds used for degraded mode and retention behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorThresholds {
    /// Byte-size threshold above which buffers open in degraded mode.
    pub large_file_threshold_bytes: usize,
    /// Max retained undo/redo snapshots per buffer before trimming oldest history.
    pub retention_budget_snapshots: usize,
}

impl Default for EditorThresholds {
    fn default() -> Self {
        Self {
            large_file_threshold_bytes: DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES,
            retention_budget_snapshots: DEFAULT_RETENTION_BUDGET_SNAPSHOTS,
        }
    }
}

/// Preference used when snapshot retention budgets require eviction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotEvictionPreference {
    /// Evict the oldest undo-history snapshot before redo-history snapshots.
    UndoThenRedo,
    /// Evict the oldest redo-history snapshot before undo-history snapshots.
    RedoThenUndo,
}

/// Snapshot retention budgets for editor-owned undo/redo history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotRetentionPolicy {
    /// Maximum number of retained snapshots, including current and pending-save pins.
    pub max_snapshot_count: usize,
    /// Maximum estimated bytes retained across tracked snapshots.
    pub max_estimated_bytes: usize,
    /// Preferred eviction order for unpinned history snapshots.
    pub eviction_preference: SnapshotEvictionPreference,
}

impl Default for SnapshotRetentionPolicy {
    fn default() -> Self {
        Self {
            max_snapshot_count: DEFAULT_RETENTION_BUDGET_SNAPSHOTS,
            max_estimated_bytes: DEFAULT_RETENTION_BUDGET_BYTES,
            eviction_preference: SnapshotEvictionPreference::UndoThenRedo,
        }
    }
}

impl Default for EditorEngine {
    fn default() -> Self {
        Self {
            next_buffer_id: 1,
            buffers: HashMap::new(),
            file_to_buffer: HashMap::new(),
            transaction_log: Vec::new(),
            transaction_events: VecDeque::with_capacity(DEFAULT_TRANSACTION_EVENT_QUEUE_CAPACITY),
            transaction_event_queue_capacity: DEFAULT_TRANSACTION_EVENT_QUEUE_CAPACITY,
            dropped_transaction_event_count: 0,
            pending_save_requests: Vec::new(),
            snapshot_leases: HashMap::new(),
            pinned_snapshot_ids: HashSet::new(),
            thresholds: EditorThresholds::default(),
            snapshot_retention_policy: SnapshotRetentionPolicy::default(),
            retained_snapshots: VecDeque::new(),
        }
    }
}

impl EditorEngine {
    /// Create a new empty engine.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an engine with explicit threshold tuning for degraded mode and retention controls.
    pub fn with_thresholds(thresholds: EditorThresholds) -> Self {
        let mut engine = Self::new();
        engine.thresholds = thresholds;
        engine
    }

    /// Create an engine with explicit snapshot retention policy.
    pub fn with_snapshot_retention_policy(policy: SnapshotRetentionPolicy) -> Self {
        let mut engine = Self::new();
        engine.snapshot_retention_policy = policy;
        engine
    }

    /// Create an engine with an explicit bounded transaction event queue capacity.
    pub fn with_transaction_event_queue_capacity(capacity: usize) -> Self {
        let mut engine = Self::new();
        engine.transaction_event_queue_capacity = capacity.max(1);
        engine.transaction_events =
            VecDeque::with_capacity(engine.transaction_event_queue_capacity.max(1));
        engine
    }

    /// Returns the threshold configuration currently active for this editor.
    pub fn thresholds(&self) -> EditorThresholds {
        self.thresholds
    }

    /// Returns the active snapshot retention policy.
    pub fn snapshot_retention_policy(&self) -> SnapshotRetentionPolicy {
        self.snapshot_retention_policy
    }

    /// Open a new buffer for a workspace file.
    pub fn open_buffer(
        &mut self,
        workspace_id: WorkspaceId,
        file_id: FileId,
        file_path: impl Into<String>,
        initial_text: impl Into<String>,
    ) -> Result<BufferId, EditorError> {
        if self.file_to_buffer.contains_key(&(workspace_id, file_id)) {
            return Err(EditorError::FileAlreadyOpen(file_id));
        }
        let buffer_id = BufferId(self.next_buffer_id);
        self.next_buffer_id += 1;

        let initial_text = initial_text.into();
        let file_path = file_path.into();
        let detection = legion_text::detect_binary(initial_text.as_bytes());
        if let legion_text::BinaryDetectionResult::Binary { first_nul_offset } = detection {
            return Err(EditorError::BinaryFileRefused {
                path: file_path.clone(),
                nul_offset: first_nul_offset,
            });
        }
        let mode = self.mode_for_byte_len(initial_text.len());
        let buffer = TextBuffer::try_with_version_and_cache_policy(
            initial_text,
            BufferVersion(0),
            matches!(mode, BufferMode::Normal),
        )?;
        let state = EditorBufferState::build(
            workspace_id,
            buffer_id,
            file_id,
            file_path,
            buffer,
            mode,
            // Handed a complete String: the whole file was in memory
            // before this call, so it is not streamed however large it is.
            false,
        )?;
        self.retain_snapshot_descriptor(buffer_id, state.current_snapshot.descriptor());
        self.file_to_buffer
            .insert((workspace_id, file_id), buffer_id);
        self.buffers.insert(buffer_id, state);
        Ok(buffer_id)
    }

    /// Open a buffer by streaming from a file path, avoiding a full `String` intermediate.
    ///
    /// This is the preferred open path for large files. The rope is built incrementally via
    /// [`Rope::from_reader`](ropey::Rope::from_reader) which reads through a
    /// [`BufReader`](std::io::BufReader) without ever allocating the entire file as a
    /// contiguous `String`. Binary detection is performed by reading the first 8 KiB of the
    /// file before building the rope, consistent with the NUL-byte heuristic used by `git`.
    pub fn open_buffer_streaming(
        &mut self,
        workspace_id: WorkspaceId,
        file_id: FileId,
        file_path: impl Into<String>,
        disk_path: &std::path::Path,
    ) -> Result<BufferId, EditorError> {
        if self.file_to_buffer.contains_key(&(workspace_id, file_id)) {
            return Err(EditorError::FileAlreadyOpen(file_id));
        }
        let file_path_str = file_path.into();

        // Open the file once and reuse the handle for header, metadata, and rope building.
        let mut file = std::fs::File::open(disk_path).map_err(|e| {
            EditorError::Text(legion_text::TextError::Io {
                kind: e.kind(),
                message: e.to_string(),
            })
        })?;

        // Read the first 8 KiB for binary detection.
        let header = {
            let mut buf = vec![0u8; legion_text::binary::BINARY_DETECTION_WINDOW_BYTES];
            let n = std::io::Read::read(&mut file, &mut buf).map_err(|e| {
                EditorError::Text(legion_text::TextError::Io {
                    kind: e.kind(),
                    message: e.to_string(),
                })
            })?;
            buf.truncate(n);
            buf
        };

        if let legion_text::BinaryDetectionResult::Binary { first_nul_offset } =
            legion_text::detect_binary(&header)
        {
            return Err(EditorError::BinaryFileRefused {
                path: file_path_str,
                nul_offset: first_nul_offset,
            });
        }

        // Get byte length from the same file handle's metadata.
        let byte_len = file
            .metadata()
            .map_err(|e| {
                EditorError::Text(legion_text::TextError::Io {
                    kind: e.kind(),
                    message: e.to_string(),
                })
            })?
            .len() as usize;
        let mode = self.mode_for_byte_len(byte_len);

        // Seek back to start and build the rope from the same handle.
        std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(0)).map_err(|e| {
            EditorError::Text(legion_text::TextError::Io {
                kind: e.kind(),
                message: e.to_string(),
            })
        })?;
        let reader = std::io::BufReader::new(file);
        let buffer = TextBuffer::from_reader_with_version_and_cache_policy(
            reader,
            BufferVersion(0),
            matches!(mode, BufferMode::Normal),
        )?;

        let buffer_id = BufferId(self.next_buffer_id);
        self.next_buffer_id += 1;

        let state = EditorBufferState::build(
            workspace_id,
            buffer_id,
            file_id,
            file_path_str,
            buffer,
            mode,
            true,
        )?;
        self.retain_snapshot_descriptor(buffer_id, state.current_snapshot.descriptor());
        self.file_to_buffer
            .insert((workspace_id, file_id), buffer_id);
        self.buffers.insert(buffer_id, state);
        Ok(buffer_id)
    }

    /// Close a buffer.
    pub fn close_buffer(&mut self, buffer_id: BufferId) -> Result<(), EditorError> {
        let state = self
            .buffers
            .remove(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        self.file_to_buffer
            .remove(&(state.workspace_id, state.file_id));
        self.release_snapshot_descriptor_if_unreferenced(state.current_snapshot.snapshot_id());
        for entry in state.undo_stack.iter().chain(state.redo_stack.iter()) {
            self.release_snapshot_descriptor_if_unreferenced(entry.snapshot.snapshot_id());
        }
        Ok(())
    }

    /// Get immutable text for a buffer.
    pub fn text(&self, buffer_id: BufferId) -> Result<&str, EditorError> {
        self.buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .buffer
            .try_full_text()
            .map_err(EditorError::from)
    }

    /// Get file path for a buffer.
    pub fn file_path(&self, buffer_id: BufferId) -> Result<&str, EditorError> {
        Ok(&self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .file_path)
    }

    /// Returns true when buffer has unsaved changes.
    pub fn is_dirty(&self, buffer_id: BufferId) -> Result<bool, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .dirty)
    }

    /// Return current buffer version.
    pub fn buffer_version(&self, buffer_id: BufferId) -> Result<BufferVersion, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .buffer
            .version())
    }

    /// Resolve a logical line and UTF-8 byte column through the authoritative buffer index.
    pub fn buffer_byte_offset(
        &self,
        buffer_id: BufferId,
        position: TextPosition,
    ) -> Result<usize, EditorError> {
        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        Ok(state.buffer.try_byte_offset(position)?)
    }

    /// Return the current operating mode for a buffer.
    pub fn buffer_mode(&self, buffer_id: BufferId) -> Result<BufferMode, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .mode)
    }

    /// Return the current snapshot descriptor for a buffer.
    pub fn current_snapshot(
        &self,
        buffer_id: BufferId,
    ) -> Result<&TextSnapshotDescriptor, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .current_snapshot
            .descriptor())
    }

    /// Return a bounded immutable text window around a caret in the current buffer snapshot.
    ///
    /// The text model owns UTF-8, logical-line, and grapheme-boundary validation; this editor
    /// authority method only resolves the buffer and delegates without materializing full text.
    pub fn line_window_around_byte(
        &self,
        buffer_id: BufferId,
        caret_byte: usize,
        max_bytes: usize,
    ) -> Result<legion_text::TextWindow, EditorError> {
        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        Ok(state
            .current_snapshot
            .line_window_around_byte(caret_byte, max_bytes)?)
    }

    /// Return protocol chunk descriptors for the current snapshot of a buffer.
    pub fn snapshot_chunk_descriptors(
        &self,
        buffer_id: BufferId,
    ) -> Result<Vec<SnapshotChunkDescriptor>, EditorError> {
        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        Ok(Self::protocol_snapshot_chunk_descriptors(
            &state.current_snapshot,
        ))
    }

    /// Acquire a descriptor-only lease over the current snapshot for a downstream consumer.
    pub fn lease_snapshot(
        &mut self,
        buffer_id: BufferId,
        consumer_kind: SnapshotConsumerKind,
    ) -> Result<SnapshotLeaseDescriptor, EditorError> {
        self.sweep_expired_snapshot_leases();
        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        let snapshot = state.current_snapshot.clone();
        let now = TimestampMillis::now();
        let descriptor = SnapshotLeaseDescriptor {
            lease_id: Uuid::now_v7(),
            buffer_id,
            snapshot_id: snapshot.snapshot_id(),
            buffer_version: snapshot.buffer_version(),
            consumer_kind,
            expires_at: TimestampMillis(now.0.saturating_add(DEFAULT_SNAPSHOT_LEASE_TTL_MILLIS)),
            chunk_count: snapshot.chunk_descriptors().len() as u32,
            schema_version: 2,
        };
        self.snapshot_leases.insert(
            descriptor.lease_id,
            SnapshotLeaseRecord {
                snapshot,
                descriptor: descriptor.clone(),
                owned_state: None,
            },
        );
        Ok(descriptor)
    }

    /// Create an owned worker-readable view from an existing validated lease.
    ///
    /// The returned handle shares the immutable snapshot and may be moved to a background thread.
    /// Releasing the editor lease revokes all clones. The descriptor is checked exactly before the
    /// snapshot is exposed to the worker.
    pub fn owned_snapshot_lease(
        &mut self,
        expected_lease: &SnapshotLeaseDescriptor,
    ) -> Result<OwnedSnapshotLease, EditorError> {
        let now = TimestampMillis::now();
        if let Some(lease) = self.snapshot_leases.get(&expected_lease.lease_id)
            && now.0 > lease.descriptor.expires_at.0
        {
            let expired_at = lease.descriptor.expires_at;
            self.release_snapshot_lease(expected_lease.lease_id);
            return Err(EditorError::SnapshotLeaseExpired {
                lease_id: expected_lease.lease_id,
                expired_at,
                now,
            });
        }
        self.sweep_expired_snapshot_leases();
        let lease = self
            .snapshot_leases
            .get_mut(&expected_lease.lease_id)
            .ok_or(EditorError::SnapshotLeaseNotFound(expected_lease.lease_id))?;
        if lease.descriptor != *expected_lease {
            return Err(EditorError::SnapshotLeaseStale {
                lease_id: expected_lease.lease_id,
                expected_buffer_id: expected_lease.buffer_id,
                actual_buffer_id: lease.descriptor.buffer_id,
                expected_snapshot_id: expected_lease.snapshot_id,
                actual_snapshot_id: lease.descriptor.snapshot_id,
                expected_buffer_version: expected_lease.buffer_version,
                actual_buffer_version: lease.descriptor.buffer_version,
            });
        }
        if lease.owned_state.is_none() {
            lease.owned_state = Some(OwnedSnapshotCell::new(lease.snapshot.clone()));
        }
        let state = lease
            .owned_state
            .as_ref()
            .expect("owned state initialized")
            .clone();
        Ok(OwnedSnapshotLease {
            descriptor: lease.descriptor.clone(),
            cell: state,
        })
    }

    /// Read a bounded chunk through an active snapshot lease after validating identity and expiry.
    pub fn read_snapshot_lease_chunk(
        &self,
        lease_id: Uuid,
        expected_buffer_id: BufferId,
        expected_snapshot_id: SnapshotId,
        expected_buffer_version: BufferVersion,
        chunk_index: u32,
    ) -> Result<SnapshotLeaseChunk, EditorError> {
        self.read_snapshot_lease_chunk_at(
            lease_id,
            expected_buffer_id,
            expected_snapshot_id,
            expected_buffer_version,
            chunk_index,
            TimestampMillis::now(),
        )
    }

    fn read_snapshot_lease_chunk_at(
        &self,
        lease_id: Uuid,
        expected_buffer_id: BufferId,
        expected_snapshot_id: SnapshotId,
        expected_buffer_version: BufferVersion,
        chunk_index: u32,
        now: TimestampMillis,
    ) -> Result<SnapshotLeaseChunk, EditorError> {
        let lease = self
            .snapshot_leases
            .get(&lease_id)
            .ok_or(EditorError::SnapshotLeaseNotFound(lease_id))?;
        let descriptor = &lease.descriptor;
        if now.0 > descriptor.expires_at.0 {
            return Err(EditorError::SnapshotLeaseExpired {
                lease_id,
                expired_at: descriptor.expires_at,
                now,
            });
        }
        if descriptor.buffer_id != expected_buffer_id
            || descriptor.snapshot_id != expected_snapshot_id
            || descriptor.buffer_version != expected_buffer_version
        {
            return Err(EditorError::SnapshotLeaseStale {
                lease_id,
                expected_buffer_id,
                actual_buffer_id: descriptor.buffer_id,
                expected_snapshot_id,
                actual_snapshot_id: descriptor.snapshot_id,
                expected_buffer_version,
                actual_buffer_version: descriptor.buffer_version,
            });
        }

        let text = lease.snapshot.chunk_text(chunk_index as usize)?;
        let chunk = lease
            .snapshot
            .chunk_descriptors()
            .get(chunk_index as usize)
            .ok_or(TextError::ChunkOutOfBounds {
                chunk: chunk_index as usize,
                chunk_count: lease.snapshot.chunk_descriptors().len(),
            })?;
        Ok(SnapshotLeaseChunk {
            lease: descriptor.clone(),
            chunk: Self::protocol_snapshot_chunk_descriptor(descriptor.snapshot_id, chunk),
            text,
            schema_version: 1,
        })
    }

    /// Read one bounded logical-line chunk through an active snapshot lease.
    ///
    /// `start_byte` is an absolute snapshot byte offset. Pass the returned
    /// `line.end_byte` to continue; the method validates the supplied lease
    /// descriptor and expiry before touching the retained snapshot.
    pub fn read_snapshot_lease_line_chunk(
        &self,
        expected_lease: &SnapshotLeaseDescriptor,
        line: usize,
        start_byte: usize,
        max_bytes: usize,
    ) -> Result<SnapshotLeaseLineChunk, EditorError> {
        self.read_snapshot_lease_line_chunk_at(
            expected_lease,
            line,
            start_byte,
            max_bytes,
            TimestampMillis::now(),
        )
    }

    fn read_snapshot_lease_line_chunk_at(
        &self,
        expected_lease: &SnapshotLeaseDescriptor,
        line: usize,
        start_byte: usize,
        max_bytes: usize,
        now: TimestampMillis,
    ) -> Result<SnapshotLeaseLineChunk, EditorError> {
        let lease = self
            .snapshot_leases
            .get(&expected_lease.lease_id)
            .ok_or(EditorError::SnapshotLeaseNotFound(expected_lease.lease_id))?;
        let descriptor = &lease.descriptor;
        if now.0 > descriptor.expires_at.0 {
            return Err(EditorError::SnapshotLeaseExpired {
                lease_id: expected_lease.lease_id,
                expired_at: descriptor.expires_at,
                now,
            });
        }
        if descriptor != expected_lease {
            return Err(EditorError::SnapshotLeaseStale {
                lease_id: expected_lease.lease_id,
                expected_buffer_id: expected_lease.buffer_id,
                actual_buffer_id: descriptor.buffer_id,
                expected_snapshot_id: expected_lease.snapshot_id,
                actual_snapshot_id: descriptor.snapshot_id,
                expected_buffer_version: expected_lease.buffer_version,
                actual_buffer_version: descriptor.buffer_version,
            });
        }
        let line = lease
            .snapshot
            .line_chunk_from_byte(line, start_byte, max_bytes)?;
        Ok(SnapshotLeaseLineChunk {
            lease: descriptor.clone(),
            line,
            schema_version: 1,
        })
    }

    /// Release a previously acquired snapshot lease.
    pub fn release_snapshot_lease(&mut self, lease_id: Uuid) -> Option<SnapshotLeaseDescriptor> {
        let lease = self.snapshot_leases.remove(&lease_id)?;
        if let Some(state) = lease.owned_state {
            state.revoke();
        }
        self.release_snapshot_descriptor_if_unreferenced(lease.snapshot.snapshot_id());
        Some(lease.descriptor)
    }

    fn sweep_expired_snapshot_leases(&mut self) {
        let now = TimestampMillis::now();
        let expired: Vec<Uuid> = self
            .snapshot_leases
            .iter()
            .filter(|(_, lease)| now.0 > lease.descriptor.expires_at.0)
            .map(|(lease_id, _)| *lease_id)
            .collect();
        for lease_id in expired {
            self.release_snapshot_lease(lease_id);
        }
    }

    #[cfg(test)]
    fn expire_snapshot_lease_for_test(&mut self, lease_id: Uuid) {
        if let Some(lease) = self.snapshot_leases.get_mut(&lease_id) {
            lease.descriptor.expires_at = TimestampMillis(0);
        }
    }

    /// Return the workspace id and file id for a buffer.
    pub fn buffer_identity(
        &self,
        buffer_id: BufferId,
    ) -> Result<(WorkspaceId, FileId), EditorError> {
        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        Ok((state.workspace_id, state.file_id))
    }

    /// Return the open buffer for a workspace file when it is already editor-owned.
    pub fn buffer_for_file(&self, workspace_id: WorkspaceId, file_id: FileId) -> Option<BufferId> {
        self.file_to_buffer.get(&(workspace_id, file_id)).copied()
    }

    /// Return the open buffer for a workspace path when it is already editor-owned.
    pub fn buffer_for_path(&self, workspace_id: WorkspaceId, file_path: &str) -> Option<BufferId> {
        self.buffers
            .values()
            .find(|state| state.workspace_id == workspace_id && state.file_path == file_path)
            .map(|state| state.buffer_id)
    }

    /// Return protocol metadata for a buffer.
    pub fn buffer_metadata(
        &self,
        buffer_id: BufferId,
    ) -> Result<EditorBufferMetadata, EditorError> {
        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        let descriptor = state.current_snapshot.descriptor();

        Ok(EditorBufferMetadata {
            workspace_id: state.workspace_id,
            buffer_id: state.buffer_id,
            file_id: state.file_id,
            path: CanonicalPath(state.file_path.clone()),
            snapshot_id: descriptor.snapshot_id,
            buffer_version: descriptor.buffer_version,
            byte_len: descriptor.byte_len as u64,
            content_hash: Some(descriptor.content_hash.clone()),
            dirty: state.dirty,
            save_state: state.save_state,
            conflict: state.conflict_state.clone(),
            undo_len: state.undo_stack.len(),
            redo_len: state.redo_stack.len(),
            file_size_classification: match state.mode {
                BufferMode::Normal => legion_protocol::FileSizeClassification::Normal,
                BufferMode::Degraded => legion_protocol::FileSizeClassification::Large,
            },
            schema_version: 1,
        })
    }

    /// Build deterministic lexical completions for the current editor snapshot.
    pub fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<LspCompletionResponse, EditorError> {
        let buffer_id = self
            .buffer_for_file(request.workspace_id, request.file_id)
            .ok_or(EditorError::CompletionBufferNotFound {
                workspace_id: request.workspace_id,
                file_id: request.file_id,
            })?;
        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        let current_snapshot_id = state.current_snapshot.snapshot_id();
        if current_snapshot_id != request.snapshot_id {
            return Err(EditorError::StaleCompletionSnapshot {
                requested: request.snapshot_id,
                current: current_snapshot_id,
            });
        }

        let byte_offset = Self::completion_byte_offset(state, request.position)?;
        let text = match state.buffer.try_full_text() {
            Ok(text) => text,
            Err(TextError::FullCacheBudgetExceeded { .. }) => {
                return Ok(LspCompletionResponse {
                    correlation_id: request.correlation_id,
                    items: Vec::new(),
                });
            }
            Err(error) => return Err(EditorError::Text(error)),
        };

        Ok(LspCompletionResponse {
            correlation_id: request.correlation_id,
            items: lexical_completion_items(text, byte_offset),
        })
    }

    /// Build a protocol viewport projection over the current buffer snapshot.
    pub fn viewport_projection(
        &self,
        request: EditorViewportRequest,
    ) -> Result<ViewportProjection, EditorError> {
        let state = self
            .buffers
            .get(&request.buffer_id)
            .ok_or(EditorError::BufferNotFound(request.buffer_id))?;
        let descriptor = state.current_snapshot.descriptor();
        let line_count = state.current_snapshot.line_count().max(1);
        let top_line = (request.scroll.top_line as usize).min(line_count.saturating_sub(1));
        let approx_visible_lines = ((request.dimensions.height_px / 16).max(1)) as usize;
        let end_line = (top_line + approx_visible_lines).min(line_count);
        let visible_line_slices = state
            .current_snapshot
            .visible_line_slices(top_line, end_line)?;
        let line_metrics = visible_line_slices
            .iter()
            .map(|slice| {
                let line_start_byte = state
                    .current_snapshot
                    .line_index()
                    .byte_offset(TextPosition::new(slice.line, 0))?;
                let line_start_utf16 = state
                    .current_snapshot
                    .line_index()
                    .utf16_offset(line_start_byte)?;
                Ok(ViewportLineMetric {
                    byte_length: state
                        .current_snapshot
                        .line_index()
                        .line_byte_len(slice.line)? as u64,
                    utf16_length: state
                        .current_snapshot
                        .line_index()
                        .line_utf16_len(slice.line)? as u64,
                    line_start_byte_offset: Some(line_start_byte as u64),
                    line_start_utf16_offset: Some(line_start_utf16 as u64),
                    line_ending_width: state
                        .current_snapshot
                        .line_index()
                        .line_ending_bytes(slice.line)?
                        as u8,
                    exact: true,
                })
            })
            .collect::<Result<Vec<_>, EditorError>>()?;
        let line_slices = visible_line_slices
            .iter()
            .map(|slice| {
                Ok(ViewportLineSlice {
                    line_number: slice.line as u32,
                    visible_text: slice.text.clone(),
                    byte_range: ByteRange::new(
                        slice.line_start_byte as u64,
                        slice.slice_end_byte as u64,
                    ),
                    utf16_range: Self::protocol_utf16_range(
                        state
                            .current_snapshot
                            .line_index()
                            .utf16_position(slice.line_start_byte)?,
                        state
                            .current_snapshot
                            .line_index()
                            .utf16_position(slice.slice_end_byte)?,
                    ),
                    chunk_hash: Self::chunk_hash_for_line(&state.current_snapshot, slice.line),
                    truncation_state: if slice.truncated {
                        ViewportLineTruncationState::Trailing
                    } else {
                        ViewportLineTruncationState::None
                    },
                })
            })
            .collect::<Result<Vec<_>, EditorError>>()?;
        let start = state
            .buffer
            .try_byte_offset(TextPosition::new(top_line, 0))?;
        let end = if end_line >= line_count {
            state.buffer.len()
        } else {
            state
                .buffer
                .try_byte_offset(TextPosition::new(end_line, 0))?
        };
        let cursor = state
            .carets
            .first()
            .map(|caret| caret.head)
            .unwrap_or_else(TextPosition::zero);
        // A streamed buffer is reported as streaming even though its
        // `BufferMode` is also Degraded: both defer overlays, but only the
        // streamed one never had the whole file in memory, and that is the
        // difference a user needs told.
        let mode = match (state.mode, state.streamed) {
            (BufferMode::Normal, _) => ViewportProjectionMode::Normal,
            (BufferMode::Degraded, true) => ViewportProjectionMode::StreamingLargeFile,
            (BufferMode::Degraded, false) => ViewportProjectionMode::DegradedLargeFile,
        };
        let selections = state
            .carets
            .iter()
            .filter_map(|caret| caret.anchor.map(|anchor| (anchor, caret.head)))
            .map(|(anchor, head)| {
                let anchor_offset = state.buffer.try_byte_offset(anchor)?;
                let head_offset = state.buffer.try_byte_offset(head)?;
                let range = if anchor_offset <= head_offset {
                    TextRange::new(anchor, head)
                } else {
                    TextRange::new(head, anchor)
                };
                Self::protocol_range(&state.buffer, range)
            })
            .collect::<Result<Vec<_>, EditorError>>()?;

        Ok(ViewportProjection {
            workspace_id: state.workspace_id,
            buffer_id: state.buffer_id,
            file_id: Some(state.file_id),
            snapshot_id: descriptor.snapshot_id,
            buffer_version: descriptor.buffer_version,
            visible_range: ProtocolTextRange {
                start: Self::protocol_coordinate(&state.buffer, top_line, start)?,
                end: Self::protocol_coordinate_from_offset(&state.buffer, end)?,
            },
            selections,
            cursor: Self::protocol_coordinate_from_offset(&state.buffer, state.buffer.try_byte_offset(cursor)?)?,
            cursors: state
                .carets
                .iter()
                .map(|caret| {
                    let offset = state.buffer.try_byte_offset(caret.head)?;
                    Self::protocol_coordinate_from_offset(&state.buffer, offset)
                })
                .collect::<Result<Vec<_>, _>>()?,
            cursor_affinities: state
                .carets
                .iter()
                .map(|caret| caret.affinity)
                .collect(),
            scroll: request.scroll,
            dimensions: request.dimensions,
            line_wrapping_policy: legion_protocol::LineWrappingPolicy::Off,
            wrap_column: None,
            mode,
            line_slices,
            line_metrics,
            decoration_spans: Vec::<ViewportDecorationSpan>::new(),
            fold_ranges: Vec::<ViewportFoldRange>::new(),
            semantic_token_overlays: Vec::<ViewportSemanticTokenOverlay>::new(),
            large_file_status: matches!(state.mode, BufferMode::Degraded).then(|| LargeFileStatus {
                threshold_bytes: self.thresholds.large_file_threshold_bytes as u64,
                byte_len: descriptor.byte_len as u64,
                disabled_overlay_reasons: vec![
                    "decorations deferred in degraded large-file mode".to_string(),
                    "fold computation deferred in degraded large-file mode".to_string(),
                    "semantic token overlays deferred in degraded large-file mode".to_string(),
                ],
                bounded_search_enabled: true,
                message: format!(
                    "Large file degraded mode is active for {} bytes; viewport payloads are chunked and saves assemble the full file from chunks on request.",
                    descriptor.byte_len
                ),
            }),
            schema_version: 2,
        })
    }

    fn protocol_range(
        buffer: &TextBuffer,
        range: TextRange,
    ) -> Result<ProtocolTextRange, EditorError> {
        Ok(ProtocolTextRange {
            start: Self::protocol_coordinate(
                buffer,
                range.start.line,
                buffer.try_byte_offset(range.start)?,
            )?,
            end: Self::protocol_coordinate(
                buffer,
                range.end.line,
                buffer.try_byte_offset(range.end)?,
            )?,
        })
    }

    fn protocol_coordinate_from_offset(
        buffer: &TextBuffer,
        offset: usize,
    ) -> Result<TextCoordinate, EditorError> {
        let position = buffer.try_position(offset)?;
        Self::protocol_coordinate(buffer, position.line, offset)
    }

    fn protocol_coordinate(
        buffer: &TextBuffer,
        _line: usize,
        byte_offset: usize,
    ) -> Result<TextCoordinate, EditorError> {
        let position = buffer.try_position(byte_offset)?;
        let utf16_offset = Some(Self::absolute_utf16_offset(buffer, byte_offset)?);
        Ok(TextCoordinate {
            line: position.line as u32,
            character: position.column as u32,
            byte_offset: Some(byte_offset as u64),
            utf16_offset,
        })
    }

    /// Apply a single edit as an atomic transaction.
    pub fn apply_edit(
        &mut self,
        buffer_id: BufferId,
        edit: TextEdit,
        source: TransactionSource,
        undo_group_id: Option<Uuid>,
        correlation_id: Option<CorrelationId>,
    ) -> Result<TransactionRecord, EditorError> {
        self.apply_edits(buffer_id, vec![edit], source, undo_group_id, correlation_id)
    }

    /// Apply a batch of edits atomically as one deterministic transaction record.
    pub fn apply_edits(
        &mut self,
        buffer_id: BufferId,
        edits: Vec<TextEdit>,
        source: TransactionSource,
        undo_group_id: Option<Uuid>,
        correlation_id: Option<CorrelationId>,
    ) -> Result<TransactionRecord, EditorError> {
        self.apply_edits_with_caret_policy(
            buffer_id,
            edits,
            source,
            undo_group_id,
            correlation_id,
            false,
        )
    }

    fn apply_edits_with_caret_policy(
        &mut self,
        buffer_id: BufferId,
        edits: Vec<TextEdit>,
        source: TransactionSource,
        undo_group_id: Option<Uuid>,
        correlation_id: Option<CorrelationId>,
        collapse_anchors: bool,
    ) -> Result<TransactionRecord, EditorError> {
        if edits.is_empty() {
            return Err(EditorError::InvalidEdit("edit batch cannot be empty"));
        }

        let thresholds = self.thresholds;

        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        let plan = Self::prepare_batch_edit_plan(state, edits)?;
        let mut staged_buffer = state.buffer.clone();
        let mut deltas = Vec::with_capacity(plan.edits.len());

        // `plan.edits` is ordered descending by start offset so that applying an
        // edit never shifts the offsets of edits that have not been applied yet.
        for prepared in &plan.edits {
            staged_buffer.try_replace_range(prepared.start, prepared.end, &prepared.new_text)?;
        }

        let mapped_carets = plan
            .pre_carets
            .iter()
            .map(|caret| {
                let head =
                    map_edit_offset(state.buffer.try_byte_offset(caret.head)?, true, &plan.edits);
                let anchor = match caret.anchor {
                    Some(anchor) => Some(map_edit_offset(
                        state.buffer.try_byte_offset(anchor)?,
                        false,
                        &plan.edits,
                    )),
                    None => None,
                };
                Ok(DirectedCaret::new(
                    staged_buffer.try_position(head)?,
                    anchor
                        .map(|offset| staged_buffer.try_position(offset))
                        .transpose()?,
                ))
            })
            .collect::<Result<Vec<_>, EditorError>>()?;
        let mapped_carets = if collapse_anchors {
            mapped_carets
                .into_iter()
                .map(|caret| DirectedCaret::new(caret.head, None))
                .collect()
        } else {
            mapped_carets
        };

        // Recompute each delta against the *final* staged buffer. Iterating
        // ascending (the reverse of `plan.edits`) lets us carry a running
        // cumulative length shift introduced by lower-offset edits so each
        // recorded byte/UTF-16 range reflects its position in the post-edit
        // buffer rather than the original coordinates.
        let mut cumulative_shift: i64 = 0;
        for prepared in plan.edits.iter().rev() {
            let final_start = (prepared.start as i64 + cumulative_shift) as usize;
            let final_end = final_start + prepared.new_text.len();
            let utf16 = staged_buffer
                .line_index()
                .utf16_range(final_start, final_end)?;
            deltas.push(ChangedDelta {
                byte_range: ByteRange::new(final_start as u64, final_end as u64),
                utf16_range: utf16,
            });
            let removed = (prepared.end - prepared.start) as i64;
            cumulative_shift += prepared.new_text.len() as i64 - removed;
        }

        let next_version = BufferVersion(plan.pre_version.0 + 1);
        staged_buffer.set_version(next_version);
        let next_mode = Self::mode_for_byte_len_with_thresholds(thresholds, staged_buffer.len());
        staged_buffer.set_full_cache_policy(matches!(next_mode, BufferMode::Normal))?;
        let post_snapshot =
            staged_buffer.try_snapshot_with_retention(RetentionPinReason::CurrentBuffer)?;
        let post_descriptor = post_snapshot.descriptor().clone();

        let (
            workspace_id,
            file_id,
            old_current_snapshot_id,
            redo_snapshot_ids,
            post_descriptor_for_retention,
            history_anchor_added,
        ) = {
            let state = self
                .buffers
                .get_mut(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            let redo_snapshot_ids = state
                .redo_stack
                .iter()
                .map(|entry| entry.snapshot.snapshot_id())
                .collect::<Vec<_>>();
            let old_current_snapshot_id = state.current_snapshot.snapshot_id();
            let coalesce = undo_group_id.is_some()
                && state.active_undo_group == undo_group_id
                && !state.active_group_evicted;
            let history_anchor_added = !(coalesce
                || (undo_group_id.is_some()
                    && state.active_group_evicted
                    && state.active_undo_group == undo_group_id));
            if history_anchor_added {
                state.undo_stack.push(UndoEntry {
                    snapshot: plan.pre_snapshot.clone(),
                    carets: plan.pre_carets.clone(),
                    vertical_layout_id: state.vertical_layout_id,
                    undo_group_id,
                });
            }
            if !(undo_group_id.is_some()
                && state.active_group_evicted
                && state.active_undo_group == undo_group_id)
            {
                state.active_group_evicted = false;
            }
            state.active_undo_group = undo_group_id;
            state.redo_stack.clear();
            state.buffer = staged_buffer;
            state.carets = mapped_carets;
            state.vertical_layout_id = None;
            state.mode = next_mode;
            state.current_snapshot = post_snapshot;
            state.dirty = true;
            state.save_state = if state.conflict_state.is_some() {
                FileConflictLifecycleState::ConflictDirty
            } else {
                FileConflictLifecycleState::Dirty
            };
            (
                state.workspace_id,
                state.file_id,
                old_current_snapshot_id,
                redo_snapshot_ids,
                state.current_snapshot.descriptor().clone(),
                history_anchor_added,
            )
        };

        for snapshot_id in redo_snapshot_ids {
            self.release_snapshot_descriptor_if_unreferenced(snapshot_id);
        }
        self.release_snapshot_descriptor_if_unreferenced(old_current_snapshot_id);
        if history_anchor_added {
            let mut undo_descriptor = plan.pre_snapshot.descriptor().clone();
            undo_descriptor.retention_pin_reason = RetentionPinReason::UndoHistory;
            self.retain_snapshot_descriptor(buffer_id, &undo_descriptor);
        }
        self.retain_snapshot_descriptor(buffer_id, &post_descriptor_for_retention);

        self.enforce_snapshot_retention_policy();

        let tx = TransactionRecord {
            transaction_id: Uuid::now_v7(),
            causality_trace_id: Uuid::now_v7(),
            workspace_id,
            buffer_id,
            file_id,
            source,
            pre_snapshot: plan.pre_descriptor,
            post_snapshot: post_descriptor,
            deltas,
            undo_group_id,
            occurred_at: TimestampMillis::now(),
            correlation_id,
        };

        self.transaction_log.push(tx.clone());
        self.enqueue_transaction_event(&tx);
        Ok(tx)
    }

    /// Apply protocol byte-coordinate edits after checking the target buffer identity.
    pub fn apply_protocol_edits(
        &mut self,
        request: EditorApplyTransactionRequest,
    ) -> Result<TransactionRecord, EditorError> {
        let EditorApplyTransactionRequest {
            workspace_id,
            buffer_id,
            file_id,
            edits,
            source,
            undo_group_id,
            correlation_id,
        } = request;
        let (actual_workspace_id, actual_file_id) = self.buffer_identity(buffer_id)?;
        if actual_workspace_id != workspace_id {
            return Err(EditorError::InvalidEdit(
                "workspace id does not match buffer",
            ));
        }
        if actual_file_id != file_id {
            return Err(EditorError::InvalidEdit("file id does not match buffer"));
        }

        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        let edits = edits
            .edits
            .into_iter()
            .map(|edit| {
                let range = edit.range.as_byte_range().ok_or(EditorError::InvalidEdit(
                    "editor apply requires byte-coordinate ranges",
                ))?;
                let start = state.buffer.try_position(range.start as usize)?;
                let end = state.buffer.try_position(range.end as usize)?;
                Ok(TextEdit::new(TextRange::new(start, end), edit.replacement))
            })
            .collect::<Result<Vec<_>, EditorError>>()?;

        self.apply_edits(
            buffer_id,
            edits,
            source,
            undo_group_id,
            Some(correlation_id),
        )
    }

    fn prepare_batch_edit_plan(
        state: &EditorBufferState,
        edits: Vec<TextEdit>,
    ) -> Result<BatchEditPlan, EditorError> {
        let mut prepared = Vec::with_capacity(edits.len());
        for edit in edits {
            let start = state.buffer.try_byte_offset(edit.range.start)?;
            let end = state.buffer.try_byte_offset(edit.range.end)?;
            if start > end {
                return Err(EditorError::InvalidEdit("edit range start must be <= end"));
            }
            prepared.push(PreparedBatchEdit {
                start,
                end,
                new_text: edit.new_text,
            });
        }

        prepared.sort_by_key(|edit| edit.start);
        for pair in prepared.windows(2) {
            if pair[0].end > pair[1].start {
                return Err(EditorError::InvalidEdit(
                    "edit batch ranges must not overlap",
                ));
            }
        }
        prepared.reverse();

        Ok(BatchEditPlan {
            pre_snapshot: state.current_snapshot.clone(),
            pre_descriptor: state.current_snapshot.descriptor().clone(),
            pre_version: state.buffer.version(),
            pre_carets: state.carets.clone(),
            edits: prepared,
        })
    }

    fn retain_snapshot_descriptor(
        &mut self,
        buffer_id: BufferId,
        descriptor: &TextSnapshotDescriptor,
    ) {
        self.remove_snapshot_descriptor(descriptor.snapshot_id);
        self.pinned_snapshot_ids.insert(descriptor.snapshot_id);
        self.retained_snapshots
            .push_back(RetainedSnapshotDescriptor {
                buffer_id,
                reason: descriptor.retention_pin_reason.clone(),
                descriptor: descriptor.clone(),
            });
    }

    fn remove_snapshot_descriptor(&mut self, snapshot_id: SnapshotId) {
        self.pinned_snapshot_ids.remove(&snapshot_id);
        self.retained_snapshots
            .retain(|snapshot| snapshot.descriptor.snapshot_id != snapshot_id);
    }

    fn retained_snapshot_bytes(&self) -> usize {
        self.retained_snapshots
            .iter()
            .map(|snapshot| snapshot.descriptor.memory_footprint_bytes)
            .sum()
    }

    fn enforce_snapshot_retention_policy(&mut self) {
        self.sweep_expired_snapshot_leases();
        loop {
            let over_count =
                self.retained_snapshots.len() > self.snapshot_retention_policy.max_snapshot_count;
            let over_bytes =
                self.retained_snapshot_bytes() > self.snapshot_retention_policy.max_estimated_bytes;
            if !over_count && !over_bytes {
                break;
            }

            let Some((buffer_id, stack_kind, snapshot_id)) =
                self.oldest_evictable_history_snapshot()
            else {
                break;
            };

            let mut evicted_snapshot_ids = Vec::new();
            if let Some(state) = self.buffers.get_mut(&buffer_id) {
                match stack_kind {
                    SnapshotStackKind::Undo => {
                        if let Some(idx) = state
                            .undo_stack
                            .iter()
                            .position(|entry| entry.snapshot.snapshot_id() == snapshot_id)
                        {
                            let group = state.undo_stack[idx].undo_group_id;
                            let end = group
                                .map(|group| {
                                    state.undo_stack[..=idx]
                                        .iter()
                                        .rposition(|entry| entry.undo_group_id == Some(group))
                                        .unwrap_or(idx)
                                })
                                .unwrap_or(idx);
                            evicted_snapshot_ids.extend(
                                state.undo_stack[..=end]
                                    .iter()
                                    .map(|entry| entry.snapshot.snapshot_id()),
                            );
                            state.undo_stack.drain(..=end);
                            if state.active_undo_group == group {
                                state.active_group_evicted = true;
                            }
                        }
                    }
                    SnapshotStackKind::Redo => {
                        if let Some(idx) = state
                            .redo_stack
                            .iter()
                            .position(|entry| entry.snapshot.snapshot_id() == snapshot_id)
                        {
                            let group = state.redo_stack[idx].undo_group_id;
                            let end = group
                                .map(|group| {
                                    state.redo_stack[..=idx]
                                        .iter()
                                        .rposition(|entry| entry.undo_group_id == Some(group))
                                        .unwrap_or(idx)
                                })
                                .unwrap_or(idx);
                            evicted_snapshot_ids.extend(
                                state.redo_stack[..=end]
                                    .iter()
                                    .map(|entry| entry.snapshot.snapshot_id()),
                            );
                            state.redo_stack.drain(..=end);
                        }
                    }
                }
            }
            // Every drained history entry is released independently. A lease,
            // current buffer, or pending save may still keep a snapshot alive.
            for snapshot_id in evicted_snapshot_ids {
                self.release_snapshot_descriptor_if_unreferenced(snapshot_id);
            }
        }
    }

    fn oldest_evictable_history_snapshot(
        &self,
    ) -> Option<(BufferId, SnapshotStackKind, SnapshotId)> {
        match self.snapshot_retention_policy.eviction_preference {
            SnapshotEvictionPreference::UndoThenRedo => self
                .oldest_evictable_history_snapshot_for(RetentionPinReason::UndoHistory)
                .or_else(|| {
                    self.oldest_evictable_history_snapshot_for(RetentionPinReason::RedoHistory)
                }),
            SnapshotEvictionPreference::RedoThenUndo => self
                .oldest_evictable_history_snapshot_for(RetentionPinReason::RedoHistory)
                .or_else(|| {
                    self.oldest_evictable_history_snapshot_for(RetentionPinReason::UndoHistory)
                }),
        }
    }

    fn oldest_evictable_history_snapshot_for(
        &self,
        reason: RetentionPinReason,
    ) -> Option<(BufferId, SnapshotStackKind, SnapshotId)> {
        self.retained_snapshots
            .iter()
            .find(|snapshot| {
                snapshot.reason == reason
                    && !self.is_snapshot_pinned(snapshot.descriptor.snapshot_id)
            })
            .map(|snapshot| {
                let kind = if reason == RetentionPinReason::UndoHistory {
                    SnapshotStackKind::Undo
                } else {
                    SnapshotStackKind::Redo
                };
                (snapshot.buffer_id, kind, snapshot.descriptor.snapshot_id)
            })
    }

    fn is_snapshot_pinned(&self, snapshot_id: SnapshotId) -> bool {
        self.pending_save_requests
            .iter()
            .any(|request| request.snapshot_id == snapshot_id)
            || self
                .snapshot_leases
                .values()
                .any(|lease| lease.snapshot.snapshot_id() == snapshot_id)
            || self
                .buffers
                .values()
                .any(|state| state.current_snapshot.snapshot_id() == snapshot_id)
    }

    /// Undo one transaction for the given buffer.
    pub fn undo(
        &mut self,
        buffer_id: BufferId,
        correlation_id: Option<CorrelationId>,
    ) -> Result<TransactionRecord, EditorError> {
        let thresholds = self.thresholds;
        let (
            workspace_id,
            file_id,
            undo_group_id,
            pre_snapshot_descriptor,
            redo_snapshot_descriptor,
            post_snapshot_descriptor,
            undo_snapshot_id,
            delta,
            restored_mode,
        ) = {
            let state = self
                .buffers
                .get_mut(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            let undo_entry = state
                .undo_stack
                .last()
                .cloned()
                .ok_or(EditorError::NothingToUndo)?;
            let pre_snapshot = state.current_snapshot.clone();
            let next_version = BufferVersion(state.buffer.version().0 + 1);
            let restored_mode =
                Self::mode_for_byte_len_with_thresholds(thresholds, undo_entry.snapshot.len());
            let mut restored_buffer = TextBuffer::try_from_rope_with_cache_policy(
                undo_entry.snapshot.rope(),
                next_version,
                matches!(restored_mode, BufferMode::Normal),
            )?;
            restored_buffer.set_version(next_version);
            let restored_snapshot =
                restored_buffer.try_snapshot_with_retention(RetentionPinReason::CurrentBuffer)?;
            let delta = ChangedDelta {
                byte_range: ByteRange::new(0, restored_buffer.len() as u64),
                utf16_range: restored_buffer
                    .line_index()
                    .utf16_range(0, restored_buffer.len())?,
            };
            let restored_snapshot_descriptor = restored_snapshot.descriptor().clone();
            let pre_snapshot_descriptor = pre_snapshot.descriptor().clone();
            let mut redo_snapshot_descriptor = pre_snapshot_descriptor.clone();
            redo_snapshot_descriptor.retention_pin_reason = RetentionPinReason::RedoHistory;
            let undo_snapshot_id = undo_entry.snapshot.snapshot_id();

            state.undo_stack.pop();
            state.redo_stack.push(UndoEntry {
                snapshot: pre_snapshot.clone(),
                carets: state.carets.clone(),
                vertical_layout_id: state.vertical_layout_id,
                undo_group_id: undo_entry.undo_group_id,
            });
            state.buffer = restored_buffer;
            state.mode = restored_mode;
            state.current_snapshot = restored_snapshot;
            state.carets = undo_entry.carets.clone();
            state.vertical_layout_id = undo_entry.vertical_layout_id;
            state.active_undo_group = None;
            state.active_group_evicted = false;
            state.dirty = true;
            state.save_state = if state.conflict_state.is_some() {
                FileConflictLifecycleState::ConflictDirty
            } else {
                FileConflictLifecycleState::Dirty
            };

            (
                state.workspace_id,
                state.file_id,
                undo_entry.undo_group_id,
                pre_snapshot_descriptor,
                redo_snapshot_descriptor,
                restored_snapshot_descriptor,
                undo_snapshot_id,
                delta,
                restored_mode,
            )
        };
        self.release_snapshot_descriptor_if_unreferenced(undo_snapshot_id);
        self.retain_snapshot_descriptor(buffer_id, &redo_snapshot_descriptor);
        self.retain_snapshot_descriptor(buffer_id, &post_snapshot_descriptor);
        self.enforce_snapshot_retention_policy();

        let tx = TransactionRecord {
            transaction_id: Uuid::now_v7(),
            causality_trace_id: Uuid::now_v7(),
            workspace_id,
            buffer_id,
            file_id,
            source: TransactionSource::Restore,
            pre_snapshot: pre_snapshot_descriptor,
            post_snapshot: post_snapshot_descriptor,
            deltas: vec![delta],
            undo_group_id,
            occurred_at: TimestampMillis::now(),
            correlation_id,
        };
        let _ = restored_mode;
        self.transaction_log.push(tx.clone());
        self.enqueue_transaction_event(&tx);
        Ok(tx)
    }

    /// Redo one transaction for the given buffer.
    pub fn redo(
        &mut self,
        buffer_id: BufferId,
        correlation_id: Option<CorrelationId>,
    ) -> Result<TransactionRecord, EditorError> {
        let thresholds = self.thresholds;
        let (
            workspace_id,
            file_id,
            undo_group_id,
            pre_snapshot_descriptor,
            undo_snapshot_descriptor,
            post_snapshot_descriptor,
            redo_snapshot_id,
            delta,
            restored_mode,
        ) = {
            let state = self
                .buffers
                .get_mut(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            let redo_entry = state
                .redo_stack
                .last()
                .cloned()
                .ok_or(EditorError::NothingToRedo)?;
            let pre_snapshot = state.current_snapshot.clone();
            let next_version = BufferVersion(state.buffer.version().0 + 1);
            let restored_mode =
                Self::mode_for_byte_len_with_thresholds(thresholds, redo_entry.snapshot.len());
            let mut restored_buffer = TextBuffer::try_from_rope_with_cache_policy(
                redo_entry.snapshot.rope(),
                next_version,
                matches!(restored_mode, BufferMode::Normal),
            )?;
            restored_buffer.set_version(next_version);
            let restored_snapshot =
                restored_buffer.try_snapshot_with_retention(RetentionPinReason::CurrentBuffer)?;
            let delta = ChangedDelta {
                byte_range: ByteRange::new(0, restored_buffer.len() as u64),
                utf16_range: restored_buffer
                    .line_index()
                    .utf16_range(0, restored_buffer.len())?,
            };
            let restored_snapshot_descriptor = restored_snapshot.descriptor().clone();
            let pre_snapshot_descriptor = pre_snapshot.descriptor().clone();
            let mut undo_snapshot_descriptor = pre_snapshot_descriptor.clone();
            undo_snapshot_descriptor.retention_pin_reason = RetentionPinReason::UndoHistory;
            let redo_snapshot_id = redo_entry.snapshot.snapshot_id();

            state.redo_stack.pop();
            state.undo_stack.push(UndoEntry {
                snapshot: pre_snapshot.clone(),
                carets: state.carets.clone(),
                vertical_layout_id: state.vertical_layout_id,
                undo_group_id: redo_entry.undo_group_id,
            });
            state.buffer = restored_buffer;
            state.mode = restored_mode;
            state.current_snapshot = restored_snapshot;
            state.carets = redo_entry.carets.clone();
            state.vertical_layout_id = redo_entry.vertical_layout_id;
            state.active_undo_group = None;
            state.active_group_evicted = false;
            state.dirty = true;
            state.save_state = if state.conflict_state.is_some() {
                FileConflictLifecycleState::ConflictDirty
            } else {
                FileConflictLifecycleState::Dirty
            };

            (
                state.workspace_id,
                state.file_id,
                redo_entry.undo_group_id,
                pre_snapshot_descriptor,
                undo_snapshot_descriptor,
                restored_snapshot_descriptor,
                redo_snapshot_id,
                delta,
                restored_mode,
            )
        };
        self.release_snapshot_descriptor_if_unreferenced(redo_snapshot_id);
        self.retain_snapshot_descriptor(buffer_id, &undo_snapshot_descriptor);
        self.retain_snapshot_descriptor(buffer_id, &post_snapshot_descriptor);
        self.enforce_snapshot_retention_policy();

        let tx = TransactionRecord {
            transaction_id: Uuid::now_v7(),
            causality_trace_id: Uuid::now_v7(),
            workspace_id,
            buffer_id,
            file_id,
            source: TransactionSource::Restore,
            pre_snapshot: pre_snapshot_descriptor,
            post_snapshot: post_snapshot_descriptor,
            deltas: vec![delta],
            undo_group_id,
            occurred_at: TimestampMillis::now(),
            correlation_id,
        };
        let _ = restored_mode;
        self.transaction_log.push(tx.clone());
        self.enqueue_transaction_event(&tx);
        Ok(tx)
    }

    /// Emit a save request DTO and keep buffer logic decoupled from persistence.
    pub fn request_save(
        &mut self,
        buffer_id: BufferId,
        correlation_id: Option<CorrelationId>,
    ) -> Result<SaveRequestDto, EditorError> {
        let payload = {
            let state = self
                .buffers
                .get_mut(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            let snapshot = state
                .buffer
                .try_snapshot_with_retention(RetentionPinReason::BackgroundSave)?;
            let text = match snapshot.try_full_text() {
                Ok(full_text) => full_text.to_string(),
                Err(TextError::FullCacheBudgetExceeded { .. }) => {
                    snapshot.materialize_full_text_from_chunks()?
                }
                Err(err) => return Err(EditorError::Text(err)),
            };
            let dto = SaveRequestDto {
                request_id: Uuid::now_v7(),
                workspace_id: state.workspace_id,
                buffer_id: state.buffer_id,
                file_id: state.file_id,
                snapshot_id: snapshot.snapshot_id(),
                buffer_version: snapshot.buffer_version(),
                content_hash: snapshot.content_hash().to_string(),
                payload_byte_len: text.len() as u64,
                text,
                requested_at: TimestampMillis::now(),
                correlation_id: correlation_id
                    .unwrap_or_else(|| CorrelationId(TimestampMillis::now().0)),
            };
            SaveSnapshotPayload { snapshot, dto }
        };

        self.retain_snapshot_descriptor(buffer_id, payload.snapshot.descriptor());
        if let Some(state) = self.buffers.get_mut(&buffer_id) {
            state.save_state = FileConflictLifecycleState::Saving;
        }
        self.pending_save_requests.push(payload.dto.clone());
        self.enforce_snapshot_retention_policy();
        Ok(payload.dto)
    }

    /// Mark that a save request completed and clear dirty state only on matching successful snapshots.
    pub fn acknowledge_save(&mut self, request_id: Uuid, success: bool) {
        let acknowledgement = if success {
            SaveAcknowledgement::Saved
        } else {
            SaveAcknowledgement::Failed {
                diagnostics: Vec::new(),
            }
        };
        self.acknowledge_save_outcome(request_id, acknowledgement);
    }

    /// Mark that a save request completed with a typed proposal outcome.
    pub fn acknowledge_save_outcome(
        &mut self,
        request_id: Uuid,
        acknowledgement: SaveAcknowledgement,
    ) {
        if let Some(idx) = self
            .pending_save_requests
            .iter()
            .position(|request| request.request_id == request_id)
        {
            let request = self.pending_save_requests.remove(idx);
            if let Some(state) = self.buffers.get_mut(&request.buffer_id) {
                match acknowledgement {
                    SaveAcknowledgement::Saved => {
                        if state.current_snapshot.snapshot_id() == request.snapshot_id
                            || state.current_snapshot.content_hash() == request.content_hash
                        {
                            state.dirty = false;
                            state.save_state = FileConflictLifecycleState::Clean;
                            state.save_diagnostics.clear();
                            state.conflict_state = None;
                        } else if state.dirty {
                            state.save_state = FileConflictLifecycleState::Dirty;
                        }
                    }
                    SaveAcknowledgement::Stale {
                        conflict,
                        diagnostics,
                    } => {
                        state.dirty = true;
                        state.save_state = FileConflictLifecycleState::ConflictDirty;
                        state.save_diagnostics = diagnostics;
                        state.conflict_state = conflict;
                    }
                    SaveAcknowledgement::Conflict { conflict } => {
                        state.dirty = true;
                        state.save_state = FileConflictLifecycleState::ConflictDirty;
                        state.save_diagnostics = conflict.diagnostics.clone();
                        state.conflict_state = Some(conflict);
                    }
                    SaveAcknowledgement::Denied { diagnostics }
                    | SaveAcknowledgement::Failed { diagnostics } => {
                        state.dirty = true;
                        state.save_state = FileConflictLifecycleState::SaveFailed;
                        state.save_diagnostics = diagnostics;
                    }
                }
            }
            self.release_save_snapshot_if_unreferenced(request.snapshot_id);
        }
    }

    fn release_save_snapshot_if_unreferenced(&mut self, snapshot_id: SnapshotId) {
        self.release_snapshot_descriptor_if_unreferenced(snapshot_id);
    }

    /// Read-only transaction log.
    pub fn transaction_log(&self) -> &[TransactionRecord] {
        &self.transaction_log
    }

    /// Drain already-produced metadata-only transaction descriptors from the bounded event queue.
    pub fn drain_transaction_events(&mut self) -> DrainedTransactionEvents {
        let descriptors = self.transaction_events.drain(..).collect();
        let dropped_before_drain = std::mem::take(&mut self.dropped_transaction_event_count);
        DrainedTransactionEvents {
            descriptors,
            dropped_before_drain,
        }
    }

    /// Read-only pending save queue.
    pub fn pending_save_requests(&self) -> &[SaveRequestDto] {
        &self.pending_save_requests
    }

    /// Current save/conflict lifecycle state for a buffer.
    pub fn buffer_save_state(
        &self,
        buffer_id: BufferId,
    ) -> Result<FileConflictLifecycleState, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .save_state)
    }

    /// Query the latest conflict state captured for a buffer.
    pub fn conflict_state(
        &self,
        buffer_id: BufferId,
    ) -> Result<Option<&FileConflictState>, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .conflict_state
            .as_ref())
    }

    /// Save diagnostics captured for the most recent failed/stale/conflicting save.
    pub fn save_diagnostics(
        &self,
        buffer_id: BufferId,
    ) -> Result<&[ProtocolDiagnostic], EditorError> {
        Ok(&self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .save_diagnostics)
    }

    /// Number of pinned snapshots retained by active undo/redo/save references.
    pub fn pinned_snapshot_count(&self) -> usize {
        self.pinned_snapshot_ids.len()
    }

    /// Number of retained snapshot descriptors tracked by the retention policy.
    pub fn retained_snapshot_count(&self) -> usize {
        self.retained_snapshots.len()
    }

    /// Estimated bytes retained by tracked snapshot descriptors.
    pub fn retained_snapshot_estimated_bytes(&self) -> usize {
        self.retained_snapshot_bytes()
    }

    /// Undo entries retained for a buffer.
    pub fn undo_len(&self, buffer_id: BufferId) -> Result<usize, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .undo_stack
            .len())
    }

    /// Redo entries retained for a buffer.
    pub fn redo_len(&self, buffer_id: BufferId) -> Result<usize, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .redo_stack
            .len())
    }

    /// The buffer's primary cursor.
    ///
    /// The first cursor is the primary one — the same one viewport projection
    /// reports. A buffer is seeded with one at open and the set is never
    /// emptied, so this returns a position rather than an option. Callers that
    /// need every cursor for multi-cursor editing should use
    /// [`cursors`](Self::cursors).
    pub fn primary_cursor(&self, buffer_id: BufferId) -> Result<TextPosition, EditorError> {
        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        Ok(state
            .carets
            .first()
            .map(|caret| caret.head)
            .expect("BufferState is constructed with one cursor and never empties them"))
    }

    /// Resolve an LSP UTF-16 line/character coordinate to the editor's byte
    /// column without materializing the buffer text.
    pub fn protocol_position(
        &self,
        buffer_id: BufferId,
        line: u32,
        character: u32,
    ) -> Result<TextPosition, EditorError> {
        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        let line =
            usize::try_from(line).map_err(|_| EditorError::InvalidEdit("invalid protocol line"))?;
        let character = usize::try_from(character)
            .map_err(|_| EditorError::InvalidEdit("invalid protocol character"))?;
        let absolute_byte = state
            .buffer
            .byte_offset_from_utf16(Utf16Position::new(line, character))?;
        let line_start = state
            .buffer
            .line_index()
            .byte_offset(TextPosition::new(line, 0))?;
        let byte_column = absolute_byte
            .checked_sub(line_start)
            .ok_or(EditorError::InvalidEdit("invalid protocol coordinate"))?;
        Ok(TextPosition::new(line, byte_column))
    }

    /// Whether a buffer's text was streamed from disk.
    ///
    /// Exposed separately from the viewport because a caller deciding what to
    /// offer — a whole-file search, a formatter — needs the answer without
    /// building a projection first.
    pub fn buffer_is_streamed(&self, buffer_id: BufferId) -> Result<bool, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .streamed)
    }

    /// Every cursor for a buffer, in stored order.
    pub fn cursors(&self, buffer_id: BufferId) -> Result<Vec<Cursor>, EditorError> {
        Ok(self
            .directed_carets(buffer_id)?
            .into_iter()
            .map(|caret| Cursor {
                position: caret.head,
            })
            .collect())
    }

    /// Every directed caret for a buffer, in stored order.
    pub fn directed_carets(&self, buffer_id: BufferId) -> Result<Vec<DirectedCaret>, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .carets
            .clone())
    }

    /// Derived normalized selection views for a buffer.
    pub fn selections(&self, buffer_id: BufferId) -> Result<Vec<Selection>, EditorError> {
        let state = self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        state
            .carets
            .iter()
            .filter_map(|caret| caret.anchor.map(|anchor| (anchor, caret.head)))
            .map(|(anchor, head)| {
                let start = state.buffer.try_byte_offset(anchor)?;
                let end = state.buffer.try_byte_offset(head)?;
                Ok(Selection {
                    range: if start <= end {
                        TextRange::new(anchor, head)
                    } else {
                        TextRange::new(head, anchor)
                    },
                })
            })
            .collect()
    }

    /// Replace cursors for a buffer.
    pub fn set_cursors(
        &mut self,
        buffer_id: BufferId,
        cursors: Vec<Cursor>,
    ) -> Result<(), EditorError> {
        if cursors.is_empty() {
            return Err(EditorError::InvalidEdit(
                "buffer must retain at least one caret",
            ));
        }
        self.set_directed_carets(
            buffer_id,
            cursors
                .into_iter()
                .map(|cursor| DirectedCaret::new(cursor.position, None))
                .collect(),
        )
    }

    /// Replace selections for a buffer.
    pub fn set_selections(
        &mut self,
        buffer_id: BufferId,
        selections: Vec<Selection>,
    ) -> Result<(), EditorError> {
        let carets = {
            let state = self
                .buffers
                .get(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            if selections.is_empty() {
                state
                    .carets
                    .iter()
                    .map(|caret| DirectedCaret::new(caret.head, None))
                    .collect()
            } else {
                for selection in &selections {
                    // Validate both endpoints and ordering so projection (which calls
                    // protocol_range on every stored selection) cannot fail on state we
                    // accepted here.
                    let start = state.buffer.try_byte_offset(selection.range.start)?;
                    let end = state.buffer.try_byte_offset(selection.range.end)?;
                    if start > end {
                        return Err(EditorError::InvalidEdit(
                            "selection range start must be <= end",
                        ));
                    }
                }
                selections
                    .into_iter()
                    .map(|selection| {
                        DirectedCaret::new(selection.range.end, Some(selection.range.start))
                    })
                    .collect()
            }
        };
        self.set_directed_carets(buffer_id, carets)
    }

    /// Replace the authoritative directed caret vector atomically.
    pub fn set_directed_carets(
        &mut self,
        buffer_id: BufferId,
        carets: Vec<DirectedCaret>,
    ) -> Result<(), EditorError> {
        if carets.is_empty() {
            return Err(EditorError::InvalidEdit(
                "buffer must retain at least one caret",
            ));
        }
        {
            let state = self
                .buffers
                .get(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            for caret in &carets {
                state.buffer.try_byte_offset(caret.head)?;
                if let Some(anchor) = caret.anchor {
                    state.buffer.try_byte_offset(anchor)?;
                }
            }
        }
        let state = self.buffers.get_mut(&buffer_id).expect("buffer checked");
        state.carets = carets
            .into_iter()
            .map(|caret| DirectedCaret::new(caret.head, caret.anchor).with_affinity(caret.affinity))
            .collect();
        state.vertical_layout_id = None;
        Ok(())
    }

    /// Atomically install visually placed directed carets for a snapshot.
    ///
    /// Visual placement changes only caret state: it does not create a text
    /// transaction, change the buffer version, or add an undo entry. Snapshot
    /// and version checks, followed by endpoint validation, complete before
    /// any authoritative state is changed.
    pub fn set_visual_directed_carets(
        &mut self,
        buffer_id: BufferId,
        expected_snapshot_id: SnapshotId,
        expected_buffer_version: BufferVersion,
        carets: Vec<DirectedCaret>,
    ) -> Result<(), EditorError> {
        if carets.is_empty() {
            return Err(EditorError::InvalidEdit(
                "buffer must retain at least one caret",
            ));
        }
        {
            let state = self
                .buffers
                .get(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            let actual_snapshot_id = state.current_snapshot.snapshot_id();
            let actual_buffer_version = state.current_snapshot.buffer_version();
            if actual_snapshot_id != expected_snapshot_id
                || actual_buffer_version != expected_buffer_version
            {
                return Err(EditorError::StaleVisualCaretPlacement {
                    expected_snapshot_id,
                    actual_snapshot_id,
                    expected_buffer_version,
                    actual_buffer_version,
                });
            }
            for caret in &carets {
                state.buffer.try_byte_offset(caret.head)?;
                if let Some(anchor) = caret.anchor {
                    state.buffer.try_byte_offset(anchor)?;
                }
            }
        }
        let state = self.buffers.get_mut(&buffer_id).expect("buffer checked");
        state.carets = carets
            .into_iter()
            .map(|caret| DirectedCaret::new(caret.head, caret.anchor).with_affinity(caret.affinity))
            .collect();
        state.vertical_layout_id = None;
        Ok(())
    }

    /// Move every caret to a logical line or document boundary.
    ///
    /// Boundary resolution uses the rope line index, so it remains valid for
    /// streamed buffers and preserves each caret's directed anchor when
    /// extending a selection.
    pub fn move_to_boundary(
        &mut self,
        buffer_id: BufferId,
        boundary: BoundaryKind,
        extend: bool,
    ) -> Result<(), EditorError> {
        let targets = {
            let state = self
                .buffers
                .get(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            let line_count = state.buffer.line_index().line_count().max(1);
            let last_line = line_count.saturating_sub(1);
            state
                .carets
                .iter()
                .map(|caret| {
                    let target = match boundary {
                        BoundaryKind::LineStart => TextPosition::new(caret.head.line, 0),
                        BoundaryKind::LineEnd => TextPosition::new(
                            caret.head.line,
                            state.buffer.line_index().line_byte_len(caret.head.line)?,
                        ),
                        BoundaryKind::DocumentStart => TextPosition::zero(),
                        BoundaryKind::DocumentEnd => TextPosition::new(
                            last_line,
                            state.buffer.line_index().line_byte_len(last_line)?,
                        ),
                    };
                    let anchor = extend.then_some(caret.anchor.unwrap_or(caret.head));
                    Ok(DirectedCaret::new(target, anchor))
                })
                .collect::<Result<Vec<_>, EditorError>>()?
        };
        self.set_directed_carets(buffer_id, targets)
    }

    /// Replace the contents of every directed caret range in one transaction.
    ///
    /// A caret without an anchor contributes a zero-width insertion.  Ranges
    /// are built from the rope coordinates and applied atomically, so this is
    /// bounded for streamed buffers and preserves multi-caret editing.
    pub fn replace_directed_carets(
        &mut self,
        buffer_id: BufferId,
        text: impl Into<String>,
        correlation_id: Option<CorrelationId>,
    ) -> Result<TransactionRecord, EditorError> {
        let text = text.into();
        let edits = {
            let state = self
                .buffers
                .get(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            state
                .carets
                .iter()
                .map(|caret| {
                    let head = state.buffer.try_byte_offset(caret.head)?;
                    let anchor = caret
                        .anchor
                        .map(|anchor| state.buffer.try_byte_offset(anchor))
                        .transpose()?;
                    let (start, end) = anchor
                        .map(|anchor| (anchor.min(head), anchor.max(head)))
                        .unwrap_or((head, head));
                    Ok(TextEdit::new(
                        TextRange::new(
                            state.buffer.try_position(start)?,
                            state.buffer.try_position(end)?,
                        ),
                        text.clone(),
                    ))
                })
                .collect::<Result<Vec<_>, EditorError>>()?
        };
        self.apply_edits_with_caret_policy(
            buffer_id,
            edits,
            TransactionSource::User,
            None,
            correlation_id,
            true,
        )
    }

    /// Delete the selected ranges or adjacent extended graphemes for every
    /// directed caret in one transaction.
    ///
    /// Selection ranges retain their exact byte endpoints. Collapsed carets
    /// delete a whole grapheme, including the containing grapheme when the
    /// supplied scalar position is inside one. Ranges are unioned before the
    /// edit is committed so coincident and touching carets remain one atomic
    /// operation. Returns `None` when every caret is a boundary no-op.
    pub fn delete_directed_carets(
        &mut self,
        buffer_id: BufferId,
        direction: DeleteDirection,
        correlation_id: Option<CorrelationId>,
    ) -> Result<Option<TransactionRecord>, EditorError> {
        let ranges = {
            let state = self
                .buffers
                .get(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            let mut ranges = Vec::new();
            for caret in &state.carets {
                let head = state.buffer.try_byte_offset(caret.head)?;
                let anchor = caret
                    .anchor
                    .map(|anchor| state.buffer.try_byte_offset(anchor))
                    .transpose()?;
                if let Some(anchor) = anchor {
                    let (start, end) = (anchor.min(head), anchor.max(head));
                    if start < end {
                        ranges.push((start, end));
                        continue;
                    }
                }

                let (start, end) = match direction {
                    DeleteDirection::Backward => {
                        let Some(previous) = state.buffer.previous_grapheme_boundary(head)? else {
                            continue;
                        };
                        let end = state
                            .buffer
                            .next_grapheme_boundary(previous)?
                            .unwrap_or(head);
                        (previous, end)
                    }
                    DeleteDirection::Forward => {
                        let Some(next) = state.buffer.next_grapheme_boundary(head)? else {
                            continue;
                        };
                        let start = state
                            .buffer
                            .previous_grapheme_boundary(next)?
                            .unwrap_or(head);
                        (start, next)
                    }
                };
                if start < end {
                    ranges.push((start, end));
                }
            }
            ranges.sort_unstable();
            let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
            for (start, end) in ranges {
                if let Some(last) = merged.last_mut()
                    && start <= last.1
                {
                    last.1 = last.1.max(end);
                    continue;
                }
                merged.push((start, end));
            }
            merged
        };

        if ranges.is_empty() {
            return Ok(None);
        }
        let edits = {
            let state = self
                .buffers
                .get(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            ranges
                .into_iter()
                .map(|(start, end)| {
                    Ok(TextEdit::new(
                        TextRange::new(
                            state.buffer.try_position(start)?,
                            state.buffer.try_position(end)?,
                        ),
                        String::new(),
                    ))
                })
                .collect::<Result<Vec<_>, EditorError>>()?
        };
        self.apply_edits_with_caret_policy(
            buffer_id,
            edits,
            TransactionSource::User,
            None,
            correlation_id,
            true,
        )
        .map(Some)
    }

    /// Replace transient overlays for a buffer.
    pub fn set_overlays(
        &mut self,
        buffer_id: BufferId,
        overlays: Vec<UiOverlay>,
    ) -> Result<(), EditorError> {
        let state = self
            .buffers
            .get_mut(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        state.overlays = overlays;
        Ok(())
    }

    /// Move every directed caret across one extended grapheme boundary.
    ///
    /// Movement is resolved against the original buffer and committed only
    /// after every head and anchor has been validated. Plain movement first
    /// collapses a nonempty selection; extending movement retains (or creates)
    /// each caret's anchor.
    pub fn move_horizontally(
        &mut self,
        buffer_id: BufferId,
        direction: HorizontalDirection,
        extend: bool,
    ) -> Result<(), EditorError> {
        let targets = {
            let state = self
                .buffers
                .get(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            state
                .carets
                .iter()
                .map(|caret| {
                    let head = state.buffer.try_byte_offset(caret.head)?;
                    let anchor = caret
                        .anchor
                        .map(|anchor| state.buffer.try_byte_offset(anchor))
                        .transpose()?;
                    let selection = anchor.is_some_and(|anchor| anchor != head);
                    if !extend && selection {
                        let target = match direction {
                            HorizontalDirection::Left => {
                                head.min(anchor.expect("selection anchor"))
                            }
                            HorizontalDirection::Right => {
                                head.max(anchor.expect("selection anchor"))
                            }
                        };
                        return Ok(DirectedCaret::new(state.buffer.try_position(target)?, None));
                    }

                    let target = match direction {
                        HorizontalDirection::Left => {
                            state.buffer.previous_grapheme_boundary(head)?
                        }
                        HorizontalDirection::Right => state.buffer.next_grapheme_boundary(head)?,
                    }
                    .unwrap_or(head);
                    let target = state.buffer.try_position(target)?;
                    let retained_anchor = extend.then_some(caret.anchor.unwrap_or(caret.head));
                    Ok(DirectedCaret::new(target, retained_anchor))
                })
                .collect::<Result<Vec<_>, EditorError>>()?
        };
        self.set_directed_carets(buffer_id, targets)
    }

    /// Move every directed caret across one renderer-shaped visual row.
    ///
    /// All source and target facts are validated against the current buffer
    /// before any caret is changed. The renderer supplies shaped row spans and
    /// stops; the editor chooses the closest stop to each caret's preferred X.
    pub fn move_vertically(
        &mut self,
        buffer_id: BufferId,
        request: VerticalMovementRequest,
    ) -> Result<(), EditorError> {
        const MAX_STOPS_PER_ROW: usize = 4096;
        let (targets, layout_id) = {
            let state = self
                .buffers
                .get(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            if state.current_snapshot.snapshot_id() != request.expected_snapshot_id
                || state.current_snapshot.buffer_version() != request.expected_buffer_version
            {
                return Err(EditorError::StaleVisualCaretPlacement {
                    expected_snapshot_id: request.expected_snapshot_id,
                    actual_snapshot_id: state.current_snapshot.snapshot_id(),
                    expected_buffer_version: request.expected_buffer_version,
                    actual_buffer_version: state.current_snapshot.buffer_version(),
                });
            }
            if !carets_match_vertical_source(&state.carets, &request.expected_carets)
                || request.source_rows.len() != state.carets.len()
                || request.target_rows.len() != state.carets.len()
            {
                return Err(EditorError::InvalidEdit(
                    "vertical source caret or row fact cardinality is stale",
                ));
            }
            let layout_id = request.layout_id;
            let same_layout = state.vertical_layout_id == Some(layout_id);
            let line_count = state.buffer.line_index().line_count().max(1) as u32;
            let mut targets = Vec::with_capacity(state.carets.len());
            for (index, caret) in state.carets.iter().enumerate() {
                let source = &request.source_rows[index];
                let target_row = &request.target_rows[index];
                Self::validate_shaped_row(&state.buffer, &source.row, false, MAX_STOPS_PER_ROW)?;
                Self::validate_shaped_row(&state.buffer, target_row, true, MAX_STOPS_PER_ROW)?;
                let head_offset = state.buffer.try_byte_offset(caret.head)?;
                Self::ensure_grapheme_boundary(&state.buffer, head_offset)?;
                let source_start = state.buffer.try_byte_offset(source.row.start)?;
                let source_end = state.buffer.try_byte_offset(source.row.end)?;
                if caret.head.line as u32 != source.row.logical_line
                    || head_offset < source_start
                    || head_offset > source_end
                {
                    return Err(EditorError::InvalidEdit(
                        "caret is outside its shaped source row",
                    ));
                }
                Self::validate_row_boundary_affinity(
                    &state.buffer,
                    &source.row,
                    caret.head,
                    caret.affinity,
                )?;
                let target_is_same_row = Self::validate_vertical_adjacency(
                    &state.buffer,
                    source,
                    target_row,
                    request.direction,
                    line_count,
                )?;
                let preferred = if same_layout {
                    caret.preferred_x.unwrap_or(source.source_x)
                } else {
                    source.source_x
                };
                let (head, affinity) = if target_is_same_row {
                    if !target_row
                        .stops
                        .iter()
                        .any(|stop| stop.position == caret.head)
                    {
                        return Err(EditorError::InvalidEdit(
                            "document-edge no-op row lacks the source caret stop",
                        ));
                    }
                    (caret.head, caret.affinity)
                } else {
                    let mut stop = target_row
                        .stops
                        .first()
                        .ok_or(EditorError::InvalidEdit("vertical target row has no stops"))?;
                    let mut best_distance = (stop.x.get() - preferred.get()).abs();
                    for candidate in target_row.stops.iter().skip(1) {
                        let distance = (candidate.x.get() - preferred.get()).abs();
                        if distance < best_distance {
                            stop = candidate;
                            best_distance = distance;
                        }
                    }
                    (stop.position, stop.affinity)
                };
                let anchor = request.extend.then_some(caret.anchor.unwrap_or(caret.head));
                targets.push(
                    DirectedCaret::new(head, anchor)
                        .with_affinity(affinity)
                        .with_preferred_x(preferred),
                );
            }
            (targets, layout_id)
        };
        let state = self.buffers.get_mut(&buffer_id).expect("buffer checked");
        state.carets = targets;
        state.vertical_layout_id = Some(layout_id);
        Ok(())
    }

    fn validate_shaped_row(
        buffer: &TextBuffer,
        row: &ShapedVisualRow,
        target: bool,
        max_stops: usize,
    ) -> Result<(), EditorError> {
        if row.row_count == Some(0)
            || matches!((row.row_index, row.row_count), (Some(index), Some(count)) if index >= count)
            || row.row_index.is_some() != row.row_count.is_some()
        {
            return Err(EditorError::InvalidEdit(
                "invalid shaped visual row identity",
            ));
        }
        if target && row.stops.is_empty() {
            return Err(EditorError::InvalidEdit("vertical target row has no stops"));
        }
        if row.stops.len() > max_stops {
            return Err(EditorError::InvalidEdit(
                "vertical target row exceeds stop bound",
            ));
        }
        if row.start.line as u32 != row.logical_line || row.end.line as u32 != row.logical_line {
            return Err(EditorError::InvalidEdit(
                "shaped row spans multiple logical lines",
            ));
        }
        let start = buffer.try_byte_offset(row.start)?;
        let end = buffer.try_byte_offset(row.end)?;
        if start > end {
            return Err(EditorError::InvalidEdit("shaped row start follows its end"));
        }
        for stop in &row.stops {
            if stop.position.line as u32 != row.logical_line {
                return Err(EditorError::InvalidEdit(
                    "vertical stop is on the wrong line",
                ));
            }
            let offset = buffer.try_byte_offset(stop.position)?;
            if offset < start || offset > end {
                return Err(EditorError::InvalidEdit(
                    "vertical stop is outside its row span",
                ));
            }
            Self::ensure_grapheme_boundary(buffer, offset)?;
            Self::validate_row_boundary_affinity(buffer, row, stop.position, stop.affinity)?;
            let _ = stop.x.get();
        }
        Self::ensure_grapheme_boundary(buffer, start)?;
        Self::ensure_grapheme_boundary(buffer, end)?;
        Ok(())
    }

    fn ensure_grapheme_boundary(buffer: &TextBuffer, offset: usize) -> Result<(), EditorError> {
        if offset == 0 || offset == buffer.len() {
            return Ok(());
        }
        let previous = buffer
            .previous_grapheme_boundary(offset)?
            .ok_or(EditorError::InvalidEdit("invalid grapheme boundary"))?;
        if buffer.next_grapheme_boundary(previous)? == Some(offset) {
            Ok(())
        } else {
            Err(EditorError::InvalidEdit("vertical stop splits a grapheme"))
        }
    }

    fn validate_row_boundary_affinity(
        buffer: &TextBuffer,
        row: &ShapedVisualRow,
        position: TextPosition,
        affinity: CaretAffinity,
    ) -> Result<(), EditorError> {
        let position = buffer.try_byte_offset(position)?;
        let start = buffer.try_byte_offset(row.start)?;
        let end = buffer.try_byte_offset(row.end)?;
        let line_end = buffer
            .line_index()
            .line_byte_len(row.logical_line as usize)?;
        if (row.row_index.is_some_and(|index| index > 0) || row.start.column > 0)
            && position == start
            && affinity != CaretAffinity::Downstream
        {
            return Err(EditorError::InvalidEdit(
                "source row start requires downstream affinity",
            ));
        }
        if (row
            .row_index
            .zip(row.row_count)
            .is_some_and(|(index, count)| index + 1 < count)
            || row.end.column < line_end)
            && position == end
            && affinity != CaretAffinity::Upstream
        {
            return Err(EditorError::InvalidEdit(
                "source row end requires upstream affinity",
            ));
        }
        Ok(())
    }

    fn validate_vertical_adjacency(
        buffer: &TextBuffer,
        source: &VerticalSourceRow,
        target: &ShapedVisualRow,
        direction: VerticalDirection,
        line_count: u32,
    ) -> Result<bool, EditorError> {
        let source_line = source.row.logical_line;
        let target_line = target.logical_line;
        if source_line == target_line {
            let contiguous = match direction {
                VerticalDirection::Up => {
                    target.end == source.row.start
                        && target.start != target.end
                        && Self::rows_are_adjacent_if_numbered(&source.row, target)
                }
                VerticalDirection::Down => {
                    target.start == source.row.end
                        && target.start != target.end
                        && Self::rows_are_adjacent_if_numbered(&source.row, target)
                }
            };
            let at_edge = match direction {
                VerticalDirection::Up => {
                    source_line == 0
                        && source.row.start.column == 0
                        && source.row.start == target.start
                        && source.row.end == target.end
                }
                VerticalDirection::Down => {
                    source_line + 1 == line_count
                        && source.row.start == target.start
                        && source.row.end == target.end
                        && source.row.end.column
                            == buffer.line_index().line_byte_len(source_line as usize)?
                }
            };
            if at_edge {
                return Ok(true);
            }
            if contiguous {
                return Ok(false);
            }
            return Err(EditorError::InvalidEdit(
                "vertical target row is not adjacent",
            ));
        }
        let adjacent_logical = match direction {
            VerticalDirection::Up => {
                source_line > 0
                    && target_line + 1 == source_line
                    && Self::source_row_is_first(&source.row)
                    && Self::target_row_is_last(target)
                    && source.row.start.column == 0
                    && target.end.column
                        == buffer.line_index().line_byte_len(target_line as usize)?
            }
            VerticalDirection::Down => {
                target_line == source_line + 1
                    && target_line < line_count
                    && Self::source_row_is_last(&source.row)
                    && Self::target_row_is_first(target)
                    && source.row.end.column
                        == buffer.line_index().line_byte_len(source_line as usize)?
                    && target.start.column == 0
            }
        };
        if adjacent_logical {
            Ok(false)
        } else {
            Err(EditorError::InvalidEdit(
                "vertical target row is not adjacent",
            ))
        }
    }

    fn rows_are_adjacent_if_numbered(source: &ShapedVisualRow, target: &ShapedVisualRow) -> bool {
        match (
            source.row_index,
            source.row_count,
            target.row_index,
            target.row_count,
        ) {
            (Some(source_index), Some(source_count), Some(target_index), Some(target_count)) => {
                source_count == target_count && source_index.abs_diff(target_index) == 1
            }
            (None, None, None, None) => true,
            _ => false,
        }
    }

    fn source_row_is_first(row: &ShapedVisualRow) -> bool {
        row.row_index.is_none_or(|index| index == 0)
    }

    fn source_row_is_last(row: &ShapedVisualRow) -> bool {
        row.row_index
            .zip(row.row_count)
            .is_none_or(|(index, count)| index + 1 == count)
    }

    fn target_row_is_first(row: &ShapedVisualRow) -> bool {
        row.row_index.is_none_or(|index| index == 0)
    }

    fn target_row_is_last(row: &ShapedVisualRow) -> bool {
        row.row_index
            .zip(row.row_count)
            .is_none_or(|(index, count)| index + 1 == count)
    }

    fn mode_for_byte_len(&self, len: usize) -> BufferMode {
        Self::mode_for_byte_len_with_thresholds(self.thresholds, len)
    }

    fn mode_for_byte_len_with_thresholds(thresholds: EditorThresholds, len: usize) -> BufferMode {
        if len > thresholds.large_file_threshold_bytes || len > DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES
        {
            BufferMode::Degraded
        } else {
            BufferMode::Normal
        }
    }

    fn protocol_snapshot_chunk_descriptors(
        snapshot: &legion_text::TextSnapshot,
    ) -> Vec<SnapshotChunkDescriptor> {
        let snapshot_id = snapshot.snapshot_id();
        snapshot
            .chunk_descriptors()
            .iter()
            .map(|chunk| Self::protocol_snapshot_chunk_descriptor(snapshot_id, chunk))
            .collect()
    }

    fn protocol_snapshot_chunk_descriptor(
        snapshot_id: SnapshotId,
        chunk: &legion_text::TextChunkDescriptor,
    ) -> SnapshotChunkDescriptor {
        SnapshotChunkDescriptor {
            snapshot_id,
            chunk_index: chunk.ordinal as u32,
            byte_range: ByteRange::new(chunk.start_byte as u64, chunk.end_byte as u64),
            line_range: LineIndexRange {
                start: chunk.start_line as u32,
                end: chunk.end_line.saturating_add(1) as u32,
            },
            byte_len: chunk.byte_len as u64,
            chunk_hash: Self::protocol_fingerprint(&chunk.hash),
            schema_version: 1,
        }
    }

    fn protocol_utf16_range(
        start: legion_text::Utf16Position,
        end: legion_text::Utf16Position,
    ) -> ProtocolUtf16Range {
        ProtocolUtf16Range {
            start: ProtocolUtf16Position {
                line: start.line as u32,
                character: start.character as u32,
            },
            end: ProtocolUtf16Position {
                line: end.line as u32,
                character: end.character as u32,
            },
        }
    }

    fn chunk_hash_for_line(snapshot: &legion_text::TextSnapshot, line: usize) -> FileFingerprint {
        snapshot
            .chunk_descriptors()
            .iter()
            .find(|chunk| chunk.start_line <= line && line <= chunk.end_line)
            .map(|chunk| Self::protocol_fingerprint(&chunk.hash))
            .unwrap_or_else(|| Self::protocol_fingerprint(snapshot.content_hash()))
    }

    fn protocol_fingerprint(hash: &str) -> FileFingerprint {
        FileFingerprint {
            algorithm: "sha256".to_string(),
            value: hash.strip_prefix("sha256:").unwrap_or(hash).to_string(),
        }
    }

    /// Absolute UTF-16 offset of `byte_offset` from the start of the buffer.
    ///
    /// Delegated to the line index, which answers this in O(log n) against the rope.
    /// This summed `line_utf16_len` and `line_ending_bytes` over every preceding line
    /// until 2026-08-17, which made it O(scroll depth): `viewport_projection` calls it
    /// twice per projection, so a viewport at line 500,000 walked two million line
    /// lookups and cost milliseconds that had nothing to do with the twenty-four lines
    /// being projected. See `plans/evidence/production/WS-MANUAL-02/`.
    fn absolute_utf16_offset(buffer: &TextBuffer, byte_offset: usize) -> Result<u64, EditorError> {
        Ok(buffer.line_index().utf16_offset(byte_offset)? as u64)
    }

    fn completion_byte_offset(
        state: &EditorBufferState,
        position: TextOffset,
    ) -> Result<usize, EditorError> {
        if let Some(byte_offset) = position.as_byte() {
            let offset = usize::try_from(byte_offset.value)
                .map_err(|_| EditorError::InvalidCompletionPosition("byte offset overflow"))?;
            state.buffer.try_position(offset)?;
            return Ok(offset);
        }

        let utf16_offset = position
            .as_utf16()
            .ok_or(EditorError::InvalidCompletionPosition(
                "unsupported text offset encoding",
            ))?
            .value;
        let requested = usize::try_from(utf16_offset)
            .map_err(|_| EditorError::InvalidCompletionPosition("utf16 offset overflow"))?;
        Self::byte_offset_from_absolute_utf16(&state.buffer, requested)
    }

    /// Resolve an absolute UTF-16 offset to a byte offset.
    ///
    /// This walked lines from the start of the buffer until 2026-08-17, subtracting each
    /// line's content and ending lengths. That was O(document length) on the completion
    /// path — and because `completion` resolves the position before it decides it cannot
    /// serve a large file, the walk was longest on exactly the buffers whose result is
    /// then discarded. The line is now found in O(log n).
    ///
    /// The walk also had an off-by-one that the rewrite removes: it could never leave a
    /// residual of zero for any line after the first, because an offset landing on a line
    /// ending was clamped to that line's content end before the next line was considered.
    /// A UTF-16 offset addressing the *start* of a line therefore resolved to the end of
    /// the previous one — and LSP positions are UTF-16, so that was every completion
    /// requested at column 0. `utf16_and_byte_encodings_agree_on_a_line_start` pins it.
    ///
    /// An offset inside a line ending still clamps to the end of that line's content,
    /// which is what `LineIndex::utf16_position` does and what the walk did; and an offset
    /// inside a surrogate pair is still rejected rather than rounded, by
    /// `byte_offset_from_utf16`.
    fn byte_offset_from_absolute_utf16(
        buffer: &TextBuffer,
        requested: usize,
    ) -> Result<usize, EditorError> {
        let line_index = buffer.line_index();
        let (line, within_line) = line_index.utf16_offset_to_line(requested).ok_or(
            EditorError::InvalidCompletionPosition("utf16 offset outside buffer"),
        )?;
        let column = within_line.min(line_index.line_utf16_len(line)?);
        buffer
            .byte_offset_from_utf16(Utf16Position::new(line, column))
            .map_err(EditorError::from)
    }

    fn enqueue_transaction_event(&mut self, record: &TransactionRecord) {
        if self.transaction_events.len() >= self.transaction_event_queue_capacity {
            self.transaction_events.pop_front();
            self.dropped_transaction_event_count =
                self.dropped_transaction_event_count.saturating_add(1);
        }
        self.transaction_events
            .push_back(record.to_protocol_descriptor());
    }

    fn release_snapshot_descriptor_if_unreferenced(&mut self, snapshot_id: SnapshotId) {
        if !self.snapshot_is_referenced(snapshot_id) {
            self.remove_snapshot_descriptor(snapshot_id);
        }
    }

    fn snapshot_is_referenced(&self, snapshot_id: SnapshotId) -> bool {
        self.pending_save_requests
            .iter()
            .any(|request| request.snapshot_id == snapshot_id)
            || self
                .snapshot_leases
                .values()
                .any(|lease| lease.snapshot.snapshot_id() == snapshot_id)
            || self.buffers.values().any(|state| {
                state.current_snapshot.snapshot_id() == snapshot_id
                    || state
                        .undo_stack
                        .iter()
                        .any(|entry| entry.snapshot.snapshot_id() == snapshot_id)
                    || state
                        .redo_stack
                        .iter()
                        .any(|entry| entry.snapshot.snapshot_id() == snapshot_id)
            })
    }
}

const MAX_COMPLETION_ITEMS: usize = 32;
const COMPLETION_SCAN_WINDOW_BYTES: usize = 64 * 1024;
const MAX_COMPLETION_SCAN_IDENTIFIERS: usize = 1024;

fn lexical_completion_items(text: &str, byte_offset: usize) -> Vec<CompletionItem> {
    if !text.is_char_boundary(byte_offset) {
        return Vec::new();
    }
    let prefix_start = identifier_start_before(text, byte_offset);
    let prefix = &text[prefix_start..byte_offset];
    let (scan_start, scan_end) = completion_scan_window(text, byte_offset);
    let mut labels = Vec::new();
    let mut seen = HashSet::new();

    for_each_identifier_range(text, scan_start, scan_end, |start, end| {
        let label = &text[start..end];
        if label.is_empty() || (!prefix.is_empty() && !label.starts_with(prefix)) {
            return true;
        }
        if seen.insert(label.to_string()) {
            labels.push(label.to_string());
        }
        seen.len() < MAX_COMPLETION_SCAN_IDENTIFIERS
    });

    labels.sort_by(|left, right| left.len().cmp(&right.len()).then_with(|| left.cmp(right)));
    labels.truncate(MAX_COMPLETION_ITEMS);
    labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| CompletionItem {
            label: label.clone(),
            detail: Some("editor lexical completion".to_string()),
            insert_text: label,
            kind: "Text".to_string(),
            score: Some((MAX_COMPLETION_ITEMS.saturating_sub(index)) as u32),
            documentation: None,
        })
        .collect()
}

fn completion_scan_window(text: &str, byte_offset: usize) -> (usize, usize) {
    let half_window = COMPLETION_SCAN_WINDOW_BYTES / 2;
    let mut start = byte_offset.saturating_sub(half_window);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = byte_offset.saturating_add(half_window).min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    (start, end)
}

fn for_each_identifier_range(
    text: &str,
    scan_start: usize,
    scan_end: usize,
    mut visit: impl FnMut(usize, usize) -> bool,
) {
    let mut start = None;
    let mut skip_open_identifier = scan_start > 0
        && text[..scan_start]
            .chars()
            .next_back()
            .is_some_and(is_identifier_character);

    for (relative_offset, character) in text[scan_start..scan_end].char_indices() {
        let offset = scan_start + relative_offset;
        if is_identifier_character(character) {
            if !skip_open_identifier {
                start.get_or_insert(offset);
            }
        } else if let Some(start_offset) = start.take() {
            if !visit(start_offset, offset) {
                return;
            }
        } else {
            skip_open_identifier = false;
        }
        if !is_identifier_character(character) {
            skip_open_identifier = false;
        }
    }

    if let Some(start_offset) = start {
        let continues_after_window = scan_end < text.len()
            && text[scan_end..]
                .chars()
                .next()
                .is_some_and(is_identifier_character);
        if !continues_after_window {
            let _ = visit(start_offset, scan_end);
        }
    }
}

fn identifier_start_before(text: &str, byte_offset: usize) -> usize {
    let mut start = byte_offset.min(text.len());
    while start > 0 {
        let Some(previous) = text[..start].chars().next_back() else {
            break;
        };
        if !is_identifier_character(previous) {
            break;
        }
        start -= previous.len_utf8();
    }
    start
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

impl From<SaveRequestDto> for EditorSaveRequest {
    fn from(value: SaveRequestDto) -> Self {
        Self {
            request_id: value.request_id,
            workspace_id: value.workspace_id,
            buffer_id: value.buffer_id,
            file_id: value.file_id,
            snapshot_id: value.snapshot_id,
            buffer_version: value.buffer_version,
            content_hash: value.content_hash,
            payload_byte_len: value.payload_byte_len,
            text: value.text,
            requested_at: value.requested_at,
            correlation_id: value.correlation_id,
        }
    }
}

impl From<EditorSaveRequest> for SaveRequestDto {
    fn from(value: EditorSaveRequest) -> Self {
        Self {
            request_id: value.request_id,
            workspace_id: value.workspace_id,
            buffer_id: value.buffer_id,
            file_id: value.file_id,
            snapshot_id: value.snapshot_id,
            buffer_version: value.buffer_version,
            content_hash: value.content_hash,
            payload_byte_len: value.payload_byte_len,
            text: value.text,
            requested_at: value.requested_at,
            correlation_id: value.correlation_id,
        }
    }
}

impl From<EditorSaveOutcome> for SaveAcknowledgement {
    fn from(value: EditorSaveOutcome) -> Self {
        match value {
            EditorSaveOutcome::Saved => Self::Saved,
            EditorSaveOutcome::Stale {
                conflict,
                diagnostics,
            } => Self::Stale {
                conflict,
                diagnostics,
            },
            EditorSaveOutcome::Conflict { conflict } => Self::Conflict { conflict },
            EditorSaveOutcome::Denied { diagnostics } => Self::Denied { diagnostics },
            EditorSaveOutcome::Failed { diagnostics } => Self::Failed { diagnostics },
        }
    }
}

impl From<SaveAcknowledgement> for EditorSaveOutcome {
    fn from(value: SaveAcknowledgement) -> Self {
        match value {
            SaveAcknowledgement::Saved => Self::Saved,
            SaveAcknowledgement::Stale {
                conflict,
                diagnostics,
            } => Self::Stale {
                conflict,
                diagnostics,
            },
            SaveAcknowledgement::Conflict { conflict } => Self::Conflict { conflict },
            SaveAcknowledgement::Denied { diagnostics } => Self::Denied { diagnostics },
            SaveAcknowledgement::Failed { diagnostics } => Self::Failed { diagnostics },
        }
    }
}

impl EditorEnginePort {
    fn protocol_error(error: EditorError) -> ProtocolError {
        ProtocolError {
            code: "editor_error".to_string(),
            message: error.to_string(),
        }
    }

    fn poisoned_error() -> ProtocolError {
        ProtocolError {
            code: "editor_lock_poisoned".to_string(),
            message: "editor engine lock poisoned".to_string(),
        }
    }

    fn open_buffer_text(
        engine: &mut EditorEngine,
        request: EditorOpenBufferRequest,
    ) -> Result<EditorResponse, EditorError> {
        let buffer_id = engine.open_buffer(
            request.workspace_id,
            request.file_id,
            request.path.0,
            request.initial_text,
        )?;
        Ok(EditorResponse::BufferOpened(BufferOpened {
            project_id: None,
            file_id: Some(request.file_id),
            buffer_id,
        }))
    }

    fn apply_edit(
        engine: &mut EditorEngine,
        request: EditorApplyTransactionRequest,
        event_context: &mut EditorEventContext<'_>,
    ) -> Result<EditorResponse, EditorError> {
        let (workspace_id, file_id) = engine.buffer_identity(request.buffer_id)?;
        if workspace_id != request.workspace_id {
            return Err(EditorError::InvalidEdit(
                "workspace id does not match buffer",
            ));
        }
        if file_id != request.file_id {
            return Err(EditorError::InvalidEdit("file id does not match buffer"));
        }
        let edits = request
            .edits
            .edits
            .into_iter()
            .map(|edit| {
                let range = edit.range.as_byte_range().ok_or(EditorError::InvalidEdit(
                    "editor port apply edit requires byte-coordinate ranges",
                ))?;
                let start = engine
                    .buffers
                    .get(&request.buffer_id)
                    .ok_or(EditorError::BufferNotFound(request.buffer_id))?
                    .buffer
                    .try_position(range.start as usize)?;
                let end = engine
                    .buffers
                    .get(&request.buffer_id)
                    .ok_or(EditorError::BufferNotFound(request.buffer_id))?
                    .buffer
                    .try_position(range.end as usize)?;
                Ok(TextEdit::new(TextRange::new(start, end), edit.replacement))
            })
            .collect::<Result<Vec<_>, EditorError>>()?;
        let record = engine.apply_edits(
            request.buffer_id,
            edits,
            request.source,
            request.undo_group_id,
            Some(request.correlation_id),
        )?;
        event_context.emit_transaction(&record, true, None);
        Ok(EditorResponse::Transaction(record.to_protocol_descriptor()))
    }
}

impl EditorPort for EditorEnginePort {
    fn handle(&self, request: EditorRequest) -> ProtocolResult<EditorResponse> {
        let mut engine = self.engine.lock().map_err(|_| Self::poisoned_error())?;
        let mut next_event_sequence =
            self.next_event_sequence.lock().map_err(|_| ProtocolError {
                code: "editor_event_sequence_lock_poisoned".to_string(),
                message: "editor event sequence lock poisoned".to_string(),
            })?;
        let mut event_context = EditorEventContext {
            event_sink: self.event_sink.as_ref(),
            next_sequence: &mut next_event_sequence,
        };
        match request {
            EditorRequest::OpenBuffer { .. } => Err(ProtocolError::unsupported(
                "workspace-resolved text is required; use OpenBufferText",
            )),
            EditorRequest::OpenBufferText(request) => {
                Self::open_buffer_text(&mut engine, request).map_err(Self::protocol_error)
            }
            EditorRequest::ApplyTransaction(descriptor) => {
                // A `TextTransactionDescriptor` carries only changed-range
                // metadata (no replacement text), so the buffer cannot be
                // mutated from it. Returning `EditorResponse::Transaction`
                // would falsely report an applied transaction. Validate buffer
                // identity for a clear diagnostic, then fail closed and direct
                // callers to `ApplyEdit`, which performs the real mutation.
                let metadata = engine
                    .buffer_metadata(descriptor.buffer_id)
                    .map_err(Self::protocol_error)?;
                if metadata.workspace_id == descriptor.workspace_id
                    && metadata.file_id == descriptor.file_id
                {
                    Err(ProtocolError::unsupported(
                        "ApplyTransaction cannot mutate from a descriptor; use ApplyEdit",
                    ))
                } else {
                    Err(ProtocolError {
                        code: "editor_transaction_mismatch".to_string(),
                        message: "transaction descriptor does not match buffer identity"
                            .to_string(),
                    })
                }
            }
            EditorRequest::ApplyEdit(request) => {
                Self::apply_edit(&mut engine, request, &mut event_context)
                    .map_err(Self::protocol_error)
            }
            EditorRequest::RequestSave {
                buffer_id,
                correlation_id,
            } => engine
                .request_save(buffer_id, Some(correlation_id))
                .map(|save| EditorResponse::SaveRequested(save.into()))
                .map_err(Self::protocol_error),
            EditorRequest::AcknowledgeSave(EditorSaveAcknowledgement {
                request_id,
                outcome,
            }) => {
                let buffer_id = engine
                    .pending_save_requests()
                    .iter()
                    .find(|request| request.request_id == request_id)
                    .map(|request| request.buffer_id);
                engine.acknowledge_save_outcome(request_id, outcome.into());
                Ok(EditorResponse::SaveAcknowledged { buffer_id })
            }
            EditorRequest::Viewport(request) => engine
                .viewport_projection(request)
                .map(EditorResponse::Viewport)
                .map_err(Self::protocol_error),
            EditorRequest::BufferMetadata(buffer_id) => engine
                .buffer_metadata(buffer_id)
                .map(EditorResponse::BufferMetadata)
                .map_err(Self::protocol_error),
            EditorRequest::BufferState(buffer_id) => engine
                .buffer_metadata(buffer_id)
                .map(EditorResponse::BufferState)
                .map_err(Self::protocol_error),
            EditorRequest::Completion(request) => engine
                .completion(request)
                .map(EditorResponse::Completion)
                .map_err(Self::protocol_error),
            EditorRequest::Snapshot(snapshot) => Ok(EditorResponse::Snapshot(snapshot)),
            EditorRequest::Overlay(_overlay) => {
                // Diagnostic overlays are not yet wired into buffer state
                // (`set_overlays` results are never projected), and the request
                // carries no buffer identity to resolve a target. Returning
                // `OverlayApplied` would falsely report a stored overlay, so
                // fail closed until overlay routing is implemented.
                Err(ProtocolError::unsupported(
                    "diagnostic overlay application is not supported by the editor engine",
                ))
            }
        }
    }
}

/// Compatibility-only wrapper around one active buffer.
///
/// `EditorSession` is retained solely as a legacy spike-test shim during the
/// editor-engine migration. New application and UI code must not own or route
/// commands through this wrapper; it must use [`EditorEngine`] through protocol
/// and workspace ports so buffer IDs, file IDs, transactions, saves, and
/// observability metadata remain explicit.
#[derive(Debug)]
pub struct EditorSession {
    engine: EditorEngine,
    active_buffer_id: BufferId,
}

impl EditorSession {
    /// Compatibility constructor for a single-buffer legacy session.
    pub fn open(
        file_path: impl Into<String>,
        project_info: legion_protocol::ProjectInfo,
        initial_text: impl Into<String>,
    ) -> Self {
        let mut engine = EditorEngine::new();
        let buffer_id = engine
            .open_buffer(
                WorkspaceId(project_info.project_id.0),
                project_info.file_id,
                file_path,
                initial_text,
            )
            .expect("open buffer in session should not fail");

        Self {
            engine,
            active_buffer_id: buffer_id,
        }
    }

    /// Compatibility constructor with an ignored legacy buffer id.
    pub fn open_with_buffer_id(
        file_path: impl Into<String>,
        _buffer_id: BufferId,
        project_info: legion_protocol::ProjectInfo,
        initial_text: impl Into<String>,
    ) -> Self {
        Self::open(file_path, project_info, initial_text)
    }

    /// Current editable text.
    pub fn text(&self) -> &str {
        self.engine
            .text(self.active_buffer_id)
            .expect("active buffer should exist")
    }

    /// Current file path for this session.
    pub fn file_path(&self) -> &str {
        self.engine
            .file_path(self.active_buffer_id)
            .expect("active buffer should exist")
    }

    /// Apply an edit and return protocol descriptor metadata.
    pub fn apply_edit(&mut self, edit: TextEdit) -> Result<TextTransactionDescriptor, EditorError> {
        let record = self.engine.apply_edit(
            self.active_buffer_id,
            edit,
            TransactionSource::User,
            None,
            None,
        )?;
        Ok(record.to_protocol_descriptor())
    }

    /// Applies an edit by explicit range + replacement payload.
    pub fn apply_edit_range(
        &mut self,
        start: TextPosition,
        end: TextPosition,
        replacement: impl Into<String>,
    ) -> Result<TextTransactionDescriptor, EditorError> {
        self.apply_edit(TextEdit::new(TextRange::new(start, end), replacement))
    }

    /// Inserts at a byte offset.
    pub fn insert_offset(
        &mut self,
        offset: usize,
        text: impl Into<String>,
    ) -> Result<TextTransactionDescriptor, EditorError> {
        let position = self
            .engine
            .buffers
            .get(&self.active_buffer_id)
            .ok_or(EditorError::BufferNotFound(self.active_buffer_id))?
            .buffer
            .try_position(offset)?;
        self.insert_at(position, text)
    }

    /// Replaces a byte offset range.
    pub fn replace_offset(
        &mut self,
        start: usize,
        end: usize,
        replacement: impl Into<String>,
    ) -> Result<TextTransactionDescriptor, EditorError> {
        let state = self
            .engine
            .buffers
            .get(&self.active_buffer_id)
            .ok_or(EditorError::BufferNotFound(self.active_buffer_id))?;
        let start = state.buffer.try_position(start)?;
        let end = state.buffer.try_position(end)?;
        self.replace_range(TextRange::new(start, end), replacement)
    }

    /// Deletes a byte offset range.
    pub fn delete_offset(
        &mut self,
        start: usize,
        end: usize,
    ) -> Result<TextTransactionDescriptor, EditorError> {
        let state = self
            .engine
            .buffers
            .get(&self.active_buffer_id)
            .ok_or(EditorError::BufferNotFound(self.active_buffer_id))?;
        let start = state.buffer.try_position(start)?;
        let end = state.buffer.try_position(end)?;
        self.delete_range(TextRange::new(start, end))
    }

    /// Inserts at a [`TextPosition`].
    pub fn insert_at(
        &mut self,
        at: TextPosition,
        text: impl Into<String>,
    ) -> Result<TextTransactionDescriptor, EditorError> {
        self.apply_edit(TextEdit::insert(at, text))
    }

    /// Replaces a [`TextRange`].
    pub fn replace_range(
        &mut self,
        range: TextRange,
        replacement: impl Into<String>,
    ) -> Result<TextTransactionDescriptor, EditorError> {
        self.apply_edit(TextEdit::new(range, replacement))
    }

    /// Deletes a [`TextRange`].
    pub fn delete_range(
        &mut self,
        range: TextRange,
    ) -> Result<TextTransactionDescriptor, EditorError> {
        self.apply_edit(TextEdit::delete(range))
    }

    /// Undo the latest mutation.
    pub fn undo(&mut self) -> bool {
        self.engine.undo(self.active_buffer_id, None).is_ok()
    }

    /// Redo the latest undone mutation.
    pub fn redo(&mut self) -> bool {
        self.engine.redo(self.active_buffer_id, None).is_ok()
    }

    /// Number of undo entries available.
    pub fn undo_len(&self) -> usize {
        self.engine
            .buffers
            .get(&self.active_buffer_id)
            .map(|state| state.undo_stack.len())
            .unwrap_or(0)
    }

    /// Number of redo entries available.
    pub fn redo_len(&self) -> usize {
        self.engine
            .buffers
            .get(&self.active_buffer_id)
            .map(|state| state.redo_stack.len())
            .unwrap_or(0)
    }

    /// Snapshot of the current buffer for downstream consumers.
    pub fn snapshot(&self) -> legion_text::TextSnapshot {
        self.engine
            .buffers
            .get(&self.active_buffer_id)
            .expect("active buffer should exist")
            .buffer
            .snapshot_with_retention(RetentionPinReason::CurrentBuffer)
    }

    /// Emit a save request DTO instead of writing directly to disk.
    pub fn request_save(&mut self) -> Result<SaveRequestDto, EditorError> {
        self.engine.request_save(self.active_buffer_id, None)
    }
}

// ── Buffer search state (Phase 4 – Navigation & UI Essentials) ──

/// Standalone buffer search state for find/replace.
///
/// Owned by the app layer, not by `EditorEngine`.  The engine supplies
/// buffer text via `EditorEngine::text()`; this struct runs regex-based
/// matching and tracks navigation state.
#[derive(Debug, Clone, Default)]
pub struct BufferSearchState {
    /// Current query string.
    pub query: String,
    /// Current replacement text.
    pub replace_text: String,
    /// Whether matching is case-sensitive.
    pub case_sensitive: bool,
    /// Whether matching restricts to whole words.
    pub whole_word: bool,
    /// Whether the query is interpreted as a regex pattern.
    pub use_regex: bool,
    /// Whether the find bar is visible.
    pub find_bar_visible: bool,
    /// Whether the replace input is visible.
    pub replace_visible: bool,
    /// Match results as `(start_line, start_col, end_line, end_col)` tuples.
    pub matches: Vec<(u32, u32, u32, u32)>,
    /// Zero-based index of the currently highlighted match.
    pub current_match_index: usize,
}

impl BufferSearchState {
    /// Run the current query against `text` and populate `self.matches`.
    ///
    /// Literal searches with whole-word enabled are wrapped in word
    /// boundaries. Regex searches are matched exactly as authored so the
    /// pattern remains responsible for its own boundaries, anchors, and
    /// groups. An empty query or invalid regex produces zero matches without
    /// panicking.
    pub fn find_matches(&mut self, text: &str) -> usize {
        self.matches.clear();
        self.current_match_index = 0;
        if self.query.is_empty() {
            return 0;
        }

        let pattern = if self.use_regex {
            self.query.clone()
        } else {
            regex::escape(&self.query)
        };
        let pattern = if self.whole_word && !self.use_regex {
            format!(r"\b{}\b", pattern)
        } else {
            pattern
        };

        let regex = match RegexBuilder::new(&pattern)
            .case_insensitive(!self.case_sensitive)
            .build()
        {
            Ok(r) => r,
            Err(_) => return 0,
        };

        let line_starts: Vec<usize> = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();

        let offset_to_line_col = |offset: usize| -> (u32, u32) {
            let line = line_starts.partition_point(|&start| start <= offset) - 1;
            let col = offset - line_starts[line];
            (line as u32, col as u32)
        };

        for m in regex.find_iter(text) {
            let (start_line, start_char) = offset_to_line_col(m.start());
            let (end_line, end_char) = offset_to_line_col(m.end());
            self.matches
                .push((start_line, start_char, end_line, end_char));
        }
        self.matches.len()
    }

    /// Navigate to the next match, wrapping around at the end.
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match_index = (self.current_match_index + 1) % self.matches.len();
        }
    }

    /// Navigate to the previous match, wrapping around at the beginning.
    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            if self.current_match_index == 0 {
                self.current_match_index = self.matches.len() - 1;
            } else {
                self.current_match_index -= 1;
            }
        }
    }

    /// Return the currently highlighted match, if any.
    pub fn current_match(&self) -> Option<(u32, u32, u32, u32)> {
        self.matches.get(self.current_match_index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_observability::{InMemoryEventSink, SharedEventSink};
    use legion_protocol::{
        EditBatch, EditorViewportRequest, ProjectId, ProjectInfo, SnapshotConsumerKind,
        TextEdit as ProtocolTextEdit, TextRange as ProtocolTextRange, ViewportDimensions,
        ViewportProjectionMode, ViewportScroll,
    };
    use quickcheck::quickcheck;

    #[test]
    fn map_edit_offset_keeps_each_zero_width_insert_on_its_own_caret() {
        let edits = [
            PreparedBatchEdit {
                start: 10,
                end: 10,
                new_text: "YY".into(),
            },
            PreparedBatchEdit {
                start: 5,
                end: 5,
                new_text: "X".into(),
            },
        ];
        assert_eq!(map_edit_offset(5, true, &edits), 6);
        assert_eq!(map_edit_offset(10, true, &edits), 13);
        assert_eq!(map_edit_offset(5, false, &edits), 5);
    }

    fn project(file_id: u128) -> ProjectInfo {
        ProjectInfo {
            project_id: ProjectId(1),
            root_path: "root".into(),
            language_id: Some("rust".into()),
            file_id: FileId(file_id),
        }
    }

    #[test]
    fn engine_multi_buffer_lifecycle() {
        let mut engine = EditorEngine::new();
        let a = engine
            .open_buffer(WorkspaceId(1), FileId(10), "src/a.rs", "fn a() {}\n")
            .unwrap();
        let b = engine
            .open_buffer(WorkspaceId(1), FileId(11), "src/b.rs", "fn b() {}\n")
            .unwrap();

        assert_eq!(engine.text(a).unwrap(), "fn a() {}\n");
        assert_eq!(engine.text(b).unwrap(), "fn b() {}\n");

        engine.close_buffer(a).unwrap();
        assert!(matches!(
            engine.text(a),
            Err(EditorError::BufferNotFound(_))
        ));
    }

    #[test]
    fn protocol_position_resolves_utf16_without_materializing_text() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(
                WorkspaceId(1),
                FileId(12),
                "src/unicode.rs",
                "head\n😀tail\n",
            )
            .unwrap();
        let position = engine.protocol_position(buffer, 1, 2).unwrap();
        assert_eq!(position.line, 1);
        assert_eq!(position.column, 4);
        assert!(engine.protocol_position(buffer, 1, 1).is_err());
        assert!(engine.protocol_position(buffer, 99, 0).is_err());
        assert!(engine.protocol_position(buffer, 1, 99).is_err());
    }

    #[test]
    fn protocol_position_handles_crlf_and_large_buffer() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(
                WorkspaceId(1),
                FileId(13),
                "src/large.rs",
                format!("first\r\n😀{}\r\n", "x".repeat(5 * 1024 * 1024)),
            )
            .unwrap();
        let position = engine.protocol_position(buffer, 1, 2).unwrap();
        assert_eq!(position.line, 1);
        assert_eq!(position.column, 4);
        assert!(engine.protocol_position(buffer, 1, 1).is_err());
    }

    #[test]
    fn retention_drained_prefix_releases_each_unpinned_descriptor_and_preserves_lease() {
        // Component-level invariant test: retained descriptor order is made
        // deliberately adversarial because normal chronological operations
        // select the oldest evictable entry and cannot naturally select a
        // later entry while an earlier sibling is still unpinned.
        let policy = SnapshotRetentionPolicy {
            max_snapshot_count: 4,
            max_estimated_bytes: usize::MAX,
            eviction_preference: SnapshotEvictionPreference::UndoThenRedo,
        };
        let mut engine = EditorEngine::with_snapshot_retention_policy(policy);
        let buffer_id = engine
            .open_buffer(
                WorkspaceId(1),
                FileId(991),
                "retention-component.txt",
                "seed",
            )
            .unwrap();
        let (snapshots, carets) = {
            let state = engine.buffers.get(&buffer_id).unwrap();
            let snapshots = (0..4)
                .map(|_| {
                    state
                        .buffer
                        .try_snapshot_with_retention(RetentionPinReason::UndoHistory)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            (snapshots, state.carets.clone())
        };
        let current_id = engine
            .buffers
            .get(&buffer_id)
            .unwrap()
            .current_snapshot
            .snapshot_id();
        let ids = snapshots
            .iter()
            .map(|snapshot| snapshot.snapshot_id())
            .collect::<Vec<_>>();
        let lease_id = Uuid::now_v7();
        let lease_snapshot = snapshots[0].clone();
        // `snapshots[0]` is not the buffer's current snapshot, and the only lease
        // constructor, `EditorEngine::lease_snapshot`, always leases the current
        // one, so this record has to be hand-inserted. Give it the same real
        // time-to-live that constructor gives (`TimestampMillis::now()` plus
        // `DEFAULT_SNAPSHOT_LEASE_TTL_MILLIS`); with a zero TTL the lease expires
        // the instant it is created and
        // `enforce_snapshot_retention_policy`'s opening `sweep_expired_snapshot_leases`
        // removes the only pin holding `ids[0]` whenever the millisecond ticks first.
        let now = TimestampMillis::now();
        let expires_at = TimestampMillis(now.0.saturating_add(DEFAULT_SNAPSHOT_LEASE_TTL_MILLIS));
        engine.snapshot_leases.insert(
            lease_id,
            SnapshotLeaseRecord {
                snapshot: lease_snapshot.clone(),
                descriptor: SnapshotLeaseDescriptor {
                    lease_id,
                    buffer_id,
                    snapshot_id: ids[0],
                    buffer_version: lease_snapshot.buffer_version(),
                    consumer_kind: SnapshotConsumerKind::Ui,
                    expires_at,
                    chunk_count: lease_snapshot.chunk_descriptors().len() as u32,
                    schema_version: 2,
                },
                owned_state: None,
            },
        );
        {
            let state = engine.buffers.get_mut(&buffer_id).unwrap();
            state.undo_stack = snapshots
                .iter()
                .map(|snapshot| UndoEntry {
                    snapshot: snapshot.clone(),
                    carets: carets.clone(),
                    vertical_layout_id: None,
                    undo_group_id: None,
                })
                .collect();
        }
        // The selected id is the later entry while an earlier sibling is
        // unpinned. This forces one drain operation to remove multiple ids.
        for index in [2, 1, 0, 3] {
            let snapshot = &snapshots[index];
            engine.pinned_snapshot_ids.insert(snapshot.snapshot_id());
            engine
                .retained_snapshots
                .push_back(RetainedSnapshotDescriptor {
                    buffer_id,
                    reason: RetentionPinReason::UndoHistory,
                    descriptor: snapshot.descriptor().clone(),
                });
        }
        engine.enforce_snapshot_retention_policy();
        let retained_ids = engine
            .retained_snapshots
            .iter()
            .map(|entry| entry.descriptor.snapshot_id)
            .collect::<Vec<_>>();
        assert_eq!(retained_ids, vec![current_id, ids[0], ids[3]]);
        assert!(
            retained_ids.contains(&ids[0]),
            "lease-pinned older descriptor must survive"
        );
        assert!(
            retained_ids.contains(&ids[3]),
            "remaining suffix descriptor must survive"
        );
        assert!(
            !retained_ids.contains(&ids[1]),
            "unleased drained sibling must be removed"
        );
        assert!(
            !retained_ids.contains(&ids[2]),
            "selected drained descriptor must be removed"
        );
        assert_eq!(
            engine
                .buffers
                .get(&buffer_id)
                .unwrap()
                .undo_stack
                .iter()
                .map(|entry| entry.snapshot.snapshot_id())
                .collect::<Vec<_>>(),
            vec![ids[3]],
            "history remains a contiguous suffix after prefix eviction"
        );
        engine.release_snapshot_lease(lease_id);
        assert!(
            !engine
                .retained_snapshots
                .iter()
                .any(|entry| entry.descriptor.snapshot_id == ids[0])
        );
    }

    #[test]
    fn retention_budget_evicts_oldest_unpinned_undo_snapshots() {
        // Lib-level companion to the integration test of the same name. The
        // integration harness can only observe `retained_snapshot_count()`, so
        // *which* descriptor the budget evicts is asserted here against the
        // private descriptor register.
        let policy = SnapshotRetentionPolicy {
            max_snapshot_count: 4,
            max_estimated_bytes: usize::MAX,
            eviction_preference: SnapshotEvictionPreference::UndoThenRedo,
        };
        let mut engine = EditorEngine::with_snapshot_retention_policy(policy);
        let buffer_id = engine
            .open_buffer(WorkspaceId(1), FileId(992), "retention-budget.txt", "seed")
            .expect("open buffer");
        let mut post_edit_ids = Vec::new();
        for _ in 0..8 {
            engine
                .apply_edit(
                    buffer_id,
                    TextEdit::insert(TextPosition::new(0, 0), "x"),
                    TransactionSource::User,
                    None,
                    None,
                )
                .expect("edit under retention pressure");
            post_edit_ids.push(
                engine
                    .current_snapshot(buffer_id)
                    .expect("current snapshot")
                    .snapshot_id,
            );
        }
        let current_id = *post_edit_ids.last().expect("eight edits were applied");
        let retained_ids = engine
            .retained_snapshots
            .iter()
            .map(|entry| entry.descriptor.snapshot_id)
            .collect::<Vec<_>>();
        assert!(
            retained_ids.len() <= 4,
            "retention budget must bound the descriptor register: {retained_ids:?}"
        );
        assert!(
            retained_ids.contains(&current_id),
            "the current snapshot is never evictable: {retained_ids:?}"
        );
        assert!(
            !retained_ids.contains(&post_edit_ids[0]),
            "the oldest unpinned undo snapshot is evicted first: {retained_ids:?}"
        );
        assert!(
            !engine.pinned_snapshot_ids.contains(&post_edit_ids[0]),
            "evicting a descriptor also drops its retention pin"
        );
        assert!(
            engine
                .retained_snapshots
                .iter()
                .all(|entry| entry.buffer_id == buffer_id),
            "eviction must not leave descriptors attributed to another buffer"
        );
        assert!(engine.undo_len(buffer_id).expect("undo len") <= 3);
        assert_eq!(engine.text(buffer_id).expect("text"), "xxxxxxxxseed");
    }

    #[test]
    fn current_and_pending_save_snapshots_remain_pinned_under_retention_pressure() {
        // Lib-level companion to the integration test of the same name. The
        // integration harness asserts `pinned_snapshot_count() >= 2`; this one
        // asserts the *identity* of the two survivors and that the pin predicate
        // `is_snapshot_pinned` is what keeps them, since the budget of 2 is
        // already saturated by them alone.
        let policy = SnapshotRetentionPolicy {
            max_snapshot_count: 2,
            max_estimated_bytes: usize::MAX,
            eviction_preference: SnapshotEvictionPreference::UndoThenRedo,
        };
        let mut engine = EditorEngine::with_snapshot_retention_policy(policy);
        let buffer_id = engine
            .open_buffer(WorkspaceId(1), FileId(993), "pins-component.txt", "seed")
            .expect("open buffer");
        engine
            .apply_edit(
                buffer_id,
                TextEdit::insert(TextPosition::new(0, 4), "!"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("edit before save");
        let pending_snapshot_id = engine
            .request_save(buffer_id, None)
            .expect("request save")
            .snapshot_id;
        for _ in 0..8 {
            engine
                .apply_edit(
                    buffer_id,
                    TextEdit::insert(TextPosition::new(0, 0), "x"),
                    TransactionSource::User,
                    None,
                    None,
                )
                .expect("edit under retention pressure");
        }
        let current_id = engine
            .current_snapshot(buffer_id)
            .expect("current snapshot")
            .snapshot_id;
        assert_ne!(
            current_id, pending_snapshot_id,
            "the pending save must be an older snapshot than the current one"
        );
        let retained_ids = engine
            .retained_snapshots
            .iter()
            .map(|entry| entry.descriptor.snapshot_id)
            .collect::<Vec<_>>();
        assert!(
            retained_ids.contains(&pending_snapshot_id),
            "the pending save descriptor survives a budget of two: {retained_ids:?}"
        );
        assert!(
            retained_ids.contains(&current_id),
            "the current descriptor survives a budget of two: {retained_ids:?}"
        );
        assert!(
            engine.is_snapshot_pinned(pending_snapshot_id),
            "the pending save request is what pins the older descriptor"
        );
        assert!(
            engine.is_snapshot_pinned(current_id),
            "being a buffer's current snapshot is what pins the newest descriptor"
        );
        assert!(
            engine
                .pending_save_requests()
                .iter()
                .any(|request| request.snapshot_id == pending_snapshot_id),
            "the save request is still pending after eight further edits"
        );
    }

    #[test]
    fn engine_preserves_multiple_cursors_and_selections_in_projection() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(
                WorkspaceId(1),
                FileId(12),
                "src/multi.rs",
                "alpha beta gamma\n",
            )
            .unwrap();

        engine
            .set_cursors(
                buffer,
                vec![
                    Cursor {
                        position: TextPosition::new(0, 2),
                    },
                    Cursor {
                        position: TextPosition::new(0, 8),
                    },
                ],
            )
            .unwrap();
        engine
            .set_selections(
                buffer,
                vec![
                    Selection {
                        range: TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5)),
                    },
                    Selection {
                        range: TextRange::new(TextPosition::new(0, 6), TextPosition::new(0, 10)),
                    },
                ],
            )
            .unwrap();

        let state = engine
            .buffers
            .get(&buffer)
            .expect("buffer state should exist");
        assert_eq!(state.carets.len(), 2);
        assert_eq!(state.carets[0].head, TextPosition::new(0, 5));
        assert_eq!(state.carets[1].head, TextPosition::new(0, 10));
        assert_eq!(state.carets[0].anchor, Some(TextPosition::new(0, 0)));
        assert_eq!(state.carets[1].anchor, Some(TextPosition::new(0, 6)));

        let projection = engine
            .viewport_projection(EditorViewportRequest {
                buffer_id: buffer,
                scroll: ViewportScroll {
                    top_line: 0,
                    left_column: 0,
                },
                dimensions: ViewportDimensions {
                    width_px: 800,
                    height_px: 16,
                },
            })
            .expect("viewport projection");

        assert_eq!(projection.cursor.line, 0);
        // Nonempty selection replacement is authoritative and its forward
        // endpoints become the caret heads.
        assert_eq!(projection.cursor.character, 5);
        assert_eq!(projection.selections.len(), 2);
        assert_eq!(projection.selections[0].start.line, 0);
        assert_eq!(projection.selections[0].start.character, 0);
        assert_eq!(projection.selections[1].end.character, 10);
    }

    #[test]
    fn transaction_has_pre_post_snapshots_and_causality() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(20), "main.rs", "hello")
            .unwrap();
        let tx = engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 5), " world"),
                TransactionSource::User,
                Some(Uuid::now_v7()),
                Some(CorrelationId(99)),
            )
            .unwrap();

        assert_ne!(tx.pre_snapshot.snapshot_id, tx.post_snapshot.snapshot_id);
        assert!(!tx.deltas.is_empty());
        assert_ne!(tx.transaction_id, tx.causality_trace_id);
        assert_eq!(engine.transaction_log().len(), 1);
    }

    #[test]
    fn transaction_log_keeps_snapshot_anchors_through_arbitrary_edits() {
        fn prop(seed: String, edits: Vec<(u8, u8, String)>) -> bool {
            let seed: String = seed
                .chars()
                .filter(|c| c.is_ascii() && *c != '\0' && *c != '\n' && *c != '\r')
                .take(96)
                .collect();
            let mut engine = EditorEngine::new();
            let buffer = engine
                .open_buffer(WorkspaceId(1), FileId(20), "main.rs", seed.as_str())
                .unwrap();

            let mut model = seed;
            let mut expected_version = BufferVersion(0);
            let mut correlation_seed = 1_u64;

            for (start_seed, end_seed, replacement_seed) in edits.into_iter().take(12) {
                let before = engine.current_snapshot(buffer).unwrap().clone();
                assert_eq!(before.buffer_version, expected_version);
                assert_eq!(engine.text(buffer).unwrap(), model);

                let len = model.len();
                let start = if len == 0 {
                    0
                } else {
                    (start_seed as usize) % (len + 1)
                };
                let end = if len == 0 {
                    0
                } else {
                    (end_seed as usize) % (len + 1)
                };
                let (start, end) = if start <= end {
                    (start, end)
                } else {
                    (end, start)
                };
                let replacement: String = replacement_seed
                    .chars()
                    .filter(|c| c.is_ascii() && *c != '\0' && *c != '\n' && *c != '\r')
                    .take(24)
                    .collect();
                let replacement = if replacement.is_empty() {
                    "x".to_string()
                } else {
                    replacement
                };

                model.replace_range(start..end, &replacement);
                let tx = engine
                    .apply_edit(
                        buffer,
                        TextEdit::new(
                            TextRange::new(TextPosition::new(0, start), TextPosition::new(0, end)),
                            replacement,
                        ),
                        TransactionSource::User,
                        Some(Uuid::now_v7()),
                        Some(CorrelationId(correlation_seed)),
                    )
                    .unwrap();
                correlation_seed = correlation_seed.saturating_add(1);

                assert_eq!(tx.pre_snapshot.snapshot_id, before.snapshot_id);
                assert_eq!(tx.pre_snapshot.buffer_version, expected_version);
                assert_eq!(tx.pre_snapshot.content_hash, before.content_hash);
                assert_eq!(
                    tx.post_snapshot.buffer_version,
                    BufferVersion(expected_version.0 + 1)
                );
                assert_eq!(
                    tx.post_snapshot,
                    engine.current_snapshot(buffer).unwrap().clone()
                );
                assert_eq!(engine.text(buffer).unwrap(), model);
                expected_version = BufferVersion(expected_version.0 + 1);
            }

            let log = engine.transaction_log();
            if let Some(first) = log.first() {
                assert_eq!(first.pre_snapshot.buffer_version, BufferVersion(0));
                assert_eq!(
                    first.pre_snapshot.snapshot_id,
                    log[0].pre_snapshot.snapshot_id
                );
            }

            true
        }

        quickcheck(prop as fn(String, Vec<(u8, u8, String)>) -> bool);
    }

    #[test]
    fn collaboration_participant_edit_uses_editor_transaction_authority() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(20), "main.rs", "hello")
            .unwrap();
        let tx = engine
            .apply_protocol_edits(EditorApplyTransactionRequest {
                workspace_id: WorkspaceId(1),
                buffer_id: buffer,
                file_id: FileId(20),
                edits: EditBatch {
                    edits: vec![ProtocolTextEdit {
                        range: ProtocolTextRange::byte(5, 5),
                        replacement: " collab".to_string(),
                    }],
                },
                source: TransactionSource::CollaborationParticipant {
                    session_id: legion_protocol::CollaborationSessionId(1001),
                    participant_id: legion_protocol::CollaborationParticipantId(2001),
                    operation_id: legion_protocol::CollaborationOperationId(3001),
                },
                undo_group_id: Some(Uuid::now_v7()),
                correlation_id: CorrelationId(99),
            })
            .unwrap();

        assert_eq!(engine.text(buffer).unwrap(), "hello collab");
        let descriptor = tx.to_protocol_descriptor();
        match descriptor.source {
            TransactionSource::CollaborationParticipant {
                session_id,
                participant_id,
                operation_id,
            } => {
                assert_eq!(session_id, legion_protocol::CollaborationSessionId(1001));
                assert_eq!(
                    participant_id,
                    legion_protocol::CollaborationParticipantId(2001)
                );
                assert_eq!(
                    operation_id,
                    legion_protocol::CollaborationOperationId(3001)
                );
            }
            other => panic!("unexpected transaction source: {other:?}"),
        }
        assert_eq!(descriptor.correlation_id, CorrelationId(99));
        assert!(descriptor.undo_group_id.is_some());
        assert_eq!(descriptor.pre_buffer_version, BufferVersion(0));
        assert_eq!(descriptor.post_buffer_version, BufferVersion(1));
    }

    #[test]
    fn compatibility_session_undo_redo_invariants() {
        let mut session = EditorSession::open("src/main.rs", project(7), "hello");
        session
            .insert_at(TextPosition::new(0, 5), " world")
            .expect("insert should succeed");
        assert_eq!(session.text(), "hello world");
        assert!(session.undo());
        assert_eq!(session.text(), "hello");
        assert!(session.redo());
        assert_eq!(session.text(), "hello world");
    }

    #[test]
    fn compatibility_session_save_request_is_decoupled_from_disk_writes() {
        let mut session = EditorSession::open("src/main.rs", project(8), "hello");
        session
            .insert_at(TextPosition::new(0, 5), "!")
            .expect("insert should succeed");

        let save = session.request_save().expect("save request should emit");
        assert_eq!(save.text, "hello!");
        assert!(save.content_hash.starts_with("sha256:"));
    }

    #[test]
    fn deterministic_log_order_with_sequential_transactions() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(30), "lib.rs", "abc")
            .unwrap();
        for _ in 0..4 {
            engine
                .apply_edit(
                    buffer,
                    TextEdit::insert(TextPosition::new(0, 0), "x"),
                    TransactionSource::User,
                    None,
                    None,
                )
                .unwrap();
        }

        assert_eq!(engine.transaction_log().len(), 4);
        for pair in engine.transaction_log().windows(2) {
            assert_ne!(pair[0].transaction_id, pair[1].transaction_id);
        }
        for record in engine.transaction_log() {
            let descriptor = record.to_protocol_descriptor();
            assert_ne!(descriptor.correlation_id.0, 0);
            assert_ne!(descriptor.causality_id.0, Uuid::nil());
        }
    }

    #[test]
    fn editor_port_emits_non_zero_transaction_event_for_routed_edit() {
        let sink = InMemoryEventSink::new();
        let port = EditorEnginePort::with_event_sink(
            EditorEngine::new(),
            Box::new(SharedEventSink::new(sink.clone())),
        );
        let opened = port
            .handle(EditorRequest::OpenBufferText(EditorOpenBufferRequest {
                workspace_id: WorkspaceId(1),
                file_id: FileId(2),
                path: CanonicalPath("src/lib.rs".to_string()),
                initial_text: "abc".to_string(),
                correlation_id: CorrelationId(7),
            }))
            .expect("open buffer through editor port");
        let buffer_id = match opened {
            EditorResponse::BufferOpened(opened) => opened.buffer_id,
            other => panic!("expected buffer opened, got {other:?}"),
        };

        let response = port
            .handle(EditorRequest::ApplyEdit(EditorApplyTransactionRequest {
                workspace_id: WorkspaceId(1),
                buffer_id,
                file_id: FileId(2),
                edits: EditBatch {
                    edits: vec![ProtocolTextEdit {
                        range: ProtocolTextRange::byte(3, 3),
                        replacement: "!".to_string(),
                    }],
                },
                source: TransactionSource::User,
                undo_group_id: None,
                correlation_id: CorrelationId(42),
            }))
            .expect("apply edit through editor port");
        let descriptor = match response {
            EditorResponse::Transaction(descriptor) => descriptor,
            other => panic!("expected transaction response, got {other:?}"),
        };

        let events = sink.events().expect("editor transaction event");
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.event, "editor.transaction_applied");
        assert_eq!(event.correlation_id, CorrelationId(42));
        assert_eq!(event.causality_id, descriptor.causality_id);
        assert_ne!(event.correlation_id.0, 0);
        assert_ne!(event.causality_id.0, Uuid::nil());
        assert_ne!(event.sequence.0, 0);
    }

    #[test]
    fn editor_port_completion_returns_bounded_lexical_items_without_mutation() {
        let source = "fn print_value() {}\nfn main() {\n    pri\n}\n";
        let port = EditorEnginePort::new(EditorEngine::new());
        let opened = port
            .handle(EditorRequest::OpenBufferText(EditorOpenBufferRequest {
                workspace_id: WorkspaceId(1),
                file_id: FileId(2),
                path: CanonicalPath("src/lib.rs".to_string()),
                initial_text: source.to_string(),
                correlation_id: CorrelationId(7),
            }))
            .expect("open buffer through editor port");
        let buffer_id = match opened {
            EditorResponse::BufferOpened(opened) => opened.buffer_id,
            other => panic!("expected buffer opened, got {other:?}"),
        };
        let metadata = match port
            .handle(EditorRequest::BufferMetadata(buffer_id))
            .expect("buffer metadata")
        {
            EditorResponse::BufferMetadata(metadata) => metadata,
            other => panic!("expected buffer metadata, got {other:?}"),
        };
        let completion_offset = source.rfind("pri").expect("completion prefix") + 3;

        let completion = port
            .handle(EditorRequest::Completion(CompletionRequest {
                workspace_id: WorkspaceId(1),
                file_id: FileId(2),
                snapshot_id: metadata.snapshot_id,
                position: TextOffset::byte(completion_offset as u64),
                correlation_id: CorrelationId(42),
            }))
            .expect("completion through editor port");
        let completion = match completion {
            EditorResponse::Completion(completion) => completion,
            other => panic!("expected completion response, got {other:?}"),
        };

        assert_eq!(completion.correlation_id, CorrelationId(42));
        assert!(completion.items.len() <= 32);
        assert!(
            completion
                .items
                .iter()
                .any(|item| item.label == "print_value"
                    && item.insert_text == "print_value"
                    && item.detail.as_deref() == Some("editor lexical completion"))
        );
        let editor = port.into_inner().expect("editor engine");
        assert_eq!(editor.text(buffer_id).expect("editor text"), source);
    }

    #[test]
    fn lexical_completion_rejects_invalid_byte_offsets_without_panic() {
        assert!(lexical_completion_items("a🦀b", 2).is_empty());
        assert!(lexical_completion_items("abc", 4).is_empty());
    }

    #[test]
    fn lexical_completion_scan_is_bounded_around_cursor() {
        let padding = "x\n".repeat(COMPLETION_SCAN_WINDOW_BYTES);
        let source = format!("local_far\n{padding}\nlocal_near loc");
        let items = lexical_completion_items(&source, source.len());
        let labels = items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();

        assert!(labels.contains(&"local_near"));
        assert!(!labels.contains(&"local_far"));
        assert!(items.len() <= MAX_COMPLETION_ITEMS);
    }

    #[test]
    fn editor_port_completion_rejects_stale_snapshot() {
        let source = "fn print_value() {}\nfn main() {\n    pri\n}\n";
        let port = EditorEnginePort::new(EditorEngine::new());
        let opened = port
            .handle(EditorRequest::OpenBufferText(EditorOpenBufferRequest {
                workspace_id: WorkspaceId(1),
                file_id: FileId(2),
                path: CanonicalPath("src/lib.rs".to_string()),
                initial_text: source.to_string(),
                correlation_id: CorrelationId(7),
            }))
            .expect("open buffer through editor port");
        let buffer_id = match opened {
            EditorResponse::BufferOpened(opened) => opened.buffer_id,
            other => panic!("expected buffer opened, got {other:?}"),
        };
        let metadata = match port
            .handle(EditorRequest::BufferMetadata(buffer_id))
            .expect("buffer metadata")
        {
            EditorResponse::BufferMetadata(metadata) => metadata,
            other => panic!("expected buffer metadata, got {other:?}"),
        };

        let error = port
            .handle(EditorRequest::Completion(CompletionRequest {
                workspace_id: WorkspaceId(1),
                file_id: FileId(2),
                snapshot_id: SnapshotId(metadata.snapshot_id.0 + 1),
                position: TextOffset::byte(3),
                correlation_id: CorrelationId(42),
            }))
            .expect_err("stale snapshot is rejected");

        assert_eq!(error.code, "editor_error");
        assert!(error.message.contains("completion snapshot"));
        assert!(error.message.contains("stale"));
    }

    #[test]
    fn degraded_completion_returns_empty_without_full_text_materialization() {
        let mut engine = EditorEngine::with_thresholds(EditorThresholds {
            large_file_threshold_bytes: 32,
            retention_budget_snapshots: 8,
        });
        let text = format!("fn print_value() {{}}\n{}\n", "x".repeat(128));
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(2), "big.rs", text)
            .expect("open degraded buffer");
        let metadata = engine.buffer_metadata(buffer).expect("buffer metadata");

        let completion = engine
            .completion(CompletionRequest {
                workspace_id: WorkspaceId(1),
                file_id: FileId(2),
                snapshot_id: metadata.snapshot_id,
                position: TextOffset::byte(3),
                correlation_id: CorrelationId(42),
            })
            .expect("degraded completion fails closed to empty");

        assert!(completion.items.is_empty());
        assert!(matches!(
            engine.text(buffer),
            Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
        ));
    }

    #[test]
    fn degraded_viewport_projection_is_bounded_and_metadata_only() {
        let mut engine = EditorEngine::with_thresholds(EditorThresholds {
            large_file_threshold_bytes: 32,
            retention_budget_snapshots: 8,
        });
        let text = format!("line zero\ncursor🙂line\n{}\ntail\n", "x".repeat(128));
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(40), "big.rs", text)
            .expect("open degraded buffer");

        assert_eq!(
            engine.buffer_mode(buffer).expect("mode"),
            BufferMode::Degraded
        );
        assert!(matches!(
            engine.text(buffer),
            Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
        ));

        engine
            .set_cursors(
                buffer,
                vec![Cursor {
                    position: TextPosition::new(1, 10),
                }],
            )
            .expect("set cursor");
        engine
            .set_selections(
                buffer,
                vec![Selection {
                    range: TextRange::new(TextPosition::new(1, 0), TextPosition::new(1, 10)),
                }],
            )
            .expect("set selection");

        let projection = engine
            .viewport_projection(EditorViewportRequest {
                buffer_id: buffer,
                scroll: ViewportScroll {
                    top_line: 1,
                    left_column: 0,
                },
                dimensions: ViewportDimensions {
                    width_px: 800,
                    height_px: 32,
                },
            })
            .expect("viewport projection");

        assert_eq!(projection.mode, ViewportProjectionMode::DegradedLargeFile);
        assert_eq!(
            projection.snapshot_id,
            engine
                .current_snapshot(buffer)
                .expect("snapshot")
                .snapshot_id
        );
        assert_eq!(projection.visible_range.start.line, 1);
        assert_eq!(projection.cursor.line, 1);
        assert_eq!(projection.cursor.character, 10);
        assert!(projection.cursor.utf16_offset.is_some());
        assert_eq!(projection.selections.len(), 1);
        assert_eq!(projection.line_slices.len(), projection.line_metrics.len());
        assert!(!projection.line_slices.is_empty());
        assert!(
            projection
                .line_slices
                .iter()
                .all(|slice| slice.chunk_hash.algorithm == "sha256"
                    && !slice.chunk_hash.value.is_empty())
        );
        assert!(projection.line_metrics.iter().all(|metric| metric.exact));
        assert!(projection.decoration_spans.is_empty());
        assert!(projection.fold_ranges.is_empty());
        assert!(projection.semantic_token_overlays.is_empty());

        let status = projection.large_file_status.expect("large-file status");
        assert_eq!(status.threshold_bytes, 32);
        assert!(status.message.contains("degraded mode"));

        let chunks = engine
            .snapshot_chunk_descriptors(buffer)
            .expect("chunk descriptors");
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].snapshot_id, projection.snapshot_id);
    }

    #[test]
    fn coordinate_conversion_avoids_full_text_for_degraded_buffers() {
        let mut engine = EditorEngine::with_thresholds(EditorThresholds {
            large_file_threshold_bytes: 8,
            retention_budget_snapshots: 8,
        });
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(41), "emoji.txt", "a🦀b\nrest")
            .expect("open degraded buffer");
        engine
            .set_cursors(
                buffer,
                vec![Cursor {
                    position: TextPosition::new(0, 5),
                }],
            )
            .expect("set cursor");

        assert!(matches!(
            engine.text(buffer),
            Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
        ));

        let projection = engine
            .viewport_projection(EditorViewportRequest {
                buffer_id: buffer,
                scroll: ViewportScroll {
                    top_line: 0,
                    left_column: 0,
                },
                dimensions: ViewportDimensions {
                    width_px: 800,
                    height_px: 16,
                },
            })
            .expect("viewport projection");

        assert_eq!(projection.cursor.line, 0);
        assert_eq!(projection.cursor.character, 5);
        assert_eq!(projection.cursor.utf16_offset, Some(3));
    }

    #[test]
    fn snapshot_leases_are_descriptor_only_for_all_consumer_kinds() {
        let mut engine = EditorEngine::with_thresholds(EditorThresholds {
            large_file_threshold_bytes: 16,
            retention_budget_snapshots: 8,
        });
        let buffer = engine
            .open_buffer(
                WorkspaceId(1),
                FileId(42),
                "leases.txt",
                "0123456789abcdef0123456789",
            )
            .expect("open buffer");
        let snapshot_id = engine
            .current_snapshot(buffer)
            .expect("snapshot")
            .snapshot_id;
        let consumers = [
            SnapshotConsumerKind::Editor,
            SnapshotConsumerKind::Ui,
            SnapshotConsumerKind::Lsp,
            SnapshotConsumerKind::Index,
            SnapshotConsumerKind::Plugin,
            SnapshotConsumerKind::Ai,
            SnapshotConsumerKind::Collaboration,
            SnapshotConsumerKind::Storage,
            SnapshotConsumerKind::Observability,
        ];
        let leases = consumers
            .into_iter()
            .map(|consumer_kind| {
                let lease = engine
                    .lease_snapshot(buffer, consumer_kind)
                    .expect("lease snapshot");
                assert_eq!(lease.snapshot_id, snapshot_id);
                assert_eq!(lease.buffer_id, buffer);
                assert_eq!(lease.buffer_version, BufferVersion(0));
                assert_eq!(lease.consumer_kind, consumer_kind);
                assert!(lease.chunk_count >= 1);
                assert_eq!(lease.schema_version, 2);
                lease
            })
            .collect::<Vec<_>>();

        let retained_before_edit = engine.retained_snapshot_count();
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 0), "!"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("edit while leased");
        assert!(engine.retained_snapshot_count() >= retained_before_edit);

        for lease in leases {
            let released = engine
                .release_snapshot_lease(lease.lease_id)
                .expect("release lease");
            assert_eq!(released.lease_id, lease.lease_id);
            assert_eq!(released.snapshot_id, snapshot_id);
        }
    }

    #[test]
    fn snapshot_lease_consumer_reads_valid_bounded_chunk_by_descriptor() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(
                WorkspaceId(1),
                FileId(44),
                "chunked.rs",
                format!("head\n{}\ntail\n", "x".repeat(128 * 1024)),
            )
            .expect("open buffer");
        let lease = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Index)
            .expect("lease snapshot");

        let chunk = engine
            .read_snapshot_lease_chunk(
                lease.lease_id,
                lease.buffer_id,
                lease.snapshot_id,
                lease.buffer_version,
                0,
            )
            .expect("read leased chunk");

        assert_eq!(chunk.lease, lease);
        assert_eq!(chunk.chunk.snapshot_id, chunk.lease.snapshot_id);
        assert_eq!(chunk.chunk.chunk_index, 0);
        assert_eq!(chunk.text.len() as u64, chunk.chunk.byte_len);
        assert!(chunk.text.len() < engine.current_snapshot(buffer).unwrap().byte_len);
        assert_eq!(chunk.schema_version, 1);
    }

    #[test]
    fn snapshot_lease_reads_huge_line_in_bounded_absolute_chunks() {
        let mut engine = EditorEngine::new();
        let text = "🦀".repeat(3 * 1024 * 1024);
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(48), "huge-line.rs", text)
            .expect("open huge buffer");
        let lease = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Editor)
            .expect("lease snapshot");
        assert!(matches!(
            engine.text(buffer),
            Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
        ));

        let mut offset = 0;
        let mut chunks = 0;
        loop {
            let payload = engine
                .read_snapshot_lease_line_chunk(&lease, 0, offset, 4096)
                .expect("read leased line chunk");
            assert_eq!(payload.lease, lease);
            assert_eq!(payload.line.line, 0);
            assert_eq!(payload.line.start_byte, offset);
            assert!(payload.line.text.len() <= 4096 + 3);
            assert!(payload.line.end_byte > offset || payload.line.is_final);
            chunks += 1;
            offset = payload.line.end_byte;
            if payload.line.is_final {
                assert_eq!(payload.line.end_byte, payload.line.logical_end_byte);
                break;
            }
        }
        assert!(chunks > 1);
    }

    #[test]
    fn owned_snapshot_lease_is_send_sync_and_reads_from_worker() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<OwnedSnapshotLease>();

        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(481), "owned.rs", "éclair\nsecond")
            .expect("open buffer");
        let descriptor = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Ui)
            .expect("lease snapshot");
        let owned = engine
            .owned_snapshot_lease(&descriptor)
            .expect("owned lease");
        let worker = std::thread::spawn(move || {
            owned
                .read_line_chunk(0, 0, 16)
                .expect("worker read")
                .line
                .text
        });
        assert_eq!(worker.join().expect("worker joined"), "éclair");
    }

    #[test]
    fn owned_snapshot_lease_keeps_old_version_after_edit_without_full_cache() {
        let mut engine = EditorEngine::new();
        let original = format!(
            "before\n{}",
            "x".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES)
        );
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(482), "owned-large.rs", original)
            .expect("open buffer");
        let descriptor = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Ui)
            .expect("lease snapshot");
        let owned = engine
            .owned_snapshot_lease(&descriptor)
            .expect("owned lease");
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 0), "new "),
                TransactionSource::User,
                None,
                None,
            )
            .expect("edit");
        assert!(matches!(
            engine.text(buffer),
            Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
        ));
        let chunk = owned
            .read_line_chunk(0, 0, 4096)
            .expect("read old snapshot");
        assert_eq!(chunk.line.text, "before");
        assert!(chunk.line.text.len() <= 4096);
    }

    #[test]
    fn releasing_owned_snapshot_lease_revokes_all_clones() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(483), "owned-release.rs", "abc")
            .expect("open buffer");
        let descriptor = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Ui)
            .expect("lease snapshot");
        let first = engine
            .owned_snapshot_lease(&descriptor)
            .expect("owned lease");
        let second = first.clone();
        engine
            .release_snapshot_lease(descriptor.lease_id)
            .expect("release lease");
        assert!(matches!(
            first.read_line_chunk(0, 0, 8),
            Err(EditorError::SnapshotLeaseRevoked(id)) if id == descriptor.lease_id
        ));
        assert!(matches!(
            second.read_line_chunk(0, 0, 8),
            Err(EditorError::SnapshotLeaseRevoked(id)) if id == descriptor.lease_id
        ));
    }

    #[test]
    fn dropping_editor_engine_revokes_owned_snapshot_clones() {
        let owned = {
            let mut engine = EditorEngine::new();
            let buffer = engine
                .open_buffer(WorkspaceId(1), FileId(485), "owned-drop.rs", "abc")
                .expect("open buffer");
            let descriptor = engine
                .lease_snapshot(buffer, SnapshotConsumerKind::Ui)
                .expect("lease snapshot");
            engine
                .owned_snapshot_lease(&descriptor)
                .expect("owned lease")
        };

        assert!(matches!(
            owned.read_line_chunk(0, 0, 8),
            Err(EditorError::SnapshotLeaseRevoked(_))
        ));
    }

    #[test]
    fn revoked_flag_fails_closed_even_if_cell_still_holds_snapshot() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(487), "owned-revoked.rs", "abc")
            .expect("open buffer");
        let descriptor = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Ui)
            .expect("lease snapshot");
        let owned = engine
            .owned_snapshot_lease(&descriptor)
            .expect("owned lease");
        owned.cell.revoked.store(true, Ordering::Release);
        assert!(matches!(
            owned.read_line_chunk(0, 0, 8),
            Err(EditorError::SnapshotLeaseRevoked(id)) if id == descriptor.lease_id
        ));
    }

    #[test]
    fn owned_snapshot_lease_rejects_expiry_and_oversize_reads() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(484), "owned-expiry.rs", "abc")
            .expect("open buffer");
        let descriptor = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Ui)
            .expect("lease snapshot");
        let owned = engine
            .owned_snapshot_lease(&descriptor)
            .expect("owned lease");
        assert!(matches!(
            owned.read_line_chunk(0, 0, MAX_OWNED_SNAPSHOT_LINE_CHUNK_BYTES + 1),
            Err(EditorError::Text(TextError::InvalidWindowBudget { maximum, .. }))
                if maximum == MAX_OWNED_SNAPSHOT_LINE_CHUNK_BYTES
        ));
        let mut expired = owned.clone();
        expired.descriptor.expires_at = TimestampMillis(0);
        assert!(matches!(
            expired.read_line_chunk(0, 0, 8),
            Err(EditorError::SnapshotLeaseExpired { lease_id, .. })
                if lease_id == descriptor.lease_id
        ));
    }

    #[test]
    fn expired_snapshot_lease_records_are_released_on_next_acquire() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(486), "owned-sweep.rs", "abc")
            .expect("open buffer");
        let expired = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Ui)
            .expect("lease snapshot");
        engine.expire_snapshot_lease_for_test(expired.lease_id);
        let replacement = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Editor)
            .expect("replacement lease");
        assert_ne!(replacement.lease_id, expired.lease_id);
        assert!(engine.release_snapshot_lease(expired.lease_id).is_none());
        assert_eq!(
            engine
                .release_snapshot_lease(replacement.lease_id)
                .map(|lease| lease.lease_id),
            Some(replacement.lease_id)
        );
    }

    #[test]
    fn snapshot_lease_line_chunk_rejects_stale_and_expired_reads() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(49), "line.rs", "abc")
            .expect("open buffer");
        let lease = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Editor)
            .expect("lease snapshot");
        let stale = engine.read_snapshot_lease_line_chunk(
            &SnapshotLeaseDescriptor {
                snapshot_id: SnapshotId(0),
                ..lease.clone()
            },
            0,
            0,
            8,
        );
        assert!(matches!(stale, Err(EditorError::SnapshotLeaseStale { .. })));
        let wrong_buffer = SnapshotLeaseDescriptor {
            buffer_id: BufferId(999),
            ..lease.clone()
        };
        assert!(matches!(
            engine.read_snapshot_lease_line_chunk(&wrong_buffer, 0, 0, 8),
            Err(EditorError::SnapshotLeaseStale { .. })
        ));
        let wrong_version = SnapshotLeaseDescriptor {
            buffer_version: BufferVersion(99),
            ..lease.clone()
        };
        assert!(matches!(
            engine.read_snapshot_lease_line_chunk(&wrong_version, 0, 0, 8),
            Err(EditorError::SnapshotLeaseStale { .. })
        ));
        let expired = engine.read_snapshot_lease_line_chunk_at(
            &lease,
            0,
            0,
            8,
            TimestampMillis(lease.expires_at.0.saturating_add(1)),
        );
        assert!(matches!(
            expired,
            Err(EditorError::SnapshotLeaseExpired { .. })
        ));
    }

    #[test]
    fn snapshot_lease_consumer_must_resynchronize_after_expiry() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(45), "expired.rs", "abc")
            .expect("open buffer");
        let lease = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Lsp)
            .expect("lease snapshot");

        let result = engine.read_snapshot_lease_chunk_at(
            lease.lease_id,
            lease.buffer_id,
            lease.snapshot_id,
            lease.buffer_version,
            0,
            TimestampMillis(lease.expires_at.0.saturating_add(1)),
        );

        assert!(matches!(
            result,
            Err(EditorError::SnapshotLeaseExpired { lease_id, .. }) if lease_id == lease.lease_id
        ));
    }

    #[test]
    fn snapshot_lease_consumer_must_resynchronize_on_stale_snapshot_id() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(46), "stale.rs", "abc")
            .expect("open buffer");
        let lease = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Ai)
            .expect("lease snapshot");
        let current_before = engine.current_snapshot(buffer).unwrap().clone();

        engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 0), "z"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("apply edit");
        let current_after = engine.current_snapshot(buffer).unwrap().clone();
        assert_ne!(current_before.snapshot_id, current_after.snapshot_id);

        let result = engine.read_snapshot_lease_chunk(
            lease.lease_id,
            lease.buffer_id,
            current_after.snapshot_id,
            current_after.buffer_version,
            0,
        );

        assert!(matches!(
            result,
            Err(EditorError::SnapshotLeaseStale {
                lease_id,
                actual_snapshot_id,
                expected_snapshot_id,
                ..
            }) if lease_id == lease.lease_id
                && actual_snapshot_id == current_before.snapshot_id
                && expected_snapshot_id == current_after.snapshot_id
        ));
    }

    #[test]
    fn snapshot_lease_large_file_denies_full_text_but_allows_bounded_chunks() {
        let mut engine = EditorEngine::new();
        let buffer = engine
            .open_buffer(
                WorkspaceId(1),
                FileId(47),
                "large.rs",
                "x".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 1),
            )
            .expect("open large buffer");
        assert!(matches!(
            engine.text(buffer),
            Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
        ));

        let lease = engine
            .lease_snapshot(buffer, SnapshotConsumerKind::Storage)
            .expect("lease snapshot");
        let chunk = engine
            .read_snapshot_lease_chunk(
                lease.lease_id,
                lease.buffer_id,
                lease.snapshot_id,
                lease.buffer_version,
                0,
            )
            .expect("read bounded chunk");

        assert!(lease.chunk_count > 1);
        assert!(chunk.text.len() <= 96 * 1024);
        assert!(chunk.text.len() < DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES);
        assert_eq!(chunk.chunk.snapshot_id, lease.snapshot_id);
    }

    #[test]
    fn draining_transaction_events_is_bounded_and_non_blocking() {
        let mut engine = EditorEngine::with_transaction_event_queue_capacity(2);
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(43), "events.txt", "seed")
            .expect("open buffer");

        for _ in 0..3 {
            engine
                .apply_edit(
                    buffer,
                    TextEdit::insert(TextPosition::new(0, 0), "x"),
                    TransactionSource::User,
                    None,
                    None,
                )
                .expect("apply edit");
        }

        let drained = engine.drain_transaction_events();
        assert_eq!(drained.descriptors.len(), 2);
        assert_eq!(drained.dropped_before_drain, 1);
        assert_eq!(drained.descriptors[0].post_buffer_version, BufferVersion(2));
        assert_eq!(drained.descriptors[1].post_buffer_version, BufferVersion(3));

        let empty = engine.drain_transaction_events();
        assert!(empty.descriptors.is_empty());
        assert_eq!(empty.dropped_before_drain, 0);
    }

    #[test]
    fn degraded_save_assembles_payload_from_chunks_without_full_text_cache() {
        let mut engine = EditorEngine::new();
        let text = format!(
            "head\n{}\ntail\n",
            "x".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 1024)
        );
        let buffer = engine
            .open_buffer(WorkspaceId(1), FileId(44), "save.txt", text.clone())
            .expect("open degraded buffer");
        assert!(matches!(
            engine.text(buffer),
            Err(EditorError::Text(TextError::FullCacheBudgetExceeded { .. }))
        ));
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(TextPosition::new(0, 0), "!"),
                TransactionSource::User,
                None,
                None,
            )
            .expect("edit");

        let save = engine
            .request_save(buffer, None)
            .expect("degraded save should assemble from chunks");
        assert_eq!(save.text, format!("!{text}"));
        assert_eq!(save.payload_byte_len, save.text.len() as u64);
        assert_eq!(
            engine.buffer_save_state(buffer).expect("save state"),
            FileConflictLifecycleState::Saving
        );
        assert!(engine.is_dirty(buffer).expect("dirty"));
        assert_eq!(engine.pending_save_requests().len(), 1);
        assert_eq!(
            engine.pending_save_requests()[0].snapshot_id,
            save.snapshot_id
        );
    }
}
