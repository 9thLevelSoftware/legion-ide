# 111: Debug a real JavaScript Node program

Status: ready-for-agent

**What to build:** Launch/attach the declared JavaScript Node adapter/debuggee and demonstrate breakpoint, step, inspect, console and terminate with restart/unavailable behavior.

**Blocked by:** [88 — Ratify language and platform qualification configurations](88-ratify-language-and-platform-qualification-configurations.md); [18 — Debug a real Rust failure](18-debug-a-real-rust-failure.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 111.

**Traceability:** M4; S2-05. **Verification target:** A16.

## Acceptance criteria

- [ ] Launch/attach the declared JavaScript Node adapter/debuggee and demonstrate breakpoint, step, inspect, console and terminate with restart/unavailable behavior.
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
