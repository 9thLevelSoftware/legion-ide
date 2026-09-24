# S0-01k Source control scope audit

This bounded inventory replaces only `COMP-SCOPE-FAMILY-08` from committed baseline `d241239` with `COMP-SCM-001..010`. The approved family is **“Source control”** (`docs/superpowers/specs/2026-09-04-product-completion-design.md`, Required feature families, family-08). The full approved S1-07 deliverable is **“Visible and usable status/diff/stage/unstage/commit/branch/conflict/remote verbs, reviewable changes, local history browsing/restoration, dirty recovery, external-change conflict, checkpoint and storage-failure recovery.”** All product acceptance remains unassessed.

## Source mapping

| inspected source and promise | canonical mapping | implementation limit |
|---|---|---|
| `docs/superpowers/plans/2026-09-04-manual-language-completion.md`, S1-07 and native `manual-git-history-recovery` | SCM-001..010 | governing full scope; native host effects remain unassessed |
| `docs/superpowers/specs/2026-09-04-product-completion-design.md`, family-08 Source control and Work preservation | SCM-001..010 | status is not acceptance; dirty recovery/external edits/storage failure are required |
| `docs/superpowers/plans/2026-09-04-completion-traceability.md`, P2.F5.T1–T4 | SCM-001..006, SCM-010 | retained P2.F5 rows own exact live gutter/status/remote/worktree outcomes; new rows cover full S1-07 workflow and recovery |
| retained master plan, roadmap, readiness/phase ledgers, installed sequence, adaptive plans, foundational/remaining plans, and `docs/ui/canvas-workspace-direction.md` | SCM-001..010 | inspected as leads; canvas adds no source-control promise |
| `crates/legion-app/src/git_inspection.rs` | SCM-001, SCM-005, SCM-009 | Git read/projection authority, not proof of every operation |
| `crates/legion-app/src/git_remote.rs`, `git_policy.rs` | SCM-006 | remote verbs and host policy boundaries; no network/auth call is claimed |
| `crates/legion-app/src/lib.rs` | SCM-002..004, SCM-007..009 | command/policy/proposal authority and commit guard symbols; current implementation classifications remain partial |
| `crates/legion-desktop/src/bridge.rs` | SCM-002, SCM-004..006, SCM-008 | rendered actions route intents; action reachability is not host-observed Git proof |
| `crates/legion-desktop/src/view.rs` | SCM-001, SCM-007, SCM-010 | status/diff/checkpoint projections; visual rows do not prove Git effects |
| `crates/legion-app/tests/workspace_vfs_integration.rs` | SCM-009 | external overwrite conflict test source; product acceptance remains unassessed |
| `plans/evidence/production/M8/WS-GIT-01-evidence.md` | SCM-001..010 | historical evidence attachment, not current acceptance |
| `xtask/src/completion/schema.rs` | SCM-010 | EvidenceRun schema, not evidence record presence |

The traceability additional-source section's unavailable historical generator/status files remain unavailable and are not treated as requirements. Parking-lot/GAP and deferral documents were checked for source-control conflicts; none authorizes scope reduction. No GitHub mutation or credential call was performed. Remote verbs are requirements with policy-visible denial and actual-result evidence, not permission to call a remote in this inventory.

## Atomic outcomes and classification

| ID | normalized outcome | committed-source trace and limit |
|---|---|---|
| SCM-001 | status, dirty/untracked, staged/unstaged, diffs, branch/remote/conflict rows from authoritative Git state | `git_inspection.rs` and desktop `git_rows` projections; projection alone is insufficient |
| SCM-002 | file/hunk stage and unstage preserve unrelated dirty/untracked work | bridge actions and app dispatch symbols; host index effect unassessed |
| SCM-003 | selected-change commit with message, actual history result, truthful failure without dirty-work loss | app commit command/policy symbols; commit remains unassessed |
| SCM-004 | branch list/create/switch/delete with unsafe dirty/conflict denial | bridge branch actions and `git_policy.rs`; operation effects unassessed |
| SCM-005 | conflict path identification, ours/theirs choice, unresolved preservation, index/worktree verification | bridge conflict actions and inspection source; no native merge claim |
| SCM-006 | policy-visible fetch/pull/push, host/credential denial, actual output/exit/result, no implicit network | `git_remote.rs`, `git_policy.rs`, bridge remote actions; no auth/network call performed |
| SCM-007 | local history browse/restore through proposal/checkpoint with stale/conflict/external/storage recovery | app/view checkpoint and proposal paths; restoration acceptance unassessed |
| SCM-008 | worktree/agent branch visibility and safe prune/remove without unrelated deletion | bridge worktree actions and app authority; no destructive operation performed |
| SCM-009 | external edit/restart/storage/remote/merge recovery preserves disk/index/worktree/buffer/history | inspection, save/conflict paths and workspace VFS test source; test source is not product proof |
| SCM-010 | native host-observed journey records Git effects, OS/reviewer coverage and recovery | EvidenceRun schema and historical WS-GIT-01 only; records are not asserted present |

## Exact deduplication

Retained P2.F5 rows remain byte-for-byte unchanged: `COMP-P2-F5-T1-1` maps to SCM-001, `COMP-P2-F5-T2-1` maps to SCM-001/002/003/006, `COMP-P2-F5-T3-1` maps to SCM-008, and `COMP-P2-F5-T4-1` maps to SCM-006. Those retained rows are exact canonical outcomes where applicable; SCM rows preserve narrower partial-hunk, conflict, history, dirty/restart/storage recovery, and host-observed effects. Git DTOs, read models, UI labels, or harness assertions are not treated as actual Git operation proof. All ten new rows use `implementation: partial` only as committed-source classification and `acceptance: unassessed`. This increment does not claim S0 completion.
