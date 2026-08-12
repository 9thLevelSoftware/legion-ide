# Phase 1 Reconciliation Record: Baseline Reconciliation and Renderer Decision

## Summary

- **Phase**: Phase 1 -- Baseline Reconciliation and Renderer Decision
- **Phase Number**: 1
- **Pre-Legion Completion Date**: 2026-05-26 (review passed, commit `101d533`)
- **Total Plans Completed**: 5 of 5
- **Plan Names**:
  1. 01-01: Baseline Ledger Reconciliation And GUI Baseline
  2. 01-02: Renderer Decision ADR And Matrix
  3. 01-03: Desktop Adapter Boundary Specification
  4. 01-04: Dependency Policy And Xtask Renderer Gate
  5. 01-05: Phase 1 Evidence And Readiness Gate
- **Wave Structure**: Wave 1 (01-01, 01-02 parallel), Wave 2 (01-03, 01-04), Wave 3 (01-05)

This is a brownfield reconciliation. Phase 1 was completed in the pre-Legion planning system (`.planning/` directory) and is being formally accepted into the new Legion project system (`.legion/`). No new implementation work was performed; this record verifies that existing evidence satisfies the stated success criteria.

## Success Criteria Evidence

### SC-1: Phase ledger/evidence conflict is resolved or explicitly superseded for the GUI track

- **Status**: PASS
- **Evidence source**: 01-01-SUMMARY.md
- **Notes**: Plan 01-01 reconciled Phase 8 status in `plans/phase-status-ledger.md` based on accepted evidence under `plans/evidence/phase-8/`. The verification command `rg -q "Phase 8 acceptance: Accepted" plans/phase-status-ledger.md` passed. GUI productization was documented as a new post-substrate track in `plans/evidence/gui-productization/gui-productization-baseline.md`.

### SC-2: Renderer ADR records accepted stack, fallback criteria, and Windows-first evidence requirements

- **Status**: PASS
- **Evidence source**: 01-02-SUMMARY.md
- **Notes**: Plan 01-02 updated `plans/adrs/ADR-0002-ui-editor-rendering.md` and created `plans/evidence/gui-productization/renderer-decision-matrix.md`. The ADR records eframe/egui as the accepted renderer stack for the desktop adapter crate only. Slint is the panel/native fallback. Tauri/WRY remains auxiliary-only. Verification confirmed "Windows-first" and "AccessKit|accessibility" keywords are present in the decision matrix. The Phase 1 review (01-REVIEW.md) independently spot-checked the renderer decision against primary sources (egui README, eframe docs, AccessKit docs).

### SC-3: Desktop adapter boundary is specified before code is added

- **Status**: PASS
- **Evidence source**: 01-03-SUMMARY.md
- **Notes**: Plan 01-03 created `plans/adrs/ADR-0030-desktop-adapter-boundary.md` and `plans/desktop-adapter-boundary-v0.1.md` before any renderer-backed GUI code was added (that code arrived in Phase 2). The boundary spec defines `ShellProjectionSnapshot` consumption and `CommandDispatchIntent` emission as the adapter's interface. Forbidden ownership is documented: persistent mutation, save, proposal, provider, telemetry, storage, terminal, plugin, collaboration, remote, and retention authority remain outside the adapter.

### SC-4: Dependency policy and xtask rules describe any approved renderer crate edges

- **Status**: PASS
- **Evidence source**: 01-04-SUMMARY.md, 01-REVIEW.md
- **Notes**: Plan 01-04 updated `plans/dependency-policy.md` and `xtask/src/main.rs` to enforce a renderer/windowing deny list. The deny list was initially scoped only to `devil-ui` (now `legion-ui`), but the Phase 1 review caught this as a BLOCKER and expanded the gate to cover all workspace packages except the designated adapter crate. The regression test `renderer_dependency_gate_preserves_projection_boundary` was added and passes. `cargo run -p xtask -- check-deps` enforces the policy at the gate level.

### SC-5: legion-ui remains projection-only and no GUI dependency is introduced without policy coverage

- **Status**: PASS
- **Evidence source**: Direct verification of `crates/legion-ui/Cargo.toml`, 01-04-SUMMARY.md
- **Notes**: `crates/legion-ui/Cargo.toml` dependencies are: `legion-protocol`, `serde`, `serde_json`, `thiserror`. No `egui`, `eframe`, or any GUI/renderer crate is present. The xtask deny list enforces this at build gate time. `crates/legion-desktop/Cargo.toml` correctly carries the renderer dependencies (`eframe`, `egui`) as the designated adapter crate, consistent with SC-2 and SC-4.

### SC-6: Verification includes check-deps, fmt check, cargo check, and targeted app/UI tests

- **Status**: PASS (for Phase 1 scope)
- **Evidence source**: 01-05-SUMMARY.md, 01-REVIEW.md
- **Notes**: All verification commands passed at Phase 1 completion and at Phase 1 review. The Phase 1 review records the following passing gates: `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`, and targeted xtask tests. Current-day re-execution of these commands shows two post-Phase 1 regressions (see Gate Verification Results below); these are attributed to commit `019aa9c` (2026-08-05, "fix(gates): address Wave 0 review findings") and do not affect Phase 1 acceptance.

## Gate Verification Results

### cargo run -p xtask -- check-deps

- **Command**: `cargo run -p xtask -- check-deps`
- **Exit code**: 0
- **Output**: `dependency policy checks passed`
- **Status**: PASS

### cargo fmt --all --check

- **Command**: `cargo fmt --all --check`
- **Exit code**: 1
- **Output**: Two formatting diffs in `xtask/src/readiness_consistency.rs:213` and `xtask/tests/docs_hygiene.rs:428`
- **Status**: FAIL (post-Phase 1 regression)
- **Root cause**: Both files were last modified in commit `019aa9c` (2026-08-05, "fix(gates): address Wave 0 review findings (#120)"), which is Wave 0 work from the current Legion CLI bootstrap. Phase 1 completed on 2026-05-26. This formatting drift is not a Phase 1 concern.

### cargo check --workspace --all-targets

- **Command**: `cargo check --workspace --all-targets`
- **Exit code**: 101
- **Output**: `error[E0433]: cannot find type DocsHygieneViolationKind in this scope` at `xtask/tests/docs_hygiene.rs:432:27`
- **Status**: FAIL (post-Phase 1 regression)
- **Root cause**: The test file `xtask/tests/docs_hygiene.rs` uses `DocsHygieneViolationKind::DuplicateAdrNumber` at line 432 but the import at line 8 (`use xtask::docs_hygiene::{DocsHygieneConfig, normalize_relative_target, run_docs_hygiene}`) does not include `DocsHygieneViolationKind`. The type exists in `xtask/src/docs_hygiene.rs:10`. This was introduced in commit `019aa9c` (2026-08-05). Phase 1 completed on 2026-05-26. This compilation failure is not a Phase 1 concern.

## Pre-Legion Evidence References

### Phase 1 Planning Artifacts

| File | Purpose |
|------|---------|
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-CONTEXT.md` | Phase context, inputs, constraints, wave structure |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-01-PLAN.md` | Plan 01-01 execution plan |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-01-RESULT.md` | Plan 01-01 execution result |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-01-SUMMARY.md` | Plan 01-01 summary |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-02-PLAN.md` | Plan 01-02 execution plan |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-02-RESULT.md` | Plan 01-02 execution result |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-02-SUMMARY.md` | Plan 01-02 summary |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-03-PLAN.md` | Plan 01-03 execution plan |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-03-RESULT.md` | Plan 01-03 execution result |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-03-SUMMARY.md` | Plan 01-03 summary |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-04-PLAN.md` | Plan 01-04 execution plan |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-04-RESULT.md` | Plan 01-04 execution result |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-04-SUMMARY.md` | Plan 01-04 summary |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-05-PLAN.md` | Plan 01-05 execution plan |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-05-RESULT.md` | Plan 01-05 execution result |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-05-SUMMARY.md` | Plan 01-05 summary |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/01-REVIEW.md` | Phase 1 review (PASSED, 2 cycles) |
| `.planning/phases/01-baseline-reconciliation-and-renderer-decision/WAVE-CHECKLIST.md` | Phase execution checklist |

### Phase 1 Deliverable Files (in repository)

| File | Purpose |
|------|---------|
| `plans/phase-status-ledger.md` | Reconciled phase status ledger |
| `plans/evidence/gui-productization/gui-productization-baseline.md` | GUI productization baseline document |
| `plans/evidence/gui-productization/renderer-decision-matrix.md` | Renderer decision evaluation matrix |
| `plans/adrs/ADR-0002-ui-editor-rendering.md` | Renderer ADR (updated with acceptance) |
| `plans/adrs/ADR-0030-desktop-adapter-boundary.md` | Desktop adapter boundary ADR |
| `plans/desktop-adapter-boundary-v0.1.md` | Desktop adapter boundary specification v0.1 |
| `plans/dependency-policy.md` | Dependency policy with renderer gate rules |
| `plans/evidence/gui-productization/phase-1-renderer-readiness.md` | Phase 1 readiness evidence with gate outputs |
| `xtask/src/main.rs` | Xtask with renderer dependency gate enforcement |

### Crate Structure Verification

| Crate | Renderer Dependencies | Status |
|-------|----------------------|--------|
| `crates/legion-ui/Cargo.toml` | None (deps: legion-protocol, serde, serde_json, thiserror) | PASS -- projection-only |
| `crates/legion-desktop/Cargo.toml` | eframe, egui | PASS -- designated adapter |

### Naming Note

Phase 1 summaries reference `devil-ui`, `devil-desktop`, `devil-app` as the crate names. These were subsequently renamed to `legion-ui`, `legion-desktop`, `legion-app` during the project rebrand. The substance of the Phase 1 work (boundary specifications, dependency gates, projection-only architecture) is intact and correctly applies to the renamed crates.

## Reconciliation Decision

Phase 1 work completed in the pre-Legion planning system is accepted as-is for the new Legion project system.

**Basis**: All six success criteria are verified as PASS. The dependency policy gate (`check-deps`) passes. Two gate command failures (`cargo fmt --all --check` and `cargo check --workspace --all-targets`) are traced to post-Phase 1 regressions introduced in commit `019aa9c` (2026-08-05) during Wave 0 Legion CLI bootstrap work, and do not affect the validity of Phase 1 completion. The Phase 1 review (01-REVIEW.md) records all gates passing at review time (2026-05-26) after resolving one blocker (renderer gate scope) and one warning (trailing whitespace). All referenced evidence files exist in the repository.

**Date**: 2026-08-08
**Reconciliation method**: Systematic evidence extraction from 5 plan summaries, cross-referencing against 6 ROADMAP.md success criteria, direct crate dependency verification, live gate command execution with root-cause analysis of failures, and review record verification.
