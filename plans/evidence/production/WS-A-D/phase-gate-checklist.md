# WS-A-D phase gate checklist

Use before starting the next phase. Standing gates remain required for every code merge.

## Phase 0 — Scaffolding

- [x] Campaign charter exists (`campaign-charter.md`)
- [x] Evidence folders created
- [ ] Charter PR merged to main
- [ ] Product-readiness ledger points at WS-A-D as next wave (no false “ready” flips)

## Phase 1 — Dogfood (A)

- [ ] ≥3 template-complete journals under `plans/evidence/dogfood/` (2 recorded; need interactive GUI + optional third)
- [ ] At least one interactive GUI session (not source-review-only)
- [x] Floor-bug queue triaged (fixed or accepted with owner) — F1 fixed; K1–K3 accepted cut lines
- [x] Phase 1 interim closeout written in `phase-1-dogfood/`
- [x] Go/no-go for Phase 2 recorded (**go** for B0 ADR)

### Session workflow minimum (from charter)

| # | Workflow | Pass? |
|---|----------|-------|
| 1 | Open workspace / tree / files | yes (automated/beta) |
| 2 | Edit + proposal-mediated save | yes |
| 3 | Keys do not leak into BYOK/terminal | yes (input_conformance) |
| 4 | Terminal PTY | yes (app terminal_workflow) |
| 5 | Assist proposal (fixture and/or live) | yes after F1 fix |
| 6 | BYOK store/clear | yes (provider_key_entry) |
| 7 | Delegate chat stream | yes (delegated_task_integration) |
| 8 | Git projection (if used) | yes |
| 9 | Debug shows honest simulated banner (until Phase 2) | yes |
| 10 | Sandbox panel matches enforcement report | yes |

## Phase 2 — Real DAP (B)

- [ ] B0 ADR merged (Proposed draft: `plans/adrs/ADR-0044-dap-client-architecture.md`)
- [ ] B1 fake-adapter CI green (`B1-framing-fake-adapter.md` + `live_dap_handshake` test)
- [ ] B2 breakpoints / stack / step / disconnect (`B2-breakpoints-stack-step.md`)
- [ ] B3 policy deny untrusted + adapter resolve + dual-mode banner (`B3-resolution-trust-dual-mode.md`)
- [ ] USER_GUIDE dual-mode honesty (simulated vs live)
- [ ] Evidence under `phase-2-dap/`

## Phase 3 — Sandbox isolation (C)

- [x] C0 threat model / acceptance matrix (stub: `C0-threat-model-stub.md`)
- [ ] C1 Linux network isolation + escape probe (`C1-linux-network-isolation.md` — bwrap unshare-net)
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
