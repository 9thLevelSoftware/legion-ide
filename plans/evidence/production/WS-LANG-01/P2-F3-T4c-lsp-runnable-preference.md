# P2.F3.T4c — Prefer LSP runnable code lenses for test explorer

**Date:** 2026-07-23  
**Status:** Substrate thin slice (not product-workflow-validated)

## Scope delivered

| Item | Notes |
| --- | --- |
| Discovery preference | If runnable code lenses are present, skip `cargo test --list` |
| Controller label | `lsp-runnable` when lenses drive the list; else `cargo-test` |
| Per-item run | Lens items launch policy-gated terminal via command label |
| Cargo fallback | Unchanged when no runnable lenses are projected |

## Explicit non-claims

- Does **not** claim full rust-analyzer code-lens product UX.
- Terminal launch for LSP runnables is fire-and-forget (`last_run_status=launched`); exit codes come from cargo exact runs only.
- **PR-LANG-002** remains **Substrate validated**.

## Verification

```text
cargo test -p legion-app --test test_explorer_workflow
cargo test -p legion-app --lib test_explorer
```
