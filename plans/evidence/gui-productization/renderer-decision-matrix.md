# Renderer Decision Matrix

Date: 2026-05-26

## Source Set

Official and primary sources checked:

- GPUI README in `zed-industries/zed`: https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md
- egui README: https://github.com/emilk/egui
- eframe feature documentation: https://docs.rs/crate/eframe/latest/features
- Slint desktop platform documentation: https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/
- Tauri v2 architecture documentation: https://v2.tauri.app/concept/architecture/
- WRY README: https://github.com/tauri-apps/wry
- AccessKit documentation: https://accesskit.dev/

Repository sources checked:

- `Cargo.toml`
- `crates/devil-ui/Cargo.toml`
- `crates/devil-app/Cargo.toml`
- `plans/adrs/ADR-0002-ui-editor-rendering.md`
- `plans/spikes/SPIKE-001A-result.md`
- `plans/dependency-policy.md`
- `.planning/CODEBASE.md`

## Required Criteria

Phase 2 needs a Windows-first desktop shell that can open a real window, render current projection DTOs, route input to app-owned commands, and produce measurable renderer evidence without moving editor/workspace/proposal authority into UI.

Minimum criteria:

- Windows-first viability for Windows 10/11 development.
- Rust-owned app composition remains authoritative.
- Text rendering path can be measured and later replaced or deepened for editor-grade needs.
- Key input, IME, clipboard, focus, high-DPI, and file-dialog/native bridge behavior have an explicit proof obligation.
- Accessibility has a credible AccessKit or native accessibility path.
- Input-to-paint instrumentation can be added at the adapter boundary.
- Renderer dependencies stay out of `devil-ui` and core crates.
- The approach does not require webview or renderer state to own editor text, save state, proposal lifecycle, provider calls, telemetry storage, or workspace mutation.

## Candidate Matrix

| Candidate | Evidence | Fit | Risks | Decision |
| --- | --- | --- | --- | --- |
| GPUI exact dependency | Official README says GPUI is a hybrid immediate/retained GPU framework for Rust, is pre-1.0, and currently requires macOS or Linux. | Strong conceptual fit for a custom editor surface and Rust-native control. | Not Windows-first from official README; pre-1.0 breakage risk; accessibility/IME proof would need direct project evidence before adoption. | Not selected for Phase 2 dependency. Preserve as long-term architecture influence only. |
| Custom Rust-native GPU adapter | Matches ADR-0002 direction and gives maximum control over text rendering, input-to-paint metrics, and authority boundaries. | Best eventual fit for editor-grade rendering. | Too broad for Phase 2 foundation unless paired with a smaller proof path; requires direct windowing, input, clipboard, IME, accessibility, and GPU pipeline work. | Keep as strategic target after foundation evidence. |
| egui/eframe | egui README says eframe supports Web, Linux, Mac, Windows, and Android; egui includes widgets, text editing, copy/paste, windows/panels, custom painting, and accessibility via AccessKit. eframe defaults include accesskit, winit, wgpu, x11/wayland, and clipboard/windowing integrations through the framework stack. | Good Phase 2 foundation fit: Rust-first, fast to integrate, native Windows path through eframe, easy projection rendering, custom painting hooks, AccessKit path, and no JavaScript state split. | Immediate-mode layout is not the final editor architecture; native look is a non-goal; deeper IME/editor text behavior must be proven before daily-driver claims. | Selected for Phase 2 foundation adapter proof, scoped to `devil-desktop` only. |
| Slint | Official docs state Slint generally runs on Windows, macOS, and Linux and specifically lists Windows 10 and Windows 11. AccessKit docs list Slint among Rust projects integrating AccessKit. | Strong for native panels, platform support, and declarative UI. | Less direct control for custom editor text surface; adds Slint language/build integration; may be better as a fallback for panel-heavy UI than the initial editor shell. | Fallback if eframe cannot satisfy focus/IME/accessibility proof obligations. |
| Tauri/WRY | Tauri docs describe Rust plus HTML in a WebView with message passing; WRY supports Windows/macOS/Linux and uses WebView2 on Windows. | Mature desktop packaging and native bridge story; strong if a web UI already existed. | Reintroduces webview state split and browser/UI boundary for an IDE whose current architecture is Rust protocol projection plus app authority. Harder to preserve the no-renderer-ownership rule by default. | Reserved for auxiliary/non-core surfaces only. Not selected for editor shell. |

## Decision

Phase 2 should implement the first renderer-backed desktop proof as a new `devil-desktop` adapter using `eframe`/`egui` plus the framework-provided native integration stack. This does not move `devil-ui` or core crates to egui. `devil-ui` remains projection-only, and `devil-desktop` consumes `ShellProjectionSnapshot` and emits `CommandDispatchIntent` through `devil-app`.

This decision narrows ADR-0002 from "GPUI-style native GPU" to a Windows-first Rust-native proof path. GPUI remains an architectural reference for long-term custom editor rendering, but the official GPUI README does not currently satisfy the Windows-first requirement for a Phase 2 dependency.

## Fallback Triggers

Fallback from eframe/egui to Slint or a custom winit/wgpu/AccessKit adapter if any of the following cannot be proven in Phase 2:

- p95 input-to-paint for ordinary editing interaction exceeds the accepted budget.
- IME composition cannot be represented without corrupting editor/app command boundaries.
- Clipboard integration cannot preserve app-owned command correlation where mutation is emitted.
- Focus traversal or keyboard handling cannot be made deterministic enough for editor workflows.
- Accessibility tree publication cannot expose meaningful panels, editor viewport summaries, and command targets.
- High-DPI behavior produces unreadable or unstable layout on Windows.
- The adapter requires `devil-ui` or core crates to depend on renderer/windowing crates.

Tauri/WRY is a fallback only for auxiliary surfaces where HTML rendering is appropriate and app authority remains Rust-owned through explicit IPC. It is not a fallback for the core editor shell unless a later ADR supersedes this decision.

## Phase 2 Proof Obligations

- Add `devil-desktop` as the only crate allowed to depend on `eframe`, `egui`, and renderer/windowing integration crates.
- Render layout, explorer, active buffer viewport, status, proposal summary, and trust summaries from `ShellProjectionSnapshot`.
- Route keyboard, menu, mouse, close, open, save, and file-dialog results into `CommandDispatchIntent` or app-owned requests.
- Archive p50/p95 input-to-paint, frame variance, high-DPI, IME, clipboard, focus, and accessibility evidence.
- Keep saves proposal-mediated and preserve stale/conflict/denial outcomes.
- Run `cargo run -p xtask -- check-deps`, `cargo fmt --all --check`, `cargo check --workspace --all-targets`, targeted app/UI tests, and future desktop smoke tests before claiming the GUI shell is ready.
