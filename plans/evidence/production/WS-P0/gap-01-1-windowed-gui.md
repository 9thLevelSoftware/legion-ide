# GAP-01.1 — Windowed GUI E2E on an extracted native package

**Date:** 2026-09-01  
**Wave:** 2 proof surface  
**Task:** GAP-01.1

## What this is

A local Windows run of `cargo run -p xtask -- windowed-gui-e2e`:

1. `cargo build -p legion-desktop`
2. Copy `legion-desktop.exe`, `LICENSE`, `PRIVACY.md`, and `THIRD_PARTY_NOTICES.md` into `target/windowed-gui/package/`
3. Launch that packaged binary with `--windowed-e2e`
4. `eframe::run_native` creates a window, then open / edit / save through `DesktopRuntime` (proposal-mediated save)

Report: [`gap-01-1-windowed-gui-e2e.toml`](gap-01-1-windowed-gui-e2e.toml)

| Field | Value |
| --- | --- |
| binary_path | `D:/legion-ide/target/windowed-gui/package/legion-desktop.exe` |
| OS | windows / x86_64 |
| window_created | true |
| window_backend | `eframe::run_native` |
| open / edit / save | passed |
| fixture after save | `WINDOWED_E2E_EDIT` then `seed` |

## What this is not

- Not `--beta-smoke` (headless DesktopRuntime, no window)
- Not AppComposition `golden-path-5`
- Not GAP-01.2 (no 3-OS hosted job URLs yet)
- Not a signed installer
- Not a ledger promotion
- Not a general-availability claim

## Verification

```text
cargo test -p legion-desktop --test windowed_e2e
cargo run -p xtask -- windowed-gui-e2e
```

Primary files:

- `crates/legion-desktop/src/windowed_e2e.rs`
- `crates/legion-desktop/src/workflow.rs` (`--windowed-e2e`)
- `xtask/src/windowed_gui_e2e.rs`
