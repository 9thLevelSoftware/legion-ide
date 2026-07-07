# PKT-PROPOSAL-SURFACE — Evidence

**Campaign:** M10  
**Date closed:** 2026-07-07  
**Status:** DONE

## What was implemented

### 1. `ToolExecutionOutput` return type + agent_loop.rs plumbing

- Added `pub struct ToolExecutionOutput { pub content: String, pub proposal: Option<AssistedAiEditProposalOutput> }` to `crates/legion-agent/src/agent_loop.rs`.
- Changed `execute_edit_as_proposal` to return `Result<ToolExecutionOutput, LegionToolCallFeedback>`, keeping all existing proposal-building logic and returning the `AssistedAiEditProposalOutput` alongside the text summary.
- Updated `validate_and_execute` to return `Result<ToolExecutionOutput, LegionToolCallFeedback>`. Non-proposal tools (Read, Grep, Glob, Outline, TerminalCommand, McpPassthrough) wrap their `String` outputs with `proposal: None`.
- Extended `DelegatedTaskLoopResult::Completed` with `proposals: Vec<AssistedAiEditProposalOutput>`. Only `Completed` carries proposals — `Blocked`, `BudgetExhausted`, and `Cancelled` discard partial proposals.
- `run_delegated_task_loop` accumulates proposals in a `Vec` across all model turns and passes them in the `Completed` result.

### 2. TDD evidence (red → green)

Tests written BEFORE implementation (red: compile failure on missing `proposals` field):

- `proposal_surfacing_single_edit` — scripted provider does read→edit-as-proposal→end_turn; asserts `Completed` carries exactly 1 proposal targeting `src/main.rs`. **GREEN.**
- `proposal_surfacing_multi_edit` — scripted provider does 2 edit-as-proposal calls; asserts 2 proposals in submission order. **GREEN.**
- `blocked_run_discards_proposals` — scripted provider does edit-as-proposal (succeeds) then read outside scope (Blocked); asserts loop returns `Blocked` with no proposals surfaced. **GREEN.**

All 12 agent_loop_integration tests pass (0 failures).

### 3. App-side proposal registration in `start_delegated_task`

File: `crates/legion-app/src/lib.rs`

- When `DelegatedTaskLoopResult::Completed { proposals }` is received, each proposal gets a real `ProposalId` via `self.proposal_coordinator.next_id()`.
- `WorkspaceProposal` is built directly from the proposal output (bypassing `to_workspace_proposal()` which requires full buffer/snapshot versioning that the agent loop does not have).
- Each proposal is registered via `self.register_proposal_lifecycle(&workspace_proposal)`.
- `AppDelegatedTaskOutcome::Completed.proposals` is populated with the re-stamped proposals.
- Deferral comment removed.

Integration test coverage: all 14 `delegated_task_integration` tests pass, including `execute_delegated_task_returns_proposal_after_explicit_write_allow` and `start_delegated_task_audit_steps_are_paired_for_tool_call`.

### 4. GP-3 s3 tightened (PKT-GP3 deferral closed)

File: `crates/legion-app/src/bin/golden_path_3.rs`

Removed the permissive `eprintln!("[s3] ... (extraction deferred per PKT-PROPOSAL-SURFACE)")` assertion.

Replaced with strict assertions:
- `assert proposals.len() == 1`
- Assert `proposal.payload` targets `src/main.rs` (CreateFile, path ends_with "main.rs")
- Assert ledger row exists + hunk review dispatched via `app.review_delegate_proposal_hunk(proposal_id, "delegate:proposal:{id}:metadata-chunk:0", Accepted)`

s8's `ProposalId(800)` is hardcoded; s3 proposals get sequentially assigned IDs (1+) — no collision.

### 5. Ripple sites fixed

Only one ripple site: `lib.rs:18070` (match on `DelegatedTaskLoopResult::Completed`). Fixed as part of deliverable 3.

The test file at `agent_loop_integration.rs:251` also had `if let Completed { final_message }` which was updated to `{ final_message, .. }`.

## Files changed

- `crates/legion-agent/src/agent_loop.rs` — `ToolExecutionOutput`, updated `Completed`, threaded proposals
- `crates/legion-agent/tests/agent_loop_integration.rs` — 3 new TDD tests (proposal_surfacing_single_edit, proposal_surfacing_multi_edit, blocked_run_discards_proposals)
- `crates/legion-app/src/lib.rs` — register delegate proposals in `start_delegated_task`
- `crates/legion-app/tests/delegated_task_integration.rs` — new `start_delegated_task_surfaces_proposal_and_review_succeeds` integration test
- `crates/legion-app/src/bin/golden_path_3.rs` — tightened s3; added `DelegatedTaskProposalHunkDisposition` import; corrected hunk_id to `metadata-chunk:0`
- `xtask/src/main.rs` — updated s3 doc comment to reflect proposal assertion

## Self-review findings

None. The implementation is complete with no stubs or deferred work.

## Concerns

The agent loop stamps `ProposalId(0)` as a placeholder, and app-side `proposal_coordinator.next_id()` reassigns real IDs. This means the proposals in `AppDelegatedTaskOutcome::Completed.proposals` have the real IDs (set before `register_proposal_lifecycle`), not the loop-assigned `ProposalId(0)`. This is correct per the architecture.

The `WorkspaceProposal` construction bypasses `to_workspace_proposal()` because agent-loop preconditions lack `buffer_version`, `snapshot_id`, and `workspace_generation` — these are app-level concepts that the agent loop (which operates on the sandbox filesystem) cannot populate. A future packet could enrich preconditions at the app layer after the loop completes.
