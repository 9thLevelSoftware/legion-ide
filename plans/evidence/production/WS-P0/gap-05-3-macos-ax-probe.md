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

Ledger row statuses are unchanged.
