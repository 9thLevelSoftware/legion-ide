//! Resolving a Vim motion against buffer text.
//!
//! [`VimState`](crate::vim::VimState) turns keystrokes into a
//! [`VimMotionKind`], which says *what* the user asked for but nothing about
//! where that lands: `w` means "start of the next word", and only the text
//! knows where that is. This module is the missing half, and it is deliberately
//! pure — text and a cursor in, a cursor out — so the whole of Vim's motion
//! behaviour is testable without a buffer, an editor, or an app.
//!
//! Two conventions worth stating, because they are where a naive
//! implementation and real Vim part company:
//!
//! * **Columns are characters, not bytes.** A motion over `café` moves by one
//!   per character; measuring in bytes would put the cursor inside `é`.
//! * **Clamping is per line.** Moving down from a long line onto a short one
//!   lands at the short line's end rather than failing or overshooting, which
//!   is what Vim does.

use legion_protocol::TextCoordinate;

use crate::vim::VimMotionKind;

/// Resolve `motion`, repeated `count` times, from `cursor` in `text`.
///
/// `count` is clamped to at least 1, matching the parser, which never emits
/// zero. The returned coordinate carries no byte or UTF-16 offset: those are
/// snapshot-relative and belong to whoever applies the move.
pub fn resolve_motion(
    text: &str,
    cursor: TextCoordinate,
    motion: VimMotionKind,
    count: usize,
) -> TextCoordinate {
    let lines = line_chars(text);
    let mut position = clamp_to_text(&lines, cursor);
    let repeats = count.max(1);

    for _ in 0..repeats {
        position = step(&lines, position, motion);
    }
    position
}

/// One application of `motion`.
///
/// Split out so `count` is a loop rather than arithmetic inside each arm —
/// `3w` is three word motions, and word motions are not a fixed distance.
fn step(lines: &[Vec<char>], position: TextCoordinate, motion: VimMotionKind) -> TextCoordinate {
    let line = position.line as usize;
    let column = position.character as usize;
    match motion {
        VimMotionKind::Left => coordinate(line, column.saturating_sub(1)),
        VimMotionKind::Right => {
            // Stops at the last character rather than one past it: in normal
            // mode the cursor sits *on* a character, so the end of the line is
            // the last index, not the length.
            let limit = last_index(lines, line);
            coordinate(line, (column + 1).min(limit))
        }
        VimMotionKind::Up => {
            let target = line.saturating_sub(1);
            coordinate(target, column.min(last_index(lines, target)))
        }
        VimMotionKind::Down => {
            let target = (line + 1).min(lines.len().saturating_sub(1));
            coordinate(target, column.min(last_index(lines, target)))
        }
        VimMotionKind::LineStart => coordinate(line, 0),
        VimMotionKind::LineEnd => coordinate(line, last_index(lines, line)),
        VimMotionKind::FirstNonBlank => {
            let first = lines
                .get(line)
                .and_then(|chars| chars.iter().position(|c| !c.is_whitespace()))
                .unwrap_or(0);
            coordinate(line, first)
        }
        VimMotionKind::FileStart => coordinate(0, 0),
        VimMotionKind::FileEnd => coordinate(lines.len().saturating_sub(1), 0),
        VimMotionKind::WordForward => word_forward(lines, line, column),
        VimMotionKind::WordBackward => word_backward(lines, line, column),
        VimMotionKind::WordEnd => word_end(lines, line, column),
        VimMotionKind::FindChar(target) => find_char(lines, line, column, target, 0),
        // `t` stops one short of the match; with no match it does not move,
        // which is why the offset is applied to the found column rather than
        // to the returned one.
        VimMotionKind::TillChar(target) => find_char(lines, line, column, target, 1),
    }
}

/// Character classes, as Vim distinguishes them for word motions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Whitespace,
    /// Letters, digits and `_` — what Vim calls a "word".
    Word,
    /// Everything else: punctuation, operators, brackets.
    Punctuation,
}

fn class_of(c: char) -> CharClass {
    if c.is_whitespace() {
        CharClass::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Punctuation
    }
}

/// `w` — to the start of the next word, crossing lines when needed.
fn word_forward(lines: &[Vec<char>], line: usize, column: usize) -> TextCoordinate {
    let mut line_index = line;
    let mut index = column;
    let start_class = char_at(lines, line_index, index).map(class_of);

    // Leave the current run.
    while let Some(current) = char_at(lines, line_index, index) {
        if Some(class_of(current)) != start_class || start_class == Some(CharClass::Whitespace) {
            break;
        }
        index += 1;
        if index >= line_len(lines, line_index) {
            break;
        }
    }
    // Then skip whitespace, including across line ends.
    loop {
        if index >= line_len(lines, line_index) {
            if line_index + 1 >= lines.len() {
                return coordinate(line_index, last_index(lines, line_index));
            }
            line_index += 1;
            index = 0;
            continue;
        }
        match char_at(lines, line_index, index) {
            Some(current) if class_of(current) == CharClass::Whitespace => index += 1,
            _ => return coordinate(line_index, index),
        }
    }
}

/// `b` — back to the start of this word, or of the previous one.
fn word_backward(lines: &[Vec<char>], line: usize, column: usize) -> TextCoordinate {
    let mut line_index = line;
    let mut index = column;

    loop {
        if index == 0 {
            if line_index == 0 {
                return coordinate(0, 0);
            }
            line_index -= 1;
            index = line_len(lines, line_index);
            continue;
        }
        index -= 1;
        match char_at(lines, line_index, index) {
            Some(current) if class_of(current) != CharClass::Whitespace => break,
            _ => continue,
        }
    }

    let class = char_at(lines, line_index, index).map(class_of);
    while index > 0 {
        match char_at(lines, line_index, index - 1) {
            Some(previous) if Some(class_of(previous)) == class => index -= 1,
            _ => break,
        }
    }
    coordinate(line_index, index)
}

/// `e` — to the last character of this word, or of the next one.
fn word_end(lines: &[Vec<char>], line: usize, column: usize) -> TextCoordinate {
    let mut line_index = line;
    let mut index = column + 1;

    loop {
        if index >= line_len(lines, line_index) {
            if line_index + 1 >= lines.len() {
                return coordinate(line_index, last_index(lines, line_index));
            }
            line_index += 1;
            index = 0;
            continue;
        }
        match char_at(lines, line_index, index) {
            Some(current) if class_of(current) == CharClass::Whitespace => index += 1,
            _ => break,
        }
    }

    let class = char_at(lines, line_index, index).map(class_of);
    while index + 1 < line_len(lines, line_index) {
        match char_at(lines, line_index, index + 1) {
            Some(next) if Some(class_of(next)) == class => index += 1,
            _ => break,
        }
    }
    coordinate(line_index, index)
}

/// `f{char}` and `t{char}` — forward within the current line only.
///
/// Vim confines both to the line; a miss leaves the cursor untouched, which is
/// why "no match" returns the original position rather than the line end.
fn find_char(
    lines: &[Vec<char>],
    line: usize,
    column: usize,
    target: char,
    back_off: usize,
) -> TextCoordinate {
    let Some(chars) = lines.get(line) else {
        return coordinate(line, column);
    };
    for (offset, current) in chars.iter().enumerate().skip(column + 1) {
        if *current == target {
            return coordinate(line, offset.saturating_sub(back_off).max(column));
        }
    }
    coordinate(line, column)
}

fn coordinate(line: usize, character: usize) -> TextCoordinate {
    TextCoordinate {
        line: line as u32,
        character: character as u32,
        byte_offset: None,
        utf16_offset: None,
    }
}

/// Split into lines of characters.
///
/// Characters rather than bytes because every column in this module is a
/// character index; a text with no trailing newline still has a final line,
/// and an empty text has one empty line so a cursor at 0,0 is always valid.
fn line_chars(text: &str) -> Vec<Vec<char>> {
    let mut lines: Vec<Vec<char>> = text.split('\n').map(|l| l.chars().collect()).collect();
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    lines
}

fn line_len(lines: &[Vec<char>], line: usize) -> usize {
    lines.get(line).map(Vec::len).unwrap_or(0)
}

/// The last valid cursor column on `line`.
///
/// Zero for an empty line: normal-mode Vim puts the cursor *on* a character,
/// so the rightmost position is `len - 1`, not `len`.
fn last_index(lines: &[Vec<char>], line: usize) -> usize {
    line_len(lines, line).saturating_sub(1)
}

fn char_at(lines: &[Vec<char>], line: usize, column: usize) -> Option<char> {
    lines.get(line).and_then(|chars| chars.get(column)).copied()
}

/// Bring a cursor from elsewhere into range before moving it.
fn clamp_to_text(lines: &[Vec<char>], cursor: TextCoordinate) -> TextCoordinate {
    let line = (cursor.line as usize).min(lines.len().saturating_sub(1));
    let column = (cursor.character as usize).min(last_index(lines, line));
    coordinate(line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "fn main() {\n    let total = 1;\n}\n";

    fn at(line: u32, character: u32) -> TextCoordinate {
        TextCoordinate {
            line,
            character,
            byte_offset: None,
            utf16_offset: None,
        }
    }

    fn go(cursor: TextCoordinate, motion: VimMotionKind, count: usize) -> (u32, u32) {
        let out = resolve_motion(TEXT, cursor, motion, count);
        (out.line, out.character)
    }

    #[test]
    fn character_motions_move_one_at_a_time() {
        assert_eq!(go(at(0, 3), VimMotionKind::Left, 1), (0, 2));
        assert_eq!(go(at(0, 3), VimMotionKind::Right, 1), (0, 4));
        assert_eq!(go(at(0, 3), VimMotionKind::Left, 3), (0, 0));
    }

    #[test]
    fn a_count_repeats_the_motion() {
        assert_eq!(go(at(0, 0), VimMotionKind::Right, 5), (0, 5));
        assert_eq!(
            go(at(0, 0), VimMotionKind::Right, 0),
            (0, 1),
            "a zero count is treated as one, matching the parser"
        );
    }

    #[test]
    fn motions_stop_at_the_buffer_edges() {
        assert_eq!(go(at(0, 0), VimMotionKind::Left, 10), (0, 0));
        assert_eq!(go(at(0, 0), VimMotionKind::Up, 5), (0, 0));
        assert_eq!(
            go(at(0, 0), VimMotionKind::Right, 500),
            (0, 10),
            "the last column is the last character, not one past it"
        );
    }

    #[test]
    fn moving_onto_a_shorter_line_clamps_to_its_end() {
        // Line 1 is 18 characters; line 2 is just `}`.
        assert_eq!(
            go(at(1, 15), VimMotionKind::Down, 1),
            (2, 0),
            "Vim lands at the short line's end rather than overshooting"
        );
    }

    #[test]
    fn line_motions_find_the_right_columns() {
        assert_eq!(go(at(1, 9), VimMotionKind::LineStart, 1), (1, 0));
        assert_eq!(go(at(1, 0), VimMotionKind::LineEnd, 1), (1, 17));
        assert_eq!(
            go(at(1, 0), VimMotionKind::FirstNonBlank, 1),
            (1, 4),
            "line 1 is indented four spaces"
        );
    }

    #[test]
    fn file_motions_reach_both_ends() {
        assert_eq!(go(at(1, 5), VimMotionKind::FileStart, 1), (0, 0));
        assert_eq!(go(at(0, 0), VimMotionKind::FileEnd, 1), (3, 0));
    }

    #[test]
    fn word_forward_stops_at_each_word_start() {
        // `fn main() {`
        assert_eq!(go(at(0, 0), VimMotionKind::WordForward, 1), (0, 3));
        assert_eq!(
            go(at(0, 3), VimMotionKind::WordForward, 1),
            (0, 7),
            "punctuation is its own word class"
        );
    }

    #[test]
    fn word_forward_crosses_a_line_end() {
        let out = resolve_motion(TEXT, at(0, 10), VimMotionKind::WordForward, 1);
        assert_eq!(
            (out.line, out.character),
            (1, 4),
            "the next word is on the following line, past its indentation"
        );
    }

    #[test]
    fn word_backward_returns_to_the_word_start() {
        assert_eq!(go(at(1, 8), VimMotionKind::WordBackward, 1), (1, 4));
        assert_eq!(go(at(1, 4), VimMotionKind::WordBackward, 1), (0, 10));
    }

    #[test]
    fn word_end_lands_on_the_last_character() {
        assert_eq!(go(at(0, 0), VimMotionKind::WordEnd, 1), (0, 1));
        assert_eq!(go(at(1, 4), VimMotionKind::WordEnd, 1), (1, 6));
    }

    #[test]
    fn find_char_moves_onto_the_match_and_till_stops_before_it() {
        assert_eq!(go(at(0, 0), VimMotionKind::FindChar('('), 1), (0, 7));
        assert_eq!(go(at(0, 0), VimMotionKind::TillChar('('), 1), (0, 6));
    }

    #[test]
    fn find_char_that_misses_does_not_move() {
        assert_eq!(
            go(at(0, 0), VimMotionKind::FindChar('z'), 1),
            (0, 0),
            "Vim confines f/t to the line and leaves the cursor put on a miss"
        );
    }

    #[test]
    fn columns_are_characters_not_bytes() {
        let text = "café au lait\n";
        let out = resolve_motion(text, at(0, 0), VimMotionKind::WordForward, 1);
        assert_eq!(
            (out.line, out.character),
            (0, 5),
            "`au` starts at character 5; counting bytes would land inside the é"
        );
    }

    #[test]
    fn an_empty_buffer_is_navigable() {
        for motion in [
            VimMotionKind::Left,
            VimMotionKind::Right,
            VimMotionKind::Up,
            VimMotionKind::Down,
            VimMotionKind::WordForward,
            VimMotionKind::WordBackward,
            VimMotionKind::WordEnd,
            VimMotionKind::LineEnd,
            VimMotionKind::FileEnd,
        ] {
            let out = resolve_motion("", at(0, 0), motion, 3);
            assert_eq!(
                (out.line, out.character),
                (0, 0),
                "{motion:?} on empty text"
            );
        }
    }

    #[test]
    fn a_cursor_from_outside_the_text_is_brought_into_range() {
        let out = resolve_motion(TEXT, at(99, 99), VimMotionKind::Left, 1);
        assert!(
            out.line <= 3 && out.character == 0,
            "an out-of-range cursor is clamped before moving, got {out:?}"
        );
    }

    #[test]
    fn resolution_never_reports_stale_offsets() {
        let cursor = TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: Some(42),
            utf16_offset: Some(42),
        };
        let out = resolve_motion(TEXT, cursor, VimMotionKind::Right, 1);
        assert_eq!(
            (out.byte_offset, out.utf16_offset),
            (None, None),
            "carrying the old offsets forward would describe a position that moved"
        );
    }
}
