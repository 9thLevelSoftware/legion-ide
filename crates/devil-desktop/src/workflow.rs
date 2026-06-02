//! Desktop runtime workflow boundary.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use devil_app::{
    AppAiRunOutcome, AppCloseTabOutcome, AppCommandOutcome, AppComposition, AppProductMode,
    AppSaveAllItemOutcome, AppSaveAllItemStatus, AppSaveAllOutcome, AppSaveAllStatus,
    AppSaveOutcome, AppSessionRestoreOutcome,
};
use devil_protocol::{
    AgentRunId, BufferId, CanonicalPath, CollaborationSessionId, DelegatedTaskPlanContract,
    DelegatedTaskPlanId, LegionWorkflowMergeReadinessState, LegionWorkflowSessionId,
    PluginDenialReason, PluginHostCallResponse, PluginId, PluginManifest, PrincipalId, ProposalId,
    ProposalLifecycleState, ProposalLifecycleTransition, ProposalResponse, ProtocolTextRange,
    RemoteWorkspaceSessionId, SessionPanelState, TextCoordinate, ViewportScroll,
    WorkspaceSessionRecord, WorkspaceTrustState,
};
use devil_ui::{
    CommandDispatchIntent, DockMode, SearchScopeProjection, Shell, ShellProjectionSnapshot,
    StatusMessageProjection, StatusSeverity,
};

use crate::{
    beta::{self, BetaWorkflowConfig},
    bridge::{
        DesktopAction, DesktopAppRequest, DesktopBridgeError, DesktopBridgeOutput,
        DesktopCommandBridge,
    },
    diagnostics::DesktopDiagnosticsExport,
    health::DesktopOperationalHealthSnapshot,
    platform::{
        NativePlatformObservation, build_platform_adapter_checks, build_platform_smoke_snapshot,
    },
    session::DesktopSessionStore,
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
    /// Optional non-native-window GUI Phase 7 beta smoke configuration.
    pub beta: Option<BetaWorkflowConfig>,
    /// Optional metadata-only session JSON path.
    pub session_state: Option<PathBuf>,
    /// Optional metadata-only diagnostics markdown path.
    pub diagnostics_export: Option<PathBuf>,
}

impl DesktopLaunchConfig {
    /// Build a launch config with the default desktop principal.
    pub fn new(workspace_root: PathBuf, initial_file: Option<String>) -> Self {
        Self {
            workspace_root,
            initial_file,
            principal: PrincipalId("desktop".to_string()),
            smoke: None,
            beta: None,
            session_state: None,
            diagnostics_export: None,
        }
    }

    /// Attach a metadata-only session state path.
    pub fn with_session_state(mut self, path: PathBuf) -> Self {
        self.session_state = Some(path);
        self
    }

    /// Attach a metadata-only diagnostics export path.
    pub fn with_diagnostics_export(mut self, path: PathBuf) -> Self {
        self.diagnostics_export = Some(path);
        self
    }

    /// Parse launch config from process arguments.
    pub fn from_env_args() -> Result<Self> {
        Self::from_args(std::env::args_os().skip(1))
    }

    /// Parse launch config from an argument iterator.
    pub fn from_args(args: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut smoke_enabled = false;
        let mut beta_enabled = false;
        let mut workspace_root = None;
        let mut beta_workspace_root = None;
        let mut initial_file = None;
        let mut duration_ms = 1500;
        let mut evidence_path =
            PathBuf::from("plans/evidence/gui-productization/phase-2-renderer-smoke.md");
        let mut session_state = None;
        let mut diagnostics_export = None;
        let mut positionals = Vec::new();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            let arg_text = arg.to_string_lossy();
            match arg_text.as_ref() {
                "--smoke" => smoke_enabled = true,
                "--beta-smoke" => beta_enabled = true,
                "--workspace" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--workspace requires a path"))?;
                    workspace_root = Some(PathBuf::from(value));
                }
                "--beta-workspace" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--beta-workspace requires a path"))?;
                    beta_workspace_root = Some(PathBuf::from(value));
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
                "--session-state" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--session-state requires a path"))?;
                    session_state = Some(PathBuf::from(value));
                }
                "--diagnostics-export" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--diagnostics-export requires a path"))?;
                    diagnostics_export = Some(PathBuf::from(value));
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
        if smoke_enabled && beta_enabled {
            return Err(anyhow!("--smoke and --beta-smoke cannot be combined"));
        }

        let initial_file = initial_file
            .or_else(|| {
                positionals
                    .get(1)
                    .map(|path| path.to_string_lossy().into_owned())
            })
            .filter(|path| !path.trim().is_empty());

        let smoke = if smoke_enabled {
            Some(RendererSmokeConfig::new(
                duration_ms,
                evidence_path.clone(),
            )?)
        } else {
            None
        };
        let beta = if beta_enabled {
            Some(BetaWorkflowConfig::new(
                workspace_root.clone(),
                beta_workspace_root
                    .unwrap_or_else(|| PathBuf::from(beta::DEFAULT_BETA_WORKSPACE_PATH)),
                evidence_path,
                session_state
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(beta::DEFAULT_BETA_SESSION_STATE_PATH)),
                diagnostics_export
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(beta::DEFAULT_BETA_DIAGNOSTICS_EXPORT_PATH)),
            )?)
        } else {
            None
        };

        Ok(Self {
            workspace_root,
            initial_file,
            principal: PrincipalId("desktop".to_string()),
            smoke,
            beta,
            session_state,
            diagnostics_export,
        })
    }
}

/// User-visible outcome from the desktop workflow harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopWorkflowOutcome {
    /// Command had no effect.
    Noop,
    /// Product mode changed through app authority.
    ProductModeChanged {
        /// Active app-owned product mode.
        mode: DockMode,
    },
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
    /// Dirty-close prompt was cancelled without closing or discarding text.
    DirtyCloseCancelled(BufferId),
    /// Cursor update completed through editor authority.
    CursorSet(BufferId),
    /// Selection update completed through editor authority.
    SelectionSet(BufferId),
    /// Viewport scroll update completed through app authority.
    ViewportScrollSet(BufferId),
    /// Search projection changed through app authority.
    SearchUpdated,
    /// Structural search projection changed through app authority.
    StructuralSearchUpdated,
    /// Git projection changed through app authority.
    GitUpdated,
    /// Debug projection changed through app authority.
    DebugProjectionUpdated,
    /// Language tooling projection changed through app authority.
    LanguageToolingUpdated,
    /// Assist inline prediction projection changed through app authority.
    AssistInlinePredictionUpdated {
        /// Whether an active ghost prediction is projected.
        active: bool,
        /// Number of projected inline prediction rows.
        row_count: usize,
        /// Number of stale projected predictions.
        stale_count: usize,
        /// User-visible status summary.
        status: String,
    },
    /// Terminal panel projection changed through app authority.
    TerminalPanelUpdated,
    /// Proposal lifecycle state changed through app authority.
    ProposalLifecycleUpdated {
        /// Proposal whose lifecycle changed.
        proposal_id: ProposalId,
        /// Resulting lifecycle state.
        lifecycle_state: ProposalLifecycleState,
        /// User-visible status summary.
        status: String,
    },
    /// Proposal detail projection selection changed through app authority.
    ProposalDetailsOpened(ProposalId),
    /// Assisted-AI metadata changed through app authority.
    AssistedAiUpdated {
        /// Assisted-AI run represented by the app outcome.
        run_id: AgentRunId,
        /// Proposal created by the run, when the run was proposal-producing.
        proposal_id: Option<ProposalId>,
        /// User-visible status summary.
        status: String,
    },
    /// Plugin command invocation changed through app-owned plugin authority.
    PluginCommand {
        /// Plugin selected from projection data.
        plugin_id: PluginId,
        /// Command selected from projection data.
        command_id: String,
        /// Normalized desktop command status.
        status: DesktopPluginCommandStatus,
        /// User-visible status summary.
        message: String,
    },
    /// Collaboration workflow state changed through app-owned collaboration/proposal authority.
    CollaborationUpdated {
        /// Collaboration session represented by the outcome, if available.
        session_id: Option<CollaborationSessionId>,
        /// Normalized desktop collaboration status.
        status: DesktopCollaborationStatus,
        /// User-visible status summary.
        message: String,
    },
    /// Remote workspace workflow state changed through app-owned remote authority.
    RemoteUpdated {
        /// Remote workspace session represented by the outcome.
        session_id: RemoteWorkspaceSessionId,
        /// Normalized desktop remote status.
        status: DesktopRemoteStatus,
        /// User-visible status summary.
        message: String,
    },
    /// Delegated task command-center review changed through app-owned proposal authority.
    DelegatedTaskReviewed {
        /// Delegated task plan represented by the outcome, if plan-scoped.
        plan_id: Option<DelegatedTaskPlanId>,
        /// Proposal represented by the outcome, if proposal-scoped.
        proposal_id: Option<ProposalId>,
        /// Normalized desktop delegated task status.
        status: DesktopDelegatedTaskStatus,
        /// User-visible status summary.
        message: String,
    },
    /// Legion workflow command-center request changed through app-owned workflow authority.
    LegionWorkflowReviewed {
        /// Workflow session represented by the outcome.
        session_id: LegionWorkflowSessionId,
        /// Proposal represented by the outcome, if proposal-scoped.
        proposal_id: Option<ProposalId>,
        /// Normalized desktop Legion workflow status.
        status: DesktopLegionWorkflowStatus,
        /// User-visible status summary.
        message: String,
    },
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

/// Desktop-facing status for a projected plugin command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopPluginCommandStatus {
    /// Plugin runtime accepted the metadata-only command.
    Invoked,
    /// Plugin command created a proposal through app authority.
    ProposalCreated,
    /// Plugin runtime denied the command.
    Denied,
    /// Plugin runtime was absent or unavailable for a projected command.
    NoRuntime,
}

/// Desktop-facing status for collaboration GUI workflows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopCollaborationStatus {
    /// Session join completed through app authority.
    Joined,
    /// Session leave completed through app authority.
    Left,
    /// Metadata-only presence publication completed.
    PresencePublished,
    /// Collaboration operation was accepted by app/editor authority.
    OperationApplied,
}

/// Desktop-facing status for remote workspace GUI workflows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopRemoteStatus {
    /// Remote workspace session connected or reconnected through app authority.
    Connected,
}

/// Desktop-facing status for delegated task command-center workflows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopDelegatedTaskStatus {
    /// Plan metadata was inspected without runtime activation.
    PlanInspected,
    /// Linked proposal preview was opened through proposal authority.
    ProposalPreviewOpened,
    /// Linked proposal details were opened through proposal authority.
    ProposalDetailsOpened,
    /// Delegate chat turn completed through app authority.
    ChatSent,
    /// Delegate proposal hunk review was recorded.
    ProposalHunkReviewed,
    /// Delegate tool permission decision was recorded.
    ToolPermissionRecorded,
}

/// Desktop-facing status for Legion workflow command-center requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopLegionWorkflowStatus {
    /// Session metadata was inspected without execution.
    SessionInspected,
    /// Linked proposal preview was requested through proposal authority.
    ProposalPreviewOpened,
    /// Linked proposal details were requested through proposal authority.
    ProposalDetailsOpened,
    /// Verification metadata recording was requested.
    VerificationRequested,
    /// Sign-off metadata recording was requested.
    SignOffRequested,
    /// Conflict-resolution metadata was requested.
    ConflictResolutionRequested,
    /// Merge readiness was requested and remains blocked.
    MergeReadinessBlocked,
    /// Merge readiness was requested and waits for approval.
    MergeReadinessWaitingForApproval,
    /// Merge readiness was requested and is proposal-mediated ready.
    MergeReadinessReady,
    /// Automate MCP tool permission changed.
    ToolPermissionRecorded,
    /// Automate kill switch was triggered.
    KillSwitchTriggered,
}

/// Renderer-backed desktop runtime.
pub struct DesktopRuntime {
    app: AppComposition,
    shell: Shell,
    bridge: DesktopCommandBridge,
    view: ProjectionView,
    workspace_root: PathBuf,
    principal: PrincipalId,
    open_path_prompt: bool,
    open_path_text: String,
    search_prompt: bool,
    search_query_text: String,
    search_scope: SearchScopeProjection,
    structural_search_prompt: bool,
    structural_search_pattern_text: String,
    structural_search_rewrite_text: String,
    structural_search_scope: SearchScopeProjection,
    explorer_expansion: BTreeSet<String>,
    panel_state: SessionPanelState,
    session_state_path: Option<PathBuf>,
    diagnostics_export_path: Option<PathBuf>,
    quit_requested: bool,
    last_status: Option<StatusMessageProjection>,
    last_status_details: Vec<StatusMessageProjection>,
    last_outcome: DesktopWorkflowOutcome,
}

impl DesktopRuntime {
    /// Open the configured workspace and optional initial file.
    pub fn open(config: DesktopLaunchConfig) -> Result<Self> {
        let session_record = match &config.session_state {
            Some(path) => DesktopSessionStore::load(path)?,
            None => None,
        };
        let mut app = AppComposition::new();
        app.open_workspace(
            &config.workspace_root,
            WorkspaceTrustState::Trusted,
            config.principal.clone(),
        )?;

        let mut explorer_expansion = BTreeSet::new();
        let mut panel_state = default_panel_state();
        let mut status = status_message(StatusSeverity::Info, "Desktop adapter ready");
        let mut status_details = Vec::new();

        if let Some(record) = &session_record {
            if session_workspace_matches(&config.workspace_root, record) {
                let restore = app.restore_workspace_session_record(record)?;
                explorer_expansion = record
                    .explorer_expansion
                    .iter()
                    .map(|path| path.0.clone())
                    .collect();
                panel_state = record.panel_state.clone();
                let (restore_status, restore_details) = restore_status_messages(&restore);
                status = restore_status;
                status_details = restore_details;
            } else {
                status = status_message(
                    StatusSeverity::Warning,
                    "Session restore skipped: workspace mismatch",
                );
                status_details.push(status_message(
                    StatusSeverity::Warning,
                    "Session last_workspace_path did not match launch workspace",
                ));
            }
        }

        if let Some(initial_file) = &config.initial_file {
            app.open_file(initial_file)?;
        }

        let mut snapshot = app.shell_projection_snapshot(WINDOW_TITLE)?;
        snapshot.status_messages.push(status.clone());
        snapshot
            .status_messages
            .extend(status_details.iter().cloned());

        let mut runtime = Self {
            app,
            shell: Shell::new(snapshot),
            bridge: DesktopCommandBridge::new(),
            view: ProjectionView::new(),
            workspace_root: config.workspace_root.clone(),
            principal: config.principal,
            open_path_prompt: false,
            open_path_text: String::new(),
            search_prompt: false,
            search_query_text: String::new(),
            search_scope: SearchScopeProjection::ActiveFile,
            structural_search_prompt: false,
            structural_search_pattern_text: String::new(),
            structural_search_rewrite_text: String::new(),
            structural_search_scope: SearchScopeProjection::ActiveFile,
            explorer_expansion,
            panel_state,
            session_state_path: config.session_state,
            diagnostics_export_path: config.diagnostics_export,
            quit_requested: false,
            last_status: Some(status),
            last_status_details: status_details,
            last_outcome: DesktopWorkflowOutcome::Noop,
        };
        runtime.persist_diagnostics_if_configured();
        Ok(runtime)
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

        self.persist_session_if_configured();
        self.refresh_projection()?;
        self.last_outcome = outcome.clone();
        self.persist_diagnostics_if_configured();
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

    /// Set the app-owned product mode used by AI dispatch authority.
    pub fn set_product_mode(&mut self, mode: AppProductMode) -> Result<()> {
        self.app.set_product_mode(mode);
        self.set_status(
            StatusSeverity::Info,
            format!("Product mode changed to {}", mode.to_dock_mode().label()),
        );
        self.refresh_projection()
    }

    /// Load a plugin manifest through app-owned plugin authority and refresh projections.
    pub fn load_plugin_manifest(&mut self, manifest: PluginManifest) -> Result<PluginId> {
        let plugin_id = self.app.load_plugin_manifest(manifest)?;
        self.set_status(
            StatusSeverity::Info,
            format!("Plugin {} loaded", plugin_id.0),
        );
        self.refresh_projection()?;
        Ok(plugin_id)
    }

    /// Enable app-owned local collaboration runtime for explicit test or launch harnesses.
    pub fn enable_local_collaboration_runtime(&mut self) -> Result<()> {
        self.app.enable_local_collaboration_runtime();
        self.set_status(
            StatusSeverity::Info,
            "Collaboration runtime enabled by app policy",
        );
        self.refresh_projection()
    }

    /// Enable app-owned remote workspace runtime for explicit test or launch harnesses.
    pub fn enable_remote_development_runtime(&mut self) -> Result<()> {
        self.app.enable_remote_development_runtime();
        self.set_status(
            StatusSeverity::Info,
            "Remote workspace runtime enabled by app policy",
        );
        self.refresh_projection()
    }

    /// Enable the app-owned deterministic debug fixture for test harnesses.
    pub fn enable_debug_fixture_for_tests(&mut self) {
        self.app.enable_debug_fixture_for_tests();
        self.set_status(StatusSeverity::Info, "Debug fixture enabled by app policy");
        self.refresh_projection()
            .expect("debug fixture projection refresh should succeed");
    }

    /// Seed delegated task plan contracts for projection-only command-center harnesses.
    pub fn seed_delegated_task_plan_contracts(
        &mut self,
        plans: Vec<DelegatedTaskPlanContract>,
    ) -> Result<()> {
        let plan_count = plans.len();
        self.app.seed_delegated_task_plan_contracts(plans);
        self.set_status(
            StatusSeverity::Info,
            format!("Delegated task plan contracts projected: {plan_count}"),
        );
        self.refresh_projection()
    }

    /// Returns whether an explorer path is expanded by adapter-local state.
    pub fn explorer_path_expanded(&self, path: &str) -> bool {
        self.explorer_expansion.contains(path)
    }

    /// Return the adapter-local restored panel state.
    pub fn panel_state(&self) -> &SessionPanelState {
        &self.panel_state
    }

    /// Replace adapter-local panel state for future session captures.
    pub fn set_panel_state(&mut self, panel_state: SessionPanelState) {
        self.panel_state = panel_state;
    }

    /// Capture a metadata-only session record with adapter-local desktop state applied.
    pub fn capture_session_record(&self) -> Result<WorkspaceSessionRecord> {
        let mut record = self.app.capture_workspace_session_record()?;
        record.explorer_expansion = self
            .explorer_expansion
            .iter()
            .cloned()
            .map(CanonicalPath)
            .collect();
        record.panel_state = self.panel_state.clone();
        Ok(record)
    }

    /// Save the current session to the configured session path.
    pub fn save_session_state(&self) -> Result<()> {
        let Some(path) = &self.session_state_path else {
            return Ok(());
        };
        let record = self.capture_session_record()?;
        DesktopSessionStore::save(path, &record)?;
        Ok(())
    }

    /// Build metadata-only diagnostics from the current projection.
    pub fn diagnostics_export(&self) -> DesktopDiagnosticsExport {
        let snapshot = self.projection_snapshot();
        let tabs = &snapshot.daily_editing_projection.tabs.tabs;
        let dirty_tab_count = tabs.iter().filter(|tab| tab.dirty).count();
        let platform = build_platform_smoke_snapshot(
            &snapshot,
            build_platform_adapter_checks(&snapshot),
            NativePlatformObservation::default(),
        );
        let last_outcome = format!("{:?}", self.last_outcome);
        let health = DesktopOperationalHealthSnapshot::from_runtime(
            &snapshot,
            self.workspace_root.display().to_string(),
            &self.last_outcome,
            self.session_state_path.is_some(),
            self.diagnostics_export_path.is_some(),
        );

        DesktopDiagnosticsExport {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            workspace: self.workspace_root.display().to_string(),
            open_tab_count: tabs.len(),
            dirty_tab_count,
            status_message_count: snapshot.status_messages.len(),
            session_state_configured: self.session_state_path.is_some(),
            last_outcome,
            health,
            platform,
        }
    }

    fn projection_view_state(&self) -> DesktopProjectionViewState {
        DesktopProjectionViewState {
            expanded_explorer_paths: self.explorer_expansion.clone(),
            selected_explorer_file: None,
            dock_layouts: devil_ui::DockLayout::standard_all_modes(),
        }
    }

    fn editor_input_enabled(&self, snapshot: &ShellProjectionSnapshot) -> bool {
        !self.open_path_prompt
            && !self.search_prompt
            && !self.structural_search_prompt
            && !close_dirty_prompt_active(snapshot)
    }

    fn dispatch_intent(&mut self, intent: CommandDispatchIntent) -> Result<DesktopWorkflowOutcome> {
        let plugin_context = plugin_intent_context(&intent);
        match self.app.dispatch_ui_intent(intent) {
            Ok(outcome) => Ok(self.map_app_outcome(outcome, plugin_context)),
            Err(error) => {
                let message = error.to_string();
                if let Some((plugin_id, command_id)) = plugin_context {
                    let status = format!(
                        "Plugin command unavailable {} {}: {message}",
                        plugin_id.0, command_id
                    );
                    self.set_status(StatusSeverity::Warning, status.clone());
                    return Ok(DesktopWorkflowOutcome::PluginCommand {
                        plugin_id,
                        command_id,
                        status: DesktopPluginCommandStatus::NoRuntime,
                        message: status,
                    });
                }
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
            DesktopAppRequest::ShowStructuralSearchPrompt { scope } => {
                self.structural_search_prompt = true;
                self.structural_search_scope = scope;
                self.set_status(StatusSeverity::Info, "Structural search requested");
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAppRequest::ToggleExplorerPath { path } => {
                if !self.explorer_expansion.remove(&path) {
                    self.explorer_expansion.insert(path.clone());
                }
                self.set_status(StatusSeverity::Info, format!("Explorer toggled {path}"));
                Ok(DesktopWorkflowOutcome::ExplorerPathToggled(path))
            }
            DesktopAppRequest::CancelDirtyClose { buffer_id } => {
                match self.app.cancel_dirty_close(buffer_id) {
                    Ok(()) => {
                        self.set_status(
                            StatusSeverity::Info,
                            format!("Close cancelled {}", buffer_id.0),
                        );
                        Ok(DesktopWorkflowOutcome::DirtyCloseCancelled(buffer_id))
                    }
                    Err(error) => {
                        let message = error.to_string();
                        self.set_status(StatusSeverity::Error, message.clone());
                        Ok(DesktopWorkflowOutcome::Error(message))
                    }
                }
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
            DesktopAppRequest::ConnectRemoteWorkspace {
                session_id,
                authority_label,
            } => match self
                .app
                .connect_remote_workspace_session(session_id, authority_label.clone())
            {
                Ok(_) => {
                    let message = format!(
                        "Remote workspace connected {} authority={authority_label}",
                        session_id.0
                    );
                    self.set_status(StatusSeverity::Info, message.clone());
                    Ok(DesktopWorkflowOutcome::RemoteUpdated {
                        session_id,
                        status: DesktopRemoteStatus::Connected,
                        message,
                    })
                }
                Err(error) => {
                    let message = error.to_string();
                    self.set_status(StatusSeverity::Error, message.clone());
                    Ok(DesktopWorkflowOutcome::Error(message))
                }
            },
            DesktopAppRequest::InspectDelegatedTaskPlan { plan_id } => {
                let message = format!(
                    "Delegated task plan inspected {}: approval-gated, autonomous apply unsupported",
                    plan_id.0
                );
                self.set_status(StatusSeverity::Info, message.clone());
                Ok(DesktopWorkflowOutcome::DelegatedTaskReviewed {
                    plan_id: Some(plan_id),
                    proposal_id: None,
                    status: DesktopDelegatedTaskStatus::PlanInspected,
                    message,
                })
            }
            DesktopAppRequest::OpenDelegatedProposalPreview { proposal_id } => {
                match self
                    .app
                    .dispatch_ui_intent(CommandDispatchIntent::PreviewProposal { proposal_id })
                {
                    Ok(_) => {
                        let message = format!(
                            "Delegated task proposal preview opened {}: proposal-mediated",
                            proposal_id.0
                        );
                        self.set_status(StatusSeverity::Info, message.clone());
                        Ok(DesktopWorkflowOutcome::DelegatedTaskReviewed {
                            plan_id: None,
                            proposal_id: Some(proposal_id),
                            status: DesktopDelegatedTaskStatus::ProposalPreviewOpened,
                            message,
                        })
                    }
                    Err(error) => {
                        let message = error.to_string();
                        self.set_status(StatusSeverity::Error, message.clone());
                        Ok(DesktopWorkflowOutcome::Error(message))
                    }
                }
            }
            DesktopAppRequest::OpenDelegatedProposalDetails { proposal_id } => {
                match self
                    .app
                    .dispatch_ui_intent(CommandDispatchIntent::OpenProposalDetails { proposal_id })
                {
                    Ok(_) => {
                        let message = format!(
                            "Delegated task proposal details opened {}: proposal-mediated",
                            proposal_id.0
                        );
                        self.set_status(StatusSeverity::Info, message.clone());
                        Ok(DesktopWorkflowOutcome::DelegatedTaskReviewed {
                            plan_id: None,
                            proposal_id: Some(proposal_id),
                            status: DesktopDelegatedTaskStatus::ProposalDetailsOpened,
                            message,
                        })
                    }
                    Err(error) => {
                        let message = error.to_string();
                        self.set_status(StatusSeverity::Error, message.clone());
                        Ok(DesktopWorkflowOutcome::Error(message))
                    }
                }
            }
            DesktopAppRequest::InspectLegionWorkflowSession { session_id } => {
                let message = format!(
                    "Legion workflow session inspected {}: app-owned, proposal-mediated, autonomous merge unsupported",
                    session_id.0
                );
                self.set_status(StatusSeverity::Info, message.clone());
                Ok(DesktopWorkflowOutcome::LegionWorkflowReviewed {
                    session_id,
                    proposal_id: None,
                    status: DesktopLegionWorkflowStatus::SessionInspected,
                    message,
                })
            }
            DesktopAppRequest::OpenLegionWorkflowProposalPreview {
                session_id,
                proposal_id,
            } => match self
                .app
                .dispatch_ui_intent(CommandDispatchIntent::PreviewProposal { proposal_id })
            {
                Ok(_) => {
                    let message = format!(
                        "Legion workflow proposal preview opened {} for {}: proposal-mediated",
                        proposal_id.0, session_id.0
                    );
                    self.set_status(StatusSeverity::Info, message.clone());
                    Ok(DesktopWorkflowOutcome::LegionWorkflowReviewed {
                        session_id,
                        proposal_id: Some(proposal_id),
                        status: DesktopLegionWorkflowStatus::ProposalPreviewOpened,
                        message,
                    })
                }
                Err(error) => {
                    let message = error.to_string();
                    self.set_status(StatusSeverity::Error, message.clone());
                    Ok(DesktopWorkflowOutcome::Error(message))
                }
            },
            DesktopAppRequest::OpenLegionWorkflowProposalDetails {
                session_id,
                proposal_id,
            } => match self
                .app
                .dispatch_ui_intent(CommandDispatchIntent::OpenProposalDetails { proposal_id })
            {
                Ok(_) => {
                    let message = format!(
                        "Legion workflow proposal details opened {} for {}: proposal-mediated",
                        proposal_id.0, session_id.0
                    );
                    self.set_status(StatusSeverity::Info, message.clone());
                    Ok(DesktopWorkflowOutcome::LegionWorkflowReviewed {
                        session_id,
                        proposal_id: Some(proposal_id),
                        status: DesktopLegionWorkflowStatus::ProposalDetailsOpened,
                        message,
                    })
                }
                Err(error) => {
                    let message = error.to_string();
                    self.set_status(StatusSeverity::Error, message.clone());
                    Ok(DesktopWorkflowOutcome::Error(message))
                }
            },
            DesktopAppRequest::RequestLegionWorkflowVerification {
                session_id,
                gate_id,
            } => {
                let message = format!(
                    "Legion workflow verification requested {} gate={}: app-owned metadata request",
                    session_id.0, gate_id.0
                );
                Ok(self.legion_workflow_request_outcome(
                    session_id,
                    None,
                    DesktopLegionWorkflowStatus::VerificationRequested,
                    message,
                ))
            }
            DesktopAppRequest::RequestLegionWorkflowSignOff {
                session_id,
                sign_off_id,
            } => {
                let message = format!(
                    "Legion workflow sign-off requested {} signoff={}: app-owned metadata request",
                    session_id.0, sign_off_id.0
                );
                Ok(self.legion_workflow_request_outcome(
                    session_id,
                    None,
                    DesktopLegionWorkflowStatus::SignOffRequested,
                    message,
                ))
            }
            DesktopAppRequest::ResolveLegionWorkflowConflict {
                session_id,
                conflict_id,
            } => {
                let message = format!(
                    "Legion workflow conflict resolution requested {} conflict={}: app-owned metadata request",
                    session_id.0, conflict_id.0
                );
                Ok(self.legion_workflow_request_outcome(
                    session_id,
                    None,
                    DesktopLegionWorkflowStatus::ConflictResolutionRequested,
                    message,
                ))
            }
            DesktopAppRequest::RequestLegionWorkflowMergeReadiness { session_id } => {
                let readiness_state = self
                    .projection_snapshot()
                    .legion_workflow_projection
                    .rows
                    .iter()
                    .find(|row| row.session_id == session_id)
                    .map(|row| row.merge_readiness.state)
                    .unwrap_or(LegionWorkflowMergeReadinessState::Blocked);
                let status = match readiness_state {
                    LegionWorkflowMergeReadinessState::Ready => {
                        DesktopLegionWorkflowStatus::MergeReadinessReady
                    }
                    LegionWorkflowMergeReadinessState::WaitingForApproval => {
                        DesktopLegionWorkflowStatus::MergeReadinessWaitingForApproval
                    }
                    LegionWorkflowMergeReadinessState::Blocked => {
                        DesktopLegionWorkflowStatus::MergeReadinessBlocked
                    }
                };
                Ok(self.legion_workflow_request_outcome(
                    session_id,
                    None,
                    status,
                    format!(
                        "Legion workflow merge readiness requested: {readiness_state:?}; Autonomous merge unsupported until approval"
                    ),
                ))
            }
            DesktopAppRequest::RecordLegionWorkflowToolPermission {
                session_id,
                server_id,
                tool_name,
                decision,
            } => Ok(self.legion_workflow_request_outcome(
                session_id,
                None,
                DesktopLegionWorkflowStatus::ToolPermissionRecorded,
                format!(
                    "Legion workflow tool permission requested server={} tool={} decision={decision:?}",
                    server_id.0, tool_name.0
                ),
            )),
            DesktopAppRequest::TriggerLegionWorkflowKillSwitch {
                session_id,
                reason_label,
            } => Ok(self.legion_workflow_request_outcome(
                session_id,
                None,
                DesktopLegionWorkflowStatus::KillSwitchTriggered,
                format!("Legion workflow kill switch requested: {reason_label}"),
            )),
        }
    }

    fn legion_workflow_request_outcome(
        &mut self,
        session_id: LegionWorkflowSessionId,
        proposal_id: Option<ProposalId>,
        status: DesktopLegionWorkflowStatus,
        message: String,
    ) -> DesktopWorkflowOutcome {
        self.set_status(StatusSeverity::Info, message.clone());
        DesktopWorkflowOutcome::LegionWorkflowReviewed {
            session_id,
            proposal_id,
            status,
            message,
        }
    }

    fn handle_bridge_error(&mut self, error: DesktopBridgeError) -> DesktopWorkflowOutcome {
        let message = error.to_string();
        self.set_status(StatusSeverity::Error, message.clone());
        DesktopWorkflowOutcome::Error(message)
    }

    fn map_app_outcome(
        &mut self,
        outcome: AppCommandOutcome,
        plugin_context: Option<(PluginId, String)>,
    ) -> DesktopWorkflowOutcome {
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
            AppCommandOutcome::ProductModeChanged(mode) => {
                let dock_mode = mode.to_dock_mode();
                self.set_status(
                    StatusSeverity::Info,
                    format!("Product mode changed to {}", dock_mode.label()),
                );
                DesktopWorkflowOutcome::ProductModeChanged { mode: dock_mode }
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
                self.set_save_all_status(&outcome);
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
            AppCommandOutcome::StructuralSearchUpdated(projection) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!(
                        "Structural search: {:?} matches={} proposal={:?}",
                        projection.status.kind,
                        projection.matches.len(),
                        projection.proposal_id.map(|proposal| proposal.0)
                    ),
                );
                DesktopWorkflowOutcome::StructuralSearchUpdated
            }
            AppCommandOutcome::GitUpdated(projection) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!(
                        "Git: changes={} hunks={} conflicts={}",
                        projection.changed_files.len(),
                        projection.hunks.len(),
                        projection.conflicts.len()
                    ),
                );
                DesktopWorkflowOutcome::GitUpdated
            }
            AppCommandOutcome::DebugProjectionUpdated(projection) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!("Debug: {}", projection.status.message),
                );
                DesktopWorkflowOutcome::DebugProjectionUpdated
            }
            AppCommandOutcome::LanguageToolingUpdated(projection) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!("Language: {}", projection.status_message),
                );
                DesktopWorkflowOutcome::LanguageToolingUpdated
            }
            AppCommandOutcome::AssistInlinePredictionUpdated(projection) => {
                let active = projection.active_prediction.is_some();
                let status = format!(
                    "Assist inline prediction: active={} rows={} stale={}",
                    active,
                    projection.rows.len(),
                    projection.stale_prediction_count
                );
                self.set_status(StatusSeverity::Info, status.clone());
                DesktopWorkflowOutcome::AssistInlinePredictionUpdated {
                    active,
                    row_count: projection.rows.len(),
                    stale_count: projection.stale_prediction_count,
                    status,
                }
            }
            AppCommandOutcome::TerminalPanelUpdated(projection) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!("Terminal: {}", projection.status.message),
                );
                DesktopWorkflowOutcome::TerminalPanelUpdated
            }
            AppCommandOutcome::ProposalLifecycleUpdated(response) => {
                let transition = proposal_response_transition(&response);
                let kind = proposal_response_kind(&response);
                let severity = proposal_response_status_severity(&response);
                let status = format!(
                    "Proposal {} {kind} ({:?})",
                    transition.proposal_id.0, transition.lifecycle_state
                );
                self.set_status(severity, status.clone());
                DesktopWorkflowOutcome::ProposalLifecycleUpdated {
                    proposal_id: transition.proposal_id,
                    lifecycle_state: transition.lifecycle_state,
                    status,
                }
            }
            AppCommandOutcome::ProposalDetailsOpened(proposal_id) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!("Proposal details opened {}", proposal_id.0),
                );
                DesktopWorkflowOutcome::ProposalDetailsOpened(proposal_id)
            }
            AppCommandOutcome::ExplorerRefreshed(_) => {
                self.set_status(StatusSeverity::Info, "Explorer refreshed");
                DesktopWorkflowOutcome::ExplorerRefreshed
            }
            AppCommandOutcome::Opened(_) => {
                self.set_status(StatusSeverity::Info, "Opened");
                DesktopWorkflowOutcome::Opened
            }
            AppCommandOutcome::AiRunStarted(outcome) => self.map_ai_run_started(&outcome),
            AppCommandOutcome::AiRunCancelled(run_id) => {
                let status = format!("Assisted AI run cancelled {}", run_id.0);
                self.set_status(StatusSeverity::Warning, status.clone());
                DesktopWorkflowOutcome::AssistedAiUpdated {
                    run_id,
                    proposal_id: None,
                    status,
                }
            }
            AppCommandOutcome::AiRunReplayed(manifest) => {
                let status = format!(
                    "Assisted AI run replayed {} transitions={} proposals={}",
                    manifest.run_id.0,
                    manifest.transitions.len(),
                    manifest.proposal_ids.len()
                );
                self.set_status(StatusSeverity::Info, status.clone());
                DesktopWorkflowOutcome::AssistedAiUpdated {
                    run_id: manifest.run_id.clone(),
                    proposal_id: manifest.proposal_ids.first().copied(),
                    status,
                }
            }
            AppCommandOutcome::AiRunInspected(snapshot) => {
                let status = format!(
                    "Assisted AI run inspected {} requests={} refusals={}",
                    snapshot.run_id.0,
                    snapshot.assisted_ai_projection.request_count,
                    snapshot.assisted_ai_projection.refusal_count
                );
                self.set_status(StatusSeverity::Info, status.clone());
                DesktopWorkflowOutcome::AssistedAiUpdated {
                    run_id: snapshot.run_id.clone(),
                    proposal_id: snapshot
                        .assisted_ai_projection
                        .proposal_previews
                        .first()
                        .map(|preview| preview.proposal_id),
                    status,
                }
            }
            AppCommandOutcome::DelegateChatCompleted(outcome) => {
                let message = format!("Delegate chat sent citations={}", outcome.citation_count);
                self.set_status(StatusSeverity::Info, message.clone());
                DesktopWorkflowOutcome::DelegatedTaskReviewed {
                    plan_id: None,
                    proposal_id: None,
                    status: DesktopDelegatedTaskStatus::ChatSent,
                    message,
                }
            }
            AppCommandOutcome::DelegateProposalHunkReviewed(projection) => {
                let message = format!(
                    "Delegate proposal hunk review recorded reviews={}",
                    projection.proposal_review_count
                );
                self.set_status(StatusSeverity::Info, message.clone());
                DesktopWorkflowOutcome::DelegatedTaskReviewed {
                    plan_id: None,
                    proposal_id: projection
                        .proposal_reviews
                        .first()
                        .map(|review| review.proposal_id),
                    status: DesktopDelegatedTaskStatus::ProposalHunkReviewed,
                    message,
                }
            }
            AppCommandOutcome::DelegateToolPermissionRecorded(projection) => {
                let message = format!(
                    "Delegate tool permission recorded requests={}",
                    projection.tool_permission_request_count
                );
                self.set_status(StatusSeverity::Info, message.clone());
                DesktopWorkflowOutcome::DelegatedTaskReviewed {
                    plan_id: None,
                    proposal_id: None,
                    status: DesktopDelegatedTaskStatus::ToolPermissionRecorded,
                    message,
                }
            }
            AppCommandOutcome::LegionWorkflowUpdated(projection) => {
                let killed = projection.kill_switches.iter().any(|switch| {
                    switch.state == devil_protocol::LegionWorkflowKillSwitchState::Triggered
                });
                let session_id = projection
                    .rows
                    .first()
                    .map(|row| row.session_id.clone())
                    .or_else(|| {
                        projection
                            .kill_switches
                            .first()
                            .map(|switch| switch.session_id.clone())
                    })
                    .unwrap_or_else(|| LegionWorkflowSessionId("session:unknown".to_string()));
                let message = format!(
                    "Legion workflow updated decisions={} permissions={} risk_monitors={} kill_switches={}",
                    projection.decision_feed_count,
                    projection.tool_permission_request_count,
                    projection.risk_monitor_count,
                    projection.kill_switch_count
                );
                self.set_status(
                    if killed {
                        StatusSeverity::Warning
                    } else {
                        StatusSeverity::Info
                    },
                    message.clone(),
                );
                DesktopWorkflowOutcome::LegionWorkflowReviewed {
                    session_id,
                    proposal_id: None,
                    status: if killed {
                        DesktopLegionWorkflowStatus::KillSwitchTriggered
                    } else {
                        DesktopLegionWorkflowStatus::ToolPermissionRecorded
                    },
                    message,
                }
            }
            AppCommandOutcome::PluginCommandInvoked(response) => {
                let Some((plugin_id, command_id)) = plugin_context else {
                    self.set_status(StatusSeverity::Info, "Plugin command handled");
                    return DesktopWorkflowOutcome::Noop;
                };
                self.map_plugin_command_response(plugin_id, command_id, response.as_ref())
            }
            AppCommandOutcome::CollaborationSessionJoined(session_id) => {
                let message = format!("Collaboration session joined {}", session_id.0);
                self.set_status(StatusSeverity::Info, message.clone());
                DesktopWorkflowOutcome::CollaborationUpdated {
                    session_id: Some(session_id),
                    status: DesktopCollaborationStatus::Joined,
                    message,
                }
            }
            AppCommandOutcome::CollaborationSessionLeft(session_id) => {
                let message = format!("Collaboration session left {}", session_id.0);
                self.set_status(StatusSeverity::Info, message.clone());
                DesktopWorkflowOutcome::CollaborationUpdated {
                    session_id: Some(session_id),
                    status: DesktopCollaborationStatus::Left,
                    message,
                }
            }
            AppCommandOutcome::CollaborationPresencePublished(session_id) => {
                let message = format!("Collaboration presence published {}", session_id.0);
                self.set_status(StatusSeverity::Info, message.clone());
                DesktopWorkflowOutcome::CollaborationUpdated {
                    session_id: Some(session_id),
                    status: DesktopCollaborationStatus::PresencePublished,
                    message,
                }
            }
            AppCommandOutcome::CollaborationOperationApplied(descriptor) => {
                let session_id = match &descriptor.source {
                    devil_protocol::TransactionSource::CollaborationParticipant {
                        session_id,
                        ..
                    } => Some(*session_id),
                    _ => None,
                };
                let message = match session_id {
                    Some(session_id) => {
                        format!("Collaboration operation applied {}", session_id.0)
                    }
                    None => "Collaboration operation applied".to_string(),
                };
                self.set_status(StatusSeverity::Info, message.clone());
                DesktopWorkflowOutcome::CollaborationUpdated {
                    session_id,
                    status: DesktopCollaborationStatus::OperationApplied,
                    message,
                }
            }
        }
    }

    fn map_plugin_command_response(
        &mut self,
        plugin_id: PluginId,
        command_id: String,
        response: &PluginHostCallResponse,
    ) -> DesktopWorkflowOutcome {
        let (severity, status, message) = match response {
            PluginHostCallResponse::Accepted { metadata_label } => (
                StatusSeverity::Info,
                DesktopPluginCommandStatus::Invoked,
                format!(
                    "Plugin command invoked {} {}: {metadata_label}",
                    plugin_id.0, command_id
                ),
            ),
            PluginHostCallResponse::ProposalCreated(proposal) => (
                StatusSeverity::Info,
                DesktopPluginCommandStatus::ProposalCreated,
                format!(
                    "Plugin command created proposal {} {}: proposal {}",
                    plugin_id.0, command_id, proposal.proposal.proposal_id.0
                ),
            ),
            PluginHostCallResponse::Denied { reason, message } => {
                let status = if *reason == PluginDenialReason::UnsupportedHostCall {
                    DesktopPluginCommandStatus::NoRuntime
                } else {
                    DesktopPluginCommandStatus::Denied
                };
                (
                    StatusSeverity::Warning,
                    status,
                    format!(
                        "Plugin command denied {} {}: {:?} {message}",
                        plugin_id.0, command_id, reason
                    ),
                )
            }
        };
        self.set_status(severity, message.clone());
        DesktopWorkflowOutcome::PluginCommand {
            plugin_id,
            command_id,
            status,
            message,
        }
    }

    fn map_ai_run_started(&mut self, outcome: &AppAiRunOutcome) -> DesktopWorkflowOutcome {
        let (severity, status) = if let Some(refusal) = &outcome.refusal {
            (
                StatusSeverity::Warning,
                format!(
                    "Assisted AI run refused {}: {} {}",
                    outcome.run_id.0, refusal.reason_code, refusal.label
                ),
            )
        } else if let Some(proposal_id) = outcome.proposal_id {
            (
                StatusSeverity::Info,
                format!(
                    "Assisted AI proposal run {} created proposal {}",
                    outcome.run_id.0, proposal_id.0
                ),
            )
        } else {
            (
                StatusSeverity::Info,
                format!(
                    "Assisted AI explain run {} completed metadata-only",
                    outcome.run_id.0
                ),
            )
        };
        self.set_status(severity, status.clone());
        DesktopWorkflowOutcome::AssistedAiUpdated {
            run_id: outcome.run_id.clone(),
            proposal_id: outcome.proposal_id,
            status,
        }
    }

    fn refresh_projection(&mut self) -> Result<()> {
        let mut snapshot = self.app.shell_projection_snapshot(WINDOW_TITLE)?;
        if let Some(status) = &self.last_status {
            snapshot.status_messages.push(status.clone());
        }
        snapshot
            .status_messages
            .extend(self.last_status_details.iter().cloned());
        self.shell.replace_projection_snapshot(snapshot);
        Ok(())
    }

    fn set_status(&mut self, severity: StatusSeverity, message: impl Into<String>) {
        self.last_status = Some(status_message(severity, message));
        self.last_status_details.clear();
    }

    fn set_status_with_details(
        &mut self,
        severity: StatusSeverity,
        message: impl Into<String>,
        details: Vec<StatusMessageProjection>,
    ) {
        self.last_status = Some(status_message(severity, message));
        self.last_status_details = details;
    }

    fn set_save_all_status(&mut self, outcome: &AppSaveAllOutcome) {
        let severity = match outcome.status {
            AppSaveAllStatus::Noop | AppSaveAllStatus::Saved => StatusSeverity::Info,
            AppSaveAllStatus::Partial | AppSaveAllStatus::Rejected => StatusSeverity::Warning,
        };
        let status_label = match outcome.status {
            AppSaveAllStatus::Noop => "no-op",
            AppSaveAllStatus::Saved => "saved",
            AppSaveAllStatus::Partial => "partial",
            AppSaveAllStatus::Rejected => "rejected",
        };
        let details = outcome
            .results
            .iter()
            .map(save_all_item_status_message)
            .collect();
        self.set_status_with_details(
            severity,
            format!(
                "Save all {status_label}: {} saved, {} rejected",
                outcome.saved_count, outcome.rejected_count
            ),
            details,
        );
    }

    fn persist_session_if_configured(&mut self) {
        if let Err(error) = self.save_session_state() {
            self.set_status(
                StatusSeverity::Warning,
                format!("Session save failed: {error}"),
            );
        }
    }

    fn persist_diagnostics_if_configured(&mut self) {
        let Some(path) = &self.diagnostics_export_path else {
            return;
        };
        if let Err(error) = self.diagnostics_export().write_to_path(path) {
            self.set_status(
                StatusSeverity::Warning,
                format!("Diagnostics export failed: {error}"),
            );
        }
    }
}

fn default_panel_state() -> SessionPanelState {
    SessionPanelState {
        bottom_visible: false,
        side_visible: true,
        active_panel: None,
        bottom_height_px: None,
        side_width_px: None,
    }
}

fn restore_status_messages(
    restore: &AppSessionRestoreOutcome,
) -> (StatusMessageProjection, Vec<StatusMessageProjection>) {
    let severity = if restore.skipped_tabs.is_empty() {
        StatusSeverity::Info
    } else {
        StatusSeverity::Warning
    };
    let status = status_message(
        severity,
        format!(
            "Session restored: {} tabs, {} skipped",
            restore.restored_file_ids.len(),
            restore.skipped_tabs.len()
        ),
    );
    let details = restore
        .skipped_tabs
        .iter()
        .map(|tab| {
            status_message(
                StatusSeverity::Warning,
                format!("Session skipped tab {}: {}", tab.tab_id, tab.reason),
            )
        })
        .collect();
    (status, details)
}

fn session_workspace_matches(workspace_root: &Path, record: &WorkspaceSessionRecord) -> bool {
    let Some(session_root) = &record.last_workspace_path else {
        return true;
    };
    paths_equivalent(workspace_root, Path::new(&session_root.0))
}

fn paths_equivalent(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn save_all_item_status_message(item: &AppSaveAllItemOutcome) -> StatusMessageProjection {
    let path = item
        .file_path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<unknown>");
    match item.status {
        AppSaveAllItemStatus::Saved => status_message(
            StatusSeverity::Info,
            format!(
                "Save all item saved: buffer {} path={path} dirty={}",
                item.buffer_id.0, item.final_dirty
            ),
        ),
        AppSaveAllItemStatus::Rejected | AppSaveAllItemStatus::MetadataMissing => {
            let kind = item
                .rejection_metadata
                .as_ref()
                .map(|metadata| metadata.response_kind.as_str())
                .unwrap_or("Rejected");
            let diagnostics = item
                .rejection_metadata
                .as_ref()
                .and_then(|metadata| metadata.diagnostic_messages.first())
                .map(String::as_str)
                .unwrap_or("no diagnostic message");
            status_message(
                StatusSeverity::Warning,
                format!(
                    "Save all item rejected: buffer {} path={path} response={kind} dirty={} diagnostic={diagnostics}",
                    item.buffer_id.0, item.final_dirty
                ),
            )
        }
    }
}

fn plugin_intent_context(intent: &CommandDispatchIntent) -> Option<(PluginId, String)> {
    match intent {
        CommandDispatchIntent::InvokePluginCommand {
            plugin_id,
            command_id,
            ..
        } => Some((*plugin_id, command_id.clone())),
        _ => None,
    }
}

/// Run the desktop adapter from process arguments.
pub fn run_from_env() -> Result<()> {
    let config = DesktopLaunchConfig::from_env_args()?;
    if let Some(beta_config) = config.beta.clone() {
        beta::run_beta_workflow(beta_config).map(|_| ())
    } else if let Some(smoke_config) = config.smoke.clone() {
        smoke::run_smoke(config, smoke_config)
    } else {
        run_native(config)
    }
}

fn run_native(config: DesktopLaunchConfig) -> Result<()> {
    let native_options = desktop_native_options(WINDOW_TITLE);
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

/// Build the native desktop options shared by normal and smoke launches.
#[must_use]
pub fn desktop_native_options(title: &str) -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([960.0, 720.0]),
        ..Default::default()
    }
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
            if command
                && input.key_pressed(egui::Key::W)
                && let Some(buffer_id) = active_buffer_for_input(&snapshot)
            {
                actions.push(DesktopAction::CloseTab { buffer_id });
            }
            if command
                && input.key_pressed(egui::Key::Tab)
                && let Some(buffer_id) =
                    adjacent_tab_id(&snapshot, if input.modifiers.shift { -1 } else { 1 })
            {
                actions.push(DesktopAction::SwitchTab { buffer_id });
            }
            if command && input.key_pressed(egui::Key::O) {
                actions.push(DesktopAction::ShowOpenPathPrompt);
            }
            if command && input.modifiers.alt && input.key_pressed(egui::Key::F) {
                actions.push(DesktopAction::ShowStructuralSearchPrompt {
                    scope: if input.modifiers.shift {
                        SearchScopeProjection::Workspace
                    } else {
                        SearchScopeProjection::ActiveFile
                    },
                });
            } else if command && input.key_pressed(egui::Key::F) {
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

    fn show_structural_search_prompt(&mut self, ctx: &egui::Context) {
        if !self.runtime.structural_search_prompt {
            return;
        }

        let mut open = true;
        egui::Window::new("Structural Search")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.runtime.structural_search_scope,
                        SearchScopeProjection::ActiveFile,
                        "File",
                    );
                    ui.radio_value(
                        &mut self.runtime.structural_search_scope,
                        SearchScopeProjection::Workspace,
                        "Workspace",
                    );
                });
                ui.label("Pattern");
                ui.text_edit_singleline(&mut self.runtime.structural_search_pattern_text);
                ui.label("Rewrite");
                ui.text_edit_singleline(&mut self.runtime.structural_search_rewrite_text);
                ui.horizontal(|ui| {
                    if ui.button("Preview").clicked() {
                        let pattern = self.runtime.structural_search_pattern_text.clone();
                        let rewrite =
                            trimmed_optional_text(&self.runtime.structural_search_rewrite_text);
                        self.runtime.structural_search_prompt = false;
                        let _ = self
                            .runtime
                            .handle_action(DesktopAction::RunStructuralSearch {
                                scope: self.runtime.structural_search_scope,
                                pattern,
                                rewrite,
                                limit: 0,
                            });
                    }
                    if ui.button("Cancel").clicked() {
                        self.runtime.structural_search_prompt = false;
                    }
                });
            });

        if !open {
            self.runtime.structural_search_prompt = false;
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
        self.show_structural_search_prompt(ui.ctx());
        if self.runtime.quit_requested() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn trimmed_optional_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn status_message(severity: StatusSeverity, message: impl Into<String>) -> StatusMessageProjection {
    StatusMessageProjection {
        severity,
        message: message.into(),
    }
}

fn proposal_response_transition(response: &ProposalResponse) -> &ProposalLifecycleTransition {
    match response {
        ProposalResponse::Created(transition)
        | ProposalResponse::Validated(transition)
        | ProposalResponse::Approved(transition)
        | ProposalResponse::Applied(transition) => transition,
        ProposalResponse::Previewed { transition, .. }
        | ProposalResponse::Rejected { transition, .. }
        | ProposalResponse::Denied { transition, .. }
        | ProposalResponse::Failed { transition, .. }
        | ProposalResponse::RolledBack { transition, .. }
        | ProposalResponse::Stale { transition, .. }
        | ProposalResponse::Conflict { transition, .. }
        | ProposalResponse::Cancelled { transition, .. } => transition,
    }
}

fn proposal_response_kind(response: &ProposalResponse) -> &'static str {
    match response {
        ProposalResponse::Created(_) => "created",
        ProposalResponse::Validated(_) => "validated",
        ProposalResponse::Previewed { .. } => "previewed",
        ProposalResponse::Approved(_) => "approved",
        ProposalResponse::Rejected { .. } => "rejected",
        ProposalResponse::Applied(_) => "applied",
        ProposalResponse::Denied { .. } => "denied",
        ProposalResponse::Failed { .. } => "failed",
        ProposalResponse::RolledBack { .. } => "rolled back",
        ProposalResponse::Stale { .. } => "stale",
        ProposalResponse::Conflict { .. } => "conflict",
        ProposalResponse::Cancelled { .. } => "cancelled",
    }
}

fn proposal_response_status_severity(response: &ProposalResponse) -> StatusSeverity {
    match response {
        ProposalResponse::Failed { .. } => StatusSeverity::Error,
        ProposalResponse::Rejected { .. }
        | ProposalResponse::Denied { .. }
        | ProposalResponse::Stale { .. }
        | ProposalResponse::Conflict { .. }
        | ProposalResponse::Cancelled { .. } => StatusSeverity::Warning,
        ProposalResponse::Created(_)
        | ProposalResponse::Validated(_)
        | ProposalResponse::Previewed { .. }
        | ProposalResponse::Approved(_)
        | ProposalResponse::Applied(_)
        | ProposalResponse::RolledBack { .. } => StatusSeverity::Info,
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
    if input.key_pressed(egui::Key::Tab)
        && snapshot
            .assist_inline_prediction_projection
            .active_prediction
            .is_some()
    {
        actions.push(DesktopAction::AcceptCurrentAssistInlinePrediction);
        return actions;
    }
    if input.key_pressed(egui::Key::Escape) {
        if snapshot
            .assist_inline_prediction_projection
            .request_in_flight
        {
            actions.push(DesktopAction::CancelAssistInlinePrediction);
            return actions;
        }
        if snapshot
            .assist_inline_prediction_projection
            .active_prediction
            .is_some()
        {
            actions.push(DesktopAction::DismissCurrentAssistInlinePrediction);
            return actions;
        }
    }
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
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use devil_protocol::{
        BufferId, CanonicalPath, CapabilityId, FileId, PluginCommandDescriptor, PluginContribution,
        PluginContributionProjection,
    };
    use devil_ui::ui::{CloseDirtyPromptProjection, DailyEditingProjection};
    use devil_ui::{ActiveBufferProjection, Shell};

    use super::*;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempWorkspace {
        root: PathBuf,
    }

    impl TempWorkspace {
        fn new() -> Self {
            let temp_root = std::env::temp_dir();
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos();
            let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
            let root = temp_root.join(format!(
                "devil_desktop_workflow_plugin_management_{}_{}_{}",
                std::process::id(),
                nanos,
                id
            ));
            fs::create_dir(&root).expect("temp workspace should be created");
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let temp_root = std::env::temp_dir();
            let file_name = self.root.file_name().and_then(|name| name.to_str());
            if self.root.starts_with(&temp_root)
                && file_name.is_some_and(|name| {
                    name.starts_with("devil_desktop_workflow_plugin_management_")
                })
            {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }

    fn snapshot_with_active_buffer() -> ShellProjectionSnapshot {
        let mut snapshot = Shell::empty("Keyboard").projection_snapshot();
        snapshot.active_buffer_projection = ActiveBufferProjection {
            buffer_id: Some(BufferId(1)),
            ..ActiveBufferProjection::empty()
        };
        snapshot
    }

    fn plugin_management_projection(plugin_id: PluginId) -> PluginContributionProjection {
        PluginContributionProjection {
            plugin_id,
            contributions: vec![PluginContribution::Command(PluginCommandDescriptor {
                command_id: "phase8.run".to_string(),
                title: "Phase 8 Run".to_string(),
                required_capability: CapabilityId("plugin.command".to_string()),
            })],
            status_label: "loaded".to_string(),
        }
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

    #[test]
    fn plugin_management_workflow_reports_no_runtime_for_stale_projection() {
        let workspace = TempWorkspace::new();
        let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
            workspace.path().to_path_buf(),
            None,
        ))
        .expect("runtime should open temp workspace");
        let mut snapshot = runtime.projection_snapshot();
        snapshot
            .plugin_contribution_projections
            .push(plugin_management_projection(PluginId(77)));
        runtime.shell.replace_projection_snapshot(snapshot);

        let outcome = runtime
            .handle_action(DesktopAction::InvokePluginCommand {
                plugin_id: PluginId(77),
                command_id: "phase8.run".to_string(),
            })
            .expect("no-runtime plugin denial should become a workflow outcome");

        assert!(matches!(
            outcome,
            DesktopWorkflowOutcome::PluginCommand {
                plugin_id: PluginId(77),
                ref command_id,
                status: DesktopPluginCommandStatus::NoRuntime,
                ref message,
            } if command_id == "phase8.run"
                && message.contains("Plugin command denied")
                && message.contains("UnsupportedHostCall")
        ));
    }
}
