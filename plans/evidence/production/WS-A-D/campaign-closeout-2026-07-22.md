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
| 2+ | **Post-closeout DAP residuals (B4–B11):** Microsoft wire, persistent live, non-blocking continue, desktop auto-poll, system handshake dogfood, headless continue dogfood, debug toolbar | #78–#84 + B11; `phase-2-dap/B4`…`B11` |
| 3 | C1 Linux bwrap net, C2 Windows residual, C3 product spawn → panel | #71, #73, #74; `phase-3-sandbox/` |
| 4 | D0 design, D1 3-OS preview CI, D2 unsigned-beta retained, D3 local update drill, D4 ledger note | #72, #73, #75, #76, this closeout; `phase-4-release/` |

### Phase 4 D4 smoke proof

Hosted **Legion Preview** 3-OS success:  
https://github.com/9thLevelSoftware/legion-ide/actions/runs/29887799213

## Residual cut lines (still open)

| Residual | Track | Notes |
| --- | --- | --- |
| Interactive GUI dogfood journals (≥1 remaining vs Phase 1 “≥3”) | A | Phase 1 floor automated; human **windowed** GUI session still needed (B10 is headless only) |
| ~~Microsoft DAP wire + fake contract~~ | B | **Closed** B4 (`B4-microsoft-dap-codec.md`) |
| ~~Persistent live DAP session~~ | B | **Closed** B5–B6 |
| ~~Non-blocking continue + desktop auto-poll~~ | B | **Closed** B7–B8; B10 headless dogfood |
| System adapter full launch/step vs host debugee | B | B9 = optional initialize handshake only |
| DAP adapter sandbox wrap | C | Deferred P2 |
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

1. Interactive Legion-on-Legion **windowed** GUI dogfood journal on current `main` (Phase 1 residual / Phase 5 installed-preview dogfood); use B11 debug toolbar.
2. System `lldb-dap`/`codelldb` full launch/step against a host debugee when a working adapter is available (`LEGION_DAP_DOGFOOD=1`).
3. D2.1 / D3.1 when CI secrets + staging hosting are available.
4. Optional D4.1 fresh-VM matrix when installers exist.

## Sign-off

| Item | Value |
| --- | --- |
| Campaign | WS-A-D |
| Closeout date | 2026-07-22 |
| Main tip at closeout | see merge of this PR |
| False readiness claims | **None** |
