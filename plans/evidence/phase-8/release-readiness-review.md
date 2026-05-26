# Phase 8 Release Readiness Review

Status: implementation evidence and platform matrix evidence are archived; final GA signoff still pending release-owner approval.

Validated commands:
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny check`
- `cargo run -p xtask -- check-deps`

Implemented release controls:
- Phase 8 runtimes are explicit-config and default-deny.
- Security policy keeps hosted egress and storage repair gated.
- Metadata-only validators reject raw payload markers before persistence/export.
- Raw-source vault uses an approved AEAD dependency chain (`chacha20poly1305`, `rand_core`, `sha2`, `zeroize`) with metadata-only envelope evidence.
- Remote transport now includes a rustls/tokio outbound TLS/mTLS carrier with credential references and metadata-only diagnostics.
- Terminal platform boundary now records native PTY evidence separately from degraded process-backed sessions; Windows validates ConPTY availability and Unix PTY plumbing is implemented behind `nix`.
- Hosted telemetry now includes a rustls-only reqwest HTTP exporter over the durable metadata spool.
- Raw-source vault now records local key rotation, recovery-report drill coverage, OS-keyring key-provider metadata, KMS envelope-provider conformance, and hosted encrypted raw export linkage.
- Cargo-deny completed with warning-level duplicate dependency findings only after reviewing the Phase 8 rustls/keyring dependency graph and allowing `ISC` plus `CDLA-Permissive-2.0` in addition to the existing AEAD license baseline.

Remaining GA blockers:
- Review the archived Linux, Windows, and macOS CI matrix run for release approval.
- Review the archived full final gate command set for release approval.
- Record explicit release readiness signoff for security, privacy, operations, rollback, canary, incident response, and supply-chain review before flipping Phase 8 acceptance.

Final signoff checklist:
- Signoff date: pending release approval.
- Security signoff: Pending release approval.
- Privacy signoff: Pending release approval.
- Operations signoff: Pending release approval.
- Rollback signoff: Pending release approval.
- Canary signoff: Pending release approval.
- Incident response signoff: Pending release approval.
- Supply-chain signoff: Pending release approval.
