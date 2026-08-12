# Phase 2 B2 — Breakpoints, stack, step (fake adapter)

**Date:** 2026-07-21

## Delivered

| Item | Location |
| --- | --- |
| Fake adapter: setBreakpoints, launch, configurationDone, stopped, stackTrace, scopes, variables, next, continue, pause | `crates/legion-debug/src/bin/fake_dap_adapter.rs` |
| Live session helpers | `set_breakpoints`, `launch_until_stopped`, `step_over_until_stopped`, `continue_execution` |
| Contract test | `live_dap_breakpoints_launch_stack_step_against_fake_adapter` |

## Explicitly not in B2

- App/desktop live wiring (still fixture UI)
- Real CodeLLDB (B3)
- Trust/capability broker (B3)

## Verification

```text
cargo test -p legion-debug --all-targets
cargo clippy -p legion-debug --all-targets -- -D warnings
```
