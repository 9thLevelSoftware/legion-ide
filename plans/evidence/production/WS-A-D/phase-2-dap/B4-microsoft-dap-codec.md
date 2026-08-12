# Phase 2 B4 — Microsoft DAP wire codec

**Date:** 2026-07-22  
**Status:** Landed

## Delivered

| Item | Location |
| --- | --- |
| Microsoft DAP message types | `crates/legion-debug/src/framing.rs` (`DapMessage` request/response/event) |
| Content-Length framer | same (encode/decode/`read_from`/`write_to`) |
| Live session client | `crates/legion-debug/src/live_session.rs` |
| Fake adapter (contract) | `crates/legion-debug/src/bin/fake_dap_adapter.rs` |
| PATH resolution re-enabled | `adapter_resolve.rs` (wire now standards-shaped) |

## Wire shape

```text
Content-Length: N\r\n\r\n
{"seq":1,"type":"request","command":"initialize","arguments":{...}}
```

Responses: `type=response`, `request_seq`, `success`, `body`.  
Events: `type=event`, `event`, `body`.

**Not** used: Legion provisional JSON-RPC (`jsonrpc`/`method`/`params`).

## Contract test strategy

CI always runs the in-tree `fake_dap_adapter`, which speaks Microsoft DAP only.
Handshake + breakpoints + launch/stack/step tests in `live_dap_handshake` exercise the real codec path without requiring CodeLLDB on runners.

## Residual

- Persistent multi-step live session in product UI (one-shot launch still common)
- Optional dogfood against system `lldb-dap` / CodeLLDB when installed
- Pre-launch `cargo build` for real binaries

## Verification

```text
cargo test -p legion-debug --all-targets
cargo test -p legion-app --test debug_workflow
```
