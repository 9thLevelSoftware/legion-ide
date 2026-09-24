# P9.F3.T3 — Cloud Lane egress manifest and mid-flight cancellation

**Date:** 2026-08-19
**Task:** P9.F3.T3 — productize Cloud Lane with visible upload scope, budget,
cancellation, and egress manifest
**Authorisation:** ADR-0046 Amendment 1 (owner decision, 2026-08-19)
**Acceptance:** "Every Cloud Lane upload is paired with a visible egress
manifest and is cancellable mid-flight."
**Stop condition:** "Stop if a Cloud Lane upload can complete without surfacing
its manifest."

## What was already there, and why it did not meet the bar

Cloud Lane had real substrate: `LegionCloudLaneClient` with cost and upload
caps, a transport trait with `submit_task` / `stream_task_events` /
`cancel_task`, DTO validators, and a security broker that denies
`cloud.lane.submit` on several grounds. The gaps were not missing code. They
were three places where something *looked* enforced and was not.

**1. "Visible" was a boolean the caller set.**
`LegionCloudLaneUploadManifest::scope_visible_to_user` is documented as
"whether the user-visible upload scope was presented before submission".
`validate_legion_cloud_lane_upload_manifest` rejects `false`, and
`DenyByDefaultBroker` denies submit when `require_scope_visibility` is set and
the flag is absent. So the *flag* was enforced rigorously — and nothing in the
tree ever rendered a manifest. The flag attested to an event that could not
happen. A caller set it to `true` and uploaded whatever it liked, passing every
check on the way.

**2. Cancellation existed everywhere except where it could be reached.**
`LegionCloudLaneTransport::cancel_task` has been on the trait since the
substrate landed, with validation for the cancellation token and the reason
label. `AppComposition` exposed `submit_legion_cloud_lane_task` and
`legion_cloud_lane_projection` and no cancel at all. "Cancellable mid-flight"
was true of the transport and false of the application.

**3. Nothing rendered any of it.** `legion_cloud_lane_projection()` was built,
validated with a `debug_assert!`, and read by no desktop or UI code.
`crates/legion-desktop/src/view/cloud_lane.rs` — the file this task names — did
not exist.

## What changed

### The acknowledgement (`crates/legion-app/src/cloud_lane_egress.rs`)

`CloudLaneEgressManifestView::from_request` builds what a renderer shows: every
allowed *and* forbidden scope, the byte total, the estimated cost against the
cap, whether that cap is enforced, and the secret-scan disposition. Withheld
scopes are rendered too — "what did it keep back?" is the question a user asks
to decide whether to trust the answer to "what is it sending?".

`acknowledge()` is the only way to obtain a `CloudLaneEgressAcknowledgement`,
and `submit_legion_cloud_lane_task` now requires one.

**The acknowledgement carries a digest over the manifest's contents, not its
id.** Binding to the id alone would stop nothing: show a two-file manifest,
acknowledge it, submit two hundred under the same task id. The digest covers
every scope with its disposition, the byte total, both cost figures, the
hard-cap flag, and the scan status. It is length-prefixed for the same reason
the extension signing payload is (P7.F2): a delimiter-joined encoding lets one
field's contents impersonate a field boundary.

### Cancellation

`AppComposition::cancel_legion_cloud_lane_task` refuses a blank reason, an
untracked task, and any task already in a terminal state. That last refusal is
the point: a cancel that reports success against a finished upload tells the
user their data was withheld when it has already left.

The path is complete rather than partial: `DesktopAction::CancelCloudLaneTask`
→ `DesktopCommandBridge` guard → `CommandDispatchIntent::CancelCloudLaneTask` →
`AppCommandRequest::CloudLane` → app authority. The bridge repeats the terminal
-state check because the view model withholding a button does not stop a
keybinding or command-palette entry synthesising the same action.

### The panel (`crates/legion-desktop/src/view/cloud_lane.rs`)

One row per task with state, bytes, cost, a cancel control on non-terminal rows
only, and a badge reporting whether that upload's scope was surfaced — shown for
every row, not only the bad ones, because its absence has to be visible rather
than inferred. Rendered in the Legion Workflows rail, which Manual mode never
reaches; the `RemoteWorkspace` panel carrying `CloudProvider`/`NetworkEgress` is
already excluded from Manual by a capability-based regression suite in
`legion-ui`.

## What is deliberately not here

**There is no pre-submit dialog in the desktop, because there is no desktop
submit flow.** Cloud Lane submission is programmatic today. I wrote a
`DesktopCloudLaneEgressViewModel` and a painter for it, found they had no
caller, and deleted both rather than ship a surface nothing draws — that is the
same dead-surface defect this task exists to fix, and clippy's `dead_code`
warning was the thing that caught me. The pre-submit manifest is rendered at the
app layer via `CloudLaneEgressManifestView::rendered_lines()`, which is what the
acknowledging caller displays.

So the honest statement of the acceptance: **every submit is now bound to a
manifest that was built and acknowledged, with the acknowledgement invalidated
by any change to what was shown; and every in-flight task is cancellable from
the panel.** A desktop submit flow with a pre-submit review dialog is the
remaining product work, and PR-ENT-002 stays deferred in the ledger for that
reason among others.

## Coverage

`crates/legion-app/tests/cloud_lane_egress.rs` (9) and
`crates/legion-desktop/tests/cloud_lane_panel.rs` (7).

| Test | What it pins |
| --- | --- |
| `the_manifest_lists_what_leaves_and_what_is_withheld` | Uploaded and withheld scopes both render, ordinals dense, summary carries bytes/cost/cap. |
| `an_acknowledgement_covers_only_the_manifest_it_was_shown_for` | Extra file, changed estimate, unenforced cap, raised cap, changed byte total, different task — all invalidate. |
| `a_withheld_file_moving_into_the_upload_invalidates_the_acknowledgement` | Row count and byte total held constant, so only a digest over dispositions catches it. |
| `submit_is_refused_without_an_acknowledgement_for_that_exact_upload` | The swollen manifest is refused and creates no row; the acknowledged one submits. |
| `an_in_flight_task_can_be_cancelled_and_a_finished_one_cannot` | Cancel works once, reports the reason, and refuses a second time. |
| `cancelling_an_unknown_task_is_refused` / `a_cancellation_reason_is_required` | No silent success. |
| `the_projection_carries_the_manifest_visibility_flag_into_the_shell` | The flag reaches `ShellProjectionSnapshot`. |
| `an_in_flight_task_gets_a_cancel_control_and_a_finished_one_does_not` | Controls exist exactly where cancelling is real, across all five states. |
| `the_bridge_translates_a_cancel_into_an_app_intent` | The button reaches app authority. |
| `the_bridge_refuses_...unknown` / `...finished_task_even_when_synthesised` | The guard holds against callers other than the button. |
| `a_disabled_runtime_renders_its_reason_rather_than_an_empty_panel` | Disabled and idle are distinguishable. |
| `the_panel_reports_whether_each_upload_had_its_scope_surfaced` | The visibility flag reaches the panel unchanged. |

## Mutation testing

Each guard broken, suite run, source restored, `git status` clean afterwards.

| # | Mutation | Result |
| --- | --- | --- |
| M1 | acknowledgement compares task id only, ignoring the digest | KILLED (3 tests) |
| M2 | submit does not check the acknowledgement | KILLED |
| M3 | digest omits dispositions, hashing paths only | KILLED |
| M4 | cancel permits terminal states | KILLED |
| M5 | view model offers cancel on terminal tasks | KILLED |
| M6 | bridge does not refuse terminal cancels | KILLED |
| M7 | bridge accepts unknown task ids | KILLED |
| M8 | digest omits the cost cap the user was shown | **SURVIVED, then KILLED** |

M8 is the one worth reading. The acknowledgement test changed the *estimated*
cost and the hard-cap flag but never the *cap itself*, so removing
`max_cost_cents` from the digest broke nothing. A user shown "75 cent cap" and
submitted under a 7500 cent cap is the same bait-and-switch as swapping the file
list. Case added, mutation re-run: KILLED.

## Verification

```
cargo test -p legion-app --test cloud_lane_egress      # 9 passed
cargo test -p legion-desktop --test cloud_lane_panel   # 7 passed
cargo test --workspace                                 # 327 suites ok
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p xtask -- extract-before-modify            # chokepoints within slack
```

## Readiness

`PR-ENT-002` stays *Deferred with explicit cut line*. ADR-0046 Amendment 1
grants permission to build this surface; it is not evidence of collaboration and
admin controls being product workflows, and the `deferred-surfaces` gate still
requires ADR, policy, tests and product evidence before that row moves. This
document is one of those four, not all of them.
