# Installed preview journal — 2026-09-01

Unsigned local package layout, not a signed installer.

## Session

- **Branch:** `gap/p0-windowed-gui`
- **Commit SHA:** `0a18441fde4c203ee6ce9497bcf3bde8d6045a7e` (origin/main) plus this harness change
- **OS / Platform:** Windows x86_64
- **Build method:** `cargo run -p xtask -- windowed-gui-e2e` (debug `legion-desktop`, copied into `target/windowed-gui/package/` with LICENSE, PRIVACY.md, THIRD_PARTY_NOTICES.md)
- **Session type:** windowed, `eframe::run_native`, automated open/edit/save (GAP-01.1)
- **Channel:** unsigned-beta

## Result

Passed. A native window was created. The packaged binary opened the fixture file, inserted `WINDOWED_E2E_EDIT`, and saved through proposal-mediated `SaveActive`. Disk after save: `WINDOWED_E2E_EDIT` then `seed`.

Report: `plans/evidence/production/WS-P0/gap-01-1-windowed-gui-e2e.toml`

## Checklist

| # | Action | Pass? | Notes |
| --- | --- | --- | --- |
| 1 | Binary starts without a false production claim | Yes | Unsigned package layout; report `not_beta_smoke` / `not_golden_path_5` |
| 2 | Open a workspace | Yes | Fixture `target/windowed-gui/workspace` |
| 3 | Manual edit + save | Yes | Automated in-window `InsertText` + `SaveActive` |
| 4 | Terminal launch | Not exercised | GAP-01.1 scope is open/edit/save |
| 5 | Debug | Not exercised | |
| 6 | Sandbox panel | Not exercised | |
| 7 | No crash on quit | Yes | Viewport close after save |

## What this is not

Not 3-OS. Not GAP-01.2 hosted evidence. Not a signed clean-VM install. Ledger row PR-REL-001 remains in progress.
