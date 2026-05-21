# P2.2 Snapshot Lease Consumer Contract Evidence

## Scope

This note records the P2.2 integration slice for snapshot lease consumer contracts from the granular control-first implementation plan. It covers descriptor-first and chunk-bounded reads only. It does not accept a broader phase and does not start P3 semantic-index remediation.

## Contract coverage

- Snapshot leases now include buffer identity, snapshot identity, buffer version, consumer kind, expiry, chunk count, and schema version so consumers can validate the lease before using bounded text.
- Consumers read text through `SnapshotLeaseChunk`, which carries the authorizing lease descriptor, a chunk descriptor, a bounded chunk text payload, and schema version.
- Consumers must treat `SnapshotLeaseExpired` as stale work and reacquire from the current snapshot rather than applying prior results.
- Consumers must treat `SnapshotLeaseStale` as stale work when the lease buffer identity, snapshot id, or buffer version differs from the consumer's expected current identity.
- Large-file full-text compatibility remains denied; bounded lease chunk reads remain available.

## Consumer ownership restrictions

Semantic, LSP, AI context, and proposal preview consumers may not own editor text, editor sessions, workspace actors, durable source bodies, prompt bodies, provider payloads, or full diff bodies. They must use hashes, ranges, snapshot descriptors, chunk descriptors, bounded chunks, freshness metadata, and stale/resynchronization outcomes. Future semantic, LSP, AI, agent, plugin, terminal, remote, or collaboration runtime activation remains outside this slice.

## Tests added or strengthened

- `snapshot_lease_consumer_reads_valid_bounded_chunk_by_descriptor` verifies a valid lease authorizes a bounded chunk read with lease and chunk descriptors.
- `snapshot_lease_consumer_must_resynchronize_after_expiry` verifies expired leases are rejected explicitly.
- `snapshot_lease_consumer_must_resynchronize_on_stale_snapshot_id` verifies consumers cannot apply leased work against a newer current snapshot id/version.
- `snapshot_lease_large_file_denies_full_text_but_allows_bounded_chunks` verifies large-file full text remains denied while chunk reads stay bounded.
- `snapshot_leases_are_descriptor_only_for_all_consumer_kinds` now asserts buffer id, buffer version, and lease schema version.
- `dto_contracts_snapshot_lease_descriptor_golden_and_required_fields` now makes buffer id and buffer version required lease DTO fields.

## Validation

- `cargo test -p devil-editor snapshot_lease --lib && cargo test -p devil-protocol dto_contracts_snapshot_lease_descriptor_golden_and_required_fields --test dto_contracts` passed locally.
- `cargo run -p xtask -- check-deps` passed locally.
- `cargo fmt --all --check` initially reported formatting drift in `crates/devil-editor/src/lib.rs`; `cargo fmt --all` was run, then `cargo fmt --all --check` passed.
- `cargo check --workspace --all-targets` passed locally.
- `cargo test --workspace --all-targets` passed locally.
- `cargo clippy --workspace --all-targets -- -D warnings` passed locally.
- `cargo deny check` was attempted but unavailable in this environment (`cargo` reported no such command: `deny`).

## P2.2-only confirmation

This slice added descriptor/chunk/lease contract helpers, DTO fields, contract tests, and this evidence note only. It did not implement P3 semantic-index boundary remediation, LSP, AI, agent, plugin, terminal, remote, collaboration runtime behavior, alternate write paths, or direct UI-to-app/editor/project/storage dependencies.
