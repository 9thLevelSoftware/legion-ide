//! GAP-01.1 windowed GUI E2E: `eframe::run_native`, then open/edit/save.
//!
//! This is not `--beta-smoke` (headless DesktopRuntime) and not AppComposition
//! `golden-path-5`. A run that cannot create a window is blocked, not passed.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use legion_protocol::{PRODUCT_NAME, TextCoordinate};

use crate::{
    bridge::DesktopAction,
    view::ProjectionView,
    workflow::{
        DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome, desktop_native_options,
    },
};

/// Marker inserted at the start of the fixture file during the windowed edit.
pub const WINDOWED_E2E_EDIT: &str = "WINDOWED_E2E_EDIT\n";

const DEFAULT_REPORT_PATH: &str = "target/windowed-gui/report.toml";
const WINDOW_WAIT: Duration = Duration::from_secs(10);

/// Launch-time configuration for the windowed GUI E2E.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowedGuiE2eConfig {
    /// TOML report path written even when the run is blocked or failed.
    pub report_path: PathBuf,
}

impl WindowedGuiE2eConfig {
    /// Create a windowed GUI E2E config.
    pub fn new(report_path: PathBuf) -> Result<Self> {
        if report_path.as_os_str().is_empty() {
            return Err(anyhow!("windowed E2E report path cannot be empty"));
        }
        Ok(Self { report_path })
    }

    /// Default report path used when `--report` is omitted.
    #[must_use]
    pub fn default_report_path() -> PathBuf {
        PathBuf::from(DEFAULT_REPORT_PATH)
    }
}

/// Overall windowed E2E status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowedGuiE2eStatus {
    /// A native window opened and open/edit/save completed.
    Passed,
    /// No native window could be created in this environment.
    Blocked,
    /// A window opened but open, edit, or save failed.
    Failed,
}

impl WindowedGuiE2eStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
        }
    }
}

/// Metadata-only report for GAP-01.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowedGuiE2eReport {
    /// Packaged binary that launched `eframe::run_native`.
    pub binary_path: String,
    /// Unsigned package directory the binary was copied into, when known.
    pub package_dir: String,
    /// Operating system (`std::env::consts::OS`).
    pub os: String,
    /// CPU architecture.
    pub arch: String,
    /// Git SHA injected by xtask, or `unknown`.
    pub git_sha: String,
    /// Whether a native window was observed (pixels-per-point / focus).
    pub window_created: bool,
    /// Backend used to create the window.
    pub window_backend: String,
    /// Open step status.
    pub open: String,
    /// Edit step status.
    pub edit: String,
    /// Save step status.
    pub save: String,
    /// Overall status.
    pub status: WindowedGuiE2eStatus,
    /// Errors or blockers.
    pub errors: Vec<String>,
}

impl WindowedGuiE2eReport {
    fn blocked(detail: String) -> Self {
        let mut report = Self::blank();
        report.status = WindowedGuiE2eStatus::Blocked;
        report.errors.push(detail);
        report
    }

    fn blank() -> Self {
        Self {
            binary_path: current_exe_display(),
            package_dir: env_or_unknown("LEGION_WINDOWED_E2E_PACKAGE_DIR"),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            git_sha: env_or_unknown("LEGION_WINDOWED_E2E_GIT_SHA"),
            window_created: false,
            window_backend: "eframe::run_native".to_string(),
            open: "not-run".to_string(),
            edit: "not-run".to_string(),
            save: "not-run".to_string(),
            status: WindowedGuiE2eStatus::Blocked,
            errors: Vec::new(),
        }
    }

    /// Render a stable TOML report. Metadata only: no buffer bodies.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str("schema_version = 1\n");
        out.push_str("task = \"GAP-01.1\"\n");
        out.push_str("harness = \"windowed-gui-e2e\"\n");
        out.push_str("not_golden_path_5 = true\n");
        out.push_str("not_beta_smoke = true\n");
        out.push_str(&format!(
            "binary_path = {}\n",
            toml_string(&self.binary_path)
        ));
        out.push_str(&format!(
            "package_dir = {}\n",
            toml_string(&self.package_dir)
        ));
        out.push_str(&format!("os = {}\n", toml_string(&self.os)));
        out.push_str(&format!("arch = {}\n", toml_string(&self.arch)));
        out.push_str(&format!("git_sha = {}\n", toml_string(&self.git_sha)));
        out.push_str(&format!("window_created = {}\n", self.window_created));
        out.push_str(&format!(
            "window_backend = {}\n",
            toml_string(&self.window_backend)
        ));
        out.push_str(&format!("open = {}\n", toml_string(&self.open)));
        out.push_str(&format!("edit = {}\n", toml_string(&self.edit)));
        out.push_str(&format!("save = {}\n", toml_string(&self.save)));
        out.push_str(&format!("status = {}\n", toml_string(self.status.as_str())));
        out.push_str("errors = [");
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(&toml_string(error));
        }
        out.push_str("]\n");
        out
    }

    fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_toml())?;
        Ok(())
    }
}

/// Run GAP-01.1: create a native window, then open/edit/save through app authority.
pub fn run_windowed_gui_e2e(config: DesktopLaunchConfig, e2e: WindowedGuiE2eConfig) -> Result<()> {
    let report_path = e2e.report_path.clone();
    match run_windowed_gui_e2e_window(config) {
        Ok(report) => finish_windowed_gui_e2e(&report, &report_path),
        Err(error) => {
            let report = WindowedGuiE2eReport::blocked(error.to_string());
            report.write(&report_path)?;
            Err(error)
        }
    }
}

fn finish_windowed_gui_e2e(report: &WindowedGuiE2eReport, report_path: &Path) -> Result<()> {
    report.write(report_path)?;
    match report.status {
        WindowedGuiE2eStatus::Passed => Ok(()),
        WindowedGuiE2eStatus::Failed => Err(anyhow!(
            "windowed GUI E2E failed ({} error(s)); see {}",
            report.errors.len(),
            report_path.display()
        )),
        WindowedGuiE2eStatus::Blocked => Err(anyhow!(
            "windowed GUI E2E blocked ({} error(s)); see {}",
            report.errors.len(),
            report_path.display()
        )),
    }
}

fn run_windowed_gui_e2e_window(config: DesktopLaunchConfig) -> Result<WindowedGuiE2eReport> {
    let observations = std::sync::Arc::new(std::sync::Mutex::new(WindowedGuiE2eReport::blank()));
    let observations_for_app = std::sync::Arc::clone(&observations);
    let title = format!("{PRODUCT_NAME} Windowed E2E");
    let native_options = desktop_native_options(&title);
    let save_path = config.initial_file.clone().map(|file| {
        let path = PathBuf::from(&file);
        if path.is_absolute() {
            path
        } else {
            config.workspace_root.join(path)
        }
    });

    eframe::run_native(
        &title,
        native_options,
        Box::new(move |_cc| {
            let runtime = DesktopRuntime::open(config)
                .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { error.into() })?;
            Ok(Box::new(WindowedGuiE2eApp {
                runtime,
                view: ProjectionView::new(),
                save_path,
                observations: observations_for_app,
                started_at: Instant::now(),
                phase: E2ePhase::WaitPaint,
            }))
        }),
    )
    .map_err(|error| anyhow!(error.to_string()))?;

    let report = observations
        .lock()
        .map_err(|_| anyhow!("windowed E2E report lock was poisoned"))?
        .clone();
    Ok(report)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum E2ePhase {
    WaitPaint,
    Edit,
    Save,
    Close,
}

struct WindowedGuiE2eApp {
    runtime: DesktopRuntime,
    view: ProjectionView,
    save_path: Option<PathBuf>,
    observations: std::sync::Arc<std::sync::Mutex<WindowedGuiE2eReport>>,
    started_at: Instant,
    phase: E2ePhase,
}

impl eframe::App for WindowedGuiE2eApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let snapshot = self.runtime.projection_snapshot();
        let _ = self.view.render(ui, &snapshot);

        let focused = ui.ctx().input(|input| input.viewport().focused);
        let scale = ui.ctx().pixels_per_point();
        let window_created = focused.is_some() || (scale.is_finite() && scale > 0.0);

        if let Ok(mut report) = self.observations.lock() {
            if window_created {
                report.window_created = true;
            }
            if snapshot.active_buffer_projection.buffer_id.is_some() {
                report.open = "passed".to_string();
            }
        }

        match self.phase {
            E2ePhase::WaitPaint => {
                if window_created {
                    self.phase = E2ePhase::Edit;
                }
            }
            E2ePhase::Edit => {
                let edit = self.runtime.handle_action(DesktopAction::InsertText {
                    text: WINDOWED_E2E_EDIT.to_string(),
                    at: TextCoordinate {
                        line: 0,
                        character: 0,
                        byte_offset: Some(0),
                        utf16_offset: Some(0),
                    },
                });
                if let Ok(mut report) = self.observations.lock() {
                    report.edit = match edit {
                        Ok(DesktopWorkflowOutcome::Edited) => "passed".to_string(),
                        Ok(other) => format!("unexpected:{other:?}"),
                        Err(error) => format!("error:{error}"),
                    };
                }
                self.phase = E2ePhase::Save;
            }
            E2ePhase::Save => {
                let save = self.runtime.handle_action(DesktopAction::SaveActive);
                let disk_ok = self
                    .save_path
                    .as_ref()
                    .and_then(|path| fs::read_to_string(path).ok())
                    .is_some_and(|body| body.starts_with(WINDOWED_E2E_EDIT));
                if let Ok(mut report) = self.observations.lock() {
                    report.save = match (save, disk_ok) {
                        (Ok(DesktopWorkflowOutcome::Saved), true)
                        | (Ok(DesktopWorkflowOutcome::SavedWithAuditFailure(_)), true) => {
                            "passed".to_string()
                        }
                        (Ok(other), disk_ok) => {
                            format!("unexpected:{other:?} disk_ok={disk_ok}")
                        }
                        (Err(error), _) => format!("error:{error}"),
                    };
                    finalize_status(&mut report);
                }
                self.phase = E2ePhase::Close;
            }
            E2ePhase::Close => {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        if self.started_at.elapsed() >= WINDOW_WAIT {
            if let Ok(mut report) = self.observations.lock() {
                if !report.window_created {
                    report
                        .errors
                        .push("native window was not observed within the wait budget".to_string());
                    report.status = WindowedGuiE2eStatus::Blocked;
                } else {
                    finalize_status(&mut report);
                }
            }
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
    }
}

fn finalize_status(report: &mut WindowedGuiE2eReport) {
    let steps_ok = report.window_created
        && report.open == "passed"
        && report.edit == "passed"
        && report.save == "passed";
    if steps_ok {
        report.status = WindowedGuiE2eStatus::Passed;
        report.errors.clear();
    } else if report.window_created {
        report.status = WindowedGuiE2eStatus::Failed;
        if report.open != "passed" {
            report.errors.push(format!("open: {}", report.open));
        }
        if report.edit != "passed" {
            report.errors.push(format!("edit: {}", report.edit));
        }
        if report.save != "passed" {
            report.errors.push(format!("save: {}", report.save));
        }
    } else {
        report.status = WindowedGuiE2eStatus::Blocked;
        report
            .errors
            .push("native window was not created".to_string());
    }
}

fn current_exe_display() -> String {
    std::env::current_exe()
        .map(|path| path.display().to_string().replace('\\', "/"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn env_or_unknown(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| "unknown".to_string())
}

fn toml_string(value: &str) -> String {
    use std::fmt::Write;
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
    format!("\"{escaped}\"")
}
