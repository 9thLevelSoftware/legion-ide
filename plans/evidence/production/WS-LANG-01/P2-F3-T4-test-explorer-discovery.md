# P2.F3.T4 — Test explorer cargo discovery (thin slice)

**Date:** 2026-07-23  
**Status:** Substrate thin slice (not product-workflow-validated)

## Scope delivered

| Item | Notes |
| --- | --- |
| `cargo test -- --list` parser | Metadata-only labels (`item_id`, leaf label, kind, parent) |
| Discovery timeout | 60s hard timeout; kill + honest `timeout` status |
| Projection | `TestExplorerProjection` on shell snapshot |
| Commands | `:test-refresh` / palette `refresh-tests` / desktop **Refresh tests** |
| UI | Manual Context **Tests** panel shows discovered rows |
| Cap | 500 items max with omission diagnostic |

## Explicit non-claims

- Does **not** run tests from the explorer (still "Run cargo test" terminal launch).
- Does **not** use LSP runnables yet (kanban stop condition prefers LSP when available).
- Does **not** flip **PR-LANG-002** off Substrate validated.
- Windows/macOS/Linux: discovery depends on local `cargo` on PATH.

## Verification

```text
cargo test -p legion-app --test test_explorer_workflow
cargo test -p legion-app --lib test_explorer
```

## Follow-ons

1. Prefer LSP code-lens runnables when present; fall back to cargo list.
2. Per-item run → `TestRunSummary` + verification projection.
3. Tree grouping by module path in desktop dock.
