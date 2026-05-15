# Phase 3 Predictive Semantic Fabric Evidence Scaffold

Date: 2026-05-15

## Scope

This scaffold records the required evidence for completing Phase 3 of [`implementation-plan.md`](../../implementation-plan.md:246). Phase 3 activates [`lib.rs`](../../../crates/devil-index/src/lib.rs:1) as the semantic fabric and introduces supervised LSP fusion only after governance gates are accepted.

This document is not implementation evidence yet. It defines the workflows, validation commands, artifacts, and final checklist that implementation subtasks must complete.

## Governance prerequisites

- [`ADR-0017-semantic-fabric-indexing.md`](../../adrs/ADR-0017-semantic-fabric-indexing.md:1) is accepted before semantic indexing runtime behavior lands.
- [`ADR-0018-lsp-runtime-supervision.md`](../../adrs/ADR-0018-lsp-runtime-supervision.md:1) is accepted before LSP runtime supervision or LSP mutation routing lands.
- [`dependency-policy.md`](../../dependency-policy.md:88) explicitly limits Phase 3 [`Cargo.toml`](../../../crates/devil-index/Cargo.toml:1) activation to [`Cargo.toml`](../../../crates/devil-protocol/Cargo.toml:1), [`Cargo.toml`](../../../crates/devil-storage/Cargo.toml:1), and [`Cargo.toml`](../../../crates/devil-text/Cargo.toml:1).
- Placeholder crates and planned runtime surfaces remain inert unless they have an accepted ADR, dependency-policy entry, phase gate, protocol contracts, contract tests, ownership tests, and evidence.

## Phase 3 requirements

- Implement actor-owned indexing with bounded queues, priority scheduling, cancellation tokens, and backpressure.
- Add repository discovery, ignore handling, file fingerprints, lexical symbol maps, and symbol-file lookup.
- Add tree-sitter parsing workers with syntax caches keyed by content hash and grammar version.
- Extract normalized graph records for symbols, references, imports, exports, call edges, type relationships, test links, diagnostics links, and ownership metadata.
- Integrate LSP diagnostics, completion, hover, definition, reference, rename, formatting, semantic-token, and code-action flows through protocol DTOs.
- Add low-latency query APIs for UI navigation, completion ranking, AI context selection, agent planning, test impact, and refactoring previews.
- Invalidate records by content hash, grammar version, model version, and privacy scope.
- Preserve projection-only UI: UI receives semantic projections and command intents only, never editor session ownership or text ownership.
- Preserve proposal-mediated saves and mutations: semantic indexing and LSP workers cannot mutate buffers or workspace files directly.
- Ensure semantic and LSP work cannot block editor input, viewport projection, proposal validation, or save workflows.
- Defer vector indexing until syntax-aware chunking, provenance, privacy scope, model identity, invalidation contracts, storage retention, contract tests, and dependency policy changes are accepted.

## Workflows to validate

### 1. Repository discovery to lexical map

1. Receive workspace discovery metadata, file identities, ignore decisions, trust state, and privacy labels through protocol-facing contracts.
2. Exclude or downgrade ignored, generated, binary, vendored, oversized, or privacy-restricted files.
3. Compute or consume content hashes and workspace fingerprints as separate concepts.
4. Populate lexical symbol maps keyed by file identity, language, content hash, snapshot identity, and privacy scope.
5. Publish freshness and degradation state without emitting full source text by default.

### 2. Open-buffer priority and supersession

1. Receive editor transaction metadata with snapshot identifiers, buffer versions, changed ranges, chunk hashes, and causality.
2. Prioritize open-buffer incremental work over background repository scans.
3. Cancel obsolete parse, graph, LSP, ranking, or query-refresh work when a newer snapshot supersedes it.
4. Return stale-marked or degraded query answers when fresh work is still pending.
5. Prove editor input and save workflows continue without waiting for semantic completion.

### 3. Tree-sitter parse and graph extraction

1. Schedule parse workers through bounded queues and cancellation tokens.
2. Reuse syntax cache entries only when content hash and grammar version match.
3. Invalidate syntax and graph records on content hash mismatch, grammar version change, privacy-scope change, schema upgrade, or file identity replacement.
4. Extract normalized graph records with provenance, confidence, freshness, and privacy metadata.
5. Persist metadata-only graph state according to storage and redaction policy.

### 4. LSP fusion

1. Launch or connect supervised LSP workers only through accepted policy and trust gates.
2. Send cancellable requests tied to snapshot identity, buffer version, content hash, timeout budget, correlation, and causality.
3. Convert diagnostics, completion, hover, definition, reference, semantic-token, formatting, rename, and code-action results into protocol DTOs.
4. Suppress stale responses and publish timeout or degraded status for slow or unavailable servers.
5. Feed versioned LSP enrichment into the semantic graph without overwriting newer lexical or syntax records.

### 5. Proposal-mediated semantic mutations

1. Translate formatting, rename, organize-imports, quick-fix, and refactor outputs into proposal payloads.
2. Include target coverage, version preconditions, capability requirements, rollback metadata, privacy metadata, correlation, and causality.
3. Validate and preview through the proposal service before any edit or workspace write.
4. Reject unsupported, stale, conflicting, or partial edits without changing editor buffers or disk.
5. Preserve dirty editor text and existing save conflict behavior.

### 6. Vector-index deferral

1. Confirm no embedding generation, vector database, model-provider dependency, or semantic chunk retrieval is activated in Phase 3.
2. Confirm cache keys may record model version only for future invalidation compatibility.
3. Record a follow-on ADR requirement for syntax-aware chunking, provenance, privacy scope, model identity, invalidation contracts, storage retention, contract tests, and dependency policy changes.

## Target validation commands

Record command output artifacts under [`phase-3`](./) before final acceptance.

| Gate | Command | Expected artifact |
| --- | --- | --- |
| Dependency policy | `cargo run -p xtask -- check-deps` | `check-deps.txt` showing policy pass and no unauthorized [`Cargo.toml`](../../../crates/devil-index/Cargo.toml:1) dependencies. |
| Formatting | `cargo fmt --all --check` | `cargo-fmt-check.txt` showing formatted workspace. |
| Workspace check | `cargo check --workspace --all-targets` | `cargo-check-workspace-all-targets.txt` showing successful compile. |
| Workspace tests | `cargo test --workspace --all-targets` | `cargo-test-workspace-all-targets.txt` showing all required tests pass. |
| Workspace clippy | `cargo clippy --workspace --all-targets -- -D warnings` | `cargo-clippy-workspace-all-targets.txt` showing zero warnings. |
| Index contract tests | `cargo test -p devil-index --all-targets` | `devil-index-tests.txt` covering scheduler, cancellation, backpressure, lexical maps, parser caches, graph records, and invalidation. |
| Protocol DTO contracts | `cargo test -p devil-protocol --test dto_contracts` | `devil-protocol-dto-contracts.txt` covering semantic and LSP DTO round trips. |
| Save regression | `cargo test -p devil-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict` | `save-conflict-regression.txt` showing proposal-mediated save conflicts remain protected. |
| Editor latency and background work | `cargo test -p devil-editor --test performance_suite -- --list` plus Phase 3 performance runs | `editor-semantic-latency.txt` showing semantic work does not block input-oriented paths. |

## Expected evidence artifacts

- `semantic-fabric-architecture-map.md`: actor ownership, queues, priorities, cancellation, backpressure, cache keys, and invalidation paths.
- `index-dependency-boundary.txt`: dependency-policy output proving only permitted internal dependencies for [`Cargo.toml`](../../../crates/devil-index/Cargo.toml:1).
- `repository-discovery-ignore-fingerprint.md`: discovery and ignore handling cases with file fingerprints and content hashes distinguished.
- `lexical-symbol-map-tests.txt`: lexical extraction, symbol-file lookup, ignored-file behavior, and stale-work cancellation results.
- `tree-sitter-cache-tests.txt`: content-hash and grammar-version cache reuse and invalidation results.
- `normalized-graph-contract-tests.txt`: symbols, references, imports, exports, call edges, type relationships, test links, diagnostics links, and ownership metadata validation.
- `semantic-query-api-tests.txt`: UI navigation, completion ranking, AI context selection, agent planning, test impact, and refactoring-preview query behavior.
- `lsp-supervision-tests.txt`: cancellation, backpressure, timeout, stale response suppression, diagnostics, completion, hover, definition, reference, rename, formatting, and code-action DTO flow.
- `proposal-routing-regression.txt`: proof that LSP and semantic mutation outputs route through proposals and cannot directly mutate buffers or workspaces.
- `privacy-redaction-audit.md`: evidence that events, caches, and persisted records default to metadata-only retention and honor privacy scope invalidation.
- `vector-deferral-audit.md`: proof that vector indexing, embeddings, vector storage, and model-backed retrieval remain inactive.

## Final validation checklist

- [ ] [`ADR-0017-semantic-fabric-indexing.md`](../../adrs/ADR-0017-semantic-fabric-indexing.md:1) accepted and cited by implementation evidence.
- [ ] [`ADR-0018-lsp-runtime-supervision.md`](../../adrs/ADR-0018-lsp-runtime-supervision.md:1) accepted and cited by implementation evidence.
- [ ] [`dependency-policy.md`](../../dependency-policy.md:88) permits only [`Cargo.toml`](../../../crates/devil-protocol/Cargo.toml:1), [`Cargo.toml`](../../../crates/devil-storage/Cargo.toml:1), and [`Cargo.toml`](../../../crates/devil-text/Cargo.toml:1) as Phase 3 internal dependencies for [`Cargo.toml`](../../../crates/devil-index/Cargo.toml:1).
- [ ] [`dependency-policy.md`](../../dependency-policy.md:52) still forbids [`Cargo.toml`](../../../crates/devil-editor/Cargo.toml:1) from depending on [`Cargo.toml`](../../../crates/devil-project/Cargo.toml:1).
- [ ] Actor-owned index queues are bounded and have observable accepted, coalesced, cancelled, rejected, degraded, and resync-required outcomes.
- [ ] Live editor snapshots supersede background scans and cancel obsolete parse, LSP, ranking, and graph work.
- [ ] Repository discovery honors ignore, generated, binary, oversized, trust, and privacy decisions.
- [ ] File fingerprints, content hashes, snapshot identifiers, file content versions, and workspace generations are used for their distinct authorities.
- [ ] Lexical symbol maps and symbol-file lookup work before tree-sitter enrichment is available.
- [ ] Tree-sitter syntax caches are keyed by content hash and grammar version and invalidate on either mismatch.
- [ ] Normalized graph records include symbols, references, imports, exports, call edges, type relationships, test links, diagnostics links, ownership metadata, provenance, freshness, and privacy scope.
- [ ] Query APIs return freshness and degradation metadata for UI navigation, completion ranking, AI context selection, agent planning, test impact, and refactoring previews.
- [ ] LSP operations are cancellable, supervised, bounded, timeout-aware, and stale-response-safe.
- [ ] Diagnostics, completion, hover, definition, reference, rename, formatting, semantic-token, and code-action flows use protocol DTOs.
- [ ] Formatting, rename, code actions, and semantic refactor suggestions route through proposals and cannot directly edit buffers or workspace files.
- [ ] UI remains projection-only and does not own editor sessions, text buffers, index state, LSP workers, or workspace VFS authority.
- [ ] Semantic and LSP work does not block editor input, viewport projection, proposal validation, or save workflows.
- [ ] Existing proposal-mediated save conflict and dirty-buffer preservation tests remain green.
- [ ] Metadata-only redaction remains the default for caches, observability, audit, and evidence.
- [ ] Vector indexing, embeddings, model-backed retrieval, and vector storage remain deferred and inactive.
