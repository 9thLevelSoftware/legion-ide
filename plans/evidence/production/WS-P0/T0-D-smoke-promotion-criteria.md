# T0-D — Golden-path smoke promotion criteria

**Date:** 2026-07-21  
**Packet:** Tier 0 truth repair — smoke remains non-blocking  

## Decision

**Do not** make `.github/workflows/legion-smoke.yml` a merge blocker or fold GP-1–4 into `legion-gates.yml` in Tier 0.

The same clock applies to `.github/workflows/legion-windowed-gui.yml` (GAP-01.2). Do not add it as a required check until four consecutive green 3-OS runs and owner sign-off exist. The GUI step on that workflow is hard-fail (`continue-on-error` forbidden).

## Promotion criteria (all required)

1. Four consecutive green scheduled (or fully equivalent) 3-OS smoke runs.  
2. rust-analyzer provision success (or accepted OS-specific skip with sign-off).  
3. Maintainer acceptance of PR-path cost.  
4. Written owner sign-off with run URLs/SHAs under `plans/evidence/production/`.

## Where documented

- `docs/OPERATOR_RUNBOOK.md` § Golden-path smoke promotion criteria  
- `AGENTS.md` standing-gate / CI paragraph  

## Non-change

Workflow `on:` triggers and independence from PR gates unchanged.
