# 88: Ratify language and platform qualification configurations

Status: ready-for-agent

**What to build:** Resolve the existing provisional matrix into named OS/tool/server/runner/debugger/project/AT versions and fixture identities for required Rust, TS, JS and Python categories without narrowing approved coverage.

**Blocked by:** [01 — Reconcile the 2026 program delta](01-reconcile-the-2026-program-delta.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 88.

**Traceability:** M4; S0-02/S2-01. **Verification target:** A16.

## Acceptance criteria

- [ ] Resolve the existing provisional matrix into named OS/tool/server/runner/debugger/project/AT versions and fixture identities for required Rust, TS, JS and Python categories without narrowing approved coverage.
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
