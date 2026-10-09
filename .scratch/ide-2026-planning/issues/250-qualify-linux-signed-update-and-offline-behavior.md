# 250: Qualify Linux signed update and offline behavior

Status: ready-for-agent

**What to build:** Use actual signed installed artifacts to prove update/restart/rollback and Manual no-egress on Linux, recording packaging identity and external OS evidence.

**Blocked by:** [249 — Qualify signed Linux clean installation](249-qualify-signed-linux-clean-installation.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 250.

**Traceability:** M7; XQ-04/06 acceptance. **Verification target:** A19.

## Acceptance criteria

- [ ] Use actual signed installed artifacts to prove update/restart/rollback and Manual no-egress on Linux, recording packaging identity and external OS evidence.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.

## Verification

- Existing starting seam: `cargo test -p xtask --test completion_evidence`. Inspect/select the relevant behavior before editing; this existing suite alone does not establish the new outcome.
- Assert observable results and the failure/denial/recovery conditions named above. Visible product claims require native-input and external-effect evidence; projections, direct dispatch and browser traces do not qualify native IDE input.
- Run planned checks once; rerun only affected failures after supported fixes. After two distinct failed repairs, return evidence and escalate. Separate local checks from real-provider, platform and release qualification.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
