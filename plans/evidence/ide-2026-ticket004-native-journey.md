# Ticket 04 native journey preparation and blocked attempt

Date: 2026-10-09. Worker: `codex/ide-2026-ticket-004`, base f8b375a.
Status: incomplete, blocked by zero matching Explorer README.md UIA elements
before input. Earlier partial driver deliverable independently reviewed PASS;
the bounded compact-Explorer navigation delta below awaits review and a separately
coordinated native run. Ticket04 remains needs-info. No acceptance promotion.

The scope is one real open/edit/save journey. It is distinct from full
SC-MANUAL-OPEN-TYPE-SAVE qualification, six-class COMP-PLAT-002 conformance and the
five-day pilot. Read the TDD skill, glossary, authority boundaries, ADR-0056,
ticket 04 and the frozen pilot configuration before implementation. Authority
remains external process/input/UIA/disk/Git observation; no product hooks, direct
dispatch, app/provider edits, network or original-workspace mutations.

## Artifact and representative repository

Coordinator supplied and approved native use of the staged offline unsigned MSI
0.0.2 candidate 07d2187619af9556cf3fd14ae2d3e429d4ea1c36:

- Executable: `D:/legion-ide-2026-candidate-r2/target/native-input-acceptance/package/legion-desktop.exe`.
- Executable SHA-256: `184c81243778b12aa2dadccc51a33bc0551dfaa1eb4f5c607f96449cb12218b5`, read independently and matched before launch.
- MSI SHA-256: `e0476a80f0c560d00f7441f637f7f00be86c0f7168b4bbf015f80bd24cc1b333` from coordinator staging evidence. Actual verifier/staging checks were coordinator-owned and not repeated here.
- Source: read-only `D:/legion-ide-2026-notes/pilot-candidate-07d2187.md` and staged `STAGING-EVIDENCE.toml`.
- Representative disposable clone: `D:/legion-ide-2026-tools/ticket004-reference-c2a6578`, created via `git clone --no-hardlinks --no-checkout D:/legion-ide-2026-integration <clone>` then detached checkout of ratified reference `c2a65786862e759e67ebac28c32c1cbc618047f0`. Initial status clean.
- Cargo.lock SHA-256: `0709c888cba93763226663f9314dee5198bf2dc1387d8b8c334422df3f08d6c7`, matches the frozen pilot contract.
- README.md pre/post SHA-256: `5da9ac0a7844b4f215829bc2a6523d120fdbf97e3e1759234aa12623be29bc9a`, unchanged after the blocked attempts. Final clone status clean.

No generated seed fixture substitutes for Legion-on-Legion. No product launch
uses `--file` to bypass Explorer. No native journey success is inferred from the
coordinator's headless Manual smoke pass.

## Existing driver gap and bounded extension

The existing conformance driver seeds `native_input_probe.txt` with `seed\n`,
opens via `--file`, and observes keyboard/pointer/text/clipboard/IME/command
classes. That route remains unchanged and cannot establish this frozen journey.
It does not clone the reference, open through Explorer or qualify Save All,
Save As and recovery cases. Its IME oracle requires a real active CJK layout;
supplied host inventory is en-US only, so CJK qualification is blocked, never pass.

The added `--open-edit-save-run --workspace <clone> --target <relative-file>` mode
extends the same driver. It rejects mixed modes, requires explicit paths, verifies
a clean tracked target within the repository, records external source/file and
executable identity, then launches the packaged subprocess with workspace only.
Its planned scope is steps 1–6: Explorer pointer open, clean/dirty target tab,
editor pointer focus, unique typed prefix, save, exact resulting bytes, and disk/Git
oracles. Missing target UIA labels/text or dirty affordance block, not pass. The
text oracle must identify the selected document before text injection. The
external driver refuses unrelated tracked or untracked Git status changes.

Every pointer/key batch and each typed character checks the expected product
foreground window immediately before sending; focus loss aborts without
refocusing or redirecting subsequent input. This check is an external OS guard,
not a product hook or a guarantee against an OS focus change during an individual
atomic input batch. Ctrl+S exact bytes and dirty clearing are recorded independently;
if palette fallback is used, the report names `save_path=command-palette` and does
not credit the shortcut. No actual save route was reached in these attempts.

The journey report sets `complete_scenario=false` and
`full_input_conformance=false` even for a completed bounded journey. Planned
coverage is not completed coverage. The six-class harness protocol/exit statuses
remain unchanged; xtask has no changes and is not given this partial report as a
full conformance result.

### Optional attended foreground handoff

After the foreground host blocker was accepted, the coordinator requested
`--await-foreground`, permitted only with `--open-edit-save-run`. Once the window
appears, this opt-in path waits at most 60 seconds for the exact product HWND to
be foreground. It observes window/process availability and foreground state
every 100 ms; it performs no activation or input while waiting. Timeout or window
exit returns blocked, and the existing owned-child termination/wait path runs.
The default automatic foreground handshake is unchanged. Per-batch guards still
abort if focus leaves after handoff; attended waiting does not disable them or
establish native focus conformance.

No OS run of this new option is authorized until user availability is confirmed
and the coordinator instructs the worker. The three earlier blocked attempts are
not repeated. Future exact command will add `--await-foreground` to the command
below and use a new report path, announced before launch.

## Actual native attempts

Commands were announced to the coordinator before each launch. Final guarded
command, run from this worker root:

```powershell
& D:/legion-ide-2026-tools/native-input-target/debug/legion-input-driver.exe --open-edit-save-run --product D:/legion-ide-2026-candidate-r2/target/native-input-acceptance/package/legion-desktop.exe --workspace D:/legion-ide-2026-tools/ticket004-reference-c2a6578 --target README.md --report D:/legion-ide-2026-notes/ticket004-native-journey-guarded.toml
```

Three retained reports:

1. `ticket004-native-journey.toml`: native window observed, foreground refused,
   no input injected; report blocked/exit 3. The PowerShell tool wrapper returned
   1 for the native nonzero result; the report identifies the driver status.
2. `ticket004-native-journey-attempt2.toml`: repeated only the blocked attempt
   after matching the existing driver's ten 50-ms foreground retries. Explicit
   `$LASTEXITCODE` recorded 3; same blocked result, no input injected.
3. `ticket004-native-journey-guarded.toml`: coordinator requested before-batch
   focus guards and separate primary-save evidence; guarded build retried once,
   driver exit 3, same foreground refusal before any injected input. Native
   attempts then stopped and the prerequisite was escalated to the coordinator.

All reports reside in `D:/legion-ide-2026-notes/`. The only achieved native
observation is a visible top-level packaged-product window. Keyboard, pointer,
text, clipboard, command and active product IME composition were not observed.
No file edit/save, dirty-state transition or primary/palette save path passed.
The reports were preserved rather than overwritten, with no filesystem cleanup.

## Focused local verification

Checks were planned before changes at the public external-driver boundary.
All Cargo commands use `--target-dir D:/legion-ide-2026-tools/native-input-target`.

- New public CLI regression `native_journey_requires_explicit_workspace_and_target_without_launching_product`: red before implementation; green on its affected rerun. Commands: `cargo test -p legion-input-driver --test driver_contract <test-name> --target-dir <above>`. No real product or OS input in this test.
- Existing driver contracts: `cargo test -p legion-input-driver --test driver_contract --target-dir <above> -- --skip native_journey_requires_explicit_workspace_and_target_without_launching_product`: 13 passed, 0 failed, 1 filtered. Successful new regression was excluded rather than repeated.
- `focus_loss_blocks_external_input_batch_without_sending_text`: 1 passed, 0 failed; external input-boundary stub confirms a non-target foreground receives no batch. Headless component evidence only, not native focus conformance.
- `native_journey_missing_package_records_blocked_without_claiming_scenario_acceptance`: 1 passed, 0 failed; invokes the real driver with a nonexistent product and checks blocked report/full-scenario flags. No product launch or OS input.
- `cargo build -p legion-input-driver --target-dir <above>` passed for the initial journey and after wiring requested per-batch guards. Rebuild followed new guard changes, not an unchanged repeated check.
- Formatting and whitespace checks are recorded in the final coordinator handoff. No unrelated workspace gates or successful native checks were repeated.

Attended-option checks (no OS input/product launch):

- Public binary regression `attended_journey_flag_is_opt_in_and_rejected_outside_journey` failed red on unknown `--await-foreground`, then passed after implementation. A nonexistent product prevents any launch; the flag is rejected with probe mode.
- `cargo test -p legion-input-driver --test driver_contract attended_ --target-dir D:/legion-ide-2026-tools/native-input-target`: 2 passed, 0 failed (the affected CLI rerun plus new headless wait check for target/other window, window exit and exactly 60-second deadline). OS/time are injected only at the external driver boundary.
- `cargo test -p legion-input-driver --test driver_contract unattended_journey_default_preserves_existing_foreground_mode --target-dir D:/legion-ide-2026-tools/native-input-target`: 1 passed, 0 failed; default remains non-attended. Earlier successful tests were not repeated.
- Owned Rust formatting and `git diff --check` passed after this bounded addition. No attended native run, commit or acceptance promotion.

## Remaining scope and handoff

Ticket 04 remains open. The diagnostic below supersedes the earlier host-blocker
interpretation: the driver selected a helper HWND rather than the user-clicked
main window. Review the supported selector repair before a coordinator-owned
native retry. No edit/save outcome has yet been observed.

Full scenario steps 7–8 (second buffer/Save All and Save As encoding/newlines) and
RC-FOCUS-LOSS, RC-TRUST-DENIED, RC-EXTERNAL-OVERWRITE, RC-RESTART-DIRTY remain
explicit downstream work under the pilot contract (05/06/08/12/21 as applicable).
They are not waived or credited by this bounded ticket. Ticket 07/23 retains the
real CJK/AT prerequisite; all-input acceptance and ticket 26 pilot observation
remain separate. Canonical records, other workers' product code, map/execution
and original workspace WIP were not edited.

## Euclid reviewer delta before attended launch

P1/P2 fixed without OS input: pointer movement/down/up now use one guarded
SendInput batch. The journey selects exactly one TextPattern matching the complete
external baseline, allowing only CRLF/LF transport equivalence, and compares the
complete marker-plus-baseline text after typing. Ambiguous, truncated or unavailable
exact text blocks; shared first lines and marker substring matches cannot qualify.

Changed code paths for delta review: src/inject.rs, src/observe.rs, src/journey.rs
and tests/driver_contract.rs under crates/legion-input-driver. New focused tests
initially failed compilation on missing required APIs, then each passed after
repair: document_oracle_rejects_partial_or_extra_text_and_preserves_unicode and
pointer_click_delivers_move_down_up_in_one_external_batch. Exact invocation:
cargo test -p legion-input-driver --test driver_contract <test-name> --target-dir D:/legion-ide-2026-tools/native-input-target.
Both compiled the current debug driver. Earlier successful tests were not repeated.
Changed Rust formatting and whitespace checks passed. No attended/native launch,
OS input, commit or acceptance promotion. Awaiting delta review and coordinator
announcement before launch; user availability alone does not release the hold.

## Attended outcome and empirical window-selection repair

Coordinator session 54315 returned outer PowerShell exit 1; preserved report
`D:/legion-ide-2026-notes/ticket004-native-journey-attended.toml` records driver
exit_code 3, blocked after 60 seconds, zero injected input. Selected HWND was
0x481dc8, PID 32684. The wrapper/report exit discrepancy is recorded separately;
it is not evidence of a product failure. User confirmed seeing and clicking the
window. The report's wording about not foregrounding the exact window describes
the driver's identity comparison, not an established absence of user action.
README digest remained 5da9ac0a7844b4f215829bc2a6523d120fdbf97e3e1759234aa12623be29bc9a;
clone status was clean. Integration f4dbeed was merged before this repair.
Coordinator relayed Euclid PASS for the prior P1/P2 code delta; commit remains held.

Read-only diagnostic `D:/legion-ide-2026-notes/ticket004-foreground-diagnostic-r2.jsonl`
observed PID 31936: at 459 ms the first visible window was 0x51d6a, Winit Thread
Event Target, empty title, zero client area. Main HWND 0x491dc8 had title
`Legion IDE`, initially hidden, client area 960x720; it became visible at 1534 ms.
At 3210 ms foreground was that main HWND in the same PID, while the old selected
handle still did not match. No activation or input was used. The initial external
script failed before launch on bare `false`; coordinator corrected `$false` and
used a new report. No diagnostic completion/cleanup is inferred from the partial
observations while coordinator session 81879 is still running.

Supported repair changes only external window enumeration/selection: enumerate
all same-PID candidates and require exactly one visible, unowned `Legion IDE`
window with positive client width and height. Enumeration failure, ambiguity or
missing eligible window continues the bounded wait. Exact foreground equality
and per-input guards remain unchanged; other-process windows cannot qualify.

Focused regression initially failed compilation on the missing selector API,
then passed: `cargo test -p legion-input-driver --test driver_contract
product_window_selection_waits_past_observed_helper_and_rejects_ambiguity
--target-dir D:/legion-ide-2026-tools/native-input-target` (1 passed). Its observed
helper-before-hidden-main fixture also rejects wrong PID/title, zero area, owned
windows and ambiguity. `cargo build -p legion-input-driver --target-dir
D:/legion-ide-2026-tools/native-input-target` passed after this source change.
These are headless selector/build checks, not native input or scenario acceptance.
Delta review and coordinator-announced retry remain pending; no launch or commit.

Coordinator inspection caught an adapter error before launch: the local windows
binding converts `GetWindow(GW_OWNER)` null into Err, excluding valid unowned
windows. Replaced it with `GetWindowLongPtrW(GWLP_HWNDPARENT)` for enumerated
top-level windows, with cleared/checked last error distinguishing valid zero
owner from API failure. Build passed after this adapter correction. The successful
pure selector regression was not repeated; native adapter behavior remains for
review and the coordinator-owned retry (outcome below).

## Selector-fixed retry: COM prerequisite blocked

Coordinator's single selector-fixed attended retry completed in 7.39 seconds,
outer PowerShell exit 1. Preserved report
`D:/legion-ide-2026-notes/ticket004-native-journey-attended-selector-fixed.toml`
records blocked driver exit_code 3:
`CoInitializeEx failed: Access is denied. (0x80070005)`.
Coordinator observed selected main HWND 0x61d70, PID 52384, passing exact foreground
before failing UIA bootstrap. No input was injected; clone remained clean.
This is a new COM initialization blocker, separate from the empirically proven
wrong-helper selection. No edit/save or input-class acceptance is credited.
Coordinator relayed existing code PASS; final documentation review remains pending.

Read-only source inspection: UiaOracle::open calls
`CoInitializeEx(None, COINIT_APARTMENTTHREADED)` and checks the raw HRESULT before
`CoCreateInstance(CUIAutomation)`. No prior COM initialization was found in the
driver path; desktop attachment uses OpenInputDesktop/SetThreadDesktop. The local
windows binding passes a null reserved pointer and returns the native HRESULT
directly. The failure therefore precedes UIA instance creation, element lookup
and any input. Source does not establish why this host denies initialization.

One non-product, read-only PowerShell Add-Type probe called the same ole32 API
on a fresh managed Thread, uninitializing only on success. It returned
`fresh_thread_sta_hresult=0x80010106` (different apartment mode), not the driver's
0x80070005. Managed-thread initialization is not equivalent to the native driver
thread; this probe does not diagnose or clear the native blocker. No permission,
elevation, apartment-mode or COM security workaround was applied.

Diagnostic r2's completed report now records observation_end at 60041 ms and
termination of only spawned PID 31936, superseding the earlier in-progress note.
Bounded next step: coordinator-owned host investigation using a minimal native
COM-only probe in the same execution context, recording raw HRESULT before and
after the existing desktop-attachment sequence, with no product launch or input.
Root cause remains unproven; further native journeys are stopped. Ticket 04 stays
needs-info; partial deliverable is prepared for documentation review, uncommitted.

## Supported desktop-access repair (no product/input run)

Coordinator's fresh native COM-only probe succeeded with STA HRESULT 0x00000000,
exit 0 (`D:/legion-ide-2026-notes/ticket004-com-probe.rs`). General host COM denial
is therefore not established. Follow-up isolated native probe source:
`D:/legion-ide-2026-notes/ticket004-com-desktop-probe.rs`; binary:
`D:/legion-ide-2026-tools/ticket004-com-desktop-probe.exe`.
Compiled once with rustc --edition 2024; separate process invocations:

- `ticket004-com-desktop-probe.exe 0x81`: attached=true, STA 0x80070005, exit 1.
- `ticket004-com-desktop-probe.exe 0x83`: attached=true, STA 0x00000000, exit 0.

The only changed variable was DESKTOP_CREATEWINDOW (0x2). Microsoft's
[SetThreadDesktop contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddesktop)
states subsequent operations use the attached handle's access rights;
[desktop access rights](https://learn.microsoft.com/en-us/windows/win32/winstation/desktop-security-and-access-rights)
require DESKTOP_CREATEWINDOW for window creation. Results support the missing
window-creation access diagnosis for STA bootstrap after attachment.

Minimal session.rs repair requests READOBJECTS|WRITEOBJECTS|CREATEWINDOW (0x83),
with unchanged attachment failure handling, no ACL/elevation/security override,
and unchanged foreground/input guards. New public attachment-to-native-COM
regression failed red with HRESULT 0x80070005, then passed on affected rerun:
`cargo test -p legion-input-driver --test driver_contract
attached_input_desktop_allows_native_sta_com_initialization --target-dir
D:/legion-ide-2026-tools/native-input-target -- --ignored` (1 passed).
This interactive host-only test is opt-in and filtered into its own test process;
it opens no product or UIA client and sends no input. Driver build passed after
the new source change; prior successful checks were not repeated.
Code delta session.rs/tests/driver_contract.rs requires independent review.
COM bootstrap evidence is not UIA oracle or native journey acceptance. No launch,
commit or canonical promotion; ticket04 remains needs-info pending actual journey.

## Final bounded outcome: Explorer UIA prerequisite escalated

Coordinator's latest actual attended attempt used the COM-fixed driver. Preserved
report `D:/legion-ide-2026-notes/ticket004-native-journey-attended-com-fixed.toml`
records blocked driver exit_code 3:
`Explorer label "README.md" must identify exactly one accessible element; found 0`.
Outer PowerShell returned 1, separately from the report's driver code. Coordinator
confirmed window selection, exact foreground, COM and UIA bootstrap passed before
the Explorer lookup failed. No input was injected; disposable clone remained clean.
Candidate/reference/digest identities in this report match the frozen identities
above. No file open, edit, dirty transition, save path or disk effect passed.

After two supported driver repairs (wrong helper HWND and insufficient attached
desktop rights), the new UIA/product blocker is escalated. Native attempts are
stopped. Native widget-tree/Explorer accessibility investigation belongs to a new
bounded slice; this deliverable does not add fallback input, relax identity/text
oracles or substitute dispatch/--file for the required Explorer journey.

Integration 77166f70c8a38ce190d84b20584297b49e03b370 was merged ff-only, preserving
all owned WIP; incoming provider work did not overlap this patch. No successful
runtime checks were repeated after this merge. Final document/merge whitespace
verification is recorded in the handoff. Existing selector/P1/P2 review PASS does
not substitute for the pending session.rs delta and final documentation review.
Partial deliverable remains uncommitted until those reviews pass. Ticket04 stays
needs-info; full SC steps 7–8/recovery, all-input/CJK and pilot qualification remain
unaccepted. No canonical acceptance promotion, push, cleanup or further launch.

Final independent review PASS with no blocking findings, relayed by coordinator
on 2026-10-09, covers the session access delta and final documentation. Coordinator
authorized the partial commit and clean same-base ff-only integration merge.

## Compact Explorer navigation delta after read-only tree diagnosis

Resumed from the clean ticket004 branch and merged integration
1f647cc589779e4ee4b42707c3f9f3034bf1caa0 ff-only; ticket088 worktree preserved.
Coordinator supplied read-only UIA evidence at
`D:/legion-ide-2026-notes/ticket004-uia-observer/reports/uia-observation-1791522205266947100.jsonl`:
60 samples over approximately 15 seconds, stable 29 elements/27 named elements,
zero exact README.md matches, with `Explorer drawer`, `Bottom panel drawer`,
TERMINAL/PROBLEMS and no file entries. This observation injected no input; clone
remained clean. Product source render_compact_drawer_strip in view.rs confirms a
real `Explorer drawer` button toggles the compact layout. No startup tree change
was observed. This supports a missing navigation step, not a new product acceptance
result or a claim that the prior zero-match report was false.

Driver-only change: first use a unique exact visible target without toggling any
drawer. If absent, require exactly one visible `Explorer drawer`, read its current
enabled/visible positive UIA bounds, and click once using the existing guarded
atomic OS pointer batch. Poll the external tree every 100 ms for at most three
seconds after that click; require a unique exact visible target. Missing controls,
ambiguity, UIA property failures and timeout block further navigation. Re-read
current target bounds before its guarded click. No arbitrary coordinates, scroll,
directory traversal, direct dispatch, --file shortcut or product change added.
All later exact-document, dirty/save, disk/Git and per-input foreground guards stay
unchanged. A drawer click is input and is recorded if it occurs; no such click has
been performed in this slice.

Focused checks planned before edits at the existing external driver seam:

- `cargo test -p legion-input-driver --test driver_contract explorer_navigation_ --target-dir D:/legion-ide-2026-tools/native-input-target`: red on missing navigation API, then green on the affected rerun, 2 passed/23 filtered. Recorded-tree regression verifies one drawer click, bounded polling and visible-target preference. Negative regression covers missing/ambiguous exact labels, focus-loss no-send, post-click ambiguity and timeout without another click. These use recorded/modelled trees and input stubs; post-click target appearance is not an observed native result.
- `cargo build -p legion-input-driver --target-dir D:/legion-ide-2026-tools/native-input-target`: passed once after the source delta. No previously successful runtime checks repeated.
- Changed Rust formatting and `git diff --check` are the final source/doc checks; no broad gates or new qualification run.

New debug driver SHA256:
400d068d7e87cd78d0c3e3101b50d2056d55027506bc6ee4e5e3d08cfba7fadb,
at D:/legion-ide-2026-tools/native-input-target/debug/legion-input-driver.exe;
source is integration1f647cc plus this uncommitted navigation patch.
Coordinator archived the prior177caa5 binary as
D:/legion-ide-2026-tools/legion-input-driver-177caa5-adb9111c.exe, SHA256
adb9111c841bdb0cb2a6d48d12e90f2e48a787bba9e2c2b6bd344e43c6eb6a75.
Identity note: D:/legion-ide-2026-notes/ticket004-final-driver-identity.md.
That old hash was measured after the final COM-fixed run, not embedded in earlier
reports; do not attribute it to earlier driver revisions.

Review scope: journey.rs, observe.rs, driver_contract.rs, this evidence and issue04.
Build/source ready; independent delta review and coordinator-announced user
availability are required before the next native run. No commit or OS run in this
slice; ticket04 remains needs-info with no full scenario/input/pilot qualification.

Euclid independent navigation delta review PASS, relayed by coordinator on
2026-10-09: exact visible unique labels, current enabled positive bounds, guarded
atomic clicks, single drawer toggle/three-second deadline and fail-closed missing,
ambiguous or unreadable observations. Compiled binary hash400d068d above confirmed.
Coordinator authorized partial commit and clean same-base1f647cc ff-only merge.
User availability remains pending; no attended launch authorized yet. Earlier
successful checks were not repeated. Ticket04 remains needs-info.
