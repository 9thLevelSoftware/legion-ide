# Phase 2 B9 — System adapter dogfood (lldb-dap / CodeLLDB)

**Date:** 2026-07-22  
**Status:** Ready to land

## Problem

B4–B8 prove Microsoft DAP wire + product continue/poll against the **in-tree
fake adapter**. Hosts with real `lldb-dap` / `codelldb` still lacked a
fail-closed optional gate and a documented dogfood path that never falls back
to fake.

## Delivered

| Item | Location |
| --- | --- |
| `resolve_system_adapter` | Explicit + PATH only; **never** fake |
| PATH preference | Preferred type first (no alphabetical demotion of `lldb-dap`) |
| Optional integration test | `system_adapter_dogfood` — initialize + disconnect |
| `LEGION_DAP_DOGFOOD=1` | Require system adapter (fail if missing) |
| Runbook | This file + USER_GUIDE product-area note |

## How to dogfood (local)

```text
# Install LLVM lldb-dap or CodeLLDB so a binary is on PATH, or:
set LEGION_DAP_ADAPTER=C:\path\to\lldb-dap.exe   # Windows
export LEGION_DAP_ADAPTER=/usr/bin/lldb-dap      # Unix

# Optional: fail if adapter missing
set LEGION_DAP_DOGFOOD=1

cargo test -p legion-debug --test system_adapter_dogfood -- --nocapture
```

Product live path (desktop / app) still uses `resolve_live_adapter` (system
first, then `LEGION_DAP_USE_FAKE`).

## Explicitly out of scope (residual)

- Full launch/step/continue against a host debugee binary (needs program path +
  target-specific launch args; interactive GUI dogfood)
- Windows-only CodeLLDB packaging / install UX
- Sandbox wrap of adapter spawn

## Verification

```text
cargo test -p legion-debug --all-targets
# without system adapter: dogfood test skips (pass)
# with LEGION_DAP_DOGFOOD=1 and no adapter: dogfood test fails
```
