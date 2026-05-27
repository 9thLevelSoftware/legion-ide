//! Renderer smoke harness boundary.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use devil_protocol::TextCoordinate;
use devil_ui::ShellProjectionSnapshot;

use crate::{
    bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge},
    metrics::{FrameTimingRecorder, FrameTimingSummary},
    view::ProjectionView,
    workflow::{DesktopLaunchConfig, DesktopRuntime},
};

const ADAPTER_PATH_PASSED: &str = "adapter-path passed";
const NOT_OBSERVED: &str = "not observed";

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
    /// Clipboard smoke status.
    pub clipboard_smoke: String,
    /// IME smoke status.
    pub ime_smoke: String,
    /// High-DPI smoke status.
    pub high_dpi_smoke: String,
    /// File-dialog smoke status.
    pub file_dialog_smoke: String,
    /// Accessibility smoke status.
    pub accessibility_smoke: String,
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
            clipboard_smoke: NOT_OBSERVED.to_string(),
            ime_smoke: NOT_OBSERVED.to_string(),
            high_dpi_smoke: NOT_OBSERVED.to_string(),
            file_dialog_smoke: NOT_OBSERVED.to_string(),
            accessibility_smoke: NOT_OBSERVED.to_string(),
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
                "# Phase 2 Renderer Smoke Evidence\n\n",
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
                "clipboard_smoke: {clipboard}\n",
                "ime_smoke: {ime}\n",
                "high_dpi_smoke: {high_dpi}\n",
                "file_dialog_smoke: {file_dialog}\n",
                "accessibility_smoke: {accessibility}\n\n",
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
            clipboard = self.clipboard_smoke,
            ime = self.ime_smoke,
            high_dpi = self.high_dpi_smoke,
            file_dialog = self.file_dialog_smoke,
            accessibility = self.accessibility_smoke,
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

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Devil IDE Smoke")
            .with_inner_size([960.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Devil IDE Smoke",
        native_options,
        Box::new(move |_cc| {
            let runtime = DesktopRuntime::open(config)
                .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { error.into() })?;
            if let Ok(mut observations) = observations_for_app.lock() {
                observations
                    .apply_adapter_checks(adapter_platform_checks(&runtime.projection_snapshot()));
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

    let mut errors = Vec::new();
    if observations.frame_count == 0 {
        errors.push("no frames were observed during smoke run".to_string());
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
        clipboard_smoke: adapter_status(observations.clipboard_adapter_path),
        ime_smoke: adapter_status(observations.ime_adapter_path),
        high_dpi_smoke: observations.high_dpi_status(),
        file_dialog_smoke: adapter_status(observations.file_dialog_adapter_path),
        accessibility_smoke: NOT_OBSERVED.to_string(),
        errors,
    })
}

fn smoke_command(config: &DesktopLaunchConfig, smoke: &RendererSmokeConfig) -> String {
    let mut parts = vec![
        "cargo run -p devil-desktop -- --smoke".to_string(),
        format!("--workspace {}", config.workspace_root.display()),
    ];
    if let Some(file) = &config.initial_file {
        parts.push(format!("--file {file}"));
    }
    parts.push(format!("--duration-ms {}", smoke.duration_ms));
    parts.push(format!("--evidence {}", smoke.evidence_path.display()));
    parts.join(" ")
}

fn adapter_status(passed: bool) -> String {
    if passed {
        ADAPTER_PATH_PASSED.to_string()
    } else {
        "failed".to_string()
    }
}

#[derive(Debug, Clone, Default)]
struct SmokeObservations {
    frame_count: u64,
    focused: Option<bool>,
    pixels_per_point: Option<f32>,
    clipboard_adapter_path: bool,
    ime_adapter_path: bool,
    file_dialog_adapter_path: bool,
}

impl SmokeObservations {
    fn apply_adapter_checks(&mut self, checks: AdapterPlatformChecks) {
        self.clipboard_adapter_path = checks.clipboard_adapter_path;
        self.ime_adapter_path = checks.ime_adapter_path;
        self.file_dialog_adapter_path = checks.file_dialog_adapter_path;
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
            Some(scale) if scale > 1.0 => format!("os-observed scale {scale:.3}"),
            Some(_) | None => NOT_OBSERVED.to_string(),
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
        let _ = self.view.render(ui, &snapshot);

        if now.saturating_duration_since(self.started_at) >= self.duration {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AdapterPlatformChecks {
    clipboard_adapter_path: bool,
    ime_adapter_path: bool,
    file_dialog_adapter_path: bool,
}

fn adapter_platform_checks(snapshot: &ShellProjectionSnapshot) -> AdapterPlatformChecks {
    let bridge = DesktopCommandBridge::new();
    let at = projected_cursor(snapshot);
    AdapterPlatformChecks {
        clipboard_adapter_path: matches!(
            bridge.translate(
                DesktopAction::ClipboardPaste {
                    text: "clipboard-smoke".to_string(),
                    at,
                },
                snapshot,
            ),
            DesktopBridgeOutput::Intent(_)
        ),
        ime_adapter_path: matches!(
            bridge.translate(
                DesktopAction::ImeCommit {
                    text: "ime-smoke".to_string(),
                    at,
                },
                snapshot,
            ),
            DesktopBridgeOutput::Intent(_)
        ),
        file_dialog_adapter_path: matches!(
            bridge.translate(
                DesktopAction::OpenPathDialogSelected("Cargo.toml".to_string()),
                snapshot,
            ),
            DesktopBridgeOutput::Intent(_)
        ),
    }
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
