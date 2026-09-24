//! Call-hierarchy rows for the language section of the diagnostics panel.
//!
//! Call-hierarchy results are `LanguageLocationProjection` rows, the same type
//! the reference and definition lists use, so they render in the same surface
//! rather than getting one of their own. What that row type cannot carry is
//! *which question produced it*: "who calls this" and "what does this call" are
//! opposite answers that look identical as a list of file positions. A reader
//! who cannot tell them apart has been misled, so the direction is stated in a
//! heading and repeated as the noun on every row — the heading can be scrolled
//! or trimmed away, a row cannot lose its own first word.
//!
//! Extracted from `view.rs` rather than added to it: `view.rs` is a chokepoint
//! file the `extract-before-modify` gate keeps from growing, so the logic lives
//! here and `language_rows` only calls in.

use legion_protocol::{
    CallHierarchyDirection, LanguageLocationProjection, LanguageToolingProjection,
};

/// Rows shown before the list is summarised as "N more".
///
/// Matches the cap `language_rows` applies to definitions and references, so a
/// call-hierarchy answer cannot crowd the panel more than a reference list can.
const MAX_ROWS: usize = 12;

/// Marks a row whose location is the callee/caller's *declaration*, not a call.
///
/// Front-loaded because the panel trims long rows through the middle: a marker
/// appended at the end can be trimmed off, and a degraded row that has lost its
/// marker reads as a precise call site that does not exist.
const DEGRADED_MARKER: &str = "\u{26a0} declaration site";

/// Language-section rows describing the current call-hierarchy answer.
///
/// Empty when no call-hierarchy query has been answered for this buffer. A
/// query that came back with nothing still produces a heading: "nobody calls
/// this" is an answer, and silence would read as "not asked yet".
pub(super) fn call_hierarchy_rows(language: &LanguageToolingProjection) -> Vec<String> {
    let Some(heading) = call_hierarchy_heading(
        language.call_hierarchy_direction,
        language.call_hierarchy.len(),
        language.call_hierarchy_awaiting,
    ) else {
        return Vec::new();
    };
    let mut rows = vec![heading];
    let noun = row_noun(language.call_hierarchy_direction);
    rows.extend(
        language
            .call_hierarchy
            .iter()
            .take(MAX_ROWS)
            .map(|call| call_hierarchy_row(noun, call)),
    );
    if language.call_hierarchy.len() > MAX_ROWS {
        rows.push(format!(
            "call hierarchy: {} more {noun}s not shown",
            language.call_hierarchy.len() - MAX_ROWS
        ));
    }
    rows
}

/// The heading naming the question these rows answer, if one was asked.
///
/// `None` for a direction of `None` with no rows: that is the untouched initial
/// state, not an answered query. Rows *with* no direction are a projection bug
/// rather than a state to hide — labelling them "direction unknown" keeps the
/// reader from assuming whichever direction they asked for last.
fn call_hierarchy_heading(
    direction: Option<CallHierarchyDirection>,
    count: usize,
    awaiting: bool,
) -> Option<String> {
    // Asked and not yet answered. Distinct from an empty answer on purpose:
    // "nothing calls this symbol" is a conclusion, and stating it while the
    // question is still in flight would be wrong — permanently so if the answer
    // never arrives because the server lacks the capability or the caret was on
    // whitespace.
    if awaiting {
        return Some(match direction {
            Some(CallHierarchyDirection::Incoming) => {
                "call hierarchy: incoming \u{2014} asking\u{2026}".to_string()
            }
            Some(CallHierarchyDirection::Outgoing) => {
                "call hierarchy: outgoing \u{2014} asking\u{2026}".to_string()
            }
            None => "call hierarchy: asking\u{2026}".to_string(),
        });
    }
    match (direction, count) {
        (None, 0) => None,
        (None, count) => Some(format!(
            "call hierarchy: direction unknown \u{2014} {count} row(s)"
        )),
        (Some(CallHierarchyDirection::Incoming), 0) => {
            Some("call hierarchy: incoming \u{2014} nothing calls this symbol".to_string())
        }
        (Some(CallHierarchyDirection::Outgoing), 0) => {
            Some("call hierarchy: outgoing \u{2014} this symbol calls nothing".to_string())
        }
        (Some(CallHierarchyDirection::Incoming), count) => Some(format!(
            "call hierarchy: incoming \u{2014} {count} caller(s) of this symbol"
        )),
        (Some(CallHierarchyDirection::Outgoing), count) => Some(format!(
            "call hierarchy: outgoing \u{2014} {count} call(s) made by this symbol"
        )),
    }
}

/// The word each row leads with, so direction survives without the heading.
fn row_noun(direction: Option<CallHierarchyDirection>) -> &'static str {
    match direction {
        Some(CallHierarchyDirection::Incoming) => "caller",
        Some(CallHierarchyDirection::Outgoing) => "callee",
        None => "call",
    }
}

/// One row, in the shape `language_rows` already uses for reference locations.
///
/// The trailing `degraded={bool}` field matches the definition/reference rows;
/// the leading marker is what a person reads. Both are kept because dropping
/// the field would make call-hierarchy rows the one location list that does not
/// report degradation the way the others do.
fn call_hierarchy_row(noun: &str, call: &LanguageLocationProjection) -> String {
    let location = call
        .path
        .as_ref()
        .map(|path| {
            if let Some(range) = &call.range {
                format!("{}:{}", path.0, range.start.line)
            } else {
                path.0.clone()
            }
        })
        .unwrap_or_else(|| "<unknown-path>".to_string());
    let marker = if call.degraded {
        format!(" {DEGRADED_MARKER}")
    } else {
        String::new()
    };
    format!(
        "{noun}{marker} {} {location} {} degraded={}",
        call.location_id, call.label, call.degraded
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{CanonicalPath, ProtocolTextRange, TextCoordinate};

    fn coordinate(line: u32, character: u32) -> TextCoordinate {
        TextCoordinate {
            line,
            character,
            byte_offset: None,
            utf16_offset: None,
        }
    }

    fn location(id: &str, label: &str, line: u32, degraded: bool) -> LanguageLocationProjection {
        LanguageLocationProjection {
            location_id: id.to_string(),
            file_id: None,
            path: Some(CanonicalPath(format!("/workspace/src/{id}.rs"))),
            range: Some(ProtocolTextRange {
                start: coordinate(line, 0),
                end: coordinate(line, 4),
            }),
            label: label.to_string(),
            degraded,
            schema_version: 1,
        }
    }

    fn projection(
        direction: Option<CallHierarchyDirection>,
        calls: Vec<LanguageLocationProjection>,
    ) -> LanguageToolingProjection {
        LanguageToolingProjection {
            call_hierarchy: calls,
            call_hierarchy_direction: direction,
            ..LanguageToolingProjection::empty()
        }
    }

    #[test]
    fn untouched_projection_produces_no_rows() {
        assert!(call_hierarchy_rows(&projection(None, Vec::new())).is_empty());
    }

    #[test]
    fn incoming_and_outgoing_headings_are_distinguishable() {
        let calls = vec![location("a", "run", 3, false)];
        let incoming = call_hierarchy_rows(&projection(
            Some(CallHierarchyDirection::Incoming),
            calls.clone(),
        ));
        let outgoing =
            call_hierarchy_rows(&projection(Some(CallHierarchyDirection::Outgoing), calls));
        // The whole point of the heading: the two answers share a row type, so
        // if the rendered text matched, the reader could not tell them apart.
        assert_ne!(incoming, outgoing);
        assert!(incoming[0].contains("incoming"), "{}", incoming[0]);
        assert!(incoming[0].contains("caller"), "{}", incoming[0]);
        assert!(incoming[1].starts_with("caller "), "{}", incoming[1]);
        assert!(outgoing[0].contains("outgoing"), "{}", outgoing[0]);
        assert!(outgoing[1].starts_with("callee "), "{}", outgoing[1]);
    }

    #[test]
    fn a_question_in_flight_does_not_read_as_an_answer() {
        // Empty rows mean two opposite things: "the server said nobody calls
        // this" and "we have not heard back". Rendering the second as the
        // first states a conclusion the product does not have — and states it
        // permanently when the answer never comes, because the server lacks
        // `callHierarchyProvider` or the caret was on whitespace.
        let awaiting = LanguageToolingProjection {
            call_hierarchy_awaiting: true,
            ..projection(Some(CallHierarchyDirection::Incoming), Vec::new())
        };
        let rows = call_hierarchy_rows(&awaiting);
        assert_eq!(rows.len(), 1);
        assert!(
            !rows[0].contains("nothing calls this"),
            "an unanswered question must not be reported as an empty answer: {}",
            rows[0]
        );
        assert!(
            rows[0].contains("incoming"),
            "the direction asked for is still worth saying: {}",
            rows[0]
        );
    }

    #[test]
    fn empty_answer_still_states_the_direction() {
        let rows = call_hierarchy_rows(&projection(
            Some(CallHierarchyDirection::Incoming),
            Vec::new(),
        ));
        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains("nothing calls this"), "{}", rows[0]);
    }

    #[test]
    fn degraded_row_is_marked_before_any_trimming_point() {
        let rows = call_hierarchy_rows(&projection(
            Some(CallHierarchyDirection::Incoming),
            vec![location("a", "run", 3, true)],
        ));
        let row = &rows[1];
        assert!(row.contains(DEGRADED_MARKER), "{row}");
        assert!(row.ends_with("degraded=true"), "{row}");
        // The panel trims rows through the middle, so a marker that is not in
        // the first few words is a marker that can disappear.
        let marker_at = row.find(DEGRADED_MARKER).expect("marker present");
        assert!(marker_at < 16, "marker at {marker_at} in {row}");
    }

    #[test]
    fn precise_row_is_not_marked_as_a_declaration() {
        let rows = call_hierarchy_rows(&projection(
            Some(CallHierarchyDirection::Outgoing),
            vec![location("a", "run", 3, false)],
        ));
        assert!(!rows[1].contains(DEGRADED_MARKER), "{}", rows[1]);
        assert!(rows[1].ends_with("degraded=false"), "{}", rows[1]);
        assert!(rows[1].contains("/workspace/src/a.rs:3"), "{}", rows[1]);
    }

    #[test]
    fn overflow_is_reported_rather_than_dropped() {
        let calls = (0..MAX_ROWS + 3)
            .map(|i| location(&format!("c{i}"), "run", i as u32, false))
            .collect();
        let rows = call_hierarchy_rows(&projection(Some(CallHierarchyDirection::Incoming), calls));
        assert_eq!(rows.len(), MAX_ROWS + 2, "heading + cap + overflow notice");
        assert!(
            rows.last().expect("rows").contains("3 more caller"),
            "{:?}",
            rows.last()
        );
    }

    #[test]
    fn rows_without_a_direction_admit_they_lack_one() {
        let rows = call_hierarchy_rows(&projection(None, vec![location("a", "run", 3, false)]));
        assert!(rows[0].contains("direction unknown"), "{}", rows[0]);
        // Not "caller" and not "callee": claiming either would be a guess.
        assert!(rows[1].starts_with("call "), "{}", rows[1]);
    }
}
