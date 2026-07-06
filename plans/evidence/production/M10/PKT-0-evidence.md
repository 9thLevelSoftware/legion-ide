# PKT-0 Evidence — Orphan Sweep + Honesty Fixes

**Milestone:** M10
**Date:** 2026-07-06
**Branch:** m10/residuals
**Author:** Devil (dasblueeyeddevil@gmail.com)

---

## Deliverables Completed

### D1 — legion-agent orphan modules declared

- `crates/legion-agent/src/lib.rs`: added `pub mod evidence;` and `pub mod external;`
- `crates/legion-agent/Cargo.toml`: added `legion-debug = { workspace = true }` and `sha2 = { workspace = true }`
- Root cause: `evidence.rs` and `external.rs` existed on disk but were never declared in `lib.rs`, making them invisible to the compiler.

### D2 — legion-debug orphan module declared

- `crates/legion-debug/src/lib.rs`: added `pub mod evidence;` and re-exported `EvidenceProjectionError`, `debug_adapter_audit_evidence`, `debug_adapter_audit_summary`, `test_run_summary_evidence`, `test_run_summary_text`
- `crates/legion-debug/Cargo.toml`: added `sha2 = { workspace = true }`
- Note: `evidence.rs` in legion-debug was itself an undeclared orphan; fixed as part of D1's dependency chain.

### D3 — legion-desktop orphan files deleted

Deleted (undeclared, no PKT owner, no pending feature):
- `crates/legion-desktop/src/view/privacy_inspector.rs`
- `crates/legion-desktop/src/view/provider_setup.rs`
- `crates/legion-desktop/src/view/fleet_card.rs`
- `crates/legion-desktop/src/view/cloud_lane.rs`

Kept:
- `crates/legion-desktop/src/view/worker_panel.rs` (PKT-WORKER will adopt)

### D4 — sandbox_panel.rs honesty fixes

- `sandbox_strength_label` no longer returns `"strong"` for any backend variant; returns `"descriptor-only"` (all non-fallback backends) or `"fallback"` (`DocumentedFallback`).
- Fake scope string `"/workspace/project"` replaced with `"(no active sandbox — descriptor only)"`.
- Tests added:
  - `sandbox_strength_label_never_returns_strong` — iterates all `SandboxBackend` variants, asserts none produce `"strong"`
  - `rows_output_contains_descriptor_only_not_strong` — asserts `rows()` output contains `"descriptor-only"` or `"fallback"` and never `"strong"`

### D5 — WorkspaceNotTrusted trust gate for CreateGitWorktree

- `crates/legion-app/src/lib.rs`: added `AppCompositionError::WorkspaceNotTrusted(String)` variant; gate inserted before `create_git_worktree` call.
- `crates/legion-app/tests/worktree_creation_workflow.rs`: two integration tests added:
  - `create_git_worktree_denied_for_untrusted_workspace` — asserts `WorkspaceNotTrusted` error returned
  - `create_git_worktree_trusted_workspace_passes_trust_gate` — asserts error is NOT `WorkspaceNotTrusted` when trusted

### D6 — Kanban bookkeeping

- P3 epic: milestone `"M2"` → `"M9"`
- P4 epic: milestone `"M2"` → `"M9"`
- P5 epic: milestone `"M3"` → `"M10"`
- All P3 tasks (F1.T1–T4, F2.T1–T4, F3.T1–T4, F4.T1–T4): `status = "done"`, `evidence = "plans/evidence/production/M9/"`
- All P4 tasks (F1.T1–T4, F2.T1–T4, F3.T1–T4, F4.T1–T3): `status = "done"`, `evidence = "plans/evidence/production/M9/"`

### D7 — Stale notice prepended

- `plans/evidence/accessibility/gp-3-delegate-walkthrough.md`: blockquote stale notice prepended referencing M10 campaign and `plans/evidence/production/M10/`.

---

## Test Results

All tests pass (`cargo test --all-targets -j 4`). See commit notes for exact counts.

---

## Honesty Constraints Satisfied

| Constraint | Status |
|---|---|
| No "strong" sandbox label without OS enforcement | PASS — `descriptor-only` replaces `strong` |
| No fake workspace scope paths | PASS — `(no active sandbox — descriptor only)` |
| No undeclared orphan modules | PASS — all orphans declared or deleted |
| WorkspaceTrust gate on worktree creation | PASS — gate returns `WorkspaceNotTrusted` for untrusted/none |
| No stubs or deferred product paths | PASS — all changes are real, tests are real |
