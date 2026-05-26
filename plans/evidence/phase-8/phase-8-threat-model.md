# Phase 8 Threat Model

Status: initial implementation-gate evidence; not GA acceptance evidence.

## Scope

This threat model covers the Phase 8 production activation surfaces while they remain default-off: remote network transport, native terminal/PTTY, hosted telemetry egress, raw-source retention, storage migrations, diagnostics, supply chain, and release operations.

## Assets

- Workspace source, editor buffers, save/proposal preconditions, and file fingerprints.
- Principal identity, capability decisions, endpoint policy, consent grants, and trust state.
- Remote transport envelopes, operation sequencing, resume metadata, and agent package manifests.
- Terminal lifecycle metadata, process identifiers, bounded redacted output projections, and cleanup state.
- Telemetry spool metadata, hosted export batches, retry/drop state, and consent revocations.
- Raw-source retention descriptors, encrypted vault metadata, key references, leases, audits, and tombstones.
- Storage schema manifests, migration backups, checksums, recovery markers, diagnostics, and evidence archives.

## Threats And Mitigations

| Surface | Threat | Required mitigation | Current status |
| --- | --- | --- | --- |
| Remote transport | Unauthorized endpoint, replay, tamper, downgrade, duplicate frame, causal gap, or payload persistence. | Typed-envelope-only protocol, endpoint allowlist, credential reference validation, trusted workspace/principal/capability checks, schema compatibility checks, event sequence validation, duplicate detection, metadata-only audit. | Default-off protocol core plus rustls/tokio outbound TLS/mTLS carrier implemented; final matrix evidence remains required. |
| Remote agent package | Wrong authority, incompatible agent, compromised package, failed rollback. | Manifest integrity, authority binding, compatibility result, startup health, shutdown, upgrade and rollback checks before activation. | Contract planning only; production activation deferred. |
| Terminal/PTTY | Untrusted launch, cwd escape, shell/env abuse, transcript persistence, output flood, orphan process, input after exit. | Default-deny terminal capabilities, trusted workspace, allowed shell/env/cwd policy, bounded redacted output, lifecycle state machine, kill tree and cleanup evidence. | Security broker denies runtime by default; Windows ConPTY lifecycle and Unix PTY/process-group lifecycle paths implemented with final matrix evidence pending. |
| Hosted telemetry | Silent egress, raw source or secrets in telemetry, consent bypass, spool growth, exporter blocking editor operations. | Explicit consent, category/endpoint allowlist, air-gap denial, classifier rejection of raw markers, durable bounded spool, retry/drop summaries, non-blocking export. | Durable metadata spool plus rustls-only hosted HTTP exporter implemented behind default-off policy. |
| Raw-source retention | Raw capture without consent, out-of-scope path, over-retention, unauthorized read/export, deletion failure, hosted raw upload in air-gap. | Scoped consent, purpose/TTL/max-byte/path enforcement, AEAD encryption metadata, key references, access audit, tombstone/delete/revoke lifecycle, local key rotation/recovery reports, air-gap hosted export denial. | ChaCha20-Poly1305 file vault, OS-keyring provider, KMS envelope contract, hosted encrypted export linkage, local key rotation, and metadata-only recovery reports are implemented behind default-deny policy. |
| Storage migrations | Corrupt migration, downgrade, interrupted write, backup loss, raw source in diagnostics. | Explicit migration registry, dry-run, backup, checksum, roll-forward/recover, quarantine, metadata-only replay, default-deny apply/repair capabilities. | Metadata contracts and default-deny repair/apply policy exist; production registry/recovery deferred. |
| Diagnostics/evidence | Repair command mutates state unexpectedly or evidence overclaims unsupported platforms. | Read-only diagnostics by default, explicit repair flags, artifact checks, final checklist enforcement, no acceptance with scaffold disclaimer. | `xtask` acceptance governance active. |
| Supply chain | Network/crypto/process dependency risk. | Dependency policy review, `cargo deny check`, ADR-approved production dependency additions only. | Production dependency rebaseline is reflected in dependency policy and local cargo-deny evidence; final matrix archive remains required. |
| Release operations | GA flip before evidence, rollback gap, incident response gap. | Gate G0-G10 ordering, archived full commands, release readiness review, rollback/canary/incident signoff, residual risk owners. | Not accepted. |

## Owners

| Area | Owner role |
| --- | --- |
| Remote transport | Remote runtime owner |
| Terminal/PTTY | Platform runtime owner |
| Telemetry/privacy | Privacy and observability owner |
| Raw-source retention | Security/privacy owner |
| Storage/migrations | Storage owner |
| Security policy | Security owner |
| Platform QA | Platform QA owner |
| Release operations | Release owner |

## Open Risks

- Remote transport has default-off rustls/tokio outbound TLS/mTLS carrier coverage, but still needs fresh Linux/Windows/macOS matrix evidence before GA acceptance.
- Native PTY lifecycle now has Windows ConPTY session lifecycle and Unix PTY/process-group coverage, but kill/orphan parity evidence must come from the final platform matrix before GA acceptance.
- Hosted telemetry has a rustls-only HTTP exporter and durable spool coverage, but still needs final hosted-export gate output and release evidence archived.
- Raw-source vault AEAD sealing, scoped consent, TTL purge, delete tests, local key rotation, OS-keyring provider, KMS provider contract, hosted encrypted export linkage, and metadata-only recovery reports are implemented; final platform/signoff evidence remains open.
- Storage migrations now have metadata contracts and default-deny apply/repair capability policy, but still lack a production registry, durable dry-run/apply implementation, backup/recover execution, and interruption drill evidence.
- Phase 8 remains `Not accepted` until final Linux/Windows/macOS matrix evidence, full gate output archive, and release readiness signoff are recorded.
