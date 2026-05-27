# Renderer Smoke Evidence

## Status

status: passed
workspace: .
file: Cargo.toml
duration_ms: 250

## Command

`cargo run -p devil-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 250 --evidence plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md --session-state target/gui-phase6-session.json --diagnostics-export target/gui-phase6-diagnostics.md`

## Timing

sample_count: 1
p50_input_to_paint_ms: 2.904
p95_input_to_paint_ms: 2.904
frame_count: 2
average_frame_ms: 175.234
frame_variance_ms2: 26467.987

## Platform Smoke

focus_smoke: os-observed not focused
menu_smoke: projection command surface present
shortcut_smoke: adapter shortcut targets projected
clipboard_smoke: adapter-path passed
ime_smoke: adapter-path passed
theme_smoke: adapter theme defaults available
high_dpi_smoke: os-observed scale 1.500
focus_traversal_smoke: projection focus traversal nodes 5; viewport not focused
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
