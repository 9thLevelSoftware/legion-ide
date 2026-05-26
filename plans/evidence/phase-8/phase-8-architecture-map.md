# Phase 8 Architecture Map

## Acceptance Status

- Phase 8 acceptance: Not accepted.

Runtime surface status: Production-gated Phase 8 runtime surfaces now include a rustls/tokio outbound TLS/mTLS carrier, native PTY platform boundary evidence, rustls-only hosted HTTP telemetry exporter, OS-keyring raw-source key-provider, KMS envelope-provider contract, and hosted encrypted raw-source export linkage. Phase 8 GA remains deferred until release signoff is archived and the acceptance status is flipped.

This document is Phase 8 implementation evidence. It is not final GA acceptance evidence until release signoff changes the acceptance status.

## Scope

Phase 8 remains future-gated for final GA release acceptance, not for the primary implementation slices. ADR-0025 through ADR-0029 now record production implementation direction, and the current repository records protocol contracts, default-deny policy surfaces, metadata-only storage/observability paths, deterministic fixture crates, and production-gated runtime adapters. Production activation still requires release signoff and the acceptance flip.

## Boundary Summary

- Production remote transport core is typed-envelope-only, flow-controlled, replay-aware, package-gated, and cannot mutate workspace, editor, UI, or disk state directly. The outbound carrier uses rustls/tokio with TLS/mTLS credential references and metadata-only diagnostics.
- Standalone terminal runtime is policy-gated and platform-composed through the PTY boundary. Windows uses ConPTY process attachment and isolated standard handles, Unix uses PTY/process-group plumbing through `nix`, and runtime sessions expose spawn/input/resize/output/close/kill/orphan cleanup while keeping non-native/degraded sessions distinguishable in metadata.
- Hosted telemetry has a durable local metadata spool/exporter path plus a rustls-only reqwest HTTP exporter and remains disabled by default and denied by air-gap/non-allowlisted endpoint policy.
- Raw-source retention has a ChaCha20-Poly1305 AEAD file-backed vault path, OS-keyring key-provider support, a KMS envelope-provider contract, and hosted encrypted-bundle export linkage. It remains disabled by default with explicit scoped local and hosted consent required before capture/export.
- Deterministic fixture crates are active for contract validation only: `devil-remote-transport`, `devil-terminal`, `devil-telemetry`, and `devil-retention`.
- Normal observability, storage, remote, terminal, telemetry, plugin, AI, and collaboration metadata continue to reject raw source, raw transcripts, process output, transport payload bodies, prompts, provider payloads, and secrets by default.

## Ownership And Activation Gates

| Area | Owner role | Activation gate | Current posture |
| --- | --- | --- | --- |
| Remote transport | Remote runtime owner | `remote.transport.connect`, endpoint allowlist, credential reference, schema compatibility, replay/duplicate defense, agent package manifest, and proposal-mediated mutation evidence. | Default-off production core plus rustls/tokio TLS/mTLS carrier implemented and tested. |
| Terminal/PTTY | Platform runtime owner | `terminal.launch`, `terminal.input`, `terminal.resize`, `terminal.close`, `terminal.kill`, trusted workspace, cwd/shell/env policy, bounded output, cleanup/orphan evidence. | Native platform boundary implemented with Windows ConPTY session lifecycle and Unix PTY/process-group lifecycle path; final CI matrix evidence is archived. |
| Hosted telemetry | Privacy and observability owner | `telemetry.export.hosted`, explicit consent, category/endpoint allowlist, air-gap denial, durable bounded spool, classifier audit, retry/drop evidence. | Durable metadata spool/exporter plus rustls-only HTTP exporter implemented and tested. |
| Raw-source retention | Security/privacy owner | `retention.raw_source.capture/read/delete/export.hosted`, scoped consent, TTL, max bytes, path scope, AEAD vault, audit, delete/revoke evidence, key-provider review, and recovery drills. | AEAD file-backed vault, OS-keyring provider, KMS envelope contract, hosted encrypted export linkage, local key rotation, and metadata-only recovery reports implemented and tested. |
| Storage migrations | Storage owner | Explicit registry, dry-run, backup, checksum, recovery, quarantine, replay evidence, explicit repair flags. | Metadata registry, dry-run, backup, checksum, and recovery implemented and tested. |
| Release operations | Release owner | Full artifact set, archived gates, platform matrix, fault/performance drills, cargo-deny review, rollback/canary/incident signoff. | Not accepted. |

## Dependency And Authority Map

- Phase 8 runtime crates may depend only on declared protocol/security/platform/storage/observability boundaries in `plans/dependency-policy.md`.
- `devil-ui` remains projection-only and may not own Phase 8 runtime sessions, transports, PTYs, telemetry spools, retention vaults, storage migrations, or mutation authority.
- `devil-app` may compose accepted runtime surfaces only through protocol DTOs/ports after the relevant acceptance gates pass.
- File/editor mutations from remote, terminal, retention, telemetry, diagnostics, plugin, AI, or collaboration surfaces must remain proposal-mediated with existing workspace/editor preconditions.
- Air-gap policy denies hosted telemetry, hosted raw-source export, hosted providers, update checks, and non-loopback remote transport.

## Expected Evidence Artifacts

- `phase-8-architecture-map.md`
- `phase-8-threat-model.md`
- `dependency-boundary.txt`
- `protocol-dto-contract-tests.txt`
- `remote-production-transport-security-tests.txt`
- `remote-agent-packaging-tests.txt`
- `terminal-runtime-policy-tests.txt`
- `terminal-pty-platform-tests.txt`
- `hosted-telemetry-consent-policy-tests.txt`
- `hosted-telemetry-failure-mode-tests.txt`
- `privacy-redaction-classifier-audit.md`
- `raw-source-retention-policy-tests.txt`
- `raw-source-retention-lifecycle-tests.txt`
- `storage-migration-recovery-tests.txt`
- `operational-health-diagnostics.txt`
- `enterprise-policy-profile-ci.txt`
- `performance-budget-tests.txt`
- `metadata-replay-drills.txt`
- `fault-drill-results.txt`
- `platform-matrix-evidence.txt`
- `release-readiness-review.md`
- `cargo-fmt-check.txt`
- `cargo-check-workspace-all-targets.txt`
- `cargo-test-workspace-all-targets.txt`
- `cargo-clippy-workspace-all-targets.txt`
- `cargo-deny-check.txt`
- `xtask-check-deps.txt`

## Final Validation Checklist

- [x] Phase 8 ADRs are accepted for production implementation direction.
- [x] Dependency policy and `xtask` are aligned for active runtime crates.
- [x] Protocol DTO contract tests pass for all Phase 8 contracts in this implementation slice.
- [x] Security, privacy, storage, migration, fault-injection, and ownership tests pass for this implementation slice.
- [x] Full workspace gates pass and outputs are archived.
- [x] Linux, Windows, and macOS CI matrix evidence is archived after the production runtime dependency rebaseline.
- [ ] Release readiness signoff is updated after the final matrix run.
