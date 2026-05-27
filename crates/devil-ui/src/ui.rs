//! Projection-only UI primitives for the native shell.

use devil_protocol::{
    AgentRunId, AssistedAiProjection, BufferId, CanonicalPath, CheckpointRollbackProjection,
    CollaborationParticipantId, CollaborationPresenceProjection, CollaborationSessionId,
    ContextManifestEgressStatus, ContextManifestProjection, ContextManifestPurpose,
    ContextManifestRecord, DelegatedTaskProjection, DelegatedTaskRuntimeActivationState, FileId,
    LanguageToolingProjection, PermissionBudgetProjection, PluginContributionProjection, PluginId,
    PrivacyInspectorProjection, ProposalApprovalChecklistProjection, ProposalCancellationReason,
    ProposalId, ProposalLedgerProjection, ProposalPrivacyLabel, ProposalRejectionReason,
    ProposalRiskLabel, ProposalRollbackReason, ProtocolTextRange, RedactionHint,
    TerminalPanelProjection, TerminalSessionId, TextCoordinate, TimestampMillis, ViewportScroll,
    WorkspaceId,
};
use thiserror::Error;

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
    pub viewport: Option<devil_protocol::ViewportProjection>,
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

/// Typed command intent emitted by UI input handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDispatchIntent {
    /// No command was recognized.
    Noop,
    /// Quit the active shell loop.
    Quit,
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
    /// Run bounded lexical search through app authority.
    RunSearch {
        /// Search scope.
        scope: SearchScopeProjection,
        /// User-provided query.
        query: String,
        /// Requested result limit; zero means app default.
        limit: usize,
    },
    /// Cancel the currently projected search by query id.
    CancelSearch {
        /// Query id to cancel.
        query_id: String,
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

/// Projection snapshot provided to the shell by the application layer.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellProjectionSnapshot {
    /// Layout projection.
    pub layout_projection: ShellLayoutProjection,
    /// Explorer projection.
    pub explorer_projection: ExplorerProjection,
    /// Active buffer projection.
    pub active_buffer_projection: ActiveBufferProjection,
    /// Status message projections.
    pub status_messages: Vec<StatusMessageProjection>,
    /// Proposal ledger projection supplied by the application layer.
    pub proposal_ledger_projection: ProposalLedgerProjection,
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
    /// Delegated-task plan projection supplied by the application layer.
    pub delegated_task_projection: DelegatedTaskProjection,
    /// Plugin contribution projections supplied by the application layer.
    pub plugin_contribution_projections: Vec<PluginContributionProjection>,
    /// Collaboration presence projections supplied by the application layer.
    pub collaboration_presence_projections: Vec<CollaborationPresenceProjection>,
    /// Daily-editing projection supplied by the application layer.
    pub daily_editing_projection: DailyEditingProjection,
    /// Search projection supplied by the application layer.
    pub search_projection: SearchProjection,
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
}

/// Projection-only IDE shell state.
#[derive(Debug)]
pub struct Shell {
    /// Projection-only layout state.
    pub layout_projection: ShellLayoutProjection,
    /// Projection-only explorer state.
    pub explorer_projection: ExplorerProjection,
    /// Projection-only active buffer state.
    pub active_buffer_projection: ActiveBufferProjection,
    /// Projected status messages.
    pub status_messages: Vec<StatusMessageProjection>,
    /// Static proposal ledger projection.
    pub proposal_ledger_projection: ProposalLedgerProjection,
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
    /// Static delegated-task plan projection.
    pub delegated_task_projection: DelegatedTaskProjection,
    /// Static plugin contribution projections.
    pub plugin_contribution_projections: Vec<PluginContributionProjection>,
    /// Static collaboration presence projections.
    pub collaboration_presence_projections: Vec<CollaborationPresenceProjection>,
    /// Static daily-editing projection.
    pub daily_editing_projection: DailyEditingProjection,
    /// Static search projection.
    pub search_projection: SearchProjection,
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
            explorer_projection: snapshot.explorer_projection,
            active_buffer_projection: snapshot.active_buffer_projection,
            status_messages: snapshot.status_messages,
            proposal_ledger_projection: snapshot.proposal_ledger_projection,
            context_manifest_projection: snapshot.context_manifest_projection,
            privacy_inspector_projection: snapshot.privacy_inspector_projection,
            permission_budget_projection: snapshot.permission_budget_projection,
            approval_checklist_projection: snapshot.approval_checklist_projection,
            checkpoint_rollback_projection: snapshot.checkpoint_rollback_projection,
            assisted_ai_projection: snapshot.assisted_ai_projection,
            delegated_task_projection: snapshot.delegated_task_projection,
            plugin_contribution_projections: snapshot.plugin_contribution_projections,
            collaboration_presence_projections: snapshot.collaboration_presence_projections,
            daily_editing_projection: snapshot.daily_editing_projection,
            search_projection: snapshot.search_projection,
            language_tooling_projection: snapshot.language_tooling_projection,
            terminal_panel_projection: snapshot.terminal_panel_projection,
            command_dispatch_intents: Vec::new(),
        }
    }

    /// Create an empty projection-only shell.
    pub fn empty(title: impl Into<String>) -> Self {
        Self::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain(title),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: empty_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        })
    }

    /// Return a cloned shell projection snapshot.
    pub fn projection_snapshot(&self) -> ShellProjectionSnapshot {
        ShellProjectionSnapshot {
            layout_projection: self.layout_projection.clone(),
            explorer_projection: self.explorer_projection.clone(),
            active_buffer_projection: self.active_buffer_projection.clone(),
            status_messages: self.status_messages.clone(),
            proposal_ledger_projection: self.proposal_ledger_projection.clone(),
            context_manifest_projection: self.context_manifest_projection.clone(),
            privacy_inspector_projection: self.privacy_inspector_projection.clone(),
            permission_budget_projection: self.permission_budget_projection.clone(),
            approval_checklist_projection: self.approval_checklist_projection.clone(),
            checkpoint_rollback_projection: self.checkpoint_rollback_projection.clone(),
            assisted_ai_projection: self.assisted_ai_projection.clone(),
            delegated_task_projection: self.delegated_task_projection.clone(),
            plugin_contribution_projections: self.plugin_contribution_projections.clone(),
            collaboration_presence_projections: self.collaboration_presence_projections.clone(),
            daily_editing_projection: self.daily_editing_projection.clone(),
            search_projection: self.search_projection.clone(),
            language_tooling_projection: self.language_tooling_projection.clone(),
            terminal_panel_projection: self.terminal_panel_projection.clone(),
        }
    }

    /// Replace all render projections at once.
    pub fn replace_projection_snapshot(&mut self, snapshot: ShellProjectionSnapshot) {
        self.layout_projection = snapshot.layout_projection;
        self.explorer_projection = snapshot.explorer_projection;
        self.active_buffer_projection = snapshot.active_buffer_projection;
        self.status_messages = snapshot.status_messages;
        self.proposal_ledger_projection = snapshot.proposal_ledger_projection;
        self.context_manifest_projection = snapshot.context_manifest_projection;
        self.privacy_inspector_projection = snapshot.privacy_inspector_projection;
        self.permission_budget_projection = snapshot.permission_budget_projection;
        self.approval_checklist_projection = snapshot.approval_checklist_projection;
        self.checkpoint_rollback_projection = snapshot.checkpoint_rollback_projection;
        self.assisted_ai_projection = snapshot.assisted_ai_projection;
        self.delegated_task_projection = snapshot.delegated_task_projection;
        self.plugin_contribution_projections = snapshot.plugin_contribution_projections;
        self.collaboration_presence_projections = snapshot.collaboration_presence_projections;
        self.daily_editing_projection = snapshot.daily_editing_projection;
        self.search_projection = snapshot.search_projection;
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
        if !self.context_manifest_projection.manifest.items.is_empty() {
            let manifest = &self.context_manifest_projection.manifest;
            println!(
                "Context manifest {} | items={} omitted={} risk={:?} privacy={:?} egress={:?}",
                manifest.manifest_id,
                manifest.items.len(),
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
        println!(
            "Commands: :i text | :d start,end | :r start,end,text | :w | :wa | :tab id | :close id | :hover | :completion | :definition | :references | :outline | :format | :rename name | :code-action id | :term-launch label | :term-input text | :term-close | :plugin id command | :ai-start label | :ai-explain label | :ai-propose label | :u | :redo | :q"
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
        payload_kind: devil_protocol::ProposalPayloadKind::SaveFile,
        lifecycle_state: devil_protocol::ProposalLifecycleState::Created,
        correlation_id: devil_protocol::CorrelationId(0),
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
    let preconditions = devil_protocol::ContextManifestPreconditionSummary::from_preconditions(
        &devil_protocol::ProposalVersionPreconditions {
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
        payload_kind: devil_protocol::ProposalPayloadKind::SaveFile,
        lifecycle_state: devil_protocol::ProposalLifecycleState::Created,
        correlation_id: devil_protocol::CorrelationId(0),
        causality_id: None,
        checkpoint: devil_protocol::ProposalCheckpointProjection {
            checkpoint_id: "checkpoint:empty".to_string(),
            available: false,
            target_count: 0,
            expected_preconditions: preconditions,
            hashes: Vec::new(),
            audit_status: devil_protocol::CheckpointRollbackAuditStatus::NotRequired,
            labels: Vec::new(),
            limitations: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        rollback: devil_protocol::ProposalRollbackProjection {
            availability: devil_protocol::ProposalRollbackAvailability::NotRequired,
            rollback_step_count: 0,
            reversible_target_count: 0,
            irreversible_target_count: 0,
            audit_status: devil_protocol::CheckpointRollbackAuditStatus::NotRequired,
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
        provider_invocation: devil_protocol::AssistedAiProviderInvocationState::NotEncoded,
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
        generated_at: TimestampMillis(0),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn parse_proposal_id(payload: Option<&str>) -> Option<ProposalId> {
    payload
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(ProposalId)
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
    use devil_protocol::{
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

    fn test_coordinate(line: u32, character: u32) -> TextCoordinate {
        TextCoordinate {
            line,
            character,
            byte_offset: Some(character as u64),
            utf16_offset: None,
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
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
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
    fn shell_renders_proposal_ledger_from_static_snapshot() {
        let ledger = test_proposal_ledger_projection();
        let shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("t"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: ledger.clone(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
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
    fn shell_snapshot_large_file_projection_carries_only_viewport_slices() {
        let large_source_len = 6 * 1024 * 1024;
        let shell = Shell::new(ShellProjectionSnapshot {
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
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
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
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
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
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
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
        manifest
            .manifest
            .items
            .push(devil_protocol::ContextManifestItem {
                item_id: "semantic-job:0".to_string(),
                kind: devil_protocol::ContextManifestItemKind::SemanticFabricJob,
                inclusion: devil_protocol::ContextManifestInclusionState::Included,
                workspace_id: Some(WorkspaceId(1)),
                file_id: Some(FileId(9)),
                buffer_id: Some(BufferId(2)),
                proposal_id: Some(ProposalId(42)),
                target_id: Some("target-buffer-main".to_string()),
                path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
                ranges: vec![ByteRange::new(10, 20)],
                counts: vec![devil_protocol::ContextManifestItemCount {
                    label: "diagnostics".to_string(),
                    count: 2,
                }],
                hashes: vec![FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "content".to_string(),
                }],
                privacy_scope: Some(devil_protocol::SemanticPrivacyScope::Workspace),
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                risk_label: ProposalRiskLabel::Medium,
                egress: devil_protocol::ContextManifestEgressStatus::LocalOnly,
                freshness: None,
                preconditions: None,
                labels: vec!["semantic.fabric.metadata".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            });

        let shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: manifest.clone(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.context_manifest_projection, manifest);
        assert_eq!(snapshot.context_manifest_projection.manifest.items.len(), 1);
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn shell_renders_privacy_and_budget_summaries_from_static_snapshot_without_ownership() {
        let mut privacy = empty_privacy_inspector_projection();
        privacy.inspector_id = "privacy:trust".to_string();
        privacy.records = vec![devil_protocol::PrivacyInspectorExposureRecord {
            exposure_id: "exposure:semantic".to_string(),
            source_kind: devil_protocol::PrivacyInspectorSourceKind::SemanticMetadata,
            context_item_id: Some("semantic:0".to_string()),
            proposal_id: Some(ProposalId(42)),
            target_id: Some("target-0".to_string()),
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(9)),
            buffer_id: Some(BufferId(2)),
            privacy_scope: Some(devil_protocol::SemanticPrivacyScope::Workspace),
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_state: devil_protocol::PrivacyInspectorRedactionState::MetadataOnly,
            inclusion: devil_protocol::ContextManifestInclusionState::Included,
            egress: devil_protocol::ContextManifestEgressStatus::LocalOnly,
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
            privacy_scope: devil_protocol::SemanticPrivacyScope::MetadataOnly,
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
            layout_projection: ShellLayoutProjection::plain("trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: privacy.clone(),
            permission_budget_projection: budgets.clone(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
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
        checklist.gates = vec![devil_protocol::ApprovalChecklistGateSummary {
            gate: devil_protocol::ApprovalChecklistGateKind::AuditBeforeSuccess,
            status: devil_protocol::ApprovalChecklistGateStatus::Satisfied,
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
        rollback.rollback.availability = devil_protocol::ProposalRollbackAvailability::Available;
        rollback.targets = vec![devil_protocol::CheckpointRollbackTargetSummary {
            target_id: "target-buffer-main".to_string(),
            kind: devil_protocol::ProposalTargetKind::OpenBuffer,
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
            expected_file_content_version: Some(devil_protocol::FileContentVersion(44)),
            expected_buffer_version: Some(BufferVersion(55)),
            expected_snapshot_id: Some(SnapshotId(66)),
            expected_workspace_generation: Some(devil_protocol::WorkspaceGeneration(77)),
            labels: vec!["target.kind.OpenBuffer".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];

        let shell = Shell::new(ShellProjectionSnapshot {
            layout_projection: ShellLayoutProjection::plain("trust"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: checklist.clone(),
            checkpoint_rollback_projection: rollback.clone(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: empty_delegated_task_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
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
        assisted.providers = vec![devil_protocol::AssistedAiProviderCapabilitySummary {
            provider_id: "provider:local-redacted".to_string(),
            provider_label: "Local metadata provider".to_string(),
            provider_class: devil_protocol::AssistedAiProviderClass::Local,
            supported_operations: vec![devil_protocol::AssistedAiOperationClass::ProposeEdit],
            supported_operation_count: 1,
            model_capability_label_count: 1,
            tool_capability_label_count: 0,
            context_window_label: "bounded".to_string(),
            cost_budget_label: "capped".to_string(),
            risk_budget_label: "review-required".to_string(),
            privacy_retention_label: "metadata-only".to_string(),
            availability: devil_protocol::AssistedAiProviderAvailabilityState::Available,
            refusal: None,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];
        assisted.proposal_previews = vec![devil_protocol::AssistedAiProposalPreviewSummary {
            preview_id: "assist:preview:42".to_string(),
            output_id: "assist:output:42".to_string(),
            request_id: "assist:req:42".to_string(),
            provider_id: "provider:local-redacted".to_string(),
            proposal_id: ProposalId(42),
            payload_kind: ProposalPayloadKind::TextEdit,
            lifecycle_state: ProposalLifecycleState::Previewed,
            readiness: devil_protocol::AssistedAiProposalPreviewReadiness::PreviewReady,
            ready_for_preview: true,
            ready_for_approval: true,
            ready_for_apply: false,
            correlation_id: devil_protocol::CorrelationId(901),
            causality_id: devil_protocol::CausalityId(
                uuid::Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
            ),
            context_manifest: devil_protocol::AssistedAiTrustProjectionReference {
                reference_id: "manifest:p5:context".to_string(),
                kind: devil_protocol::AssistedAiTrustProjectionKind::ContextManifest,
                projection_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "manifest".to_string(),
                },
                schema_version: 1,
            },
            approval_checklist: devil_protocol::AssistedAiTrustProjectionReference {
                reference_id: "checklist:p5:approval".to_string(),
                kind: devil_protocol::AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
                projection_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "checklist".to_string(),
                },
                schema_version: 1,
            },
            checkpoint_rollback: None,
            preconditions: devil_protocol::ContextManifestPreconditionSummary::from_preconditions(
                &devil_protocol::ProposalVersionPreconditions {
                    file_version: Some(devil_protocol::FileContentVersion(44)),
                    buffer_version: Some(BufferVersion(55)),
                    snapshot_id: Some(SnapshotId(66)),
                    generation: Some(devil_protocol::WorkspaceGeneration(77)),
                    file_content_version: Some(devil_protocol::FileContentVersion(44)),
                    workspace_generation: Some(devil_protocol::WorkspaceGeneration(77)),
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
            layout_projection: ShellLayoutProjection::plain("assisted"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: assisted.clone(),
            delegated_task_projection: empty_delegated_task_projection(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.assisted_ai_projection, assisted);
        assert_eq!(
            snapshot.assisted_ai_projection.provider_invocation,
            devil_protocol::AssistedAiProviderInvocationState::NotEncoded
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
        delegated.plan_rows = vec![devil_protocol::DelegatedTaskPlanRow {
            plan_id: devil_protocol::DelegatedTaskPlanId("plan:p7-1".to_string()),
            workspace_id: Some(WorkspaceId(1)),
            objective_summary_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "objective".to_string(),
            },
            plan_state: devil_protocol::DelegatedTaskPlanState::AwaitingApproval,
            readiness: devil_protocol::DelegatedTaskPlanReadinessStatus::PlanReady,
            step_count: 1,
            affected_target_count: 1,
            blocker_count: 0,
            refusal_count: 0,
            proposal_preview_link_count: 1,
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            correlation_id: devil_protocol::CorrelationId(901),
            causality_id: devil_protocol::CausalityId(
                uuid::Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
            ),
            runtime_activation: devil_protocol::DelegatedTaskRuntimeActivationState::NotEncoded,
            labels: vec!["delegated_task.plan_row.metadata_only".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }];
        delegated.step_summaries = vec![devil_protocol::DelegatedTaskStepSummary {
            step_id: devil_protocol::DelegatedTaskStepId("step:preview".to_string()),
            plan_id: devil_protocol::DelegatedTaskPlanId("plan:p7-1".to_string()),
            order: 1,
            objective_summary_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "step".to_string(),
            },
            operation_class: devil_protocol::DelegatedTaskOperationClass::LinkProposalPreview,
            state: devil_protocol::DelegatedTaskStepState::ProposalPreviewLinked,
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
            layout_projection: ShellLayoutProjection::plain("delegated"),
            explorer_projection: ExplorerProjection {
                nodes: Vec::new(),
                selection: None,
            },
            active_buffer_projection: ActiveBufferProjection::empty(),
            status_messages: Vec::new(),
            proposal_ledger_projection: test_proposal_ledger_projection(),
            context_manifest_projection: empty_context_manifest_projection(),
            privacy_inspector_projection: empty_privacy_inspector_projection(),
            permission_budget_projection: empty_permission_budget_projection(),
            approval_checklist_projection: empty_approval_checklist_projection(),
            checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
            assisted_ai_projection: empty_assisted_ai_projection(),
            delegated_task_projection: delegated.clone(),
            plugin_contribution_projections: Vec::new(),
            collaboration_presence_projections: Vec::new(),
            daily_editing_projection: DailyEditingProjection::empty(),
            search_projection: SearchProjection::idle(),
            language_tooling_projection: LanguageToolingProjection::empty(),
            terminal_panel_projection: TerminalPanelProjection::empty(),
        });

        let snapshot = shell.projection_snapshot();
        assert_eq!(snapshot.delegated_task_projection, delegated);
        assert_eq!(
            snapshot.delegated_task_projection.runtime_activation,
            devil_protocol::DelegatedTaskRuntimeActivationState::NotEncoded
        );
        assert_eq!(
            snapshot.delegated_task_projection.step_summaries[0].proposal_id,
            Some(ProposalId(42))
        );
        assert!(shell.command_dispatch_intents.is_empty());
    }

    #[test]
    fn ui_plugin_contributions_are_projection_only_command_intents() {
        let mut shell = Shell::empty("plugins");
        shell.plugin_contribution_projections = vec![PluginContributionProjection {
            plugin_id: PluginId(7),
            contributions: vec![devil_protocol::PluginContribution::Command(
                devil_protocol::PluginCommandDescriptor {
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
