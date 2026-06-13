//! Projection-only UI primitives for the native shell.

use legion_protocol::{
    AgentRunId, ArtifactLedgerProjection, AssistedAiProjection, BufferId, BufferVersion,
    CanonicalPath, CheckpointRollbackProjection, CollaborationGuiProjection,
    CollaborationParticipantId, CollaborationPresenceProjection, CollaborationSessionId,
    CommandRegistryProjection, ContextManifestEgressStatus, ContextManifestProjection,
    ContextManifestPurpose, ContextManifestRecord, DebugBreakpointId, DebugConfigurationId,
    DebugSessionId, DebugSessionState, DelegatedTaskProjection,
    DelegatedTaskProposalHunkDisposition, DelegatedTaskRuntimeActivationState,
    DelegatedTaskToolPermissionDecision, FileFingerprint, FileId, LanguageToolingProjection,
    LegionWorkflowConflictId, LegionWorkflowProjection, LegionWorkflowSessionId,
    LegionWorkflowSignOffId, LegionWorkflowVerificationGateId, PermissionBudgetProjection,
    PluginContributionProjection, PluginId, PrivacyInspectorProjection, ProductMode,
    ProductRuntimeSurface, ProposalApprovalChecklistProjection, ProposalCancellationReason,
    ProposalId, ProposalLedgerProjection, ProposalPrivacyLabel, ProposalRejectionReason,
    ProposalRiskLabel, ProposalRollbackReason, ProtocolTextRange, RedactionHint,
    RemoteGuiProjection, SnapshotId, SystemGraphProjection, TerminalPanelProjection,
    TerminalSessionId, TextCoordinate, TimestampMillis, Utf16Range, VerificationRunProjection,
    ViewportLineTruncationState, ViewportScroll, WorkbenchTelemetryConsent, WorkspaceId,
    product_mode_allows_runtime_surface,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Dock-panel capability contract used for mode filtering.
///
/// The UI layer intentionally aliases the shared protocol runtime-surface
/// contract instead of maintaining a parallel enum that could drift from app
/// and security policy.
pub type PanelCapability = ProductRuntimeSurface;

/// Render mode for shell projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Basic projection listing.
    Plain,
}

/// Explorer tree projection consumed by shell-style UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerProjection {
    /// Flat node list from workspace tree snapshot.
    pub nodes: Vec<ExplorerNodeProjection>,
    /// Optional selected node in the explorer.
    pub selection: Option<ExplorerSelectionProjection>,
}

/// Projected explorer node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerNodeProjection {
    /// Stable file identifier.
    pub file_id: FileId,
    /// Canonical file path.
    pub canonical_path: CanonicalPath,
    /// Display name for UI list/tree rows.
    pub name: String,
    /// Child identifiers for directory rows.
    pub children: Vec<FileId>,
}

/// Projected explorer selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerSelectionProjection {
    /// Selected file identifier.
    pub file_id: FileId,
}

/// Minimal layout model used by the shell projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Window title for the shell.
    pub title: String,
    /// Width of the frame.
    pub width: u16,
    /// Height of the frame.
    pub height: u16,
}

impl Layout {
    /// Construct a layout.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: 80,
            height: 24,
        }
    }
}

/// Top-level layout projection consumed by the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellLayoutProjection {
    /// Window layout.
    pub layout: Layout,
    /// Current render mode.
    pub mode: RenderMode,
}

impl ShellLayoutProjection {
    /// Construct a plain layout projection.
    pub fn plain(title: impl Into<String>) -> Self {
        Self {
            layout: Layout::new(title),
            mode: RenderMode::Plain,
        }
    }
}

/// Product mode used by dock registry filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DockMode {
    /// Manual deterministic mode. AI-backed panels are not constructible here.
    Manual,
    /// Assist mode exposes inline/model-assisted panels without delegation.
    Assist,
    /// Delegate mode exposes chat, approval, and bounded delegated-task panels.
    Delegate,
    /// Automate mode exposes workflow/fleet panels.
    Automate,
}

impl DockMode {
    /// Stable user-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Assist => "Assist",
            Self::Delegate => "Delegate",
            Self::Automate => "Automate",
        }
    }

    /// Convert to the shared protocol product mode.
    pub fn to_product_mode(self) -> ProductMode {
        match self {
            Self::Manual => ProductMode::Manual,
            Self::Assist => ProductMode::Assist,
            Self::Delegate => ProductMode::Delegates,
            Self::Automate => ProductMode::Automate,
        }
    }

    /// Parse a stable user-facing or persisted mode label.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Manual" | "manual" => Some(Self::Manual),
            "Assist" | "assist" => Some(Self::Assist),
            "Delegate" | "Delegates" | "delegate" | "delegates" => Some(Self::Delegate),
            "Automate" | "LegionWorkflows" | "Legion Workflows" | "automate"
            | "legion_workflows" => Some(Self::Automate),
            _ => None,
        }
    }
}

/// Stable dock side identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DockSide {
    /// Left dock.
    Left,
    /// Right dock.
    Right,
    /// Bottom dock.
    Bottom,
}

impl DockSide {
    /// Stable user-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Bottom => "Bottom",
        }
    }

    /// Parse a stable user-facing or persisted side label.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Left" | "left" => Some(Self::Left),
            "Right" | "right" => Some(Self::Right),
            "Bottom" | "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }
}

/// Stable panel identifier used by shared dock registry and persisted layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PanelId {
    /// Workspace file explorer.
    ProjectExplorer,
    /// Symbol outline.
    SymbolOutline,
    /// Search results.
    Search,
    /// Diagnostics/problems.
    Diagnostics,
    /// Quick-fix/code action projection.
    QuickFixes,
    /// References/definitions results.
    References,
    /// Structural search and replace toolkit.
    StructuralSearch,
    /// Git status/history/diff projection.
    Git,
    /// Debugger projection.
    Debug,
    /// Test explorer.
    TestExplorer,
    /// Coverage projection.
    Coverage,
    /// Dependency/security inspector.
    DependencyInspector,
    /// REPL/scratchpad terminal.
    Repl,
    /// Terminal panel.
    Terminal,
    /// Manual trust/context inspector.
    Context,
    /// Inline assistant panel.
    Assistant,
    /// Delegated task panel.
    Delegation,
    /// Approval queue panel.
    ApprovalQueue,
    /// Automate/fleet console.
    AgentFleet,
    /// Agent decision feed.
    DecisionFeed,
    /// Agent log stream.
    AgentLogs,
    /// Legion workflow command center.
    Workflow,
    /// Plugin contribution manager.
    PluginManager,
    /// Collaboration panel.
    Collaboration,
    /// Remote workspace panel.
    RemoteWorkspace,
    /// Workbench preferences and editor settings.
    Settings,
}

impl PanelId {
    /// Stable lowercase identifier used in persisted layout state.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProjectExplorer => "project_explorer",
            Self::SymbolOutline => "symbol_outline",
            Self::Search => "search",
            Self::Diagnostics => "diagnostics",
            Self::QuickFixes => "quick_fixes",
            Self::References => "references",
            Self::StructuralSearch => "structural_search",
            Self::Git => "git",
            Self::Debug => "debug",
            Self::TestExplorer => "test_explorer",
            Self::Coverage => "coverage",
            Self::DependencyInspector => "dependency_inspector",
            Self::Repl => "repl",
            Self::Terminal => "terminal",
            Self::Context => "context",
            Self::Assistant => "assistant",
            Self::Delegation => "delegation",
            Self::ApprovalQueue => "approval_queue",
            Self::AgentFleet => "agent_fleet",
            Self::DecisionFeed => "decision_feed",
            Self::AgentLogs => "agent_logs",
            Self::Workflow => "workflow",
            Self::PluginManager => "plugin_manager",
            Self::Collaboration => "collaboration",
            Self::RemoteWorkspace => "remote_workspace",
            Self::Settings => "settings",
        }
    }

    /// Parse a persisted panel identifier.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "project_explorer" => Some(Self::ProjectExplorer),
            "symbol_outline" => Some(Self::SymbolOutline),
            "search" => Some(Self::Search),
            "diagnostics" => Some(Self::Diagnostics),
            "quick_fixes" => Some(Self::QuickFixes),
            "references" => Some(Self::References),
            "structural_search" => Some(Self::StructuralSearch),
            "git" => Some(Self::Git),
            "debug" => Some(Self::Debug),
            "test_explorer" => Some(Self::TestExplorer),
            "coverage" => Some(Self::Coverage),
            "dependency_inspector" => Some(Self::DependencyInspector),
            "repl" => Some(Self::Repl),
            "terminal" => Some(Self::Terminal),
            "context" => Some(Self::Context),
            "assistant" => Some(Self::Assistant),
            "delegation" => Some(Self::Delegation),
            "approval_queue" => Some(Self::ApprovalQueue),
            "agent_fleet" => Some(Self::AgentFleet),
            "decision_feed" => Some(Self::DecisionFeed),
            "agent_logs" => Some(Self::AgentLogs),
            "workflow" => Some(Self::Workflow),
            "plugin_manager" => Some(Self::PluginManager),
            "collaboration" => Some(Self::Collaboration),
            "remote_workspace" => Some(Self::RemoteWorkspace),
            "settings" => Some(Self::Settings),
            _ => None,
        }
    }
}

/// Registered dock panel metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockPanelDescriptor {
    /// Stable panel id.
    pub id: PanelId,
    /// Display title.
    pub title: String,
    /// Short icon label for renderers that do not have an icon set.
    pub icon: String,
    /// Default dock side.
    pub default_dock: DockSide,
    /// Runtime surfaces required to construct or render this panel.
    pub capabilities: Vec<PanelCapability>,
    /// Compatibility summary derived from capabilities for older render rows.
    pub requires_ai: bool,
}

impl DockPanelDescriptor {
    /// Construct a panel descriptor.
    pub fn new(
        id: PanelId,
        title: impl Into<String>,
        icon: impl Into<String>,
        default_dock: DockSide,
        requires_ai: bool,
    ) -> Self {
        let capabilities = if requires_ai {
            vec![PanelCapability::AssistedAi]
        } else {
            vec![PanelCapability::ManualIde]
        };
        Self::with_capabilities(id, title, icon, default_dock, capabilities)
    }

    /// Construct a panel descriptor with explicit runtime-surface capabilities.
    pub fn with_capabilities(
        id: PanelId,
        title: impl Into<String>,
        icon: impl Into<String>,
        default_dock: DockSide,
        capabilities: impl Into<Vec<PanelCapability>>,
    ) -> Self {
        let mut capabilities = capabilities.into();
        if capabilities.is_empty() {
            capabilities.push(PanelCapability::ManualIde);
        }
        let requires_ai = capabilities.iter().any(|capability| {
            !matches!(
                capability,
                PanelCapability::ManualIde | PanelCapability::PluginManagement
            )
        });
        Self {
            id,
            title: title.into(),
            icon: icon.into(),
            default_dock,
            capabilities,
            requires_ai,
        }
    }

    /// Whether this panel is constructible in the requested product mode.
    pub fn is_visible_in_mode(&self, mode: DockMode) -> bool {
        self.capabilities.iter().all(|capability| {
            product_mode_allows_runtime_surface(mode.to_product_mode(), *capability)
        })
    }
}

/// Errors returned when persisted dock-panel state cannot be restored.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockPanelStateError {
    /// Persisted state is malformed or belongs to another panel.
    #[error("invalid dock panel state: {message}")]
    InvalidState {
        /// Display-safe validation reason.
        message: String,
    },
}

/// Projection-safe dock panel contract.
///
/// The UI crate owns panel identity, default placement, AI filtering, and
/// persistence metadata. Renderer-specific drawing stays in adapter crates such
/// as `legion-desktop` so `legion-ui` remains projection-only and egui-free.
pub trait DockPanel {
    /// Stable panel id.
    fn id(&self) -> PanelId;

    /// Display title.
    fn title(&self) -> &str;

    /// Short icon label for renderers that do not have an icon set.
    fn icon(&self) -> &str;

    /// Default dock side.
    fn default_dock(&self) -> DockSide;

    /// Compatibility summary derived from capabilities for older render rows.
    fn requires_ai(&self) -> bool;

    /// Runtime surfaces required by this panel.
    fn capabilities(&self) -> Vec<PanelCapability> {
        if self.requires_ai() {
            vec![PanelCapability::AssistedAi]
        } else {
            vec![PanelCapability::ManualIde]
        }
    }

    /// Return this panel as a registry descriptor.
    fn descriptor(&self) -> DockPanelDescriptor {
        DockPanelDescriptor::with_capabilities(
            self.id(),
            self.title(),
            self.icon(),
            self.default_dock(),
            self.capabilities(),
        )
    }

    /// Serialize panel-owned projection state.
    fn persist_state(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id().as_str(),
            "schema_version": 1,
        })
    }

    /// Restore panel-owned projection state.
    fn restore_state(&mut self, value: serde_json::Value) -> Result<(), DockPanelStateError> {
        let state = value
            .as_object()
            .ok_or_else(|| DockPanelStateError::InvalidState {
                message: "state must be an object".to_string(),
            })?;
        let schema_version = state
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| DockPanelStateError::InvalidState {
                message: "schema_version is required".to_string(),
            })?;
        if schema_version != 1 {
            return Err(DockPanelStateError::InvalidState {
                message: format!("unsupported schema_version {schema_version}"),
            });
        }
        let state_id = state
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| DockPanelStateError::InvalidState {
                message: "id is required".to_string(),
            })?;
        if state_id != self.id().as_str() {
            return Err(DockPanelStateError::InvalidState {
                message: format!(
                    "state id `{state_id}` does not match panel `{}`",
                    self.id().as_str()
                ),
            });
        }
        Ok(())
    }
}

impl DockPanel for DockPanelDescriptor {
    fn id(&self) -> PanelId {
        self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn icon(&self) -> &str {
        &self.icon
    }

    fn default_dock(&self) -> DockSide {
        self.default_dock
    }

    fn requires_ai(&self) -> bool {
        self.requires_ai
    }

    fn capabilities(&self) -> Vec<PanelCapability> {
        self.capabilities.clone()
    }
}

/// Shared panel registry filtered by product mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelRegistry {
    panels: Vec<DockPanelDescriptor>,
}

impl PanelRegistry {
    /// Construct the standard dock panel registry.
    pub fn standard() -> Self {
        use DockSide::{Bottom, Left, Right};
        use PanelId::{
            AgentFleet, AgentLogs, ApprovalQueue, Assistant, Collaboration, Context, Coverage,
            Debug, DecisionFeed, Delegation, DependencyInspector, Diagnostics, Git, PluginManager,
            ProjectExplorer, QuickFixes, References, RemoteWorkspace, Repl, Search, Settings,
            StructuralSearch, SymbolOutline, Terminal, TestExplorer, Workflow,
        };
        use ProductRuntimeSurface::{
            AssistedAi, Automation, CloudProvider, Collaboration as CollaborationSurface,
            DelegatedTask, NetworkEgress, PluginManagement, RemoteWorkspace as RemoteSurface,
            WorkerRuntime,
        };

        Self {
            panels: vec![
                DockPanelDescriptor::new(ProjectExplorer, "Project", "files", Left, false),
                DockPanelDescriptor::new(SymbolOutline, "Outline", "outline", Left, false),
                DockPanelDescriptor::new(Search, "Search", "search", Bottom, false),
                DockPanelDescriptor::new(Diagnostics, "Problems", "alert", Bottom, false),
                DockPanelDescriptor::new(QuickFixes, "Quick Fixes", "lightbulb", Bottom, false),
                DockPanelDescriptor::new(References, "References", "target", Bottom, false),
                DockPanelDescriptor::new(
                    StructuralSearch,
                    "Structural Search",
                    "tree-search",
                    Right,
                    false,
                ),
                DockPanelDescriptor::new(Git, "Git", "branch", Left, false),
                DockPanelDescriptor::new(Debug, "Debug", "bug", Right, false),
                DockPanelDescriptor::new(TestExplorer, "Tests", "test", Left, false),
                DockPanelDescriptor::new(Coverage, "Coverage", "coverage", Right, false),
                DockPanelDescriptor::new(
                    DependencyInspector,
                    "Dependencies",
                    "shield",
                    Right,
                    false,
                ),
                DockPanelDescriptor::new(Repl, "Scratchpad", "repl", Bottom, false),
                DockPanelDescriptor::new(Terminal, "Terminal", "terminal", Bottom, false),
                DockPanelDescriptor::new(Context, "Context", "context", Right, false),
                DockPanelDescriptor::new(Settings, "Settings", "settings", Right, false),
                DockPanelDescriptor::with_capabilities(
                    PluginManager,
                    "Plugins",
                    "plug",
                    Right,
                    [PluginManagement],
                ),
                DockPanelDescriptor::with_capabilities(
                    Collaboration,
                    "Collaboration",
                    "users",
                    Right,
                    [CollaborationSurface, NetworkEgress],
                ),
                DockPanelDescriptor::with_capabilities(
                    RemoteWorkspace,
                    "Remote",
                    "cloud",
                    Right,
                    [RemoteSurface, NetworkEgress, CloudProvider],
                ),
                DockPanelDescriptor::with_capabilities(
                    Assistant,
                    "Assistant",
                    "spark",
                    Right,
                    [AssistedAi],
                ),
                DockPanelDescriptor::with_capabilities(
                    Delegation,
                    "Delegation",
                    "delegate",
                    Right,
                    [AssistedAi, DelegatedTask],
                ),
                DockPanelDescriptor::with_capabilities(
                    ApprovalQueue,
                    "Approval Queue",
                    "checklist",
                    Right,
                    [DelegatedTask],
                ),
                DockPanelDescriptor::with_capabilities(
                    AgentFleet,
                    "Agent Fleet",
                    "fleet",
                    Right,
                    [Automation, WorkerRuntime],
                ),
                DockPanelDescriptor::with_capabilities(
                    DecisionFeed,
                    "Decision Feed",
                    "feed",
                    Right,
                    [Automation],
                ),
                DockPanelDescriptor::with_capabilities(
                    AgentLogs,
                    "Agent Logs",
                    "logs",
                    Bottom,
                    [Automation, WorkerRuntime],
                ),
                DockPanelDescriptor::with_capabilities(
                    Workflow,
                    "Workflow",
                    "workflow",
                    Bottom,
                    [Automation, WorkerRuntime],
                ),
            ],
        }
    }

    /// Construct a registry from panel descriptors.
    pub fn from_panel_descriptors(panels: impl IntoIterator<Item = DockPanelDescriptor>) -> Self {
        Self {
            panels: panels.into_iter().collect(),
        }
    }

    /// Construct a registry from projection-safe panel contracts.
    pub fn from_dock_panels<'a>(panels: impl IntoIterator<Item = &'a dyn DockPanel>) -> Self {
        Self {
            panels: panels.into_iter().map(DockPanel::descriptor).collect(),
        }
    }

    /// Returns all registered panels.
    pub fn panels(&self) -> &[DockPanelDescriptor] {
        &self.panels
    }

    /// Look up a panel by id.
    pub fn panel(&self, id: PanelId) -> Option<&DockPanelDescriptor> {
        self.panels.iter().find(|panel| panel.id == id)
    }

    /// Return panels constructible in the requested mode.
    pub fn visible_for(&self, mode: DockMode) -> Vec<&DockPanelDescriptor> {
        self.panels
            .iter()
            .filter(|panel| panel.is_visible_in_mode(mode))
            .collect()
    }

    /// Whether a panel can be constructed in the requested mode.
    pub fn is_visible_in(&self, id: PanelId, mode: DockMode) -> bool {
        self.panel(id)
            .is_some_and(|panel| panel.is_visible_in_mode(mode))
    }
}

impl Default for PanelRegistry {
    fn default() -> Self {
        Self::standard()
    }
}

/// Persisted layout state for one dock side in one product mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockSideLayout {
    /// Pinned primary panel for the side.
    pub pinned_default: PanelId,
    /// Additional toolkit panels rendered below/alongside the pinned panel.
    pub custom_toolkit: Vec<PanelId>,
    /// Splitter fraction in the inclusive range `[0.15, 0.85]`.
    pub splitter_fraction: f32,
    /// Whether this side is collapsed.
    pub collapsed: bool,
}

impl DockSideLayout {
    /// Construct a side layout and normalize the splitter fraction.
    pub fn new(
        pinned_default: PanelId,
        custom_toolkit: Vec<PanelId>,
        splitter_fraction: f32,
        collapsed: bool,
    ) -> Self {
        Self {
            pinned_default,
            custom_toolkit,
            splitter_fraction: splitter_fraction.clamp(0.15, 0.85),
            collapsed,
        }
    }

    /// Panel ids for this side, with the pinned panel first.
    pub fn panel_ids(&self) -> impl Iterator<Item = PanelId> + '_ {
        std::iter::once(self.pinned_default).chain(self.custom_toolkit.iter().copied())
    }
}

/// Mode-scoped dock layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockLayout {
    /// Product mode the layout belongs to.
    pub mode: DockMode,
    /// Left dock state.
    pub left: DockSideLayout,
    /// Right dock state.
    pub right: DockSideLayout,
    /// Bottom dock state.
    pub bottom: DockSideLayout,
}

impl DockLayout {
    /// Construct the standard layout for a mode.
    pub fn standard(mode: DockMode) -> Self {
        use PanelId::{
            AgentFleet, AgentLogs, ApprovalQueue, Assistant, Context, DecisionFeed, Delegation,
            DependencyInspector, Diagnostics, PluginManager, ProjectExplorer, Search, Settings,
            StructuralSearch, SymbolOutline, Terminal, TestExplorer, Workflow,
        };

        match mode {
            DockMode::Manual => Self {
                mode,
                left: DockSideLayout::new(
                    ProjectExplorer,
                    vec![SymbolOutline, TestExplorer],
                    0.32,
                    false,
                ),
                right: DockSideLayout::new(
                    Context,
                    vec![
                        Search,
                        Diagnostics,
                        StructuralSearch,
                        DependencyInspector,
                        Settings,
                        PluginManager,
                    ],
                    0.42,
                    false,
                ),
                bottom: DockSideLayout::new(Terminal, vec![Diagnostics], 0.28, false),
            },
            DockMode::Assist => Self {
                mode,
                left: DockSideLayout::new(ProjectExplorer, vec![SymbolOutline], 0.30, false),
                right: DockSideLayout::new(Assistant, vec![Context, Search, Settings], 0.48, false),
                bottom: DockSideLayout::new(Terminal, vec![Diagnostics], 0.30, false),
            },
            DockMode::Delegate => Self {
                mode,
                left: DockSideLayout::new(ProjectExplorer, vec![SymbolOutline], 0.30, false),
                right: DockSideLayout::new(
                    Delegation,
                    vec![ApprovalQueue, Context, Settings],
                    0.52,
                    false,
                ),
                bottom: DockSideLayout::new(Terminal, vec![AgentLogs, Diagnostics], 0.34, false),
            },
            DockMode::Automate => Self {
                mode,
                left: DockSideLayout::new(ProjectExplorer, vec![AgentFleet], 0.28, false),
                right: DockSideLayout::new(
                    AgentFleet,
                    vec![DecisionFeed, ApprovalQueue, Settings],
                    0.55,
                    false,
                ),
                bottom: DockSideLayout::new(Workflow, vec![AgentLogs, Terminal], 0.38, false),
            },
        }
    }

    /// Construct layouts for all modes.
    pub fn standard_all_modes() -> Vec<Self> {
        vec![
            Self::standard(DockMode::Manual),
            Self::standard(DockMode::Assist),
            Self::standard(DockMode::Delegate),
            Self::standard(DockMode::Automate),
        ]
    }

    /// Return the side layout.
    pub fn side(&self, side: DockSide) -> &DockSideLayout {
        match side {
            DockSide::Left => &self.left,
            DockSide::Right => &self.right,
            DockSide::Bottom => &self.bottom,
        }
    }

    /// Return panel ids visible in this layout for the given registry.
    pub fn visible_panel_ids(&self, side: DockSide, registry: &PanelRegistry) -> Vec<PanelId> {
        self.side(side)
            .panel_ids()
            .filter(|id| registry.is_visible_in(*id, self.mode))
            .collect()
    }
}

/// Active editor-buffer projection received by the UI from application state.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveBufferProjection {
    /// Owning workspace identifier if a workspace is open.
    pub workspace_id: Option<WorkspaceId>,
    /// Active editor buffer identifier.
    pub buffer_id: Option<BufferId>,
    /// Active workspace file identifier.
    pub file_id: Option<FileId>,
    /// Canonical path for display only.
    pub file_path: Option<CanonicalPath>,
    /// Bounded viewport projection instead of unbounded text.
    pub viewport: Option<legion_protocol::ViewportProjection>,
    /// Degraded status from the application layer.
    pub degraded: bool,
    /// Bounded small-buffer preview, requested explicitly.
    pub small_buffer_preview: Option<String>,
    /// Dirty indicator projected from the editor engine.
    pub dirty: bool,
}

impl ActiveBufferProjection {
    /// Construct an empty active-buffer projection.
    pub fn empty() -> Self {
        Self {
            workspace_id: None,
            buffer_id: None,
            file_id: None,
            file_path: None,
            viewport: None,
            degraded: false,
            small_buffer_preview: None,
            dirty: false,
        }
    }

    /// Return a bounded small-buffer preview if available.
    pub fn small_buffer_text(&self) -> Option<&str> {
        self.small_buffer_preview.as_deref()
    }
}

impl Default for ActiveBufferProjection {
    fn default() -> Self {
        Self::empty()
    }
}

/// Status for a projected inline Assist prediction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistInlinePredictionStatusProjection {
    /// No prediction is currently available.
    Idle,
    /// A prediction request was issued and is pending.
    Requested,
    /// A provider is streaming or incrementally preparing the prediction.
    Streaming,
    /// A prediction is ready to display as ghost text.
    Ready,
    /// The prediction no longer matches the projected buffer metadata.
    Stale,
    /// The prediction was accepted through app/editor authority.
    Accepted,
    /// The prediction was dismissed locally or by app authority.
    Dismissed,
    /// The prediction request was cancelled.
    Cancelled,
    /// The prediction request failed without producing ghost text.
    Failed,
}

/// One display-only inline Assist prediction row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistInlinePredictionRowProjection {
    /// Projection-local prediction identifier supplied by the app layer.
    pub prediction_id: String,
    /// Workspace that produced the prediction, when available.
    pub workspace_id: Option<WorkspaceId>,
    /// Buffer that produced the prediction, when available.
    pub buffer_id: Option<BufferId>,
    /// File that produced the prediction, when available.
    pub file_id: Option<FileId>,
    /// Display-safe provider label supplied by the app layer.
    pub provider_label: String,
    /// Stable status used by UI branching.
    pub status: AssistInlinePredictionStatusProjection,
    /// Display-safe status label supplied by the app layer.
    pub status_label: String,
    /// Provider latency in milliseconds, when measured.
    pub latency_ms: Option<u64>,
    /// Time the prediction was requested.
    pub requested_at: TimestampMillis,
    /// Time the prediction completed, when known.
    pub completed_at: Option<TimestampMillis>,
    /// Snapshot id used to produce the prediction, when supplied.
    pub snapshot_id: Option<SnapshotId>,
    /// Buffer version used to produce the prediction, when supplied.
    pub buffer_version: Option<BufferVersion>,
    /// File fingerprint used to produce the prediction, when supplied.
    pub file_fingerprint: Option<FileFingerprint>,
    /// Whether the prediction is stale relative to current projected metadata.
    pub stale: bool,
    /// Display-safe stale reason label supplied by the app layer.
    pub stale_reason_label: Option<String>,
    /// Bounded ghost text display label supplied by the app layer.
    pub ghost_text_label: String,
    /// Bounded replacement preview label supplied by the app layer.
    pub replacement_preview_label: Option<String>,
    /// Range the app would replace if the prediction is accepted.
    pub apply_range: ProtocolTextRange,
    /// Display-safe apply range label supplied by the app layer.
    pub apply_range_label: String,
    /// Display-safe diagnostics for prediction state.
    pub diagnostics: Vec<String>,
}

/// Projection-only Assist inline prediction surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistInlinePredictionProjection {
    /// Current ghost prediction, if one should be shown.
    pub active_prediction: Option<AssistInlinePredictionRowProjection>,
    /// Bounded recent prediction rows supplied by the app layer.
    pub rows: Vec<AssistInlinePredictionRowProjection>,
    /// Whether an app-owned prediction request is currently in flight.
    pub request_in_flight: bool,
    /// Number of omitted or stale prediction rows represented by metadata.
    pub stale_prediction_count: usize,
    /// After-edit prediction attempts represented in the current projection.
    pub after_edit_prediction_attempts: usize,
    /// After-edit prediction accepts represented in the current projection.
    pub after_edit_prediction_accepts: usize,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Projection schema version.
    pub schema_version: u16,
}

impl AssistInlinePredictionProjection {
    /// Construct an empty Assist inline prediction projection.
    pub fn empty() -> Self {
        Self {
            active_prediction: None,
            rows: Vec::new(),
            request_in_flight: false,
            stale_prediction_count: 0,
            after_edit_prediction_attempts: 0,
            after_edit_prediction_accepts: 0,
            generated_at: TimestampMillis(0),
            schema_version: 1,
        }
    }

    /// Count display rows including the current active prediction when it is not duplicated.
    pub fn display_row_count(&self) -> usize {
        self.rows.len()
            + usize::from(self.active_prediction.as_ref().is_some_and(|active| {
                !self
                    .rows
                    .iter()
                    .any(|row| row.prediction_id == active.prediction_id)
            }))
    }

    /// Return whether any Assist prediction metadata should activate Assist UI mode.
    pub fn has_activity(&self) -> bool {
        self.request_in_flight
            || self.active_prediction.is_some()
            || !self.rows.is_empty()
            || self.stale_prediction_count > 0
    }
}

impl Default for AssistInlinePredictionProjection {
    fn default() -> Self {
        Self::empty()
    }
}

/// Metadata-only tab row projected from application-owned editor state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorTabProjection {
    /// Backing editor buffer identifier.
    pub buffer_id: BufferId,
    /// Backing workspace file identifier when the tab is file-backed.
    pub file_id: Option<FileId>,
    /// Canonical path for display and restore metadata.
    pub file_path: Option<CanonicalPath>,
    /// Display title.
    pub title: String,
    /// Whether this tab is currently active.
    pub active: bool,
    /// Whether the backing buffer has unsaved changes.
    pub dirty: bool,
    /// Whether this tab is pinned.
    pub pinned: bool,
    /// Whether this tab is a preview tab.
    pub preview: bool,
}

/// Projection-only tab list for daily editing surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditorTabsProjection {
    /// Open tabs in display order.
    pub tabs: Vec<EditorTabProjection>,
    /// Active buffer identifier when a tab is selected.
    pub active_buffer_id: Option<BufferId>,
}

/// Metadata-only close prompt for a dirty buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseDirtyPromptProjection {
    /// Dirty buffer that requested close.
    pub buffer_id: BufferId,
    /// File identifier when the dirty buffer is file-backed.
    pub file_id: Option<FileId>,
    /// Canonical path for display.
    pub file_path: Option<CanonicalPath>,
    /// Display title.
    pub title: String,
    /// User-visible prompt message.
    pub message: String,
}

/// Per-buffer viewport input state preserved by app authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorViewportStateProjection {
    /// Buffer represented by this viewport state.
    pub buffer_id: BufferId,
    /// Last known viewport scroll.
    pub scroll: ViewportScroll,
    /// Last projected primary cursor, if available.
    pub cursor: Option<TextCoordinate>,
    /// Last projected selections, if available.
    pub selections: Vec<ProtocolTextRange>,
}

/// Metadata-only session summary derived from a workspace session record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSessionRecordProjection {
    /// Session identifier.
    pub session_id: String,
    /// Last workspace identifier.
    pub last_workspace: Option<WorkspaceId>,
    /// Number of open tabs represented by the record.
    pub open_tab_count: usize,
    /// Active buffer identifier.
    pub active_buffer: Option<BufferId>,
    /// Last saved timestamp.
    pub saved_at: TimestampMillis,
    /// Session schema version.
    pub schema_version: u16,
}

/// Daily-editing projection composed from app/editor metadata only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyEditingProjection {
    /// Open editor tabs.
    pub tabs: EditorTabsProjection,
    /// Prompt state for attempted dirty close.
    pub close_dirty_prompt: Option<CloseDirtyPromptProjection>,
    /// Per-buffer viewport state.
    pub viewport_states: Vec<EditorViewportStateProjection>,
    /// Metadata-only session summary for restore surfaces.
    pub session_record: Option<WorkspaceSessionRecordProjection>,
}

/// One excerpt row in a multibuffer excerpt surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcerptSurfaceLineProjection {
    /// Zero-based source line number.
    pub line_number: u32,
    /// Visible excerpt text.
    pub visible_text: String,
    /// Source range for the visible excerpt.
    pub range: Utf16Range,
    /// Truncation state for the visible excerpt slice.
    pub truncation_state: ViewportLineTruncationState,
}

/// One excerpt section composed from a source buffer snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcerptSurfaceSectionProjection {
    /// Stable excerpt section identifier.
    pub excerpt_id: String,
    /// Owning workspace identifier when available.
    pub workspace_id: Option<WorkspaceId>,
    /// Source buffer identifier when available.
    pub buffer_id: Option<BufferId>,
    /// Source file identifier when available.
    pub file_id: Option<FileId>,
    /// Canonical source path when available.
    pub file_path: Option<CanonicalPath>,
    /// Display title for the source buffer.
    pub title: String,
    /// Whether the source buffer currently has unsaved edits.
    pub dirty: bool,
    /// Whether the source buffer remains directly editable.
    pub editable: bool,
    /// Snapshot identifier used to produce this excerpt section.
    pub snapshot_id: Option<SnapshotId>,
    /// Projected cursor for the source buffer when available.
    pub cursor: Option<TextCoordinate>,
    /// Visible lines from the source buffer snapshot.
    pub lines: Vec<ExcerptSurfaceLineProjection>,
}

/// Projection-only multibuffer excerpt surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcerptSurfaceProjection {
    /// Ordered excerpt sections projected from open buffers.
    pub sections: Vec<ExcerptSurfaceSectionProjection>,
    /// Active excerpt section identifier when one is focused.
    pub active_excerpt_id: Option<String>,
    /// Projection schema version.
    pub schema_version: u16,
}

impl ExcerptSurfaceProjection {
    /// Construct an empty excerpt surface projection.
    pub fn empty() -> Self {
        Self {
            sections: Vec::new(),
            active_excerpt_id: None,
            schema_version: 1,
        }
    }
}

impl Default for ExcerptSurfaceProjection {
    fn default() -> Self {
        Self::empty()
    }
}

impl DailyEditingProjection {
    /// Construct an empty daily-editing projection.
    pub fn empty() -> Self {
        Self {
            tabs: EditorTabsProjection::default(),
            close_dirty_prompt: None,
            viewport_states: Vec::new(),
            session_record: None,
        }
    }
}

impl Default for DailyEditingProjection {
    fn default() -> Self {
        Self::empty()
    }
}

/// Search scope selected by projection-only UI controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchScopeProjection {
    /// Search only the active editor buffer.
    #[default]
    ActiveFile,
    /// Search workspace files through app/workspace authority.
    Workspace,
}

/// High-level search status for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchStatusKindProjection {
    /// No search has run.
    Idle,
    /// Search is in progress.
    Running,
    /// Search completed with one or more results.
    Completed,
    /// Search completed without results.
    NoResults,
    /// Search was cancelled by query id.
    Cancelled,
    /// Search could not run because user input was invalid.
    ValidationError,
    /// Search ran in a bounded degraded mode.
    DegradedLimited,
    /// Search failed without panicking.
    Error,
}

/// Display-safe search status message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchStatusProjection {
    /// Status kind for stable view logic.
    pub kind: SearchStatusKindProjection,
    /// User-visible status message.
    pub message: String,
}

impl SearchStatusProjection {
    /// Construct an idle status.
    pub fn idle() -> Self {
        Self {
            kind: SearchStatusKindProjection::Idle,
            message: "Search idle".to_string(),
        }
    }
}

/// One bounded lexical search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResultProjection {
    /// Search query id that produced this row.
    pub query_id: String,
    /// Search scope that produced this row.
    pub scope: SearchScopeProjection,
    /// Workspace containing the result when known.
    pub workspace_id: Option<WorkspaceId>,
    /// Buffer containing the result when it is open.
    pub buffer_id: Option<BufferId>,
    /// Workspace file containing the result when known.
    pub file_id: Option<FileId>,
    /// Canonical path containing the result when known.
    pub file_path: Option<CanonicalPath>,
    /// Zero-based result line number.
    pub line_number: u32,
    /// Bounded result range in projection coordinates.
    pub range: ProtocolTextRange,
    /// Bounded snippet around the match.
    pub snippet: String,
    /// Whether the snippet was truncated.
    pub snippet_truncated: bool,
}

/// Projection-only bounded search surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchProjection {
    /// Current query id when a search has run.
    pub query_id: Option<String>,
    /// Current search scope.
    pub scope: SearchScopeProjection,
    /// Display-safe query label.
    pub query_label: String,
    /// Current status.
    pub status: SearchStatusProjection,
    /// Bounded result rows.
    pub results: Vec<SearchResultProjection>,
    /// Applied result limit.
    pub result_limit: usize,
    /// Count of result rows omitted by result limit.
    pub omitted_result_count: usize,
    /// Count of files skipped or omitted by bounds/errors.
    pub omitted_file_count: usize,
    /// Display-safe diagnostics for skipped/limited search.
    pub diagnostics: Vec<String>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Projection schema version.
    pub schema_version: u16,
}

impl SearchProjection {
    /// Construct an idle search projection.
    pub fn idle() -> Self {
        Self {
            query_id: None,
            scope: SearchScopeProjection::ActiveFile,
            query_label: String::new(),
            status: SearchStatusProjection::idle(),
            results: Vec::new(),
            result_limit: 0,
            omitted_result_count: 0,
            omitted_file_count: 0,
            diagnostics: Vec::new(),
            generated_at: TimestampMillis(0),
            schema_version: 1,
        }
    }
}

impl Default for SearchProjection {
    fn default() -> Self {
        Self::idle()
    }
}

/// One metavariable capture projected by structural search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralSearchCaptureProjection {
    /// Capture name without the `$` prefix.
    pub name: String,
    /// Display-safe captured value.
    pub value: String,
    /// Captured source range.
    pub range: ProtocolTextRange,
}

/// One structural search result projected to the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralSearchMatchProjection {
    /// Query id that produced this row.
    pub query_id: String,
    /// Workspace containing the match.
    pub workspace_id: WorkspaceId,
    /// File containing the match.
    pub file_id: FileId,
    /// Canonical path containing the match.
    pub file_path: CanonicalPath,
    /// Matched source range.
    pub range: ProtocolTextRange,
    /// Captured metavariable values.
    pub captures: Vec<StructuralSearchCaptureProjection>,
    /// Bounded matched source snippet.
    pub snippet: String,
    /// Replacement preview for this row, when a rewrite template was provided.
    pub replacement_preview: Option<String>,
}

/// Projection-only structural search and replace surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralSearchProjection {
    /// Current query id when structural search has run.
    pub query_id: Option<String>,
    /// Search scope used for the current result set.
    pub scope: SearchScopeProjection,
    /// Display-safe structural pattern label.
    pub pattern_label: String,
    /// Display-safe rewrite label, when supplied.
    pub rewrite_label: Option<String>,
    /// Current status.
    pub status: SearchStatusProjection,
    /// Bounded structural match rows.
    pub matches: Vec<StructuralSearchMatchProjection>,
    /// Applied result limit.
    pub result_limit: usize,
    /// Count of match rows omitted by result limit.
    pub omitted_match_count: usize,
    /// Count of files skipped or omitted by bounds/errors.
    pub omitted_file_count: usize,
    /// Display-safe diagnostics for skipped, suppressed, or invalid structural searches.
    pub diagnostics: Vec<String>,
    /// Proposal preview created for rewrite-capable search, when available.
    pub proposal_id: Option<ProposalId>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Projection schema version.
    pub schema_version: u16,
}

impl StructuralSearchProjection {
    /// Construct an idle structural search projection.
    pub fn idle() -> Self {
        Self {
            query_id: None,
            scope: SearchScopeProjection::Workspace,
            pattern_label: String::new(),
            rewrite_label: None,
            status: SearchStatusProjection::idle(),
            matches: Vec::new(),
            result_limit: 0,
            omitted_match_count: 0,
            omitted_file_count: 0,
            diagnostics: Vec::new(),
            proposal_id: None,
            generated_at: TimestampMillis(0),
            schema_version: 1,
        }
    }
}

impl Default for StructuralSearchProjection {
    fn default() -> Self {
        Self::idle()
    }
}

/// Diff strategy shown for a changed git file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitDiffStrategyProjection {
    /// Syntax-aware diff metadata is available.
    Syntactic,
    /// Line diff fallback is being used.
    LineFallback,
}

/// Current stage of a projected git hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHunkStageProjection {
    /// Hunk is in the working tree only.
    Unstaged,
    /// Hunk is in the git index.
    Staged,
}

/// One changed file in the git projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitFileProjection {
    /// Repository-relative path.
    pub path: String,
    /// Two-column porcelain status.
    pub status: String,
    /// Inserted line count.
    pub inserted_lines: u32,
    /// Deleted line count.
    pub deleted_lines: u32,
    /// Number of unstaged hunks.
    pub unstaged_hunk_count: usize,
    /// Number of staged hunks.
    pub staged_hunk_count: usize,
    /// Whether stage/unstage hunk actions are available.
    pub stageable: bool,
    /// Diff strategy used for this file.
    pub diff_strategy: GitDiffStrategyProjection,
    /// Reason for line fallback, when present.
    pub fallback_reason: Option<String>,
    /// Whether conflict markers were detected.
    pub conflict: bool,
}

/// One hunk in the git projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHunkProjection {
    /// Stable hunk identifier.
    pub hunk_id: String,
    /// Repository-relative path.
    pub path: String,
    /// Current hunk stage.
    pub stage: GitHunkStageProjection,
    /// Unified diff hunk header.
    pub header: String,
    /// Old-file start line in the patch header.
    pub old_start: u32,
    /// Old-file line count in the patch header.
    pub old_lines: u32,
    /// New-file start line in the patch header.
    pub new_start: u32,
    /// New-file line count in the patch header.
    pub new_lines: u32,
    /// Added line count.
    pub added_lines: u32,
    /// Deleted line count.
    pub deleted_lines: u32,
    /// Optional scope/function context.
    pub context: Option<String>,
}

/// One inline blame row for the active file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBlameLineProjection {
    /// Repository-relative path.
    pub path: String,
    /// One-based line number.
    pub line_number: u32,
    /// Short commit hash.
    pub commit_short: String,
    /// Author label.
    pub author: String,
    /// Commit summary.
    pub summary: String,
    /// Bounded source preview.
    pub line_preview: String,
}

/// One commit row in the git graph/history projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommitProjection {
    /// Full commit hash.
    pub hash: String,
    /// Short commit hash.
    pub short_hash: String,
    /// Author label.
    pub author: String,
    /// Commit date label.
    pub date: String,
    /// Commit summary.
    pub summary: String,
    /// Number of parents.
    pub parent_count: usize,
    /// Decorated refs.
    pub refs: Vec<String>,
}

/// One conflict marker summary in the git projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitConflictProjection {
    /// Repository-relative path.
    pub path: String,
    /// Number of conflict marker lines.
    pub marker_count: usize,
    /// Projected conflict resolution actions.
    pub actions: Vec<String>,
}

/// Which side of a conflict to keep when resolving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitConflictChoiceProjection {
    /// Keep the current (ours) side.
    AcceptCurrent,
    /// Keep the incoming (theirs) side.
    AcceptIncoming,
}

/// Projected git worktree classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitWorktreeKindProjection {
    /// Worktree used for delegated agent isolation.
    Agent,
    /// Human-managed worktree.
    Manual,
}

/// Projected git worktree row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorktreeProjection {
    /// Worktree path.
    pub path: String,
    /// Current branch label when available.
    pub branch_label: Option<String>,
    /// Current short HEAD hash when available.
    pub head_short: Option<String>,
    /// Worktree category.
    pub kind: GitWorktreeKindProjection,
    /// Whether git considers the worktree prunable/orphaned.
    pub prunable: bool,
}

/// Projection-only git status, syntactic diff, blame, graph, and conflict surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitProjection {
    /// Repository root label.
    pub root_label: Option<String>,
    /// Current branch label.
    pub branch_label: Option<String>,
    /// Current short HEAD hash.
    pub head_short: Option<String>,
    /// Repository origin remote URL.
    pub remote_url: Option<String>,
    /// Origin default branch label.
    pub remote_default_branch: Option<String>,
    /// Changed files.
    pub changed_files: Vec<GitFileProjection>,
    /// Staged and unstaged hunks.
    pub hunks: Vec<GitHunkProjection>,
    /// Inline blame rows for the active file.
    pub blame_lines: Vec<GitBlameLineProjection>,
    /// Commit graph/history rows.
    pub commits: Vec<GitCommitProjection>,
    /// Conflict marker rows.
    pub conflicts: Vec<GitConflictProjection>,
    /// Projected worktree rows.
    pub worktrees: Vec<GitWorktreeProjection>,
    /// Display-safe diagnostics.
    pub diagnostics: Vec<String>,
    /// Generated timestamp.
    pub generated_at: TimestampMillis,
    /// Projection schema version.
    pub schema_version: u32,
}

impl GitProjection {
    /// Construct an idle git projection.
    pub fn idle() -> Self {
        Self {
            root_label: None,
            branch_label: None,
            head_short: None,
            remote_url: None,
            remote_default_branch: None,
            changed_files: Vec::new(),
            hunks: Vec::new(),
            blame_lines: Vec::new(),
            commits: Vec::new(),
            conflicts: Vec::new(),
            worktrees: Vec::new(),
            diagnostics: Vec::new(),
            generated_at: TimestampMillis(0),
            schema_version: 1,
        }
    }
}

impl Default for GitProjection {
    fn default() -> Self {
        Self::idle()
    }
}

/// Debugger status kind projected by the application layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugStatusKindProjection {
    /// No debug workflow has run.
    Idle,
    /// Debug configuration or adapter launch is running.
    Launching,
    /// Program is running.
    Running,
    /// Program is paused at a breakpoint or step.
    Paused,
    /// Debug session exited.
    Exited,
    /// Debug workflow was denied.
    Denied,
    /// Debug workflow failed.
    Failed,
}

/// Debugger status projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugStatusProjection {
    /// Status kind.
    pub kind: DebugStatusKindProjection,
    /// Display-safe status message.
    pub message: String,
}

impl DebugStatusProjection {
    /// Construct an idle debug status.
    pub fn idle() -> Self {
        Self {
            kind: DebugStatusKindProjection::Idle,
            message: "Debug idle".to_string(),
        }
    }
}

/// Debug stepping operation selected from UI projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugStepKindProjection {
    /// Continue execution.
    Continue,
    /// Step over.
    Over,
    /// Step into.
    Into,
    /// Step out.
    Out,
    /// Step backward.
    Back,
}

/// Projected debug launch configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugConfigurationProjection {
    /// Configuration identifier.
    pub configuration_id: DebugConfigurationId,
    /// Display name.
    pub name: String,
    /// Adapter type.
    pub adapter_type: String,
    /// Program label.
    pub program_label: String,
    /// Cargo package name.
    pub cargo_package: Option<String>,
    /// Cargo target name.
    pub cargo_target: Option<String>,
    /// Whether this configuration is deterministic/manual eligible.
    pub deterministic: bool,
}

/// Projected debug breakpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugBreakpointProjection {
    /// Breakpoint identifier.
    pub breakpoint_id: DebugBreakpointId,
    /// Last verifying session, if any.
    pub session_id: Option<DebugSessionId>,
    /// Source path label.
    pub path: CanonicalPath,
    /// One-based line label.
    pub line: u32,
    /// Whether the breakpoint is enabled.
    pub enabled: bool,
    /// Conditional expression label.
    pub condition: Option<String>,
    /// Hit condition label.
    pub hit_condition: Option<String>,
    /// Logpoint message label.
    pub log_message: Option<String>,
    /// Whether the adapter verified this breakpoint.
    pub verified: bool,
    /// Verification message.
    pub message: Option<String>,
}

/// Projected debug stack frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugStackFrameProjection {
    /// Owning session.
    pub session_id: DebugSessionId,
    /// Frame id from the adapter.
    pub frame_id: u64,
    /// Display name.
    pub name: String,
    /// Source path label.
    pub path: Option<CanonicalPath>,
    /// One-based line label.
    pub line: Option<u32>,
}

/// Projected debug variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugVariableProjection {
    /// Owning session.
    pub session_id: DebugSessionId,
    /// Variable name.
    pub name: String,
    /// Metadata-only value label.
    pub value_label: String,
    /// Optional type label.
    pub type_label: Option<String>,
    /// Whether children are available.
    pub has_children: bool,
}

/// Projected debug watch expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugWatchProjection {
    /// Watch identifier.
    pub watch_id: legion_protocol::DebugWatchId,
    /// Owning session.
    pub session_id: DebugSessionId,
    /// Expression label.
    pub expression_label: String,
    /// Metadata-only value label.
    pub value_label: String,
    /// Optional type label.
    pub type_label: Option<String>,
}

/// Projected debug console entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugConsoleProjection {
    /// Owning session.
    pub session_id: DebugSessionId,
    /// Category label.
    pub category_label: String,
    /// Metadata-only message label.
    pub message_label: String,
}

/// Projected inline debug value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugInlineValueProjection {
    /// Owning session.
    pub session_id: DebugSessionId,
    /// Source path.
    pub path: CanonicalPath,
    /// One-based line label.
    pub line: u32,
    /// Expression label.
    pub expression_label: String,
    /// Metadata-only value label.
    pub value_label: String,
}

/// Projection-only debugger surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugProjection {
    /// Current status.
    pub status: DebugStatusProjection,
    /// Active session id.
    pub active_session_id: Option<DebugSessionId>,
    /// Active session state.
    pub session_state: Option<DebugSessionState>,
    /// Discovered launch configurations.
    pub configurations: Vec<DebugConfigurationProjection>,
    /// Persisted breakpoints.
    pub breakpoints: Vec<DebugBreakpointProjection>,
    /// Variables for the right dock.
    pub variables: Vec<DebugVariableProjection>,
    /// Watch expressions for the right dock.
    pub watches: Vec<DebugWatchProjection>,
    /// Call stack frames for the bottom dock.
    pub stack_frames: Vec<DebugStackFrameProjection>,
    /// Debug console rows for the bottom dock.
    pub console: Vec<DebugConsoleProjection>,
    /// Inline values projected in-editor.
    pub inline_values: Vec<DebugInlineValueProjection>,
    /// Display-safe diagnostics.
    pub diagnostics: Vec<String>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Projection schema version.
    pub schema_version: u16,
}

impl DebugProjection {
    /// Construct an empty debug projection.
    pub fn empty() -> Self {
        Self {
            status: DebugStatusProjection::idle(),
            active_session_id: None,
            session_state: None,
            configurations: Vec::new(),
            breakpoints: Vec::new(),
            variables: Vec::new(),
            watches: Vec::new(),
            stack_frames: Vec::new(),
            console: Vec::new(),
            inline_values: Vec::new(),
            diagnostics: Vec::new(),
            generated_at: TimestampMillis(0),
            schema_version: 1,
        }
    }
}

impl Default for DebugProjection {
    fn default() -> Self {
        Self::empty()
    }
}

/// UI status severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusSeverity {
    /// Informational status message.
    Info,
    /// Warning status message.
    Warning,
    /// Error status message.
    Error,
}

/// Projected status message shown by the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMessageProjection {
    /// Severity classification.
    pub severity: StatusSeverity,
    /// Human-readable message.
    pub message: String,
}

/// App-owned command palette mode projected to renderer adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    /// Workspace file opener mode.
    File,
    /// Workspace symbol finder mode.
    Symbol,
    /// Recent open buffers switcher mode.
    RecentBuffers,
    /// Curated command-dispatch mode.
    Command,
    /// Lexical search mode.
    Search,
    /// Structural search/rewrite-preview mode.
    StructuralSearch,
}

impl PaletteMode {
    /// Stable label for display-only renderer surfaces.
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "Files",
            Self::Symbol => "Symbols",
            Self::RecentBuffers => "Recent Buffers",
            Self::Command => "Commands",
            Self::Search => "Search",
            Self::StructuralSearch => "Structural Search",
        }
    }

    /// Prefix used to force this mode from the palette input.
    pub fn prefix(self) -> Option<char> {
        match self {
            Self::File => None,
            Self::Symbol => Some('@'),
            Self::RecentBuffers => Some('^'),
            Self::Command => Some('>'),
            Self::Search => Some('/'),
            Self::StructuralSearch => Some('#'),
        }
    }
}

/// Kind of a projected command palette result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteResultKind {
    /// Workspace file result.
    File,
    /// Workspace symbol result.
    Symbol,
    /// Recent open buffer result.
    RecentBuffers,
    /// Curated command result.
    Command,
    /// Lexical search execution result.
    Search,
    /// Structural search execution result.
    StructuralSearch,
}

/// One app-ranked result in the command palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteResult {
    /// Stable result identifier used by app-side selected-result dispatch.
    pub id: String,
    /// Result kind.
    pub kind: PaletteResultKind,
    /// Primary display title.
    pub title: String,
    /// Secondary metadata label.
    pub detail: Option<String>,
    /// Shortcut or action hint label.
    pub shortcut_label: Option<String>,
    /// Workspace path for file, symbol, or buffer-backed results.
    pub path: Option<String>,
    /// Buffer identifier for buffer-switching results.
    pub buffer_id: Option<BufferId>,
    /// Cursor position for jump-to-location results.
    pub position: Option<TextCoordinate>,
    /// Character indices in `title` that matched the current query.
    pub match_indices: Vec<usize>,
    /// Reason the row is displayed but not dispatchable.
    pub disabled_reason: Option<String>,
}

/// App-owned command palette projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteProjection {
    /// Whether the foreground palette overlay should be visible.
    pub open: bool,
    /// Active palette mode.
    pub mode: PaletteMode,
    /// Current input query including a mode prefix when present.
    pub query: String,
    /// Search scope used by search and structural-search modes.
    pub scope: SearchScopeProjection,
    /// Selected result index, clamped to `results`.
    pub selected_index: usize,
    /// Ranked palette results.
    pub results: Vec<PaletteResult>,
}

impl PaletteProjection {
    /// Empty closed palette projection.
    pub fn closed() -> Self {
        Self {
            open: false,
            mode: PaletteMode::File,
            query: String::new(),
            scope: SearchScopeProjection::ActiveFile,
            selected_index: 0,
            results: Vec::new(),
        }
    }
}

impl Default for PaletteProjection {
    fn default() -> Self {
        Self::closed()
    }
}

/// User preference for resolving the active workbench theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemePreferenceProjection {
    /// Always use the dark theme.
    #[default]
    Dark,
    /// Always use the light theme.
    Light,
    /// Follow the operating-system theme when available.
    System,
}

impl ThemePreferenceProjection {
    /// Stable display label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::System => "System",
        }
    }

    /// Stable persisted label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }

    /// Parse a persisted or user-facing label.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "dark" | "Dark" => Some(Self::Dark),
            "light" | "Light" => Some(Self::Light),
            "system" | "System" => Some(Self::System),
            _ => None,
        }
    }
}

/// User preference for which status messages become foreground toasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ToastVerbosityProjection {
    /// Show only error toasts.
    ErrorsOnly,
    /// Show warning and error toasts.
    #[default]
    WarningsAndErrors,
    /// Show all status messages as toasts.
    All,
}

impl ToastVerbosityProjection {
    /// Stable display label.
    pub fn label(self) -> &'static str {
        match self {
            Self::ErrorsOnly => "Errors only",
            Self::WarningsAndErrors => "Warnings and errors",
            Self::All => "All statuses",
        }
    }

    /// Stable persisted label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ErrorsOnly => "errors_only",
            Self::WarningsAndErrors => "warnings_and_errors",
            Self::All => "all",
        }
    }

    /// Parse a persisted or user-facing label.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "errors_only" | "Errors only" => Some(Self::ErrorsOnly),
            "warnings_and_errors" | "Warnings and errors" => Some(Self::WarningsAndErrors),
            "all" | "All statuses" => Some(Self::All),
            _ => None,
        }
    }
}

/// Editor-specific user settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorSettingsProjection {
    /// Whether line numbers are visible in the editor gutter.
    pub line_numbers_visible: bool,
    /// Whether the active line receives a background highlight.
    pub current_line_highlight: bool,
    /// Whether sticky function/scope headers are visible.
    pub sticky_headers_visible: bool,
    /// Whether code folding indicators are visible.
    pub code_folding_visible: bool,
    /// Whether the minimap is visible.
    pub minimap_visible: bool,
    /// Whether whitespace guides are visible.
    pub whitespace_guides_visible: bool,
    /// Whether indent guides are visible.
    pub indent_guides_visible: bool,
    /// Whether smooth scrolling is enabled.
    pub smooth_scrolling_enabled: bool,
}

impl Default for EditorSettingsProjection {
    fn default() -> Self {
        Self {
            line_numbers_visible: true,
            current_line_highlight: true,
            sticky_headers_visible: true,
            code_folding_visible: true,
            minimap_visible: false,
            whitespace_guides_visible: false,
            indent_guides_visible: false,
            smooth_scrolling_enabled: true,
        }
    }
}

/// App-owned settings projected to renderers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsProjection {
    /// Theme preference.
    pub theme_preference: ThemePreferenceProjection,
    /// UI zoom percentage.
    pub zoom_percent: u16,
    /// Editor font size in points.
    pub editor_font_size_pt: u16,
    /// Toast verbosity.
    pub toast_verbosity: ToastVerbosityProjection,
    /// Editor options.
    pub editor: EditorSettingsProjection,
    /// Telemetry consent state.
    pub telemetry: WorkbenchTelemetryConsent,
    /// Whether workspace search may use the optional indexed backend.
    pub indexed_workspace_search_enabled: bool,
    /// Whether next-edit prediction should auto-trigger after edits.
    pub next_edit_prediction_enabled: bool,
    /// Projection schema version.
    pub schema_version: u16,
}

impl SettingsProjection {
    /// Minimum supported zoom percentage.
    pub const MIN_ZOOM_PERCENT: u16 = 80;
    /// Maximum supported zoom percentage.
    pub const MAX_ZOOM_PERCENT: u16 = 200;
    /// Minimum supported editor font size in points.
    pub const MIN_EDITOR_FONT_SIZE_PT: u16 = 10;
    /// Maximum supported editor font size in points.
    pub const MAX_EDITOR_FONT_SIZE_PT: u16 = 24;

    /// Return a copy with bounded numeric values.
    pub fn normalized(mut self) -> Self {
        self.zoom_percent = self
            .zoom_percent
            .clamp(Self::MIN_ZOOM_PERCENT, Self::MAX_ZOOM_PERCENT);
        self.editor_font_size_pt = self
            .editor_font_size_pt
            .clamp(Self::MIN_EDITOR_FONT_SIZE_PT, Self::MAX_EDITOR_FONT_SIZE_PT);
        self.telemetry.enabled = self.telemetry.crash_reports_enabled;
        self.telemetry.raw_source_allowed = false;
        self.telemetry.consent_label = if self.telemetry.crash_reports_enabled {
            "crash-reports".to_string()
        } else {
            "local-only".to_string()
        };
        if self.schema_version == 0 {
            self.schema_version = 1;
        }
        self
    }
}

impl Default for SettingsProjection {
    fn default() -> Self {
        Self {
            theme_preference: ThemePreferenceProjection::Dark,
            zoom_percent: 100,
            editor_font_size_pt: 12,
            toast_verbosity: ToastVerbosityProjection::WarningsAndErrors,
            editor: EditorSettingsProjection::default(),
            telemetry: WorkbenchTelemetryConsent::default(),
            indexed_workspace_search_enabled: false,
            next_edit_prediction_enabled: false,
            schema_version: 1,
        }
    }
}

/// Typed command intent emitted by UI input handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDispatchIntent {
    /// No command was recognized.
    Noop,
    /// Quit the active shell loop.
    Quit,
    /// Set the app-owned product mode used for dock filtering and AI dispatch gates.
    SetProductMode {
        /// Target product mode.
        mode: DockMode,
    },
    /// Undo through application/editor authority for the target buffer.
    Undo {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Redo through application/editor authority for the target buffer.
    Redo {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Insert text through application/editor authority for the target buffer.
    Insert {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Insertion position in projected protocol text coordinates.
        at: TextCoordinate,
        /// Replacement payload.
        text: String,
    },
    /// Delete a protocol text range through application/editor authority for the target buffer.
    Delete {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Range to delete.
        range: ProtocolTextRange,
    },
    /// Replace a protocol text range through application/editor authority for the target buffer.
    Replace {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Range to replace.
        range: ProtocolTextRange,
        /// Replacement payload.
        replacement: String,
    },
    /// Save through the editor save-request and workspace write path.
    Save {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Switch the active editor tab through app authority.
    SwitchTab {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Request close for a tab through app authority.
    CloseTab {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Save all open buffers through app-owned save workflows.
    SaveAll,
    /// Set primary cursor through editor authority.
    SetCursor {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor coordinate from projection space.
        cursor: TextCoordinate,
    },
    /// Set selection through editor authority.
    SetSelection {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Selection range from projection space.
        range: ProtocolTextRange,
    },
    /// Set viewport scroll through app-owned viewport state.
    SetViewportScroll {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Scroll offsets.
        scroll: ViewportScroll,
    },
    /// Open the app-owned command palette in the requested mode.
    OpenPalette {
        /// Requested palette mode.
        mode: PaletteMode,
        /// Initial query text.
        query: String,
        /// Search scope used by search-flavored palette modes.
        scope: SearchScopeProjection,
    },
    /// Close the app-owned command palette.
    ClosePalette,
    /// Update the app-owned palette query.
    UpdatePaletteQuery {
        /// Updated query text.
        query: String,
    },
    /// Move the selected palette result by a signed delta.
    MovePaletteSelection {
        /// Signed selection delta.
        delta: i32,
    },
    /// Complete the current palette selection where supported.
    CompletePaletteSelection,
    /// Dispatch the currently selected palette result through app authority.
    DispatchPaletteSelection,
    /// Open the projected Settings surface.
    OpenSettings,
    /// Update the app-owned theme preference.
    SetThemePreference {
        /// Requested theme preference.
        preference: ThemePreferenceProjection,
    },
    /// Update the app-owned UI zoom percentage.
    SetZoomPercent {
        /// Requested zoom percentage.
        zoom_percent: u16,
    },
    /// Update the app-owned editor font size.
    SetEditorFontSize {
        /// Requested editor font size in points.
        font_size_pt: u16,
    },
    /// Update app-owned toast verbosity.
    SetToastVerbosity {
        /// Requested toast verbosity.
        verbosity: ToastVerbosityProjection,
    },
    /// Toggle editor line-number visibility.
    SetLineNumbersVisible {
        /// Whether line numbers should be visible.
        visible: bool,
    },
    /// Toggle current-line highlighting.
    SetCurrentLineHighlight {
        /// Whether current-line highlighting is enabled.
        enabled: bool,
    },
    /// Toggle sticky headers.
    SetStickyHeadersVisible {
        /// Whether sticky headers should be visible.
        visible: bool,
    },
    /// Toggle code folding indicators.
    SetCodeFoldingVisible {
        /// Whether code folding indicators should be visible.
        visible: bool,
    },
    /// Toggle the minimap.
    SetMinimapVisible {
        /// Whether the minimap should be visible.
        visible: bool,
    },
    /// Toggle whitespace guides.
    SetWhitespaceGuidesVisible {
        /// Whether whitespace guides should be visible.
        visible: bool,
    },
    /// Toggle indent guides.
    SetIndentGuidesVisible {
        /// Whether indent guides should be visible.
        visible: bool,
    },
    /// Toggle smooth scrolling.
    SetSmoothScrollingEnabled {
        /// Whether smooth scrolling should be enabled.
        enabled: bool,
    },
    /// Toggle workspace search using the optional indexed backend.
    SetIndexedWorkspaceSearchEnabled {
        /// Whether workspace search should use the optional indexed backend.
        enabled: bool,
    },
    /// Toggle next-edit prediction after buffer edits.
    SetNextEditPredictionEnabled {
        /// Whether next-edit prediction should auto-trigger after edits.
        enabled: bool,
    },
    /// Toggle crash report consent.
    SetCrashReportsEnabled {
        /// Whether crash reports should be enabled.
        enabled: bool,
    },
    /// Reset app-owned settings to defaults.
    ResetSettings,
    /// Run bounded lexical search through app authority.
    RunSearch {
        /// Search scope.
        scope: SearchScopeProjection,
        /// User-provided query.
        query: String,
        /// Requested result limit; zero means app default.
        limit: usize,
    },
    /// Run deterministic structural search/rewrite preview through app authority.
    RunStructuralSearch {
        /// Search scope.
        scope: SearchScopeProjection,
        /// User-provided structural pattern.
        pattern: String,
        /// Optional rewrite template.
        rewrite: Option<String>,
        /// Requested result limit; zero means app default.
        limit: usize,
    },
    /// Cancel the currently projected search by query id.
    CancelSearch {
        /// Query id to cancel.
        query_id: String,
    },
    /// Refresh git status, syntactic diff, blame, graph, and conflict projections.
    RefreshGit,
    /// Stage one cached git hunk by projected hunk id.
    StageGitHunk {
        /// Projected hunk identifier.
        hunk_id: String,
    },
    /// Unstage one cached git hunk by projected hunk id.
    UnstageGitHunk {
        /// Projected hunk identifier.
        hunk_id: String,
    },
    /// Resolve one conflicted file by keeping the chosen side.
    ResolveGitConflict {
        /// Repository-relative path.
        path: String,
        /// Conflict resolution choice.
        choice: GitConflictChoiceProjection,
    },
    /// Commit the current staged index with a validated message.
    CommitGitChanges {
        /// Commit message entered in the git editor.
        message: String,
    },
    /// Switch to an existing git branch.
    SwitchGitBranch {
        /// Branch label entered by the user.
        branch: String,
    },
    /// Create and switch to a new git branch.
    CreateGitBranch {
        /// New branch label entered by the user.
        branch: String,
    },
    /// Delete a git branch.
    DeleteGitBranch {
        /// Branch label entered by the user.
        branch: String,
    },
    /// Stash local git changes.
    StashGitChanges {
        /// Optional stash message.
        message: Option<String>,
    },
    /// Push the current branch to a remote.
    PushGitRemote {
        /// Remote name.
        remote: String,
    },
    /// Prune orphaned worktree metadata.
    PruneGitWorktrees,
    /// Remove a projected worktree by path.
    RemoveGitWorktree {
        /// Projected worktree path.
        path: String,
    },
    /// Refresh debugger configuration projections.
    RefreshDebugConfigurations,

    /// Toggle a breakpoint or configure a logpoint/conditional breakpoint.
    ToggleDebugBreakpoint {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Zero-based line.
        line: u32,
        /// Conditional expression label.
        condition: Option<String>,
        /// Hit condition label.
        hit_condition: Option<String>,
        /// Logpoint message label.
        log_message: Option<String>,
    },
    /// Launch a debug session through app-owned debug authority.
    LaunchDebugSession {
        /// Configuration identifier selected from projection data.
        configuration_id: DebugConfigurationId,
    },
    /// Step or continue a debug session.
    DebugStep {
        /// Session identifier selected from projection data.
        session_id: DebugSessionId,
        /// Step kind.
        kind: DebugStepKindProjection,
    },
    /// Run to a projected cursor position.
    DebugRunToCursor {
        /// Session identifier selected from projection data.
        session_id: DebugSessionId,
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position.
        position: TextCoordinate,
    },
    /// Evaluate a selected expression.
    DebugEvaluateSelection {
        /// Session identifier selected from projection data.
        session_id: DebugSessionId,
        /// Bounded expression label.
        expression_label: String,
    },
    /// Add a watch expression.
    DebugAddWatch {
        /// Session identifier selected from projection data.
        session_id: DebugSessionId,
        /// Bounded expression label.
        expression_label: String,
    },
    /// Request hover data through app-owned language tooling.
    RequestHover {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position from projection space.
        position: TextCoordinate,
    },
    /// Request completion rows through app-owned language tooling.
    RequestCompletion {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position from projection space.
        position: TextCoordinate,
    },
    /// Request an Assist inline prediction through app-owned provider authority.
    RequestAssistInlinePrediction {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position from projection space.
        position: TextCoordinate,
    },
    /// Accept the currently projected Assist ghost prediction through app authority.
    AcceptAssistInlinePrediction {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Current prediction identifier selected from projection data, when available.
        prediction_id: Option<String>,
    },
    /// Dismiss the currently projected Assist ghost prediction through app authority.
    DismissAssistInlinePrediction {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Current prediction identifier selected from projection data, when available.
        prediction_id: Option<String>,
    },
    /// Cancel an in-flight Assist inline prediction through app authority.
    CancelAssistInlinePrediction {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Current prediction identifier selected from projection data, when available.
        prediction_id: Option<String>,
    },
    /// Request definition locations through app-owned language tooling.
    GoToDefinition {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position from projection space.
        position: TextCoordinate,
    },
    /// Request reference locations through app-owned language tooling.
    FindReferences {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position from projection space.
        position: TextCoordinate,
    },
    /// Refresh the active document outline through app-owned language tooling.
    RefreshOutline {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Request a formatting proposal preview through app authority.
    RequestFormattingProposal {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Request a rename proposal preview through app authority.
    RequestRenameProposal {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position from projection space.
        position: TextCoordinate,
        /// New symbol name label.
        new_name: String,
    },
    /// Request an organize-imports proposal preview through app authority.
    RequestOrganizeImportsProposal {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Request a code-action proposal preview through app authority.
    RequestCodeActionProposal {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Code-action identifier selected from projection data.
        action_id: String,
    },
    /// Activate a projected language code lens through app authority.
    ActivateLanguageCodeLens {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Code lens identifier selected from projection data.
        lens_id: String,
    },
    /// Cancel an in-flight language operation through app authority.
    CancelLanguageOperation {
        /// Operation identifier selected from projection data.
        operation_id: String,
    },
    /// Launch a policy-gated terminal session through app authority.
    TerminalLaunch {
        /// Display-safe command label or fixture command.
        command_label: String,
    },
    /// Send input to an active terminal session through app authority.
    TerminalInput {
        /// Terminal session identifier selected from projection data.
        session_id: TerminalSessionId,
        /// Input payload.
        payload: String,
    },
    /// Resize an active terminal session through app authority.
    TerminalResize {
        /// Terminal session identifier selected from projection data.
        session_id: TerminalSessionId,
        /// Column count.
        cols: u16,
        /// Row count.
        rows: u16,
    },
    /// Kill an active terminal session through app authority.
    TerminalKill {
        /// Terminal session identifier selected from projection data.
        session_id: TerminalSessionId,
    },
    /// Close an active terminal session through app authority.
    TerminalClose {
        /// Terminal session identifier selected from projection data.
        session_id: TerminalSessionId,
    },
    /// Poll terminal output through app authority.
    TerminalOutputPoll {
        /// Terminal session identifier selected from projection data.
        session_id: TerminalSessionId,
    },
    /// Search projected terminal output through app authority.
    TerminalSearch {
        /// Terminal session identifier selected from projection data.
        session_id: TerminalSessionId,
        /// Bounded query label.
        query: String,
    },
    /// Open a file by path through workspace authority.
    OpenPath {
        /// User-provided path text.
        path: String,
    },
    /// Open a file by path and position the cursor in the opened buffer.
    OpenPathAtPosition {
        /// User-provided path text.
        path: String,
        /// Cursor coordinate in the opened buffer.
        position: TextCoordinate,
    },
    /// Refresh explorer state through workspace ports.
    RefreshExplorer,
    /// Reveal a workspace file in the explorer projection.
    RevealInExplorer {
        /// File identifier to reveal.
        file_id: FileId,
    },
    /// Request a proposal preview through app/protocol authority.
    PreviewProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
    },
    /// Approve a proposal through app/protocol authority.
    ApproveProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
    },
    /// Reject a proposal through app/protocol authority.
    RejectProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
        /// User rejection reason.
        reason: ProposalRejectionReason,
    },
    /// Apply a proposal through app/protocol authority.
    ApplyProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
    },
    /// Roll back a proposal through app/protocol authority.
    RollbackProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
        /// User rollback reason.
        reason: ProposalRollbackReason,
    },
    /// Cancel a proposal through app/protocol authority.
    CancelProposal {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
        /// User cancellation reason.
        reason: ProposalCancellationReason,
    },
    /// Open proposal details by selecting static projection data.
    OpenProposalDetails {
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
    },
    /// Inspect a Legion workflow session using projection metadata.
    InspectLegionWorkflowSession {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
    },
    /// Open a Legion workflow linked proposal preview through app authority.
    OpenLegionWorkflowProposalPreview {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
    },
    /// Open Legion workflow linked proposal details through app authority.
    OpenLegionWorkflowProposalDetails {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Proposal identifier selected from projection data.
        proposal_id: ProposalId,
    },
    /// Request verification metadata recording for a Legion workflow gate.
    RequestLegionWorkflowVerification {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Verification gate identifier selected from projection data.
        gate_id: LegionWorkflowVerificationGateId,
    },
    /// Request sign-off metadata recording for a Legion workflow.
    RequestLegionWorkflowSignOff {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Sign-off identifier selected from projection data.
        sign_off_id: LegionWorkflowSignOffId,
    },
    /// Request conflict resolution metadata for a Legion workflow.
    ResolveLegionWorkflowConflict {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Conflict identifier selected from projection data.
        conflict_id: LegionWorkflowConflictId,
    },
    /// Request app-owned merge readiness evaluation for a Legion workflow.
    RequestLegionWorkflowMergeReadiness {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
    },
    /// Record a human decision for an Automate MCP tool permission request.
    RecordLegionWorkflowToolPermission {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// MCP server identifier selected from projection data.
        server_id: legion_protocol::McpServerId,
        /// MCP tool name selected from projection data.
        tool_name: legion_protocol::McpToolName,
        /// Human decision.
        decision: DelegatedTaskToolPermissionDecision,
    },
    /// Trigger the hard Automate kill switch for a workflow session.
    TriggerLegionWorkflowKillSwitch {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Display-safe reason label.
        reason_label: String,
    },
    /// Start a Phase 4 AI run through app-owned composition.
    StartAiRun {
        /// Display-safe instruction label.
        instruction_label: String,
    },
    /// Start a metadata-only assisted-AI explain run through app-owned composition.
    StartAiExplain {
        /// Display-safe instruction label.
        instruction_label: String,
    },
    /// Start a proposal-only assisted-AI edit run through app-owned composition.
    StartAiProposal {
        /// Display-safe instruction label.
        instruction_label: String,
    },
    /// Send a Delegate chat turn with codebase-context retrieval.
    SendDelegateChat {
        /// Display-safe prompt label.
        prompt_label: String,
    },
    /// Record a human decision for one Delegate proposal hunk.
    ReviewDelegateProposalHunk {
        /// Proposal being reviewed.
        proposal_id: ProposalId,
        /// Stable Delegate hunk identifier.
        hunk_id: String,
        /// Human disposition for the hunk.
        disposition: DelegatedTaskProposalHunkDisposition,
    },
    /// Record a human decision for one Delegate tool permission request.
    RecordDelegateToolPermission {
        /// Permission request identifier.
        request_id: String,
        /// Human permission decision.
        decision: DelegatedTaskToolPermissionDecision,
    },
    /// Cancel a Phase 4 AI run through app-owned composition.
    CancelAiRun {
        /// Agent run identifier selected from projection data or user input.
        run_id: AgentRunId,
    },
    /// Replay a Phase 4 AI run from metadata.
    ReplayAiRun {
        /// Agent run identifier selected from projection data or user input.
        run_id: AgentRunId,
    },
    /// Inspect a Phase 4 AI run using projection metadata.
    InspectAiRun {
        /// Agent run identifier selected from projection data or user input.
        run_id: AgentRunId,
    },
    /// Invoke a plugin command through app-owned plugin composition.
    InvokePluginCommand {
        /// Plugin identifier selected from projection data.
        plugin_id: PluginId,
        /// Command id selected from projection data.
        command_id: String,
        /// Metadata-only label for audit/UI display.
        metadata_label: String,
    },
    /// Join a collaboration session through app-owned collaboration composition.
    JoinCollaborationSession {
        /// Session identifier selected from projection data or user input.
        session_id: CollaborationSessionId,
    },
    /// Leave a collaboration session through app-owned collaboration composition.
    LeaveCollaborationSession {
        /// Session identifier selected from projection data or user input.
        session_id: CollaborationSessionId,
    },
    /// Publish metadata-only collaboration presence through app-owned composition.
    PublishCollaborationPresence {
        /// Session identifier selected from projection data or user input.
        session_id: CollaborationSessionId,
        /// Participant identifier selected from projection data or user input.
        participant_id: CollaborationParticipantId,
    },
}

/// Maximum visible foreground toast notifications.
pub const TOAST_VISIBLE_LIMIT: usize = 5;

/// Optional action attached to a foreground toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastActionProjection {
    /// Button label shown by the renderer.
    pub label: String,
    /// Existing command authority intent dispatched when the action is selected.
    pub intent: CommandDispatchIntent,
}

/// Renderer-agnostic foreground notification projected from shell status state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastProjection {
    /// Stable deterministic id for dismissal and testing.
    pub id: u64,
    /// Severity classification.
    pub severity: StatusSeverity,
    /// Primary notification title.
    pub title: String,
    /// Optional secondary notification text.
    pub body: Option<String>,
    /// Optional action routed through existing command authority.
    pub action: Option<ToastActionProjection>,
    /// Whether the toast should remain visible until explicitly dismissed.
    pub sticky: bool,
}

/// Bounded foreground notification stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastStackProjection {
    /// Visible notification cards.
    pub visible: Vec<ToastProjection>,
    /// Count of additional non-dismissed notifications hidden by the visible cap.
    pub overflow_count: usize,
}

impl ToastStackProjection {
    /// Build a bounded toast stack from shell status messages.
    pub fn from_status_messages(
        messages: &[StatusMessageProjection],
        dismissed_ids: &[u64],
    ) -> Self {
        Self::from_status_messages_with_verbosity(
            messages,
            dismissed_ids,
            ToastVerbosityProjection::WarningsAndErrors,
        )
    }

    /// Build a bounded toast stack from shell status messages using a user verbosity preference.
    pub fn from_status_messages_with_verbosity(
        messages: &[StatusMessageProjection],
        dismissed_ids: &[u64],
        verbosity: ToastVerbosityProjection,
    ) -> Self {
        let mut toasts = messages
            .iter()
            .filter(|message| toast_severity_included(message.severity, verbosity))
            .map(ToastProjection::from_status_message)
            .filter(|toast| !dismissed_ids.contains(&toast.id))
            .collect::<Vec<_>>();
        toasts.reverse();
        let overflow_count = toasts.len().saturating_sub(TOAST_VISIBLE_LIMIT);
        toasts.truncate(TOAST_VISIBLE_LIMIT);
        Self {
            visible: toasts,
            overflow_count,
        }
    }

    /// Empty toast stack.
    pub fn empty() -> Self {
        Self {
            visible: Vec::new(),
            overflow_count: 0,
        }
    }
}

fn toast_severity_included(severity: StatusSeverity, verbosity: ToastVerbosityProjection) -> bool {
    match verbosity {
        ToastVerbosityProjection::ErrorsOnly => severity == StatusSeverity::Error,
        ToastVerbosityProjection::WarningsAndErrors => severity != StatusSeverity::Info,
        ToastVerbosityProjection::All => true,
    }
}

impl Default for ToastStackProjection {
    fn default() -> Self {
        Self::empty()
    }
}

impl ToastProjection {
    /// Build a toast from an existing status message.
    pub fn from_status_message(message: &StatusMessageProjection) -> Self {
        let mut parts = message.message.splitn(2, ':');
        let first = parts.next().unwrap_or("").trim();
        let second = parts.next().map(str::trim).filter(|body| !body.is_empty());
        let title = if first.is_empty() {
            severity_label(message.severity).to_string()
        } else {
            first.to_string()
        };
        let body = second.map(ToString::to_string);
        Self {
            id: toast_id(message.severity, &message.message),
            severity: message.severity,
            title,
            body,
            action: None,
            sticky: message.severity == StatusSeverity::Error,
        }
    }
}

fn severity_label(severity: StatusSeverity) -> &'static str {
    match severity {
        StatusSeverity::Info => "Info",
        StatusSeverity::Warning => "Warning",
        StatusSeverity::Error => "Error",
    }
}

fn toast_id(severity: StatusSeverity, message: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash ^= match severity {
        StatusSeverity::Info => 0,
        StatusSeverity::Warning => 1,
        StatusSeverity::Error => 2,
    };
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    for byte in message.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Projection snapshot provided to the shell by the application layer.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellProjectionSnapshot {
    /// Layout projection.
    pub layout_projection: ShellLayoutProjection,
    /// App-owned product mode used by projection and dock filtering.
    pub product_mode: DockMode,
    /// Explorer projection.
    pub explorer_projection: ExplorerProjection,
    /// Active buffer projection.
    pub active_buffer_projection: ActiveBufferProjection,
    /// Status message projections.
    pub status_messages: Vec<StatusMessageProjection>,
    /// Command palette projection supplied by the application layer.
    pub palette_projection: PaletteProjection,
    /// Command registry projection supplied by the application layer.
    pub command_registry_projection: CommandRegistryProjection,
    /// App-owned workbench settings projection.
    pub settings_projection: SettingsProjection,
    /// Proposal ledger projection supplied by the application layer.
    pub proposal_ledger_projection: ProposalLedgerProjection,
    /// Artifact ledger projection supplied by the application layer.
    pub artifact_ledger_projection: ArtifactLedgerProjection,
    /// Verification-run projection supplied by the application layer.
    pub verification_run_projection: VerificationRunProjection,
    /// System graph summary projection supplied by the application layer.
    pub system_graph_projection: SystemGraphProjection,
    /// Trust-layer context manifest projection supplied by the application layer.
    pub context_manifest_projection: ContextManifestProjection,
    /// Trust-layer privacy inspector projection supplied by the application layer.
    pub privacy_inspector_projection: PrivacyInspectorProjection,
    /// Trust-layer permission budget projection supplied by the application layer.
    pub permission_budget_projection: PermissionBudgetProjection,
    /// Trust-layer approval checklist projection supplied by the application layer.
    pub approval_checklist_projection: ProposalApprovalChecklistProjection,
    /// Trust-layer checkpoint/rollback projection supplied by the application layer.
    pub checkpoint_rollback_projection: CheckpointRollbackProjection,
    /// Assisted-AI projection supplied by the application layer.
    pub assisted_ai_projection: AssistedAiProjection,
    /// Assist inline prediction projection supplied by the application layer.
    pub assist_inline_prediction_projection: AssistInlinePredictionProjection,
    /// Delegated-task plan projection supplied by the application layer.
    pub delegated_task_projection: DelegatedTaskProjection,
    /// Legion workflow projection supplied by the application layer.
    pub legion_workflow_projection: LegionWorkflowProjection,
    /// Plugin contribution projections supplied by the application layer.
    pub plugin_contribution_projections: Vec<PluginContributionProjection>,
    /// Collaboration presence projections supplied by the application layer.
    pub collaboration_presence_projections: Vec<CollaborationPresenceProjection>,
    /// Collaboration GUI summary projection supplied by the application layer.
    pub collaboration_gui_projection: CollaborationGuiProjection,
    /// Static remote workspace GUI summary projection.
    pub remote_gui_projection: RemoteGuiProjection,
    /// Static daily-editing projection.
    pub daily_editing_projection: DailyEditingProjection,
    /// Static multibuffer excerpt projection.
    pub excerpt_surface_projection: ExcerptSurfaceProjection,
    /// Static search projection.
    pub search_projection: SearchProjection,
    /// Structural search projection supplied by the application layer.
    pub structural_search_projection: StructuralSearchProjection,
    /// Git status, syntactic diff, blame, graph, and conflict projection supplied by app layer.
    pub git_projection: GitProjection,
    /// Debugger projection supplied by the application layer.
    pub debug_projection: DebugProjection,
    /// Language tooling projection supplied by the application layer.
    pub language_tooling_projection: LanguageToolingProjection,
    /// Terminal panel projection supplied by the application layer.
    pub terminal_panel_projection: TerminalPanelProjection,
}

/// Command parsing errors surfaced by projection-only shell input handling.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ShellCommandError {
    /// A command requires an active buffer projection, but none is present.
    #[error("active buffer projection is missing")]
    ActiveBufferMissing,
    /// A command supplied a range with start after end.
    #[error("command range start must be <= end")]
    InvalidRange,
    /// A terminal command requires an active terminal session projection.
    #[error("active terminal session projection is missing")]
    ActiveTerminalSessionMissing,
    /// A debug command requires an active debug session projection.
    #[error("active debug session projection is missing")]
    ActiveDebugSessionMissing,
    /// A context-manifest command targeted an unknown item.
    #[error("context manifest item is missing")]
    ContextManifestItemMissing,
}

/// Projection-only IDE shell state.
#[derive(Debug)]
pub struct Shell {
    /// Projection-only layout state.
    pub layout_projection: ShellLayoutProjection,
    /// App-owned product mode used by projection and dock filtering.
    pub product_mode: DockMode,
    /// Projection-only explorer state.
    pub explorer_projection: ExplorerProjection,
    /// Projection-only active buffer state.
    pub active_buffer_projection: ActiveBufferProjection,
    /// Projected status messages.
    pub status_messages: Vec<StatusMessageProjection>,
    /// App-owned command palette projection.
    pub palette_projection: PaletteProjection,
    /// Static command registry projection.
    pub command_registry_projection: CommandRegistryProjection,
    /// App-owned workbench settings projection.
    pub settings_projection: SettingsProjection,
    /// Static proposal ledger projection.
    pub proposal_ledger_projection: ProposalLedgerProjection,
    /// Static artifact ledger projection.
    pub artifact_ledger_projection: ArtifactLedgerProjection,
    /// Static verification-run projection.
    pub verification_run_projection: VerificationRunProjection,
    /// Static system graph projection.
    pub system_graph_projection: SystemGraphProjection,
    /// Static trust-layer context manifest projection.
    pub context_manifest_projection: ContextManifestProjection,
    /// Static trust-layer privacy inspector projection.
    pub privacy_inspector_projection: PrivacyInspectorProjection,
    /// Static trust-layer permission budget projection.
    pub permission_budget_projection: PermissionBudgetProjection,
    /// Static trust-layer approval checklist projection.
    pub approval_checklist_projection: ProposalApprovalChecklistProjection,
    /// Static trust-layer checkpoint/rollback projection.
    pub checkpoint_rollback_projection: CheckpointRollbackProjection,
    /// Static assisted-AI projection.
    pub assisted_ai_projection: AssistedAiProjection,
    /// Static Assist inline prediction projection.
    pub assist_inline_prediction_projection: AssistInlinePredictionProjection,
    /// Static delegated-task plan projection.
    pub delegated_task_projection: DelegatedTaskProjection,
    /// Static Legion workflow projection.
    pub legion_workflow_projection: LegionWorkflowProjection,
    /// Static plugin contribution projections.
    pub plugin_contribution_projections: Vec<PluginContributionProjection>,
    /// Static collaboration presence projections.
    pub collaboration_presence_projections: Vec<CollaborationPresenceProjection>,
    /// Static collaboration GUI summary projection.
    pub collaboration_gui_projection: CollaborationGuiProjection,
    /// Static remote workspace GUI summary projection.
    pub remote_gui_projection: RemoteGuiProjection,
    /// Static daily-editing projection.
    pub daily_editing_projection: DailyEditingProjection,
    /// Static multibuffer excerpt projection.
    pub excerpt_surface_projection: ExcerptSurfaceProjection,
    /// Static search projection.
    pub search_projection: SearchProjection,
    /// Static structural search projection.
    pub structural_search_projection: StructuralSearchProjection,
    /// Static git projection.
    pub git_projection: GitProjection,
    /// Static debugger projection.
    pub debug_projection: DebugProjection,
    /// Static language tooling projection.
    pub language_tooling_projection: LanguageToolingProjection,
    /// Static terminal panel projection.
    pub terminal_panel_projection: TerminalPanelProjection,
    /// Command dispatch intents emitted by input parsing.
    pub command_dispatch_intents: Vec<CommandDispatchIntent>,
}

impl Shell {
    /// Create a shell from a projection snapshot.
    pub fn new(snapshot: ShellProjectionSnapshot) -> Self {
        Self {
            layout_projection: snapshot.layout_projection,
            product_mode: snapshot.product_mode,
            explorer_projection: snapshot.explorer_projection,
            active_buffer_projection: snapshot.active_buffer_projection,
            status_messages: snapshot.status_messages,
            palette_projection: snapshot.palette_projection,
            command_registry_projection: snapshot.command_registry_projection,
            settings_projection: snapshot.settings_projection,
            proposal_ledger_projection: snapshot.proposal_ledger_projection,
            artifact_ledger_projection: snapshot.artifact_ledger_projection,
            verification_run_projection: snapshot.verification_run_projection,
            system_graph_projection: snapshot.system_graph_projection,
            context_manifest_projection: snapshot.context_manifest_projection,
            privacy_inspector_projection: snapshot.privacy_inspector_projection,
            permission_budget_projection: snapshot.permission_budget_projection,
            approval_checklist_projection: snapshot.approval_checklist_projection,
            checkpoint_rollback_projection: snapshot.checkpoint_rollback_projection,
            assisted_ai_projection: snapshot.assisted_ai_projection,
            assist_inline_prediction_projection: snapshot.assist_inline_prediction_projection,
            delegated_task_projection: snapshot.delegated_task_projection,
            legion_workflow_projection: snapshot.legion_workflow_projection,
            plugin_contribution_projections: snapshot.plugin_contribution_projections,
            collaboration_presence_projections: snapshot.collaboration_presence_projections,
            collaboration_gui_projection: snapshot.collaboration_gui_projection,
            remote_gui_projection: snapshot.remote_gui_projection,
            daily_editing_projection: snapshot.daily_editing_projection,
            excerpt_surface_projection: snapshot.excerpt_surface_projection,
            search_projection: snapshot.search_projection,
            structural_search_projection: snapshot.structural_search_projection,
            git_projection: snapshot.git_projection,
            debug_projection: snapshot.debug_projection,
            language_tooling_projection: snapshot.language_tooling_projection,
            terminal_panel_projection: snapshot.terminal_panel_projection,
            command_dispatch_intents: Vec::new(),
        }
    }

    /// Create an empty projection-only shell.
    pub fn empty(title: impl Into<String>) -> Self {
        Self::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain(title),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: empty_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        })
    }

    /// Return a cloned shell projection snapshot.
    pub fn projection_snapshot(&self) -> ShellProjectionSnapshot {
        ShellProjectionSnapshot {
            product_mode: self.product_mode,
            layout_projection: self.layout_projection.clone(),
            explorer_projection: self.explorer_projection.clone(),
            active_buffer_projection: self.active_buffer_projection.clone(),
            status_messages: self.status_messages.clone(),
            palette_projection: self.palette_projection.clone(),
            command_registry_projection: self.command_registry_projection.clone(),
            settings_projection: self.settings_projection.clone(),
            proposal_ledger_projection: self.proposal_ledger_projection.clone(),
            artifact_ledger_projection: self.artifact_ledger_projection.clone(),
            verification_run_projection: self.verification_run_projection.clone(),
            system_graph_projection: self.system_graph_projection.clone(),
            context_manifest_projection: self.context_manifest_projection.clone(),
            privacy_inspector_projection: self.privacy_inspector_projection.clone(),
            permission_budget_projection: self.permission_budget_projection.clone(),
            approval_checklist_projection: self.approval_checklist_projection.clone(),
            checkpoint_rollback_projection: self.checkpoint_rollback_projection.clone(),
            assisted_ai_projection: self.assisted_ai_projection.clone(),
            assist_inline_prediction_projection: self.assist_inline_prediction_projection.clone(),
            delegated_task_projection: self.delegated_task_projection.clone(),
            legion_workflow_projection: self.legion_workflow_projection.clone(),
            plugin_contribution_projections: self.plugin_contribution_projections.clone(),
            collaboration_presence_projections: self.collaboration_presence_projections.clone(),
            collaboration_gui_projection: self.collaboration_gui_projection.clone(),
            remote_gui_projection: self.remote_gui_projection.clone(),
            daily_editing_projection: self.daily_editing_projection.clone(),
            excerpt_surface_projection: self.excerpt_surface_projection.clone(),
            search_projection: self.search_projection.clone(),
            structural_search_projection: self.structural_search_projection.clone(),
            git_projection: self.git_projection.clone(),
            debug_projection: self.debug_projection.clone(),
            language_tooling_projection: self.language_tooling_projection.clone(),
            terminal_panel_projection: self.terminal_panel_projection.clone(),
        }
    }

    /// Replace all render projections at once.
    pub fn replace_projection_snapshot(&mut self, snapshot: ShellProjectionSnapshot) {
        self.layout_projection = snapshot.layout_projection;
        self.product_mode = snapshot.product_mode;
        self.explorer_projection = snapshot.explorer_projection;
        self.active_buffer_projection = snapshot.active_buffer_projection;
        self.status_messages = snapshot.status_messages;
        self.palette_projection = snapshot.palette_projection;
        self.command_registry_projection = snapshot.command_registry_projection;
        self.settings_projection = snapshot.settings_projection;
        self.proposal_ledger_projection = snapshot.proposal_ledger_projection;
        self.artifact_ledger_projection = snapshot.artifact_ledger_projection;
        self.verification_run_projection = snapshot.verification_run_projection;
        self.system_graph_projection = snapshot.system_graph_projection;
        self.context_manifest_projection = snapshot.context_manifest_projection;
        self.privacy_inspector_projection = snapshot.privacy_inspector_projection;
        self.permission_budget_projection = snapshot.permission_budget_projection;
        self.approval_checklist_projection = snapshot.approval_checklist_projection;
        self.checkpoint_rollback_projection = snapshot.checkpoint_rollback_projection;
        self.assisted_ai_projection = snapshot.assisted_ai_projection;
        self.assist_inline_prediction_projection = snapshot.assist_inline_prediction_projection;
        self.delegated_task_projection = snapshot.delegated_task_projection;
        self.legion_workflow_projection = snapshot.legion_workflow_projection;
        self.plugin_contribution_projections = snapshot.plugin_contribution_projections;
        self.collaboration_presence_projections = snapshot.collaboration_presence_projections;
        self.collaboration_gui_projection = snapshot.collaboration_gui_projection;
        self.remote_gui_projection = snapshot.remote_gui_projection;
        self.daily_editing_projection = snapshot.daily_editing_projection;
        self.excerpt_surface_projection = snapshot.excerpt_surface_projection;
        self.search_projection = snapshot.search_projection;
        self.structural_search_projection = snapshot.structural_search_projection;
        self.git_projection = snapshot.git_projection;
        self.debug_projection = snapshot.debug_projection;
        self.language_tooling_projection = snapshot.language_tooling_projection;
        self.terminal_panel_projection = snapshot.terminal_panel_projection;
    }

    /// Drain queued command-dispatch intents.
    pub fn drain_command_dispatch_intents(&mut self) -> Vec<CommandDispatchIntent> {
        self.command_dispatch_intents.drain(..).collect()
    }

    /// Render basic status and file content.
    pub fn render(&self) {
        print!("\x1b[2J\x1b[H");
        println!("{}", self.layout_projection.layout.title);
        println!(
            "Mode: {:?} | {}x{}",
            self.layout_projection.mode,
            self.layout_projection.layout.width,
            self.layout_projection.layout.height
        );
        println!(
            "{}",
            "-".repeat(self.layout_projection.layout.width as usize)
        );

        if self.active_buffer_projection.degraded {
            println!("<Degraded Mode: Large File>");
        }
        if !self.daily_editing_projection.tabs.tabs.is_empty() {
            let rows = self
                .daily_editing_projection
                .tabs
                .tabs
                .iter()
                .map(|tab| {
                    format!(
                        "{}{}{}",
                        if tab.active { "*" } else { "" },
                        tab.title,
                        if tab.dirty { " +" } else { "" }
                    )
                })
                .collect::<Vec<_>>();
            println!("Tabs: {}", rows.join(" | "));
        }
        if let Some(prompt) = &self.daily_editing_projection.close_dirty_prompt {
            println!("Close dirty: {}", prompt.message);
        }

        if let Some(text) = self.active_buffer_projection.small_buffer_text() {
            println!("{}", text);
        } else if let Some(viewport) = &self.active_buffer_projection.viewport {
            for slice in &viewport.line_slices {
                println!("{}", slice.visible_text);
            }
        } else {
            println!("<no active buffer>");
        }

        println!(
            "{}",
            "-".repeat(self.layout_projection.layout.width as usize)
        );
        let path = self
            .active_buffer_projection
            .file_path
            .as_ref()
            .map(|path| path.0.as_str())
            .unwrap_or("<no active file>");
        println!("Path: {}", path);
        if !self.command_registry_projection.commands.is_empty() {
            let registry = &self.command_registry_projection;
            let enabled_count = registry
                .commands
                .iter()
                .filter(|command| command.enabled)
                .count();
            println!(
                "Command registry {} | commands={} enabled={} omitted={}",
                registry.projection_id,
                registry.commands.len(),
                enabled_count,
                registry.omitted_command_count
            );
            for command in &registry.commands {
                println!(
                    "- command {} scope={} enabled={} risk={:?} target={:?}",
                    command.command_id,
                    command.scope,
                    command.enabled,
                    command.risk_label,
                    command.target
                );
            }
        }
        if !self.proposal_ledger_projection.rows.is_empty() {
            println!("Proposals:");
            for row in &self.proposal_ledger_projection.rows {
                println!(
                    "#{} [{}] {} | risk={:?} privacy={:?} rollback={:?} targets={} hunks={} redacted={}",
                    row.proposal_id.0,
                    row.lifecycle.label,
                    row.title,
                    row.risk_label,
                    row.privacy_label,
                    row.rollback,
                    row.diff_summary.target_count,
                    row.diff_summary.hunk_count,
                    row.diff_summary.full_source_redacted
                );
            }
        }
        if !self.artifact_ledger_projection.rows.is_empty() {
            let ledger = &self.artifact_ledger_projection;
            println!(
                "Artifact ledger {} | artifacts={} omitted={}",
                ledger.projection_id,
                ledger.rows.len(),
                ledger.omitted_row_count
            );
            for row in &ledger.rows {
                println!(
                    "- artifact {} kind={:?} state={} raw_retained={} risk={:?} privacy={:?}",
                    row.artifact_id,
                    row.kind,
                    row.state_label,
                    row.raw_payload_retained,
                    row.risk_label,
                    row.privacy_label
                );
            }
        }
        if !self.verification_run_projection.rows.is_empty() {
            let verification = &self.verification_run_projection;
            println!(
                "Verification runs {} | runs={} omitted={}",
                verification.projection_id,
                verification.rows.len(),
                verification.omitted_row_count
            );
            for row in &verification.rows {
                println!(
                    "- verification {} state={:?} class={} command_redacted={} evidence={:?}",
                    row.run_id,
                    row.state,
                    row.command_class_label,
                    row.command_body_redacted,
                    row.evidence_artifact_id
                );
            }
        }
        if !self.system_graph_projection.nodes.is_empty()
            || !self.system_graph_projection.edges.is_empty()
        {
            let graph = &self.system_graph_projection;
            println!(
                "System graph {} | nodes={} edges={} omitted_nodes={} omitted_edges={}",
                graph.projection_id,
                graph.nodes.len(),
                graph.edges.len(),
                graph.omitted_node_count,
                graph.omitted_edge_count
            );
        }
        if !self.context_manifest_projection.manifest.items.is_empty() {
            let manifest = &self.context_manifest_projection.manifest;
            let excluded_count = manifest
                .items
                .iter()
                .filter(|item| {
                    item.inclusion == legion_protocol::ContextManifestInclusionState::Excluded
                })
                .count();
            let selected_item_id = self
                .context_manifest_projection
                .selected_item_id
                .as_deref()
                .unwrap_or("none");
            println!(
                "Context manifest {} | items={} excluded={} selected={} omitted={} risk={:?} privacy={:?} egress={:?}",
                manifest.manifest_id,
                manifest.items.len(),
                excluded_count,
                selected_item_id,
                manifest.omitted_item_count,
                manifest.risk_label,
                manifest.privacy_label,
                manifest.egress
            );
            for item in &manifest.items {
                println!(
                    "- {} {:?} {:?} ranges={} hashes={} risk={:?} privacy={:?}",
                    item.item_id,
                    item.kind,
                    item.inclusion,
                    item.ranges.len(),
                    item.hashes.len(),
                    item.risk_label,
                    item.privacy_label
                );
            }
        }
        if !self.privacy_inspector_projection.records.is_empty() {
            let inspector = &self.privacy_inspector_projection;
            println!(
                "Privacy inspector {} | records={} denied={} redacted={} egress={} high_risk={}",
                inspector.inspector_id,
                inspector.records.len(),
                inspector.denied_record_count,
                inspector.redacted_record_count,
                inspector.external_egress_record_count,
                inspector.high_risk_record_count
            );
            for record in &inspector.records {
                println!(
                    "- {} {:?} {:?} ranges={} hashes={} risk={:?} privacy={:?} redaction={:?}",
                    record.exposure_id,
                    record.source_kind,
                    record.inclusion,
                    record.ranges.len(),
                    record.hashes.len(),
                    record.risk_label,
                    record.privacy_label,
                    record.redaction_state
                );
            }
        }
        if !self.permission_budget_projection.budgets.is_empty()
            || !self.permission_budget_projection.evaluations.is_empty()
        {
            let budgets = &self.permission_budget_projection;
            println!(
                "Permission budgets {} | budgets={} denied={} depleted={} refused_evaluations={}",
                budgets.projection_id,
                budgets.budgets.len(),
                budgets.denied_budget_count,
                budgets.depleted_budget_count,
                budgets.refused_evaluation_count
            );
            for budget in &budgets.budgets {
                println!(
                    "- {} {:?} state={:?} used={} ceiling={:?} risk={:?}",
                    budget.budget_id,
                    budget.action_class,
                    budget.state,
                    budget.usage.used,
                    budget.usage.ceiling,
                    budget.risk_label
                );
            }
        }
        if !self.approval_checklist_projection.gates.is_empty() {
            let checklist = &self.approval_checklist_projection;
            println!(
                "Approval checklist {} | proposal={} ready={} blockers={}",
                checklist.checklist_id,
                checklist.proposal_id.0,
                checklist.ready_for_approval,
                checklist.blockers.len()
            );
            for gate in &checklist.gates {
                println!(
                    "- {:?} status={:?} risk={:?} privacy={:?} reasons={}",
                    gate.gate,
                    gate.status,
                    gate.risk_label,
                    gate.privacy_label,
                    gate.reasons.len()
                );
            }
        }
        if !self.checkpoint_rollback_projection.targets.is_empty()
            || !self
                .checkpoint_rollback_projection
                .rollback
                .limitations
                .is_empty()
        {
            let rollback = &self.checkpoint_rollback_projection;
            println!(
                "Checkpoint/Rollback {} | proposal={} checkpoint_available={} rollback={:?} targets={} limitations={}",
                rollback.projection_id,
                rollback.proposal_id.0,
                rollback.checkpoint.available,
                rollback.rollback.availability,
                rollback.targets.len(),
                rollback.rollback.limitations.len()
            );
        }
        if !self.assisted_ai_projection.providers.is_empty()
            || !self.assisted_ai_projection.requests.is_empty()
            || !self.assisted_ai_projection.proposal_previews.is_empty()
        {
            let assisted = &self.assisted_ai_projection;
            println!(
                "Assisted AI {} | providers={} requests={} refusals={} preview_ready={} invocation={:?}",
                assisted.projection_id,
                assisted.provider_count,
                assisted.request_count,
                assisted.refusal_count,
                assisted.preview_ready_count,
                assisted.provider_invocation
            );
            for provider in &assisted.providers {
                println!(
                    "- provider {} class={:?} availability={:?} ops={} model_labels={} tool_labels={} risk={:?} privacy={:?}",
                    provider.provider_id,
                    provider.provider_class,
                    provider.availability,
                    provider.supported_operation_count,
                    provider.model_capability_label_count,
                    provider.tool_capability_label_count,
                    provider.risk_label,
                    provider.privacy_label
                );
            }
            for route in &assisted.routes {
                println!(
                    "- route {} provider={} op={:?} disposition={:?} invocation={:?} refused_budgets={}",
                    route.request_id,
                    route.provider_id,
                    route.operation_class,
                    route.disposition,
                    route.provider_invocation,
                    route.refused_permission_budget_evaluation_count
                );
            }
            for preview in &assisted.proposal_previews {
                println!(
                    "- preview {} proposal={} readiness={:?} ready_preview={} ready_approval={} ready_apply={} targets={} hunks={} preconditions={}",
                    preview.preview_id,
                    preview.proposal_id.0,
                    preview.readiness,
                    preview.ready_for_preview,
                    preview.ready_for_approval,
                    preview.ready_for_apply,
                    preview.target_coverage.targets.len(),
                    preview.diff_summary.hunk_count,
                    preview.preconditions.core_preconditions_present
                );
            }
        }
        if self.assist_inline_prediction_projection.has_activity() {
            let assist = &self.assist_inline_prediction_projection;
            println!(
                "Assist inline predictions | active={} rows={} in_flight={} stale={} generated_at={}",
                assist.active_prediction.is_some(),
                assist.rows.len(),
                assist.request_in_flight,
                assist.stale_prediction_count,
                assist.generated_at.0
            );
            if let Some(prediction) = &assist.active_prediction {
                println!(
                    "- ghost {} provider={} status={:?} latency={:?} stale={} range={} preview={}",
                    prediction.prediction_id,
                    prediction.provider_label,
                    prediction.status,
                    prediction.latency_ms,
                    prediction.stale,
                    prediction.apply_range_label,
                    prediction
                        .replacement_preview_label
                        .as_deref()
                        .unwrap_or("<none>")
                );
            }
        }
        if !self.delegated_task_projection.plan_rows.is_empty()
            || !self.delegated_task_projection.blockers.is_empty()
            || !self.delegated_task_projection.refusals.is_empty()
        {
            let delegated = &self.delegated_task_projection;
            println!(
                "Delegated tasks {} | plans={} blocked={} refused={} activation={:?}",
                delegated.projection_id,
                delegated.plan_count,
                delegated.blocked_plan_count,
                delegated.refused_plan_count,
                delegated.runtime_activation
            );
            for row in &delegated.plan_rows {
                println!(
                    "- plan {} state={:?} readiness={:?} steps={} targets={} blockers={} refusals={} previews={} risk={:?} privacy={:?}",
                    row.plan_id.0,
                    row.plan_state,
                    row.readiness,
                    row.step_count,
                    row.affected_target_count,
                    row.blocker_count,
                    row.refusal_count,
                    row.proposal_preview_link_count,
                    row.risk_label,
                    row.privacy_label
                );
            }
            for step in &delegated.step_summaries {
                println!(
                    "- step {} plan={} op={:?} state={:?} deps={} targets={} proposal={:?} blockers={}",
                    step.step_id.0,
                    step.plan_id.0,
                    step.operation_class,
                    step.state,
                    step.dependency_count,
                    step.target_count,
                    step.proposal_id.map(|proposal| proposal.0),
                    step.blocker_count
                );
            }
        }
        if !self.legion_workflow_projection.rows.is_empty() {
            let workflows = &self.legion_workflow_projection;
            println!(
                "Legion workflows {} | sessions={} omitted={} autonomous_merge=unsupported_until_approval",
                workflows.projection_id, workflows.total_session_count, workflows.omitted_row_count
            );
            for row in &workflows.rows {
                println!(
                    "- workflow {} state={:?} workers={} provider_routes={} dependencies={} conflicts={} verification={}/{} signoff={}/{} proposals={} directive_artifact={} spec_artifact={} task_graph_artifact={} merge={:?} labels={}",
                    row.session_id.0,
                    row.lifecycle_state,
                    row.worker_count,
                    row.provider_route_required_count,
                    row.dependency_count,
                    row.unresolved_conflict_count,
                    row.passed_verification_count,
                    row.verification_gate_count,
                    row.signed_off_count,
                    row.sign_off_count,
                    row.linked_proposals.len(),
                    row.directive_artifact_id.as_deref().unwrap_or("<none>"),
                    row.spec_artifact_id.as_deref().unwrap_or("<none>"),
                    row.task_graph_artifact_id.as_deref().unwrap_or("<none>"),
                    row.merge_readiness.state,
                    row.display_safe_labels.join("|")
                );
            }
        }
        if self.language_tooling_projection.buffer_id.is_some()
            || !self.language_tooling_projection.operations.is_empty()
            || !self.language_tooling_projection.problems.is_empty()
        {
            let language = &self.language_tooling_projection;
            println!(
                "Language tooling {:?} | problems={} completions={} definitions={} references={} outline={} stale={} cancelled={}",
                language.status,
                language.problems.len(),
                language.completions.len(),
                language.definitions.len(),
                language.references.len(),
                language.outline.len(),
                language.stale_result_count,
                language.cancellation_count
            );
            if let Some(hover) = &language.hover {
                println!("- hover {} {}", hover.label, hover.summary);
            }
            for operation in &language.operations {
                println!(
                    "- operation {} {:?} {:?} proposal={:?}",
                    operation.operation_id,
                    operation.kind,
                    operation.status,
                    operation.proposal_id.map(|proposal| proposal.0)
                );
            }
        }
        if self.terminal_panel_projection.active_session_id.is_some()
            || !self.terminal_panel_projection.output_rows.is_empty()
            || self.terminal_panel_projection.last_denial.is_some()
        {
            let terminal = &self.terminal_panel_projection;
            println!(
                "Terminal {:?} | session={:?} rows={} omitted={} matches={}",
                terminal.status.kind,
                terminal.active_session_id.map(|session| session.0),
                terminal.output_rows.len(),
                terminal.scrollback.omitted_row_count,
                terminal.search.match_count
            );
            if let Some(denial) = &terminal.last_denial {
                println!("- denial {}", denial);
            }
            for row in &terminal.output_rows {
                println!("- [{}] {}", row.sequence.0, row.redacted_payload);
            }
        }
        if self.debug_projection.active_session_id.is_some()
            || !self.debug_projection.configurations.is_empty()
            || !self.debug_projection.breakpoints.is_empty()
        {
            let debug = &self.debug_projection;
            println!(
                "Debug {:?} | session={:?} configs={} breakpoints={} frames={} variables={} watches={} console={}",
                debug.status.kind,
                debug
                    .active_session_id
                    .as_ref()
                    .map(|session| session.0.as_str()),
                debug.configurations.len(),
                debug.breakpoints.len(),
                debug.stack_frames.len(),
                debug.variables.len(),
                debug.watches.len(),
                debug.console.len()
            );
            for config in &debug.configurations {
                println!(
                    "- debug config {} adapter={} program={}",
                    config.configuration_id.0, config.adapter_type, config.program_label
                );
            }
            for breakpoint in &debug.breakpoints {
                println!(
                    "- debug breakpoint {} {}:{} verified={}",
                    breakpoint.breakpoint_id.0,
                    breakpoint.path.0,
                    breakpoint.line,
                    breakpoint.verified
                );
            }
            for frame in &debug.stack_frames {
                println!("- debug frame {} {}", frame.frame_id, frame.name);
            }
            for variable in &debug.variables {
                println!(
                    "- debug variable {}={}",
                    variable.name, variable.value_label
                );
            }
            for watch in &debug.watches {
                println!(
                    "- debug watch {}={}",
                    watch.expression_label, watch.value_label
                );
            }
            for entry in &debug.console {
                println!("- debug console {}", entry.message_label);
            }
        }
        println!(
            "Commands: :mode manual|assist|delegate|automate | :i text | :d start,end | :r start,end,text | :w | :wa | :tab id | :tab | :assist-predict offset | :assist-dismiss | :assist-cancel | :close id | :hover | :completion | :definition | :references | :outline | :format | :rename name | :code-action id | :debug-configs | :debug-launch id | :debug-step over | :term-launch label | :term-input text | :term-close | :plugin id command | :ai-start label | :ai-explain label | :ai-propose label | :u | :redo | :q"
        );
    }

    /// Parse a command and emit a typed dispatch intent without mutating editor or workspace state.
    pub fn handle_command(
        &mut self,
        input: &str,
    ) -> Result<Option<CommandDispatchIntent>, ShellCommandError> {
        let trimmed = input.trim();
        if trimmed == ":q" {
            return Ok(Some(self.push_intent(CommandDispatchIntent::Quit)));
        }
        if let Some(payload) = trimmed.strip_prefix(":mode") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::SetProductMode {
                    mode: parse_dock_mode(payload.trim()),
                },
            )));
        }
        if trimmed == ":u" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Undo { buffer_id }),
            ));
        }
        if trimmed == ":redo" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Redo { buffer_id }),
            ));
        }
        if trimmed == ":w" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::Save { buffer_id }),
            ));
        }
        if trimmed == ":wa" {
            return Ok(Some(self.push_intent(CommandDispatchIntent::SaveAll)));
        }
        if let Some(payload) = trimmed.strip_prefix(":assist-predict") {
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim());
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestAssistInlinePrediction {
                    buffer_id,
                    position,
                },
            )));
        }
        if trimmed == ":tab" || trimmed == ":assist-accept" {
            let buffer_id = self.active_buffer_id()?;
            let prediction_id = self.active_assist_prediction_id();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::AcceptAssistInlinePrediction {
                    buffer_id,
                    prediction_id,
                },
            )));
        }
        if trimmed == ":assist-dismiss" {
            let buffer_id = self.active_buffer_id()?;
            let prediction_id = self.active_assist_prediction_id();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::DismissAssistInlinePrediction {
                    buffer_id,
                    prediction_id,
                },
            )));
        }
        if trimmed == ":assist-cancel" {
            let buffer_id = self.active_buffer_id()?;
            let prediction_id = self.active_assist_prediction_id();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CancelAssistInlinePrediction {
                    buffer_id,
                    prediction_id,
                },
            )));
        }
        if let Some(buffer_id) = parse_buffer_id(trimmed.strip_prefix(":tab ")) {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::SwitchTab { buffer_id }),
            ));
        }
        if let Some(buffer_id) = parse_buffer_id(trimmed.strip_prefix(":close ")) {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::CloseTab { buffer_id }),
            ));
        }
        if let Some(item_id) = trimmed.strip_prefix(":context-manifest-select ") {
            let item_id = item_id.trim();
            if self
                .context_manifest_projection
                .manifest
                .items
                .iter()
                .any(|item| item.item_id == item_id)
            {
                self.context_manifest_projection.selected_item_id = Some(item_id.to_string());
                return Ok(None);
            }
            return Err(ShellCommandError::ContextManifestItemMissing);
        }
        if trimmed == ":context-manifest-clear" || trimmed == ":context-manifest-clear-selection" {
            self.context_manifest_projection.selected_item_id = None;
            return Ok(None);
        }
        if let Some(query) = trimmed.strip_prefix(":search ") {
            return Ok(Some(self.push_intent(CommandDispatchIntent::RunSearch {
                scope: SearchScopeProjection::ActiveFile,
                query: query.trim().to_string(),
                limit: 0,
            })));
        }
        if let Some(query) = trimmed.strip_prefix(":search-workspace ") {
            return Ok(Some(self.push_intent(CommandDispatchIntent::RunSearch {
                scope: SearchScopeProjection::Workspace,
                query: query.trim().to_string(),
                limit: 0,
            })));
        }
        if let Some(query_id) = trimmed.strip_prefix(":search-cancel ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CancelSearch {
                    query_id: query_id.trim().to_string(),
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":hover") {
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim());
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestHover {
                    buffer_id,
                    position,
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":completion") {
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim());
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestCompletion {
                    buffer_id,
                    position,
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":definition") {
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim());
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::GoToDefinition {
                    buffer_id,
                    position,
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":references") {
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim());
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::FindReferences {
                    buffer_id,
                    position,
                },
            )));
        }
        if trimmed == ":outline" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::RefreshOutline { buffer_id }),
            ));
        }
        if trimmed == ":format" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestFormattingProposal { buffer_id },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":rename ") {
            let buffer_id = self.active_buffer_id()?;
            let mut split = payload.splitn(2, ',');
            let first = split.next().unwrap_or_default().trim();
            let (position, new_name) = if let Some(name) = split.next() {
                (
                    first
                        .parse::<usize>()
                        .map(|offset| self.parse_pos(offset))
                        .unwrap_or_else(|_| self.parse_pos(0)),
                    name.trim(),
                )
            } else {
                (self.parse_pos(0), first)
            };
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestRenameProposal {
                    buffer_id,
                    position,
                    new_name: new_name.to_string(),
                },
            )));
        }
        if trimmed == ":organize-imports" {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestOrganizeImportsProposal { buffer_id },
            )));
        }
        if let Some(action_id) = trimmed.strip_prefix(":code-action ") {
            let buffer_id = self.active_buffer_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestCodeActionProposal {
                    buffer_id,
                    action_id: action_id.trim().to_string(),
                },
            )));
        }
        if let Some(operation_id) = trimmed.strip_prefix(":language-cancel ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CancelLanguageOperation {
                    operation_id: operation_id.trim().to_string(),
                },
            )));
        }
        if trimmed == ":git-refresh" {
            return Ok(Some(self.push_intent(CommandDispatchIntent::RefreshGit)));
        }
        if let Some(branch) = trimmed.strip_prefix(":git-switch-branch ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::SwitchGitBranch {
                    branch: branch.trim().to_string(),
                },
            )));
        }
        if let Some(branch) = trimmed.strip_prefix(":git-create-branch ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CreateGitBranch {
                    branch: branch.trim().to_string(),
                },
            )));
        }
        if let Some(branch) = trimmed.strip_prefix(":git-delete-branch ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::DeleteGitBranch {
                    branch: branch.trim().to_string(),
                },
            )));
        }
        if let Some(message) = trimmed.strip_prefix(":git-stash ") {
            let message = message.trim();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::StashGitChanges {
                    message: (!message.is_empty()).then(|| message.to_string()),
                },
            )));
        }
        if trimmed == ":git-push" {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::PushGitRemote {
                    remote: "origin".to_string(),
                },
            )));
        }
        if trimmed == ":git-prune-worktrees" {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::PruneGitWorktrees),
            ));
        }
        if let Some(path) = trimmed.strip_prefix(":git-remove-worktree ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RemoveGitWorktree {
                    path: path.trim().to_string(),
                },
            )));
        }
        if let Some(hunk_id) = trimmed.strip_prefix(":git-stage-hunk ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::StageGitHunk {
                    hunk_id: hunk_id.trim().to_string(),
                },
            )));
        }
        if let Some(hunk_id) = trimmed.strip_prefix(":git-unstage-hunk ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::UnstageGitHunk {
                    hunk_id: hunk_id.trim().to_string(),
                },
            )));
        }
        if let Some(path) = trimmed.strip_prefix(":git-accept-current-conflict ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ResolveGitConflict {
                    path: path.trim().to_string(),
                    choice: GitConflictChoiceProjection::AcceptCurrent,
                },
            )));
        }
        if let Some(path) = trimmed.strip_prefix(":git-accept-incoming-conflict ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ResolveGitConflict {
                    path: path.trim().to_string(),
                    choice: GitConflictChoiceProjection::AcceptIncoming,
                },
            )));
        }
        if trimmed == ":debug-configs" {
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::RefreshDebugConfigurations),
            ));
        }
        if let Some(configuration_id) = trimmed.strip_prefix(":debug-launch ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::LaunchDebugSession {
                    configuration_id: DebugConfigurationId(configuration_id.trim().to_string()),
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":debug-breakpoint ") {
            let buffer_id = self.active_buffer_id()?;
            let mut parts = payload.splitn(4, ',');
            let line = parts
                .next()
                .and_then(|value| value.trim().parse::<u32>().ok())
                .unwrap_or(0);
            let condition = non_empty_string(parts.next().map(str::trim));
            let hit_condition = non_empty_string(parts.next().map(str::trim));
            let log_message = non_empty_string(parts.next().map(str::trim));
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ToggleDebugBreakpoint {
                    buffer_id,
                    line,
                    condition,
                    hit_condition,
                    log_message,
                },
            )));
        }
        if let Some(kind) = trimmed.strip_prefix(":debug-step ") {
            let session_id = self.active_debug_session_id()?;
            return Ok(Some(self.push_intent(CommandDispatchIntent::DebugStep {
                session_id,
                kind: parse_debug_step_kind(kind.trim()),
            })));
        }
        if let Some(payload) = trimmed.strip_prefix(":debug-run-to-cursor ") {
            let session_id = self.active_debug_session_id()?;
            let buffer_id = self.active_buffer_id()?;
            let position = self.command_position(payload.trim());
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::DebugRunToCursor {
                    session_id,
                    buffer_id,
                    position,
                },
            )));
        }
        if let Some(expression_label) = trimmed.strip_prefix(":debug-eval ") {
            let session_id = self.active_debug_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::DebugEvaluateSelection {
                    session_id,
                    expression_label: expression_label.trim().to_string(),
                },
            )));
        }
        if let Some(expression_label) = trimmed.strip_prefix(":debug-watch ") {
            let session_id = self.active_debug_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::DebugAddWatch {
                    session_id,
                    expression_label: expression_label.trim().to_string(),
                },
            )));
        }
        if let Some(command_label) = trimmed.strip_prefix(":term-launch ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TerminalLaunch {
                    command_label: command_label.trim().to_string(),
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":term-input ") {
            let session_id = self.active_terminal_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TerminalInput {
                    session_id,
                    payload: payload.to_string(),
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":term-resize ") {
            let session_id = self.active_terminal_session_id()?;
            let mut split = payload.split_whitespace();
            let cols = split
                .next()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(80);
            let rows = split
                .next()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(24);
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TerminalResize {
                    session_id,
                    cols,
                    rows,
                },
            )));
        }
        if trimmed == ":term-kill" {
            let session_id = self.active_terminal_session_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::TerminalKill { session_id }),
            ));
        }
        if trimmed == ":term-close" {
            let session_id = self.active_terminal_session_id()?;
            return Ok(Some(
                self.push_intent(CommandDispatchIntent::TerminalClose { session_id }),
            ));
        }
        if trimmed == ":term-poll" {
            let session_id = self.active_terminal_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TerminalOutputPoll { session_id },
            )));
        }
        if let Some(query) = trimmed.strip_prefix(":term-search ") {
            let session_id = self.active_terminal_session_id()?;
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TerminalSearch {
                    session_id,
                    query: query.trim().to_string(),
                },
            )));
        }

        if let Some(label) = trimmed.strip_prefix(":ai-start") {
            let instruction_label = label.trim();
            return Ok(Some(self.push_intent(CommandDispatchIntent::StartAiRun {
                instruction_label: if instruction_label.is_empty() {
                    "phase4.local_proposal".to_string()
                } else {
                    instruction_label.to_string()
                },
            })));
        }
        if let Some(label) = trimmed.strip_prefix(":ai-explain") {
            let instruction_label = label.trim();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::StartAiExplain {
                    instruction_label: if instruction_label.is_empty() {
                        "phase5.local_explain".to_string()
                    } else {
                        instruction_label.to_string()
                    },
                },
            )));
        }
        if let Some(label) = trimmed.strip_prefix(":ai-propose") {
            let instruction_label = label.trim();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::StartAiProposal {
                    instruction_label: if instruction_label.is_empty() {
                        "phase5.local_proposal".to_string()
                    } else {
                        instruction_label.to_string()
                    },
                },
            )));
        }
        if let Some(prompt) = trimmed.strip_prefix(":delegate-chat") {
            let prompt_label = prompt.trim();
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::SendDelegateChat {
                    prompt_label: if prompt_label.is_empty() {
                        "delegate.context".to_string()
                    } else {
                        prompt_label.to_string()
                    },
                },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":delegate-hunk ") {
            let mut split = payload.splitn(3, ' ');
            let proposal_id = split
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .map(ProposalId);
            let hunk_id = split.next().unwrap_or_default().trim();
            let disposition = parse_delegate_hunk_disposition(split.next().unwrap_or_default());
            if let (Some(proposal_id), Some(disposition)) = (proposal_id, disposition)
                && !hunk_id.is_empty()
            {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::ReviewDelegateProposalHunk {
                        proposal_id,
                        hunk_id: hunk_id.to_string(),
                        disposition,
                    },
                )));
            }
        }
        if let Some(payload) = trimmed.strip_prefix(":delegate-permission ") {
            let mut split = payload.splitn(2, ' ');
            let request_id = split.next().unwrap_or_default().trim();
            let decision =
                parse_delegate_tool_permission_decision(split.next().unwrap_or_default());
            if !request_id.is_empty()
                && let Some(decision) = decision
            {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::RecordDelegateToolPermission {
                        request_id: request_id.to_string(),
                        decision,
                    },
                )));
            }
        }
        if let Some(run_id) = trimmed.strip_prefix(":ai-cancel ") {
            return Ok(Some(self.push_intent(CommandDispatchIntent::CancelAiRun {
                run_id: AgentRunId(run_id.trim().to_string()),
            })));
        }
        if let Some(run_id) = trimmed.strip_prefix(":ai-replay ") {
            return Ok(Some(self.push_intent(CommandDispatchIntent::ReplayAiRun {
                run_id: AgentRunId(run_id.trim().to_string()),
            })));
        }
        if let Some(run_id) = trimmed.strip_prefix(":ai-inspect ") {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::InspectAiRun {
                    run_id: AgentRunId(run_id.trim().to_string()),
                },
            )));
        }

        if let Some(payload) = trimmed.strip_prefix(":plugin ") {
            let mut split = payload.splitn(3, ' ');
            let plugin_id = split
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .map(PluginId);
            let command_id = split.next().unwrap_or_default().trim();
            let metadata_label = split.next().unwrap_or(command_id).trim();
            if let Some(plugin_id) = plugin_id
                && plugin_id.0 != 0
                && !command_id.is_empty()
            {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::InvokePluginCommand {
                        plugin_id,
                        command_id: command_id.to_string(),
                        metadata_label: if metadata_label.is_empty() {
                            command_id.to_string()
                        } else {
                            metadata_label.to_string()
                        },
                    },
                )));
            }
        }

        if let Some(session_id) =
            parse_collaboration_session_id(trimmed.strip_prefix(":collab-join "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::JoinCollaborationSession { session_id },
            )));
        }
        if let Some(session_id) =
            parse_collaboration_session_id(trimmed.strip_prefix(":collab-leave "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::LeaveCollaborationSession { session_id },
            )));
        }
        if let Some(payload) = trimmed.strip_prefix(":collab-presence ") {
            let mut split = payload.split_whitespace();
            let session_id = split
                .next()
                .and_then(|value| value.parse::<u128>().ok())
                .map(CollaborationSessionId);
            let participant_id = split
                .next()
                .and_then(|value| value.parse::<u128>().ok())
                .map(CollaborationParticipantId);
            if let (Some(session_id), Some(participant_id)) = (session_id, participant_id)
                && session_id.0 != 0
                && participant_id.0 != 0
            {
                return Ok(Some(self.push_intent(
                    CommandDispatchIntent::PublishCollaborationPresence {
                        session_id,
                        participant_id,
                    },
                )));
            }
        }

        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-preview ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::PreviewProposal { proposal_id },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-approve ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ApproveProposal { proposal_id },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-reject ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RejectProposal {
                    proposal_id,
                    reason: ProposalRejectionReason::UserRejected,
                },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-apply ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ApplyProposal { proposal_id },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-rollback ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RollbackProposal {
                    proposal_id,
                    reason: ProposalRollbackReason::UserRequested,
                },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-cancel ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::CancelProposal {
                    proposal_id,
                    reason: ProposalCancellationReason::UserCancelled,
                },
            )));
        }
        if let Some(proposal_id) = parse_proposal_id(trimmed.strip_prefix(":proposal-details ")) {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::OpenProposalDetails { proposal_id },
            )));
        }
        if let Some(session_id) = parse_legion_session_id(trimmed.strip_prefix(":legion-inspect "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::InspectLegionWorkflowSession { session_id },
            )));
        }
        if let Some((session_id, proposal_id)) =
            parse_legion_session_proposal(trimmed.strip_prefix(":legion-proposal-preview "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::OpenLegionWorkflowProposalPreview {
                    session_id,
                    proposal_id,
                },
            )));
        }
        if let Some((session_id, proposal_id)) =
            parse_legion_session_proposal(trimmed.strip_prefix(":legion-proposal-details "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::OpenLegionWorkflowProposalDetails {
                    session_id,
                    proposal_id,
                },
            )));
        }
        if let Some((session_id, gate_id)) =
            parse_legion_session_label(trimmed.strip_prefix(":legion-verify "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestLegionWorkflowVerification {
                    session_id,
                    gate_id: LegionWorkflowVerificationGateId(gate_id),
                },
            )));
        }
        if let Some((session_id, sign_off_id)) =
            parse_legion_session_label(trimmed.strip_prefix(":legion-signoff "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestLegionWorkflowSignOff {
                    session_id,
                    sign_off_id: LegionWorkflowSignOffId(sign_off_id),
                },
            )));
        }
        if let Some((session_id, conflict_id)) =
            parse_legion_session_label(trimmed.strip_prefix(":legion-resolve "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::ResolveLegionWorkflowConflict {
                    session_id,
                    conflict_id: LegionWorkflowConflictId(conflict_id),
                },
            )));
        }
        if let Some(session_id) =
            parse_legion_session_id(trimmed.strip_prefix(":legion-readiness "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RequestLegionWorkflowMergeReadiness { session_id },
            )));
        }
        if let Some((session_id, server_id, tool_name, decision)) =
            parse_legion_tool_permission(trimmed.strip_prefix(":legion-permission "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::RecordLegionWorkflowToolPermission {
                    session_id,
                    server_id,
                    tool_name,
                    decision,
                },
            )));
        }
        if let Some((session_id, reason_label)) =
            parse_legion_kill_switch(trimmed.strip_prefix(":legion-kill "))
        {
            return Ok(Some(self.push_intent(
                CommandDispatchIntent::TriggerLegionWorkflowKillSwitch {
                    session_id,
                    reason_label,
                },
            )));
        }

        if let Some(payload) = trimmed.strip_prefix(":i ") {
            let buffer_id = self.active_buffer_id()?;
            let pos = protocol_text_coordinate(0, 0, Some(0));
            return Ok(Some(self.push_intent(CommandDispatchIntent::Insert {
                buffer_id,
                at: pos,
                text: payload.to_string(),
            })));
        }

        if let Some(payload) = trimmed.strip_prefix(":d ") {
            let buffer_id = self.active_buffer_id()?;
            let mut split = payload.split(',');
            let start = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let end = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            if start > end {
                return Err(ShellCommandError::InvalidRange);
            }
            let start = self.parse_pos(start);
            let end = self.parse_pos(end);
            return Ok(Some(self.push_intent(CommandDispatchIntent::Delete {
                buffer_id,
                range: ProtocolTextRange { start, end },
            })));
        }

        if let Some(payload) = trimmed.strip_prefix(":r ") {
            let buffer_id = self.active_buffer_id()?;
            let mut split = payload.splitn(3, ',');
            let start = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let end = split.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let replacement = split.next().unwrap_or("");
            if start > end {
                return Err(ShellCommandError::InvalidRange);
            }
            let start = self.parse_pos(start);
            let end = self.parse_pos(end);
            return Ok(Some(self.push_intent(CommandDispatchIntent::Replace {
                buffer_id,
                range: ProtocolTextRange { start, end },
                replacement: replacement.to_string(),
            })));
        }

        Ok(Some(self.push_intent(CommandDispatchIntent::Noop)))
    }

    fn active_buffer_id(&self) -> Result<BufferId, ShellCommandError> {
        self.active_buffer_projection
            .buffer_id
            .ok_or(ShellCommandError::ActiveBufferMissing)
    }

    fn active_terminal_session_id(&self) -> Result<TerminalSessionId, ShellCommandError> {
        self.terminal_panel_projection
            .active_session_id
            .ok_or(ShellCommandError::ActiveTerminalSessionMissing)
    }

    fn active_debug_session_id(&self) -> Result<DebugSessionId, ShellCommandError> {
        self.debug_projection
            .active_session_id
            .clone()
            .ok_or(ShellCommandError::ActiveDebugSessionMissing)
    }

    fn active_assist_prediction_id(&self) -> Option<String> {
        self.assist_inline_prediction_projection
            .active_prediction
            .as_ref()
            .map(|prediction| prediction.prediction_id.clone())
    }

    fn push_intent(&mut self, intent: CommandDispatchIntent) -> CommandDispatchIntent {
        self.command_dispatch_intents.push(intent.clone());
        intent
    }

    fn command_position(&self, payload: &str) -> TextCoordinate {
        if payload.is_empty() {
            return self.parse_pos(0);
        }
        payload
            .parse::<usize>()
            .map(|offset| self.parse_pos(offset))
            .unwrap_or_else(|_| self.parse_pos(0))
    }

    fn parse_pos(&self, byte_offset: usize) -> TextCoordinate {
        if let Some(text) = self.active_buffer_projection.small_buffer_text() {
            return text
                .as_bytes()
                .get(..byte_offset)
                .map(|prefix| {
                    let line = prefix.iter().filter(|b| **b == b'\n').count() as u32;
                    let character = prefix.iter().rev().take_while(|b| **b != b'\n').count() as u32;
                    protocol_text_coordinate(line, character, Some(byte_offset as u64))
                })
                .unwrap_or_else(|| protocol_text_coordinate(0, 0, Some(0)));
        }

        if let Some(viewport) = &self.active_buffer_projection.viewport {
            let mut current_offset = 0;
            for (i, slice) in viewport.line_slices.iter().enumerate() {
                let slice_len = slice.visible_text.len() + 1; // +1 for newline
                if current_offset + slice_len > byte_offset {
                    let character = (byte_offset - current_offset) as u32;
                    let line = viewport.scroll.top_line + i as u32;
                    return protocol_text_coordinate(line, character, Some(byte_offset as u64));
                }
                current_offset += slice_len;
            }
        }

        protocol_text_coordinate(0, 0, Some(0))
    }
}

fn protocol_text_coordinate(line: u32, character: u32, byte_offset: Option<u64>) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset,
        utf16_offset: None,
    }
}

fn parse_buffer_id(input: Option<&str>) -> Option<BufferId> {
    input
        .and_then(|value| value.trim().parse::<u128>().ok())
        .filter(|value| *value != 0)
        .map(BufferId)
}

fn non_empty_string(input: Option<&str>) -> Option<String> {
    input
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn parse_debug_step_kind(input: &str) -> DebugStepKindProjection {
    match input {
        "continue" | "cont" => DebugStepKindProjection::Continue,
        "into" | "in" => DebugStepKindProjection::Into,
        "out" => DebugStepKindProjection::Out,
        "back" => DebugStepKindProjection::Back,
        _ => DebugStepKindProjection::Over,
    }
}

fn parse_dock_mode(input: &str) -> DockMode {
    match input.trim().to_ascii_lowercase().as_str() {
        "assist" | "a" => DockMode::Assist,
        "delegate" | "delegates" | "d" => DockMode::Delegate,
        "automate" | "automation" | "legion" | "workflow" | "workflows" | "w" => DockMode::Automate,
        _ => DockMode::Manual,
    }
}

fn parse_delegate_hunk_disposition(input: &str) -> Option<DelegatedTaskProposalHunkDisposition> {
    match input.trim().to_ascii_lowercase().as_str() {
        "pending" | "p" => Some(DelegatedTaskProposalHunkDisposition::Pending),
        "accept" | "accepted" | "a" => Some(DelegatedTaskProposalHunkDisposition::Accepted),
        "reject" | "rejected" | "r" => Some(DelegatedTaskProposalHunkDisposition::Rejected),
        _ => None,
    }
}

fn parse_delegate_tool_permission_decision(
    input: &str,
) -> Option<DelegatedTaskToolPermissionDecision> {
    match input.trim().to_ascii_lowercase().as_str() {
        "confirm" | "c" => Some(DelegatedTaskToolPermissionDecision::Confirm),
        "allow" | "a" => Some(DelegatedTaskToolPermissionDecision::Allow),
        "deny" | "d" => Some(DelegatedTaskToolPermissionDecision::Deny),
        "always" => Some(DelegatedTaskToolPermissionDecision::Always),
        _ => None,
    }
}

fn parse_legion_tool_permission(
    payload: Option<&str>,
) -> Option<(
    LegionWorkflowSessionId,
    legion_protocol::McpServerId,
    legion_protocol::McpToolName,
    DelegatedTaskToolPermissionDecision,
)> {
    let mut split = payload?.split_whitespace();
    let session_id = split.next()?.trim();
    let server_id = split.next()?.trim();
    let tool_name = split.next()?.trim();
    let decision = parse_delegate_tool_permission_decision(split.next().unwrap_or_default())?;
    if session_id.is_empty()
        || server_id.is_empty()
        || tool_name.is_empty()
        || split.next().is_some()
    {
        return None;
    }
    Some((
        LegionWorkflowSessionId(session_id.to_string()),
        legion_protocol::McpServerId(server_id.to_string()),
        legion_protocol::McpToolName(tool_name.to_string()),
        decision,
    ))
}

fn parse_legion_kill_switch(payload: Option<&str>) -> Option<(LegionWorkflowSessionId, String)> {
    let payload = payload?.trim();
    let mut split = payload.splitn(2, char::is_whitespace);
    let session_id = split.next()?.trim();
    let reason = split.next().unwrap_or("user requested").trim();
    if session_id.is_empty() {
        return None;
    }
    Some((
        LegionWorkflowSessionId(session_id.to_string()),
        if reason.is_empty() {
            "user requested".to_string()
        } else {
            reason.to_string()
        },
    ))
}

fn empty_proposal_ledger_projection() -> ProposalLedgerProjection {
    ProposalLedgerProjection {
        rows: Vec::new(),
        selected_proposal_id: None,
        omitted_row_count: 0,
        generated_at: TimestampMillis(0),
        redaction_hints: Vec::new(),
        schema_version: 1,
    }
}

fn empty_command_registry_projection() -> CommandRegistryProjection {
    CommandRegistryProjection::empty("command-registry:empty", TimestampMillis(0), 1)
}

fn empty_artifact_ledger_projection() -> ArtifactLedgerProjection {
    ArtifactLedgerProjection::empty("artifact-ledger:empty", TimestampMillis(0), 1)
}

fn empty_verification_run_projection() -> VerificationRunProjection {
    VerificationRunProjection::empty("verification-runs:empty", TimestampMillis(0), 1)
}

fn empty_system_graph_projection() -> SystemGraphProjection {
    SystemGraphProjection::empty("system-graph:empty", TimestampMillis(0), 1)
}

fn empty_context_manifest_projection() -> ContextManifestProjection {
    ContextManifestProjection {
        manifest: ContextManifestRecord {
            manifest_id: "manifest:empty".to_string(),
            workspace_id: None,
            proposal_id: None,
            purpose: ContextManifestPurpose::TrustReview,
            workspace_trust_state: None,
            privacy_label: ProposalPrivacyLabel::PublicMetadata,
            risk_label: ProposalRiskLabel::Informational,
            egress: ContextManifestEgressStatus::LocalOnly,
            items: Vec::new(),
            permissions: Vec::new(),
            omitted_item_count: 0,
            stale_or_missing_metadata_risk_present: false,
            generated_at: TimestampMillis(0),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        selected_item_id: None,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_privacy_inspector_projection() -> PrivacyInspectorProjection {
    PrivacyInspectorProjection {
        inspector_id: "privacy:empty".to_string(),
        manifest_id: None,
        workspace_id: None,
        proposal_id: None,
        records: Vec::new(),
        denied_record_count: 0,
        redacted_record_count: 0,
        external_egress_record_count: 0,
        high_risk_record_count: 0,
        refusal: None,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_permission_budget_projection() -> PermissionBudgetProjection {
    PermissionBudgetProjection {
        projection_id: "permission-budgets:empty".to_string(),
        budgets: Vec::new(),
        evaluations: Vec::new(),
        denied_budget_count: 0,
        depleted_budget_count: 0,
        refused_evaluation_count: 0,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_approval_checklist_projection() -> ProposalApprovalChecklistProjection {
    ProposalApprovalChecklistProjection {
        checklist_id: "approval-checklist:empty".to_string(),
        proposal_id: ProposalId(0),
        workspace_id: None,
        payload_kind: legion_protocol::ProposalPayloadKind::SaveFile,
        lifecycle_state: legion_protocol::ProposalLifecycleState::Created,
        correlation_id: legion_protocol::CorrelationId(0),
        causality_id: None,
        ready_for_approval: false,
        gates: Vec::new(),
        blockers: Vec::new(),
        risk_labels: Vec::new(),
        privacy_labels: Vec::new(),
        explicit_denial_reasons: Vec::new(),
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_checkpoint_rollback_projection() -> CheckpointRollbackProjection {
    let preconditions = legion_protocol::ContextManifestPreconditionSummary::from_preconditions(
        &legion_protocol::ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: None,
            workspace_generation: None,
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        1,
    );
    CheckpointRollbackProjection {
        projection_id: "checkpoint-rollback:empty".to_string(),
        proposal_id: ProposalId(0),
        workspace_id: None,
        payload_kind: legion_protocol::ProposalPayloadKind::SaveFile,
        lifecycle_state: legion_protocol::ProposalLifecycleState::Created,
        correlation_id: legion_protocol::CorrelationId(0),
        causality_id: None,
        checkpoint: legion_protocol::ProposalCheckpointProjection {
            checkpoint_id: "checkpoint:empty".to_string(),
            available: false,
            target_count: 0,
            expected_preconditions: preconditions,
            hashes: Vec::new(),
            audit_status: legion_protocol::CheckpointRollbackAuditStatus::NotRequired,
            labels: Vec::new(),
            limitations: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        rollback: legion_protocol::ProposalRollbackProjection {
            availability: legion_protocol::ProposalRollbackAvailability::NotRequired,
            rollback_step_count: 0,
            reversible_target_count: 0,
            irreversible_target_count: 0,
            audit_status: legion_protocol::CheckpointRollbackAuditStatus::NotRequired,
            labels: Vec::new(),
            limitations: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        targets: Vec::new(),
        risk_labels: Vec::new(),
        privacy_labels: Vec::new(),
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_assisted_ai_projection() -> AssistedAiProjection {
    AssistedAiProjection {
        projection_id: "assisted-ai:empty".to_string(),
        providers: Vec::new(),
        routes: Vec::new(),
        requests: Vec::new(),
        refusals: Vec::new(),
        proposal_previews: Vec::new(),
        provider_count: 0,
        request_count: 0,
        refusal_count: 0,
        preview_ready_count: 0,
        provider_invocation: legion_protocol::AssistedAiProviderInvocationState::NotEncoded,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_delegated_task_projection() -> DelegatedTaskProjection {
    DelegatedTaskProjection {
        projection_id: "delegated-task:empty".to_string(),
        plan_rows: Vec::new(),
        step_summaries: Vec::new(),
        blockers: Vec::new(),
        refusals: Vec::new(),
        required_approvals: Vec::new(),
        proposal_preview_links: Vec::new(),
        audit_readiness: Vec::new(),
        plan_only_disclaimers: vec!["delegated_task.plan_only_no_runtime".to_string()],
        plan_count: 0,
        blocked_plan_count: 0,
        refused_plan_count: 0,
        runtime_activation: DelegatedTaskRuntimeActivationState::NotEncoded,
        chat_messages: Vec::new(),
        context_citations: Vec::new(),
        proposal_reviews: Vec::new(),
        tool_permission_requests: Vec::new(),
        chat_message_count: 0,
        context_citation_count: 0,
        proposal_review_count: 0,
        tool_permission_request_count: 0,
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_legion_workflow_projection() -> LegionWorkflowProjection {
    LegionWorkflowProjection::empty("legion-workflow:empty", TimestampMillis(0), 1)
}

fn parse_proposal_id(payload: Option<&str>) -> Option<ProposalId> {
    payload
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(ProposalId)
}

fn parse_legion_session_id(payload: Option<&str>) -> Option<LegionWorkflowSessionId> {
    payload
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| LegionWorkflowSessionId(value.to_string()))
}

fn parse_legion_session_label(payload: Option<&str>) -> Option<(LegionWorkflowSessionId, String)> {
    let mut split = payload?.split_whitespace();
    let session_id = split.next()?.trim();
    let metadata_id = split.next()?.trim();
    if session_id.is_empty() || metadata_id.is_empty() || split.next().is_some() {
        return None;
    }
    Some((
        LegionWorkflowSessionId(session_id.to_string()),
        metadata_id.to_string(),
    ))
}

fn parse_legion_session_proposal(
    payload: Option<&str>,
) -> Option<(LegionWorkflowSessionId, ProposalId)> {
    let (session_id, proposal_id) = parse_legion_session_label(payload)?;
    let proposal_id = proposal_id.parse::<u64>().ok().map(ProposalId)?;
    Some((session_id, proposal_id))
}

fn parse_collaboration_session_id(payload: Option<&str>) -> Option<CollaborationSessionId> {
    payload
        .and_then(|value| value.trim().parse::<u128>().ok())
        .filter(|value| *value != 0)
        .map(CollaborationSessionId)
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        BufferId, BufferVersion, ByteRange, CanonicalPath, CapabilityId, FileFingerprint, FileId,
        LargeFileStatus, PermissionBudgetActionClass, PermissionBudgetConsentRequirementLabel,
        PermissionBudgetContract, PermissionBudgetResetPolicyLabel, PermissionBudgetState,
        PermissionBudgetUsageSummary, PrincipalId, ProposalContextManifestEntrySummary,
        ProposalContextManifestSummary, ProposalDiffChunkDescriptor, ProposalDiffSummary,
        ProposalDiffSummaryKind, ProposalLedgerRow, ProposalLifecycleState,
        ProposalLifecycleStateDisplay, ProposalPayloadKind, ProposalPrivacyLabel,
        ProposalRiskLabel, ProposalRollbackAvailability, ProposalTargetCoverage,
        ProposalTargetCoverageKind, ProtocolTextRange, RedactionHint, SnapshotId, Utf16Position,
        Utf16Range, ViewportDimensions, ViewportLineMetric, ViewportLineSlice,
        ViewportLineTruncationState, ViewportProjection, ViewportProjectionMode, ViewportScroll,
        WorkspaceId,
    };

    #[test]
    fn panel_registry_filters_restricted_panels_out_of_manual_mode() {
        let registry = PanelRegistry::standard();
        let manual = registry.visible_for(DockMode::Manual);

        assert!(!manual.is_empty());
        assert!(
            manual.iter().all(|panel| !panel.requires_ai),
            "manual mode must not construct restricted panels: {manual:?}"
        );
        assert!(registry.is_visible_in(PanelId::ProjectExplorer, DockMode::Manual));
        assert!(registry.is_visible_in(PanelId::Terminal, DockMode::Manual));
        assert!(registry.is_visible_in(PanelId::PluginManager, DockMode::Manual));
        assert!(registry.is_visible_in(PanelId::Settings, DockMode::Manual));
        assert!(!registry.is_visible_in(PanelId::Assistant, DockMode::Manual));
        assert!(!registry.is_visible_in(PanelId::Delegation, DockMode::Manual));
        assert!(!registry.is_visible_in(PanelId::ApprovalQueue, DockMode::Manual));
        assert!(!registry.is_visible_in(PanelId::AgentFleet, DockMode::Manual));
        assert!(!registry.is_visible_in(PanelId::DecisionFeed, DockMode::Manual));
        assert!(!registry.is_visible_in(PanelId::Workflow, DockMode::Manual));
        assert!(!registry.is_visible_in(PanelId::Collaboration, DockMode::Manual));
        assert!(!registry.is_visible_in(PanelId::RemoteWorkspace, DockMode::Manual));
        assert!(registry.is_visible_in(PanelId::Assistant, DockMode::Assist));
        assert!(!registry.is_visible_in(PanelId::Delegation, DockMode::Assist));
        assert!(registry.is_visible_in(PanelId::Delegation, DockMode::Delegate));
        assert!(registry.is_visible_in(PanelId::Collaboration, DockMode::Delegate));
        assert!(!registry.is_visible_in(PanelId::RemoteWorkspace, DockMode::Delegate));
        assert!(registry.is_visible_in(PanelId::AgentFleet, DockMode::Automate));
        assert!(registry.is_visible_in(PanelId::RemoteWorkspace, DockMode::Automate));
    }

    #[test]
    fn dock_panel_descriptor_roundtrips_projection_state() {
        let mut panel = DockPanelDescriptor::new(
            PanelId::Diagnostics,
            "Problems",
            "alert",
            DockSide::Bottom,
            false,
        );

        let state = panel.persist_state();
        assert_eq!(state["id"], "diagnostics");
        let expected: Vec<PanelCapability> = vec![PanelCapability::ManualIde];
        assert_eq!(panel.capabilities, expected);
        panel
            .restore_state(state)
            .expect("descriptor state restores");

        let error = panel
            .restore_state(serde_json::json!({
                "id": "assistant",
                "schema_version": 1,
            }))
            .expect_err("state for another panel is rejected");
        assert!(matches!(
            error,
            DockPanelStateError::InvalidState { message } if message.contains("does not match")
        ));
    }

    #[test]
    fn dock_persisted_ids_parse_for_session_restore() {
        assert_eq!(DockMode::parse("Manual"), Some(DockMode::Manual));
        assert_eq!(
            DockMode::parse("Legion Workflows"),
            Some(DockMode::Automate)
        );
        assert_eq!(DockSide::parse("Right"), Some(DockSide::Right));
        assert_eq!(
            PanelId::parse("approval_queue"),
            Some(PanelId::ApprovalQueue)
        );
        assert_eq!(PanelId::parse("settings"), Some(PanelId::Settings));
        assert_eq!(PanelId::parse("unknown_panel"), None);
    }

    #[test]
    fn settings_projection_parses_labels_and_normalizes_bounds() {
        let settings = SettingsProjection {
            theme_preference: ThemePreferenceProjection::parse("System")
                .expect("theme label should parse"),
            zoom_percent: 999,
            editor_font_size_pt: 1,
            toast_verbosity: ToastVerbosityProjection::parse("All statuses")
                .expect("toast label should parse"),
            editor: EditorSettingsProjection {
                line_numbers_visible: false,
                current_line_highlight: false,
                sticky_headers_visible: true,
                code_folding_visible: true,
                minimap_visible: false,
                whitespace_guides_visible: false,
                indent_guides_visible: false,
                smooth_scrolling_enabled: true,
            },
            telemetry: WorkbenchTelemetryConsent::default(),
            indexed_workspace_search_enabled: false,
            next_edit_prediction_enabled: false,
            schema_version: 0,
        }
        .normalized();

        assert_eq!(settings.theme_preference, ThemePreferenceProjection::System);
        assert_eq!(settings.zoom_percent, SettingsProjection::MAX_ZOOM_PERCENT);
        assert_eq!(
            settings.editor_font_size_pt,
            SettingsProjection::MIN_EDITOR_FONT_SIZE_PT
        );
        assert_eq!(settings.toast_verbosity, ToastVerbosityProjection::All);
        assert!(!settings.editor.line_numbers_visible);
        assert!(!settings.editor.current_line_highlight);
        assert!(!settings.telemetry.crash_reports_enabled);
        assert_eq!(settings.telemetry.consent_label, "local-only");
        assert_eq!(settings.schema_version, 1);
    }

    #[test]
    fn panel_registry_constructs_from_dock_panel_contracts() {
        let diagnostics = DockPanelDescriptor::new(
            PanelId::Diagnostics,
            "Problems",
            "alert",
            DockSide::Bottom,
            false,
        );
        let assistant = DockPanelDescriptor::new(
            PanelId::Assistant,
            "Assistant",
            "spark",
            DockSide::Right,
            true,
        );
        let panels: [&dyn DockPanel; 2] = [&diagnostics, &assistant];

        let registry = PanelRegistry::from_dock_panels(panels);

        assert!(registry.is_visible_in(PanelId::Diagnostics, DockMode::Manual));
        assert!(!registry.is_visible_in(PanelId::Assistant, DockMode::Manual));
        assert!(registry.is_visible_in(PanelId::Assistant, DockMode::Assist));
    }

    #[test]
    fn dock_layouts_are_mode_scoped_and_manual_layout_is_ai_free() {
        let registry = PanelRegistry::standard();
        let manual = DockLayout::standard(DockMode::Manual);
        let automate = DockLayout::standard(DockMode::Automate);

        for side in [DockSide::Left, DockSide::Right, DockSide::Bottom] {
            let visible = manual.visible_panel_ids(side, &registry);
            assert!(
                visible
                    .iter()
                    .all(|id| registry.is_visible_in(*id, DockMode::Manual)),
                "manual {side:?} layout exposed an AI panel: {visible:?}"
            );
        }

        assert!(
            automate
                .visible_panel_ids(DockSide::Right, &registry)
                .contains(&PanelId::AgentFleet)
        );
        assert_ne!(manual.right.pinned_default, automate.right.pinned_default);
    }

    fn test_coordinate(line: u32, character: u32) -> TextCoordinate {
        TextCoordinate {
            line,
            character,
            byte_offset: Some(character as u64),
            utf16_offset: None,
        }
    }

    fn test_legion_workflow_projection() -> LegionWorkflowProjection {
        LegionWorkflowProjection {
            projection_id: "legion-workflow:test".to_string(),
            rows: vec![legion_protocol::LegionWorkflowProjectionRow {
                session_id: LegionWorkflowSessionId("session:legion:test".to_string()),
                directive_artifact_id: Some("artifact:directive:legion:test".to_string()),
                spec_artifact_id: Some("artifact:spec:legion:test".to_string()),
                task_graph_artifact_id: Some("artifact:task-graph:legion:test".to_string()),
                lifecycle_state: legion_protocol::LegionWorkflowState::WaitingForApproval,
                worker_count: 3,
                provider_route_required_count: 1,
                dependency_count: 2,
                unresolved_conflict_count: 1,
                verification_gate_count: 2,
                passed_verification_count: 1,
                sign_off_count: 2,
                signed_off_count: 1,
                linked_proposals: vec![ProposalId(42)],
                merge_readiness: legion_protocol::LegionWorkflowMergeReadiness {
                    state: legion_protocol::LegionWorkflowMergeReadinessState::WaitingForApproval,
                    blockers: vec![
                        legion_protocol::LegionWorkflowMergeReadinessBlocker::ApprovalRequired,
                    ],
                    labels: vec!["legion_workflow.waiting_for_approval".to_string()],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                },
                display_safe_labels: vec![
                    "implementer.local".to_string(),
                    "Autonomous merge unsupported until approval".to_string(),
                ],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            mcp_registries: Vec::new(),
            decision_feed: Vec::new(),
            risk_monitors: Vec::new(),
            kill_switches: Vec::new(),
            tool_permission_requests: Vec::new(),
            total_session_count: 1,
            mcp_registry_count: 0,
            decision_feed_count: 0,
            risk_monitor_count: 0,
            kill_switch_count: 0,
            tool_permission_request_count: 0,
            omitted_row_count: 0,
            generated_at: TimestampMillis(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn test_proposal_ledger_projection() -> ProposalLedgerProjection {
        ProposalLedgerProjection {
            rows: vec![ProposalLedgerRow {
                proposal_id: ProposalId(42),
                workspace_id: Some(WorkspaceId(1)),
                title: "bounded save preview".to_string(),
                payload_kind: ProposalPayloadKind::SaveFile,
                lifecycle: ProposalLifecycleStateDisplay {
                    state: ProposalLifecycleState::Previewed,
                    label: "Previewed".to_string(),
                    description: "ready for user review".to_string(),
                },
                principal: PrincipalId("trusted".to_string()),
                capability: CapabilityId("fs.write".to_string()),
                created_at: TimestampMillis(1),
                updated_at: TimestampMillis(2),
                expires_at: None,
                risk_label: ProposalRiskLabel::Low,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                rollback: ProposalRollbackAvailability::Available,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: Vec::new(),
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                context_manifest: ProposalContextManifestSummary {
                    manifest_id: "manifest:42".to_string(),
                    category_count: 1,
                    total_item_count: 1,
                    omitted_item_count: 0,
                    categories: vec![ProposalContextManifestEntrySummary {
                        category: "files".to_string(),
                        item_count: 1,
                        omitted_item_count: 0,
                        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                        manifest_hash: Some(FileFingerprint {
                            algorithm: "sha256".to_string(),
                            value: "ctx".to_string(),
                        }),
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                    }],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                diff_summary: ProposalDiffSummary {
                    kind: ProposalDiffSummaryKind::Text,
                    target_count: 1,
                    hunk_count: 1,
                    inserted_line_count: 2,
                    deleted_line_count: 1,
                    omitted_hunk_count: 99,
                    full_source_redacted: true,
                    diff_hash: Some(FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "diff".to_string(),
                    }),
                    chunks: vec![ProposalDiffChunkDescriptor {
                        chunk_id: "chunk-0".to_string(),
                        target_id: None,
                        byte_range: Some(ByteRange::new(10, 20)),
                        changed_line_count: 3,
                        inserted_line_count: 2,
                        deleted_line_count: 1,
                        content_hash: Some(FileFingerprint {
                            algorithm: "blake3".to_string(),
                            value: "chunk".to_string(),
                        }),
                    }],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                preview_warnings: Vec::new(),
                diagnostics: Vec::new(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            selected_proposal_id: Some(ProposalId(42)),
            omitted_row_count: 0,
            generated_at: TimestampMillis(3),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn degraded_viewport_projection() -> ViewportProjection {
        ViewportProjection {
            workspace_id: WorkspaceId(1),
            buffer_id: BufferId(2),
            file_id: Some(FileId(9)),
            snapshot_id: SnapshotId(3),
            buffer_version: BufferVersion(4),
            visible_range: ProtocolTextRange {
                start: test_coordinate(10, 0),
                end: test_coordinate(12, 14),
            },
            selections: Vec::new(),
            cursor: test_coordinate(10, 0),
            scroll: ViewportScroll {
                top_line: 10,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 32,
            },
            mode: ViewportProjectionMode::DegradedLargeFile,
            line_slices: vec![
                ViewportLineSlice {
                    line_number: 10,
                    visible_text: "bounded-alpha".to_string(),
                    byte_range: ByteRange::new(1024, 1037),
                    utf16_range: Utf16Range {
                        start: Utf16Position {
                            line: 10,
                            character: 0,
                        },
                        end: Utf16Position {
                            line: 10,
                            character: 13,
                        },
                    },
                    chunk_hash: FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "chunk-a".to_string(),
                    },
                    truncation_state: ViewportLineTruncationState::None,
                },
                ViewportLineSlice {
                    line_number: 11,
                    visible_text: "bounded-beta".to_string(),
                    byte_range: ByteRange::new(2048, 2060),
                    utf16_range: Utf16Range {
                        start: Utf16Position {
                            line: 11,
                            character: 0,
                        },
                        end: Utf16Position {
                            line: 11,
                            character: 12,
                        },
                    },
                    chunk_hash: FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "chunk-b".to_string(),
                    },
                    truncation_state: ViewportLineTruncationState::Trailing,
                },
            ],
            line_metrics: vec![
                ViewportLineMetric {
                    byte_length: 13,
                    utf16_length: 13,
                    line_ending_width: 1,
                    exact: true,
                },
                ViewportLineMetric {
                    byte_length: 4096,
                    utf16_length: 4096,
                    line_ending_width: 1,
                    exact: true,
                },
            ],
            decoration_spans: Vec::new(),
            fold_ranges: Vec::new(),
            semantic_token_overlays: Vec::new(),
            large_file_status: Some(LargeFileStatus {
                threshold_bytes: 5 * 1024 * 1024,
                byte_len: 6 * 1024 * 1024,
                disabled_overlay_reasons: vec!["semantic token overlays deferred".to_string()],
                bounded_search_enabled: true,
                message: "Large file degraded mode: viewport payloads are chunked".to_string(),
            }),
            schema_version: 2,
        }
    }

    #[test]
    fn shell_parses_commands_into_dispatch_intents_without_editor_ownership() {
        let mut shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("t"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection {
                workspace_id: Some(WorkspaceId(1)),
                buffer_id: Some(BufferId(2)),
                file_id: Some(FileId(9)),
                file_path: Some(CanonicalPath("a.md".to_string())),
                viewport: None,
                degraded: false,
                small_buffer_preview: Some("first".to_string()),
                dirty: false,
            },
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let intent = shell
            .handle_command(":i \\n")
            .expect("insert command should parse")
            .expect("intent should be emitted");

        assert_eq!(
            intent,
            CommandDispatchIntent::Insert {
                buffer_id: BufferId(2),
                at: test_coordinate(0, 0),
                text: "\\n".to_string(),
            }
        );
        assert_eq!(
            shell.active_buffer_projection.small_buffer_text(),
            Some("first")
        );
        assert_eq!(shell.command_dispatch_intents.len(), 1);
    }

    #[test]
    fn toast_stack_filters_info_bounds_visible_and_tracks_overflow() {
        let messages = (0..(TOAST_VISIBLE_LIMIT + 2))
            .map(|index| StatusMessageProjection {
                severity: StatusSeverity::Warning,
                message: format!("Warning {index}: detail"),
            })
            .chain(std::iter::once(StatusMessageProjection {
                severity: StatusSeverity::Info,
                message: "Info-only status".to_string(),
            }))
            .collect::<Vec<_>>();

        let stack = ToastStackProjection::from_status_messages(&messages, &[]);
        let all_stack = ToastStackProjection::from_status_messages_with_verbosity(
            &messages,
            &[],
            ToastVerbosityProjection::All,
        );
        let errors_only_stack = ToastStackProjection::from_status_messages_with_verbosity(
            &messages,
            &[],
            ToastVerbosityProjection::ErrorsOnly,
        );
        let dismissed = stack.visible[0].id;
        let dismissed_stack = ToastStackProjection::from_status_messages(&messages, &[dismissed]);

        assert_eq!(stack.visible.len(), TOAST_VISIBLE_LIMIT);
        assert_eq!(stack.overflow_count, 2);
        assert!(
            stack
                .visible
                .iter()
                .all(|toast| toast.severity != StatusSeverity::Info)
        );
        assert_eq!(stack.visible[0].title, "Warning 6");
        assert_eq!(stack.visible[0].body.as_deref(), Some("detail"));
        assert_eq!(all_stack.visible.len(), TOAST_VISIBLE_LIMIT);
        assert_eq!(all_stack.overflow_count, 3);
        assert_eq!(all_stack.visible[0].severity, StatusSeverity::Info);
        assert!(errors_only_stack.visible.is_empty());
        assert_eq!(errors_only_stack.overflow_count, 0);
        assert_eq!(dismissed_stack.visible.len(), TOAST_VISIBLE_LIMIT);
        assert_eq!(dismissed_stack.overflow_count, 1);
        assert!(
            dismissed_stack
                .visible
                .iter()
                .all(|toast| toast.id != dismissed)
        );
    }

    #[test]
    fn shell_renders_proposal_ledger_from_static_snapshot() {
        let ledger = test_proposal_ledger_projection();
        let shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("t"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: ledger.clone(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.proposal_ledger_projection, ledger);
        assert_eq!(
            snapshot.proposal_ledger_projection.rows[0].proposal_id,
            ProposalId(42)
        );
        assert!(
            snapshot.proposal_ledger_projection.rows[0]
                .diff_summary
                .full_source_redacted
        );
    }

    #[test]
    fn shell_carries_post_ga_work_surface_projections_without_ownership() {
        let mut snapshot = Shell::empty("work-surfaces").projection_snapshot();
        snapshot.command_registry_projection = legion_protocol::CommandRegistryProjection {
            projection_id: "command-registry:test".to_string(),
            commands: vec![legion_protocol::CommandDescriptor {
                command_id: "delegated.inspect_plan".to_string(),
                title: "Inspect Delegated Plan".to_string(),
                scope: "agents".to_string(),
                enabled: true,
                disabled_reason: None,
                shortcut: None,
                risk_label: legion_protocol::CommandRiskLabel::Safe,
                required_permission: Some(CapabilityId("delegated.plan.inspect".to_string())),
                target: Some("plan:1".to_string()),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            selected_command_id: None,
            omitted_command_count: 0,
            generated_at: TimestampMillis(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        snapshot.artifact_ledger_projection = legion_protocol::ArtifactLedgerProjection {
            projection_id: "artifact-ledger:test".to_string(),
            rows: vec![legion_protocol::ArtifactLedgerRow {
                artifact_id: "artifact:directive:1".to_string(),
                kind: legion_protocol::ArtifactKind::Directive,
                title: "Directive".to_string(),
                state_label: "Planned".to_string(),
                linked_proposal_id: None,
                linked_session_id: None,
                raw_payload_retained: false,
                risk_label: ProposalRiskLabel::Medium,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            omitted_row_count: 0,
            generated_at: TimestampMillis(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        snapshot.verification_run_projection = legion_protocol::VerificationRunProjection {
            projection_id: "verification-runs:test".to_string(),
            rows: vec![legion_protocol::VerificationRunRow {
                run_id: "verification:1".to_string(),
                label: "cargo test".to_string(),
                state: legion_protocol::VerificationRunState::Planned,
                command_class_label: "test".to_string(),
                command_body_redacted: true,
                exit_code: None,
                target_labels: vec!["workspace".to_string()],
                evidence_artifact_id: None,
                started_at: None,
                completed_at: None,
                risk_label: ProposalRiskLabel::Low,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            omitted_row_count: 0,
            generated_at: TimestampMillis(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        snapshot.system_graph_projection = legion_protocol::SystemGraphProjection {
            projection_id: "system-graph:test".to_string(),
            nodes: vec![legion_protocol::SystemGraphNode {
                node_id: "system:workspace".to_string(),
                kind_label: "workspace".to_string(),
                display_label: "Active workspace".to_string(),
                target_count: 1,
                risk_label: ProposalRiskLabel::Low,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            edges: Vec::new(),
            omitted_node_count: 0,
            omitted_edge_count: 0,
            generated_at: TimestampMillis(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let shell = Shell::new(snapshot.clone());
        let roundtrip = shell.projection_snapshot();
        assert_eq!(
            roundtrip.command_registry_projection,
            snapshot.command_registry_projection
        );
        assert_eq!(
            roundtrip.artifact_ledger_projection,
            snapshot.artifact_ledger_projection
        );
        assert_eq!(
            roundtrip.verification_run_projection,
            snapshot.verification_run_projection
        );
        assert_eq!(
            roundtrip.system_graph_projection,
            snapshot.system_graph_projection
        );
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn shell_snapshot_large_file_projection_carries_only_viewport_slices() {
        let large_source_len = 6 * 1024 * 1024;
        let shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("large"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection {
                workspace_id: Some(WorkspaceId(1)),
                buffer_id: Some(BufferId(2)),
                file_id: Some(FileId(9)),
                file_path: Some(CanonicalPath("large.txt".to_string())),
                viewport: Some(degraded_viewport_projection()),
                degraded: true,
                small_buffer_preview: None,
                dirty: false,
            },
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let snapshot = shell.projection_snapshot();
        let active = snapshot.active_buffer_projection;
        let viewport = active.viewport.as_ref().expect("viewport projection");
        let payload_bytes = viewport
            .line_slices
            .iter()
            .map(|slice| slice.visible_text.len())
            .sum::<usize>();

        assert!(active.degraded);
        assert!(active.small_buffer_text().is_none());
        assert_eq!(viewport.mode, ViewportProjectionMode::DegradedLargeFile);
        assert!(viewport.large_file_status.is_some());
        assert!(payload_bytes < large_source_len / 1000);
        assert!(
            viewport
                .line_slices
                .iter()
                .all(|slice| slice.visible_text.len() < large_source_len)
        );
    }

    #[test]
    fn shell_proposal_intents_do_not_mutate_editor_or_workspace_projection() {
        let mut shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("t"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection {
                workspace_id: Some(WorkspaceId(1)),
                buffer_id: Some(BufferId(2)),
                file_id: Some(FileId(9)),
                file_path: Some(CanonicalPath("a.md".to_string())),
                viewport: None,
                degraded: false,
                small_buffer_preview: Some("first".to_string()),
                dirty: false,
            },
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let before = shell.projection_snapshot();
        let intent = shell
            .handle_command(":proposal-approve 42")
            .expect("proposal command should parse")
            .expect("intent should be emitted");

        assert_eq!(
            intent,
            CommandDispatchIntent::ApproveProposal {
                proposal_id: ProposalId(42)
            }
        );
        assert_eq!(shell.projection_snapshot(), before);
        assert_eq!(shell.command_dispatch_intents.len(), 1);
    }

    #[test]
    fn control_trust_command_intents_remain_projection_only() {
        let mut shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("control-trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection {
                workspace_id: Some(WorkspaceId(1)),
                buffer_id: Some(BufferId(2)),
                file_id: Some(FileId(9)),
                file_path: Some(CanonicalPath("a.md".to_string())),
                viewport: None,
                degraded: false,
                small_buffer_preview: Some("first".to_string()),
                dirty: true,
            },
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });
        let before = shell.projection_snapshot();

        let commands = vec![
            (
                ":proposal-preview 42",
                CommandDispatchIntent::PreviewProposal {
                    proposal_id: ProposalId(42),
                },
            ),
            (
                ":proposal-approve 42",
                CommandDispatchIntent::ApproveProposal {
                    proposal_id: ProposalId(42),
                },
            ),
            (
                ":proposal-reject 42",
                CommandDispatchIntent::RejectProposal {
                    proposal_id: ProposalId(42),
                    reason: ProposalRejectionReason::UserRejected,
                },
            ),
            (
                ":proposal-apply 42",
                CommandDispatchIntent::ApplyProposal {
                    proposal_id: ProposalId(42),
                },
            ),
            (
                ":proposal-rollback 42",
                CommandDispatchIntent::RollbackProposal {
                    proposal_id: ProposalId(42),
                    reason: ProposalRollbackReason::UserRequested,
                },
            ),
            (
                ":proposal-cancel 42",
                CommandDispatchIntent::CancelProposal {
                    proposal_id: ProposalId(42),
                    reason: ProposalCancellationReason::UserCancelled,
                },
            ),
            (
                ":proposal-details 42",
                CommandDispatchIntent::OpenProposalDetails {
                    proposal_id: ProposalId(42),
                },
            ),
            (
                ":ai-start summarize context",
                CommandDispatchIntent::StartAiRun {
                    instruction_label: "summarize context".to_string(),
                },
            ),
            (
                ":ai-explain summarize context",
                CommandDispatchIntent::StartAiExplain {
                    instruction_label: "summarize context".to_string(),
                },
            ),
            (
                ":ai-propose add guard",
                CommandDispatchIntent::StartAiProposal {
                    instruction_label: "add guard".to_string(),
                },
            ),
            (
                ":delegate-chat explain impacted files",
                CommandDispatchIntent::SendDelegateChat {
                    prompt_label: "explain impacted files".to_string(),
                },
            ),
            (
                ":delegate-hunk 42 delegate-hunk-1 accept",
                CommandDispatchIntent::ReviewDelegateProposalHunk {
                    proposal_id: ProposalId(42),
                    hunk_id: "delegate-hunk-1".to_string(),
                    disposition: DelegatedTaskProposalHunkDisposition::Accepted,
                },
            ),
            (
                ":delegate-permission delegate-permission-1 always",
                CommandDispatchIntent::RecordDelegateToolPermission {
                    request_id: "delegate-permission-1".to_string(),
                    decision: DelegatedTaskToolPermissionDecision::Always,
                },
            ),
            (
                ":ai-cancel run-1",
                CommandDispatchIntent::CancelAiRun {
                    run_id: AgentRunId("run-1".to_string()),
                },
            ),
            (
                ":ai-replay run-1",
                CommandDispatchIntent::ReplayAiRun {
                    run_id: AgentRunId("run-1".to_string()),
                },
            ),
            (
                ":ai-inspect run-1",
                CommandDispatchIntent::InspectAiRun {
                    run_id: AgentRunId("run-1".to_string()),
                },
            ),
        ];

        let command_count = commands.len();
        for (command, expected) in commands {
            let intent = shell
                .handle_command(command)
                .expect("control trust command should parse")
                .expect("intent should be emitted");
            assert_eq!(intent, expected);
            assert_eq!(shell.projection_snapshot(), before);
        }

        assert!(shell.command_dispatch_intents.len() >= command_count);
    }

    #[test]
    fn assisted_ai_command_intents_remain_projection_only() {
        control_trust_command_intents_remain_projection_only();
    }

    #[test]
    fn control_trust_shell_carries_static_projection_contracts_without_ownership() {
        shell_renders_context_manifest_from_static_snapshot_without_ownership();
        shell_renders_privacy_and_budget_summaries_from_static_snapshot_without_ownership();
        shell_renders_approval_and_rollback_summaries_from_static_snapshot_without_ownership();
        shell_renders_assisted_ai_projection_from_static_snapshot_without_ownership();
    }

    #[test]
    fn shell_renders_context_manifest_from_static_snapshot_without_ownership() {
        let mut manifest = empty_context_manifest_projection();
        manifest.manifest.manifest_id = "manifest:trust-review".to_string();
        manifest.manifest.risk_label = ProposalRiskLabel::Medium;
        manifest.manifest.privacy_label = ProposalPrivacyLabel::WorkspaceMetadata;
        manifest.selected_item_id = Some("semantic-job:0".to_string());
        manifest
            .manifest
            .items
            .push(legion_protocol::ContextManifestItem {
                item_id: "semantic-job:0".to_string(),
                kind: legion_protocol::ContextManifestItemKind::SemanticFabricJob,
                inclusion: legion_protocol::ContextManifestInclusionState::Included,
                workspace_id: Some(WorkspaceId(1)),
                file_id: Some(FileId(9)),
                buffer_id: Some(BufferId(2)),
                proposal_id: Some(ProposalId(42)),
                target_id: Some("target-buffer-main".to_string()),
                path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
                ranges: vec![ByteRange::new(10, 20)],
                counts: vec![legion_protocol::ContextManifestItemCount {
                    label: "diagnostics".to_string(),
                    count: 2,
                }],
                hashes: vec![FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "content".to_string(),
                }],
                privacy_scope: Some(legion_protocol::SemanticPrivacyScope::Workspace),
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                risk_label: ProposalRiskLabel::Medium,
                egress: legion_protocol::ContextManifestEgressStatus::LocalOnly,
                freshness: None,
                preconditions: None,
                labels: vec!["semantic.fabric.metadata".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            });
        manifest
            .manifest
            .items
            .push(legion_protocol::ContextManifestItem {
                item_id: "lsp-diagnostics:0".to_string(),
                kind: legion_protocol::ContextManifestItemKind::LspDiagnosticSummary,
                inclusion: legion_protocol::ContextManifestInclusionState::Excluded,
                workspace_id: Some(WorkspaceId(1)),
                file_id: Some(FileId(10)),
                buffer_id: Some(BufferId(3)),
                proposal_id: Some(ProposalId(42)),
                target_id: Some("target-buffer-secondary".to_string()),
                path: Some(CanonicalPath("C:/repo/src/lib.rs".to_string())),
                ranges: Vec::new(),
                counts: Vec::new(),
                hashes: Vec::new(),
                privacy_scope: Some(legion_protocol::SemanticPrivacyScope::Workspace),
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                risk_label: ProposalRiskLabel::Medium,
                egress: legion_protocol::ContextManifestEgressStatus::LocalOnly,
                freshness: None,
                preconditions: None,
                labels: vec!["retrieval.excluded".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            });

        let shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: manifest.clone(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.context_manifest_projection, manifest);
        assert_eq!(snapshot.context_manifest_projection.manifest.items.len(), 2);
        assert_eq!(
            snapshot
                .context_manifest_projection
                .selected_item_id
                .as_deref(),
            Some("semantic-job:0")
        );
        assert_eq!(
            snapshot.context_manifest_projection.manifest.items[1].inclusion,
            legion_protocol::ContextManifestInclusionState::Excluded
        );
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn context_manifest_selection_commands_remain_projection_only() {
        let mut manifest = empty_context_manifest_projection();
        manifest
            .manifest
            .items
            .push(legion_protocol::ContextManifestItem {
                item_id: "semantic-job:0".to_string(),
                kind: legion_protocol::ContextManifestItemKind::SemanticFabricJob,
                inclusion: legion_protocol::ContextManifestInclusionState::Included,
                workspace_id: Some(WorkspaceId(1)),
                file_id: Some(FileId(9)),
                buffer_id: Some(BufferId(2)),
                proposal_id: Some(ProposalId(42)),
                target_id: Some("target-buffer-main".to_string()),
                path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
                ranges: vec![ByteRange::new(10, 20)],
                counts: vec![legion_protocol::ContextManifestItemCount {
                    label: "diagnostics".to_string(),
                    count: 2,
                }],
                hashes: vec![FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "content".to_string(),
                }],
                privacy_scope: Some(legion_protocol::SemanticPrivacyScope::Workspace),
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                risk_label: ProposalRiskLabel::Medium,
                egress: legion_protocol::ContextManifestEgressStatus::LocalOnly,
                freshness: None,
                preconditions: None,
                labels: vec!["semantic.fabric.metadata".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            });

        let mut shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: manifest,
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        assert!(shell.command_dispatch_intents.is_empty());
        assert!(
            shell
                .handle_command(":context-manifest-select semantic-job:0")
                .expect("context manifest select should parse")
                .is_none()
        );
        assert_eq!(
            shell
                .projection_snapshot()
                .context_manifest_projection
                .selected_item_id
                .as_deref(),
            Some("semantic-job:0")
        );
        assert!(
            shell
                .handle_command(":context-manifest-clear")
                .expect("context manifest clear should parse")
                .is_none()
        );
        assert_eq!(
            shell
                .projection_snapshot()
                .context_manifest_projection
                .selected_item_id,
            None
        );
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn shell_renders_privacy_and_budget_summaries_from_static_snapshot_without_ownership() {
        let mut privacy = empty_privacy_inspector_projection();
        privacy.inspector_id = "privacy:trust".to_string();
        privacy.records = vec![legion_protocol::PrivacyInspectorExposureRecord {
            exposure_id: "exposure:semantic".to_string(),
            source_kind: legion_protocol::PrivacyInspectorSourceKind::SemanticMetadata,
            context_item_id: Some("semantic:0".to_string()),
            proposal_id: Some(ProposalId(42)),
            target_id: Some("target-0".to_string()),
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(9)),
            buffer_id: Some(BufferId(2)),
            privacy_scope: Some(legion_protocol::SemanticPrivacyScope::Workspace),
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_state: legion_protocol::PrivacyInspectorRedactionState::MetadataOnly,
            inclusion: legion_protocol::ContextManifestInclusionState::Included,
            egress: legion_protocol::ContextManifestEgressStatus::LocalOnly,
            risk_label: ProposalRiskLabel::Low,
            permission_label: Some(CapabilityId("semantic.read".to_string())),
            ranges: vec![ByteRange::new(10, 20)],
            counts: Vec::new(),
            hashes: vec![FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "metadata-hash".to_string(),
            }],
            labels: vec!["semantic.metadata".to_string()],
            reasons: vec!["context.included".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let mut budgets = empty_permission_budget_projection();
        budgets.projection_id = "budgets:trust".to_string();
        budgets.budgets = vec![PermissionBudgetContract {
            budget_id: "budget:semantic".to_string(),
            action_class: PermissionBudgetActionClass::ReadSemanticMetadata,
            capability: Some(CapabilityId("semantic.read".to_string())),
            state: PermissionBudgetState::Allowed,
            privacy_scope: legion_protocol::SemanticPrivacyScope::MetadataOnly,
            usage: PermissionBudgetUsageSummary {
                unit_label: "items".to_string(),
                used: 1,
                ceiling: Some(10),
                remaining: Some(9),
                attempted: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            reset_policy_label: PermissionBudgetResetPolicyLabel::Session,
            consent_requirement_label: PermissionBudgetConsentRequirementLabel::NotRequired,
            risk_label: ProposalRiskLabel::Low,
            reasons: vec!["budget.seeded".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: privacy.clone(),
            permission_budget_projection: budgets.clone(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.privacy_inspector_projection, privacy);
        assert_eq!(snapshot.permission_budget_projection, budgets);
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn shell_renders_approval_and_rollback_summaries_from_static_snapshot_without_ownership() {
        let mut checklist = empty_approval_checklist_projection();
        checklist.checklist_id = "approval-checklist:42".to_string();
        checklist.proposal_id = ProposalId(42);
        checklist.ready_for_approval = true;
        checklist.gates = vec![legion_protocol::ApprovalChecklistGateSummary {
            gate: legion_protocol::ApprovalChecklistGateKind::AuditBeforeSuccess,
            status: legion_protocol::ApprovalChecklistGateStatus::Satisfied,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["audit.metadata_only".to_string()],
            reasons: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let mut rollback = empty_checkpoint_rollback_projection();
        rollback.projection_id = "checkpoint-rollback:42".to_string();
        rollback.proposal_id = ProposalId(42);
        rollback.checkpoint.available = true;
        rollback.rollback.availability = legion_protocol::ProposalRollbackAvailability::Available;
        rollback.targets = vec![legion_protocol::CheckpointRollbackTargetSummary {
            target_id: "target-buffer-main".to_string(),
            kind: legion_protocol::ProposalTargetKind::OpenBuffer,
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(9)),
            buffer_id: Some(BufferId(2)),
            terminal_session_id: None,
            plugin_id: None,
            ranges: vec![ByteRange::new(10, 20)],
            hashes: vec![FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "expected".to_string(),
            }],
            expected_file_content_version: Some(legion_protocol::FileContentVersion(44)),
            expected_buffer_version: Some(BufferVersion(55)),
            expected_snapshot_id: Some(SnapshotId(66)),
            expected_workspace_generation: Some(legion_protocol::WorkspaceGeneration(77)),
            labels: vec!["target.kind.OpenBuffer".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: checklist.clone(),
            checkpoint_rollback_projection: rollback.clone(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.approval_checklist_projection, checklist);
        assert_eq!(snapshot.checkpoint_rollback_projection, rollback);
        assert!(snapshot.approval_checklist_projection.ready_for_approval);
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn shell_renders_assisted_ai_projection_from_static_snapshot_without_ownership() {
        let mut assisted = empty_assisted_ai_projection();
        assisted.projection_id = "assisted-ai:p6-2".to_string();
        assisted.provider_count = 1;
        assisted.request_count = 1;
        assisted.preview_ready_count = 1;
        assisted.providers = vec![legion_protocol::AssistedAiProviderCapabilitySummary {
            provider_id: "provider:local-redacted".to_string(),
            provider_label: "Local metadata provider".to_string(),
            provider_class: legion_protocol::AssistedAiProviderClass::Local,
            supported_operations: vec![legion_protocol::AssistedAiOperationClass::ProposeEdit],
            supported_operation_count: 1,
            model_capability_label_count: 1,
            tool_capability_label_count: 0,
            context_window_label: "bounded".to_string(),
            cost_budget_label: "capped".to_string(),
            risk_budget_label: "review-required".to_string(),
            privacy_retention_label: "metadata-only".to_string(),
            availability: legion_protocol::AssistedAiProviderAvailabilityState::Available,
            refusal: None,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];
        assisted.proposal_previews = vec![legion_protocol::AssistedAiProposalPreviewSummary {
            preview_id: "assist:preview:42".to_string(),
            output_id: "assist:output:42".to_string(),
            request_id: "assist:req:42".to_string(),
            provider_id: "provider:local-redacted".to_string(),
            proposal_id: ProposalId(42),
            payload_kind: ProposalPayloadKind::TextEdit,
            lifecycle_state: ProposalLifecycleState::Previewed,
            readiness: legion_protocol::AssistedAiProposalPreviewReadiness::PreviewReady,
            ready_for_preview: true,
            ready_for_approval: true,
            ready_for_apply: false,
            correlation_id: legion_protocol::CorrelationId(901),
            causality_id: legion_protocol::CausalityId(
                uuid::Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
            ),
            context_manifest: legion_protocol::AssistedAiTrustProjectionReference {
                reference_id: "manifest:p5:context".to_string(),
                kind: legion_protocol::AssistedAiTrustProjectionKind::ContextManifest,
                projection_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "manifest".to_string(),
                },
                schema_version: 1,
            },
            approval_checklist: legion_protocol::AssistedAiTrustProjectionReference {
                reference_id: "checklist:p5:approval".to_string(),
                kind: legion_protocol::AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
                projection_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "checklist".to_string(),
                },
                schema_version: 1,
            },
            checkpoint_rollback: None,
            preconditions: legion_protocol::ContextManifestPreconditionSummary::from_preconditions(
                &legion_protocol::ProposalVersionPreconditions {
                    file_version: Some(legion_protocol::FileContentVersion(44)),
                    buffer_version: Some(BufferVersion(55)),
                    snapshot_id: Some(SnapshotId(66)),
                    generation: Some(legion_protocol::WorkspaceGeneration(77)),
                    file_content_version: Some(legion_protocol::FileContentVersion(44)),
                    workspace_generation: Some(legion_protocol::WorkspaceGeneration(77)),
                    expected_fingerprint: Some(FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "expected".to_string(),
                    }),
                    expected_file_length: Some(1234),
                    expected_modified_at: Some(TimestampMillis(9876)),
                },
                1,
            ),
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: Vec::new(),
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            diff_summary: ProposalDiffSummary {
                kind: ProposalDiffSummaryKind::Text,
                target_count: 1,
                hunk_count: 1,
                inserted_line_count: 0,
                deleted_line_count: 0,
                omitted_hunk_count: 0,
                full_source_redacted: true,
                diff_hash: None,
                chunks: Vec::new(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            trust_projection_references: Vec::new(),
            ledger_row_present: true,
            preview_warning_count: 0,
            refusal: None,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["proposal.apply.not_encoded".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("assisted"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: assisted.clone(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: empty_delegated_task_projection(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.assisted_ai_projection, assisted);
        assert_eq!(
            snapshot.assisted_ai_projection.provider_invocation,
            legion_protocol::AssistedAiProviderInvocationState::NotEncoded
        );
        assert!(snapshot.assisted_ai_projection.proposal_previews[0].ready_for_preview);
        assert!(!snapshot.assisted_ai_projection.proposal_previews[0].ready_for_apply);
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn shell_renders_delegated_task_projection_from_static_snapshot_without_ownership() {
        let mut delegated = empty_delegated_task_projection();
        delegated.projection_id = "delegated-task:p7-1".to_string();
        delegated.plan_count = 1;
        delegated.plan_rows = vec![legion_protocol::DelegatedTaskPlanRow {
            plan_id: legion_protocol::DelegatedTaskPlanId("plan:p7-1".to_string()),
            workspace_id: Some(WorkspaceId(1)),
            objective_summary_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "objective".to_string(),
            },
            plan_state: legion_protocol::DelegatedTaskPlanState::AwaitingApproval,
            readiness: legion_protocol::DelegatedTaskPlanReadinessStatus::PlanReady,
            step_count: 1,
            affected_target_count: 1,
            blocker_count: 0,
            refusal_count: 0,
            proposal_preview_link_count: 1,
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            correlation_id: legion_protocol::CorrelationId(901),
            causality_id: legion_protocol::CausalityId(
                uuid::Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
            ),
            runtime_activation: legion_protocol::DelegatedTaskRuntimeActivationState::NotEncoded,
            labels: vec!["delegated_task.plan_row.metadata_only".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];
        delegated.step_summaries = vec![legion_protocol::DelegatedTaskStepSummary {
            step_id: legion_protocol::DelegatedTaskStepId("step:preview".to_string()),
            plan_id: legion_protocol::DelegatedTaskPlanId("plan:p7-1".to_string()),
            order: 1,
            objective_summary_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "step".to_string(),
            },
            operation_class: legion_protocol::DelegatedTaskOperationClass::LinkProposalPreview,
            state: legion_protocol::DelegatedTaskStepState::ProposalPreviewLinked,
            dependency_count: 0,
            target_count: 1,
            proposal_id: Some(ProposalId(42)),
            blocker_count: 0,
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["proposal-preview-link-only".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let shell = Shell::new(ShellProjectionSnapshot {
            product_mode: DockMode::Manual,
            layout_projection: ShellLayoutProjection::plain("delegated"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            palette_projection: PaletteProjection::closed(),
            command_registry_projection: empty_command_registry_projection(),
            settings_projection: SettingsProjection::default(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            artifact_ledger_projection: empty_artifact_ledger_projection(),
            verification_run_projection: empty_verification_run_projection(),
            system_graph_projection: empty_system_graph_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
            delegated_task_projection: delegated.clone(),
            legion_workflow_projection: empty_legion_workflow_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.delegated_task_projection, delegated);
        assert_eq!(
            snapshot.delegated_task_projection.runtime_activation,
            legion_protocol::DelegatedTaskRuntimeActivationState::NotEncoded
        );
        assert_eq!(
            snapshot.delegated_task_projection.step_summaries[0].proposal_id,
            Some(ProposalId(42))
        );
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn legion_workflow_empty_projection_is_metadata_only() {
        let shell = Shell::empty("legion");
        let snapshot = shell.projection_snapshot();

        assert!(snapshot.legion_workflow_projection.rows.is_empty());
        assert_eq!(
            snapshot.legion_workflow_projection.redaction_hints,
            vec![RedactionHint::MetadataOnly]
        );
    }

    #[test]
    fn legion_workflow_projection_roundtrips_without_ui_authority() {
        let mut snapshot = Shell::empty("legion").projection_snapshot();
        snapshot.legion_workflow_projection = test_legion_workflow_projection();

        let shell = Shell::new(snapshot.clone());
        let roundtrip = shell.projection_snapshot();

        assert_eq!(
            roundtrip.legion_workflow_projection,
            snapshot.legion_workflow_projection
        );
        assert_eq!(
            roundtrip.legion_workflow_projection.rows[0]
                .merge_readiness
                .state,
            legion_protocol::LegionWorkflowMergeReadinessState::WaitingForApproval
        );
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn legion_workflow_commands_emit_projection_only_intents() {
        let mut shell = Shell::empty("legion commands");

        let inspect = shell
            .handle_command(":legion-inspect session:legion:test")
            .expect("legion inspect parses")
            .expect("intent emitted");
        assert_eq!(
            inspect,
            CommandDispatchIntent::InspectLegionWorkflowSession {
                session_id: LegionWorkflowSessionId("session:legion:test".to_string())
            }
        );

        let verify = shell
            .handle_command(":legion-verify session:legion:test verification:unit")
            .expect("legion verification parses")
            .expect("intent emitted");
        assert_eq!(
            verify,
            CommandDispatchIntent::RequestLegionWorkflowVerification {
                session_id: LegionWorkflowSessionId("session:legion:test".to_string()),
                gate_id: LegionWorkflowVerificationGateId("verification:unit".to_string()),
            }
        );

        let readiness = shell
            .handle_command(":legion-readiness session:legion:test")
            .expect("legion readiness parses")
            .expect("intent emitted");
        assert_eq!(
            readiness,
            CommandDispatchIntent::RequestLegionWorkflowMergeReadiness {
                session_id: LegionWorkflowSessionId("session:legion:test".to_string())
            }
        );
        assert_eq!(shell.command_dispatch_intents.len(), 3);
    }

    #[test]
    fn legion_workflow_malformed_command_does_not_emit_privileged_intent() {
        let mut shell = Shell::empty("legion malformed");
        let before = shell.projection_snapshot();

        assert_eq!(
            shell
                .handle_command(":legion-verify session-only")
                .expect("malformed command is ignored"),
            Some(CommandDispatchIntent::Noop)
        );
        assert_eq!(shell.projection_snapshot(), before);
        assert_eq!(
            shell.command_dispatch_intents,
            vec![CommandDispatchIntent::Noop]
        );
    }

    #[test]
    fn ui_plugin_contributions_are_projection_only_command_intents() {
        let mut shell = Shell::empty("plugins");
        shell.plugin_contribution_projections = vec![PluginContributionProjection {
            plugin_id: PluginId(7),
            contributions: vec![legion_protocol::PluginContribution::Command(
                legion_protocol::PluginCommandDescriptor {
                    command_id: "phase5.run".to_string(),
                    title: "Phase 5 Run".to_string(),
                    required_capability: CapabilityId("plugin.command".to_string()),
                },
            )],
            status_label: "loaded".to_string(),
        }];

        let before = shell.projection_snapshot();
        let intent = shell
            .handle_command(":plugin 7 phase5.run metadata-only")
            .expect("plugin command should parse")
            .expect("intent should be emitted");

        assert_eq!(
            intent,
            CommandDispatchIntent::InvokePluginCommand {
                plugin_id: PluginId(7),
                command_id: "phase5.run".to_string(),
                metadata_label: "metadata-only".to_string(),
            }
        );
        assert_eq!(shell.projection_snapshot(), before);
        assert_eq!(shell.command_dispatch_intents.len(), 1);
    }

    #[test]
    fn ui_collaboration_presence_is_projection_only_command_intent() {
        let mut shell = Shell::empty("collaboration");
        shell.collaboration_presence_projections = vec![CollaborationPresenceProjection {
            session_id: CollaborationSessionId(1001),
            participant_id: CollaborationParticipantId(2001),
            cursor: Some(test_coordinate(0, 0)),
            selections: Vec::new(),
            activity_label: Some("editing metadata-only range".to_string()),
            reconnecting: false,
            schema_version: 1,
        }];

        let before = shell.projection_snapshot();
        let intent = shell
            .handle_command(":collab-presence 1001 2001")
            .expect("collaboration command should parse")
            .expect("intent should be emitted");

        assert_eq!(
            intent,
            CommandDispatchIntent::PublishCollaborationPresence {
                session_id: CollaborationSessionId(1001),
                participant_id: CollaborationParticipantId(2001),
            }
        );
        assert_eq!(shell.projection_snapshot(), before);
        assert_eq!(shell.command_dispatch_intents.len(), 1);
    }

    #[test]
    fn explorer_projection_holds_nodes_and_selection() {
        let projection = ExplorerProjection {
            nodes: vec![ExplorerNodeProjection {
                file_id: FileId(10),
                canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
                name: "main.rs".to_string(),
                children: vec![],
            }],
            selection: Some(ExplorerSelectionProjection {
                file_id: FileId(10),
            }),
        };

        assert_eq!(projection.nodes.len(), 1);
        assert_eq!(projection.nodes[0].name, "main.rs");
        assert_eq!(
            projection.selection.map(|sel| sel.file_id),
            Some(FileId(10))
        );
    }
}
