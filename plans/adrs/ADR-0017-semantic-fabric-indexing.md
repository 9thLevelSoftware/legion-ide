# ADR-0017: Semantic Fabric Indexing

## Status

Accepted. Phase 3 implementation evidence satisfies this ADR in [`predictive-semantic-fabric.md`](../evidence/phase-3/predictive-semantic-fabric.md:1).

## Context

Phase 3 of [`implementation-plan.md`](../implementation-plan.md:246) activates [`lib.rs`](../../crates/legion-index/src/lib.rs:1) from a placeholder into the predictive semantic fabric for navigation, completion ranking, AI context selection, refactoring previews, and test impact analysis.

This activation depends on the Phase 1 streaming text substrate in [`ADR-0015-streaming-text-viewport.md`](ADR-0015-streaming-text-viewport.md:1) and the Phase 2 proposal substrate in [`ADR-0016-generalized-proposal-service.md`](ADR-0016-generalized-proposal-service.md:1). The index must consume text snapshots, file identities, and event metadata without reintroducing full-source UI projection or direct workspace mutation.

The dependency boundary for Phase 3 is intentionally narrow: [`Cargo.toml`](../../crates/legion-index/Cargo.toml:1) can use shared DTOs and ports from [`Cargo.toml`](../../crates/legion-protocol/Cargo.toml:1), persisted index metadata from [`Cargo.toml`](../../crates/legion-storage/Cargo.toml:1), and text snapshot or chunk primitives from [`Cargo.toml`](../../crates/legion-text/Cargo.toml:1). It must not gain direct authority over editor sessions, UI state, app orchestration, or workspace VFS writes.

## Decision

Phase 3 will implement semantic indexing as an actor-owned, bounded, cancellable fabric. The index owns semantic cache state and derived metadata only. It never owns editor buffers, UI projections, save orchestration, or workspace write authority.

### 1. Actor-owned indexing boundary

- The indexing engine is an actor-owned service behind protocol contracts, not an app helper or UI-owned cache.
- Inputs are immutable snapshot descriptors, content hashes, chunk leases, file identities, discovery deltas, LSP DTOs, and metadata-only event envelopes.
- Outputs are index records, semantic query responses, diagnostics links, graph records, cache freshness markers, and metadata-only observability events.
- Indexing may request bounded text leases from the text substrate, but it must not mutate editor buffers, workspace files, or UI projections.
- Any mutation suggested by semantic analysis, refactoring preview, LSP rename, formatting, or code actions must route through [`WorkspaceProposal`](../../crates/legion-protocol/src/lib.rs:1343) and the generalized proposal lifecycle accepted in [`ADR-0016-generalized-proposal-service.md`](ADR-0016-generalized-proposal-service.md:15).

### 2. Bounded queues, priority scheduling, cancellation, and backpressure

- Index work enters bounded queues with explicit admission results: accepted, coalesced, downgraded, rejected, or resync-required.
- Priority scheduling favors live editor snapshots and visible navigation requests over repository scans. A representative priority order is current open-buffer edit, visible viewport or symbol query, save-adjacent invalidation, LSP response fusion, watcher delta, shallow repository discovery, and full background rescan.
- Every parse, scan, graph extraction, LSP fusion, ranking, and cache refresh job carries a cancellation token tied to snapshot identity, content hash, workspace generation, privacy scope, shutdown state, and user cancellation where applicable.
- Obsolete jobs are cancelled when a newer snapshot supersedes them, when their content hash no longer matches, when grammar or model versions change, when privacy scope is reduced, or when queue pressure requires stale work to yield.
- Backpressure must not propagate to editor input or save workflows. Under pressure, the fabric coalesces older file deltas, drops stale repository scan work, returns stale-but-marked query results, requests resynchronization, or pauses low-priority background indexing.
- Lossless consumers that cannot tolerate gaps must resynchronize from the newest snapshot descriptor rather than requiring editor transactions to wait.

### 3. Repository discovery, ignore handling, and file fingerprints

- Repository discovery is metadata-first. The fabric indexes file identities, canonical paths, language identifiers, workspace roots, ownership labels, file kinds, generated or binary flags, and trust or privacy labels without directly owning the workspace actor.
- Ignore handling follows workspace-discovered ignore decisions and policy metadata. Ignored, generated, binary, vendored, or oversized files are either excluded or indexed as metadata-only according to policy.
- File fingerprints separate persistence authority from cache identity. Workspace fingerprints and content versions remain save preconditions owned by the workspace path, while index cache keys use content hashes, snapshot identifiers, file content versions, grammar versions, model versions, and privacy scopes.
- File identity changes, rename-like metadata, watcher deltas, and external overwrite signals invalidate affected records before new records become query-authoritative.

### 4. Lexical symbol maps

- The first active layer is a shallow lexical map keyed by workspace, file identity, language, content hash, snapshot identifier, and privacy scope.
- Lexical records contain symbol text hashes or bounded identifiers, symbol kind where known, byte and UTF-16 ranges, declaration or reference hints, and freshness metadata.
- Lexical maps support fast symbol-file lookup and degraded query results before tree-sitter or LSP enrichment completes.
- Lexical extraction must observe ignore policy, file size budgets, queue budgets, and cancellation.

### 5. Tree-sitter parsing workers and syntax caches

- Tree-sitter parsing runs in bounded worker pools supervised by the index actor.
- A parse request is keyed by language, content hash, grammar version, snapshot identifier, changed chunk set, and privacy scope.
- Syntax caches are keyed primarily by content hash and grammar version, with privacy scope, language, parser options, and schema version included in freshness metadata.
- Cached syntax trees are reusable only when the exact content hash and grammar version match. Grammar upgrades invalidate the affected language cache even when source content is unchanged.
- Workers emit parse status records, syntax diagnostics, changed ranges, and extraction-ready tree handles. They do not emit full source text into durable storage or observability by default.
- Parse worker crashes or timeouts mark the affected file stale and reschedule according to bounded retry policy without blocking editor input.

### 6. Normalized graph records

- The semantic graph stores normalized records for symbols, references, imports, exports, call edges, type relationships, test links, diagnostics links, ownership metadata, and source provenance.
- Records carry stable identifiers when available plus content hash, snapshot identifier, grammar version, model version where relevant, privacy scope, extraction version, confidence, and stale markers.
- LSP diagnostics and semantic data can enrich the graph, but graph authority remains versioned and invalidatable by the same cache keys as parser and lexical outputs.
- The graph stores metadata and bounded excerpts only when policy allows. It must not persist full source snapshots by default.

### 7. Query APIs

- Query APIs are low-latency protocol-facing operations for UI navigation, symbol lookup, completion ranking, hover enrichment, definition and reference lookup, AI context selection, agent planning, test impact analysis, and refactoring previews.
- Query results include freshness, source snapshot identity, content hash, privacy scope, and degradation status so consumers can explain stale or partial answers.
- UI consumers receive projection-safe metadata and bounded previews only. They must not receive editor session ownership or direct text ownership through the index API.
- Queries may use stale cached results when explicitly marked stale, but must not claim freshness across content hash, grammar version, model version, or privacy-scope mismatch.

### 8. Invalidation model

- Every index record is invalidated by content hash mismatch, grammar version mismatch, model version mismatch, privacy scope mismatch, file identity replacement, workspace generation changes that affect the file, or cache schema upgrades.
- Content-hash invalidation controls lexical, syntax, graph, and query-cache freshness.
- Grammar-version invalidation controls parser outputs and graph relationships derived from parser outputs.
- Model-version invalidation is reserved for ranking, summarization, embedding, or learned classification outputs and is present as a cache key before vector indexing is enabled.
- Privacy-scope invalidation removes or downgrades records whose collection scope no longer permits storage or query exposure.
- Invalidated records become unavailable or stale before replacement records are published as authoritative.

### 9. Explicit vector-index deferral

- Vector indexing, embeddings, semantic chunk retrieval, model-backed summaries, and vector-store dependencies are deferred.
- Phase 3 may carry model version and privacy scope fields for future invalidation, but it must not compute or persist embeddings.
- Vector activation requires a later accepted ADR covering syntax-aware chunking, provenance, privacy scope, model identity, invalidation contracts, storage retention, contract tests, and dependency policy changes.
- Until that later decision is accepted, semantic fabric query APIs must rely on lexical, syntax, graph, LSP, and metadata ranking only.

### 10. Non-blocking editor and save constraints

- Semantic work must never block editor keystrokes, viewport rendering, or save workflows.
- Save preconditions, external overwrite conflict handling, capability checks, and fail-closed write semantics remain owned by the existing proposal-mediated save path.
- The fabric may observe save outcomes and file identity changes as metadata, but it cannot force a save, rewrite disk, or repair conflicts directly.
- If index freshness is unavailable during save, save proceeds or fails according to workspace/proposal rules rather than waiting for semantic work.

## Consequences

- **Positive**: Phase 3 can activate [`lib.rs`](../../crates/legion-index/src/lib.rs:1) without giving it app, UI, editor, or workspace mutation authority.
- **Positive**: Live editor work has deterministic priority over repository scans and stale semantic jobs.
- **Positive**: Cache invalidation keys are explicit before LSP fusion, learned ranking, or future vector features depend on them.
- **Negative**: Query APIs must expose freshness and degradation state because the fabric intentionally prefers responsiveness over blocking for perfect completeness.
- **Negative**: Vector retrieval remains unavailable until a later ADR and policy gate accepts model, privacy, provenance, and invalidation contracts.

## Non-goals

- This ADR does not implement Rust source, manifests, tests, build scripts, or runtime behavior.
- This ADR does not activate vector indexing or embeddings.
- This ADR does not create direct dependencies on [`Cargo.toml`](../../crates/legion-editor/Cargo.toml:1), [`Cargo.toml`](../../crates/legion-project/Cargo.toml:1), [`Cargo.toml`](../../crates/legion-app/Cargo.toml:1), or UI internals.
- This ADR does not weaken the projection-only UI or proposal-mediated save constraints.

## Exit condition

This ADR is satisfied by the Phase 3 implementation evidence in [`predictive-semantic-fabric.md`](../evidence/phase-3/predictive-semantic-fabric.md:1), which demonstrates bounded actor-owned indexing, priority scheduling, cancellation, backpressure, repository discovery, ignore handling, fingerprints, lexical maps, syntax-cache freshness, normalized graph records, low-latency query APIs, invalidation by content hash, grammar version, model version, and privacy scope, plus documented vector-index deferral.
