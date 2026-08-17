//! Does a keystroke cost more the earlier in the file it lands?
//!
//! Diagnostic, not a gate. `rebuild_from_simple_edit` clones the whole line
//! metrics vector and then shifts every line after the edit, so the cost of one
//! keystroke should scale with how much file is *below* the cursor. If that is
//! what is happening, editing the first line is the worst case and editing the
//! last line is nearly free — and the shape of that curve is the proof.
//!
//! Ignored because it builds a large buffer and reports timings rather than
//! asserting on them; run with `--ignored --nocapture`.

use std::time::Instant;

use legion_text::{TextBuffer, TextEdit, TextPosition, TextRange};

fn buffer_with_lines(count: usize) -> TextBuffer {
    let mut text = String::with_capacity(count * 40);
    for index in 0..count {
        text.push_str(&format!(
            "line {index:07} of the fixture, padded out a bit\n"
        ));
    }
    TextBuffer::new(text)
}

fn median_edit_micros(buffer: &mut TextBuffer, line: usize, samples: usize) -> u128 {
    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let at = TextPosition::new(line, 0);
        let start = Instant::now();
        buffer
            .try_apply_edit(&TextEdit {
                range: TextRange::new(at, at),
                new_text: "x".to_string(),
            })
            .expect("insert");
        timings.push(start.elapsed().as_micros());
        buffer
            .try_apply_edit(&TextEdit {
                range: TextRange::new(at, TextPosition::new(line, 1)),
                new_text: String::new(),
            })
            .expect("delete");
    }
    timings.sort_unstable();
    timings[timings.len() / 2]
}

/// Same bytes, a tenth the lines. If the cost is the line-metrics vector it
/// should fall by roughly ten; if it is the rope or the chunk hash it should
/// barely move.
fn buffer_with_long_lines(count: usize, per_line: usize) -> TextBuffer {
    let mut text = String::with_capacity(count * (per_line + 1));
    for index in 0..count {
        let head = format!("line {index:07} ");
        text.push_str(&head);
        text.extend(std::iter::repeat_n(
            'y',
            per_line.saturating_sub(head.len()),
        ));
        text.push(0x0a as char);
    }
    TextBuffer::new(text)
}

#[test]
#[ignore = "diagnostic: reports timings rather than asserting; run with --ignored --nocapture"]
fn keystroke_cost_tracks_line_count_not_byte_count() {
    // ~40MB either way; only the line count differs.
    let mut many_short = buffer_with_lines(1_000_000);
    let mut few_long = buffer_with_long_lines(100_000, 400);
    println!(
        "1,000,000 short lines ({} bytes): {}us",
        many_short.len(),
        median_edit_micros(&mut many_short, 500_000, 16)
    );
    println!(
        "  100,000 long  lines ({} bytes): {}us",
        few_long.len(),
        median_edit_micros(&mut few_long, 50_000, 16)
    );
    println!(
        "Comparable bytes. If the second is roughly a tenth of the first, the cost          is the per-edit copy of the line-metrics vector, not the rope."
    );
}

#[test]
#[ignore = "diagnostic: reports timings rather than asserting; run with --ignored --nocapture"]
fn keystroke_cost_by_position_in_the_file() {
    for line_count in [50_000usize, 200_000, 800_000] {
        let mut buffer = buffer_with_lines(line_count);
        let first = median_edit_micros(&mut buffer, 0, 16);
        let middle = median_edit_micros(&mut buffer, line_count / 2, 16);
        let last = median_edit_micros(&mut buffer, line_count - 2, 16);
        println!(
            "lines={line_count:>7}  first={first:>7}us  middle={middle:>7}us  last={last:>7}us"
        );
    }
    println!(
        "If `first` grows with line_count and `last` stays flat, the cost is the \
         per-edit shift of every line below the cursor."
    );
}
