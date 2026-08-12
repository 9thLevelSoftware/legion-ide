# Phase 3 — Sandbox isolation evidence

**Matrix:** `docs/SECURITY.md` (§ Sandbox guarantees and platform caveats).

**Escape probe:** `crates/legion-sandbox` + `tests/escape_attempts.rs`.

| Slice | Status | Evidence |
| --- | --- | --- |
| C0 threat model stub | Draft | `C0-threat-model-stub.md` |
| C1 Linux network | Landed | `C1-linux-network-isolation.md` (`bwrap --unshare-net`) |
| C2 Windows FS residual | Landed | `C2-windows-fs-residual.md` (honest non-enforcement) |
| C3 product spawn integration | Landed | `C3-product-spawn-integration.md` (live report → UI) |
| C4 DAP sandboxed stdio spawn | Landed | `C4-dap-stdio-sandbox.md` (`spawn_sandboxed_stdio`; #93) |
