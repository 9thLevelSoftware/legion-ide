# 215: Reconcile concurrent edits in a shared document

Status: ready-for-agent

**What to build:** Use the ratified shared-document semantics for concurrent edits and offline return, preserving dirty text and surfacing unresolved conflicts instead of silent overwrite.

**Blocked by:** [214 — Recover durable team events after service restart](214-recover-durable-team-events-after-service-restart.md); [05 — Preserve edits across save conflicts](05-preserve-edits-across-save-conflicts.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 215.

**Traceability:** M5; S5-10. **Verification target:** A18.

## Acceptance criteria

- [ ] Use the ratified shared-document semantics for concurrent edits and offline return, preserving dirty text and surfacing unresolved conflicts instead of silent overwrite.
- [ ] Every scope, configuration and ownership decision is reviewable and traceable to the approved specification and existing canonical records; unresolved prerequisites are explicitly recorded.
- [ ] Preserve existing requirement IDs and approved scope; planning evidence does not promote implementation or product acceptance.

## Verification

- Run `cargo run -p xtask -- docs-hygiene` for documentation changes; use the existing completion/register validators when canonical records change. Record commands and their actual scope.
- Resolve named configurations and ownership against existing records; missing tools, access or ratification stay explicit, without fabricated acceptance.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
