# GAP-01.2 — Independent 3-OS windowed GUI job

**Date:** 2026-09-01  
**Wave:** 2 proof surface  
**Task:** GAP-01.2

## What this is

A hosted 3-OS job that runs the GAP-01.1 harness (`xtask windowed-gui-e2e` →
packaged `legion-desktop --windowed-e2e` → `eframe::run_native` open/edit/save).

Workflow: `.github/workflows/legion-windowed-gui.yml`

| Property | Value |
| --- | --- |
| Triggers | `workflow_dispatch` and weekly Mondays 08:00 UTC |
| Matrix | ubuntu-latest, windows-latest, macos-latest |
| GUI step | `Run windowed GUI E2E` — hard-fail |
| `continue-on-error` on GUI step | forbidden (absent) |
| `\|\| true` on GUI step | forbidden (absent) |
| Linux display | `xvfb-run --auto-servernum` (still `eframe::run_native`, not `--beta-smoke`) |
| PR merge gate | no — independent, same clock as T0-D |

Linux uses a virtual framebuffer so a window exists; that is still a windowed
path. It is not headless `--beta-smoke` and not AppComposition `golden-path-5`.

## What this is not

- Not a required check on `protect-main` (id `21950476`)
- Not folded into `legion-gates.yml`
- Not a 22nd local standing gate
- Not GAP-01.1 local proof (that landed on #196)
- Not hosted run URLs yet — those exist only after a dispatch against `main`

Promote to merge-blocking only after four consecutive green 3-OS runs and
owner sign-off (`docs/OPERATOR_RUNBOOK.md`, `T0-D-smoke-promotion-criteria.md`).

## Hosted evidence

| OS | Run URL | Artifact | Result |
| --- | --- | --- | --- |
| ubuntu-latest | pending first dispatch after merge | `windowed-gui-report-ubuntu-latest` | |
| windows-latest | pending first dispatch after merge | `windowed-gui-report-windows-latest` | |
| macos-latest | pending first dispatch after merge | `windowed-gui-report-macos-latest` | |

A filled row needs the run URL plus the uploaded `report.toml` with
`window_created = true`. Until those exist, GAP-01.2 is not closed.

Ledger row statuses are unchanged.
