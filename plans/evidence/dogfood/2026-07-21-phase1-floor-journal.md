# Dogfood Journal — 2026-07-21 (WS-A-D Phase 1 floor verification)

## Session

- **Branch:** `phase-1/dogfood-session-2026-07-21` (on top of `docs/ws-a-d-campaign-charter` / main tip including #66)
- **Commit SHA:** `62549df0a096808e258b275da82486b8e5181f0f` at session start; floor-fix commits may follow on this branch
- **OS / Platform:** Microsoft Windows NT 10.0.26200.0
- **Build method:** local `cargo test` / headless `DesktopRuntime` beta workflow (GUI interactive window not driven this session)
- **Legion version / channel:** workspace 0.1.0 / pre-beta

## Workflow Attempted

WS-A-D **Phase 1** dogfood against post-WS-P0 floor (charter checklist). Primary method: targeted integration suites that exercise the same product paths as interactive dogfood, plus GUI Phase 7 beta harness through desktop runtime.

| # | Workflow | Result | Evidence |
|---|----------|--------|----------|
| 1 | Open workspace / explorer | **success** (beta browse + projection tests) | `beta_workflow` |
| 2 | Edit + proposal-mediated save / conflict | **success** | `workspace_vfs_integration_external_overwrite_between_open_and_save_yields_conflict`; control_trust dirty-text |
| 3 | Keys / clipboard / BYOK focus isolation | **success** | `input_conformance` (8); provider_key_entry |
| 4 | Terminal | **success** | `terminal_workflow` (app) |
| 5 | Assist proposal | **partial → fixed** | Live Auto/Ollama caused beta Assist to return `proposal_id=None` (streaming). Fixed beta to force Deterministic + poll fallback |
| 6 | BYOK key entry | **success** (test harness) | `provider_key_entry` |
| 7 | Delegate | **success** | `delegated_task_integration` |
| 8 | Git projection | **success** | `git_workflow` (app) |
| 9 | Debug simulated banner | **success** (honest simulated path) | `debug_workflow` (desktop) |
| 10 | Sandbox honesty panel | **success** | `sandbox_panel` (4) |

## Modes Used

- [x] Manual (via app/desktop tests + beta)
- [x] Assist (fixture + discovered live streaming interaction)
- [x] Delegate
- [ ] Legion Workflows (not required this session)

## Evidence

| Item | Path / Description |
| --- | --- |
| Automated floor log | `plans/evidence/production/WS-A-D/phase-1-dogfood/automated-floor-run-2026-07-21.txt` |
| Screenshots | N/A — no interactive eframe window this session |
| Test results | ALL targeted suites green after beta fix; see log |
| Floor bug | Beta Assist assumed sync `proposal_id` after #66 non-blocking live Assist |
| Charter | `plans/evidence/production/WS-A-D/campaign-charter.md` |

## Result

- **Outcome:** **partial → success** (floor bug found and fixed in-session)
- **What worked:** Save/conflict, keys/clipboard, terminal, git, language tooling, sandbox honesty, debug simulated projection, Assist fixture path, Delegate
- **What failed initially:** GUI Phase 7 `beta_workflow` proposal gate when Ollama reachable (Auto live stream)
- **Blockers encountered:** None remaining after fix

## Floor-bug triage

| ID | Severity | Item | Disposition |
|----|----------|------|-------------|
| F1 | P1 (CI/beta) | `run_proposal_actions` expected immediate Assist `proposal_id` | **Fixed** this branch: force Deterministic + poll fallback for live stream |
| K1 | Known cut line | DAP is simulated only | Accept for Phase 1; owned by Phase 2 |
| K2 | Known cut line | Windows sandbox FS/network not OS-enforced | Accept for Phase 1; owned by Phase 3 |
| K3 | Known cut line | No signed installers | Accept for Phase 1; owned by Phase 4 |

## Product-readiness impact

- Unblocks beta acceptance harness on machines with live Ollama after #66.
- Confirms post-WS-P0 floor regressions are limited; go/no-go for Phase 2 (Real DAP) is **go** from automated floor perspective.
- Interactive GUI dogfood (human-driven eframe window) still recommended as a second journal entry for completeness (charter asks ≥3 journals total).

## Next dogfood sessions (for ≥3 journal requirement)

1. Human interactive GUI on Windows against this repo SHA (Assist live + BYOK if keys available).
2. Optional Linux/macOS interactive or headless matrix once available.
