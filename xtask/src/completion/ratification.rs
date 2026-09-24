//! Fail-closed detection of provisional ratification markers on matrix cells.
//!
//! This is a marker check over the *text* of a configuration's
//! `owner_approval_ref`, and it is **not** authentication of owner
//! ratification. No string in a JSON file can prove that a human ratified
//! anything, so a reference that carries no marker here establishes nothing
//! positive: it only means this specific self-declared "provisional,
//! agent-made" marker is absent. The check exists to remove one way for a
//! knowingly provisional register to pass release mode unnoticed. A clean
//! release run must never be read as "the owner ratified the matrix".
//!
//! The module is pure: no IO, no register access, and no dependency on the
//! rest of `completion`.

/// Marker words the completion register uses when it records that a
/// ratification is provisional and agent-made rather than owner-ratified.
///
/// Entries must be lowercase ASCII: matching lowercases the reference with
/// [`str::to_ascii_lowercase`], which preserves byte offsets exactly.
const PROVISIONAL_RATIFICATION_MARKERS: [&str; 2] = ["provisional", "agent-made"];

/// Returns the provisional-ratification marker text found in
/// `owner_approval_ref`, or `None` when the reference carries no such marker.
///
/// Matching is case-insensitive over ASCII and matches anywhere in the
/// reference. The returned slice borrows `owner_approval_ref`, so it preserves
/// the register's own casing and lets the caller quote exactly what it matched
/// instead of asserting a verdict of its own. When several markers are present
/// the leftmost is returned, with ties broken by the order of
/// [`PROVISIONAL_RATIFICATION_MARKERS`], so the result is deterministic for a
/// given input.
///
/// A blank or whitespace-only reference is deliberately **not** this module's
/// concern and returns `None`: structure validation already rejects a blank
/// `owner_approval_ref` (see `super::structure`), and reporting it here as well
/// would double-report the same defect. Do not "fix" that apparent gap.
pub fn provisional_ratification_marker(owner_approval_ref: &str) -> Option<&str> {
    let lowercased = owner_approval_ref.to_ascii_lowercase();
    let (at, len) = PROVISIONAL_RATIFICATION_MARKERS
        .iter()
        .enumerate()
        .filter_map(|(rank, marker)| lowercased.find(*marker).map(|at| (at, rank, marker.len())))
        .min()
        .map(|(at, _, len)| (at, len))?;
    owner_approval_ref.get(at..at + len)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact reference every configuration in `plans/completion/matrix.json`
    /// carries today. Written out literally so that a reword of the register's
    /// marker is caught by a failing test rather than by silence.
    const REGISTER_REFERENCE_TODAY: &str = "plans/completion/decisions.md#2026-09-08-provisional-register-ratification (PROVISIONAL: agent-made, not owner-ratified)";

    #[test]
    fn a_reference_without_a_marker_is_not_flagged() {
        assert_eq!(provisional_ratification_marker("approval"), None);
        assert_eq!(
            provisional_ratification_marker(
                "plans/completion/decisions.md#2026-09-08-owner-ratification"
            ),
            None
        );
    }

    #[test]
    fn the_reference_the_register_uses_today_is_flagged() {
        assert_eq!(
            provisional_ratification_marker(REGISTER_REFERENCE_TODAY),
            Some("provisional")
        );
    }

    #[test]
    fn both_marker_words_are_detected_case_insensitively() {
        assert_eq!(
            provisional_ratification_marker("ratified (PROVISIONAL)"),
            Some("PROVISIONAL")
        );
        assert_eq!(
            provisional_ratification_marker("ratified (Provisional)"),
            Some("Provisional")
        );
        assert_eq!(
            provisional_ratification_marker("ratified, AGENT-Made"),
            Some("AGENT-Made")
        );
    }

    #[test]
    fn the_leftmost_marker_is_returned() {
        assert_eq!(
            provisional_ratification_marker("agent-made and provisional"),
            Some("agent-made")
        );
        assert_eq!(
            provisional_ratification_marker("provisional and agent-made"),
            Some("provisional")
        );
    }

    #[test]
    fn a_blank_reference_is_left_to_structure_validation() {
        assert_eq!(provisional_ratification_marker(""), None);
        assert_eq!(provisional_ratification_marker("   "), None);
    }

    #[test]
    fn a_multibyte_reference_is_sliced_on_character_boundaries() {
        assert_eq!(
            provisional_ratification_marker("ratificación — PROVISIONAL"),
            Some("PROVISIONAL")
        );
    }
}
