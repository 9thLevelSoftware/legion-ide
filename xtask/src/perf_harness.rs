//! M0 performance-harness skeleton (WS18.T1).
//!
//! The full WS18.T1 implementation will exercise the Legion editor, indexer,
//! and protocol layers across the reference workloads described in
//! `plans/legion-production-master-plan-v0.2.md` quality bars (input-to-paint p50/p95,
//! scroll jank, startup, memory ceiling on the Legion repo, 100K-file fixture,
//! 100MB file). The M0 acceptance is a **skeleton** that lands in CI, emits a
//! dashboard report, and demonstrates a failing-gate. The full per-OS
//! reference workloads remain owned by WS18.T1's follow-on work and depend on
//! the streaming-mode (WS01.T7) and AccessKit (WS18.T2) substrates.
//!
//! Scope of this module:
//!   * Plan a deterministic skeleton: one synthetic "input-to-paint" hot
//!     path (a small text edit loop) executed against an in-memory byte
//!     buffer, with a configurable sample count.
//!   * Run the plan, capture p50/p95/total as `Duration` values, classify
//!     each measurement against a per-skeleton budget.
//!   * Write a `perf_report.toml` describing the measurements + summary
//!     (total/passed/failed/skipped). This file is the M0 "dashboard".
//!   * Expose a `--fail-on-budget <N>` (env override) so a CI leg can
//!     demonstrate the failing-gate by tightening the budget below the
//!     measured value.
//!
//! The skeleton does not require `legion-editor` as a dependency; the
//! in-process hot path is a stand-in for the editor input-to-paint loop
//! and is the only substrate the M0 CI matrix owns. Real editor / indexer
//! / protocol benchmarks (the post-M0 WS18.T1 follow-on) will replace
//! this stand-in without changing the report shape.

use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

const SKELETON_FIXTURE_BYTES: usize = 64 * 1024;
const SKELETON_EDIT_SAMPLES: usize = 32;
const SKELETON_DEFAULT_BUDGET_MILLIS: u64 = 250;
const LINE_GALLEY_FIXTURE_LINES: usize = 10_000;
const LINE_GALLEY_VISIBLE_ROWS: usize = 80;
const LINE_GALLEY_DEFAULT_BUDGET_MILLIS: u64 = 2;
const MANUAL_RENDERER_KEYPRESS_P50_BUDGET_MILLIS: u64 = 16;
const MANUAL_RENDERER_KEYPRESS_P95_BUDGET_MILLIS: u64 = 32;
const MANUAL_RENDERER_SCROLL_P95_BUDGET_MILLIS: u64 = 32;
const MANUAL_RENDERER_SAMPLE_COUNT: usize = 16;
const MANUAL_RENDERER_SCENARIO: &str = "manual_editor_input_to_paint";
pub const PERF_REPORT_FILE: &str = "perf_report.toml";
pub const MANUAL_RENDERER_PERF_REPORT_FILE: &str = "manual_renderer_perf.toml";

/// Environment variable that, when set to a positive millisecond count,
/// overrides the per-skeleton budget. Used by the failing-gate CI leg.
pub const FAIL_ON_BUDGET_ENV: &str = "LEGION_PERF_FAIL_ON_BUDGET_MS";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkeletonKind {
    /// Synthetic input-to-paint stand-in: small text edits against a
    /// fixed-size in-memory byte buffer. Mirrors the hot path the editor
    /// p50/p95 input-to-paint budget will gate against, but does not
    /// require `legion-editor` as an `xtask` dependency.
    #[serde(
        rename = "input_to_paint_microbenchmark",
        alias = "inputtopaintmicrobenchmark"
    )]
    InputToPaintMicrobenchmark,
    /// Synthetic line-galley shaping-cache frame: a 10K-line fixture with
    /// only the visible viewport rows looked up/shaped per frame.
    #[serde(rename = "line_galley_shaping_cache", alias = "linegalleyshapingcache")]
    LineGalleyShapingCache,
    /// Renderer-backed Manual editor input-to-paint measurement supplied by
    /// the `legion-desktop --manual-perf` subprocess.
    #[serde(rename = "renderer_backed_manual_input_to_paint")]
    RendererBackedManualInputToPaint,
}

impl SkeletonKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InputToPaintMicrobenchmark => "input_to_paint_microbenchmark",
            Self::LineGalleyShapingCache => "line_galley_shaping_cache",
            Self::RendererBackedManualInputToPaint => "renderer_backed_manual_input_to_paint",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkeletonDescriptor {
    pub name: String,
    pub kind: SkeletonKind,
    pub fixture_bytes: usize,
    pub sample_count: usize,
    /// Per-skeleton budget in milliseconds, inclusive. The CI leg can
    /// override the budget via the `LEGION_PERF_FAIL_ON_BUDGET_MS`
    /// environment variable; setting the budget to `0` means
    /// "report-only" (no gate).
    pub budget_millis: u64,
    /// Free-form note describing what this skeleton stands in for.
    pub note: String,
}

impl SkeletonDescriptor {
    pub fn m0_input_to_paint() -> Self {
        Self {
            name: "m0.input_to_paint_microbenchmark".to_string(),
            kind: SkeletonKind::InputToPaintMicrobenchmark,
            fixture_bytes: SKELETON_FIXTURE_BYTES,
            sample_count: SKELETON_EDIT_SAMPLES,
            budget_millis: SKELETON_DEFAULT_BUDGET_MILLIS,
            note: concat!(
                "Stand-in for the editor input-to-paint hot path. Replaced ",
                "by the WS18.T1 follow-on that exercises `legion-editor` and ",
                "the indexer on the Legion repo + 100K-file fixture + 100MB ",
                "file per master-plan §11.",
            )
            .to_string(),
        }
    }

    pub fn m1_line_galley_shaping_cache() -> Self {
        Self {
            name: "m1.line_galley_shaping_cache".to_string(),
            kind: SkeletonKind::LineGalleyShapingCache,
            fixture_bytes: LINE_GALLEY_FIXTURE_LINES,
            sample_count: 1,
            budget_millis: LINE_GALLEY_DEFAULT_BUDGET_MILLIS,
            note: concat!(
                "WS01.T2 line-galley shaping-cache gate: represents a ",
                "10K-line editor buffer where only visible viewport rows ",
                "are shaped/looked up for a frame; strict budget is <2ms."
            )
            .to_string(),
        }
    }

    pub fn budget(&self) -> Option<Duration> {
        if self.budget_millis == 0 {
            None
        } else {
            Some(Duration::from_millis(self.budget_millis))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkeletonMeasurement {
    pub name: String,
    pub kind: SkeletonKind,
    pub fixture_bytes: usize,
    pub sample_count: usize,
    pub total_micros: u64,
    pub p50_micros: u64,
    pub p95_micros: u64,
    pub budget_millis: u64,
    pub status: SkeletonStatus,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkeletonStatus {
    /// Measurement is within budget. Counts toward `passed`.
    Passed,
    /// Measurement exceeds budget. Counts toward `failed`. CI leg must exit non-zero.
    Failed,
    /// Budget is `0` (report-only mode). Counts toward `skipped`.
    Skipped,
}

impl SkeletonStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PerfSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerfReport {
    pub schema_version: u32,
    pub package_name: String,
    pub measured_at_utc: String,
    pub git_sha: String,
    pub summary: PerfSummary,
    pub skeletons: Vec<SkeletonMeasurement>,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfHarnessError {
    pub message: String,
}

impl std::fmt::Display for PerfHarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PerfHarnessError {}

/// Plan a deterministic skeleton run. Pure function: no I/O, no clock.
pub fn plan_perf_harness(skeleton: &SkeletonDescriptor) -> SkeletonMeasurement {
    let samples = match skeleton.kind {
        SkeletonKind::InputToPaintMicrobenchmark => {
            run_input_to_paint_microbenchmark(skeleton.fixture_bytes, skeleton.sample_count)
        }
        SkeletonKind::LineGalleyShapingCache => run_line_galley_shaping_cache_microbenchmark(
            skeleton.fixture_bytes,
            skeleton.sample_count,
        ),
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
                message:
                    "renderer-backed Manual measurement is supplied by legion-desktop subprocess"
                        .to_string(),
            };
        }
    };
    let total = samples.iter().copied().sum::<Duration>();
    let mut sorted = samples.clone();
    sorted.sort();
    let p50 = percentile_micros(&sorted, 0.50);
    let p95 = percentile_micros(&sorted, 0.95);

    let budget = skeleton.budget();
    let total_millis = total.as_millis() as u64;
    let (status, message) = match budget {
        None => (
            SkeletonStatus::Skipped,
            "budget is 0; report-only (no gate)".to_string(),
        ),
        Some(budget) if total <= budget => (
            SkeletonStatus::Passed,
            format!(
                "total {total_millis}ms within budget {}ms",
                budget.as_millis()
            ),
        ),
        Some(budget) => (
            SkeletonStatus::Failed,
            format!(
                "total {total_millis}ms exceeded budget {}ms (p50={}us p95={}us)",
                budget.as_millis(),
                p50,
                p95,
            ),
        ),
    };

    SkeletonMeasurement {
        name: skeleton.name.clone(),
        kind: skeleton.kind,
        fixture_bytes: skeleton.fixture_bytes,
        sample_count: skeleton.sample_count,
        total_micros: total.as_micros() as u64,
        p50_micros: p50,
        p95_micros: p95,
        budget_millis: skeleton.budget_millis,
        status,
        message,
    }
}

fn run_input_to_paint_microbenchmark(fixture_bytes: usize, sample_count: usize) -> Vec<Duration> {
    // Synthetic fixture: a small byte buffer the hot path mutates. Sized to
    // mirror the editor's typical-input budget (64 KiB). The M0 skeleton
    // intentionally stays small so CI noise does not flake the gate.
    let mut buffer = vec![b'a'; fixture_bytes];
    let mut samples = Vec::with_capacity(sample_count);

    for i in 0..sample_count {
        let pivot = (i * 13 + 7) % fixture_bytes;
        let start = Instant::now();
        // Stand-in for the editor input-to-paint hot path: a small byte
        // edit at a deterministic offset. The WS18.T1 follow-on replaces
        // this with `legion_editor::EditorEngine::apply_edit` calls.
        buffer[pivot] = b'b';
        // Touch the surrounding bytes so the optimizer cannot fold the
        // mutation into a dead store. This keeps the stand-in honest
        // about the cost a real editor hot path pays. The walk length
        // is large enough that each sample takes well above 1µs on every
        // CI runner, which makes sorted percentile samples agree on the
        // µs boundary across runs.
        let mut acc: u64 = 0;
        for byte in &buffer[pivot..] {
            acc = acc.wrapping_add(u64::from(*byte));
        }
        std::hint::black_box(acc);
        samples.push(start.elapsed());
    }
    samples
}

fn run_line_galley_shaping_cache_microbenchmark(
    fixture_lines: usize,
    sample_count: usize,
) -> Vec<Duration> {
    let fixture_lines = fixture_lines.max(LINE_GALLEY_VISIBLE_ROWS);
    let mut line_hashes = Vec::with_capacity(fixture_lines);
    for line in 0..fixture_lines {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in format!("fn generated_line_{line:05}() -> usize {{ {line} }}").bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        line_hashes.push(hash);
    }

    let mut cache = HashMap::with_capacity(LINE_GALLEY_VISIBLE_ROWS * 2);
    let mut samples = Vec::with_capacity(sample_count);
    for frame in 0..sample_count {
        let scroll_span = fixture_lines
            .saturating_sub(LINE_GALLEY_VISIBLE_ROWS)
            .max(1);
        let scroll_base = (frame * 97) % scroll_span;
        let start = Instant::now();
        let mut frame_vertices = 0_u64;
        for visible_row in 0..LINE_GALLEY_VISIBLE_ROWS {
            let line_index = scroll_base + visible_row;
            let content_hash = line_hashes[line_index];
            let key = (content_hash, 14_u32, 240_u32);
            let shaped_vertices = *cache.entry(key).or_insert_with(|| {
                // Stand-in for renderer galley shaping output. The production
                // path caches egui `Galley` values; this synthetic gate keeps
                // CI deterministic without depending on a graphics/font stack.
                content_hash.count_ones() as u64 + 12
            });
            frame_vertices = frame_vertices.wrapping_add(shaped_vertices);
        }
        std::hint::black_box(frame_vertices);
        samples.push(start.elapsed());
    }
    samples
}

fn percentile_micros(sorted: &[Duration], pct: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * pct).round() as usize;
    sorted[idx].as_micros() as u64
}

/// Plan a full M0 skeleton run (currently one skeleton) and return a
/// populated report (no I/O).
pub fn plan_m0_skeletons(
    package_name: &str,
    git_sha: &str,
    skeleton: &SkeletonDescriptor,
) -> PerfReport {
    let measurement = plan_perf_harness(skeleton);
    let skeletons = vec![measurement];
    let summary = summarize_measurements(&skeletons);
    PerfReport {
        schema_version: 1,
        package_name: package_name.to_string(),
        measured_at_utc: current_utc_rfc3339(),
        git_sha: git_sha.to_string(),
        summary,
        skeletons,
    }
}

pub fn plan_perf_skeletons(
    package_name: &str,
    git_sha: &str,
    skeletons: &[SkeletonDescriptor],
) -> PerfReport {
    let measurements = skeletons.iter().map(plan_perf_harness).collect::<Vec<_>>();
    let summary = summarize_measurements(&measurements);
    PerfReport {
        schema_version: 1,
        package_name: package_name.to_string(),
        measured_at_utc: current_utc_rfc3339(),
        git_sha: git_sha.to_string(),
        summary,
        skeletons: measurements,
    }
}

pub fn summarize_measurements(measurements: &[SkeletonMeasurement]) -> PerfSummary {
    let mut summary = PerfSummary {
        total: measurements.len(),
        ..PerfSummary::default()
    };
    for measurement in measurements {
        match measurement.status {
            SkeletonStatus::Passed => summary.passed += 1,
            SkeletonStatus::Failed => summary.failed += 1,
            SkeletonStatus::Skipped => summary.skipped += 1,
        }
    }
    summary
}

pub fn read_manual_renderer_perf_report(path: &Path) -> Result<ManualRendererPerfToml, String> {
    let text = fs::read_to_string(path).map_err(|err| {
        format!(
            "unable to read Manual renderer perf report `{}`: {err}",
            path.display()
        )
    })?;
    let report: ManualRendererPerfToml = toml::from_str(&text).map_err(|err| {
        format!(
            "unable to parse Manual renderer perf report `{}`: {err}",
            path.display()
        )
    })?;
    if report.schema_version != 1 {
        return Err(format!(
            "Manual renderer perf report `{}` uses unsupported schema_version {}",
            path.display(),
            report.schema_version
        ));
    }
    if report.scenario != MANUAL_RENDERER_SCENARIO {
        return Err(format!(
            "Manual renderer perf report `{}` has unexpected scenario `{}` (expected `{}`)",
            path.display(),
            report.scenario,
            MANUAL_RENDERER_SCENARIO
        ));
    }
    Ok(report)
}

pub fn manual_renderer_perf_measurement(report: &ManualRendererPerfToml) -> SkeletonMeasurement {
    let status = match report.status.as_str() {
        "passed" => SkeletonStatus::Passed,
        "skipped" => SkeletonStatus::Skipped,
        _ => SkeletonStatus::Failed,
    };
    let p95_micros = report.keypress_p95_micros.max(report.scroll_p95_micros);
    SkeletonMeasurement {
        name: "manual.renderer_input_to_paint".to_string(),
        kind: SkeletonKind::RendererBackedManualInputToPaint,
        fixture_bytes: 0,
        sample_count: report.sample_count,
        total_micros: report
            .keypress_p95_micros
            .saturating_add(report.scroll_p95_micros),
        p50_micros: report.keypress_p50_micros,
        p95_micros,
        budget_millis: report
            .keypress_p95_budget_ms
            .max(report.scroll_p95_budget_ms),
        status,
        message: if report.message.trim().is_empty() {
            format!("Manual renderer report status `{}`", report.status)
        } else {
            report.message.clone()
        },
    }
}

/// Write the report to `<out_dir>/perf_report.toml`. Returns the absolute
/// path of the written file on success.
pub fn write_report(out_dir: &Path, report: &PerfReport) -> Result<PathBuf, String> {
    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "unable to create perf-harness output dir `{}`: {err}",
            out_dir.display()
        )
    })?;
    let path = out_dir.join(PERF_REPORT_FILE);
    let text = toml::to_string_pretty(report)
        .map_err(|err| format!("unable to serialize perf report: {err}"))?;
    let mut file = fs::File::create(&path).map_err(|err| {
        format!(
            "unable to create perf-harness report `{}`: {err}",
            path.display()
        )
    })?;
    file.write_all(text.as_bytes()).map_err(|err| {
        format!(
            "unable to write perf-harness report `{}`: {err}",
            path.display()
        )
    })?;
    file.write_all(b"\n").map_err(|err| {
        format!(
            "unable to finalize perf-harness report `{}`: {err}",
            path.display()
        )
    })?;
    Ok(path)
}

/// Read the report back from disk. Used by CI / external tooling to assert
/// the report survived a round trip without re-running the harness.
pub fn read_report(path: &Path) -> Result<PerfReport, String> {
    let text = fs::read_to_string(path).map_err(|err| {
        format!(
            "unable to read perf-harness report `{}`: {err}",
            path.display()
        )
    })?;
    toml::from_str(&text).map_err(|err| {
        format!(
            "unable to parse perf-harness report `{}`: {err}",
            path.display()
        )
    })
}

/// Resolve the workspace git SHA. Mirrors the `release_pipeline` helper
/// so the perf report and the release stamp agree on the same revision.
pub fn resolve_workspace_git_sha(workspace_root: &Path) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args(["rev-parse", "HEAD"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if sha.is_empty() {
                "unknown".to_string()
            } else {
                sha
            }
        }
        _ => "unknown".to_string(),
    }
}

/// Apply the `LEGION_PERF_FAIL_ON_BUDGET_MS` environment override to a
/// skeleton, if set. The override lets the failing-gate CI leg force a
/// sub-measurement budget to demonstrate the gate.
pub fn apply_fail_on_budget_override(skeleton: &mut SkeletonDescriptor) {
    let Ok(value) = std::env::var(FAIL_ON_BUDGET_ENV) else {
        return;
    };
    let Ok(parsed) = value.trim().parse::<u64>() else {
        return;
    };
    if parsed == 0 {
        // Explicit zero is honored as a way to disable the gate in a
        // single CI leg without touching the descriptor.
        skeleton.budget_millis = 0;
        return;
    }
    skeleton.budget_millis = parsed;
}

fn current_utc_rfc3339() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86_400;
    let secs_of_day = secs % 86_400;
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant's `civil_from_days` algorithm. Returns (year, month, day)
/// for the given count of days since the Unix epoch (1970-01-01).
/// Identical to the helper in `xtask::release_pipeline`; duplicated here
/// to keep the perf module self-contained.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
