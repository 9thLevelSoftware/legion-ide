//! Pins the observable behaviour of [`LineIndex`] so a performance change to the
//! incremental edit path cannot move it.
//!
//! Every crate in the workspace resolves diagnostics, breakpoints, LSP positions and
//! proposal ranges through this arithmetic. A drift in line numbering or UTF-16 offsets
//! would misplace all of them, and almost none of it would fail an existing test. These
//! tests therefore assert the *current* mapping directly, and — more importantly — assert
//! that an index maintained incrementally across edits stays indistinguishable from one
//! scanned fresh over the same final text.
//!
//! If a change makes one of these fail, the change is wrong, not the test.

use legion_text::{LineIndex, TextBuffer, TextEdit, TextPosition, TextRange, Utf16Position};

/// Compare every observable output of two indices.
///
/// Debug formatting is used rather than `==` so that a mismatch reports the differing
/// value, and so the comparison does not depend on which traits the result types derive.
fn assert_indices_agree(actual: &LineIndex, expected: &LineIndex, text: &str, context: &str) {
    assert_eq!(
        actual.line_count(),
        expected.line_count(),
        "{context}: line_count"
    );
    assert_eq!(actual.len(), expected.len(), "{context}: len");
    assert_eq!(
        actual.is_empty(),
        expected.is_empty(),
        "{context}: is_empty"
    );

    for line in 0..expected.line_count() {
        assert_eq!(
            format!("{:?}", actual.line_byte_len(line)),
            format!("{:?}", expected.line_byte_len(line)),
            "{context}: line_byte_len({line})"
        );
        assert_eq!(
            format!("{:?}", actual.line_utf16_len(line)),
            format!("{:?}", expected.line_utf16_len(line)),
            "{context}: line_utf16_len({line})"
        );
        assert_eq!(
            format!("{:?}", actual.line_ending_bytes(line)),
            format!("{:?}", expected.line_ending_bytes(line)),
            "{context}: line_ending_bytes({line})"
        );
        assert_eq!(
            format!("{:?}", actual.line_slice(line, usize::MAX)),
            format!("{:?}", expected.line_slice(line, usize::MAX)),
            "{context}: line_slice({line})"
        );
        // A deliberately tiny budget exercises the truncation path, which reads
        // start_byte/content_end_byte/byte_len together.
        assert_eq!(
            format!("{:?}", actual.line_slice(line, 3)),
            format!("{:?}", expected.line_slice(line, 3)),
            "{context}: line_slice({line}, 3)"
        );

        let byte_len = expected.line_byte_len(line).expect("line in range");
        for column in 0..=byte_len {
            assert_eq!(
                format!("{:?}", actual.byte_offset(TextPosition::new(line, column))),
                format!(
                    "{:?}",
                    expected.byte_offset(TextPosition::new(line, column))
                ),
                "{context}: byte_offset({line}, {column})"
            );
        }

        let utf16_len = expected.line_utf16_len(line).expect("line in range");
        for character in 0..=utf16_len {
            let pos = Utf16Position::new(line, character);
            assert_eq!(
                format!("{:?}", actual.byte_offset_from_utf16(pos)),
                format!("{:?}", expected.byte_offset_from_utf16(pos)),
                "{context}: byte_offset_from_utf16({line}, {character})"
            );
        }
    }

    for offset in 0..=text.len() {
        assert_eq!(
            format!("{:?}", actual.position(offset)),
            format!("{:?}", expected.position(offset)),
            "{context}: position({offset})"
        );
        assert_eq!(
            format!("{:?}", actual.utf16_position(offset)),
            format!("{:?}", expected.utf16_position(offset)),
            "{context}: utf16_position({offset})"
        );
    }
}

/// Line/column and UTF-16 mappings for a fixture, as a stable snapshot string.
fn mapping_digest(index: &LineIndex, text: &str) -> String {
    let mut out = String::new();
    for line in 0..index.line_count() {
        out.push_str(&format!(
            "L{line} bytes={:?} utf16={:?} ending={:?}\n",
            index.line_byte_len(line),
            index.line_utf16_len(line),
            index.line_ending_bytes(line)
        ));
    }
    for offset in 0..=text.len() {
        if text.is_char_boundary(offset) {
            out.push_str(&format!(
                "@{offset} -> {:?} / {:?}\n",
                index.position(offset),
                index.utf16_position(offset)
            ));
        }
    }
    out
}

#[test]
fn empty_buffer_has_a_single_empty_line() {
    let index = LineIndex::new("");
    assert_eq!(index.line_count(), 1);
    assert_eq!(index.len(), 0);
    assert!(index.is_empty());
    assert_eq!(index.line_byte_len(0).expect("line 0"), 0);
    assert_eq!(index.line_utf16_len(0).expect("line 0"), 0);
    assert_eq!(index.line_ending_bytes(0).expect("line 0"), 0);
    assert_eq!(
        index.position(0).expect("offset 0"),
        TextPosition::new(0, 0)
    );
}

#[test]
fn trailing_newline_produces_a_final_empty_line() {
    let with_trailing = LineIndex::new("a\nb\n");
    assert_eq!(with_trailing.line_count(), 3);
    assert_eq!(with_trailing.line_byte_len(2).expect("last"), 0);
    assert_eq!(with_trailing.line_ending_bytes(2).expect("last"), 0);
    assert_eq!(with_trailing.line_ending_bytes(0).expect("first"), 1);

    let without_trailing = LineIndex::new("a\nb");
    assert_eq!(without_trailing.line_count(), 2);
    assert_eq!(without_trailing.line_byte_len(1).expect("last"), 1);
    assert_eq!(without_trailing.line_ending_bytes(1).expect("last"), 0);
}

#[test]
fn crlf_is_a_single_two_byte_line_ending() {
    let index = LineIndex::new("alpha\r\nbeta\r\n");
    assert_eq!(index.line_count(), 3);
    assert_eq!(index.line_ending_bytes(0).expect("line 0"), 2);
    assert_eq!(index.line_ending_bytes(1).expect("line 1"), 2);
    // The carriage return is excluded from the column length.
    assert_eq!(index.line_byte_len(0).expect("line 0"), 5);
    assert_eq!(index.line_utf16_len(0).expect("line 0"), 5);
    assert_eq!(
        index.position(5).expect("end of line 0"),
        TextPosition::new(0, 5)
    );
}

#[test]
fn a_lone_carriage_return_ends_a_line() {
    // Classic-Mac line endings: Legion breaks on a bare `\r` with one ending byte.
    let index = LineIndex::new("alpha\rbeta\r");
    assert_eq!(index.line_count(), 3);
    assert_eq!(index.line_ending_bytes(0).expect("line 0"), 1);
    assert_eq!(index.line_byte_len(0).expect("line 0"), 5);
    assert_eq!(index.line_byte_len(1).expect("line 1"), 4);
}

#[test]
fn unicode_line_like_characters_do_not_break_lines() {
    // ropey's default `unicode_lines` feature breaks on every character below. Legion
    // does not. If an index implementation ever adopts ropey's line APIs without
    // restricting them to `cr_lines`, this fixture renumbers and every position in the
    // product moves with it.
    let text = "a\u{000B}b\u{000C}c\u{0085}d\u{2028}e\u{2029}f\n";
    let index = LineIndex::new(text);
    assert_eq!(
        index.line_count(),
        2,
        "only the trailing \\n may break this fixture"
    );
    assert_eq!(index.line_ending_bytes(0).expect("line 0"), 1);

    // Each of these has a UTF-8 length above one, so byte and UTF-16 columns differ.
    let expected_utf16 = text.trim_end_matches('\n').encode_utf16().count();
    assert_eq!(index.line_utf16_len(0).expect("line 0"), expected_utf16);
}

#[test]
fn multibyte_and_astral_plane_characters_map_correctly() {
    // "e-acute" is 2 bytes / 1 UTF-16 unit; the CJK ideograph is 3 bytes / 1 unit;
    // the emoji is 4 bytes / 2 UTF-16 units (a surrogate pair).
    let text = "é漢🙂\nx\n";
    let index = LineIndex::new(text);
    assert_eq!(index.line_count(), 3);
    assert_eq!(index.line_byte_len(0).expect("line 0"), 2 + 3 + 4);
    assert_eq!(index.line_utf16_len(0).expect("line 0"), 1 + 1 + 2);

    // The emoji starts at byte 5 and at UTF-16 unit 2.
    assert_eq!(
        index.utf16_position(5).expect("emoji start"),
        Utf16Position::new(0, 2)
    );
    assert_eq!(
        index
            .byte_offset_from_utf16(Utf16Position::new(0, 2))
            .expect("emoji start"),
        5
    );

    // Addressing the middle of a surrogate pair is rejected rather than rounded.
    assert!(
        index
            .byte_offset_from_utf16(Utf16Position::new(0, 3))
            .is_err(),
        "an offset inside a surrogate pair must not resolve"
    );
}

#[test]
fn byte_and_utf16_positions_round_trip_across_fixtures() {
    for text in [
        "",
        "\n",
        "a",
        "a\nb\n",
        "a\nb",
        "alpha\r\nbeta\r\n",
        "alpha\rbeta\r",
        "é漢🙂\nx\n",
        "mixed \r\n endings \r and \n here",
        "a\u{000B}b\u{000C}c\u{0085}d\u{2028}e\u{2029}f\n",
    ] {
        let index = LineIndex::new(text);
        for offset in 0..=text.len() {
            if !text.is_char_boundary(offset) {
                continue;
            }
            let position = index.position(offset).expect("valid offset");
            let back = index.byte_offset(position).expect("valid position");
            // Offsets inside a line ending clamp onto the line's content end, so the
            // round trip is idempotent rather than identity there.
            let reposition = index.position(back).expect("valid offset");
            assert_eq!(
                position, reposition,
                "position round trip is not idempotent at {offset} in {text:?}"
            );
        }
    }
}

/// Apply `edits` to a buffer and require the incrementally maintained index to stay
/// indistinguishable from a fresh scan of the resulting text after every single edit.
fn assert_incremental_matches_fresh(initial: &str, edits: &[(usize, usize, &str)]) {
    let mut buffer = TextBuffer::new(initial);
    for (step, (start, end, replacement)) in edits.iter().enumerate() {
        buffer
            .try_replace_range(*start, *end, replacement)
            .unwrap_or_else(|error| {
                panic!("edit {step} ({start}..{end} -> {replacement:?}): {error:?}")
            });

        let text = buffer.text().to_string();
        let fresh = LineIndex::new(&text);
        assert_indices_agree(
            buffer.line_index(),
            &fresh,
            &text,
            &format!("after edit {step} on {initial:?}"),
        );
    }
}

#[test]
fn incremental_insertions_match_a_fresh_scan() {
    assert_incremental_matches_fresh(
        "alpha\nbeta\ngamma\n",
        &[
            (0, 0, "x"),
            (3, 3, "y"),
            (7, 7, "z"),
            (18, 18, "w"),
            (1, 2, ""),
            (0, 1, ""),
        ],
    );
}

#[test]
fn incremental_deletions_match_a_fresh_scan() {
    assert_incremental_matches_fresh(
        "alpha\nbeta\ngamma\ndelta\n",
        &[(0, 2, ""), (4, 6, ""), (1, 1, "QQ"), (8, 9, "")],
    );
}

#[test]
fn incremental_edits_on_crlf_text_match_a_fresh_scan() {
    assert_incremental_matches_fresh(
        "alpha\r\nbeta\r\ngamma\r\n",
        &[(0, 0, "x"), (2, 3, ""), (9, 9, "yy"), (1, 1, "")],
    );
}

#[test]
fn incremental_edits_around_astral_characters_match_a_fresh_scan() {
    // Editing beside a surrogate pair is where a UTF-16 delta error would surface.
    assert_incremental_matches_fresh(
        "é漢🙂 tail\nsecond 🙂 line\nthird\n",
        // Offsets are char boundaries at the point each edit is applied; the buffer
        // rejects anything else, so these track the shifting layout deliberately.
        &[
            (0, 0, "x"),
            (10, 10, "🙂"),
            (0, 1, ""),
            (5, 5, "é"),
            (7, 7, "plain"),
        ],
    );
}

#[test]
fn many_sequential_edits_match_a_fresh_scan() {
    // A long run of same-line keystrokes is the case an overlay/deferred-shift design
    // has to compact through. Correctness must not depend on where the threshold lands.
    let mut buffer = TextBuffer::new("alpha\nbeta\ngamma\ndelta\nepsilon\n");
    for step in 0..400usize {
        let offset = 2 + (step % 3);
        buffer
            .try_replace_range(offset, offset, "k")
            .expect("insert");
        if step % 7 == 0 {
            buffer
                .try_replace_range(offset, offset + 1, "")
                .expect("delete");
        }

        if step % 25 == 0 || step == 399 {
            let text = buffer.text().to_string();
            let fresh = LineIndex::new(&text);
            assert_indices_agree(
                buffer.line_index(),
                &fresh,
                &text,
                &format!("after {step} sequential edits"),
            );
        }
    }
}

#[test]
fn edits_across_many_lines_match_a_fresh_scan() {
    // Enough lines that the edited line is far from both ends, so a shift applied to the
    // wrong side of the edit point would show up.
    let mut text = String::new();
    for line in 0..200 {
        text.push_str(&format!("line {line:04} contents\n"));
    }
    let mut buffer = TextBuffer::new(text);

    for line in [0usize, 1, 99, 100, 198, 199] {
        let offset = buffer
            .try_byte_offset(TextPosition::new(line, 2))
            .expect("position in range");
        buffer
            .try_replace_range(offset, offset, "Z")
            .expect("insert");

        let current = buffer.text().to_string();
        let fresh = LineIndex::new(&current);
        assert_indices_agree(
            buffer.line_index(),
            &fresh,
            &current,
            &format!("after editing line {line}"),
        );
    }
}

#[test]
fn edit_applied_via_text_edit_api_matches_a_fresh_scan() {
    let mut buffer = TextBuffer::new("alpha\nbeta\ngamma\n");
    let at = TextPosition::new(1, 2);
    buffer
        .try_apply_edit(&TextEdit {
            range: TextRange::new(at, at),
            new_text: "QQ".to_string(),
        })
        .expect("apply edit");

    let text = buffer.text().to_string();
    let fresh = LineIndex::new(&text);
    assert_indices_agree(buffer.line_index(), &fresh, &text, "after TextEdit");
}

/// Fixtures whose full line/column mapping is pinned byte-for-byte by the golden file.
const GOLDEN_FIXTURES: &[&str] = &[
    "",
    "\n",
    "a",
    "a\nb\n",
    "a\nb",
    "\n\n\n",
    "alpha\r\nbeta\r\n",
    "alpha\rbeta\r",
    "mixed \r\n endings \r and \n here",
    "é漢🙂\nx\n",
    "a\u{000B}b\u{000C}c\u{0085}d\u{2028}e\u{2029}f\n",
];

#[test]
fn fixture_mappings_match_golden() {
    // The tests above compare an incrementally maintained index against a freshly scanned
    // one. Both run the same lookup code, so a change to the *lookup itself* — the
    // binary search over line starts, say — would move both sides together and pass. This
    // golden pins the absolute mapping recorded from the pre-change implementation, which
    // is the only check that catches that class of drift.
    let mut actual = String::new();
    for text in GOLDEN_FIXTURES {
        actual.push_str(&format!("=== {text:?}\n"));
        actual.push_str(&mapping_digest(&LineIndex::new(text), text));
    }

    let golden = include_str!("golden/line_index_mappings.txt");
    assert_eq!(
        actual, golden,
        "line index mapping drifted from the recorded golden"
    );
}
