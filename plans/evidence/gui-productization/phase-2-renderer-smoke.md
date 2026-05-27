# Phase 2 Renderer Smoke Evidence

## Status

status: passed
workspace: .
file: Cargo.toml
duration_ms: 1500

## Command

`cargo run -p devil-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 1500 --evidence plans/evidence/gui-productization/phase-2-renderer-smoke.md`

## Timing

sample_count: 1
p50_input_to_paint_ms: 3.120
p95_input_to_paint_ms: 3.120
frame_count: 127
average_frame_ms: 11.884
frame_variance_ms2: 1027.753

## Platform Smoke

focus_smoke: os-observed focused
clipboard_smoke: adapter-path passed
ime_smoke: adapter-path passed
high_dpi_smoke: os-observed scale 1.500
file_dialog_smoke: adapter-path passed
accessibility_smoke: not observed

## Errors

- none

## Residual Risk

- Clipboard, IME, and file-dialog checks are adapter-path smoke unless an OS-observed status says otherwise.
- Accessibility and high-DPI status are not promoted beyond the observation recorded above.
