# 123: Qualify learned retrieval across required languages

Status: ready-for-agent

**What to build:** Evaluate frozen held-out ranking/citations/resource targets across Rust/TS/JS/Python, revoked consent and cache pressure; retain negative conclusions without claiming support.

**Blocked by:** [84 — Inspect bounded learned context for one request](84-inspect-bounded-learned-context-for-one-request.md); [72 — Record a real frontier qualification run](72-record-a-real-frontier-qualification-run.md); [120 — Inspect real TypeScript structural impact](120-inspect-real-typescript-structural-impact.md); [121 — Inspect real JavaScript structural impact](121-inspect-real-javascript-structural-impact.md); [122 — Inspect real Python structural impact](122-inspect-real-python-structural-impact.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 123.

**Traceability:** M6; F7/F8. **Verification target:** A15/A20.

## Acceptance criteria

- [ ] Evaluate frozen held-out ranking/citations/resource targets across Rust/TS/JS/Python, revoked consent and cache pressure.
- [ ] Retain negative conclusions without claiming support.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.
- [ ] Freeze applicable baselines, workloads and limits before evaluation; separate reviewed negative outcomes from blocked/unfinished work and supported capability.

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
