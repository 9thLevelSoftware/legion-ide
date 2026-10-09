# 40: Configure an explicit real provider and credentials

Status: ready-for-agent

**What to build:** Add/select/validate a named endpoint/model, securely replace/revoke credentials and display capability/locality/health; no credentials in config, no fallback or Manual calls.

**Prerequisite resolved:** [01 — Reconcile the 2026 program delta](01-reconcile-the-2026-program-delta.md). Native wiring and live qualification prerequisites remain open; see the core evidence below.

**Source:** [Approved specification](../spec.md); approved breakdown ticket 40.

**Traceability:** M3; S3-01/02. **Verification target:** A10/A19.

## Acceptance criteria

- [ ] Add/select/validate a named endpoint/model, securely replace/revoke credentials and display capability/locality/health.
- [ ] No credentials in config, no fallback or Manual calls.
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
- 2026-10-09: Partial app/provider core implemented at the user-approved AppComposition seam: explicit metadata profiles, route-bound secure-store APIs, truthful refusal without fixtures, bounded MiMo chat wire options, cancellation/drain guards and selected-profile ghost revision invalidation. Owner selected OpenAI-compatible `https://token-plan-sgp.xiaomimimo.com/v1`, model `mimo-v2.6-pro`; this supersedes the direct PAYG candidate. No key or paid/live-call authority supplied. [Focused core evidence and exact checks](../../../plans/evidence/ticket-040-explicit-provider-core.md) records behavioral red/green, compiler repairs/escalation and the three early review fixes. Coordinator reported Euclid final core review PASS, all three prior blockers fixed and no new material findings; partial core commit authorized after integration `f4dbeed`. Ticket remains open: native UI/credential wiring, durable metadata persistence, platform keyring evidence and independent native/live acceptance are separate remaining gates. Acceptance boxes are intentionally unchanged.
