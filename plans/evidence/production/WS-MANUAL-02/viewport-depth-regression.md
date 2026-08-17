# The viewport regression was an O(scroll depth) loop in the editor (P1.F4.T2)

Date: 2026-08-17. Follows `large-file-typing-fix.md`, which recorded a viewport
measurement that got ~2.7 ms slower alongside the typing fix and said, correctly,
that the cause was not established.

It is established now. It was not the line table, and it was not really the
viewport either.

## Conclusion first

`EditorEngine::viewport_projection` computed the absolute UTF-16 offset of its
visible range by **summing `line_utf16_len` and `line_ending_bytes` over every
line from the start of the buffer**. That loop is O(scroll depth), it runs twice
per projection, and at `top_line = 500,000` it made two million line lookups.

The line-table change (`354c7da`) did not add that loop — it has always been
there. It made each lookup cost about 1.4 ns more, because `LineIndex::line` went
from returning `&LineMetric` (a pointer into a flat `Vec`) to returning a
`LineMetric` by value (a 48-byte copy plus an overlay check). Two million lookups
× 1.4 ns ≈ **2.8 ms**, which is the regression that was reported as 2.7 ms.

So the regression is real, its cause is a pre-existing algorithmic defect in
`legion-editor` amplified by a per-call cost increase in `legion-text`, and the
right fix is to delete the loop rather than to revert the typing fix.

`LineIndex::utf16_offset` now answers the same question against the rope in
O(log n). **Viewport at line 500,000 of the 100MB fixture: 5.92 ms → 0.05 ms.**

## Controlling for contamination

The prior note recorded that the harness reports `open` as 349 ms, 393 ms or
181 ms on the same tree depending on what else was compiling. Every number below
was taken by running `target/release/large_file_perf.exe` directly with no cargo
build in flight, and the stability of `open` across runs is the check that the
control held — it stays inside a 12 ms band throughout, where contaminated runs
moved it by 200 ms.

The regression reproduced under that control before anything was changed, so it
is not a measurement artifact:

| run | open (ms) | viewport (ms) | edit p50 (ms) |
| --- | ---: | ---: | ---: |
| 1 | 174.54 | 6.153 | 0.248 |
| 2 | 180.13 | 6.046 | — |
| 3 | 176.95 | 5.267 | — |
| 4 | 178.89 | 6.083 | — |
| 5 | 181.56 | 6.042 | — |
| **mean** | **178.4** | **5.92** | |

That agrees with the 5.44–5.94 ms recorded in `large-file-typing-fix.md`, on the
same machine, uncontaminated. The number was real.

## The measurement that found it

`crates/legion-editor/tests/viewport_depth_scaling.rs` sweeps `top_line` across a
fixed 1,000,000-line buffer and projects the same twenty-four lines at each depth.
The visible line count is identical at every point, so any slope is depth-dependent
work rather than projection work.

Release build, best of five projections per depth, µs:

| `top_line` | before `354c7da` | after `354c7da` | after this fix |
| ---: | ---: | ---: | ---: |
| 0 | 18 | 18 | 17 |
| 62,500 | 218 | 675 | 18 |
| 125,000 | 419 | 1,314 | 20 |
| 250,000 | 839 | 2,369 | 20 |
| 500,000 | 1,983 | 4,806 | 22 |
| 999,000 | 4,894 | 9,953 | 29 |
| **slope** | **4.88 ns/line** | **9.94 ns/line** | **flat** |

Three things fall out of that table, and each rules something out.

**At depth 0 the projection costs 18 µs, before and after, identically.** The
work of projecting twenty-four lines did not change. Whatever the harness was
measuring at 5.9 ms, 99.7% of it was not projection.

**The cost is linear in depth on both revisions.** Doubling `top_line` from
500,000 to 999,000 doubles the time (1,983 → 4,894 and 4,806 → 9,953). That is
the signature of a loop over preceding lines, not of heap layout, page-cache
behaviour, or anything about the rope's working set — the hypothesis the prior
note offered, and the one this rules out.

**The slope roughly doubled; the intercept did not move.** The change made each
iteration more expensive and touched nothing else. At the harness's
`top_line = 500,000` the table predicts a 2.82 ms delta; the reported regression
was 2.7 ms.

## Why the earlier isolated probes missed it

`large-file-typing-fix.md` probed exactly the right primitives —
`line_byte_len`, `line_utf16_len`, `line_ending_bytes`, `position`,
`utf16_position` — and found them unchanged. That result was correct and the
conclusion drawn from it was wrong, for one reason: the probe ran **twenty
iterations over twenty-four lines**, about 480 calls. The regression is ~1.4 ns
per call. Detecting it needs a workload around four thousand times larger, which
is what the real projection was doing all along and what the probe was not.

A per-call regression small enough to hide in the noise of a short probe can
still dominate a measurement, if something calls it two million times. The probe
was measuring the right function at the wrong scale.

## The fix

`LineIndex::utf16_offset(byte_offset)` (`crates/legion-text/src/lib.rs`) returns
the absolute UTF-16 code-unit offset in O(log n) via `Rope::byte_to_char` and
`Rope::char_to_utf16_cu`. `EditorEngine::absolute_utf16_offset`
(`crates/legion-editor/src/lib.rs`) delegates to it instead of summing lines.

**This is not the rope-backed line index that `large-file-typing-fix.md`
rejected.** That design would have let ropey decide where lines end, which is the
hazard that note documents — `unicode_lines` breaks on `\v`, `\f`, U+0085,
U+2028 and U+2029, and building without default features would have stopped
breaking on a lone `\r`. This uses the rope only to *count UTF-16 code units in a
byte range*. It asks nothing about lines, so no line-breaking rule is involved,
and `scan_line_metrics_from_byte` remains the only thing in the crate that
decides where a line ends. The rejection in that note stands; it does not apply
here.

The two formulations agree because every line ending Legion recognises — `\n`,
`\r\n`, and a lone `\r` — is ASCII, so its byte length equals its UTF-16 length.
That is why summing `line_ending_bytes` as if it were a UTF-16 count was correct,
and why counting the same bytes through the rope gives the same answer.

### Proving it is the same answer

UTF-16 offsets address LSP positions, so a one-unit drift would move diagnostics,
breakpoints and proposal ranges with nothing failing.
`crates/legion-text/tests/utf16_offset_equivalence.rs` keeps the old summation
verbatim as an oracle and compares the two at **every byte offset** of twelve
fixtures: the three line endings and a mixture of them, an offset inside a CRLF
pair, blank lines, 2-/3-/4-byte characters, surrogate pairs adjacent to CRLF, and
the five Unicode line-like characters that Legion deliberately does not break on.
It also asserts the two reject the same offsets, so the surrogate-interior
rejection is preserved rather than silently rounded.

One behaviour worth naming because it is easy to get wrong: an offset *inside* a
CRLF pair is a character boundary, so it is addressable, and the old code clamped
it to the end of the line's content. `utf16_offset` clamps identically, and
`an_offset_inside_a_crlf_pair_clamps_to_the_end_of_the_line_content` pins it.

The 15 tests in `line_index_characterization.rs` pass unchanged, including the
recorded golden.

### After

Standalone, no build in flight, five runs:

| run | open (ms) | viewport (ms) | edit p50 (ms) |
| --- | ---: | ---: | ---: |
| 1 | 185.35 | 0.0496 | 0.2485 |
| 2 | 183.49 | 0.0512 | 0.2696 |
| 3 | 184.21 | 0.0516 | 0.2761 |
| 4 | 184.55 | 0.0519 | 0.2708 |
| 5 | 186.82 | 0.0517 | 0.2720 |

| metric | before `354c7da` | after `354c7da` | after this fix |
| --- | ---: | ---: | ---: |
| viewport at line 500,000 | ~2.9 ms | 5.92 ms | **0.052 ms** |
| open | ~182 ms | 178.4 ms | 184.9 ms |
| edit p50 | 22.6 ms | 0.25 ms | 0.27 ms |

Open and typing are unchanged, which is the check that this fix is confined to
the path it was aimed at. Viewport is 114× faster than the regressed measurement
and 56× faster than the pre-regression baseline.

## The part that matters more than the regression

The regression was 19% of the 32 ms scroll budget and not gated — the harness
gates `m9.large_file_100mb` on keypress p50 only. The defect underneath it was
worse than the regression, and it predates the typing fix.

Because the loop is O(scroll depth), its cost grows with how far down the file
the user has scrolled, without bound. The 100MB fixture is ~1,906,000 lines. At
the measured slopes, a projection at the **bottom** of that file would have cost
about 9.3 ms before `354c7da` and about **18.9 ms after** — 59% of the 32 ms
scroll budget, on a metric nothing gates, reached by the ordinary act of
scrolling to the end of a large file. It is now ~0.05 ms at any depth.

That is the finding worth keeping: the harness measures depth 500,000, which is
about a quarter of the way into the fixture, so the reported number understated
the real worst case by roughly 4×.

## Not claimed

**The 18.9 ms bottom-of-file figure is extrapolation, not measurement.** It is
the measured 9.94 ns/line slope multiplied by the fixture's line count. I did not
project at the bottom of the 100MB file; the sweep that produced the slope ran on
a 1,000,000-line in-memory buffer, not on the streamed 100MB one. The slope is
well supported by six points across two revisions; the multiplication is
arithmetic, not evidence.

**The two buffers are not the same workload.** The depth sweep uses
`open_buffer` on an in-memory 55 MB string; `large_file_perf` uses
`open_buffer_streaming` on a 100MB file on disk. They agree on the shape and
roughly on the magnitude (4.8 ms vs 5.9 ms at depth 500,000), and the fix moves
both to ~20–50 µs, but the absolute numbers are not interchangeable and the
difference between them is not explained here.

**One machine, one OS, Windows.** As with the note this follows, the before/after
comparison is same-machine and that is the part that carries; the absolute
numbers are not portable. The 3-OS matrix (P8.F4.T2) is what would say whether
they are representative.

**No claim that this was the only cost in the projection.** It was 99.7% of the
measurement at depth 500,000, and what remains at depth 0 (17–18 µs) is unchanged
by this work and unexamined. If a future scroll budget gets tight, that 18 µs is
where to look next, and `chunk_hash_for_line` — a linear scan over ~1,600 chunk
descriptors per visible line, with two string allocations each — is the first
thing in it worth measuring.

**The renderer is still not measured.** Unchanged from
`large-file-typing-fix.md`: `m9.large_file_100mb` measures the text model and the
viewport projection and does not paint. This does not supply renderer-backed
evidence for PR-UI-002 promotion.

**`absolute_utf16_offset` is the only caller fixed, and it is not the only one
with the defect.** A sweep of `legion-editor` and `legion-text` for the same
shape found one more:
`EditorEngine::byte_offset_from_absolute_utf16` (`crates/legion-editor/src/lib.rs`,
~line 2213) is the inverse conversion and walks lines from zero the same way,
subtracting `line_utf16_len` and `line_ending_bytes` until it finds the line.

It is left alone deliberately. It sits on the LSP completion path
(`completion_byte_offset`), not the viewport path, so it is not part of this
measurement and no instrument here covers it. `Rope::utf16_cu_to_char` would
answer it in O(log n) exactly as `char_to_utf16_cu` does for the forward
direction, so the remedy is known — but it needs its own equivalence proof and
its own before/after on a path I have not characterized, and folding it in would
have widened this diff past what the measurement supports. **It is a real
O(document length) cost on completion requests, unmeasured.**

## Gate results

`cargo fmt --all` clean · `cargo test --workspace --all-targets --no-fail-fast`
exit 0 · `cargo clippy --workspace --all-targets -- -D warnings` exit 0, zero
diagnostics · `verify-perf-harness` total=6 passed=5 failed=0 skipped=1,
strict=true · `extract-before-modify` no chokepoint grew past its slack ·
`docs-hygiene` passed · `claim-audit` passed.

`perf-harness`, the row this note is about:

```
skeleton=m9.large_file_100mb kind=large_file_100mb total_us=8640 p50_us=270
p95_us=275 budget_ms=16 status=passed message=100MB streaming open=186.0ms
viewport=0.1ms edit_p50=0.3ms edit_p95=0.3ms viewport_payload=1296B
```

`viewport=0.1ms`, against 5.6 ms before this change. The harness rounds to a
tenth of a millisecond, so the standalone figure (0.052 ms) is the more precise
one.

## A pre-existing flake found on the way, not fixed

The first `cargo test --workspace --all-targets` run failed one test:
`terminal_orphan_cleanup_kills_and_records_evidence`
(`crates/legion-app/tests/terminal_workflow.rs:697`), with
`cleanup must return exactly one audit record for the orphaned session; got: []`.

It is not related to this change and it is not caused by it. Evidence: the test
passes standalone on a tree with these changes stashed, passes standalone with
them applied (all 9 tests in the file), and the second full-workspace run passed
with exit 0 and no failures.

The cause is visible in the test: it launches `cmd /C exit`, sleeps a fixed
400 ms, and then asserts the process has already exited and been detected. Under
the parallel load of `--all-targets` across the whole workspace, 400 ms is not
reliably enough for a process to spawn and exit. That is a wall-clock assumption
about the machine the test runs on — the same class as the three CI failures this
workstream already recorded — and it will flake on a shared runner. It is
reported here rather than fixed: it is a different subsystem from this task and
its fix is a synchronisation change, not a timing constant.

## Reproduction

```
cargo test -p legion-text --test utf16_offset_equivalence
cargo test -p legion-text --test line_index_characterization
cargo test -p legion-editor --release --test viewport_depth_scaling -- --ignored --nocapture
cargo build --release -p legion-app --bin large_file_perf
./target/release/large_file_perf --report <path>     # with nothing else building
```
