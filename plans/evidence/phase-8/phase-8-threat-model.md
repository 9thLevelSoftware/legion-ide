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
| Remote transport | Unauthorized endpoint, replay, tamper, downgrade, duplicate frame, causal gap, or payload persistence. | Typed-envelope-only protocol, endpoint allowlist, credential reference validation, trusted workspace/principal/capability checks, schema compatibility checks, event sequence validation, duplicate detection, metadata-only audit. | Fixture and protocol validation only; production backend deferred. |
| Remote agent package | Wrong authority, incompatible agent, compromised package, failed rollback. | Manifest integrity, authority binding, compatibility result, startup health, shutdown, upgrade and rollback checks before activation. | Contract planning only; production activation deferred. |
| Terminal/PTTY | Untrusted launch, cwd escape, shell/env abuse, transcript persistence, output flood, orphan process, input after exit. | Default-deny terminal capabilities, trusted workspace, allowed shell/env/cwd policy, bounded redacted output, lifecycle state machine, kill tree and cleanup evidence. | Security broker denies runtime by default; native PTY deferred. |
| Hosted telemetry | Silent egress, raw source or secrets in telemetry, consent bypass, spool growth, exporter blocking editor operations. | Explicit consent, category/endpoint allowlist, air-gap denial, classifier rejection of raw markers, durable bounded spool, retry/drop summaries, non-blocking export. | Metadata validators and default-deny policy only; hosted exporter deferred. |
| Raw-source retention | Raw capture without consent, out-of-scope path, over-retention, unauthorized read/export, deletion failure, hosted raw upload in air-gap. | Scoped consent, purpose/TTL/max-byte/path enforcement, encryption metadata, key references, access audit, tombstone/delete/revoke lifecycle, air-gap hosted export denial. | Descriptor and policy contracts only; encrypted vault deferred. |
| Storage migrations | Corrupt migration, downgrade, interrupted write, backup loss, raw source in diagnostics. | Explicit migration registry, dry-run, backup, checksum, roll-forward/recover, quarantine, metadata-only replay, default-deny apply/repair capabilities. | Metadata contracts and default-deny repair/apply policy exist; production registry/recovery deferred. |
| Diagnostics/evidence | Repair command mutates state unexpectedly or evidence overclaims unsupported platforms. | Read-only diagnostics by default, explicit repair flags, artifact checks, final checklist enforcement, no acceptance with scaffold disclaimer. | `xtask` acceptance governance active. |
| Supply chain | Network/crypto/process dependency risk. | Dependency policy review, `cargo deny check`, ADR-approved production dependency additions only. | Fixture dependency policy active; production additions deferred. |
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

- Production network transport has no backend, TLS/mTLS decision, endpoint policy runtime, reconnect/resume implementation, or platform evidence yet.
- Native PTY lifecycle, kill-tree cleanup, Windows ConPTY and Unix PTY parity evidence are not implemented.
- Hosted telemetry lacks durable spool, hosted exporter, retry/drop semantics, consent purge, and operations evidence.
- Raw-source vault encryption, key management, TTL scanner, restore, and deletion guarantees are not implemented.
- Storage migrations now have metadata contracts and default-deny apply/repair capability policy, but still lack a production registry, durable dry-run/apply implementation, backup/recover execution, and interruption drill evidence.
- Phase 8 remains `Not accepted` until these risks are closed or explicitly reserved in release readiness evidence.
