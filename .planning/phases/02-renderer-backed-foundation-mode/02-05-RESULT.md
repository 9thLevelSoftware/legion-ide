# Plan 02-05 Result: Renderer Timing And Platform Smoke Evidence

Status: Complete
Wave: 4
Agents: testing-performance-benchmarker, testing-qa-verification-specialist, engineering-frontend-developer

## Files Changed

- `crates/devil-desktop/src/metrics.rs`: added metadata-only `InputPaintSample`, `FrameTimingRecorder`, and `FrameTimingSummary` with deterministic percentile and variance calculation.
- `crates/devil-desktop/src/smoke.rs`: added `RendererSmokeConfig`, `RendererSmokeReport`, markdown evidence writing, adapter-path platform checks, and a timed eframe smoke app that closes itself.
- `crates/devil-desktop/src/workflow.rs`: added smoke flag parsing and `--smoke` routing.
- `crates/devil-desktop/tests/platform_smoke.rs`: added deterministic metric/report/parser/adapter-path coverage.
- `plans/evidence/gui-productization/phase-2-renderer-smoke.md`: recorded the actual timed smoke run from this checkout.

`crates/devil-desktop/src/main.rs` already delegated to `workflow::run_from_env()` and required no functional change.

## Smoke Status

- Status: passed
- Command: `cargo run -p devil-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 1500 --evidence plans/evidence/gui-productization/phase-2-renderer-smoke.md`
- Duration: 1500 ms
- Workspace: `.`
- File: `Cargo.toml`

## Metrics

- `sample_count`: 1
- `p50_input_to_paint_ms`: 3.120
- `p95_input_to_paint_ms`: 3.120
- `frame_count`: 127
- `average_frame_ms`: 11.884
- `frame_variance_ms2`: 1027.753

## Platform Smoke

- `focus_smoke`: os-observed focused
- `clipboard_smoke`: adapter-path passed
- `ime_smoke`: adapter-path passed
- `high_dpi_smoke`: os-observed scale 1.500
- `file_dialog_smoke`: adapter-path passed
- `accessibility_smoke`: not observed

## Verification

| Command | Result |
| --- | --- |
| `rg -q "FrameTimingRecorder" crates/devil-desktop/src/metrics.rs` | passed |
| `rg -q "RendererSmokeReport" crates/devil-desktop/src/smoke.rs` | passed |
| `cargo test -p devil-desktop platform_smoke --test platform_smoke` | passed; 6 passed |
| `cargo check -p devil-desktop --all-targets` | passed |
| `cargo run -p devil-desktop -- --smoke --workspace . --file Cargo.toml --duration-ms 1500 --evidence plans/evidence/gui-productization/phase-2-renderer-smoke.md` | passed |
| `rg -q "p95_input_to_paint_ms" plans/evidence/gui-productization/phase-2-renderer-smoke.md` | passed |
| `rg -q "accessibility_smoke" plans/evidence/gui-productization/phase-2-renderer-smoke.md` | passed |

## Residual Risk

Clipboard, IME, and file-dialog checks are adapter-path smoke in this phase, not OS-level interaction proof. Accessibility remains explicitly `not observed`.

## Issues

None.
