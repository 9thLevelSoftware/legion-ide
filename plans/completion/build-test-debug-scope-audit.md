# S0-01j Build/test/debug scope audit

This bounded inventory replaces only `COMP-SCOPE-FAMILY-06` from committed baseline `64626ff` with `COMP-BTD-001..009`. Product acceptance is unassessed throughout. The approved family is **“Build/test/debug”** (`docs/superpowers/specs/2026-09-04-product-completion-design.md`, Required feature families, family-06).

## Governing promises and source mapping

The approved S2-04 plan requires **“Build and test workflow adapters for all three language groups”**: discovery, configured build/task/test commands, run groups and targeted tests, actual output/exit, failure navigation, rerun and unavailable-runner behavior for Rust, TS/JS and Python. S2-05 requires named real adapters and launch/attach, breakpoints, step, variables, expression evaluation, debug console, terminate and crash recovery; fixture/fake DAP tests are regression-only. S2-06 requires a native journey for every declared configuration and canonical `EvidenceRun` records, with external process outputs/exits, recovery, OS and reviewer coverage. The manual plan also explicitly says headless actions, mocked LSP/DAP, replayed fixtures and projection assertions are not layer-3 product acceptance.

The traceability source `docs/superpowers/plans/2026-09-04-completion-traceability.md` P2.F3 maps DAP runtime/CodeLLDB/breakpoints, test explorer discovery, and metadata-only evidence. Those retained implementation rows are exact deduplication targets where their promise is identical; this family adds the narrower user outcomes for configuration, actual process results, targeted/group execution, lifecycle failure, inspection, and per-language native journeys.

| inspected source | exact promise mapped | canonical rows / limits |
|---|---|---|
| `docs/superpowers/specs/2026-09-04-product-completion-design.md`, family-06; finite supported configuration contracts; qualification and evidence | build/test/debug family, language configuration and external evidence | BTD-001..009; full approved scope remains required |
| `docs/superpowers/plans/2026-09-04-manual-language-completion.md`, S2-04 | discovery, run/stop/rerun, targeted/group tests, output/exit/problems | BTD-001..005; actual native acceptance remains unassessed |
| same, S2-05 | adapter prerequisites, launch/attach, breakpoint/step, variables/scopes, expressions/console, terminate/crash recovery | BTD-002, BTD-006..008 |
| same, S2-06 | native per-configuration journey, versions, external effects, recovery, OS/reviewer | BTD-009 |
| `docs/superpowers/plans/2026-09-04-completion-traceability.md`, P2.F3.T1–T5 | runtime, CodeLLDB, breakpoints, test explorer, evidence | exact retained P2.F3 rows map to BTD-004/006/007/009; mocks and metadata do not prove product behavior |
| retained master plan, roadmap, readiness/phase ledgers, installed sequence, adaptive plans, foundational/remaining plans, and `docs/ui/canvas-workspace-direction.md` | build/test/debug completion and cross-family boundaries | BTD-001..009 where language workflow is promised; canvas direction adds no BTD-specific user outcome |
| `crates/legion-app/src/test_explorer.rs`, `crates/legion-desktop/src/bridge.rs` | test discovery, run groups, result/failure projection, stop/rerun routing | BTD-003..005; source traces only |
| `crates/legion-debug/src/adapter_resolve.rs`, `crates/legion-app/src/debug_workflow.rs` | policy-approved adapter resolution and session lifecycle | BTD-002, BTD-006..008; no claim that every language adapter is live |
| `crates/legion-desktop/src/bridge.rs`, `crates/legion-desktop/src/view.rs` | debug actions, session validation, inspector/console projection | BTD-007/008; projection is not native acceptance |
| `crates/legion-lsp/src/lib.rs`, `crates/legion-project/src/lib.rs` | declared language adapters and project/configuration inputs | BTD-001/002; registry/protocol declarations are not real process proof |
| `xtask/src/completion/schema.rs` and retained `plans/evidence/**` P2.F3 artifacts | EvidenceRun data model and historical evidence | BTD-009; historical evidence remains unassessed |

The traceability additional-source section's unavailable historical generator/status files were recorded as unavailable rather than inferred. The named parking-lot/GAP and deferral documents were checked for conflicts; none weakens the approved S2-04/S2-05/S2-06 promises. No multi-session promise was added because the approved source inspected here does not require it explicitly.

## Canonical outcomes and implementation limits

| ID | atomic outcome | committed `64626ff` evidence and limit |
|---|---|---|
| BTD-001 | matrix names four language configurations, runners/adapters, versions/OS, discovery and unavailable prerequisites | `legion-lsp/src/lib.rs` registry is a declaration; canonical matrix is unavailable, so acceptance is unassessed |
| BTD-002 | validate project/tool/test/debug configuration, trust/isolation, policy and visible prerequisite failures | `legion-project/src/lib.rs` and `adapter_resolve.rs` are authority traces; not all-language native proof |
| BTD-003 | run/stop/rerun build/tasks with actual output/exit and problems/unavailable states | test explorer and desktop intents exist; process acceptance remains unassessed |
| BTD-004 | targeted and grouped tests preserve genuine results, output/exit and failure navigation | test explorer group/result symbols exist; fixture/projection results are not native proof |
| BTD-005 | cancellation/failure/repair/rerun/missing-runner/stale/exit recovery is truthful | routing and result lifecycle traces exist; no fabricated success is claimed |
| BTD-006 | policy-approved adapters launch/attach, fail visibly, terminate and recover after crash | resolver/workflow traces exist; per-language adapters remain unverified |
| BTD-007 | breakpoints, stepping, run-to-cursor and source navigation preserve session identity | desktop actions and workflow symbols exist; acceptance unassessed |
| BTD-008 | stack/scopes/variables/watches/exceptions/console project truthful state and ended-session errors | bridge/view inspection routes exist; no native DAP claim |
| BTD-009 | per-config native EvidenceRun covers build/test/debug, external effects, recovery, OS/reviewer | `EvidenceRun` schema exists; required records are not asserted present |

## Exact deduplication

Retained `COMP-P2-F3-T1-1` through `COMP-P2-F3-T5-1` remain byte-for-byte unchanged and map respectively to BTD-006, BTD-006/007, BTD-007, BTD-004, and BTD-009 where their outcomes are identical. `COMP-LANG-011` owns the exact broad four-language build/test runner promise and `COMP-LANG-012` owns the exact broad four-language debug adapter promise; the BTD rows retain narrower configuration, targeted/group output, stop/rerun/recovery, inspection, and native journey outcomes not proven by those rows. Protocol models, fixture DAP, mock servers, and UI projection tests are never used as live process acceptance.

Every BTD row has `implementation: partial` as a committed-source classification only and `acceptance: unassessed`. This increment does not claim S0 completion.
