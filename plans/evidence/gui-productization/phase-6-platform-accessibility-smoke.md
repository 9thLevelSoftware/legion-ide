# Renderer Smoke Evidence

## Status

status: passed
workspace: .
file: Cargo.toml
duration_ms: 60000

## Command

`cargo run -p legion-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 60000 --evidence plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md --session-state target/gui-phase6-session.json --diagnostics-export target/gui-phase6-diagnostics.md`

## Timing

sample_count: 1
p50_input_to_paint_ms: 7.450
p95_input_to_paint_ms: 7.450
frame_count: 3575
average_frame_ms: 16.787
frame_variance_ms2: 29.476

## Platform Smoke

focus_smoke: os-observed focused
menu_smoke: projection command surface present
shortcut_smoke: adapter shortcut targets projected
clipboard_smoke: adapter-path passed
ime_smoke: adapter-path passed
theme_smoke: adapter theme defaults available
high_dpi_smoke: not observed
focus_traversal_smoke: projection focus traversal nodes 5; viewport focused
file_dialog_smoke: adapter-path passed
accessibility_smoke: not observed
accessibility_tree_smoke: metadata-only projection accessibility nodes 5; OS tree not observed
accessibility_projection_node_count: 5

## Large File Guardrails

large_file_degraded_status: not observed
bounded_search_status: not observed
full_text_projection_status: not observed

## Errors

- none

## Residual Risk

- Clipboard, IME, and file-dialog checks are adapter-path smoke unless an OS-observed status says otherwise.
- Accessibility and high-DPI status are not promoted beyond the observation recorded above.

## Follow-up OS Accessibility Tree Inspection

- A separate Swift Accessibility API probe against the running smoke process observed the native window `Legion IDE Smoke` with `AXWindow` / `AXStandardWindow` and accessible descendants, including `AXGroup`, `AXButton`, and `AXStaticText` nodes.
- The inspection saw the expected product-shell labels (`Legion IDE`, `branch - workspace`, `Engine idle - 0 proposals`, `PRODUCT MODE`, and the mode labels `Manual M`, `Assist A`, `Delegates D`, `Legion Workflows W`).
- That external OS tree probe is recorded in `plans/evidence/production/M5/WS18-T2-accesskit-product-pass.md`.
