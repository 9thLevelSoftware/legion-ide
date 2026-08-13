# ADR-0015: Streaming Text, Chunked Snapshots, and Viewport Projection

## Status
Accepted for Phase 1 Workstream 0.

## Context

Phase 1 of [`plans/implementation-plan.md`](plans/implementation-plan.md:76) removes full-buffer materialization from the editor and text substrate. The current scalability risk is that [`ActiveBufferProjection`](crates/legion-ui/src/ui.rs:86) can carry full text, [`DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES`](crates/legion-text/src/lib.rs:22) bounds full-cache behavior, and [`LineIndex`](crates/legion-text/src/lib.rs:457) is still treated as full-buffer infrastructure. The Phase 1 acceptance gate in [`plans/implementation-plan.md`](plans/implementation-plan.md:98) requires very large files to open without sending full source text to UI projections and requires UI full-source access only in explicitly bounded small-buffer mode.

This ADR extends the text ownership direction accepted in [`plans/adrs/ADR-0003-editor-core-text-model.md`](plans/adrs/ADR-0003-editor-core-text-model.md:1): the editor/text layer owns text buffers, snapshots, transactions, and line metrics; UI remains projection-only. The UI shell represented by [`Shell`](crates/legion-ui/src/ui.rs:228) must not regain editor buffer, workspace actor, or persistence ownership.

The existing proposal-mediated save path remains authoritative for Phase 1. Saves must continue through [`SaveWorkflowService::save_active_buffer()`](crates/legion-app/src/lib.rs:938) and [`WorkspaceActor::save_file_with_proposal()`](crates/legion-project/src/lib.rs:1620), preserving the conflict behavior covered by [`workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict()`](crates/legion-app/tests/workspace_vfs_integration.rs:280). Future placeholder subsystems in [`crates/legion-index/src/lib.rs`](crates/legion-index/src/lib.rs:1), [`crates/legion-agent/src/lib.rs`](crates/legion-agent/src/lib.rs:1), [`crates/legion-tracker/src/lib.rs`](crates/legion-tracker/src/lib.rs:1), and [`crates/legion-memory/src/lib.rs`](crates/legion-memory/src/lib.rs:1) remain inert in this workstream.

## Decision

Phase 1 adopts a streaming editor substrate with two explicit operating modes, viewport-only UI projection by default, chunked immutable snapshot descriptors, leased snapshot access, incremental line metrics, and non-blocking transaction event fanout. This ADR is a guardrail for Phase 1 only and does not generalize the proposal model or introduce new runtime subsystems.

### 1. Small-buffer full-source mode versus large-file degraded mode

- **Small-buffer full-source mode** is allowed only when the buffer is within the existing full-cache budget represented by [`DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES`](crates/legion-text/src/lib.rs:22), the buffer has not been promoted to degraded mode for memory or encoding reasons, and the projection request explicitly asks for bounded full-source compatibility.
- In small-buffer full-source mode, UI may receive full source text as a compatibility projection, but the projection must be explicitly tagged as bounded and must carry the snapshot identifier and buffer version that produced it.
- **Large-file degraded mode** is required for buffers above [`DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES`](crates/legion-text/src/lib.rs:22) and may also be selected for files that exceed configured memory budgets, line metric budgets, or semantic overlay budgets.
- In large-file degraded mode, UI receives viewport slices only. Whole-file source text, all-line materialization, unbounded search results, full semantic overlays, and eager full line metrics are forbidden.
- Degraded mode must surface user-visible status: viewport-only rendering, bounded search, disabled or stale expensive overlays, and any save limitation caused by current Phase 1 compatibility constraints.
- The accepted Phase 1 guard is absolute: UI never receives full source except in explicitly bounded small-buffer mode.

### 2. Viewport projection shape

The editor/app boundary provides a viewport projection rather than a full active-buffer projection. The projection shape is defined conceptually for Phase 1 as:

- identity: workspace identifier, file identifier, buffer identifier, snapshot identifier, buffer version, and a viewport request identifier;
- mode metadata: small-buffer full-source, normal streaming, or large-file degraded;
- viewport bounds: requested anchor, visible line range, byte range coverage, overscan line count, and whether the projection is complete or clipped;
- visible line slices: line number, stable line key for the snapshot, byte range, UTF-8 byte length, UTF-16 length when known, text slice for that line or segment, end-of-line marker, and truncation marker when a single line exceeds the viewport budget;
- cursor and selection projections in visible or clipped coordinates;
- decorations, diagnostics, fold ranges, semantic token overlays, and inlay surfaces only for covered visible ranges;
- lazy line metrics with explicit freshness state so stale or background-computed metrics never block rendering;
- degraded-mode status and omissions so UI can explain disabled features without owning policy or text.

Projection consumers may request a new viewport, scroll anchor, or command dispatch intent, but projection consumers do not mutate text, own editor sessions, own workspace actors, or persist files. [`Shell`](crates/legion-ui/src/ui.rs:228) remains a projection renderer and command-intent surface.

### 3. Chunk size and chunk hash policy

- Snapshot chunks target 64 KiB of UTF-8 source bytes.
- Chunk boundaries prefer line endings within an 8 KiB adjustment window around the target size to keep viewport and line metric work localized.
- A chunk must never split an invalid UTF-8 sequence; for valid text it should not split a CRLF pair. If no suitable line boundary exists, a hard chunk may extend up to 96 KiB before a boundary is forced at a valid byte boundary.
- Very long logical lines may span multiple chunks. The viewport projection must represent such lines as clipped segments with continuation metadata rather than materializing the whole line for UI.
- Each chunk descriptor records snapshot identifier, chunk ordinal, absolute byte range, byte length, line-start count, line-end count when known, and a content-addressed chunk hash.
- Chunk hashes are algorithm-tagged, domain-separated for Legion text chunks, and computed over the exact source bytes represented by the chunk. Identical byte content may share cache entries, but chunk identity still includes snapshot identifier, ordinal, and byte range.
- The snapshot content hash is derived from ordered chunk descriptors and chunk hashes so consumers can invalidate by changed chunks without receiving full source.
- Hashes are cache and invalidation keys, not security or save-precondition authority in Phase 1. Save preconditions continue to use the existing workspace fingerprint and file content version path.
- Chunk hash events and observability records may include identifiers, ranges, lengths, and hashes, but must not include full source by default.

### 4. Snapshot lease rules

- A snapshot lease grants a named consumer access to a snapshot descriptor and to bounded chunk reads for that snapshot. It does not imply full-source materialization.
- Leases are keyed by snapshot identifier, buffer identifier, buffer version, lease owner, lease reason, acquisition sequence, and expiry or retention budget.
- Viewport rendering uses short-lived viewport leases for the visible range and overscan only.
- Background consumers such as future LSP, index, plugin, AI, collaboration, storage, and observability services may lease snapshots by descriptor, but Phase 1 does not activate placeholder subsystems or create new runtime surfaces for them.
- A lease pins only the descriptors and chunks needed by the lease reason. Full-buffer pinning requires explicit small-buffer eligibility or a future ADR outside Phase 1.
- New edit transactions create newer snapshots without waiting for existing leases. Old leased chunks remain readable until release or expiry; unleased old chunks may be evicted under the retention budget.
- Consumers must validate the snapshot identifier and buffer version before using leased data. If a lease expires, the consumer reacquires from the current snapshot or reports stale work; it must not block editor input.
- Lease metadata is auditable without storing full source. Observability records retain lease owner, reason, snapshot identifier, chunk ranges, and hash metadata only.

### 5. Incremental line metric invalidation rules

- Line metrics are chunk-scoped with prefix summaries that map line numbers, byte offsets, UTF-8 offsets, and UTF-16 offsets without rebuilding the whole buffer after every edit.
- Every edit transaction records the pre-edit changed range, post-edit changed range, affected snapshot identifiers, and affected chunk ordinals.
- Applying an edit invalidates metrics for chunks intersecting the changed byte ranges plus adjacent chunks when the edit can affect newline boundaries, CRLF pairing, or long-line continuation state.
- Recalculation starts from the nearest valid prefix checkpoint before the first invalidated chunk and proceeds only through invalidated chunks until prefix summaries are consistent again.
- Viewport-visible metrics are recomputed before or with the next viewport projection. Non-visible metrics are recomputed lazily or by cancellable background work.
- Stale diagnostics, semantic overlays, fold ranges, and decorations are omitted or marked stale for invalidated ranges instead of blocking viewport rendering.
- Whole-buffer line metric rebuilds are allowed only for bounded small-buffer mode or explicit validation tooling, not for large-file degraded mode.

### 6. Non-blocking transaction event stream semantics

- [`EditorEngine`](crates/legion-editor/src/lib.rs:312) emits transaction events only after an edit transaction commits to editor-owned state and a post-transaction snapshot descriptor exists.
- Events carry metadata: transaction identifier, source kind, workspace identifier, file identifier, buffer identifier, pre-snapshot identifier, post-snapshot identifier, buffer version, changed byte ranges, changed UTF-16 ranges when available, affected chunk ordinals and hashes, undo group, causality identifier, and freshness flags.
- Events must not carry full source text. They may reference chunk descriptors and bounded changed slices only when a consumer holds an appropriate lease and the slice remains within projection or small-buffer limits.
- Event ordering is total per buffer. Cross-buffer ordering is expressed by causality metadata and must not require editor input to wait for background consumers.
- Fanout uses bounded non-blocking queues. If a consumer lags, its queue may coalesce obsolete events, mark a gap, or require resynchronization from the newest snapshot descriptor.
- Lossless consumers that cannot miss intermediate edits must stop consuming and request resynchronization when a gap is reported. They still must not backpressure keystrokes.
- Background indexing, semantic analysis, AI retrieval, collaboration replay, storage observation, and observability sinks consume the event stream asynchronously. Phase 1 defines the contract but keeps placeholder subsystems inert until their later ADR gates.
- Observability events default to metadata-only redaction and retain hashes, ranges, identifiers, and ordering metadata rather than source text.

### 7. Save-path compatibility limits for Phase 1

- Phase 1 does not replace the existing proposal-mediated save path and does not introduce generalized proposals. That belongs to later workstreams.
- Save remains routed through [`SaveWorkflowService::save_active_buffer()`](crates/legion-app/src/lib.rs:938) and [`WorkspaceActor::save_file_with_proposal()`](crates/legion-project/src/lib.rs:1620).
- The editor/app save assembly may materialize a complete save payload only inside the save workflow boundary, never inside UI projection and never as a background consumer convenience.
- Existing save preconditions remain mandatory: expected fingerprint, expected file content version, expected workspace generation, buffer version, snapshot identifier, non-zero correlation identifier, and causality identifier.
- If a degraded large-file buffer cannot be saved through the current Phase 1 full-payload compatibility path within configured budgets, the save must fail closed with a typed rejection or degraded status while preserving dirty editor text.
- External overwrite, stale fingerprint, stale content version, trust denial, or write-policy denial must continue to reject rather than clobber disk, preserving the behavior verified by [`workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict()`](crates/legion-app/tests/workspace_vfs_integration.rs:280).
- Chunk hashes are not substitutes for workspace fingerprints or file content versions in Phase 1.

## Phase 1 guardrails and non-goals

- UI full-source projection is forbidden except in explicitly bounded small-buffer mode.
- UI remains projection-only and never owns editor buffers, workspace actors, save orchestration, or persistence.
- No Rust source, tests, dependency policy, or placeholder crate behavior is changed by this ADR.
- No new dependencies are required by this ADR.
- [`crates/legion-index/src/lib.rs`](crates/legion-index/src/lib.rs:1), [`crates/legion-agent/src/lib.rs`](crates/legion-agent/src/lib.rs:1), [`crates/legion-tracker/src/lib.rs`](crates/legion-tracker/src/lib.rs:1), and [`crates/legion-memory/src/lib.rs`](crates/legion-memory/src/lib.rs:1) remain inert.
- Phase 1 does not design multi-file atomic proposals, generalized mutation approval, plugin mutation paths, LSP mutation paths, AI-generated edit application, collaboration protocols, or durable event replay storage.
- Phase 1 does not weaken existing save conflict semantics or fail-closed persistence behavior.

## Exit condition

The ADR is satisfied only when the Phase 1 implementation can demonstrate that very large files open and render through viewport projections without sending full source text to UI, edits update viewport state, line metrics, and snapshot chunks incrementally, transaction event fanout cannot block editor input, and UI receives full source text only in explicitly bounded small-buffer mode.

## Consequences

- **Positive**: The editor/text substrate can scale beyond the current full-cache budget while preserving deterministic text ownership from [`plans/adrs/ADR-0003-editor-core-text-model.md`](plans/adrs/ADR-0003-editor-core-text-model.md:1).
- **Positive**: UI remains a projection-only renderer and can explain degraded mode without owning text or persistence.
- **Positive**: Snapshot descriptors, leases, chunk hashes, and incremental line metrics create a stable substrate for later LSP, semantic, AI, collaboration, and storage consumers without activating those systems in Phase 1.
- **Positive**: Existing proposal-mediated save conflict behavior remains intact during the streaming substrate transition.
- **Negative**: Phase 1 saves may still require full-payload compatibility materialization at the save workflow boundary, so very large degraded files may need typed save limitations until later proposal and streaming-write workstreams.
- **Negative**: Consumers must tolerate stale, clipped, or omitted metadata and must implement resynchronization rather than relying on blocking event delivery.
