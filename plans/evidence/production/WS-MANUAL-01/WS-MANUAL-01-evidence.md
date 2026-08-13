# WS-MANUAL-01 Evidence

Date: 2026-06-19
Scope: Manual editor feel, rendering, input, focus, font, wrapping, degraded-mode messaging, deterministic renderer evidence, and zero-egress smoke.

## Branch State

- Branch: `codex/ws-manual-01`
- Starting dirty files: none (`git status --short --branch` showed only the branch line).
- Ending dirty files: none expected after the final evidence commit.

Current branch history contains WS-MANUAL-01 implementation and evidence commits through the final Phase 10 verification update.

## Workstream Coverage

| Master-plan task | Planned evidence / target check |
| --- | --- |
| MANUAL.01 latency budgets | `plans/evidence/production/WS-MANUAL-01/editor-latency-budgets.md`; `cargo test -p xtask --test perf_harness` |
| MANUAL.02 renderer-backed input-to-paint | `target/perf-harness/perf_report.toml`; `cargo run -p xtask -- perf-harness`; `cargo run -p xtask -- verify-perf-harness`; `cargo test -p legion-desktop --test manual_perf` |
| MANUAL.03 custom editor path / no TextEdit | `cargo run -p xtask -- no-egui-textedit`; `cargo test -p xtask --test no_egui_textedit` |
| MANUAL.04 IME smoke | `cargo test -p legion-desktop --test manual_input_conformance manual_input_conformance_commits_ime_text_and_advances_projected_cursor -- --exact` |
| MANUAL.05 clipboard tests | `cargo test -p legion-desktop --test manual_input_conformance manual_input_conformance_clipboard_copy_cut_paste_and_select_all_are_app_owned -- --exact`; copy/cut are suppressed during active IME composition. |
| MANUAL.06 multi-cursor / rectangular selection | Covered for v1 Manual scope by the editor projection substrate test; rectangular selection is explicitly deferred. See MANUAL.06 Decision below. |
| MANUAL.07 keyboard focus | `cargo test -p legion-desktop --test manual_input_conformance manual_input_conformance_palette_focus_blocks_direct_editor_insert -- --exact` |
| MANUAL.08 font fallback diagnostics | `cargo test -p legion-desktop --test manual_renderer_evidence font_fallback_diagnostics_are_projected_without_raw_font_paths -- --exact` |
| MANUAL.09 line wrapping policy | `cargo test -p legion-desktop --test manual_renderer_evidence line_wrapping_policy_keeps_viewport_math_stable -- --exact` |
| MANUAL.10 degraded-mode banner | `cargo test -p legion-desktop --test large_file_guardrails large_file_guardrails_degraded_banner_names_capability_reduction -- --exact` |
| MANUAL.11 deterministic renderer evidence | `cargo test -p legion-desktop --test manual_renderer_evidence deterministic_renderer_evidence_covers_core_editor_states -- --exact` |
| MANUAL.12 zero-egress smoke | `crates/legion-app/tests/manual_zero_egress.rs`; `plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md`; `cargo test -p legion-app --test manual_zero_egress` |

## MANUAL.06 Decision

Decision status: recorded. Multi-cursor substrate is in scope for v1 Manual mode and is covered by `crates/legion-editor/src/lib.rs::engine_preserves_multiple_cursors_and_selections_in_projection`, which verifies that the editor engine preserves multiple cursors and selections through viewport projection.

Rectangular selection is intentionally deferred out of the v1 product-workflow gate until the editor exposes a rectangular selection command with stable protocol DTOs, keyboard/mouse gestures, and renderer evidence. The Manual UI must not advertise rectangular selection as complete.

## Phase 0 Baseline Mapping

| Command | Result | Notes |
| --- | --- | --- |
| `git status --short --branch` | Pass | Starting state was clean on `codex/ws-manual-01`; output showed only `## codex/ws-manual-01`. |
| `rg -n "WS-MANUAL-01\|MANUAL\\.0\|PR-UI-001" plans/legion-production-master-plan-v0.2.md plans/product-readiness-ledger.md` | Pass | Confirmed WS-MANUAL-01 at `plans/legion-production-master-plan-v0.2.md:276`, MANUAL.01 through MANUAL.09 task rows at lines 282-290, PR-UI-001 decision text at line 306, and product-readiness PR-UI-001 ledger rows. |
| `rg -n "TextEdit\|CodeCanvas\|manual editor\|FrameTimingRecorder\|perf-harness\|zero-egress\|egress" crates xtask plans -g "*.rs" -g "*.md" -g "*.toml"` | Pass | Confirmed existing renderer/timing/gate/manual trust surfaces, including `CodeCanvasPainter`, `FrameTimingRecorder`, `xtask no-egui-textedit`, Manual mode projection rows, perf-harness evidence, and egress policy references. Zero-egress hits are plan/evidence targets until MANUAL.12 creates smoke evidence. Output also included expected incidental protocol `TextEdit` and historical evidence matches, so this ledger records the signal summary rather than the full search output. |

## Verification

| Command | Result | Notes |
| --- | --- | --- |
| `cargo test -p xtask --test perf_harness` | Pass | Phase 10 rerun passed: 20 passed, 0 failed. |
| `cargo run -p xtask -- perf-harness` | Pass | Generated `target/perf-harness/perf_report.toml` at git `04207b9db7f69bdccbc5ced0abd8d5416bb5b7f6`: 3 passed, 0 failed, 0 skipped; `manual.renderer_input_to_paint` p50 2077 us / p95 18223 us under the 32 ms budget. |
| `cargo run -p xtask -- verify-perf-harness` | Pass | Strict verification passed for the final perf report: 3 passed, 0 failed, 0 skipped. |
| `cargo run -p xtask -- no-egui-textedit` | Pass | Phase 10 rerun passed; custom Manual editor path still avoids `egui::TextEdit`. |
| `cargo test -p xtask --test no_egui_textedit` | Pass | Phase 10 rerun passed: 6 passed, 0 failed. |
| `cargo test -p legion-desktop --test manual_perf` | Pass | Phase 10 rerun passed: 3 passed, 0 failed. |
| `cargo test -p legion-desktop --test manual_input_conformance` | Pass | Phase 10 rerun passed: 3 passed, 0 failed; covers Manual focus, IME, clipboard, copy/cut composition suppression, and selection-scope behavior. |
| `cargo test -p legion-desktop --test manual_renderer_evidence` | Pass | Phase 10 rerun passed: 4 passed, 0 failed; covers MANUAL.08, MANUAL.09, MANUAL.11, and renderer zero-egress trust-boundary rows. |
| `cargo test -p legion-desktop --test large_file_guardrails` | Pass | Phase 10 rerun passed: 3 passed, 0 failed; covers the MANUAL.10 degraded-mode banner and no-source-leak guardrails. |
| `cargo test -p legion-app --test manual_zero_egress` | Pass | Phase 10 rerun passed: 1 passed, 0 failed; app-level Manual open/edit/save/search zero-egress smoke remains green. |
| `cargo test -p legion-app --test settings` | Pass | Phase 10 rerun passed: 2 passed, 0 failed; settings projection and command-palette settings dispatch remain green after font/wrapping settings additions. |
| `cargo run -p xtask -- check-deps` | Pass | Phase 10 rerun passed; dependency policy checks passed. |
| `cargo run -p xtask -- docs-hygiene` | Pass | Phase 10 rerun passed; documentation hygiene checks passed. |
| `cargo fmt --all --check` | Pass | Phase 10 rerun passed. |
| `cargo check --workspace --all-targets` | Pass | Phase 10 rerun passed for the workspace. |
| `cargo test --workspace --all-targets --no-fail-fast -j 1` | Pass | Default-concurrency attempt failed during Windows MSVC linking with `LNK1102: out of memory`; single-job rerun passed all workspace targets. |
| `cargo clippy --workspace --all-targets -j 1 -- -D warnings` | Pass | Phase 10 rerun passed with single-job concurrency to avoid the same Windows linker/resource pressure. |
| `git diff --check` | Pass | Phase 10 rerun passed after the final evidence update. |

## Product-Readiness Decision

`PR-UI-001` remains bounded by the evidence above. Do not mark it product-workflow validated unless all required Manual input, focus, accessibility, renderer-backed performance, platform, and zero-egress checks pass in the current tree.

## Residual Risk

- Native OS IME, clipboard, focus, high-DPI, and accessibility evidence must name the host OS where it was observed.
- Renderer-backed perf can still be blocked on machines without a native window or GPU path; blocked runs must be recorded as blocked, not passed.
- WS-MANUAL-02 owns full large-workspace and 100MB streaming performance; WS-MANUAL-01 only improves visible capability reduction and renderer/input evidence.

## Deterministic Renderer Evidence

Core Manual editor states are represented by `DesktopProjectionViewModel::deterministic_editor_evidence()`. The evidence rows are textual, stable, and metadata-only: title, editor status, viewport metadata, flags, code-line lengths, truncation state, and large-file capability rows. They do not persist raw source or full clipboard/IME payloads.
