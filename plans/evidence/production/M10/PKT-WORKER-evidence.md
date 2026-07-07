# PKT-WORKER Evidence

**Branch:** `m10/worker-panel`
**Status:** DONE

## D1 — Worker panel module wired into Delegate dock

`pub mod worker_panel;` declared in `crates/legion-desktop/src/view.rs` after `pub mod scope_picker;`.

`render_delegation_console` gains a "Worker" section below the existing controls:

```rust
section_label(ui, "Worker", Some(theme::tokens().accent.violet));
{
    let panel_vm = worker_panel::DesktopWorkerPanelViewModel::from_snapshot(snapshot);
    let evidence = proposal_review::DesktopProposalEvidencePanelViewModel::default();
    worker_panel::render_worker_panel(ui, &panel_vm, &evidence, actions);
}
```

`DesktopProposalEvidencePanelViewModel` gained `#[derive(Default)]` so it can be used as a default here (all fields are `Option` or `Vec`).

## D2 — SharedCancellationFlag and cancel_delegated_task

`SharedCancellationFlag` added near the delegated-task machinery in `crates/legion-app/src/lib.rs`:

- Wraps `Arc<AtomicBool>` with `Ordering::Release` writes and `Ordering::Acquire` reads.
- `cancel(&self)` stores `true`; `is_cancelled(&self)` loads.
- `Default` impl calls `Self::new()`.
- Under `#[cfg(feature = "ai")]` implements `DelegatedTaskCancellationProbe` from `legion-agent`.

`AppComposition` gains field `active_cancellation_flag: Option<SharedCancellationFlag>` (init `None`).

`cancel_delegated_task(&self)` method: returns `Err("no delegated task running")` if no flag is set; calls `flag.cancel()` and returns `Ok(())` otherwise.

`inject_cancellation_flag_for_test` helper added under `#[cfg(any(test, feature = "test-helpers"))]`.

## D3 — Executing and Cancelled runtime activation states

In `start_delegated_task`:

1. A fresh `SharedCancellationFlag` is created and stored into `self.active_cancellation_flag` before the agent loop runs.
2. `self.delegate_workflow.set_runtime_activation(DelegatedTaskRuntimeActivationState::Executing)` is called before entering the loop.
3. The `loop_result` match arm for `DelegatedTaskLoopResult::Cancelled` sets `DelegatedTaskRuntimeActivationState::Cancelled` and returns `AppDelegatedTaskOutcome::Cancelled`.
4. After the loop (regardless of outcome) `self.active_cancellation_flag` is reset to `None`.

`NeverCancelled` stub removed; replaced by the live `SharedCancellationFlag`.

Test added: `pre_cancelled_flag_causes_loop_to_exit_with_cancelled_state` — opens a temp workspace, injects a pre-cancelled flag, runs `start_delegated_task` with a scripted provider, asserts `AppDelegatedTaskOutcome::Cancelled`.

## D4 — Kill button and CancelDelegatedTask command pipeline

Kill button added in `render_delegation_console` (view.rs) inside the small-card frame after the Chat button:

```rust
if primary_button(ui, "Kill", theme::tokens().accent.red).clicked() {
    actions.push(DesktopAction::CancelDelegatedTask);
}
```

Full command pipeline:

| Layer | Change |
|---|---|
| `legion-ui` `CommandDispatchIntent` | `CancelDelegatedTask` variant added |
| `legion-desktop` `DesktopAction` | `CancelDelegatedTask` variant added |
| `legion-desktop` `translate()` | maps `CancelDelegatedTask` → `Intent(CommandDispatchIntent::CancelDelegatedTask)` |
| `legion-app` `AppCommandRequest` | `CancelDelegatedTask` variant added |
| `legion-app` `CommandExecutionService::execute` pass-through | `\| AppCommandRequest::CancelDelegatedTask` added to `Ok(None)` arm |
| `legion-app` `CommandDispatcher::route_intent` | `CancelDelegatedTask => Ok(AppCommandRequest::CancelDelegatedTask)` |
| `legion-app` `dispatch_ui_intent` | handler calls `self.cancel_delegated_task()?; Ok(AppCommandOutcome::Noop)` |

## Test results

```
cargo fmt --check               → PASS (clean)
cargo clippy                    → PASS (no errors)
cargo test --all-targets -j 4   → PASS (all suites green, 1 pre-existing ignored test)
```

## Concerns

1. **Kill button gated on Executing state** — the Kill button in `render_delegation_console` is now
   conditional on `snapshot.delegated_task_projection.runtime_activation ==
   DelegatedTaskRuntimeActivationState::Executing`.  Clicking it when no task is running (idle,
   cancelled, failed, etc.) no longer produces a red error notification.

2. **Kill switch requires background-thread dispatch** — `start_delegated_task` takes `&mut self`
   and blocks the calling thread for the duration of the agent loop.  The command processor cannot
   reach `cancel_delegated_task` until the loop yields (i.e., moves to a background thread).  The
   cancellation flag mechanism is correctly wired and the integration test validates it via a
   pre-cancelled flag, but live UI-driven cancellation requires dispatching `start_delegated_task`
   onto a background thread so the UI thread remains free.  This is tracked for a future packet.
