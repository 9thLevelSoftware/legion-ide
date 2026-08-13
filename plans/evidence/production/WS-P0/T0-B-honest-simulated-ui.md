# T0-B — Honest UI for simulated / deferred surfaces

**Date:** 2026-07-21  
**Packet:** Tier 0 truth repair — desktop cut-line copy  

## Intent

Desktop status strings and panel labels must not imply full product capability for fixture/metadata/harness paths.

## String inventory (before → after)

| Surface | Before | After |
| --- | --- | --- |
| Plugin load status | `Plugin {id} loaded` | `Plugin {id} registered (metadata-only; WASM execution not available in this build)` |
| Remote runtime enable | `Remote workspace runtime enabled by app policy` | + `(fixture/harness; PR-ENT-001 product UX deferred)` |
| Remote connect | `Remote workspace connected {id} authority=…` | `Remote fixture session active … (no production transport; PR-ENT-001 deferred)` |
| Debug fixture | `Debug fixture enabled by app policy` | + `(simulated DAP — no adapter process)` |
| Plugin management row | `sandbox=metadata-only audit=app-owned` | + `execution=not-available` |
| Context packs | unlabeled hardcoded list | Section subtitle `Sample / not live inventory` |
| Deterministic provider UI | raw provider label | `Deterministic fixture (not a live model)` when id/label is deterministic |
| Sandbox strength (Windows RestrictedToken) | `process-isolated` | `process-lifetime-only` |
| Sandbox strength (Linux Landlock) | `os-enforced` | `os-enforced-fs-write-only` |
| Sandbox caveats | profile notes only | + explicit FS/network incomplete-enforcement caveats |

## Code

- New: `crates/legion-desktop/src/cut_lines.rs`
- Updated: `workflow.rs`, `view.rs`, `view/sandbox_panel.rs`, `lib.rs`
- Tests: `remote_workspace_gui.rs`, `plugin_management.rs`, `sandbox_panel.rs` (+ unit tests in sandbox_panel module)

## Gates (when cargo available)

```text
cargo test -p legion-desktop --all-targets
cargo clippy -p legion-desktop --all-targets -- -D warnings
```

## Non-goals

- Did not implement real DAP, remote transport, WASM product host, or live default AI.
