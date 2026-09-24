//! `LineIndex::utf16_offset` must answer exactly what summing the lines answered.
//!
//! `EditorEngine::absolute_utf16_offset` used to compute the absolute UTF-16 offset by
//! summing `line_utf16_len` and `line_ending_bytes` over every preceding line. That is
//! O(lines) and it dominated the cost of a deep viewport projection, so it was replaced
//! by `LineIndex::utf16_offset`, which asks the rope in O(log n).
//!
//! The replacement is only safe if it is indistinguishable from what it replaced. UTF-16
//! offsets address LSP positions, so a silent one-unit drift would land diagnostics,
//! breakpoints and proposal ranges on the wrong character with nothing failing. This
//! keeps the old summation as an oracle and compares the two at **every byte offset** of
//! fixtures chosen for the cases where they could plausibly disagree: the three line
//! endings, an offset inside a CRLF pair, surrogate pairs, and characters that look like
//! line breaks to other implementations but are not line breaks here.

use legion_text::LineIndex;

/// The implementation `utf16_offset` replaced, kept verbatim as an oracle.
///
/// Shares none of the new path's machinery: it walks lines and sums metrics, where
/// `utf16_offset` does a rope conversion.
fn summed_utf16_offset(index: &LineIndex, byte_offset: usize) -> Option<usize> {
    let position = index.utf16_position(byte_offset).ok()?;
    let mut total = position.character;
    for line in 0..position.line {
        total = total
            .saturating_add(index.line_utf16_len(line).ok()?)
            .saturating_add(index.line_ending_bytes(line).ok()?);
    }
    Some(total)
}

/// Fixtures paired with what each one is here to catch.
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
        // The emoji is a surrogate pair: two UTF-16 units for four bytes, which is where
        // a byte-counting mistake would show up as a systematic drift.
        ("astral plane", "a😀b\n😀😀\nc\n"),
        ("astral with crlf", "😀\r\n😀x\r\n"),
        // Characters other line-breaking rules treat as terminators. Legion does not, so
        // these must count as ordinary content in both formulations.
        (
            "unicode line-like",
            "a\u{0b}b\nc\u{0c}d\ne\u{85}f\ng\u{2028}h\ni\u{2029}j\n",
        ),
    ]
}

#[test]
fn utf16_offset_matches_the_summation_it_replaced_at_every_offset() {
    for (name, text) in fixtures() {
        let index = LineIndex::new(text);
        for offset in 0..=text.len() {
            let expected = summed_utf16_offset(&index, offset);
            let actual = index.utf16_offset(offset).ok();
            assert_eq!(
                actual, expected,
                "fixture {name:?} disagreed at byte offset {offset}"
            );
        }
    }
}

#[test]
fn utf16_offset_rejects_the_same_offsets_the_summation_rejected() {
    for (name, text) in fixtures() {
        let index = LineIndex::new(text);
        for offset in 0..=text.len() {
            assert_eq!(
                index.utf16_offset(offset).is_err(),
                index.utf16_position(offset).is_err(),
                "fixture {name:?} disagreed on whether byte offset {offset} is addressable"
            );
        }
    }
}

#[test]
fn utf16_offset_of_the_buffer_end_is_the_whole_buffer_in_utf16_units() {
    for (name, text) in fixtures() {
        let index = LineIndex::new(text);
        let expected: usize = text.chars().map(char::len_utf16).sum();
        assert_eq!(
            index.utf16_offset(text.len()).expect("end is addressable"),
            expected,
            "fixture {name:?} did not end at its own UTF-16 length"
        );
    }
}

#[test]
fn an_offset_inside_a_crlf_pair_clamps_to_the_end_of_the_line_content() {
    // "ab\r\ncd": offset 3 sits between the CR and the LF. It is a character boundary,
    // so it is addressable, and both formulations clamp it to the end of "ab".
    let index = LineIndex::new("ab\r\ncd");
    assert_eq!(index.utf16_offset(3).expect("addressable"), 2);
    assert_eq!(summed_utf16_offset(&index, 3), Some(2));
}
