//! Text primitives: rope, spans, ranges, UTF indexing, edits, and immutable snapshots.

#![warn(missing_docs)]

use std::fmt;
use std::sync::Arc;

/// A position in text expressed as zero-indexed line and column.
///
/// Both `line` and `column` are counted from `0`. A `TextPosition` is
/// always valid for a given buffer when `line` is within the line count
/// and `column` is within the line length (or at the end of the line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextPosition {
    /// Zero-indexed line number.
    pub line: usize,
    /// Zero-indexed column (byte offset within the line for this minimal implementation).
    pub column: usize,
}

impl TextPosition {
    /// Create a new position at the given line and column.
    pub fn new(line: usize, column: usize) -> Self {
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
/// `start` is inclusive and `end` is exclusive. It is valid when
/// `start` is less than or equal to `end` within the same buffer.
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
    pub fn empty(pos: TextPosition) -> Self {
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
///
/// When `range` is empty this behaves as an insertion at `range.start`.
/// When `new_text` is empty this behaves as a deletion.
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

/// An immutable snapshot of buffer contents.
///
/// Snapshots are cheap to clone because they share the underlying text via [`Arc`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSnapshot {
    text: Arc<String>,
}

impl TextSnapshot {
    /// Create a snapshot from a string.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Arc::new(text.into()),
        }
    }

    /// Return the full text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Return the number of lines in the snapshot.
    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            return 1;
        }
        let mut count = 1;
        for ch in self.text.chars() {
            if ch == '\n' {
                count += 1;
            }
        }
        count
    }

    /// Return the byte length of the text.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns `true` if the snapshot contains no text.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// A mutable text buffer backed by a [`String`].
///
/// Provides basic operations: insert, delete, replace, line counting,
/// and conversion between [`TextPosition`] and byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBuffer {
    text: String,
}

impl TextBuffer {
    /// Create a new buffer from a string.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    /// Return the full text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Return the byte length of the text.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns `true` if the buffer contains no text.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Return the number of lines.
    ///
    /// An empty buffer is considered to have one line.
    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            return 1;
        }
        let mut count = 1;
        for ch in self.text.chars() {
            if ch == '\n' {
                count += 1;
            }
        }
        count
    }

    /// Convert a [`TextPosition`] to a byte offset.
    ///
    /// Returns `None` if the position is out of bounds or not on a UTF-8
    /// character boundary.
    pub fn byte_offset(&self, pos: TextPosition) -> Option<usize> {
        let mut line = 0usize;
        let mut col = 0usize;
        let mut offset = 0usize;

        for ch in self.text.chars() {
            if line == pos.line && col == pos.column {
                return Some(offset);
            }
            if ch == '\n' {
                if line == pos.line && pos.column == col {
                    // Position at end of line (before newline)
                    return Some(offset);
                }
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
            offset += ch.len_utf8();
        }

        // End-of-text position
        if line == pos.line && col == pos.column {
            return Some(offset);
        }

        None
    }

    /// Convert a byte offset to a [`TextPosition`].
    ///
    /// Returns `None` if the offset is out of bounds or not on a UTF-8
    /// character boundary.
    pub fn position(&self, offset: usize) -> Option<TextPosition> {
        if offset > self.text.len() {
            return None;
        }
        if !self.text.is_char_boundary(offset) {
            return None;
        }

        let mut line = 0usize;
        let mut column = 0usize;
        let mut current = 0usize;

        for ch in self.text.chars() {
            if current == offset {
                return Some(TextPosition::new(line, column));
            }
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
            current += ch.len_utf8();
        }

        if current == offset {
            Some(TextPosition::new(line, column))
        } else {
            None
        }
    }

    /// Insert `text` at the given byte offset.
    ///
    /// Returns `None` if the offset is out of bounds or not on a char boundary.
    pub fn insert(&mut self, offset: usize, text: &str) -> Option<()> {
        if offset > self.text.len() || !self.text.is_char_boundary(offset) {
            return None;
        }
        self.text.insert_str(offset, text);
        Some(())
    }

    /// Delete the byte range `[start, end)`.
    ///
    /// Returns `None` if the range is invalid or not on char boundaries.
    pub fn delete_range(&mut self, start: usize, end: usize) -> Option<()> {
        if start > end
            || end > self.text.len()
            || !self.text.is_char_boundary(start)
            || !self.text.is_char_boundary(end)
        {
            return None;
        }
        self.text.replace_range(start..end, "");
        Some(())
    }

    /// Replace the byte range `[start, end)` with `text`.
    ///
    /// Returns `None` if the range is invalid or not on char boundaries.
    pub fn replace_range(&mut self, start: usize, end: usize, text: &str) -> Option<()> {
        if start > end
            || end > self.text.len()
            || !self.text.is_char_boundary(start)
            || !self.text.is_char_boundary(end)
        {
            return None;
        }
        self.text.replace_range(start..end, text);
        Some(())
    }

    /// Apply a [`TextEdit`] to the buffer.
    ///
    /// Returns `None` if the edit range cannot be resolved to valid byte offsets.
    pub fn apply_edit(&mut self, edit: &TextEdit) -> Option<()> {
        let start = self.byte_offset(edit.range.start)?;
        let end = self.byte_offset(edit.range.end)?;
        self.replace_range(start, end, &edit.new_text)
    }

    /// Create an immutable [`TextSnapshot`] of the current contents.
    pub fn snapshot(&self) -> TextSnapshot {
        TextSnapshot {
            text: Arc::new(self.text.clone()),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // TextPosition
    // ------------------------------------------------------------------

    #[test]
    fn text_position_new() {
        let pos = TextPosition::new(3, 7);
        assert_eq!(pos.line, 3);
        assert_eq!(pos.column, 7);
    }

    #[test]
    fn text_position_zero() {
        let pos = TextPosition::zero();
        assert_eq!(pos, TextPosition::new(0, 0));
    }

    #[test]
    fn text_position_display() {
        assert_eq!(TextPosition::new(1, 5).to_string(), "1:5");
    }

    // ------------------------------------------------------------------
    // TextRange
    // ------------------------------------------------------------------

    #[test]
    fn text_range_new() {
        let r = TextRange::new(TextPosition::new(0, 0), TextPosition::new(1, 3));
        assert_eq!(r.start, TextPosition::new(0, 0));
        assert_eq!(r.end, TextPosition::new(1, 3));
    }

    #[test]
    fn text_range_empty() {
        let r = TextRange::empty(TextPosition::new(2, 4));
        assert!(r.is_empty());
        assert_eq!(r.start, r.end);
    }

    #[test]
    fn text_range_display() {
        let r = TextRange::new(TextPosition::new(0, 0), TextPosition::new(1, 3));
        assert_eq!(r.to_string(), "0:0..1:3");
    }

    // ------------------------------------------------------------------
    // TextEdit
    // ------------------------------------------------------------------

    #[test]
    fn text_edit_new() {
        let edit = TextEdit::new(
            TextRange::new(TextPosition::new(0, 1), TextPosition::new(0, 3)),
            "xx",
        );
        assert_eq!(edit.new_text, "xx");
    }

    #[test]
    fn text_edit_insert() {
        let edit = TextEdit::insert(TextPosition::new(0, 0), "hello");
        assert!(edit.range.is_empty());
        assert_eq!(edit.new_text, "hello");
    }

    #[test]
    fn text_edit_delete() {
        let edit = TextEdit::delete(TextRange::new(
            TextPosition::new(0, 0),
            TextPosition::new(0, 3),
        ));
        assert_eq!(edit.new_text, "");
    }

    // ------------------------------------------------------------------
    // TextSnapshot
    // ------------------------------------------------------------------

    #[test]
    fn snapshot_text() {
        let snap = TextSnapshot::new("hello");
        assert_eq!(snap.text(), "hello");
    }

    #[test]
    fn snapshot_line_count_empty() {
        let snap = TextSnapshot::new("");
        assert_eq!(snap.line_count(), 1);
    }

    #[test]
    fn snapshot_line_count_single() {
        let snap = TextSnapshot::new("hello world");
        assert_eq!(snap.line_count(), 1);
    }

    #[test]
    fn snapshot_line_count_multiple() {
        let snap = TextSnapshot::new("a\nb\nc");
        assert_eq!(snap.line_count(), 3);
    }

    #[test]
    fn snapshot_len() {
        let snap = TextSnapshot::new("abc");
        assert_eq!(snap.len(), 3);
    }

    #[test]
    fn snapshot_is_empty() {
        let snap = TextSnapshot::new("");
        assert!(snap.is_empty());
    }

    #[test]
    fn snapshot_clone_is_cheap() {
        let a = TextSnapshot::new("shared");
        let b = a.clone();
        assert_eq!(a.text(), b.text());
        // Both point to the same Arc allocation
        assert!(Arc::ptr_eq(&a.text, &b.text));
    }

    // ------------------------------------------------------------------
    // TextBuffer — creation & basic properties
    // ------------------------------------------------------------------

    #[test]
    fn buffer_from_string() {
        let buf = TextBuffer::new("hello");
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn buffer_from_str() {
        let buf: TextBuffer = "hello".into();
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn buffer_default() {
        let buf = TextBuffer::default();
        assert_eq!(buf.text(), "");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn buffer_line_count_empty() {
        let buf = TextBuffer::new("");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn buffer_line_count_multiple() {
        let buf = TextBuffer::new("line1\nline2\nline3");
        assert_eq!(buf.line_count(), 3);
    }

    #[test]
    fn buffer_line_count_trailing_newline() {
        let buf = TextBuffer::new("a\nb\n");
        assert_eq!(buf.line_count(), 3);
    }

    // ------------------------------------------------------------------
    // TextBuffer — byte offset / position conversion
    // ------------------------------------------------------------------

    #[test]
    fn buffer_byte_offset_start() {
        let buf = TextBuffer::new("abc\ndef");
        assert_eq!(buf.byte_offset(TextPosition::new(0, 0)), Some(0));
    }

    #[test]
    fn buffer_byte_offset_mid_line() {
        let buf = TextBuffer::new("abc\ndef");
        assert_eq!(buf.byte_offset(TextPosition::new(0, 2)), Some(2));
    }

    #[test]
    fn buffer_byte_offset_end_of_line() {
        let buf = TextBuffer::new("abc\ndef");
        assert_eq!(buf.byte_offset(TextPosition::new(0, 3)), Some(3));
    }

    #[test]
    fn buffer_byte_offset_start_of_next_line() {
        let buf = TextBuffer::new("abc\ndef");
        assert_eq!(buf.byte_offset(TextPosition::new(1, 0)), Some(4));
    }

    #[test]
    fn buffer_byte_offset_end_of_text() {
        let buf = TextBuffer::new("abc\ndef");
        assert_eq!(buf.byte_offset(TextPosition::new(1, 3)), Some(7));
    }

    #[test]
    fn buffer_byte_offset_out_of_bounds() {
        let buf = TextBuffer::new("abc");
        assert_eq!(buf.byte_offset(TextPosition::new(0, 4)), None);
        assert_eq!(buf.byte_offset(TextPosition::new(1, 0)), None);
    }

    #[test]
    fn buffer_position_roundtrip() {
        let buf = TextBuffer::new("abc\ndef\nghi");
        for line in 0..3 {
            for col in 0..3 {
                let pos = TextPosition::new(line, col);
                let offset = buf.byte_offset(pos).unwrap();
                let back = buf.position(offset).unwrap();
                assert_eq!(back, pos, "roundtrip failed for {:?}", pos);
            }
        }
    }

    #[test]
    fn buffer_position_end_of_text() {
        let buf = TextBuffer::new("abc");
        assert_eq!(buf.position(3), Some(TextPosition::new(0, 3)));
    }

    #[test]
    fn buffer_position_out_of_bounds() {
        let buf = TextBuffer::new("abc");
        assert_eq!(buf.position(4), None);
    }

    #[test]
    fn buffer_position_not_char_boundary() {
        let buf = TextBuffer::new("é"); // 2 bytes in UTF-8
        assert_eq!(buf.position(1), None);
    }

    // ------------------------------------------------------------------
    // TextBuffer — insert
    // ------------------------------------------------------------------

    #[test]
    fn buffer_insert_at_start() {
        let mut buf = TextBuffer::new("world");
        assert!(buf.insert(0, "hello ").is_some());
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn buffer_insert_at_end() {
        let mut buf = TextBuffer::new("hello");
        assert!(buf.insert(5, " world").is_some());
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn buffer_insert_mid() {
        let mut buf = TextBuffer::new("helo");
        assert!(buf.insert(3, "l").is_some());
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn buffer_insert_out_of_bounds() {
        let mut buf = TextBuffer::new("hi");
        assert!(buf.insert(10, "x").is_none());
    }

    #[test]
    fn buffer_insert_not_char_boundary() {
        let mut buf = TextBuffer::new("é"); // 2 bytes
        assert!(buf.insert(1, "x").is_none());
    }

    // ------------------------------------------------------------------
    // TextBuffer — delete range
    // ------------------------------------------------------------------

    #[test]
    fn buffer_delete_range() {
        let mut buf = TextBuffer::new("hello world");
        assert!(buf.delete_range(6, 11).is_some());
        assert_eq!(buf.text(), "hello ");
    }

    #[test]
    fn buffer_delete_range_empty() {
        let mut buf = TextBuffer::new("abc");
        assert!(buf.delete_range(1, 1).is_some());
        assert_eq!(buf.text(), "abc");
    }

    #[test]
    fn buffer_delete_range_invalid() {
        let mut buf = TextBuffer::new("abc");
        assert!(buf.delete_range(2, 1).is_none()); // start > end
        assert!(buf.delete_range(0, 10).is_none()); // end out of bounds
    }

    #[test]
    fn buffer_delete_range_not_char_boundary() {
        let mut buf = TextBuffer::new("éà"); // 2 bytes each
        assert!(buf.delete_range(1, 2).is_none());
    }

    // ------------------------------------------------------------------
    // TextBuffer — replace range
    // ------------------------------------------------------------------

    #[test]
    fn buffer_replace_range() {
        let mut buf = TextBuffer::new("hello world");
        assert!(buf.replace_range(6, 11, "Rust").is_some());
        assert_eq!(buf.text(), "hello Rust");
    }

    #[test]
    fn buffer_replace_range_to_empty() {
        let mut buf = TextBuffer::new("hello world");
        assert!(buf.replace_range(5, 11, "").is_some());
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn buffer_replace_range_from_empty() {
        let mut buf = TextBuffer::new("hello");
        assert!(buf.replace_range(5, 5, " world").is_some());
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn buffer_replace_range_invalid() {
        let mut buf = TextBuffer::new("abc");
        assert!(buf.replace_range(2, 1, "x").is_none());
    }

    // ------------------------------------------------------------------
    // TextBuffer — apply_edit
    // ------------------------------------------------------------------

    #[test]
    fn buffer_apply_edit_replace() {
        let mut buf = TextBuffer::new("hello world");
        let edit = TextEdit::new(
            TextRange::new(TextPosition::new(0, 6), TextPosition::new(0, 11)),
            "Rust",
        );
        assert!(buf.apply_edit(&edit).is_some());
        assert_eq!(buf.text(), "hello Rust");
    }

    #[test]
    fn buffer_apply_edit_insert() {
        let mut buf = TextBuffer::new("hello");
        let edit = TextEdit::insert(TextPosition::new(0, 5), " world");
        assert!(buf.apply_edit(&edit).is_some());
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn buffer_apply_edit_delete() {
        let mut buf = TextBuffer::new("hello world");
        let edit = TextEdit::delete(TextRange::new(
            TextPosition::new(0, 5),
            TextPosition::new(0, 11),
        ));
        assert!(buf.apply_edit(&edit).is_some());
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn buffer_apply_edit_multiline() {
        let mut buf = TextBuffer::new("a\nb\nc");
        let edit = TextEdit::new(
            TextRange::new(TextPosition::new(1, 0), TextPosition::new(2, 1)),
            "X",
        );
        assert!(buf.apply_edit(&edit).is_some());
        assert_eq!(buf.text(), "a\nX");
    }

    #[test]
    fn buffer_apply_edit_invalid_range() {
        let mut buf = TextBuffer::new("short");
        let edit = TextEdit::new(
            TextRange::new(TextPosition::new(0, 0), TextPosition::new(0, 100)),
            "x",
        );
        assert!(buf.apply_edit(&edit).is_none());
    }

    // ------------------------------------------------------------------
    // TextBuffer — snapshot
    // ------------------------------------------------------------------

    #[test]
    fn buffer_snapshot_preserves_text() {
        let buf = TextBuffer::new("snapshot me");
        let snap = buf.snapshot();
        assert_eq!(snap.text(), "snapshot me");
    }

    #[test]
    fn buffer_snapshot_is_immutable() {
        let mut buf = TextBuffer::new("original");
        let snap = buf.snapshot();
        buf.insert(8, " changed");
        assert_eq!(snap.text(), "original");
        assert_eq!(buf.text(), "original changed");
    }

    #[test]
    fn buffer_snapshot_line_count() {
        let buf = TextBuffer::new("a\nb");
        let snap = buf.snapshot();
        assert_eq!(snap.line_count(), 2);
    }

    // ------------------------------------------------------------------
    // UTF-8 safety
    // ------------------------------------------------------------------

    #[test]
    fn buffer_utf8_multibyte() {
        let mut buf = TextBuffer::new("café");
        assert_eq!(buf.len(), 5); // c a f é (2 bytes)
        assert_eq!(buf.line_count(), 1);

        // Replace the 'é' with 'e'
        assert!(buf.replace_range(3, 5, "e").is_some());
        assert_eq!(buf.text(), "cafe");
    }

    #[test]
    fn buffer_utf8_emoji() {
        let mut buf = TextBuffer::new("🦀"); // 4 bytes
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.position(0), Some(TextPosition::new(0, 0)));
        assert_eq!(buf.position(4), Some(TextPosition::new(0, 1)));
        assert_eq!(buf.position(2), None); // not a char boundary

        assert!(buf.insert(4, "🚀").is_some());
        assert_eq!(buf.text(), "🦀🚀");
    }
}
