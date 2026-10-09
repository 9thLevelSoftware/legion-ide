# 152: Qualify native change intelligence on Windows

Status: ready-for-agent

**What to build:** Observe declared graph/evidence/structured/retrieval native journeys on Windows, including stale/denied/cancel/restart and accessible unsupported profiles. Per-capability outcomes remain separate; negative experiment completion cannot count as positive support.

**Blocked by:** [72 — Record a real frontier qualification run](72-record-a-real-frontier-qualification-run.md); [39 — Qualify the first complete change-review journey](39-qualify-the-first-complete-change-review-journey.md); [31 — Navigate derived canvas relationships](31-navigate-derived-canvas-relationships.md); [120 — Inspect real TypeScript structural impact](120-inspect-real-typescript-structural-impact.md); [121 — Inspect real JavaScript structural impact](121-inspect-real-javascript-structural-impact.md); [122 — Inspect real Python structural impact](122-inspect-real-python-structural-impact.md); [82 — Evaluate a provisioned XGrammar endpoint](82-evaluate-a-provisioned-xgrammar-endpoint.md); [123 — Qualify learned retrieval across required languages](123-qualify-learned-retrieval-across-required-languages.md); [04 — Drive one real native open-edit-save journey](04-drive-one-real-native-open-edit-save-journey.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 152.

**Traceability:** M4/M6; F8. **Verification target:** A03/A13/A15/A20.

## Acceptance criteria

- [ ] Observe declared graph/evidence/structured/retrieval native journeys on Windows, including stale/denied/cancel/restart and accessible unsupported profiles. Per-capability outcomes remain separate.
- [ ] Negative experiment completion cannot count as positive support.
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
