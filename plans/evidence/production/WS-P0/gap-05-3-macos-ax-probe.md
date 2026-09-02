# GAP-05.3 — macOS AX probe (VoiceOver still open)

**Date:** 2026-09-02  
**Wave:** 2 proof surface  
**Task:** GAP-05.3 (partial)

## What this is

A committed, repeatable macOS AX walk: `scripts/a11y-ax-walk.sh` (System Events
UI tree of a live `legion-desktop` window). `scripts/a11y-platform-probe.sh`
routes Darwin to that script.

Linux gets the matching AT-SPI walk `scripts/a11y-atspi-walk.sh` (GAP-05.4
OS-tree half). Hosted capture is `.github/workflows/legion-a11y-os-tree.yml`
(dispatch, not a PR gate).

## What this is not

- Not VoiceOver notes (still required to close GAP-05.3)
- Not Orca notes (still required to close GAP-05.4)
- Not a live AX dump in this change (needs Darwin + a window)
- Not a ledger promotion of PR-UI-001

## First hosted dispatch

[run 33627787793](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33627787793) on `4e0708a4` (`main` after #206). Independent; not a PR gate.

| OS | Artifact | Result |
| --- | --- | --- |
| macos-latest | `macos-ax.txt` | `PROCESS_FOUND: legion-desktop`, then osascript syntax error on Unicode `≥` (`974:982`, exit 1). `set -e` skipped the documented exit 5. |
| ubuntu-latest | `linux-atspi.txt` | `PROCESS_NOT_FOUND: legion-desktop` (exit 4). The smoke process was running; AT-SPI published no matching app. Likely missing session bus / `at-spi-bus-launcher` under xvfb. |

Follow-up in #208: ASCII `>=` in the AX script, capture osascript status without `errexit`, start dbus + AT-SPI bus on Linux, dump registered AT-SPI app names on a miss.

Second hosted dispatch: [run 33636397701](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33636397701) on `019acb9b` (`main` after #208).

| OS | Artifact | Result |
| --- | --- | --- |
| macos-latest | `macos-ax.txt` | `PROCESS_FOUND`, then `AX_WALK_FAILED` exit 5. osascript `975:983` was `dumpState` after a single-line `if ... then return dumpState` (AppleScript treats `return` as a bare exit). |
| ubuntu-latest | `linux-atspi.txt` | Registry activated from the Python walk (`SpiRegistry daemon is running`), then `ATSPI_APPS:` empty / `PROCESS_NOT_FOUND`. AccessKit registers at process start; the bus was not up yet. |

This change: multi-line `if` before `return dumpState`; warm `Atspi.init()` before launching `legion-desktop`.

Third hosted dispatch: [run 33637474631](https://github.com/9thLevelSoftware/legion-ide/actions/runs/33637474631) on `be8e27c7` (#209 branch).

| OS | Result |
| --- | --- |
| macos-latest | Still `AX_WALK_FAILED` exit 5. osascript `1228:1236` — `return dumpState` remains a compile error even in a multi-line `if`. Flattened to one `on run` handler with a 2-level window walk and no record returns. |
| ubuntu-latest | `ATSPI_LAUNCHER=/usr/libexec/at-spi-bus-launcher`, `ATSPI_REGISTRY_READY desktop_children=0`, then still empty `ATSPI_APPS` after the app was up. Registry warmup is not sufficient; AccessKit unix did not publish. Retries + session-bus name dump added. |

GAP-05.3 and GAP-05.4 stay open. Not VoiceOver. Not Orca.

Ledger row statuses are unchanged.
