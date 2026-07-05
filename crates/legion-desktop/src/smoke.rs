//! Renderer smoke harness boundary.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use legion_protocol::PRODUCT_NAME;
use legion_ui::ShellProjectionSnapshot;

use crate::{
    metrics::{FrameTimingRecorder, FrameTimingSummary},
    platform::{
        DesktopPlatformAdapterChecks, DesktopPlatformSmokeSnapshot, NativePlatformObservation,
        build_platform_adapter_checks, build_platform_smoke_snapshot,
    },
    view::ProjectionView,
    workflow::{DesktopLaunchConfig, DesktopRuntime, desktop_native_options},
};

const NOT_OBSERVED: &str = "not observed";

/// Display label for the non-native-window GUI Phase 7 beta smoke harness.
pub const GUI_PHASE7_BETA_SMOKE_LABEL: &str = "GUI Phase 7 beta smoke";

/// Smoke-mode launch configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererSmokeConfig {
    /// Timed smoke duration in milliseconds.
    pub duration_ms: u64,
    /// Evidence markdown output path.
    pub evidence_path: PathBuf,
}

impl RendererSmokeConfig {
    /// Create a smoke config.
    pub fn new(duration_ms: u64, evidence_path: PathBuf) -> Result<Self> {
        if duration_ms == 0 {
            return Err(anyhow!("smoke duration must be greater than zero"));
        }
        Ok(Self {
            duration_ms,
            evidence_path,
        })
    }
}

/// Overall native-window smoke status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererSmokeStatus {
    /// Native window opened, ran, closed, and evidence was written.
    Passed,
    /// Native window smoke could not run in the current environment.
    Blocked,
    /// Native window opened but a smoke assertion failed.
    Failed,
}

impl RendererSmokeStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
        }
    }
}

/// Metadata-only renderer/platform smoke report.
#[derive(Debug, Clone, PartialEq)]
pub struct RendererSmokeReport {
    /// Command used to produce this report.
    pub command: String,
    /// Overall smoke status.
    pub status: RendererSmokeStatus,
    /// Workspace root used by the smoke run.
    pub workspace: PathBuf,
    /// Optional opened file.
    pub file: Option<String>,
    /// Requested smoke duration in milliseconds.
    pub duration_ms: u64,
    /// Timing summary.
    pub timing: FrameTimingSummary,
    /// Focus smoke status.
    pub focus_smoke: String,
    /// Menu smoke status.
    pub menu_smoke: String,
    /// Keyboard shortcut smoke status.
    pub shortcut_smoke: String,
    /// Clipboard smoke status.
    pub clipboard_smoke: String,
    /// IME smoke status.
    pub ime_smoke: String,
    /// Theme smoke status.
    pub theme_smoke: String,
    /// High-DPI smoke status.
    pub high_dpi_smoke: String,
    /// Focus traversal smoke status.
    pub focus_traversal_smoke: String,
    /// File-dialog smoke status.
    pub file_dialog_smoke: String,
    /// Accessibility smoke status.
    pub accessibility_smoke: String,
    /// Metadata-only accessibility projection status.
    pub accessibility_tree_smoke: String,
    /// Count of metadata-only accessibility projection nodes.
    pub accessibility_projection_node_count: usize,
    /// Large-file degraded projection status.
    pub large_file_degraded_status: String,
    /// Bounded degraded search status.
    pub bounded_search_status: String,
    /// Full-text projection avoidance status.
    pub full_text_projection_status: String,
    /// Errors or blockers observed during the smoke run.
    pub errors: Vec<String>,
}

impl RendererSmokeReport {
    /// Build a blocked report for a failed native-window attempt.
    pub fn blocked(
        command: String,
        config: &DesktopLaunchConfig,
        smoke: &RendererSmokeConfig,
        error: impl Into<String>,
    ) -> Self {
        Self {
            command,
            status: RendererSmokeStatus::Blocked,
            workspace: config.workspace_root.clone(),
            file: config.initial_file.clone(),
            duration_ms: smoke.duration_ms,
            timing: FrameTimingSummary::default(),
            focus_smoke: NOT_OBSERVED.to_string(),
            menu_smoke: NOT_OBSERVED.to_string(),
            shortcut_smoke: NOT_OBSERVED.to_string(),
            clipboard_smoke: NOT_OBSERVED.to_string(),
            ime_smoke: NOT_OBSERVED.to_string(),
            theme_smoke: NOT_OBSERVED.to_string(),
            high_dpi_smoke: NOT_OBSERVED.to_string(),
            focus_traversal_smoke: NOT_OBSERVED.to_string(),
            file_dialog_smoke: NOT_OBSERVED.to_string(),
            accessibility_smoke: NOT_OBSERVED.to_string(),
            accessibility_tree_smoke: NOT_OBSERVED.to_string(),
            accessibility_projection_node_count: 0,
            large_file_degraded_status: NOT_OBSERVED.to_string(),
            bounded_search_status: NOT_OBSERVED.to_string(),
            full_text_projection_status: NOT_OBSERVED.to_string(),
            errors: vec![error.into()],
        }
    }

    /// Render the report as stable markdown evidence.
    pub fn to_markdown(&self) -> String {
        let errors = if self.errors.is_empty() {
            "- none".to_string()
        } else {
            self.errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!(
            concat!(
                "# Renderer Smoke Evidence\n\n",
                "## Status\n\n",
                "status: {status}\n",
                "workspace: {workspace}\n",
                "file: {file}\n",
                "duration_ms: {duration_ms}\n\n",
                "## Command\n\n",
                "`{command}`\n\n",
                "## Timing\n\n",
                "sample_count: {sample_count}\n",
                "p50_input_to_paint_ms: {p50:.3}\n",
                "p95_input_to_paint_ms: {p95:.3}\n",
                "frame_count: {frame_count}\n",
                "average_frame_ms: {average_frame:.3}\n",
                "frame_variance_ms2: {variance:.3}\n\n",
                "## Platform Smoke\n\n",
                "focus_smoke: {focus}\n",
                "menu_smoke: {menu}\n",
                "shortcut_smoke: {shortcut}\n",
                "clipboard_smoke: {clipboard}\n",
                "ime_smoke: {ime}\n",
                "theme_smoke: {theme}\n",
                "high_dpi_smoke: {high_dpi}\n",
                "focus_traversal_smoke: {focus_traversal}\n",
                "file_dialog_smoke: {file_dialog}\n",
                "accessibility_smoke: {accessibility}\n",
                "accessibility_tree_smoke: {accessibility_tree}\n",
                "accessibility_projection_node_count: {accessibility_node_count}\n\n",
                "## Large File Guardrails\n\n",
                "large_file_degraded_status: {large_file_degraded_status}\n",
                "bounded_search_status: {bounded_search_status}\n",
                "full_text_projection_status: {full_text_projection_status}\n\n",
                "## Errors\n\n",
                "{errors}\n\n",
                "## Residual Risk\n\n",
                "- Clipboard, IME, and file-dialog checks are adapter-path smoke unless an OS-observed status says otherwise.\n",
                "- Accessibility and high-DPI status are not promoted beyond the observation recorded above.\n"
            ),
            status = self.status.as_str(),
            workspace = self.workspace.display(),
            file = self.file.as_deref().unwrap_or("<none>"),
            duration_ms = self.duration_ms,
            command = self.command,
            sample_count = self.timing.sample_count,
            p50 = self.timing.p50_input_to_paint_ms,
            p95 = self.timing.p95_input_to_paint_ms,
            frame_count = self.timing.frame_count,
            average_frame = self.timing.average_frame_ms,
            variance = self.timing.frame_variance_ms2,
            focus = self.focus_smoke,
            menu = self.menu_smoke,
            shortcut = self.shortcut_smoke,
            clipboard = self.clipboard_smoke,
            ime = self.ime_smoke,
            theme = self.theme_smoke,
            high_dpi = self.high_dpi_smoke,
            focus_traversal = self.focus_traversal_smoke,
            file_dialog = self.file_dialog_smoke,
            accessibility = self.accessibility_smoke,
            accessibility_tree = self.accessibility_tree_smoke,
            accessibility_node_count = self.accessibility_projection_node_count,
            large_file_degraded_status = self.large_file_degraded_status,
            bounded_search_status = self.bounded_search_status,
            full_text_projection_status = self.full_text_projection_status,
            errors = errors,
        )
    }

    /// Write the markdown evidence file, creating the exact parent directory if needed.
    pub fn write_evidence(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_markdown())?;
        Ok(())
    }
}

/// Run timed native-window smoke and write evidence.
pub fn run_smoke(config: DesktopLaunchConfig, smoke: RendererSmokeConfig) -> Result<()> {
    let command = smoke_command(&config, &smoke);
    match run_smoke_window(config.clone(), smoke.clone(), command.clone()) {
        Ok(report) => {
            report.write_evidence(&smoke.evidence_path)?;
            Ok(())
        }
        Err(error) => {
            let report = RendererSmokeReport::blocked(command, &config, &smoke, error.to_string());
            report.write_evidence(&smoke.evidence_path)?;
            Err(error)
        }
    }
}

fn run_smoke_window(
    config: DesktopLaunchConfig,
    smoke: RendererSmokeConfig,
    command: String,
) -> Result<RendererSmokeReport> {
    let recorder = Arc::new(Mutex::new(FrameTimingRecorder::new()));
    let observations = Arc::new(Mutex::new(SmokeObservations::default()));
    let recorder_for_app = Arc::clone(&recorder);
    let observations_for_app = Arc::clone(&observations);
    let duration = Duration::from_millis(smoke.duration_ms);
    let report_config = config.clone();

    let smoke_title = format!("{PRODUCT_NAME} Smoke");
    let native_options = desktop_native_options(&smoke_title);

    eframe::run_native(
        &smoke_title,
        native_options,
        Box::new(move |_cc| {
            let runtime = DesktopRuntime::open(config)
                .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { error.into() })?;
            if let Ok(mut observations) = observations_for_app.lock() {
                let snapshot = runtime.projection_snapshot();
                let adapter_checks = adapter_platform_checks(&snapshot);
                observations.apply_adapter_checks(adapter_checks);
                observations.apply_platform_snapshot(build_platform_smoke_snapshot(
                    &snapshot,
                    adapter_checks,
                    NativePlatformObservation::default(),
                ));
            }
            Ok(Box::new(RendererSmokeApp::new(
                runtime,
                duration,
                recorder_for_app,
                observations_for_app,
            )))
        }),
    )
    .map_err(|error| anyhow!(error.to_string()))?;

    let timing = recorder
        .lock()
        .map_err(|_| anyhow!("smoke timing recorder lock was poisoned"))?
        .summary();
    let observations = observations
        .lock()
        .map_err(|_| anyhow!("smoke observations lock was poisoned"))?
        .clone();
    let platform_snapshot = observations.platform_snapshot.clone().unwrap_or_default();

    let mut errors = Vec::new();
    if observations.frame_count == 0 {
        errors.push("no frames were observed during smoke run".to_string());
    }

    // Gate required adapter paths: a missing or failed adapter path is a smoke
    // failure, not a silent pass.
    let adapter_checks = observations.adapter_checks();
    push_adapter_gate(
        &mut errors,
        "clipboard",
        adapter_checks.clipboard_adapter_path,
    );
    push_adapter_gate(&mut errors, "ime", adapter_checks.ime_adapter_path);
    push_adapter_gate(
        &mut errors,
        "file_dialog",
        adapter_checks.file_dialog_adapter_path,
    );

    // Gate required native observations (focus and high-DPI scale).
    if observations.focused.is_none() {
        errors.push("viewport focus was not observed during smoke run".to_string());
    }
    match observations.pixels_per_point {
        Some(scale) if scale.is_finite() && scale > 0.0 => {}
        _ => errors.push("high-DPI scale was not observed during smoke run".to_string()),
    }

    let status = if errors.is_empty() {
        RendererSmokeStatus::Passed
    } else {
        RendererSmokeStatus::Failed
    };

    Ok(RendererSmokeReport {
        command,
        status,
        workspace: report_config.workspace_root,
        file: report_config.initial_file,
        duration_ms: smoke.duration_ms,
        timing,
        focus_smoke: observations.focus_status(),
        menu_smoke: platform_snapshot.menu_smoke,
        shortcut_smoke: platform_snapshot.shortcut_smoke,
        clipboard_smoke: platform_snapshot.clipboard_smoke,
        ime_smoke: platform_snapshot.ime_smoke,
        theme_smoke: platform_snapshot.theme_smoke,
        high_dpi_smoke: observations.high_dpi_status(),
        focus_traversal_smoke: platform_snapshot.focus_traversal_smoke,
        file_dialog_smoke: platform_snapshot.file_dialog_smoke,
        accessibility_smoke: NOT_OBSERVED.to_string(),
        accessibility_tree_smoke: platform_snapshot.accessibility_tree_smoke,
        accessibility_projection_node_count: platform_snapshot.accessibility_projection_node_count,
        large_file_degraded_status: observations.large_file_degraded_status(),
        bounded_search_status: observations.bounded_search_status(),
        full_text_projection_status: observations.full_text_projection_status(),
        errors,
    })
}

/// Push a smoke-gate error when a required adapter path is missing or failed.
fn push_adapter_gate(errors: &mut Vec<String>, name: &str, path: Option<bool>) {
    match path {
        Some(true) => {}
        Some(false) => errors.push(format!("{name} adapter path failed during smoke run")),
        None => errors.push(format!(
            "{name} adapter path was not observed during smoke run"
        )),
    }
}

fn smoke_command(config: &DesktopLaunchConfig, smoke: &RendererSmokeConfig) -> String {
    let mut parts = vec![
        "cargo run -p legion-desktop -- --smoke".to_string(),
        format!("--workspace {}", shell_quote_path(&config.workspace_root)),
    ];
    if let Some(file) = &config.initial_file {
        parts.push(format!("--file {}", shell_quote(file)));
    }
    parts.push(format!("--duration-ms {}", smoke.duration_ms));
    parts.push(format!(
        "--evidence {}",
        shell_quote_path(&smoke.evidence_path)
    ));
    if let Some(path) = &config.session_state {
        parts.push(format!("--session-state {}", shell_quote_path(path)));
    }
    if let Some(path) = &config.diagnostics_export {
        parts.push(format!("--diagnostics-export {}", shell_quote_path(path)));
    }
    parts.join(" ")
}

/// Render a path as a single, shell-safe argument so paths with spaces or shell
/// metacharacters produce a re-runnable command line.
fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.display().to_string())
}

/// POSIX shell single-quote a value, leaving simple unambiguous arguments bare.
fn shell_quote(value: &str) -> String {
    let is_simple = !value.is_empty()
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '=' | ',')
        });
    if is_simple {
        return value.to_string();
    }
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[derive(Debug, Clone, Default)]
struct SmokeObservations {
    frame_count: u64,
    focused: Option<bool>,
    pixels_per_point: Option<f32>,
    clipboard_adapter_path: Option<bool>,
    ime_adapter_path: Option<bool>,
    file_dialog_adapter_path: Option<bool>,
    platform_snapshot: Option<DesktopPlatformSmokeSnapshot>,
    large_file_degraded_observed: bool,
    degraded_small_preview_absent: bool,
    bounded_search_observed: bool,
}

impl SmokeObservations {
    fn apply_adapter_checks(&mut self, checks: DesktopPlatformAdapterChecks) {
        self.clipboard_adapter_path = checks.clipboard_adapter_path;
        self.ime_adapter_path = checks.ime_adapter_path;
        self.file_dialog_adapter_path = checks.file_dialog_adapter_path;
    }

    fn apply_platform_snapshot(&mut self, snapshot: DesktopPlatformSmokeSnapshot) {
        self.platform_snapshot = Some(snapshot);
    }

    fn adapter_checks(&self) -> DesktopPlatformAdapterChecks {
        DesktopPlatformAdapterChecks {
            clipboard_adapter_path: self.clipboard_adapter_path,
            ime_adapter_path: self.ime_adapter_path,
            file_dialog_adapter_path: self.file_dialog_adapter_path,
        }
    }

    fn native_observation(&self) -> NativePlatformObservation {
        NativePlatformObservation {
            focused: self.focused,
            pixels_per_point: self.pixels_per_point,
        }
    }

    fn apply_projection_checks(&mut self, snapshot: &ShellProjectionSnapshot) {
        if snapshot.active_buffer_projection.degraded {
            self.large_file_degraded_observed = true;
            self.degraded_small_preview_absent = snapshot
                .active_buffer_projection
                .small_buffer_preview
                .is_none();
        }
        if snapshot
            .search_projection
            .status
            .message
            .to_ascii_lowercase()
            .contains("degraded")
        {
            self.bounded_search_observed = true;
        }
    }

    fn focus_status(&self) -> String {
        match self.focused {
            Some(true) => "os-observed focused".to_string(),
            Some(false) => "os-observed not focused".to_string(),
            None => NOT_OBSERVED.to_string(),
        }
    }

    fn high_dpi_status(&self) -> String {
        match self.pixels_per_point {
            Some(scale) if scale.is_finite() && scale > 0.0 => {
                format!("os-observed scale {scale:.3}")
            }
            Some(_) | None => NOT_OBSERVED.to_string(),
        }
    }

    fn large_file_degraded_status(&self) -> String {
        if self.large_file_degraded_observed {
            "projection observed degraded large-file mode".to_string()
        } else {
            NOT_OBSERVED.to_string()
        }
    }

    fn bounded_search_status(&self) -> String {
        if self.bounded_search_observed {
            "search observed degraded bounded viewport mode".to_string()
        } else {
            NOT_OBSERVED.to_string()
        }
    }

    fn full_text_projection_status(&self) -> String {
        if self.degraded_small_preview_absent {
            "degraded projection omitted full small-buffer preview".to_string()
        } else {
            NOT_OBSERVED.to_string()
        }
    }
}

struct RendererSmokeApp {
    runtime: DesktopRuntime,
    view: ProjectionView,
    duration: Duration,
    started_at: Instant,
    last_frame_at: Option<Instant>,
    first_paint_recorded: bool,
    recorder: Arc<Mutex<FrameTimingRecorder>>,
    observations: Arc<Mutex<SmokeObservations>>,
}

impl RendererSmokeApp {
    fn new(
        runtime: DesktopRuntime,
        duration: Duration,
        recorder: Arc<Mutex<FrameTimingRecorder>>,
        observations: Arc<Mutex<SmokeObservations>>,
    ) -> Self {
        let started_at = Instant::now();
        if let Ok(mut recorder) = recorder.lock() {
            recorder.record_input(started_at);
        }
        Self {
            runtime,
            view: ProjectionView::new(),
            duration,
            started_at,
            last_frame_at: None,
            first_paint_recorded: false,
            recorder,
            observations,
        }
    }
}

impl eframe::App for RendererSmokeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        if let Ok(mut recorder) = self.recorder.lock() {
            if !self.first_paint_recorded {
                recorder.record_paint(now);
                self.first_paint_recorded = true;
            }
            if let Some(last_frame_at) = self.last_frame_at {
                recorder.record_frame_duration(now.saturating_duration_since(last_frame_at));
            }
        }
        self.last_frame_at = Some(now);

        if let Ok(mut observations) = self.observations.lock() {
            observations.frame_count += 1;
            ui.ctx().input(|input| {
                observations.focused = input.viewport().focused;
            });
            observations.pixels_per_point = Some(ui.ctx().pixels_per_point());
        }

        let snapshot = self.runtime.projection_snapshot();
        if let Ok(mut observations) = self.observations.lock() {
            observations.apply_projection_checks(&snapshot);
            let adapter_checks = observations.adapter_checks();
            let native = observations.native_observation();
            observations.apply_platform_snapshot(build_platform_smoke_snapshot(
                &snapshot,
                adapter_checks,
                native,
            ));
        }
        let _ = self.view.render(ui, &snapshot);

        if now.saturating_duration_since(self.started_at) >= self.duration {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
    }
}

fn adapter_platform_checks(snapshot: &ShellProjectionSnapshot) -> DesktopPlatformAdapterChecks {
    build_platform_adapter_checks(snapshot)
}
