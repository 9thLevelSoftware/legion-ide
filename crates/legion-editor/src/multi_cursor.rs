//! Multiple cursors: creating them, and editing at all of them at once.
//!
//! The buffer has stored `Vec<Cursor>` since before this module existed, and
//! the viewport projection has reported every one of them — but nothing ever
//! created a second, and every edit went to a single position. This is the
//! missing middle.
//!
//! Everything here is pure: text and positions in, text and positions out. The
//! ordering rule that makes multi-cursor editing tractable is easier to get
//! right, and much easier to test, without a buffer in the way.
//!
//! **Edits are applied last-first.** Inserting at three cursors shifts every
//! position after the first one, so applying them in document order means each
//! edit lands at an offset the previous edit already invalidated. Walking
//! backwards means no applied edit can move a position not yet reached.

use legion_text::TextPosition;

/// Sort cursors into document order and drop duplicates.
///
/// Two cursors at one position are one cursor: leaving both would type every
/// character twice at that spot, and the user has no way to tell them apart to
/// remove one.
pub fn normalize(mut cursors: Vec<TextPosition>) -> Vec<TextPosition> {
    cursors.sort_by_key(|position| (position.line, position.column));
    cursors.dedup_by_key(|position| (position.line, position.column));
    cursors
}

/// Add a cursor one line above or below each existing one.
///
/// `delta` is -1 for above and 1 for below. A cursor with nowhere to go — the
/// first line going up, the last going down — adds nothing rather than
/// stacking a duplicate on the line it is already on.
///
/// The column is clamped per line, so a cursor in a long line landing on a
/// short one sits at that line's end. It is *not* remembered: Vim and most
/// editors keep a "desired column" across vertical motion, but a multi-cursor
/// set has no single desired column, and inventing one would make the second
/// `Ctrl-Alt-Down` jump somewhere the user did not point at.
pub fn add_vertical(text: &str, cursors: &[TextPosition], delta: i32) -> Vec<TextPosition> {
    let lines = line_lengths(text);
    let mut out = cursors.to_vec();

    for cursor in cursors {
        let target = match delta {
            d if d < 0 => cursor.line.checked_sub(d.unsigned_abs() as usize),
            d if d > 0 => {
                let next = cursor.line + d as usize;
                (next < lines.len()).then_some(next)
            }
            _ => None,
        };
        let Some(line) = target else { continue };
        out.push(TextPosition::new(
            line,
            cursor.column.min(lines.get(line).copied().unwrap_or(0)),
        ));
    }
    normalize(out)
}

/// Insert `insert` at every cursor, returning the new text and cursors.
///
/// Each cursor ends after the text it inserted, which is what typing feels
/// like: the caret follows the character.
pub fn insert_at_all(
    text: &str,
    cursors: &[TextPosition],
    insert: &str,
) -> (String, Vec<TextPosition>) {
    let cursors = normalize(cursors.to_vec());
    let mut out = text.to_string();

    // Last first: an insertion shifts every position after it, so applying in
    // document order would land each edit at an offset the previous one had
    // already moved.
    for cursor in cursors.iter().rev() {
        let offset = byte_offset(&out, *cursor);
        out.insert_str(offset, insert);
    }

    let moved = shift_cursors(&cursors, insert);
    (out, moved)
}

/// Delete one character before every cursor — what Backspace does.
///
/// A cursor at the very start of the buffer deletes nothing, and keeps its
/// place. Returning it unchanged rather than dropping it matters: losing a
/// cursor because it happened to sit at offset zero would silently shrink the
/// set the user is editing with.
pub fn delete_before_all(text: &str, cursors: &[TextPosition]) -> (String, Vec<TextPosition>) {
    let cursors = normalize(cursors.to_vec());
    let mut out = text.to_string();
    let mut moved: Vec<TextPosition> = Vec::with_capacity(cursors.len());

    // Compute every deletion against the original text first, so the offsets
    // do not depend on the order they are applied in.
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for cursor in &cursors {
        let offset = byte_offset(text, *cursor);
        if offset == 0 {
            continue;
        }
        let previous = text[..offset]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
        cuts.push((previous, offset));
    }

    for (start, end) in cuts.iter().rev() {
        out.replace_range(start..end, "");
    }

    // Each surviving cursor moves back by every deletion at or before it.
    for cursor in &cursors {
        let offset = byte_offset(text, *cursor);
        let removed: usize = cuts
            .iter()
            .filter(|(_, end)| *end <= offset)
            .map(|(start, end)| end - start)
            .sum();
        moved.push(position_of(&out, offset.saturating_sub(removed)));
    }
    (out, normalize(moved))
}

/// Move each cursor forward by the insertions made at and before it.
fn shift_cursors(cursors: &[TextPosition], insert: &str) -> Vec<TextPosition> {
    let inserted_lines = insert.matches('\n').count();
    let trailing = insert.rsplit('\n').next().unwrap_or("").len();

    cursors
        .iter()
        .enumerate()
        .map(|(index, cursor)| {
            // Every earlier cursor's insertion pushes this one down by the
            // lines it added; its own insertion moves it along its line.
            let line = cursor.line + inserted_lines * (index + 1);
            let column = if inserted_lines > 0 {
                trailing
            } else {
                cursor.column + insert.len() * (index + 1) - insert.len() * index
            };
            TextPosition::new(line, column)
        })
        .collect()
}

/// Byte lengths of each line, excluding terminators.
fn line_lengths(text: &str) -> Vec<usize> {
    text.split('\n').map(str::len).collect()
}

/// Byte offset of a position in `text`, saturating at the end.
fn byte_offset(text: &str, position: TextPosition) -> usize {
    let mut offset = 0;
    for (index, line) in text.split('\n').enumerate() {
        if index == position.line {
            return (offset + position.column.min(line.len())).min(text.len());
        }
        offset += line.len() + 1;
    }
    text.len()
}

/// Position of a byte offset in `text`.
fn position_of(text: &str, offset: usize) -> TextPosition {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count();
    let column = before.rsplit('\n').next().unwrap_or("").len();
    TextPosition::new(line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "one\ntwo\nthree\n";

    fn at(line: usize, column: usize) -> TextPosition {
        TextPosition::new(line, column)
    }

    #[test]
    fn duplicate_cursors_collapse_to_one() {
        let cursors = normalize(vec![at(1, 2), at(0, 0), at(1, 2)]);
        assert_eq!(
            cursors,
            vec![at(0, 0), at(1, 2)],
            "two cursors in one place would type every character twice there"
        );
    }

    #[test]
    fn adding_below_puts_one_on_each_following_line() {
        let cursors = add_vertical(TEXT, &[at(0, 1)], 1);
        assert_eq!(cursors, vec![at(0, 1), at(1, 1)]);
    }

    #[test]
    fn adding_above_puts_one_on_each_preceding_line() {
        let cursors = add_vertical(TEXT, &[at(2, 1)], -1);
        assert_eq!(cursors, vec![at(1, 1), at(2, 1)]);
    }

    #[test]
    fn a_cursor_with_nowhere_to_go_adds_nothing() {
        assert_eq!(
            add_vertical(TEXT, &[at(0, 0)], -1),
            vec![at(0, 0)],
            "stacking a duplicate on the same line would be worse than nothing"
        );
    }

    #[test]
    fn a_new_cursor_clamps_to_a_shorter_line() {
        let text = "longer line\nab\n";
        assert_eq!(add_vertical(text, &[at(0, 9)], 1), vec![at(0, 9), at(1, 2)]);
    }

    #[test]
    fn inserting_at_two_cursors_lands_in_both_places() {
        let (text, cursors) = insert_at_all("ab\ncd\n", &[at(0, 1), at(1, 1)], "X");
        assert_eq!(text, "aXb\ncXd\n");
        assert_eq!(
            cursors,
            vec![at(0, 2), at(1, 2)],
            "each caret follows the character it typed"
        );
    }

    #[test]
    fn insertion_order_does_not_corrupt_later_positions() {
        // Three cursors on one line: applying front-to-back would place the
        // second and third insertions at offsets the first had already moved.
        let (text, _) = insert_at_all("abcd\n", &[at(0, 1), at(0, 2), at(0, 3)], "-");
        assert_eq!(text, "a-b-c-d\n");
    }

    #[test]
    fn backspace_at_two_cursors_removes_both_characters() {
        let (text, cursors) = delete_before_all("ab\ncd\n", &[at(0, 2), at(1, 2)]);
        assert_eq!(text, "a\nc\n");
        assert_eq!(cursors, vec![at(0, 1), at(1, 1)]);
    }

    #[test]
    fn backspace_at_the_start_of_the_buffer_keeps_the_cursor() {
        let (text, cursors) = delete_before_all("ab\n", &[at(0, 0)]);
        assert_eq!(text, "ab\n");
        assert_eq!(
            cursors,
            vec![at(0, 0)],
            "dropping it would silently shrink the set the user is editing with"
        );
    }

    #[test]
    fn backspace_joins_a_line_when_the_cursor_is_at_its_start() {
        let (text, _) = delete_before_all("ab\ncd\n", &[at(1, 0)]);
        assert_eq!(text, "abcd\n");
    }

    #[test]
    fn inserting_a_newline_moves_every_later_cursor_down() {
        let (text, cursors) = insert_at_all("ab\ncd\n", &[at(0, 1), at(1, 1)], "\n");
        assert_eq!(text, "a\nb\nc\nd\n");
        assert_eq!(
            cursors.len(),
            2,
            "both cursors survive a newline insertion: {cursors:?}"
        );
        assert!(
            cursors[0].line < cursors[1].line,
            "the second cursor moved below the line the first one split"
        );
    }

    #[test]
    fn multibyte_text_is_not_split() {
        let (text, _) = insert_at_all("café\n", &[at(0, 5)], "!");
        assert_eq!(
            text, "café!\n",
            "column 5 is the byte after é, so the insert lands cleanly"
        );

        let (deleted, _) = delete_before_all("café\n", &[at(0, 5)]);
        assert_eq!(
            deleted, "caf\n",
            "backspace removes the whole é, not one of its two bytes"
        );
    }

    #[test]
    fn an_empty_cursor_set_changes_nothing() {
        let (text, cursors) = insert_at_all(TEXT, &[], "X");
        assert_eq!(text, TEXT);
        assert!(cursors.is_empty());
    }
}
