# 254: Run final candidate acceptance and record the release decision

Status: ready-for-agent

**What to build:** Run all final gates on a nominated immutable signed candidate, complete ten consecutive owner working days and two independent usability sessions, and perform the complete canonical acceptance join including audit/guidance/campaign outcomes; repairs invalidate affected evidence. Record the separate authorized release decision without treating this ticket as permission to ship.

**Blocked by:** [252 — Close independent security/privacy audit findings](252-close-independent-security-privacy-audit-findings.md); [253 — Publish evidence-backed operator and compatibility guidance](253-publish-evidence-backed-operator-and-compatibility-guidance.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 254.

**Traceability:** M7; S6-01. **Verification target:** A01/A19/A20.

## Acceptance criteria

- [ ] Run all final gates on a nominated immutable signed candidate, complete ten consecutive owner working days and two independent usability sessions, and perform the complete canonical acceptance join including audit/guidance/campaign outcomes.
- [ ] Repairs invalidate affected evidence. Record the separate authorized release decision without treating this ticket as permission to ship.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.
- [ ] The complete final join includes audit, guidance and campaign evidence on the immutable candidate; this ticket does not authorize shipping.

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
