# Phase 1: Baseline Reconciliation and Renderer Decision -- Context

## Phase Goal
Establish the exact current state, reconcile planning truth, and choose a GUI renderer path without weakening architecture gates.

## Requirements Covered
- R-001: Baseline reconciliation — reconcile current planning truth before treating the GUI track as authoritative
- R-002: Renderer decision gate — accept a renderer integration ADR before adding a GUI framework dependency
- R-003: Dependency policy update — update dependency-policy.md and xtask checks before introducing renderer dependencies
- R-004: Desktop adapter boundary — add legion-desktop that consumes projections and emits intents without owning authority

## Brownfield Status
This phase was **fully completed** in the pre-Legion planning system (.planning/phases/01-baseline-reconciliation-and-renderer-decision/). Five plans executed with results and summaries:
- 01-01: Baseline Ledger Reconciliation And GUI Baseline — complete
- 01-02: Renderer Decision ADR And Matrix — complete
- 01-03: Desktop Adapter Boundary Specification — complete
- 01-04: Dependency Policy And Xtask Renderer Gate — complete
- 01-05: Phase 1 Evidence And Readiness Gate — complete

The project has since completed all 13 original phases (64 plans total). The purpose of this plan is to **formally reconcile** the completed work into the new Legion project system, verify that gate commands still pass on the current codebase, and produce a reconciliation record.

## What Already Exists
- egui/eframe 0.34 accepted as the renderer (ADR documented in project)
- legion-desktop crate exists as the renderer-backed desktop adapter
- legion-ui remains projection-only (emits CommandDispatchIntent, accepts snapshots)
- Dependency policy enforced via `cargo run -p xtask -- check-deps`
- 21 standing gates merge-blocking
- 543 commits, 120+ PRs of implemented and reviewed work
- Phase 1 evidence in .planning/phases/01-baseline-reconciliation-and-renderer-decision/

## Key Design Decisions
- **Single reconciliation plan**: Phase is already complete; one plan covers verification of all success criteria
- **Agent choice**: testing-qa-verification-specialist — evidence-focused verification, not implementation
- **Scope**: Read-only verification with evidence capture; no code changes expected

## Plan Structure
- **Plan 01-01 (Wave 1)**: Phase 1 Evidence Reconciliation & Gate Verification — verify all 6 success criteria, run gates, produce formal reconciliation record
