# Vector Deferral Audit

Date: 2026-05-24

Accepted deferral findings:

- No `legion-index` dependency on a vector database, embedding library, AI provider, model provider, or retrieval crate exists.
- No `legion-index` code path computes embeddings or stores vector records.
- `SemanticModelVersion` is present only as cache and freshness metadata for future invalidation compatibility.
- Semantic query APIs use lexical, graph, metadata, and LSP DTO records only.
- `legion-agent`, `legion-memory`, `legion-tracker`, AI provider, plugin, remote, terminal, and collaboration runtime surfaces remain outside accepted Phase 3 activation.
- Future vector activation still requires a separate accepted ADR, dependency-policy update, syntax-aware chunking contract, provenance contract, privacy-scope contract, model-identity contract, invalidation contract, storage-retention decision, and contract-test suite.
