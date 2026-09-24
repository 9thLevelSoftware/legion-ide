use std::io::Cursor;

use legion_text::{DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES, TextBuffer, TextError};
use ropey::Rope;
use unicode_segmentation::UnicodeSegmentation;

fn boundaries(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            text.grapheme_indices(true)
                .map(|(start, grapheme)| start + grapheme.len()),
        )
        .collect()
}

fn oracle(boundaries: &[usize], offset: usize) -> (Option<usize>, Option<usize>) {
    (
        boundaries.iter().copied().rfind(|&b| b < offset),
        boundaries.iter().copied().find(|&b| b > offset),
    )
}

#[test]
fn grapheme_boundaries_cover_unicode_cases_and_explicit_bytes() {
    let cases = [
        ("", &[0][..]),
        ("abc", &[0, 1, 2, 3][..]),
        ("a\u{301}b", &[0, 3, 4][..]),
        ("a\r\nb", &[0, 1, 3, 4][..]),
        ("👩‍💻x", &[0, 11, 12][..]),
        ("🇺🇸🇨🇦", &[0, 8, 16][..]),
        ("क्‍षx", &[0, 12, 13][..]),
    ];
    for (text, expected_boundaries) in cases {
        assert_eq!(
            boundaries(text),
            expected_boundaries,
            "oracle bytes for {text:?}"
        );
        let buffer = TextBuffer::new(text);
        for offset in (0..=text.len()).filter(|offset| text.is_char_boundary(*offset)) {
            let expected = oracle(expected_boundaries, offset);
            assert_eq!(
                buffer.previous_grapheme_boundary(offset).unwrap(),
                expected.0
            );
            assert_eq!(buffer.next_grapheme_boundary(offset).unwrap(), expected.1);
        }
    }
}

#[test]
fn grapheme_boundaries_reject_invalid_offsets_in_both_directions() {
    let buffer = TextBuffer::new("é");
    for result in [
        buffer.next_grapheme_boundary(1),
        buffer.previous_grapheme_boundary(1),
    ] {
        assert_eq!(result, Err(TextError::NotUtf8Boundary { offset: 1 }));
    }
    for result in [
        buffer.next_grapheme_boundary(3),
        buffer.previous_grapheme_boundary(3),
    ] {
        assert_eq!(
            result,
            Err(TextError::ByteOffsetOutOfBounds { offset: 3, len: 2 })
        );
    }
}

#[test]
fn grapheme_traversal_handles_actual_chunk_seams_and_long_context() {
    let text = format!(
        "{}a{}{}{}",
        "x".repeat(1024),
        "\u{301}".repeat(3_000),
        "🇺🇸".repeat(3_000),
        "क्‍ष".repeat(2_000),
    );
    let rope = Rope::from_str(&text);
    assert!(rope.chunks().count() > 1);
    let buffer =
        TextBuffer::try_from_rope(rope.clone(), legion_protocol::BufferVersion(0)).unwrap();
    let expected_boundaries = boundaries(&text);
    let mut seam_offsets = Vec::new();
    let mut start = 0;
    for chunk in rope.chunks() {
        if start != 0 {
            seam_offsets.push(start);
        }
        start += chunk.len();
    }
    assert!(seam_offsets.iter().any(|&offset| {
        let (previous, next) = oracle(&expected_boundaries, offset);
        !expected_boundaries.contains(&offset)
            && previous.is_some_and(|b| b < offset)
            && next.is_some_and(|b| b > offset)
    }));
    let combining_start = 1024;
    let combining_end = combining_start + 1 + 3_000 * "\u{301}".len();
    let ri_start = combining_end;
    let ri_middle = ri_start + 8 * 1_500;
    assert!(seam_offsets.iter().any(|&offset| {
        offset > combining_start && offset < combining_end && !expected_boundaries.contains(&offset)
    }));
    let combining_middle = combining_start + 1 + 1_500 * "\u{301}".len();
    let mut offsets = vec![0, text.len(), combining_middle, ri_middle];
    for seam in seam_offsets {
        offsets.push(seam);
        if let Some((relative, _)) = text[..seam].char_indices().next_back() {
            offsets.push(relative);
        }
        if let Some((relative, _)) = text[seam..].char_indices().nth(1) {
            offsets.push(seam + relative);
        }
    }
    offsets.sort_unstable();
    offsets.dedup();
    for offset in offsets {
        let expected = oracle(&expected_boundaries, offset);
        assert_eq!(
            buffer.previous_grapheme_boundary(offset).unwrap(),
            expected.0
        );
        assert_eq!(buffer.next_grapheme_boundary(offset).unwrap(), expected.1);
    }
}

#[test]
fn streamed_over_budget_buffer_keeps_full_cache_disabled_after_traversal() {
    let text = "a".repeat(DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES + 1);
    let buffer = TextBuffer::from_reader(Cursor::new(text.clone())).unwrap();
    assert!(buffer.try_full_text().is_err());
    let middle = text.len() / 2;
    assert_eq!(
        buffer.previous_grapheme_boundary(middle).unwrap(),
        Some(middle - 1)
    );
    assert_eq!(
        buffer.next_grapheme_boundary(middle).unwrap(),
        Some(middle + 1)
    );
    assert!(buffer.try_full_text().is_err());
}
