# Phase 2 B12 — Live DAP cargo prebuild

**Date:** 2026-07-23  
**Status:** Ready to land

## Problem

Live launch against a **system** adapter needs a real program binary. Configs
from `legion-project` already carry `cargo_args` (`build --package … --bin …`),
but `launch_live` never ran them (commented residual). Fake adapter CI does not
need a real binary.

## Delivered

| Item | Location |
| --- | --- |
| Prebuild gate | `live_dap_should_prebuild` — non-fake + non-empty `cargo_args` |
| Prebuild runner | `run_live_dap_prebuild` — `cargo` in config cwd, 180s timeout, null stdio |
| Console | `LIVE DAP prebuild: cargo … ok` before initialize row |
| Tests | `tests/live_dap_prebuild.rs` — predicate + real cargo smoke |

## Explicitly out of scope

- Full system lldb-dap launch/step dogfood against host debugee (still residual;
  prebuild is the prerequisite product step)
- Human windowed GUI journal

## Verification

```text
cargo test -p legion-app --test live_dap_prebuild
cargo test -p legion-app --test debug_workflow
```
