# Dogfood Journal — 2026-07-21

## Session

- **Branch:** main
- **Commit SHA:** e8cecaa5f6478406eb2d8c534564ebaddc30d356 (pre-Tier-0 baseline at session start; worktree may include uncommitted T0 truth-repair changes)
- **OS / Platform:** Microsoft Windows 10.0.26200 (Windows NT 10.0.26200.0)
- **Build method:** headless / document + source review (cargo toolchain not available in this agent environment; GUI interactive dogfood deferred to next entry)
- **Legion version / channel:** workspace 0.1.0 / pre-beta substrate

## Workflow Attempted

Tier 0 truth-repair bootstrap dogfood: reconcile product-readiness claims, inventory simulated UI surfaces (plugin/remote/debug/AI fixture), and record known daily-driver blockers from the 2026-07-20 gap review against current sources.

Attempted / re-verified via source and prior audit evidence (not a full interactive GUI session):

1. Editor keyboard path: Backspace/Delete/Enter not mapped for buffer mutation (`editor_text_input_actions` / `editor_keyboard_control_actions`) — **blocked (A1)**.
2. Product storage: `AppComposition` uses `InMemoryStorageRepositoryPort` — **blocked (A10)**.
3. File tree: `MAX_TREE_CHILDREN_DEPTH = 2` — **blocked (A12)**.
4. Default Assist path: deterministic-local fixture — **partial (A3)**; inspectable control surfaces remain PR-AI-001 validated for privacy/manifests.
5. DAP: metadata-only fixture — **blocked as product debugger (A7)**.
6. Remote “connect”: fixture harness only — **deferred PR-ENT-001**.

## Modes Used

- [x] Manual (source/path review)
- [x] Assist (fixture path honesty review)
- [ ] Delegate
- [ ] Legion Workflows

## Evidence

| Item | Path / Description |
| --- | --- |
| Screenshots | N/A — no GUI session this entry |
| Terminal output | cargo not on PATH in agent environment |
| Test results | Not re-run this session; gates pending cargo install |
| Logs / traces | Gap review + `audit-reports/2026-07-13-release-readiness-codebase-map-and-gaps.md` |
| Tier 0 packets | `plans/evidence/production/WS-P0/T0-A-ledger-claim-repair.md`, `T0-B-honest-simulated-ui.md` |

## Result

- **Outcome:** partial / blocked
- **What worked:** Product docs and ledgers can be reconciled; honest cut-line UI copy is implementable without feature work; dogfood template path is now usable.
- **What failed:** Cannot complete a real Manual edit loop (A1) or claim daily-driver readiness.
- **Blockers encountered:** A1 Backspace/Delete/Enter; A2 clipboard; A10 in-memory storage; A11 shallow watcher; A12 tree depth; A7 simulated DAP; A3–A5 deterministic default AI; cargo unavailable for gate re-run in this environment.

## Product-Readiness Impact

No readiness row promotions. Reinforces:

- PR-UI-001 remains substrate validated with explicit editor keyboard gap.
- PR-AI-001 remains product-workflow-validated for inspectability only; deterministic GUI default gap documented.
- PR-VSC-001 / PR-ENT-001 claims narrowed / deferred as appropriate.
- M8 “one week dogfood” exit criterion still unmet (first journal only).

## Known blockers to re-verify weekly

- [ ] A1 Backspace/Delete/Enter buffer keys
- [ ] A2 OS clipboard copy/cut
- [ ] A8 interactive terminal GUI loop
- [ ] A10 durable product storage
- [ ] A11 recursive file watcher
- [ ] A12 tree depth > 2
- [ ] A3–A5 real default AI path
- [ ] A7 DAP real or honest cut line (T0-B interim honesty landed)

## Follow-Up

- [x] Issues filed: tracked as Tier 1+ in implementation plan
- [x] Fixes needed: Tier 1 editor/workspace first
- [x] Ledger updates: T0-A applied
