//! Editor Core: buffers, transactions, undo/redo, diagnostics, deterministic edit APIs.

#![warn(missing_docs)]

use devil_protocol::{BufferId, ByteRange, EditorTransactionEvent, FileId, ProjectId, ProjectInfo};
use devil_text::{TextBuffer, TextEdit};
use thiserror::Error;

/// Errors returned by editor operations.
#[derive(Debug, Error)]
pub enum EditorError {
    /// Position or range for the edit command is invalid for the current buffer.
    #[error("invalid edit command: {0}")]
    InvalidEdit(&'static str),
}

/// Transaction kind generated from each edit operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    /// Insert operation.
    Insert,
    /// Replace operation.
    Replace,
    /// Delete operation.
    Delete,
}

/// Minimal in-memory editor session.
#[derive(Debug)]
pub struct EditorSession {
    project_id: ProjectId,
    _file_id: FileId,
    buffer_id: BufferId,
    file_path: String,
    buffer: TextBuffer,
    undo_stack: Vec<TextBuffer>,
    redo_stack: Vec<TextBuffer>,
    edit_sequence: u64,
}

impl EditorSession {
    /// Creates a new editor session.
    pub fn open(
        file_path: impl Into<String>,
        project_info: ProjectInfo,
        initial_text: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_info.project_id,
            _file_id: project_info.file_id,
            buffer_id: BufferId(0),
            file_path: file_path.into(),
            buffer: TextBuffer::new(initial_text),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            edit_sequence: 0,
        }
    }

    /// Creates a new session tied to an opened shell buffer and project information.
    pub fn open_with_buffer_id(
        file_path: impl Into<String>,
        buffer_id: BufferId,
        project_info: ProjectInfo,
        initial_text: impl Into<String>,
    ) -> Self {
        Self {
            buffer_id,
            project_id: project_info.project_id,
            _file_id: project_info.file_id,
            file_path: file_path.into(),
            buffer: TextBuffer::new(initial_text),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            edit_sequence: 0,
        }
    }

    /// Current editable text.
    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    /// Current file path for this session.
    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    /// Applies an edit and emits a protocol transaction event.
    pub fn apply_edit(&mut self, edit: TextEdit) -> Result<EditorTransactionEvent, EditorError> {
        let start = self
            .buffer
            .byte_offset(edit.range.start)
            .ok_or(EditorError::InvalidEdit(
                "start position is outside the buffer",
            ))?;
        let end = self
            .buffer
            .byte_offset(edit.range.end)
            .ok_or(EditorError::InvalidEdit(
                "end position is outside the buffer",
            ))?;

        let kind = if edit.range.is_empty() && !edit.new_text.is_empty() {
            EditKind::Insert
        } else if edit.new_text.is_empty() {
            EditKind::Delete
        } else {
            EditKind::Replace
        };

        self.undo_stack.push(self.buffer.clone());
        self.redo_stack.clear();

        self.buffer
            .replace_range(start, end, &edit.new_text)
            .ok_or(EditorError::InvalidEdit(
                "range is invalid or not UTF-8 aligned",
            ))?;

        self.edit_sequence += 1;
        let changed_range = match kind {
            EditKind::Insert => {
                let inserted_bytes = edit.new_text.len() as u64;
                ByteRange::new(start as u64, start as u64 + inserted_bytes)
            }
            _ => ByteRange::new(start as u64, end as u64),
        };

        Ok(EditorTransactionEvent {
            buffer_id: self.buffer_id,
            project_id: self.project_id,
            file_path: self.file_path.clone(),
            changed_range: Some(changed_range),
            transaction_id: format!("tx-{}", self.edit_sequence),
        })
    }

    /// Applies an edit by explicit range + replacement payload.
    pub fn apply_edit_range(
        &mut self,
        start: TextPosition,
        end: TextPosition,
        replacement: impl Into<String>,
    ) -> Result<EditorTransactionEvent, EditorError> {
        self.apply_edit(TextEdit::new(TextRange::new(start, end), replacement))
    }

    /// Inserts at a byte offset.
    pub fn insert_offset(
        &mut self,
        offset: usize,
        text: impl Into<String>,
    ) -> Result<EditorTransactionEvent, EditorError> {
        let position = self
            .buffer
            .position(offset)
            .ok_or(EditorError::InvalidEdit("offset is outside the buffer"))?;
        self.insert_at(position, text)
    }

    /// Replaces a byte offset range.
    pub fn replace_offset(
        &mut self,
        start: usize,
        end: usize,
        replacement: impl Into<String>,
    ) -> Result<EditorTransactionEvent, EditorError> {
        let start = self.buffer.position(start).ok_or(EditorError::InvalidEdit(
            "start offset is outside the buffer",
        ))?;
        let end = self
            .buffer
            .position(end)
            .ok_or(EditorError::InvalidEdit("end offset is outside the buffer"))?;
        self.replace_range(TextRange::new(start, end), replacement)
    }

    /// Deletes a byte offset range.
    pub fn delete_offset(
        &mut self,
        start: usize,
        end: usize,
    ) -> Result<EditorTransactionEvent, EditorError> {
        let start = self.buffer.position(start).ok_or(EditorError::InvalidEdit(
            "start offset is outside the buffer",
        ))?;
        let end = self
            .buffer
            .position(end)
            .ok_or(EditorError::InvalidEdit("end offset is outside the buffer"))?;
        self.delete_range(TextRange::new(start, end))
    }

    /// Inserts at a `TextPosition`.
    pub fn insert_at(
        &mut self,
        at: TextPosition,
        text: impl Into<String>,
    ) -> Result<EditorTransactionEvent, EditorError> {
        self.apply_edit(TextEdit::insert(at, text))
    }

    /// Replaces a `TextRange`.
    pub fn replace_range(
        &mut self,
        range: TextRange,
        replacement: impl Into<String>,
    ) -> Result<EditorTransactionEvent, EditorError> {
        self.apply_edit(TextEdit::new(range, replacement))
    }

    /// Deletes a `TextRange`.
    pub fn delete_range(
        &mut self,
        range: TextRange,
    ) -> Result<EditorTransactionEvent, EditorError> {
        self.apply_edit(TextEdit::delete(range))
    }

    /// Undo the latest mutation.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo_stack.pop() else {
            return false;
        };

        self.redo_stack.push(self.buffer.clone());
        self.buffer = previous;
        true
    }

    /// Redo the latest undone mutation.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };

        self.undo_stack.push(self.buffer.clone());
        self.buffer = next;
        true
    }

    /// Number of undo entries available.
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of redo entries available.
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Snapshot of the current buffer for persistence layers.
    pub fn snapshot(&self) -> devil_text::TextSnapshot {
        self.buffer.snapshot()
    }
}

/// Returns a human-readable label for a protocol event category.
impl EditKind {
    /// Human-readable category label.
    pub const fn as_str(&self) -> &'static str {
        match self {
            EditKind::Insert => "insert",
            EditKind::Replace => "replace",
            EditKind::Delete => "delete",
        }
    }
}

pub use devil_text::{TextPosition, TextRange};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_insert_undo_redo_roundtrip() {
        let project = ProjectInfo {
            project_id: ProjectId(1),
            root_path: "root".into(),
            language_id: Some("rust".into()),
            file_id: FileId(7),
        };

        let mut session = EditorSession::open("src/main.rs", project, "hello");

        session
            .insert_at(TextPosition::new(0, 0), "hey ")
            .expect("insert should succeed");
        assert_eq!(session.text(), "hey hello");
        assert_eq!(session.undo_len(), 1);

        assert!(session.undo());
        assert_eq!(session.text(), "hello");

        assert!(session.redo());
        assert_eq!(session.text(), "hey hello");
    }
}
