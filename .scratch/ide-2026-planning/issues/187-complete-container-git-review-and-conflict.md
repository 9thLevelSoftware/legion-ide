# 187: Complete container Git review and conflict

Status: ready-for-agent

**What to build:** Use the declared container to complete its Git review and conflict journey with correct workspace/host identity, permission denials, cancellation or reconnect as applicable, and real external effects; SSH evidence cannot substitute.

**Blocked by:** [174 — Open, edit and save a declared container workspace](174-open-edit-and-save-a-declared-container-workspace.md); [20 — Resolve a Git conflict and restore history](20-resolve-a-git-conflict-and-restore-history.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 187.

**Traceability:** M5; S5-06/07. **Verification target:** A18.

## Acceptance criteria

- [ ] Use the declared container to complete its Git review and conflict journey with correct workspace/host identity, permission denials, cancellation or reconnect as applicable, and real external effects.
- [ ] SSH evidence cannot substitute.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.

## Verification

- Existing starting seam: `cargo test -p legion-app --test git_workflow`. Inspect/select the relevant behavior before editing; this existing suite alone does not establish the new outcome.
- Assert observable results and the failure/denial/recovery conditions named above. Visible product claims require native-input and external-effect evidence; projections, direct dispatch and browser traces do not qualify native IDE input.
- Run planned checks once; rerun only affected failures after supported fixes. After two distinct failed repairs, return evidence and escalate. Separate local checks from real-provider, platform and release qualification.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
