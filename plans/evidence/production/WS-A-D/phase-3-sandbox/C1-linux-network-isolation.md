# Phase 3 C1 — Linux network isolation

**Date:** 2026-07-21

## Decision

When `SandboxSpawnSpec.allowed_egress` is **empty**, Linux spawn wraps the child with **bubblewrap** `--unshare-net` (if `bwrap` is on PATH or at `/usr/bin/bwrap` / `/bin/bwrap`). Landlock FS write rules still apply via `pre_exec`.

`SandboxEnforcementReport.network_enforced` is set **true** only when bwrap unshare is used. Otherwise caveats:

- `bwrap-unshare-net-unavailable` — deny-all requested but bwrap missing
- `linux-egress-allowlist-not-implemented` — non-empty allowlist (macOS still handles selective egress)

## Code

- `crates/legion-sandbox/src/spawn.rs` (`linux::spawn_sandboxed_linux`)
- `docs/SECURITY.md` matrix updated

## Also

- `golden_path_4` `required-features = ["ai", "test-helpers"]` so `cargo check --all-targets` with default features does not build a bin that needs `inject_cancellation_flag_for_test`.

## Verification

```text
cargo test -p legion-sandbox
# On Linux hosts with bwrap: network_enforced should be true for empty egress
```
