# 48: Freeze prediction provider and usefulness baseline

Status: ready-for-agent

**What to build:** Pin one real compatible prediction profile and comparable ordinary-completion tasks, predeclare latency/retention/rejection/undo targets and prerequisites before scoring a candidate.

**Blocked by:** [40 — Configure an explicit real provider and credentials](40-configure-an-explicit-real-provider-and-credentials.md); [02 — Ratify the Windows/Rust pilot configuration](02-ratify-the-windows-rust-pilot-configuration.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 48.

**Traceability:** M3; Q25/Q29; F8. **Verification target:** A10.

## Acceptance criteria

- [ ] Pin one real compatible prediction profile and comparable ordinary-completion tasks, predeclare latency/retention/rejection/undo targets and prerequisites before scoring a candidate.
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
