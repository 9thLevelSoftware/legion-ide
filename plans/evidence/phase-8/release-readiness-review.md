# Phase 8 Release Readiness Review

Status: implementation evidence, platform matrix evidence, and final GA signoff are archived for Phase 8 acceptance.

Latest archived CI matrix:
- Run URL: https://github.com/9thLevelSoftware/devil-ide/actions/runs/26470308103
- Head SHA: b3ca8f8efe9f4e68bf55bbfd098512e4bc0ead22
- Completed: 2026-05-26T19:34:19Z
- Matrix: ubuntu-latest, windows-latest, and macos-latest passed.

Validated commands:
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny check`
- `cargo run -p devil-cli -- evidence check --phase phase8`
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

Final signoff checklist:
- Signoff date: 2026-05-26
- Security signoff: Complete.
- Privacy signoff: Complete.
- Operations signoff: Complete.
- Rollback signoff: Complete.
- Canary signoff: Complete.
- Incident response signoff: Complete.
- Supply-chain signoff: Complete.

Release-owner approval:
- Phase 8 GA signoff was provided by the release owner in the active Codex thread on 2026-05-26 after review of the archived implementation evidence, platform matrix evidence, PR review state, and final gate requirements.
