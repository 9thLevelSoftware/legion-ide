# WS-P0 Rebaseline Evidence

Date: 2026-06-19
Scope: WS-P0 rebaseline, ledgers, plan hygiene, docs-hygiene latest-plan guard, dirty-worktree caveat audit, and dogfood journal template.

## Branch State

- Branch: `codex/legion-production-plan-v0.2`
- Starting dirty files before Packet 4: modified `plans/legion-production-master-plan-v0.1.md`; modified `plans/legion-production-master-plan-v0.2.md`; modified `plans/product-readiness-ledger.md`; modified `xtask/src/docs_hygiene.rs`; modified `xtask/src/perf_harness.rs`; modified `xtask/tests/docs_hygiene.rs`; untracked `docs/superpowers/`; untracked `plans/dogfood/`.
- Baseline before WS-P0 implementation packets: the only dirty item was untracked `docs/superpowers/`, containing the controller-created implementation plan.
- Ending dirty files after Packet 4: the starting dirty files remain, with added untracked `plans/evidence/production/WS-P0/dirty-worktree-caveat-audit.md` and `plans/evidence/production/WS-P0/WS-P0-rebaseline-evidence.md`.

## Completed Tasks

| Task | Evidence |
| --- | --- |
| P0.01 | `plans/legion-production-master-plan-v0.2.md` exists and is identified as the current production master plan. |
| P0.02 | `README.md` and `docs/INDEX.md` point production readers at `plans/legion-production-master-plan-v0.2.md`. |
| P0.03 | `plans/legion-production-master-plan-v0.1.md` has a historical-status banner pointing readers to v0.2 and `plans/product-readiness-ledger.md`. |
| P0.04 | `plans/product-readiness-ledger.md` reconciles M0-M6 production evidence without inflating product-ready statuses. |
| P0.05 | `plans/product-readiness-ledger.md` includes the `Production Evidence Reconciliation` table mapping M0-M6 evidence to product-readiness gates and remaining gaps. |
| P0.06 | `plans/product-readiness-ledger.md` includes the standing rule that milestone evidence can be accepted while product-readiness remains open. |
| P0.07 | `xtask/src/docs_hygiene.rs` and `xtask/tests/docs_hygiene.rs` implement and test the latest production master-plan entrypoint guard. |
| P0.08 | `plans/legion-production-master-plan-v0.2.md` includes `Appendix D - What Changed Since v0.1`. |
| P0.09 | `plans/evidence/production/WS-P0/dirty-worktree-caveat-audit.md` records dirty-worktree caveats and clean-rerun decisions. |
| P0.10 | `plans/dogfood/legion-on-legion-weekly-journal-template.md` exists as the weekly Legion-on-Legion dogfood journal template. |

## Verification

| Command | Result | Notes |
| --- | --- | --- |
| `cargo test -p xtask --test docs_hygiene` | Pass: 12 passed, 0 failed, 0 ignored. | Targeted docs-hygiene regression suite passed. |
| `cargo run -p xtask -- docs-hygiene` | Pass: `documentation hygiene checks passed`. | Documentation hygiene gate accepted the current tree. |
| `cargo run -p xtask -- check-deps` | Pass: `dependency policy checks passed`. | Dependency policy gate accepted the current tree. |
| `cargo fmt --all --check` | Pass. | Formatting check completed without changes required. |
| `cargo check --workspace --all-targets` | Pass. | Finished successfully after checking workspace crates. |
| `git diff --check` | Pass: exit 0. | Emitted Git LF-to-CRLF normalization warnings for touched files only. |

## Residual Risk

- M3 remains task-level evidence unless a separate audit adds formal milestone acceptance.
- Historical dirty-tree caveats remain historical caveats; WS-P0 does not rewrite them.
- Product-readiness rows remain bounded by their current row evidence.
- `git diff --check` emitted LF-to-CRLF normalization warnings but exited 0.
