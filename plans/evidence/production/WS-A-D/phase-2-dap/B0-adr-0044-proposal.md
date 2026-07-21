# Phase 2 B0 — ADR-0044 proposal evidence

**Date:** 2026-07-21  
**Status:** Proposed (awaiting acceptance + dependency-policy follow-on)

## Delivered

| Item | Path |
| --- | --- |
| ADR | `plans/adrs/ADR-0044-dap-client-architecture.md` |
| Campaign link | `../campaign-charter.md` Phase 2 |

## Explicitly not in B0

- No adapter process spawn
- No dependency-policy edge activation yet (recorded as prerequisite for B1)
- No USER_GUIDE flip from simulated-only

## Acceptance criteria for “B0 done”

- [ ] ADR reviewed / accepted (status flipped to Accepted in ADR file)
- [ ] Dependency-policy PR ready or linked for B1 (`legion-debug` process edge)
- [ ] Fake-adapter design named in ADR (CI strategy)

## Next

B1: stdio framing + supervised spawn + fake adapter contract tests.
