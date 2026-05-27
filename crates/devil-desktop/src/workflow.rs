//! Desktop runtime workflow boundary.

use std::{collections::BTreeSet, ffi::OsString, path::PathBuf};

use anyhow::{Result, anyhow};
use devil_app::{AppCloseTabOutcome, AppCommandOutcome, AppComposition, AppSaveOutcome};
use devil_protocol::{
    BufferId, PrincipalId, ProtocolTextRange, TextCoordinate, ViewportScroll, WorkspaceTrustState,
};
use devil_ui::{
    CommandDispatchIntent, SearchScopeProjection, Shell, ShellProjectionSnapshot,
    StatusMessageProjection, StatusSeverity,
};

use crate::{
    bridge::{
        DesktopAction, DesktopAppRequest, DesktopBridgeError, DesktopBridgeOutput,
        DesktopCommandBridge,
    },
    smoke::{self, RendererSmokeConfig},
    view::{DesktopProjectionViewState, ProjectionView},
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
    /// Optional timed smoke-mode configuration.
    pub smoke: Option<RendererSmokeConfig>,
}

impl DesktopLaunchConfig {
    /// Build a launch config with the default desktop principal.
    pub fn new(workspace_root: PathBuf, initial_file: Option<String>) -> Self {
        Self {
            workspace_root,
            initial_file,
            principal: PrincipalId("desktop".to_string()),
            smoke: None,
        }
    }

    /// Parse launch config from process arguments.
    pub fn from_env_args() -> Result<Self> {
        Self::from_args(std::env::args_os().skip(1))
    }

    /// Parse launch config from an argument iterator.
    pub fn from_args(args: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut smoke_enabled = false;
        let mut workspace_root = None;
        let mut initial_file = None;
        let mut duration_ms = 1500;
        let mut evidence_path =
            PathBuf::from("plans/evidence/gui-productization/phase-2-renderer-smoke.md");
        let mut positionals = Vec::new();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            let arg_text = arg.to_string_lossy();
            match arg_text.as_ref() {
                "--smoke" => smoke_enabled = true,
                "--workspace" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--workspace requires a path"))?;
                    workspace_root = Some(PathBuf::from(value));
                }
                "--file" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--file requires a path"))?;
                    initial_file = Some(value.to_string_lossy().into_owned());
                }
                "--duration-ms" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--duration-ms requires a number"))?;
                    duration_ms = value.to_string_lossy().parse::<u64>()?;
                }
                "--evidence" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--evidence requires a path"))?;
                    evidence_path = PathBuf::from(value);
                }
                other if other.starts_with("--") => {
                    return Err(anyhow!("unsupported desktop argument: {other}"));
                }
                _ => positionals.push(arg),
            }
        }

        let workspace_root = match workspace_root.or_else(|| positionals.first().map(PathBuf::from))
        {
            Some(path) => path,
            None => std::env::current_dir()?,
        };
        if workspace_root.as_os_str().is_empty() {
            return Err(anyhow!("workspace root cannot be empty"));
        }

        let initial_file = initial_file
            .or_else(|| {
                positionals
                    .get(1)
                    .map(|path| path.to_string_lossy().into_owned())
            })
            .filter(|path| !path.trim().is_empty());

        let smoke = if smoke_enabled {
            Some(RendererSmokeConfig::new(duration_ms, evidence_path)?)
        } else {
            None
        };

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
    /// Save-all completed through app/workspace authority.
    SaveAll {
        /// Count of buffers saved successfully.
        saved_count: usize,
        /// Count of rejected saves that kept buffers dirty.
        rejected_count: usize,
    },
    /// Save was rejected without marking editor text clean.
    SaveRejected(String),
    /// Active tab changed through app authority.
    TabSwitched(BufferId),
    /// Clean tab closed through app authority.
    TabClosed(BufferId),
    /// Dirty tab close produced an app-owned prompt.
    CloseDirtyPrompt(BufferId),
    /// Cursor update completed through editor authority.
    CursorSet(BufferId),
    /// Selection update completed through editor authority.
    SelectionSet(BufferId),
    /// Viewport scroll update completed through app authority.
    ViewportScrollSet(BufferId),
    /// Search projection changed through app authority.
    SearchUpdated,
    /// Explorer projection was refreshed.
    ExplorerRefreshed,
    /// Adapter-local explorer expansion changed.
    ExplorerPathToggled(String),
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
    search_prompt: bool,
    search_query_text: String,
    search_scope: SearchScopeProjection,
    explorer_expansion: BTreeSet<String>,
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
            search_prompt: false,
            search_query_text: String::new(),
            search_scope: SearchScopeProjection::ActiveFile,
            explorer_expansion: BTreeSet::new(),
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

    /// Returns whether an explorer path is expanded by adapter-local state.
    pub fn explorer_path_expanded(&self, path: &str) -> bool {
        self.explorer_expansion.contains(path)
    }

    fn projection_view_state(&self) -> DesktopProjectionViewState {
        DesktopProjectionViewState {
            expanded_explorer_paths: self.explorer_expansion.clone(),
            selected_explorer_file: None,
        }
    }

    fn editor_input_enabled(&self, snapshot: &ShellProjectionSnapshot) -> bool {
        !self.open_path_prompt && !self.search_prompt && !close_dirty_prompt_active(snapshot)
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
            DesktopAppRequest::ShowSearchPrompt { scope } => {
                self.search_prompt = true;
                self.search_scope = scope;
                self.set_status(StatusSeverity::Info, "Search requested");
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAppRequest::ToggleExplorerPath { path } => {
                if !self.explorer_expansion.remove(&path) {
                    self.explorer_expansion.insert(path.clone());
                }
                self.set_status(StatusSeverity::Info, format!("Explorer toggled {path}"));
                Ok(DesktopWorkflowOutcome::ExplorerPathToggled(path))
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
            AppCommandOutcome::SaveAll(outcome) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!(
                        "Save all completed: {} saved, {} rejected",
                        outcome.saved_count, outcome.rejected_count
                    ),
                );
                DesktopWorkflowOutcome::SaveAll {
                    saved_count: outcome.saved_count,
                    rejected_count: outcome.rejected_count,
                }
            }
            AppCommandOutcome::TabSwitched(buffer_id) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!("Tab switched {}", buffer_id.0),
                );
                DesktopWorkflowOutcome::TabSwitched(buffer_id)
            }
            AppCommandOutcome::TabClose(AppCloseTabOutcome::Closed { buffer_id }) => {
                self.set_status(StatusSeverity::Info, format!("Tab closed {}", buffer_id.0));
                DesktopWorkflowOutcome::TabClosed(buffer_id)
            }
            AppCommandOutcome::TabClose(AppCloseTabOutcome::CloseDirtyPrompt {
                buffer_id, ..
            }) => {
                self.set_status(
                    StatusSeverity::Warning,
                    format!("Close dirty prompt {}", buffer_id.0),
                );
                DesktopWorkflowOutcome::CloseDirtyPrompt(buffer_id)
            }
            AppCommandOutcome::CursorSet(buffer_id) => {
                self.set_status(StatusSeverity::Info, format!("Cursor set {}", buffer_id.0));
                DesktopWorkflowOutcome::CursorSet(buffer_id)
            }
            AppCommandOutcome::SelectionSet(buffer_id) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!("Selection set {}", buffer_id.0),
                );
                DesktopWorkflowOutcome::SelectionSet(buffer_id)
            }
            AppCommandOutcome::ViewportScrollSet(buffer_id) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!("Viewport scroll set {}", buffer_id.0),
                );
                DesktopWorkflowOutcome::ViewportScrollSet(buffer_id)
            }
            AppCommandOutcome::SearchUpdated(projection) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!("Search: {}", projection.status.message),
                );
                DesktopWorkflowOutcome::SearchUpdated
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
    if let Some(smoke_config) = config.smoke.clone() {
        smoke::run_smoke(config, smoke_config)
    } else {
        run_native(config)
    }
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
        let snapshot = self.runtime.projection_snapshot();
        let editor_input_enabled = self.runtime.editor_input_enabled(&snapshot);
        ui.input(|input| {
            let command = input.modifiers.command;
            if command && input.key_pressed(egui::Key::S) {
                if input.modifiers.shift {
                    actions.push(DesktopAction::SaveAll);
                } else {
                    actions.push(DesktopAction::SaveActive);
                }
            }
            if command && input.key_pressed(egui::Key::Q) {
                actions.push(DesktopAction::Quit);
            }
            if command && input.key_pressed(egui::Key::W) {
                if let Some(buffer_id) = active_buffer_for_input(&snapshot) {
                    actions.push(DesktopAction::CloseTab { buffer_id });
                }
            }
            if command && input.key_pressed(egui::Key::Tab) {
                if let Some(buffer_id) =
                    adjacent_tab_id(&snapshot, if input.modifiers.shift { -1 } else { 1 })
                {
                    actions.push(DesktopAction::SwitchTab { buffer_id });
                }
            }
            if command && input.key_pressed(egui::Key::O) {
                actions.push(DesktopAction::ShowOpenPathPrompt);
            }
            if command && input.key_pressed(egui::Key::F) {
                actions.push(DesktopAction::ShowSearchPrompt {
                    scope: if input.modifiers.shift {
                        SearchScopeProjection::Workspace
                    } else {
                        SearchScopeProjection::ActiveFile
                    },
                });
            }
            if input.key_pressed(egui::Key::F5) {
                actions.push(DesktopAction::RefreshExplorer);
            }
            if command && input.key_pressed(egui::Key::Z) {
                if input.modifiers.shift {
                    actions.push(DesktopAction::Redo);
                } else {
                    actions.push(DesktopAction::Undo);
                }
            }

            actions.extend(editor_text_input_actions(
                &input.events,
                &snapshot,
                editor_input_enabled,
            ));
            actions.extend(editor_keyboard_control_actions(
                input,
                &snapshot,
                editor_input_enabled,
            ));
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

    fn show_search_prompt(&mut self, ctx: &egui::Context) {
        if !self.runtime.search_prompt {
            return;
        }

        let mut open = true;
        egui::Window::new("Search").open(&mut open).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(
                    &mut self.runtime.search_scope,
                    SearchScopeProjection::ActiveFile,
                    "File",
                );
                ui.radio_value(
                    &mut self.runtime.search_scope,
                    SearchScopeProjection::Workspace,
                    "Workspace",
                );
            });
            ui.text_edit_singleline(&mut self.runtime.search_query_text);
            ui.horizontal(|ui| {
                if ui.button("Search").clicked() {
                    let query = self.runtime.search_query_text.clone();
                    self.runtime.search_prompt = false;
                    let _ = self.runtime.handle_action(DesktopAction::RunSearch {
                        scope: self.runtime.search_scope,
                        query,
                        limit: 0,
                    });
                }
                if ui.button("Cancel").clicked() {
                    self.runtime.search_prompt = false;
                    if let Some(query_id) = self
                        .runtime
                        .projection_snapshot()
                        .search_projection
                        .query_id
                        .clone()
                    {
                        let _ = self
                            .runtime
                            .handle_action(DesktopAction::CancelSearch { query_id });
                    }
                }
            });
        });

        if !open {
            self.runtime.search_prompt = false;
        }
    }
}

impl eframe::App for DesktopEframeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_keyboard(ui);
        let snapshot = self.runtime.projection_snapshot();
        let view_state = self.runtime.projection_view_state();
        let output = self
            .runtime
            .view
            .render_with_state(ui, &snapshot, &view_state);
        for action in output.actions {
            let _ = self.runtime.handle_action(action);
        }
        if output.needs_repaint {
            ui.ctx().request_repaint();
        }
        self.show_open_path_prompt(ui.ctx());
        self.show_search_prompt(ui.ctx());
        if self.runtime.quit_requested() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn status_message(severity: StatusSeverity, message: impl Into<String>) -> StatusMessageProjection {
    StatusMessageProjection {
        severity,
        message: message.into(),
    }
}

fn close_dirty_prompt_active(snapshot: &ShellProjectionSnapshot) -> bool {
    snapshot
        .daily_editing_projection
        .close_dirty_prompt
        .is_some()
}

fn active_buffer_for_input(snapshot: &ShellProjectionSnapshot) -> Option<BufferId> {
    snapshot
        .daily_editing_projection
        .tabs
        .active_buffer_id
        .or(snapshot.active_buffer_projection.buffer_id)
}

fn adjacent_tab_id(snapshot: &ShellProjectionSnapshot, direction: isize) -> Option<BufferId> {
    let tabs = &snapshot.daily_editing_projection.tabs.tabs;
    if tabs.is_empty() {
        return active_buffer_for_input(snapshot);
    }

    let active = active_buffer_for_input(snapshot)?;
    let active_index = tabs
        .iter()
        .position(|tab| tab.buffer_id == active)
        .or_else(|| tabs.iter().position(|tab| tab.active))
        .unwrap_or(0);
    let len = tabs.len() as isize;
    let next = (active_index as isize + direction).rem_euclid(len) as usize;
    Some(tabs[next].buffer_id)
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

fn projected_scroll(snapshot: &ShellProjectionSnapshot) -> ViewportScroll {
    let active = active_buffer_for_input(snapshot);
    if let Some(state) = snapshot
        .daily_editing_projection
        .viewport_states
        .iter()
        .find(|state| Some(state.buffer_id) == active)
    {
        return state.scroll;
    }

    snapshot
        .active_buffer_projection
        .viewport
        .as_ref()
        .map(|viewport| viewport.scroll)
        .unwrap_or(ViewportScroll {
            top_line: 0,
            left_column: 0,
        })
}

fn editor_text_input_actions(
    events: &[egui::Event],
    snapshot: &ShellProjectionSnapshot,
    editor_input_enabled: bool,
) -> Vec<DesktopAction> {
    if !editor_input_enabled {
        return Vec::new();
    }

    let at = projected_cursor(snapshot);
    let mut actions = Vec::new();
    for event in events {
        match event {
            egui::Event::Text(text) if !text.is_empty() => {
                actions.push(DesktopAction::InsertText {
                    text: text.clone(),
                    at,
                });
            }
            egui::Event::Paste(text) if !text.is_empty() => {
                actions.push(DesktopAction::ClipboardPaste {
                    text: text.clone(),
                    at,
                });
            }
            _ => {}
        }
    }
    actions
}

fn editor_keyboard_control_actions(
    input: &egui::InputState,
    snapshot: &ShellProjectionSnapshot,
    editor_input_enabled: bool,
) -> Vec<DesktopAction> {
    if !editor_input_enabled || input.modifiers.command {
        return Vec::new();
    }

    let Some(buffer_id) = active_buffer_for_input(snapshot) else {
        return Vec::new();
    };

    let mut actions = Vec::new();
    if input.key_pressed(egui::Key::ArrowLeft) {
        actions.push(cursor_or_selection_action(
            buffer_id,
            projected_cursor(snapshot),
            0,
            -1,
            input.modifiers.shift,
        ));
    }
    if input.key_pressed(egui::Key::ArrowRight) {
        actions.push(cursor_or_selection_action(
            buffer_id,
            projected_cursor(snapshot),
            0,
            1,
            input.modifiers.shift,
        ));
    }
    if input.key_pressed(egui::Key::ArrowUp) {
        actions.push(cursor_or_selection_action(
            buffer_id,
            projected_cursor(snapshot),
            -1,
            0,
            input.modifiers.shift,
        ));
    }
    if input.key_pressed(egui::Key::ArrowDown) {
        actions.push(cursor_or_selection_action(
            buffer_id,
            projected_cursor(snapshot),
            1,
            0,
            input.modifiers.shift,
        ));
    }
    if input.key_pressed(egui::Key::PageUp) {
        let scroll = projected_scroll(snapshot);
        actions.push(DesktopAction::SetViewportScroll {
            buffer_id: Some(buffer_id),
            scroll: ViewportScroll {
                top_line: scroll.top_line.saturating_sub(25),
                left_column: scroll.left_column,
            },
        });
    }
    if input.key_pressed(egui::Key::PageDown) {
        let scroll = projected_scroll(snapshot);
        actions.push(DesktopAction::SetViewportScroll {
            buffer_id: Some(buffer_id),
            scroll: ViewportScroll {
                top_line: scroll.top_line.saturating_add(25),
                left_column: scroll.left_column,
            },
        });
    }

    actions
}

fn cursor_or_selection_action(
    buffer_id: BufferId,
    cursor: TextCoordinate,
    line_delta: i32,
    character_delta: i32,
    selecting: bool,
) -> DesktopAction {
    let target = moved_coordinate(cursor, line_delta, character_delta);
    if selecting {
        DesktopAction::SetSelection {
            buffer_id: Some(buffer_id),
            range: ordered_range(cursor, target),
        }
    } else {
        DesktopAction::SetCursor {
            buffer_id: Some(buffer_id),
            cursor: target,
        }
    }
}

fn moved_coordinate(
    coordinate: TextCoordinate,
    line_delta: i32,
    character_delta: i32,
) -> TextCoordinate {
    let line = if line_delta.is_negative() {
        coordinate.line.saturating_sub(line_delta.unsigned_abs())
    } else {
        coordinate.line.saturating_add(line_delta as u32)
    };
    let character = if character_delta.is_negative() {
        coordinate
            .character
            .saturating_sub(character_delta.unsigned_abs())
    } else {
        coordinate.character.saturating_add(character_delta as u32)
    };

    TextCoordinate {
        line,
        character,
        byte_offset: None,
        utf16_offset: Some(character as u64),
    }
}

fn ordered_range(first: TextCoordinate, second: TextCoordinate) -> ProtocolTextRange {
    if (first.line, first.character) <= (second.line, second.character) {
        ProtocolTextRange {
            start: first,
            end: second,
        }
    } else {
        ProtocolTextRange {
            start: second,
            end: first,
        }
    }
}

#[cfg(test)]
mod tests {
    use devil_protocol::{BufferId, CanonicalPath, FileId};
    use devil_ui::ui::{CloseDirtyPromptProjection, DailyEditingProjection};
    use devil_ui::{ActiveBufferProjection, Shell};

    use super::*;

    fn snapshot_with_active_buffer() -> ShellProjectionSnapshot {
        let mut snapshot = Shell::empty("Keyboard").projection_snapshot();
        snapshot.active_buffer_projection = ActiveBufferProjection {
            buffer_id: Some(BufferId(1)),
            ..ActiveBufferProjection::empty()
        };
        snapshot
    }

    #[test]
    fn prompt_active_text_input_does_not_route_to_editor() {
        let events = vec![
            egui::Event::Text("Cargo.toml".to_string()),
            egui::Event::Paste("pasted/path.rs".to_string()),
        ];

        assert!(
            editor_text_input_actions(&events, &snapshot_with_active_buffer(), false).is_empty()
        );
    }

    #[test]
    fn editor_text_input_routes_text_and_clipboard_paste() {
        let events = vec![
            egui::Event::Text("x".to_string()),
            egui::Event::Paste("clip".to_string()),
        ];
        let at = TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: Some(0),
            utf16_offset: Some(0),
        };

        assert_eq!(
            editor_text_input_actions(&events, &snapshot_with_active_buffer(), true),
            vec![
                DesktopAction::InsertText {
                    text: "x".to_string(),
                    at,
                },
                DesktopAction::ClipboardPaste {
                    text: "clip".to_string(),
                    at,
                },
            ]
        );
    }

    #[test]
    fn close_dirty_prompt_disables_editor_text_input() {
        let mut snapshot = snapshot_with_active_buffer();
        snapshot.daily_editing_projection = DailyEditingProjection {
            close_dirty_prompt: Some(CloseDirtyPromptProjection {
                buffer_id: BufferId(1),
                file_id: Some(FileId(2)),
                file_path: Some(CanonicalPath("dirty.txt".to_string())),
                title: "dirty.txt".to_string(),
                message: "Save changes before closing dirty.txt?".to_string(),
            }),
            ..DailyEditingProjection::empty()
        };
        let events = vec![egui::Event::Text("x".to_string())];

        assert!(close_dirty_prompt_active(&snapshot));
        assert!(
            editor_text_input_actions(&events, &snapshot, !close_dirty_prompt_active(&snapshot))
                .is_empty()
        );
    }
}
