//! Desktop runtime workflow boundary.

use std::{
    ffi::OsString,
    path::PathBuf,
};

use anyhow::{Result, anyhow};
use devil_app::{AppCommandOutcome, AppComposition, AppSaveOutcome};
use devil_protocol::{PrincipalId, TextCoordinate, WorkspaceTrustState};
use devil_ui::{
    CommandDispatchIntent, Shell, ShellProjectionSnapshot, StatusMessageProjection, StatusSeverity,
};

use crate::{
    bridge::{
        DesktopAction, DesktopAppRequest, DesktopBridgeError, DesktopBridgeOutput,
        DesktopCommandBridge,
    },
    view::ProjectionView,
};

const WINDOW_TITLE: &str = "Devil IDE";

/// Process launch configuration for the desktop adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopLaunchConfig {
    /// Workspace root to open through app authority.
    pub workspace_root: PathBuf,
    /// Optional file path to open after the workspace is bound.
    pub initial_file: Option<String>,
    /// Principal used for app-owned workspace trust/open requests.
    pub principal: PrincipalId,
    /// Reserved Plan 02-05 smoke-mode switch.
    pub smoke: bool,
}

impl DesktopLaunchConfig {
    /// Build a launch config with the default desktop principal.
    pub fn new(workspace_root: PathBuf, initial_file: Option<String>) -> Self {
        Self {
            workspace_root,
            initial_file,
            principal: PrincipalId("desktop".to_string()),
            smoke: false,
        }
    }

    /// Parse launch config from process arguments.
    pub fn from_env_args() -> Result<Self> {
        Self::from_args(std::env::args_os().skip(1))
    }

    /// Parse launch config from an argument iterator.
    pub fn from_args(args: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut smoke = false;
        let mut positionals = Vec::new();
        for arg in args {
            if arg == "--smoke" {
                smoke = true;
            } else {
                positionals.push(arg);
            }
        }

        let workspace_root = match positionals.first() {
            Some(path) => PathBuf::from(path),
            None => std::env::current_dir()?,
        };
        if workspace_root.as_os_str().is_empty() {
            return Err(anyhow!("workspace root cannot be empty"));
        }

        let initial_file = positionals
            .get(1)
            .map(|path| path.to_string_lossy().into_owned())
            .filter(|path| !path.trim().is_empty());

        Ok(Self {
            workspace_root,
            initial_file,
            principal: PrincipalId("desktop".to_string()),
            smoke,
        })
    }
}

/// User-visible outcome from the desktop workflow harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopWorkflowOutcome {
    /// Command had no effect.
    Noop,
    /// App authority opened a file.
    Opened,
    /// App authority applied an editor transaction.
    Edited,
    /// Save completed through app/workspace authority.
    Saved,
    /// Save was rejected without marking editor text clean.
    SaveRejected(String),
    /// Explorer projection was refreshed.
    ExplorerRefreshed,
    /// Open-path prompt should be shown by the adapter.
    OpenPathPromptRequested,
    /// Workspace root was opened through app authority.
    WorkspaceOpened,
    /// Adapter-local quit was requested.
    QuitRequested,
    /// Bridge or app command failed without implying success.
    Error(String),
}

/// Renderer-backed desktop runtime.
pub struct DesktopRuntime {
    app: AppComposition,
    shell: Shell,
    bridge: DesktopCommandBridge,
    view: ProjectionView,
    principal: PrincipalId,
    open_path_prompt: bool,
    open_path_text: String,
    quit_requested: bool,
    last_status: Option<StatusMessageProjection>,
    last_outcome: DesktopWorkflowOutcome,
}

impl DesktopRuntime {
    /// Open the configured workspace and optional initial file.
    pub fn open(config: DesktopLaunchConfig) -> Result<Self> {
        let mut app = AppComposition::new();
        app.open_workspace(
            &config.workspace_root,
            WorkspaceTrustState::Trusted,
            config.principal.clone(),
        )?;

        if let Some(initial_file) = &config.initial_file {
            app.open_file(initial_file)?;
        }

        let mut snapshot = app.shell_projection_snapshot(WINDOW_TITLE)?;
        snapshot.status_messages.push(status_message(
            StatusSeverity::Info,
            "Desktop adapter ready",
        ));

        Ok(Self {
            app,
            shell: Shell::new(snapshot),
            bridge: DesktopCommandBridge::new(),
            view: ProjectionView::new(),
            principal: config.principal,
            open_path_prompt: false,
            open_path_text: String::new(),
            quit_requested: false,
            last_status: Some(status_message(
                StatusSeverity::Info,
                "Desktop adapter ready",
            )),
            last_outcome: DesktopWorkflowOutcome::Noop,
        })
    }

    /// Handle a desktop action through bridge and app-owned authority.
    pub fn handle_action(&mut self, action: DesktopAction) -> Result<DesktopWorkflowOutcome> {
        let snapshot = self.shell.projection_snapshot();
        let bridge_output = self.bridge.translate(action, &snapshot);
        let outcome = match bridge_output {
            DesktopBridgeOutput::Intent(CommandDispatchIntent::Quit) => {
                self.quit_requested = true;
                self.set_status(StatusSeverity::Info, "Quit requested");
                DesktopWorkflowOutcome::QuitRequested
            }
            DesktopBridgeOutput::Intent(intent) => self.dispatch_intent(intent)?,
            DesktopBridgeOutput::AppRequest(request) => self.handle_app_request(request)?,
            DesktopBridgeOutput::Noop => {
                self.set_status(StatusSeverity::Info, "No action");
                DesktopWorkflowOutcome::Noop
            }
            DesktopBridgeOutput::Error(error) => self.handle_bridge_error(error),
        };

        self.refresh_projection()?;
        self.last_outcome = outcome.clone();
        Ok(outcome)
    }

    /// Returns whether the adapter has requested shutdown.
    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    /// Return the latest shell projection snapshot.
    pub fn projection_snapshot(&self) -> ShellProjectionSnapshot {
        self.shell.projection_snapshot()
    }

    /// Return the last workflow outcome.
    pub fn last_outcome(&self) -> &DesktopWorkflowOutcome {
        &self.last_outcome
    }

    fn dispatch_intent(&mut self, intent: CommandDispatchIntent) -> Result<DesktopWorkflowOutcome> {
        match self.app.dispatch_ui_intent(intent) {
            Ok(outcome) => Ok(self.map_app_outcome(outcome)),
            Err(error) => {
                let message = error.to_string();
                self.set_status(StatusSeverity::Error, message.clone());
                Ok(DesktopWorkflowOutcome::Error(message))
            }
        }
    }

    fn handle_app_request(&mut self, request: DesktopAppRequest) -> Result<DesktopWorkflowOutcome> {
        match request {
            DesktopAppRequest::ShowOpenPathPrompt => {
                self.open_path_prompt = true;
                self.set_status(StatusSeverity::Info, "Open path requested");
                Ok(DesktopWorkflowOutcome::OpenPathPromptRequested)
            }
            DesktopAppRequest::OpenWorkspace { root } => match self.app.open_workspace(
                &root,
                WorkspaceTrustState::Trusted,
                self.principal.clone(),
            ) {
                Ok(_) => {
                    self.set_status(StatusSeverity::Info, format!("Opened {}", root.display()));
                    Ok(DesktopWorkflowOutcome::WorkspaceOpened)
                }
                Err(error) => {
                    let message = error.to_string();
                    self.set_status(StatusSeverity::Error, message.clone());
                    Ok(DesktopWorkflowOutcome::Error(message))
                }
            },
        }
    }

    fn handle_bridge_error(&mut self, error: DesktopBridgeError) -> DesktopWorkflowOutcome {
        let message = error.to_string();
        self.set_status(StatusSeverity::Error, message.clone());
        DesktopWorkflowOutcome::Error(message)
    }

    fn map_app_outcome(&mut self, outcome: AppCommandOutcome) -> DesktopWorkflowOutcome {
        match outcome {
            AppCommandOutcome::Noop => {
                self.set_status(StatusSeverity::Info, "No action");
                DesktopWorkflowOutcome::Noop
            }
            AppCommandOutcome::Quit => {
                self.quit_requested = true;
                self.set_status(StatusSeverity::Info, "Quit requested");
                DesktopWorkflowOutcome::QuitRequested
            }
            AppCommandOutcome::Edited(_) => {
                self.set_status(StatusSeverity::Info, "Edited");
                DesktopWorkflowOutcome::Edited
            }
            AppCommandOutcome::Save(AppSaveOutcome::Saved(_)) => {
                self.set_status(StatusSeverity::Info, "Saved");
                DesktopWorkflowOutcome::Saved
            }
            AppCommandOutcome::Save(AppSaveOutcome::Rejected(response)) => {
                let message = format!("Save rejected: {response:?}");
                self.set_status(StatusSeverity::Warning, message.clone());
                DesktopWorkflowOutcome::SaveRejected(message)
            }
            AppCommandOutcome::ExplorerRefreshed(_) => {
                self.set_status(StatusSeverity::Info, "Explorer refreshed");
                DesktopWorkflowOutcome::ExplorerRefreshed
            }
            AppCommandOutcome::Opened(_) => {
                self.set_status(StatusSeverity::Info, "Opened");
                DesktopWorkflowOutcome::Opened
            }
            AppCommandOutcome::AiRunStarted(_)
            | AppCommandOutcome::AiRunCancelled(_)
            | AppCommandOutcome::AiRunReplayed(_)
            | AppCommandOutcome::AiRunInspected(_)
            | AppCommandOutcome::PluginCommandInvoked(_)
            | AppCommandOutcome::CollaborationSessionJoined(_)
            | AppCommandOutcome::CollaborationSessionLeft(_)
            | AppCommandOutcome::CollaborationPresencePublished(_)
            | AppCommandOutcome::CollaborationOperationApplied(_) => {
                self.set_status(StatusSeverity::Info, "Command handled");
                DesktopWorkflowOutcome::Noop
            }
        }
    }

    fn refresh_projection(&mut self) -> Result<()> {
        let mut snapshot = self.app.shell_projection_snapshot(WINDOW_TITLE)?;
        if let Some(status) = &self.last_status {
            snapshot.status_messages.push(status.clone());
        }
        self.shell.replace_projection_snapshot(snapshot);
        Ok(())
    }

    fn set_status(&mut self, severity: StatusSeverity, message: impl Into<String>) {
        self.last_status = Some(status_message(severity, message));
    }
}

/// Run the desktop adapter from process arguments.
pub fn run_from_env() -> Result<()> {
    let config = DesktopLaunchConfig::from_env_args()?;
    run_native(config)
}

fn run_native(config: DesktopLaunchConfig) -> Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        WINDOW_TITLE,
        native_options,
        Box::new(move |_cc| {
            let runtime = DesktopRuntime::open(config)
                .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { error.into() })?;
            Ok(Box::new(DesktopEframeApp::new(runtime)))
        }),
    )
    .map_err(|error| anyhow!(error.to_string()))
}

struct DesktopEframeApp {
    runtime: DesktopRuntime,
}

impl DesktopEframeApp {
    fn new(runtime: DesktopRuntime) -> Self {
        Self { runtime }
    }

    fn handle_keyboard(&mut self, ui: &egui::Ui) {
        let mut actions = Vec::new();
        ui.input(|input| {
            let command = input.modifiers.command;
            if command && input.key_pressed(egui::Key::S) {
                actions.push(DesktopAction::SaveActive);
            }
            if command && input.key_pressed(egui::Key::Q) {
                actions.push(DesktopAction::Quit);
            }
            if command && input.key_pressed(egui::Key::O) {
                actions.push(DesktopAction::ShowOpenPathPrompt);
            }
            if command && input.key_pressed(egui::Key::Z) {
                if input.modifiers.shift {
                    actions.push(DesktopAction::Redo);
                } else {
                    actions.push(DesktopAction::Undo);
                }
            }

            let at = projected_cursor(&self.runtime.projection_snapshot());
            for event in &input.events {
                if let egui::Event::Text(text) = event
                    && !text.is_empty()
                {
                    actions.push(DesktopAction::InsertText {
                        text: text.clone(),
                        at,
                    });
                }
            }
        });

        for action in actions {
            let _ = self.runtime.handle_action(action);
        }
    }

    fn show_open_path_prompt(&mut self, ctx: &egui::Context) {
        if !self.runtime.open_path_prompt {
            return;
        }

        let mut open = true;
        egui::Window::new("Open path")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.text_edit_singleline(&mut self.runtime.open_path_text);
                ui.horizontal(|ui| {
                    if ui.button("Open").clicked() {
                        let path = std::mem::take(&mut self.runtime.open_path_text);
                        self.runtime.open_path_prompt = false;
                        let _ = self
                            .runtime
                            .handle_action(DesktopAction::OpenPathText(path));
                    }
                    if ui.button("Cancel").clicked() {
                        self.runtime.open_path_prompt = false;
                        self.runtime.open_path_text.clear();
                        let _ = self
                            .runtime
                            .handle_action(DesktopAction::OpenPathDialogCancelled);
                    }
                });
            });

        if !open {
            self.runtime.open_path_prompt = false;
        }
    }
}

impl eframe::App for DesktopEframeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_keyboard(ui);
        let snapshot = self.runtime.projection_snapshot();
        let output = self.runtime.view.render(ui, &snapshot);
        if output.needs_repaint {
            ui.ctx().request_repaint();
        }
        self.show_open_path_prompt(ui.ctx());
        if self.runtime.quit_requested() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn status_message(
    severity: StatusSeverity,
    message: impl Into<String>,
) -> StatusMessageProjection {
    StatusMessageProjection {
        severity,
        message: message.into(),
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
