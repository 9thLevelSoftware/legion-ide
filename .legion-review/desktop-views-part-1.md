# Desktop Views Part 1 Review

Scope reviewed:
- `crates/legion-desktop/src/view.rs`
- `crates/legion-desktop/src/view/code_canvas_painter.rs`
- `crates/legion-desktop/src/view/proposal_review.rs`
- `crates/legion-desktop/src/view/plan_editor.rs`
- `crates/legion-desktop/src/view/scope_picker.rs`
- `crates/legion-desktop/src/view/sandbox_panel.rs`
- `crates/legion-desktop/src/view/privacy_inspector.rs`

## Summary

Findings count: 11

Severity breakdown:
- Critical: 0
- High: 4
- Medium: 6
- Low: 1

Category breakdown:
- Bug: 5
- Stub: 1
- Error: 1
- Failure-point: 4

## `crates/legion-desktop/src/view.rs`

### Finding 1
- Category: bug
- Severity: medium
- Line numbers: 1859-1910, 2082-2104, 4296-4304
- Description: Drag selection ranges are emitted as `{ start: drag_anchor, end: coordinate }` without normalizing start/end order. The paint path in `selection_span_for_line` assumes `start <= end`; if the user drags backwards on the same line or across lines, the computed range can have `start` after `end`, so the selection highlight disappears and downstream editor actions may receive an inverted `ProtocolTextRange`.
- Suggested fix direction: Normalize text ranges before emitting `DesktopAction::SetSelection`, or make `selection_span_for_line` and the command handler canonicalize ranges consistently for backwards selections.

### Finding 2
- Category: stub
- Severity: medium
- Line numbers: 2704-2712, 2725-2728
- Description: The Legion workflow header renders `Pause Workflow` and `Add Constraint` buttons but discards their responses, so the controls are visible and clickable-looking while producing no `DesktopAction`. The same surface hard-codes `confidence 87%`, which can mislead operators because it is not derived from projection state.
- Suggested fix direction: Either wire these controls to real workflow pause/constraint actions and projected confidence data, or render them disabled/hidden with explicit unavailable text until supported.

### Finding 3
- Category: bug
- Severity: high
- Line numbers: 3333-3336
- Description: The command center always renders the delegated-task scope picker with `DesktopScopePickerViewModel::default()`. That shows a repo-scoped, balanced-risk, read-only default regardless of the active delegated task's actual target, risk tolerance, allowed tools, or forbidden paths. Because scope is a trust-boundary surface, this can display false safety information to a human reviewer.
- Suggested fix direction: Build the scope picker view model from the task/snapshot scope projection, and show an explicit missing-scope warning if no scope is projected.

### Finding 4
- Category: failure-point
- Severity: high
- Line numbers: 3389-3499
- Description: Hunk-review controls silently cap visible reviews to 4, file groups to 6, and hunk rows to 6. Hidden reviews/files cannot be accepted, rejected, marked pending, or edited from this surface, and there is no overflow warning or navigation. A large proposal can therefore leave unreviewed hunks invisible to the operator.
- Suggested fix direction: Add pagination/scrolling/overflow counts and actions for all reviews/files/hunks, or provide proposal-level controls that deliberately include hidden items with clear counts.

### Finding 5
- Category: bug
- Severity: medium
- Line numbers: 5490-5497
- Description: `git_relative_path` uses plain string `strip_prefix(root)` and then trims separators. This is not path-boundary aware: a file such as `/repo2/src/lib.rs` can be treated as relative to root `/repo`, yielding `2/src/lib.rs`. That can mis-associate git hunks/blame with the wrong active buffer when workspace roots share prefixes.
- Suggested fix direction: Use `Path::strip_prefix` on normalized/canonical paths, or explicitly require a separator/path-component boundary after the root prefix before accepting the relative path.

## `crates/legion-desktop/src/view/code_canvas_painter.rs`

No findings identified in this file.

## `crates/legion-desktop/src/view/proposal_review.rs`

### Finding 6
- Category: bug
- Severity: medium
- Line numbers: 229-241, 341-347
- Description: `proposal_evidence_panel` treats `ProposalId(0)` as a sentinel for an absent checkpoint proposal, but if there is no selected proposal it still returns `Some(checkpoint_projection.proposal_id)`, which is `Some(ProposalId(0))`. The renderer then displays `checkpoint timeline proposal=0`, creating a fake proposal association.
- Suggested fix direction: Return `None` when the checkpoint projection has the sentinel id and no selected proposal exists; only render the timeline proposal label when the id is real.

### Finding 7
- Category: failure-point
- Severity: low
- Line numbers: 255-268
- Description: The evidence panel truncates proposal rows to 4 and verification rows to 6 without any overflow count or indication that additional evidence exists. This can make a proposal look less supported or less risky than it is when later rows contain warnings, failed verification runs, or relevant provenance.
- Suggested fix direction: Include hidden-row counts and a way to expand/paginate evidence rows, especially for failed or high-risk verification entries.

## `crates/legion-desktop/src/view/plan_editor.rs`

### Finding 8
- Category: bug
- Severity: medium
- Line numbers: 128-134
- Description: `render_plan_editor` only appends missing draft section bodies and never reconciles an existing draft when `model.sections` shrinks, reorders, or changes kind. Because drafts are keyed only by artifact id in `view.rs`, a refreshed plan artifact can display stale body text under the wrong section heading.
- Suggested fix direction: Key draft entries by stable section kind/id rather than vector index, and reconcile/remove stale entries whenever the model section list changes.

## `crates/legion-desktop/src/view/scope_picker.rs`

### Finding 9
- Category: failure-point
- Severity: medium
- Line numbers: 67-81, 87-99
- Description: The conversion from `DesktopScopePickerViewModel` to `DelegatedTaskScope` permits `File` or `Module` scopes with `target_path: None`, and the summary falls back to the placeholder strings `file` or `module`. The resulting protocol scope denies all concrete file/module targets while the UI presents it as a selected scope, making it easy to create an unusable delegated task scope.
- Suggested fix direction: Validate that `target_path` is present for file/module targets before conversion, expose a validation error in the picker, and avoid placeholder summaries that look like real paths.

## `crates/legion-desktop/src/view/sandbox_panel.rs`

### Finding 10
- Category: error
- Severity: high
- Line numbers: 74-119
- Description: `host_profile_summary` compiles and displays a sandbox profile for the hard-coded path `/workspace/project` instead of the active workspace/task scope. On Windows it also unwraps the profile compilation with `expect`, so a future real compile error would panic while rendering the panel. The result is a security surface that can report a strong sandbox for a dummy scope rather than the actual runtime boundary.
- Suggested fix direction: Derive the sandbox scope from the active snapshot/task workspace, propagate compile errors into caveat rows instead of panicking, and clearly distinguish host capability from the actual allocated sandbox.

## `crates/legion-desktop/src/view/privacy_inspector.rs`

### Finding 11
- Category: failure-point
- Severity: high
- Line numbers: 19-32, 39-68
- Description: The privacy inspector header reports aggregate counts for all records, but the detail rows silently show only the first 10 records. If high-risk, denied, external-egress, or redacted records occur after the first 10, the operator sees aggregate counts without the corresponding record details or an omitted-record warning.
- Suggested fix direction: Show an explicit omitted count and prioritize denied/high-risk/external-egress records before truncation, or add expansion/pagination so every exposure record can be inspected.
