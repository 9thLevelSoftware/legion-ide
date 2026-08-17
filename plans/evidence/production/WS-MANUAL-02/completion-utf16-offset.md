# The inverse conversion: measured, it mattered, and it was also wrong (P1.F4.T2)

Date: 2026-08-17. Follows `viewport-depth-regression.md`, which fixed the forward
UTF-16 conversion on the viewport path and recorded the inverse conversion —
`EditorEngine::byte_offset_from_absolute_utf16` — as "a real O(document length)
cost on completion requests, **unmeasured**".

It is measured now. It cost 5.6 ms at a million lines, and while measuring it I
found it also resolved the wrong position.

## Conclusion first

Two findings, one change.

**Performance.** The function walked lines from the start of the buffer,
subtracting each one's UTF-16 content and ending lengths until the offset fell
inside a line. That is O(document length). A completion request at line 999,000
of a 1,000,000-line buffer cost **5.65 ms**, essentially all of it this walk. It
is now under the 1 µs resolution of the measurement at every depth.

**Correctness, found while measuring.** The walk could never leave a residual of
zero for any line after the first, because an offset landing on a line ending was
clamped to that line's content end *before* the next line was considered. A
UTF-16 offset addressing the **start of a line therefore resolved to the end of
the previous line**. LSP positions are UTF-16 natively, so this was every
completion requested at column 0.

The second finding is why this note is not simply "the same fix as last time".

## Measured first, as instructed

`crates/legion-editor/tests/completion_depth_scaling.rs` issues a real
`EditorEngine::completion` request — not a synthetic call to the private
conversion — at increasing depths in a fixed 1,000,000-line buffer, using
`TextOffset::utf16` because that is the encoding an LSP client sends. Only the
depth of the requested position varies, so any slope is depth-dependent work
inside the request.

Release build, best of five, µs:

| depth (line) | before | after |
| ---: | ---: | ---: |
| 0 | 0 | 0 |
| 62,500 | 303 | 0 |
| 125,000 | 590 | 0 |
| 250,000 | 1,198 | 0 |
| 500,000 | 2,685 | 0 |
| 999,000 | 5,645 | 0 |
| **slope** | **5.66 ns/line** | **flat** |

Paired A/B in immediate succession with nothing else building — the changes
stashed for the "before" column and restored for the "after". An earlier
independent "before" run of the same instrument gave 5,656 µs at depth 999,000
against this run's 5,645 µs, so the measurement is stable to about 0.2%.

**Depth 0 costs nothing**, which attributes the entire cost to the walk rather
than to the rest of the request. That is the same reading that settled the
viewport question, and it is the reason this instrument is trustworthy: the
buffer is large enough to be degraded, so `completion` fails closed to an empty
item list almost immediately. On exactly the files where the walk was longest,
its result was then thrown away.

"After" is not zero, it is below the microsecond resolution of the instrument.
The honest statement is **under 1 µs at a million lines**, not "free".

### Was it big enough to matter?

Yes, and this note would have said so plainly if not. 5.65 ms at a million lines
is the same order as the 2.7 ms viewport regression this workstream was opened
to explain, it is linear in document length so it grows without bound, and it
sits on a request a user triggers while typing. At the 100MB fixture's ~1,906,000
lines the same slope extrapolates to about 10.8 ms — extrapolation, not
measurement, and flagged as such.

## The correctness finding

The walk:

```rust
if remaining <= line_utf16_len { return ...(line, remaining); }
remaining -= line_utf16_len;
let line_ending_len = ...;
if remaining <= line_ending_len { return ...(line, line_utf16_len); }  // clamp
remaining -= line_ending_len;
```

To reach line `L` with a residual of zero, `remaining` would have to be exactly
zero at the top of iteration `L`. It never is: the ending branch fires when
`remaining <= line_ending_len`, so the only way to continue is
`remaining > line_ending_len`, which leaves `remaining >= 1` after the
subtraction. Every line start but the first was therefore unreachable, and
resolved instead to the previous line's content end.

**Established by observation, not by reading.** The resolved byte offset is not
returned by `completion`, but it is observable: completions are filtered by the
identifier prefix ending at the resolved offset. On the all-ASCII buffer
`"alpha_one\nbravo_two\n"`, where a byte offset and a UTF-16 offset are the same
number, offset 10 is the start of the second line:

```
byte(10)  -> ["alpha_one", "bravo_two"]     # empty prefix, both offered
utf16(10) -> ["alpha_one"]                  # prefix "alpha_one" — resolved to byte 9
```

The two encodings of the same position disagreed. That probe is now a permanent
test, `utf16_and_byte_encodings_agree_on_a_line_start`.

## Why the two findings could not be separated

Repository practice is to keep a bug fix out of a performance change. That was
not available here: the walk *is* the search, and replacing a linear search with
a rope conversion forces an answer to "what is at a line start?". The old code
answered "the end of the previous line" as an artifact of its clamping, and no
O(log n) formulation reproduces that artifact without deliberately reintroducing
it. Keeping the two apart would have meant shipping a faster version of a known
wrong answer.

So it is one change, and the behaviour difference is bounded by proof rather than
by argument.

## The change

`LineIndex::utf16_offset_to_line` (`crates/legion-text/src/lib.rs`) finds the line
holding a UTF-16 offset in O(log n) via the rope, returning the line and the
offset's UTF-16 position within it. `byte_offset_from_absolute_utf16`
(`crates/legion-editor/src/lib.rs`) uses it and keeps the per-line logic it always
had — clamp into the line's content, then delegate to
`TextBuffer::byte_offset_from_utf16`.

What is preserved, deliberately:

- **An offset inside a line ending still clamps to the end of that line's
  content.** That clamp was correct — a line ending has no column — and it is the
  same rule `LineIndex::utf16_position` follows.
- **An offset inside a surrogate pair is still rejected, not rounded.**
  `byte_offset_from_utf16` raises `Utf16InsideSurrogatePair`; the new search
  deliberately reports a residual that can land mid-pair rather than silently
  snapping to a character boundary, so that rejection still fires.
- **An offset past the end of the buffer is still refused** with
  `InvalidCompletionPosition("utf16 offset outside buffer")`.

As with the forward direction, the rope is used only to count and locate UTF-16
code units. It is not asked where lines end, so ropey's `unicode_lines` rule is
not involved and `scan_line_metrics_from_byte` remains the sole authority on line
breaks.

### Bounding the behaviour change by proof

`crates/legion-text/tests/utf16_offset_to_line_equivalence.rs`, over the same
twelve fixtures as the forward direction (three line endings and mixtures, blank
lines, 2-/3-/4-byte characters, surrogate pairs beside CRLF, and the five Unicode
line-like characters Legion does not break on):

- `the_search_matches_a_correct_reference_at_every_offset` — checks the new
  search against a straightforward correct reference at every UTF-16 offset.
- `offsets_past_the_end_of_the_buffer_resolve_to_nothing`.
- `the_only_offsets_that_changed_are_line_starts` — keeps the old walk verbatim
  as an oracle and asserts that old and new agree at **every** offset that is not
  a line start, and differ at **every** offset that is. This is the load-bearing
  one: it makes "the change affects exactly the line starts" a proved statement
  over the whole corpus rather than a claim about the code I happened to read.

`crates/legion-editor/tests/utf16_completion_position.rs` covers the observable
behaviour through the public API, including a multibyte case
(`"日本語\nbravo_two\n"`, where UTF-16 offset 4 and byte offset 10 are the same
position) so the tests would fail if the resolver started treating UTF-16 offsets
as bytes.

The 15 `line_index_characterization.rs` tests and the four
`utf16_offset_equivalence.rs` tests pass unchanged. No existing test in the
workspace depended on the old line-start behaviour: the full suite passes at exit
0 without modification.

## Gate results

`cargo fmt --all` clean; `--check` exit 0 · `cargo test --workspace --all-targets
--no-fail-fast` **exit 0**, no failures · `cargo clippy --workspace --all-targets
-- -D warnings` exit 0, zero diagnostics · `perf-harness` exit 0 ·
`verify-perf-harness` total=6 passed=5 failed=0 skipped=1, strict=true ·
`extract-before-modify` no chokepoint grew past its slack · `docs-hygiene` passed
· `claim-audit` passed.

`m9.large_file_100mb`: `open=205.5ms viewport=0.1ms edit_p50=0.3ms edit_p95=0.3ms
status=passed`. The completion path is not in this row — the harness does not
measure completions — so the row is here as a no-regression check, not as
evidence for this change. `open` at 205.5 ms is higher than the 186 ms recorded
in the previous note; both are harness-run figures taken while cargo was active,
which is the variance that note documented, and the standalone figure is the one
to compare.

## Not claimed

**No harness coverage.** `perf-harness` has no completion row, so nothing gates
this and nothing will catch a regression in it. The instrument is an `#[ignore]`d
diagnostic that reports rather than asserts, following the workspace convention
for wall-clock measurements. A gated completion budget does not exist and this
does not add one.

**The "after" number is a resolution floor, not a measurement of zero.** The
instrument reports whole microseconds and the operation is now below that at a
million lines. I did not measure what it actually costs; I measured that it is
under 1 µs.

**The 10.8 ms figure for the 100MB fixture is extrapolation.** Measured slope ×
line count, on an in-memory buffer rather than the streamed 100MB one. The same
caveat as the previous note, for the same reason.

**One machine, one OS, Windows.** The before/after is same-machine and paired,
which is the part that carries. Absolute numbers are not portable.

**The correctness fix is scoped to the UTF-16 offset encoding on the completion
path.** `TextOffset::byte` short-circuits this resolver entirely and is
unaffected. I did not audit whether other UTF-16 offset consumers elsewhere in
the workspace carry the same off-by-one; `byte_offset_from_absolute_utf16` was
the only caller of the walk.

**Whether any client was relying on the old behaviour is unknown.** No test was,
and the old behaviour is not defensible as an intentional rule, but "no test
depended on it" is a weaker statement than "nothing depended on it".

## Still open, unchanged from the previous note

The residual 17–18 µs of a viewport projection at depth 0 remains unexamined, and
`chunk_hash_for_line` remains the first thing in it worth measuring. Left as a
pointer deliberately; not chased here.

## Reproduction

```
cargo test -p legion-text --test utf16_offset_to_line_equivalence
cargo test -p legion-editor --test utf16_completion_position
cargo test -p legion-editor --release --test completion_depth_scaling -- --ignored --nocapture
```
