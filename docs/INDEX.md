# Legion IDE — Documentation Index

This index is the canonical entry point for the Legion IDE documentation set under `docs/`. Use it as a starting point whenever you are looking for the authoritative doc for a topic; if a doc is not listed here, treat it as supporting material rather than a primary reference.

## Audience map

| If you are a… | Start here |
| --- | --- |
| New agent or contributor | `AGENTS.md` at the repo root, then this index |
| Architect / reviewer | `ARCHITECTURE_AUTHORITY_BOUNDARIES.md` |
| Operator running the gates | `OPERATOR_RUNBOOK.md` |
| Product / roadmap reader | `LEGION_PIVOT.md` |
| Mode-policy reader (Manual / Assist / Delegate / Legion Workflow) | `MODES.md` |
| Reading the rename / historical naming | `LEGION_RENAME.md` |

## Canonical documents

- `ARCHITECTURE_AUTHORITY_BOUNDARIES.md` — canonical ownership rules across the UI, app composition, workspace/project, AI/provider, and other layers. Read this before making any change that crosses a layer boundary.
- `LEGION_PIVOT.md` — product direction, pivot context, and the boundaries between the current validated substrate and the eventual product surface.
- `MODES.md` — semantics and boundaries of the Manual, Assist, Delegate, and Legion Workflow modes; includes what each mode permits and forbids.
- `OPERATOR_RUNBOOK.md` — operator-oriented gate list, subagent execution pattern, safety checks, and Phase 8 dry-run procedures. The runbook is the day-to-day reference for running the phase gates and dispatching work.
- `LEGION_RENAME.md` — history of the Devil → Legion rename, including the rationale for allowing old Devil-era markers to remain in archived evidence and the rules for current user-facing docs.

## Supporting material outside `docs/`

- `../AGENTS.md` — concise agent/developer invariants and required phase gates. Lives at the repo root so that agents see it first.
- `../ENGINEERING_STATUS.md` — historical audit status snapshot (date, branch, HEAD) for the prior engineering audit cycle. **Historical**; do not treat it as a current status report. See the "Current Status" section of `../README.md` and `../plans/product-readiness-ledger.md` for the current state.
- `../plans/` — phase plans, evidence packages, ADRs, and the product-readiness ledger.
- `../audit-reports/` — durable audit artifacts.

## How to use this index

- When a doc is listed here, treat its rules and definitions as the ones in force for current work.
- When you need to update a doc, update the doc itself and, if the topic or audience changes, update the audience map above.
- When a new doc is added under `docs/`, add an entry to the "Canonical documents" section and, if appropriate, to the audience map.
