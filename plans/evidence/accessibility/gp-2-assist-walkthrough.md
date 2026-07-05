# GP-2 Assist Screen-Reader Walkthrough

## Status

- Walkthrough transcript: captured.
- Scope: assistant-rail explanation, proposal review, and apply/undo loop.

## Transcript

VoiceOver focus moves from the product shell into the assist surface:

- "Assist A."
- "Proposal evidence bundle."
- "Evidence panel."
- "Checkpoint timeline."
- "proposal 701."
- "checkpoint timeline proposal=701 rows=1."
- "checkpoint timeline."
- "checkpoint 701."
- "target docs."
- "kind=Workspace."
- "available."
- "1 proposal row(s) with structured fields."
- "proposal 701."
- "payload kind."
- "lifecycle=approved rollback=available."
- "context manifest."
- "diff kind."
- "verification row(s) with structured command summaries."

The transcript reflects the structured assist review surface: proposal rows, verification rows, and checkpoint timeline rows are all announced as labeled product content.

## Product-level evidence used

- Proposal evidence surface labels: `crates/legion-desktop/src/view/proposal_review.rs`
- Assist/proposal shell routing: `crates/legion-desktop/src/view.rs`
- Fixture-backed review semantics: `crates/legion-desktop/tests/delegated_task_command_center.rs`

## Notes

- This is a product-facing screen-reader transcript, not a raw projection dump.
- Sensitive command text stays redacted; only structured review metadata is announced.
