# Phase 2 B13 — System adapter launch + step dogfood

**Date:** 2026-07-23  
**Status:** Ready to land

## Problem

B9 only handshakes a system adapter. Product residual was full **launch/step**
against a host debugee. B12 added cargo prebuild for non-fake live product
launch; this packet adds an optional integration dogfood for the wire path.

## Delivered

| Item | Location |
| --- | --- |
| `launch_until_stopped_with` | `cwd` + `stopOnEntry` on Microsoft DAP launch |
| App live launch | Passes config `cwd` + `stop_on_entry` |
| Optional dogfood test | `system_adapter_launch_step_dogfood` |
| Soft-skip default | Missing/broken adapter or launch → skip |
| `LEGION_DAP_DOGFOOD=1` | Fail closed on any step |

## Dogfood run

```text
# working lldb-dap or codelldb on PATH, or LEGION_DAP_ADAPTER=
set LEGION_DAP_DOGFOOD=1
cargo test -p legion-debug --test system_adapter_launch_step_dogfood -- --nocapture
```

## Residual

- Human windowed GUI journal (toolbar exists; needs human session)
- Guaranteeing every CI host has a functional LLDB (explicitly not claimed)

## Verification

```text
cargo test -p legion-debug --all-targets
# without working system adapter: launch-step dogfood skips (pass)
```
