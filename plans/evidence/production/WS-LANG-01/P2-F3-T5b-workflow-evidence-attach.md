# P2.F3.T5b — Attach test-explorer evidence into workflow export

**Date:** 2026-07-23  
**Status:** Substrate thin slice (not product-workflow-validated)

## Scope delivered

| Item | Notes |
| --- | --- |
| Legion records | `test_explorer_legion_evidence_records()` via agent helper + synthetic worker `test-explorer` |
| Session attach | `:test-attach-evidence <session_id>` stores into session artifact bag (deduped) |
| Export merge | `export_legion_workflow_evidence_bundle` also merges latest explorer runs (deduped) |
| Redaction | Metadata-only; raw cargo logs never attached |

## Explicit non-claims

- Does **not** invent worker loops or auto-start a workflow.
- Synthetic worker id is for packaging only; not a live agent process.
- **PR-LANG-002** remains **Substrate validated**.

## Verification

```text
cargo test -p legion-app --test test_explorer_workflow
```
