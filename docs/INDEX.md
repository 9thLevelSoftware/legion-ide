# Legion IDE — Documentation Index

This index is the canonical entry point for the Legion IDE documentation set under `docs/`. Use it as a starting point whenever you are looking for the authoritative doc for a topic; if a doc is not listed here, treat it as supporting material rather than a primary reference.

## Audience map

| If you are a… | Start here |
| --- | --- |
| New agent or contributor | `AGENTS.md` at the repo root, then this index |
| End user or product reader | `USER_GUIDE.md` |
| Architect / reviewer | `ARCHITECTURE_AUTHORITY_BOUNDARIES.md` |
| Security reviewer / vulnerability reporter | `SECURITY.md` |
| Operator running the gates | `OPERATOR_RUNBOOK.md` |
| Keyboard reference reader | `KEYBOARD_REFERENCE.md` |
| Support / triage responder | `TROUBLESHOOTING.md` |
| Product / roadmap reader | `LEGION_PIVOT.md` |
| Mode-policy reader (Manual / Assist / Delegate / Legion Workflow) | `MODES.md` |
| Reading the rename / historical naming | `LEGION_RENAME.md` |

## Canonical documents

- `USER_GUIDE.md` — end-user entry point for the current product paths, support surfaces, and where to go next.
- `KEYBOARD_REFERENCE.md` — the projected shortcut labels that are currently surfaced by the product UI.
- `TROUBLESHOOTING.md` — diagnostic bundle guidance for smoke failures, package failures, and release support.
- `ARCHITECTURE_AUTHORITY_BOUNDARIES.md` — canonical ownership rules across the UI, app composition, workspace/project, AI/provider, and other layers. Read this before making any change that crosses a layer boundary.
- `SECURITY.md` — public-facing security model, platform caveats, egress policy, secret-handling posture, plugin isolation, and responsible-disclosure policy.
- `LEGION_PIVOT.md` — product direction, pivot context, and the boundaries between the current validated substrate and the eventual product surface.
- `MODES.md` — semantics and boundaries of the Manual, Assist, Delegate, and Legion Workflow modes; includes what each mode permits and forbids.
- `OPERATOR_RUNBOOK.md` — operator-oriented gate list, subagent execution pattern, safety checks, and Phase 8 dry-run procedures. The runbook is the day-to-day reference for running the phase gates and dispatching work.
- `LEGION_RENAME.md` — history of the Devil → Legion rename, including the rationale for allowing old Devil-era markers to remain in archived evidence and the rules for current user-facing docs.

## Supporting material outside `docs/`

- `../AGENTS.md` — concise agent/developer invariants and required phase gates. Lives at the repo root so that agents see it first.
- `../ENGINEERING_STATUS.md` — historical audit status snapshot (date, branch, HEAD) for the prior engineering audit cycle. **Historical**; do not treat it as a current status report. See the "Current Status" section of `../README.md` and `../plans/product-readiness-ledger.md` for the current state.
- `../plans/` — phase plans, evidence packages, ADRs, and the product-readiness ledger.
- `../plans/legion-production-master-plan-v0.1.md` — the production master plan: current-state assessment, 2026 market/technology gap analysis, ADR queue (ADR-0032..0040), workstreams WS-01..WS-20, and milestones M0..M6 from validated substrate to product GA.
- `../plans/legion-customizable-autonomy-continuation-plan-v0.1.md` — continuation plan re-centering the path forward on three pillars: an old-school manual IDE, deep customizability (config-as-code, keymaps, themes, layouts, profiles), and a configurable autonomy continuum (ADR-0041..0045, WS-21..WS-29, milestones C1..C5) scaling from zero AI to a gated full-automation envelope.
- `../audit-reports/` — durable audit artifacts.

## How to use this index

- When a doc is listed here, treat its rules and definitions as the ones in force for current work.
- When you need to update a doc, update the doc itself and, if the topic or audience changes, update the audience map above.
- When a new doc is added under `docs/`, add an entry to the "Canonical documents" section and, if appropriate, to the audience map.
