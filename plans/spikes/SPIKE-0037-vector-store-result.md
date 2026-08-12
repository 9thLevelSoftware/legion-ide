# SPIKE-0037: Vector Store Choice for ADR-0037

- Status: Draft for M0 ratification
- Date: 2026-06-09
- Parent: ADR-0037, WS-10.T3, M0 exit criteria
- Scope: decide the first embedded vector-store candidate for Legion's semantic retrieval enhancement layer without activating vector storage in product code.

## Context

ADR-0037 makes agentic search and the deterministic repo map the default retrieval path. Vector retrieval is an enhancement layer: AST-aware chunks, local embeddings by default, hosted embeddings only with explicit consent, model name/version recorded per index, and lazy re-embedding after model changes.

The current repository intentionally defers vector activation. `legion-ai` exposes embedding DTOs, but provider-backed embedding generation and persistent vector storage are not activated. `legion-index` has no vector-store dependency today, and dependency-policy changes are required before adding one.

## Spike constraints

1. No product runtime activation in M0.
2. No hosted embedding dependency.
3. No new dependency without a matching dependency-policy update and phase-gate evidence.
4. Store metadata next to vectors: workspace, language, snapshot/file identity, chunk identity, model name, model version, and schema version.
5. Support air-gap and CI/headless operation.
6. Keep `legion-ui` and editor/session ownership out of the retrieval storage layer.

## Evaluation criteria

| Criterion | Requirement |
| --- | --- |
| Deployment | Embedded/in-process; no separate service required |
| Air-gap | Fully usable with local embeddings and local storage |
| Metadata | Row-level metadata sidecar for model/version and workspace/chunk identity |
| Scale target | Viable at 10k-100k vectors for beta-sized workspaces |
| Determinism | Stable top-k ordering with tie-breakers the index layer can record |
| Dependency policy | Compatible with `deny.toml` and explicit `plans/dependency-policy.md` entries |
| Build risk | Does not materially destabilize workspace check/test/clippy gates |
| Future fit | Can support lazy re-embed and hybrid lexical/vector/repo-map fusion |

## Candidate matrix

| Candidate | Evidence | Fit | Risks | Decision |
| --- | --- | --- | --- | --- |
| LanceDB (`lancedb` crate, Apache-2.0) | Persistent Lance/Arrow-style storage; vector indexes and metadata columns; Rust crate available. | Best fit for 100k+ vectors and durable per-row metadata. Model/version metadata can live as ordinary columns. | Heavier transitive dependency set; build time and binary size must be measured; likely belongs behind a dedicated vector/index boundary rather than directly widening `legion-index`. | Primary candidate, pending build/bench proof. |
| sqlite-vec (SQLite extension, MIT) | Single-file local vector extension; colocates metadata and vectors in SQL tables. | Small operational footprint and strong air-gap story; simple metadata schema; likely easiest CI fixture. | ANN/scale story may be weaker than LanceDB; Rust binding/extension packaging needs scrutiny; may require bundled SQLite policy. | Fallback if LanceDB footprint or policy cost is too high. |
| Tantivy plus custom flat vector scan | Tantivy is already planned for search indexing; a flat vector table could be kept in Rust data structures. | Minimal new storage concept and useful for tiny fixtures. | Not a real ANN store; risks mixing search and vector concerns; poor 100k-vector scaling. | Not selected for product vector storage. |

## Draft decision

Use LanceDB as the primary candidate for the follow-up build-and-benchmark spike, with sqlite-vec reserved as the fallback if LanceDB's dependency footprint, build reliability, or policy surface exceeds the M0/M1 budget.

Do not add either dependency to product crates in this M0 artifact. The next spike must use a bounded fixture and produce concrete build, latency, recall, binary-size, and dependency-policy evidence before any runtime activation.

## Follow-up build spike obligations

The follow-up implementation spike must provide:

1. A synthetic 64-dimensional fixture matching the current deterministic embedding dimension used by the index layer.
2. At least 10k vectors; 100k vectors if runtime is reasonable on CI/reference hardware.
3. Top-k search with deterministic tie-breakers.
4. Metadata fields for model name, model version, workspace id, file id/path, chunk id, and schema version.
5. Lazy re-embed simulation: a model-version change marks stale rows without deleting unrelated metadata.
6. Reported p50/p95 insert and query latency.
7. Dependency-policy and cargo-deny review notes for the selected candidate.
8. Green gates: `check-deps`, `docs-hygiene`, fmt, check, tests, clippy, and cargo-deny when installed.

## Fallback triggers

Switch from LanceDB primary to sqlite-vec if any of these are true during the follow-up spike:

- LanceDB adds transitive dependencies that fail license/advisory policy.
- Workspace `cargo check --workspace --all-targets` or clippy becomes materially unstable.
- Build time or binary size exceeds the agreed beta budget without a feature-gated boundary.
- The metadata model requires a second sidecar store instead of row-level metadata.
- sqlite-vec meets the 10k-vector latency/recall bar with substantially lower dependency risk.

## M0 outcome

M0 records the decision matrix and the conditional primary/fallback choice. Product vector storage remains disabled until the follow-up build spike lands with dependency-policy updates and contract tests.
