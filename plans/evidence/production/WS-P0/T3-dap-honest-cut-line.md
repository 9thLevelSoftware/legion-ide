# T3 slice — DAP honest cut line (simulated fixture)

**Date:** 2026-07-21

## Decision

Ship an **explicit simulated-DAP cut line** rather than a real adapter process in this PR. Real CodeLLDB / wire-protocol DAP remains backlog.

## Changes

| Surface | Honesty update |
| --- | --- |
| `legion-debug` module docs | States no adapter process / no DAP wire protocol; fixture projections only |
| Breakpoint verify message | `simulated verified (no DAP adapter process)` |
| Launch / step console | Prefixed `SIMULATED DAP:` |
| App `DebugWorkflow` status | Enable / launch / step messages say simulated fixture |
| Desktop `debug_rows` | Leads with `DEBUG_SIMULATED_BANNER` ("Debugger is simulated in this build") + status message |
| `enable_debug_fixture_for_tests` | Already uses `DEBUG_FIXTURE_ENABLED` (prior T0/T1) |
| `docs/USER_GUIDE.md` | Already documents DAP as fixture/projection (product areas callout) |

## Explicitly still open

- Real DAP client: adapter launch, initialize handshake, setBreakpoints, threads/stackTrace/scopes/variables over JSON-RPC
- Stop / terminate session end
- Non-fixture variables and source mapping

## Verification

```text
cargo test -p legion-debug
cargo test -p legion-app --test debug_workflow
cargo test -p legion-desktop --test debug_workflow
cargo test -p legion-desktop --test breakpoint_hit
```
