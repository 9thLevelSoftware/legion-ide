# Dogfood Journal — 2026-07-22 (preview artifact / CI)

## Session

- **Branch:** main
- **Commit SHA:** da4815d8fda523f72b9b2c17b891691551b7a852 (preview workflow head for successful run)
- **OS / Platform:** GitHub Actions matrix — ubuntu-latest, windows-latest, macos-latest
- **Build method:** Hosted `Legion Preview` workflow (`workflow_dispatch` run 29887799213) — not interactive desktop GUI
- **Legion version / channel:** workspace 0.1.0 / preview unsigned-beta portable

## Workflow Attempted

WS-A-D Phase 4/5 residual: prove **installable preview packaging path** on three OS families without claiming daily-driver GUI dogfood.

1. Dispatch Legion Preview on main after D2 nullglob fix.
2. Confirm package scripts produce `UNSIGNED-BETA.toml` + archive per OS.
3. Confirm smoke layout steps pass on all three runners.
4. Record that this is **CI packaging dogfood**, not interactive Manual/Assist/Delegate session.

## Modes Used

- [ ] Manual (interactive GUI)
- [ ] Assist
- [ ] Delegate
- [x] Packaging / release path (preview CI)

## Evidence

| Item | Path / Description |
| --- | --- |
| CI run | https://github.com/9thLevelSoftware/legion-ide/actions/runs/29887799213 (all three Preview package jobs **success**) |
| Workflow | `.github/workflows/legion-preview.yml` |
| Scripts | `scripts/package-preview.ps1`, `scripts/package-preview.sh` |
| Policy | `UNSIGNED-BETA.toml` → `production = false`, D2 policy_ref |
| D4 note | `plans/evidence/production/WS-A-D/phase-4-release/D4-readiness-close.md` |

## Result

- **Outcome:** pass (packaging/CI only)
- **What worked:** 3-OS portable unsigned-beta bundles build and layout-smoke green.
- **What failed / not attempted:** Interactive GUI dogfood; extract-and-use on a personal machine; signed installers.
- **Blockers:** Human interactive session still required for Phase 1 “interactive GUI” and Phase 5 “dogfood on installed preview” checkboxes.

## Product-Readiness Impact

- Strengthens PR-REL-001 **evidence** only; status remains **In progress**.
- Does **not** count as a full Legion-on-Legion interactive journal for daily-driver exit.

## Known residuals

- [ ] Interactive GUI journal on local machine
- [ ] Dogfood after extracting preview artifact on a clean VM
- [ ] Signed installer path (D2.1)
