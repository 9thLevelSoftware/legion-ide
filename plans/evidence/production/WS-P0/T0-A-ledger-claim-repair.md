# T0-A — Product-readiness ledger and public-doc claim repair

**Date:** 2026-07-21  
**Packet:** Tier 0 truth repair — ledger / USER_GUIDE / INDEX  
**Authority:** Implementation plan Tier 0; gap review 2026-07-20; audit `audit-reports/2026-07-13-release-readiness-codebase-map-and-gaps.md`

## Intent

Make the product-readiness ledger and user-facing docs match substrate-vs-product reality so later wiring work cannot hide behind overclaims.

## Changes

| Target | Before | After |
| --- | --- | --- |
| **PR-VSC-001 acceptance criteria** | Listed activation-event routing, enable/disable/update metadata, API coverage reporting as implemented | Criteria limited to metadata classification + diagnostic reports; explicit “not implemented / deferred” for routing and product enable/update workflows |
| **PR-VSC-001 remaining gaps** | Runtime execution deferred only | + Unwired from product binaries; classify-only activation/contribution semantics |
| **PR-AI-001 acceptance criteria** | Implied “local providers are real first-class paths” as full product path | Scoped to inspectable local-first control surfaces (manifests, privacy, consent) |
| **PR-AI-001 remaining gap** | None explicit for deterministic GUI default | Explicit: default Assist/inline/chat still `deterministic-local`; real model-by-default is Tier 2 |
| **PR-AI-001 status** | Product workflow validated | **Unchanged** (inspectability claim retained; gap prose added) |
| **PR-ENT-001 evidence** | Mock/default-deny + “production remote transport not yet implemented” | + Cloud Lane / phase evidence is substrate/harness only; fixture “connected” ≠ product UX |
| **PR-UI-001 evidence** | `cargo test --workspace` “(909 pass)” frozen count | Points at WS-P0 closure for whole-workspace totals; keeps targeted tests; notes editor Backspace/Delete/Enter gap |
| **docs/USER_GUIDE.md deferred banner** | “no live WASM host” | Product composition does not run plugin WASM; wasmtime for boundary tests; VSIX runtime deferred |
| **docs/USER_GUIDE.md Assist** | No default-path caveat | Deterministic fixture default until Tier 2 |
| **docs/INDEX.md** | No v8.0.0 entry | Forward-looking template section for `releases/v8.0.0/` |

## Explicit non-changes

- Did not demote PR-AI-001 or PR-VSC-001 status vocabulary values.
- Did not rewrite historical phase evidence bodies or ENGINEERING_STATUS.
- Did not expand claim-audit forbidden-phrase list.

## Gates

```text
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- claim-audit
```

(Record exit codes when the package is landed.)

## Follow-on

- T0-B: honest desktop status strings for plugin/remote/debug/sandbox/AI fixture
- Tier 2: close PR-AI-001 remaining product gap (real default providers + keyring load)
