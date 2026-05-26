# Phase 1 Renderer Readiness

## Phase 1 readiness: Accepted

Date: 2026-05-26

Phase 1 is accepted for starting Phase 2 renderer-backed foundation work. The phase reconciled accepted Phase 8 substrate evidence, selected a Windows-first renderer proof path, documented the desktop adapter ownership boundary, and added dependency-policy plus `xtask` enforcement so renderer dependencies stay out of every workspace package except `devil-desktop`.

## Artifact Inventory

| Artifact | Status | Evidence |
| --- | --- | --- |
| `plans/phase-status-ledger.md` | Accepted | Contains `Phase 8 acceptance: Accepted` and records GUI productization as post-substrate work. |
| `plans/evidence/gui-productization/gui-productization-baseline.md` | Accepted | Documents current CLI shell proof, projection-only UI, proposal-mediated saves, metadata-only defaults, renderer gap, and Phase 2 entry criteria. |
| `plans/evidence/gui-productization/renderer-decision-matrix.md` | Accepted | Compares GPUI, custom Rust-native GPU, egui/eframe, Slint, and Tauri/WRY using official documentation and repository constraints. |
| `plans/adrs/ADR-0002-ui-editor-rendering.md` | Accepted with Phase 1 update | Selects `eframe`/`egui` for Phase 2 foundation proof in `devil-desktop` only. |
| `plans/adrs/ADR-0030-desktop-adapter-boundary.md` | Accepted | Defines `devil-desktop` as an adapter, not an editor/workspace/proposal authority. |
| `plans/desktop-adapter-boundary-v0.1.md` | Accepted | Specifies startup flow, projection input, intent output, side effects, forbidden authority, recovery semantics, and test harness requirements. |
| `plans/dependency-policy.md` | Accepted | Adds adapter-only renderer dependency policy and a conservative non-adapter renderer deny list. |
| `xtask/src/main.rs` | Accepted | Adds `renderer_dependency_gate_preserves_projection_boundary` and runtime `check-deps` validation across non-adapter workspace packages. |

## Decision Summary

- Phase 8 substrate status is accepted; GUI productization does not reopen Phase 8 runtime hardening.
- Phase 2 uses `devil-desktop` as the renderer-backed adapter crate name.
- `eframe`/`egui` is the accepted Phase 2 foundation renderer proof path because it is Rust-first, Windows-capable, and has an AccessKit accessibility path.
- GPUI remains an architectural influence but is not a Phase 2 dependency because current official documentation does not satisfy the Windows-first requirement.
- Slint is the fallback if Phase 2 evidence shows egui cannot satisfy IME, clipboard, focus, accessibility, or high-DPI obligations.
- Tauri/WRY remains auxiliary-only unless a later ADR supersedes ADR-0002.

## Gate Results

- `cargo run -p xtask -- check-deps: passed`
- `cargo fmt --all --check: passed`
- `cargo check --workspace --all-targets: passed`
- `cargo test -p xtask: passed`
- `cargo check -p devil-ui --all-targets: passed`
- `cargo check -p devil-app --all-targets: passed`
- `cargo test -p xtask renderer_dependency_gate_preserves_projection_boundary -- --exact: passed`

Formatting note: the first `cargo fmt --all --check` found one formatting diff in `xtask/src/main.rs`. `cargo fmt --all` was run, and the check passed afterward.

## Phase 2 Entry Criteria

- [x] Renderer path and fallback triggers are documented.
- [x] `devil-desktop` ownership and dependency boundaries are explicit enough to scaffold without moving editor or workspace authority into UI.
- [x] `devil-ui` remains projection-only and no non-adapter workspace package may depend on renderer/windowing crates.
- [x] Saves remain proposal-mediated through app/workspace services.
- [x] Dependency policy and `xtask` fail closed for renderer-boundary drift.
- [x] Phase 2 must archive p50/p95 input-to-paint, frame variance, IME, clipboard, focus, high-DPI, file-dialog, and accessibility proof before claiming GUI acceptance.

## Residual Risks

- `eframe`/`egui` is accepted only for the Phase 2 foundation proof. It is not yet accepted as the final daily-driver editor renderer.
- IME, accessibility, focus traversal, clipboard behavior, high-DPI rendering, and input-to-paint budgets remain unproven until Phase 2 implements the desktop shell.
- No `devil-desktop` crate exists yet; Phase 2 must add the crate and manifest changes under the policy gate.
- Tauri/WRY and GPUI are intentionally not selected for the core editor shell in this phase; changing that requires a later ADR and policy update.
