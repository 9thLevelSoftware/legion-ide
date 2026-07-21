# WS-A-D phase gate checklist

Use before starting the next phase. Standing gates remain required for every code merge.

## Phase 0 — Scaffolding

- [x] Campaign charter exists (`campaign-charter.md`)
- [x] Evidence folders created
- [ ] Charter PR merged to main
- [ ] Product-readiness ledger points at WS-A-D as next wave (no false “ready” flips)

## Phase 1 — Dogfood (A)

- [ ] ≥3 template-complete journals under `plans/evidence/dogfood/`
- [ ] At least one interactive GUI session (not source-review-only)
- [ ] Floor-bug queue triaged (fixed or accepted with owner)
- [ ] Phase 1 closeout written in `phase-1-dogfood/`
- [ ] Go/no-go for Phase 2 recorded

### Session workflow minimum (from charter)

| # | Workflow | Pass? |
|---|----------|-------|
| 1 | Open workspace / tree / files | |
| 2 | Edit + proposal-mediated save | |
| 3 | Keys do not leak into BYOK/terminal | |
| 4 | Terminal PTY | |
| 5 | Assist proposal (fixture and/or live) | |
| 6 | BYOK store/clear | |
| 7 | Delegate chat stream | |
| 8 | Git projection (if used) | |
| 9 | Debug shows honest simulated banner (until Phase 2) | |
| 10 | Sandbox panel matches enforcement report | |

## Phase 2 — Real DAP (B)

- [ ] B0 ADR merged
- [ ] B1 fake-adapter CI green
- [ ] B2 breakpoints / stack / step / disconnect
- [ ] B3 policy deny untrusted + documented adapter install
- [ ] USER_GUIDE dual-mode honesty (simulated vs live)
- [ ] Evidence under `phase-2-dap/`

## Phase 3 — Sandbox isolation (C)

- [ ] C0 threat model / acceptance matrix
- [ ] C1 Linux network isolation + escape probe
- [ ] C2 Windows FS path (enforced **or** residual cut line still honest)
- [ ] C3 product spawn integration
- [ ] `docs/SECURITY.md` matrix updated
- [ ] Evidence under `phase-3-sandbox/`

## Phase 4 — WS17 release (D)

- [ ] D0 packaging design (artifact matrix + secrets inventory)
- [ ] D1 unsigned preview artifacts build on 3 OS families
- [ ] D2 signing path **or** explicit unsigned-beta retained
- [ ] D3 update channel + drill against staging
- [ ] D4 readiness close (fresh-VM smoke or ledger note)
- [ ] Evidence under `phase-4-release/`

## Phase 5 — Program close-out

- [ ] Campaign closeout MD
- [ ] Dogfood on **installed** preview build
- [ ] Ledger rows updated only where evidence supports
- [ ] Residual cut lines listed (still no VSIX)
