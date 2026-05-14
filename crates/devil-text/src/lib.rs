//! Text primitives: rope-backed buffers, UTF index conversion, edits, and immutable snapshots.

#![warn(missing_docs)]

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use devil_protocol::{BufferVersion, SnapshotId};
use ropey::Rope;
use sha2::{Digest, Sha256};
use thiserror::Error;

static NEXT_SNAPSHOT_ID: AtomicU64 = AtomicU64::new(1);

const DEFAULT_LEAF_TARGET_BYTES: usize = 1024;
/// Maximum UTF-8 bytes allowed in the full text cache used by the text model.
///
/// This aligns with the workspace large-file exclusion threshold so buffers that need a larger
/// payload can be routed through degraded/streaming editor paths instead of materializing a full
/// cache and line index.
pub const DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES: usize = 5 * 1024 * 1024;

/// A position in text expressed as zero-indexed line and UTF-8 column within that line.
///
/// `line` is zero-based. `column` is a UTF-8 byte offset from the start of the logical line,
/// excluding the line ending. This preserves compatibility with the spike API while the richer
/// [`LineIndex`] APIs expose explicit UTF-16 and byte-coordinate conversions for LSP use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextPosition {
    /// Zero-indexed line number.
    pub line: usize,
    /// Zero-indexed UTF-8 byte column within the line.
    pub column: usize,
}

impl TextPosition {
    /// Create a new position at the given line and column.
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Return a position at the start of the document (`0,0`).
    pub const fn zero() -> Self {
        Self { line: 0, column: 0 }
    }
}

impl fmt::Display for TextPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A range between two [`TextPosition`]s.
///
/// `start` is inclusive and `end` is exclusive. It is valid when `start` is less than or equal to
/// `end` within the same buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextRange {
    /// Inclusive start position.
    pub start: TextPosition,
    /// Exclusive end position.
    pub end: TextPosition,
}

impl TextRange {
    /// Create a new range from `start` to `end`.
    ///
    /// # Panics
    ///
    /// Panics in debug mode if `start` is after `end`.
    pub fn new(start: TextPosition, end: TextPosition) -> Self {
        debug_assert!(
            start.line < end.line || (start.line == end.line && start.column <= end.column),
            "TextRange start must be <= end"
        );
        Self { start, end }
    }

    /// Create a zero-length (collapsed) range at `pos`.
    pub const fn empty(pos: TextPosition) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }

    /// Returns `true` if the range contains no characters.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

impl fmt::Display for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// A single text edit operation: replace the contents of [`TextRange`] with `new_text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// The range to replace.
    pub range: TextRange,
    /// The text to insert in place of the range.
    pub new_text: String,
}

impl TextEdit {
    /// Create a new edit.
    pub fn new(range: TextRange, new_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }

    /// Create an insertion edit (empty range).
    pub fn insert(pos: TextPosition, text: impl Into<String>) -> Self {
        Self {
            range: TextRange::empty(pos),
            new_text: text.into(),
        }
    }

    /// Create a deletion edit (empty replacement text).
    pub fn delete(range: TextRange) -> Self {
        Self {
            range,
            new_text: String::new(),
        }
    }
}

/// Reason that keeps a snapshot retained by the editor engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RetentionPinReason {
    /// Current live buffer state.
    CurrentBuffer,
    /// Referenced by undo history.
    UndoHistory,
    /// Referenced by redo history.
    RedoHistory,
    /// Referenced by an in-flight background save.
    BackgroundSave,
    /// Referenced by an LSP synchronization operation.
    LspSync,
    /// Referenced by indexing or analysis.
    Indexing,
    /// Explicit caller-specified reason.
    Explicit(String),
}

impl fmt::Display for RetentionPinReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RetentionPinReason::CurrentBuffer => write!(f, "current-buffer"),
            RetentionPinReason::UndoHistory => write!(f, "undo-history"),
            RetentionPinReason::RedoHistory => write!(f, "redo-history"),
            RetentionPinReason::BackgroundSave => write!(f, "background-save"),
            RetentionPinReason::LspSync => write!(f, "lsp-sync"),
            RetentionPinReason::Indexing => write!(f, "indexing"),
            RetentionPinReason::Explicit(reason) => write!(f, "{reason}"),
        }
    }
}

/// Immutable metadata describing a text snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSnapshotDescriptor {
    /// Globally unique snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Buffer version captured by the snapshot.
    pub buffer_version: BufferVersion,
    /// SHA-256 hash of UTF-8 content, hex encoded.
    pub content_hash: String,
    /// Total byte length.
    pub byte_len: usize,
    /// Total logical line count.
    pub line_count: usize,
    /// Estimated memory footprint in bytes.
    pub memory_footprint_bytes: usize,
    /// Reason this snapshot is pinned for retention.
    pub retention_pin_reason: RetentionPinReason,
}

/// Immutable snapshot of buffer contents.
///
/// Snapshots are cheap to clone because rope nodes are shared through [`Arc`].
#[derive(Debug, Clone)]
pub struct TextSnapshot {
    rope: Arc<Rope>,
    text_cache: Arc<String>,
    line_index: Arc<LineIndex>,
    descriptor: TextSnapshotDescriptor,
}

impl PartialEq for TextSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor && self.text() == other.text()
    }
}

impl Eq for TextSnapshot {}

impl TextSnapshot {
    /// Create a snapshot from a string with an automatically assigned ID and version 0.
    ///
    /// This compatibility constructor panics when the text exceeds the default full-cache budget.
    /// Prefer [`TextSnapshot::try_new`] or [`TextSnapshot::try_from_rope`] on fallible paths.
    pub fn new(text: impl Into<String>) -> Self {
        Self::try_from_rope(
            Rope::from_str(&text.into()),
            BufferVersion(0),
            RetentionPinReason::Explicit("standalone snapshot".to_string()),
        )
        .expect("standalone snapshot text must fit the full-cache budget")
    }

    /// Try to create a snapshot from a string with an automatically assigned ID and version 0.
    pub fn try_new(text: impl Into<String>) -> TextResult<Self> {
        Self::try_from_rope(
            Rope::from_str(&text.into()),
            BufferVersion(0),
            RetentionPinReason::Explicit("standalone snapshot".to_string()),
        )
    }

    /// Create a snapshot from a rope, version, and retention reason.
    ///
    /// This compatibility constructor panics when the rope exceeds the default full-cache budget.
    /// Prefer [`TextSnapshot::try_from_rope`] on fallible paths.
    pub fn from_rope(
        rope: Rope,
        buffer_version: BufferVersion,
        retention_pin_reason: RetentionPinReason,
    ) -> Self {
        Self::try_from_rope(rope, buffer_version, retention_pin_reason)
            .expect("snapshot rope must fit the full-cache budget")
    }

    /// Try to create a snapshot from a rope, version, and retention reason.
    pub fn try_from_rope(
        rope: Rope,
        buffer_version: BufferVersion,
        retention_pin_reason: RetentionPinReason,
    ) -> TextResult<Self> {
        enforce_full_cache_budget(rope.len_bytes())?;
        let text = rope.to_string();
        enforce_full_cache_budget(text.len())?;
        let line_index = LineIndex::new(&text);
        let content_hash = content_hash(&text);
        let descriptor = TextSnapshotDescriptor {
            snapshot_id: SnapshotId(NEXT_SNAPSHOT_ID.fetch_add(1, Ordering::Relaxed) as u128),
            buffer_version,
            content_hash,
            byte_len: text.len(),
            line_count: line_index.line_count(),
            memory_footprint_bytes: estimate_rope_memory(
                &rope,
                text.len(),
                line_index.memory_footprint_bytes(),
            ),
            retention_pin_reason,
        };

        Ok(Self {
            rope: Arc::new(rope),
            text_cache: Arc::new(text),
            line_index: Arc::new(line_index),
            descriptor,
        })
    }

    /// Return the full text.
    pub fn text(&self) -> &str {
        &self.text_cache
    }

    /// Return the immutable descriptor.
    pub fn descriptor(&self) -> &TextSnapshotDescriptor {
        &self.descriptor
    }

    /// Return the snapshot ID.
    pub fn snapshot_id(&self) -> SnapshotId {
        self.descriptor.snapshot_id
    }

    /// Return the buffer version captured by this snapshot.
    pub fn buffer_version(&self) -> BufferVersion {
        self.descriptor.buffer_version
    }

    /// Return the SHA-256 content hash.
    pub fn content_hash(&self) -> &str {
        &self.descriptor.content_hash
    }

    /// Return the line index for offset conversion.
    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    /// Return the number of lines in the snapshot.
    pub fn line_count(&self) -> usize {
        self.descriptor.line_count
    }

    /// Return the byte length of the text.
    pub fn len(&self) -> usize {
        self.descriptor.byte_len
    }

    /// Returns `true` if the snapshot contains no text.
    pub fn is_empty(&self) -> bool {
        self.descriptor.byte_len == 0
    }

    /// Return an estimated memory footprint in bytes.
    pub fn memory_footprint_bytes(&self) -> usize {
        self.descriptor.memory_footprint_bytes
    }

    /// Convert this snapshot back into a rope clone.
    pub fn rope(&self) -> Rope {
        self.rope.as_ref().clone()
    }
}

/// Errors returned by text model offset and edit operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TextError {
    /// A byte offset was outside the document.
    #[error("byte offset {offset} is outside document length {len}")]
    ByteOffsetOutOfBounds {
        /// Requested offset.
        offset: usize,
        /// Document length.
        len: usize,
    },
    /// A byte offset was not on a UTF-8 character boundary.
    #[error("byte offset {offset} is not a UTF-8 character boundary")]
    NotUtf8Boundary {
        /// Requested offset.
        offset: usize,
    },
    /// A line index was outside the line table.
    #[error("line {line} is outside line count {line_count}")]
    LineOutOfBounds {
        /// Requested line.
        line: usize,
        /// Total line count.
        line_count: usize,
    },
    /// A column was outside its line.
    #[error("{unit} column {column} is outside line {line} length {line_len}")]
    ColumnOutOfBounds {
        /// Requested line.
        line: usize,
        /// Requested column.
        column: usize,
        /// Line length in the requested unit.
        line_len: usize,
        /// Unit name.
        unit: &'static str,
    },
    /// A UTF-16 offset landed inside a surrogate pair.
    #[error("UTF-16 column {column} on line {line} splits a surrogate pair")]
    Utf16InsideSurrogatePair {
        /// Requested line.
        line: usize,
        /// Requested UTF-16 column.
        column: usize,
    },
    /// A range had start greater than end.
    #[error("invalid range {start}..{end}")]
    InvalidRange {
        /// Start offset.
        start: usize,
        /// End offset.
        end: usize,
    },
    /// A full-cache operation exceeded the text model byte budget and must use degraded mode.
    #[error(
        "text byte length {byte_len} exceeds full-cache budget {budget}; degraded large-file mode required"
    )]
    FullCacheBudgetExceeded {
        /// Requested post-operation text length in bytes.
        byte_len: usize,
        /// Maximum allowed full-cache text length in bytes.
        budget: usize,
    },
}

/// Result type for text model operations.
pub type TextResult<T> = Result<T, TextError>;

/// LSP-compatible UTF-16 position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Utf16Position {
    /// Zero-based line number.
    pub line: usize,
    /// Zero-based UTF-16 code-unit column.
    pub character: usize,
}

impl Utf16Position {
    /// Construct a UTF-16 position.
    pub const fn new(line: usize, character: usize) -> Self {
        Self { line, character }
    }
}

/// LSP-compatible UTF-16 range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Utf16Range {
    /// Inclusive start position.
    pub start: Utf16Position,
    /// Exclusive end position.
    pub end: Utf16Position,
}

impl Utf16Range {
    /// Construct a UTF-16 range.
    pub const fn new(start: Utf16Position, end: Utf16Position) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineMetric {
    start_byte: usize,
    content_end_byte: usize,
    end_byte: usize,
    byte_len: usize,
    utf16_len: usize,
    line_ending_bytes: usize,
}

impl LineMetric {
    fn contains_offset(&self, offset: usize, is_last_line: bool) -> bool {
        if is_last_line {
            self.start_byte <= offset && offset <= self.end_byte
        } else {
            self.start_byte <= offset && offset < self.end_byte
        }
    }
}

/// Line index supporting byte, UTF-8, and UTF-16 coordinate conversion.
///
/// The index treats CRLF (`\r\n`) as a single line ending for LSP mapping and excludes line-ending
/// bytes from column lengths. UTF-16 conversions reject offsets inside surrogate pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    text: Arc<String>,
    lines: Vec<LineMetric>,
}

impl LineIndex {
    /// Build a line index for the provided UTF-8 text.
    pub fn new(text: &str) -> Self {
        let mut lines = Vec::new();
        let mut start = 0usize;
        let bytes = text.as_bytes();
        let mut i = 0usize;

        while i < bytes.len() {
            match bytes[i] {
                b'\r' if i + 1 < bytes.len() && bytes[i + 1] == b'\n' => {
                    Self::push_line(text, &mut lines, start, i, i + 2, 2);
                    i += 2;
                    start = i;
                }
                b'\n' => {
                    Self::push_line(text, &mut lines, start, i, i + 1, 1);
                    i += 1;
                    start = i;
                }
                _ => {
                    i += 1;
                }
            }
        }

        Self::push_line(text, &mut lines, start, text.len(), text.len(), 0);

        Self {
            text: Arc::new(text.to_string()),
            lines,
        }
    }

    fn push_line(
        text: &str,
        lines: &mut Vec<LineMetric>,
        start_byte: usize,
        content_end_byte: usize,
        end_byte: usize,
        line_ending_bytes: usize,
    ) {
        let slice = &text[start_byte..content_end_byte];
        lines.push(LineMetric {
            start_byte,
            content_end_byte,
            end_byte,
            byte_len: content_end_byte.saturating_sub(start_byte),
            utf16_len: slice.encode_utf16().count(),
            line_ending_bytes,
        });
    }

    /// Return the number of logical lines. Empty text has one line.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Return the full document byte length.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns `true` when the indexed text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Return the byte length of a logical line excluding CRLF/LF line endings.
    pub fn line_byte_len(&self, line: usize) -> TextResult<usize> {
        Ok(self.line(line)?.byte_len)
    }

    /// Return the UTF-16 length of a logical line excluding CRLF/LF line endings.
    pub fn line_utf16_len(&self, line: usize) -> TextResult<usize> {
        Ok(self.line(line)?.utf16_len)
    }

    /// Return the line-ending byte length for a line: 0, 1, or 2.
    pub fn line_ending_bytes(&self, line: usize) -> TextResult<usize> {
        Ok(self.line(line)?.line_ending_bytes)
    }

    /// Convert line + UTF-8 byte column to absolute byte offset.
    pub fn byte_offset(&self, pos: TextPosition) -> TextResult<usize> {
        let line = self.line(pos.line)?;
        if pos.column > line.byte_len {
            return Err(TextError::ColumnOutOfBounds {
                line: pos.line,
                column: pos.column,
                line_len: line.byte_len,
                unit: "byte",
            });
        }
        let offset = line.start_byte + pos.column;
        self.ensure_char_boundary(offset)?;
        Ok(offset)
    }

    /// Convert an absolute byte offset to line + UTF-8 byte column.
    pub fn position(&self, offset: usize) -> TextResult<TextPosition> {
        self.ensure_valid_offset(offset)?;
        self.ensure_char_boundary(offset)?;
        let line_index = self.line_for_offset(offset)?;
        let line = self.line(line_index)?;
        let column = offset.saturating_sub(line.start_byte).min(line.byte_len);
        Ok(TextPosition::new(line_index, column))
    }

    /// Convert an absolute byte offset to an LSP UTF-16 position.
    pub fn utf16_position(&self, offset: usize) -> TextResult<Utf16Position> {
        self.ensure_valid_offset(offset)?;
        self.ensure_char_boundary(offset)?;
        let line_index = self.line_for_offset(offset)?;
        let line = self.line(line_index)?;
        let column_end = offset.min(line.content_end_byte);
        let prefix = &self.text[line.start_byte..column_end];
        Ok(Utf16Position::new(
            line_index,
            prefix.encode_utf16().count(),
        ))
    }

    /// Convert an LSP UTF-16 position to an absolute byte offset.
    pub fn byte_offset_from_utf16(&self, pos: Utf16Position) -> TextResult<usize> {
        let line = self.line(pos.line)?;
        if pos.character > line.utf16_len {
            return Err(TextError::ColumnOutOfBounds {
                line: pos.line,
                column: pos.character,
                line_len: line.utf16_len,
                unit: "utf16",
            });
        }

        let slice = &self.text[line.start_byte..line.content_end_byte];
        let mut utf16 = 0usize;
        for (byte, ch) in slice.char_indices() {
            if utf16 == pos.character {
                return Ok(line.start_byte + byte);
            }
            utf16 += ch.len_utf16();
            if utf16 > pos.character {
                return Err(TextError::Utf16InsideSurrogatePair {
                    line: pos.line,
                    column: pos.character,
                });
            }
        }

        if utf16 == pos.character {
            Ok(line.content_end_byte)
        } else {
            Err(TextError::ColumnOutOfBounds {
                line: pos.line,
                column: pos.character,
                line_len: line.utf16_len,
                unit: "utf16",
            })
        }
    }

    /// Convert an absolute byte range to an LSP UTF-16 range.
    pub fn utf16_range(&self, start: usize, end: usize) -> TextResult<Utf16Range> {
        if start > end {
            return Err(TextError::InvalidRange { start, end });
        }
        Ok(Utf16Range::new(
            self.utf16_position(start)?,
            self.utf16_position(end)?,
        ))
    }

    /// Return an estimated memory footprint in bytes.
    pub fn memory_footprint_bytes(&self) -> usize {
        self.text.len() + self.lines.len() * std::mem::size_of::<LineMetric>()
    }

    fn line(&self, line: usize) -> TextResult<&LineMetric> {
        self.lines.get(line).ok_or(TextError::LineOutOfBounds {
            line,
            line_count: self.lines.len(),
        })
    }

    fn line_for_offset(&self, offset: usize) -> TextResult<usize> {
        self.ensure_valid_offset(offset)?;
        match self.lines.binary_search_by(|line| {
            if offset < line.start_byte {
                std::cmp::Ordering::Greater
            } else if offset > line.end_byte {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        }) {
            Ok(mut idx) => {
                while idx + 1 < self.lines.len() && !self.lines[idx].contains_offset(offset, false)
                {
                    idx += 1;
                }
                Ok(idx)
            }
            Err(idx) if idx > 0 => Ok(idx - 1),
            Err(_) => Ok(0),
        }
    }

    fn ensure_valid_offset(&self, offset: usize) -> TextResult<()> {
        if offset > self.text.len() {
            Err(TextError::ByteOffsetOutOfBounds {
                offset,
                len: self.text.len(),
            })
        } else {
            Ok(())
        }
    }

    fn ensure_char_boundary(&self, offset: usize) -> TextResult<()> {
        if self.text.is_char_boundary(offset) {
            Ok(())
        } else {
            Err(TextError::NotUtf8Boundary { offset })
        }
    }
}

/// A mutable text buffer backed by a [`Rope`].
///
/// Rope-backed edits are logarithmic in document size and avoid whole-buffer copies for typical
/// insertions and deletions. Compatibility helper methods retain the previous `Option` API while
/// richer `try_*` methods report detailed conversion failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBuffer {
    rope: Rope,
    line_index: LineIndex,
    version: BufferVersion,
    text_cache: String,
}

impl TextBuffer {
    /// Create a new buffer from a string.
    pub fn new(text: impl Into<String>) -> Self {
        Self::with_version(text, BufferVersion(0))
    }

    /// Create a new buffer with an explicit buffer version.
    pub fn with_version(text: impl Into<String>, version: BufferVersion) -> Self {
        Self::try_with_version(text, version)
            .expect("text buffer must fit the full-cache budget; use degraded large-file mode")
    }

    /// Try to create a new buffer with an explicit buffer version.
    pub fn try_with_version(text: impl Into<String>, version: BufferVersion) -> TextResult<Self> {
        let text = text.into();
        enforce_full_cache_budget(text.len())?;
        let rope = Rope::from_str(&text);
        let line_index = LineIndex::new(&text);
        Ok(Self {
            rope,
            line_index,
            version,
            text_cache: text,
        })
    }

    /// Return the full text.
    pub fn text(&self) -> &str {
        &self.text_cache
    }

    /// Return the current buffer version.
    pub fn version(&self) -> BufferVersion {
        self.version
    }

    /// Set the current buffer version.
    pub fn set_version(&mut self, version: BufferVersion) {
        self.version = version;
    }

    /// Return the byte length of the text.
    pub fn len(&self) -> usize {
        self.text_cache.len()
    }

    /// Returns `true` if the buffer contains no text.
    pub fn is_empty(&self) -> bool {
        self.text_cache.is_empty()
    }

    /// Return the number of logical lines. Empty buffers have one line.
    pub fn line_count(&self) -> usize {
        self.line_index.line_count()
    }

    /// Return the current line index.
    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    /// Convert a [`TextPosition`] to a byte offset.
    pub fn byte_offset(&self, pos: TextPosition) -> Option<usize> {
        self.try_byte_offset(pos).ok()
    }

    /// Convert a [`TextPosition`] to a byte offset with detailed error reporting.
    pub fn try_byte_offset(&self, pos: TextPosition) -> TextResult<usize> {
        self.line_index.byte_offset(pos)
    }

    /// Convert a byte offset to a [`TextPosition`].
    pub fn position(&self, offset: usize) -> Option<TextPosition> {
        self.try_position(offset).ok()
    }

    /// Convert a byte offset to a [`TextPosition`] with detailed error reporting.
    pub fn try_position(&self, offset: usize) -> TextResult<TextPosition> {
        self.line_index.position(offset)
    }

    /// Convert a byte offset to an LSP UTF-16 position.
    pub fn utf16_position(&self, offset: usize) -> TextResult<Utf16Position> {
        self.line_index.utf16_position(offset)
    }

    /// Convert an LSP UTF-16 position to a byte offset.
    pub fn byte_offset_from_utf16(&self, pos: Utf16Position) -> TextResult<usize> {
        self.line_index.byte_offset_from_utf16(pos)
    }

    /// Insert `text` at the given byte offset.
    pub fn insert(&mut self, offset: usize, text: &str) -> Option<()> {
        self.try_insert(offset, text).ok()
    }

    /// Insert `text` at the given byte offset with detailed error reporting.
    pub fn try_insert(&mut self, offset: usize, text: &str) -> TextResult<()> {
        self.try_replace_range(offset, offset, text)
    }

    /// Delete the byte range `[start, end)`.
    pub fn delete_range(&mut self, start: usize, end: usize) -> Option<()> {
        self.try_delete_range(start, end).ok()
    }

    /// Delete the byte range `[start, end)` with detailed error reporting.
    pub fn try_delete_range(&mut self, start: usize, end: usize) -> TextResult<()> {
        self.try_replace_range(start, end, "")
    }

    /// Replace the byte range `[start, end)` with `text`.
    pub fn replace_range(&mut self, start: usize, end: usize, text: &str) -> Option<()> {
        self.try_replace_range(start, end, text).ok()
    }

    /// Replace the byte range `[start, end)` with `text` and return detailed errors.
    pub fn try_replace_range(&mut self, start: usize, end: usize, text: &str) -> TextResult<()> {
        self.validate_range(start, end)?;
        let removed_len = end.saturating_sub(start);
        let post_edit_len = self
            .text_cache
            .len()
            .saturating_sub(removed_len)
            .saturating_add(text.len());
        enforce_full_cache_budget(post_edit_len)?;
        let start_char = self.rope.byte_to_char(start);
        let end_char = self.rope.byte_to_char(end);
        self.rope.remove(start_char..end_char);
        self.rope.insert(start_char, text);
        self.refresh_cache_and_index()?;
        Ok(())
    }

    /// Apply a [`TextEdit`] to the buffer.
    pub fn apply_edit(&mut self, edit: &TextEdit) -> Option<()> {
        self.try_apply_edit(edit).ok()
    }

    /// Apply a [`TextEdit`] to the buffer with detailed error reporting.
    pub fn try_apply_edit(&mut self, edit: &TextEdit) -> TextResult<()> {
        let start = self.try_byte_offset(edit.range.start)?;
        let end = self.try_byte_offset(edit.range.end)?;
        self.try_replace_range(start, end, &edit.new_text)
    }

    /// Create an immutable [`TextSnapshot`] of the current contents.
    pub fn snapshot(&self) -> TextSnapshot {
        self.snapshot_with_retention(RetentionPinReason::CurrentBuffer)
    }

    /// Try to create an immutable [`TextSnapshot`] of the current contents.
    pub fn try_snapshot(&self) -> TextResult<TextSnapshot> {
        self.try_snapshot_with_retention(RetentionPinReason::CurrentBuffer)
    }

    /// Create an immutable [`TextSnapshot`] with an explicit retention reason.
    pub fn snapshot_with_retention(&self, reason: RetentionPinReason) -> TextSnapshot {
        self.try_snapshot_with_retention(reason)
            .expect("text buffer snapshot must fit the full-cache budget")
    }

    /// Try to create an immutable [`TextSnapshot`] with an explicit retention reason.
    pub fn try_snapshot_with_retention(
        &self,
        reason: RetentionPinReason,
    ) -> TextResult<TextSnapshot> {
        TextSnapshot::try_from_rope(self.rope.clone(), self.version, reason)
    }

    /// Return the estimated memory footprint of the buffer.
    pub fn memory_footprint_bytes(&self) -> usize {
        estimate_rope_memory(
            &self.rope,
            self.text_cache.len(),
            self.line_index.memory_footprint_bytes(),
        )
    }

    fn validate_range(&self, start: usize, end: usize) -> TextResult<()> {
        if start > end {
            return Err(TextError::InvalidRange { start, end });
        }
        self.line_index.ensure_valid_offset(start)?;
        self.line_index.ensure_valid_offset(end)?;
        self.line_index.ensure_char_boundary(start)?;
        self.line_index.ensure_char_boundary(end)?;
        Ok(())
    }

    fn refresh_cache_and_index(&mut self) -> TextResult<()> {
        let refreshed = self.rope.to_string();
        enforce_full_cache_budget(refreshed.len())?;
        self.text_cache = refreshed;
        self.line_index = LineIndex::new(&self.text_cache);
        Ok(())
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new("")
    }
}

impl From<String> for TextBuffer {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<&str> for TextBuffer {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn estimate_rope_memory(rope: &Rope, byte_len: usize, line_index_bytes: usize) -> usize {
    byte_len
        + line_index_bytes
        + rope.len_lines() * std::mem::size_of::<usize>() * 4
        + DEFAULT_LEAF_TARGET_BYTES
}

fn enforce_full_cache_budget(byte_len: usize) -> TextResult<()> {
    if byte_len > DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES {
        Err(TextError::FullCacheBudgetExceeded {
            byte_len,
            budget: DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::quickcheck;

    #[test]
    fn text_position_display() {
        assert_eq!(TextPosition::new(1, 5).to_string(), "1:5");
    }

    #[test]
    fn text_range_empty() {
        let r = TextRange::empty(TextPosition::new(2, 4));
        assert!(r.is_empty());
    }

    #[test]
    fn snapshot_descriptor_has_required_metadata() {
        let snap = TextSnapshot::new("hello\nworld");
        assert_eq!(snap.len(), 11);
        assert_eq!(snap.line_count(), 2);
        assert!(snap.content_hash().starts_with("sha256:"));
        assert!(snap.memory_footprint_bytes() >= snap.len());
        assert_eq!(snap.descriptor().buffer_version, BufferVersion(0));
    }

    #[test]
    fn snapshot_clone_is_cheap_and_immutable() {
        let mut buf = TextBuffer::new("original");
        let snap = buf.snapshot();
        let clone = snap.clone();
        assert_eq!(clone.text(), "original");
        assert!(Arc::ptr_eq(&snap.text_cache, &clone.text_cache));
        buf.insert(8, " changed").unwrap();
        assert_eq!(snap.text(), "original");
        assert_eq!(buf.text(), "original changed");
    }

    #[test]
    fn buffer_insert_delete_replace() {
        let mut buf = TextBuffer::new("hello world");
        assert!(buf.insert(5, ",").is_some());
        assert_eq!(buf.text(), "hello, world");
        assert!(buf.delete_range(5, 6).is_some());
        assert_eq!(buf.text(), "hello world");
        assert!(buf.replace_range(6, 11, "Rust").is_some());
        assert_eq!(buf.text(), "hello Rust");
    }

    #[test]
    fn buffer_rejects_non_boundary_edits() {
        let mut buf = TextBuffer::new("é");
        assert!(buf.insert(1, "x").is_none());
        assert!(matches!(
            buf.try_insert(1, "x"),
            Err(TextError::NotUtf8Boundary { offset: 1 })
        ));
    }

    #[test]
    fn position_roundtrip_multibyte_columns_are_bytes() {
        let buf = TextBuffer::new("aé\n🦀z");
        assert_eq!(buf.byte_offset(TextPosition::new(0, 0)), Some(0));
        assert_eq!(buf.byte_offset(TextPosition::new(0, 1)), Some(1));
        assert_eq!(buf.byte_offset(TextPosition::new(0, 3)), Some(3));
        assert_eq!(buf.position(4), Some(TextPosition::new(1, 0)));
        assert_eq!(buf.position(8), Some(TextPosition::new(1, 4)));
    }

    #[test]
    fn crlf_is_single_line_ending_for_lsp() {
        let idx = LineIndex::new("ab\r\ncd");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.line_byte_len(0).unwrap(), 2);
        assert_eq!(idx.line_ending_bytes(0).unwrap(), 2);
        assert_eq!(idx.byte_offset(TextPosition::new(1, 0)).unwrap(), 4);
        assert_eq!(idx.utf16_position(4).unwrap(), Utf16Position::new(1, 0));
    }

    #[test]
    fn utf16_golden_surrogate_pairs() {
        let idx = LineIndex::new("a🦀b");
        assert_eq!(idx.utf16_position(0).unwrap(), Utf16Position::new(0, 0));
        assert_eq!(idx.utf16_position(1).unwrap(), Utf16Position::new(0, 1));
        assert_eq!(idx.utf16_position(5).unwrap(), Utf16Position::new(0, 3));
        assert_eq!(idx.utf16_position(6).unwrap(), Utf16Position::new(0, 4));
        assert_eq!(
            idx.byte_offset_from_utf16(Utf16Position::new(0, 3))
                .unwrap(),
            5
        );
        assert!(matches!(
            idx.byte_offset_from_utf16(Utf16Position::new(0, 2)),
            Err(TextError::Utf16InsideSurrogatePair { .. })
        ));
    }

    #[test]
    fn utf16_range_golden() {
        let idx = LineIndex::new("a🦀\r\nb");
        let range = idx.utf16_range(1, 7).unwrap();
        assert_eq!(range.start, Utf16Position::new(0, 1));
        assert_eq!(range.end, Utf16Position::new(1, 0));
    }

    #[test]
    fn property_edits_match_string_model_for_ascii() {
        fn prop(seed: String, replacement: String, a: u8, b: u8) -> bool {
            let mut model: String = seed.chars().filter(|c| c.is_ascii()).take(128).collect();
            let replacement: String = replacement
                .chars()
                .filter(|c| c.is_ascii() && *c != '\0')
                .take(32)
                .collect();
            let mut rope = TextBuffer::new(model.clone());
            let len = model.len();
            let start = if len == 0 {
                0
            } else {
                (a as usize) % (len + 1)
            };
            let end = if len == 0 {
                0
            } else {
                (b as usize) % (len + 1)
            };
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };

            model.replace_range(start..end, &replacement);
            rope.replace_range(start, end, &replacement).unwrap();
            rope.text() == model
        }

        quickcheck(prop as fn(String, String, u8, u8) -> bool);
    }

    #[test]
    fn large_file_typical_keystroke_edit_smoke() {
        let text = "a".repeat(1024 * 1024);
        let mut buf = TextBuffer::new(text);
        let before = buf.memory_footprint_bytes();
        buf.insert(512 * 1024, "x").unwrap();
        assert_eq!(buf.len(), 1024 * 1024 + 1);
        assert!(buf.memory_footprint_bytes() >= before);
    }
}
