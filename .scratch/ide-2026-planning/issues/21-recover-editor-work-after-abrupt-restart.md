# 21: Recover editor work after abrupt restart

Status: ready-for-agent

**What to build:** Restore permitted unsaved work after process interruption and refuse unsafe storage migration while preserving originals; distinguish editor from agent recovery.

**Blocked by:** [05 — Preserve edits across save conflicts](05-preserve-edits-across-save-conflicts.md); [08 — Restore tabs, splits and windows](08-restore-tabs-splits-and-windows.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 21.

**Traceability:** M1; work preservation; XQ-05. **Verification target:** A02.

## Acceptance criteria

- [ ] Restore permitted unsaved work after process interruption and refuse unsafe storage migration while preserving originals.
- [ ] Distinguish editor from agent recovery.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.

## Verification

- Existing starting seam: `cargo test -p legion-desktop --test session_restore`. Inspect/select the relevant behavior before editing; this existing suite alone does not establish the new outcome.
- Assert observable results and the failure/denial/recovery conditions named above. Visible product claims require native-input and external-effect evidence; projections, direct dispatch and browser traces do not qualify native IDE input.
- Run planned checks once; rerun only affected failures after supported fixes. After two distinct failed repairs, return evidence and escalate. Separate local checks from real-provider, platform and release qualification.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
