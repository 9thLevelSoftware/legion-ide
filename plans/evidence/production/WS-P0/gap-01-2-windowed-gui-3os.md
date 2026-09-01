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
- Not a green 3-OS clock start — first dispatch hard-failed (see below)

Promote to merge-blocking only after four consecutive green 3-OS runs and
owner sign-off (`docs/OPERATOR_RUNBOOK.md`, `T0-D-smoke-promotion-criteria.md`).

## Hosted evidence

First dispatch after merge: [run 33553239633](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33553239633) on `804fb18b`. The GUI step hard-failed on all three OSes (`continue-on-error` was not used).

| OS | Artifact | `window_created` | Result |
| --- | --- | --- | --- |
| ubuntu-latest | none (panic before report) | n/a | panic: `libxkbcommon-x11.so` not loaded |
| windows-latest | `windowed-gui-report-windows-latest` | false | blocked: `WGPU error: Failed to create surface for any enabled backend` |
| macos-latest | `windowed-gui-report-macos-latest` | false | blocked: `WGPU error: Failed to create surface for any enabled backend` |

Follow-up in this change: install `libxkbcommon-x11-0` + Mesa dri on Linux, write a blocked report on panic (so Ubuntu still uploads `report.toml`), and prefer WARP (`Microsoft Basic Render Driver`) on Windows. A later dispatch must still show `window_created = true` on each OS before GAP-01.2 is closed.

Ledger row statuses are unchanged.
