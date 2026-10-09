# 83: Freeze learned-retrieval workloads and profile

Status: ready-for-agent

**What to build:** Pin the approved model/tokenizer/vector identity and 100 independently labeled split queries, freeze ranking/citation/latency targets and record explicit provisioning/consent prerequisites.

**Blocked by:** [46 — Inspect and control context selection](46-inspect-and-control-context-selection.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 83.

**Traceability:** M6; F0/F7. **Verification target:** A15/A20.

## Acceptance criteria

- [ ] Pin the approved model/tokenizer/vector identity and 100 independently labeled split queries, freeze ranking/citation/latency targets and record explicit provisioning/consent prerequisites.
- [ ] Every scope, configuration and ownership decision is reviewable and traceable to the approved specification and existing canonical records; unresolved prerequisites are explicitly recorded.
- [ ] Preserve existing requirement IDs and approved scope; planning evidence does not promote implementation or product acceptance.
- [ ] Freeze applicable baselines, workloads and limits before evaluation; separate reviewed negative outcomes from blocked/unfinished work and supported capability.

## Verification

- Run `cargo run -p xtask -- docs-hygiene` for documentation changes; use the existing completion/register validators when canonical records change. Record commands and their actual scope.
- Resolve named configurations and ownership against existing records; missing tools, access or ratification stay explicit, without fabricated acceptance.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
