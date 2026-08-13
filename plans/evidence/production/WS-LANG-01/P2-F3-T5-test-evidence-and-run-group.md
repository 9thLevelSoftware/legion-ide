# P2.F3.T5 — Test explorer → agent evidence + run-group

**Date:** 2026-07-23  
**Status:** Substrate thin slice (not product-workflow-validated)

## Scope delivered

| Item | Notes |
| --- | --- |
| `TestRunSummary` | Each cargo explorer run maps to protocol summary (counts only) |
| Agent evidence | `test_explorer_evidence_artifacts()` → `legion_debug::test_run_summary_evidence` (metadata-only) |
| Verification link | Rows carry `evidence_artifact_id` pointing at the artifact id |
| Run group | `:test-run-group <parent>` / desktop **Run first group** uses cargo substring filter |
| Cap | 20 recent summaries / verification rows |

## Explicit non-claims

- Does **not** attach raw cargo stdout/stderr to agents.
- Does **not** auto-record into a live Legion workflow worker without a worker id; exposes artifacts for consumers.
- **PR-LANG-002** remains **Substrate validated**.

## Verification

```text
cargo test -p legion-app --test test_explorer_workflow
cargo test -p legion-app --lib test_explorer
cargo test -p legion-debug --lib evidence
cargo test -p legion-agent evidence
```
