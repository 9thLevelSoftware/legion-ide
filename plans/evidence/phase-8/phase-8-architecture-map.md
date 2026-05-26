# Phase 8 Architecture Map

## Acceptance Status

- Phase 8 acceptance: Not accepted.

Runtime surface status: Deterministic metadata-only fixture slice is active; production-capable transport, terminal, telemetry, retention, and storage migration cores are implemented behind default-deny gates; production transport, native terminal, hosted export, raw-source vault, and operational GA remain deferred until final release signoff.

This document is Phase 8 implementation evidence. It is not final GA acceptance evidence until release signoff changes the acceptance status.

## Scope

Phase 8 remains future-gated for production remote network transport, native local terminal runtime, hosted telemetry export, raw-source content retention, and operational hardening. ADR-0025 through ADR-0029 now record production implementation direction, and the current repository records protocol contracts, default-deny policy surfaces, metadata-only storage/observability paths, and deterministic fixture crates. Production activation must still provide runtime implementations, ownership tests, platform/security/privacy evidence, storage migration/recovery evidence, fault drills, and release evidence.

## Boundary Summary

- Production remote transport core is typed-envelope-only, flow-controlled, replay-aware, package-gated, and cannot mutate workspace, editor, UI, or disk state directly.
- Standalone terminal runtime is policy-gated and platform-composed through the PTY boundary, with degraded process-backed sessions recorded as metadata-only audit.
- Hosted telemetry has a durable local metadata spool/exporter path and remains disabled by default and denied by air-gap/non-allowlisted endpoint policy.
- Raw-source retention has an encrypted file-backed vault path and remains disabled by default with explicit scoped consent required before capture.
- Deterministic fixture crates are active for contract validation only: `devil-remote-transport`, `devil-terminal`, `devil-telemetry`, and `devil-retention`.
- Normal observability, storage, remote, terminal, telemetry, plugin, AI, and collaboration metadata continue to reject raw source, raw transcripts, process output, transport payload bodies, prompts, provider payloads, and secrets by default.

## Ownership And Activation Gates

| Area | Owner role | Activation gate | Current posture |
| --- | --- | --- | --- |
| Remote transport | Remote runtime owner | `remote.transport.connect`, endpoint allowlist, credential reference, schema compatibility, replay/duplicate defense, agent package manifest, and proposal-mediated mutation evidence. | Default-off production core implemented and tested. |
| Terminal/PTTY | Platform runtime owner | `terminal.launch`, `terminal.input`, `terminal.resize`, `terminal.close`, `terminal.kill`, trusted workspace, cwd/shell/env policy, bounded output, cleanup/orphan evidence. | Default-off process-backed degraded runtime implemented and tested. |
| Hosted telemetry | Privacy and observability owner | `telemetry.export.hosted`, explicit consent, category/endpoint allowlist, air-gap denial, durable bounded spool, classifier audit, retry/drop evidence. | Durable metadata spool/exporter implemented and tested. |
| Raw-source retention | Security/privacy owner | `retention.raw_source.capture/read/delete/export.hosted`, scoped consent, TTL, max bytes, path scope, encrypted vault, audit, delete/revoke evidence. | Encrypted file-backed vault implemented and tested. |
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
