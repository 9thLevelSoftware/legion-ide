# T0-E — Synthetic gate honesty

**Date:** 2026-07-21  
**Packet:** Tier 0 truth repair — bench / perf / release labeling  

## Intent

Green CI must not be readable as “product behavior verified” for synthetic scores or skeleton perf.

## Changes

| Gate | Honesty measure |
| --- | --- |
| **legion-bench** | Schema v2; report field `scoring_mode = synthetic_budget_arithmetic` (recorded) or `scripted_hostile`; task notes include `synthetic=true` and explicit “does not open fixtures / run agents” |
| **legion-bench.yml** | Header + job/step names state synthetic scoring |
| **perf-harness** | Report field `workload_kind = skeleton` (serde default for old reports) |
| **legion-gates.yml** | Step titles “report-only skeleton”; comments on budget=0 |
| **release-pipeline** | Unchanged: already `dry-run/no-production-signer` — intentional honesty |
| **AGENTS.md** | CI paragraph clarifies synthetic bench + smoke promotion criteria |

## Non-goals

- Did not implement real agent bench execution (M13).  
- Did not enable strict hosted perf budgets.  
- Did not produce real installers.

## Gates (when cargo available)

```text
cargo test -p xtask --all-targets
cargo run -p xtask -- perf-harness
cargo run -p xtask -- verify-perf-harness
cargo run -p xtask -- legion-bench --mode recorded
cargo run -p xtask -- verify-legion-bench
```
