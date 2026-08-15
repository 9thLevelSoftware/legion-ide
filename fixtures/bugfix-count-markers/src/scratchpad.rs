// Legion-Bench live-local fixture: scratchpad module with a planted bug.
//
// BUG (intentional, the bench task asks the agent to fix it): the count is
// decremented once before returning, so callers see one less occurrence than
// the input contains — and an underflow guard hides the zero case.

/// Count occurrences of the literal `SMOKE_MARKER_ALPHA` in `input`.
pub fn count_markers(input: &str) -> usize {
    let count = input.matches("SMOKE_MARKER_ALPHA").count();
    count.saturating_sub(1)
}
