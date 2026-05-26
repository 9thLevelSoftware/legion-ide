# ADR-0002: Select Primary UI/Editor Rendering Architecture

## Status
Accepted with reservations — Spike 1A validated projection-only shell behavior; renderer-backed p50/p95 input-to-paint, IME, clipboard, focus, and accessibility evidence remain follow-ups.

## Context
The UI decision is the highest-risk strategic choice. Devil IDE's brand promise depends on sub-16ms frame times, low input latency, and native editor control. Browser-based shells risk recreating VS Code architecture; experimental Rust UI libraries risk platform edge cases.

## Decision
Primary path: GPUI-style Rust-native GPU UI/editor shell. Fallback: Slint for non-editor panels. Contingency: Tauri/WRY reserved for non-core auxiliary surfaces only.

## Consequences
- **Positive**: Best alignment with latency thesis and custom editor surface requirements.
- **Positive**: Direct Rust integration avoids JS/Rust state bifurcation.
- **Negative**: Ecosystem maturity risk for accessibility, IME, native menus, and cross-platform stability.
- **Negative**: Smaller hiring pool for UI engineers compared to web technologies.
- **Mitigation**: Spike 1 must validate text rendering, input latency, IME, and accessibility roadmap before scaling UI team.

## Phase 1 GUI Productization Update

Date: 2026-05-26

### Decision

Phase 2 will use a new `devil-desktop` adapter with `eframe`/`egui` as the Windows-first renderer-backed foundation proof. The decision matrix is archived at `plans/evidence/gui-productization/renderer-decision-matrix.md`.

This keeps the original Rust-native direction but narrows the first desktop implementation to a documented Windows-capable Rust stack. The exact GPUI crate is not selected for Phase 2 because the official GPUI README describes it as pre-1.0 and currently requiring macOS or Linux. GPUI remains an architectural influence for later custom editor rendering, not an approved Phase 2 dependency.

### Dependency Boundary

Renderer dependencies may live only in the planned `devil-desktop` adapter after the dependency policy and `xtask` gate are updated. They are not authorized in `devil-ui`, editor, project, protocol, storage, observability, security, provider, plugin, collaboration, remote, terminal, telemetry, retention, or AI crates.

`devil-ui` remains projection-only. It consumes protocol projections and emits `CommandDispatchIntent`; it must not own editor text, workspace state, save decisions, provider credentials, telemetry storage, or persistence policy.

### Fallback Triggers

Fallback to Slint or a custom winit/wgpu/AccessKit adapter if the eframe/egui proof cannot demonstrate acceptable p50/p95 input-to-paint, IME, clipboard, focus, accessibility, high-DPI behavior, and projection-only compatibility on Windows.

Tauri/WRY remains reserved for auxiliary surfaces. It is not accepted for the core editor shell unless a later ADR supersedes this one, because the webview/message-passing model raises state-split risk for this repository's app-owned editor and workspace authority.

### Required Evidence

Before claiming a renderer-backed GUI shell is accepted, Phase 2 must archive:

- p50/p95 input-to-paint and frame-variance measurements.
- IME, clipboard, focus traversal, high-DPI, file-dialog, and accessibility evidence.
- Proof that `devil-desktop` consumes `ShellProjectionSnapshot` and routes user actions through `CommandDispatchIntent` or app-owned requests.
- Proof that saves remain proposal-mediated through the app/workspace workflow and preserve rejected conflict/denial outcomes.
- Passing `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, and targeted app/UI/desktop checks.
