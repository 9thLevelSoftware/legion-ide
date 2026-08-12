# GP-3 Delegate Screen-Reader Walkthrough

**Updated:** 2026-07-07 (M10 PKT-GP3 — reflects M10 delegate surface)

## Status

- Walkthrough transcript: captured against M10 delegate surface.
- Scope: delegate mode activation, worker progress panel, scope picker, sandbox enforcement notice, kill switch, and proposal review path.

## Transcript

VoiceOver focus moves into the delegate surface:

- "Delegate D."
- "Worker panel."
- "Scope picker."
- "Target: Module."
- "Allowed tools: read, grep, glob, outline, edit-as-proposal."
- "Forbidden paths: 1 path protected."
- "Risk tolerance: Balanced."

When the delegate task is running, the worker panel announces live status:

- "Worker running."
- "Tool call: read src/main.rs."
- "Tool call: grep fn main src."
- "Tool call: edit-as-proposal src/main.rs."
- "Task complete: read, searched, and proposed an edit."

When a scope denial is triggered (forbidden-path read attempt):

- "Tool call rejected: read secrets.txt — forbidden path."
- "Task blocked: scope denial."

When cancellation is active (kill switch):

- "Task cancelled."

When the sandbox teeth step runs (TerminalCommand):

- "Tool call: terminal-command echo gp3-sandbox-probe."
- "Task complete: Terminal command executed." or "Task blocked: TerminalCommand denied."

Proposal review after a delegated-task run:

- "Proposal evidence bundle."
- "Evidence panel."
- "Checkpoint timeline."
- "proposal 800."
- "checkpoint timeline proposal=800 rows=1."
- "checkpoint 800."
- "target: delegated-task-gp3-proposal.txt."
- "kind=CreateFile."
- "available."
- "1 proposal row(s) with structured fields."
- "proposal 800."
- "payload kind: CreateFile."
- "lifecycle=approved rollback=available."
- "delegated task audit readiness: all tool calls paired."
- "delegated task disclaimer: autonomous apply unsupported."

The delegate path exposes worker status, scope enforcement, audit steps, and proposal bundle as distinct accessible sections.

## Product-level evidence used

- Worker panel labels: `crates/legion-desktop/src/view/worker_panel.rs`
- Proposal-review labels: `crates/legion-desktop/src/view/proposal_review.rs`
- Scope-picker labels and route boundaries: `crates/legion-desktop/src/view/scope_picker.rs`
- Delegate workflow routing: `crates/legion-desktop/src/workflow.rs`
- GP-3 smoke binary: `crates/legion-app/src/bin/golden_path_3.rs`
- GP-3 evidence report: `target/golden-path/gp3_report.toml` (written by xtask golden-path-3)

## Platform enforcement caveats

- **Linux (Landlock):** Filesystem write enforcement active; sandbox_probe.txt write inside worktree succeeds, writes outside sandbox root are denied.
- **macOS (Seatbelt):** Filesystem write enforcement active; same policy as Linux.
- **Windows (Job object only):** `filesystem_write_enforced=false`; TerminalCommand runs through the Job object boundary only; filesystem isolation is not enforced at the OS level. This is a known caveat documented in the M10 evidence.

## Notes

- This walkthrough stays within the proposal-mediated delegate surface.
- Autonomous apply remains unsupported; all edits are proposal-mediated.
- The transcript names structured labels exposed by the current M10 product shell.
- Sensitive command text stays redacted; only structured audit metadata is announced.
