# 49: Use and qualify real next-edit predictions

Status: ready-for-agent

**What to build:** Accept/reject/undo useful current suggestions with measured frozen outcomes; outage, stale state or revoked consent leaves editing usable and never selects another provider silently.

**Blocked by:** [48 — Freeze prediction provider and usefulness baseline](48-freeze-prediction-provider-and-usefulness-baseline.md); [42 — Migrate Assist and inline calls to real cancellation](42-migrate-assist-and-inline-calls-to-real-cancellation.md); [46 — Inspect and control context selection](46-inspect-and-control-context-selection.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 49.

**Traceability:** M3; S3-05. **Verification target:** A10.

## Acceptance criteria

- [ ] Accept/reject/undo useful current suggestions with measured frozen outcomes.
- [ ] Outage, stale state or revoked consent leaves editing usable and never selects another provider silently.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.

## Verification

- Existing starting seam: `cargo test -p legion-app --test assist_inline_prediction_workflow`. Inspect/select the relevant behavior before editing; this existing suite alone does not establish the new outcome.
- Assert observable results and the failure/denial/recovery conditions named above. Visible product claims require native-input and external-effect evidence; projections, direct dispatch and browser traces do not qualify native IDE input.
- Run planned checks once; rerun only affected failures after supported fixes. After two distinct failed repairs, return evidence and escalate. Separate local checks from real-provider, platform and release qualification.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
