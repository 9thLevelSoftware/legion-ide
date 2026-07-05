# Desktop Views Part 2 Review

Scope reviewed:
- `crates/legion-desktop/src/view/agent_comm.rs`
- `crates/legion-desktop/src/view/assistant_rail.rs`
- `crates/legion-desktop/src/view/cloud_lane.rs`
- `crates/legion-desktop/src/view/fleet_card.rs`
- `crates/legion-desktop/src/view/manifest_panel.rs`
- `crates/legion-desktop/src/view/provider_setup.rs`
- `crates/legion-desktop/src/view/worker_panel.rs`

Summary: 8 findings. No production TODO/FIXME/HACK, `todo!()`, or `unimplemented!()` stubs were found in the scoped files.

Severity breakdown:
- Critical: 0
- High: 4
- Medium: 4
- Low: 0

Category breakdown:
- Bug: 4
- Stub: 0
- Error: 0
- Failure-point: 4

## `crates/legion-desktop/src/view/agent_comm.rs`

No findings.

## `crates/legion-desktop/src/view/assistant_rail.rs`

### Finding 1
- Category: bug
- Severity: high
- Line numbers: 58, 132-136
- Description: Every fenced code block gets an enabled `Apply as proposal` affordance whenever the caller supplies any `proposal_id`. The renderer then dispatches `DesktopAction::ApplyProposal { proposal_id }` from the code-block UI without proving that the code block corresponds to that proposal, and without requiring the streamed fence to be complete. Current call sites pass `first_proposal_id(snapshot)`, which falls back to the selected or first proposal in the ledger, so an unrelated assistant/code-block row can apply the wrong proposal.
- Suggested fix direction: Do not derive proposal application from arbitrary assistant markdown. Bind the affordance to a proposal-preview/proposal-ledger row that owns the proposal id, require the code block or proposal preview to be complete and reviewable, and disable or hide the button for generic assistant text. Consider making incomplete code blocks render only a preview/streaming state.

## `crates/legion-desktop/src/view/cloud_lane.rs`

### Finding 1
- Category: failure-point
- Severity: medium
- Line numbers: 31-41, 65-72
- Description: The panel always appends `cancellation: mid-flight cancel is available while the task is not terminal`, even when the cloud runtime is disabled, when there are no submitted tasks, or when every task row is terminal. This can advertise a cancellation affordance that is not actually available and conflicts with the per-row `cancelable=false` computation.
- Suggested fix direction: Only emit a cancellation-available row when `runtime_enabled` is true and at least one projected task is cancelable. Otherwise show a disabled/not-available row that explains whether the runtime is disabled, there are no active tasks, or all tasks are terminal.

## `crates/legion-desktop/src/view/fleet_card.rs`

### Finding 1
- Category: bug
- Severity: high
- Line numbers: 15-21, 32-39, 64-88
- Description: `fleet_card_view_models` turns every proposal-ledger row into a card and `render_card` always exposes `Approve`, `Review`, and `Reject` actions. The card projection only carries the lifecycle label, not the lifecycle state or action readiness, so already-applied, rejected, denied, failed, stale, conflicted, or cancelled proposals can still present active approval/rejection controls. This is especially risky because the surrounding empty state says `No pending proposals`, but the code does not filter to pending/actionable proposals.
- Suggested fix direction: Include proposal lifecycle/action-readiness in the fleet card view model. Filter to actionable pending states or disable buttons for terminal/non-actionable states, and gate approval on the same approval checklist/readiness used by the proposal workflow.

### Finding 2
- Category: failure-point
- Severity: medium
- Line numbers: 21-29
- Description: The renderer hard-limits the fleet cards to the first four cards with `cards.iter().take(4)` and does not show an overflow count. Any additional proposal rows are silently hidden, so users cannot tell that more pending or problematic proposals exist.
- Suggested fix direction: Render an explicit overflow row such as `N more proposals`, support paging/scrolling, or honor the projection's omitted-row metadata so hidden cards remain visible to the operator.

## `crates/legion-desktop/src/view/manifest_panel.rs`

### Finding 1
- Category: failure-point
- Severity: medium
- Line numbers: 8-10, 28-36
- Description: The manifest preview can report `no context items projected before invocation` whenever `manifest.items` is empty, even if `omitted_item_count`, stale metadata risk, permissions, or other manifest-level risk fields indicate context was omitted or redacted. For non-empty manifests it renders only the first 12 items without an overflow row, so additional context/egress-relevant items are silently hidden.
- Suggested fix direction: Include manifest-level `omitted_item_count`, stale/missing metadata risk, privacy/risk/egress, and permission counts in the summary even when `items` is empty. When truncating with `take(12)`, add an explicit `N more items omitted from preview` row.

## `crates/legion-desktop/src/view/provider_setup.rs`

### Finding 1
- Category: failure-point
- Severity: medium
- Line numbers: 44-75, 77-80
- Description: `provider_policy_rows` re-derives policy labels from `provider_class` using hard-coded strings and ignores the provider summary's actual `availability`, `refusal`, `risk_label`, `privacy_label`, and route/consent state. It also maps `Gateway` to `Unknown` despite the protocol documenting gateway as a future managed remote class that does not authorize network egress, which can understate the fail-closed posture operators should see.
- Suggested fix direction: Build policy rows from the authoritative provider/route projection fields and refusal metadata rather than static class labels. Treat `Gateway` and `Unknown` conservatively as denied/unavailable unless explicit route consent and policy approval are projected.

## `crates/legion-desktop/src/view/worker_panel.rs`

### Finding 1
- Category: bug
- Severity: high
- Line numbers: 197-206
- Description: Waiting-for-approval recovery actions are discovered by searching `row.display_safe_labels` for a `signoff:` prefix, but real `LegionWorkflowProjectionRow` construction currently populates `display_safe_labels` from worker model labels, conflict labels, and verification gate labels, not from `sign_off_records`. As a result, real workflows that need sign-off can fail to show the `Request sign-off` recovery action; the unit test only passes because it fabricates `signoff:` labels directly in the projection row.
- Suggested fix direction: Project sign-off identifiers/labels into the workflow row explicitly, or add a dedicated recovery-action projection sourced from `sign_off_records`. Avoid relying on generic display labels for required approval metadata.

### Finding 2
- Category: bug
- Severity: high
- Line numbers: 213-235, 241-250, 259-264
- Description: Verification and conflict recovery actions construct typed IDs from display-safe label text (`LegionWorkflowVerificationGateId(gate_id)` and `LegionWorkflowConflictId(conflict_id)`). The upstream projection adds verification gate labels and conflict labels to `display_safe_labels`, not necessarily the stable `gate_id` or `conflict_id`. Human-readable labels can therefore produce invalid IDs, miss available recovery actions, or dispatch actions for the wrong gate/conflict.
- Suggested fix direction: Carry stable `gate_id` and `conflict_id` values in the projection/recovery view model and dispatch those typed IDs directly. Keep display labels only for UI text, not as an ID transport mechanism.
