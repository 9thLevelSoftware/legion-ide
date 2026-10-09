# 160: Persist and delete extension secret storage

Status: ready-for-agent

**What to build:** Use the named extension to persist/reopen/migrate/delete secret-scoped data under quotas and isolation, with appropriate secret/retention handling and no cross-extension access.

**Blocked by:** [155 — Ratify one finite extension execution contract](155-ratify-one-finite-extension-execution-contract.md); [156 — Activate and revoke a signed WASM contribution](156-activate-and-revoke-a-signed-wasm-contribution.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 160.

**Traceability:** M4; S5-02. **Verification target:** A18/A19.

## Acceptance criteria

- [ ] Use the named extension to persist/reopen/migrate/delete secret-scoped data under quotas and isolation, with appropriate secret/retention handling and no cross-extension access.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.

## Verification

- Select the existing AppComposition workflow seam before editing. Where necessary, use a real subprocess, reopened store or protocol peer for focused contract coverage. Record the exact targeted commands; do not introduce a parallel orchestration harness.
- Assert observable results and the failure/denial/recovery conditions named above. Visible product claims require native-input and external-effect evidence; projections, direct dispatch and browser traces do not qualify native IDE input.
- Run planned checks once; rerun only affected failures after supported fixes. After two distinct failed repairs, return evidence and escalate. Separate local checks from real-provider, platform and release qualification.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
