//! Real 100MB large-file measurement for the perf harness (P1.F4.T5).
//!
//! The harness cannot depend on `legion-editor`, so every other large-file
//! number it reports is a synthetic stand-in. This binary does the real thing
//! and writes a TOML report the harness reads back — the same subprocess model
//! `legion-desktop --manual-perf` and the bench runner already use.
//!
//! What it measures, and why each one:
//!
//! * **open** — how long a 100MB file takes to become editable. The streaming
//!   path exists so this does not scale with file size; if it regresses, large
//!   files stop being openable at all.
//! * **viewport** — one projection at a deep scroll offset. This is the scroll
//!   frame budget: it must not depend on how far down the file the user is.
//! * **edit p50/p95** — insert and delete at a deep line. This is the typing
//!   budget, and the one that decides whether a large file is *usable* rather
//!   than merely openable.
//!
//! The file is generated on disk and opened through the streaming path, so the
//! measurement covers what the product actually does rather than what is
//! convenient to construct in memory.

use std::{
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};

use legion_editor::{EditorEngine, TextEdit, TextPosition, TextRange};
use legion_protocol::{
    EditorViewportRequest, FileId, TransactionSource, ViewportDimensions, ViewportProjectionMode,
    ViewportScroll, WorkspaceId,
};

/// Bytes of fixture to generate. Named rather than inlined because the report
/// records it and the harness's budget only means anything against a size.
const TARGET_BYTES: usize = 100 * 1024 * 1024;

/// Edits sampled for the typing latency percentiles.
const EDIT_SAMPLES: usize = 32;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut report_path: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        if arg == "--report" {
            report_path = args.next().map(PathBuf::from);
        }
    }
    let Some(report_path) = report_path else {
        eprintln!("large_file_perf: --report <path> is required");
        std::process::exit(2);
    };

    match measure() {
        Ok(report) => {
            if let Some(parent) = report_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::File::create(&report_path)
                .and_then(|mut file| file.write_all(report.as_bytes()))
            {
                Ok(()) => println!("large_file_perf: wrote {}", report_path.display()),
                Err(err) => {
                    eprintln!("large_file_perf: cannot write report: {err}");
                    std::process::exit(1);
                }
            }
        }
        Err(err) => {
            eprintln!("large_file_perf: {err}");
            std::process::exit(1);
        }
    }
}

fn measure() -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("legion-large-file-perf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create scratch dir: {e}"))?;
    let path = dir.join("large.txt");

    write_fixture(&path)?;

    let mut engine = EditorEngine::new();

    let open_start = Instant::now();
    let buffer = engine
        .open_buffer_streaming(WorkspaceId(1), FileId(1), "large.txt", &path)
        .map_err(|e| format!("streaming open failed: {e:?}"))?;
    let open = open_start.elapsed();

    // A deep offset on purpose: a projection that is fast only near the top of
    // the file has not solved anything.
    let viewport_start = Instant::now();
    let viewport = engine
        .viewport_projection(EditorViewportRequest {
            buffer_id: buffer,
            scroll: ViewportScroll {
                top_line: 500_000,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 1_200,
                height_px: 24 * 16,
            },
        })
        .map_err(|e| format!("viewport projection failed: {e:?}"))?;
    let viewport_elapsed = viewport_start.elapsed();

    let payload_bytes: usize = viewport
        .line_slices
        .iter()
        .map(|slice| slice.visible_text.len())
        .sum();

    let mut samples = Vec::with_capacity(EDIT_SAMPLES);
    for index in 0..EDIT_SAMPLES {
        let at = TextPosition::new(100_000 + index, 0);
        let start = Instant::now();
        engine
            .apply_edit(
                buffer,
                TextEdit::insert(at, "x"),
                TransactionSource::User,
                None,
                None,
            )
            .map_err(|e| format!("insert failed: {e:?}"))?;
        samples.push(start.elapsed());

        engine
            .apply_edit(
                buffer,
                TextEdit::delete(TextRange::new(
                    at,
                    TextPosition::new(at.line, at.column + 1),
                )),
                TransactionSource::User,
                None,
                None,
            )
            .map_err(|e| format!("delete failed: {e:?}"))?;
    }

    let _ = std::fs::remove_dir_all(&dir);

    samples.sort();
    let p50 = percentile(&samples, 0.50);
    let p95 = percentile(&samples, 0.95);

    Ok(format!(
        concat!(
            "schema_version = 1
",
            "byte_len = {}
",
            "streaming = {}
",
            "open_millis = {}
",
            "viewport_millis = {}
",
            "edit_p50_millis = {}
",
            "edit_p95_millis = {}
",
            "viewport_payload_bytes = {}
"
        ),
        TARGET_BYTES,
        viewport.mode == ViewportProjectionMode::StreamingLargeFile,
        millis(open),
        millis(viewport_elapsed),
        millis(p50),
        millis(p95),
        payload_bytes,
    ))
}

/// Write a deterministic fixture of at least [`TARGET_BYTES`].
///
/// Written in chunks rather than built as one `String`: materializing 100MB to
/// generate a file whose whole point is never being materialized would make
/// the measurement's peak memory a property of the harness rather than the
/// editor.
fn write_fixture(path: &std::path::Path) -> Result<(), String> {
    let mut file = std::io::BufWriter::new(
        std::fs::File::create(path).map_err(|e| format!("cannot create fixture: {e}"))?,
    );
    let line = "the quick brown fox jumps over the lazy dog 0123456789\n";
    let per_chunk = 1024;
    let chunk: String = line.repeat(per_chunk);
    let mut written = 0usize;
    while written < TARGET_BYTES {
        file.write_all(chunk.as_bytes())
            .map_err(|e| format!("cannot write fixture: {e}"))?;
        written += chunk.len();
    }
    file.flush().map_err(|e| format!("cannot flush: {e}"))?;
    Ok(())
}

fn percentile(sorted: &[Duration], quantile: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let index = ((sorted.len() as f64 - 1.0) * quantile).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
