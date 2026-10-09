# 147: Qualify TypeScript Node workflows on Linux

Status: ready-for-agent

**What to build:** Observe the declared TypeScript Node native navigation/edit/test/debug/restart journey on Linux with real tools and external effects; record this matrix cell independently, including blocked prerequisites.

**Blocked by:** [88 — Ratify language and platform qualification configurations](88-ratify-language-and-platform-qualification-configurations.md); [93 — Complete Linux native input and window restoration](93-complete-linux-native-input-and-window-restoration.md); [101 — Review language edits in a TypeScript Node project](101-review-language-edits-in-a-typescript-node-project.md); [102 — Build and diagnose tests in a TypeScript Node project](102-build-and-diagnose-tests-in-a-typescript-node-project.md); [103 — Debug a real TypeScript Node program](103-debug-a-real-typescript-node-program.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 147.

**Traceability:** M4; S2-06. **Verification target:** A16.

## Acceptance criteria

- [ ] Observe the declared TypeScript Node native navigation/edit/test/debug/restart journey on Linux with real tools and external effects.
- [ ] Record this matrix cell independently, including blocked prerequisites.
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
