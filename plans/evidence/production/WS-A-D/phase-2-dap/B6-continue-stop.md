# Phase 2 B6 — Continue-until-stop + disconnect

**Date:** 2026-07-22  
**Status:** Landed

## Delivered

| Item | Location |
| --- | --- |
| `continue_until_stopped` | `LiveDapSession` — continue then wait for next `stopped` |
| Fake adapter re-stop | After `continue`, emit `continued` then `stopped` (breakpoint) |
| Product Continue | Live path uses continue-until-stop; re-projects stack/vars |
| Stop / disconnect | `StopDebugSession` intent + `:debug-stop` / `:debug-disconnect` / `:debug-quit` |
| Tear-down | Disconnects live child; clears session; status Exited |

## Commands

```text
:debug-step continue   → live continue until next stop (or fixture continue)
:debug-stop            → disconnect live adapter / stop fixture session
:debug-disconnect      → same
:debug-quit            → same
```

## Tests

- `live_dap_handshake`: continue_until_stopped → reason=breakpoint
- `debug_workflow`: live fake continue re-pauses; stop clears live session
- `debug_projection`: parses `:debug-stop`

## Residual

- Background poll while Running (UI idle until next command still blocks on continue)
- System lldb-dap dogfood
