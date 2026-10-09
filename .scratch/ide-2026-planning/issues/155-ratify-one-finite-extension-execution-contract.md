# 155: Ratify one finite extension execution contract

Status: ready-for-agent

**What to build:** Name required extension versions, runtime/API clusters, contribution and storage contracts; review ADR/dependency/policy gates and expose unsupported mappings before runtime activation.

**Blocked by:** [03 — Name essential migration workflows](03-name-essential-migration-workflows.md); [01 — Reconcile the 2026 program delta](01-reconcile-the-2026-program-delta.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 155.

**Traceability:** M4; S5-01. **Verification target:** A11/A18.

## Acceptance criteria

- [ ] Name required extension versions, runtime/API clusters, contribution and storage contracts.
- [ ] Review ADR/dependency/policy gates and expose unsupported mappings before runtime activation.
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

- 2026-10-09: Coordinator assigned Pauli a documentation-only draft in isolated `wave1-extension-contract` at `e671d8c`; implementation-lane reviews take priority. The [finite extension contract draft](../../../plans/completion/ide-2026-extension-execution-contract.md) carries ticket 03's installed Containers 2.5.2 and Remote Containers 0.469.0 pins, observed manifest hashes/entrypoints/dependencies, proposed finite scenarios, host/storage/lifecycle authority, and explicit unavailable representatives and configuration fields. Runtime selection, numeric limits, API inventory and S5 activation contracts are authorized engineering work, not requests for renewed owner permission. Missing real artifacts/representatives and qualification environments remain separate prerequisites; root independent review PASS for bounded documentation only. Observed ADR-0050 46.0.2 versus current Wasmtime 48.0.5 version drift is recorded for reconciliation, not silently ratified. No runtime activation, owner-ratification claim, canonical acceptance change, commit or integration. Initial scoped whitespace checks reported no whitespace defects; the coordinator-requested authority clarification is the final delta. Docs-hygiene remains reserved once for the composed wave.
