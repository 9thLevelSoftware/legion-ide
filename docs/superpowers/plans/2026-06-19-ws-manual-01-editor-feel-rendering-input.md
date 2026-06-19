# WS-MANUAL-01 Editor Feel, Rendering, and Input Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete WS-MANUAL-01 from `plans/legion-production-master-plan-v0.2.md` by turning Manual mode into a measurable, renderer-backed, input-safe native IDE path with latency budgets, keyboard/IME/clipboard/focus coverage, font and wrapping policy, large-file capability messaging, deterministic renderer evidence, and zero-egress smoke.

**Architecture:** Keep the existing authority split: `legion-ui` remains projection-only and emits typed `CommandDispatchIntent`; `legion-app` owns editor/workspace commands and Manual-mode policy; `legion-editor` and `legion-text` own buffer, viewport, cursor, selection, degraded-mode, and line-slice behavior; `legion-desktop` owns egui/eframe rendering, native adapter observations, and renderer evidence. `xtask` must not take a direct dependency on `legion-desktop`; it should launch a desktop-owned manual perf command as a subprocess and fold the metadata-only result into the existing perf report shape.

**Tech Stack:** Rust 2024 workspace, `legion-app`, `legion-desktop`, `legion-editor`, `legion-text`, `legion-ui`, `legion-protocol`, `xtask`, eframe/egui 0.34.2, Markdown evidence under `plans/evidence/production/WS-MANUAL-01/`, targeted Rust integration tests, `cargo run -p xtask -- perf-harness`, `cargo run -p xtask -- verify-perf-harness`, `cargo run -p xtask -- no-egui-textedit`, and the repo-standard Rust gates.

---

## Current Branch Facts to Preserve

- `plans/legion-production-master-plan-v0.2.md` defines WS-MANUAL-01 at lines 276-306.
- `crates/legion-desktop/src/view.rs` already renders Manual mode through `render_editor_canvas`, `render_code_lines`, `DesktopProjectionViewModel`, `DesktopCodeLineViewModel`, and cached egui galleys.
- `crates/legion-desktop/src/workflow.rs` already routes keyboard, paste, IME commit, cursor movement, selection, palette, save, search, undo, redo, and tab actions through `DesktopAction` and `DesktopRuntime::handle_action`.
- `crates/legion-desktop/src/metrics.rs` already contains `FrameTimingRecorder` and `FrameTimingSummary`.
- `crates/legion-desktop/src/smoke.rs` already writes metadata-only renderer/platform smoke evidence and records first input-to-paint timing.
- `xtask/src/perf_harness.rs` still owns the current `perf_report.toml` shape and includes synthetic input-to-paint and line-galley gates.
- `xtask/Cargo.toml` currently has no internal crate dependencies. Preserve that by spawning `cargo run -p legion-desktop` for renderer-backed Manual measurements instead of importing desktop code into `xtask`.
- `xtask/no-egui-textedit.toml` and `xtask/src/no_egui_textedit.rs` already guard the custom editor render path against `egui::TextEdit`.
- `crates/legion-desktop/src/theme.rs` already attempts host OS CJK fallback loading, but it does not surface structured fallback diagnostics to projections or evidence.
- `SettingsProjection` exposes editor font size and editor toggles, but it does not yet expose font family, font fallback diagnostics, or explicit line wrapping policy.
- `ViewportProjection` has dimensions, line slices, metrics, scroll, truncation state, and large-file status, but no explicit wrapping policy field.
- `plans/product-readiness-ledger.md` currently keeps `PR-UI-001` at substrate validated. WS-MANUAL-01 evidence may support movement toward product-workflow validation only after real renderer-backed input, focus, accessibility, and platform evidence is recorded.

## Files to Create

- `plans/evidence/production/WS-MANUAL-01/editor-latency-budgets.md`
- `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md`
- `plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md`
- `crates/legion-desktop/src/manual_perf.rs`
- `crates/legion-desktop/tests/manual_perf.rs`
- `crates/legion-desktop/tests/manual_input_conformance.rs`
- `crates/legion-desktop/tests/manual_renderer_evidence.rs`
- `crates/legion-app/tests/manual_zero_egress.rs`

## Files to Modify

- `crates/legion-protocol/src/lib.rs`
- `crates/legion-ui/src/ui.rs`
- `crates/legion-app/src/lib.rs`
- `crates/legion-app/tests/settings.rs`
- `crates/legion-desktop/Cargo.toml`
- `crates/legion-desktop/src/lib.rs`
- `crates/legion-desktop/src/workflow.rs`
- `crates/legion-desktop/src/smoke.rs`
- `crates/legion-desktop/src/theme.rs`
- `crates/legion-desktop/src/view.rs`
- `crates/legion-desktop/tests/projection_rendering.rs`
- `crates/legion-desktop/tests/daily_editing_controls.rs`
- `crates/legion-desktop/tests/large_file_guardrails.rs`
- `xtask/src/perf_harness.rs`
- `xtask/src/main.rs`
- `xtask/tests/perf_harness.rs`
- `plans/product-readiness-ledger.md`

## Non-Goals

- Do not put editor text ownership, editor sessions, workspace mutation, provider routing, or terminal authority into `legion-ui` or `legion-desktop`.
- Do not replace the custom code-canvas path with `egui::TextEdit`.
- Do not promote `PR-UI-001` to product-workflow validated until the evidence file records passing renderer-backed Manual input/focus/accessibility/platform checks.
- Do not add new hosted egress, network permissions, cloud provider startup, telemetry export, or raw-source diagnostics to Manual mode.
- Do not solve WS-MANUAL-02 100MB streaming performance in this workstream. WS-MANUAL-01 may improve the visible degraded-mode banner and capability messaging only.

---

## Phase 0 - Baseline and Workstream Mapping

- [ ] Run:
  ```powershell
  git status --short --branch
  rg -n "WS-MANUAL-01|MANUAL\\.0|PR-UI-001" plans/legion-production-master-plan-v0.2.md plans/product-readiness-ledger.md
  rg -n "TextEdit|CodeCanvas|manual editor|FrameTimingRecorder|perf-harness|zero-egress|egress" crates xtask plans -g "*.rs" -g "*.md" -g "*.toml"
  ```
  Expected: clean or consciously recorded branch state; WS-MANUAL-01 task lines visible; existing renderer, timing, `no-egui-textedit`, and Manual trust rows visible.

- [ ] Create `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md` with this initial body:
  ```markdown
  # WS-MANUAL-01 Evidence

  Date: 2026-06-19
  Scope: Manual editor feel, rendering, input, focus, font, wrapping, degraded-mode messaging, deterministic renderer evidence, and zero-egress smoke.

  ## Branch State

  - Branch:
  - Starting dirty files:
  - Ending dirty files:

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
  ```

- [ ] Fill the Branch State rows immediately after the first `git status --short --branch` run. Leave Verification rows empty until commands have been run, then fill every result cell before commit.

- [ ] Commit after Phase 0 if desired:
  ```powershell
  git add plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md
  git commit -m "docs: seed WS-MANUAL-01 evidence"
  ```

---

## Phase 1 - MANUAL.01 Latency Budgets

### Task 1.1: Define Manual editor budgets as an evidence-controlled contract

**Files:**
- Create: `plans/evidence/production/WS-MANUAL-01/editor-latency-budgets.md`
- Modify: `xtask/src/perf_harness.rs`
- Modify: `xtask/tests/perf_harness.rs`

- [ ] Write `plans/evidence/production/WS-MANUAL-01/editor-latency-budgets.md`:
  ```markdown
  # WS-MANUAL-01 Editor Latency Budgets

  Date: 2026-06-19
  Scope: Manual-mode daily editing on a trusted local workspace.

  ## Budget Table

  | Interaction | Metric | Budget | Gate owner | Evidence |
  | --- | --- | --- | --- | --- |
  | Keypress to paint, normal buffer | p50 | <= 16 ms | `xtask perf-harness` renderer-backed Manual measurement | `target/perf-harness/perf_report.toml` |
  | Keypress to paint, normal buffer | p95 | <= 32 ms | `xtask perf-harness` renderer-backed Manual measurement | `target/perf-harness/perf_report.toml` |
  | Scroll to paint, normal buffer | p95 | <= 32 ms | desktop manual perf scenario | `target/perf-harness/perf_report.toml` |
  | Open 1 MiB file | total | <= 250 ms | app/editor integration test plus perf report | `crates/legion-app/tests/daily_editing_contracts.rs` |
  | Save normal buffer | total | <= 250 ms | app save workflow integration test | `crates/legion-app/tests/workspace_vfs_integration.rs` |
  | Active-file search | total | <= 100 ms | app search integration test | `crates/legion-app/tests/daily_editing_search.rs` |
  | LSP completion projection | p95 | <= 100 ms | language tooling workflow test | `crates/legion-app/tests/language_tooling_workflow.rs` |

  ## Enforcement Rules

  - A budget can be report-only only when the evidence row records the blocker and the workstream remains open.
  - A budget is green only when the current-tree command listed in the Evidence column passes.
  - Manual-mode measurements must be metadata-only. Do not persist raw source, clipboard text, IME composition text, or full buffer contents in evidence files.
  - Renderer-backed measurements must exercise `legion-desktop` rendering and app/editor routing, not only synthetic `xtask` loops.
  - Large-file degraded-mode measurements remain bounded by WS-MANUAL-02 unless this workstream explicitly names a visible Manual-mode capability reduction.
  ```

- [ ] Add constants to `xtask/src/perf_harness.rs` near the existing budget constants:
  ```rust
  const MANUAL_RENDERER_KEYPRESS_P50_BUDGET_MILLIS: u64 = 16;
  const MANUAL_RENDERER_KEYPRESS_P95_BUDGET_MILLIS: u64 = 32;
  const MANUAL_RENDERER_SCROLL_P95_BUDGET_MILLIS: u64 = 32;
  const MANUAL_RENDERER_SAMPLE_COUNT: usize = 16;
  pub const MANUAL_RENDERER_PERF_REPORT_FILE: &str = "manual_renderer_perf.toml";
  ```

- [ ] Add this test to `xtask/tests/perf_harness.rs` before the git SHA test:
  ```rust
  #[test]
  fn perf_harness_manual_renderer_budget_constants_match_ws_manual_01() {
      assert_eq!(xtask::perf_harness::MANUAL_RENDERER_PERF_REPORT_FILE, "manual_renderer_perf.toml");
      let budgets = xtask::perf_harness::manual_renderer_budgets();
      assert_eq!(budgets.keypress_p50_millis, 16);
      assert_eq!(budgets.keypress_p95_millis, 32);
      assert_eq!(budgets.scroll_p95_millis, 32);
      assert_eq!(budgets.sample_count, 16);
  }
  ```

- [ ] Add the public budget DTO and function to `xtask/src/perf_harness.rs`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct ManualRendererBudgets {
      pub keypress_p50_millis: u64,
      pub keypress_p95_millis: u64,
      pub scroll_p95_millis: u64,
      pub sample_count: usize,
  }

  pub fn manual_renderer_budgets() -> ManualRendererBudgets {
      ManualRendererBudgets {
          keypress_p50_millis: MANUAL_RENDERER_KEYPRESS_P50_BUDGET_MILLIS,
          keypress_p95_millis: MANUAL_RENDERER_KEYPRESS_P95_BUDGET_MILLIS,
          scroll_p95_millis: MANUAL_RENDERER_SCROLL_P95_BUDGET_MILLIS,
          sample_count: MANUAL_RENDERER_SAMPLE_COUNT,
      }
  }
  ```

- [ ] Run:
  ```powershell
  cargo test -p xtask --test perf_harness perf_harness_manual_renderer_budget_constants_match_ws_manual_01 -- --exact
  cargo fmt --all --check
  ```
  Expected: targeted test passes after the DTO is added; formatting passes.

- [ ] Commit:
  ```powershell
  git add plans/evidence/production/WS-MANUAL-01/editor-latency-budgets.md xtask/src/perf_harness.rs xtask/tests/perf_harness.rs
  git commit -m "perf: define manual editor latency budgets"
  ```

---

## Phase 2 - MANUAL.02 Renderer-Backed Perf Harness

### Task 2.1: Add desktop-owned Manual perf report writer

**Files:**
- Create: `crates/legion-desktop/src/manual_perf.rs`
- Modify: `crates/legion-desktop/src/lib.rs`
- Modify: `crates/legion-desktop/src/workflow.rs`
- Create: `crates/legion-desktop/tests/manual_perf.rs`

- [ ] Create `crates/legion-desktop/src/manual_perf.rs` with this API surface:
  ```rust
  //! Renderer-backed Manual-mode performance harness.

  use std::{
      fs,
      path::{Path, PathBuf},
      time::{Duration, Instant},
  };

  use anyhow::{Result, anyhow};
  use legion_protocol::{BufferId, TextCoordinate, ViewportScroll};

  use crate::{
      bridge::DesktopAction,
      metrics::FrameTimingRecorder,
      view::ProjectionView,
      workflow::{DesktopLaunchConfig, DesktopRuntime},
  };

  pub const MANUAL_PERF_SCHEMA_VERSION: u32 = 1;
  pub const MANUAL_PERF_SCENARIO: &str = "manual.renderer_input_to_paint";

  #[derive(Debug, Clone, PartialEq)]
  pub struct ManualPerfConfig {
      pub workspace_root: PathBuf,
      pub initial_file: Option<String>,
      pub report_path: PathBuf,
      pub sample_count: usize,
      pub keypress_p50_budget_ms: u64,
      pub keypress_p95_budget_ms: u64,
      pub scroll_p95_budget_ms: u64,
  }

  #[derive(Debug, Clone, PartialEq)]
  pub struct ManualPerfReport {
      pub schema_version: u32,
      pub scenario: String,
      pub status: String,
      pub sample_count: usize,
      pub keypress_p50_micros: u64,
      pub keypress_p95_micros: u64,
      pub scroll_p95_micros: u64,
      pub keypress_p50_budget_ms: u64,
      pub keypress_p95_budget_ms: u64,
      pub scroll_p95_budget_ms: u64,
      pub message: String,
  }

  impl ManualPerfReport {
      pub fn to_toml(&self) -> String {
          format!(
              concat!(
                  "schema_version = {schema_version}\n",
                  "scenario = \"{scenario}\"\n",
                  "status = \"{status}\"\n",
                  "sample_count = {sample_count}\n",
                  "keypress_p50_micros = {keypress_p50_micros}\n",
                  "keypress_p95_micros = {keypress_p95_micros}\n",
                  "scroll_p95_micros = {scroll_p95_micros}\n",
                  "keypress_p50_budget_ms = {keypress_p50_budget_ms}\n",
                  "keypress_p95_budget_ms = {keypress_p95_budget_ms}\n",
                  "scroll_p95_budget_ms = {scroll_p95_budget_ms}\n",
                  "message = \"{message}\"\n"
              ),
              schema_version = self.schema_version,
              scenario = escape_toml_string(&self.scenario),
              status = escape_toml_string(&self.status),
              sample_count = self.sample_count,
              keypress_p50_micros = self.keypress_p50_micros,
              keypress_p95_micros = self.keypress_p95_micros,
              scroll_p95_micros = self.scroll_p95_micros,
              keypress_p50_budget_ms = self.keypress_p50_budget_ms,
              keypress_p95_budget_ms = self.keypress_p95_budget_ms,
              scroll_p95_budget_ms = self.scroll_p95_budget_ms,
              message = escape_toml_string(&self.message),
          )
      }

      pub fn write(&self, path: &Path) -> Result<()> {
          if let Some(parent) = path.parent()
              && !parent.as_os_str().is_empty()
          {
              fs::create_dir_all(parent)?;
          }
          fs::write(path, self.to_toml())?;
          Ok(())
      }
  }

  pub fn run_manual_perf(config: ManualPerfConfig) -> Result<ManualPerfReport> {
      if config.sample_count == 0 {
          return Err(anyhow!("manual perf sample count must be greater than zero"));
      }
      let launch = DesktopLaunchConfig::new(config.workspace_root.clone(), config.initial_file.clone());
      let mut runtime = DesktopRuntime::open(launch)?;
      let mut view = ProjectionView::new();
      let mut keypress_samples = Vec::with_capacity(config.sample_count);
      let mut scroll_samples = Vec::with_capacity(config.sample_count);

      for sample in 0..config.sample_count {
          let snapshot = runtime.projection_snapshot();
          let buffer_id = active_buffer(&snapshot)?;
          let cursor = projected_cursor(&snapshot);
          let started = Instant::now();
          runtime.handle_action(DesktopAction::InsertText {
              text: "x".to_string(),
              at: cursor,
          })?;
          render_once(&mut view, &runtime);
          keypress_samples.push(started.elapsed());

          let scroll_started = Instant::now();
          runtime.handle_action(DesktopAction::SetViewportScroll {
              buffer_id: Some(buffer_id),
              scroll: ViewportScroll {
                  top_line: sample as u32,
                  left_column: 0,
              },
          })?;
          render_once(&mut view, &runtime);
          scroll_samples.push(scroll_started.elapsed());
      }

      let keypress_p50 = percentile_micros(&mut keypress_samples, 0.50);
      let keypress_p95 = percentile_micros(&mut keypress_samples, 0.95);
      let scroll_p95 = percentile_micros(&mut scroll_samples, 0.95);
      let passed = keypress_p50 <= config.keypress_p50_budget_ms * 1_000
          && keypress_p95 <= config.keypress_p95_budget_ms * 1_000
          && scroll_p95 <= config.scroll_p95_budget_ms * 1_000;
      let report = ManualPerfReport {
          schema_version: MANUAL_PERF_SCHEMA_VERSION,
          scenario: MANUAL_PERF_SCENARIO.to_string(),
          status: if passed { "passed" } else { "failed" }.to_string(),
          sample_count: config.sample_count,
          keypress_p50_micros: keypress_p50,
          keypress_p95_micros: keypress_p95,
          scroll_p95_micros: scroll_p95,
          keypress_p50_budget_ms: config.keypress_p50_budget_ms,
          keypress_p95_budget_ms: config.keypress_p95_budget_ms,
          scroll_p95_budget_ms: config.scroll_p95_budget_ms,
          message: format!(
              "manual renderer p50={}us p95={}us scroll_p95={}us",
              keypress_p50, keypress_p95, scroll_p95
          ),
      };
      report.write(&config.report_path)?;
      Ok(report)
  }
  ```

- [ ] In the same file, add these helpers below `run_manual_perf`. Keep `render_once` intentionally small so it can be replaced by native-window sampling later without changing the report shape:
  ```rust
  fn render_once(view: &mut ProjectionView, runtime: &DesktopRuntime) {
      let snapshot = runtime.projection_snapshot();
      egui::__run_test_ui(|ui| {
          let _ = view.render(ui, &snapshot);
      });
  }

  fn active_buffer(snapshot: &legion_ui::ShellProjectionSnapshot) -> Result<BufferId> {
      snapshot
          .active_buffer_projection
          .buffer_id
          .ok_or_else(|| anyhow!("manual perf requires an active buffer"))
  }

  fn projected_cursor(snapshot: &legion_ui::ShellProjectionSnapshot) -> TextCoordinate {
      snapshot
          .active_buffer_projection
          .viewport
          .as_ref()
          .map(|viewport| viewport.cursor)
          .unwrap_or(TextCoordinate {
              line: 0,
              character: 0,
              byte_offset: Some(0),
              utf16_offset: Some(0),
          })
  }

  fn percentile_micros(samples: &mut [Duration], pct: f64) -> u64 {
      samples.sort();
      let idx = ((samples.len() as f64 - 1.0) * pct).round() as usize;
      samples[idx].as_micros() as u64
  }

  fn escape_toml_string(value: &str) -> String {
      value.replace('\\', "\\\\").replace('"', "\\\"")
  }
  ```

- [ ] If `egui::__run_test_ui` is not exported outside test builds on this dependency version, replace `render_once` with a native-window `eframe::run_native` loop modeled after `crates/legion-desktop/src/smoke.rs::run_smoke_window`, and record blocked status when the native window cannot open. Keep the public `ManualPerfReport` fields identical.

- [ ] Modify `crates/legion-desktop/src/lib.rs`:
  ```rust
  pub mod manual_perf;
  ```

- [ ] Modify `crates/legion-desktop/src/workflow.rs`:
  - Add `manual_perf: Option<crate::manual_perf::ManualPerfConfig>` to `DesktopLaunchConfig`.
  - Parse:
    - `--manual-perf`
    - `--perf-report <path>`
    - `--perf-samples <usize>`
  - Build the default manual perf config with:
    - `report_path`: `target/perf-harness/manual_renderer_perf.toml`
    - `sample_count`: `16`
    - `keypress_p50_budget_ms`: `16`
    - `keypress_p95_budget_ms`: `32`
    - `scroll_p95_budget_ms`: `32`
  - In `run_from_env`, before smoke and normal window launch, route `Some(config.manual_perf)` to `crate::manual_perf::run_manual_perf`.

- [ ] Add `crates/legion-desktop/tests/manual_perf.rs`:
  ```rust
  use std::{fs, sync::atomic::{AtomicU64, Ordering}};

  use legion_desktop::manual_perf::{ManualPerfConfig, MANUAL_PERF_SCENARIO, run_manual_perf};

  struct TempWorkspace {
      root: std::path::PathBuf,
  }

  impl TempWorkspace {
      fn new() -> Self {
          static COUNTER: AtomicU64 = AtomicU64::new(0);
          let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
          let root = std::env::temp_dir().join(format!("legion-manual-perf-{seq}"));
          fs::create_dir_all(&root).expect("create temp workspace");
          fs::write(root.join("main.rs"), "fn main() {\n    println!(\"hi\");\n}\n")
              .expect("write source");
          Self { root }
      }

      fn path(&self) -> &std::path::Path {
          &self.root
      }
  }

  impl Drop for TempWorkspace {
      fn drop(&mut self) {
          let _ = fs::remove_dir_all(&self.root);
      }
  }

  #[test]
  fn manual_perf_runs_renderer_backed_edit_and_writes_metadata_report() {
      let workspace = TempWorkspace::new();
      let report_path = workspace.path().join("manual_renderer_perf.toml");

      let report = run_manual_perf(ManualPerfConfig {
          workspace_root: workspace.path().to_path_buf(),
          initial_file: Some("main.rs".to_string()),
          report_path: report_path.clone(),
          sample_count: 2,
          keypress_p50_budget_ms: 5_000,
          keypress_p95_budget_ms: 5_000,
          scroll_p95_budget_ms: 5_000,
      })
      .expect("manual perf should run");

      assert_eq!(report.scenario, MANUAL_PERF_SCENARIO);
      assert_eq!(report.status, "passed");
      assert_eq!(report.sample_count, 2);
      assert!(report.keypress_p50_micros <= report.keypress_p95_micros);
      assert!(report.scroll_p95_micros > 0);

      let text = fs::read_to_string(report_path).expect("report should be written");
      assert!(text.contains("scenario = \"manual.renderer_input_to_paint\""));
      assert!(text.contains("status = \"passed\""));
      assert!(!text.contains("println!"));
      assert!(!text.contains("fn main"));
  }
  ```

- [ ] Run:
  ```powershell
  cargo test -p legion-desktop --test manual_perf manual_perf_runs_renderer_backed_edit_and_writes_metadata_report -- --exact
  ```
  Expected: pass. If it fails because `egui::__run_test_ui` is unavailable outside tests, switch to the native-window variant described above and rerun.

- [ ] Commit:
  ```powershell
  git add crates/legion-desktop/src/manual_perf.rs crates/legion-desktop/src/lib.rs crates/legion-desktop/src/workflow.rs crates/legion-desktop/tests/manual_perf.rs
  git commit -m "perf: add renderer-backed manual editor measurement"
  ```

### Task 2.2: Fold the desktop Manual perf report into `xtask perf-harness`

**Files:**
- Modify: `xtask/src/perf_harness.rs`
- Modify: `xtask/src/main.rs`
- Modify: `xtask/tests/perf_harness.rs`

- [ ] Add a third skeleton kind to `xtask/src/perf_harness.rs`:
  ```rust
  RendererBackedManualInputToPaint,
  ```
  and extend `SkeletonKind::as_str`:
  ```rust
  Self::RendererBackedManualInputToPaint => "renderer_backed_manual_input_to_paint",
  ```

- [ ] Add DTO and parser to `xtask/src/perf_harness.rs`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
  pub struct ManualRendererPerfToml {
      pub schema_version: u32,
      pub scenario: String,
      pub status: String,
      pub sample_count: usize,
      pub keypress_p50_micros: u64,
      pub keypress_p95_micros: u64,
      pub scroll_p95_micros: u64,
      pub keypress_p50_budget_ms: u64,
      pub keypress_p95_budget_ms: u64,
      pub scroll_p95_budget_ms: u64,
      pub message: String,
  }

  pub fn read_manual_renderer_perf_report(path: &Path) -> Result<ManualRendererPerfToml, String> {
      let text = fs::read_to_string(path).map_err(|err| {
          format!(
              "unable to read manual renderer perf report `{}`: {err}",
              path.display()
          )
      })?;
      toml::from_str(&text).map_err(|err| {
          format!(
              "unable to parse manual renderer perf report `{}`: {err}",
              path.display()
          )
      })
  }

  pub fn manual_renderer_perf_measurement(report: &ManualRendererPerfToml) -> SkeletonMeasurement {
      let status = if report.status == "passed" {
          SkeletonStatus::Passed
      } else {
          SkeletonStatus::Failed
      };
      SkeletonMeasurement {
          name: "manual.renderer_input_to_paint".to_string(),
          kind: SkeletonKind::RendererBackedManualInputToPaint,
          fixture_bytes: 0,
          sample_count: report.sample_count,
          total_micros: report.keypress_p95_micros.saturating_add(report.scroll_p95_micros),
          p50_micros: report.keypress_p50_micros,
          p95_micros: report.keypress_p95_micros.max(report.scroll_p95_micros),
          budget_millis: report.keypress_p95_budget_ms.max(report.scroll_p95_budget_ms),
          status,
          message: report.message.clone(),
      }
  }
  ```

- [ ] Update `plan_perf_harness` to reject this kind if called directly:
  ```rust
  SkeletonKind::RendererBackedManualInputToPaint => {
      return SkeletonMeasurement {
          name: skeleton.name.clone(),
          kind: skeleton.kind,
          fixture_bytes: skeleton.fixture_bytes,
          sample_count: skeleton.sample_count,
          total_micros: 0,
          p50_micros: 0,
          p95_micros: 0,
          budget_millis: skeleton.budget_millis,
          status: SkeletonStatus::Skipped,
          message: "renderer-backed Manual measurement is supplied by legion-desktop subprocess".to_string(),
      };
  }
  ```

- [ ] Modify `xtask/src/main.rs::run_perf_harness_command` to:
  - Plan the two existing synthetic skeletons.
  - Run `cargo run -p legion-desktop --no-default-features --features offline -- --manual-perf --workspace . --file Cargo.toml --perf-report target/perf-harness/manual_renderer_perf.toml --perf-samples 16`.
  - Read `target/perf-harness/manual_renderer_perf.toml`.
  - Append `manual_renderer_perf_measurement(&manual_report)` to the report before writing `target/perf-harness/perf_report.toml`.
  - If the subprocess fails to launch because native rendering is unavailable, append a `Skipped` measurement with message `renderer-backed Manual measurement blocked: <reason>`. If it runs and reports `failed`, keep the measurement failed.

- [ ] Add tests to `xtask/tests/perf_harness.rs`:
  ```rust
  #[test]
  fn manual_renderer_perf_report_maps_to_perf_measurement() {
      let report = xtask::perf_harness::ManualRendererPerfToml {
          schema_version: 1,
          scenario: "manual.renderer_input_to_paint".to_string(),
          status: "passed".to_string(),
          sample_count: 16,
          keypress_p50_micros: 8_000,
          keypress_p95_micros: 21_000,
          scroll_p95_micros: 19_000,
          keypress_p50_budget_ms: 16,
          keypress_p95_budget_ms: 32,
          scroll_p95_budget_ms: 32,
          message: "manual renderer p50=8000us p95=21000us scroll_p95=19000us".to_string(),
      };

      let measurement = xtask::perf_harness::manual_renderer_perf_measurement(&report);

      assert_eq!(measurement.kind, SkeletonKind::RendererBackedManualInputToPaint);
      assert_eq!(measurement.status, SkeletonStatus::Passed);
      assert_eq!(measurement.sample_count, 16);
      assert_eq!(measurement.p50_micros, 8_000);
      assert_eq!(measurement.p95_micros, 21_000);
      assert_eq!(measurement.budget_millis, 32);
  }

  #[test]
  fn manual_renderer_perf_report_failed_status_fails_measurement() {
      let mut report = xtask::perf_harness::ManualRendererPerfToml {
          schema_version: 1,
          scenario: "manual.renderer_input_to_paint".to_string(),
          status: "failed".to_string(),
          sample_count: 16,
          keypress_p50_micros: 20_000,
          keypress_p95_micros: 60_000,
          scroll_p95_micros: 45_000,
          keypress_p50_budget_ms: 16,
          keypress_p95_budget_ms: 32,
          scroll_p95_budget_ms: 32,
          message: "manual renderer exceeded budget".to_string(),
      };
      let measurement = xtask::perf_harness::manual_renderer_perf_measurement(&report);
      assert_eq!(measurement.status, SkeletonStatus::Failed);
      report.status = "passed".to_string();
      assert_eq!(
          xtask::perf_harness::manual_renderer_perf_measurement(&report).status,
          SkeletonStatus::Passed
      );
  }
  ```

- [ ] Run:
  ```powershell
  cargo test -p xtask --test perf_harness
  cargo run -p xtask -- perf-harness
  cargo run -p xtask -- verify-perf-harness
  ```
  Expected: perf tests pass; perf report includes `renderer_backed_manual_input_to_paint`; verify passes if the desktop subprocess passed or was honestly skipped as blocked.

- [ ] Commit:
  ```powershell
  git add xtask/src/perf_harness.rs xtask/src/main.rs xtask/tests/perf_harness.rs
  git commit -m "perf: include manual renderer measurement in perf harness"
  ```

---

## Phase 3 - MANUAL.03, MANUAL.04, MANUAL.05, MANUAL.07 Input and Focus Conformance

### Task 3.1: Add focused desktop input conformance tests

**Files:**
- Create: `crates/legion-desktop/tests/manual_input_conformance.rs`
- Modify: `crates/legion-desktop/src/workflow.rs` only if helper visibility blocks testing
- Modify: `crates/legion-desktop/src/bridge.rs` only if clipboard copy/cut/select-all actions do not exist
- Modify: `crates/legion-ui/src/ui.rs` only if new typed intents are needed
- Modify: `crates/legion-app/src/lib.rs` only if new typed intents are needed

- [ ] First run the existing coverage:
  ```powershell
  cargo test -p legion-desktop --test daily_editing_controls
  cargo test -p legion-desktop --test projection_rendering
  cargo test -p xtask --test no_egui_textedit
  cargo run -p xtask -- no-egui-textedit
  ```
  Expected: existing tests pass before adding new input conformance tests.

- [ ] Create `crates/legion-desktop/tests/manual_input_conformance.rs` with reusable temp workspace helpers:
  ```rust
  use std::{fs, sync::atomic::{AtomicU64, Ordering}};

  use legion_desktop::{
      bridge::DesktopAction,
      workflow::{DesktopLaunchConfig, DesktopRuntime},
  };
  use legion_protocol::{BufferId, ProtocolTextRange, TextCoordinate};

  struct TempWorkspace {
      root: std::path::PathBuf,
  }

  impl TempWorkspace {
      fn new(name: &str, text: &str) -> Self {
          static COUNTER: AtomicU64 = AtomicU64::new(0);
          let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
          let root = std::env::temp_dir().join(format!("legion-manual-input-{name}-{seq}"));
          fs::create_dir_all(&root).expect("create temp workspace");
          fs::write(root.join("main.rs"), text).expect("write source");
          Self { root }
      }

      fn path(&self) -> &std::path::Path {
          &self.root
      }
  }

  impl Drop for TempWorkspace {
      fn drop(&mut self) {
          let _ = fs::remove_dir_all(&self.root);
      }
  }

  fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
      TextCoordinate {
          line,
          character,
          byte_offset: Some(byte_offset),
          utf16_offset: Some(byte_offset),
      }
  }

  fn range(start: TextCoordinate, end: TextCoordinate) -> ProtocolTextRange {
      ProtocolTextRange { start, end }
  }

  fn open_runtime(workspace: &TempWorkspace) -> DesktopRuntime {
      DesktopRuntime::open(DesktopLaunchConfig::new(
          workspace.path().to_path_buf(),
          Some("main.rs".to_string()),
      ))
      .expect("runtime should open")
  }

  fn active_buffer(runtime: &DesktopRuntime) -> BufferId {
      runtime
          .projection_snapshot()
          .active_buffer_projection
          .buffer_id
          .expect("active buffer")
  }
  ```

- [ ] Add IME test:
  ```rust
  #[test]
  fn ime_composition_suppresses_shortcuts_and_commits_text() {
      let workspace = TempWorkspace::new("ime", "fn main() {}\n");
      let mut runtime = open_runtime(&workspace);
      let buffer_id = active_buffer(&runtime);

      runtime
          .handle_action(DesktopAction::ImeCommit {
              text: "漢".to_string(),
              at: coord(0, 0, 0),
          })
          .expect("IME commit should route through app authority");

      let snapshot = runtime.projection_snapshot();
      let text = snapshot
          .active_buffer_projection
          .small_buffer_text()
          .expect("small buffer preview");
      assert!(text.starts_with("漢fn main"));
      assert_eq!(
          snapshot
              .daily_editing_projection
              .viewport_states
              .iter()
              .find(|state| state.buffer_id == buffer_id)
              .and_then(|state| state.cursor)
              .expect("cursor after commit")
              .character,
          1
      );
  }
  ```

- [ ] Add clipboard/select-all test. If `DesktopAction` lacks copy/cut/select-all variants, add these variants to `crates/legion-desktop/src/bridge.rs` and corresponding `CommandDispatchIntent` variants in `crates/legion-ui/src/ui.rs`: `ClipboardCopy`, `ClipboardCut`, and `SelectAll`. In `legion-app`, implement them through app/editor authority so copy returns a metadata-safe `AppCommandOutcome::ClipboardUpdated { byte_len, line_count }`, cut mutates through `Delete`, and select-all stores a full-buffer selection range without placing raw clipboard text in evidence.
  ```rust
  #[test]
  fn clipboard_copy_cut_paste_select_all_round_trips_through_app_authority() {
      let workspace = TempWorkspace::new("clipboard", "alpha\nbeta\n");
      let mut runtime = open_runtime(&workspace);
      let buffer_id = active_buffer(&runtime);

      runtime
          .handle_action(DesktopAction::SetSelection {
              buffer_id: Some(buffer_id),
              range: range(coord(0, 0, 0), coord(0, 5, 5)),
          })
          .expect("selection should route");
      runtime
          .handle_action(DesktopAction::ClipboardCopy)
          .expect("copy should route");
      runtime
          .handle_action(DesktopAction::ClipboardCut)
          .expect("cut should route");
      runtime
          .handle_action(DesktopAction::ClipboardPaste {
              text: "alpha".to_string(),
              at: coord(1, 0, 1),
          })
          .expect("paste should route");
      runtime
          .handle_action(DesktopAction::SelectAll { buffer_id: Some(buffer_id) })
          .expect("select all should route");

      let snapshot = runtime.projection_snapshot();
      let text = snapshot
          .active_buffer_projection
          .small_buffer_text()
          .expect("small buffer preview");
      assert!(text.contains("alpha"));
      assert!(snapshot
          .daily_editing_projection
          .viewport_states
          .iter()
          .any(|state| state.buffer_id == buffer_id && !state.selections.is_empty()));
  }
  ```

- [ ] Add focus-routing test:
  ```rust
  #[test]
  fn manual_focus_routes_text_to_active_surface_only() {
      let workspace = TempWorkspace::new("focus", "fn main() {}\n");
      let mut runtime = open_runtime(&workspace);

      runtime
          .handle_action(DesktopAction::OpenPalette {
              mode: legion_ui::PaletteMode::File,
              query: String::new(),
              scope: legion_ui::SearchScopeProjection::ActiveFile,
          })
          .expect("open palette");
      runtime
          .handle_action(DesktopAction::InsertText {
              text: "x".to_string(),
              at: coord(0, 0, 0),
          })
          .expect("focused palette should not mutate editor text");

      let snapshot = runtime.projection_snapshot();
      let text = snapshot
          .active_buffer_projection
          .small_buffer_text()
          .expect("small buffer preview");
      assert_eq!(text, "fn main() {}\n");
      assert!(snapshot.palette_projection.open);
  }
  ```

- [ ] Run:
  ```powershell
  cargo test -p legion-desktop --test manual_input_conformance
  cargo run -p xtask -- no-egui-textedit
  ```
  Expected: tests pass and the no-TextEdit guard remains green.

- [ ] Commit:
  ```powershell
  git add crates/legion-desktop/tests/manual_input_conformance.rs crates/legion-desktop/src/workflow.rs crates/legion-desktop/src/bridge.rs crates/legion-ui/src/ui.rs crates/legion-app/src/lib.rs
  git commit -m "test: cover manual editor input conformance"
  ```

### Task 3.2: Record multi-cursor and rectangular selection decision

**Files:**
- Modify: `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md`
- Modify: `crates/legion-editor/src/lib.rs`
- Modify: `crates/legion-desktop/tests/manual_input_conformance.rs`

- [ ] Verify current multi-cursor substrate:
  ```powershell
  rg -n "engine_preserves_multiple_cursors_and_selections_in_projection|struct Cursor|struct Selection|set_cursors|set_selections" crates/legion-editor/src/lib.rs
  ```
  Expected: editor already preserves multiple cursors and selections in projection.

- [ ] Add or keep this editor-level test if it is not already equivalent:
  ```rust
  #[test]
  fn engine_preserves_multiple_cursors_and_selections_in_projection() {
      let mut engine = EditorEngine::new();
      let buffer = engine
          .open_buffer(WorkspaceId(1), FileId(1), "src/multi.rs", "one\ntwo\nthree\n".to_string())
          .unwrap();
      engine
          .set_cursors(
              buffer,
              vec![
                  Cursor { position: TextPosition::new(0, 0) },
                  Cursor { position: TextPosition::new(1, 0) },
              ],
          )
          .unwrap();
      engine
          .set_selections(
              buffer,
              vec![
                  Selection { anchor: TextPosition::new(0, 0), active: TextPosition::new(0, 3) },
                  Selection { anchor: TextPosition::new(1, 0), active: TextPosition::new(1, 3) },
              ],
          )
          .unwrap();

      let projection = engine
          .viewport_projection(buffer, ViewportScroll { top_line: 0, left_column: 0 })
          .unwrap();
      assert_eq!(projection.selections.len(), 2);
  }
  ```

- [ ] Add this decision row under `## Product-Readiness Decision` in `WS-MANUAL-01-evidence.md`:
  ```markdown
  ## Multi-Cursor and Rectangular Selection Decision

  Multi-cursor substrate is in scope for v1 Manual mode and must remain covered by editor projection tests. Rectangular selection is intentionally out of the v1 product-workflow gate until the editor exposes a rectangular selection command with stable protocol DTOs, keyboard/mouse gestures, and renderer evidence. The v1 Manual-mode UI must not advertise rectangular selection as complete.
  ```

- [ ] Run:
  ```powershell
  cargo test -p legion-editor engine_preserves_multiple_cursors_and_selections_in_projection -- --exact
  cargo test -p legion-desktop --test manual_input_conformance
  ```
  Expected: multi-cursor substrate stays green; rectangular selection is explicitly not promoted.

- [ ] Commit:
  ```powershell
  git add crates/legion-editor/src/lib.rs crates/legion-desktop/tests/manual_input_conformance.rs plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md
  git commit -m "docs: record manual selection scope"
  ```

---

## Phase 4 - MANUAL.08 Font Fallback Diagnostics

### Task 4.1: Project configured font family and metadata-only fallback diagnostics

**Files:**
- Modify: `crates/legion-protocol/src/lib.rs`
- Modify: `crates/legion-ui/src/ui.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-app/tests/settings.rs`
- Modify: `crates/legion-desktop/src/theme.rs`
- Modify: `crates/legion-desktop/src/view.rs`
- Create: `crates/legion-desktop/tests/manual_renderer_evidence.rs`

- [ ] In `crates/legion-protocol/src/lib.rs`, add a metadata-only diagnostic DTO near `WorkbenchFontSettings`:
  ```rust
  /// Metadata-only font fallback diagnostic surfaced by the desktop renderer.
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  pub struct WorkbenchFontFallbackDiagnostic {
      /// Requested family label, never a raw filesystem path.
      pub requested_family_label: String,
      /// Resolved family label, never a raw filesystem path.
      pub resolved_family_label: String,
      /// Script or glyph coverage label.
      pub coverage_label: String,
      /// Whether a host fallback was found.
      pub fallback_found: bool,
      /// Display-safe diagnostic message.
      pub message: String,
      /// DTO schema version.
      pub schema_version: u16,
  }
  ```

- [ ] In `crates/legion-ui/src/ui.rs`, extend `SettingsProjection`:
  ```rust
  pub editor_font_family: String,
  pub font_fallback_diagnostics: Vec<WorkbenchFontFallbackDiagnostic>,
  ```
  and default it with:
  ```rust
  editor_font_family: "monospace".to_string(),
  font_fallback_diagnostics: Vec::new(),
  ```

- [ ] Add a typed command intent in `CommandDispatchIntent`:
  ```rust
  SetEditorFontFamily {
      /// Requested editor font family label.
      family: String,
  },
  ```

- [ ] In `SettingsProjection::normalized`, clamp the family label to display-safe text:
  ```rust
  self.editor_font_family = normalize_font_family_label(&self.editor_font_family);
  self.font_fallback_diagnostics.truncate(8);
  ```
  with:
  ```rust
  fn normalize_font_family_label(value: &str) -> String {
      let label = value.trim();
      if label.is_empty() {
          "monospace".to_string()
      } else {
          label
              .chars()
              .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
              .take(64)
              .collect::<String>()
      }
  }
  ```

- [ ] In `crates/legion-app/src/lib.rs`, route `SetEditorFontFamily` the same way `SetEditorFontSize` is routed: app-owned settings mutate, normalized projection refreshes, no renderer ownership.

- [ ] In `crates/legion-desktop/src/theme.rs`, add a public metadata probe:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct FontFallbackProbe {
      pub requested_family_label: String,
      pub resolved_family_label: String,
      pub coverage_label: String,
      pub fallback_found: bool,
      pub message: String,
  }

  pub(crate) fn font_fallback_probe(requested_family_label: &str) -> FontFallbackProbe {
      let fallback_found = cjk_font_definitions().is_some();
      FontFallbackProbe {
          requested_family_label: requested_family_label.to_string(),
          resolved_family_label: if fallback_found {
              "legion-cjk-fallback".to_string()
          } else {
              "egui-default".to_string()
          },
          coverage_label: "cjk".to_string(),
          fallback_found,
          message: if fallback_found {
              "CJK fallback loaded from host font catalog".to_string()
          } else {
              "CJK fallback not found in host font catalog".to_string()
          },
      }
  }
  ```

- [ ] In `crates/legion-desktop/src/view.rs`, include fallback diagnostics in `DesktopSettingsViewModel`:
  ```rust
  pub editor_font_family: String,
  pub font_fallback_rows: Vec<String>,
  ```
  and build rows from `font_fallback_diagnostics`:
  ```rust
  font_fallback_rows: normalized
      .font_fallback_diagnostics
      .iter()
      .map(|diagnostic| {
          format!(
              "font fallback: requested={} resolved={} coverage={} found={}",
              diagnostic.requested_family_label,
              diagnostic.resolved_family_label,
              diagnostic.coverage_label,
              diagnostic.fallback_found
          )
      })
      .collect(),
  ```

- [ ] Add `crates/legion-desktop/tests/manual_renderer_evidence.rs` with this first test:
  ```rust
  use legion_desktop::view::DesktopProjectionViewModel;
  use legion_protocol::WorkbenchFontFallbackDiagnostic;
  use legion_ui::{SettingsProjection, Shell};

  #[test]
  fn font_fallback_diagnostics_are_projected_without_raw_font_paths() {
      let mut snapshot = Shell::empty("Font").projection_snapshot();
      snapshot.settings_projection = SettingsProjection {
          editor_font_family: "JetBrains Mono".to_string(),
          font_fallback_diagnostics: vec![WorkbenchFontFallbackDiagnostic {
              requested_family_label: "JetBrains Mono".to_string(),
              resolved_family_label: "legion-cjk-fallback".to_string(),
              coverage_label: "cjk".to_string(),
              fallback_found: true,
              message: "CJK fallback loaded from host font catalog".to_string(),
              schema_version: 1,
          }],
          ..SettingsProjection::default()
      };

      let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

      assert_eq!(model.settings.editor_font_family, "JetBrains Mono");
      assert!(model
          .settings
          .font_fallback_rows
          .iter()
          .any(|row| row.contains("coverage=cjk") && row.contains("found=true")));
      assert!(model
          .settings
          .font_fallback_rows
          .iter()
          .all(|row| !row.contains("\\Windows\\Fonts") && !row.contains("/usr/share/fonts")));
  }
  ```

- [ ] Run:
  ```powershell
  cargo test -p legion-app --test settings
  cargo test -p legion-desktop --test manual_renderer_evidence font_fallback_diagnostics_are_projected_without_raw_font_paths -- --exact
  cargo run -p xtask -- check-deps
  ```
  Expected: settings route works; diagnostics are metadata-only; dependency policy still passes.

- [ ] Commit:
  ```powershell
  git add crates/legion-protocol/src/lib.rs crates/legion-ui/src/ui.rs crates/legion-app/src/lib.rs crates/legion-app/tests/settings.rs crates/legion-desktop/src/theme.rs crates/legion-desktop/src/view.rs crates/legion-desktop/tests/manual_renderer_evidence.rs
  git commit -m "feat: project manual editor font fallback diagnostics"
  ```

---

## Phase 5 - MANUAL.09 Line Wrapping Policy

### Task 5.1: Add stable wrap policy without breaking viewport math

**Files:**
- Modify: `crates/legion-protocol/src/lib.rs`
- Modify: `crates/legion-ui/src/ui.rs`
- Modify: `crates/legion-app/src/lib.rs`
- Modify: `crates/legion-desktop/src/view.rs`
- Modify: `crates/legion-desktop/tests/manual_renderer_evidence.rs`

- [ ] In `crates/legion-protocol/src/lib.rs`, add:
  ```rust
  /// Editor line wrapping policy.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
  #[serde(rename_all = "snake_case")]
  pub enum LineWrappingPolicy {
      /// Do not soft-wrap editor lines.
      #[default]
      Off,
      /// Soft-wrap at the visible viewport width while preserving logical coordinates.
      Viewport,
      /// Soft-wrap at a fixed character column while preserving logical coordinates.
      FixedColumn,
  }
  ```

- [ ] Add fields to `ViewportProjection`:
  ```rust
  #[serde(default)]
  pub line_wrapping_policy: LineWrappingPolicy,
  #[serde(default)]
  pub wrap_column: Option<u32>,
  ```

- [ ] Add fields to `EditorSettingsProjection`:
  ```rust
  pub line_wrapping_policy: LineWrappingPolicy,
  pub wrap_column: Option<u32>,
  ```
  with default:
  ```rust
  line_wrapping_policy: LineWrappingPolicy::Off,
  wrap_column: Some(120),
  ```

- [ ] Add `CommandDispatchIntent::SetLineWrappingPolicy`:
  ```rust
  SetLineWrappingPolicy {
      /// Requested line wrapping policy.
      policy: LineWrappingPolicy,
      /// Optional fixed wrap column.
      wrap_column: Option<u32>,
  },
  ```

- [ ] In `SettingsProjection::normalized`, normalize fixed column:
  ```rust
  self.editor.wrap_column = match self.editor.line_wrapping_policy {
      LineWrappingPolicy::FixedColumn => Some(self.editor.wrap_column.unwrap_or(120).clamp(40, 240)),
      LineWrappingPolicy::Off | LineWrappingPolicy::Viewport => None,
  };
  ```

- [ ] In `crates/legion-desktop/src/view.rs`, add wrap fields to `DesktopSettingsViewModel` and build a stable row:
  ```rust
  pub line_wrapping_policy: LineWrappingPolicy,
  pub wrap_column: Option<u32>,
  pub wrapping_row: String,
  ```
  with:
  ```rust
  wrapping_row: match normalized.editor.line_wrapping_policy {
      LineWrappingPolicy::Off => "wrapping: off".to_string(),
      LineWrappingPolicy::Viewport => "wrapping: viewport".to_string(),
      LineWrappingPolicy::FixedColumn => {
          format!("wrapping: fixed_column {}", normalized.editor.wrap_column.unwrap_or(120))
      }
  },
  ```

- [ ] In `shape_code_line_galley`, use wrap width based on policy:
  ```rust
  fn code_line_wrap_width(model: &DesktopProjectionViewModel, available_width: f32) -> f32 {
      match model.settings.line_wrapping_policy {
          LineWrappingPolicy::Off => f32::INFINITY,
          LineWrappingPolicy::Viewport => available_width.max(1.0),
          LineWrappingPolicy::FixedColumn => {
              let column = model.settings.wrap_column.unwrap_or(120).max(1);
              column as f32 * code_char_width()
          }
      }
  }
  ```
  and call `cached_code_line_galley` with `code_line_wrap_width(model, ui.available_width())`.

- [ ] Add this test to `crates/legion-desktop/tests/manual_renderer_evidence.rs`:
  ```rust
  #[test]
  fn line_wrapping_policy_keeps_viewport_math_stable() {
      let mut snapshot = legion_ui::Shell::empty("Wrap").projection_snapshot();
      snapshot.settings_projection.editor.line_wrapping_policy =
          legion_protocol::LineWrappingPolicy::FixedColumn;
      snapshot.settings_projection.editor.wrap_column = Some(80);

      let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

      assert_eq!(model.settings.line_wrapping_policy, legion_protocol::LineWrappingPolicy::FixedColumn);
      assert_eq!(model.settings.wrap_column, Some(80));
      assert_eq!(model.settings.wrapping_row, "wrapping: fixed_column 80");
      assert!(model
          .viewport_metadata_rows
          .iter()
          .all(|row| !row.contains("visual_line")));
  }
  ```

- [ ] Run:
  ```powershell
  cargo test -p legion-desktop --test manual_renderer_evidence line_wrapping_policy_keeps_viewport_math_stable -- --exact
  cargo test -p legion-app --test settings
  cargo run -p xtask -- check-deps
  ```
  Expected: stable logical viewport rows remain unchanged; wrapping is a renderer presentation policy.

- [ ] Commit:
  ```powershell
  git add crates/legion-protocol/src/lib.rs crates/legion-ui/src/ui.rs crates/legion-app/src/lib.rs crates/legion-desktop/src/view.rs crates/legion-desktop/tests/manual_renderer_evidence.rs
  git commit -m "feat: add manual editor wrapping policy"
  ```

---

## Phase 6 - MANUAL.10 Large-File Degraded-Mode Banner

### Task 6.1: Make capability reduction visible and testable

**Files:**
- Modify: `crates/legion-desktop/src/view.rs`
- Modify: `crates/legion-desktop/tests/large_file_guardrails.rs`
- Modify: `crates/legion-desktop/tests/projection_rendering.rs`

- [ ] In `DesktopProjectionViewModel`, add:
  ```rust
  pub large_file_banner_rows: Vec<String>,
  ```

- [ ] Add helper in `crates/legion-desktop/src/view.rs`:
  ```rust
  fn large_file_banner_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
      let Some(viewport) = &snapshot.active_buffer_projection.viewport else {
          return Vec::new();
      };
      let Some(status) = &viewport.large_file_status else {
          return Vec::new();
      };
      let mut rows = vec![format!(
          "large-file degraded: bytes={} threshold={} bounded_search={}",
          status.byte_len, status.threshold_bytes, status.bounded_search_enabled
      )];
      rows.extend(
          status
              .disabled_overlay_reasons
              .iter()
              .map(|reason| format!("capability reduced: {reason}")),
      );
      rows
  }
  ```

- [ ] Render the banner at the top of `render_editor_canvas` before the code frame:
  ```rust
  if !model.large_file_banner_rows.is_empty() {
      theme::card_frame_tinted(theme::tokens().bg.card, theme::tokens().accent.orange)
          .show(ui, |ui| {
              for row in &model.large_file_banner_rows {
                  ui.label(theme::code_muted(row));
              }
          });
      ui.add_space(6.0);
  }
  ```

- [ ] Add this test to `crates/legion-desktop/tests/large_file_guardrails.rs`:
  ```rust
  #[test]
  fn large_file_guardrails_degraded_banner_names_capability_reduction() {
      let workspace = TempWorkspace::new();
      let large = write_large_file(workspace.path());
      let runtime = open_runtime(workspace.path(), &large);
      let snapshot = runtime.projection_snapshot();
      let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

      assert!(model
          .large_file_banner_rows
          .iter()
          .any(|row| row.contains("large-file degraded")));
      assert!(model
          .large_file_banner_rows
          .iter()
          .any(|row| row.contains("capability reduced")));
      assert!(model
          .large_file_banner_rows
          .iter()
          .all(|row| !row.contains("HIDDEN_NEEDLE_AFTER_VIEWPORT")));
  }
  ```

- [ ] Run:
  ```powershell
  cargo test -p legion-desktop --test large_file_guardrails
  cargo test -p legion-desktop --test projection_rendering projection_rendering_handles_empty_and_degraded_snapshots -- --exact
  ```
  Expected: large-file banner is visible in model rows and does not leak full hidden content.

- [ ] Commit:
  ```powershell
  git add crates/legion-desktop/src/view.rs crates/legion-desktop/tests/large_file_guardrails.rs crates/legion-desktop/tests/projection_rendering.rs
  git commit -m "feat: surface manual large-file capability reduction"
  ```

---

## Phase 7 - MANUAL.11 Deterministic Renderer Evidence

### Task 7.1: Add textual renderer evidence for core editor states

**Files:**
- Modify: `crates/legion-desktop/src/view.rs`
- Modify: `crates/legion-desktop/tests/manual_renderer_evidence.rs`
- Modify: `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md`

- [ ] Add a public method to `DesktopProjectionViewModel`:
  ```rust
  impl DesktopProjectionViewModel {
      pub fn deterministic_editor_evidence(&self) -> Vec<String> {
          let mut rows = Vec::new();
          rows.push(format!("title={}", self.layout_title));
          rows.extend(self.editor_status_rows.iter().map(|row| format!("editor_status={row}")));
          rows.extend(self.viewport_metadata_rows.iter().map(|row| format!("viewport={row}")));
          rows.extend(self.empty_or_degraded_flags.iter().map(|flag| format!("flag={flag}")));
          rows.extend(self.active_buffer_code_lines.iter().take(8).map(|line| {
              format!(
                  "code_line={} len={} truncation={:?}",
                  line.number,
                  line.text.chars().count(),
                  line.truncation_state
              )
          }));
          rows.extend(self.large_file_banner_rows.iter().map(|row| format!("large_file={row}")));
          rows
      }
  }
  ```

- [ ] Add this test:
  ```rust
  #[test]
  fn deterministic_renderer_evidence_covers_core_editor_states() {
      let snapshot = legion_ui::Shell::empty("Evidence").projection_snapshot();
      let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

      let evidence = model.deterministic_editor_evidence();

      assert!(evidence.iter().any(|row| row.starts_with("title=")));
      assert!(evidence.iter().any(|row| row.starts_with("editor_status=")));
      assert!(evidence.iter().any(|row| row.starts_with("viewport=") || row == "flag=no_active_buffer"));
      assert!(evidence.iter().all(|row| !row.contains("raw_source=")));
  }
  ```

- [ ] Append a `## Deterministic Renderer Evidence` section to `WS-MANUAL-01-evidence.md`:
  ```markdown
  ## Deterministic Renderer Evidence

  Core Manual editor states are represented by `DesktopProjectionViewModel::deterministic_editor_evidence()`. The evidence rows are textual, stable, and metadata-only: title, editor status, viewport metadata, flags, code-line lengths, truncation state, and large-file capability rows. They do not persist raw source or full clipboard/IME payloads.
  ```

- [ ] Run:
  ```powershell
  cargo test -p legion-desktop --test manual_renderer_evidence deterministic_renderer_evidence_covers_core_editor_states -- --exact
  ```
  Expected: evidence method covers no-active-buffer and active editor paths without raw source persistence.

- [ ] Commit:
  ```powershell
  git add crates/legion-desktop/src/view.rs crates/legion-desktop/tests/manual_renderer_evidence.rs plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md
  git commit -m "test: add deterministic manual renderer evidence"
  ```

---

## Phase 8 - MANUAL.12 Manual-Mode Zero-Egress Smoke

### Task 8.1: Add Manual zero-egress app smoke

**Files:**
- Create: `crates/legion-app/tests/manual_zero_egress.rs`
- Create: `plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md`
- Modify: `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md`

- [ ] Create `crates/legion-app/tests/manual_zero_egress.rs`:
  ```rust
  use std::{fs, sync::atomic::{AtomicU64, Ordering}};

  use legion_app::{AppCommandOutcome, AppComposition, AppProductMode};
  use legion_protocol::{PrincipalId, TextCoordinate, WorkspaceTrustState};
  use legion_ui::{CommandDispatchIntent, SearchScopeProjection};

  struct TempWorkspace {
      root: std::path::PathBuf,
  }

  impl TempWorkspace {
      fn new() -> Self {
          static COUNTER: AtomicU64 = AtomicU64::new(0);
          let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
          let root = std::env::temp_dir().join(format!("legion-manual-zero-egress-{seq}"));
          fs::create_dir_all(&root).expect("create temp workspace");
          fs::write(root.join("main.rs"), "fn main() {\n    let value = 1;\n}\n")
              .expect("write source");
          Self { root }
      }

      fn path(&self) -> &std::path::Path {
          &self.root
      }
  }

  impl Drop for TempWorkspace {
      fn drop(&mut self) {
          let _ = fs::remove_dir_all(&self.root);
      }
  }

  fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
      TextCoordinate {
          line,
          character,
          byte_offset: Some(byte_offset),
          utf16_offset: Some(byte_offset),
      }
  }

  #[test]
  fn manual_mode_open_edit_save_search_records_no_hosted_egress() {
      let workspace = TempWorkspace::new();
      let mut app = AppComposition::new();
      app.set_product_mode(AppProductMode::Manual);
      app.open_workspace(
          workspace.path(),
          WorkspaceTrustState::Trusted,
          PrincipalId("manual-smoke".to_string()),
      )
      .expect("open workspace");
      app.open_file("main.rs").expect("open file");
      let snapshot = app.shell_projection_snapshot("Manual").expect("snapshot");
      let buffer_id = snapshot.active_buffer_projection.buffer_id.expect("active buffer");

      app.dispatch_ui_intent(CommandDispatchIntent::Insert {
          buffer_id,
          at: coord(1, 4, 16),
          text: "let local_only = true;\n    ".to_string(),
      })
      .expect("insert should route");
      app.dispatch_ui_intent(CommandDispatchIntent::RunSearch {
          scope: SearchScopeProjection::ActiveFile,
          query: "local_only".to_string(),
          limit: 10,
      })
      .expect("search should route");
      let save = app
          .dispatch_ui_intent(CommandDispatchIntent::Save { buffer_id })
          .expect("save should route");
      assert!(matches!(save, AppCommandOutcome::Saved(_)));

      let snapshot = app.shell_projection_snapshot("Manual").expect("snapshot");
      assert_eq!(snapshot.product_mode, legion_ui::DockMode::Manual);
      assert_eq!(snapshot.assisted_ai_projection.preview_ready_count, 0);
      assert!(snapshot.assist_inline_prediction_projection.rows.is_empty());
      assert_eq!(snapshot.delegated_task_projection.plan_count, 0);
      assert!(snapshot.delegated_task_projection.chat_messages.is_empty());
      assert!(snapshot
          .status_messages
          .iter()
          .all(|status| !status.message.to_ascii_lowercase().contains("http")));
  }
  ```

- [ ] Add this renderer-model trust-boundary assertion to `crates/legion-desktop/tests/manual_renderer_evidence.rs`:
  ```rust
  #[test]
  fn manual_renderer_evidence_names_zero_egress_trust_boundary() {
      let mut snapshot = legion_ui::Shell::empty("Manual").projection_snapshot();
      snapshot.product_mode = legion_ui::DockMode::Manual;

      let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

      assert!(model.manual_control_rows.iter().any(|row| {
          row.contains("manual trust boundary")
              && row.contains("no provider dispatch")
              && row.contains("no agent context")
      }));
  }
  ```

- [ ] Create `plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md`:
  ```markdown
  # Manual Mode Zero-Egress Smoke

  Date: 2026-06-19

  ## Contract

  Manual mode can open, edit, save, and search a trusted local workspace without hosted provider dispatch, agent context retrieval, autonomous writes, telemetry export, or network target records.

  ## Verification Command

  `cargo test -p legion-app --test manual_zero_egress`

  ## Evidence Rules

  - The test must operate through `AppComposition` and `CommandDispatchIntent`, not direct buffer mutation.
  - The test must assert Manual product mode.
  - The app-level test must assert no assisted-AI, inline-prediction, or delegated-task activity is created by the Manual open/edit/save/search path.
  - The desktop renderer evidence test must assert Manual trust-boundary rows that name no provider dispatch and no agent context.
  - This smoke does not prove OS-level network denial. A later sandbox/firewall packet capture may strengthen the row, but this test is the required app-level regression guard for WS-MANUAL-01.
  ```

- [ ] Run:
  ```powershell
  cargo test -p legion-app --test manual_zero_egress
  cargo test -p legion-desktop --test manual_renderer_evidence manual_renderer_evidence_names_zero_egress_trust_boundary -- --exact
  ```
  Expected: pass, with open/edit/save/search routed through app authority, no AI/delegate activity, and renderer-model Manual trust rows visible.

- [ ] Commit:
  ```powershell
  git add crates/legion-app/tests/manual_zero_egress.rs crates/legion-desktop/tests/manual_renderer_evidence.rs plans/evidence/production/WS-MANUAL-01/manual-mode-zero-egress.md plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md
  git commit -m "test: add manual mode zero-egress smoke"
  ```

---

## Phase 9 - Product Ledger and Evidence Closure

### Task 9.1: Update readiness ledger conservatively

**Files:**
- Modify: `plans/product-readiness-ledger.md`
- Modify: `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md`

- [ ] Update only the `PR-UI-001` evidence cell in `plans/product-readiness-ledger.md` to reference WS-MANUAL-01 evidence. Keep status `Substrate validated` unless all renderer-backed and platform checks passed in the current tree. Use this wording:
  ```markdown
  WS-MANUAL-01 adds renderer-backed Manual input-to-paint perf evidence, input/focus/IME/clipboard tests, font/wrapping/degraded-mode renderer evidence, deterministic editor evidence, and an app-level Manual zero-egress smoke under `plans/evidence/production/WS-MANUAL-01/`. Promotion beyond substrate validated still requires current native platform accessibility/focus evidence across supported OSes.
  ```

- [ ] Fill every Verification row in `WS-MANUAL-01-evidence.md` with `pass`, `fail`, or `blocked`, plus a short note. Do not leave blank result cells.

- [ ] Run:
  ```powershell
  cargo run -p xtask -- docs-hygiene
  git diff --check
  ```
  Expected: docs hygiene and whitespace checks pass.

- [ ] Commit:
  ```powershell
  git add plans/product-readiness-ledger.md plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md
  git commit -m "docs: record WS-MANUAL-01 manual editor evidence"
  ```

---

## Phase 10 - Final Verification

- [ ] Run targeted tests:
  ```powershell
  cargo test -p xtask --test perf_harness
  cargo test -p xtask --test no_egui_textedit
  cargo test -p legion-desktop --test manual_perf
  cargo test -p legion-desktop --test manual_input_conformance
  cargo test -p legion-desktop --test manual_renderer_evidence
  cargo test -p legion-desktop --test large_file_guardrails
  cargo test -p legion-app --test manual_zero_egress
  cargo test -p legion-app --test settings
  ```
  Required result: pass, except native renderer perf may be recorded as blocked only if the environment cannot open or simulate the renderer path. A blocked renderer perf run keeps WS-MANUAL-01 open.

- [ ] Run phase gates directly tied to WS-MANUAL-01:
  ```powershell
  cargo run -p xtask -- perf-harness
  cargo run -p xtask -- verify-perf-harness
  cargo run -p xtask -- no-egui-textedit
  cargo run -p xtask -- docs-hygiene
  cargo run -p xtask -- check-deps
  cargo fmt --all --check
  cargo check --workspace --all-targets
  git diff --check
  ```
  Required result: pass. If `perf-harness` reports the Manual renderer measurement as skipped because native rendering is blocked, the final summary must say WS-MANUAL-01 is not fully complete.

- [ ] Run full workspace confidence checks before final handoff:
  ```powershell
  cargo test --workspace --all-targets --no-fail-fast
  cargo clippy --workspace --all-targets -- -D warnings
  ```
  Required result: pass for a completion claim. If an unrelated pre-existing failure appears, capture the failing command, first failing test target, and why it is unrelated. Do not mark WS-MANUAL-01 complete if the failure blocks Manual editor verification.

- [ ] Update `plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md` with final command results.

- [ ] Final commit after evidence update:
  ```powershell
  git add plans/evidence/production/WS-MANUAL-01/WS-MANUAL-01-evidence.md
  git commit -m "docs: finalize WS-MANUAL-01 verification evidence"
  ```

---

## Completion Checklist

- [ ] MANUAL.01 has explicit budget evidence in `editor-latency-budgets.md`.
- [ ] MANUAL.02 extends `xtask perf-harness` with a renderer-backed Manual measurement from `legion-desktop`.
- [ ] MANUAL.03 keeps `cargo run -p xtask -- no-egui-textedit` green.
- [ ] MANUAL.04 has IME composition and commit coverage.
- [ ] MANUAL.05 has clipboard copy/cut/paste/select-all coverage or an explicit failing test driving the missing typed intents.
- [ ] MANUAL.06 records the multi-cursor substrate and the rectangular-selection v1 cut line.
- [ ] MANUAL.07 validates focus routing across editor and palette at minimum; terminal and diff review focus must be added before promoting PR-UI-001.
- [ ] MANUAL.08 surfaces configured font labels and metadata-only fallback diagnostics.
- [ ] MANUAL.09 adds line wrapping policy without changing logical viewport coordinates.
- [ ] MANUAL.10 shows a degraded-mode banner with capability reduction and no full-source leakage.
- [ ] MANUAL.11 exposes deterministic textual renderer evidence for core editor states.
- [ ] MANUAL.12 proves app-level Manual open/edit/save/search does not trigger AI/delegate/network-facing surfaces.
- [ ] `plans/product-readiness-ledger.md` references WS-MANUAL-01 evidence without overstating product readiness.
- [ ] All targeted tests and phase gates listed in Phase 10 are run and recorded.
