//! Editor core with multi-buffer transactions, undo/redo grouping, and save-request DTO emission.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;

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
use thiserror::Error;
use uuid::Uuid;

pub use legion_text::{TextEdit, TextPosition, TextRange};

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
}

/// Cursor state for a single caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Cursor position.
    pub position: TextPosition,
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
    edits: Vec<PreparedBatchEdit>,
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
    cursors: Vec<Cursor>,
    selections: Vec<Selection>,
    overlays: Vec<UiOverlay>,
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    current_snapshot: legion_text::TextSnapshot,
    save_state: FileConflictLifecycleState,
    save_diagnostics: Vec<ProtocolDiagnostic>,
    conflict_state: Option<FileConflictState>,
}

impl EditorBufferState {
    fn new(
        workspace_id: WorkspaceId,
        buffer_id: BufferId,
        file_id: FileId,
        file_path: impl Into<String>,
        initial_text: impl Into<String>,
        mode: BufferMode,
    ) -> Result<Self, EditorError> {
        let mut buffer = TextBuffer::try_with_version_and_cache_policy(
            initial_text.into(),
            BufferVersion(0),
            matches!(mode, BufferMode::Normal),
        )?;
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
            dirty: false,
            cursors: vec![Cursor {
                position: TextPosition::zero(),
            }],
            selections: Vec::new(),
            overlays: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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
        let envelope = transaction_event(
            &record.to_protocol_descriptor(),
            applied,
            reason,
            self.sequence(),
        );
        let _ = self.event_sink.emit(EventSinkRequest { envelope });
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
        Self {
            thresholds,
            ..Self::new()
        }
    }

    /// Create an engine with explicit snapshot retention policy.
    pub fn with_snapshot_retention_policy(policy: SnapshotRetentionPolicy) -> Self {
        Self {
            snapshot_retention_policy: policy,
            ..Self::new()
        }
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
        let mode = self.mode_for_byte_len(initial_text.len());
        let state = EditorBufferState::new(
            workspace_id,
            buffer_id,
            file_id,
            file_path,
            initial_text,
            mode,
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
            },
        );
        Ok(descriptor)
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

    /// Release a previously acquired snapshot lease.
    pub fn release_snapshot_lease(&mut self, lease_id: Uuid) -> Option<SnapshotLeaseDescriptor> {
        let lease = self.snapshot_leases.remove(&lease_id)?;
        self.release_snapshot_descriptor_if_unreferenced(lease.snapshot.snapshot_id());
        Some(lease.descriptor)
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
                Ok(ViewportLineMetric {
                    byte_length: state
                        .current_snapshot
                        .line_index()
                        .line_byte_len(slice.line)? as u64,
                    utf16_length: state
                        .current_snapshot
                        .line_index()
                        .line_utf16_len(slice.line)? as u64,
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
            .cursors
            .first()
            .map(|cursor| cursor.position)
            .unwrap_or_else(TextPosition::zero);
        let mode = match state.mode {
            BufferMode::Normal => ViewportProjectionMode::Normal,
            BufferMode::Degraded => ViewportProjectionMode::DegradedLargeFile,
        };

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
            selections: state
                .selections
                .iter()
                .map(|selection| Self::protocol_range(&state.buffer, selection.range))
                .collect::<Result<Vec<_>, _>>()?,
            cursor: Self::protocol_coordinate_from_offset(&state.buffer, state.buffer.try_byte_offset(cursor)?)?,
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
                    "Large file degraded mode is active for {} bytes; viewport payloads are chunked and Phase 1 saves fail closed.",
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

        for prepared in &plan.edits {
            staged_buffer.try_replace_range(prepared.start, prepared.end, &prepared.new_text)?;
            let changed_end = prepared.start + prepared.new_text.len();
            let utf16 = staged_buffer
                .line_index()
                .utf16_range(prepared.start, changed_end)?;
            deltas.push(ChangedDelta {
                byte_range: ByteRange::new(prepared.start as u64, changed_end as u64),
                utf16_range: utf16,
            });
        }

        deltas.reverse();

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
            state.undo_stack.push(UndoEntry {
                snapshot: plan.pre_snapshot.clone(),
                undo_group_id,
            });
            state.redo_stack.clear();
            state.buffer = staged_buffer;
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
            )
        };

        for snapshot_id in redo_snapshot_ids {
            self.release_snapshot_descriptor_if_unreferenced(snapshot_id);
        }
        self.release_snapshot_descriptor_if_unreferenced(old_current_snapshot_id);
        let mut undo_descriptor = plan.pre_snapshot.descriptor().clone();
        undo_descriptor.retention_pin_reason = RetentionPinReason::UndoHistory;
        self.retain_snapshot_descriptor(buffer_id, &undo_descriptor);
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

            if let Some(state) = self.buffers.get_mut(&buffer_id) {
                match stack_kind {
                    SnapshotStackKind::Undo => {
                        if let Some(idx) = state
                            .undo_stack
                            .iter()
                            .position(|entry| entry.snapshot.snapshot_id() == snapshot_id)
                        {
                            state.undo_stack.remove(idx);
                        }
                    }
                    SnapshotStackKind::Redo => {
                        if let Some(idx) = state
                            .redo_stack
                            .iter()
                            .position(|entry| entry.snapshot.snapshot_id() == snapshot_id)
                        {
                            state.redo_stack.remove(idx);
                        }
                    }
                }
            }
            self.release_snapshot_descriptor_if_unreferenced(snapshot_id);
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
                undo_group_id: undo_entry.undo_group_id,
            });
            state.buffer = restored_buffer;
            state.mode = restored_mode;
            state.current_snapshot = restored_snapshot;
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
                undo_group_id: redo_entry.undo_group_id,
            });
            state.buffer = restored_buffer;
            state.mode = restored_mode;
            state.current_snapshot = restored_snapshot;
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

    /// Replace cursors for a buffer.
    pub fn set_cursors(
        &mut self,
        buffer_id: BufferId,
        cursors: Vec<Cursor>,
    ) -> Result<(), EditorError> {
        let state = self
            .buffers
            .get_mut(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        state.cursors = cursors;
        Ok(())
    }

    /// Replace selections for a buffer.
    pub fn set_selections(
        &mut self,
        buffer_id: BufferId,
        selections: Vec<Selection>,
    ) -> Result<(), EditorError> {
        let state = self
            .buffers
            .get_mut(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        state.selections = selections;
        Ok(())
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

    fn absolute_utf16_offset(buffer: &TextBuffer, byte_offset: usize) -> Result<u64, EditorError> {
        let utf16_position = buffer.utf16_position(byte_offset)?;
        let mut total = utf16_position.character as u64;
        for line in 0..utf16_position.line {
            total = total
                .saturating_add(buffer.line_index().line_utf16_len(line)? as u64)
                .saturating_add(buffer.line_index().line_ending_bytes(line)? as u64);
        }
        Ok(total)
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

    fn byte_offset_from_absolute_utf16(
        buffer: &TextBuffer,
        requested: usize,
    ) -> Result<usize, EditorError> {
        let line_index = buffer.line_index();
        let mut remaining = requested;
        for line in 0..line_index.line_count() {
            let line_utf16_len = line_index.line_utf16_len(line)?;
            if remaining <= line_utf16_len {
                return buffer
                    .byte_offset_from_utf16(Utf16Position::new(line, remaining))
                    .map_err(EditorError::from);
            }
            remaining -= line_utf16_len;

            let line_ending_len = line_index.line_ending_bytes(line)?;
            if remaining <= line_ending_len {
                return buffer
                    .byte_offset_from_utf16(Utf16Position::new(line, line_utf16_len))
                    .map_err(EditorError::from);
            }
            remaining -= line_ending_len;
        }

        Err(EditorError::InvalidCompletionPosition(
            "utf16 offset outside buffer",
        ))
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
                let metadata = engine
                    .buffer_metadata(descriptor.buffer_id)
                    .map_err(Self::protocol_error)?;
                if metadata.workspace_id == descriptor.workspace_id
                    && metadata.file_id == descriptor.file_id
                {
                    Ok(EditorResponse::Transaction(descriptor))
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
            EditorRequest::Overlay(overlay) => {
                Ok(EditorResponse::OverlayApplied(overlay.overlay_id))
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
        assert_eq!(state.cursors.len(), 2);
        assert_eq!(state.cursors[0].position, TextPosition::new(0, 2));
        assert_eq!(state.cursors[1].position, TextPosition::new(0, 8));
        assert_eq!(state.selections.len(), 2);
        assert_eq!(
            state.selections[0].range,
            TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 5))
        );
        assert_eq!(
            state.selections[1].range,
            TextRange::new(TextPosition::new(0, 6), TextPosition::new(0, 10))
        );

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
        assert_eq!(projection.cursor.character, 2);
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
