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
- Not a required check. The T0-D four-green clock is complete (see [`gap-01-2-windowed-gui-clock-signoff.md`](gap-01-2-windowed-gui-clock-signoff.md)); completing the clock is not merge-blocking.

## Hosted evidence

First dispatch after merge: [run 33553239633](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33553239633) on `804fb18b`. The GUI step hard-failed on all three OSes (`continue-on-error` was not used).

| OS | Artifact | `window_created` | Result |
| --- | --- | --- | --- |
| ubuntu-latest | none (panic before report) | n/a | panic: `libxkbcommon-x11.so` not loaded |
| windows-latest | `windowed-gui-report-windows-latest` | false | blocked: `WGPU error: Failed to create surface for any enabled backend` |
| macos-latest | `windowed-gui-report-macos-latest` | false | blocked: `WGPU error: Failed to create surface for any enabled backend` |

Follow-up in #199: install `libxkbcommon-x11-0` + Mesa dri on Linux, write a blocked report on panic (so Ubuntu still uploads `report.toml`), and prefer WARP (`Microsoft Basic Render Driver`) on Windows.

Second dispatch after #199: [run 33570231649](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33570231649) on `b94a0dc0`.

| OS | Result |
| --- | --- |
| ubuntu-latest | passed (`xvfb` + xkb) |
| windows-latest | failed — `WGPU error: Failed to create surface for any enabled backend: {}` |
| macos-latest | failed — same empty-backend surface error |

Cause of the empty `{}`: the GUI step exported `WGPU_BACKEND=` (empty string) on Windows and macOS. `wgpu::Backends::from_env` treats a set variable as the whole mask, so the instance had **zero** backends. Linux was fine because it set `gl`.

#204 stopped exporting empty `WGPU_BACKEND` and set `gl` / `dx12` / `metal` per OS.

Third dispatch after that fix: [run 33584539014](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33584539014) on `982f6789` (merge #204). All three jobs passed. GUI step hard-fail (`continue-on-error` absent).

| OS | Artifact | `window_created` | open / edit / save | Result |
| --- | --- | --- | --- | --- |
| ubuntu-latest | [`ubuntu-33584539014.toml`](windowed-gui-3os/ubuntu-33584539014.toml) | true | passed | passed (`xvfb` + `WGPU_BACKEND=gl`) |
| windows-latest | [`windows-33584539014.toml`](windowed-gui-3os/windows-33584539014.toml) | true | passed | passed (`WGPU_BACKEND=dx12`) |
| macos-latest | [`macos-33584539014.toml`](windowed-gui-3os/macos-33584539014.toml) | true | passed | passed (`WGPU_BACKEND=metal`) |

This is **clock run 1 of 4**. It is not owner sign-off and not a required check.

Clock run 2: [run 33587226828](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33587226828) on `3d92bf35` (`main` after #204). All three jobs passed.

| OS | Artifact | `window_created` | Result |
| --- | --- | --- | --- |
| ubuntu-latest | [`ubuntu-33587226828.toml`](windowed-gui-3os/ubuntu-33587226828.toml) | true | passed |
| windows-latest | [`windows-33587226828.toml`](windowed-gui-3os/windows-33587226828.toml) | true | passed |
| macos-latest | [`macos-33587226828.toml`](windowed-gui-3os/macos-33587226828.toml) | true | passed |

This is **clock run 2 of 4**. Consecutive with run 1. Not owner sign-off. Not a required check.

Clock run 3: [run 33621158550](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33621158550) on `f2a356c3` (`main` after #205). All three jobs passed.

| OS | Artifact | `window_created` | Result |
| --- | --- | --- | --- |
| ubuntu-latest | [`ubuntu-33621158550.toml`](windowed-gui-3os/ubuntu-33621158550.toml) | true | passed |
| windows-latest | [`windows-33621158550.toml`](windowed-gui-3os/windows-33621158550.toml) | true | passed |
| macos-latest | [`macos-33621158550.toml`](windowed-gui-3os/macos-33621158550.toml) | true | passed |

This is **clock run 3 of 4**. Consecutive with runs 1 and 2. Not owner sign-off. Not a required check.

Clock run 4: [run 33627790410](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33627790410) on `4e0708a4` (`main` after #206). All three jobs passed.

| OS | Artifact | `window_created` | Result |
| --- | --- | --- | --- |
| ubuntu-latest | [`ubuntu-33627790410.toml`](windowed-gui-3os/ubuntu-33627790410.toml) | true | passed |
| windows-latest | [`windows-33627790410.toml`](windowed-gui-3os/windows-33627790410.toml) | true | passed |
| macos-latest | [`macos-33627790410.toml`](windowed-gui-3os/macos-33627790410.toml) | true | passed |

This is **clock run 4 of 4**. Consecutive with runs 1–3.

Owner sign-off 2026-09-02: [`gap-01-2-windowed-gui-clock-signoff.md`](gap-01-2-windowed-gui-clock-signoff.md). The clock is complete. **Not a required check.** Completing the clock is not the same as adding windowed-gui to `protect-main` or `legion-gates.yml`, and this file does not do that.

Ledger row statuses are unchanged.
