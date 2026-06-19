//! Desktop-owned Manual mode renderer performance report writer.

use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use legion_app::AppProductMode;
use legion_protocol::{BufferId, TextCoordinate, ViewportScroll};
use legion_ui::ShellProjectionSnapshot;

use crate::{
    bridge::DesktopAction,
    workflow::{DesktopLaunchConfig, DesktopRuntime},
};

/// Stable scenario name used by the Manual renderer performance report.
pub const MANUAL_RENDERER_SCENARIO: &str = "manual_editor_input_to_paint";

/// Default Manual renderer report path used by the desktop CLI.
pub const DEFAULT_MANUAL_RENDERER_REPORT_PATH: &str =
    "target/perf-harness/manual_renderer_perf.toml";

/// Default number of renderer-backed samples collected by the desktop CLI.
pub const DEFAULT_MANUAL_RENDERER_SAMPLE_COUNT: usize = 16;

/// Default keypress p50 budget, in milliseconds.
pub const DEFAULT_KEYPRESS_P50_BUDGET_MS: u64 = 16;

/// Default keypress p95 budget, in milliseconds.
pub const DEFAULT_KEYPRESS_P95_BUDGET_MS: u64 = 32;

/// Default scroll p95 budget, in milliseconds.
pub const DEFAULT_SCROLL_P95_BUDGET_MS: u64 = 32;

/// Configuration for a desktop-owned Manual renderer performance run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualPerfConfig {
    /// Workspace root to open through app authority.
    pub workspace_root: PathBuf,
    /// Optional initial file to open before measuring editor interactions.
    pub initial_file: Option<PathBuf>,
    /// TOML report path to write.
    pub report_path: PathBuf,
    /// Number of keypress and scroll samples to collect.
    pub sample_count: usize,
    /// Keypress p50 budget, in milliseconds.
    pub keypress_p50_budget_ms: u64,
    /// Keypress p95 budget, in milliseconds.
    pub keypress_p95_budget_ms: u64,
    /// Scroll p95 budget, in milliseconds.
    pub scroll_p95_budget_ms: u64,
}

impl ManualPerfConfig {
    /// Validate and build a Manual renderer performance run configuration.
    pub fn new(
        workspace_root: PathBuf,
        initial_file: Option<PathBuf>,
        report_path: PathBuf,
        sample_count: usize,
        keypress_p50_budget_ms: u64,
        keypress_p95_budget_ms: u64,
        scroll_p95_budget_ms: u64,
    ) -> Result<Self> {
        if report_path.as_os_str().is_empty() {
            return Err(anyhow!("manual perf report path cannot be empty"));
        }
        if sample_count == 0 {
            return Err(anyhow!(
                "manual perf sample count must be greater than zero"
            ));
        }
        if keypress_p50_budget_ms == 0 || keypress_p95_budget_ms == 0 || scroll_p95_budget_ms == 0 {
            return Err(anyhow!("manual perf budgets must be greater than zero"));
        }

        Ok(Self {
            workspace_root,
            initial_file,
            report_path,
            sample_count,
            keypress_p50_budget_ms,
            keypress_p95_budget_ms,
            scroll_p95_budget_ms,
        })
    }
}

/// Metadata-only Manual renderer performance report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualPerfReport {
    /// Report schema version.
    pub schema_version: u32,
    /// Stable measured scenario identifier.
    pub scenario: String,
    /// Report status: `passed`, `failed`, or `skipped`.
    pub status: String,
    /// Workspace root used for the measured run.
    pub workspace_root: PathBuf,
    /// Optional initial file opened before measuring.
    pub initial_file: Option<PathBuf>,
    /// Report path requested by the caller.
    pub report_path: PathBuf,
    /// Number of samples requested for each measured path.
    pub sample_count: usize,
    /// Keypress input-to-paint p50, in microseconds.
    pub keypress_p50_micros: u64,
    /// Keypress input-to-paint p95, in microseconds.
    pub keypress_p95_micros: u64,
    /// Scroll-to-paint p95, in microseconds.
    pub scroll_p95_micros: u64,
    /// Keypress p50 budget, in milliseconds.
    pub keypress_p50_budget_ms: u64,
    /// Keypress p95 budget, in milliseconds.
    pub keypress_p95_budget_ms: u64,
    /// Scroll p95 budget, in milliseconds.
    pub scroll_p95_budget_ms: u64,
    /// Human-readable summary or blocker.
    pub message: String,
}

impl ManualPerfReport {
    /// Serialize the report as deterministic TOML.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let initial_file = self
            .initial_file
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        format!(
            concat!(
                "schema_version = {schema_version}\n",
                "scenario = {scenario}\n",
                "status = {status}\n",
                "workspace_root = {workspace_root}\n",
                "initial_file = {initial_file}\n",
                "report_path = {report_path}\n",
                "sample_count = {sample_count}\n",
                "keypress_p50_micros = {keypress_p50_micros}\n",
                "keypress_p95_micros = {keypress_p95_micros}\n",
                "scroll_p95_micros = {scroll_p95_micros}\n",
                "keypress_p50_budget_ms = {keypress_p50_budget_ms}\n",
                "keypress_p95_budget_ms = {keypress_p95_budget_ms}\n",
                "scroll_p95_budget_ms = {scroll_p95_budget_ms}\n",
                "message = {message}\n",
            ),
            schema_version = self.schema_version,
            scenario = toml_string(&self.scenario),
            status = toml_string(&self.status),
            workspace_root = toml_string(&self.workspace_root.display().to_string()),
            initial_file = toml_string(&initial_file),
            report_path = toml_string(&self.report_path.display().to_string()),
            sample_count = self.sample_count,
            keypress_p50_micros = self.keypress_p50_micros,
            keypress_p95_micros = self.keypress_p95_micros,
            scroll_p95_micros = self.scroll_p95_micros,
            keypress_p50_budget_ms = self.keypress_p50_budget_ms,
            keypress_p95_budget_ms = self.keypress_p95_budget_ms,
            scroll_p95_budget_ms = self.scroll_p95_budget_ms,
            message = toml_string(&self.message),
        )
    }

    /// Write the TOML report, creating parent directories when needed.
    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_toml())?;
        Ok(())
    }

    fn from_measurements(config: &ManualPerfConfig, measurements: ManualPerfMeasurements) -> Self {
        let keypress_p50_budget_micros = config.keypress_p50_budget_ms.saturating_mul(1_000);
        let keypress_p95_budget_micros = config.keypress_p95_budget_ms.saturating_mul(1_000);
        let scroll_p95_budget_micros = config.scroll_p95_budget_ms.saturating_mul(1_000);
        let passed = measurements.keypress_p50_micros <= keypress_p50_budget_micros
            && measurements.keypress_p95_micros <= keypress_p95_budget_micros
            && measurements.scroll_p95_micros <= scroll_p95_budget_micros;
        let status = if passed { "passed" } else { "failed" };
        let message = if passed {
            "desktop egui projection render path stayed within Manual latency budgets".to_string()
        } else {
            format!(
                "Manual renderer measurement exceeded budget: keypress_p50={}us/{}us, keypress_p95={}us/{}us, scroll_p95={}us/{}us",
                measurements.keypress_p50_micros,
                keypress_p50_budget_micros,
                measurements.keypress_p95_micros,
                keypress_p95_budget_micros,
                measurements.scroll_p95_micros,
                scroll_p95_budget_micros
            )
        };

        Self {
            schema_version: 1,
            scenario: MANUAL_RENDERER_SCENARIO.to_string(),
            status: status.to_string(),
            workspace_root: config.workspace_root.clone(),
            initial_file: config.initial_file.clone(),
            report_path: config.report_path.clone(),
            sample_count: measurements.sample_count,
            keypress_p50_micros: measurements.keypress_p50_micros,
            keypress_p95_micros: measurements.keypress_p95_micros,
            scroll_p95_micros: measurements.scroll_p95_micros,
            keypress_p50_budget_ms: config.keypress_p50_budget_ms,
            keypress_p95_budget_ms: config.keypress_p95_budget_ms,
            scroll_p95_budget_ms: config.scroll_p95_budget_ms,
            message,
        }
    }

    fn failed(config: &ManualPerfConfig, message: String) -> Self {
        Self {
            schema_version: 1,
            scenario: MANUAL_RENDERER_SCENARIO.to_string(),
            status: "failed".to_string(),
            workspace_root: config.workspace_root.clone(),
            initial_file: config.initial_file.clone(),
            report_path: config.report_path.clone(),
            sample_count: config.sample_count,
            keypress_p50_micros: 0,
            keypress_p95_micros: 0,
            scroll_p95_micros: 0,
            keypress_p50_budget_ms: config.keypress_p50_budget_ms,
            keypress_p95_budget_ms: config.keypress_p95_budget_ms,
            scroll_p95_budget_ms: config.scroll_p95_budget_ms,
            message,
        }
    }
}

/// Run the Manual renderer performance measurement and write its TOML report.
pub fn run_manual_perf(config: ManualPerfConfig) -> Result<()> {
    let report = match measure_manual_perf(&config) {
        Ok(measurements) => ManualPerfReport::from_measurements(&config, measurements),
        Err(error) => ManualPerfReport::failed(
            &config,
            format!("Manual renderer measurement failed: {error}"),
        ),
    };
    report.write(&config.report_path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManualPerfMeasurements {
    sample_count: usize,
    keypress_p50_micros: u64,
    keypress_p95_micros: u64,
    scroll_p95_micros: u64,
}

fn measure_manual_perf(config: &ManualPerfConfig) -> Result<ManualPerfMeasurements> {
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        config.workspace_root.clone(),
        config
            .initial_file
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
    ))?;
    runtime.set_product_mode(AppProductMode::Manual)?;
    let renderer = ManualPerfRenderer::default();
    renderer.render_once(&mut runtime)?;

    let mut keypress_samples = Vec::with_capacity(config.sample_count);
    for sample_index in 0..config.sample_count {
        let snapshot = runtime.projection_snapshot();
        let cursor = projected_cursor(&snapshot);
        let input = ((b'a' + (sample_index % 26) as u8) as char).to_string();
        let started_at = Instant::now();
        runtime.handle_action(DesktopAction::InsertText {
            text: input,
            at: cursor,
        })?;
        renderer.render_once(&mut runtime)?;
        keypress_samples.push(started_at.elapsed());
    }

    let mut scroll_samples = Vec::with_capacity(config.sample_count);
    for sample_index in 0..config.sample_count {
        let snapshot = runtime.projection_snapshot();
        let buffer_id = active_buffer(&snapshot)?;
        let started_at = Instant::now();
        runtime.handle_action(DesktopAction::SetViewportScroll {
            buffer_id: Some(buffer_id),
            scroll: ViewportScroll {
                top_line: sample_index as u32,
                left_column: 0,
            },
        })?;
        renderer.render_once(&mut runtime)?;
        scroll_samples.push(started_at.elapsed());
    }

    keypress_samples.sort_unstable();
    scroll_samples.sort_unstable();
    Ok(ManualPerfMeasurements {
        sample_count: config.sample_count,
        keypress_p50_micros: percentile_micros(&keypress_samples, 50),
        keypress_p95_micros: percentile_micros(&keypress_samples, 95),
        scroll_p95_micros: percentile_micros(&scroll_samples, 95),
    })
}

#[derive(Default)]
struct ManualPerfRenderer {
    context: egui::Context,
}

impl ManualPerfRenderer {
    fn render_once(&self, runtime: &mut DesktopRuntime) -> Result<()> {
        runtime.render_projection_once_for_perf(&self.context)
    }
}

fn active_buffer(snapshot: &ShellProjectionSnapshot) -> Result<BufferId> {
    snapshot
        .active_buffer_projection
        .buffer_id
        .ok_or_else(|| anyhow!("Manual perf requires an active editor buffer"))
}

fn projected_cursor(snapshot: &ShellProjectionSnapshot) -> TextCoordinate {
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

fn percentile_micros(sorted: &[Duration], percentile: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((percentile as f64 / 100.0) * sorted.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[index].as_micros() as u64
}

fn toml_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(escaped, "\\u{:04X}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}
