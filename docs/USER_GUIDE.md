# Legion User Guide

This guide is the end-user entry point for the current Legion product paths.
It assumes the reader already has a working build or a packaged desktop app.

> **Current state caveat.** The repo currently proves a validated substrate and a deterministic desktop projection workflow (CLI `:w` / `:q`, projection-only UI, headless desktop smoke harness). It is **not** a renderer-backed daily-driver product yet. Treat anything below as a description of the design and the gated surfaces that are exercised by tests, not as a claim of a shipped user experience. For the readiness matrix and remaining product gaps, see `plans/product-readiness-ledger.md` and `README.md` "Current Status".

> **Product areas that are currently projection-only, gated, or otherwise not yet productized.** The following are explicitly *not* full product paths today: terminal productization (real PTY behind capability policy; currently denied/fixture-gated in app-level paths), debug productization (DAP is fixture/projection; no real product debug adapter launch yet), runtime plugin execution (manifest/capability boundary only; no live WASM host), collaboration GUI / production collaboration, remote workspace / Cloud Lane UX, and signing / notarization / auto-update / crash reporting (dry-run descriptors only; no private signing credentials may be committed). Autonomous apply/merge is unsupported outside explicitly approved proposal paths. See `docs/LEGION_PIVOT.md` and `plans/legion-production-master-plan-v0.2.md` for the path to activating these surfaces.

## Start here

1. Read `docs/INDEX.md` for the canonical documentation map.
2. Use `docs/MODES.md` to understand what each product mode allows and forbids.
3. Use `docs/KEYBOARD_REFERENCE.md` for the current projected shortcut labels.
4. Use `docs/TROUBLESHOOTING.md` when a smoke, package, or release path fails.

## Core product paths

### Manual

Manual mode is the deterministic local editing path.
Use it when you want the projection-only UI, workspace navigation, and trusted local file operations without any AI or worker surfaces.

### Assist

Assist mode keeps the human in control while exposing AI-backed suggestions.
Use it when you want previews, explanations, and proposal-mediated edits without giving the model direct mutation authority.

### Delegate

Delegate mode is for bounded worker execution.
Use it when a task should run in a disposable lane with explicit scope, evidence, and review before anything reaches the main workspace.

### Legion Workflows

Legion Workflows coordinates multi-step product workflows.
Use it for task graphs, approval gates, risk tracking, and release-oriented orchestration.

## Support and release surfaces

- For packaging and release preparation, start with `docs/OPERATOR_RUNBOOK.md`.
- For diagnostic exports, session state, and bug-report payloads, use `docs/TROUBLESHOOTING.md`.
- For release-readiness status, check `plans/product-readiness-ledger.md`.

## Source control (SCM)

Legion integrates with Git through the command palette and the SCM panel.

### Diff review navigation

When a diff is open, use the following palette commands (or their projected key bindings) to move through changes:

| Command | Description |
| --- | --- |
| Git: Next Hunk | Move focus to the next changed hunk in the current file. |
| Git: Previous Hunk | Move focus to the previous changed hunk in the current file. |
| Git: Next File | Move focus to the next changed file in the diff. |
| Git: Previous File | Move focus to the previous changed file in the diff. |

Focus state is tracked in the application layer; the desktop projection reflects the current `focused_hunk_id` from `GitProjection`.

### Commit validation

Before committing, Legion validates:

- **Summary line** — must be non-empty (hard error; the commit action is blocked).
- **Author identity** — name and email are read from `git config`; missing values are a hard error.
- **Conventional Commits prefix** — if the summary does not start with a recognised CC prefix (`feat`, `fix`, `refactor`, etc.) an advisory warning is surfaced, but the commit is not blocked.

### Local file history

Legion records a content snapshot every time a file is successfully saved through the proposal workflow.
Snapshots are bounded to 50 entries or 50 MiB per file (whichever limit is reached first).

Metadata (timestamps, content hash, file identity) is stored in memory by `LocalHistoryMetadataStore`.
Content blobs are written to `.legion/local-history/<path-key>/` inside the workspace and are workspace-local — they are never pushed to remote.

To browse or restore from local history:

1. Open the command palette and run **Git: Local History**.
2. Select the entry to restore.
3. The restore goes through the standard proposal/save workflow — it inherits fingerprint, version, generation, and correlation/causality tracking; no direct writes occur.

### Git worktrees

Use the **Git: New Worktree** palette command to create a linked worktree for an existing branch.
Worktree creation never touches the network.

### Evidence export

Run **Git: Export Worktree Evidence** to write a metadata-only TOML snapshot of the current worktree state to `.legion/evidence/`.
The export contains only metadata (branch, HEAD, path); no file content is included.

## What this guide does not cover

- low-level architecture ownership rules: see `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`;
- mode-policy details: see `docs/MODES.md`;
- historical rename context: see `docs/LEGION_RENAME.md`.
