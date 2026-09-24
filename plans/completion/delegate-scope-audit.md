# Delegate scope audit

Baseline: `6390afd` / requirements baseline supplied for S0-01p. Source
reviewed: the approved product-completion design, the approved AI-team
completion plan, and current Delegate source files.

## Atomic coverage

| Row | Promise | Current classification | Owner/dependency boundary |
| --- | --- | --- | --- |
| 13-01 | User task, scope, ordered plan, route, budget, approval policy | partial | Provider/context setup are `COMP-PROV-001`, `COMP-PROV-004`, `COMP-CTX-001` |
| 13-02 | Disposable worktree/copy and enforced sandbox scope | partial | Existing sandbox guarantees are `COMP-P5-F2-T2-1`, `COMP-P5-F2-T4-1` |
| 13-03 | Real bounded read/edit/project-command tool loop | partial | Context budget is `COMP-CTX-006`; worker loop is Delegate-specific |
| 13-04 | Time/token/tool/output/process budgets | partial | Usage ceilings remain `COMP-PROV-006` |
| 13-05 | Real verification and output fingerprint before review | partial | Build/test and terminal execution remain `COMP-BTD-009`, `COMP-TERM-013` |
| 13-06 | Complete proposal and per-hunk review; approval-gated apply | partial | Proposal review remains canonical in `COMP-P3-F2-T1-1..4-1` |
| 13-07 | Graceful cancel, hard kill, no further calls, child reaping | partial | Delegate lifecycle, with tool-loop dependency |
| 13-08 | Durable checkpoints and restart resume without replay | absent | Checkpoint primitives remain `COMP-P3-F3-T1-1..4-1` |
| 13-09 | Cleanup after success/failure/cancel/crash/restart | partial | Uses isolated execution and cancellation rows |
| 13-10 | Truthful task/sandbox/evidence/review controls | partial | Delegate projection owns the command-center surface |
| 13-11 | Packaged native local/hosted three-language acceptance | absent | Native provider/context gates are `COMP-PROV-011`, `COMP-CTX-010` |

All eleven rows retain `acceptance = unassessed`. No source path uses a
fragment; section anchors are carried in `identity` where needed.

## Exact retained mappings

| Existing owner | Delegate overlap | Treatment |
| --- | --- | --- |
| `COMP-PROV-001`, `COMP-PROV-004`, `COMP-PROV-006`, `COMP-PROV-011` | route, capabilities, budgets, native provider matrix | dependencies only; no duplicate provider rows |
| `COMP-CTX-001`, `COMP-CTX-006`, `COMP-CTX-010` | context manifest, budget binding, native context matrix | dependencies only; no duplicate context rows |
| `COMP-P3-F2-T1-1..4-1` | proposal completeness, hunk review, evidence, keyboard reachability | canonical proposal owners; 13-06 adds Delegate production flow |
| `COMP-P3-F3-T1-1..4-1` | checkpoint creation, restore, preservation, auditability | canonical checkpoint owners; 13-08 adds durable Delegate resume |
| `COMP-P5-F2-T2-1`, `COMP-P5-F2-T4-1` | disposable scope and sandbox caveats | dependencies only; 13-02 remains Delegate orchestration |
| `COMP-P5-F3-T3-1`, `COMP-TERM-013`, `COMP-BTD-009` | tool calls, command execution, verification evidence | dependencies only; no duplicated terminal/build rows |

No Delegate promise from S3-06 outputs is dropped. Multi-agent coordination
belongs to the separate S4 family and is not pulled into this S3 Delegate
inventory.
