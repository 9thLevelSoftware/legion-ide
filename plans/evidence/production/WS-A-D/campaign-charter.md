# WS-A-D campaign charter — Dogfood → DAP → Sandbox → Release

**Opened:** 2026-07-21  
**Predecessor:** WS-P0 product wiring (closed: #63, #64, #66)  
**Out of scope:** VSIX / extension host / marketplace, collab transport, SSH remote UX.

## Purpose

Ship the next product wave after the daily-driver floor:

| Phase | Track | Goal |
| --- | --- | --- |
| **0** | Scaffolding | This charter, evidence layout, ledger honesty |
| **1** | **A — Dogfood** | Legion-on-Legion journals + floor-bug triage |
| **2** | **B — Real DAP** | Adapter process + wire protocol (CI via fake adapter) |
| **3** | **C — Sandbox isolation** | Stronger OS enforcement with honest reports |
| **4** | **D — WS17 release** | Real installers, signing path, update channel |
| **5** | Close-out | Ledger flips only where evidence supports them |

## Sequencing

```text
Phase 0  Scaffolding
    │
Phase 1  Dogfood (gate for everything else)
    │
    ├──────────────┬──────────────┐
Phase 2 Real DAP   Phase 3 Sandbox   (parallel after Phase 1)
    │              │
    └──────┬───────┘
           ▼
Phase 4  WS17 release
           ▼
Phase 5  Close-out
```

**Why:** Dogfood surfaces broken-floor bugs first. DAP and sandbox are independent after a stable floor. Signed installers are only worth the ops cost once dogfood says the product is usable enough to distribute (unsigned preview artifacts may start as D1 earlier).

## Merge policy

- Standing 20-gate matrix (`AGENTS.md`) is merge authority for code PRs.
- Each phase lands **evidence** under this tree or `plans/evidence/dogfood/`.
- Dual paths until proven: fixture DAP, dry-run release descriptors, honest sandbox reports.
- No private signing keys/certs/tokens in-repo.
- Saves stay proposal-mediated; UI stays projection-only.

## Phase definitions of done (summary)

| Phase | Done when |
| --- | --- |
| **0** | Charter + folders exist; no false readiness claims |
| **1** | ≥3 dogfood journals (template-complete); floor bugs fixed or explicitly accepted |
| **2** | Fake-adapter CI green; real adapter path documented; untrusted launch denied; SIMULATED banner dual-mode |
| **3** | Linux network isolation enforced + tested; Windows improved or residual cut line still honest; SECURITY.md updated |
| **4** | Preview installers on 3 OS families; signing **or** explicit unsigned-beta; update drill against staging feed |
| **5** | Closeout MD; dogfood on **installed** preview; ledger rows honest |

## Evidence index

| Path | Role |
| --- | --- |
| `campaign-charter.md` | This document |
| `phase-gate-checklist.md` | Go/no-go checklist per phase |
| `phase-1-dogfood/` | Phase 1 closeout + links to journals |
| `phase-2-dap/` | DAP ADR links + slice evidence |
| `phase-3-sandbox/` | Sandbox slice evidence |
| `phase-4-release/` | WS17 install/sign/update evidence |
| `plans/evidence/dogfood/` | Session journals (Phase 1 + ongoing) |

## Cross-links (substrate)

| Topic | Source |
| --- | --- |
| WS-P0 closeout | `plans/evidence/production/WS-P0/campaign-closeout-2026-07-21.md` |
| Dogfood template | `plans/dogfood/legion-on-legion-weekly-journal-template.md` |
| Dogfood evidence README | `plans/evidence/dogfood/README.md` |
| Simulated DAP cut line | `plans/evidence/production/WS-P0/T3-dap-honest-cut-line.md` |
| Sandbox matrix | `docs/SECURITY.md` (§ Sandbox guarantees) |
| WS17.T1 dry-run | `AGENTS.md` + `xtask` release-pipeline |
| WS17.T2 unsigned-beta | `plans/evidence/production/M5/WS17-T2-signing-notarization.md` |
| Update strategy | `plans/adrs/ADR-0042-auto-update-strategy-and-signed-manifest.md` |
| Product readiness | `plans/product-readiness-ledger.md` |
| USER_GUIDE cut lines | `docs/USER_GUIDE.md` product-areas callout |

## Immediate next steps (updated 2026-07-21)

1. ~~Charter / Phase 0~~ — done.
2. Phase 1 residual: ≥1 interactive GUI dogfood journal (+ third if needed).
3. ~~Phase 2 B0–B3~~ — on main (Microsoft DAP codec still follow-on).
4. ~~Phase 3 C1 + C2 + C3~~ — on main (DAP adapter sandbox wrap still residual).
5. ~~Phase 4 D0–D2~~ — design + portable CI archives; **unsigned-beta retained** until OS signing secrets (D2.1); D3 update feed next.
