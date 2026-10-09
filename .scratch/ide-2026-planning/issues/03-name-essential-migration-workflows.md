# 03: Name essential migration workflows

Status: resolved

**What to build:** Freeze the supported settings/keybinding subset and named essential extensions/versions; map each pilot workflow to a native equivalent, compatibility target or explicit blocker.

**Blocked by:** [01 — Reconcile the 2026 program delta](01-reconcile-the-2026-program-delta.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 03.

**Traceability:** M0; Q27/Q30. **Verification target:** A11.

## Acceptance criteria

- [x] Freeze the supported settings/keybinding subset and named essential extensions/versions.
- [x] Map each pilot workflow to a native equivalent, compatibility target or explicit blocker.
- [x] Every scope, configuration and ownership decision is reviewable and traceable to the approved specification and existing canonical records; unresolved prerequisites are explicitly recorded.
- [x] Preserve existing requirement IDs and approved scope; planning evidence does not promote implementation or product acceptance.

## Verification

- Run `cargo run -p xtask -- docs-hygiene` for documentation changes; use the existing completion/register validators when canonical records change. Record commands and their actual scope.
- Resolve named configurations and ownership against existing records; missing tools, access or ratification stay explicit, without fabricated acceptance.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
- 2026-10-08: Bounded contract implemented on `codex/ide-2026-ticket-003`; [migration contract](../../../plans/completion/ide-2026-migration-contract.md) is review-ready. Inspected source at integration c2a6578, read TDD/glossary/authority/ADR guidance and supplied external preflight/setup notes; no personal settings or secrets inspected. Integration fast-forwarded to 87580fa before edits. Only this issue and the contract are worker-owned; map/execution and canonical acceptance records are unchanged by this patch. No commit before independent review.
- 2026-10-08: Frozen subset means selected planned import mappings, not current importer support. Ticket 22 owns preview, collisions, malformed/unsupported reporting, persistence and reversal. Native Windows/Rust edit/test/debug/Git equivalents use already approved pilot scope; installed Containers 2.5.2 and Remote Containers 0.469.0 remain later required, unqualified targets. Real adapter/native workflow evidence, candidate/configuration, Manual observation and later extension/container resources remain prerequisites.

## Local verification evidence

Planned before editing: existing docs-hygiene, completion-register validator and
diff whitespace check, once each; affected failures only may be rerun after repair.
No runtime behavior was changed and no artificial runtime tests were added.

```text
& D:/legion-ide/target/debug/xtask.exe docs-hygiene
documentation hygiene checks passed; exit 0

& D:/legion-ide/target/debug/xtask.exe verify-completion-register --root .
absent 43; implemented 143; partial 233; acceptance unassessed 419
verify-completion-register passed: register structure only, no evidence or acceptance was assessed
exit 0
```

Both validators used the existing shared xtask executable from this worker root.
These results establish document hygiene and register structure only, not native
input, settings import, runtime compatibility, deployment or product acceptance.
Independent review PASS (Chandrasekhar), no blocking findings, reported by the
coordinator on 2026-10-08. The bounded documentation deliverable is resolved;
downstream implementation, qualification and canonical acceptance remain open.

Final planned whitespace check: `git diff --check` (exit 0, no output).

- 2026-10-08: Coordinator authorized recording independent review PASS and
  committing only this issue and the migration contract. Integration 87580fa is
  merged; successful checks were not repeated. No canonical acceptance promotion.
