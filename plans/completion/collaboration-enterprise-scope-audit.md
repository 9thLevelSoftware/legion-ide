# Collaboration and enterprise scope audit

Date: 2026-09-05

This audit expands `COMP-SCOPE-FAMILY-19` from baseline `07f068d`. It uses
the approved family entry in `docs/superpowers/specs/2026-09-04-product-completion-design.md`,
the master and AI/team completion plans, the production qualification plan,
current collaboration/security/retention source, and retained evidence. Every
source path in the replacement rows is a literal repository path. Historical
readiness claims are not treated as product acceptance.

| ID | Outcome boundary | Current trace and classification | Product limit |
| --- | --- | --- | --- |
| COMP-COLLAB-001 | Finite collaboration/enterprise identity and policy matrix with executable oracles | The approved design, master plan S0-02, and qualification plan require the matrix; no current matrix or real endpoint contract was found. **absent** | This is an S0-02 prerequisite; supported IdP, service, transport, tenant, retention, and recovery entries remain unassessed. |
| COMP-COLLAB-002 | Tenant/workspace/document/participant identity, roles, permissions, policy epochs, and degraded/revoked behavior | `crates/legion-protocol/src/lib.rs` and `crates/legion-collaboration/src/lib.rs` provide bounded session, participant, permission, presence, operation, replay, and audit DTO/runtime behavior; ADR-0021 defines identity/permission/retention boundaries. **partial** | No enterprise subject, tenant-backed identity provider, signed epoch contract, or packaged identity workflow exists. |
| COMP-COLLAB-003 | Authenticated deployable collaboration control plane, desktop transport, durable append/checkpoint storage | `CollaborationSessionRuntime` and transport-envelope handling in `crates/legion-collaboration/src/lib.rs` are in-process; phase-6 architecture evidence explicitly describes the local deterministic substrate. **partial** | No independent server binary, authenticated network transport, durable operation store, or desktop service connection exists. |
| COMP-COLLAB-004 | Ordered/replay-safe delivery, reconnect, restart, backup/restore, health, cancellation, and bounded shutdown | Current runtime implements duplicate suppression, causal gaps, replay manifests, reconnect state, and shutdown; `plans/evidence/phase-6/disconnect-reconnect-replay-tests.txt` records deterministic harness evidence. **partial** | No multi-process service restart, durable checkpoint restore, backup/restore, listener cleanup, or real-client recovery evidence. |
| COMP-COLLAB-005 | Concurrent participant convergence, presence, offline edits, reconciliation, identity removal, and restart | `crates/legion-collaboration/src/lib.rs` has deterministic operation ordering/replay and presence; `plans/evidence/phase-6/collaboration-convergence-tests.txt` covers local convergence behavior. **partial** | No ratified CRDT service or independently launched participants with externally observed byte-for-byte convergence. |
| COMP-COLLAB-006 | Shared proposal review/quorum/revocation/conflict handling with proposal-mediated apply | App composition and security code provide capability-gated shared proposal checks; `plans/evidence/phase-6/shared-proposal-approval-tests.txt` records current app-owned gates. **partial** | No durable multi-user proposal service, native concurrent review workflow, or real service-to-workspace apply evidence. |
| COMP-ENT-001 | OIDC SSO login/logout/refresh, token validation, revocation, and recovery | No OIDC identity service or product route is present; current security policy is a capability boundary only. **absent** | No real IdP endpoint, keyring-backed session lifecycle, or packaged account workflow. |
| COMP-ENT-002 | Authenticated tenant-bound SCIM user/group lifecycle and standards profile | No SCIM API, identity module, or server control-plane endpoint is present. **absent** | User/group create/read/replace/patch/delete, filters, pagination, idempotency, and revocation remain unassessed. |
| COMP-ENT-003 | Signed policy distribution/enforcement across AI, extensions, remote, collaboration, telemetry, retention, and export | `crates/legion-security/src/policy.rs` and `plans/evidence/production/P9-F2-T3-signed-policy-bundles.md` show signed org-policy and mode-ceiling substrate. **partial** | No collaboration-service distribution, tenant effective snapshot, offline expiry/downgrade handling, or full Stage-5 enforcement evidence. |
| COMP-ENT-004 | Tenant-scoped audit export and retention/deletion with metadata-safe evidence | `crates/legion-retention/src/lib.rs` provides retention/vault controls; phase-6 storage/observability evidence proves metadata-only collaboration audit paths. **partial** | No enterprise audit export service or externally verified tenant retention/deletion workflow. |
| COMP-ENT-005 | Deployable collaboration/enterprise service operations and artifact provenance | S5-14 defines the required package, config, migration, health, backup, restore, rollback, uninstall, provenance, and SBOM workflow; no deployable service artifacts are present. **absent** | No service package, production configuration validation, compatibility manifest, or operations drill exists. |
| COMP-ENT-006 | Packaged Stage-5 acceptance across concurrent collaboration, IdP/SCIM/policy, audit/retention, operations, interruption, and recovery | S5-15 and production qualification define the required real endpoint and externally observed acceptance; no accepted packaged EvidenceRun was found. **absent** | Fixture, loopback, metadata-only, or headless evidence cannot close this row; all acceptance remains unassessed. |

## Exact deduplication boundaries

The two retained internal protection links that formerly pointed at the
aggregate family are rewired as follows: `COMP-P9-F2-T1-1` (security model and
sandbox caveats) protects `COMP-ENT-003` (enterprise policy distribution and
enforcement), while `COMP-P9-F2-T4-1` (external audit before a strong enterprise
claim) protects `COMP-ENT-006` (packaged Stage-5 enterprise qualification).
These are the only retained-object field changes authorized for referential
integrity after aggregate replacement.

`COMP-COLLAB-006` depends on `COMP-TRUST-001`, `COMP-TRUST-002`,
`COMP-TRUST-004`, and `COMP-PRES-008`; proposal lifecycle, authenticated
validation, audit-before-success, and dirty-buffer conflict preservation remain
owned by those canonical rows. `COMP-ENT-003` depends on `COMP-TRUST-005` for
projection and Manual zero-egress semantics. The collaboration rows add only
tenant/service/concurrent-session authority and do not duplicate local save,
language, terminal, Git, provider, context, extension, remote, or training /
telemetry outcomes.

## Evidence limits

Phase-6 evidence establishes a deterministic local runtime, metadata-only audit
and storage paths, capability policy, and projection boundaries. It does not
establish authenticated durable multi-user service behavior, OIDC/SCIM, tenant
isolation, deployable service operations, or packaged Stage-5 acceptance. The
older deferred-surface language is recorded as a current gap classification;
the approved Stage-5 plans remain the required target and are not converted
into permanent deferrals.
