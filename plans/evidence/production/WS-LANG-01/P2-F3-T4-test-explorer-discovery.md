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

## Explicit non-claims (original slice)

- Original slice was list-only; later slices added run/LSP/tree (see follow-ons).
- Does **not** flip **PR-LANG-002** off Substrate validated.
- Windows/macOS/Linux: cargo discovery depends on local `cargo` on PATH.

## Verification

```text
cargo test -p legion-app --test test_explorer_workflow
cargo test -p legion-app --lib test_explorer
```

## Follow-ons

1. ~~Prefer LSP code-lens runnables when present; fall back to cargo list~~ — closed in `P2-F3-T4c-lsp-runnable-preference.md`.
2. ~~Per-item run → verification projection~~ — closed in `P2-F3-T4b-test-explorer-run.md`.
3. ~~Tree grouping by module path in desktop dock~~ — closed in `P2-F3-T4d-test-explorer-tree.md`.
