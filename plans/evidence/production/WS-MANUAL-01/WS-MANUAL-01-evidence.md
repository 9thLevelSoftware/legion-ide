# WS-MANUAL-01 Evidence

Date: 2026-06-19
Scope: Manual editor feel, rendering, input, focus, font, wrapping, degraded-mode messaging, deterministic renderer evidence, and zero-egress smoke.

## Branch State

- Branch: `codex/ws-manual-01`
- Starting dirty files: none (`git status --short --branch` showed only the branch line).
- Ending dirty files: `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md` before the Phase 0 evidence commits; none expected after the latest Phase 0 evidence commit.

Current branch history already contains `a6666ff docs: add WS-MANUAL-01 implementation plan`, `7ca090b test: fix Windows baseline portability`, and `84fce4c docs: seed WS-MANUAL-01 evidence`.

## Workstream Coverage

| Master-plan task | Planned evidence / target check |
| --- | --- |
| MANUAL.01 latency budgets | To be created in MANUAL.01: `plans/evidence/production/WS-MANUAL-01/editor-latency-budgets.md` |
| MANUAL.02 renderer-backed input-to-paint | To be created in MANUAL.02: `target/perf-harness/perf_report.toml`; `cargo run -p xtask -- perf-harness`; `cargo run -p xtask -- verify-perf-harness` |
| MANUAL.03 custom editor path / no TextEdit | `cargo run -p xtask -- no-egui-textedit`; `cargo test -p xtask --test no_egui_textedit` |
| MANUAL.04 IME smoke | To be created in MANUAL.04: `cargo test -p legion-desktop --test manual_input_conformance ime_composition_suppresses_shortcuts_and_commits_text -- --exact` |
| MANUAL.05 clipboard tests | To be created in MANUAL.05: `cargo test -p legion-desktop --test manual_input_conformance clipboard_copy_cut_paste_select_all_round_trips_through_app_authority -- --exact` |
| MANUAL.06 multi-cursor / rectangular selection | Covered for v1 Manual scope by the editor projection substrate test; rectangular selection is explicitly deferred. See MANUAL.06 Decision below. |
| MANUAL.07 keyboard focus | To be created in MANUAL.07: `cargo test -p legion-desktop --test manual_input_conformance manual_focus_routes_text_to_active_surface_only -- --exact` |
| MANUAL.08 font fallback diagnostics | To be created in MANUAL.08: `cargo test -p legion-desktop --test manual_renderer_evidence font_fallback_diagnostics_are_projected_without_raw_font_paths -- --exact` |
| MANUAL.09 line wrapping policy | To be created in MANUAL.09: `cargo test -p legion-desktop --test manual_renderer_evidence line_wrapping_policy_keeps_viewport_math_stable -- --exact` |
| MANUAL.10 degraded-mode banner | To be created in MANUAL.10: `cargo test -p legion-desktop --test large_file_guardrails large_file_guardrails_degraded_banner_names_capability_reduction -- --exact` |
| MANUAL.11 deterministic renderer evidence | To be created in MANUAL.11: `cargo test -p legion-desktop --test manual_renderer_evidence deterministic_renderer_evidence_covers_core_editor_states -- --exact` |
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
| `cargo test -p xtask --test perf_harness` | Pass | WS-MANUAL-01 perf-harness phase added Manual renderer budget/report coverage in `xtask/tests/perf_harness.rs`; Phase 10 will re-run the final current-tree target. |
| `cargo run -p xtask -- perf-harness` | Pass | Generated `target/perf-harness/perf_report.toml` includes `manual.renderer_input_to_paint` with 3 passed, 0 failed, 0 skipped at git `787b7030c3a622d55625fbcb1233cc768a56901c`. |
| `cargo run -p xtask -- verify-perf-harness` | Pass | MANUAL.02 perf-harness phase verified the generated report; Phase 10 will re-run this against the final tree. |
| `cargo run -p xtask -- no-egui-textedit` | Pass | MANUAL.03 keeps the custom Manual editor path covered by the no-`egui::TextEdit` xtask gate and companion test. |
| `cargo test -p legion-desktop --test manual_perf` | Pass | MANUAL.02 added the renderer-backed Manual edit perf test and metadata-only report path in `crates/legion-desktop/tests/manual_perf.rs`. |
| `cargo test -p legion-desktop --test manual_input_conformance` | Pass | MANUAL.04, MANUAL.05, MANUAL.06, and MANUAL.07 are covered by Manual focus, IME, clipboard, copy/cut composition suppression, and selection-scope tests. |
| `cargo test -p legion-desktop --test manual_renderer_evidence` | Pass | MANUAL.08, MANUAL.09, MANUAL.11, and the renderer zero-egress trust-boundary check are covered by `manual_renderer_evidence`. |
| `cargo test -p legion-desktop --test large_file_guardrails large_file_guardrails_degraded_banner_names_capability_reduction -- --exact` | Pass | MANUAL.10 degraded-mode banner evidence is covered by the focused large-file guardrail test. |
| `cargo test -p legion-app --test manual_zero_egress` | Pass | MANUAL.12 app-level open/edit/save/search zero-egress smoke is recorded in `manual-mode-zero-egress.md` and strengthened by commits `98f0b6b` and `64b3a85`. |
| `cargo run -p xtask -- check-deps` | Blocked | Pending final Phase 10 sweep; not claimed by this docs-only evidence closure. |
| `cargo run -p xtask -- docs-hygiene` | Pass | Phase 9 docs-hygiene command passed after the ledger/evidence updates. |
| `cargo fmt --all --check` | Blocked | Pending final Phase 10 sweep; not claimed by this docs-only evidence closure. |
| `cargo check --workspace --all-targets` | Blocked | Pending final Phase 10 sweep; not claimed by this docs-only evidence closure. |
| `cargo test --workspace --all-targets --no-fail-fast` | Blocked | Pending final Phase 10 sweep; not claimed by this docs-only evidence closure. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Blocked | Pending final Phase 10 sweep; not claimed by this docs-only evidence closure. |
| `git diff --check` | Pass | Phase 9 whitespace check passed after the docs updates. |

## Product-Readiness Decision

`PR-UI-001` remains bounded by the evidence above. Do not mark it product-workflow validated unless all required Manual input, focus, accessibility, renderer-backed performance, platform, and zero-egress checks pass in the current tree.

## Residual Risk

- Native OS IME, clipboard, focus, high-DPI, and accessibility evidence must name the host OS where it was observed.
- Renderer-backed perf can still be blocked on machines without a native window or GPU path; blocked runs must be recorded as blocked, not passed.
- WS-MANUAL-02 owns full large-workspace and 100MB streaming performance; WS-MANUAL-01 only improves visible capability reduction and renderer/input evidence.

## Deterministic Renderer Evidence

Core Manual editor states are represented by `DesktopProjectionViewModel::deterministic_editor_evidence()`. The evidence rows are textual, stable, and metadata-only: title, editor status, viewport metadata, flags, code-line lengths, truncation state, and large-file capability rows. They do not persist raw source or full clipboard/IME payloads.
