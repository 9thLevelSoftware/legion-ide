# 04: Drive one real native open-edit-save journey

Status: needs-info

**What to build:** Use actual OS input on the selected desktop build and verify exact disk effects and candidate identity; extend existing acceptance infrastructure rather than direct-dispatch smoke.

**Blocked by:** [02 — Ratify the Windows/Rust pilot configuration](02-ratify-the-windows-rust-pilot-configuration.md).

**Source:** [Approved specification](../spec.md); approved breakdown ticket 04.

**Traceability:** M1; S1-02. **Verification target:** A01.

## Acceptance criteria

- [ ] Use actual OS input on the selected desktop build and verify exact disk effects and candidate identity.
- [ ] Extend existing acceptance infrastructure rather than direct-dispatch smoke.
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

- 2026-10-09: Frozen r4 packaging validation passed, but actual native UIA focus still reports the window root after both indexed and coordinate editor clicks. Exact foreground/document/Clean-tab/hit checks passed. The compact-layout renderer regression passed without a runtime fix and therefore does not reproduce the native discrepancy. No typing/save attempt or acceptance promotion; Computer Use stopped with Escape. [Updated focus evidence](../../../plans/evidence/ide-2026-ticket004-editor-focus-prerequisite.md#r4-native-follow-up-during-parallel-execution) records source/artifact identities, logs and the additional component check.

- 2026-10-09: [Editor focus repair](../../../plans/evidence/ide-2026-ticket004-editor-focus-prerequisite.md) passes 18 distinct focused renderer/input/save tests plus default/offline compilation, canvas and formatting checks. Independent review passed and source `fb9982f` integrated locally; packaged r4 native focus/typing/save verification remains pending. The existing foreground wait uses neutral provenance when Computer Use activates the exact test window; no human attendance is inferred. Status and acceptance remain unchanged.

- 2026-10-09: Scoped navigation, isolated sessions and guarded clicks/focus passed independent review and integrated locally as `5d7b298`; 11 distinct focused tests passed. Native read-only file and close-button point checks passed with duplicate labels present. The remaining product prerequisite is confirmed: after editor click, the exact foreground app exposes its root as UIA keyboard focus, not the document. Turing owns a bounded focus-projection repair; the strict guard remains. README/default session unchanged, diagnostic child closed. No typing/save acceptance or ticket closure.

- 2026-10-09: Fresh attended r3 attempt detected the owner's foreground click, then blocked before driver input on three global README labels. Owned-child cleanup explains the immediate window close. Traces identified tab/breadcrumb/Excerpts duplicates and restored session metadata; a separate fresh-session native desktop diagnostic confirmed the actual Explorer Dialog/Button ancestry and opened README without typing or saving. Reference bytes and tracked Git remain unchanged. Scoped navigation/session isolation repair is in progress; [r3 evidence](../../../plans/evidence/ide-2026-ticket004-candidate-r3.md) preserves report, traces and screenshots. No acceptance promotion.

- 2026-10-09: Bounded editor accessibility repair passed independent review and integrated as `caf1821`. Frozen offline MSI 0.0.3 passed verification/staging, then the read-only native probe returned the exact complete README, Clean tab and valid editor geometry on its first sample. Reference checkout remains clean. [Candidate r3 evidence](../../../plans/evidence/ide-2026-ticket004-candidate-r3.md) binds source/artifacts/logs. Fresh attendance requested; native typing/save, recovery and full conformance remain unqualified. Status remains needs-info.

- 2026-10-09: Euclid navigation delta PASS; coordinator confirmed compiled hash400d068d and authorized partial commit/clean same-base1f647cc integration merge. User availability pending, no launch. Reviewed binary will be archived under the resulting commit ID without overwriting existing files. Ticket04 remains needs-info; no acceptance promotion or repeated successful checks.

- 2026-10-09: New bounded navigation slice after coordinator's read-only stable compact UIA tree (29 elements/27 names, Explorer drawer present, README absent). Merged integration1f647cc preserving ticket088. Driver now prefers a unique visible exact target, otherwise clicks the unique real Explorer drawer via guarded atomic UIA-bounds pointer and waits up to3 seconds for the target; missing/ambiguous results block. Two focused recorded-tree/no-input regressions red→green; build passed. Post-click behavior remains simulated test evidence, not a native observation. Five-file delta awaits independent review; no native run until coordinator announces availability, no commit. Status needs-info unchanged.

- 2026-10-09: Final independent review PASS, no blockers, including session access delta and documentation. Coordinator authorized partial commit/integration; status remains needs-info and latest Explorer UIA blocker unchanged. No acceptance promotion.

- 2026-10-09: Final COM-fixed actual attempt passed main-window/focus/COM then blocked before input: exact Explorer README.md UIA label found 0, driver report exit 3 versus outer PowerShell exit 1. Clone clean. After two supported driver repairs, new UIA/product blocker escalated; all native attempts stopped. Widget-tree investigation deferred to next bounded slice. Merged integration 77166f7 ff-only preserving WIP; partial deliverable awaits session delta/final doc review before commit. Status needs-info; no workflow/input/full-scenario acceptance.

- 2026-10-09: Isolated native paired probe reproduced COM denial after desktop attachment at access 0x81 and success at 0x83. Added only DESKTOP_CREATEWINDOW to existing session access request; focused interactive attachment-to-COM regression failed red then passed, driver build passed. No product/input launch, ACL/elevation change or acceptance promotion. session.rs and driver_contract.rs delta awaits independent review; needs-info remains until actual journey evidence.

- 2026-10-09: Coordinator selector-fixed attended retry selected the main HWND and passed foreground, then blocked before input at CoInitializeEx access denied (0x80070005), report driver exit 3 versus outer PowerShell exit 1. Clone clean; no edit/save outcome. Read-only source/probe evidence distinguishes this new COM prerequisite from the proven wrong-HWND defect; cause remains unproven. No more native launches or permission/workaround changes. Existing code PASS relayed; partial deliverable awaits final doc review before commit. Status remains needs-info.

- 2026-10-09: Empirical read-only r2 diagnostic supersedes earlier host-focus explanation: driver selected zero-area Winit helper; user foregrounded the same-PID `Legion IDE` main HWND. Minimal unique visible titled positive-area unowned window selector repair and observed-sequence regression passed focused test/build. Exact foreground guards unchanged. Prior P1/P2 reviewer PASS recorded in evidence; selector delta awaits review and coordinator-owned retry. Status remains needs-info; no native edit/save acceptance or commit.

- 2026-10-08: User approved the 254-ticket breakdown. Published locally with the approved title, scope and blockers; execution has not started.
- 2026-10-09: Claimed in isolated ticket-004 worktree; read frozen pilot/reference scenario and extended only the external driver, with explicit clone/target selection and expected-window guards before every input batch. [Preparation and actual evidence](../../../plans/evidence/ide-2026-ticket004-native-journey.md) records exact identities, commands, headless checks and preserved reports. No product/test hooks or dispatch substitution.
- 2026-10-09: Coordinator approved ready staged offline candidate 07d2187/MSI 0.0.2. Actual native attempts observed a packaged window but returned blocked/exit 3 because Windows refused foregrounding, before any injected input. Disposable Legion reference c2a6578 remains clean with unchanged README bytes. Native attempts stopped; coordinator owns interactive foreground prerequisite. Ticket remains incomplete, awaiting host prerequisite and independent review before commit. Full SC steps 7–8/recovery and six-class/CJK/pilot qualification are not promoted.
- 2026-10-09: Coordinator accepted the host blocker and requested optional `--await-foreground`. Implemented a bounded read-only 60-second exact-window handoff wait with window/process exit detection, existing owned-child termination, unchanged default mode and retained per-input guards. Focused CLI/wait/default tests passed; no additional OS run. User availability and coordinator instruction remain pending; patch uncommitted for independent review.
- 2026-10-09: Euclid P1/P2 delta fixed: one guarded click batch, unique complete-document oracle and exact post-type comparison. Two focused regressions passed and compiled debug binary; no OS run. Four code paths reported to coordinator for rapid delta review. Awaiting approval/launch announcement; no commit.
