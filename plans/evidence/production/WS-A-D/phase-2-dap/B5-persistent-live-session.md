# Phase 2 B5 — Persistent live DAP session

**Date:** 2026-07-22  
**Status:** Landed

## Problem

B0–B4 live launch disconnected the adapter immediately after first stop, so
product step/continue always hit the **fixture** runtime even when
`live_adapter=true` on the projection.

## Delivered

| Item | Location |
| --- | --- |
| Owned live handle | `DebugWorkflow.live: Option<LiveDebugSession>` |
| Keep process after launch | `launch_live` no longer calls `disconnect_and_wait` |
| Live step / continue | `step` routes to `step_live` when session matches |
| Tear-down | `clear_workspace_state` / re-launch disconnect prior session |
| Step APIs on client | `step_command_until_stopped` / stepIn / stepOut on `LiveDapSession` |

## Behaviour

1. Live launch → Microsoft DAP handshake → stop → **session retained**
2. `DebugStep` Over/Into/Out → live `next`/`stepIn`/`stepOut` + re-project stack/vars
3. `DebugStep` Continue → live `continue` → status Running (stack cleared until next stop)
4. Reverse-step (`Back`) → explicit fail (not supported)
5. Workspace switch / new live launch → disconnect previous child

## Tests

- `debug_workflow_live_fake_adapter_sets_live_projection_flag` asserts `persistent=true` and step-over remains live

## Residual

- Product UI may still need a dedicated Stop/Disconnect command (workspace clear tears down)
- Continue until next breakpoint (re-stop after continue) not auto-polled yet
- Dogfood against system `lldb-dap`

## Verification

```text
cargo test -p legion-debug --all-targets
cargo test -p legion-app --test debug_workflow
```
