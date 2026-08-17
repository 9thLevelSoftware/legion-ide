//! Where a diagnostic underline goes on one line of the editor.
//!
//! Extracted from `view.rs` so the decision can be tested without a frame — and
//! because `view.rs` is a chokepoint file that `xtask extract-before-modify`
//! watches. The painter still owns the painting; only the arithmetic moved.

use legion_protocol::ProtocolTextRange;

/// The character span of `range` that falls on the line at `line_zero`.
///
/// `None` when the range does not touch this line, or touches it emptily —
/// a zero-width underline is invisible and painting one costs a segment per
/// frame for nothing.
///
/// A range spanning several lines is clipped to this one: it starts at column
/// zero unless it began here, and runs to the line's end unless it ends here.
/// That clipping is the whole difficulty and the reason this is a function
/// rather than four lines inside a paint loop — it is off-by-one territory,
/// and a painter cannot be tested without a frame.
pub(crate) fn diagnostic_underline_span(
    line_zero: u32,
    line_chars: u32,
    range: &ProtocolTextRange,
) -> Option<(u32, u32)> {
    if range.start.line > line_zero || range.end.line < line_zero {
        return None;
    }
    let start_char = if range.start.line == line_zero {
        range.start.character
    } else {
        0
    };
    let end_char = if range.end.line == line_zero {
        range.end.character
    } else {
        line_chars
    };
    (start_char < end_char).then_some((start_char, end_char))
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::TextCoordinate;

    fn range_at(start: (u32, u32), end: (u32, u32)) -> ProtocolTextRange {
        ProtocolTextRange {
            start: TextCoordinate {
                line: start.0,
                character: start.1,
                byte_offset: None,
                utf16_offset: None,
            },
            end: TextCoordinate {
                line: end.0,
                character: end.1,
                byte_offset: None,
                utf16_offset: None,
            },
        }
    }

    #[test]
    fn a_single_line_diagnostic_underlines_its_own_columns() {
        let span = diagnostic_underline_span(3, 40, &range_at((3, 4), (3, 9)));
        assert_eq!(span, Some((4, 9)));
    }

    #[test]
    fn a_diagnostic_on_another_line_underlines_nothing() {
        assert_eq!(
            diagnostic_underline_span(3, 40, &range_at((5, 0), (5, 9))),
            None
        );
        assert_eq!(
            diagnostic_underline_span(9, 40, &range_at((3, 0), (4, 9))),
            None
        );
    }

    /// The clipping that makes this worth extracting.
    #[test]
    fn a_multi_line_diagnostic_is_clipped_to_each_line_it_crosses() {
        let range = range_at((2, 6), (4, 3));
        assert_eq!(
            diagnostic_underline_span(2, 40, &range),
            Some((6, 40)),
            "the first line runs from where it started to the line's end"
        );
        assert_eq!(
            diagnostic_underline_span(3, 40, &range),
            Some((0, 40)),
            "a line fully inside the range is underlined end to end"
        );
        assert_eq!(
            diagnostic_underline_span(4, 40, &range),
            Some((0, 3)),
            "the last line stops where the range does"
        );
    }

    #[test]
    fn an_empty_span_is_not_painted() {
        assert_eq!(
            diagnostic_underline_span(3, 40, &range_at((3, 7), (3, 7))),
            None,
            "a zero-width underline is invisible and costs a segment per frame"
        );
        assert_eq!(
            diagnostic_underline_span(3, 40, &range_at((3, 9), (3, 4))),
            None,
            "an inverted range is not a span"
        );
    }

    #[test]
    fn a_range_ending_at_the_start_of_a_later_line_does_not_underline_it() {
        // `end.character == 0` on this line means the range stopped before it.
        assert_eq!(
            diagnostic_underline_span(4, 40, &range_at((2, 6), (4, 0))),
            None
        );
    }
}
