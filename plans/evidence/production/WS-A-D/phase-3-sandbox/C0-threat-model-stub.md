# Phase 3 C0 — Sandbox threat model stub

**Date:** 2026-07-21  
**Status:** Draft stub (full C0 can expand after Phase 1 interactive dogfood / parallel to B1)

## Current enforcement (source of truth)

See `docs/SECURITY.md` § Sandbox guarantees and platform caveats.

| Target for WS-A-D Phase 3 | Priority |
| --- | --- |
| Linux network deny-by-default for sandboxed spawns (+ loopback allowlist) | P0 |
| Windows FS isolation best-effort without elevation; honest residual if incomplete | P0 |
| Escape probe matrix updates | P0 |
| Optional DAP adapter spawn wrap | P2 (after B1) |

## Non-goals

- Equal strength across all OS (SECURITY.md forbids this claim)
- Kernel/hypervisor assurance
- VSIX host sandboxing

## Acceptance sketch

| Probe | Linux (target) | Windows (target) |
| --- | --- | --- |
| Write outside workspace | DENIED | DENIED if enforcement true; else report false |
| Connect non-allowlisted | DENIED | Best-effort / report false if impossible |

## Next

Expand into full C0 note + acceptance table PR; implement C1 Linux network.
