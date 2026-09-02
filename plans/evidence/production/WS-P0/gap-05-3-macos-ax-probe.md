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

Follow-up in this change: ASCII `>=` in the AX script, capture osascript status without `errexit`, start dbus + AT-SPI bus on Linux, dump registered AT-SPI app names on a miss.

GAP-05.3 and GAP-05.4 stay open. Not VoiceOver. Not Orca.

Ledger row statuses are unchanged.
