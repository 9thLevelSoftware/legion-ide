// GP-1 smoke fixture: main entry point.
//
// The literal SMOKE_MARKER_ALPHA below is the target for the search step (s4).
// `mod scratchpad` hosts the file that the smoke edits at runtime to introduce
// and fix a compile error (s3). Both modules are valid at rest so `cargo test`
// passes without any runtime intervention.

mod scratchpad;

fn main() {
    println!("SMOKE_MARKER_ALPHA");
}

#[cfg(test)]
mod tests {
    #[test]
    fn fixture_passes_at_rest() {
        assert_eq!(2 + 2, 4);
    }
}
