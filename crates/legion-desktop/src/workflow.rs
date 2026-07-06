//! Desktop runtime workflow boundary.

#[cfg(not(target_os = "windows"))]
use std::process::Command;
use std::{
    collections::BTreeSet,
    ffi::OsString,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Result, anyhow};
use legion_app::{
    AppAiRunOutcome, AppCloseTabOutcome, AppCommandOutcome, AppComposition, AppProductMode,
    AppSaveAllItemOutcome, AppSaveAllItemStatus, AppSaveAllOutcome, AppSaveAllStatus,
    AppSaveOutcome, AppSessionRestoreOutcome, DurableCheckpointSummary, LspDebounceKind,
    proposal::{ProposalHunkDispositionState, filtered_batch_proposal_for_accepted_targets},
};
use legion_protocol::{
    AgentRunId, BufferId, CanonicalPath, CollaborationOperationId, CollaborationParticipantId,
    CollaborationSessionId, CollaborationSharedProposalApproval, CollaborationTransportEnvelope,
    DelegatedTaskPlanContract, DelegatedTaskPlanId, DelegatedTaskProposalHunkDisposition,
    LegionWorkflowMergeReadinessState, LegionWorkflowSessionId, PRODUCT_NAME, PluginDenialReason,
    PluginHostCallResponse, PluginId, PluginManifest, PrincipalId, ProposalId,
    ProposalLifecycleState, ProposalLifecycleTransition, ProposalResponse, ProtocolTextRange,
    RemoteTransportEnvelope, RemoteWorkspaceSessionDescriptor, RemoteWorkspaceSessionId,
    SessionDockLayout, SessionDockSideLayout, SessionPanelState, TextCoordinate, ViewportScroll,
    WorkspaceSessionRecord, WorkspaceTrustState,
};
use legion_remote::RemoteOperationOutcome;
use legion_storage::{OsKeyringSecretStore, SecretStore, provider_secret_reference};
use legion_ui::{
    CommandDispatchIntent, DockLayout, DockMode, DockSide, DockSideLayout, PaletteMode, PanelId,
    SearchScopeProjection, SettingsProjection, Shell, ShellProjectionSnapshot,
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
    manual_perf::{
        DEFAULT_KEYPRESS_P50_BUDGET_MS, DEFAULT_KEYPRESS_P95_BUDGET_MS,
        DEFAULT_MANUAL_RENDERER_REPORT_PATH, DEFAULT_MANUAL_RENDERER_SAMPLE_COUNT,
        DEFAULT_SCROLL_P95_BUDGET_MS, ManualPerfConfig,
    },
    platform::{
        NativePlatformObservation, build_platform_adapter_checks, build_platform_smoke_snapshot,
    },
    session::DesktopSessionStore,
    smoke::{self, RendererSmokeConfig},
    theme,
    view::{
        DesktopProjectionViewState, ImeCompositionProjection, ProjectionView,
        ime_composition_state, ime_composition_state_id,
        proposal_review::DesktopCheckpointTimelineRow,
    },
};

const WINDOW_TITLE: &str = PRODUCT_NAME;
const COMMAND_PALETTE_VISIBLE_RESULT_ROWS: usize = 10;

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
    /// Optional desktop-owned Manual renderer performance configuration.
    pub manual_perf: Option<ManualPerfConfig>,
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
            manual_perf: None,
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
        let mut manual_perf_enabled = false;
        let mut workspace_root = None;
        let mut beta_workspace_root = None;
        let mut initial_file = None;
        let mut duration_ms = 1500;
        let mut evidence_path =
            PathBuf::from("plans/evidence/gui-productization/phase-2-renderer-smoke.md");
        let mut perf_report_path = PathBuf::from(DEFAULT_MANUAL_RENDERER_REPORT_PATH);
        let mut perf_report_seen = false;
        let mut perf_samples = DEFAULT_MANUAL_RENDERER_SAMPLE_COUNT;
        let mut perf_samples_seen = false;
        let mut session_state = None;
        let mut diagnostics_export = None;
        let mut positionals = Vec::new();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            let arg_text = arg.to_string_lossy();
            match arg_text.as_ref() {
                "--smoke" => smoke_enabled = true,
                "--beta-smoke" => beta_enabled = true,
                "--manual-perf" => manual_perf_enabled = true,
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
                "--perf-report" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--perf-report requires a path"))?;
                    perf_report_seen = true;
                    perf_report_path = PathBuf::from(value);
                }
                "--perf-samples" => {
                    let value = args
                        .next()
                        .ok_or_else(|| anyhow!("--perf-samples requires a positive integer"))?;
                    perf_samples_seen = true;
                    perf_samples = value.to_string_lossy().parse::<usize>().map_err(|error| {
                        anyhow!("--perf-samples requires a positive integer: {error}")
                    })?;
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
        if manual_perf_enabled && smoke_enabled {
            return Err(anyhow!("--manual-perf and --smoke cannot be combined"));
        }
        if manual_perf_enabled && beta_enabled {
            return Err(anyhow!("--manual-perf and --beta-smoke cannot be combined"));
        }
        if !manual_perf_enabled && (perf_report_seen || perf_samples_seen) {
            return Err(anyhow!(
                "--perf-report and --perf-samples require --manual-perf"
            ));
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
        let manual_perf = if manual_perf_enabled {
            Some(ManualPerfConfig::new(
                workspace_root.clone(),
                initial_file.as_ref().map(PathBuf::from),
                perf_report_path,
                perf_samples,
                DEFAULT_KEYPRESS_P50_BUDGET_MS,
                DEFAULT_KEYPRESS_P95_BUDGET_MS,
                DEFAULT_SCROLL_P95_BUDGET_MS,
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
            manual_perf,
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
    /// App-owned clipboard metadata changed without exposing copied text.
    ClipboardUpdated {
        /// Buffer whose selection was copied or cut.
        buffer_id: BufferId,
        /// UTF-8 byte length of the selected text.
        byte_len: usize,
        /// Selected line count.
        line_count: usize,
        /// Whether the action also cut text from the buffer.
        cut: bool,
    },
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
    /// Workbench settings projection changed through app authority.
    SettingsUpdated {
        /// User-visible status summary.
        status: String,
    },
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
    explorer_expansion: BTreeSet<String>,
    dismissed_toast_ids: BTreeSet<u64>,
    panel_state: SessionPanelState,
    dock_layouts: Vec<DockLayout>,
    session_state_path: Option<PathBuf>,
    diagnostics_export_path: Option<PathBuf>,
    onboarding_visible: bool,
    quit_requested: bool,
    last_status: Option<StatusMessageProjection>,
    last_status_details: Vec<StatusMessageProjection>,
    last_outcome: DesktopWorkflowOutcome,
    /// Whether the LSP completion popup is currently visible (T6).
    completion_popup_open: bool,
    /// Zero-based index of the selected item in the completion popup (T6).
    completion_selected_index: usize,
    /// Whether the LSP hover tooltip should be shown (T7).
    hover_tooltip_visible: bool,
    /// Whether a GoToDefinition response navigation is pending (T7).
    definition_navigation_queued: bool,
    /// Definition count from the last projection refresh, for detecting new arrivals (T7).
    last_definition_count: usize,
    /// Keyboard-focused row index in the Problems panel (T4).
    problems_selected_index: usize,
    /// Keyboard-focused hunk index in the proposal review surface (PKT-DIFF).
    review_hunk_selected_index: usize,
    /// Per-hunk accept/reject disposition state for the multi-file proposal review
    /// surface (PKT-DIFF).  Keyed by (ProposalId, hunk_id) as projected by the
    /// delegated-task review hunks.  Lives in the desktop runtime because it is
    /// ephemeral UI state — proposals may be re-reviewed across sessions.
    hunk_dispositions: ProposalHunkDispositionState,
    // NOTE: completion_debounce, last_completion_count, hover_debounce, last_hover_id
    // have moved to AppComposition (I1 boundary fix: timing state is app authority).
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

        // Palette usage persistence is app-composition work: the app owns the
        // storage wiring (workspace-local `.legion/` state dir); the renderer
        // edge only asks for it, keeping legion-desktop free of storage deps.
        app.enable_palette_usage_persistence(&config.workspace_root);

        let mut explorer_expansion = BTreeSet::new();
        let mut panel_state = default_panel_state();
        let mut dock_layouts = DockLayout::standard_all_modes();
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
                dock_layouts = restore_dock_layouts(record);
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
            explorer_expansion,
            dismissed_toast_ids: BTreeSet::new(),
            panel_state,
            dock_layouts,
            session_state_path: config.session_state,
            diagnostics_export_path: config.diagnostics_export,
            onboarding_visible: session_record.is_none(),
            quit_requested: false,
            last_status: Some(status),
            last_status_details: status_details,
            last_outcome: DesktopWorkflowOutcome::Noop,
            completion_popup_open: false,
            completion_selected_index: 0,
            hover_tooltip_visible: false,
            definition_navigation_queued: false,
            last_definition_count: 0,
            problems_selected_index: 0,
            review_hunk_selected_index: 0,
            hunk_dispositions: ProposalHunkDispositionState::new(),
        };
        runtime.persist_diagnostics_if_configured();
        Ok(runtime)
    }

    /// Handle a desktop action through bridge and app-owned authority.
    pub fn handle_action(&mut self, action: DesktopAction) -> Result<DesktopWorkflowOutcome> {
        match action {
            DesktopAction::DismissToast { toast_id } => {
                self.dismissed_toast_ids.insert(toast_id);
                self.refresh_projection()?;
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::DismissOnboarding => {
                self.onboarding_visible = false;
                self.refresh_projection()?;
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            // T4: problems panel keyboard navigation — handled before the bridge.
            // Refresh first so the snapshot reflects any diagnostics just injected,
            // then read the current problem count for index arithmetic.
            DesktopAction::ProblemNext => {
                self.refresh_projection()?;
                let count = self
                    .shell
                    .projection_snapshot()
                    .language_tooling_projection
                    .problems
                    .len();
                if count > 0 {
                    self.problems_selected_index = (self.problems_selected_index + 1) % count;
                }
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::ProblemPrev => {
                self.refresh_projection()?;
                let count = self
                    .shell
                    .projection_snapshot()
                    .language_tooling_projection
                    .problems
                    .len();
                if count > 0 {
                    self.problems_selected_index =
                        (self.problems_selected_index + count.saturating_sub(1)) % count;
                }
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::ProblemActivate => {
                self.refresh_projection()?;
                let outcome = self.activate_selected_problem()?;
                self.last_outcome = outcome.clone();
                self.persist_diagnostics_if_configured();
                Ok(outcome)
            }
            // PKT-DIFF: proposal review hunk keyboard navigation.
            //
            // The flat hunk count is derived from all proposal reviews in the
            // delegated-task projection.  When no reviews are present the
            // actions are no-ops so the runtime never panics on an empty list.
            DesktopAction::ReviewHunkNext => {
                self.refresh_projection()?;
                let count: usize = self
                    .shell
                    .projection_snapshot()
                    .delegated_task_projection
                    .proposal_reviews
                    .iter()
                    .map(|r| r.hunks.len())
                    .sum();
                if count > 0 {
                    self.review_hunk_selected_index = (self.review_hunk_selected_index + 1) % count;
                }
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::ReviewHunkPrev => {
                self.refresh_projection()?;
                let count: usize = self
                    .shell
                    .projection_snapshot()
                    .delegated_task_projection
                    .proposal_reviews
                    .iter()
                    .map(|r| r.hunks.len())
                    .sum();
                if count > 0 {
                    self.review_hunk_selected_index =
                        (self.review_hunk_selected_index + count.saturating_sub(1)) % count;
                }
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            // PKT-DIFF: Accept/Reject wired to ProposalHunkDispositionState.
            //
            // The flat hunk list is derived from the current projection snapshot.
            // Each hunk carries proposal_id + hunk_id so we can record a disposition
            // without re-reading from the app.  An empty projection is a no-op.
            DesktopAction::ReviewHunkAccept => {
                self.refresh_projection()?;
                let flat_hunks: Vec<(ProposalId, String)> = self
                    .shell
                    .projection_snapshot()
                    .delegated_task_projection
                    .proposal_reviews
                    .iter()
                    .flat_map(|r| r.hunks.iter().map(|h| (h.proposal_id, h.hunk_id.clone())))
                    .collect();
                if let Some((proposal_id, hunk_id)) =
                    flat_hunks.get(self.review_hunk_selected_index)
                {
                    self.hunk_dispositions.set_hunk_disposition(
                        *proposal_id,
                        hunk_id.clone(),
                        DelegatedTaskProposalHunkDisposition::Accepted,
                    );
                }
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::ReviewHunkReject => {
                self.refresh_projection()?;
                let flat_hunks: Vec<(ProposalId, String)> = self
                    .shell
                    .projection_snapshot()
                    .delegated_task_projection
                    .proposal_reviews
                    .iter()
                    .flat_map(|r| r.hunks.iter().map(|h| (h.proposal_id, h.hunk_id.clone())))
                    .collect();
                if let Some((proposal_id, hunk_id)) =
                    flat_hunks.get(self.review_hunk_selected_index)
                {
                    self.hunk_dispositions.set_hunk_disposition(
                        *proposal_id,
                        hunk_id.clone(),
                        DelegatedTaskProposalHunkDisposition::Rejected,
                    );
                }
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::ReviewAcceptAll => {
                self.refresh_projection()?;
                let flat_hunks: Vec<(ProposalId, String)> = self
                    .shell
                    .projection_snapshot()
                    .delegated_task_projection
                    .proposal_reviews
                    .iter()
                    .flat_map(|r| r.hunks.iter().map(|h| (h.proposal_id, h.hunk_id.clone())))
                    .collect();
                for (proposal_id, hunk_id) in flat_hunks {
                    self.hunk_dispositions.set_hunk_disposition(
                        proposal_id,
                        hunk_id,
                        DelegatedTaskProposalHunkDisposition::Accepted,
                    );
                }
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::ReviewRejectAll => {
                self.refresh_projection()?;
                let flat_hunks: Vec<(ProposalId, String)> = self
                    .shell
                    .projection_snapshot()
                    .delegated_task_projection
                    .proposal_reviews
                    .iter()
                    .flat_map(|r| r.hunks.iter().map(|h| (h.proposal_id, h.hunk_id.clone())))
                    .collect();
                for (proposal_id, hunk_id) in flat_hunks {
                    self.hunk_dispositions.set_hunk_disposition(
                        proposal_id,
                        hunk_id,
                        DelegatedTaskProposalHunkDisposition::Rejected,
                    );
                }
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::ReviewApply => {
                self.refresh_projection()?;
                let outcome = self.apply_accepted_review_hunks()?;
                self.last_outcome = outcome.clone();
                self.persist_diagnostics_if_configured();
                Ok(outcome)
            }
            DesktopAction::ReviewDismiss => {
                // Reset disposition state and navigation index.
                self.hunk_dispositions = ProposalHunkDispositionState::new();
                self.review_hunk_selected_index = 0;
                self.refresh_projection()?;
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            // T6: completion popup navigation — handled before the bridge.
            DesktopAction::CompletionNext => {
                let count = self
                    .shell
                    .projection_snapshot()
                    .language_tooling_projection
                    .completions
                    .len();
                if count > 0 {
                    self.completion_selected_index = (self.completion_selected_index + 1) % count;
                }
                self.refresh_projection()?;
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::CompletionPrev => {
                let count = self
                    .shell
                    .projection_snapshot()
                    .language_tooling_projection
                    .completions
                    .len();
                if count > 0 {
                    self.completion_selected_index =
                        (self.completion_selected_index + count.saturating_sub(1)) % count;
                }
                self.refresh_projection()?;
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::CompletionDismiss => {
                self.completion_popup_open = false;
                self.app.disarm_lsp_completion_debounce();
                // Pre-sync the completion count so refresh_projection doesn't
                // treat the same batch as "newly arrived" and re-open the popup.
                if let Ok(snap) = self.app.shell_projection_snapshot(WINDOW_TITLE) {
                    self.app.pre_sync_lsp_completion_count(
                        snap.language_tooling_projection.completions.len(),
                    );
                }
                self.refresh_projection()?;
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            DesktopAction::CompletionAccept => {
                let outcome = self.accept_completion()?;
                self.persist_session_if_configured();
                self.refresh_projection()?;
                self.last_outcome = outcome.clone();
                self.persist_diagnostics_if_configured();
                Ok(outcome)
            }
            // T7: hover tooltip dismiss — handled before the bridge.
            DesktopAction::HoverDismiss => {
                self.hover_tooltip_visible = false;
                self.app.disarm_lsp_hover_debounce();
                // Pre-sync last_hover_id so refresh_projection won't re-open the
                // same hover data on the very next frame (mirrors CompletionDismiss).
                if let Ok(snap) = self.app.shell_projection_snapshot(WINDOW_TITLE) {
                    self.app.pre_sync_lsp_hover_id(
                        snap.language_tooling_projection
                            .hover
                            .as_ref()
                            .map(|h| h.hover_id.clone()),
                    );
                }
                self.refresh_projection()?;
                self.last_outcome = DesktopWorkflowOutcome::Noop;
                self.persist_diagnostics_if_configured();
                Ok(DesktopWorkflowOutcome::Noop)
            }
            // T7: navigate to a projected definition location.
            DesktopAction::NavigateToDefinition { index } => {
                let outcome = self.navigate_to_definition(index)?;
                self.persist_session_if_configured();
                self.refresh_projection()?;
                self.last_outcome = outcome.clone();
                self.persist_diagnostics_if_configured();
                Ok(outcome)
            }
            // PKT-PROV: store a BYOK API key in the OS keyring.
            DesktopAction::SetProviderApiKey {
                provider_id,
                api_key,
            } => {
                let reference = provider_secret_reference(&provider_id, "api_key");
                match OsKeyringSecretStore.store(&reference, &api_key) {
                    Ok(()) => {
                        // SensitiveString::drop() zeroizes the key bytes before deallocation.
                        drop(api_key);
                        self.set_status(
                            StatusSeverity::Info,
                            format!("API key stored for provider: {provider_id}"),
                        );
                        self.refresh_projection()?;
                        self.last_outcome = DesktopWorkflowOutcome::Noop;
                        self.persist_diagnostics_if_configured();
                        Ok(DesktopWorkflowOutcome::Noop)
                    }
                    Err(err) => {
                        let message = err.to_string();
                        self.set_status(StatusSeverity::Error, message.clone());
                        self.last_outcome = DesktopWorkflowOutcome::Error(message.clone());
                        self.persist_diagnostics_if_configured();
                        Ok(DesktopWorkflowOutcome::Error(message))
                    }
                }
            }
            // PKT-PROV: delete a BYOK API key from the OS keyring.
            DesktopAction::DeleteProviderApiKey { provider_id } => {
                let reference = provider_secret_reference(&provider_id, "api_key");
                match OsKeyringSecretStore.delete(&reference) {
                    Ok(()) => {
                        self.set_status(
                            StatusSeverity::Info,
                            format!("API key deleted for provider: {provider_id}"),
                        );
                        self.refresh_projection()?;
                        self.last_outcome = DesktopWorkflowOutcome::Noop;
                        self.persist_diagnostics_if_configured();
                        Ok(DesktopWorkflowOutcome::Noop)
                    }
                    Err(err) => {
                        let message = err.to_string();
                        self.set_status(StatusSeverity::Error, message.clone());
                        self.last_outcome = DesktopWorkflowOutcome::Error(message.clone());
                        self.persist_diagnostics_if_configured();
                        Ok(DesktopWorkflowOutcome::Error(message))
                    }
                }
            }
            // PKT-CKPT: restore a durable checkpoint through app authority.
            DesktopAction::RestoreCheckpoint { checkpoint_id } => {
                match self.app.restore_checkpoint(&checkpoint_id) {
                    Ok(()) => {
                        self.set_status(
                            StatusSeverity::Info,
                            format!("Checkpoint {checkpoint_id} restored"),
                        );
                        self.persist_session_if_configured();
                        self.refresh_projection()?;
                        self.last_outcome = DesktopWorkflowOutcome::Noop;
                        self.persist_diagnostics_if_configured();
                        Ok(DesktopWorkflowOutcome::Noop)
                    }
                    Err(err) => {
                        let message = err.to_string();
                        self.set_status(StatusSeverity::Error, message.clone());
                        self.last_outcome = DesktopWorkflowOutcome::Error(message.clone());
                        self.persist_diagnostics_if_configured();
                        Ok(DesktopWorkflowOutcome::Error(message))
                    }
                }
            }
            action => {
                let snapshot = self.shell.projection_snapshot();

                // T6: dismiss popup and arm debounce on text-edit actions.
                if let Some((buffer_id, at)) = completion_debounce_info(&action, &snapshot) {
                    self.completion_popup_open = false;
                    self.app.arm_lsp_completion_debounce(buffer_id, at);
                }

                // T6: dismiss popup on tab switch/close (stale popup rule).
                // T7: dismiss hover tooltip on tab switch too.
                if matches!(
                    action,
                    DesktopAction::SwitchTab { .. } | DesktopAction::CloseTab { .. }
                ) {
                    self.completion_popup_open = false;
                    self.app.disarm_lsp_completion_debounce();
                    self.completion_selected_index = 0;
                    self.hover_tooltip_visible = false;
                    self.app.disarm_lsp_hover_debounce();
                    // Do NOT clear last_hover_id: the old id prevents the dismissed
                    // tooltip from re-appearing on the new tab until a genuinely new
                    // hover response arrives with a different id.
                }

                // T7: arm hover debounce on cursor movement (200ms settle window).
                if let Some((buffer_id, at)) = hover_debounce_info(&action, &snapshot) {
                    self.hover_tooltip_visible = false;
                    self.app.arm_lsp_hover_debounce(buffer_id, at);
                }

                // T7: flag that a definition navigation is expected on next refresh.
                if matches!(action, DesktopAction::GoToDefinition { .. }) {
                    self.definition_navigation_queued = true;
                }

                if editor_text_action_blocked_by_palette(&action, &snapshot) {
                    self.set_status(
                        StatusSeverity::Info,
                        "Command palette owns text input while open",
                    );
                    self.persist_session_if_configured();
                    self.refresh_projection()?;
                    self.last_outcome = DesktopWorkflowOutcome::Noop;
                    self.persist_diagnostics_if_configured();
                    return Ok(DesktopWorkflowOutcome::Noop);
                }
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
        }
    }

    /// Dispatch a UI-originated action, surfacing any failure as an error
    /// status (and refreshing the projection) instead of silently discarding
    /// the `Result`.
    pub fn dispatch_ui_action(&mut self, action: DesktopAction) {
        if let Err(error) = self.handle_action(action) {
            self.set_status(StatusSeverity::Error, format!("Action failed: {error}"));
            let _ = self.refresh_projection();
        }
    }

    /// Returns whether the adapter has requested shutdown.
    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    /// Return the latest shell projection snapshot.
    pub fn projection_snapshot(&self) -> ShellProjectionSnapshot {
        self.shell.projection_snapshot()
    }

    /// Return durable checkpoint summaries from the app-owned checkpoint store.
    ///
    /// Usable by callers that cannot depend on `legion-storage` directly.
    pub fn list_checkpoints(&self) -> Vec<DurableCheckpointSummary> {
        self.app.list_checkpoints()
    }

    /// Map durable checkpoint summaries to `DesktopCheckpointTimelineRow` entries for
    /// display in the checkpoint timeline panel.
    ///
    /// One row is emitted per checkpoint (not per-target), ordered newest-first.
    /// This data flow is testable from real checkpoint data without requiring a
    /// dependency on `legion-storage`.
    ///
    /// # I1 — panel wiring (PKT-CKPT)
    /// The `DesktopCheckpointTimelineRow` struct was previously only populated from
    /// the per-proposal `CheckpointRollbackProjection`.  This method provides a
    /// durable-store-backed path that surfaces all checkpoints, not just the one
    /// belonging to the currently reviewed proposal.
    pub fn list_checkpoint_timeline_rows(&self) -> Vec<DesktopCheckpointTimelineRow> {
        self.app
            .list_checkpoints()
            .into_iter()
            .map(|summary| DesktopCheckpointTimelineRow {
                target_id: summary.checkpoint_id.clone(),
                kind_label: format!(
                    "{} target(s) — proposal {}",
                    summary.target_count, summary.proposal_id.0
                ),
                checkpoint_id: summary.checkpoint_id,
                labels: vec![
                    format!("principal: {}", summary.principal.0),
                    format!("created: {}", summary.created_at.0),
                ],
                available: summary.available,
            })
            .collect()
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

    /// Ingest a collaboration transport envelope through the app-owned
    /// collaboration runtime and refresh the projection.
    /// Ingest LSP publishDiagnostics for a specific buffer and refresh the projection.
    pub fn ingest_lsp_publish_diagnostics_for_buffer(
        &mut self,
        buffer_id: BufferId,
        params: &serde_json::Value,
        disclose_ranges: bool,
        request_id: Option<legion_protocol::LspRequestId>,
    ) -> Result<()> {
        self.app.ingest_lsp_publish_diagnostics_for_buffer(
            buffer_id,
            params,
            disclose_ranges,
            request_id,
        )?;
        self.refresh_projection()?;
        Ok(())
    }

    ///
    /// This is the production ingestion seam used by loopback/fake collaboration
    /// transports: presence, conflict, and operation envelopes flow through the
    /// same `AppComposition::receive_collaboration_transport_envelope` path the
    /// real transport uses. Operations that are accepted are applied through
    /// editor authority; metadata-only payloads (presence and so on) never
    /// mutate the active buffer.
    pub fn ingest_collaboration_transport_envelope(
        &mut self,
        envelope: CollaborationTransportEnvelope,
    ) -> Result<Option<AppCommandOutcome>> {
        let outcome = self
            .app
            .receive_collaboration_transport_envelope(envelope)?;
        self.refresh_projection()?;
        Ok(outcome)
    }

    /// Register a shared collaboration proposal gate through app-owned authority
    /// and refresh the projection so the shared-proposal row is visible.
    pub fn register_shared_collaboration_proposal(
        &mut self,
        session_id: CollaborationSessionId,
        proposal_id: ProposalId,
        required_approvers: Vec<CollaborationParticipantId>,
        authorized_approvers: Vec<CollaborationParticipantId>,
        applied_operation_ids: Vec<CollaborationOperationId>,
    ) -> Result<()> {
        self.app.register_shared_collaboration_proposal(
            session_id,
            proposal_id,
            required_approvers,
            authorized_approvers,
            applied_operation_ids,
        );
        self.refresh_projection()
    }

    /// Record a shared collaboration proposal approval or denial through
    /// app-owned authority and refresh the projection.
    pub fn record_shared_collaboration_approval(
        &mut self,
        approval: CollaborationSharedProposalApproval,
    ) -> Result<()> {
        self.app.record_shared_collaboration_approval(approval)?;
        self.refresh_projection()
    }

    /// Ingest a remote transport envelope through the app-owned remote runtime
    /// and refresh the projection.
    ///
    /// This is the production ingestion seam used by loopback/fake remote
    /// backends: session-descriptor (reconnect/offline), PTY/LSP descriptor, and
    /// proposal-mediated filesystem envelopes flow through the same
    /// `AppComposition::receive_remote_transport_envelope` path the real remote
    /// transport uses. Mutations remain proposal-mediated and never touch the
    /// local workspace disk.
    pub fn ingest_remote_transport_envelope(
        &mut self,
        envelope: RemoteTransportEnvelope,
    ) -> Result<RemoteOperationOutcome> {
        let outcome = self.app.receive_remote_transport_envelope(envelope)?;
        self.refresh_projection()?;
        Ok(outcome)
    }

    /// Return projection-safe remote session descriptors owned by the app remote
    /// runtime. Callers use these as the authoritative source for round-tripping
    /// modified session-descriptor envelopes (for example reconnect/offline).
    pub fn remote_session_descriptors(&self) -> Vec<RemoteWorkspaceSessionDescriptor> {
        self.app.remote_session_projections()
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

    /// Return the adapter-local restored dock layouts.
    pub fn dock_layouts(&self) -> &[DockLayout] {
        &self.dock_layouts
    }

    /// Replace adapter-local panel state for future session captures.
    pub fn set_panel_state(&mut self, panel_state: SessionPanelState) {
        self.panel_state = panel_state;
    }

    /// Replace adapter-local dock layouts for future session captures.
    pub fn set_dock_layouts(&mut self, dock_layouts: Vec<DockLayout>) {
        self.dock_layouts = normalized_dock_layouts(dock_layouts);
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
        record.dock_layouts = session_dock_layouts_from_ui(&self.dock_layouts);
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

    /// Render one projection frame through the same persistent view/state path used by native runs.
    pub(crate) fn render_projection_once_for_perf(
        &mut self,
        context: &egui::Context,
    ) -> Result<()> {
        let snapshot = self.projection_snapshot();
        let view_state = self.projection_view_state();
        let mut rendered_output = None;
        let full_output = context.run_ui(egui::RawInput::default(), |ui| {
            rendered_output = Some(self.view.render_with_state(ui, &snapshot, &view_state));
        });
        std::hint::black_box(full_output);
        let output = rendered_output
            .ok_or_else(|| anyhow!("manual perf renderer did not produce a projection frame"))?;

        let needs_repaint = output.needs_repaint;
        for action in output.actions {
            self.handle_action(action)?;
        }
        if needs_repaint {
            context.request_repaint();
        }
        Ok(())
    }

    fn projection_view_state(&self) -> DesktopProjectionViewState {
        DesktopProjectionViewState {
            expanded_explorer_paths: self.explorer_expansion.clone(),
            selected_explorer_file: None,
            dock_layouts: self.dock_layouts.clone(),
            dismissed_toast_ids: self.dismissed_toast_ids.clone(),
            first_run_onboarding_visible: self.onboarding_visible,
            completion_popup_open: self.completion_popup_open,
            completion_selected_index: self.completion_selected_index,
            hover_tooltip_visible: self.hover_tooltip_visible,
            problems_selected_index: self.problems_selected_index,
            review_hunk_selected_index: self.review_hunk_selected_index,
            durable_checkpoint_timeline_rows: self.list_checkpoint_timeline_rows(),
        }
    }

    /// Navigate to a specific definition location by zero-based index (T7).
    ///
    /// Returns `Noop` when the index is out of range or the definition lacks
    /// path/range data.  Returns `Opened` on successful navigation.
    fn navigate_to_definition(&mut self, index: usize) -> Result<DesktopWorkflowOutcome> {
        let snapshot = self.app.shell_projection_snapshot(WINDOW_TITLE)?;
        let Some(def) = snapshot.language_tooling_projection.definitions.get(index) else {
            return Ok(DesktopWorkflowOutcome::Noop);
        };
        let (Some(path), Some(range)) = (&def.path, &def.range) else {
            return Ok(DesktopWorkflowOutcome::Noop);
        };
        self.dispatch_intent(CommandDispatchIntent::OpenPathAtPosition {
            path: path.0.clone(),
            position: range.start,
        })
    }

    /// Accept the currently selected completion item by inserting its label
    /// into the active buffer through the existing editor insert path (T6).
    ///
    /// Returns `Noop` when no completions are projected or the active buffer
    /// is unknown; returns `Edited` on successful insertion.
    fn accept_completion(&mut self) -> Result<DesktopWorkflowOutcome> {
        // Read current app state directly so we see the latest completions
        // even if the shell snapshot is one frame stale.
        let snapshot = self.app.shell_projection_snapshot(WINDOW_TITLE)?;
        let completions = &snapshot.language_tooling_projection.completions;
        let Some(completion) = completions.get(self.completion_selected_index) else {
            return Ok(DesktopWorkflowOutcome::Noop);
        };
        // M3: honor `insert_text` (LSP `insertText` field) first; fall back to label.
        // `textEdit` is not yet handled (write-side, deferred); `insertText` covers
        // snippet-free items that provide a different insertion string than the label.
        let text = completion
            .insert_text
            .as_deref()
            .unwrap_or(&completion.label)
            .to_owned();
        let Some(buffer_id) = snapshot.active_buffer_projection.buffer_id else {
            return Ok(DesktopWorkflowOutcome::Noop);
        };
        // Locate the primary cursor from the per-buffer viewport state.
        let cursor = snapshot
            .daily_editing_projection
            .viewport_states
            .iter()
            .find(|vs| vs.buffer_id == buffer_id)
            .and_then(|vs| vs.cursor)
            .unwrap_or(TextCoordinate {
                line: 0,
                character: 0,
                byte_offset: None,
                utf16_offset: None,
            });
        self.completion_popup_open = false;
        self.app.disarm_lsp_completion_debounce();
        self.completion_selected_index = 0;
        // Pre-sync count so refresh_projection doesn't re-open for same batch.
        self.app
            .pre_sync_lsp_completion_count(snapshot.language_tooling_projection.completions.len());
        self.dispatch_intent(CommandDispatchIntent::Insert {
            buffer_id,
            at: cursor,
            text,
        })
    }

    /// Navigate to the currently selected problem in the Problems panel (T4).
    ///
    /// Returns `Noop` when there are no problems or no path/range on the
    /// selected problem; returns `Edited`-equivalent on successful navigation.
    /// Apply the filtered proposal built from currently-accepted review hunks.
    ///
    /// Each proposal_review in the current projection is checked for accepted
    /// hunk dispositions.  For each proposal that has at least one accepted
    /// target, a filtered proposal is registered and applied through the
    /// standard `ApplyProposal` path.  If no hunks are accepted, returns Noop.
    ///
    /// Note: hunk-level filtering operates at target granularity because batch
    /// proposal items are atomic per-target.  A target is included in the
    /// filtered result only if its accepted-hunk delegate IDs are present in
    /// the dispositions.  True intra-target partial-apply requires apply-engine
    /// support for partial operations (deferred to PKT-APPLY).
    fn apply_accepted_review_hunks(&mut self) -> Result<DesktopWorkflowOutcome> {
        use std::collections::HashSet;

        let snapshot = self.app.shell_projection_snapshot(WINDOW_TITLE)?;
        let reviews = &snapshot.delegated_task_projection.proposal_reviews;

        for review in reviews {
            let proposal_id = review.proposal_id;
            let accepted_hunk_ids = self.hunk_dispositions.accepted_hunk_ids(proposal_id);
            if accepted_hunk_ids.is_empty() {
                continue;
            }

            let accepted_target_ids: HashSet<String> = review
                .hunks
                .iter()
                .filter(|h| accepted_hunk_ids.contains(&h.hunk_id))
                .filter_map(|h| h.target_id.clone())
                .collect();

            if accepted_target_ids.is_empty() {
                continue;
            }

            let Some(proposal) = self.app.workspace_proposal_for_id(proposal_id) else {
                continue;
            };

            let Some(filtered) =
                filtered_batch_proposal_for_accepted_targets(&proposal, &accepted_target_ids)
            else {
                continue;
            };

            let _ = self.app.register_proposal_lifecycle(&filtered);
            let outcome = self.dispatch_intent(CommandDispatchIntent::ApplyProposal {
                proposal_id: filtered.proposal_id,
            })?;

            if outcome != DesktopWorkflowOutcome::Noop {
                return Ok(outcome);
            }
        }

        Ok(DesktopWorkflowOutcome::Noop)
    }

    fn activate_selected_problem(&mut self) -> Result<DesktopWorkflowOutcome> {
        let snapshot = self.app.shell_projection_snapshot(WINDOW_TITLE)?;
        let problems = &snapshot.language_tooling_projection.problems;
        let Some(problem) = problems.get(self.problems_selected_index) else {
            return Ok(DesktopWorkflowOutcome::Noop);
        };
        let (Some(path), line) = (
            problem.path.as_ref().map(|p| p.0.clone()),
            problem.range.as_ref().map(|r| r.start.line).unwrap_or(0),
        ) else {
            return Ok(DesktopWorkflowOutcome::Noop);
        };
        self.dispatch_intent(CommandDispatchIntent::OpenPathAtPosition {
            path,
            position: legion_protocol::TextCoordinate {
                line,
                character: 0,
                byte_offset: None,
                utf16_offset: None,
            },
        })
    }

    /// Test-only accessor to inject app-level state for completion popup tests.
    ///
    /// Exposed as `pub` so integration tests (tests/*) can reach it without a
    /// feature flag.  Named with a `_for_test` suffix to signal that production
    /// code must never call this.
    pub fn app_mut_for_test(&mut self) -> &mut AppComposition {
        &mut self.app
    }

    /// Expose problems selected index for assertion in tests.
    pub fn problems_selected_index_for_test(&self) -> usize {
        self.problems_selected_index
    }

    /// Expose review hunk selected index for assertion in tests (PKT-DIFF).
    pub fn review_hunk_selected_index_for_test(&self) -> usize {
        self.review_hunk_selected_index
    }

    /// Read-only access to the hunk disposition state for tests (PKT-DIFF).
    pub fn hunk_dispositions(&self) -> &ProposalHunkDispositionState {
        &self.hunk_dispositions
    }

    /// Test-only setter for completion popup visibility.
    pub fn set_completion_popup_open_for_test(&mut self, open: bool) {
        self.completion_popup_open = open;
    }

    /// Expose popup state for assertion in tests.
    pub fn completion_popup_open_for_test(&self) -> bool {
        self.completion_popup_open
    }

    /// Expose selected index for assertion in tests.
    pub fn completion_selected_index_for_test(&self) -> usize {
        self.completion_selected_index
    }

    /// Expose hover tooltip visibility for assertion in tests.
    pub fn hover_tooltip_visible_for_test(&self) -> bool {
        self.hover_tooltip_visible
    }

    /// Set hover tooltip visibility for test setup.
    pub fn set_hover_tooltip_visible_for_test(&mut self, visible: bool) {
        self.hover_tooltip_visible = visible;
        if visible {
            // Pre-sync hover id so refresh_projection doesn't immediately
            // re-open the tooltip it just sees as "already known".
            if let Ok(snap) = self.app.shell_projection_snapshot(WINDOW_TITLE) {
                self.app.pre_sync_lsp_hover_id(
                    snap.language_tooling_projection
                        .hover
                        .as_ref()
                        .map(|h| h.hover_id.clone()),
                );
            }
        }
    }

    fn editor_input_enabled(&self, snapshot: &ShellProjectionSnapshot) -> bool {
        !snapshot.palette_projection.open && !close_dirty_prompt_active(snapshot)
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
            DesktopAppRequest::ToggleExplorerPath { path } => {
                if !self.explorer_expansion.remove(&path) {
                    self.explorer_expansion.insert(path.clone());
                }
                self.set_status(StatusSeverity::Info, format!("Explorer toggled {path}"));
                Ok(DesktopWorkflowOutcome::ExplorerPathToggled(path))
            }
            DesktopAppRequest::OpenExternalUrl { url } => {
                open_url_in_system_browser(&url)?;
                self.set_status(StatusSeverity::Info, format!("Opened {url}"));
                Ok(DesktopWorkflowOutcome::Opened)
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
                    self.workspace_root = root.clone();
                    self.explorer_expansion.clear();
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
            AppCommandOutcome::ClipboardUpdated(metadata) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!(
                        "Clipboard {} metadata: bytes={} lines={}",
                        if metadata.cut { "cut" } else { "copy" },
                        metadata.byte_len,
                        metadata.line_count
                    ),
                );
                DesktopWorkflowOutcome::ClipboardUpdated {
                    buffer_id: metadata.buffer_id,
                    byte_len: metadata.byte_len,
                    line_count: metadata.line_count,
                    cut: metadata.cut,
                }
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
            AppCommandOutcome::PaletteUpdated(projection) => {
                self.set_status(
                    StatusSeverity::Info,
                    if projection.open {
                        format!(
                            "Command palette: {} results={}",
                            projection.mode.label(),
                            projection.results.len()
                        )
                    } else {
                        "Command palette closed".to_string()
                    },
                );
                DesktopWorkflowOutcome::Noop
            }
            AppCommandOutcome::SettingsUpdated(projection) => {
                let status = settings_status_label(&projection);
                self.set_status(StatusSeverity::Info, status.clone());
                DesktopWorkflowOutcome::SettingsUpdated { status }
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
                    switch.state == legion_protocol::LegionWorkflowKillSwitchState::Triggered
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
                    legion_protocol::TransactionSource::CollaborationParticipant {
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
            AppCommandOutcome::LocalHistoryEntriesUpdated(_) => {
                // Local history entries are consumed by the shell projection directly;
                // the desktop runtime has no additional action to take here.
                DesktopWorkflowOutcome::Noop
            }
            AppCommandOutcome::WorktreeEvidenceExported(path) => {
                self.set_status(
                    StatusSeverity::Info,
                    format!("Worktree evidence exported to {path}"),
                );
                DesktopWorkflowOutcome::Noop
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
        // T6/T7: check all armed debounces (completion=50ms, hover=200ms).
        // Decision logic lives in AppComposition; desktop dispatches returned events.
        for event in self.app.tick_lsp_debounces(Instant::now()) {
            let intent = match event.kind {
                LspDebounceKind::Completion => CommandDispatchIntent::RequestCompletion {
                    buffer_id: event.buffer_id,
                    position: event.position,
                },
                LspDebounceKind::Hover => CommandDispatchIntent::RequestHover {
                    buffer_id: event.buffer_id,
                    position: event.position,
                },
            };
            // Non-fatal: LSP may be unavailable; swallow error.
            let _ = self.app.dispatch_ui_intent(intent);
        }

        // PKT-LSP-B T1 (D4): non-blocking per-frame drain; never blocks.
        self.app.drain_lsp_session();
        let mut snapshot = self.app.shell_projection_snapshot(WINDOW_TITLE)?;

        // T6: auto-open popup when new completions arrive from the LSP worker.
        let new_count = snapshot.language_tooling_projection.completions.len();
        if new_count > 0 && new_count != self.app.last_lsp_completion_count() {
            self.completion_popup_open = true;
            self.completion_selected_index = 0;
        }
        if new_count == 0 {
            // Completions were cleared (e.g. new request in-flight); close popup.
            self.completion_popup_open = false;
        }
        self.app.pre_sync_lsp_completion_count(new_count);

        // T7: auto-show hover tooltip only when a genuinely new hover_id arrives.
        // Comparing ids prevents dismissed tooltips from re-opening for the same data.
        let new_hover_id = snapshot
            .language_tooling_projection
            .hover
            .as_ref()
            .map(|h| h.hover_id.clone());
        if let Some(ref id) = new_hover_id {
            if self.app.last_lsp_hover_id() != Some(id.as_str()) {
                // Different hover data arrived → auto-show.
                self.hover_tooltip_visible = true;
                self.app.pre_sync_lsp_hover_id(Some(id.clone()));
            }
        } else {
            // No hover data → hide tooltip.
            self.hover_tooltip_visible = false;
        }

        // T7: auto-navigate to definition when a queued GoToDefinition response arrives.
        let new_def_count = snapshot.language_tooling_projection.definitions.len();
        if self.definition_navigation_queued
            && new_def_count > 0
            && new_def_count != self.last_definition_count
        {
            self.definition_navigation_queued = false;
            // Navigate to the first definition location.  Non-fatal if unavailable.
            if let Some(def) = snapshot.language_tooling_projection.definitions.first()
                && let (Some(path), Some(range)) = (&def.path, &def.range)
            {
                let _ = self
                    .app
                    .dispatch_ui_intent(CommandDispatchIntent::OpenPathAtPosition {
                        path: path.0.clone(),
                        position: range.start,
                    });
            }
        }
        self.last_definition_count = new_def_count;

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

fn restore_dock_layouts(record: &WorkspaceSessionRecord) -> Vec<DockLayout> {
    if record.dock_layouts.is_empty() {
        return DockLayout::standard_all_modes();
    }
    let mut layouts = DockLayout::standard_all_modes();
    for persisted in &record.dock_layouts {
        let Some(restored) = dock_layout_from_session(persisted) else {
            continue;
        };
        if let Some(existing) = layouts
            .iter_mut()
            .find(|layout| layout.mode == restored.mode)
        {
            *existing = restored;
        }
    }
    layouts
}

fn normalized_dock_layouts(layouts: Vec<DockLayout>) -> Vec<DockLayout> {
    let mut normalized = DockLayout::standard_all_modes();
    for layout in layouts {
        if let Some(existing) = normalized
            .iter_mut()
            .find(|candidate| candidate.mode == layout.mode)
        {
            *existing = layout;
        }
    }
    normalized
}

fn dock_layout_from_session(record: &SessionDockLayout) -> Option<DockLayout> {
    if record.schema_version == 0 {
        return None;
    }
    let mode = DockMode::parse(&record.mode)?;
    let mut layout = DockLayout::standard(mode);
    for side_record in &record.sides {
        let (side, side_layout) = dock_side_layout_from_session(side_record)?;
        match side {
            DockSide::Left => layout.left = side_layout,
            DockSide::Right => layout.right = side_layout,
            DockSide::Bottom => layout.bottom = side_layout,
        }
    }
    Some(layout)
}

fn dock_side_layout_from_session(
    record: &SessionDockSideLayout,
) -> Option<(DockSide, DockSideLayout)> {
    if record.schema_version == 0 {
        return None;
    }
    let side = DockSide::parse(&record.side)?;
    let pinned_default = PanelId::parse(&record.pinned_default_panel_id)?;
    let custom_toolkit = record
        .custom_toolkit_panel_ids
        .iter()
        .map(|id| PanelId::parse(id))
        .collect::<Option<Vec<_>>>()?;
    Some((
        side,
        DockSideLayout::new(
            pinned_default,
            custom_toolkit,
            record.splitter_fraction,
            record.collapsed,
        ),
    ))
}

fn session_dock_layouts_from_ui(layouts: &[DockLayout]) -> Vec<SessionDockLayout> {
    layouts
        .iter()
        .map(|layout| SessionDockLayout {
            mode: layout.mode.label().to_string(),
            sides: vec![
                session_dock_side_layout(DockSide::Left, &layout.left),
                session_dock_side_layout(DockSide::Right, &layout.right),
                session_dock_side_layout(DockSide::Bottom, &layout.bottom),
            ],
            schema_version: 1,
        })
        .collect()
}

fn session_dock_side_layout(side: DockSide, layout: &DockSideLayout) -> SessionDockSideLayout {
    SessionDockSideLayout {
        side: side.label().to_string(),
        pinned_default_panel_id: layout.pinned_default.as_str().to_string(),
        custom_toolkit_panel_ids: layout
            .custom_toolkit
            .iter()
            .map(|panel| panel.as_str().to_string())
            .collect(),
        splitter_fraction: layout.splitter_fraction,
        collapsed: layout.collapsed,
        schema_version: 1,
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

fn editor_text_action_blocked_by_palette(
    action: &DesktopAction,
    snapshot: &ShellProjectionSnapshot,
) -> bool {
    snapshot.palette_projection.open
        && matches!(
            action,
            DesktopAction::InsertText { .. }
                | DesktopAction::ReplaceRange { .. }
                | DesktopAction::DeleteRange { .. }
                | DesktopAction::ClipboardPaste { .. }
                | DesktopAction::ClipboardCut
                | DesktopAction::ImeCommit { .. }
                | DesktopAction::SelectAll { .. }
        )
}

/// Extract `(buffer_id, cursor)` from text-edit actions that should arm the
/// completion debounce timer (T6).  Returns `None` for non-edit actions.
///
/// M5: `DeleteRange` re-arms the debounce so backspace/delete triggers a fresh
/// completion request (the preceding token may have changed).
fn completion_debounce_info(
    action: &DesktopAction,
    snapshot: &ShellProjectionSnapshot,
) -> Option<(BufferId, TextCoordinate)> {
    let at = match action {
        DesktopAction::InsertText { at, .. }
        | DesktopAction::ClipboardPaste { at, .. }
        | DesktopAction::ImeCommit { at, .. } => *at,
        // M5: treat delete/backspace as an edit that re-arms completion.
        // Use the start of the deleted range as the new trigger position.
        DesktopAction::DeleteRange { range } => range.start,
        _ => return None,
    };
    let buffer_id = snapshot.active_buffer_projection.buffer_id?;
    Some((buffer_id, at))
}

/// Extract `(buffer_id, cursor)` from cursor-movement actions that should arm
/// the hover debounce timer (T7).  Returns `None` for non-cursor actions.
fn hover_debounce_info(
    action: &DesktopAction,
    snapshot: &ShellProjectionSnapshot,
) -> Option<(BufferId, TextCoordinate)> {
    let cursor = match action {
        DesktopAction::SetCursor {
            cursor,
            buffer_id: _,
        } => *cursor,
        _ => return None,
    };
    let buffer_id = snapshot.active_buffer_projection.buffer_id?;
    Some((buffer_id, cursor))
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
    if let Some(manual_perf_config) = config.manual_perf.clone() {
        crate::manual_perf::run_manual_perf(manual_perf_config)
    } else if let Some(beta_config) = config.beta.clone() {
        beta::run_beta_workflow(beta_config).map(|_| ())
    } else if let Some(smoke_config) = config.smoke.clone() {
        smoke::run_smoke(config, smoke_config)
    } else {
        run_native(config)
    }
}

fn run_native(config: DesktopLaunchConfig) -> Result<()> {
    // Reap sandboxes orphaned by a crashed/abandoned lane from a prior
    // process, before this process's `DesktopRuntime` (and any delegated
    // lane it may start) comes up. Deliberately not done inside
    // `DesktopRuntime::open` itself, since that constructor is also used by
    // the headless test harness and unit tests, which may run concurrently
    // against the same relative `target/delegated-tasks` path.
    reap_orphaned_delegated_task_sandboxes_at_startup();

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

/// Reaps delegated-task sandboxes orphaned by a crashed/abandoned lane from a
/// prior process, using the default `target/delegated-tasks` root. Failures
/// are logged and otherwise ignored — a reap failure must never block desktop
/// startup.
fn reap_orphaned_delegated_task_sandboxes_at_startup() {
    match AppComposition::reap_orphaned_delegated_task_sandboxes() {
        Ok(removed) if !removed.is_empty() => {
            eprintln!(
                "Reaped {} orphaned delegated-task sandbox(es):",
                removed.len()
            );
            for path in &removed {
                eprintln!("  {}", path.display());
            }
        }
        Ok(_) => {}
        Err(err) => {
            eprintln!("Warning: failed to reap orphaned delegated-task sandboxes: {err}");
        }
    }
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

/// Renderer-backed eframe app wrapping a [`DesktopRuntime`].
///
/// This is the adapter-local root widget. It is intentionally public so the
/// headless input harness in `tests/headless_input.rs` can drive the same
/// keyboard handler that production uses, without spinning up a real
/// `winit` window.
pub struct DesktopEframeApp {
    runtime: DesktopRuntime,
    /// Persistent egui context reused across frames so frame-to-frame input
    /// state (focus, modifiers, active widgets) survives between calls. A
    /// fresh context per frame would silently drop that state.
    ctx: egui::Context,
}

impl DesktopEframeApp {
    /// Build a desktop eframe app around an already-opened runtime.
    pub fn new(runtime: DesktopRuntime) -> Self {
        Self {
            runtime,
            ctx: egui::Context::default(),
        }
    }

    /// Return a clone of the current app-owned shell projection snapshot.
    ///
    /// Used by the headless input harness to assert that synthetic input
    /// flowed through to app-owned state without ever touching workspace
    /// storage directly.
    pub fn runtime_snapshot(&self) -> legion_ui::ShellProjectionSnapshot {
        self.runtime.projection_snapshot()
    }

    /// Drive a desktop action through the wrapped runtime.
    ///
    /// The eframe app owns the runtime, so headless harnesses use this seam to
    /// dispatch an action and then assert through [`Self::runtime_snapshot`]
    /// exactly as production keyboard/command handling would.
    pub fn handle_action(&mut self, action: DesktopAction) -> Result<DesktopWorkflowOutcome> {
        self.runtime.handle_action(action)
    }

    /// Drive a synthetic [`egui::RawInput`] through the same keyboard handler
    /// that production uses, then return the `egui::FullOutput` produced by
    /// the frame.
    ///
    /// This is the headless test seam for `P1.F1`: it lets a CI test push a
    /// keystroke into a real `egui::Context` and observe the resulting
    /// app-owned projection without needing a native window. Renderer output
    /// is discarded; the harness asserts through the projection snapshot.
    pub fn run_headless_input(&mut self, raw_input: egui::RawInput) -> egui::FullOutput {
        let ctx = self.ctx.clone();
        ctx.run_ui(raw_input, |ui| {
            // Editor text/shortcuts are routed by `handle_keyboard`; command
            // palette text is routed through the overlay's `TextEdit`, the same
            // path production uses. The heavy workbench view is intentionally
            // not rendered here: it is irrelevant to input routing and rendering
            // it repeatedly in a headless context is costly.
            self.handle_keyboard(ui);
            self.render_command_palette_overlay(ui.ctx());
        })
    }

    /// Headless test seam that runs a **complete** frame — `handle_keyboard`,
    /// the full workbench projection view (so `render_problems_keyboard` and
    /// similar view-level key bindings fire), and the command-palette overlay.
    ///
    /// Use this when a test needs to assert that a key event wired in
    /// `view.rs` dispatches the expected action (e.g. ArrowDown →
    /// `ProblemNext`).  For tests that only need to exercise editor shortcuts
    /// or command-palette routing, prefer the lighter `run_headless_input`.
    pub fn run_headless_full_frame(&mut self, raw_input: egui::RawInput) -> egui::FullOutput {
        let ctx = self.ctx.clone();
        ctx.run_ui(raw_input, |ui| {
            self.render_app_frame(ui);
        })
    }

    /// Return the zero-based index of the currently selected problem row.
    ///
    /// Test-only delegate that forwards to the runtime so tests that wrap a
    /// [`DesktopEframeApp`] can assert navigation state without reaching inside
    /// the runtime directly.
    pub fn problems_selected_index_for_test(&self) -> usize {
        self.runtime.problems_selected_index_for_test()
    }

    /// Return the zero-based index of the currently focused review hunk (PKT-DIFF).
    ///
    /// Test-only delegate that forwards to the runtime so tests can assert
    /// keyboard navigation state without reaching inside the runtime directly.
    pub fn review_hunk_selected_index_for_test(&self) -> usize {
        self.runtime.review_hunk_selected_index_for_test()
    }

    /// Render one full application frame: keyboard handling, the projection
    /// view, and the command-palette overlay.
    fn render_app_frame(&mut self, ui: &mut egui::Ui) {
        self.handle_keyboard(ui);
        let snapshot = self.runtime.projection_snapshot();
        let view_state = self.runtime.projection_view_state();
        let output = self
            .runtime
            .view
            .render_with_state(ui, &snapshot, &view_state);
        for action in output.actions {
            self.runtime.dispatch_ui_action(action);
        }
        if output.needs_repaint {
            ui.ctx().request_repaint();
        }
        self.render_command_palette_overlay(ui.ctx());
        if self.runtime.quit_requested() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn handle_keyboard(&mut self, ui: &egui::Ui) {
        let mut actions = Vec::new();
        let snapshot = self.runtime.projection_snapshot();
        let editor_input_enabled = self.runtime.editor_input_enabled(&snapshot);

        // Clone the input state up front and release the context lock before
        // doing anything else. `Context::input` takes the context's write lock
        // for the duration of its closure, and `editor_text_input_actions` /
        // `ime_composition_state` re-enter the context via `data_mut`/`data`.
        // Running them inside the closure would deadlock on that lock, so all
        // handling below works from the cloned snapshot instead.
        let input = ui.input(|input| input.clone());
        let command = input.modifiers.command;

        if snapshot.palette_projection.open {
            if input.key_pressed(egui::Key::Escape) {
                actions.push(DesktopAction::ClosePalette);
            }
            if input.key_pressed(egui::Key::Enter) {
                actions.push(DesktopAction::DispatchPaletteSelection);
            }
            if input.key_pressed(egui::Key::ArrowUp) {
                actions.push(DesktopAction::MovePaletteSelection { delta: -1 });
            }
            if input.key_pressed(egui::Key::ArrowDown) {
                actions.push(DesktopAction::MovePaletteSelection { delta: 1 });
            }
            if input.key_pressed(egui::Key::PageUp) {
                actions.push(DesktopAction::MovePaletteSelection { delta: -8 });
            }
            if input.key_pressed(egui::Key::PageDown) {
                actions.push(DesktopAction::MovePaletteSelection { delta: 8 });
            }
            if input.key_pressed(egui::Key::Tab) {
                actions.push(DesktopAction::CompletePaletteSelection);
            }
        } else {
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
                actions.push(DesktopAction::OpenPalette {
                    mode: PaletteMode::File,
                    query: String::new(),
                    scope: SearchScopeProjection::ActiveFile,
                });
            }
            if command && input.key_pressed(egui::Key::P) {
                actions.push(DesktopAction::OpenPalette {
                    mode: PaletteMode::File,
                    query: String::new(),
                    scope: SearchScopeProjection::ActiveFile,
                });
            }
            if command && input.modifiers.alt && input.key_pressed(egui::Key::F) {
                actions.push(DesktopAction::OpenPalette {
                    mode: PaletteMode::StructuralSearch,
                    query: "#".to_string(),
                    scope: if input.modifiers.shift {
                        SearchScopeProjection::Workspace
                    } else {
                        SearchScopeProjection::ActiveFile
                    },
                });
            } else if command && input.key_pressed(egui::Key::F) {
                actions.push(DesktopAction::OpenPalette {
                    mode: PaletteMode::Search,
                    query: "/".to_string(),
                    scope: if input.modifiers.shift {
                        SearchScopeProjection::Workspace
                    } else {
                        SearchScopeProjection::ActiveFile
                    },
                });
            }
            if command && input.modifiers.alt && input.key_pressed(egui::Key::M) {
                // Return to deterministic Manual mode from any assisted mode.
                actions.push(DesktopAction::SetProductMode {
                    mode: DockMode::Manual,
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

            // T4: Problems-panel keyboard navigation.
            //
            // Handled here in `handle_keyboard` (from a cloned `InputState`)
            // rather than in a view-layer `ctx.input(|i| {...})` overlay
            // function.  The view layer runs inside `egui::Panel` closures
            // whose internal scroll areas can consume Arrow events through
            // egui's focus-navigation mechanism before any overlay function
            // gets to read them, making the key invisible even when called
            // first in the render pass.  Cloning the `InputState` up-front
            // (as `handle_keyboard` does for all other shortcuts) sidesteps
            // that consumption window.
            //
            // Focus scope: fires only when the problems list is non-empty
            // AND the completion popup is not open (completion popup already
            // owns ArrowUp / ArrowDown / Enter through its own handler).
            {
                let view_state = self.runtime.projection_view_state();
                let problems_non_empty = !snapshot.language_tooling_projection.problems.is_empty();
                if problems_non_empty && !view_state.completion_popup_open {
                    if input.key_pressed(egui::Key::ArrowDown) {
                        actions.push(DesktopAction::ProblemNext);
                    }
                    if input.key_pressed(egui::Key::ArrowUp) {
                        actions.push(DesktopAction::ProblemPrev);
                    }
                    if input.key_pressed(egui::Key::Enter) {
                        actions.push(DesktopAction::ProblemActivate);
                    }
                }
            }

            // PKT-DIFF: Proposal review hunk keyboard navigation.
            //
            // Alt+ArrowRight / Alt+ArrowLeft navigate forward/backward through
            // the flat hunk list.  Alt+Y / Alt+X accept or reject the focused
            // hunk; Alt+Shift+Y / Alt+Shift+X accept or reject all hunks.
            // These bindings use Alt to avoid conflicting with the Problems
            // panel's plain ArrowDown/Up bindings above.
            {
                let alt = input.modifiers.alt && !command;
                if alt && input.key_pressed(egui::Key::ArrowRight) {
                    actions.push(DesktopAction::ReviewHunkNext);
                }
                if alt && input.key_pressed(egui::Key::ArrowLeft) {
                    actions.push(DesktopAction::ReviewHunkPrev);
                }
                if alt && !input.modifiers.shift && input.key_pressed(egui::Key::Y) {
                    actions.push(DesktopAction::ReviewHunkAccept);
                }
                if alt && !input.modifiers.shift && input.key_pressed(egui::Key::X) {
                    actions.push(DesktopAction::ReviewHunkReject);
                }
                if alt && input.modifiers.shift && input.key_pressed(egui::Key::Y) {
                    actions.push(DesktopAction::ReviewAcceptAll);
                }
                if alt && input.modifiers.shift && input.key_pressed(egui::Key::X) {
                    actions.push(DesktopAction::ReviewRejectAll);
                }
                // Alt+Enter applies the filtered proposal for accepted hunks.
                // Alt+Escape dismisses the review surface and resets dispositions.
                // Alt+Escape avoids conflict with the completion-popup Escape binding
                // which operates without an Alt modifier.
                if alt && !input.modifiers.shift && input.key_pressed(egui::Key::Enter) {
                    actions.push(DesktopAction::ReviewApply);
                }
                if alt && input.key_pressed(egui::Key::Escape) {
                    actions.push(DesktopAction::ReviewDismiss);
                }
            }

            // PKT-CKPT: Alt+Z restores the most-recent available durable checkpoint.
            //
            // Alt+Z is distinct from Ctrl+Z (undo) to avoid conflicting with the
            // editor undo binding declared earlier in this function.  Restore
            // requires a specific checkpoint_id, so the most-recent available
            // checkpoint is selected here from the durable store.
            {
                let alt = input.modifiers.alt && !command;
                if alt
                    && !input.modifiers.shift
                    && input.key_pressed(egui::Key::Z)
                    && let Some(ckpt) = self
                        .runtime
                        .list_checkpoints()
                        .into_iter()
                        .find(|c| c.available)
                {
                    actions.push(DesktopAction::RestoreCheckpoint {
                        checkpoint_id: ckpt.checkpoint_id.clone(),
                    });
                }
            }

            actions.extend(editor_text_input_actions(
                ui,
                &input.events,
                &snapshot,
                editor_input_enabled,
            ));
            let ime_composition_active = snapshot
                .active_buffer_projection
                .buffer_id
                .and_then(|buffer_id| ime_composition_state(ui, buffer_id))
                .is_some_and(|composition| composition.active);
            actions.extend(editor_keyboard_control_actions(
                &input,
                &snapshot,
                editor_input_enabled,
                ime_composition_active,
            ));
        }

        for action in actions {
            self.runtime.dispatch_ui_action(action);
        }
    }

    fn render_command_palette_overlay(&mut self, ctx: &egui::Context) {
        let snapshot = self.runtime.projection_snapshot();
        let palette = &snapshot.palette_projection;
        if !palette.open {
            return;
        }

        let screen = ctx.content_rect();
        let tokens = theme::tokens();
        egui::Area::new("command_palette_scrim".into())
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .interactable(false)
            .show(ctx, |ui| {
                ui.painter().rect_filled(screen, 0.0, tokens.bg.scrim);
            });

        let width = screen.width().clamp(320.0, 760.0);
        let pos = egui::pos2(screen.center().x - width / 2.0, screen.top() + 72.0);
        egui::Area::new("command_palette_overlay".into())
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(tokens.bg.overlay)
                    .stroke(egui::Stroke::new(1.0, tokens.border.strong))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.horizontal(|ui| {
                            ui.label(theme::body_strong(palette.mode.label()));
                            ui.separator();
                            ui.label(theme::muted(match palette.scope {
                                SearchScopeProjection::ActiveFile => "Active file",
                                SearchScopeProjection::Workspace => "Workspace",
                            }));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(theme::muted("Esc"));
                                },
                            );
                        });
                        ui.add_space(8.0);

                        let mut query = palette.query.clone();
                        let response = ui.add_sized(
                            [width - 28.0, 32.0],
                            egui::TextEdit::singleline(&mut query)
                                .hint_text("Files, >commands, /search, #structural search"),
                        );
                        response.request_focus();
                        if response.changed() {
                            self.runtime
                                .dispatch_ui_action(DesktopAction::UpdatePaletteQuery { query });
                        }

                        ui.add_space(10.0);
                        if palette.results.is_empty() {
                            ui.label(theme::muted("No results"));
                        } else {
                            let row_height = 34.0;
                            let visible_start = palette_visible_result_start(
                                palette.results.len(),
                                palette.selected_index,
                            );
                            for (offset, result) in palette
                                .results
                                .iter()
                                .skip(visible_start)
                                .take(COMMAND_PALETTE_VISIBLE_RESULT_ROWS)
                                .enumerate()
                            {
                                let index = visible_start + offset;
                                let selected = index == palette.selected_index;
                                let (row_rect, row_response) = ui.allocate_exact_size(
                                    egui::vec2(width - 28.0, row_height),
                                    egui::Sense::click(),
                                );
                                let row_response =
                                    row_response.on_hover_cursor(egui::CursorIcon::PointingHand);
                                if selected {
                                    ui.painter().rect_filled(
                                        row_rect,
                                        egui::CornerRadius::same(6),
                                        tokens.bg.active,
                                    );
                                }
                                let mut row_ui = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(row_rect)
                                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                                );
                                row_ui.add_space(8.0);
                                row_ui.vertical(|ui| {
                                    ui.label(theme::body_strong(&result.title));
                                    let detail = result
                                        .disabled_reason
                                        .as_deref()
                                        .or(result.detail.as_deref())
                                        .unwrap_or("");
                                    if !detail.is_empty() {
                                        ui.label(theme::muted(detail));
                                    }
                                });
                                row_ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if let Some(shortcut) = &result.shortcut_label {
                                            ui.label(theme::muted(shortcut));
                                        }
                                    },
                                );
                                if row_response.clicked() {
                                    let delta = index as i32 - palette.selected_index as i32;
                                    if delta != 0 {
                                        self.runtime.dispatch_ui_action(
                                            DesktopAction::MovePaletteSelection { delta },
                                        );
                                    }
                                    self.runtime.dispatch_ui_action(
                                        DesktopAction::DispatchPaletteSelection,
                                    );
                                }
                            }
                        }
                    });
            });
    }
}

impl eframe::App for DesktopEframeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render_app_frame(ui);
    }
}

fn palette_visible_result_start(total: usize, selected_index: usize) -> usize {
    if total <= COMMAND_PALETTE_VISIBLE_RESULT_ROWS {
        return 0;
    }

    let selected_index = selected_index.min(total.saturating_sub(1));
    selected_index
        .saturating_add(1)
        .saturating_sub(COMMAND_PALETTE_VISIBLE_RESULT_ROWS)
        .min(total - COMMAND_PALETTE_VISIBLE_RESULT_ROWS)
}

fn settings_status_label(projection: &SettingsProjection) -> String {
    let settings = projection.clone().normalized();
    format!(
        "Settings: theme={} zoom={}% font={}pt toasts={} crash_reports={}",
        settings.theme_preference.label(),
        settings.zoom_percent,
        settings.editor_font_size_pt,
        settings.toast_verbosity.label(),
        if settings.telemetry.crash_reports_enabled {
            "on"
        } else {
            "off"
        }
    )
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

/// Advance a text coordinate past `inserted`, used to keep batched insertions
/// within one frame targeting sequential positions rather than the same offset.
fn advance_text_coordinate(at: TextCoordinate, inserted: &str) -> TextCoordinate {
    let mut line = at.line;
    let mut character = at.character;
    for ch in inserted.chars() {
        match ch {
            '\n' => {
                line = line.saturating_add(1);
                character = 0;
            }
            '\r' => {}
            _ => character = character.saturating_add(1),
        }
    }
    TextCoordinate {
        line,
        character,
        byte_offset: at
            .byte_offset
            .map(|offset| offset.saturating_add(inserted.len() as u64)),
        utf16_offset: at
            .utf16_offset
            .map(|offset| offset.saturating_add(inserted.encode_utf16().count() as u64)),
    }
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
    ui: &egui::Ui,
    events: &[egui::Event],
    snapshot: &ShellProjectionSnapshot,
    editor_input_enabled: bool,
) -> Vec<DesktopAction> {
    if !editor_input_enabled {
        return Vec::new();
    }

    let Some(buffer_id) = snapshot.active_buffer_projection.buffer_id else {
        return Vec::new();
    };
    // Track a running insertion coordinate so multiple text/paste/IME-commit
    // events batched in a single frame target sequential positions instead of
    // all re-inserting at the original cursor offset.
    let mut at = projected_cursor(snapshot);
    let composition_id = ime_composition_state_id(buffer_id);
    let mut composition = ui.ctx().data_mut(|data| {
        data.get_temp::<ImeCompositionProjection>(composition_id)
            .unwrap_or_default()
    });

    let mut actions = Vec::new();
    for event in events {
        match event {
            egui::Event::Text(text) if !text.is_empty() => {
                actions.push(DesktopAction::InsertText {
                    text: text.clone(),
                    at,
                });
                at = advance_text_coordinate(at, text);
            }
            egui::Event::Paste(text) if !text.is_empty() => {
                actions.push(DesktopAction::ClipboardPaste {
                    text: text.clone(),
                    at,
                });
                at = advance_text_coordinate(at, text);
            }
            egui::Event::Copy if !composition.active && composition.preedit.is_empty() => {
                actions.push(DesktopAction::ClipboardCopy);
            }
            egui::Event::Cut if !composition.active && composition.preedit.is_empty() => {
                actions.push(DesktopAction::ClipboardCut);
            }
            egui::Event::Ime(egui::ImeEvent::Enabled) => {
                composition.active = true;
            }
            egui::Event::Ime(egui::ImeEvent::Preedit(preedit)) => {
                composition.active = !preedit.is_empty();
                composition.preedit = preedit.clone();
            }
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                if !text.is_empty() {
                    actions.push(DesktopAction::ImeCommit {
                        text: text.clone(),
                        at,
                    });
                    at = advance_text_coordinate(at, text);
                }
                composition.active = false;
                composition.preedit.clear();
            }
            egui::Event::Ime(egui::ImeEvent::Disabled) => {
                composition.active = false;
                composition.preedit.clear();
            }
            _ => {}
        }
    }

    ui.ctx().data_mut(|data| {
        if composition.active || !composition.preedit.is_empty() {
            data.insert_temp(composition_id, composition);
        } else {
            data.remove::<ImeCompositionProjection>(composition_id);
        }
    });

    actions
}

/// Test seam for exercising editor text input synthesis without a native window.
pub fn test_editor_text_input_actions(
    ui: &egui::Ui,
    events: &[egui::Event],
    snapshot: &ShellProjectionSnapshot,
    editor_input_enabled: bool,
) -> Vec<DesktopAction> {
    editor_text_input_actions(ui, events, snapshot, editor_input_enabled)
}

fn editor_keyboard_control_actions(
    input: &egui::InputState,
    snapshot: &ShellProjectionSnapshot,
    editor_input_enabled: bool,
    ime_composition_active: bool,
) -> Vec<DesktopAction> {
    // Local workaround for upstream IME issues in egui/winit:
    // - egui#248 tracks composition events and candidate positioning
    // - egui#7908 tracks composition-time key consumption bugs
    // Keep editor shortcuts out of the way while the IME is active.
    if !editor_input_enabled || ime_composition_active {
        return Vec::new();
    }

    let Some(buffer_id) = active_buffer_for_input(snapshot) else {
        return Vec::new();
    };

    if input.modifiers.command {
        if input.key_pressed(egui::Key::A) {
            return vec![DesktopAction::SelectAll {
                buffer_id: Some(buffer_id),
            }];
        }
        return Vec::new();
    }

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

/// Test seam for exercising editor keyboard-control synthesis without a native window.
pub fn test_editor_keyboard_control_actions(
    input: &egui::InputState,
    snapshot: &ShellProjectionSnapshot,
    editor_input_enabled: bool,
    ime_composition_active: bool,
) -> Vec<DesktopAction> {
    editor_keyboard_control_actions(
        input,
        snapshot,
        editor_input_enabled,
        ime_composition_active,
    )
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

fn open_url_in_system_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        // Open via ShellExecuteW rather than `cmd /C start`, so URL
        // metacharacters (`&`, `^`, `%`, ...) common in forge/GitLab MR URLs
        // are not reinterpreted by the cmd parser.
        open_url_windows(url)
    }
    #[cfg(not(target_os = "windows"))]
    {
        #[cfg(target_os = "macos")]
        let mut command = Command::new("open");
        #[cfg(target_os = "linux")]
        let mut command = Command::new("xdg-open");
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let mut command = Command::new("open");
        let status = command.arg(url).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("browser opener exited with status {status}"))
        }
    }
}

#[cfg(target_os = "windows")]
fn open_url_windows(url: &str) -> Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: *mut core::ffi::c_void,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> *mut core::ffi::c_void;
    }

    const SW_SHOWNORMAL: i32 = 1;

    let operation: Vec<u16> = OsStr::new("open").encode_wide().chain(Some(0)).collect();
    // The URL is passed as the single lpFile argument; the shell treats it as
    // one opaque string, so embedded metacharacters are not parsed.
    let file: Vec<u16> = OsStr::new(url).encode_wide().chain(Some(0)).collect();

    let result = unsafe {
        ShellExecuteW(
            core::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            core::ptr::null(),
            core::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns a value greater than 32 on success.
    if result as isize > 32 {
        Ok(())
    } else {
        Err(anyhow!(
            "ShellExecuteW failed to open URL (code {})",
            result as isize
        ))
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

    use legion_protocol::{
        BufferId, CanonicalPath, CapabilityId, FileId, PluginCommandDescriptor, PluginContribution,
        PluginContributionProjection,
    };
    use legion_ui::ui::{CloseDirtyPromptProjection, DailyEditingProjection};
    use legion_ui::{ActiveBufferProjection, Shell};

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
                "legion_desktop_workflow_plugin_management_{}_{}_{}",
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
                    name.starts_with("legion_desktop_workflow_plugin_management_")
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
            permission_review_rows: Vec::new(),
        }
    }

    #[test]
    fn prompt_active_text_input_does_not_route_to_editor() {
        let events = vec![
            egui::Event::Text("Cargo.toml".to_string()),
            egui::Event::Paste("pasted/path.rs".to_string()),
        ];

        egui::__run_test_ui(|ui| {
            assert!(
                editor_text_input_actions(ui, &events, &snapshot_with_active_buffer(), false)
                    .is_empty()
            );
        });
    }

    #[test]
    fn editor_text_input_routes_text_clipboard_and_ime_commit() {
        let events = vec![
            egui::Event::Text("x".to_string()),
            egui::Event::Paste("clip".to_string()),
            egui::Event::Copy,
            egui::Event::Cut,
            egui::Event::Ime(egui::ImeEvent::Commit("漢".to_string())),
        ];
        // F022: each text/paste/IME-commit event advances the running insertion
        // coordinate, so batched events target sequential offsets rather than all
        // re-inserting at the original cursor position.
        let at_text = TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: Some(0),
            utf16_offset: Some(0),
        };
        // After inserting "x" (1 char / 1 byte / 1 utf16).
        let at_paste = TextCoordinate {
            line: 0,
            character: 1,
            byte_offset: Some(1),
            utf16_offset: Some(1),
        };
        // After pasting "clip" (4 chars / 4 bytes / 4 utf16).
        let at_ime = TextCoordinate {
            line: 0,
            character: 5,
            byte_offset: Some(5),
            utf16_offset: Some(5),
        };

        egui::__run_test_ui(|ui| {
            assert_eq!(
                editor_text_input_actions(ui, &events, &snapshot_with_active_buffer(), true),
                vec![
                    DesktopAction::InsertText {
                        text: "x".to_string(),
                        at: at_text,
                    },
                    DesktopAction::ClipboardPaste {
                        text: "clip".to_string(),
                        at: at_paste,
                    },
                    DesktopAction::ClipboardCopy,
                    DesktopAction::ClipboardCut,
                    DesktopAction::ImeCommit {
                        text: "漢".to_string(),
                        at: at_ime,
                    },
                ]
            );
        });
    }

    #[test]
    fn editor_text_input_suppresses_copy_cut_during_ime_composition() {
        let events = vec![egui::Event::Copy, egui::Event::Cut];

        egui::__run_test_ui(|ui| {
            ui.ctx().data_mut(|data| {
                data.insert_temp(
                    ime_composition_state_id(BufferId(1)),
                    ImeCompositionProjection {
                        active: true,
                        preedit: "kana".to_string(),
                    },
                );
            });

            assert!(
                editor_text_input_actions(ui, &events, &snapshot_with_active_buffer(), true)
                    .is_empty()
            );
        });
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

        egui::__run_test_ui(|ui| {
            assert!(close_dirty_prompt_active(&snapshot));
            assert!(
                editor_text_input_actions(
                    ui,
                    &events,
                    &snapshot,
                    !close_dirty_prompt_active(&snapshot),
                )
                .is_empty()
            );
        });
    }

    fn coordinate(line: u32, character: u32) -> TextCoordinate {
        TextCoordinate {
            line,
            character,
            byte_offset: None,
            utf16_offset: Some(character as u64),
        }
    }

    fn input_state_for_key(key: egui::Key, modifiers: egui::Modifiers) -> egui::InputState {
        let mut input = egui::InputState::default();
        input.modifiers = modifiers;
        input.events = vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }];
        input
    }

    #[test]
    fn editor_keyboard_control_actions_move_cursor_and_extend_selection() {
        let mut snapshot = snapshot_with_active_buffer();
        snapshot.active_buffer_projection.viewport = Some(legion_protocol::ViewportProjection {
            workspace_id: legion_protocol::WorkspaceId(1),
            buffer_id: BufferId(1),
            file_id: Some(FileId(1)),
            snapshot_id: legion_protocol::SnapshotId(1),
            buffer_version: legion_protocol::BufferVersion(1),
            visible_range: ProtocolTextRange {
                start: coordinate(0, 0),
                end: coordinate(0, 12),
            },
            selections: vec![],
            cursor: coordinate(7, 6),
            cursors: vec![],
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: legion_protocol::ViewportDimensions {
                width_px: 800,
                height_px: 600,
            },
            line_wrapping_policy: legion_protocol::LineWrappingPolicy::Off,
            wrap_column: None,
            mode: legion_protocol::ViewportProjectionMode::default(),
            line_slices: vec![],
            line_metrics: vec![],
            decoration_spans: vec![],
            fold_ranges: vec![],
            semantic_token_overlays: vec![],
            large_file_status: None,
            schema_version: 1,
        });

        let move_left = input_state_for_key(egui::Key::ArrowLeft, egui::Modifiers::default());
        assert_eq!(
            editor_keyboard_control_actions(&move_left, &snapshot, true, false),
            vec![DesktopAction::SetCursor {
                buffer_id: Some(BufferId(1)),
                cursor: coordinate(7, 5),
            }]
        );

        let shift_left = input_state_for_key(
            egui::Key::ArrowLeft,
            egui::Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(
            editor_keyboard_control_actions(&shift_left, &snapshot, true, false),
            vec![DesktopAction::SetSelection {
                buffer_id: Some(BufferId(1)),
                range: ProtocolTextRange {
                    start: coordinate(7, 5),
                    end: coordinate(7, 6),
                },
            }]
        );
    }

    #[test]
    fn editor_keyboard_control_actions_routes_command_a_to_select_all() {
        let snapshot = snapshot_with_active_buffer();
        let command_a = input_state_for_key(
            egui::Key::A,
            egui::Modifiers {
                command: true,
                ..Default::default()
            },
        );

        assert_eq!(
            editor_keyboard_control_actions(&command_a, &snapshot, true, false),
            vec![DesktopAction::SelectAll {
                buffer_id: Some(BufferId(1)),
            }]
        );
    }

    #[test]
    fn editor_keyboard_control_actions_scrolls_with_page_keys() {
        let snapshot = snapshot_with_active_buffer();

        let page_up = input_state_for_key(egui::Key::PageUp, egui::Modifiers::default());
        assert_eq!(
            editor_keyboard_control_actions(&page_up, &snapshot, true, false),
            vec![DesktopAction::SetViewportScroll {
                buffer_id: Some(BufferId(1)),
                scroll: ViewportScroll {
                    top_line: 0,
                    left_column: 0,
                },
            }]
        );

        let page_down = input_state_for_key(egui::Key::PageDown, egui::Modifiers::default());
        assert_eq!(
            editor_keyboard_control_actions(&page_down, &snapshot, true, false),
            vec![DesktopAction::SetViewportScroll {
                buffer_id: Some(BufferId(1)),
                scroll: ViewportScroll {
                    top_line: 25,
                    left_column: 0,
                },
            }]
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
