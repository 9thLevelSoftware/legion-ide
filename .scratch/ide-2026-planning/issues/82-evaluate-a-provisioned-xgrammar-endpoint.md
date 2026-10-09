# 82: Evaluate a provisioned XGrammar endpoint

Status: ready-for-agent

**What to build:** Freeze workloads and compare syntax, semantic tool success and latency for the selected serving-side backend; retain semantic/permission checks and publish an independent result.

**Blocked by:** [81 — Select a versioned structured-tool profile](81-select-a-versioned-structured-tool-profile.md); [47 — Review a real Assist multi-file proposal](47-review-a-real-assist-multi-file-proposal.md); [72 — Record a real frontier qualification run](72-record-a-real-frontier-qualification-run.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 82.

**Traceability:** M6; F6/F8. **Verification target:** A15/A20.

## Acceptance criteria

- [ ] Freeze workloads and compare syntax, semantic tool success and latency for the selected serving-side backend.
- [ ] Retain semantic/permission checks and publish an independent result.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.
- [ ] Freeze applicable baselines, workloads and limits before evaluation; separate reviewed negative outcomes from blocked/unfinished work and supported capability.

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
