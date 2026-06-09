# Privacy Redaction Audit

Date: 2026-05-24

Commands:

- `cargo test -p legion-storage --all-targets` passed with 16 tests.
- `cargo test -p legion-protocol --test dto_contracts` passed with 57 tests.
- `cargo test -p legion-index --all-targets` passed with 28 tests.
- `cargo test -p legion-app --test workspace_vfs_integration` passed with 54 tests.

Accepted privacy behavior:

- Semantic metadata storage rejects stale/freshness-mismatched records and tombstones privacy revocations before query exposure.
- Large descriptor records persist hashes, ranges, descriptor metadata, and chunk references without source bodies.
- LSP diagnostics and capability summaries store counts, ranges, hashes, statuses, and redaction hints without raw diagnostic messages or source bodies.
- Proposal audit records redact sensitive edit payloads and preserve metadata-only lifecycle evidence.
- Semantic query DTOs expose labels only when the privacy scope permits display names; metadata-only privacy downgrades remove display labels.
- Evidence artifacts avoid raw source snapshots, embeddings, provider outputs, command output bodies beyond validation summaries, and full diff payloads.
