# 01: Reconcile the 2026 program delta

Status: claimed

**What to build:** Map the 36 decisions and new outcomes onto existing canonical requirements; preserve IDs and expose unmapped or conflicting obligations without recreating the completion register.

**Blocked by:** None (can start immediately).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 01.

**Traceability:** M0; S0-01/06; F0. **Verification target:** A20.

## Acceptance criteria

- [ ] Map the 36 decisions and new outcomes onto existing canonical requirements.
- [ ] Preserve IDs and expose unmapped or conflicting obligations without recreating the completion register.
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
- 2026-10-08: Claimed for implementation on `codex/ide-2026-integration`; isolated worker branch owns reconciliation.
