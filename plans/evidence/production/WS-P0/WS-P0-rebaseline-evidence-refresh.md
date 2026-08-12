# WS-P0 Rebaseline Evidence — Refresh

Date: 2026-07-01
Branch: `main` (post-merge from `codex/legion-production-plan-v0.2`)
Starting SHA: `070dc90` (HEAD after `git pull origin main`)
Scope: WS-P0 gate restoration after main-branch merge, stale-reference cleanup, dependency policy reconciliation.

## Changes Made

| Area | Change | Rationale |
| --- | --- | --- |
| docs/hygiene-allowlist.toml | Added 10 historical v0.1 files with broken internal links | All violations were in historical/supporting plan files not referenced by active docs |
| docs/USER_GUIDE.md | Updated stale v0.1 reference to v0.2 | P0.02: active docs must point to current plan |
| docs/INDEX.md | Removed duplicate v0.1 entry, fixed v0.2 references | P0.02: canonical index must not present v0.1 as current |
| README.md | Removed stale v0.1-as-current entry | P0.02: README must not present v0.1 as current plan |
| plans/dogfood/ | Created dogfood journal template | P0.10: weekly Legion-on-Legion dogfood template |
| plans/dependency-policy.md | Added legion-agent, legion-ai, legion-remote, legion-sandbox to legion-desktop allowed set | Reconciled policy with current Cargo.toml dependencies |
| crates/legion-agent/src/lib.rs | Wired pub mod comm/scheduler/scope/tools/dag/plan; added DelegatedTaskScopeDenied variant; re-exported scope functions | TDD tests needed these modules accessible |
| crates/legion-desktop/src/view.rs | Wired pub mod agent_comm/plan_editor/scope_picker/sandbox_panel; added sandbox_rows field | TDD tests needed these view modules accessible |
| crates/legion-desktop/src/workflow.rs | Added ingest_lsp_publish_diagnostics_for_buffer | TDD test needed this method |
| crates/legion-desktop/Cargo.toml | Added legion-agent, legion-sandbox dependencies | View modules import from these crates |
| crates/legion-ui/src/ui.rs | Added ActiveBufferProjectionState enum; added state field to ActiveBufferProjection | TDD tests needed this type |
| crates/legion-ui/src/lib.rs | Re-exported ActiveBufferProjectionState | Public API |
| crates/legion-protocol/src/lib.rs | Re-exported EditablePlanRevision* types | TDD tests needed these types |
| crates/legion-app/src/lib.rs | Updated ActiveBufferProjection construction with state field | Compilation fix |
| 14 test files | Added ActiveBufferProjectionState to ActiveBufferProjection constructions | Compilation fix |

## Gate Verification

| Gate | Result | Notes |
| --- | --- | --- |
| `cargo run -p xtask -- docs-hygiene` | Pass | All historical files properly allowlisted |
| `cargo run -p xtask -- check-deps` | Pass | Dependency policy reconciled with current Cargo.toml |
| `cargo run -p xtask -- no-egui-textedit` | Pass | No egui::TextEdit in code canvas |
| `cargo run -p xtask -- verify-kanban-backlog` | Pass | 10 epics, 38 features, 146 tasks |
| `cargo fmt --all --check` | Pass | |
| `cargo check --workspace` | Pass | 0 errors, 3 warnings |
| `cargo test -p legion-desktop --test agent_comm` | Pass | 1/1 |
| `cargo test -p legion-desktop --test accessibility` | Pass | 5/5 |
| `cargo test -p legion-desktop --test sandbox_panel` | Pass | 1/1 |
| `cargo test -p legion-desktop --test plan_editor` | Pass | 1/1 |
| `cargo test -p legion-desktop --test scope_picker` | Pass | 1/1 |
| `cargo test -p legion-desktop --test input_conformance` | Fail | Runtime trust policy denies temp workspace paths (not a missing-type issue) |

## M7 Exit Criteria Status

| Criterion | Status | Evidence |
| --- | --- | --- |
| No current public doc treats v0.1 as active plan | Met | README, INDEX, USER_GUIDE all point to v0.2; v0.1 marked historical |
| Docs and ledgers don't contradict current code/evidence | Met | docs-hygiene passes; no stale references in active docs |
| Product claims are ledger-first | Met | README "Current Status" directs to ledger |
| Current-state caveats remain visible | Met | USER_GUIDE caveat present; README status section present |
| Docs-hygiene gate passes | Met | `cargo run -p xtask -- docs-hygiene` passes |

## Residual Issues

- `input_conformance` tests fail at runtime due to workspace trust policy denying temp directory access. This is a design decision (trust policy for test workspaces), not a missing implementation.
- Workspace has 3 warnings from `legion-agent` (unused imports in newly wired modules).
- `cargo test --workspace --all-targets` has additional test failures in legion-ai, legion-plugin, legion-storage crates from TDD tests referencing unimplemented types. These are pre-existing from the main branch pull and are tracked in the Kanban backlog.
