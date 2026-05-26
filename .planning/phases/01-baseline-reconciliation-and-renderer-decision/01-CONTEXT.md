# Phase 1 Context: Baseline Reconciliation and Renderer Decision

## Workflow Inputs

- User command: `$legion plan 1`.
- Project root: `C:\Users\dasbl\RustroverProjects\devil-ide`.
- Phase source: `.planning/ROADMAP.md`, phase 1, "Baseline Reconciliation and Renderer Decision".
- Code map source: `.planning/CODEBASE.md` plus `.planning/codebase/index.jsonl` and `.planning/codebase/symbols.json`.
- Structured Legion `AskUserQuestion` confirmation was not available in this runtime. The plan uses the safe default for the direct command: create executable phase plans from the current roadmap, code map, and repository constraints.
- `.planning/REQUIREMENTS.md`, `.planning/memory/RETRO.md`, and `.planning/memory/OUTCOMES.md` were absent at plan-generation time.
- `.planning/settings.json` was absent, so each plan is capped at three concrete tasks.
- GitHub issue creation is skipped for this phase plan because `gh auth status` succeeded but `git remote get-url origin` failed with `No such remote 'origin'`.

## Current State Evidence

- `devil-ui` is projection-only: it depends on `devil-protocol`, emits `CommandDispatchIntent`, and accepts protocol snapshots.
- `devil-app` owns orchestration and depends on `devil-ui`; the current runnable shell is a CLI proof with `:w` and `:q`.
- Saves remain proposal-mediated through `AppComposition::save_active_buffer` and `SaveWorkflowService`.
- `plans/adrs/ADR-0002-ui-editor-rendering.md` is accepted with reservations and identifies the native Rust GPU path as the primary renderer direction, with explicit follow-up evidence required for renderer-backed latency, IME, clipboard, focus, and accessibility.
- `plans/spikes/SPIKE-001A-result.md` validates the projection-only shell but does not validate real GUI renderer behavior.
- `plans/phase-status-ledger.md` still contains stale or contradictory Phase 8 language relative to accepted Phase 8 evidence under `plans/evidence/phase-8/`.
- `plans/dependency-policy.md` does not yet define a GUI desktop adapter crate or renderer dependency gate.
- `xtask/src/main.rs` enforces dependency policy and contains existing architecture tests such as `ui_shell_remains_projection_only`.

## Phase Goal

Phase 1 must convert the current post-substrate state into a clean GUI productization baseline:

- Reconcile Phase 8 status so the GUI track starts from accepted substrate evidence, not a stale hardening blocker.
- Reconfirm the renderer decision against current primary documentation and the repository's projection-only architecture.
- Define the desktop adapter boundary without moving editor state, workspace state, save ownership, telemetry storage, or provider runtime behavior into UI.
- Add policy and `xtask` enforcement so future renderer work cannot accidentally place GUI dependencies in `devil-ui` or core crates.
- Produce a phase readiness evidence document with exact gate outputs and Phase 2 entry criteria.

## Waves

- Wave 1: plan 01 baseline reconciliation and plan 02 renderer decision can run in parallel. They write disjoint files.
- Wave 2: plan 03 desktop adapter boundary and plan 04 dependency policy/xtask enforcement run after Wave 1. Plan 04 depends on the chosen renderer and adapter boundary.
- Wave 3: plan 05 evidence and readiness runs after plans 01 through 04.

## Non-Negotiable Constraints

- Preserve projection-only UI. `devil-ui` must not own editor text, workspace state, save workflows, provider state, telemetry storage, or filesystem side effects.
- Preserve proposal-mediated saves and conflict rejection behavior.
- Preserve metadata-only observability defaults and zero-id rejection semantics.
- Keep placeholder/gated crates inert unless their ADR, dependency-policy entry, and contract tests exist.
- Update dependency policy and `xtask` together whenever dependency protocol symbols change.
- Verify before reporting completion. Do not mark Phase 1 accepted from documentation alone if the planned verification commands fail.

