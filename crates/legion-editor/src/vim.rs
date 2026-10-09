//! Vim text resolution owned by the editor. No parser, effects, or workspace writes live here.
//!
//! Navigation reads one bounded line chunk at a time, including on oversized single lines.
//! Only register extraction allocates the requested range; it never materializes the snapshot.

use legion_protocol::BufferId;
use legion_text::{TextLineChunk, TextSnapshot};

use crate::{EditorEngine, EditorError, TextPosition, TextRange};

const READ_BYTES: usize = 4096;

/// Text semantics of a parsed Vim motion, independent of UI parser types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// Previous character on this line.
    Left,
    /// Next character on this line.
    Right,
    /// Previous line, clamping the character column.
    Up,
    /// Next line, clamping the character column.
    Down,
    /// Start of the next word class.
    WordForward,
    /// Start of this or the preceding word class.
    WordBackward,
    /// End of this or the next word class.
    WordEnd,
    /// First character of this line.
    LineStart,
    /// Last character of this line.
    LineEnd,
    /// First non-whitespace character, or zero on a blank line.
    FirstNonBlank,
    /// First line, column zero.
    FileStart,
    /// Last line, column zero.
    FileEnd,
    /// Next matching character on this line.
    FindChar(char),
    /// Character before the next match on this line.
    TillChar(char),
}

/// An exclusive native-coordinate range with register paste semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorRange {
    /// Byte-based editor range.
    pub range: TextRange,
    /// Whether the register should be put as whole lines.
    pub linewise: bool,
}

impl EditorEngine {
    /// Resolve a motion against the current immutable snapshot without reading full text.
    pub fn resolve_vim_motion(
        &self,
        buffer_id: BufferId,
        cursor: TextPosition,
        motion: Motion,
        count: usize,
    ) -> Result<TextPosition, EditorError> {
        Reader::new(self.vim_snapshot(buffer_id)?).motion(cursor, motion, count)
    }

    /// Resolve an operator motion; a stationary motion has no range and costs no undo entry.
    pub fn resolve_vim_operator_range(
        &self,
        buffer_id: BufferId,
        cursor: TextPosition,
        motion: Motion,
        count: usize,
    ) -> Result<Option<OperatorRange>, EditorError> {
        let mut reader = Reader::new(self.vim_snapshot(buffer_id)?);
        let from = reader.clamp(cursor)?;
        let to = reader.motion(from, motion, count)?;
        if from == to {
            return Ok(None);
        }
        let forward = (from.line, from.column) <= (to.line, to.column);
        let (start, mut end) = if forward { (from, to) } else { (to, from) };
        if forward
            && matches!(
                motion,
                Motion::WordEnd | Motion::FindChar(_) | Motion::TillChar(_) | Motion::LineEnd
            )
        {
            end = reader.next(end)?;
        }
        Ok(Some(OperatorRange {
            range: TextRange::new(start, end),
            linewise: false,
        }))
    }

    /// Resolve whole lines, including terminators and an unterminated final line.
    pub fn resolve_vim_linewise_range(
        &self,
        buffer_id: BufferId,
        cursor: TextPosition,
        count: usize,
    ) -> Result<OperatorRange, EditorError> {
        let snapshot = self.vim_snapshot(buffer_id)?;
        let first = cursor.line.min(snapshot.line_count() - 1);
        let last = first
            .saturating_add(count.max(1) - 1)
            .min(snapshot.line_count() - 1);
        let end = if last + 1 < snapshot.line_count() {
            TextPosition::new(last + 1, 0)
        } else {
            TextPosition::new(last, snapshot.line_index().line_byte_len(last)?)
        };
        Ok(OperatorRange {
            range: TextRange::new(TextPosition::new(first, 0), end),
            linewise: true,
        })
    }

    /// Resolve `x` without treating an empty line as a character or consuming its terminator.
    pub fn resolve_vim_character_range(
        &self,
        buffer_id: BufferId,
        cursor: TextPosition,
    ) -> Result<Option<OperatorRange>, EditorError> {
        let mut reader = Reader::new(self.vim_snapshot(buffer_id)?);
        // Unlike normal motion clamping, `x` at an insertion position after the last
        // character remains a no-op, matching the existing dispatch behavior.
        if cursor.line >= reader.snapshot.line_count()
            || cursor.column >= reader.len(cursor.line)?
        {
            return Ok(None);
        }
        let start = reader.clamp(cursor)?;
        Ok(Some(OperatorRange {
            range: TextRange::new(start, reader.next(start)?),
            linewise: false,
        }))
    }

    /// Copy only the requested register payload from rope slices, retaining exact line endings.
    pub fn vim_range_text(
        &self,
        buffer_id: BufferId,
        range: TextRange,
    ) -> Result<String, EditorError> {
        let snapshot = self.vim_snapshot(buffer_id)?;
        let start = snapshot.line_index().byte_offset(range.start)?;
        let end = snapshot.line_index().byte_offset(range.end)?;
        if start > end {
            return Err(EditorError::InvalidEdit("Vim range is reversed"));
        }
        let rope = snapshot.rope();
        let mut text = String::with_capacity(end - start);
        for chunk in rope.byte_slice(start..end).chunks() {
            text.push_str(chunk);
        }
        Ok(text)
    }

    fn vim_snapshot(&self, buffer_id: BufferId) -> Result<&TextSnapshot, EditorError> {
        Ok(&self
            .buffers
            .get(&buffer_id)
            .ok_or(EditorError::BufferNotFound(buffer_id))?
            .current_snapshot)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Whitespace,
    Word,
    Punctuation,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Punctuation
    }
}

struct Reader<'a> {
    snapshot: &'a TextSnapshot,
    chunk: Option<TextLineChunk>,
    #[cfg(test)]
    read_count: usize,
}

impl<'a> Reader<'a> {
    fn new(snapshot: &'a TextSnapshot) -> Self {
        Self {
            snapshot,
            chunk: None,
            #[cfg(test)]
            read_count: 0,
        }
    }

    fn len(&self, line: usize) -> Result<usize, EditorError> {
        Ok(self.snapshot.line_index().line_byte_len(line)?)
    }

    fn start(&self, line: usize) -> Result<usize, EditorError> {
        Ok(self
            .snapshot
            .line_index()
            .byte_offset(TextPosition::new(line, 0))?)
    }

    // Rope clones share storage. Offset arithmetic reads no line strings and handles
    // character boundaries even when the requested byte column falls inside UTF-8.
    fn at_character(&self, line: usize, character: usize) -> Result<TextPosition, EditorError> {
        let rope = self.snapshot.rope();
        let start = self.start(line)?;
        let first = rope.byte_to_char(start);
        let end = rope.byte_to_char(start + self.len(line)?);
        let target = first
            .saturating_add(character)
            .min(end.saturating_sub(1).max(first));
        Ok(TextPosition::new(line, rope.char_to_byte(target) - start))
    }

    fn character_column(&self, position: TextPosition) -> Result<usize, EditorError> {
        let rope = self.snapshot.rope();
        let start = self.start(position.line)?;
        Ok(rope.byte_to_char(start + position.column) - rope.byte_to_char(start))
    }

    fn clamp(&self, cursor: TextPosition) -> Result<TextPosition, EditorError> {
        let line = cursor.line.min(self.snapshot.line_count() - 1);
        let column = cursor.column.min(self.len(line)?);
        self.at_character(
            line,
            self.character_column(TextPosition::new(line, column))?,
        )
    }

    fn previous(&self, position: TextPosition) -> Result<TextPosition, EditorError> {
        let rope = self.snapshot.rope();
        let start = self.start(position.line)?;
        let first = rope.byte_to_char(start);
        let target = rope
            .byte_to_char(start + position.column)
            .saturating_sub(1)
            .max(first);
        Ok(TextPosition::new(
            position.line,
            rope.char_to_byte(target) - start,
        ))
    }

    fn char_at(&mut self, position: TextPosition) -> Result<Option<char>, EditorError> {
        if position.column >= self.len(position.line)? {
            return Ok(None);
        }
        let absolute = self.start(position.line)? + position.column;
        if self.chunk.as_ref().is_none_or(|chunk| {
            chunk.line != position.line || absolute < chunk.start_byte || absolute >= chunk.end_byte
        }) {
            // Center the bounded read so backward scans also reuse a chunk. Align
            // through the rope rather than assuming a byte subtraction is UTF-8 safe.
            let rope = self.snapshot.rope();
            let candidate = absolute
                .saturating_sub(READ_BYTES / 2)
                .max(self.start(position.line)?);
            let start = rope.char_to_byte(rope.byte_to_char(candidate));
            self.chunk = Some(self.snapshot.line_chunk_from_byte(
                position.line,
                start,
                READ_BYTES,
            )?);
            #[cfg(test)]
            {
                self.read_count += 1;
            }
        }
        let chunk = self.chunk.as_ref().expect("bounded chunk loaded");
        Ok(chunk.text[absolute - chunk.start_byte..].chars().next())
    }

    fn next(&mut self, position: TextPosition) -> Result<TextPosition, EditorError> {
        Ok(TextPosition::new(
            position.line,
            position.column + self.char_at(position)?.map_or(0, char::len_utf8),
        ))
    }

    fn motion(
        &mut self,
        cursor: TextPosition,
        motion: Motion,
        count: usize,
    ) -> Result<TextPosition, EditorError> {
        let mut position = self.clamp(cursor)?;
        for _ in 0..count.max(1) {
            let next = self.step(position, motion)?;
            if next == position {
                break;
            }
            position = next;
        }
        Ok(position)
    }

    fn step(&mut self, p: TextPosition, motion: Motion) -> Result<TextPosition, EditorError> {
        match motion {
            Motion::Left => self.previous(p),
            Motion::Right => {
                let next = self.next(p)?;
                self.clamp(next)
            }
            Motion::Up | Motion::Down => {
                let line = if motion == Motion::Up {
                    p.line.saturating_sub(1)
                } else {
                    (p.line + 1).min(self.snapshot.line_count() - 1)
                };
                self.at_character(line, self.character_column(p)?)
            }
            Motion::LineStart => Ok(TextPosition::new(p.line, 0)),
            Motion::LineEnd => self.at_character(p.line, usize::MAX),
            Motion::FirstNonBlank => {
                let mut at = TextPosition::new(p.line, 0);
                while let Some(c) = self.char_at(at)? {
                    if !c.is_whitespace() {
                        return Ok(at);
                    }
                    at = self.next(at)?;
                }
                Ok(TextPosition::new(p.line, 0))
            }
            Motion::FileStart => Ok(TextPosition::zero()),
            Motion::FileEnd => Ok(TextPosition::new(self.snapshot.line_count() - 1, 0)),
            Motion::WordForward => self.word_forward(p),
            Motion::WordBackward => self.word_backward(p),
            Motion::WordEnd => self.word_end(p),
            Motion::FindChar(target) | Motion::TillChar(target) => {
                let mut at = self.next(p)?;
                while let Some(c) = self.char_at(at)? {
                    if c == target {
                        return if matches!(motion, Motion::TillChar(_)) {
                            self.previous(at)
                        } else {
                            Ok(at)
                        };
                    }
                    at = self.next(at)?;
                }
                Ok(p)
            }
        }
    }

    fn word_forward(&mut self, mut p: TextPosition) -> Result<TextPosition, EditorError> {
        let initial = self.char_at(p)?.map(class);
        while let Some(c) = self.char_at(p)? {
            if Some(class(c)) != initial || initial == Some(Class::Whitespace) {
                break;
            }
            p = self.next(p)?;
        }
        loop {
            if p.column >= self.len(p.line)? {
                if p.line + 1 >= self.snapshot.line_count() {
                    return self.at_character(p.line, usize::MAX);
                }
                p = TextPosition::new(p.line + 1, 0);
            } else if self
                .char_at(p)?
                .is_some_and(|c| class(c) == Class::Whitespace)
            {
                p = self.next(p)?;
            } else {
                return Ok(p);
            }
        }
    }

    fn word_backward(&mut self, mut p: TextPosition) -> Result<TextPosition, EditorError> {
        loop {
            if p.column == 0 {
                if p.line == 0 {
                    return Ok(TextPosition::zero());
                }
                p = TextPosition::new(p.line - 1, self.len(p.line - 1)?);
                continue;
            }
            p = self.previous(p)?;
            if self
                .char_at(p)?
                .is_some_and(|c| class(c) != Class::Whitespace)
            {
                break;
            }
        }
        let initial = self.char_at(p)?.map(class);
        while p.column > 0 {
            let previous = self.previous(p)?;
            if self.char_at(previous)?.map(class) != initial {
                break;
            }
            p = previous;
        }
        Ok(p)
    }

    fn word_end(&mut self, cursor: TextPosition) -> Result<TextPosition, EditorError> {
        let mut p = self.next(cursor)?;
        loop {
            if p.column >= self.len(p.line)? {
                if p.line + 1 >= self.snapshot.line_count() {
                    return self.at_character(p.line, usize::MAX);
                }
                p = TextPosition::new(p.line + 1, 0);
            } else if self
                .char_at(p)?
                .is_some_and(|c| class(c) == Class::Whitespace)
            {
                p = self.next(p)?;
            } else {
                break;
            }
        }
        let initial = self.char_at(p)?.map(class);
        loop {
            let next = self.next(p)?;
            if self.char_at(next)?.map(class) != initial {
                break;
            }
            p = next;
        }
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use legion_protocol::{FileId, WorkspaceId};
    use legion_text::DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES;

    use super::*;

    fn engine(text: &str) -> (EditorEngine, BufferId) {
        let mut editor = EditorEngine::new();
        let id = editor
            .open_buffer(WorkspaceId(1), FileId(1), "vim.txt", text)
            .unwrap();
        (editor, id)
    }

    #[test]
    fn native_positions_keep_unicode_and_short_line_clamping() {
        let (editor, id) = engine("é🦀 abc\nx\n");
        let to = editor
            .resolve_vim_motion(id, TextPosition::zero(), Motion::Right, 1)
            .unwrap();
        assert_eq!(to, TextPosition::new(0, 2));
        let to = editor.resolve_vim_motion(id, to, Motion::Down, 1).unwrap();
        assert_eq!(to, TextPosition::new(1, 0));
        let range = editor
            .resolve_vim_operator_range(id, TextPosition::zero(), Motion::FindChar('🦀'), 1)
            .unwrap()
            .unwrap();
        assert_eq!(
            range.range,
            TextRange::new(TextPosition::zero(), TextPosition::new(0, 6))
        );
        assert_eq!(editor.vim_range_text(id, range.range).unwrap(), "é🦀");
    }

    #[test]
    fn linewise_ranges_preserve_crlf_and_unterminated_final_text() {
        let (editor, id) = engine("é\r\n\r\n🦀");
        let range = editor
            .resolve_vim_linewise_range(id, TextPosition::zero(), 2)
            .unwrap();
        assert!(range.linewise);
        assert_eq!(editor.vim_range_text(id, range.range).unwrap(), "é\r\n\r\n");
        let range = editor
            .resolve_vim_linewise_range(id, TextPosition::new(2, 0), usize::MAX)
            .unwrap();
        assert_eq!(editor.vim_range_text(id, range.range).unwrap(), "🦀");
        assert_eq!(range.range.end, TextPosition::new(2, 4));
    }

    #[test]
    fn empty_and_stationary_motions_have_no_operator_range() {
        let (editor, id) = engine("");
        for motion in [
            Motion::Right,
            Motion::WordForward,
            Motion::WordBackward,
            Motion::WordEnd,
            Motion::LineEnd,
            Motion::FindChar('z'),
            Motion::TillChar('z'),
        ] {
            assert_eq!(
                editor
                    .resolve_vim_motion(id, TextPosition::zero(), motion, usize::MAX)
                    .unwrap(),
                TextPosition::zero()
            );
            assert!(
                editor
                    .resolve_vim_operator_range(id, TextPosition::zero(), motion, 1)
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            editor
                .resolve_vim_character_range(id, TextPosition::zero())
                .unwrap()
                .is_none()
        );
        // Resolution retains an empty linewise range; the app decides change-mode effects.
        assert!(
            editor
                .resolve_vim_linewise_range(id, TextPosition::zero(), 1)
                .unwrap()
                .range
                .is_empty()
        );
        let (editor, id) = engine("é");
        assert!(
            editor
                .resolve_vim_character_range(id, TextPosition::new(0, 2))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn chunk_edges_do_not_split_multibyte_find_or_backward_word_ranges() {
        let text = format!("{}é🦀 next", " ".repeat(READ_BYTES - 1));
        let (editor, id) = engine(&text);
        let to = editor
            .resolve_vim_motion(id, TextPosition::zero(), Motion::FindChar('🦀'), 1)
            .unwrap();
        assert_eq!(to.column, READ_BYTES + 1);
        let range = editor
            .resolve_vim_operator_range(id, to, Motion::WordBackward, 1)
            .unwrap()
            .unwrap();
        assert_eq!(editor.vim_range_text(id, range.range).unwrap(), "é");
        let to = editor
            .resolve_vim_motion(id, TextPosition::zero(), Motion::FirstNonBlank, 1)
            .unwrap();
        assert_eq!(to.column, READ_BYTES - 1);
    }

    #[test]
    fn oversized_streamed_document_resolves_near_eof_with_bounded_reads() {
        // Neither fixture construction nor resolution builds a document-sized String.
        // The first logical line itself exceeds the cache budget.
        let length = DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 1024;
        let mut source = std::io::repeat(b'x')
            .take(length as u64)
            .chain(std::io::Cursor::new("\né🦀 fin\n".as_bytes()));
        let mut fixture = tempfile::NamedTempFile::new().unwrap();
        std::io::copy(&mut source, &mut fixture).unwrap();
        fixture.flush().unwrap();
        let mut editor = EditorEngine::new();
        let id = editor
            .open_buffer_streaming(WorkspaceId(1), FileId(1), "large.txt", fixture.path())
            .unwrap();
        let snapshot = editor.vim_snapshot(id).unwrap();
        assert!(
            editor
                .buffers
                .get(&id)
                .unwrap()
                .buffer
                .try_full_text()
                .is_err()
        );
        assert!(snapshot.try_full_text().is_err());
        let mut reader = Reader::new(snapshot);
        let tail = reader
            .motion(TextPosition::zero(), Motion::FileEnd, 1)
            .unwrap();
        assert_eq!(tail, TextPosition::new(2, 0));
        assert_eq!(reader.read_count, 0, "file motion reads only line metadata");
        let end = reader
            .motion(TextPosition::new(0, length), Motion::LineEnd, 1)
            .unwrap();
        assert_eq!(end, TextPosition::new(0, length - 1));
        assert_eq!(
            reader.read_count, 0,
            "oversized line end does not read the line"
        );
        let to = reader
            .motion(TextPosition::new(1, 0), Motion::FindChar('🦀'), 1)
            .unwrap();
        assert_eq!(to, TextPosition::new(1, 2));
        assert_eq!(
            reader.read_count, 1,
            "near-EOF search reads only the requested line chunk"
        );
        assert!(reader.chunk.as_ref().unwrap().text.len() <= READ_BYTES);
        // Exercise the same current-snapshot entry points used by app operators.
        let range = editor
            .resolve_vim_operator_range(id, TextPosition::new(1, 0), Motion::FindChar('🦀'), 1)
            .unwrap()
            .unwrap();
        assert_eq!(editor.vim_range_text(id, range.range).unwrap(), "é🦀");
        assert!(editor.vim_snapshot(id).unwrap().try_full_text().is_err());
    }
}
