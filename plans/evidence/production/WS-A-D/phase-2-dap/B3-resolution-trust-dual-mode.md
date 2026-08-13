# Phase 2 B3 — Adapter resolution, trust, dual-mode honesty

**Date:** 2026-07-21

## Delivered

| Item | Location |
| --- | --- |
| Adapter resolution | `crates/legion-debug/src/adapter_resolve.rs` (`LEGION_DAP_MODE`, `LEGION_DAP_ADAPTER`, `LEGION_DAP_USE_FAKE`) |
| Wire honesty | No PATH auto-discovery of Microsoft DAP adapters; framing is Legion provisional JSON-RPC |
| Trust deny | `DebugWorkflow::launch` — untrusted → `debug.adapter.launch denied` |
| Live one-shot launch | App uses `LiveDapSession` when adapter resolves; else fixture (`auto`) |
| Live fail-closed | `LEGION_DAP_MODE=live` → no fixture fallback on missing adapter or spawn failure |
| Per-source breakpoints | Live launch groups `setBreakpoints` by path |
| Program path | Relative `program_label` joined to configuration `cwd` |
| Projection flag | `DebugProjection.live_adapter` |
| Desktop dual banner | `DEBUG_LIVE_BANNER` vs `DEBUG_SIMULATED_BANNER` |
| Tests | untrusted deny + live fake path in `debug_workflow` app tests |
| USER_GUIDE | product-areas note updated |

## Env (operators)

| Variable | Meaning |
| --- | --- |
| `LEGION_DAP_MODE` | `fixture` \| `live` \| `auto` (default). `live` fails closed. |
| `LEGION_DAP_ADAPTER` | Absolute path to a **Legion-compatible** adapter (provisional JSON-RPC envelope) |
| `LEGION_DAP_USE_FAKE` | `1` — allow in-tree `fake_dap_adapter` for CI/dev |

## Explicitly still open

- Microsoft DAP message codec (`seq`/`type`/`command`) + contract test vs real adapter
- PATH auto-discovery of `lldb-dap` / CodeLLDB (blocked on codec)
- Pre-launch `cargo build` from `cargo_args` for product binaries
- Persistent live session for step/continue after launch (one-shot live then disconnect today)
- Documented CodeLLDB install UX polish
- Sandbox wrap of adapter spawn (Phase 3)

## Verification

```text
cargo test -p legion-debug --all-targets
cargo test -p legion-app --test debug_workflow
cargo test -p legion-desktop --test debug_workflow
cargo test -p legion-ui --test debug_projection
```
