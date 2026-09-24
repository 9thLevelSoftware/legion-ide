//! Does a viewport projection cost more the further down the file it is taken?
//!
//! `m9.large_file_100mb` measures one projection at line 500,000 and reports a few
//! milliseconds for it. That number is only meaningful if you know what it is made of:
//! the twenty-four lines actually being projected, or something proportional to how far
//! down the file the request sits. Those two have completely different consequences —
//! the first is a fixed frame cost, the second means scrolling gets slower the deeper a
//! user goes, and a 100MB file is deep.
//!
//! So this sweeps `top_line` across a buffer of fixed size and reports the projection
//! cost at each depth. The visible line count is identical at every point, so any slope
//! is depth-dependent work rather than projection work.
//!
//! Reporting rather than asserting, and `#[ignore]`d, for the reason the rest of the
//! perf diagnostics in this workspace are: wall-clock thresholds in the workspace suite
//! flake on shared runners. `perf-harness` is where budgets are enforced.

use std::time::Instant;

use legion_editor::EditorEngine;
use legion_protocol::{
    EditorViewportRequest, FileId, ViewportDimensions, ViewportScroll, WorkspaceId,
};

/// Lines in the fixture. Large enough that a linear term over `top_line` separates
/// clearly from constant per-projection work, small enough to build in a test.
const FIXTURE_LINES: usize = 1_000_000;

/// The harness fixture's line, so the shape of the text matches what `m9` measures.
const LINE: &str = "the quick brown fox jumps over the lazy dog 0123456789\n";

/// Projections taken at each depth, to keep one unlucky sample from setting the number.
const REPEATS: usize = 5;

/// Build a buffer of [`FIXTURE_LINES`] identical lines.
fn fixture() -> String {
    let mut text = String::with_capacity(FIXTURE_LINES * LINE.len());
    for _ in 0..FIXTURE_LINES {
        text.push_str(LINE);
    }
    text
}

/// Report projection cost against scroll depth.
///
/// Run with
/// `cargo test -p legion-editor --release --test viewport_depth_scaling -- --ignored --nocapture`.
#[test]
#[ignore = "perf diagnostic: reports timings, does not assert on them"]
fn viewport_projection_cost_against_scroll_depth() {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(1), "large.txt", fixture())
        .expect("open buffer");

    // Every projection below asks for the same twenty-four lines' worth of height, so
    // the only thing varying across the sweep is how far down the request sits.
    let dimensions = ViewportDimensions {
        width_px: 1_200,
        height_px: 24 * 16,
    };

    println!("lines={FIXTURE_LINES} visible=24 repeats={REPEATS}");
    for depth in [0usize, 62_500, 125_000, 250_000, 500_000, 999_000] {
        let mut best = u128::MAX;
        let mut total = 0u128;
        for _ in 0..REPEATS {
            let start = Instant::now();
            let projection = engine
                .viewport_projection(EditorViewportRequest {
                    buffer_id: buffer,
                    scroll: ViewportScroll {
                        top_line: depth as u32,
                        left_column: 0,
                    },
                    dimensions,
                })
                .expect("viewport projection");
            let elapsed = start.elapsed().as_micros();
            // Read a field so the projection cannot be optimized away.
            assert!(!projection.line_slices.is_empty());
            best = best.min(elapsed);
            total += elapsed;
        }
        let mean = total / REPEATS as u128;
        println!("top_line={depth:>7}  best={best:>7}us  mean={mean:>7}us");
    }
}
