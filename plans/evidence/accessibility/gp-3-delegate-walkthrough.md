# GP-3 Delegate Screen-Reader Walkthrough

## Status

- Walkthrough transcript: captured.
- Scope: delegated-task scoping, worker progress, evidence review, and proposal approval path.

## Transcript

VoiceOver focus moves into the delegate surface:

- "Delegates D."
- "Worker panel."
- "scoped task -> worker -> evidence -> review -> apply."
- "Live status."
- "Plan."
- "Tool calls."
- "Test evidence."
- "Proposal bundle."
- "Proposal evidence bundle."
- "Evidence panel."
- "No proposal evidence projected" when the surface is empty.

When a scoped worker is present, the same panel announces the structured delegate rows:

- "delegated task command center: projection=… plans=… blocked=… refused=… chat=… citations=… reviews=… permissions=… runtime=… autonomous_apply=unsupported redaction=…"
- "delegate chat …"
- "delegate citation …"
- "delegate proposal review …"
- "delegate proposal hunk …"
- "delegate tool permission …"
- "delegated task disclaimer: … autonomous apply unsupported"
- "delegated task plan …"
- "delegated task step …"
- "delegated task blocker …"
- "delegated task refusal …"
- "delegated task proposal preview … proposal-mediated"
- "delegated task audit readiness …"

The delegate path therefore exposes the worker, evidence, and proposal bundle as distinct accessible sections rather than a single undifferentiated blob.

## Product-level evidence used

- Worker panel labels: `crates/legion-desktop/src/view/worker_panel.rs`
- Proposal-review labels: `crates/legion-desktop/src/view/proposal_review.rs`
- Scope-picker labels and route boundaries: `crates/legion-desktop/src/view/scope_picker.rs`
- Delegate workflow routing: `crates/legion-desktop/src/workflow.rs`

## Notes

- This walkthrough stays within the proposal-mediated delegate surface.
- The transcript only names structured labels that are exposed by the current product shell.
