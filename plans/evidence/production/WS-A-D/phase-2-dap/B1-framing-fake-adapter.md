# Phase 2 B1 — DAP framing + fake adapter

**Date:** 2026-07-21  
**ADR:** `plans/adrs/ADR-0044-dap-client-architecture.md`

## Delivered

| Item | Location |
| --- | --- |
| Content-Length framing | `crates/legion-debug/src/framing.rs` |
| Live session (spawn + initialize + disconnect) | `crates/legion-debug/src/live_session.rs` |
| Fake DAP adapter binary | `crates/legion-debug/src/bin/fake_dap_adapter.rs` |
| Contract test | `crates/legion-debug/tests/live_dap_handshake.rs` |
| Dependency policy | `serde` / `serde_json` + std process note |

## Explicitly not in B1

- App/desktop wiring of live path (still fixture by default)
- Breakpoints / stack / variables / step (B2)
- CodeLLDB resolution + untrusted deny (B3)
- `legion-platform` spawn abstraction

## Verification

```text
cargo test -p legion-debug
cargo test -p legion-debug --test live_dap_handshake
cargo run -p xtask -- check-deps
```
