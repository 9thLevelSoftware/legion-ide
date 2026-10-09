# 81: Select a versioned structured-tool profile

Status: ready-for-agent

**What to build:** Choose NativeTools or JsonSchema explicitly, bind schema/profile identity to requests and restart, and reject stale/unsupported dialects without prose-repair or environment-override fallback.

**Blocked by:** [40 — Configure an explicit real provider and credentials](40-configure-an-explicit-real-provider-and-credentials.md); [45 — Remove obsolete provider cancellation paths](45-remove-obsolete-provider-cancellation-paths.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 81.

**Traceability:** M6; F6. **Verification target:** A15.

## Acceptance criteria

- [ ] Choose NativeTools or JsonSchema explicitly, bind schema/profile identity to requests and restart, and reject stale/unsupported dialects without prose-repair or environment-override fallback.
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
