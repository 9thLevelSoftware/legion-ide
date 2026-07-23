# WS-A-D campaign closeout

**Closed:** 2026-07-22  
**Predecessor:** WS-P0 product wiring  
**Scope:** Dogfood → real DAP → sandbox isolation → WS17 release (**no VSIX**)

## Outcome

Campaign **program close** for the planned A–D wave, with **honest residuals**. No product-readiness row was flipped to “ready” without evidence.

### Delivered (on `main`)

| Phase | Delivered | Key PRs / evidence |
| --- | --- | --- |
| 0 | Charter + evidence tree | `campaign-charter.md` |
| 1 | Dogfood floor + interim closeout (interactive GUI journal residual) | #68; `phase-1-dogfood/` |
| 2 | DAP ADR-0044, fake adapter CI, B0–B3 dual-mode + trust deny | #69, #70; `phase-2-dap/` |
| 2+ | **Post-closeout DAP residuals (B4–B17):** Microsoft wire through smart F5, prebuild, system launch dogfood, keyboard, stop-on-entry default | #78–#91; `phase-2-dap/B4`…`B17` |
| 3 | C1 Linux bwrap net, C2 Windows residual, C3 product spawn → panel | #71, #73, #74; `phase-3-sandbox/` |
| 4 | D0 design, D1 3-OS preview CI, D2 unsigned-beta retained, D3 local update drill, D4 ledger note | #72, #73, #75, #76, this closeout; `phase-4-release/` |

### Phase 4 D4 smoke proof

Hosted **Legion Preview** 3-OS success:  
https://github.com/9thLevelSoftware/legion-ide/actions/runs/29887799213

## Residual cut lines (still open)

| Residual | Track | Notes |
| --- | --- | --- |
| Interactive GUI dogfood journals (≥1 remaining vs Phase 1 “≥3”) | A | Checklist ready (`INTERACTIVE-GUI-CHECKLIST.md`); human must still fill journal |
| Installed unsigned-beta preview dogfood | D/A | Checklist ready (`INSTALLED-PREVIEW-CHECKLIST.md`); human extract/run residual |
| ~~Microsoft DAP wire + fake contract~~ | B | **Closed** B4 |
| ~~Persistent live DAP session~~ | B | **Closed** B5–B6 |
| ~~Non-blocking continue + desktop auto-poll~~ | B | **Closed** B7–B8; B10 headless dogfood |
| ~~System adapter launch/step dogfood path~~ | B | **Closed** B12–B13 (`LEGION_DAP_DOGFOOD=1`); host LLDB quality not guaranteed |
| ~~Debug product keys / toolbar / smart F5~~ | B | **Closed** B11, B14–B17 |
| DAP adapter sandbox wrap | C | Deferred — needs long-lived piped spawn API (C0 P2) |
| Windows FS/network OS isolation beyond job object | C | C2 residual accepted honest |
| OS installer formats (MSI/DMG/deb) + D2.1 signing | D | Portable zip/tar.gz only |
| Hosted update feed D3.1 | D | Local `update-drill` only |
| Fresh-VM signed install smoke | D | Explicitly not claimed |
| VSIX / extension host | out of scope | Unchanged |

## Ledger honesty

- **PR-REL-001:** remains **In progress** — WS-A-D evidence added; signed installers / fresh-VM still open.
- **PR-LANG-002 (debug):** dual-mode honesty + live substrate advanced; not full product debugger.
- No silent “ready” flips.

## What to do next (post-campaign)

1. Human **windowed** GUI journal (`INTERACTIVE-GUI-CHECKLIST.md`) and/or **installed preview** journal (`INSTALLED-PREVIEW-CHECKLIST.md`).
2. Optional: re-run `LEGION_DAP_DOGFOOD=1` system adapter tests when a working host LLDB is available.
3. D2.1 / D3.1 when CI secrets + staging hosting are available.
4. Optional D4.1 fresh-VM matrix when installers exist.
5. DAP adapter sandbox wrap only after a streaming/sandboxed-stdio spawn API exists.

## Sign-off

| Item | Value |
| --- | --- |
| Campaign | WS-A-D |
| Closeout date | 2026-07-22 |
| Main tip at closeout | see merge of this PR |
| False readiness claims | **None** |
