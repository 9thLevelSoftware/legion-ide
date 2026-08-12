# P2.F3.T4b — Test explorer per-item exact run

**Date:** 2026-07-23  
**Status:** Substrate thin slice (not product-workflow-validated)

## Scope delivered

| Item | Notes |
| --- | --- |
| Exact run | `cargo test -- --exact <item_id>` with hard timeout (120s) |
| Item id validation | Fail-closed alphanumeric / `_` / `:` / `.` only |
| Projection | `last_run_*` fields + diagnostics on `TestExplorerProjection` |
| Verification merge | Recent runs (cap 20) appear in `VerificationRunProjection` as `cargo-test-exact` |
| Commands | `:test-run <item_id>` / desktop **Run first listed test** |
| Redaction | Raw stdout used only to parse summary counts; not projected |

## Explicit non-claims

- Does **not** prefer LSP runnables (still follow-on).
- Does **not** provide a rich tree UI or per-row click handlers for all items.
- Does **not** flip **PR-LANG-002** off Substrate validated.
- Full cargo logs remain redacted (`command_body_redacted = true`).

## Verification

```text
cargo test -p legion-app --test test_explorer_workflow
cargo test -p legion-app --lib test_explorer
```
