# P2.F3.T4d — Test explorer module-path tree grouping

**Date:** 2026-07-23  
**Status:** Substrate thin slice (not product-workflow-validated)

## Scope delivered

| Item | Notes |
| --- | --- |
| Grouping | `group_items_by_parent` buckets by `parent_label` (or `<root>`) |
| Display rows | `format_tree_rows` emits `group <path> (n)` headers + indented items |
| Cap | `MAX_TREE_DISPLAY_ROWS` (48) with `tree-omitted-items=` honesty |
| Desktop | Manual Context Tests panel uses tree rows; flat items unchanged for commands |

## Explicit non-claims

- Display-only tree; no collapsible egui tree widget yet.
- No multi-select / run-group command.
- **PR-LANG-002** remains **Substrate validated**.

## Verification

```text
cargo test -p legion-app --lib test_explorer
cargo test -p legion-app --test test_explorer_workflow
```
