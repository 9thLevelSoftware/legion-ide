# 45: Remove obsolete provider cancellation paths

Status: ready-for-agent

**What to build:** Prove no caller uses the old/no-op API, remove it and exercise slow connect/body cancellation across call kinds within F6 limits.

**Blocked by:** [42 — Migrate Assist and inline calls to real cancellation](42-migrate-assist-and-inline-calls-to-real-cancellation.md); [43 — Migrate Delegate and repair calls to real cancellation](43-migrate-delegate-and-repair-calls-to-real-cancellation.md); [44 — Migrate embedding and utility calls to real cancellation](44-migrate-embedding-and-utility-calls-to-real-cancellation.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 45.

**Traceability:** M3; F6; contract. **Verification target:** A08.

## Acceptance criteria

- [ ] Prove no caller uses the old/no-op API, remove it and exercise slow connect/body cancellation across call kinds within F6 limits.
- [ ] Demonstrate the named outcome with an independent observable oracle appropriate to the workflow: resulting bytes, actual process state, protocol exchange or reopened stored state.
- [ ] Exercise the failure, stale, cancellation, denial or recovery cases specified above without losing work, bypassing proposal authority or silently changing provider/environment.
- [ ] Attach targeted verification and affected canonical evidence. Distinguish component tests from live/native/release qualification; missing prerequisites remain blocked rather than passed.
- [ ] All migration blockers are complete and reference inspection confirms no remaining old/no-op consumers before removing the compatibility path.

## Verification

- Select the existing AppComposition workflow seam before editing. Where necessary, use a real subprocess, reopened store or protocol peer for focused contract coverage. Record the exact targeted commands; do not introduce a parallel orchestration harness.
- Verify the approved cancellation API/caller transition with focused transport and composed-app contracts: deadlines, bounded responses, actual abort/termination and rejection of late actions as applicable. This migration slice does not add a native-input qualification gate; the assisted/native acceptance tickets own that separate evidence.
- Run planned checks once; rerun only affected failures after supported fixes. After two distinct failed repairs, return evidence and escalate. Separate local checks from real-provider, platform and release qualification.

## Execution boundaries

- Consume named versions, configuration and authority contracts from prerequisites; preserve the specification, applicable ADR/dependency gates and existing services.
- Record missing hosts, real peers/providers, tool/model artifacts, credentials, signers or human observation required for this outcome. Publication is not provisioning or deployment authority.
- Preserve unrelated WIP. If inspection exposes a larger gap, propose a bounded repair slice and its edges before closing this ticket.

## Comments

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
