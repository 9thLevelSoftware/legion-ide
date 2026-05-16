# ADR-0005: Select Local Metadata, Lexical Index, and Vector Store Backends

## Status
Accepted with reservations — SQLite/Tantivy metadata baseline accepted; vector-store selection and durable semantic/tracker/memory/plugin/collaboration/replay storage require follow-up ADR.

## Context
The IDE must store tracker state, search indexes, embedding vectors, and parse artifacts locally. The storage choices affect query latency, incremental update performance, and memory footprint on large repositories.

## Decision
- Metadata and tracker: SQLite via `devil-storage` wrapper with migrations.
- Lexical search: Tantivy or equivalent Rust-native inverted index.
- Vector search: Evaluate LanceDB, Qdrant embedded mode, sqlite-vec, and internal HNSW through Spike 3.
- Parse artifacts: Content-addressed blob cache on filesystem.

## Consequences
- **Positive**: SQLite and Tantivy are mature, well-maintained Rust ecosystem choices.
- **Positive**: Separating storage concerns allows backend swaps without feature crate changes.
- **Negative**: Vector store selection is immature and requires benchmark proof.
- **Negative**: Multiple storage backends increase operational complexity for backup and corruption recovery.
