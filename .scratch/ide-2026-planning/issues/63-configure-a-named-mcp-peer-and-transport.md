# 63: Configure a named MCP peer and transport

Status: ready-for-agent

**What to build:** Ratify supported client/server roles, named peers/transports, protocol versions, scoped authentication and privacy; inspect endpoint health and revocation using existing credential services without requiring an inference provider.

**Blocked by:** [01 — Reconcile the 2026 program delta](01-reconcile-the-2026-program-delta.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 63.

**Traceability:** S0-02/S4-01/04. **Verification target:** A06/A19.

## Acceptance criteria

- [ ] Ratify supported client/server roles, named peers/transports, protocol versions, scoped authentication and privacy.
- [ ] Inspect endpoint health and revocation using existing credential services without requiring an inference provider.
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

- 2026-10-09: Coordinator received Euclid independent persistence review PASS for all seven files in patch SHA256 `D07D2A9911F050D2BC0C4D50219F8CB69AA99398A09C74E892FAA31FB706532C` and authorized a bounded local commit. Fast-forwarded through `f3f9436` and `200f8fb` without overlap conflicts; only issue/evidence approval notes changed after review. No passed checks repeated, push, native run or live-peer operation. Ticket 63 remains open and prior `0xc0000409` remains unresolved.

- 2026-10-09: Next bounded HTTP metadata persistence slice uses the existing DesktopSessionStore/workspace record, strict bounded codec and cross-domain restore preflight. Six new public persistence checks passed through targeted runs; five existing provider persistence checks passed once. Scoped formatting/clippy and offline desktop compile passed. Fast-forwarded to integration `26821c7` without conflicts. Independent review pending; no commit or ticket closure. See [persistence evidence](../../../plans/evidence/ide-2026-ticket063-mcp-persistence.md). Native UI, stdio/server and real-peer qualification remain open.

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
- 2026-10-09: Bounded named-peer HTTP core checkpoint implemented on `codex/ide-2026-ticket-063` after merging integration `77166f7`. Configuration/metadata, protocol/role/transport validation, explicit grants, SecretStore credential binding, protocol health and local revocation are observable through AppComposition and a controlled loopback peer. Stdio/server activation, UI/persistence and independently operated real-peer qualification remain open; no acceptance checkbox is credited and this ticket is not resolved. Exact checks and the unresolved Windows `0xc0000409` aborted run are recorded in [checkpoint evidence](../../../plans/evidence/ide-2026-ticket063-named-mcp-peer.md). Independent review pending.
- 2026-10-09: After integration `177caa5`, Euclid held the checkpoint for peer-ID-only credential reuse across endpoints and raw transport-error retention. The bounded delta binds credentials to the reviewed route/authentication/scopes and maps named send failures to fixed redacted labels. Both new public workflow/protocol regressions reproduced their finding before the fix and passed individually afterward. Earlier successful checks were not repeated; delta review remains pending. No implementation commit or ticket-resolution claim.
- 2026-10-09: Coordinator reported Euclid's delta review PASS, authorized the bounded partial commit, and requested merge of integration `928275e`. That fast-forward merge restored the MCP WIP without conflicts and preserved provider-40 persistence. No new tests were run for the merge. This remains a reviewed HTTP core partial delivery; ticket 63 and real-peer/native/platform qualification remain open, including the recorded unresolved `0xc0000409` abort.

- 2026-10-09: Parallel wave source `ddba84fe` integrates independently reviewed MCP settings for add/edit/select, route-bound masked credentials, explicit grant/connect/probe/revoke and durable metadata. App-owned background operations keep HTTP off the UI thread; active and retired requests retain the common drain lease, and a lowered ceiling denies new dispatch immediately. Twenty-five distinct focused MCP tests have passing evidence at their recorded revisions; the composed provider/MCP regression covers four active/retired and completion-order branches. Default/offline desktop compilation, dependency/editor-canvas gates and the default executable build passed. See [settings evidence](../../../plans/evidence/ide-2026-ticket063-mcp-settings.md) and [composed evidence](../../../plans/evidence/ide-2026-provider-mcp-integration.md). Both historical Windows `0xc0000409` aborts remain unexplained; the final two-test debugger invocation passed without reproducing the crash and is not a repair or an uninterrupted green-suite claim. Native UI, independently operated real peers, stdio/server, broader cancellation/recovery and platform/product qualification remain open. No native GUI or live remote operation occurred in this lane. Ticket remains open; acceptance boxes are unchanged.
