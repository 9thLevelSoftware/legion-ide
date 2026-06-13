# Legion User Guide

This guide is the end-user entry point for the current Legion product paths.
It assumes the reader already has a working build or a packaged desktop app.

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

## What this guide does not cover

- low-level architecture ownership rules: see `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`;
- mode-policy details: see `docs/MODES.md`;
- historical rename context: see `docs/LEGION_RENAME.md`.
