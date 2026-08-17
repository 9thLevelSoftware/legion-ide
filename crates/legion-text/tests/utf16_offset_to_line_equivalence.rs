//! `LineIndex::utf16_offset_to_line` against the line walk it replaced.
//!
//! `EditorEngine::byte_offset_from_absolute_utf16` used to find the line holding a UTF-16
//! offset by walking from the start of the buffer, subtracting each line's content and
//! ending lengths. That is O(document length) on the completion path, so the search was
//! replaced with an O(log n) rope conversion.
//!
//! Unlike the forward direction (`utf16_offset_equivalence.rs`), the replacement is **not**
//! bug-for-bug identical, and that is deliberate. The walk could never leave a residual of
//! zero for any line after the first: an offset landing on a line ending was clamped to
//! that line's content end before the next line was considered, so an offset addressing
//! the *start* of a line resolved to the end of the previous one.
//!
//! Two tests, because "it is different" is only acceptable with both halves shown:
//!
//! * the new search is checked against a straightforward correct reference, at every
//!   UTF-16 offset of every fixture; and
//! * the divergence from the old walk is pinned to **exactly** the set of line-start
//!   offsets, so the behaviour change is bounded and proved rather than asserted.

use legion_text::LineIndex;

/// Fixtures paired with what each is here to catch. Same set as the forward direction,
/// so the two notes describe the same corpus.
fn fixtures() -> Vec<(&'static str, &'static str)> {
    vec![
        ("empty", ""),
        ("no trailing newline", "alpha\nbeta\ngamma"),
        ("trailing newline", "alpha\nbeta\ngamma\n"),
        ("blank lines", "\n\n\nalpha\n\n"),
        ("crlf", "alpha\r\nbeta\r\ngamma\r\n"),
        ("lone cr", "alpha\rbeta\rgamma"),
        ("mixed endings", "alpha\r\nbeta\ngamma\rdelta\n"),
        ("two byte", "café\nnaïve\nsoupçon\n"),
        ("three byte", "日本語\nテスト\n"),
        ("astral plane", "a😀b\n😀😀\nc\n"),
        ("astral with crlf", "😀\r\n😀x\r\n"),
        (
            "unicode line-like",
            "a\u{0b}b\nc\u{0c}d\ne\u{85}f\ng\u{2028}h\ni\u{2029}j\n",
        ),
    ]
}

/// Total UTF-16 code units in `text`.
fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// A straightforward correct reference: the line whose UTF-16 span contains `offset`,
/// with the last line owning its end so end-of-buffer resolves.
///
/// Shares no machinery with the rope conversion under test — it sums the same per-line
/// metrics the old walk did, but without the clamp that made the walk skip line starts.
fn reference(index: &LineIndex, offset: usize) -> Option<(usize, usize)> {
    let count = index.line_count();
    let mut cumulative = 0usize;
    for line in 0..count {
        let span = index.line_utf16_len(line).ok()? + index.line_ending_bytes(line).ok()?;
        let is_last = line + 1 == count;
        if offset < cumulative + span || (is_last && offset == cumulative + span) {
            return Some((line, offset - cumulative));
        }
        cumulative += span;
    }
    None
}

/// The walk `byte_offset_from_absolute_utf16` used to do, kept verbatim, reported as the
/// `(line, column)` it would have resolved to — the column already clamped, since that is
/// what determined the byte offset it returned.
fn walked(index: &LineIndex, requested: usize) -> Option<(usize, usize)> {
    let mut remaining = requested;
    for line in 0..index.line_count() {
        let content = index.line_utf16_len(line).ok()?;
        if remaining <= content {
            return Some((line, remaining));
        }
        remaining -= content;

        let ending = index.line_ending_bytes(line).ok()?;
        if remaining <= ending {
            return Some((line, content));
        }
        remaining -= ending;
    }
    None
}

/// The clamped column the new path resolves to, for comparison with [`walked`].
fn resolved(index: &LineIndex, offset: usize) -> Option<(usize, usize)> {
    let (line, within) = index.utf16_offset_to_line(offset)?;
    let column = within.min(index.line_utf16_len(line).ok()?);
    Some((line, column))
}

#[test]
fn the_search_matches_a_correct_reference_at_every_offset() {
    for (name, text) in fixtures() {
        let index = LineIndex::new(text);
        for offset in 0..=utf16_len(text) {
            assert_eq!(
                index.utf16_offset_to_line(offset),
                reference(&index, offset),
                "fixture {name:?} disagreed at UTF-16 offset {offset}"
            );
        }
    }
}

#[test]
fn offsets_past_the_end_of_the_buffer_resolve_to_nothing() {
    for (name, text) in fixtures() {
        let index = LineIndex::new(text);
        let past = utf16_len(text) + 1;
        assert_eq!(
            index.utf16_offset_to_line(past),
            None,
            "fixture {name:?} resolved an offset past its end"
        );
    }
}

#[test]
fn the_only_offsets_that_changed_are_line_starts() {
    for (name, text) in fixtures() {
        let index = LineIndex::new(text);

        // The UTF-16 offset of the start of each line after the first: the offsets the
        // old walk could not represent.
        let mut line_starts = Vec::new();
        let mut cumulative = 0usize;
        for line in 0..index.line_count() {
            if line > 0 {
                line_starts.push(cumulative);
            }
            cumulative += index.line_utf16_len(line).expect("line in range")
                + index.line_ending_bytes(line).expect("line in range");
        }

        for offset in 0..=utf16_len(text) {
            let old = walked(&index, offset);
            let new = resolved(&index, offset);
            if line_starts.contains(&offset) {
                assert_ne!(
                    old, new,
                    "fixture {name:?}: line start {offset} was expected to change"
                );
                assert_eq!(
                    new.map(|(_, column)| column),
                    Some(0),
                    "fixture {name:?}: line start {offset} must resolve to column 0"
                );
            } else {
                assert_eq!(
                    old, new,
                    "fixture {name:?}: offset {offset} is not a line start and must not change"
                );
            }
        }
    }
}
