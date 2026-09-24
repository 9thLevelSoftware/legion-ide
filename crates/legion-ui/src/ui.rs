//! Projection-only UI primitives for the native shell.

use legion_protocol::{
    AgentRunId, ArtifactLedgerProjection, AssistedAiProjection, BufferId, BufferVersion,
    CanonicalPath, CapabilityId, CaretAffinity, CheckpointRollbackProjection,
    CollaborationGuiProjection, CollaborationParticipantId, CollaborationPresenceProjection,
    CollaborationSessionId, CommandRegistryProjection, ContextManifestEgressStatus,
    ContextManifestProjection, ContextManifestPurpose, ContextManifestRecord, DebugBreakpointId,
    DebugConfigurationId, DebugSessionId, DebugSessionState, DelegatedTaskProjection,
    DelegatedTaskProposalHunkDisposition, DelegatedTaskRuntimeActivationState,
    DelegatedTaskToolPermissionDecision, ExtensionCatalogEntry, FileFingerprint, FileId,
    LanguageToolingProjection, LegionCloudLaneProjection, LegionWorkflowConflictId,
    LegionWorkflowProjection, LegionWorkflowSessionId, LegionWorkflowSignOffId,
    LegionWorkflowVerificationGateId, LineWrappingPolicy, PermissionBudgetProjection,
    PluginContributionProjection, PluginId, PrivacyInspectorProjection, ProductMode,
    ProductRuntimeSurface, ProposalApprovalChecklistProjection, ProposalCancellationReason,
    ProposalId, ProposalLedgerProjection, ProposalPrivacyLabel, ProposalRejectionReason,
    ProposalRiskLabel, ProposalRollbackReason, ProtocolTextRange, RedactionHint,
    RemoteGuiProjection, SnapshotId, SystemGraphProjection, TerminalPanelProjection,
    TerminalSessionId, TextCoordinate, TimestampMillis, Utf16Range, VerificationRunProjection,
    ViewportLineTruncationState, ViewportScroll, VisualNavigationRequest,
    WorkbenchFontFallbackDiagnostic, WorkbenchTelemetryConsent, WorkspaceId,
    product_mode_allows_runtime_surface,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::projection::{
    LegionWorkflowBoardColumnProjection, LegionWorkflowBudgetUsageRowProjection,
    LegionWorkflowFleetCardProjection,
};

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
    /// Whether this row is a directory.
    ///
    /// The renderer cannot infer this from [`Self::children`]: an empty
    /// directory has none, and a directory whose children have not been
    /// projected yet looks identical to a file. Activating a row does
    /// different things for the two — a file opens, a directory expands — so
    /// the distinction has to come from the authority that knows it.
    pub is_directory: bool,
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

fn normalize_dock_mode_label(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && !matches!(*character, '_' | '-'))
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

impl DockMode {
    /// Stable user-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Assist => "Assist",
            Self::Delegate => "Delegate",
            Self::Automate => "Legion Workflows",
        }
    }

    /// Convert to the shared protocol product mode.
    pub fn to_product_mode(self) -> ProductMode {
        match self {
            Self::Manual => ProductMode::Manual,
            Self::Assist => ProductMode::Assist,
            Self::Delegate => ProductMode::Delegates,
            Self::Automate => ProductMode::LegionWorkflows,
        }
    }

    /// Parse a stable user-facing or persisted mode label.
    pub fn parse(value: &str) -> Option<Self> {
        match normalize_dock_mode_label(value).as_str() {
            "manual" | "m" => Some(Self::Manual),
            "assist" | "a" => Some(Self::Assist),
            "delegate" | "delegates" | "d" => Some(Self::Delegate),
            "automate" | "automation" | "autonomous" | "legion" | "legionworkflows"
            | "workflow" | "workflows" | "w" => Some(Self::Automate),
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
                // AgentLogs is deliberately absent: it declares the Automation
                // runtime surface, which Delegate does not grant, so placing it
                // here only produced a placement the registry always filtered
                // out. Dropping the placement is the fail-closed half of the
                // fix — granting Delegate the Automation surface instead would
                // widen a mode boundary and is not a layout decision.
                bottom: DockSideLayout::new(Terminal, vec![Diagnostics], 0.34, false),
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
    /// Degraded/full state for the active buffer projection.
    pub state: ActiveBufferProjectionState,
    /// Degraded status from the application layer.
    pub degraded: bool,
    /// Bounded small-buffer preview, requested explicitly.
    pub small_buffer_preview: Option<String>,
    /// Dirty indicator projected from the editor engine.
    pub dirty: bool,
}

/// Degraded/full state for the active buffer projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveBufferProjectionState {
    /// Full projection is available.
    Full,
    /// Projection is degraded (streaming, large file, etc.).
    Degraded,
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
            state: ActiveBufferProjectionState::Degraded,
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
    /// `true` when this result belongs to a superseded query.  The desktop
    /// should render stale rows de-emphasised (dimmed) until they are replaced
    /// by results from the current query.
    pub stale: bool,
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
    /// Count of files skipped because they were detected as binary by the
    /// NUL-byte heuristic.  Distinct from `omitted_file_count` which
    /// covers error / oversized skips.
    pub skipped_binary_count: usize,
    /// Effective case-sensitive setting for this search result.
    pub case_sensitive: bool,
    /// Effective whole-word setting for this search result.
    pub whole_word: bool,
    /// Effective regex mode for this search result.
    pub use_regex: bool,
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
            skipped_binary_count: 0,
            case_sensitive: true,
            whole_word: false,
            use_regex: false,
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

// ── Find-bar projections (Phase 4 – Navigation & UI Essentials) ──

/// One projected find-bar match range in protocol text coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMatchProjection {
    /// Start coordinate of the match.
    pub start: TextCoordinate,
    /// End coordinate of the match.
    pub end: TextCoordinate,
}

/// Projection-only find-bar surface for the active buffer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FindBarProjection {
    /// Whether the find bar is visible.
    pub visible: bool,
    /// Current query string.
    pub query: String,
    /// Replacement text.
    pub replace_text: String,
    /// Whether the replace input is visible.
    pub replace_visible: bool,
    /// Whether case-sensitive matching is active.
    pub case_sensitive: bool,
    /// Whether whole-word matching is active.
    pub whole_word: bool,
    /// Whether regex mode is active.
    pub use_regex: bool,
    /// Total match count.
    pub match_count: usize,
    /// Zero-based index of the currently highlighted match.
    pub current_match_index: usize,
    /// Projected match ranges for the active buffer.
    pub matches: Vec<FindMatchProjection>,
}

// ── Keybinding types (Phase 4 – Navigation & UI Essentials) ──

/// A string-based key combination for keybinding dispatch.
///
/// Uses string-based key representation so that `legion-ui` does not depend
/// on any renderer crate.  Conversion to renderer-specific key codes happens
/// in `legion-desktop`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    /// Key label (e.g. `"S"`, `"F3"`, `"Tab"`).
    pub key: String,
    /// Whether the Ctrl modifier is required.
    pub ctrl: bool,
    /// Whether the Shift modifier is required.
    pub shift: bool,
    /// Whether the Alt modifier is required.
    pub alt: bool,
}

impl KeyCombo {
    /// Construct a key combination.
    pub fn new(key: impl Into<String>, ctrl: bool, shift: bool, alt: bool) -> Self {
        Self {
            key: key.into(),
            ctrl,
            shift,
            alt,
        }
    }
}

/// A single keybinding entry mapping a key combination to a command label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingEntry {
    /// Key combination.
    pub combo: KeyCombo,
    /// Action label matching a `CommandDispatchIntent` variant or app command.
    pub action_label: String,
}

/// Return the default keymap entries.
pub fn default_keymap() -> Vec<KeybindingEntry> {
    vec![
        KeybindingEntry {
            combo: KeyCombo::new("S", true, false, false),
            action_label: "SaveActive".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("S", true, true, false),
            action_label: "SaveAll".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F", true, false, false),
            action_label: "ToggleFindBar".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F", true, true, false),
            action_label: "SearchWorkspace".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("H", true, false, false),
            action_label: "ToggleFindReplace".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F3", false, false, false),
            action_label: "FindNext".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F3", false, true, false),
            action_label: "FindPrevious".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("G", true, false, false),
            action_label: "GoToLine".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("G", true, true, false),
            action_label: "StageFocusedGitHunk".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("P", true, false, false),
            action_label: "OpenPalette".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("P", true, true, false),
            action_label: "OpenCommandPalette".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("Z", true, false, false),
            action_label: "Undo".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("Z", true, true, false),
            action_label: "Redo".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("W", true, false, false),
            action_label: "CloseTab".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("Tab", true, false, false),
            action_label: "NextTab".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("Tab", true, true, false),
            action_label: "PrevTab".into(),
        },
        // Ctrl+Alt+Up/Down, matching the convention most editors use for
        // stacking cursors down a column. Plain Ctrl+Up/Down is scroll in many
        // of them, and Alt alone is the menu key on Windows.
        KeybindingEntry {
            combo: KeyCombo::new("ArrowUp", true, false, true),
            action_label: "AddCursorAbove".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("ArrowDown", true, false, true),
            action_label: "AddCursorBelow".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F12", false, false, false),
            action_label: "GoToDefinition".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F8", false, false, false),
            action_label: "ProblemNext".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F8", false, true, false),
            action_label: "ProblemPrev".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F2", false, false, false),
            action_label: "RenameSymbol".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F", false, true, true),
            action_label: "FormatDocument".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("O", true, true, false),
            action_label: "OrganizeImports".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F5", false, false, false),
            action_label: "DebugStart".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F5", false, true, false),
            action_label: "DebugStop".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F9", false, false, false),
            action_label: "ToggleBreakpoint".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F10", false, false, false),
            action_label: "DebugStepOver".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F11", false, false, false),
            action_label: "DebugStepInto".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("F11", false, true, false),
            action_label: "DebugStepOut".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("ArrowUp", false, false, true),
            action_label: "DebugStackPrevious".into(),
        },
        KeybindingEntry {
            combo: KeyCombo::new("ArrowDown", false, false, true),
            action_label: "DebugStackNext".into(),
        },
    ]
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
    /// Whether this hunk only reports a submodule with a dirty worktree.
    ///
    /// Nothing in the parent repository can be staged from it: the recorded
    /// commit has not changed, so `git apply --cached` succeeds without
    /// touching the index and the same control comes back on the next refresh.
    /// A surface that offers it reports a success that changed nothing.
    pub submodule_dirty_only: bool,
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

/// One projected policy decision for a git operation that contacts a remote.
///
/// P2.F5.T4: a network operation may not run without the user being able to see
/// the verdict, so the app layer records one of these for every push/fetch/pull
/// it evaluates — allowed or denied — and the SCM surface renders them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRemotePolicyProjection {
    /// Operation label (`push`, `fetch`, `pull`).
    pub operation: String,
    /// Configured remote name (`origin`).
    pub remote: String,
    /// Classified target (`ssh://github.com`, `local-path`).
    pub target: String,
    /// Host that policy matched on, when the target was a network host.
    ///
    /// `None` for filesystem remotes and for consent rows. The SCM surface uses
    /// this to offer a grant for exactly the host that was denied, so the user
    /// never has to retype it.
    pub host: Option<String>,
    /// Whether policy permitted the operation.
    pub allowed: bool,
    /// Display-safe audit row: metadata only, never credentials or command output.
    pub detail: String,
}

/// One local history entry for the active file, projected for the panel surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalHistoryEntryProjection {
    /// Stable entry identifier.
    pub entry_id: String,
    /// Human-readable timestamp label (seconds since epoch as a string).
    pub timestamp_label: String,
    /// SHA-256 content hash hex string.
    pub content_hash: String,
    /// Content size in bytes.
    pub size_bytes: u64,
}

/// Background Git projection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitRefreshState {
    /// No Git job is pending.
    Idle,
    /// A Git snapshot or mutation is running.
    Refreshing,
    /// The worker exceeded its bounded command timeout.
    TimedOut,
    /// The worker returned a non-timeout failure.
    Failed,
    /// Git requested credentials or another interactive action.
    AuthRequired,
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
    /// Whether `hunks` omits hunks the repository actually has.
    ///
    /// A surface counting what it received cannot tell "twelve of fourteen"
    /// from "twelve of thousands", so a panel that states an exact number of
    /// hidden hunks from a truncated list states a wrong one.
    pub hunks_truncated: bool,
    /// Whether a merge is underway that a bare `git commit` would conclude.
    ///
    /// True mid-merge, where a commit finishes the merge even with an index
    /// identical to `HEAD`. Cherry-pick and revert are deliberately excluded:
    /// git refuses an empty commit for those without `--allow-empty`.
    pub merge_awaiting_commit: bool,
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
    /// Hunk identifier of the currently keyboard-focused hunk in the diff review surface.
    /// `None` when no hunk has been explicitly focused by navigation.
    pub focused_hunk_id: Option<String>,
    /// Advisory commit-message validation warnings (e.g. missing CC prefix).
    /// Empty when the last validated message was clean.
    pub commit_validation_warnings: Vec<String>,
    /// Hard commit-message validation errors (e.g. empty summary, missing author identity).
    /// Non-empty means the commit action is blocked until these are resolved.
    pub commit_validation_errors: Vec<String>,
    /// Local history entries for the currently active file, newest first.
    /// Populated by `RequestLocalHistoryEntries`; empty on idle.
    pub local_history_entries: Vec<LocalHistoryEntryProjection>,
    /// Policy decisions for git operations that contacted a remote, newest last.
    /// Retained across refreshes so the verdict stays visible after the
    /// projection rebuilds; empty until a push/fetch/pull is attempted.
    pub remote_policy_audit: Vec<GitRemotePolicyProjection>,
    /// Current background inspection state.
    pub refresh_state: GitRefreshState,
    /// Whether the displayed rows predate the latest refresh request.
    pub stale: bool,
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
            hunks_truncated: false,
            merge_awaiting_commit: false,
            blame_lines: Vec::new(),
            commits: Vec::new(),
            conflicts: Vec::new(),
            worktrees: Vec::new(),
            diagnostics: Vec::new(),
            generated_at: TimestampMillis(0),
            schema_version: 1,
            focused_hunk_id: None,
            commit_validation_warnings: Vec::new(),
            commit_validation_errors: Vec::new(),
            local_history_entries: Vec::new(),
            remote_policy_audit: Vec::new(),
            refresh_state: GitRefreshState::Idle,
            stale: false,
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
    /// True when the last successful session used a live adapter process (not fixture).
    pub live_adapter: bool,
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
            live_adapter: false,
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

/// One discovered test or benchmark row (metadata-only; no run output).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestExplorerItemProjection {
    /// Stable item id (typically the full cargo test path or lens id).
    pub item_id: String,
    /// Display leaf label.
    pub label: String,
    /// Kind label (`test`, `bench`, or `runnable`).
    pub kind_label: String,
    /// Optional parent module path label.
    pub parent_label: Option<String>,
    /// Optional display-safe run command label (LSP runnable path).
    ///
    /// When present, per-item run launches this terminal command instead of
    /// `cargo test --exact`. Never raw secrets; projection metadata only.
    pub run_command_label: Option<String>,
}

/// Projection-only test explorer surface (P2.F3.T4 discovery + run substrate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestExplorerProjection {
    /// Status label (`idle`, `ready`, `empty`, `error`, `timeout`, `running`).
    pub status_label: String,
    /// Controller label (e.g. `cargo-test`).
    pub controller_label: String,
    /// Discovered items (capped).
    pub items: Vec<TestExplorerItemProjection>,
    /// Display-safe diagnostics (timeouts, caps, spawn failures).
    pub diagnostics: Vec<String>,
    /// Last run item id when a per-item run completed.
    pub last_run_item_id: Option<String>,
    /// Last run status label (`passed`, `failed`, `timeout`, `error`, `empty`).
    pub last_run_status: Option<String>,
    /// Last run process exit code when available.
    pub last_run_exit_code: Option<i32>,
    /// Last run duration in milliseconds.
    pub last_run_duration_ms: Option<u64>,
    /// Projection generation timestamp.
    pub generated_at: TimestampMillis,
    /// Projection schema version.
    pub schema_version: u16,
}

impl TestExplorerProjection {
    /// Construct an idle empty test explorer projection.
    pub fn empty() -> Self {
        Self {
            status_label: "idle".to_string(),
            controller_label: "cargo-test".to_string(),
            items: Vec::new(),
            diagnostics: Vec::new(),
            last_run_item_id: None,
            last_run_status: None,
            last_run_exit_code: None,
            last_run_duration_ms: None,
            generated_at: TimestampMillis(0),
            schema_version: 1,
        }
    }
}

impl Default for TestExplorerProjection {
    fn default() -> Self {
        Self::empty()
    }
}

/// Maximum tree display rows for the desktop Tests panel (group headers + items).
pub const MAX_TEST_EXPLORER_TREE_DISPLAY_ROWS: usize = 48;

/// One module-path group for tree presentation (display-only; items stay flat).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestExplorerGroup<'a> {
    /// Parent module path, or `"<root>"` when absent.
    pub parent_label: String,
    /// Items under this parent, in original order.
    pub items: Vec<&'a TestExplorerItemProjection>,
}

/// Group discovered items by `parent_label` for tree presentation.
///
/// Groups are sorted by parent path (stable); items within a group keep input order.
pub fn group_test_explorer_items_by_parent(
    items: &[TestExplorerItemProjection],
) -> Vec<TestExplorerGroup<'_>> {
    let mut map: std::collections::BTreeMap<String, Vec<&TestExplorerItemProjection>> =
        std::collections::BTreeMap::new();
    for item in items {
        let parent = item
            .parent_label
            .clone()
            .unwrap_or_else(|| "<root>".to_string());
        map.entry(parent).or_default().push(item);
    }
    map.into_iter()
        .map(|(parent_label, items)| TestExplorerGroup {
            parent_label,
            items,
        })
        .collect()
}

/// Format grouped items as display-safe tree rows for the Tests panel.
pub fn format_test_explorer_tree_rows(
    items: &[TestExplorerItemProjection],
    max_rows: usize,
) -> Vec<String> {
    let groups = group_test_explorer_items_by_parent(items);
    let mut rows = Vec::new();
    let mut omitted_items = 0usize;
    for group in groups {
        if rows.len() >= max_rows {
            omitted_items = omitted_items.saturating_add(group.items.len());
            continue;
        }
        rows.push(format!(
            "group {} ({})",
            group.parent_label,
            group.items.len()
        ));
        for item in group.items {
            if rows.len() >= max_rows {
                omitted_items = omitted_items.saturating_add(1);
                continue;
            }
            rows.push(format!(
                "  item {}: kind={} label={}",
                item.item_id, item.kind_label, item.label
            ));
        }
    }
    if omitted_items > 0 {
        rows.push(format!("tree-omitted-items={omitted_items}"));
    }
    rows
}

/// Projection-only metadata row for a supervised language-server health record.
///
/// No authority. All fields are display-safe labels derived from protocol metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerHealthProjection {
    /// Display label for the server identity (e.g. "rust-analyzer#1").
    pub server_label: String,
    /// Display label for the binary provenance (e.g. "system PATH").
    pub provenance_label: String,
    /// Version string reported by the server, or "unknown".
    pub version_label: String,
    /// Display label for the initialization status (e.g. "ready").
    pub status_label: String,
    /// Number of restarts observed in this session.
    pub restart_count: u32,
    /// Whether a policy-gated binary download was refused.
    pub download_refused: bool,
}

/// Maps a protocol [`legion_protocol::LspServerHealthRecord`] to a
/// [`LspServerHealthProjection`] without claiming any product authority.
pub fn project_lsp_health(
    record: &legion_protocol::LspServerHealthRecord,
    download_refused: bool,
) -> LspServerHealthProjection {
    use legion_protocol::{LspResultStatus, LspServerBinaryProvenance as P};

    let provenance_label = match record.binary_provenance {
        P::Configured => "configured path",
        P::ProjectLocal => "project-local",
        P::SystemPath => "system PATH",
        P::Bundled => "bundled",
        P::Downloaded => "downloaded",
    }
    .to_string();

    let status_label = match record.init_status {
        LspResultStatus::Fresh => "ready",
        LspResultStatus::Stale => "stale",
        LspResultStatus::Partial => "partial",
        LspResultStatus::Cancelled => "cancelled",
        LspResultStatus::Timeout => "timed out",
        LspResultStatus::Unavailable => "unavailable",
        LspResultStatus::Degraded => "degraded",
    }
    .to_string();

    LspServerHealthProjection {
        server_label: format!("{}#{}", record.language_id.0, record.server_id.0),
        provenance_label,
        version_label: record.version.clone().unwrap_or_else(|| "unknown".into()),
        status_label,
        restart_count: record.restart_count,
        download_refused,
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

/// Stable product group used to order and label command-palette results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaletteCommandGroup {
    /// Frequently useful workspace-level commands.
    Suggested,
    /// File and buffer commands.
    Files,
    /// View and preference commands.
    View,
    /// Run and language-tool commands.
    Run,
    /// Source-control commands.
    Git,
    /// Destructive commands that require confirmation.
    Destructive,
}

impl PaletteCommandGroup {
    /// User-facing heading for this product group.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Suggested => "Suggested",
            Self::Files => "Files",
            Self::View => "View",
            Self::Run => "Run",
            Self::Git => "Git",
            Self::Destructive => "Destructive",
        }
    }
}

/// Classify a palette result into Legion's canonical command group.
pub fn palette_command_group(result_id: &str) -> PaletteCommandGroup {
    let command_id = result_id.strip_prefix("command:").unwrap_or(result_id);
    if matches!(
        command_id,
        "git-delete-branch"
            | "git-prune-worktrees"
            | "git-remove-worktree"
            | "preferences-settings-reset"
    ) {
        return PaletteCommandGroup::Destructive;
    }
    if command_id.starts_with("git-") || command_id == "refresh-git" {
        return PaletteCommandGroup::Git;
    }
    if command_id.starts_with("lsp-") {
        return PaletteCommandGroup::Run;
    }
    if command_id.starts_with("preferences-")
        || command_id.starts_with("help-")
        || command_id == "close-palette"
    {
        return PaletteCommandGroup::View;
    }
    if matches!(
        command_id,
        "save-all" | "save-active-buffer" | "close-active-tab" | "reveal-active-file"
    ) {
        return PaletteCommandGroup::Files;
    }
    PaletteCommandGroup::Suggested
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

/// App-owned confirmation request for a palette command that can change
/// authority or destroy state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteConfirmationProjection {
    /// Monotonic app-issued token identifying this exact pending request.
    pub token: u64,
    /// Stable command identifier selected when the request was created.
    pub command_id: String,
    /// Canonical parsed operands, excluding the command title or query prefix.
    pub operands: Vec<String>,
    /// User-facing command title.
    pub title: String,
    /// User-facing description of the exact target, when available.
    pub detail: Option<String>,
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
    /// Selected result index, or `results.len()` when no result can be selected.
    pub selected_index: usize,
    /// Ranked palette results.
    pub results: Vec<PaletteResult>,
    /// Pending confirmation owned by the application authority boundary.
    pub pending_confirmation: Option<PaletteConfirmationProjection>,
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
            pending_confirmation: None,
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
    /// Editor line wrapping policy.
    #[serde(default)]
    pub line_wrapping_policy: LineWrappingPolicy,
    /// Optional fixed wrapping column.
    #[serde(default = "default_wrap_column")]
    pub wrap_column: Option<u32>,
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
            line_wrapping_policy: LineWrappingPolicy::Off,
            wrap_column: default_wrap_column(),
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
    /// Editor font family label.
    #[serde(default = "default_editor_font_family_label")]
    pub editor_font_family: String,
    /// Editor font size in points.
    pub editor_font_size_pt: u16,
    /// Metadata-only renderer fallback diagnostics.
    #[serde(default)]
    pub font_fallback_diagnostics: Vec<WorkbenchFontFallbackDiagnostic>,
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
    /// User-level terminal shell preference label (e.g. `"pwsh"`, `"bash"`, `"cmd"`).
    /// Empty string means "use platform default." Workspace-level setting overrides this.
    #[serde(default)]
    pub terminal_shell_selection: String,
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
        self.editor_font_family = normalize_font_family_label(&self.editor_font_family);
        self.editor_font_size_pt = self
            .editor_font_size_pt
            .clamp(Self::MIN_EDITOR_FONT_SIZE_PT, Self::MAX_EDITOR_FONT_SIZE_PT);
        self.font_fallback_diagnostics.truncate(8);
        self.editor.wrap_column = match self.editor.line_wrapping_policy {
            LineWrappingPolicy::FixedColumn => {
                Some(self.editor.wrap_column.unwrap_or(120).clamp(40, 240))
            }
            LineWrappingPolicy::Off | LineWrappingPolicy::Viewport => None,
        };
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

fn normalize_font_family_label(value: &str) -> String {
    let label = value.trim();
    if label.is_empty() {
        return default_editor_font_family_label();
    }

    let normalized = label
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
        .take(64)
        .collect::<String>();
    if normalized.trim().is_empty() {
        default_editor_font_family_label()
    } else {
        normalized
    }
}

fn default_editor_font_family_label() -> String {
    "monospace".to_string()
}

fn default_wrap_column() -> Option<u32> {
    Some(120)
}

impl Default for SettingsProjection {
    fn default() -> Self {
        Self {
            theme_preference: ThemePreferenceProjection::Dark,
            zoom_percent: 100,
            editor_font_family: default_editor_font_family_label(),
            editor_font_size_pt: 12,
            font_fallback_diagnostics: Vec::new(),
            toast_verbosity: ToastVerbosityProjection::WarningsAndErrors,
            editor: EditorSettingsProjection::default(),
            telemetry: WorkbenchTelemetryConsent::default(),
            indexed_workspace_search_enabled: false,
            next_edit_prediction_enabled: false,
            terminal_shell_selection: String::new(),
            schema_version: 1,
        }
    }
}

/// Semantic editor boundary requested by native input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorBoundaryKind {
    /// Start of the current logical line.
    LineStart,
    /// End of the current logical line.
    LineEnd,
    /// Start of the document.
    DocumentStart,
    /// End of the document.
    DocumentEnd,
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
    /// Configure the app-owned TypeScript toolchain from approved archive and runtime metadata.
    ConfigureTypeScriptToolchain {
        /// Archive path for the language server package.
        server_archive: String,
        /// Archive path for the TypeScript compiler package.
        compiler_archive: String,
        /// Node executable path selected by app authority.
        node_executable: String,
    },
    /// Clear the app-owned TypeScript toolchain configuration.
    ClearTypeScriptToolchain,
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
    /// Replace every directed caret range through editor authority.
    ReplaceDirectedCarets {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Replacement or insertion payload.
        text: String,
    },
    /// Delete each directed caret's selection or adjacent grapheme cluster.
    DeleteDirectedCarets {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Delete toward the document start when true; otherwise toward the end.
        backward: bool,
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
    /// Set a directed pointer selection while preserving anchor/head order.
    SetDirectedSelection {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Fixed selection anchor.
        anchor: TextCoordinate,
        /// Current selection head.
        head: TextCoordinate,
    },
    /// Place the visual cursor using the rendered wrap-side affinity and layout identity.
    SetVisualCursor {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Layout snapshot identity.
        expected_snapshot_id: SnapshotId,
        /// Layout buffer version.
        expected_buffer_version: BufferVersion,
        /// Cursor coordinate from projection space.
        cursor: TextCoordinate,
        /// Rendered wrap-side affinity.
        affinity: CaretAffinity,
    },
    /// Place a visual directed selection using the rendered wrap-side affinity.
    SetVisualDirectedSelection {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Layout snapshot identity.
        expected_snapshot_id: SnapshotId,
        /// Layout buffer version.
        expected_buffer_version: BufferVersion,
        /// Fixed selection anchor.
        anchor: TextCoordinate,
        /// Current selection head.
        head: TextCoordinate,
        /// Rendered wrap-side affinity for the head.
        head_affinity: CaretAffinity,
    },
    /// Move every ordered caret through app-owned shaped visual-row facts.
    MoveVertically {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Renderer-shaped vertical movement request.
        request: VisualNavigationRequest,
    },
    /// Copy the current editor selection through app-owned metadata-only clipboard authority.
    ClipboardCopy {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Cut the current editor selection through app/editor authority.
    ClipboardCut {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Add a cursor one line above each existing one.
    AddCursorAbove {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Add a cursor one line below each existing one.
    AddCursorBelow {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Collapse back to a single cursor.
    ///
    /// Its own intent rather than a side effect of clicking, because a user who
    /// has built a ten-cursor set needs a way to leave it that does not also
    /// move the caret somewhere.
    ClearExtraCursors {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Select the entire target buffer through editor authority.
    SelectAll {
        /// Target buffer identifier.
        buffer_id: BufferId,
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
    /// Reorder a tab to a new position through app authority.
    ReorderTab {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Zero-based target index in the tab list.
        new_index: usize,
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
    /// Move every active caret to a semantic editor boundary.
    MoveToBoundary {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Requested line or document boundary.
        boundary: EditorBoundaryKind,
        /// Preserve or initialize directed selection anchors.
        extend: bool,
    },
    /// Move every active caret one grapheme boundary horizontally.
    MoveHorizontally {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Move toward the document start when true, otherwise toward the end.
        left: bool,
        /// Preserve or initialize directed selection anchors.
        extend: bool,
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
    /// Confirm an app-owned pending palette command after revalidating its identity.
    ConfirmPaletteSelection {
        /// App-issued confirmation token.
        token: u64,
        /// Stable pending command identifier.
        command_id: String,
        /// Canonical parsed operands projected for this exact command.
        operands: Vec<String>,
    },
    /// Cancel an app-owned pending palette confirmation.
    CancelPaletteConfirmation {
        /// App-issued confirmation token to cancel.
        token: u64,
    },
    /// Open the projected Settings surface.
    OpenSettings,
    /// Open the Help/About overlay.
    OpenAbout,
    /// Export a metadata-only support bundle through app authority.
    ExportSupportBundle,
    /// Attach an optional local ACP adapter host for delegated proposal work.
    AttachAcpHost {
        /// Executable or program path.
        program: String,
        /// Arguments passed to the local adapter host.
        args: Vec<String>,
    },
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
    /// Update the app-owned editor font family.
    SetEditorFontFamily {
        /// Requested editor font family label.
        family: String,
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
    /// Update editor line wrapping policy.
    SetLineWrappingPolicy {
        /// Requested line wrapping policy.
        policy: LineWrappingPolicy,
        /// Optional fixed wrap column.
        wrap_column: Option<u32>,
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
        /// Explicit case-sensitive override; `None` defers to text-prefix parsing.
        case_sensitive: Option<bool>,
        /// Explicit whole-word override; `None` defers to text-prefix parsing.
        whole_word: Option<bool>,
        /// Explicit regex mode override; `None` defers to text-prefix parsing.
        use_regex: Option<bool>,
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
    /// Stage every change to one path, hunk or not.
    StageGitPath {
        /// Repository-relative path to stage.
        path: String,
    },
    /// Unstage every change to one path.
    UnstageGitPath {
        /// Repository-relative path to unstage.
        path: String,
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
    /// Fetch refs from a remote without touching the working tree.
    FetchGitRemote {
        /// Remote name.
        remote: String,
    },
    /// Pull the current branch from a remote.
    PullGitRemote {
        /// Remote name.
        remote: String,
    },
    /// Record user consent to reach a host for git remote operations.
    GrantGitRemoteHost {
        /// Host to consent to (`github.com`).
        host: String,
    },
    /// Withdraw consent for a git remote host.
    RevokeGitRemoteHost {
        /// Host to revoke.
        host: String,
    },
    /// Prune orphaned worktree metadata.
    PruneGitWorktrees,
    /// Remove a projected worktree by path.
    RemoveGitWorktree {
        /// Projected worktree path.
        path: String,
    },
    /// Create a new git worktree at the given path, optionally checking out a new branch.
    CreateGitWorktree {
        /// Branch name to create or check out.
        branch: String,
        /// Filesystem path for the new worktree (absolute or relative to workspace root).
        worktree_path: String,
    },
    /// Navigate to the next diff hunk in the diff review surface.
    GitNavNextHunk,
    /// Navigate to the previous diff hunk in the diff review surface.
    GitNavPrevHunk,
    /// Navigate to the first hunk in the next changed file.
    GitNavNextFile,
    /// Navigate to the first hunk in the previous changed file.
    GitNavPrevFile,
    /// Stage the hunk currently focused in the Git review surface.
    StageFocusedGitHunk,
    /// Request local history entries for the given canonical file path.
    RequestLocalHistoryEntries {
        /// Canonical path of the file to fetch history for.
        path: String,
    },
    /// Restore a file from a local history entry via proposal route.
    RestoreFromLocalHistory {
        /// Canonical path of the file to restore.
        path: String,
        /// Entry identifier from a prior `RequestLocalHistoryEntries` response.
        entry_id: String,
    },
    /// Export worktree state evidence to `.legion/evidence/` as a metadata-only TOML.
    ExportWorktreeEvidence,
    /// Validate a git commit message and surface warnings to the projection.
    ValidateGitCommitMessage {
        /// Draft commit message to validate.
        message: String,
    },
    /// Refresh debugger configuration projections.
    RefreshDebugConfigurations,
    /// Refresh test explorer discovery (cargo test --list).
    RefreshTestExplorer,
    /// Run one discovered test explorer item via cargo exact filter.
    RunTestExplorerItem {
        /// Discovered item id (cargo test path).
        item_id: String,
    },
    /// Run all tests under a module/group path (cargo substring filter).
    RunTestExplorerGroup {
        /// Parent module path label from tree grouping.
        parent_label: String,
    },
    /// Attach recent test-explorer evidence into a Legion workflow session.
    AttachTestExplorerEvidence {
        /// Workflow session id label.
        session_id: String,
    },

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
    /// Stop / disconnect a debug session (live adapter disconnect when active).
    StopDebugSession {
        /// Session identifier selected from projection data.
        session_id: DebugSessionId,
    },
    /// Poll a live debug session after non-blocking continue.
    PollDebugSession {
        /// Session identifier selected from projection data.
        session_id: DebugSessionId,
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
    /// Refresh inlay hints for the active document through app-owned language tooling.
    RefreshInlayHints {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Refresh code lenses for the active document through app-owned language tooling.
    ///
    /// This is also how runnables arrive: rust-analyzer publishes Run and Debug
    /// as code lenses, and `ActivateLanguageCodeLens` executes the one you pick.
    RefreshCodeLenses {
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
    /// Request bounded live code-action metadata for a document range.
    RequestCodeActions {
        /// Target buffer identifier.
        buffer_id: legion_protocol::BufferId,
        /// UTF-16 document range used as the code-action context.
        range: legion_protocol::ProtocolTextRange,
    },
    /// Select one candidate from a previously projected code-action response.
    SelectCodeAction {
        /// Opaque response identity that produced the candidate.
        response_id: String,
        /// Opaque candidate token scoped to `response_id`.
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
    /// Prepare call hierarchy at the cursor position through app-owned language tooling.
    PrepareCallHierarchy {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position from projection space.
        position: TextCoordinate,
    },
    /// Show incoming calls for a call-hierarchy item through app-owned language tooling.
    ShowIncomingCalls {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position from projection space.
        position: TextCoordinate,
    },
    /// Show outgoing calls for a call-hierarchy item through app-owned language tooling.
    ShowOutgoingCalls {
        /// Target buffer identifier.
        buffer_id: BufferId,
        /// Cursor position from projection space.
        position: TextCoordinate,
    },
    /// Launch a policy-gated terminal session through app authority.
    TerminalLaunch {
        /// Display-safe command label or fixture command.
        command_label: String,
        /// Optional session-timeout override in seconds.
        ///
        /// When `None`, the product default (30 s) applies.  Operators that
        /// need a longer deadline (e.g. a CI smoke running `cargo test` on a
        /// cold builder cache) may pass a larger value here; the policy
        /// contract enforces the tighter of this value and the platform limit.
        timeout_secs: Option<u64>,
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
        /// Optional text selection range to scope the proposal to.
        selection: Option<ProtocolTextRange>,
    },
    /// Send a Delegate chat turn with codebase-context retrieval.
    SendDelegateChat {
        /// Display-safe prompt label.
        prompt_label: String,
    },
    /// Start a delegated task loop using the native agent loop.
    StartDelegatedTask {
        /// Display-safe task description.
        task_description: String,
        /// Scope for the delegated task.
        scope: legion_protocol::DelegatedTaskScope,
    },
    /// Cancel the currently running delegated task loop via the shared cancellation flag.
    CancelDelegatedTask,
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
    /// Record the user's decision on exactly one extension permission (P7.F2.T2).
    ///
    /// One capability per intent. There is deliberately no "trust this
    /// extension" intent that would decide several at once.
    SetExtensionPermission {
        /// Manifest id of the extension being reviewed.
        manifest_id: String,
        /// The single capability this decision applies to.
        capability: CapabilityId,
        /// Whether the user granted that one capability.
        granted: bool,
    },
    /// Cancel an in-flight Cloud Lane task through app authority (P9.F3.T3).
    CancelCloudLaneTask {
        /// Task id selected from Cloud Lane projection data.
        task_id: String,
        /// Display-safe reason recorded with the cancellation.
        reason_label: String,
    },
    /// Install a signed extension through app-owned extension authority (P7.F2.T1).
    InstallExtension {
        /// Manifest id selected from catalog projection data.
        manifest_id: String,
    },
    /// Update an installed extension through app-owned extension authority.
    UpdateExtension {
        /// Manifest id selected from catalog projection data.
        manifest_id: String,
    },
    /// Remove an installed extension through app-owned extension authority.
    RemoveExtension {
        /// Manifest id selected from catalog projection data.
        manifest_id: String,
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
    /// Start the language server session for the active workspace (PKT-LSP-C T1).
    ///
    /// Triggers lazy session startup without opening a buffer.  Safe to call if
    /// the session is already Starting or Live (becomes a no-op via
    /// `LspSessionHandle::start_for_workspace`).
    LspStartSession,
    /// Restart the language server session, resetting the circuit breaker
    /// so recovery is attempted even after repeated crashes (PKT-LSP-C T1/T3).
    ///
    /// Routes through the same start path as `LspStartSession` after clearing
    /// any prior Refused/Failed/BackingOff state.
    LspRestartSession,

    // ── Find/Replace intents (Phase 4 – Navigation & UI Essentials) ──
    /// Toggle the in-editor find bar visibility.
    ToggleFindBar,
    /// Close the in-editor find bar.
    CloseFindBar,
    /// Set the find bar query string.
    SetFindQuery {
        /// Query text entered by the user.
        query: String,
    },
    /// Navigate to the next find match.
    FindNext,
    /// Navigate to the previous find match.
    FindPrevious,
    /// Toggle the replace input within the find bar.
    ToggleFindReplace,
    /// Set the find-bar replacement text.
    SetFindReplaceText {
        /// Replacement text entered by the user.
        text: String,
    },
    /// Replace the current match with the replacement text.
    ReplaceOne,
    /// Replace all matches with the replacement text.
    ReplaceAll,
    /// Toggle case-sensitive matching.
    SetFindCaseSensitive {
        /// Whether case-sensitive matching is enabled.
        enabled: bool,
    },
    /// Toggle whole-word matching.
    SetFindWholeWord {
        /// Whether whole-word matching is enabled.
        enabled: bool,
    },
    /// Toggle regex-based matching.
    SetFindRegex {
        /// Whether regex mode is enabled.
        enabled: bool,
    },

    // ── Vim modal editing intents ──
    /// Enable or disable Vim modal editing.
    SetVimModeEnabled(bool),
    /// Execute a Vim cursor motion with an optional repeat count.
    VimMotion {
        /// The motion to execute.
        motion: crate::vim::VimMotionKind,
        /// Repeat count (minimum 1).
        count: usize,
    },
    /// Execute a Vim operator applied to a motion-defined range.
    VimOperatorMotion {
        /// The operator to apply.
        operator: crate::vim::VimOperatorKind,
        /// Repeat count (minimum 1).
        count: usize,
        /// The motion that defines the range.
        motion: crate::vim::VimMotionKind,
    },
    /// Execute a Vim line-wise operator (e.g. `dd`, `yy`).
    VimLinewiseOperator {
        /// The operator to apply.
        operator: crate::vim::VimOperatorKind,
        /// Number of lines (minimum 1).
        count: usize,
    },
    /// Change the Vim editing mode.
    VimChangeMode(crate::vim::EditorInputMode),
    /// Enter insert mode before the cursor.
    VimInsertBefore,
    /// Enter insert mode after the cursor.
    VimInsertAfter,
    /// Open a new line below and enter insert mode.
    VimInsertLineBelow,
    /// Open a new line above and enter insert mode.
    VimInsertLineAbove,
    /// Put (paste) from the Vim register.
    VimPut,
    /// Begin Vim forward search.
    VimSearchForward,
    /// Delete the character under the cursor.
    VimDeleteChar,
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
            .enumerate()
            .filter(|(_, message)| toast_severity_included(message.severity, verbosity))
            .map(|(index, message)| ToastProjection::from_status_message(message, index))
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
    ///
    /// `index` is the position of the message within its source status-message
    /// list and is folded into the toast id so that two identical status
    /// messages produce distinct ids (dismissing one no longer dismisses all).
    pub fn from_status_message(message: &StatusMessageProjection, index: usize) -> Self {
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
            id: toast_id(message.severity, &message.message, index),
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

fn toast_id(severity: StatusSeverity, message: &str, index: usize) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash ^= match severity {
        StatusSeverity::Info => 0,
        StatusSeverity::Warning => 1,
        StatusSeverity::Error => 2,
    };
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    for byte in (index as u64).to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for byte in message.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Escape control characters in untrusted projection text before it is written
/// to a terminal, preventing ANSI/escape-sequence injection and terminal
/// corruption. C0 controls (except newline and tab), DEL, and C1 controls are
/// rendered as visible `\xNN` escapes; all other characters pass through.
fn sanitize_terminal_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\n' | '\t' => out.push(ch),
            c => {
                let code = c as u32;
                if code < 0x20 || code == 0x7f || (0x80..=0x9f).contains(&code) {
                    out.push_str(&format!("\\x{code:02x}"));
                } else {
                    out.push(c);
                }
            }
        }
    }
    out
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
    /// Legion workflow board columns supplied by the application layer.
    pub legion_workflow_board_columns: Vec<LegionWorkflowBoardColumnProjection>,
    /// Legion workflow fleet-card projections supplied by the application layer.
    pub legion_workflow_fleet_card_projections: Vec<LegionWorkflowFleetCardProjection>,
    /// Tagged Legion workflow communication rows supplied by the application layer.
    pub legion_workflow_comm_rows: Vec<String>,
    /// Per-worker Legion workflow budget rows supplied by the application layer.
    pub legion_workflow_budget_rows: Vec<LegionWorkflowBudgetUsageRowProjection>,
    /// Plugin contribution projections supplied by the application layer.
    pub plugin_contribution_projections: Vec<PluginContributionProjection>,
    /// Extension catalog entries supplied by the application layer (P7.F2).
    pub extension_catalog: Vec<ExtensionCatalogEntry>,
    /// Cloud Lane task projection supplied by the application layer (P9.F3.T3).
    pub legion_cloud_lane: LegionCloudLaneProjection,
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
    /// In-editor find-bar projection.
    pub find_bar_projection: FindBarProjection,
    /// Structural search projection supplied by the application layer.
    pub structural_search_projection: StructuralSearchProjection,
    /// Git status, syntactic diff, blame, graph, and conflict projection supplied by app layer.
    pub git_projection: GitProjection,
    /// Debugger projection supplied by the application layer.
    pub debug_projection: DebugProjection,
    /// Test explorer projection supplied by the application layer.
    pub test_explorer_projection: TestExplorerProjection,
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
    /// A command supplied a byte offset that is out of bounds or not on a
    /// UTF-8 character boundary for the active buffer projection.
    #[error("command position is out of bounds or not on a character boundary")]
    InvalidPosition,
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
    /// Static Legion workflow board columns.
    pub legion_workflow_board_columns: Vec<LegionWorkflowBoardColumnProjection>,
    /// Static Legion workflow fleet cards.
    pub legion_workflow_fleet_card_projections: Vec<LegionWorkflowFleetCardProjection>,
    /// Static tagged Legion workflow communication rows.
    pub legion_workflow_comm_rows: Vec<String>,
    /// Static per-worker Legion workflow budget rows.
    pub legion_workflow_budget_rows: Vec<LegionWorkflowBudgetUsageRowProjection>,
    /// Static plugin contribution projections.
    pub plugin_contribution_projections: Vec<PluginContributionProjection>,
    /// Static extension catalog entries (P7.F2).
    pub extension_catalog: Vec<ExtensionCatalogEntry>,
    /// Static Cloud Lane task projection (P9.F3.T3).
    pub legion_cloud_lane: LegionCloudLaneProjection,
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
    /// In-editor find-bar projection.
    pub find_bar_projection: FindBarProjection,
    /// Static structural search projection.
    pub structural_search_projection: StructuralSearchProjection,
    /// Static git projection.
    pub git_projection: GitProjection,
    /// Static debugger projection.
    pub debug_projection: DebugProjection,
    /// Static test explorer projection.
    pub test_explorer_projection: TestExplorerProjection,
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
            legion_workflow_board_columns: snapshot.legion_workflow_board_columns,
            legion_workflow_fleet_card_projections: snapshot.legion_workflow_fleet_card_projections,
            legion_workflow_comm_rows: snapshot.legion_workflow_comm_rows,
            legion_workflow_budget_rows: snapshot.legion_workflow_budget_rows,
            plugin_contribution_projections: snapshot.plugin_contribution_projections,
            extension_catalog: snapshot.extension_catalog,
            legion_cloud_lane: snapshot.legion_cloud_lane,
            collaboration_presence_projections: snapshot.collaboration_presence_projections,
            collaboration_gui_projection: snapshot.collaboration_gui_projection,
            remote_gui_projection: snapshot.remote_gui_projection,
            daily_editing_projection: snapshot.daily_editing_projection,
            excerpt_surface_projection: snapshot.excerpt_surface_projection,
            search_projection: snapshot.search_projection,
            find_bar_projection: snapshot.find_bar_projection,
            structural_search_projection: snapshot.structural_search_projection,
            git_projection: snapshot.git_projection,
            debug_projection: snapshot.debug_projection,
            test_explorer_projection: snapshot.test_explorer_projection,
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
            legion_workflow_board_columns: Vec::new(),
            legion_workflow_fleet_card_projections: Vec::new(),
            legion_workflow_comm_rows: Vec::new(),
            legion_workflow_budget_rows: Vec::new(),
            plugin_contribution_projections: Vec::new(),
            extension_catalog: Vec::new(),
            legion_cloud_lane: LegionCloudLaneProjection::disabled(),
            collaboration_presence_projections: Vec::new(),
            collaboration_gui_projection: CollaborationGuiProjection::disabled(),
            remote_gui_projection: RemoteGuiProjection::disabled(),
            daily_editing_projection: DailyEditingProjection::empty(),
            excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
            search_projection: SearchProjection::idle(),
            find_bar_projection: FindBarProjection::default(),
            structural_search_projection: StructuralSearchProjection::idle(),
            git_projection: GitProjection::idle(),
            debug_projection: DebugProjection::empty(),
            test_explorer_projection: TestExplorerProjection::empty(),
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
            legion_workflow_board_columns: self.legion_workflow_board_columns.clone(),
            legion_workflow_fleet_card_projections: self
                .legion_workflow_fleet_card_projections
                .clone(),
            legion_workflow_comm_rows: self.legion_workflow_comm_rows.clone(),
            legion_workflow_budget_rows: self.legion_workflow_budget_rows.clone(),
            plugin_contribution_projections: self.plugin_contribution_projections.clone(),
            extension_catalog: self.extension_catalog.clone(),
            legion_cloud_lane: self.legion_cloud_lane.clone(),
            collaboration_presence_projections: self.collaboration_presence_projections.clone(),
            collaboration_gui_projection: self.collaboration_gui_projection.clone(),
            remote_gui_projection: self.remote_gui_projection.clone(),
            daily_editing_projection: self.daily_editing_projection.clone(),
            excerpt_surface_projection: self.excerpt_surface_projection.clone(),
            search_projection: self.search_projection.clone(),
            find_bar_projection: self.find_bar_projection.clone(),
            structural_search_projection: self.structural_search_projection.clone(),
            git_projection: self.git_projection.clone(),
            debug_projection: self.debug_projection.clone(),
            test_explorer_projection: self.test_explorer_projection.clone(),
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
        self.legion_workflow_board_columns = snapshot.legion_workflow_board_columns;
        self.legion_workflow_fleet_card_projections =
            snapshot.legion_workflow_fleet_card_projections;
        self.legion_workflow_comm_rows = snapshot.legion_workflow_comm_rows;
        self.legion_workflow_budget_rows = snapshot.legion_workflow_budget_rows;
        self.plugin_contribution_projections = snapshot.plugin_contribution_projections;
        self.extension_catalog = snapshot.extension_catalog;
        self.collaboration_presence_projections = snapshot.collaboration_presence_projections;
        self.collaboration_gui_projection = snapshot.collaboration_gui_projection;
        self.remote_gui_projection = snapshot.remote_gui_projection;
        self.daily_editing_projection = snapshot.daily_editing_projection;
        self.excerpt_surface_projection = snapshot.excerpt_surface_projection;
        self.search_projection = snapshot.search_projection;
        self.find_bar_projection = snapshot.find_bar_projection;
        self.structural_search_projection = snapshot.structural_search_projection;
        self.git_projection = snapshot.git_projection;
        self.debug_projection = snapshot.debug_projection;
        self.test_explorer_projection = snapshot.test_explorer_projection;
        self.language_tooling_projection = snapshot.language_tooling_projection;
        self.terminal_panel_projection = snapshot.terminal_panel_projection;
    }

    /// Drain queued command-dispatch intents.
    pub fn drain_command_dispatch_intents(&mut self) -> Vec<CommandDispatchIntent> {
        // `mem::take` rather than `drain(..).collect()`: draining every element
        // into a fresh `Vec` of the same type allocates a second buffer and
        // copies into it, when the queue can simply be handed over and replaced
        // with an empty one.
        std::mem::take(&mut self.command_dispatch_intents)
    }

    /// Render basic status and file content.
    pub fn render(&self) {
        print!("\x1b[2J\x1b[H");
        println!(
            "{}",
            sanitize_terminal_text(&self.layout_projection.layout.title)
        );
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
                        sanitize_terminal_text(&tab.title),
                        if tab.dirty { " +" } else { "" }
                    )
                })
                .collect::<Vec<_>>();
            println!("Tabs: {}", rows.join(" | "));
        }
        if let Some(prompt) = &self.daily_editing_projection.close_dirty_prompt {
            println!("Close dirty: {}", sanitize_terminal_text(&prompt.message));
        }

        if let Some(text) = self.active_buffer_projection.small_buffer_text() {
            println!("{}", sanitize_terminal_text(text));
        } else if let Some(viewport) = &self.active_buffer_projection.viewport {
            for slice in &viewport.line_slices {
                println!("{}", sanitize_terminal_text(&slice.visible_text));
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
        println!("Path: {}", sanitize_terminal_text(path));
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
                    sanitize_terminal_text(&command.command_id),
                    sanitize_terminal_text(&command.scope),
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
                    sanitize_terminal_text(&row.lifecycle.label),
                    sanitize_terminal_text(&row.title),
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
                    sanitize_terminal_text(&row.artifact_id),
                    row.kind,
                    sanitize_terminal_text(&row.state_label),
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
                    sanitize_terminal_text(&row.run_id),
                    row.state,
                    sanitize_terminal_text(&row.command_class_label),
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
                sanitize_terminal_text(&manifest.manifest_id),
                manifest.items.len(),
                excluded_count,
                sanitize_terminal_text(selected_item_id),
                manifest.omitted_item_count,
                manifest.risk_label,
                manifest.privacy_label,
                manifest.egress
            );
            for item in &manifest.items {
                println!(
                    "- {} {:?} {:?} ranges={} hashes={} risk={:?} privacy={:?}",
                    sanitize_terminal_text(&item.item_id),
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
                    sanitize_terminal_text(&record.exposure_id),
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
                    sanitize_terminal_text(&budget.budget_id),
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
                    sanitize_terminal_text(&provider.provider_id),
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
                    sanitize_terminal_text(&route.request_id),
                    sanitize_terminal_text(&route.provider_id),
                    route.operation_class,
                    route.disposition,
                    route.provider_invocation,
                    route.refused_permission_budget_evaluation_count
                );
            }
            for preview in &assisted.proposal_previews {
                println!(
                    "- preview {} proposal={} readiness={:?} ready_preview={} ready_approval={} ready_apply={} targets={} hunks={} preconditions={}",
                    sanitize_terminal_text(&preview.preview_id),
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
                    sanitize_terminal_text(&prediction.prediction_id),
                    sanitize_terminal_text(&prediction.provider_label),
                    prediction.status,
                    prediction.latency_ms,
                    prediction.stale,
                    sanitize_terminal_text(&prediction.apply_range_label),
                    sanitize_terminal_text(
                        prediction
                            .replacement_preview_label
                            .as_deref()
                            .unwrap_or("<none>")
                    )
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
                    sanitize_terminal_text(
                        row.directive_artifact_id.as_deref().unwrap_or("<none>")
                    ),
                    sanitize_terminal_text(row.spec_artifact_id.as_deref().unwrap_or("<none>")),
                    sanitize_terminal_text(
                        row.task_graph_artifact_id.as_deref().unwrap_or("<none>")
                    ),
                    row.merge_readiness.state,
                    sanitize_terminal_text(&row.display_safe_labels.join("|"))
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
                println!(
                    "- hover {} {}",
                    sanitize_terminal_text(&hover.label),
                    sanitize_terminal_text(&hover.summary)
                );
            }
            for operation in &language.operations {
                println!(
                    "- operation {} {:?} {:?} proposal={:?}",
                    sanitize_terminal_text(&operation.operation_id),
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
                println!("- denial {}", sanitize_terminal_text(denial));
            }
            for row in &terminal.output_rows {
                println!(
                    "- [{}] {}",
                    row.sequence.0,
                    sanitize_terminal_text(&row.redacted_payload)
                );
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
                    sanitize_terminal_text(&config.configuration_id.0),
                    sanitize_terminal_text(&config.adapter_type),
                    sanitize_terminal_text(&config.program_label)
                );
            }
            for breakpoint in &debug.breakpoints {
                println!(
                    "- debug breakpoint {} {}:{} verified={}",
                    sanitize_terminal_text(&breakpoint.breakpoint_id.0),
                    sanitize_terminal_text(&breakpoint.path.0),
                    breakpoint.line,
                    breakpoint.verified
                );
            }
            for frame in &debug.stack_frames {
                println!(
                    "- debug frame {} {}",
                    frame.frame_id,
                    sanitize_terminal_text(&frame.name)
                );
            }
            for variable in &debug.variables {
                println!(
                    "- debug variable {}={}",
                    sanitize_terminal_text(&variable.name),
                    sanitize_terminal_text(&variable.value_label)
                );
            }
            for watch in &debug.watches {
                println!(
                    "- debug watch {}={}",
                    sanitize_terminal_text(&watch.expression_label),
                    sanitize_terminal_text(&watch.value_label)
                );
            }
            for entry in &debug.console {
                println!(
                    "- debug console {}",
                    sanitize_terminal_text(&entry.message_label)
                );
            }
        }
        println!("{}", terminal_command_help());
    }

    pub(crate) fn active_buffer_id(&self) -> Result<BufferId, ShellCommandError> {
        self.active_buffer_projection
            .buffer_id
            .ok_or(ShellCommandError::ActiveBufferMissing)
    }

    pub(crate) fn active_terminal_session_id(
        &self,
    ) -> Result<TerminalSessionId, ShellCommandError> {
        self.terminal_panel_projection
            .active_session_id
            .ok_or(ShellCommandError::ActiveTerminalSessionMissing)
    }

    pub(crate) fn active_debug_session_id(&self) -> Result<DebugSessionId, ShellCommandError> {
        self.debug_projection
            .active_session_id
            .clone()
            .ok_or(ShellCommandError::ActiveDebugSessionMissing)
    }

    pub(crate) fn active_assist_prediction_id(&self) -> Option<String> {
        self.assist_inline_prediction_projection
            .active_prediction
            .as_ref()
            .map(|prediction| prediction.prediction_id.clone())
    }

    pub(crate) fn push_intent(&mut self, intent: CommandDispatchIntent) -> CommandDispatchIntent {
        self.command_dispatch_intents.push(intent.clone());
        intent
    }

    pub(crate) fn command_position(
        &self,
        payload: &str,
    ) -> Result<TextCoordinate, ShellCommandError> {
        if payload.is_empty() {
            return self.parse_pos(0);
        }
        match payload.parse::<usize>() {
            Ok(offset) => self.parse_pos(offset),
            // Non-numeric payloads are not valid offsets; reject rather than
            // silently coercing to the start of the buffer.
            Err(_) => Err(ShellCommandError::InvalidPosition),
        }
    }

    pub(crate) fn parse_pos(
        &self,
        byte_offset: usize,
    ) -> Result<TextCoordinate, ShellCommandError> {
        if let Some(text) = self.active_buffer_projection.small_buffer_text() {
            // Reject offsets past the end of the buffer or that land in the
            // middle of a multi-byte UTF-8 character instead of coercing to
            // (0, 0) and silently mis-counting.
            if byte_offset > text.len() || !text.is_char_boundary(byte_offset) {
                return Err(ShellCommandError::InvalidPosition);
            }
            let prefix = &text.as_bytes()[..byte_offset];
            let line = prefix.iter().filter(|b| **b == b'\n').count() as u32;
            let character = prefix.iter().rev().take_while(|b| **b != b'\n').count() as u32;
            return Ok(protocol_text_coordinate(
                line,
                character,
                Some(byte_offset as u64),
            ));
        }

        if let Some(viewport) = &self.active_buffer_projection.viewport {
            let mut current_offset = 0;
            for (i, slice) in viewport.line_slices.iter().enumerate() {
                let slice_len = slice.visible_text.len() + 1; // +1 for newline
                if current_offset + slice_len > byte_offset {
                    let relative = byte_offset - current_offset;
                    // Guard the character offset against the visible slice so we
                    // do not split a multi-byte UTF-8 character. `relative` may
                    // equal visible_text.len() (the synthetic trailing newline),
                    // which is a valid boundary.
                    if relative < slice.visible_text.len()
                        && !slice.visible_text.is_char_boundary(relative)
                    {
                        return Err(ShellCommandError::InvalidPosition);
                    }
                    let character = relative as u32;
                    let line = viewport.scroll.top_line + i as u32;
                    // Translate the viewport-relative offset into an absolute
                    // buffer byte offset using the slice's byte range, and
                    // validate it lies within that slice.
                    let absolute = slice.byte_range.start + relative as u64;
                    if absolute > slice.byte_range.end {
                        return Err(ShellCommandError::InvalidPosition);
                    }
                    return Ok(protocol_text_coordinate(line, character, Some(absolute)));
                }
                current_offset += slice_len;
            }
            // The offset fell outside every visible slice; reject it rather than
            // returning a bogus (0, 0) coordinate.
            return Err(ShellCommandError::InvalidPosition);
        }

        // No buffer content is projected. Offset 0 is the only meaningful
        // position (buffer start); anything else is out of bounds.
        if byte_offset == 0 {
            Ok(protocol_text_coordinate(0, 0, Some(0)))
        } else {
            Err(ShellCommandError::InvalidPosition)
        }
    }

    pub(crate) fn active_code_action_range(&self) -> Result<ProtocolTextRange, ShellCommandError> {
        if let Some(viewport) = &self.active_buffer_projection.viewport {
            if let Some(range) = viewport.selections.first() {
                return Ok(*range);
            }
            return Ok(ProtocolTextRange {
                start: viewport.cursor,
                end: viewport.cursor,
            });
        }
        let position = self.parse_pos(0)?;
        Ok(ProtocolTextRange {
            start: position,
            end: position,
        })
    }
}

pub(crate) fn protocol_text_coordinate(
    line: u32,
    character: u32,
    byte_offset: Option<u64>,
) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset,
        utf16_offset: None,
    }
}

pub(crate) fn parse_buffer_id(input: Option<&str>) -> Option<BufferId> {
    input
        .and_then(|value| value.trim().parse::<u128>().ok())
        .filter(|value| *value != 0)
        .map(BufferId)
}

pub(crate) fn non_empty_string(input: Option<&str>) -> Option<String> {
    input
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn parse_debug_step_kind(input: &str) -> DebugStepKindProjection {
    match input {
        "continue" | "cont" => DebugStepKindProjection::Continue,
        "into" | "in" => DebugStepKindProjection::Into,
        "out" => DebugStepKindProjection::Out,
        "back" => DebugStepKindProjection::Back,
        _ => DebugStepKindProjection::Over,
    }
}

fn terminal_command_help() -> &'static str {
    "Commands: :mode Manual|Assist|Delegate|Legion Workflows | :i text | :d start,end | :r start,end,text | :w | :wa | :tab id | :tab | :assist-predict offset | :assist-dismiss | :assist-cancel | :close id | :hover | :completion | :definition | :references | :outline | :format | :rename name | :code-action | :debug-configs | :debug-launch id | :debug-step over | :term-launch label | :term-input text | :term-close | :plugin id command | :ai-start label | :ai-explain label | :ai-propose label | :u | :redo | :q"
}

pub(crate) fn parse_dock_mode(input: &str) -> DockMode {
    DockMode::parse(input).unwrap_or(DockMode::Manual)
}

pub(crate) fn parse_delegate_hunk_disposition(
    input: &str,
) -> Option<DelegatedTaskProposalHunkDisposition> {
    match input.trim().to_ascii_lowercase().as_str() {
        "pending" | "p" => Some(DelegatedTaskProposalHunkDisposition::Pending),
        "accept" | "accepted" | "a" => Some(DelegatedTaskProposalHunkDisposition::Accepted),
        "reject" | "rejected" | "r" => Some(DelegatedTaskProposalHunkDisposition::Rejected),
        _ => None,
    }
}

pub(crate) fn parse_delegate_tool_permission_decision(
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

pub(crate) fn parse_legion_tool_permission(
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

pub(crate) fn parse_legion_kill_switch(
    payload: Option<&str>,
) -> Option<(LegionWorkflowSessionId, String)> {
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
        provider_routes: Vec::new(),
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

pub(crate) fn parse_proposal_id(payload: Option<&str>) -> Option<ProposalId> {
    payload
        .and_then(|value| value.trim().parse::<u64>().ok())
        // ProposalId(0) is reserved as a sentinel (see
        // empty_approval_checklist_projection / empty_checkpoint_rollback_projection),
        // so reject it here just as parse_buffer_id rejects BufferId(0).
        .filter(|value| *value != 0)
        .map(ProposalId)
}

pub(crate) fn parse_legion_session_id(payload: Option<&str>) -> Option<LegionWorkflowSessionId> {
    payload
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| LegionWorkflowSessionId(value.to_string()))
}

pub(crate) fn parse_legion_session_label(
    payload: Option<&str>,
) -> Option<(LegionWorkflowSessionId, String)> {
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

pub(crate) fn parse_legion_session_proposal(
    payload: Option<&str>,
) -> Option<(LegionWorkflowSessionId, ProposalId)> {
    let (session_id, proposal_id) = parse_legion_session_label(payload)?;
    let proposal_id = proposal_id.parse::<u64>().ok().map(ProposalId)?;
    Some((session_id, proposal_id))
}

pub(crate) fn parse_collaboration_session_id(
    payload: Option<&str>,
) -> Option<CollaborationSessionId> {
    payload
        .and_then(|value| value.trim().parse::<u128>().ok())
        .filter(|value| *value != 0)
        .map(CollaborationSessionId)
}

#[cfg(test)]
#[path = "ui_shell_tests.rs"]
mod tests;
