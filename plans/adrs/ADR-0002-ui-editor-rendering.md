# ADR-0002: Select Primary UI/Editor Rendering Architecture

## Status
Provisional — pending Spike 1 validation

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
