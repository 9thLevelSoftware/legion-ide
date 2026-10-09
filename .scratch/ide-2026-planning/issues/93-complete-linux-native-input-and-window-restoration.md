# 93: Complete Linux native input and window restoration

Status: ready-for-agent

**What to build:** Observe the same native edit/IME/Vim/focus/DPI/window-restore flows on Linux, repair platform-specific gaps and retain exact input/disk evidence.

**Blocked by:** [88 — Ratify language and platform qualification configurations](88-ratify-language-and-platform-qualification-configurations.md); [06 — Complete selection, Unicode and undo workflows](06-complete-selection-unicode-and-undo-workflows.md); [07 — Complete IME and Vim input workflows](07-complete-ime-and-vim-input-workflows.md); [08 — Restore tabs, splits and windows](08-restore-tabs-splits-and-windows.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 93.

**Traceability:** M4; S1-02/08. **Verification target:** A01/A02/A16.

## Acceptance criteria

- [ ] Observe the same native edit/IME/Vim/focus/DPI/window-restore flows on Linux, repair platform-specific gaps and retain exact input/disk evidence.
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
