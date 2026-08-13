# Phase 1 dogfood — interim closeout (2026-07-21)

## Status

**In progress** toward full Phase 1 DoD (≥3 journals). Automated floor verification + one floor fix landed this session.

## Journals

| Journal | Outcome |
| --- | --- |
| `plans/evidence/dogfood/2026-07-21-dogfood-journal.md` | Historical Tier-0 bootstrap (partial/blocked, pre-WS-P0 shipping) |
| `plans/evidence/dogfood/2026-07-21-phase1-floor-journal.md` | **This session** — automated floor + beta fix |

## Automated suites (all green after F1 fix)

See `automated-floor-run-2026-07-21.txt`:

- `legion-app` control_trust_surfaces, language_tooling_workflow, delegated_task_integration, git_workflow, terminal_workflow, workspace_vfs conflict sample
- `legion-desktop` input_conformance, sandbox_panel, debug_workflow, provider_key_entry, beta_workflow

## Floor bugs

| ID | Disposition |
| --- | --- |
| F1 beta Assist streaming | Fixed in product code (`legion-desktop` beta harness) |
| K1–K3 known cut lines | Deferred to Phases 2–4 |

## Go / no-go

| Next phase | Decision | Notes |
| --- | --- | --- |
| Phase 2 Real DAP (B0 ADR) | **Go** (can start design) | Floor not blocking; interactive GUI journals still desired |
| Phase 3 Sandbox | **Go** for C0 threat-model docs | After or parallel with B0 |
| Phase 1 formal complete | **Not yet** | Need ≥1 interactive GUI journal (+ ideally a third session) |

## Remaining Phase 1 work

1. Interactive eframe dogfood journal (human or agent-driven window session).
2. Optional second-OS journal.
3. Tick Phase 1 boxes in `../phase-gate-checklist.md` when ≥3 journals exist.
