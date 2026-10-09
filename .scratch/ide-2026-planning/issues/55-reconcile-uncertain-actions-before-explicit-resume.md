# 55: Reconcile uncertain actions before explicit resume

Status: ready-for-agent

**What to build:** Record action IDs and known/uncertain outcomes; inspect external resulting state before retry and require inspection when uncertain, preventing duplicate consequential actions.

**Blocked by:** [54 — Restore stopped agent progress after a crash](54-restore-stopped-agent-progress-after-a-crash.md); [43 — Migrate Delegate and repair calls to real cancellation](43-migrate-delegate-and-repair-calls-to-real-cancellation.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 55.

**Traceability:** M3; Q31. **Verification target:** A09.

## Acceptance criteria

- [ ] Record action IDs and known/uncertain outcomes.
- [ ] Inspect external resulting state before retry and require inspection when uncertain, preventing duplicate consequential actions.
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
