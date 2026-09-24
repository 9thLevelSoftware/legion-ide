# AccessKit Product Pass and GP Screen-Reader Walkthroughs

## Status

- OS accessibility-tree inspection, **Windows**: captured 2026-08-16 and repeatable via `scripts/a11y-uia-walk.ps1`. 138 UIA descendants under the product window, with `Button` / `TabItem` / `StatusBar` / `Text` control types carrying product labels. Raw output: `plans/evidence/production/PR-UI-001/2026-08-16-windows-uia-tree.txt`.
- Screen-reader session, **Windows Narrator**: captured 2026-09-02 against a live `Legion IDE Smoke` window. Transcript: `plans/evidence/accessibility/2026-09-02-windows-narrator-transcript.txt`. Probe: `scripts/a11y-narrator-transcript.ps1`. A UIA tree dump is not this session.
- OS accessibility-tree inspection, **macOS**: committed probe `scripts/a11y-ax-walk.sh`. Hosted dump from that script: `plans/evidence/production/WS-P0/gap-05-3-macos-ax-dump.txt` (run 33638436515, `AX_WALK_OK`). The 2026-08-16 WS18-T2 dump remains a separate unreproducible external observation. Not VoiceOver.
- OS accessibility-tree inspection, **Linux**: committed probe `scripts/a11y-atspi-walk.sh`. Hosted xvfb dispatch did not see AccessKit on AT-SPI (`plans/evidence/production/WS-P0/gap-05-4-linux-atspi-miss.txt`). Not Orca.
- In-process AccessKit/egui projection coverage: passing (11/11 on Windows, 2026-08-16), and re-runnable on any host.
- Companion GP walkthrough documents: captured for GP-1, GP-2, and GP-3. They are **reconstructions from the macOS accessibility-tree dump and from current shell labels**, not recordings of an NVDA/VoiceOver/Orca session.

> **Scope repair, 2026-08-16.** This file previously read "Product-level accessibility evidence: passed" with no platform or reproducibility qualifier, which contradicted `plans/evidence/gui-productization/phase-7-known-limitations.md` ("OS accessibility tree inspection remains not observed in the current smoke evidence"). The contradiction was resolved by re-running the harness rather than by argument; see `plans/evidence/production/PR-UI-001/2026-08-16-promotion-verification.md`. The known-limitations entry was correct about every re-runnable harness in this tree. The status above is now scoped to match what can be reproduced.

## Purpose

Record the accessibility evidence for the Legion desktop shell and the golden-path walkthrough documents (GP-1..GP-3).

Part of this evidence is product-facing rather than projection-only. Windows has a reproducible UIA walk plus a Narrator session. macOS has a hosted AX dump from the committed probe (not VoiceOver). Linux still has no live AT-SPI dump on hosted xvfb.

PR-15 adds `scripts/a11y-platform-probe.sh` as a deterministic status
contract and `PR-15-manual-keyboard-path.md` as the honest keyboard-only path.
GAP-05.1 records which routes are renderer-certified versus residual typed-shell.
The contract delegates Windows to the committed UIA walk, macOS to the
committed AX walk (hosted dump exists), and Linux to the committed AT-SPI
walk (hosted xvfb still misses).

## Source evidence

- OS accessibility-tree inspection for the product window, **Windows, repeatable**: `scripts/a11y-uia-walk.ps1` → `plans/evidence/production/PR-UI-001/2026-08-16-windows-uia-tree.txt`
- OS accessibility-tree inspection for the product window, **macOS only, one-off, external probe with no committed source**: `plans/evidence/production/M5/WS18-T2-accesskit-product-pass.md`
- Product shell labels from the desktop view surfaces in `crates/legion-desktop/src/view/*.rs`
- Accessibility projection coverage from `crates/legion-desktop/tests/accessibility.rs` — in-process AccessKit node assertions; these never leave the process and are not OS-tree evidence
- Windows Narrator live-window transcript: `scripts/a11y-narrator-transcript.ps1` → `plans/evidence/accessibility/2026-09-02-windows-narrator-transcript.txt`

## What this evidence does not cover

- Linux OS accessibility tree: probe committed (`scripts/a11y-atspi-walk.sh`). Hosted xvfb did not get an AccessKit AT-SPI tree. Not Orca.
- macOS VoiceOver: the committed AX dump is not a VoiceOver session. The 2026-08-16 WS18-T2 dump is still a separate unreproducible external observation.
- Harness reporting: `legion-desktop --smoke` reports `Windows UIA observed N descendants` from `accessibility_tree_status` when the committed probe `scripts/a11y-uia-walk.ps1` actually prints `UIA_WALK_OK` against the live process. macOS and Linux keep `OS tree not observed` in the smoke status string until those probes have been run in the same process path. This does not close the 3-OS `PR-UI-001` bar.
- CI: `.github/workflows/legion-a11y-os-tree.yml` is an independent dispatch job for the AX/AT-SPI walks. It is not a PR merge gate.
- Depth of the Windows UIA walk: it shows control types and names reach the OS layer. It is not an audit of label quality, live regions, high contrast, or reduced motion.
- Screen-reader sessions: Windows Narrator was recorded on 2026-09-02 (GAP-05.2). No NVDA, VoiceOver, or Orca run has been recorded. `plans/evidence/production/M5/WS18-T2-accesskit-product-pass.md` still says so for those ATs under "Residual risk".

## Walkthrough documents

Read these as label inventories of the accessible surface, not as screen-reader recordings. GP-1's quoted utterances are the `AXStaticText` values from the macOS dump, in the same order.

- `plans/evidence/accessibility/gp-1-manual-walkthrough.md`
- `plans/evidence/accessibility/gp-2-assist-walkthrough.md`
- `plans/evidence/accessibility/gp-3-delegate-walkthrough.md`
- `plans/evidence/accessibility/PR-15-manual-keyboard-path.md`

## Acceptance note

The product pass is complete only when the product window is observable via the OS accessibility tree **on each supported OS, by a probe committed to this repository so the observation can be repeated**, and the GP walkthroughs each include a transcript captured from a real screen-reader session. Neither condition holds today. `PR-UI-001` therefore stays at substrate validated, consistent with `plans/product-readiness-ledger.md`.
