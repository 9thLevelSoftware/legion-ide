# PKT-LOOP: Native Delegated Task Execution Loop — Evidence

**Packet:** PKT-LOOP (M10, agent loop)
**Branch:** `m10/agent-loop`
**Base:** `0030b2b` (main)
**Date:** 2026-07-06

## Deliverables

| # | Deliverable | File(s) | Status |
|---|-------------|---------|--------|
| D1 | Extract state.rs and worktree.rs | `crates/legion-agent/src/state.rs`, `worktree.rs`, `lib.rs` | Done |
| D2 | DelegatedTaskLoopBudget + step record DTOs | `crates/legion-protocol/src/delegate_loop.rs` | Done |
| D3 | LegionToolCallInvocation + LegionToolCallResult DTOs | `crates/legion-protocol/src/tools.rs` | Done |
| D4 | Native delegated task loop with 7 tool executors | `crates/legion-agent/src/agent_loop.rs` | Done |
| D5 | Integration tests with audit pairing | `crates/legion-agent/tests/agent_loop_integration.rs` | Done |
| D6 | Evidence file | `plans/evidence/production/M10/PKT-LOOP-evidence.md` | Done |

## Commits

| # | Hash | Message |
|---|------|---------|
| 1 | `c54cad5` | `refactor: extract state.rs and worktree.rs from legion-agent lib.rs` |
| 2 | `fd07544` | `feat: add DelegatedTaskLoopBudget and step record DTOs` |
| 3 | `6617b79` | `feat: add LegionToolCallInvocation and LegionToolCallResult DTOs` |
| 4 | `8668d68` | `feat: implement native delegated task loop with tool executors` |
| 5 | `7c89adb` | `test: agent loop integration tests with audit pairing` |
| 6 | (this commit) | `docs: PKT-LOOP evidence file` |

## Audit Pairing Contract

Every tool call in `run_delegated_task_loop` follows a strict two-step audit protocol:

1. **ToolCallRequest** step emitted **before** dispatch, carrying the tool name and a freshly-generated `causality_id`.
2. **ToolCallResult** or **ToolCallRejected** step emitted **after** dispatch, carrying the **same** `causality_id`.

This pairing is load-bearing and is enforced by the integration tests. No code path can execute a tool without the request row.

Additional invariants:
- `correlation_id` is constant across the entire run (one UUID per `run_delegated_task_loop` call).
- `event_sequence` is strictly monotonically increasing across all steps in a run.
- `step_index` is monotonically increasing and never wraps within a run.

## Validation Pipeline

For each tool call, the loop enforces a five-step pipeline before dispatch:

1. **Parse tool name** → `LegionToolKind` (unknown → `UnknownTool`, non-retryable)
2. **Schema validation** → check all required fields present (missing fields → `InvalidArguments`, retryable)
3. **Containment check** → `validate_containment(worktree_root, resolved_path)` (escape → `ScopeDenied`, non-retryable)
4. **Scope validation** → `validate_delegated_task_tool_call(scope, tool, workspace_path)` (violation → `ScopeDenied`, non-retryable)
5. **Broker check** → `CapabilityBrokerPort::handle(CapabilityRequest::Request { capability_id: "delegate.tool.{name}" })` (denial → `PolicyDenied`, non-retryable)

Only `InvalidArguments` rejections are retryable; all others terminate the loop with `Blocked`.

## Budget Enforcement

| Budget cap | Field | Default |
|------------|-------|---------|
| Model turns | `max_model_turns` | 50 |
| Total tool calls | `max_tool_calls` | 200 |
| Consecutive retries | `max_consecutive_retries` | 3 |
| Per-call output bytes | `max_tool_output_bytes` | 100 000 |
| Total output bytes | `max_total_tool_output_bytes` | 5 000 000 |
| Wall-clock limit | `wall_clock_limit_ms` | 0 (no limit) |

The loop terminates with `BudgetExhausted` when any cap is exceeded.

## Tool Executors

| Tool | Executor | Disk write? |
|------|----------|-------------|
| Read | `execute_read` — `fs::read_to_string` with optional line-slice and byte cap | No |
| Grep | `execute_grep` — `regex::Regex` line-matcher, recursive walker | No |
| Glob | `execute_glob` — `globset::GlobSet` matcher, recursive walker | No |
| Outline | `execute_outline` — lexical declaration scanner (.rs) / heading extractor (.md) | No |
| EditAsProposal | `execute_edit_as_proposal` — `DelegatedTaskProposalGenerator`, **zero disk writes** | No |
| TerminalCommand | `execute_terminal_command` — delegated to `DelegatedToolHost::run_terminal_command` | No |
| McpPassthrough | `execute_mcp_passthrough` — delegated to `DelegatedToolHost::call_mcp_tool` | No |

All executors apply `redact_model_bound_output` before returning content to the model.

## Integration Test Coverage

| Test | Scenario | Result |
|------|----------|--------|
| `basic_tool_use_loop_completes` | Real file read → EndTurn → Completed + audit pairing | Pass |
| `scope_denial_blocks_the_loop` | Outside-worktree path → Blocked + ToolCallRejected | Pass |
| `budget_exhaustion_terminates_loop` | max_tool_calls=2, 3 calls scripted → BudgetExhausted | Pass |
| `cancellation_stops_the_loop` | Probe fires before turn 1 → Cancelled | Pass |
| `audit_pairing_is_maintained_across_multi_turn_loop` | 2 reads + EndTurn → all causality_ids paired | Pass |
| `retry_budget_exhausted_by_invalid_arguments` | max_consecutive_retries=2, 3 bad reads → BudgetExhausted | Pass |

## Verification

| Gate | Result |
|------|--------|
| `cargo fmt --check` | Clean |
| `cargo clippy --all-targets -- -D warnings` | 0 warnings |
| `cargo test -p legion-agent -j 4` | All pass |
| `cargo test -p legion-protocol -j 4` | All pass |
| Agent loop integration tests (6) | All pass |
| `manual_zero_egress` | Pass (1/1) |
