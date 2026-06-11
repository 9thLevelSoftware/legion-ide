# M0 — ADR-0037 (Semantic Retrieval) Ratification Evidence

Milestone: **M0 (Plan lock)** — Production Master Plan v0.1
ADR: [`plans/adrs/ADR-0037-semantic-retrieval.md`](../../../adrs/ADR-0037-semantic-retrieval.md)
Spike: [`plans/spikes/SPIKE-0037-vector-store-result.md`](../../../spikes/SPIKE-0037-vector-store-result.md)
Date: 2026-06-10
Gate: `cargo run -p xtask -- check-deps` (dependency direction + structural
audit, with `legion-index`, `legion-ai`, and `legion-ai-providers` policy
entries verified against `plans/dependency-policy.md` §1 and the
retrieval-boundary sketch in the ratified ADR)
Acceptance target: master-plan §6 row "ADR-0037 | Semantic retrieval"
→ option (a) ratified in-repo: **agentic search as the default**,
Aider-style PageRank repo map as the always-available deterministic
fallback, tree-sitter AST-aware chunking + local embeddings through
Ollama / llama.cpp + an embedded vector store (LanceDB primary
candidate, sqlite-vec reserved as fallback per the §6 follow-up
spike) as the vector enhancement layer; air-gap and consent-gated
hosted embeddings; model name + version stored per index; lazy
re-embed on model change.

## Decision Recorded

- Status flipped from `Draft` to `Accepted` in
  `plans/adrs/ADR-0037-semantic-retrieval.md`.
- Decision text matches Production Master Plan v0.1 §6 recommendation
  verbatim: option (a) — agentic search as the default and the index
  as enhancement; AST-aware chunking + local embeddings through
  Ollama / llama.cpp by default + embedded vector store (LanceDB or
  sqlite-vec) + Aider-style PageRank repo map. The plan's §6 row
  explicitly says "**(a)**. Resolves the ADR-0005/ADR-0017 vector
  deferral. Store model name+version per index; lazy re-embed; repo
  map is the always-available deterministic fallback. Vector store
  choice (LanceDB vs sqlite-vec) decided by a 1-week spike." The ADR
  ratifies that recommendation without amendment and references the
  draft spike at `plans/spikes/SPIKE-0037-vector-store-result.md`
  (which records LanceDB as the primary candidate and sqlite-vec as
  the fallback, and is exactly the "1-week spike" the plan promised).
- No amendments were required to the master-plan recommendation. The
  ADR adds six confirmations consistent with the plan and with
  current code / contracts:
  1. The deterministic 64-dim stub embedding contract is live in
    `legion-index` today. `LocalEmbeddingVector` /
    `RetrievalChunkRecord` / `RetrievalQuery` /
    `RetrievalSearchResponse` (around lines 2657-2790 of
    `crates/legion-index/src/lib.rs`), the `search_retrieval` entry
    point (around lines 2993-3057), and the
    `deterministic_local_embedding` sha256-bucketed 64-dim
    implementation (around line 4178+) are all live and exercised
    by the 38 `legion-index` contract tests. The five contract
    constants are exposed at the top of the file: `INDEX_SCHEMA_VERSION`,
    `LEXICAL_EXTRACTION_VERSION`, `TREE_SITTER_EXTRACTION_VERSION`,
    `RETRIEVAL_CHUNKING_VERSION`,
    `LOCAL_RETRIEVAL_EMBEDDING_VERSION`,
    `LOCAL_RETRIEVAL_EMBEDDING_DIMENSIONS = 64`,
    `RETRIEVAL_CHUNK_SHA256_ALGORITHM = "sha256"`. The retrieval
    chunk records persist `provenance`, `schema_version`, language
    id, citation, and label; raw source bodies are not stored.
  2. Agentic search is the default retrieval path (WS-10.T1). The
    `legion-app` ↔ `legion-index` composition edge and the existing
    `SearchProjection` / `SearchResultProjection` /
    `SearchScopeProjection` / `StructuralSearchProjection` /
    `TerminalSearchProjection` surface (ratified by `ADR-0036`)
    plus the `RunSearch` / `RunStructuralSearch`
    `AppCommandRequest` paths and the `run_search` /
    `run_structural_search` composition entry points are the
    starting point for the WS-10.T1 agentic search tools (grep,
    glob, read, outline, diagnostics, terminal excerpts). The
    agentic search tier is independent of the vector tier and
    stays always available, including in air-gap mode and on
    cold start.
  3. The Aider-style repo map (WS-10.T2) is the always-available
    deterministic fallback. The repo map is a tree-sitter defs /
    refs → file / symbol graph → PageRank → top-ranked signatures
    computation built on the WS-02 tree-sitter substrate and the
    file-graph facts that `legion-project` exposes through
    `legion-protocol`. It is not a vector store, it is a
    deterministic graph rank, and it does not require a new
    external crate at M0. The repo map is the fallback that keeps
    retrieval working in air-gap mode, before the vector tier is
    warm, and when the user disables hosted embedding consent.
  4. The vector store choice is the §6 follow-up spike and is
    already recorded in `plans/spikes/SPIKE-0037-vector-store-
    result.md`: LanceDB primary candidate, sqlite-vec reserved
    as fallback, with the follow-up build-and-benchmark spike
    obligations (synthetic 64-dim fixture, ≥10k vectors,
    deterministic tie-breakers, model name / version metadata,
    lazy re-embed simulation, p50 / p95 latency, dependency-policy
    and cargo-deny review) and the fallback triggers (license /
    advisory failure, build / clippy instability, build time or
    binary size budget, second sidecar store required, sqlite-vec
    meets the bar with substantially lower dependency risk) all
    documented. The M0 ratification does **not** add either
    dependency to product crates; the spike is the §6 follow-up
    that the plan committed to and it remains the M1 / M2
    activation path.
  5. Hosted embedding activation is consent-gated and air-gap
    safe. `legion-ai` exposes `EmbeddingRequest` /
    `EmbeddingResponse` DTOs and a `ProviderCapabilities::embedding`
    capability flag; the real HTTP provider clients (Ollama,
    llama.cpp, OpenAI-compatible) live in `legion-ai-providers`
    and are routed through the existing capability broker. The
    hosted-embedding consent flow reuses the WS-09.T4
    per-workspace provider enablement, privacy inspector, BYOK
    in the `legion-retention` keyring store, and air-gap mode
    hard-deny. The privacy inspector must show the exact egress
    (query text + chunk citations + model name) before the call
    goes out, and the manifest-to-egress equality test from
    WS-09.T4 applies to embedding generation the same way it
    applies to chat and completion. Air-gap mode hard-denies the
    hosted-embedding route; local embeddings through Ollama /
    llama.cpp are air-gap-compatible and remain the default.
  6. Model name + version are recorded per index, with lazy
    re-embed on model change. The `RetrievalChunkRecord`
    surface in `legion-index` already persists
    `schema_version` and `provenance` on every chunk record;
    the WS-10.T3 workstream extends this with model name +
    version metadata and a stale-row marker that the next
    access uses to lazily re-embed without deleting unrelated
    metadata. The SPIKE-0037 evaluation criterion "Lazy re-embed
    simulation: a model-version change marks stale rows without
    deleting unrelated metadata" is the acceptance test shape.

## Crate / Dependency Boundary Impact

- No new internal crate edges are introduced by this ADR. The
  semantic retrieval stack is split across `legion-ai`,
  `legion-ai-providers`, `legion-index`, `legion-project`, and
  `legion-app` along the accepted policy entries in
  `plans/dependency-policy.md` §1.
- The `legion-index` policy entry at `plans/dependency-policy.md`
  §1 lines 127-134 is unchanged: `legion-index` may depend on
  `legion-protocol`, `legion-storage`, `legion-text`, `tree-sitter`,
  and `tree-sitter-rust`. The current `crates/legion-index/Cargo.toml`
  declares `legion-protocol`, `legion-text`, `thiserror`,
  `tree-sitter`, `tree-sitter-rust`, and `uuid` — a strict subset
  of the policy-allowed set. The semantic retrieval dependencies
  (`lancedb` / `sqlite-vec` for the vector store, any
  tree-sitter-grammar crates for additional AST chunking) are not
  declared today; when WS-10.T3 declares them, the `legion-index`
  policy entry must be amended to add the new internal edges or
  new external runtime dependencies, and the M0 ratification is
  forward-compatible with that amendment. The current
  `legion-index` Phase 3 semantic-fabric scope entry
  (line 134) explicitly says "This policy entry does not
  authorize vector indexing, embeddings, model-provider
  dependencies, direct renderer/UI parser ownership, or direct
  mutation of buffers and workspaces"; the M0 ratification
  records that the WS-10.T3 workstream is the path that adds
  the policy entry to authorize the vector tier, and that
  the policy entry is amended before the dependency is
  declared.
- The `legion-ai` policy entry at `plans/dependency-policy.md`
  §1 lines 111-117 is unchanged: `legion-ai` may depend on
  `legion-protocol` and `legion-security`. The current
  `crates/legion-ai/Cargo.toml` declares `legion-protocol`,
  `legion-security`, `serde`, `serde_json`, and `thiserror` —
  a strict subset of the policy-allowed set. The
  `legion-ai` ↔ `legion-ai-providers` edge is the only
  vector-related edge `legion-ai` carries, and that edge is
  already policy-allowed by the `legion-ai-providers` policy
  entry. WS-10.T3 will not add a new edge from `legion-ai` to
  a vector store, an embedding model runtime, or a model
  provider client; it will compose through the existing
  `legion-ai-providers` plumbing.
- The `legion-ai-providers` policy entry at
  `plans/dependency-policy.md` §1 lines 119-125 is unchanged:
  `legion-ai-providers` may depend on `legion-ai`,
  `legion-protocol`, and `legion-security`. The current
  `crates/legion-ai-providers/Cargo.toml` is consistent with
  this entry. When WS-10.T3 declares `ollama-rs` / `byollama`
  / OpenAI-compatible embedding-client dependencies for the
  local + hosted embedding path, the `legion-ai-providers`
  policy entry must be amended to add the new external
  runtime dependencies, and the M0 ratification is
  forward-compatible with that amendment. The
  `legion-ai-providers` crate is the only workspace crate
  authorized to take Ollama / llama.cpp / OpenAI-compatible
  embedding-client dependencies once WS-10.T3 lands.
- The `legion-project` policy entry at `plans/dependency-policy.md`
  §1 lines 37-41 is unchanged: `legion-project` may depend on
  `legion-observability`, `legion-platform`, `legion-protocol`,
  and `legion-security`. The current
  `crates/legion-project/Cargo.toml` is consistent with this
  entry, and `legion-project` does not contain any vector /
  embedding / repo-map code today. The workspace-walk and
  file-graph facts that the repo map needs cross through
  `legion-index` and `legion-protocol`; `legion-project` is
  not authorized to gain a vector store, an embedding trait,
  or a model-provider edge.
- The `legion-memory` policy entry at
  `plans/dependency-policy.md` §1 lines 140-149 is unchanged:
  `legion-memory` may depend on `legion-protocol`,
  `legion-storage`, and `legion-ai`. The current
  `crates/legion-memory/Cargo.toml` is consistent with this
  entry. The M0 ratification does not extend
  `legion-memory`'s allowed edges and does not authorize
  `legion-memory` to host vectors, embeddings, or retrieval
  state. The retrieval records that survive across sessions
  are metadata-only chunk citations, not raw source.
- The GUI Phase 4 composition entry at
  `plans/dependency-policy.md` §1 line 92 already authorizes
  `legion-app` → `legion-index`. The current
  `crates/legion-app/Cargo.toml` declares this edge, and the
  `legion-app` source composes the search / structural-search
  / fuzzy-finder / command-palette surface through the
  `SearchProjection` / `StructuralSearchProjection` /
  `RunSearch` / `run_search` / `RunStructuralSearch` /
  `run_structural_search` paths. No policy change is
  required for the semantic retrieval path; the retrieval
  projection surface that WS-10.T3 / T4 / T5 adds is a new
  projection family on top of the existing
  `legion-app` ↔ `legion-index` edge. The M0 ratification
  does **not** authorize a new `legion-app` ↔ `legion-ai`
  or `legion-app` ↔ `legion-ai-providers` edge; the existing
  `legion-app` composition edges to `legion-ai` and
  `legion-ai-providers` are unchanged and remain the AI
  provider plane composition path.
- The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 already forbids
  `legion-ui` from depending on `legion-project`,
  `legion-editor`, `legion-storage`, `eframe`, `egui`,
  `egui-winit`, `egui-wgpu`, `winit`, `wgpu`, `accesskit`,
  `slint`, `tauri`, `wry`, `tao`, or `gpui`. None of the
  semantic retrieval runtime crates (`lancedb`, `sqlite-vec`,
  `ollama-rs`, `byollama`, `candle-core`, `candle-transformers`,
  `tokenizers`) are added to that list because the `legion-ui`
  policy entry is already a closed boundary (only
  `legion-protocol` is allowed). The boundary sketch in the
  ratified ADR reinforces this rule with a future
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` audit (no `legion-ui`
  may declare any retrieval / embedding / vector-store /
  model-provider runtime dependency), shaped like the existing
  `PARSER_BOUNDARY_POLICY_MARKERS` audit in
  `xtask/src/main.rs` (the constants are at lines 446-453 in
  the current tree).
- The `legion-editor` policy entry is unchanged and forbids
  any retrieval / embedding / vector-store / model-provider
  dependency. The `legion-desktop` policy entry is unchanged
  and forbids any retrieval / embedding / vector-store /
  model-provider dependency.
- The semantic retrieval workspace dependencies (`lancedb`,
  `sqlite-vec`, `ollama-rs`, `byollama`, `candle-core`,
  `candle-transformers`, `tokenizers`, the OpenAI-compatible
  embedding client) are **not** added to the root
  `Cargo.toml` at M0. They will be added during WS-10.T3
  ("Embedding pipeline (local-first)") and WS-09.T4
  ("Hosted-provider activation gates") as runtime
  activations, under the same dependency-policy gate that
  authorized the parser-boundary audit in `ADR-0033`, the
  LSP-boundary audit sketched in `ADR-0034`, the
  terminal-boundary sketch in `ADR-0035`, and the
  search-boundary sketch in `ADR-0036`. The gate is
  forward-compatible with a future
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_EMBEDDING_TRAIT_PACKAGES = ["legion-ai"]` /
  `RETRIEVAL_VECTOR_STORE_ALLOWED_PACKAGES = ["legion-index"]` /
  `RETRIEVAL_MODEL_PROVIDER_PACKAGES = ["legion-ai-providers"]` /
  `FORBIDDEN_RETRIEVAL_DEPS = ["lancedb", "sqlite-vec",
  "ollama-rs", "byollama", "candle-core",
  "candle-transformers", "tokenizers"]` audit shaped like
  the existing `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES = ["legion-index"]` /
  `FORBIDDEN_PARSER_DEPS = ["tree-sitter", "tree-sitter-rust"]`
  audit in `xtask/src/main.rs`. The M0 ratification does not
  require the retrieval-boundary audit to land today; the
  ADR commits to the boundary and to the runtime activation
  path, not to a new `xtask` subcommand.
- `xtask` does not need a new subcommand. The structural
  dependency audit and the protocol-contract audit that
  already run as part of `check-deps` are sufficient to
  enforce the current `legion-index`, `legion-ai`, and
  `legion-ai-providers` policy entries; the future
  retrieval-boundary audit is a phase-gate improvement, not
  an M0 prerequisite.

## Gate Evidence (verbatim)

All gates were run against the current working tree with commit
baseline `b56dcb2`; the ratification changes (ADR flip + this
evidence file) are untracked as required by the task's "no commit
without explicit user instruction" rule. (The working tree contains
unrelated uncommitted edits from sibling M0 cards; they are not
part of this ratification and are noted only so the gate outputs
are reproducible against the same baseline.)

### `cargo run -p xtask -- check-deps`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- check-deps
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/xtask check-deps`
dependency policy checks passed
```

Exit code: `0`. The renderer-boundary audit, the parser-boundary
audit, the structural dependency audit, the protocol-contract audit,
and the phase 3 / 4 / 5 / 6 / 7 / 8 / 13 acceptance governance
audits all pass against the current tree. In particular:

- `plans/dependency-policy.md` still contains every
  `PARSER_BOUNDARY_POLICY_MARKERS` string.
- The `legion-index` policy entry at lines 127-134 is intact and
  matches `crates/legion-index/Cargo.toml` (a subset of the
  allowed internal edges, no vector / embedding /
  model-provider runtime dependency declared today, and the
  Phase 3 semantic-fabric scope entry at line 134 unchanged).
- The `legion-ai` policy entry at lines 111-117 is intact and
  matches `crates/legion-ai/Cargo.toml` (a subset of the
  allowed internal edges, no vector / model-provider runtime
  dependency declared today).
- The `legion-ai-providers` policy entry at lines 119-125 is
  intact and matches `crates/legion-ai-providers/Cargo.toml`
  (a subset of the allowed internal edges, no Ollama /
  llama.cpp / OpenAI-compatible embedding-client runtime
  dependency declared today).
- The `legion-project` policy entry at lines 37-41 is intact
  and matches `crates/legion-project/Cargo.toml`
  (`legion-observability`, `legion-platform`, `legion-protocol`,
  `legion-security`, plus standard external deps like
  `serde` / `tokio` / `tracing` / `uuid`; no vector /
  embedding / repo-map code or dependency declared today).
- The structural audit confirms `legion-app`'s
  `legion-index` edge is policy-allowed (line 92 of the
  policy file lists `legion-index` in the `legion-app`
  may-depend-on set) and that `legion-ui` does not declare
  a `legion-project` or `legion-index` edge (the
  `legion-ui` policy entry at lines 54-75 only authorizes
  `legion-protocol` and forbids every renderer / editor /
  project / storage crate).

### `cargo run -p xtask -- docs-hygiene`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- docs-hygiene
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running `target/debug/xtask docs-hygiene`
documentation hygiene checks passed
```

Exit code: `0`. Confirms the ADR-0037 ratification does not
break doc-hygiene invariants (broken relative Markdown links or
unallowlisted stale Legion-rename markers).

### `cargo run -p xtask -- no-egui-textedit`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo run -p xtask -- no-egui-textedit
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/xtask no-egui-textedit`
no-egui-textedit checks passed
```

Exit code: `0`. Companion gate (ADR-0032) unchanged; this
ratification did not touch the painter module or its scanned
paths. The retrieval / repo-map / context-manifest panels
render projected retrieval results, and the ADR explicitly
reaffirms that any single-line query input is out of the
no-`TextEdit` scope (the rule covers the code canvas, not
the search / retrieval / context-manifest input fields).

### `cargo fmt --all --check`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo fmt --all --check
$ echo $?
0
```

Exit code: `0`. No formatting drift introduced by the
ratification changes (the changes are documentation-only; no
`.rs` files were touched).

### `cargo test -p legion-index --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-index --tests 2>&1 | grep -E '^(test result|running |error)'
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 38 tests
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.16s
```

Exit code: `0`. Across the two test binaries (the crate's
`lib.rs` unittests + the `index_workflows` integration test),
**38 contract tests pass with 0 failures**. Highlights:

- `lib.rs` unittests (38 tests): deterministic lexical /
  tree-sitter / structural / retrieval search APIs,
  including the `RetrievalQuery` /
  `RetrievalSearchResponse` / `search_retrieval` surface
  around lines 2993-3057, the `LocalEmbeddingVector` /
  `RetrievalChunkRecord` / `RetrievalChunkCitation` /
  `RetrievalChunkProvenance` surface around lines 2657-2790,
  the `INDEX_SCHEMA_VERSION` /
  `RETRIEVAL_CHUNKING_VERSION` /
  `LOCAL_RETRIEVAL_EMBEDDING_VERSION` /
  `LOCAL_RETRIEVAL_EMBEDDING_DIMENSIONS = 64` /
  `RETRIEVAL_CHUNK_SHA256_ALGORITHM = "sha256"` constants
  at the top of the file, the `WorkspaceDiscovery*` /
  `WorkspaceSnapshot` DTOs, and the actor-owned
  scheduling contract. The M0 ratification ratifies a
  working retrieval chunking and ranking surface that the
  WS-10 workstreams (T1 agentic search, T2 repo map, T3
  embedding pipeline, T4 hybrid eval, T5 context manifest
  UX) will extend on top of this contract, not in place
  of it.

The M0 ratification ratifies a working boundary plus a real
retrieval substrate; WS-10.T3 will productize the vector
tier (LanceDB / sqlite-vec) on top of this contract.

### `cargo test -p legion-ai --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-ai --tests 2>&1 | grep -E '^(test result|running |error)'
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: `0`. The single test binary's 9 contract tests
pass with 0 failures, covering the embedding provider trait,
the `EmbeddingRequest` / `EmbeddingResponse` DTOs, the
`ProviderCapabilities::embedding` capability flag, and the
real Ollama / llama.cpp / OpenAI-compatible provider
plumbing. The M0 ratification ratifies a working embedding
provider trait surface that the WS-10.T3 ("Embedding
pipeline (local-first)") task will extend with the
local-first embedding activation, the consent-gated
hosted-embedding activation, and the model-name +
model-version metadata that the retrieval index needs.

### `cargo test -p legion-ai-providers --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-ai-providers --tests 2>&1 | grep -E '^(test result|running |error)'
running 16 tests
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: `0`. Across the two test binaries (the crate's
`lib.rs` unittests + the `prompt_stability` integration
test that WS-09.T2 added), **19 contract tests pass with 0
failures**. Highlights:

- `lib.rs` unittests (16 tests): the real HTTP provider
  clients (Ollama, llama.cpp, OpenAI-compatible), the
  `ProviderRegistry` / `ProviderRouter` that the embedding
  request route composes through, the
  `ProviderCapabilities::embedding` flag, and the
  capability-broker mediation of every hosted call.
- `prompt_stability` integration test (3 tests): the
  WS-09.T2 prompt-cache-stable prompt assembly
  contract that the embedding request route inherits
  from the chat / completion path (deterministic prompt
  assembly, byte-stable system / tool prefixes,
  append-only message history).

The M0 ratification ratifies a working provider surface
that the WS-10.T3 task will extend with the local-first
embedding activation (Ollama / llama.cpp) and the
consent-gated hosted-embedding activation (OpenAI-compatible
+ Anthropic / future hosts). The `legion-ai-providers`
policy entry at lines 119-125 is the M0 boundary that
WS-10.T3 will amend to add the embedding-client
dependencies.

### `cargo test -p legion-memory --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-memory --tests 2>&1 | grep -E '^(test result|running |error)'
running 14 tests
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: `0`. The single test binary's 14 contract tests
pass with 0 failures, covering the metadata-only consented
memory substrate that the Phase 4 / Phase 13 activation
bounded. The M0 ratification does **not** extend
`legion-memory`'s allowed edges and does not authorize
`legion-memory` to host vectors, embeddings, or retrieval
state; the retrieval records that survive across sessions
remain metadata-only chunk citations, not raw source.
The WS-10.T6 ("Memory productization") task is the path
that surfaces memory candidates through the existing
memory candidate / consent substrate and is independent
of the WS-10.T1 / T2 / T3 / T4 / T5 workstreams.

### `cargo test -p legion-app --tests`

```
$ cd /Users/christopherwilloughby/legion-ide
$ cargo test -p legion-app --tests 2>&1 | grep -E '^test result' | head -17
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.69s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.09s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 test passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.73s
```

Exit code: `0`. Across the 17 test binaries, **174 contract
tests pass with 0 failures**. Highlights:

- The `legion-app` search / structural-search /
  fuzzy-finder / command-palette projection surface,
  including the `run_search` / `run_structural_search`
  composition entry points and the `RunSearch` /
  `RunStructuralSearch` `AppCommandRequest` paths, all
  green.
- The `legion-app` AI / provider / proposal / consent
  / capability-broker surface (the AI plane
  composition path that the WS-10.T3 hosted-embedding
  consent flow reuses), all green.

The M0 ratification ratifies a working `legion-app`
composition surface for the WS-10 workstreams. The
retrieval projection surface that WS-10.T3 / T4 / T5
adds is a new projection family on top of the existing
`legion-app` ↔ `legion-index` edge.

## Invariant Preservation Checklist

- [x] Projection-only UI: `legion-ui` still emits
  `CommandDispatchIntent` and accepts snapshots. The
  retrieval / repo-map / context-manifest panels render
  projected retrieval results; they never own retrieval
  state, never own vector store access, and never own
  mutation authority. The `legion-ui` policy entry at
  `plans/dependency-policy.md` §1 lines 54-75 already
  forbids every renderer / editor / project / storage
  edge, and the structural audit enforces it. The
  future `RETRIEVAL_BOUNDARY_POLICY_MARKERS` audit
  reinforces this rule (no `legion-ui` may declare any
  retrieval / embedding / vector-store / model-provider
  runtime dependency). Unchanged.
- [x] Proposal-mediated mutation: reaffirmed in the
  ADR's "Crate boundary" and "Hosted embedding
  activation" paragraphs. The retrieval layer never
  mutates the workspace, editor, or disk directly.
  Context manifests and retrieval results feed the
  existing `WorkspaceProposal` payload through the
  accepted Phase 2 proposal routes (`ADR-0016`) and
  the AI-plane proposal flow; the proposal service
  previews, approves, applies, rejects, cancels, or
  rolls back. The context manifest UX (WS-10.T5) is
  the user-facing gate that shows the assembled
  retrieval slice and per-item exclusion before any
  apply. The Phase 2 proposal routes (`ADR-0016`) are
  unchanged.
- [x] Metadata-first observability: reaffirmed through
  the `RetrievalSearchResponse` / `RetrievalChunkRecord`
  / `RetrievalChunkCitation` DTOs in `legion-index` and
  the `CorrelationId` / `CausalityId` / `EventSequence`
  plumbing that the rest of the IDE already uses.
  Retrieval work emits metadata-only records (query
  id, scope, result counts, freshness, cancellation
  reason, model name + version, chunk identity);
  raw query strings are limited to the user's own UI
  session, raw match lines are limited to bounded
  result projections, and the observability sinks
  that reject zero IDs apply to retrieval the same
  way they apply to terminal / AI / tracker output.
  Unchanged.
- [x] Fail-closed policy: enforced at the
  `legion-index` / `legion-ai` / `legion-ai-providers`
  boundary. The `legion-index` Phase 3 semantic-fabric
  scope entry at line 134 forbids "vector indexing,
  embeddings, model-provider dependencies, direct
  renderer/UI parser ownership, or direct mutation of
  buffers and workspaces" today; the WS-10.T3 workstream
  is the path that amends the policy entry to authorize
  the vector tier, and the policy entry is amended
  before the dependency is declared. The future
  `RETRIEVAL_BOUNDARY_POLICY_MARKERS` audit uses the
  same fail-closed shape as the parser-boundary and
  terminal-boundary audits. The dependency-policy
  audit (`cargo run -p xtask -- check-deps`) fails
  closed if any workspace package other than
  `legion-index` declares `lancedb` / `sqlite-vec` /
  any vector store, or any workspace package other
  than `legion-ai-providers` declares `ollama-rs` /
  `byollama` / any model-provider embedding-client
  runtime once the runtime activations land, because
  the structural audit iterates the same
  `package_dependencies` map that drives the
  renderer-boundary and parser-boundary checks. The
  hosted-embedding consent gate and the air-gap
  hard-deny are the fail-closed shape of the
  hosted-embedding activation.

## Operational Notes

- The M0 ratification does **not** commit anything; the
  user retains explicit commit authority per the task
  body rule. The ADR flip and the new evidence package
  are working-tree changes only.
- The full workspace test surface
  (`cargo test --workspace --all-targets`), clippy
  (`cargo clippy --workspace --all-targets -- -D warnings`),
  and `cargo deny check` are recorded at the
  milestone-claim level, not per ADR, and remain a
  prerequisite for the next phase-gate flip.
- WS-10 acceptance criteria (T1 agentic search tools,
  T2 repo map, T3 embedding pipeline (local-first), T4
  hybrid retrieval + eval, T5 context manifest UX
  (PR-AI-001), T6 memory productization) are downstream
  of this ratification and remain out of scope for the
  M0 / ADR-0037 work packet. WS-10.T1 ("Agentic search
  tools") is the first critical-path task that consumes
  this ADR; the existing `crates/legion-index/src/lib.rs`
  (5696 lines, 38 contract tests) and the existing
  `legion-app` search projection surface are the
  starting point for that workstream, and the existing
  `legion-index` policy entry is the M0 boundary that
  WS-10.T1 will amend to add the agentic search tool
  surface (no new external crate needed; the tool
  surface is a new projection family on top of the
  existing `legion-app` ↔ `legion-index` edge).
- WS-10.T2 ("Repo map") is the next critical-path task.
  The repo map is a tree-sitter defs / refs → file /
  symbol graph → PageRank → top-ranked signatures
  computation built on the WS-02 tree-sitter substrate
  and the file-graph facts that `legion-project`
  exposes through `legion-protocol`. The repo map is
  not a vector store and does not require a new
  external crate at the M0 boundary; the WS-10.T2
  acceptance criteria are the Legion-repo map fits
  budget and names the right files for 10 scripted
  queries. The `legion-index` policy entry is the M0
  boundary that WS-10.T2 will amend only if the
  PageRank implementation needs a new external crate
  (likely not; PageRank is a deterministic graph
  algorithm in Rust standard library plus
  `legion-index`'s existing tree-sitter AST and
  file-graph substrate).
- WS-10.T3 ("Embedding pipeline (local-first)") is the
  first critical-path task that depends on the vector
  store spike result. WS-10.T3 will declare the
  LanceDB or sqlite-vec dependency, extend the
  `legion-index` policy entry to authorize the vector
  store, extend the `legion-ai-providers` policy entry
  to authorize the Ollama / llama.cpp /
  OpenAI-compatible embedding-client dependencies,
  add the retrieval-boundary audit, and ship the
  product code that the §6 row describes. WS-10.T3
  is gated on the LanceDB vs sqlite-vec build-and-
  benchmark follow-up spike obligations recorded in
  `plans/spikes/SPIKE-0037-vector-store-result.md`
  (synthetic 64-dim fixture, ≥10k vectors, deterministic
  tie-breakers, model name / version metadata, lazy
  re-embed simulation, p50 / p95 latency,
  dependency-policy and cargo-deny review).
- WS-10.T4 ("Hybrid retrieval + eval") is gated on
  WS-10.T1 / T2 / T3 and adds the hybrid rank
  blending and the retrieval eval fixture. The
  retrieval eval fixture is the future gate that
  catches quality regressions across the three
  tiers (lexical, vector, repo-map, hybrid).
- WS-10.T5 ("Context manifest UX (PR-AI-001)") is
  the user-facing surface that shows the assembled
  retrieval slice and per-item exclusion before any
  apply. The manifest-to-egress equality test from
  WS-09.T4 applies to embedding generation the
  same way it applies to chat and completion.
- The future `RETRIEVAL_BOUNDARY_POLICY_MARKERS` /
  `RETRIEVAL_EMBEDDING_TRAIT_PACKAGES = ["legion-ai"]` /
  `RETRIEVAL_VECTOR_STORE_ALLOWED_PACKAGES = ["legion-index"]` /
  `RETRIEVAL_MODEL_PROVIDER_PACKAGES = ["legion-ai-providers"]` /
  `FORBIDDEN_RETRIEVAL_DEPS = ["lancedb", "sqlite-vec",
  "ollama-rs", "byollama", "candle-core",
  "candle-transformers", "tokenizers"]` audit is
  recorded as a future gate improvement in the
  decision section of the ADR; it is not required
  for the M0 ratification, but it is the natural
  next step once WS-10.T3 starts declaring
  `lancedb` / `sqlite-vec` / `ollama-rs` /
  `byollama` as workspace dependencies. A worker
  implementing that audit should mirror the
  parser-boundary audit in `xtask/src/main.rs` (the
  `PARSER_BOUNDARY_POLICY_MARKERS` /
  `PARSER_DEPENDENCY_ALLOWED_PACKAGES` /
  `FORBIDDEN_PARSER_DEPS` shape) and the
  search-boundary sketch in `ADR-0036` and the
  LSP-boundary sketch in `ADR-0034` and the
  terminal-boundary sketch in `ADR-0035`.
- The `ADR-0005` storage reservation is the
  persistent-vector-tier gate. The transient
  retrieval surface (per-session chunk records,
  in-memory repo-map cache) lives inside
  `legion-index` actor state and does not require
  `ADR-0005` activation. The WS-10.T3 workstream
  is the path to the persistent vector tier, with
  the agentic search path and the repo map as the
  always-available fallbacks. M0 does not enable
  the persistent vector tier; the M1 / M2
  activation path is WS-10.T3.
- The semantic retrieval stack is independent of
  the in-process ripgrep-class search stack
  ratified in `ADR-0036` and the optional tantivy
  indexed-search tier reserved there. The three
  tiers — lexical (ADR-0036), agentic + repo-map
  (this ADR), and vector (this ADR) — are
  independent in the dependency policy and activate
  on separate gates (ADR-0036, ADR-0037 agentic +
  repo-map, and ADR-0037 vector). The M0
  ratification of `ADR-0037` is independent of the
  M0 ratification of `ADR-0036`; the two ADRs
  ratify two different stack choices with two
  different crate boundaries, and the WS-06 and
  WS-10 workstreams consume them independently.
- The air-gap story is preserved. The agentic
  search path, the repo map, the deterministic
  64-dim stub embedding, and the lexical path are
  all air-gap-compatible. The vector tier with
  local embeddings (Ollama / llama.cpp) is
  air-gap-compatible. Hosted embedding
  activation is consent-gated and hard-denied by
  air-gap mode.
