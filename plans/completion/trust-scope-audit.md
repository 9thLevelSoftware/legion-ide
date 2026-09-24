# S0-01s trust/proposal scope audit

This inventory replaces only `COMP-SCOPE-FAMILY-16` with six atomic product rows,
`COMP-TRUST-001..006`. The approved family is **Trust/proposals**: a generalized
proposal lifecycle, approvals and policy, audit, cancellation, rollback, secret
handling, privacy/egress visibility, and native qualification. Every replacement
row remains `acceptance: unassessed`; `implementation` is a conservative source
classification rather than an acceptance claim.

## Source and authority basis

The governing family and its safety rules are in
`docs/superpowers/specs/2026-09-04-product-completion-design.md`. The Stage 3
contract and qualification boundary are in
`docs/superpowers/plans/2026-09-04-ai-team-completion.md`. The retained P3
requirements and evidence rules are in
`docs/superpowers/plans/2026-09-04-completion-traceability.md`.
`plans/adrs/ADR-0016-generalized-proposal-service.md` defines the universal
lifecycle, validation, batch atomicity, rollback, and audit-before-success
contract. The control-first plan defines the corresponding P1.1 through P1.7
substrate and P5 trust projections in
`plans/control-first-adaptive-ide-granular-implementation-plan-v0.1.md`.
Current authority traces are the app proposal coordinator and services,
protocol proposal DTOs, workspace/security policy, projection-only UI bridge,
observability, and storage ports. These paths do not establish native product
acceptance.

## Atomic inventory

| ID | Atomic outcome | Package | Classification and limit |
|---|---|---|---|
| `COMP-TRUST-001` | One app-owned lifecycle covers creation, validation, preview, approval, apply, reject, deny, failure, stale, conflict, cancellation, and rollback, with no skipped validation/approval transition. | S3-01 | `partial`: protocol lifecycle DTOs and app coordination traces exist; complete legal-transition behavior and native evidence remain unassessed. |
| `COMP-TRUST-002` | Authenticated principal, capability, workspace trust, total targets, versions/fingerprints, atomicity, rollback metadata, and supported payloads are required; stale, expired, ambiguous, duplicate, unsafe, and unauthorized requests deny before mutation. | S3-01 | `partial`: security/workspace/proposal seams exist; complete all-payload validation and product denial evidence remain unassessed. |
| `COMP-TRUST-003` | Batch proposals preflight all targets, declare atomic/best-effort/dry-run behavior, build deterministic rollback, deny irreversible or unsupported mixtures, and preserve dirty work on partial failure. | S3-01 | `partial`: batch and rollback DTO/authority traces exist; all executor and recovery cases remain unassessed. |
| `COMP-TRUST-004` | Apply and rollback audit before success with metadata-only records, nonzero correlation/causality/event sequence, default redaction, and fail-closed required audit persistence. | S3-01 | `partial`: observability/storage helpers and audit requirements exist; end-to-end ordering and sink-failure proof remain unassessed. |
| `COMP-TRUST-005` | Projection-only trust controls expose proposal/context/privacy, permission, risk, budget, rollback, route, and egress decisions; approval/denial are explicit intents and Manual remains zero-egress with no fallback. | S3-01 | `partial`: projection and policy traces exist; complete native trust surface and Manual OS-level proof remain unassessed. |
| `COMP-TRUST-006` | Packaged native qualification exercises real trust/proposal routes and ordinary controls with authentication, privacy/redaction, Manual zero-egress, stale/conflict/denial, cancellation, rollback, dirty preservation, audit, restart, and unavailable-dependency recovery using external oracles. | S3-07 | `absent`: existing app/desktop tests are supporting substrate; the required packaged native matrix and EvidenceRun records are not established. |

## Exact deduplication and protected canonical rows

The replacement does not copy narrower retained outcomes. `COMP-P3-F1-T2-1`
and `COMP-P3-F1-T3-1` remain the exact trusted-local-apply and visible-denial
rows. `COMP-P3-F2-T1-1..T4-1` remain the AI/agent review, per-hunk, structured
evidence, and keyboard rows. `COMP-P3-F3-T1-1..T4-1` remain checkpoint,
restore, manual-edit preservation, and checkpoint-audit rows. `COMP-P3-F4-T2-1`,
`COMP-P3-F4-T3-1`, and `COMP-P3-F4-T4-1` remain opt-in auto-approval, risk
pause, and human/policy gate rows.

The broader rows depend on those canonical owners where appropriate rather than
duplicating them. Save and stale dirty-buffer guarantees remain owned by
`COMP-PRES-001` and `COMP-PRES-008`; terminal/debug/search/language proposal
outcomes remain owned by their existing `COMP-TERM-*`, `COMP-NAV-*`, and
`COMP-LANG-*` rows. Provider credential/egress and context privacy remain
owned by `COMP-PROV-002`, `COMP-PROV-005`, `COMP-CTX-007`, and `COMP-CTX-008`.
Manual forbidden-surface evidence remains `COMP-P1-F2-T1-1`. Delegate and
Assist native qualification remain `COMP-SCOPE-FAMILY-13-11` and
`COMP-SCOPE-FAMILY-12-13`; family-14/15 interoperability and containment rows
remain their existing owners. `protected_product_ids` on the new rows record
these exact prerequisites without claiming their acceptance.

No new row claims that a fixture, projection, direct dispatch, or historical
evidence is product acceptance. All six rows use literal existing paths without
fragment references and retain `acceptance: unassessed`.

