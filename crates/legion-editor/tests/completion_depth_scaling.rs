//! Does an LSP completion request cost more the further down the file it is made?
//!
//! `EditorEngine::byte_offset_from_absolute_utf16` resolves a UTF-16 offset to a byte
//! offset by walking lines from the start of the buffer, subtracting each line's UTF-16
//! length and line-ending length until the offset falls inside a line. That is the same
//! O(document length) shape that `absolute_utf16_offset` had on the viewport path, where
//! it turned out to be the whole of a measurement — so the question is whether it costs
//! anything that matters here.
//!
//! It sits on the completion path, and `EditorEngine::completion` resolves the offset
//! *before* it decides whether it can serve completions at all. A large buffer fails
//! closed to an empty item list, so on exactly the files where this loop is longest, the
//! work it does is thrown away.
//!
//! The workload is therefore a real completion request rather than a synthetic call: the
//! sweep varies only how deep the requested position is, so any slope is depth-dependent
//! work inside the request.
//!
//! LSP positions are UTF-16 natively, so `TextOffset::utf16` is the encoding a real
//! language-server client sends. The byte-offset encoding short-circuits this path
//! entirely and is not measured here.
//!
//! Reporting rather than asserting, and `#[ignore]`d, matching the other perf
//! diagnostics in this workspace: wall-clock thresholds in the workspace suite flake on
//! shared runners, and `perf-harness` is where budgets are enforced.

use std::time::Instant;

use legion_editor::EditorEngine;
use legion_protocol::{CompletionRequest, CorrelationId, FileId, TextOffset, WorkspaceId};

/// Lines in the fixture, matching `viewport_depth_scaling` so the two sweeps are
/// directly comparable.
const FIXTURE_LINES: usize = 1_000_000;

/// The harness fixture's line. All ASCII, so a UTF-16 offset and a byte offset coincide
/// and the depth arithmetic below is exact.
const LINE: &str = "the quick brown fox jumps over the lazy dog 0123456789\n";

/// Requests issued at each depth.
const REPEATS: usize = 5;

/// Build a buffer of [`FIXTURE_LINES`] identical lines.
fn fixture() -> String {
    let mut text = String::with_capacity(FIXTURE_LINES * LINE.len());
    for _ in 0..FIXTURE_LINES {
        text.push_str(LINE);
    }
    text
}

/// Report completion-request cost against how deep the requested position is.
///
/// Run with
/// `cargo test -p legion-editor --release --test completion_depth_scaling -- --ignored --nocapture`.
#[test]
#[ignore = "perf diagnostic: reports timings, does not assert on them"]
fn completion_request_cost_against_position_depth() {
    let mut engine = EditorEngine::new();
    let buffer = engine
        .open_buffer(WorkspaceId(1), FileId(2), "large.rs", fixture())
        .expect("open buffer");
    let metadata = engine.buffer_metadata(buffer).expect("buffer metadata");

    println!("lines={FIXTURE_LINES} repeats={REPEATS}");
    for depth in [0usize, 62_500, 125_000, 250_000, 500_000, 999_000] {
        // All-ASCII fixture, so the UTF-16 offset of the start of line `depth` is just
        // the byte offset.
        let utf16_offset = (depth * LINE.len()) as u64;
        let mut best = u128::MAX;
        let mut total = 0u128;
        for _ in 0..REPEATS {
            let start = Instant::now();
            let response = engine
                .completion(CompletionRequest {
                    workspace_id: WorkspaceId(1),
                    file_id: FileId(2),
                    snapshot_id: metadata.snapshot_id,
                    position: TextOffset::utf16(utf16_offset),
                    correlation_id: CorrelationId(42),
                })
                .expect("completion request");
            let elapsed = start.elapsed().as_micros();
            // Read the response so the request cannot be optimized away.
            std::hint::black_box(response.items.len());
            best = best.min(elapsed);
            total += elapsed;
        }
        let mean = total / REPEATS as u128;
        println!("depth_line={depth:>7}  best={best:>7}us  mean={mean:>7}us");
    }
}
