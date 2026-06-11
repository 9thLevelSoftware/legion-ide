# ADR-0037: Semantic Retrieval

## Status

Accepted — ratified for Production Master Plan v0.1 M0 on 2026-06-10.

This ADR ratifies the Production Master Plan v0.1 §6 recommendation verbatim
(option (a), tree-sitter AST-aware chunking + local embeddings through
Ollama/llama.cpp + an embedded vector store (LanceDB primary candidate,
sqlite-vec reserved as fallback per the §6 follow-up spike and the draft
artifact at `plans/spikes/SPIKE-0037-vector-store-result.md`) + an
Aider-style PageRank repo map, with **agentic search as the default** and
the index as enhancement), and records the resulting crate boundary:
embedding DTOs and capability flags live in `legion-ai` and
`legion-ai-providers`; the deterministic AST-aware chunking, repo map,
retrieval chunk records, and the always-available fallback retrieval
contract live in `legion-index`; the workspace-walk and trust surface
that backs the repo map comes from `legion-project`; `legion-app` composes
all of these through the existing `legion-app` ↔ `legion-index` edge;
`legion-ui` and `legion-editor` stay projection-only and never own
retrieval state.

## Context

Legion today has the deterministic substrate for semantic retrieval but
deliberately defers the persistent vector / model-provider activation.
The current state is real and exercised by contract tests:

- `legion-index` (`crates/legion-index/src/lib.rs`, 5696 lines) implements
  the deterministic lexical / tree-sitter / structural / retrieval search
  surface that ADR-0036 ratifies, plus the always-available deterministic
  retrieval chunking and ranking contract that this ADR governs. The
  relevant pieces are visible at `crates/legion-index/src/lib.rs` around
  lines 40-65 (`INDEX_SCHEMA_VERSION`, `LEXICAL_EXTRACTION_VERSION`,
  `TREE_SITTER_EXTRACTION_VERSION`, `RETRIEVAL_CHUNKING_VERSION`,
  `LOCAL_RETRIEVAL_EMBEDDING_VERSION`,
  `LOCAL_RETRIEVAL_EMBEDDING_DIMENSIONS = 64`,
  `RETRIEVAL_CHUNK_SHA256_ALGORITHM = "sha256"`), lines 2657-2790
  (the `LocalEmbeddingVector` / `RetrievalChunkRecord` /
  `RetrievalChunkCitation` / `RetrievalQuery` / `RetrievalSearchResponse`
  surface that exposes the deterministic 64-dimensional local embedding
  contract), line 2993-3057 (`search_retrieval` — the deterministic
  local vector-style retrieval over metadata-only chunk records, ranking
  by transient query embedding against deterministic chunk embeddings),
  and lines 4178+ (`deterministic_local_embedding` — the sha256-bucketed
  64-dim stub used to populate chunk and query embeddings today). The
  chunk records persist `provenance`, `schema_version`, language id,
  citation, and label; raw source bodies are not stored in the retrieval
  chunk record, only the citation needed to fetch them through the
  existing `legion-protocol` text-snapshot contracts.

- `legion-ai` (`crates/legion-ai/src/lib.rs`, 1114 lines) defines the
  embedding provider surface: `EmbeddingRequest` / `EmbeddingResponse`
  around line 113-150, `embedding: bool` capability flag on
  `ProviderCapabilities` around lines 181-191, and the `embed` method
  on the provider trait around line 269-310. `legion-ai`'s
  `Cargo.toml` depends on `legion-protocol`, `legion-security`,
  `serde`, `serde_json`, and `thiserror` — a strict subset of the
  `legion-ai` policy entry at `plans/dependency-policy.md` §1 lines
  111-117, which authorizes `legion-protocol` and `legion-security`
  and (with `legion-ai-providers`) explicitly forbids model-provider
  runtime activation in any other crate.

- `legion-ai-providers` (`crates/legion-ai-providers/src/lib.rs`, 2446
  lines) carries the real HTTP provider clients (Ollama, llama.cpp,
  OpenAI-compatible) and `ProviderRegistry` / `ProviderRouter` that the
  embedding request route composes through. The `legion-ai-providers`
  policy entry at `plans/dependency-policy.md` §1 lines 119-125
  authorizes `legion-ai`, `legion-protocol`, and `legion-security` and
  is unchanged by this ratification. Hosted embedding activation is
  consent-gated through the existing capability broker, not a new
  boundary; air-gap mode hard-denies hosted provider calls and the
  hosted embedding request route is a no-op without consent.

- `legion-project` (`crates/legion-project/src/lib.rs`, 6802 lines) owns
  the workspace discovery, watcher, and trust-decision surface. Its
  policy entry at `plans/dependency-policy.md` §1 lines 37-41 authorizes
  `legion-observability`, `legion-platform`, `legion-protocol`, and
  `legion-security`. `legion-project` does not contain any vector /
  embedding / repo-map code today and is not authorized to gain it
  under the M0 ratification; the workspace-walk and file-graph facts
  that the repo map needs cross through `legion-index` and
  `legion-protocol`, not through `legion-project` growing a new edge.

- `legion-memory` keeps the metadata-only consented memory substrate
  (the Phase 4 / Phase 13 activation), with the policy entry at
  `plans/dependency-policy.md` §1 lines 140-149 authorizing
  `legion-protocol`, `legion-storage`, and `legion-ai`; the
  M0 ratification does not extend `legion-memory`'s allowed edges and
  does not authorize `legion-memory` to host vectors, embeddings, or
  retrieval state. The retrieval records that survive across sessions
  are metadata-only chunk citations, not raw source.

Four invariants from the master plan §2.2 constrain the semantic
retrieval stack:

- **App-composed and capability-gated** — embedding generation, vector
  storage, and any local model call go through the existing capability
  broker. The future embedding capability names are reserved alongside
  the existing `terminal.*`, `remote.transport.*`, `telemetry.*`,
  `retention.*`, and `storage.migration.*` reservation set in
  `plans/dependency-policy.md` §1. The retrieval layer may not mutate
  workspace/editor/disk outside the proposal-mediated route and may
  not persist raw source bodies (only the metadata already required
  for retrieval records, chunk references, and audit).
- **Proposal-mediated mutation** — context manifest and retrieval
  results feed the existing `WorkspaceProposal` payload through the
  accepted Phase 2 proposal routes (`ADR-0016`) and the AI-plane
  proposal flow. The retrieval layer never applies edits to buffers
  or disk; it returns `RetrievalSearchResponse` / chunk citations
  and the proposal service previews, approves, applies, rejects,
  cancels, or rolls back. The context manifest UX (WS-10.T5) is the
  user-facing gate that shows the assembled retrieval slice and
  per-item exclusion before any apply.
- **Projection-only UI boundary** — `legion-ui` consumes retrieval
  projections (the existing `SearchProjection` /
  `StructuralSearchProjection` surface and the future retrieval /
  repo-map / context-manifest projections) and emits
  `CommandDispatchIntent` only. UI never owns retrieval state, never
  owns vector store access, and never owns mutation authority. The
  `legion-ui` policy entry at `plans/dependency-policy.md` §1 lines
  54-75 already forbids `legion-ui` from depending on
  `legion-project` or any renderer crate, and the structural audit
  enforces it.
- **Metadata-first observability** — retrieval work emits
  metadata-only records (query id, scope, result counts, freshness,
  cancellation reason, model name+version, chunk identity,
  `CorrelationId` / `CausalityId` / `EventSequence`); raw query
  strings are limited to the user's own UI session, raw match lines
  are limited to bounded result projections, and the observability
  sinks that reject zero IDs apply to retrieval the same way they
  apply to terminal/AI/tracker output.

The plan compared three options: (a) the hybrid stack recorded in §6
(tree-sitter AST chunking + local embeddings by default + embedded
vector store (LanceDB primary, sqlite-vec fallback per the spike) +
Aider-style repo map, with agentic search as the default and the index
as enhancement), (b) embeddings-first retrieval, and (c) hand-rolled
hybrid without a vector store. Option (a) matches how the 2026 IDE
market converged: agentic search is the validated default (Claude Code
shipped without RAG after runtime grep/glob/read outperformed it),
the repo map is the always-available deterministic fallback that can't
go stale (Aider's validated approach), AST-aware chunking reuses the
WS-02 tree-sitter substrate, and the vector store is an enhancement
tier for very large workspaces that fails closed to the lexical/agentic
path when missing. Option (b) would re-introduce the silent-staleness
problem that the master plan §3 calls out as a known limitation of
the current deterministic stub and would conflict with the air-gap
story (every cold start would need an embedding round-trip).
Option (c) is the current state — lexical only — and is exactly what
the §6 row rejects.

## Decision

Legion will use a hybrid retrieval stack with **agentic search as the
default**, **Aider-style repo map as the always-available deterministic
fallback**, and **vector retrieval as an enhancement layer** that is
built on top of the existing `legion-index` AST-aware chunking and the
existing `legion-ai` embedding DTOs, gated by capability-broker consent
and recorded with per-index model name + version.

- **Default retrieval path (WS-10.T1).** Agentic search tools (grep,
  glob, read, outline, diagnostics, terminal excerpts) are the harness
  tool surface the model reaches for first. They need no index, cannot
  go stale, and are policy-scoped and metadata-audited through the
  existing capability broker. The agentic search tier is independent
  of the vector tier and stays always available, including in air-gap
  mode and on cold start. This matches the master plan §6 "agentic
  search as the default and the index as enhancement" line and the
  WS-10.T1 acceptance criteria.
- **Repo map (WS-10.T2).** Aider-validated structural map: tree-sitter
  defs / refs → file / symbol graph → PageRank → top-ranked
  signatures within a token budget; deterministic, cheap, always
  available; cached with watcher invalidation. The repo map is the
  fallback that keeps retrieval working in air-gap mode, before the
  vector tier is warm, and when the user disables hosted embedding
  consent. The repo map is the WS-10.T2 product and lives in
  `legion-index` over the WS-02 tree-sitter AST and the file-graph
  facts that `legion-project` exposes through `legion-protocol` —
  it is not a vector store, it is a deterministic graph-rank, and
  it does not require a new external crate at M0.
- **Vector retrieval as an enhancement layer (WS-10.T3).** AST-aware
  chunks (from WS-02.T5) → local embedding model via Ollama /
  llama.cpp by default, hosted embeddings only with explicit consent
  → embedded vector store per the §6 follow-up spike
  (LanceDB primary, sqlite-vec reserved as fallback per
  `plans/spikes/SPIKE-0037-vector-store-result.md`) → model
  name + version stored per index; lazy re-embed on model change.
  The M0 ratification ratifies the boundary and the stack choice; the
  WS-10.T3 workstream is what declares the LanceDB / sqlite-vec /
  Ollama / llama.cpp / byollama dependencies, extends the
  `legion-index` and `legion-ai` policy entries, and ships the
  product code. M0 does **not** enable the vector tier.
- **Hybrid retrieval + eval (WS-10.T4).** Lexical (WS-06) + vector
  + repo-map fusion with rank blending; retrieval eval fixture
  (queries → expected files / symbols) wired into `evals/` so
  retrieval quality is a tracked number, not vibes. Hybrid ranking
  is the WS-10.T4 product; the M0 ratification ratifies the eval
  surface as a future gate, not as an M0 prerequisite.
- **Crate boundary.** The semantic retrieval stack is split across
  `legion-ai`, `legion-ai-providers`, `legion-index`, `legion-project`,
  and `legion-app` along the accepted policy entries in
  `plans/dependency-policy.md` §1. `legion-ai` owns the embedding
  provider trait, `EmbeddingRequest` / `EmbeddingResponse` DTOs, the
  `ProviderCapabilities::embedding` capability flag, and the
  capability-gated hosted-embedding activation policy. `legion-ai`
  may **not** take a vector-store, a tree-sitter, an Ollama /
  llama.cpp runtime, or a model-provider-client edge; the embedding
  request route composes through `legion-ai-providers`, which
  carries the real HTTP provider clients and is the only crate
  authorized to take Ollama / llama.cpp / OpenAI-compatible
  client dependencies. `legion-index` owns the deterministic
  AST-aware chunking, the repo-map graph rank, the retrieval chunk
  records, the deterministic 64-dim stub embedding that backs the
  M0 contract surface, and the `search_retrieval` /
  `RetrievalSearchResponse` API. `legion-index` may **not** take a
  model-provider edge and may **not** take a vector-store
  dependency at M0; when WS-10.T3 declares LanceDB or sqlite-vec,
  the `legion-index` policy entry must be amended to add the new
  internal edges or new external runtime dependencies, and the M0
  ratification is forward-compatible with that amendment. `legion-
  project` may use `legion-index` for repo-map graph facts through
  the existing `legion-index` edge; `legion-project` may **not**
  declare a vector store, an embedding trait, or a model-provider
  edge. `legion-app` composes retrieval results through the existing
  `legion-app` ↔ `legion-index` edge (the GUI Phase 4 entry at
  `plans/dependency-policy.md` §1 line 92) and the existing
  `legion-app` ↔ `legion-ai` / `legion-app` ↔ `legion-ai-providers`
  composition edges. `legion-ui` consumes retrieval projections
  and emits `CommandDispatchIntent` only. `legion-editor` may
  **not** declare any retrieval, embedding, vector-store, or
  model-provider dependency. `legion-desktop` may **not** declare
  any retrieval, embedding, vector-store, or model-provider
  dependency. This boundary mirrors the parser-boundary audit in
  `ADR-0033`, the LSP-boundary sketch in `ADR-0034`, the
  terminal-boundary sketch in `ADR-0035`, and the search-boundary
  sketch in `ADR-0036`, and is enforced by the same
  `cargo run -p xtask -- check-deps` policy-text +
  package-dependency audit.
- **Hosted embedding activation (WS-09.T4 / WS-10.T3).** Hosted
  embedding generation is a per-workspace consent-gated capability
  routed through the same `legion-ai-providers` plumbing that
  carries the existing hosted provider clients. The consent flow
  reuses the existing hosted-provider activation gate from
  WS-09.T4 (per-workspace provider enablement, privacy inspector
  with exact egress context manifest, BYOK in the
  `legion-retention` keyring store, air-gap mode hard-denies
  hosted provider calls). The privacy inspector must show the
  exact egress (query text + chunk citations + model name) before
  the call goes out, and the manifest-to-egress equality test from
  WS-09.T4 applies to embedding generation the same way it
  applies to chat and completion.
- **Local embedding activation.** Local embedding generation is
  the default path: Ollama and llama.cpp are the two reference
  server surfaces. The model registry records model name and
  model version on every chunk record and every index; the index
  layer marks stale rows (without deleting unrelated metadata)
  when the model version changes, and the next access lazily
  re-embeds. This matches the §6 "Store model name+version per
  index; lazy re-embed" line and the SPIKE-0037 evaluation
  criterion "Lazy re-embed simulation: a model-version change
  marks stale rows without deleting unrelated metadata."
- **Crate boundary audit (future gate).** A future
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_EMBEDDING_TRAIT_PACKAGES = ["legion-ai"]` /
  `RETRIEVAL_VECTOR_STORE_ALLOWED_PACKAGES = ["legion-index"]` /
  `RETRIEVAL_MODEL_PROVIDER_PACKAGES = ["legion-ai-providers"]` /
  `FORBIDDEN_RETRIEVAL_DEPS = ["lancedb", "sqlite-vec", "ollama-rs",
  "byollama", "candle-core", "candle-transformers", "tokenizers"]`
  audit is sketched in the decision for the next phase gate,
  shaped like the existing `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_PARSER_DEPS = ["tree-sitter", "tree-sitter-rust"]`
  audit in `xtask/src/main.rs` (the constants are at lines
  446-453 in the current tree) and the future
  `SEARCH_BOUNDARY_POLICY_MARKERS` audit sketched in
  `ADR-0036`. The M0 ratification does not require the
  retrieval-boundary audit to land today; the audit is a
  phase-gate improvement that becomes useful the moment a
  workspace package actually declares one of the forbidden
  retrieval / embedding / vector-store / model-provider
  crates. Today, no package declares any of them, so the audit
  is a forward-compatibility gate, not a regression guard.
- **Compatibility with `ADR-0005` storage reservations.** Any
  persistent vector index on disk is the `ADR-0005` storage
  surface; the M0 ratification does not enable the persistent
  vector tier. The transient retrieval surface (per-session
  chunk records, in-memory repo-map cache) lives inside
  `legion-index` actor state and does not require `ADR-0005`
  activation. The WS-10.T3 workstream is the path to the
  persistent vector tier, with the agentic search and repo-map
  paths as the always-available fallbacks.
- **Compatibility with `ADR-0036` search stack.** The semantic
  retrieval stack is independent of the in-process ripgrep-class
  search stack ratified in `ADR-0036` and the optional tantivy
  indexed-search tier reserved there. The three tiers — lexical
  (ADR-0036), agentic + repo-map (this ADR), and vector (this
  ADR) — are independent in the dependency policy and activate
  on separate gates (ADR-0036, ADR-0037 agentic + repo-map, and
  ADR-0037 vector).
- **Compatibility with the air-gap story.** The agentic search
  path, the repo map, the deterministic 64-dim stub embedding,
  and the lexical path all work in air-gap mode without any
  hosted provider call. The vector tier (LanceDB / sqlite-vec
  with local embeddings only) is also air-gap-compatible; the
  hosted-embedding consent gate is the only path that requires
  egress, and that gate is hard-denied by air-gap mode.

## Consequences

- **Positive:** the M0 ratification ratifies a working default
  retrieval path (agentic search) plus a working always-available
  fallback (repo map) plus a real enhancement path (vector
  retrieval) that reuses the existing `legion-index` AST-aware
  chunking substrate (WS-02.T5) and the existing `legion-ai`
  embedding DTOs. The hybrid ranking surface (WS-10.T4) is a
  future product, not a future build.
- **Positive:** the M0 ratification ratifies the boundary that
  prevents the silent-staleness problem (agentic search can't
  silently go stale; the repo map is recomputed from the
  tree-sitter AST and the file graph on every relevant change;
  the vector index records model name + version and lazily
  re-embeds on model change). The retrieval eval fixture from
  WS-10.T4 is the future gate that catches quality regressions
  across the three tiers.
- **Positive:** the WS-10 workstreams (T1 agentic search, T2
  repo map, T3 embedding pipeline, T4 hybrid eval, T5 context
  manifest UX) have a real starting point in
  `crates/legion-index/src/lib.rs` (5696 lines, 38 contract
  tests) and `crates/legion-ai/src/lib.rs` (1114 lines, real
  provider trait + `EmbeddingRequest` / `EmbeddingResponse` /
  `ProviderCapabilities::embedding` DTOs). The `legion-app`
  composition edges to `legion-index` and `legion-ai` are
  already policy-allowed; the existing `legion-app` search and
  AI projection surfaces (the `SearchProjection` /
  `StructuralSearchProjection` family and the existing AI
  projection surface) are the starting point for the
  retrieval-projection surface that WS-10.T3 / T4 / T5 will
  add.
- **Positive:** air-gap mode is preserved. The agentic search
  path, the repo map, the deterministic 64-dim stub embedding,
  and the lexical path are all air-gap-compatible. The vector
  tier with local embeddings (Ollama / llama.cpp) is
  air-gap-compatible. Hosted embedding activation is
  consent-gated and hard-denied by air-gap mode.
- **Negative:** introducing LanceDB / sqlite-vec / Ollama /
  llama.cpp / byollama is a WS-10.T3 runtime activation, not an
  M0 prerequisite. The M0 ratification ratifies the boundary and
  the stack choice; the WS-10.T3 ("Embedding pipeline
  (local-first)") task is the workstream that declares the
  dependencies, extends the `legion-index` and `legion-ai`
  policy entries, adds the retrieval-boundary audit, and ships
  the product code that the §6 row describes.
- **Negative:** hybrid ranking adds evaluation burden. The
  retrieval eval fixture from WS-10.T4 (queries → expected
  files / symbols) is the offline gate that measures lexical /
  vector / repo-map / hybrid quality, and the WS-10.T4 task is
  the one that adds it. The M0 ratification ratifies the eval
  surface as a future gate, not as an M0 prerequisite.
- **Mitigation:** the `legion-index` policy entry is the only
  edge that needs the new `lancedb` / `sqlite-vec` /
  tree-sitter-grammar-runtime declarations for the AST-aware
  chunking path, the `legion-ai-providers` policy entry is
  the only edge that needs the new `ollama-rs` / `byollama` /
  OpenAI-compatible embedding-client declarations, and the
  future `RETRIEVAL_BOUNDARY_POLICY_MARKERS` audit is the
  structural guard that prevents any other crate from
  declaring them. The structural dependency audit that
  already runs as part of `cargo run -p xtask -- check-deps`
  is the M0 test surface; the WS-10.T3 acceptance criteria
  are the air-gap / hosted-consent / lazy-reembed /
  model-version-metadata test surface.

## Verification

- `cargo run -p xtask -- check-deps` (dependency direction +
  structural audit, with the `legion-index`, `legion-ai`, and
  `legion-ai-providers` policy entries verified against
  `plans/dependency-policy.md` §1 and the retrieval-boundary
  sketch above)
- `cargo run -p xtask -- docs-hygiene` (broken relative Markdown
  links and the unallowlisted stale Legion-rename marker)
- `cargo run -p xtask -- no-egui-textedit` (companion gate,
  unchanged from `ADR-0032`; the retrieval / repo-map / context
  manifest panel renders projected retrieval results, not an
  `egui::TextEdit`)
- `cargo fmt --all --check`
- `cargo test -p legion-index --tests` (deterministic lexical /
  structural / retrieval search APIs, 38 contract tests across
  the lib unittests + the `index_workflows` integration test,
  covering the `RetrievalQuery` / `RetrievalSearchResponse` /
  `search_retrieval` / `LocalEmbeddingVector` /
  `RetrievalChunkRecord` surface that this ADR governs, plus the
  `INDEX_SCHEMA_VERSION` / `RETRIEVAL_CHUNKING_VERSION` /
  `LOCAL_RETRIEVAL_EMBEDDING_VERSION` /
  `LOCAL_RETRIEVAL_EMBEDDING_DIMENSIONS` /
  `RETRIEVAL_CHUNK_SHA256_ALGORITHM` constants)
- `cargo test -p legion-ai --tests` (the embedding provider
  trait, `EmbeddingRequest` / `EmbeddingResponse` DTOs, and
  `ProviderCapabilities::embedding` capability flag)
- `cargo test -p legion-ai-providers --tests` (the real HTTP
  provider clients and `ProviderRegistry` / `ProviderRouter`
  that the embedding request route composes through, including
  the prompt-stability contract test that the WS-09.T2 work
  added)
- `cargo test -p legion-memory --tests` (the metadata-only
  consented memory substrate that the Phase 4 / Phase 13
  activation bounded)
- `cargo test -p legion-app --tests` (the search / structural-
  search / fuzzy-finder / command-palette projection surface
  that the retrieval projection surface will extend, including
  the `run_search` / `run_structural_search` composition entry
  points and the `RunSearch` / `RunStructuralSearch`
  `AppCommandRequest` paths)
- WS-10 evidence under `plans/evidence/production/m2/` once
  the agentic search tools (WS-10.T1) and the repo map
  (WS-10.T2) land with dependency-policy updates and contract
  tests; WS-10.T3 vector-tier evidence under
  `plans/evidence/production/m2/` once the LanceDB /
  sqlite-vec / Ollama / llama.cpp activation lands with
  dependency-policy updates and contract tests; WS-10.T4
  hybrid-eval evidence under the same directory once the
  retrieval eval fixture lands; WS-10.T5 context-manifest
  evidence under the same directory once the manifest UX
  lands. M0 ratification does not require any of these
  WS-10 evidence packages today; the M0 evidence package
  for this ratification is
  `plans/evidence/production/M0/ADR-0037-ratification.md`.
- Vector-store spike evidence at
  `plans/spikes/SPIKE-0037-vector-store-result.md` is the
  decision matrix that records LanceDB as the primary
  candidate and sqlite-vec as the fallback; the follow-up
  build-and-benchmark spike obligations (synthetic
  64-dimensional fixture, ≥10k vectors, deterministic
  tie-breakers, model name / version metadata, lazy re-embed
  simulation, p50 / p95 latency, dependency-policy and
  cargo-deny review) are recorded in the spike and become
  the WS-10.T3 acceptance criteria.
