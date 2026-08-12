# Phase 3 C3 — Product spawn integration

**Date:** 2026-07-22  
**Status:** Landed (live report is product truth)

## Decision

Product delegated tool execution uses **`spawn_sandboxed`** end-to-end:

| Layer | Role |
| --- | --- |
| `AppDelegatedToolHost` | Builds `SandboxSpawnSpec`, calls `spawn_sandboxed`, records report |
| Tool output | Includes `sandbox live enforcement: backend=… fs_write=… network=…` |
| `DelegateWorkflowState` | `last_sandbox_enforcement_label` after loop success/failure |
| `DelegatedTaskProjection` | Label pushed into `plan_only_disclaimers` |
| Desktop sandbox panel | Rows prefixed `sandbox runtime:` when live label present |

Compile-time host profile notes (Seatbelt / Landlock / Job Object) remain **advisory** strength labels; they must not contradict the live report.

## Honesty updates this slice

- Linux panel caveats: FS Landlock + **bwrap unshare-net when available** (C1); selective egress still open
- Linux strength label: `os-enforced-fs-write; net-deny-all-if-bwrap` (replaces stale “fs-write-only / network never”)
- Windows/macOS caveats note product spawn live report authority
- Tests: panel surfaces live enforcement disclaimers from projection

## Explicit residuals

| Residual | Why |
| --- | --- |
| Interactive terminal PTY not through `spawn_sandboxed` | Separate trust/capability product path |
| ~~DAP adapter process not sandbox-wrapped~~ | **Closed C4** (`spawn_sandboxed_stdio`; Windows job-only residual) |
| Selective Linux egress allowlist | Still unimplemented |

## Code map

- `crates/legion-app/src/lib.rs` — `AppDelegatedToolHost`, `format_sandbox_enforcement_summary`
- `crates/legion-desktop/src/view/sandbox_panel.rs` — profile summary + live row projection
- `docs/SECURITY.md` — product spawn section

## Verification

```text
cargo test -p legion-desktop --test sandbox_panel
cargo test -p legion-desktop --lib view::sandbox_panel
cargo test -p legion-app --test delegated_task_integration app_delegated_tool_host_runs_echo_command
```
