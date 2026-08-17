# Why typing in a 100MB file misses its budget (P1.F4.T2)

Date: 2026-08-16. Follows `large-file-100mb-measurement.md`, which recorded the
failure without explaining it.

## The failure, restated

`m9.large_file_100mb` measures edit p50 at **23.2 ms** against ADR-0048's 16 ms
keypress budget. Opening and scrolling are comfortably inside budget; typing is
not. P1.F4.T2's acceptance — "100MB file opens within budget, scrolls, and does
not block typing" — fails on the third clause.

## What it is

Every keystroke copies the entire line-metrics vector.

`TextBuffer::try_replace_range` takes a fast path for edits that introduce no
line break: `ChunkedLineIndex::rebuild_from_simple_edit`. That function opens
with `let mut lines = self.inner.lines.clone();` and then shifts the byte
offsets of every line after the edit. Both are O(total lines) — and the clone is
by far the larger half.

`LineMetric` is six `usize` fields, 48 bytes. A 100MB file of source-like lines
holds roughly 1.7 M of them, so the vector is about 82 MB. Copying 82 MB at
typical memory bandwidth is roughly 20 ms, which is the measurement.

## How that was established

`crates/legion-text/tests/edit_position_scaling.rs`, run with
`cargo test -p legion-text --release --test edit_position_scaling -- --ignored --nocapture`.

**Position barely matters.** If the suffix-shift loop dominated, editing the last
line would be nearly free and editing the first would be worst:

```
lines=  50000  first=   1246us  middle=    664us  last=    494us
lines= 200000  first=   3244us  middle=   2952us  last=   2569us
lines= 800000  first=  18945us  middle=  24022us  last=  21729us
```

First and last are the same order of magnitude at every size, so the shift loop
is the minority cost. The difference between them at 800 K lines — about 1.4 ms
— is roughly what the shift loop actually costs.

**Line count matters; byte count does not.** Two buffers of comparable size:

```
1,000,000 short lines (46,000,000 bytes): 14889us
  100,000 long  lines (40,100,000 bytes):  2068us
```

Ten times fewer lines at the same byte count is seven times faster. The cost
tracks the length of the metrics vector, not the length of the text. That is the
copy.

## Why it is structural, not a missing early-return

The index cannot simply be mutated in place. `TextBuffer` holds
`line_index: Arc<LineIndex>`, and `TextSnapshot` holds its own `Arc<LineIndex>`
so that a snapshot keeps the line mapping it was taken with. Snapshots are
created on every edit and retained, so the `Arc` is essentially always shared
and `Arc::make_mut` would clone anyway. `EditorEngine::apply_edits` also clones
the buffer into a staging copy before applying, which shares it a second time.

A new immutable index per edit is therefore the design, and a flat `Vec` cannot
produce one without copying it.

## The two ways out

**1. Share the base and defer the shift.** Keep `lines` as
`Arc<Vec<LineMetric>>`, never mutated, plus a small overlay of edits applied
since it was built — each a `(line, byte_delta, utf16_delta)`. A snapshot pins
the shared base and its own short overlay, so nothing is copied per edit. Reads
compute a metric from base plus overlay, which makes `LineIndex::line` return by
value rather than by reference (it is private; the six-field copy is free).
Compact when the overlay passes a threshold, paying the O(lines) copy once per
threshold-many keystrokes.

Contained: the overlay is internal to `ChunkedLineIndex`, and
`chunk_descriptors()` — which 36 call sites take as `&[TextChunkDescriptor]` —
does not need to change, because chunks are bounded in size and few.

**2. Stop maintaining a parallel line index and ask the rope.** ropey answers
`len_lines`, `line_to_byte`, `byte_to_line`, `line(i)` and `len_utf16_cu` in
O(log n), which is every field of `LineMetric`. This is the smaller data
structure and the smaller code.

**It carries one hazard that would not announce itself.** ropey 1.6 enables
`unicode_lines` by default, so it breaks lines on lone `\r`, `\v`, `\f`, U+0085,
U+2028 and U+2029. `scan_line_metrics_from_byte` breaks only on `\n` and treats
`\r\n` as a single ending. Any file containing one of those characters would
silently renumber, putting every diagnostic, breakpoint, LSP position and
proposal range on the wrong line — with no test failing unless a fixture happens
to contain one. Taking this route means building ropey without default features
(or with `cr_lines` only) and proving the line-break rule with a fixture
containing each of those characters.

## Not claimed

Neither fix is implemented. The measurement, the cause and the two designs are
established; the work is not done, and P1.F4.T2 stays `in-progress` until a
measurement under 16 ms exists rather than a plan for one.

The diagnostic test is `#[ignore]`d and reports timings rather than asserting on
them. It is a measuring instrument, not a gate: wall-clock thresholds in the
workspace suite would flake on a shared runner, and `perf-harness` is where
budgets are enforced.
