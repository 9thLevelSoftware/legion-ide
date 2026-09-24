# Typing in a 100MB file: the fix (P1.F4.T2)

Date: 2026-08-16. Follows `large-file-typing-diagnosis.md`, which established the
cause and named two candidate designs.

## Which design, and why not the other

**Chosen: design 1 — share the base, defer the shift.** `LineTable`
(`crates/legion-text/src/line_table.rs`) holds the scanned metrics in an `Arc`
that is never mutated, plus a bounded overlay of pending
`(line, byte_delta, utf16_delta)` shifts. A new table per edit clones only the
overlay; reads fold the pending deltas over the base on the way out, which is
why `LineIndex::line` now returns a `LineMetric` by value.

**Rejected: design 2 — ask the rope.** Two reasons, the second decisive.

The first is the hazard the diagnosis named: ropey 1.6 enables `unicode_lines`
by default and breaks lines on `\v`, `\f`, U+0085, U+2028 and U+2029, which
Legion does not. That one is manageable — it is a feature flag.

The second is that **the diagnosis states Legion's line rule incorrectly, and so
does the brief.** Both say `scan_line_metrics_from_byte` "breaks only on `\n`
and treats `\r\n` as a single ending." It also breaks on a **lone `\r`**, with
one ending byte — the classic-Mac case. The scanner holds a `pending_cr` and,
when the next byte is not `\n`, closes the line at the carriage return.

That matters because the obvious remedy for hazard one — build ropey with
default features off — would have produced a rope that does *not* break on lone
`\r`, silently merging classic-Mac lines. The only correct ropey configuration
is `cr_lines` alone, which the diagnosis hedged toward without being able to say
why. A design whose correctness depends on getting a feature flag exactly right,
against a written line rule that is itself wrong, is the wrong design for the
crate every position in the product resolves through.

Design 1 sidesteps the entire question: `scan_line_metrics_from_byte` remains
the only producer of `LineMetric`s, so the new module shifts offsets and never
decides where a line ends. `a_lone_carriage_return_ends_a_line` in the
characterization suite pins the real rule either way.

## Before and after

Both instruments, both run on this machine, before and after, by swapping only
`crates/legion-text/src/lib.rs` between the two revisions.

### `perf-harness`, row `m9.large_file_100mb` — the verdict

| metric | before | after | ADR-0048 budget | verdict |
| --- | ---: | ---: | ---: | --- |
| **edit p50** | **22.8 ms** | **0.4 ms** | keypress p50 < 16 ms | **failed → passed** |
| edit p95 | 23.9 ms | 0.5 ms | keypress p95 < 32 ms | passes |

`status=failed` → `status=passed`. Reproduce with
`cargo run -p xtask -- perf-harness` and read `m9.large_file_100mb`.

### The same measurement, run standalone

The harness spawns `cargo run --release -p legion-app --bin large_file_perf`
while its own builds are still running, so the numbers it reports for open and
viewport move with that load — the same post-change tree reported open as
348.9 ms, 392.7 ms and 184 ms depending on what else was compiling. Running the
subprocess directly, three samples per revision, removes that:

| metric | before (3 runs) | after (5 runs) | budget |
| --- | ---: | ---: | ---: |
| open | 183.0, 180.4, 183.0 ms | 183.5, 181.1, 187.2, 230.6, 188.2 ms | — |
| viewport at line 500,000 | 3.17, 2.64, 2.94 ms | 5.45, 5.44, 5.61, 5.94, 5.69 ms | scroll p95 < 32 ms |
| edit p50 | 22.56, 22.61, 22.49 ms | 0.246, 0.242, 0.261, 0.275, 0.271 ms | p50 < 16 ms |
| edit p95 | 23.32, 24.24, 23.75 ms | 0.281, 0.337, 0.309 ms | p95 < 32 ms |

> **Resolved 2026-08-17 — see `viewport-depth-regression.md`.** The regression was
> real and reproducible, and the cause was not in this change's data structure.
> `EditorEngine::viewport_projection` summed per-line metrics from the start of the
> buffer to compute its visible range's UTF-16 offset — an O(scroll depth) loop,
> run twice per projection, making two million line lookups at `top_line` 500,000.
> This change made each lookup ~1.4 ns dearer (`LineIndex::line` returns by value
> rather than by reference), which is 2.8 ms across two million of them. The loop
> has been replaced with an O(log n) rope conversion; the viewport measurement is
> now 0.05 ms. The heap-layout hypothesis below is **disproved** — the cost is
> linear in scroll depth, which heap layout would not be.

Open is unchanged. Typing is about 90× faster. **The viewport measurement is
consistently about 2.7 ms slower, and that is a real regression I have not been
able to explain.** It reproduces across five post-change runs taken both before
and after the pre-change group, so it is not drift in machine state, and the
variance within each group is small.

What rules out the obvious explanation: an isolated probe of every line-index
primitive the viewport uses — `visible_line_slices`, `line_byte_len` /
`line_utf16_len` / `line_ending_bytes`, `position`, `utf16_position` — measures
the same before and after (112 → 110 µs, 1 → 4 µs, 47 → 33 µs, 157 → 165 µs for
twenty iterations over twenty-four lines), and a cold first
`visible_line_slices` at line 500,000 costs 14 µs. The changed primitives are
three orders of magnitude smaller than the 5.6 ms being measured, which is
dominated by the editor layer materializing twenty-four line slices out of a
cold, disk-backed 100MB rope.

So the cost is real, reproducible, and not in the code this change touched by
any measurement I can construct. The most likely remaining explanation is heap
layout: the metrics vector now sits behind an `Arc` allocation rather than
inline in the index, which moves 82 MB of working set relative to the rope's
pages. That is a hypothesis, not a finding. It is 17% of the 32 ms scroll
budget and worth a follow-up; it is recorded here rather than smoothed over.

### `edit_position_scaling` — the microbenchmark

`cargo test -p legion-text --release --test edit_position_scaling -- --ignored --nocapture`

| fixture | before | after |
| --- | ---: | ---: |
| 1,000,000 short lines (46 MB) | 12,771 µs | 55 µs |
| 100,000 long lines (40 MB) | 1,247 µs | 51 µs |
| 800,000 lines, edit first line | 11,785 µs | 51 µs |
| 800,000 lines, edit middle | 13,649 µs | 51 µs |
| 800,000 lines, edit last line | 16,101 µs | 47 µs |

The shape matters more than the ratio. Before, ten times fewer lines at the same
byte count was ten times faster — the cost tracked the metrics vector. After,
1,000,000 lines and 100,000 lines cost the same 53-56 µs, because the vector is
no longer touched.

## This design amortizes; here is the whole distribution

Most keystrokes append to the overlay. One in `COMPACTION_THRESHOLD` (64) folds
the overlay into a fresh base and pays the full O(lines) copy. Reporting only
the median would describe half the design.

At 1,000,000 lines, over 1,024 timed keystrokes
(`keystroke_cost_amortized_versus_compaction`):

| statistic | measured | budget | verdict |
| --- | ---: | ---: | --- |
| p50 | 53-54 µs | keypress p50 < 16 ms | passes with three orders of magnitude spare |
| p95 | 58-67 µs | keypress p95 < 32 ms | passes |
| p99 | 9.0-9.9 ms | — | compaction begins here |
| **max** | **10.4-11.4 ms** | **p95 < 32 ms** | **within, at about a third of it** |

Ranges are across two runs of the same build; the compaction cost is the least
stable number here because it is one large allocation and copy.

The worst single keystroke costs 10-11 ms. That is a real cost and it is what a
user would feel on the unlucky keystroke, but it is inside the p95 budget and
occurs on 1.6% of edits, so it lands at p99 rather than p95. At the 100MB
file's ~1.7M lines it would scale to roughly 18-19 ms — still inside 32 ms,
though that is extrapolation rather than measurement.

**A worst case measured wrong is worse than not measuring it.** The first
version of this diagnostic timed only the insert half of each
insert/delete pair, and compaction consistently landed on the delete. It
reported a 1.8 ms maximum for a path that actually cost 97 ms. The diagnostic now
times both halves.

That 97 ms was real, and it was mine: the first `materialize` applied each
pending shift in its own pass over the table, so compaction walked 24 MB up to 63
times — about 1.5 GB of memory traffic, seven times worse than the problem being
fixed. Both folds now sum the deltas and apply them once, and `materialize`
builds the new vector directly rather than cloning and rewriting it. 97 ms →
14.4 ms → 10.4 ms.

## Characterization tests

Written and passing **before** the change, unchanged after it:
`crates/legion-text/tests/line_index_characterization.rs` (15 tests).

- `empty_buffer_has_a_single_empty_line` — an empty buffer holds one line.
- `trailing_newline_produces_a_final_empty_line` — the last line, with and
  without a trailing newline.
- `crlf_is_a_single_two_byte_line_ending` — `\r\n` is one ending; the `\r` is
  excluded from the column length.
- `a_lone_carriage_return_ends_a_line` — pins the rule the diagnosis got wrong.
- `unicode_line_like_characters_do_not_break_lines` — a fixture containing
  `\v`, `\f`, U+0085, U+2028 and U+2029, asserting they do **not** break lines.
  This is the ropey hazard, pinned whether or not anyone adopts ropey later.
- `multibyte_and_astral_plane_characters_map_correctly` — 2-, 3- and 4-byte
  characters; the emoji is a surrogate pair, and addressing its middle is
  rejected rather than rounded.
- `byte_and_utf16_positions_round_trip_across_fixtures` — round trips over ten
  fixtures.
- Six `*_match_a_fresh_scan` tests — after each edit in a sequence, the
  incrementally maintained index must be indistinguishable from one scanned
  fresh over the same final text, compared across every public accessor at every
  line and every byte offset. Insertions, deletions, CRLF text, edits beside
  surrogate pairs, 400 sequential edits through several compactions, and edits
  at both ends of a 200-line buffer.
- `fixture_mappings_match_golden` — the absolute mapping, recorded from the
  pre-change implementation into `tests/golden/line_index_mappings.txt`.

That last one exists because the `*_match_a_fresh_scan` tests have a blind spot:
both sides run the same lookup code, so a change to the binary search *itself*
would move both together and pass. The golden is the only check in the suite that
catches that class of drift, and it is the reason the lookup could be rewritten
with any confidence.

`line_table.rs` carries seven unit tests of its own, including an independent
oracle (`apply_directly`) that is the implementation the table replaced, so the
deferred and compacted paths are checked against something sharing none of their
machinery.

## Not claimed

**The perf-harness row never observes a compaction.** `large_file_perf` sets
`EDIT_SAMPLES = 32` and times only the insert half of each insert/delete pair,
so its measured window is 64 edits with the overlay never reaching its
64-shift bound. Its `edit_p50 = 0.4 ms` is therefore a clean measurement of the
overlay path and *not* of the amortized worst case. The 10.4 ms figure above
comes from the 1,024-sample diagnostic, which is the only instrument here that
exercises compaction. Both are reported because neither alone is the whole
picture.

**The renderer is not measured here.** `m9.large_file_100mb` measures the text
model and the viewport projection through `legion-app --bin large_file_perf`. It
does not paint. P1.F4.T2's acceptance clause "scrolls, and does not block
typing" is now met at the harness level on one machine; renderer-backed
large-file UX evidence is still what PR-UI-002 needs for promotion beyond
substrate validated, and this does not supply it.

**One machine, one OS.** Windows, single run per configuration. The 3-OS matrix
(P8.F4.T2) is what would say whether these numbers are representative. The
before/after comparison is same-machine, which is the part that matters for the
delta; the absolute numbers are not portable.

**The chunk vector is still cloned on every keystroke.** `rebuild_from_simple_edit`
still does `self.inner.chunks.clone()`, and each `TextChunkDescriptor` owns a
heap-allocated hash string. At 64KB chunks that is ~1,600 allocations per
keystroke for a 100MB file. It did not need fixing to meet the budget — the
whole edit path is now 0.3 ms — and the same overlay technique would apply if it
ever does. It is left alone deliberately rather than overlooked.

**Compaction is not incremental.** The 10.4 ms worst case could be spread across
keystrokes rather than paid at once. That is more machinery than the budget
requires today.

**No claim about non-simple edits.** Edits containing `\n` or `\r`, and edits
whose chunk grows past its bound, still take the full-rebuild path exactly as
before. Only the no-line-break fast path changed.

## Reproduction

```
cargo test -p legion-text --test line_index_characterization
cargo test -p legion-text --release --test edit_position_scaling -- --ignored --nocapture
cargo run -p xtask -- perf-harness      # row m9.large_file_100mb
```
