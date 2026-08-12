# Phase 3 C4 — DAP adapter sandboxed stdio spawn

**Date:** 2026-07-23  
**Status:** Landed on `main` via [#93](https://github.com/9thLevelSoftware/legion-ide/pull/93) (`0cd04dc`)

## Problem

C3 product tool spawn uses batch `spawn_sandboxed` (wait + capture). Live DAP
needs **long-lived** stdin/stdout pipes. Adapter spawn was unsandboxed.

## Delivered

| Item | Location |
| --- | --- |
| `spawn_sandboxed_stdio` | `legion-sandbox::spawn_stdio` |
| Linux | Landlock write + optional bwrap unshare-net |
| macOS | sandbox-exec SBPL + pipes |
| Windows | Job object kill-on-close when assignable (honest no FS/net) |
| App wire | Non-fake live DAP launch uses sandboxed stdio; fake skips |
| Fallback | If sandbox unavailable, plain spawn + console honesty note |
| `LiveDapSession::from_stdio` | Accept pre-spawned child |

## Honesty

- Windows remains job-object process lifetime only (same C2 residual).
- Fake adapter path stays unsandboxed for CI speed/reliability.
- Guard kept across B7 continue worker so job object is not dropped early.

## Verification

```text
cargo test -p legion-sandbox --test stdio_spawn
cargo test -p legion-app --test debug_workflow
cargo test -p legion-desktop --test debug_keyboard
```
