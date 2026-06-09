# Phase 3 Predictive Semantic Fabric Evidence

Date: 2026-05-24

## Scope

This document records accepted implementation evidence for Phase 3 of [`implementation-plan.md`](../../implementation-plan.md:246). Phase 3 activates [`legion-index`](../../../crates/legion-index/src/lib.rs:1) as the predictive semantic fabric through actor-owned bounded scheduling, workspace-authoritative discovery import, descriptor and lease-based source inputs, lexical symbol maps, syntax-cache freshness keys, normalized graph records, metadata-only semantic persistence, pure query DTOs, and LSP supervision/proposal-routing contracts.

The accepted Phase 3 runtime scope remains intentionally narrow. It does not activate embeddings, vector storage, model-backed retrieval, provider egress, terminal execution, plugin execution, remote workspaces, collaboration sessions, or direct editor/workspace mutation from semantic or LSP code.

## Acceptance status

- Runtime surface status: Accepted semantic fabric runtime is active in [`legion-index`](../../../crates/legion-index/src/lib.rs:1) with accepted protocol and storage contracts.
- Phase 3 acceptance: Accepted.
- LSP supervision acceptance: Accepted.
- ADR status note: [`ADR-0017-semantic-fabric-indexing.md`](../../adrs/ADR-0017-semantic-fabric-indexing.md:111) and [`ADR-0018-lsp-runtime-supervision.md`](../../adrs/ADR-0018-lsp-runtime-supervision.md:91) are satisfied by the implementation and evidence listed here.
- Gate behavior: [`xtask`](../../../xtask/src/main.rs:285) validates that accepted Phase 3 status keeps the required artifact list, removes scaffold disclaimers, provides all required artifact files under this directory, and leaves no unchecked final validation items.

## Governance prerequisites

- [`ADR-0017-semantic-fabric-indexing.md`](../../adrs/ADR-0017-semantic-fabric-indexing.md:1) is accepted and satisfied by the bounded `IndexingActor`, scheduler, source descriptor, cache, graph, storage, and query contracts.
- [`ADR-0018-lsp-runtime-supervision.md`](../../adrs/ADR-0018-lsp-runtime-supervision.md:1) is accepted and satisfied by cancellable LSP DTOs, metadata-only supervision events, fail-closed launch policy, stale/timeout/degraded result statuses, and proposal-only mutation conversion.
- [`dependency-policy.md`](../../dependency-policy.md:91) limits [`legion-index`](../../../crates/legion-index/Cargo.toml:1) to the approved internal Phase 3 dependencies: `legion-protocol`, `legion-storage`, and `legion-text`.
- Placeholder crates and later runtime surfaces remain inert unless they have an accepted ADR, dependency-policy entry, phase gate, protocol contracts, contract tests, ownership tests, and evidence.

## Implementation Evidence

### 1. Repository Discovery To Lexical Map

- `WorkspaceDiscoveryRecord`, `WorkspaceDiscoverySnapshot`, and `WorkspaceDiscoveryDelta` are defined in [`legion-protocol`](../../../crates/legion-protocol/src/lib.rs:735) and round-trip in `dto_contracts_workspace_discovery_dtos_golden_and_required_fields`.
- [`WorkspaceActor::semantic_discovery_snapshot()`](../../../crates/legion-project/src/lib.rs) and watcher-derived discovery deltas expose workspace-authoritative identity, ignore, generated, binary, oversized, trust, fingerprint, and privacy decisions.
- [`RepositoryDiscoveryImporter`](../../../crates/legion-index/src/lib.rs:762) consumes only protocol discovery DTOs and classifies content-allowed, metadata-only, excluded, and deleted records without scanning the filesystem or minting workspace identity.
- [`LexicalIndexer`](../../../crates/legion-index/src/lib.rs:2818) extracts symbol maps and graph records from accepted source inputs while preserving content hash, snapshot identity, workspace generation, file content version, and privacy scope.

### 2. Open-Buffer Priority And Supersession

- [`IndexingActor`](../../../crates/legion-index/src/lib.rs:259) owns a bounded queue, cancellation map, in-flight work table, latest-file identity map, parser cache, and semantic index.
- `submit`, `start_next`, `complete_started`, `execute_next`, and `cancel` report accepted, cancelled, ignored-obsolete, rejected, applied, and backpressure outcomes without blocking editor workflows.
- Live snapshot work supersedes stale background work by priority, workspace generation, file content version, and content hash. Obsolete in-flight work completes as cancelled or ignored instead of overwriting fresh records.
- Scheduler-level `Coalesce`, `Reject`, `Refresh`, and `Reindex` decisions represent coalesced, rejected, degraded/partial, and resync-required outcomes for consumers that cannot accept stale data.

### 3. Syntax Cache And Graph Extraction

- [`SourceDocument`](../../../crates/legion-index/src/lib.rs) supports descriptor-only, snapshot-lease chunks, changed ranges, and bounded full-text inputs. Large snapshots use descriptors and chunk metadata without requiring full-source materialization.
- [`SyntaxCacheKey`](../../../crates/legion-index/src/lib.rs:2035) includes workspace id, file id, snapshot id, file content version, workspace generation, content hash, language, grammar version, parser version, model version, privacy scope, schema version, and descriptor fingerprint.
- [`SyntaxTreeCache`](../../../crates/legion-index/src/lib.rs:2272) reuses cached parser outcomes only after exact key matching and invalidates grammar-version entries explicitly.
- [`FileSemanticIndex`](../../../crates/legion-index/src/lib.rs:2390) records symbols, references, imports, exports, call edges, type relationships, test links, diagnostics links, ownership metadata, provenance, freshness, privacy scope, and metadata-only source descriptors.

### 4. LSP Fusion

- [`LspOperationContext`](../../../crates/legion-protocol/src/lib.rs:10292) carries request id, workspace/file/buffer/snapshot identity, buffer version, timeout budget, cancellation token, content hash, correlation, causality, and privacy scope.
- [`LspResultStatus`](../../../crates/legion-protocol/src/lib.rs:10273) represents fresh, stale, partial, cancelled, timeout, unavailable, and degraded outcomes.
- [`LspLaunchPolicyDecision`](../../../crates/legion-protocol/src/lib.rs:10413) is fail-closed for untrusted workspaces, denied privacy scopes, missing capabilities, or runtime activation deferral; supervision records are metadata-only and redacted.
- Diagnostics, completion, hover, definition, reference, rename, formatting, semantic-token, and code-action flows use protocol DTOs. Edit-producing rename, formatting, and code-action outputs convert to proposal-ready `WorkspaceEdit` payloads through `convert_lsp_edit_to_workspace_proposal` rather than applying edits directly.
- Command-only code actions are represented as denied or deferred command descriptors and cannot execute in Phase 3.

### 5. Proposal-Mediated Semantic Mutations

- [`build_rename_preview_payload`](../../../crates/legion-index/src/lib.rs) produces proposal-only semantic refactoring previews with version preconditions and complete target coverage.
- LSP edit conversion requires non-zero correlation, non-nil causality, complete preconditions, compatible lifecycle state, complete target coverage, acceptable privacy scope, and matching capability.
- [`AppComposition::apply_workspace_proposal()`](../../../crates/legion-app/src/lib.rs:5037) remains the mutation authority for accepted proposal payloads. Semantic and LSP code do not mutate buffers or workspace files directly.
- Existing save conflict and dirty-buffer preservation tests remain green, proving semantic and LSP acceptance did not weaken the proposal-mediated save path.

### 6. Metadata-Only Persistence And Vector Deferral

- [`SemanticMetadataBatch`](../../../crates/legion-protocol/src/lib.rs) and storage requests support metadata-only semantic record persistence, reads, and tombstones.
- [`InMemoryStorage`](../../../crates/legion-storage/src/lib.rs) and [`FileBackedStorage`](../../../crates/legion-storage/src/lib.rs) round-trip semantic metadata without source bodies and tombstone records on privacy or freshness mismatches.
- No accepted Phase 3 code computes embeddings, stores vectors, invokes model providers, or performs model-backed retrieval. Model-version fields exist only for future invalidation compatibility.

## Validation Commands

| Gate | Command | Artifact |
| --- | --- | --- |
| Dependency policy | `cargo run -p xtask -- check-deps` | [`index-dependency-boundary.txt`](index-dependency-boundary.txt) |
| Formatting | `cargo fmt --all --check` | [`cargo-fmt-check.txt`](cargo-fmt-check.txt) |
| Workspace check | `cargo check --workspace --all-targets` | [`cargo-check-workspace-all-targets.txt`](cargo-check-workspace-all-targets.txt) |
| Workspace tests | `cargo test --workspace --all-targets` | [`cargo-test-workspace-all-targets.txt`](cargo-test-workspace-all-targets.txt) |
| Workspace clippy | `cargo clippy --workspace --all-targets -- -D warnings` | [`cargo-clippy-workspace-all-targets.txt`](cargo-clippy-workspace-all-targets.txt) |
| Index contract tests | `cargo test -p legion-index --all-targets` | [`legion-index-tests.txt`](legion-index-tests.txt), [`lexical-symbol-map-tests.txt`](lexical-symbol-map-tests.txt), [`tree-sitter-cache-tests.txt`](tree-sitter-cache-tests.txt), [`normalized-graph-contract-tests.txt`](normalized-graph-contract-tests.txt), [`semantic-query-api-tests.txt`](semantic-query-api-tests.txt) |
| Protocol DTO contracts | `cargo test -p legion-protocol --test dto_contracts` | [`legion-protocol-dto-contracts.txt`](legion-protocol-dto-contracts.txt), [`lsp-supervision-tests.txt`](lsp-supervision-tests.txt) |
| Storage contracts | `cargo test -p legion-storage --all-targets` | [`privacy-redaction-audit.md`](privacy-redaction-audit.md) |
| Save regression | `cargo test -p legion-app --test workspace_vfs_integration workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict` | [`save-conflict-regression.txt`](save-conflict-regression.txt), [`proposal-routing-regression.txt`](proposal-routing-regression.txt) |
| Editor latency and background work | `cargo test -p legion-editor --test performance_suite -- --list` | [`editor-semantic-latency.txt`](editor-semantic-latency.txt) |

## Expected Evidence Artifacts

- [`semantic-fabric-architecture-map.md`](semantic-fabric-architecture-map.md): actor ownership, queues, priorities, cancellation, backpressure, cache keys, invalidation paths, storage metadata, and proposal routing.
- [`index-dependency-boundary.txt`](index-dependency-boundary.txt): dependency-policy output proving only permitted internal dependencies for [`legion-index`](../../../crates/legion-index/Cargo.toml:1).
- [`repository-discovery-ignore-fingerprint.md`](repository-discovery-ignore-fingerprint.md): discovery and ignore handling cases with file fingerprints and content hashes distinguished.
- [`lexical-symbol-map-tests.txt`](lexical-symbol-map-tests.txt): lexical extraction, symbol-file lookup, ignored-file behavior, and stale-work cancellation results.
- [`tree-sitter-cache-tests.txt`](tree-sitter-cache-tests.txt): parser/syntax cache reuse and invalidation keyed by content hash, grammar version, identity, descriptor, schema, and privacy.
- [`normalized-graph-contract-tests.txt`](normalized-graph-contract-tests.txt): symbols, references, imports, exports, call edges, type relationships, test links, diagnostics links, and ownership metadata validation.
- [`semantic-query-api-tests.txt`](semantic-query-api-tests.txt): UI navigation, completion ranking, AI context selection, agent planning, test impact, and refactoring-preview query behavior.
- [`lsp-supervision-tests.txt`](lsp-supervision-tests.txt): cancellation, backpressure, timeout, stale response suppression, diagnostics, completion, hover, definition, reference, rename, formatting, semantic-token, and code-action DTO flow.
- [`proposal-routing-regression.txt`](proposal-routing-regression.txt): proof that LSP and semantic mutation outputs route through proposals and cannot directly mutate buffers or workspaces.
- [`privacy-redaction-audit.md`](privacy-redaction-audit.md): evidence that events, caches, and persisted records default to metadata-only retention and honor privacy scope invalidation.
- [`vector-deferral-audit.md`](vector-deferral-audit.md): proof that vector indexing, embeddings, vector storage, and model-backed retrieval remain inactive.

## Final validation checklist

- [x] [`ADR-0017-semantic-fabric-indexing.md`](../../adrs/ADR-0017-semantic-fabric-indexing.md:1) accepted and cited by implementation evidence.
- [x] [`ADR-0018-lsp-runtime-supervision.md`](../../adrs/ADR-0018-lsp-runtime-supervision.md:1) accepted and cited by implementation evidence.
- [x] [`dependency-policy.md`](../../dependency-policy.md:91) permits only [`legion-protocol`](../../../crates/legion-protocol/Cargo.toml:1), [`legion-storage`](../../../crates/legion-storage/Cargo.toml:1), and [`legion-text`](../../../crates/legion-text/Cargo.toml:1) as Phase 3 internal dependencies for [`legion-index`](../../../crates/legion-index/Cargo.toml:1).
- [x] [`dependency-policy.md`](../../dependency-policy.md:52) still forbids [`legion-editor`](../../../crates/legion-editor/Cargo.toml:1) from depending on [`legion-project`](../../../crates/legion-project/Cargo.toml:1).
- [x] Actor-owned index queues are bounded and have observable accepted, coalesced, cancelled, rejected, degraded, and resync-required outcomes.
- [x] Live editor snapshots supersede background scans and cancel obsolete parse, LSP, ranking, and graph work.
- [x] Repository discovery honors ignore, generated, binary, oversized, trust, and privacy decisions.
- [x] File fingerprints, content hashes, snapshot identifiers, file content versions, and workspace generations are used for their distinct authorities.
- [x] Lexical symbol maps and symbol-file lookup work before parser or LSP enrichment is available.
- [x] Syntax caches are keyed by content hash and grammar version and invalidate on either mismatch.
- [x] Normalized graph records include symbols, references, imports, exports, call edges, type relationships, test links, diagnostics links, ownership metadata, provenance, freshness, and privacy scope.
- [x] Query APIs return freshness and degradation metadata for UI navigation, completion ranking, AI context selection, agent planning, test impact, and refactoring previews.
- [x] LSP operations are cancellable, supervised, bounded, timeout-aware, and stale-response-safe through protocol DTOs and accepted supervision metadata.
- [x] Diagnostics, completion, hover, definition, reference, rename, formatting, semantic-token, and code-action flows use protocol DTOs.
- [x] Formatting, rename, code actions, and semantic refactor suggestions route through proposals and cannot directly edit buffers or workspace files.
- [x] UI remains projection-only and does not own editor sessions, text buffers, index state, LSP workers, or workspace VFS authority.
- [x] Semantic and LSP work does not block editor input, viewport projection, proposal validation, or save workflows.
- [x] Existing proposal-mediated save conflict and dirty-buffer preservation tests remain green.
- [x] Metadata-only redaction remains the default for caches, observability, audit, and evidence.
- [x] Vector indexing, embeddings, model-backed retrieval, and vector storage remain deferred and inactive.
