# PKT-START: Delegate Start Wiring — Evidence

**Packet:** PKT-START (M10, delegate start)
**Branch:** `m10/delegate-start`
**Base:** `8f7b5b1` (PKT-WORKTREE)
**Date:** 2026-07-07

## Deliverables

| # | Deliverable | File(s) | Status |
|---|-------------|---------|--------|
| D1 | Wire `render_scope_picker` into Delegate dock | `crates/legion-desktop/src/view.rs` | Done |
| D2 | `StartDelegatedTask` command variant, bridge dispatch, workflow handler | `crates/legion-ui/src/ui.rs`, `crates/legion-desktop/src/bridge.rs`, `crates/legion-desktop/src/workflow.rs`, `crates/legion-app/Cargo.toml` | Done |
| D3 | `AppDelegatedToolHost` struct implementing `DelegatedToolHost` | `crates/legion-app/src/lib.rs` | Done |
| D4 | `AppComposition::start_delegated_task` + dispatch + integration tests | `crates/legion-app/src/lib.rs`, `crates/legion-app/tests/delegated_task_integration.rs` | Partial — proposal extraction deferred (loop API doesn't yet surface proposals in DelegatedTaskLoopResult::Completed) |
| D5 | Evidence file | `plans/evidence/production/M10/PKT-START-evidence.md` | Done |

## Commits

| # | Hash | Message |
|---|------|---------|
| 1 | `086d27e` | `feat: wire scope picker into Delegate dock` |
| 2 | `a907b4f` | `feat: add StartDelegatedTask command variant, bridge dispatch, and workflow handler (PKT-START D2)` |
| 3 | `c4ae7b3` | `feat: implement AppDelegatedToolHost and start_delegated_task production dispatch (PKT-START D3+D4)` |
| 4 | (this commit) | `docs: PKT-START evidence` |

## Command Routing

`StartDelegatedTask` flows through four layers:

1. **UI intent** — `CommandDispatchIntent::StartDelegatedTask { task_description, scope }` in `legion-ui/src/ui.rs`
2. **Bridge** — `DesktopAction::StartDelegatedTask` normalises the label then emits the Intent; returns `InvalidInstructionLabel` if blank
3. **App command** — `AppCommandRequest::StartDelegatedTask` dispatched from `legion-app/src/lib.rs`; under `#[cfg(not(feature = "ai"))]` returns `Blocked`
4. **Workflow handler** — `AppCommandOutcome::DelegatedTaskCompleted` mapped to `DesktopWorkflowOutcome::DelegatedTaskReviewed` with all five outcome arms handled

## AppDelegatedToolHost

Module-private struct under `#[cfg(feature = "ai")]` implementing `DelegatedToolHost`:

| Method | Behaviour |
|--------|-----------|
| `run_terminal_command` | Builds `SandboxSpawnSpec` and calls `spawn_sandboxed`; fail-closed (never spawns unsandboxed) |
| `call_mcp_tool` | Returns `Err` — MCP integration deferred to a later packet |

Fields: `worktree_root: PathBuf`, `allowed_egress: BTreeSet<String>` (empty by default → no network).

## start_delegated_task Method

Runs the native `run_delegated_task_loop` with:

| Component | Implementation |
|-----------|---------------|
| Sandbox allocation | `DelegatedTaskSandboxOrchestrator::with_workspace_root` |
| Tool host | `AppDelegatedToolHost { worktree_root: sandbox_path }` |
| Audit sink | `VecAuditSink` (collects `DelegatedTaskLoopStepRecord` for outcome) |
| Cancellation probe | `NeverCancelled` (caller can add a probe in a later packet) |
| Capability broker | `AllowAllCapabilityBroker` (scope enforcement is the primary gate) |

`AppDelegatedTaskOutcome` maps all five `DelegatedTaskLoopResult` arms:
`Completed`, `BudgetExhausted`, `Blocked`, `Cancelled`, `SandboxAllocationFailed`.

## Integration Test Coverage

| Test | Scenario | Result |
|------|----------|--------|
| `start_delegated_task_completes_with_scripted_end_turn` | Scripted EndTurn → `Completed`, 1+ audit steps (at least one model-turn record) | Pass |
| `start_delegated_task_audit_steps_are_paired_for_tool_call` | ReadFile call → EndTurn → `Completed`, 2 audit steps (request + result) | Pass |
| `start_delegated_task_rejects_manual_mode` | `AppMode::Manual` → `AppCompositionError` (require_delegate_mode gate) | Pass |

## Constraint Compliance

| Constraint | How satisfied |
|------------|--------------|
| No disk writes from `EditAsProposal` | `execute_edit_as_proposal` is proposal-only; confirmed by PKT-LOOP |
| Every tool call has a paired audit row | Enforced by `run_delegated_task_loop` audit protocol |
| All retry loops bounded | `max_consecutive_retries` budget enforced by loop |
| `spawn_sandboxed` fail-closed | `SandboxError::PlatformUnavailable` surfaces as `SandboxAllocationFailed` |
| `AllowAllCapabilityBroker` rationale | Scope is enforced by `validate_delegated_task_tool_call`; broker grants all capabilities after scope passes |
| Offline build (`#[cfg(not(feature = "ai"))]`) | Returns `Blocked { reason: "AI feature not enabled..." }` — no panic, no stub |

## Verification

| Gate | Result |
|------|--------|
| `cargo fmt --check` | Clean |
| `cargo clippy -p legion-app -p legion-desktop --all-targets -- -D warnings` | 0 warnings |
| `cargo test -p legion-app --all-targets -j 4` | All pass |
| `cargo test -p legion-app --test manual_zero_egress` | Pass (1/1) |
| `start_delegated_task_*` integration tests (3) | All pass |
