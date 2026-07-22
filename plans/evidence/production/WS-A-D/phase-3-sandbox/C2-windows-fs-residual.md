# Phase 3 C2 — Windows FS residual cut line

**Date:** 2026-07-21  
**Status:** Residual accepted (honest enforcement report; no silent “enforced”)

## Decision

Windows sandboxed spawn remains **job-object-only** (`job-object-kill-on-close`):

| Capability | Enforced? | Notes |
| --- | --- | --- |
| Process kill on job close / timeout | **Yes** | `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` |
| Filesystem write outside `writable_root` | **No** | Residual risk; probe proves write succeeds |
| Network egress | **No** | Residual risk |

`SandboxEnforcementReport` always reports:

- `filesystem_write_enforced = false`
- `filesystem_read_enforced = false`
- `network_enforced = false`
- Caveats: `windows-no-restricted-token`, `windows-no-filesystem-enforcement`, `windows-no-network-enforcement`

## Why not “enforce now”

Restricted-token / AppContainer paths that would block arbitrary paths require privileges or packaging models Legion does not currently ship for normal user installs. Claiming enforcement without those mechanisms would violate product honesty (same rule as DAP dual-mode).

## Tests (escape probe)

`crates/legion-sandbox/tests/escape_attempts.rs` (`#[cfg(target_os = "windows")]`):

- Outside-root write **succeeds** (`WRITE_OK` + file exists) under the current backend
- Report still has `filesystem_write_enforced = false` and filesystem/token caveats
- Network remains unenforced with network caveat labels
- Backend name is `job-object-kill-on-close`

## Product implications

- Delegate/tool sandboxes on Windows provide **process lifetime** isolation, not FS quarantine
- Operators must treat untrusted code on Windows as requiring host trust / workspace trust gates first
- Linux (Landlock + optional bwrap) and macOS (Seatbelt) remain the stronger FS/network tiers

## Follow-on (not C2)

- Restricted token / AppContainer research spike with explicit privilege requirements
- Optional low-integrity experiment (not a substitute for path allowlists)
- C3 product spawn integration polish once residual is accepted

## Verification

```text
cargo test -p legion-sandbox --test escape_attempts -- --nocapture
# On Windows: write_outside_writable_root_enforcement_is_honest
```
