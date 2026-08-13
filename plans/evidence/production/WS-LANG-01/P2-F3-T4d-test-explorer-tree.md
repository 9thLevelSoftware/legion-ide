# P2.F3.T4d — Test explorer module-path tree grouping

**Date:** 2026-07-23  
**Status:** Substrate thin slice (not product-workflow-validated)

## Scope delivered

| Item | Notes |
| --- | --- |
| Grouping | `legion_ui::group_test_explorer_items_by_parent` (projection-only; no app dep) |
| Display rows | `format_test_explorer_tree_rows` emits `group <path> (n)` headers + indented items |
| Cap | `MAX_TEST_EXPLORER_TREE_DISPLAY_ROWS` (48) with `tree-omitted-items=` honesty |
| Desktop | Manual Context Tests panel uses tree rows; flat items unchanged for commands |

## Explicit non-claims

- Display-only tree; no collapsible egui tree widget yet.
- No multi-select / run-group command.
- **PR-LANG-002** remains **Substrate validated**.

## Verification

```text
cargo test -p legion-desktop --test projection_rendering projection_rendering_tests_preserve_app_boundary
cargo test -p legion-app --lib test_explorer
```
