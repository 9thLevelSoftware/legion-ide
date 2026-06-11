---
title: Workspace Save Workflow
summary: "Saving is a proposal-mediated app workflow that uses editor snapshots and workspace preconditions, and rejected saves must preserve dirty editor text."
topics: [workspace, flows, decisions, architecture]
sources:
  - id: app-save-workflow
    type: file
    path: crates/legion-app/src/lib.rs
    note: Contains `SaveWorkflowService`, `AppSaveOutcome`, and `save_active_buffer`.
  - id: workspace-actor
    type: file
    path: crates/legion-project/src/lib.rs
    note: Contains `WorkspaceActor::save_file_with_proposal` and fail-closed save preconditions.
  - id: save-conflict-test
    type: file
    path: crates/legion-app/tests/workspace_vfs_integration.rs
    note: Verifies stale/conflict/denial outcomes preserve dirty editor text and disk state.
  - id: adr-0015
    type: file
    path: plans/adrs/ADR-0015-streaming-text-viewport.md
    note: Reasserts that the proposal-mediated save path remains authoritative during viewport work.
status: active
verified: 2026-06-08
---
`AppComposition::save_active_buffer` is not a direct disk write. It routes through `SaveWorkflowService`, which asks the editor for a `SaveRequestDto`, builds a save proposal, validates it, previews it, and then calls `WorkspaceActor::save_file_with_proposal` with the proposal and full precondition set [@app-save-workflow].

## Required preconditions

The workspace layer requires all of the following before a save can apply:

- expected fingerprint
- expected file content version
- expected workspace generation
- buffer version
- snapshot id
- payload byte length matching the text payload
- non-zero correlation and causality identity carried through the request path [@app-save-workflow] [@workspace-actor]

`WorkspaceActor::save_file_with_proposal` also canonicalizes the path, rechecks capability policy, rejects workspace-generation mismatches as stale proposals, and keeps non-atomic fallback disabled so the save path fails closed instead of silently downgrading [@workspace-actor].

## Rejection is a first-class outcome

`AppSaveOutcome` distinguishes `Saved` from `Rejected`, and the app acknowledges the editor with a typed `SaveAcknowledgement` rather than forcing the buffer clean on failure [@app-save-workflow]. The integration test `workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict` shows the expected behavior:

- an external overwrite makes the save return `AppSaveOutcome::Rejected`
- on-disk content stays as the external write
- the editor buffer keeps the user's dirty text
- the buffer enters a conflict lifecycle state instead of pretending the save succeeded [@save-conflict-test]

`[[plans/adrs/ADR-0015-streaming-text-viewport.md]]` calls this save path authoritative even while the text system shifts toward viewport-driven projections [@adr-0015].

## Consequence for future work

Any feature that wants to mutate files through language tooling, AI, remote execution, plugins, collaboration, or workflow automation must either produce proposals or reuse an existing proposal/workspace authority path. Direct writes are a violation of this repository's main control boundary. Related pages: [[projection-only-ui-and-desktop]], [[assisted-ai-and-delegated-tasks]], [[legion-workflow-orchestration]].
