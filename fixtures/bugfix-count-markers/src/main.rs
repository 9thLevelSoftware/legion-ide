// Legion-Bench live-local fixture: off-by-one bug in `count_markers`.
//
// At rest, `cargo test` FAILS: `scratchpad::count_markers` returns one less
// than the true number of SMOKE_MARKER_ALPHA occurrences. The bench task asks
// the agent to fix the implementation (not the tests) so `cargo test` passes.

mod scratchpad;

fn main() {
    let sample = "SMOKE_MARKER_ALPHA and SMOKE_MARKER_ALPHA";
    println!("markers: {}", scratchpad::count_markers(sample));
}

#[cfg(test)]
mod tests {
    use crate::scratchpad::count_markers;

    #[test]
    fn counts_zero_markers() {
        assert_eq!(count_markers("no markers here"), 0);
    }

    #[test]
    fn counts_single_marker() {
        assert_eq!(count_markers("one SMOKE_MARKER_ALPHA only"), 1);
    }

    #[test]
    fn counts_three_markers() {
        assert_eq!(
            count_markers("SMOKE_MARKER_ALPHA SMOKE_MARKER_ALPHA SMOKE_MARKER_ALPHA"),
            3
        );
    }
}
