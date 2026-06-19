# WS-MANUAL-01 Evidence

Date: 2026-06-19
Scope: Manual editor feel, rendering, input, focus, font, wrapping, degraded-mode messaging, deterministic renderer evidence, and zero-egress smoke.

## Branch State

- Branch: `codex/ws-manual-01`
- Starting dirty files: none (`git status --short --branch` showed only the branch line).
- Ending dirty files: `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md` before commit; none expected after `docs: seed WS-MANUAL-01 evidence`.

Current branch history already contains `a6666ff docs: add WS-MANUAL-01 implementation plan` and `7ca090b test: fix Windows baseline portability`.

## Workstream Coverage

| Master-plan task | Evidence |
| --- | --- |
| MANUAL.01 latency budgets | `plans/evidence/production/WS-MANUAL-01/editor-latency-budgets.md` |
| MANUAL.02 renderer-backed input-to-paint | `target/perf-harness/perf_report.toml`; `cargo run -p xtask -- perf-harness`; `cargo run -p xtask -- verify-perf-harness` |
| MANUAL.03 custom editor path / no TextEdit | `cargo run -p xtask -- no-egui-textedit`; `cargo test -p xtask --test no_egui_textedit` |
| MANUAL.04 IME smoke | `cargo test -p legion-desktop --test manual_input_conformance ime_composition_suppresses_shortcuts_and_commits_text -- --exact` |
| MANUAL.05 clipboard tests | `cargo test -p legion-desktop --test manual_input_conformance clipboard_copy_cut_paste_select_all_round_trips_through_app_authority -- --exact` |
| MANUAL.06 multi-cursor / rectangular selection | design decision row in this file plus app/editor projection tests |
| MANUAL.07 keyboard focus | `cargo test -p legion-desktop --test manual_input_conformance manual_focus_routes_text_to_active_surface_only -- --exact` |
| MANUAL.08 font fallback diagnostics | `cargo test -p legion-desktop --test manual_renderer_evidence font_fallback_diagnostics_are_projected_without_raw_font_paths -- --exact` |
| MANUAL.09 line wrapping policy | `cargo test -p legion-desktop --test manual_renderer_evidence line_wrapping_policy_keeps_viewport_math_stable -- --exact` |
| MANUAL.10 degraded-mode banner | `cargo test -p legion-desktop --test large_file_guardrails large_file_guardrails_degraded_banner_names_capability_reduction -- --exact` |
| MANUAL.11 deterministic renderer evidence | `cargo test -p legion-desktop --test manual_renderer_evidence deterministic_renderer_evidence_covers_core_editor_states -- --exact` |
| MANUAL.12 zero-egress smoke | `crates/legion-app/tests/manual_zero_egress.rs`; `plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md` |

## Phase 0 Baseline Mapping

| Command | Result | Notes |
| --- | --- | --- |
| `git status --short --branch` | Pass | Starting state was clean on `codex/ws-manual-01`; output showed only `## codex/ws-manual-01`. |
| `rg -n "WS-MANUAL-01\|MANUAL\\.0\|PR-UI-001" plans/legion-production-master-plan-v0.2.md plans/product-readiness-ledger.md` | Pass | Confirmed WS-MANUAL-01 at `plans/legion-production-master-plan-v0.2.md:276`, MANUAL.01 through MANUAL.09 task rows at lines 282-290, PR-UI-001 decision text at line 306, and product-readiness PR-UI-001 ledger rows. |
| `rg -n "TextEdit\|CodeCanvas\|manual editor\|FrameTimingRecorder\|perf-harness\|zero-egress\|egress" crates xtask plans -g "*.rs" -g "*.md" -g "*.toml"` | Pass | Confirmed existing renderer/timing/gate/manual trust surfaces, including `CodeCanvasPainter`, `FrameTimingRecorder`, `xtask no-egui-textedit`, Manual mode projection rows, perf-harness evidence, and zero-egress/egress policy references. Output also included expected incidental protocol `TextEdit` and historical evidence matches, so this ledger records the signal summary rather than the full search output. |

## Verification

| Command | Result | Notes |
| --- | --- | --- |
| `cargo test -p xtask --test perf_harness` |  |  |
| `cargo run -p xtask -- perf-harness` |  |  |
| `cargo run -p xtask -- verify-perf-harness` |  |  |
| `cargo run -p xtask -- no-egui-textedit` |  |  |
| `cargo test -p legion-desktop --test manual_perf` |  |  |
| `cargo test -p legion-desktop --test manual_input_conformance` |  |  |
| `cargo test -p legion-desktop --test manual_renderer_evidence` |  |  |
| `cargo test -p legion-app --test manual_zero_egress` |  |  |
| `cargo run -p xtask -- check-deps` |  |  |
| `cargo run -p xtask -- docs-hygiene` |  |  |
| `cargo fmt --all --check` |  |  |
| `cargo check --workspace --all-targets` |  |  |
| `cargo test --workspace --all-targets --no-fail-fast` |  |  |
| `cargo clippy --workspace --all-targets -- -D warnings` |  |  |
| `git diff --check` |  |  |

## Product-Readiness Decision

`PR-UI-001` remains bounded by the evidence above. Do not mark it product-workflow validated unless all required Manual input, focus, accessibility, renderer-backed performance, platform, and zero-egress checks pass in the current tree.

## Residual Risk

- Native OS IME, clipboard, focus, high-DPI, and accessibility evidence must name the host OS where it was observed.
- Renderer-backed perf can still be blocked on machines without a native window or GPU path; blocked runs must be recorded as blocked, not passed.
- WS-MANUAL-02 owns full large-workspace and 100MB streaming performance; WS-MANUAL-01 only improves visible capability reduction and renderer/input evidence.
