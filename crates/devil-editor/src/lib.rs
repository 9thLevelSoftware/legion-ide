//! Editor core with multi-buffer transactions, undo/redo grouping, and save-request DTO emission.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet};

use devil_protocol::{
    BufferId, BufferVersion, ByteRange, CorrelationId, FileId, SnapshotId, TextOffset,
    TextRange as ProtocolTextRange, TextTransactionDescriptor, TimestampMillis, TransactionSource,
    WorkspaceId,
};
use devil_text::{
    RetentionPinReason, TextBuffer, TextEdit, TextError, TextSnapshotDescriptor, Utf16Range,
};
use thiserror::Error;
use uuid::Uuid;

pub use devil_text::{TextPosition, TextRange};

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
                .map(|delta| ProtocolTextRange {
                    start: TextOffset::byte(delta.byte_range.start),
                    end: TextOffset::byte(delta.byte_range.end),
                })
                .collect(),
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
struct EditorBufferState {
    workspace_id: WorkspaceId,
    buffer_id: BufferId,
    file_id: FileId,
    file_path: String,
    buffer: TextBuffer,
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
    ) -> Self {
        let mut buffer = TextBuffer::new(initial_text.into());
        buffer.set_version(BufferVersion(0));
        let current_snapshot = buffer.snapshot_with_retention(RetentionPinReason::CurrentBuffer);

        Self {
            workspace_id,
            buffer_id,
            file_id,
            file_path: file_path.into(),
            buffer,
            dirty: false,
            cursors: vec![Cursor {
                position: TextPosition::zero(),
            }],
            selections: Vec::new(),
            overlays: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_snapshot,
        }
    }
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
}

impl EditorEngine {
    /// Create a new empty engine.
    pub fn new() -> Self {
        Self {
            next_buffer_id: 1,
            ..Self::default()
        }
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

        let state = EditorBufferState::new(workspace_id, buffer_id, file_id, file_path, initial_text);
        self.pinned_snapshot_ids
            .insert(state.current_snapshot.snapshot_id());
        self.file_to_buffer.insert((workspace_id, file_id), buffer_id);
        self.buffers.insert(buffer_id, state);
        Ok(buffer_id)
    }

    /// Close a buffer.
    pub fn close_buffer(&mut self, buffer_id: BufferId) -> Result<(), EditorError> {
        let state = self
            .buffers
            .remove(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        self.file_to_buffer.remove(&(state.workspace_id, state.file_id));
        self.pinned_snapshot_ids
            .remove(&state.current_snapshot.snapshot_id());
        for entry in state.undo_stack.iter().chain(state.redo_stack.iter()) {
            self.pinned_snapshot_ids.remove(&entry.snapshot.snapshot_id());
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
        mut edits: Vec<TextEdit>,
        source: TransactionSource,
        undo_group_id: Option<Uuid>,
        correlation_id: Option<CorrelationId>,
    ) -> Result<TransactionRecord, EditorError> {
        if edits.is_empty() {
            return Err(EditorError::InvalidEdit("edit batch cannot be empty"));
        }

        let state = self
            .buffers
            .get_mut(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;

        let pre_snapshot = state.current_snapshot.clone();
        let pre_descriptor = pre_snapshot.descriptor().clone();

        state.undo_stack.push(UndoEntry {
            snapshot: pre_snapshot.clone(),
            undo_group_id,
        });
        self.pinned_snapshot_ids.insert(pre_snapshot.snapshot_id());
        state.redo_stack.clear();

        let mut deltas = Vec::with_capacity(edits.len());
        // Apply from highest start offset to lowest for deterministic no-shift behavior
        edits.sort_by_key(|edit| {
            state
                .buffer
                .try_byte_offset(edit.range.start)
                .unwrap_or(usize::MAX)
        });
        edits.reverse();

        for edit in edits {
            let start = state.buffer.try_byte_offset(edit.range.start)?;
            let end = state.buffer.try_byte_offset(edit.range.end)?;
            state.buffer.try_replace_range(start, end, &edit.new_text)?;

            let changed_end = start + edit.new_text.len();
            let utf16 = state.buffer.line_index().utf16_range(start, changed_end)?;
            deltas.push(ChangedDelta {
                byte_range: ByteRange::new(start as u64, changed_end as u64),
                utf16_range: utf16,
            });
        }

        let next_version = BufferVersion(state.buffer.version().0 + 1);
        state.buffer.set_version(next_version);
        state.current_snapshot =
            state
                .buffer
                .snapshot_with_retention(RetentionPinReason::CurrentBuffer);
        state.dirty = true;

        self.pinned_snapshot_ids
            .insert(state.current_snapshot.snapshot_id());

        let tx = TransactionRecord {
            transaction_id: Uuid::now_v7(),
            causality_trace_id: Uuid::now_v7(),
            workspace_id: state.workspace_id,
            buffer_id: state.buffer_id,
            file_id: state.file_id,
            source,
            pre_snapshot: pre_descriptor,
            post_snapshot: state.current_snapshot.descriptor().clone(),
            deltas,
            undo_group_id,
            occurred_at: TimestampMillis::now(),
            correlation_id,
        };

        self.transaction_log.push(tx.clone());
        Ok(tx)
    }

    /// Undo one transaction for the given buffer.
    pub fn undo(
        &mut self,
        buffer_id: BufferId,
        correlation_id: Option<CorrelationId>,
    ) -> Result<TransactionRecord, EditorError> {
        let state = self
            .buffers
            .get_mut(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        let undo_entry = state.undo_stack.pop().ok_or(EditorError::NothingToUndo)?;
        let pre_snapshot = state.current_snapshot.clone();

        state.redo_stack.push(UndoEntry {
            snapshot: pre_snapshot.clone(),
            undo_group_id: undo_entry.undo_group_id,
        });
        self.pinned_snapshot_ids.insert(pre_snapshot.snapshot_id());

        state.buffer = TextBuffer::with_version(
            undo_entry.snapshot.text().to_string(),
            BufferVersion(state.buffer.version().0 + 1),
        );
        state.current_snapshot =
            state
                .buffer
                .snapshot_with_retention(RetentionPinReason::CurrentBuffer);
        state.dirty = true;

        let tx = TransactionRecord {
            transaction_id: Uuid::now_v7(),
            causality_trace_id: Uuid::now_v7(),
            workspace_id: state.workspace_id,
            buffer_id: state.buffer_id,
            file_id: state.file_id,
            source: TransactionSource::Restore,
            pre_snapshot: pre_snapshot.descriptor().clone(),
            post_snapshot: state.current_snapshot.descriptor().clone(),
            deltas: vec![ChangedDelta {
                byte_range: ByteRange::new(0, state.buffer.len() as u64),
                utf16_range: state
                    .buffer
                    .line_index()
                    .utf16_range(0, state.buffer.len())
                    .unwrap_or(Utf16Range {
                        start: devil_text::Utf16Position {
                            line: 0,
                            character: 0,
                        },
                        end: devil_text::Utf16Position {
                            line: 0,
                            character: 0,
                        },
                    }),
            }],
            undo_group_id: undo_entry.undo_group_id,
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
        let state = self
            .buffers
            .get_mut(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        let redo_entry = state.redo_stack.pop().ok_or(EditorError::NothingToRedo)?;
        let pre_snapshot = state.current_snapshot.clone();

        state.undo_stack.push(UndoEntry {
            snapshot: pre_snapshot.clone(),
            undo_group_id: redo_entry.undo_group_id,
        });

        state.buffer = TextBuffer::with_version(
            redo_entry.snapshot.text().to_string(),
            BufferVersion(state.buffer.version().0 + 1),
        );
        state.current_snapshot =
            state
                .buffer
                .snapshot_with_retention(RetentionPinReason::CurrentBuffer);
        state.dirty = true;

        let tx = TransactionRecord {
            transaction_id: Uuid::now_v7(),
            causality_trace_id: Uuid::now_v7(),
            workspace_id: state.workspace_id,
            buffer_id: state.buffer_id,
            file_id: state.file_id,
            source: TransactionSource::Restore,
            pre_snapshot: pre_snapshot.descriptor().clone(),
            post_snapshot: state.current_snapshot.descriptor().clone(),
            deltas: vec![ChangedDelta {
                byte_range: ByteRange::new(0, state.buffer.len() as u64),
                utf16_range: state
                    .buffer
                    .line_index()
                    .utf16_range(0, state.buffer.len())
                    .unwrap_or(Utf16Range {
                        start: devil_text::Utf16Position {
                            line: 0,
                            character: 0,
                        },
                        end: devil_text::Utf16Position {
                            line: 0,
                            character: 0,
                        },
                    }),
            }],
            undo_group_id: redo_entry.undo_group_id,
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
        let state = self
            .buffers
            .get_mut(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?;
        let snapshot =
            state
                .buffer
                .snapshot_with_retention(RetentionPinReason::BackgroundSave);

        self.pinned_snapshot_ids.insert(snapshot.snapshot_id());

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
        self.pending_save_requests.push(dto.clone());
        Ok(dto)
    }

    /// Mark that a save request completed and clear dirty state when matching current snapshot.
    pub fn acknowledge_save(&mut self, request_id: Uuid, success: bool) {
        if let Some(idx) = self
            .pending_save_requests
            .iter()
            .position(|request| request.request_id == request_id)
        {
            let request = self.pending_save_requests.remove(idx);
            if success {
                if let Some(state) = self.buffers.get_mut(&request.buffer_id) {
                    if state.current_snapshot.snapshot_id() == request.snapshot_id {
                        state.dirty = false;
                    }
                }
            }

            if self.pending_save_requests.iter().all(|pending| pending.snapshot_id != request.snapshot_id)
                && self
                    .buffers
                    .values()
                    .all(|state| {
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
                self.pinned_snapshot_ids.remove(&request.snapshot_id);
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

/// Backward-compatible session wrapper around one active buffer.
#[derive(Debug)]
pub struct EditorSession {
    engine: EditorEngine,
    active_buffer_id: BufferId,
}

impl EditorSession {
    /// Create a session with one open buffer.
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

    /// Create a session with explicit buffer id ignored for compatibility.
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
    pub fn apply_edit(
        &mut self,
        edit: TextEdit,
    ) -> Result<TextTransactionDescriptor, EditorError> {
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
    pub fn delete_range(&mut self, range: TextRange) -> Result<TextTransactionDescriptor, EditorError> {
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
            .open_buffer(
                WorkspaceId(1),
                FileId(10),
                "src/a.rs",
                "fn a() {}\n",
            )
            .unwrap();
        let b = engine
            .open_buffer(
                WorkspaceId(1),
                FileId(11),
                "src/b.rs",
                "fn b() {}\n",
            )
            .unwrap();

        assert_eq!(engine.text(a).unwrap(), "fn a() {}\n");
        assert_eq!(engine.text(b).unwrap(), "fn b() {}\n");

        engine.close_buffer(a).unwrap();
        assert!(matches!(engine.text(a), Err(EditorError::BufferNotFound(_))));
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
    fn undo_redo_invariants() {
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
    fn save_request_is_decoupled_from_disk_writes() {
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
