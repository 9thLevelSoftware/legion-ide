# UI, Editor & Collaboration Review

Scope reviewed:
- `crates/legion-ui/src/lib.rs`
- `crates/legion-ui/src/projection.rs`
- `crates/legion-ui/src/ui.rs`
- `crates/legion-editor/src/lib.rs`
- `crates/legion-text/src/lib.rs`
- `crates/legion-collaboration/src/lib.rs`

Summary: 19 findings (critical: 1, high: 7, medium: 9, low: 2).

## crates/legion-ui/src/lib.rs

No findings. This file only re-exports projection/UI symbols.

## crates/legion-ui/src/projection.rs

### Finding 1
- Category: bug
- Severity: medium
- Line numbers: 202-207, 234, 240-261
- Description: `legion_workflow_fleet_card_projections` computes one aggregate `test_status_label` from the entire `VerificationRunProjection` and applies the same label to every proposal card. If multiple proposals are present, each card shows the global verification totals rather than the verification state for that proposal, making failed or blocked tests appear on unrelated cards.
- Suggested fix direction: Join verification rows to their owning proposal/workflow before projecting card status, or clearly label the field as a global aggregate outside per-proposal card data.

## crates/legion-ui/src/ui.rs

### Finding 2
- Category: failure-point
- Severity: high
- Line numbers: 3383-3427, 3457-3914
- Description: `Shell::render` writes active file text, viewport text, paths, command labels, terminal rows, debug values, and other projection strings directly to stdout with `println!` and no terminal escaping. A malicious file or remote/tool/terminal output containing ANSI escape sequences can clear the screen, spoof prompts, hide text, or manipulate terminal state.
- Suggested fix direction: Escape or sanitize control characters before rendering untrusted content to a terminal. Keep raw text only for renderer backends that explicitly support safe rich text, and add tests for ANSI/control-character payloads.

### Finding 3
- Category: bug
- Severity: high
- Line numbers: 4802-4810
- Description: `parse_pos` converts byte offsets in viewport mode by accumulating `visible_text.len() + 1` from the first visible slice and then returns `byte_offset: Some(byte_offset as u64)`. This treats the offset as relative to the visible viewport, ignoring each slice's absolute `byte_range.start`. Commands emitted from degraded/streaming viewport projections can therefore target the wrong absolute document byte offset.
- Suggested fix direction: Resolve viewport positions from `ViewportLineSlice.byte_range.start` plus a validated in-slice offset, and reject offsets outside the visible slice instead of fabricating document offsets from viewport-relative counters.

### Finding 4
- Category: failure-point
- Severity: medium
- Line numbers: 4789-4799
- Description: In small-buffer mode, `parse_pos` uses `text.as_bytes().get(..byte_offset)`. If the requested byte offset lands inside a UTF-8 codepoint or past the end, the command silently falls back to `(0,0)` with byte offset 0. A malformed command or cursor offset can turn an intended local edit into an edit at the beginning of the file.
- Suggested fix direction: Validate `byte_offset <= text.len()` and `text.is_char_boundary(byte_offset)`. Return a command error for invalid offsets rather than coercing to the file start.

### Finding 5
- Category: failure-point
- Severity: medium
- Line numbers: 4571-4613, 5153-5157
- Description: Proposal command parsing accepts `ProposalId(0)` because `parse_proposal_id` does not reject zero. Other parsers in the same file reject zero identifiers, and the empty approval/checkpoint projections use proposal id 0 as a sentinel. This allows commands such as `:proposal-approve 0` to emit privileged proposal intents for an invalid/sentinel proposal.
- Suggested fix direction: Filter parsed proposal IDs with `id != 0` and add tests that malformed or zero proposal IDs emit `Noop` or an explicit validation error.

### Finding 6
- Category: bug
- Severity: low
- Line numbers: 2942-2969, 3030-3043
- Description: Toast IDs are derived only from severity and message text. Duplicate status messages receive identical IDs, so dismissing one dismisses every duplicate, and multiple visible duplicates cannot be independently addressed.
- Suggested fix direction: Include a monotonic sequence, timestamp, source id, or caller-supplied notification id in `ToastProjection::id` while preserving deterministic IDs only where deduplication is explicitly desired.

## crates/legion-editor/src/lib.rs

### Finding 7
- Category: bug
- Severity: high
- Line numbers: 1141-1153
- Description: Batch edit deltas are computed while edits are applied in descending byte order. A later lower-offset edit can shift the post-edit coordinates of an earlier higher-offset delta, but the higher-offset delta was already recorded before that shift. After `deltas.reverse()`, transaction metadata can report stale byte/UTF-16 ranges for multi-edit batches where lower edits change length.
- Suggested fix direction: Compute all changed ranges against the final staged buffer after all edits are applied, or adjust already-recorded higher-offset deltas by the length delta of lower-offset edits before recording the transaction.

### Finding 8
- Category: failure-point
- Severity: medium
- Line numbers: 1871-1896, 1002-1037
- Description: `set_cursors` and `set_selections` accept arbitrary `TextPosition`/`TextRange` values without validating them against the buffer. The invalid state is stored and later causes `viewport_projection` to fail when it tries to convert cursor/selection positions to byte offsets.
- Suggested fix direction: Validate cursor and selection positions with `buffer.try_byte_offset` when setting them, and reject invalid ranges immediately so projection generation remains infallible for stored editor state.

### Finding 9
- Category: bug
- Severity: high
- Line numbers: 2373-2388
- Description: `EditorRequest::ApplyTransaction` validates that a transaction descriptor matches the buffer identity and then returns it as `EditorResponse::Transaction`, but it does not mutate the buffer, append the transaction log, or emit an event. Callers can receive a successful transaction response for a transaction that was never applied.
- Suggested fix direction: Either remove/rename this request if it is only an acknowledgement path, or route it through real editor mutation logic with edits/preconditions and transaction/event recording. At minimum, return `unsupported` instead of a success response when no mutation occurs.

### Finding 10
- Category: bug
- Severity: medium
- Line numbers: 2428-2431, 1899-1910
- Description: `EditorRequest::Overlay` returns `OverlayApplied` but never stores the overlay in any buffer state. `EditorEngine` has `set_overlays`, but the protocol request path is a no-op success, so UI/diagnostic overlays can be dropped silently.
- Suggested fix direction: Add buffer identity to the overlay request or resolve it from overlay metadata, then update the target buffer's overlay list. If overlays are intentionally projection-only, return an explicit unsupported/no-op response instead of `OverlayApplied`.

### Finding 11
- Category: failure-point
- Severity: medium
- Line numbers: 73-77, 1047-1060, 1674-1683
- Description: Degraded-save behavior is inconsistent and potentially unbounded. `EditorError::DegradedSaveUnavailable` and the viewport large-file message say degraded saves fail closed, but `request_save` materializes the entire degraded snapshot into a `String` via `materialize_full_text_from_chunks`. Very large files can therefore allocate whole-file payloads on the interactive path despite the degraded-mode budget.
- Suggested fix direction: Decide on one contract. If degraded saves should be supported, stream chunks through the save path without building a single `String` and update the status/error text. If they should fail closed, return `DegradedSaveUnavailable` before materialization.

## crates/legion-text/src/lib.rs

### Finding 12
- Category: failure-point
- Severity: high
- Line numbers: 806-823
- Description: `LineIndex::visible_line_slices` creates `Vec::with_capacity(end_line_exclusive.saturating_sub(start_line))` before checking that `end_line_exclusive` is within the document. A caller can request a huge end line and force a massive allocation or abort before the first out-of-bounds line check.
- Suggested fix direction: Validate `end_line_exclusive <= line_count()` and cap the requested line count before allocating. Consider returning `LineOutOfBounds` or a bounded-limit error for oversized viewport requests.

### Finding 13
- Category: failure-point
- Severity: medium
- Line numbers: 730-742, 1257-1299
- Description: The simple-edit fast path shifts chunk boundaries and updates only the containing chunk hash, but it never rebalances/splits chunks if repeated same-line edits make a chunk exceed `DEFAULT_CHUNK_FORCE_MAX_BYTES`. Over time, chunk descriptors can become much larger than the advertised bound, causing chunk reads and save materialization to lose their bounded-payload guarantee.
- Suggested fix direction: After simple edits, check the edited chunk's size against the force maximum and fall back to `rebuild_from_chunk` when it exceeds the chunk budget or crosses other chunking invariants.

### Finding 14
- Category: failure-point
- Severity: low
- Line numbers: 345-355, 1099-1108
- Description: Public convenience methods `TextSnapshot::text()` and `TextBuffer::text()` panic whenever the full text cache is unavailable. This is documented, but these methods remain easy to call from production paths and can crash on large/degraded buffers.
- Suggested fix direction: Prefer fallible APIs in production-facing code paths, consider gating the panicking methods behind test/compatibility naming, or rename them to make the panic behavior explicit.

## crates/legion-collaboration/src/lib.rs

### Finding 15
- Category: error
- Severity: critical
- Line numbers: 419-435, 558-620
- Description: `handle_transport_envelope` validates that `envelope.sender_participant_id` is nonzero, but it never checks that the sender matches the participant encoded in the payload. An envelope from one participant can carry an operation authored by another admitted participant, and `validate_operation_shape` then authorizes based on the payload's `author_participant_id` rather than the transport sender.
- Suggested fix direction: Require `envelope.sender_participant_id == operation.author_participant_id` for operation payloads and `== projection.participant_id` for presence payloads before dispatching. Add negative tests for sender/payload mismatches.

### Finding 16
- Category: bug
- Severity: medium
- Line numbers: 148-160
- Description: Participant initialization inserts participants into a `HashMap` keyed by participant id without rejecting duplicates. A duplicate participant id silently overwrites the earlier descriptor, including principal and permissions, while the initial bounds check used the pre-deduplicated vector length.
- Suggested fix direction: Reject duplicate participant IDs during session construction and validate that the deduplicated participant count equals the submitted count.

### Finding 17
- Category: bug
- Severity: high
- Line numbers: 267-306, 739-773
- Description: Submission only detects gaps in the author's `participant_sequence`. Cross-participant dependencies declared in `operation.preconditions.base_vector` are not checked against accepted participant sequences before accepting the operation. `deterministic_order` only orders dependencies that are already present in the accepted operation list, so an operation can be accepted even when it causally depends on another participant's missing sequence.
- Suggested fix direction: Validate every base-vector entry before acceptance: the runtime's observed sequence for that participant must be at least the requested sequence, otherwise emit a causal gap/resync acknowledgement rather than applying the operation.

### Finding 18
- Category: bug
- Severity: high
- Line numbers: 721-789, 794-843
- Description: Concurrent text operations are replayed in deterministic order using their original byte ranges, but ranges are never transformed against prior concurrent inserts/deletes/replacements. Concurrent operations based on the same initial document can therefore delete/replace the wrong bytes or conflict depending on deterministic ordering, even though all replicas converge on the same incorrect text.
- Suggested fix direction: Add an operational-transform/CRDT range transform step, or constrain accepted operations to a strictly linear base version/vector where raw byte ranges are valid. Conflict and resync when a concurrent range cannot be transformed safely.

### Finding 19
- Category: bug
- Severity: medium
- Line numbers: 318-323, 651-676
- Description: For accepted operations, `acknowledge` is called before `participant_sequences` is updated with the accepted sequence. The `observed_vector` in an Accepted acknowledgement therefore omits the operation being acknowledged, which can make clients believe the server has not observed their latest sequence.
- Suggested fix direction: Update the participant sequence before constructing Accepted acknowledgements, or have `acknowledge` accept an overlay vector that includes the just-accepted operation.
