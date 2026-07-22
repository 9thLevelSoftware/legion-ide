# WS-A-D phase gate checklist

Use before starting the next phase. Standing gates remain required for every code merge.

## Phase 0 — Scaffolding

- [x] Campaign charter exists (`campaign-charter.md`)
- [x] Evidence folders created
- [x] Charter on main (via early WS-A-D docs / campaign tree)
- [x] Product-readiness ledger not falsely flipped “ready”

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
| 9 | Debug dual-mode honesty (simulated vs live) | yes (Phase 2 B3) |
| 10 | Sandbox panel matches enforcement report | yes (Phase 3 C3) |

## Phase 2 — Real DAP (B)

- [x] B0 ADR (`plans/adrs/ADR-0044-dap-client-architecture.md`)
- [x] B1 fake-adapter CI green (`B1-framing-fake-adapter.md`)
- [x] B2 breakpoints / stack / step / disconnect (`B2-breakpoints-stack-step.md`)
- [x] B3 policy deny untrusted + adapter resolve + dual-mode banner (`B3-resolution-trust-dual-mode.md`)
- [x] USER_GUIDE dual-mode honesty (simulated vs live; Legion provisional wire)
- [x] Evidence under `phase-2-dap/`
- [x] B4 Microsoft DAP codec + fake-adapter contract (`B4-microsoft-dap-codec.md`; PATH resolve re-enabled)
- [x] B5 persistent live session for step/continue (`B5-persistent-live-session.md`)
- [x] B6 continue-until-stop + `:debug-stop` disconnect (`B6-continue-stop.md`)
- [x] B7 non-blocking continue + `:debug-poll` (`B7-nonblocking-continue-poll.md`)
- [x] B8 desktop auto-poll after live continue (`B8-desktop-auto-poll.md`)
- [ ] Follow-on: dogfood vs system lldb-dap; interactive GUI continue dogfood

## Phase 3 — Sandbox isolation (C)

- [x] C0 threat model / acceptance matrix (stub: `C0-threat-model-stub.md`)
- [x] C1 Linux network isolation (`C1-linux-network-isolation.md` — bwrap unshare-net)
- [x] C2 Windows FS residual cut line honest (`C2-windows-fs-residual.md`)
- [x] C3 product spawn integration (`C3-product-spawn-integration.md` — live report → panel)
- [x] `docs/SECURITY.md` matrix updated (C1 + C2 + C3 product path)
- [x] Evidence under `phase-3-sandbox/`

## Phase 4 — WS17 release (D)

- [x] D0 packaging design (artifact matrix + secrets inventory)
- [x] D1 unsigned preview portable artifacts + CI workflow (`D1-unsigned-preview-artifacts.md`)
- [x] D2 **unsigned-beta retained** for OS installers (`D2-unsigned-beta-retained.md`; real signing = D2.1 when secrets exist)
- [x] D3 update channel design + **local** staging drill proven (`D3-update-channel-staging.md`; hosted feed = D3.1)
- [x] D4 readiness close — ledger note + 3-OS preview CI proof (`D4-readiness-close.md`; no false ready flip)
- [x] Evidence under `phase-4-release/` (D0–D4)

## Phase 5 — Program close-out

- [x] Campaign closeout MD (`campaign-closeout-2026-07-22.md`)
- [ ] Dogfood on **installed** preview build (interactive residual)
- [x] Ledger rows updated only where evidence supports (PR-REL-001 remains In progress; WS-A-D cited)
- [x] Residual cut lines listed (still no VSIX)
