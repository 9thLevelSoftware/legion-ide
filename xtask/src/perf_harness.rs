//! Performance-harness report shape, in-process skeletons, and budget
//! classification.
//!
//! The reference workloads named in
//! `plans/legion-production-master-plan-v0.2.md` — input-to-paint p50/p95,
//! scroll jank, startup, memory ceiling, the Legion repo, the 100K-file
//! fixture, and the 100MB file — are all measured against real product code as
//! of P8.F4.T1. They do not live here, because `xtask` may not depend on
//! `legion-app`/`legion-editor` (`check-deps` enforces that): they live in
//! product-crate binaries that this harness spawns, and the results come back
//! through [`crate::perf_workloads`] (`legion-app --bin product_perf`),
//! [`run_renderer_backed_large_file_measurement`] and
//! [`run_renderer_backed_manual_measurement`] (`legion-desktop --manual-perf`).
//!
//! What still runs in-process here:
//!   * Two synthetic tripwires — an input-to-paint byte-walk and a line-galley
//!     shaping-cache model — kept because they need no subprocess and no
//!     display, and marked `synthetic_stand_in = true` in the report so nobody
//!     mistakes them for product measurements.
//!   * Two real-stack workloads `xtask`'s allowed dependencies can reach: a
//!     `legion_text::TextBuffer` memory guardrail and a 50K-file search-stream
//!     throughput scan through `legion-project`'s real search stack.
//!   * The report shape, the budget classification, and the
//!     `LEGION_PERF_FAIL_ON_BUDGET_MS` report-only override that hosted CI
//!     legs use so shared-runner timing noise cannot red a PR.
//!
//! `plans/evidence/perf-harness-trend/` holds the archived trend entries and
//! the regression baseline; see [`crate::perf_trend`].

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
const MEMORY_CEILING_FIXTURE_BYTES: usize = 1024 * 1024; // 1MB
const MEMORY_CEILING_DEFAULT_BUDGET_BYTES: usize = 10 * 1024 * 1024; // 10MB ceiling for 1MB doc
/// Number of synthetic text files created for the search-stream throughput workload.
const SEARCH_STREAM_50K_FILE_COUNT: usize = 50_000;
/// Budget for the 50 K search-stream throughput skeleton.
/// Set to 0 (report-only) because scan time depends heavily on host disk
/// speed and CI runner choice.  The gate can be tightened once baseline
/// numbers are collected from the reference machines.
const SEARCH_STREAM_50K_BUDGET_MILLIS: u64 = 0;
/// The 100MB workload gates on ADR-0048's keypress p50, because typing is
/// what makes a large file usable rather than merely openable, and it is the
/// budget the measurement actually strains.
const LARGE_FILE_100MB_BUDGET_MILLIS: u64 = MANUAL_RENDERER_KEYPRESS_P50_BUDGET_MILLIS;
pub const PERF_REPORT_FILE: &str = "perf_report.toml";
pub const MANUAL_RENDERER_PERF_REPORT_FILE: &str = "manual_renderer_perf.toml";
pub const LARGE_FILE_MANUAL_RENDERER_PERF_REPORT_FILE: &str =
    "large_file_manual_renderer_perf.toml";

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
    /// Real 100MB large-file measurement supplied by the
    /// `legion-desktop --manual-perf` subprocess.
    ///
    /// Every other large-file number this harness reports is a synthetic
    /// stand-in, because `xtask` cannot depend on `legion-editor`. This one
    /// opens an actual 100MB file through the desktop renderer path and
    /// measures typing and scrolling in it, which is the question that decides
    /// whether large files are usable rather than merely openable.
    #[serde(rename = "large_file_100mb", alias = "largefile100mb")]
    LargeFile100Mb,
    /// Renderer-backed Manual editor input-to-paint measurement supplied by
    /// the `legion-desktop --manual-perf` subprocess.
    #[serde(rename = "renderer_backed_manual_input_to_paint")]
    RendererBackedManualInputToPaint,
    /// Memory ceiling measurement for a reference-size text buffer.
    /// Creates a 1MB `TextBuffer` and asserts the memory footprint stays
    /// below a configurable ceiling.
    #[serde(rename = "memory_ceiling_1mb", alias = "memoryceiling1mb")]
    MemoryCeiling1MB,
    /// Search-stream throughput: scans 50 K synthetic text files in a temp
    /// directory, measuring total wall-clock time and early-cancellation
    /// latency.  Fixture is generated at runtime and cleaned up after.
    #[serde(rename = "search_stream_50k", alias = "searchstream50k")]
    SearchStream50K,
    /// Real product workload supplied by the `legion-app --bin product_perf`
    /// subprocess: startup, input-to-paint, scroll, memory ceiling, the Legion
    /// repository, and the 100K-file fixture. One variant covers all six
    /// because they share a transport and a report shape; the row's `name`
    /// says which workload it is.
    #[serde(rename = "product_workload", alias = "productworkload")]
    ProductWorkload,
}

impl SkeletonKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InputToPaintMicrobenchmark => "input_to_paint_microbenchmark",
            Self::LineGalleyShapingCache => "line_galley_shaping_cache",
            Self::RendererBackedManualInputToPaint => "renderer_backed_manual_input_to_paint",
            Self::MemoryCeiling1MB => "memory_ceiling_1mb",
            Self::SearchStream50K => "search_stream_50k",
            Self::LargeFile100Mb => "large_file_100mb",
            Self::ProductWorkload => "product_workload",
        }
    }

    /// Whether this kind measures a synthetic stand-in rather than a product
    /// code path.
    ///
    /// The two that remain are honest about what they are: the input-to-paint
    /// microbenchmark is a byte-walk over an in-memory buffer, and the
    /// line-galley skeleton models a shaping cache without a font stack. Both
    /// were stand-ins for workloads that now have real measurements
    /// (`product_workload`, `renderer_backed_manual_input_to_paint`), and they
    /// are kept only as cheap, host-independent tripwires.
    pub fn is_synthetic_stand_in(self) -> bool {
        matches!(
            self,
            Self::InputToPaintMicrobenchmark | Self::LineGalleyShapingCache
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkeletonDescriptor {
    pub name: String,
    pub kind: SkeletonKind,
    /// Fixture size in bytes.  For skeletons that measure file-count
    /// throughput rather than byte throughput, use `file_count` instead
    /// — set this field to `0` to avoid the semantic mismatch.
    pub fixture_bytes: usize,
    /// Number of files in the fixture for skeletons where the workload
    /// unit is a file count rather than a byte count (e.g. SearchStream50K).
    /// When `Some`, this value drives fixture generation; `fixture_bytes`
    /// is ignored for that skeleton's workload.
    #[serde(default)]
    pub file_count: Option<usize>,
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
            file_count: None,
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
            file_count: None,
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

    pub fn m2_memory_ceiling_1mb() -> Self {
        Self {
            name: "m2.memory_ceiling_1mb".to_string(),
            kind: SkeletonKind::MemoryCeiling1MB,
            fixture_bytes: MEMORY_CEILING_FIXTURE_BYTES,
            file_count: None,
            sample_count: 1,
            budget_millis: 0, // report-only by default (measured in bytes, not millis)
            note: concat!(
                "WS-MANUAL-02 SCALE.09 memory ceiling gate: creates a 1MB TextBuffer ",
                "and asserts the memory_footprint_bytes() stays below 10MB. The budget ",
                "field is unused (measurement is byte-based, not time-based).",
            )
            .to_string(),
        }
    }

    /// The real 100MB large-file workload.
    ///
    /// Part of the standard run rather than an opt-in flag: a budget nobody
    /// runs is not a budget. It costs a minute of wall clock, which is the
    /// price of measuring the real thing instead of a stand-in.
    pub fn m9_large_file_100mb() -> Self {
        Self {
            name: "m9.large_file_100mb".to_string(),
            kind: SkeletonKind::LargeFile100Mb,
            fixture_bytes: 100 * 1024 * 1024,
            file_count: None,
            sample_count: 32,
            budget_millis: LARGE_FILE_100MB_BUDGET_MILLIS,
            note: concat!(
                "Opens a real 100MB file through the streaming path and measures ",
                "open, a viewport projection at a deep scroll offset, and edit ",
                "latency. Gates on ADR-0048's keypress p50 (<16ms)."
            )
            .to_string(),
        }
    }

    /// M8 search-stream 50 K-file throughput + cancellation latency skeleton.
    pub fn m8_search_stream_50k() -> Self {
        Self {
            name: "m8.search_stream_50k".to_string(),
            kind: SkeletonKind::SearchStream50K,
            // fixture_bytes is not meaningful for file-count workloads; use
            // file_count instead so the field names match their units.
            fixture_bytes: 0,
            file_count: Some(SEARCH_STREAM_50K_FILE_COUNT),
            sample_count: 1,
            budget_millis: SEARCH_STREAM_50K_BUDGET_MILLIS,
            note: concat!(
                "M8 P2.F4.T4: exercises search_workspace_stream against a ",
                "50 K-file synthetic fixture generated at runtime under the ",
                "system temp directory.  Measures total scan wall-clock time ",
                "and early-cancellation latency.  Budget is 0 (report-only) ",
                "so classify_skeleton_status always returns Skipped until a ",
                "reference-machine baseline is set.  Set LEGION_PERF_FAIL_ON_BUDGET_MS ",
                "to a positive millisecond count to activate the gate.",
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
    /// Whether a measurement actually happened.
    ///
    /// `SkeletonStatus::Skipped` cannot distinguish "the budget is
    /// report-only" from "the measurement never ran", and those two need
    /// different reactions: the first is a policy choice, the second is a
    /// workload that quietly stopped existing. `verify-perf-harness` fails on
    /// `measured = false` regardless of budget strictness, which is what keeps
    /// a per-OS CI job from silently dropping a workload (P8.F4.T2).
    #[serde(default = "default_measured")]
    pub measured: bool,
    /// Result for workloads whose metric is bytes rather than time (the memory
    /// ceiling). Zero for time-valued rows. A separate field rather than
    /// overloading `total_micros`, so the field name matches its unit.
    #[serde(default)]
    pub bytes_value: u64,
    /// Whether this row is a synthetic stand-in rather than a product path.
    ///
    /// Surfaced in the report because P8.F4.T1's acceptance is about product
    /// workloads: a reader must be able to see, without reading xtask's
    /// source, which rows describe the real editor and which do not.
    #[serde(default)]
    pub synthetic_stand_in: bool,
}

/// Reports written before the `measured` field existed recorded only real
/// measurements, so absence means "measured".
fn default_measured() -> bool {
    true
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
    /// What this report measures. M0 CI harness is skeleton microbenchmarks only;
    /// full OS reference workloads remain WS18.T1 follow-on. Not a product UX proof.
    #[serde(default = "default_workload_kind")]
    pub workload_kind: String,
    /// Which OS produced this report. Archived per run so the three CI jobs'
    /// artifacts cannot be confused for one another, and so a report that was
    /// uploaded from the wrong job is visible rather than plausible.
    #[serde(default = "unknown_host")]
    pub os: String,
    #[serde(default = "unknown_host")]
    pub arch: String,
    pub summary: PerfSummary,
    pub skeletons: Vec<SkeletonMeasurement>,
}

fn default_workload_kind() -> String {
    "skeleton".to_string()
}

fn unknown_host() -> String {
    "unknown".to_string()
}

/// Host OS/arch of the process writing the report.
pub fn host_os() -> &'static str {
    std::env::consts::OS
}

pub fn host_arch() -> &'static str {
    std::env::consts::ARCH
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
        SkeletonKind::MemoryCeiling1MB => {
            return run_memory_ceiling_1mb(skeleton.fixture_bytes);
        }
        SkeletonKind::SearchStream50K => {
            return run_search_stream_50k(skeleton);
        }
        SkeletonKind::InputToPaintMicrobenchmark => {
            run_input_to_paint_microbenchmark(skeleton.fixture_bytes, skeleton.sample_count)
        }
        SkeletonKind::LineGalleyShapingCache => run_line_galley_shaping_cache_microbenchmark(
            skeleton.fixture_bytes,
            skeleton.sample_count,
        ),
        SkeletonKind::ProductWorkload => {
            // Product workloads are never planned from a descriptor; they come
            // back whole from the `product_perf` subprocess. Reaching here
            // means someone added one to the descriptor list, so say so
            // instead of inventing a measurement.
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
                message: "product workloads are supplied by the legion-app product_perf \
                          subprocess, not planned from a descriptor"
                    .to_string(),
                measured: false,
                bytes_value: 0,
                synthetic_stand_in: false,
            };
        }
        SkeletonKind::LargeFile100Mb => {
            // Supplied by the `large_file_perf` subprocess, for the same
            // reason the renderer measurement is: this harness cannot depend
            // on `legion-editor`, and a synthetic stand-in for a 100MB file
            // would measure the stand-in.
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
                message: "100MB large-file measurement is supplied by the legion-app subprocess"
                    .to_string(),
                // A planned placeholder, not a measurement: the subprocess
                // replaces this row before the report is written, and if it
                // does not, `measured = false` is exactly the signal wanted.
                measured: false,
                bytes_value: 0,
                synthetic_stand_in: false,
            };
        }
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
                measured: false,
                bytes_value: 0,
                synthetic_stand_in: false,
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
    let status = classify_skeleton_status(total, budget);
    let message = match status {
        SkeletonStatus::Skipped => "budget is 0; report-only (no gate)".to_string(),
        SkeletonStatus::Passed => {
            let budget = budget.expect("passed status implies a configured budget");
            format!(
                "total {total_millis}ms within budget {}ms",
                budget.as_millis()
            )
        }
        SkeletonStatus::Failed => {
            let budget = budget.expect("failed status implies a configured budget");
            format!(
                "total {total_millis}ms exceeded budget {}ms (p50={}us p95={}us)",
                budget.as_millis(),
                p50,
                p95,
            )
        }
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
        measured: true,
        bytes_value: 0,
        synthetic_stand_in: skeleton.kind.is_synthetic_stand_in(),
    }
}

/// Classify a measured total against an optional budget. Split out as a
/// pure function so the failure-classification path can be exercised
/// deterministically in tests without relying on host timing.
///
/// * `None` budget (report-only) -> [`SkeletonStatus::Skipped`].
/// * `total <= budget` -> [`SkeletonStatus::Passed`].
/// * `total > budget` -> [`SkeletonStatus::Failed`].
pub fn classify_skeleton_status(total: Duration, budget: Option<Duration>) -> SkeletonStatus {
    match budget {
        None => SkeletonStatus::Skipped,
        Some(budget) if total <= budget => SkeletonStatus::Passed,
        Some(_) => SkeletonStatus::Failed,
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

fn run_memory_ceiling_1mb(fixture_bytes: usize) -> SkeletonMeasurement {
    use legion_protocol::BufferVersion;
    use legion_text::TextBuffer;

    // Generate a fixture of repeating ASCII lines
    let line = "abcdefghijklmnopqrstuvwxyz0123456789_|\n"; // 39 bytes
    let line_count = fixture_bytes / line.len();
    let mut text = String::with_capacity(line_count * line.len());
    for _ in 0..line_count {
        text.push_str(line);
    }

    let buf = TextBuffer::try_with_version(text, BufferVersion(0))
        .expect("TextBuffer creation should succeed for 1MB");
    let footprint = buf.memory_footprint_bytes();

    let ceiling = MEMORY_CEILING_DEFAULT_BUDGET_BYTES;
    let (status, message) = if footprint <= ceiling {
        (
            SkeletonStatus::Passed,
            format!(
                "memory footprint {} bytes ({:.2} MB) within ceiling {} bytes ({} MB)",
                footprint,
                footprint as f64 / (1024.0 * 1024.0),
                ceiling,
                ceiling / (1024 * 1024)
            ),
        )
    } else {
        (
            SkeletonStatus::Failed,
            format!(
                "memory footprint {} bytes ({:.2} MB) exceeds ceiling {} bytes ({} MB)",
                footprint,
                footprint as f64 / (1024.0 * 1024.0),
                ceiling,
                ceiling / (1024 * 1024)
            ),
        )
    };

    SkeletonMeasurement {
        name: "m2.memory_ceiling_1mb".to_string(),
        kind: SkeletonKind::MemoryCeiling1MB,
        fixture_bytes,
        sample_count: 1,
        total_micros: 0,
        p50_micros: 0,
        p95_micros: 0,
        budget_millis: 0,
        status,
        message,
        measured: true,
        // Real `legion_text::TextBuffer`, but a generated document. The
        // product-path memory ceiling against a real file on disk is
        // `p8.memory_ceiling`; this one survives as the cheap guardrail that
        // does not need a subprocess.
        bytes_value: 0,
        synthetic_stand_in: false,
    }
}

/// P2.F4.T4 — search-stream 50 K-file throughput + cancellation workload.
///
/// Generates `file_count` small synthetic text files in a uniquely-named
/// subdirectory of the system temp dir, opens a [`WorkspaceActor`] on that
/// directory, runs a full-scan search, then runs a second search that is
/// cancelled after the first batch.  The fixture directory is cleaned up
/// regardless of whether the search succeeded or failed.
///
/// Reporting is always report-only (`Skipped`) because total scan time varies
/// widely across disk speeds.  The `message` field contains both the full-scan
/// duration and the cancellation latency so CI can track trends over time.
fn run_search_stream_50k(skeleton: &SkeletonDescriptor) -> SkeletonMeasurement {
    use std::sync::Arc;

    use legion_platform::{NativeFileSystem, NativeWatcherService};
    use legion_project::{
        ProjectFilesystemService, SearchPattern, WorkspaceActor, WorkspaceSearchFilters,
        WorkspaceSearchQuery,
    };
    use legion_protocol::{
        CanonicalPath, CorrelationId, PrincipalId, WorkspaceOpenRequest, WorkspaceTrustState,
    };
    use legion_security::DenyByDefaultBroker;

    // ── 1. Generate fixture under temp dir ───────────────────────────────────
    let pid = std::process::id();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    let fixture_root = std::env::temp_dir().join(format!("legion_perf_search_{pid}_{ts}"));

    // Use the dedicated file_count field; fixture_bytes is 0 for this skeleton kind.
    let file_count = skeleton.file_count.unwrap_or(0);
    let needle = "PERF_NEEDLE_XYZ";

    let fixture_created = (|| {
        fs::create_dir_all(&fixture_root)?;
        for i in 0..file_count {
            // Sprinkle the search needle into every 10th file (~5 000 hits).
            let content = if i % 10 == 0 {
                format!("synthetic file {i:05} contains the search target: {needle}\n")
            } else {
                format!("synthetic file {i:05} ordinary workspace content, no match here\n")
            };
            fs::write(fixture_root.join(format!("f{i:05}.txt")), content)?;
        }
        Ok::<_, std::io::Error>(())
    })();

    if let Err(err) = &fixture_created {
        return SkeletonMeasurement {
            name: skeleton.name.clone(),
            kind: skeleton.kind,
            fixture_bytes: file_count,
            sample_count: 0,
            total_micros: 0,
            p50_micros: 0,
            p95_micros: 0,
            budget_millis: skeleton.budget_millis,
            status: SkeletonStatus::Skipped,
            message: format!("fixture creation failed ({file_count} files): {err}"),
            measured: false,
            bytes_value: 0,
            synthetic_stand_in: false,
        };
    }

    // ── 2. Open workspace ────────────────────────────────────────────────────
    // `ProjectFilesystem` is a private type alias for `dyn ProjectFilesystemService`;
    // spelling out the trait is equivalent and avoids the privacy barrier.
    let actor_fs: Arc<dyn ProjectFilesystemService> = Arc::new(NativeFileSystem);
    let actor = WorkspaceActor::new(
        actor_fs,
        Arc::new(NativeWatcherService),
        DenyByDefaultBroker::default(),
    );
    let open_result = actor.open_workspace(WorkspaceOpenRequest {
        correlation_id: CorrelationId(1),
        principal_id: PrincipalId("perf".to_string()),
        root_path: CanonicalPath(fixture_root.to_string_lossy().into_owned()),
        trust: Some(WorkspaceTrustState::Trusted),
    });

    let opened = match open_result {
        Ok(o) => o,
        Err(err) => {
            let _ = fs::remove_dir_all(&fixture_root);
            return SkeletonMeasurement {
                name: skeleton.name.clone(),
                kind: skeleton.kind,
                fixture_bytes: file_count,
                sample_count: 0,
                total_micros: 0,
                p50_micros: 0,
                p95_micros: 0,
                budget_millis: skeleton.budget_millis,
                status: SkeletonStatus::Skipped,
                message: format!("workspace open failed: {err}"),
                measured: false,
                bytes_value: 0,
                synthetic_stand_in: false,
            };
        }
    };

    let pattern = match SearchPattern::literal(needle, true, false) {
        Ok(p) => p,
        Err(err) => {
            let _ = fs::remove_dir_all(&fixture_root);
            return SkeletonMeasurement {
                name: skeleton.name.clone(),
                kind: skeleton.kind,
                fixture_bytes: file_count,
                sample_count: 0,
                total_micros: 0,
                p50_micros: 0,
                p95_micros: 0,
                budget_millis: skeleton.budget_millis,
                status: SkeletonStatus::Skipped,
                message: format!("search pattern build failed: {err}"),
                measured: false,
                bytes_value: 0,
                synthetic_stand_in: false,
            };
        }
    };

    let make_query = |p: SearchPattern| WorkspaceSearchQuery {
        workspace_id: opened.workspace_id,
        pattern: p,
        search_text: needle.to_string(),
        filters: WorkspaceSearchFilters::default(),
        result_limit: usize::MAX,
        batch_size: 256,
        use_indexed_backend: false,
    };

    // ── 3. Full-scan measurement ─────────────────────────────────────────────
    let scan_start = Instant::now();
    let scan_result = actor.search_workspace_stream(
        make_query(pattern),
        |_batch| true, // consume all batches; returning false would cancel
    );
    let scan_elapsed = scan_start.elapsed();

    let hit_count = scan_result.as_ref().map(|r| r.hit_count).unwrap_or(0);

    // ── 4. Cancellation-latency measurement ──────────────────────────────────
    // Build a fresh pattern for the second query (SearchPattern is not Clone).
    let cancel_pattern = SearchPattern::literal(needle, true, false).unwrap_or_else(|_| {
        // Should not fail since we already built the same pattern above; use a
        // trivially-matching pattern as a fall-back so we still measure latency.
        SearchPattern::literal(".", false, false).expect("trivial pattern must build")
    });
    let cancel_start = Instant::now();
    let _ = actor.search_workspace_stream(
        make_query(cancel_pattern),
        |_batch| false, // cancel immediately after first batch
    );
    let cancel_elapsed = cancel_start.elapsed();

    // ── 5. Cleanup ────────────────────────────────────────────────────────────
    let _ = fs::remove_dir_all(&fixture_root);

    let total_us = scan_elapsed.as_micros() as u64;
    let cancel_us = cancel_elapsed.as_micros() as u64;
    let scan_ok = scan_result.is_ok();

    let message = format!(
        "full scan {:.0}ms ({} hits in {} files); cancellation latency {:.0}ms; ok={scan_ok}",
        scan_elapsed.as_secs_f64() * 1000.0,
        hit_count,
        file_count,
        cancel_elapsed.as_secs_f64() * 1000.0,
    );

    SkeletonMeasurement {
        name: skeleton.name.clone(),
        kind: skeleton.kind,
        fixture_bytes: file_count,
        sample_count: 1,
        total_micros: total_us,
        // Encode both timings: p50 = full-scan µs, p95 = cancel-latency µs.
        p50_micros: total_us,
        p95_micros: cancel_us,
        budget_millis: skeleton.budget_millis,
        // Delegate to classify_skeleton_status like every other skeleton.
        // With budget_millis=0 (the default), skeleton.budget() returns None
        // and this always evaluates to Skipped (report-only).  Set
        // SEARCH_STREAM_50K_BUDGET_MILLIS to a positive value (or set
        // LEGION_PERF_FAIL_ON_BUDGET_MS at runtime) to activate the gate.
        status: classify_skeleton_status(scan_elapsed, skeleton.budget()),
        message,
        measured: true,
        // Generated fixture, but the real product search stack: streaming
        // walker, native filesystem/watcher, deny-by-default broker.
        bytes_value: 0,
        synthetic_stand_in: false,
    }
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
        workload_kind: default_workload_kind(),
        os: host_os().to_string(),
        arch: host_arch().to_string(),
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
        workload_kind: default_workload_kind(),
        os: host_os().to_string(),
        arch: host_arch().to_string(),
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
        // The subprocess reports "skipped" when the host has no renderer; that
        // is a measurement that did not happen, not a report-only budget.
        measured: report.status != "skipped",
        bytes_value: 0,
        synthetic_stand_in: false,
    }
}

pub fn large_file_manual_renderer_perf_measurement(
    report: &ManualRendererPerfToml,
    fixture_bytes: usize,
    budget_millis: u64,
) -> SkeletonMeasurement {
    let p50_micros = report.keypress_p50_micros;
    let p95_micros = report.keypress_p95_micros.max(report.scroll_p95_micros);
    let status = if report.status == "skipped" {
        SkeletonStatus::Skipped
    } else if report.status == "passed" && p50_micros <= budget_millis.saturating_mul(1_000) {
        SkeletonStatus::Passed
    } else {
        SkeletonStatus::Failed
    };
    let message = if report.message.trim().is_empty() {
        format!(
            "renderer-backed 100MB file: status={} keypress_p50={}us scroll_p95={}us",
            report.status, p50_micros, report.scroll_p95_micros
        )
    } else {
        format!("renderer-backed 100MB file: {}", report.message)
    };

    SkeletonMeasurement {
        name: "m9.large_file_100mb".to_string(),
        kind: SkeletonKind::LargeFile100Mb,
        fixture_bytes,
        sample_count: report.sample_count,
        total_micros: p50_micros.saturating_add(report.scroll_p95_micros),
        p50_micros,
        p95_micros,
        budget_millis,
        status,
        message,
        measured: report.status != "skipped",
        bytes_value: 0,
        synthetic_stand_in: false,
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
    apply_fail_on_budget_value(skeleton, &value);
}

/// Apply a raw budget-override value (the contents of
/// [`FAIL_ON_BUDGET_ENV`]) to `skeleton`. Split out from
/// [`apply_fail_on_budget_override`] so tests can exercise the override
/// logic without mutating process-global environment state (which races
/// other integration tests running concurrently). A non-numeric value is
/// ignored, leaving the descriptor budget unchanged. An explicit `0`
/// disables the gate (report-only) just like setting the budget to `0`.
pub fn apply_fail_on_budget_value(skeleton: &mut SkeletonDescriptor, value: &str) {
    // GAP-09.2: renderer-backed paint rows keep their own budgets even when
    // hosted CI sets `LEGION_PERF_FAIL_ON_BUDGET_MS=0` for synthetic m0/m1.
    if matches!(
        skeleton.kind,
        SkeletonKind::RendererBackedManualInputToPaint | SkeletonKind::LargeFile100Mb
    ) {
        return;
    }
    let Ok(parsed) = value.trim().parse::<u64>() else {
        return;
    };
    skeleton.budget_millis = parsed;
}

/// Applies the [`FAIL_ON_BUDGET_ENV`] override to a measurement that did not
/// flow through a [`SkeletonDescriptor`].
///
/// GAP-09.2: renderer-backed paint rows (`manual.renderer_input_to_paint` and
/// `m9.large_file_100mb`) ignore this override, matching product workloads.
/// Hosted `LEGION_PERF_FAIL_ON_BUDGET_MS=0` stays for synthetic m0/m1 noise
/// only. A renderer budget miss must fail the gate.
pub fn apply_fail_on_budget_to_manual_measurement(measurement: &mut SkeletonMeasurement) {
    let Ok(value) = std::env::var(FAIL_ON_BUDGET_ENV) else {
        return;
    };
    apply_fail_on_budget_value_to_manual_measurement(measurement, &value);
}

/// Value-based twin of [`apply_fail_on_budget_to_manual_measurement`].
///
/// Renderer-backed kinds ignore the override. Other kinds: non-numeric values
/// are ignored; `0` means report-only (a budget failure is reclassified as
/// Skipped, measured numbers preserved); a non-zero value re-gates p95 against
/// the override in milliseconds. Already-Skipped placeholders are untouched.
pub fn apply_fail_on_budget_value_to_manual_measurement(
    measurement: &mut SkeletonMeasurement,
    value: &str,
) {
    if matches!(
        measurement.kind,
        SkeletonKind::RendererBackedManualInputToPaint | SkeletonKind::LargeFile100Mb
    ) {
        return;
    }
    let Ok(parsed) = value.trim().parse::<u64>() else {
        return;
    };
    if measurement.status == SkeletonStatus::Skipped {
        return;
    }
    measurement.budget_millis = parsed;
    if parsed == 0 {
        if measurement.status == SkeletonStatus::Failed {
            measurement.status = SkeletonStatus::Skipped;
            measurement.message = format!(
                "budget override 0; report-only (no gate). {}",
                measurement.message
            );
        }
    } else {
        let budget_micros = parsed.saturating_mul(1_000);
        measurement.status = if measurement.p95_micros > budget_micros {
            SkeletonStatus::Failed
        } else {
            SkeletonStatus::Passed
        };
    }
}

/// Spawn the `legion-desktop --manual-perf` subprocess and collect a
/// renderer-backed input-to-paint measurement.
///
/// 1. Clears any stale report file in `out_dir`.
/// 2. Spawns `cargo run -p legion-desktop --release --no-default-features
///    --features offline` with `--manual-perf` arguments.
/// 3. Reads and parses the resulting `manual_renderer_perf.toml`.
/// 4. Returns the parsed report as a [`SkeletonMeasurement`].
///
/// Errors are returned as a [`SkeletonMeasurement`] with `Skipped` or
/// `Failed` status and a diagnostic message, never as a Rust `Err`.
pub fn run_renderer_backed_manual_measurement(
    workspace_root: &Path,
    out_dir: &Path,
) -> SkeletonMeasurement {
    let budgets = manual_renderer_budgets();
    let manual_report_path = out_dir.join(MANUAL_RENDERER_PERF_REPORT_FILE);

    // Clear stale report so a leftover from a previous run cannot be
    // mistaken for this run's output.
    match fs::remove_file(&manual_report_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return placeholder_manual_measurement(
                SkeletonStatus::Failed,
                format!(
                    "renderer-backed Manual measurement failed: unable to clear stale report `{}`: {err}",
                    manual_report_path.display()
                ),
            );
        }
    }

    let sample_count = budgets.sample_count.to_string();
    let output = std::process::Command::new("cargo")
        .current_dir(workspace_root)
        .args([
            "run",
            "--release",
            "-p",
            "legion-desktop",
            "--no-default-features",
            "--features",
            "offline",
            "--",
            "--manual-perf",
            "--workspace",
            ".",
            "--file",
            "Cargo.toml",
            "--perf-report",
        ])
        .arg(&manual_report_path)
        .args(["--perf-samples", &sample_count])
        .output();

    match output {
        Err(err) => placeholder_manual_measurement(
            SkeletonStatus::Skipped,
            format!(
                "renderer-backed Manual measurement blocked: unable to spawn cargo release/offline desktop subprocess: {err}"
            ),
        ),
        Ok(output) => {
            if !output.status.success() {
                eprintln!(
                    "perf harness: Manual renderer subprocess exited with status {}",
                    output.status
                );
            }
            match read_manual_renderer_perf_report(&manual_report_path) {
                Ok(manual_report) => manual_renderer_perf_measurement(&manual_report),
                Err(read_err) => {
                    eprintln!("perf harness: {read_err}");
                    let output_text = subprocess_output_text(&output);
                    if !output.status.success() && manual_renderer_environment_blocked(&output_text)
                    {
                        placeholder_manual_measurement(
                            SkeletonStatus::Skipped,
                            format!(
                                "renderer-backed Manual measurement blocked: {}",
                                truncate_report_message(&output_text)
                            ),
                        )
                    } else if !output.status.success() && manual_renderer_build_failed(&output_text)
                    {
                        placeholder_manual_measurement(
                            SkeletonStatus::Skipped,
                            format!(
                                "renderer-backed Manual measurement skipped: desktop build failed{}",
                                command_output_suffix(&output_text)
                            ),
                        )
                    } else {
                        placeholder_manual_measurement(
                            SkeletonStatus::Failed,
                            format!(
                                "renderer-backed Manual measurement failed: {read_err}{}",
                                command_output_suffix(&output_text)
                            ),
                        )
                    }
                }
            }
        }
    }
}

pub fn run_renderer_backed_large_file_measurement(
    workspace_root: &Path,
    out_dir: &Path,
    initial_file: &Path,
    fixture_bytes: usize,
    budget_millis: u64,
) -> SkeletonMeasurement {
    let report_path = out_dir.join(LARGE_FILE_MANUAL_RENDERER_PERF_REPORT_FILE);
    match fs::remove_file(&report_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return placeholder_large_file_manual_measurement(
                SkeletonStatus::Failed,
                fixture_bytes,
                budget_millis,
                format!(
                    "unable to clear stale report `{}`: {err}",
                    report_path.display()
                ),
            );
        }
    }

    let sample_count = MANUAL_RENDERER_SAMPLE_COUNT.to_string();
    let output = std::process::Command::new("cargo")
        .current_dir(workspace_root)
        .args([
            "run",
            "--release",
            "-p",
            "legion-desktop",
            "--no-default-features",
            "--features",
            "offline",
            "--",
            "--manual-perf",
            "--workspace",
        ])
        .arg(workspace_root)
        .args(["--file"])
        .arg(initial_file)
        .args(["--perf-report"])
        .arg(&report_path)
        .args(["--perf-samples", &sample_count])
        .output();

    match output {
        Err(err) => placeholder_large_file_manual_measurement(
            SkeletonStatus::Skipped,
            fixture_bytes,
            budget_millis,
            format!("unable to spawn renderer-backed desktop subprocess: {err}"),
        ),
        Ok(output) => match read_manual_renderer_perf_report(&report_path) {
            Ok(report) => {
                large_file_manual_renderer_perf_measurement(&report, fixture_bytes, budget_millis)
            }
            Err(read_err) => {
                let output_text = subprocess_output_text(&output);
                let status = if !output.status.success()
                    && (manual_renderer_environment_blocked(&output_text)
                        || manual_renderer_build_failed(&output_text))
                {
                    SkeletonStatus::Skipped
                } else {
                    SkeletonStatus::Failed
                };
                placeholder_large_file_manual_measurement(
                    status,
                    fixture_bytes,
                    budget_millis,
                    format!(
                        "renderer-backed 100MB report unavailable: {read_err}{}",
                        command_output_suffix(&output_text)
                    ),
                )
            }
        },
    }
}

pub fn placeholder_large_file_manual_measurement(
    status: SkeletonStatus,
    fixture_bytes: usize,
    budget_millis: u64,
    message: String,
) -> SkeletonMeasurement {
    SkeletonMeasurement {
        name: "m9.large_file_100mb".to_string(),
        kind: SkeletonKind::LargeFile100Mb,
        fixture_bytes,
        sample_count: MANUAL_RENDERER_SAMPLE_COUNT,
        total_micros: 0,
        p50_micros: 0,
        p95_micros: 0,
        budget_millis,
        status,
        message,
        measured: false,
        bytes_value: 0,
        synthetic_stand_in: false,
    }
}

/// Build a placeholder measurement for the renderer-backed manual skeleton
/// when the subprocess cannot run or its report cannot be read.
pub fn placeholder_manual_measurement(
    status: SkeletonStatus,
    message: String,
) -> SkeletonMeasurement {
    let budgets = manual_renderer_budgets();
    SkeletonMeasurement {
        name: "manual.renderer_input_to_paint".to_string(),
        kind: SkeletonKind::RendererBackedManualInputToPaint,
        fixture_bytes: 0,
        sample_count: budgets.sample_count,
        total_micros: 0,
        p50_micros: 0,
        p95_micros: 0,
        budget_millis: budgets.keypress_p95_millis.max(budgets.scroll_p95_millis),
        status,
        message,
        measured: false,
        bytes_value: 0,
        synthetic_stand_in: false,
    }
}

fn subprocess_output_text(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("stdout:\n{stdout}\nstderr:\n{stderr}")
}

fn truncate_report_message(message: &str) -> String {
    let normalized = message.replace("\r\n", "\n");
    let trimmed = normalized.trim();
    const LIMIT: usize = 800;
    if trimmed.chars().count() <= LIMIT {
        trimmed.to_string()
    } else {
        format!("{}...", trimmed.chars().take(LIMIT).collect::<String>())
    }
}

fn command_output_suffix(output_text: &str) -> String {
    let output_text = truncate_report_message(output_text);
    if output_text.is_empty() {
        String::new()
    } else {
        format!("; subprocess output: {output_text}")
    }
}

/// Returns `true` when `output_text` contains patterns characteristic of a
/// Rust/Cargo build failure. A build failure means the renderer binary could
/// not be compiled at all and the measurement should be classified as
/// `Skipped` rather than `Failed`.
pub fn manual_renderer_build_failed(output_text: &str) -> bool {
    let lower = output_text.to_ascii_lowercase();
    lower.contains("could not compile")
        || lower.contains("error[e")
        || lower.contains("aborting due to")
}

/// Returns `true` when the subprocess output suggests that the host
/// environment lacks a renderer/display/GPU, making the manual measurement
/// impossible (headless CI, remote runner without a display server, etc.).
pub fn manual_renderer_environment_blocked(output_text: &str) -> bool {
    let lower = output_text.to_ascii_lowercase();
    let renderer_context = lower.contains("renderer")
        || lower.contains("native")
        || lower.contains("window")
        || lower.contains("display")
        || lower.contains("gpu");
    let blocked_context = lower.contains("blocked")
        || lower.contains("unavailable")
        || lower.contains("not available")
        || lower.contains("headless")
        || lower.contains("display not set")
        || lower.contains("no display")
        || lower.contains("no available display")
        || lower.contains("renderer unavailable")
        || lower.contains("native window unavailable")
        || lower.contains("gpu unavailable");
    renderer_context && blocked_context
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

/// Numbers reported by the `large_file_perf` subprocess.
#[derive(Debug, Clone, Deserialize)]
pub struct LargeFilePerfReport {
    /// Fixture size actually measured.
    pub byte_len: usize,
    /// Whether the buffer opened through the streaming path.
    ///
    /// Recorded because the whole measurement is about the streaming path: if
    /// this is false the numbers describe something else and must not be read
    /// as a large-file result.
    pub streaming: bool,
    /// Time to open the file.
    pub open_millis: f64,
    /// One viewport projection at a deep scroll offset.
    pub viewport_millis: f64,
    /// Median edit latency.
    pub edit_p50_millis: f64,
    /// 95th-percentile edit latency.
    pub edit_p95_millis: f64,
    /// Bytes of text the viewport actually carried.
    pub viewport_payload_bytes: usize,
}

/// Turn the subprocess report into a measurement gated on the keypress budget.
///
/// The reported percentiles are the *edit* percentiles, because typing is what
/// the budget is about. Open and viewport times ride along in the message:
/// they matter, but a single status cannot answer for three different
/// questions, and typing is the one that decides usability.
pub fn large_file_perf_measurement(
    descriptor: &SkeletonDescriptor,
    report: &LargeFilePerfReport,
) -> SkeletonMeasurement {
    let p50_micros = (report.edit_p50_millis * 1000.0).round() as u64;
    let p95_micros = (report.edit_p95_millis * 1000.0).round() as u64;
    let budget_micros = descriptor.budget_millis.saturating_mul(1000);

    let status = if !report.streaming {
        SkeletonStatus::Skipped
    } else if descriptor.budget_millis == 0 || p50_micros <= budget_micros {
        SkeletonStatus::Passed
    } else {
        SkeletonStatus::Failed
    };

    let message = if report.streaming {
        format!(
            "100MB streaming open={:.1}ms viewport={:.1}ms edit_p50={:.1}ms edit_p95={:.1}ms \
             viewport_payload={}B (budget: keypress p50 <{}ms)",
            report.open_millis,
            report.viewport_millis,
            report.edit_p50_millis,
            report.edit_p95_millis,
            report.viewport_payload_bytes,
            descriptor.budget_millis,
        )
    } else {
        "100MB measurement did not open through the streaming path; numbers describe \
         a different code path and are not reported as a large-file result"
            .to_string()
    };

    SkeletonMeasurement {
        name: descriptor.name.clone(),
        kind: descriptor.kind,
        fixture_bytes: report.byte_len,
        sample_count: descriptor.sample_count,
        total_micros: p50_micros.saturating_mul(descriptor.sample_count as u64),
        p50_micros,
        p95_micros,
        budget_millis: descriptor.budget_millis,
        status,
        message,
        // A non-streaming open means the subprocess measured a different code
        // path, so there is no large-file measurement to report.
        measured: report.streaming,
        bytes_value: 0,
        synthetic_stand_in: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── IMP-4: classify_skeleton_status delegation ────────────────────────────

    /// Verify that `classify_skeleton_status` drives the status for the 50K
    /// skeleton: a zero budget (the default) → Skipped; a positive budget →
    /// Passed or Failed depending on elapsed time.
    #[test]
    fn search_stream_50k_classify_skeleton_status_report_only_by_default() {
        let skeleton = SkeletonDescriptor::m8_search_stream_50k();
        // Default budget is 0 → report-only.
        let status = classify_skeleton_status(Duration::from_millis(9999), skeleton.budget());
        assert_eq!(status, SkeletonStatus::Skipped);
    }

    #[test]
    fn search_stream_50k_env_override_activates_gate_failed() {
        let mut skeleton = SkeletonDescriptor::m8_search_stream_50k();
        // Simulate the env override: budget = 1 ms.
        apply_fail_on_budget_value(&mut skeleton, "1");
        assert_eq!(skeleton.budget(), Some(Duration::from_millis(1)));
        // A scan that took 100 ms exceeds budget → Failed.
        let status = classify_skeleton_status(Duration::from_millis(100), skeleton.budget());
        assert_eq!(status, SkeletonStatus::Failed);
    }

    #[test]
    fn search_stream_50k_env_override_activates_gate_passed() {
        let mut skeleton = SkeletonDescriptor::m8_search_stream_50k();
        // Simulate the env override: budget = 60_000 ms (generous).
        apply_fail_on_budget_value(&mut skeleton, "60000");
        // A scan that took 1 ms is within budget → Passed.
        let status = classify_skeleton_status(Duration::from_millis(1), skeleton.budget());
        assert_eq!(status, SkeletonStatus::Passed);
    }

    // ── MIN-2: file_count field is properly used ──────────────────────────────

    #[test]
    fn m8_search_stream_50k_descriptor_uses_file_count_field() {
        let skeleton = SkeletonDescriptor::m8_search_stream_50k();
        // fixture_bytes must NOT carry the file count; file_count must.
        assert_eq!(
            skeleton.fixture_bytes, 0,
            "fixture_bytes should be 0 for file-count skeletons"
        );
        assert_eq!(
            skeleton.file_count,
            Some(SEARCH_STREAM_50K_FILE_COUNT),
            "file_count must carry the file count"
        );
    }

    #[test]
    fn large_file_renderer_measurement_gates_keypress_p50_and_marks_real_data() {
        let report = ManualRendererPerfToml {
            schema_version: 1,
            scenario: MANUAL_RENDERER_SCENARIO.to_string(),
            status: "passed".to_string(),
            sample_count: 16,
            keypress_p50_micros: 17_000,
            keypress_p95_micros: 24_000,
            scroll_p95_micros: 19_000,
            keypress_p50_budget_ms: 16,
            keypress_p95_budget_ms: 32,
            scroll_p95_budget_ms: 32,
            message: "renderer completed".to_string(),
        };

        let measurement =
            large_file_manual_renderer_perf_measurement(&report, 100 * 1024 * 1024, 16);

        assert_eq!(measurement.status, SkeletonStatus::Failed);
        assert!(measurement.measured);
        assert!(!measurement.synthetic_stand_in);
        assert_eq!(measurement.p50_micros, 17_000);
        assert_eq!(measurement.fixture_bytes, 100 * 1024 * 1024);
        assert_eq!(measurement.bytes_value, 0);
    }

    #[test]
    fn large_file_renderer_measurement_keeps_headless_result_report_only() {
        let report = ManualRendererPerfToml {
            schema_version: 1,
            scenario: MANUAL_RENDERER_SCENARIO.to_string(),
            status: "skipped".to_string(),
            sample_count: 16,
            keypress_p50_micros: 0,
            keypress_p95_micros: 0,
            scroll_p95_micros: 0,
            keypress_p50_budget_ms: 16,
            keypress_p95_budget_ms: 32,
            scroll_p95_budget_ms: 32,
            message: "renderer unavailable".to_string(),
        };

        let measurement =
            large_file_manual_renderer_perf_measurement(&report, 100 * 1024 * 1024, 16);

        assert_eq!(measurement.status, SkeletonStatus::Skipped);
        assert!(!measurement.measured);
        assert!(measurement.message.contains("renderer-backed 100MB file"));
    }

    // ── MIN-1: fuzzy_score_tuple wrapper ─────────────────────────────────────

    // (Covered by crates/legion-index/src/fuzzy.rs#tuple_adapter_returns_tuple)
}
