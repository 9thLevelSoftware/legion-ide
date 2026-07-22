# Dogfood journal evidence

Weekly Legion-on-Legion dogfood journals live here.

**Active program:** Phase 1 residual + Phase 2 DAP dogfood of [WS-A-D](../production/WS-A-D/campaign-charter.md) (dogfood → DAP → sandbox → release).

### Journals (index)

| File | Kind |
| --- | --- |
| `2026-07-21-dogfood-journal.md` | Historical Tier-0 bootstrap |
| `2026-07-21-phase1-floor-journal.md` | Phase 1 automated floor |
| `2026-07-22-preview-artifact-journal.md` | Preview packaging |
| `2026-07-22-dap-b10-headless-journal.md` | DAP B10 headless continue auto-poll (**not** human windowed GUI) |

## Naming

Copy the template from `plans/dogfood/legion-on-legion-weekly-journal-template.md` to:

```text
plans/evidence/dogfood/YYYY-MM-DD-dogfood-journal.md
```

Use the UTC (or local session) date of the dogfood session. One journal per session day.

## Requirements

A journal entry is evidence only when it names:

- branch and commit SHA  
- OS / platform  
- workflow attempted  
- result (success / partial / blocked)  
- product-readiness impact (or explicit “none”)  

See `plans/product-readiness-ledger.md` § Dogfood Evidence.
