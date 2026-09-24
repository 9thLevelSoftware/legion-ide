# Multi-agent and interoperability scope audit

Baseline requirements: `327bf69` (334 rows). Source basis: approved product completion design, approved AI-team Stage 4 plan, and current source paths.

## Atomic coverage

| Row | Promise | Package | Implementation | Dependencies |
| --- | --- | --- | --- | --- |
| `COMP-SCOPE-FAMILY-14-01` | Editable multi-agent plans define ordered workers, dependencies, scopes, permissions, budgets, approval policy, and resumable state. | `S4-02` | `partial` | `COMP-SCOPE-FAMILY-13-01`, `COMP-SCOPE-FAMILY-13-08` |
| `COMP-SCOPE-FAMILY-14-02` | Dependency scheduling executes ready workers in order, runs independent lane mates concurrently, and blocks dependents after failed or conflicted prerequisites. | `S4-02` | `partial` | `COMP-SCOPE-FAMILY-14-01` |
| `COMP-SCOPE-FAMILY-14-03` | Each worker runs in an isolated disposable worktree or copy with enforced scope, tool permissions, and process cleanup. | `S4-02` | `partial` | `COMP-SCOPE-FAMILY-14-02`, `COMP-P5-F2-T2-1`, `COMP-P5-F2-T4-1` |
| `COMP-SCOPE-FAMILY-14-04` | The fleet enforces aggregate and per-worker time, token, tool, output, and process budgets with truthful exhaustion outcomes. | `S4-02` | `partial` | `COMP-SCOPE-FAMILY-14-02`, `COMP-PROV-006` |
| `COMP-SCOPE-FAMILY-14-05` | Overlapping or conflicting worker edits are detected, pause affected dependents, and produce a reviewable conflict-resolution proposal. | `S4-02` | `partial` | `COMP-SCOPE-FAMILY-14-02`, `COMP-P3-F2-T1-1` |
| `COMP-SCOPE-FAMILY-14-06` | Worker outputs and multi-file changes remain proposal-mediated; final merge readiness includes complete evidence and per-hunk approval before mutation. | `S4-02` | `partial` | `COMP-SCOPE-FAMILY-14-05`, `COMP-P3-F2-T1-1`, `COMP-P3-F2-T2-1`, `COMP-P3-F2-T3-1`, `COMP-P3-F2-T4-1` |
| `COMP-SCOPE-FAMILY-14-07` | Fleet controls expose permission grants and revocation, graceful stop, hard kill, cancellation, and denial outcomes without further unauthorized calls. | `S4-03` | `partial` | `COMP-SCOPE-FAMILY-14-02`, `COMP-SCOPE-FAMILY-14-03` |
| `COMP-SCOPE-FAMILY-14-08` | Interrupted work persists checkpoints and resumes deterministically after cancellation, crash, or restart without replaying completed workers or duplicating effects. | `S4-02` | `partial` | `COMP-SCOPE-FAMILY-14-02`, `COMP-SCOPE-FAMILY-14-06`, `COMP-P3-F3-T1-1`, `COMP-P3-F3-T2-1`, `COMP-P3-F3-T3-1`, `COMP-P3-F3-T4-1` |
| `COMP-SCOPE-FAMILY-14-09` | The command center truthfully projects plan, dependency, worker, scope, budget, evidence, permission, conflict, and recovery state for inspection. | `S4-03` | `partial` | `COMP-SCOPE-FAMILY-14-01`, `COMP-SCOPE-FAMILY-14-05`, `COMP-SCOPE-FAMILY-14-08` |
| `COMP-SCOPE-FAMILY-14-10` | Legion acts as a production MCP client with named-server handshake, capability discovery, bounded per-tool invocation, permission and revocation, cancellation, audit, reconnect, and real resources/prompts/tools. | `S4-04` | `partial` | `COMP-SCOPE-FAMILY-14-01`, `COMP-SCOPE-FAMILY-14-02` |
| `COMP-SCOPE-FAMILY-14-11` | Legion interoperates with a named external ACP agent through negotiated identity and capabilities, scoped task/context exchange, streaming progress, proposal/evidence return, cancellation, restart, and equivalent containment. | `S4-05` | `partial` | `COMP-SCOPE-FAMILY-14-01`, `COMP-SCOPE-FAMILY-14-02`, `COMP-SCOPE-FAMILY-14-03` |
| `COMP-SCOPE-FAMILY-14-12` | Packaged native qualification demonstrates a real multi-worker cross-file task, conflict and approval handling, interruption/recovery, named MCP and ACP peers, external verification, and zero process or sandbox leaks. | `S4-06` | `absent` | `COMP-SCOPE-FAMILY-14-07`, `COMP-SCOPE-FAMILY-14-10`, `COMP-SCOPE-FAMILY-14-11` |

All twelve family-14 promises are represented exactly once. Shared provider/model setup, context/index/memory, Delegate execution, proposal lifecycle, checkpoint, terminal, and build/test outcomes remain canonical owners and are referenced as dependencies only. No Assist, Delegate, or shared PROV/CTX row is duplicated. Editor routing remains in the existing editor requirements.

Source paths in the replacement rows are literal repository paths; no source reference uses a fragment. Family-14-10 has bounded `McpClient<T>` substrate in `crates/legion-ai-providers/src/lib.rs` (registry validation, list/reload, resource/prompt/tool request construction, and permission-gated tool calls), so it is `partial`; full named-peer supervision/reconnect/qualification remains unproven. Stage 4 acceptance paths remain planned/absent implementation evidence. Acceptance is unassessed for every replacement row.
