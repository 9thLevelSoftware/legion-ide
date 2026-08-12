# WS-P0 Dirty-Worktree Caveat Audit

Date: 2026-06-19
Scope: production milestone evidence files with dirty-worktree caveats, related current-tree caveats, or missing formal milestone acceptance.

## Decision

No historical milestone should be retroactively upgraded from dirty-tree acceptance to clean-commit acceptance by WS-P0. WS-P0 should instead make the caveats explicit and require current gates to pass from the present branch before any new product-readiness status changes.

## Caveat Matrix

| Evidence | Caveat found | Clean rerun decision |
| --- | --- | --- |
| `plans/evidence/production/M0/M0-milestone-acceptance.md` | Acceptance verified a current tree with uncommitted M0 evidence, implementation files, and some later-workstream files. | No clean rerun required for historical M0. Current WS-P0 gates must pass before merging WS-P0. |
| `plans/evidence/production/M1/M1-milestone-acceptance.md` | Current workspace remained intentionally dirty with integrated predecessor workstream outputs. | No clean rerun required for historical M1. Do not use M1 alone to promote product-ready status. |
| `plans/evidence/production/M2/M2-milestone-acceptance.md` | Current workspace remained intentionally dirty with integrated predecessor workstream outputs. | No clean rerun required for historical M2. Product-readiness rows still need direct row evidence. |
| `plans/evidence/production/M3/` | No formal `M3-milestone-acceptance.md` was present during WS-P0 review; M3 contains task-level evidence files. | Do not claim formal M3 milestone acceptance unless a separate audit creates and verifies it. |
| `plans/evidence/production/M4/M4-milestone-acceptance.md` | Workspace remained intentionally dirty because it contained integrated predecessor workstream outputs. | No clean rerun required for historical M4. Use M4 as substrate/workflow evidence only. |
| `plans/evidence/production/M5/M5-milestone-acceptance.md` | Workspace contained unrelated pre-existing dirty changes from other in-flight work. | No clean rerun required for historical M5. Keep WS18.T4 and release-signing cut lines explicit. |
| `plans/evidence/production/M6/M6-milestone-acceptance.md` | Workspace contained unrelated pre-existing dirty changes from other in-flight work. | No clean rerun required for historical M6. Current WS-P0 gates provide the cleanest present-tense validation available in this packet. |

## Current WS-P0 Required Checks

WS-P0 completion requires these current commands to pass or be recorded with exact blockers:

- `cargo run -p xtask -- docs-hygiene`
- `cargo run -p xtask -- check-deps`
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo test -p xtask --test docs_hygiene`
- `git diff --check`
