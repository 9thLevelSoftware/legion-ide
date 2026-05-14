//! Editor core with multi-buffer transactions, undo/redo grouping, and save-request DTO emission.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet, VecDeque};

use devil_protocol::{
    BufferId, BufferVersion, ByteRange, CausalityId, ChangedTextRange, CorrelationId, FileId,
    SnapshotId, TextTransactionDescriptor, TimestampMillis, TransactionSource, Utf16Position,
    Utf16Range as ProtocolUtf16Range, WorkspaceId,
};
use devil_text::{
    DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES, RetentionPinReason, TextBuffer, TextError,
    TextSnapshotDescriptor, Utf16Range,
};
use thiserror::Error;
use uuid::Uuid;

pub use devil_text::{TextEdit, TextPosition, TextRange};

/// Editor operation errors.
#[derive(Debug, Error)]
pub enum EditorError {
    /// Buffer not found.
    #[error("buffer {0:?} does not exist")]
    BufferNotFound(BufferId),
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
        TextTransactionDescriptor {
            workspace_id: self.workspace_id,
            buffer_id: self.buffer_id,
            file_id: self.file_id,
            transaction_id: self.transaction_id,
            correlation_id: self.correlation_id.unwrap_or(CorrelationId(0)),
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
                        start: Utf16Position {
                            line: delta.utf16_range.start.line as u32,
                            character: delta.utf16_range.start.character as u32,
                        },
                        end: Utf16Position {
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
    /// UTF-8 text payload to persist asynchronously through workspace/proposal ports.
    pub text: String,
    /// Emission timestamp.
    pub requested_at: TimestampMillis,
    /// Optional caller correlation id.
    pub correlation_id: Option<CorrelationId>,
}

#[derive(Debug, Clone)]
struct UndoEntry {
    snapshot: devil_text::TextSnapshot,
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
    pre_snapshot: devil_text::TextSnapshot,
    pre_descriptor: TextSnapshotDescriptor,
    pre_version: BufferVersion,
    edits: Vec<PreparedBatchEdit>,
}

#[derive(Debug, Clone)]
struct SaveSnapshotPayload {
    snapshot: devil_text::TextSnapshot,
    dto: SaveRequestDto,
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
    current_snapshot: devil_text::TextSnapshot,
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
        let mut buffer = TextBuffer::try_with_version(initial_text.into(), BufferVersion(0))?;
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
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotStackKind {
    Undo,
    Redo,
}

/// Production multi-buffer editor engine.
#[derive(Debug, Default)]
pub struct EditorEngine {
    next_buffer_id: u128,
    buffers: HashMap<BufferId, EditorBufferState>,
    file_to_buffer: HashMap<(WorkspaceId, FileId), BufferId>,
    transaction_log: Vec<TransactionRecord>,
    pending_save_requests: Vec<SaveRequestDto>,
    pinned_snapshot_ids: HashSet<SnapshotId>,
    thresholds: EditorThresholds,
    snapshot_retention_policy: SnapshotRetentionPolicy,
    retained_snapshots: VecDeque<RetainedSnapshotDescriptor>,
}

const DEFAULT_RETENTION_BUDGET_SNAPSHOTS: usize = 256;
const DEFAULT_RETENTION_BUDGET_BYTES: usize = DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES * 4;

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

impl EditorEngine {
    /// Create a new empty engine.
    pub fn new() -> Self {
        Self {
            next_buffer_id: 1,
            thresholds: EditorThresholds::default(),
            snapshot_retention_policy: SnapshotRetentionPolicy::default(),
            ..Self::default()
        }
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
        let mode = if initial_text.len() > self.thresholds.large_file_threshold_bytes {
            BufferMode::Degraded
        } else {
            BufferMode::Normal
        };
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
        self.remove_snapshot_descriptor(state.current_snapshot.snapshot_id());
        for entry in state.undo_stack.iter().chain(state.redo_stack.iter()) {
            self.remove_snapshot_descriptor(entry.snapshot.snapshot_id());
        }
        Ok(())
    }

    /// Get immutable text for a buffer.
    pub fn text(&self, buffer_id: BufferId) -> Result<&str, EditorError> {
        Ok(self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .buffer
            .text())
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
            state.current_snapshot = post_snapshot;
            state.dirty = true;
            (
                state.workspace_id,
                state.file_id,
                old_current_snapshot_id,
                redo_snapshot_ids,
                state.current_snapshot.descriptor().clone(),
            )
        };

        for snapshot_id in redo_snapshot_ids {
            self.remove_snapshot_descriptor(snapshot_id);
        }
        self.remove_snapshot_descriptor(old_current_snapshot_id);
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
        Ok(tx)
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
            self.remove_snapshot_descriptor(snapshot_id);
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
        let (
            workspace_id,
            file_id,
            undo_group_id,
            pre_snapshot_descriptor,
            redo_snapshot_descriptor,
            post_snapshot_descriptor,
            undo_snapshot_id,
            delta,
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
            let mut restored_buffer =
                TextBuffer::try_with_version(undo_entry.snapshot.text().to_string(), next_version)?;
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
            state.current_snapshot = restored_snapshot;
            state.dirty = true;

            (
                state.workspace_id,
                state.file_id,
                undo_entry.undo_group_id,
                pre_snapshot_descriptor,
                redo_snapshot_descriptor,
                restored_snapshot_descriptor,
                undo_snapshot_id,
                delta,
            )
        };
        self.remove_snapshot_descriptor(undo_snapshot_id);
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
        self.transaction_log.push(tx.clone());
        Ok(tx)
    }

    /// Redo one transaction for the given buffer.
    pub fn redo(
        &mut self,
        buffer_id: BufferId,
        correlation_id: Option<CorrelationId>,
    ) -> Result<TransactionRecord, EditorError> {
        let (
            workspace_id,
            file_id,
            undo_group_id,
            pre_snapshot_descriptor,
            undo_snapshot_descriptor,
            post_snapshot_descriptor,
            redo_snapshot_id,
            delta,
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
            let mut restored_buffer =
                TextBuffer::try_with_version(redo_entry.snapshot.text().to_string(), next_version)?;
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
            state.current_snapshot = restored_snapshot;
            state.dirty = true;

            (
                state.workspace_id,
                state.file_id,
                redo_entry.undo_group_id,
                pre_snapshot_descriptor,
                undo_snapshot_descriptor,
                restored_snapshot_descriptor,
                redo_snapshot_id,
                delta,
            )
        };
        self.remove_snapshot_descriptor(redo_snapshot_id);
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
        self.transaction_log.push(tx.clone());
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
                .get(&buffer_id)
                .ok_or(EditorError::BufferNotFound(buffer_id))?;
            let snapshot = state
                .buffer
                .try_snapshot_with_retention(RetentionPinReason::BackgroundSave)?;
            let dto = SaveRequestDto {
                request_id: Uuid::now_v7(),
                workspace_id: state.workspace_id,
                buffer_id: state.buffer_id,
                file_id: state.file_id,
                snapshot_id: snapshot.snapshot_id(),
                buffer_version: snapshot.buffer_version(),
                content_hash: snapshot.content_hash().to_string(),
                text: snapshot.text().to_string(),
                requested_at: TimestampMillis::now(),
                correlation_id,
            };
            SaveSnapshotPayload { snapshot, dto }
        };

        self.retain_snapshot_descriptor(buffer_id, payload.snapshot.descriptor());
        self.pending_save_requests.push(payload.dto.clone());
        self.enforce_snapshot_retention_policy();
        Ok(payload.dto)
    }

    /// Mark that a save request completed and clear dirty state when matching current snapshot.
    pub fn acknowledge_save(&mut self, request_id: Uuid, success: bool) {
        if let Some(idx) = self
            .pending_save_requests
            .iter()
            .position(|request| request.request_id == request_id)
        {
            let request = self.pending_save_requests.remove(idx);
            if success
                && let Some(state) = self.buffers.get_mut(&request.buffer_id)
                && state.current_snapshot.snapshot_id() == request.snapshot_id
            {
                state.dirty = false;
            }

            if self
                .pending_save_requests
                .iter()
                .all(|pending| pending.snapshot_id != request.snapshot_id)
                && self.buffers.values().all(|state| {
                    state.current_snapshot.snapshot_id() != request.snapshot_id
                        && state
                            .undo_stack
                            .iter()
                            .all(|entry| entry.snapshot.snapshot_id() != request.snapshot_id)
                        && state
                            .redo_stack
                            .iter()
                            .all(|entry| entry.snapshot.snapshot_id() != request.snapshot_id)
                })
            {
                self.remove_snapshot_descriptor(request.snapshot_id);
            }
        }
    }

    /// Read-only transaction log.
    pub fn transaction_log(&self) -> &[TransactionRecord] {
        &self.transaction_log
    }

    /// Read-only pending save queue.
    pub fn pending_save_requests(&self) -> &[SaveRequestDto] {
        &self.pending_save_requests
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
        project_info: devil_protocol::ProjectInfo,
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
        project_info: devil_protocol::ProjectInfo,
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
    pub fn snapshot(&self) -> devil_text::TextSnapshot {
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
    use devil_protocol::{ProjectId, ProjectInfo};

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
    }
}
