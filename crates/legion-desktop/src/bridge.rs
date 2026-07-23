//! Desktop event to app-command bridge.

use std::fmt;
use std::path::PathBuf;

use legion_project::git_pull_request_url;
use legion_protocol::{
    AgentRunId, AssistantRailCommand, BufferId, CollaborationParticipantId, CollaborationSessionId,
    DebugConfigurationId, DebugSessionId, DelegatedTaskPlanId,
    DelegatedTaskProposalHunkDisposition, DelegatedTaskToolPermissionDecision, EditablePlanSection,
    FileId, InlinePredictionRequestId, LegionWorkflowConflictId, LegionWorkflowSessionId,
    LegionWorkflowSignOffId, LegionWorkflowVerificationGateId, ProposalCancellationReason,
    ProposalId, ProposalRejectionReason, ProposalRollbackReason, ProtocolTextRange,
    RemoteWorkspaceSessionId, TerminalSessionId, TextCoordinate, ViewportScroll,
};
use legion_protocol::{PluginContribution, PluginId};
use legion_ui::{
    CommandDispatchIntent, DebugStepKindProjection, DockMode, GitConflictChoiceProjection,
    PaletteMode, SearchScopeProjection, ShellProjectionSnapshot, ThemePreferenceProjection,
    ToastVerbosityProjection,
};
use thiserror::Error;

/// A string wrapper that redacts its value in `Debug` output and zeroizes on drop.
///
/// Use this type for fields that carry secrets (API keys, tokens) so they are
/// never accidentally printed to logs.
#[derive(Clone, PartialEq, Eq)]
pub struct SensitiveString(pub String);

impl fmt::Debug for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SensitiveString(\"<redacted>\")")
    }
}

impl From<String> for SensitiveString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::ops::Deref for SensitiveString {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl Drop for SensitiveString {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.0.zeroize();
    }
}

/// Adapter-local renderer action before app routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopAction {
    /// Quit the desktop shell.
    Quit,
    /// Save the active buffer through app authority.
    SaveActive,
    /// Save every open tab through app authority.
    SaveAll,
    /// Switch the app-owned product mode through app authority.
    SetProductMode {
        /// Target product mode.
        mode: DockMode,
    },
    /// Switch to a projected tab.
    SwitchTab {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Request close for a projected tab.
    CloseTab {
        /// Target buffer identifier.
        buffer_id: BufferId,
    },
    /// Save the buffer currently represented by a dirty-close prompt.
    SaveDirtyClose {
        /// Prompt buffer identifier.
        buffer_id: BufferId,
    },
    /// Cancel the active dirty-close prompt without closing or discarding text.
    CancelDirtyClose {
        /// Prompt buffer identifier.
        buffer_id: BufferId,
    },
    /// Open a user-entered path through workspace authority.
    OpenPathText(String),
    /// Open a path selected by a native file dialog.
    OpenPathDialogSelected(String),
    /// Native file dialog was cancelled.
    OpenPathDialogCancelled,
    /// Open the app-owned command palette.
    OpenPalette {
        /// Requested palette mode.
        mode: PaletteMode,
        /// Initial query text.
        query: String,
        /// Search scope for search-oriented palette modes.
        scope: SearchScopeProjection,
    },
    /// Close the app-owned command palette.
    ClosePalette,
    /// Update command palette query text.
    UpdatePaletteQuery {
        /// Updated query text.
        query: String,
    },
    /// Move command palette selection by a signed delta.
    MovePaletteSelection {
        /// Signed selection delta.
        delta: i32,
    },
    /// Complete command palette selection.
    CompletePaletteSelection,
    /// Dispatch command palette selection.
    DispatchPaletteSelection,
    /// Dismiss a foreground toast in adapter-local view state.
    DismissToast {
        /// Projected toast identifier.
        toast_id: u64,
    },
    /// Dismiss the first-run onboarding card in adapter-local view state.
    DismissOnboarding,
    /// Invoke a foreground toast action through existing command authority.
    InvokeToastAction {
        /// Intent attached to the projected toast action.
        intent: CommandDispatchIntent,
    },
    /// Open the projected Settings surface.
    OpenSettings,
    /// Update theme preference through app authority.
    SetThemePreference {
        /// Requested theme preference.
        preference: ThemePreferenceProjection,
    },
    /// Update UI zoom through app authority.
    SetZoomPercent {
        /// Requested zoom percentage.
        zoom_percent: u16,
    },
    /// Update editor font size through app authority.
    SetEditorFontSize {
        /// Requested editor font size in points.
        font_size_pt: u16,
    },
    /// Update toast verbosity through app authority.
    SetToastVerbosity {
        /// Requested toast verbosity.
        verbosity: ToastVerbosityProjection,
    },
    /// Toggle line numbers through app authority.
    SetLineNumbersVisible {
        /// Whether line numbers should be visible.
        visible: bool,
    },
    /// Toggle current-line highlight through app authority.
    SetCurrentLineHighlight {
        /// Whether current-line highlighting is enabled.
        enabled: bool,
    },
    /// Toggle sticky headers through app authority.
    SetStickyHeadersVisible {
        /// Whether sticky headers should be visible.
        visible: bool,
    },
    /// Toggle code folding indicators through app authority.
    SetCodeFoldingVisible {
        /// Whether code folding indicators should be visible.
        visible: bool,
    },
    /// Toggle the minimap through app authority.
    SetMinimapVisible {
        /// Whether the minimap should be visible.
        visible: bool,
    },
    /// Toggle whitespace guides through app authority.
    SetWhitespaceGuidesVisible {
        /// Whether whitespace guides should be visible.
        visible: bool,
    },
    /// Toggle indent guides through app authority.
    SetIndentGuidesVisible {
        /// Whether indent guides should be visible.
        visible: bool,
    },
    /// Toggle smooth scrolling through app authority.
    SetSmoothScrollingEnabled {
        /// Whether smooth scrolling should be enabled.
        enabled: bool,
    },
    /// Toggle indexed workspace search through app authority.
    SetIndexedWorkspaceSearchEnabled {
        /// Whether workspace search should use the optional indexed backend.
        enabled: bool,
    },
    /// Toggle next-edit prediction through app authority.
    SetNextEditPredictionEnabled {
        /// Whether next-edit prediction should auto-trigger after edits.
        enabled: bool,
    },
    /// Toggle crash reports through app authority.
    SetCrashReportsEnabled {
        /// Whether crash reports should be enabled.
        enabled: bool,
    },
    /// Reset workbench settings through app authority.
    ResetSettings,
    /// Ask the workflow layer to open a workspace root.
    OpenWorkspace {
        /// Workspace root selected by the adapter.
        root: PathBuf,
    },
    /// Refresh the explorer projection through app authority.
    RefreshExplorer,
    /// Refresh git status, syntactic diff, blame, graph, and conflict projections.
    RefreshGit,
    /// Refresh cargo test discovery for the test explorer panel.
    RefreshTestExplorer,
    /// Switch to an existing git branch.
    SwitchGitBranch {
        /// Branch label.
        branch: String,
    },
    /// Create and switch to a new git branch.
    CreateGitBranch {
        /// Branch label.
        branch: String,
    },
    /// Delete a git branch.
    DeleteGitBranch {
        /// Branch label.
        branch: String,
    },
    /// Stash local git changes.
    StashGitChanges {
        /// Optional stash message.
        message: Option<String>,
    },
    /// Push the current branch to the default remote.
    PushGitRemote,
    /// Open the branch's forge pull-request URL.
    OpenGitPullRequestUrl,
    /// Prune orphaned worktree metadata.
    PruneGitWorktrees,
    /// Remove a worktree by path.
    RemoveGitWorktree {
        /// Worktree path.
        path: String,
    },
    /// Refresh debug launch configurations and persisted breakpoints.
    RefreshDebugConfigurations,
    /// Toggle a breakpoint for the active projected buffer.
    ToggleDebugBreakpoint {
        /// Zero-based line.
        line: u32,
        /// Conditional expression label.
        condition: Option<String>,
        /// Hit condition label.
        hit_condition: Option<String>,
        /// Logpoint message label.
        log_message: Option<String>,
    },
    /// Launch a debug session from a projected configuration.
    LaunchDebugSession {
        /// Projected debug configuration identifier.
        configuration_id: DebugConfigurationId,
    },
    /// Step or continue a projected debug session.
    DebugStep {
        /// Projected debug session identifier.
        session_id: DebugSessionId,
        /// Step kind.
        kind: DebugStepKindProjection,
    },
    /// Run the active debug session to a cursor position in the active buffer.
    DebugRunToCursor {
        /// Projected debug session identifier.
        session_id: DebugSessionId,
        /// Cursor position.
        position: TextCoordinate,
    },
    /// Evaluate a display-safe expression label in a projected debug session.
    DebugEvaluateSelection {
        /// Projected debug session identifier.
        session_id: DebugSessionId,
        /// Display-safe expression label.
        expression_label: String,
    },
    /// Add a display-safe watch expression to a projected debug session.
    DebugAddWatch {
        /// Projected debug session identifier.
        session_id: DebugSessionId,
        /// Display-safe expression label.
        expression_label: String,
    },
    /// Poll a live debug session after non-blocking continue (B7/B8).
    PollDebugSession,
    /// Stop / disconnect the active debug session.
    StopDebugSession,
    /// Stage one projected git hunk.
    StageGitHunk {
        /// Projected hunk identifier.
        hunk_id: String,
    },
    /// Unstage one projected git hunk.
    UnstageGitHunk {
        /// Projected hunk identifier.
        hunk_id: String,
    },
    /// Accept the current (ours) side of a conflicted file.
    AcceptGitConflictCurrent {
        /// Repository-relative path of the conflicted file.
        path: String,
    },
    /// Accept the incoming (theirs) side of a conflicted file.
    AcceptGitConflictIncoming {
        /// Repository-relative path of the conflicted file.
        path: String,
    },
    /// Toggle adapter-local explorer expansion for a canonical path.
    ToggleExplorerPath {
        /// Canonical path represented by the explorer row.
        path: String,
    },
    /// Select/reveal an explorer file through app authority.
    SelectExplorerFile {
        /// Projected workspace file identifier.
        file_id: FileId,
    },
    /// Request a proposal preview through app authority.
    PreviewProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
    },
    /// Approve a projected proposal through app authority.
    ApproveProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
    },
    /// Reject a projected proposal through app authority.
    RejectProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
        /// Display-safe rejection reason.
        reason: ProposalRejectionReason,
    },
    /// Apply a projected proposal through app authority.
    ApplyProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
    },
    /// Roll back a projected proposal through app authority.
    RollbackProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
        /// Display-safe rollback reason.
        reason: ProposalRollbackReason,
    },
    /// Cancel a projected proposal through app authority.
    CancelProposal {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
        /// Display-safe cancellation reason.
        reason: ProposalCancellationReason,
    },
    /// Open projected proposal details without taking proposal ownership.
    OpenProposalDetails {
        /// Projected proposal identifier.
        proposal_id: ProposalId,
    },
    /// Start a metadata-only assisted-AI explain run through app authority.
    StartAiExplain {
        /// Display-safe instruction label.
        instruction_label: String,
    },
    /// Start a proposal-only assisted-AI edit run through app authority.
    StartAiProposal {
        /// Display-safe instruction label.
        instruction_label: String,
    },
    /// Cancel a projected assisted-AI run through app authority.
    CancelAiRun {
        /// Projected or user-visible run identifier.
        run_id: AgentRunId,
    },
    /// Replay a projected assisted-AI run from app-owned metadata.
    ReplayAiRun {
        /// Projected or user-visible run identifier.
        run_id: AgentRunId,
    },
    /// Inspect projected assisted-AI run metadata.
    InspectAiRun {
        /// Projected or user-visible run identifier.
        run_id: AgentRunId,
    },
    /// Invoke a projected plugin command through app-owned plugin authority.
    InvokePluginCommand {
        /// Plugin identifier selected from projection data.
        plugin_id: PluginId,
        /// Command identifier selected from projection data.
        command_id: String,
    },
    /// Join a collaboration session through app-owned collaboration authority.
    JoinCollaborationSession {
        /// Session identifier selected from projected collaboration GUI data.
        session_id: CollaborationSessionId,
    },
    /// Leave a projected collaboration session.
    LeaveCollaborationSession {
        /// Session identifier selected from projected collaboration GUI data.
        session_id: CollaborationSessionId,
    },
    /// Publish metadata-only presence for a projected collaboration session.
    PublishCollaborationPresence {
        /// Session identifier selected from projected collaboration GUI data.
        session_id: CollaborationSessionId,
        /// Participant identifier selected from projected collaboration GUI data.
        participant_id: CollaborationParticipantId,
    },
    /// Open shared collaboration proposal review through proposal details.
    OpenSharedProposalReview {
        /// Session identifier selected from projected collaboration GUI data.
        session_id: CollaborationSessionId,
        /// Shared proposal identifier selected from projected collaboration GUI data.
        proposal_id: ProposalId,
    },
    /// Connect or reconnect a remote workspace through app-owned remote authority.
    ConnectRemoteWorkspace {
        /// Session identifier selected from user input or projected remote GUI data.
        session_id: RemoteWorkspaceSessionId,
        /// Display-safe remote authority label or stable hash.
        authority_label: String,
    },
    /// Open remote proposal review through proposal details.
    OpenRemoteProposalReview {
        /// Session identifier selected from projected remote GUI data.
        session_id: RemoteWorkspaceSessionId,
        /// Remote proposal identifier selected from projected remote GUI data.
        proposal_id: ProposalId,
    },
    /// Inspect a delegated task plan without activating an agent runtime.
    InspectDelegatedTaskPlan {
        /// Delegated task plan identifier selected from projection data.
        plan_id: DelegatedTaskPlanId,
    },
    /// Open a proposal preview linked from delegated task metadata.
    OpenDelegatedProposalPreview {
        /// Proposal identifier linked from projected delegated task data.
        proposal_id: ProposalId,
    },
    /// Open proposal details linked from delegated task metadata.
    OpenDelegatedProposalDetails {
        /// Proposal identifier linked from projected delegated task data.
        proposal_id: ProposalId,
    },
    /// Send a Delegate chat turn through app-owned context retrieval.
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
    /// Record a human review decision for a projected Delegate proposal hunk.
    ReviewDelegateProposalHunk {
        /// Proposal containing the hunk.
        proposal_id: ProposalId,
        /// Projected hunk identifier.
        hunk_id: String,
        /// Human disposition.
        disposition: DelegatedTaskProposalHunkDisposition,
    },
    /// Record a human decision for a projected Delegate tool permission request.
    RecordDelegateToolPermission {
        /// Projected permission request identifier.
        request_id: String,
        /// Human decision.
        decision: DelegatedTaskToolPermissionDecision,
    },
    /// Submit edited Legion workflow plan sections for app-owned revision.
    SubmitLegionWorkflowPlanRevision {
        /// Plan artifact identifier.
        plan_id: String,
        /// Edited plan sections.
        edited_sections: Vec<EditablePlanSection>,
    },
    /// Approve a reviewed Legion workflow plan.
    ApproveLegionWorkflowPlan {
        /// Plan artifact identifier.
        plan_id: String,
    },
    /// Reject a Legion workflow plan and keep review required.
    RejectLegionWorkflowPlan {
        /// Plan artifact identifier.
        plan_id: String,
    },
    /// Inspect a projected Legion workflow session.
    InspectLegionWorkflowSession {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
    },
    /// Open a proposal preview linked from Legion workflow metadata.
    OpenLegionWorkflowProposalPreview {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Proposal identifier linked from projected Legion workflow data.
        proposal_id: ProposalId,
    },
    /// Open proposal details linked from Legion workflow metadata.
    OpenLegionWorkflowProposalDetails {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Proposal identifier linked from projected Legion workflow data.
        proposal_id: ProposalId,
    },
    /// Request app-owned verification metadata for a Legion workflow.
    RequestLegionWorkflowVerification {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Verification gate identifier selected from projection data.
        gate_id: LegionWorkflowVerificationGateId,
    },
    /// Request app-owned sign-off metadata for a Legion workflow.
    RequestLegionWorkflowSignOff {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Sign-off identifier selected from projection data.
        sign_off_id: LegionWorkflowSignOffId,
    },
    /// Request app-owned conflict-resolution metadata for a Legion workflow.
    ResolveLegionWorkflowConflict {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Conflict identifier selected from projection data.
        conflict_id: LegionWorkflowConflictId,
    },
    /// Request app-owned merge-readiness evaluation for a Legion workflow.
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
    /// Trigger the hard Automate kill switch for a workflow.
    TriggerLegionWorkflowKillSwitch {
        /// Workflow session identifier selected from projection data.
        session_id: LegionWorkflowSessionId,
        /// Display-safe reason label.
        reason_label: String,
    },
    /// Insert text at a projected coordinate.
    InsertText {
        /// Text to insert.
        text: String,
        /// Projected insertion coordinate.
        at: TextCoordinate,
    },
    /// Replace a projected range.
    ReplaceRange {
        /// Projected range to replace.
        range: ProtocolTextRange,
        /// Replacement text.
        replacement: String,
    },
    /// Delete a projected range.
    DeleteRange {
        /// Projected range to delete.
        range: ProtocolTextRange,
    },
    /// Paste clipboard text at a projected coordinate.
    ClipboardPaste {
        /// Clipboard text to insert.
        text: String,
        /// Projected insertion coordinate.
        at: TextCoordinate,
    },
    /// Copy the active editor selection without exposing clipboard text in outcomes.
    ClipboardCopy,
    /// Cut the active editor selection without exposing clipboard text in outcomes.
    ClipboardCut,
    /// Commit IME text at a projected coordinate.
    ImeCommit {
        /// IME text to insert.
        text: String,
        /// Projected insertion coordinate.
        at: TextCoordinate,
    },
    /// Undo the active buffer.
    Undo,
    /// Redo the active buffer.
    Redo,
    /// Set the primary cursor for a buffer or the active buffer.
    SetCursor {
        /// Optional target buffer; falls back to the active tab.
        buffer_id: Option<BufferId>,
        /// Cursor coordinate in projection space.
        cursor: TextCoordinate,
    },
    /// Set the primary selection for a buffer or the active buffer.
    SetSelection {
        /// Optional target buffer; falls back to the active tab.
        buffer_id: Option<BufferId>,
        /// Selection range in projection space.
        range: ProtocolTextRange,
    },
    /// Select the entire target buffer or active buffer.
    SelectAll {
        /// Optional target buffer; falls back to the active tab.
        buffer_id: Option<BufferId>,
    },
    /// Set viewport scroll for a buffer or the active buffer.
    SetViewportScroll {
        /// Optional target buffer; falls back to the active tab.
        buffer_id: Option<BufferId>,
        /// Projected viewport scroll state.
        scroll: ViewportScroll,
    },
    /// Run bounded lexical search through app authority.
    RunSearch {
        /// Search scope.
        scope: SearchScopeProjection,
        /// User-provided query text.
        query: String,
        /// Requested result limit; zero means app default.
        limit: usize,
        /// Explicit case-sensitive toggle; `None` defers to text-prefix parsing.
        case_sensitive: Option<bool>,
        /// Explicit whole-word toggle; `None` defers to text-prefix parsing.
        whole_word: Option<bool>,
        /// Explicit regex mode toggle; `None` defers to text-prefix parsing.
        use_regex: Option<bool>,
    },
    /// Run bounded structural search and rewrite preview through app authority.
    RunStructuralSearch {
        /// Search scope.
        scope: SearchScopeProjection,
        /// User-provided structural pattern.
        pattern: String,
        /// Optional rewrite template; empty prompt input is represented as None.
        rewrite: Option<String>,
        /// Requested result limit; zero means app default.
        limit: usize,
    },
    /// Cancel projected search by query id.
    CancelSearch {
        /// Query id to cancel.
        query_id: String,
    },
    /// Request language hover for the active buffer.
    RequestHover {
        /// Projected cursor position.
        position: TextCoordinate,
    },
    /// Request language completions for the active buffer.
    RequestCompletion {
        /// Projected cursor position.
        position: TextCoordinate,
    },
    /// Request an Assist inline ghost prediction for the active buffer.
    RequestAssistInlinePrediction {
        /// Projected cursor position.
        position: TextCoordinate,
    },
    /// Accept the current Assist ghost prediction.
    AcceptCurrentAssistInlinePrediction,
    /// Dismiss the current Assist ghost prediction.
    DismissCurrentAssistInlinePrediction,
    /// Cancel the current in-flight Assist ghost prediction.
    CancelAssistInlinePrediction,
    /// Request definition locations for the active buffer.
    GoToDefinition {
        /// Projected cursor position.
        position: TextCoordinate,
    },
    /// Request reference locations for the active buffer.
    FindReferences {
        /// Projected cursor position.
        position: TextCoordinate,
    },
    /// Refresh the active buffer outline.
    RefreshOutline,
    /// Request a formatting proposal preview.
    RequestFormattingProposal,
    /// Request a rename proposal preview.
    RequestRenameProposal {
        /// Projected cursor position.
        position: TextCoordinate,
        /// New symbol name.
        new_name: String,
    },
    /// Request an organize-imports proposal preview.
    RequestOrganizeImportsProposal,
    /// Request a code-action proposal preview.
    RequestCodeActionProposal {
        /// Code-action identifier.
        action_id: String,
    },
    /// Cancel a language operation.
    CancelLanguageOperation {
        /// Operation identifier.
        operation_id: String,
    },
    /// Navigate to a diagnostic problem location (D3).
    ///
    /// Opens the file at `path` and positions the cursor at (line, 0). Emits
    /// `CommandDispatchIntent::OpenPathAtPosition` through app authority.
    /// `path` is the raw canonical path from the problem projection;
    /// `line` is the zero-based LSP start line from the problem range.
    NavigateToProblem {
        /// Canonical path of the file containing the problem.
        path: String,
        /// Zero-based line number from the problem range start.
        line: u32,
    },
    /// Move keyboard focus to the next problem in the Problems panel (T4).
    ProblemNext,
    /// Move keyboard focus to the previous problem in the Problems panel (T4).
    ProblemPrev,
    /// Activate (navigate to) the currently focused problem in the Problems panel (T4).
    ProblemActivate,
    /// Move keyboard focus to the next hunk in the proposal review surface (PKT-DIFF).
    ReviewHunkNext,
    /// Move keyboard focus to the previous hunk in the proposal review surface (PKT-DIFF).
    ReviewHunkPrev,
    /// Accept the currently focused hunk in the proposal review surface (PKT-DIFF).
    ReviewHunkAccept,
    /// Reject the currently focused hunk in the proposal review surface (PKT-DIFF).
    ReviewHunkReject,
    /// Accept all hunks in the proposal review surface (PKT-DIFF).
    ReviewAcceptAll,
    /// Reject all hunks in the proposal review surface (PKT-DIFF).
    ReviewRejectAll,
    /// Apply the filtered proposal built from accepted-hunk dispositions (PKT-DIFF).
    ///
    /// Binds to Alt+Enter; intercepted in DesktopRuntime::handle_action.
    ReviewApply,
    /// Dismiss the proposal review surface, clearing all dispositions (PKT-DIFF).
    ///
    /// Binds to Alt+Escape; intercepted in DesktopRuntime::handle_action.
    ReviewDismiss,
    /// Move selection down in the LSP completion popup (T6).
    CompletionNext,
    /// Move selection up in the LSP completion popup (T6).
    CompletionPrev,
    /// Accept the currently selected item in the LSP completion popup (T6).
    ///
    /// Inserts the selected completion label into the active buffer through
    /// the existing editor insert path (editor authority).
    CompletionAccept,
    /// Dismiss the LSP completion popup without accepting any item (T6).
    CompletionDismiss,
    /// Dismiss the LSP hover tooltip (T7).
    HoverDismiss,
    /// Navigate to a specific definition location by index (T7).
    ///
    /// `index` is a zero-based position into `language_tooling_projection.definitions`.
    NavigateToDefinition {
        /// Zero-based index into the projected definitions list.
        index: usize,
    },
    /// Launch a terminal session through app authority.
    TerminalLaunch {
        /// Command label.
        command_label: String,
    },
    /// Send input to the active terminal session.
    TerminalInput {
        /// Input payload.
        payload: String,
    },
    /// Resize the active terminal session.
    TerminalResize {
        /// Column count.
        cols: u16,
        /// Row count.
        rows: u16,
    },
    /// Kill the active terminal session.
    TerminalKill,
    /// Close the active terminal session.
    TerminalClose,
    /// Poll output for the active terminal session.
    TerminalOutputPoll,
    /// Search terminal output.
    TerminalSearch {
        /// Query label.
        query: String,
    },
    /// Restore a durable checkpoint through app authority (PKT-CKPT).
    ///
    /// Binds to Alt+Z; intercepted in DesktopWorkflowRuntime::handle_action.
    RestoreCheckpoint {
        /// Stable checkpoint identifier selected from projected checkpoint timeline data.
        checkpoint_id: String,
    },
    /// Store a BYOK API key in the OS keyring for a provider (PKT-PROV).
    ///
    /// The key value is stored immediately to the OS keyring and never persisted
    /// to any config file. The key is consumed (zeroized) after the store call.
    SetProviderApiKey {
        /// Provider identifier (e.g. "anthropic", "openai").
        provider_id: String,
        /// The API key value; consumed and zeroized immediately after storage.
        /// Wrapped in `SensitiveString` so the value is never printed in debug output.
        api_key: SensitiveString,
    },
    /// Delete a stored BYOK API key from the OS keyring (PKT-PROV).
    DeleteProviderApiKey {
        /// Provider identifier whose key should be removed.
        provider_id: String,
    },
    /// Select the product AI route preference for Assist / Delegate composition.
    ///
    /// Labels: `auto` (local-first), `ollama`, `anthropic`, `deterministic`.
    SetPreferredAiProvider {
        /// Preference label (case-insensitive).
        provider_id: String,
    },
    /// Accept a ghost text prediction through the proposal pipeline (PKT-RAIL).
    ///
    /// Acceptance is proposal-mediated — this action never causes a direct buffer mutation.
    AcceptGhostText {
        /// Request identifier of the prediction to accept.
        request_id: InlinePredictionRequestId,
    },
    /// Dismiss a ghost text prediction overlay (PKT-RAIL).
    DismissGhostText {
        /// Request identifier of the prediction to dismiss.
        request_id: InlinePredictionRequestId,
    },
    /// Cancel an in-flight ghost text prediction (PKT-RAIL).
    CancelGhostText {
        /// Request identifier of the in-flight prediction to cancel.
        request_id: InlinePredictionRequestId,
    },
    /// Execute an assistant rail command through the proposal pipeline (PKT-RAIL).
    ///
    /// Commands dispatch a proposal-only AI run; no direct buffer mutation occurs.
    ExecuteRailCommand {
        /// The rail command to execute.
        command: AssistantRailCommand,
        /// Optional text selection; `None` uses cursor context.
        selection: Option<ProtocolTextRange>,
    },
    /// Accept a single hunk in the inline edit overlay (PKT-INLINE).
    ///
    /// Intercepted in DesktopRuntime before reaching the bridge; this arm
    /// satisfies exhaustiveness.
    AcceptInlineEditHunk {
        /// Instruction identifier of the inline edit session.
        instruction_id: String,
        /// Hunk identifier to accept.
        hunk_id: String,
    },
    /// Reject a single hunk in the inline edit overlay (PKT-INLINE).
    ///
    /// Intercepted in DesktopRuntime before reaching the bridge; this arm
    /// satisfies exhaustiveness.
    RejectInlineEditHunk {
        /// Instruction identifier of the inline edit session.
        instruction_id: String,
        /// Hunk identifier to reject.
        hunk_id: String,
    },
    /// Apply all accepted hunks in the inline edit overlay through the proposal
    /// pipeline (PKT-INLINE).
    ///
    /// Intercepted in DesktopRuntime before reaching the bridge; this arm
    /// satisfies exhaustiveness.
    ApplyInlineEdit {
        /// Instruction identifier of the inline edit session to apply.
        instruction_id: String,
    },
    /// Dismiss the entire inline edit overlay without applying any hunks
    /// (PKT-INLINE).
    ///
    /// Intercepted in DesktopRuntime before reaching the bridge; this arm
    /// satisfies exhaustiveness.
    DismissInlineEdit {
        /// Instruction identifier of the inline edit session to dismiss.
        instruction_id: String,
    },
}

/// App-owned request that is not a direct UI command intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopAppRequest {
    /// Open a workspace root through the app composition layer.
    OpenWorkspace {
        /// Workspace root path.
        root: PathBuf,
    },
    /// Toggle adapter-local explorer expansion.
    ToggleExplorerPath {
        /// Canonical path represented by the explorer row.
        path: String,
    },
    /// Open an external URL in the system browser.
    OpenExternalUrl {
        /// URL to open.
        url: String,
    },
    /// Cancel an app-owned dirty-close prompt.
    CancelDirtyClose {
        /// Prompt buffer identifier.
        buffer_id: BufferId,
    },
    /// Connect or reconnect a remote workspace through app composition.
    ConnectRemoteWorkspace {
        /// Remote workspace session id.
        session_id: RemoteWorkspaceSessionId,
        /// Display-safe remote authority label or stable hash.
        authority_label: String,
    },
    /// Inspect delegated task plan metadata without runtime activation.
    InspectDelegatedTaskPlan {
        /// Delegated task plan identifier.
        plan_id: DelegatedTaskPlanId,
    },
    /// Open proposal preview linked from delegated task metadata.
    OpenDelegatedProposalPreview {
        /// Proposal identifier.
        proposal_id: ProposalId,
    },
    /// Open proposal details linked from delegated task metadata.
    OpenDelegatedProposalDetails {
        /// Proposal identifier.
        proposal_id: ProposalId,
    },
    /// Submit edited Legion workflow plan sections for app-owned revision.
    SubmitLegionWorkflowPlanRevision {
        /// Plan artifact identifier.
        plan_id: String,
        /// Edited plan sections.
        edited_sections: Vec<EditablePlanSection>,
    },
    /// Approve a reviewed Legion workflow plan.
    ApproveLegionWorkflowPlan {
        /// Plan artifact identifier.
        plan_id: String,
    },
    /// Reject a Legion workflow plan and keep review required.
    RejectLegionWorkflowPlan {
        /// Plan artifact identifier.
        plan_id: String,
    },
    /// Inspect Legion workflow session metadata.
    InspectLegionWorkflowSession {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
    },
    /// Open proposal preview linked from Legion workflow metadata.
    OpenLegionWorkflowProposalPreview {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// Proposal identifier.
        proposal_id: ProposalId,
    },
    /// Open proposal details linked from Legion workflow metadata.
    OpenLegionWorkflowProposalDetails {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// Proposal identifier.
        proposal_id: ProposalId,
    },
    /// Request app-owned Legion workflow verification metadata.
    RequestLegionWorkflowVerification {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// Verification gate identifier.
        gate_id: LegionWorkflowVerificationGateId,
    },
    /// Request app-owned Legion workflow sign-off metadata.
    RequestLegionWorkflowSignOff {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// Sign-off identifier.
        sign_off_id: LegionWorkflowSignOffId,
    },
    /// Request app-owned Legion workflow conflict resolution metadata.
    ResolveLegionWorkflowConflict {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// Conflict identifier.
        conflict_id: LegionWorkflowConflictId,
    },
    /// Request app-owned Legion workflow merge-readiness metadata.
    RequestLegionWorkflowMergeReadiness {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
    },
    /// Record app-owned Automate MCP tool permission metadata.
    RecordLegionWorkflowToolPermission {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// MCP server identifier.
        server_id: legion_protocol::McpServerId,
        /// MCP tool name.
        tool_name: legion_protocol::McpToolName,
        /// Human decision.
        decision: DelegatedTaskToolPermissionDecision,
    },
    /// Trigger app-owned Automate kill-switch metadata.
    TriggerLegionWorkflowKillSwitch {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// Display-safe reason label.
        reason_label: String,
    },
}

/// Result of translating a desktop action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopBridgeOutput {
    /// Dispatch this intent to app authority.
    Intent(CommandDispatchIntent),
    /// Handle this request in the desktop workflow/app layer.
    AppRequest(DesktopAppRequest),
    /// No application work should happen.
    Noop,
    /// The action cannot be represented safely.
    Error(DesktopBridgeError),
}

/// Bridge mapping errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DesktopBridgeError {
    /// The current projection has no active buffer id.
    #[error("active buffer is required for this desktop action")]
    MissingActiveBuffer,
    /// Git pull-request flow requires a remote URL projection.
    #[error("git remote URL is unavailable in the current projection")]
    MissingGitRemoteUrl,
    /// Git pull-request flow requires a projected branch label.
    #[error("git branch label is unavailable in the current projection")]
    MissingGitBranchLabel,
    /// Git pull-request flow requires a projected remote default branch.
    #[error("git remote default branch is unavailable in the current projection")]
    MissingRemoteDefaultBranch,
    /// Git pull-request flow does not support the projected remote URL.
    #[error("unsupported git forge remote: {remote_url}")]
    UnsupportedGitForgeRemote {
        /// Remote URL that could not be mapped.
        remote_url: String,
    },
    /// Target buffer was not present in the projected tab list.
    #[error("unknown tab buffer: {buffer_id:?}")]
    UnknownTab {
        /// Unknown tab buffer.
        buffer_id: BufferId,
    },
    /// Target file was not present in the projected explorer tree.
    #[error("unknown explorer file: {file_id:?}")]
    UnknownExplorerFile {
        /// Unknown explorer file.
        file_id: FileId,
    },
    /// Target proposal was not present in the proposal ledger projection.
    #[error("unknown proposal: {proposal_id:?}")]
    UnknownProposal {
        /// Unknown proposal id.
        proposal_id: ProposalId,
    },
    /// Target debug configuration was not present in current projections.
    #[error("unknown debug configuration: {configuration_id:?}")]
    UnknownDebugConfiguration {
        /// Unknown debug configuration.
        configuration_id: DebugConfigurationId,
    },
    /// Target debug session was not present in current projections.
    #[error("unknown debug session: {session_id:?}")]
    UnknownDebugSession {
        /// Unknown debug session.
        session_id: DebugSessionId,
    },
    /// Assisted-AI run id was empty or not present in current projections.
    #[error("unknown assisted-ai run: {run_id:?}")]
    UnknownAiRun {
        /// Unknown run id.
        run_id: AgentRunId,
    },
    /// Plugin id was not present in current contribution projections.
    #[error("unknown plugin: {plugin_id:?}")]
    UnknownPlugin {
        /// Unknown plugin id.
        plugin_id: PluginId,
    },
    /// Plugin command id was empty after normalization.
    #[error("plugin command id is empty for plugin {plugin_id:?}")]
    InvalidPluginCommand {
        /// Plugin id for the invalid command request.
        plugin_id: PluginId,
    },
    /// Command id was not present in the selected plugin projection.
    #[error("unknown plugin command: plugin {plugin_id:?} command {command_id}")]
    UnknownPluginCommand {
        /// Plugin id that was present.
        plugin_id: PluginId,
        /// Unknown command id.
        command_id: String,
    },
    /// Collaboration session id was zero.
    #[error("collaboration session id must be non-zero")]
    InvalidCollaborationSession,
    /// Collaboration participant id was zero.
    #[error("collaboration participant id must be non-zero")]
    InvalidCollaborationParticipant,
    /// Collaboration runtime is not enabled in the current app projection.
    #[error("collaboration runtime is disabled by policy")]
    CollaborationRuntimeUnavailable,
    /// Collaboration session was not present in current projections.
    #[error("unknown collaboration session: {session_id:?}")]
    UnknownCollaborationSession {
        /// Unknown collaboration session.
        session_id: CollaborationSessionId,
    },
    /// Shared collaboration proposal row was not present in current projections.
    #[error(
        "unknown shared collaboration proposal: session {session_id:?} proposal {proposal_id:?}"
    )]
    UnknownSharedCollaborationProposal {
        /// Collaboration session id.
        session_id: CollaborationSessionId,
        /// Proposal id.
        proposal_id: ProposalId,
    },
    /// Remote workspace session id was zero.
    #[error("remote workspace session id must be non-zero")]
    InvalidRemoteWorkspaceSession,
    /// Remote authority label was empty after normalization.
    #[error("remote authority label is empty")]
    InvalidRemoteAuthority,
    /// Remote runtime is not enabled in the current app projection.
    #[error("remote workspace runtime is disabled by policy")]
    RemoteRuntimeUnavailable,
    /// Remote workspace session was not present in current projections.
    #[error("unknown remote workspace session: {session_id:?}")]
    UnknownRemoteWorkspaceSession {
        /// Unknown remote workspace session.
        session_id: RemoteWorkspaceSessionId,
    },
    /// Remote proposal row was not present in current projections.
    #[error("unknown remote proposal: session {session_id:?} proposal {proposal_id:?}")]
    UnknownRemoteProposal {
        /// Remote workspace session id.
        session_id: RemoteWorkspaceSessionId,
        /// Proposal id.
        proposal_id: ProposalId,
    },
    /// Delegated task plan id was empty.
    #[error("delegated task plan id is empty")]
    InvalidDelegatedTaskPlan,
    /// Delegated task plan was not present in current projections.
    #[error("unknown delegated task plan: {plan_id:?}")]
    UnknownDelegatedTaskPlan {
        /// Unknown delegated task plan.
        plan_id: DelegatedTaskPlanId,
    },
    /// Delegated proposal-preview link was not present in current projections.
    #[error("unknown delegated proposal preview: {proposal_id:?}")]
    UnknownDelegatedProposalPreview {
        /// Unknown proposal id.
        proposal_id: ProposalId,
    },
    /// Delegated proposal hunk request was invalid.
    #[error("delegated proposal hunk request is invalid")]
    InvalidDelegatedProposalHunk,
    /// Delegated proposal hunk was not present in current projections.
    #[error("unknown delegated proposal hunk: proposal {proposal_id:?} hunk {hunk_id}")]
    UnknownDelegatedProposalHunk {
        /// Unknown proposal id.
        proposal_id: ProposalId,
        /// Unknown hunk id.
        hunk_id: String,
    },
    /// Delegated tool permission request id was empty.
    #[error("delegated tool permission request id is empty")]
    InvalidDelegatedToolPermissionRequest,
    /// Delegated tool permission request was not present in current projections.
    #[error("unknown delegated tool permission request: {request_id}")]
    UnknownDelegatedToolPermissionRequest {
        /// Unknown permission request id.
        request_id: String,
    },
    /// Legion workflow plan id was empty.
    #[error("legion workflow plan id is empty")]
    InvalidLegionWorkflowPlan,
    /// Legion workflow session id was empty.
    #[error("legion workflow session id is empty")]
    InvalidLegionWorkflowSession,
    /// Legion workflow session was not present in current projections.
    #[error("unknown Legion workflow session: {session_id:?}")]
    UnknownLegionWorkflowSession {
        /// Unknown workflow session.
        session_id: LegionWorkflowSessionId,
    },
    /// Legion workflow linked proposal was not present in current projections.
    #[error("unknown Legion workflow proposal: session {session_id:?} proposal {proposal_id:?}")]
    UnknownLegionWorkflowProposal {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// Unknown proposal id.
        proposal_id: ProposalId,
    },
    /// Legion workflow verification gate was not present in current projections.
    #[error("unknown Legion workflow verification gate: session {session_id:?} gate {gate_id:?}")]
    UnknownLegionWorkflowVerification {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// Unknown verification gate id.
        gate_id: LegionWorkflowVerificationGateId,
    },
    /// Legion workflow sign-off record was not present in current projections.
    #[error("unknown Legion workflow sign-off: session {session_id:?} signoff {sign_off_id:?}")]
    UnknownLegionWorkflowSignOff {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// Unknown sign-off id.
        sign_off_id: LegionWorkflowSignOffId,
    },
    /// Legion workflow conflict id was not present in current projections.
    #[error("unknown Legion workflow conflict: session {session_id:?} conflict {conflict_id:?}")]
    UnknownLegionWorkflowConflict {
        /// Workflow session identifier.
        session_id: LegionWorkflowSessionId,
        /// Unknown conflict id.
        conflict_id: LegionWorkflowConflictId,
    },
    /// MCP server was not present in current Automate projections.
    #[error("unknown MCP server: {server_id:?}")]
    UnknownLegionWorkflowMcpServer {
        /// Unknown MCP server id.
        server_id: legion_protocol::McpServerId,
    },
    /// MCP tool was not present in current Automate projections.
    #[error("unknown MCP tool: server {server_id:?} tool {tool_name:?}")]
    UnknownLegionWorkflowMcpTool {
        /// MCP server id.
        server_id: legion_protocol::McpServerId,
        /// Unknown MCP tool name.
        tool_name: legion_protocol::McpToolName,
    },
    /// Target buffer does not own the active dirty-close prompt.
    #[error("dirty-close prompt is not active for buffer {buffer_id:?}")]
    DirtyClosePromptMissing {
        /// Target buffer without an active prompt.
        buffer_id: BufferId,
    },
    /// Assisted-AI start action was missing a display-safe instruction label.
    #[error("assisted-ai instruction label is empty")]
    InvalidInstructionLabel,
    /// Path text was empty after trimming.
    #[error("path input is empty")]
    InvalidPathInput,
    /// The action is intentionally not supported by this phase.
    #[error("unsupported desktop action: {action}")]
    UnsupportedAction {
        /// Unsupported action label.
        action: &'static str,
    },
    /// A terminal action requires an active terminal session projection.
    #[error("active terminal session is required for this desktop action")]
    MissingActiveTerminalSession,
    /// A debug poll/stop action requires an active debug session projection.
    #[error("active debug session is required for this desktop action")]
    MissingActiveDebugSession,
}

/// Adapter-local command bridge.
#[derive(Debug, Default)]
pub struct DesktopCommandBridge;

impl DesktopCommandBridge {
    /// Creates a bridge that owns no app/editor/workspace state.
    pub fn new() -> Self {
        Self
    }

    /// Translate a desktop action into a command intent, app request, no-op, or typed error.
    pub fn translate(
        &self,
        action: DesktopAction,
        snapshot: &ShellProjectionSnapshot,
    ) -> DesktopBridgeOutput {
        match action {
            DesktopAction::Quit => DesktopBridgeOutput::Intent(CommandDispatchIntent::Quit),
            DesktopAction::SetProductMode { mode } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetProductMode { mode })
            }
            DesktopAction::SaveActive => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::Save { buffer_id }
            }),
            DesktopAction::SaveAll => DesktopBridgeOutput::Intent(CommandDispatchIntent::SaveAll),
            DesktopAction::SwitchTab { buffer_id } => {
                self.with_known_tab(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::SwitchTab { buffer_id }
                })
            }
            DesktopAction::CloseTab { buffer_id } => {
                self.with_known_tab(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::CloseTab { buffer_id }
                })
            }
            DesktopAction::SaveDirtyClose { buffer_id } => {
                self.with_dirty_close_prompt(snapshot, buffer_id, |buffer_id| {
                    DesktopBridgeOutput::Intent(CommandDispatchIntent::Save { buffer_id })
                })
            }
            DesktopAction::CancelDirtyClose { buffer_id } => {
                self.with_dirty_close_prompt(snapshot, buffer_id, |buffer_id| {
                    DesktopBridgeOutput::AppRequest(DesktopAppRequest::CancelDirtyClose {
                        buffer_id,
                    })
                })
            }
            DesktopAction::OpenPathText(path) | DesktopAction::OpenPathDialogSelected(path) => {
                match normalized_path(path) {
                    Some(path) => {
                        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPath { path })
                    }
                    None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPathInput),
                }
            }
            DesktopAction::OpenPathDialogCancelled => DesktopBridgeOutput::Noop,
            DesktopAction::OpenPalette { mode, query, scope } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPalette {
                    mode,
                    query,
                    scope,
                })
            }
            DesktopAction::ClosePalette => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::ClosePalette)
            }
            DesktopAction::UpdatePaletteQuery { query } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::UpdatePaletteQuery { query })
            }
            DesktopAction::MovePaletteSelection { delta } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::MovePaletteSelection { delta })
            }
            DesktopAction::CompletePaletteSelection => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::CompletePaletteSelection)
            }
            DesktopAction::DispatchPaletteSelection => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::DispatchPaletteSelection)
            }
            DesktopAction::DismissToast { .. } => DesktopBridgeOutput::Noop,
            DesktopAction::DismissOnboarding => DesktopBridgeOutput::Noop,
            DesktopAction::InvokeToastAction { intent } => DesktopBridgeOutput::Intent(intent),
            DesktopAction::OpenSettings => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenSettings)
            }
            DesktopAction::SetThemePreference { preference } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetThemePreference {
                    preference,
                })
            }
            DesktopAction::SetZoomPercent { zoom_percent } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetZoomPercent { zoom_percent })
            }
            DesktopAction::SetEditorFontSize { font_size_pt } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetEditorFontSize {
                    font_size_pt,
                })
            }
            DesktopAction::SetToastVerbosity { verbosity } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetToastVerbosity { verbosity })
            }
            DesktopAction::SetLineNumbersVisible { visible } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetLineNumbersVisible {
                    visible,
                })
            }
            DesktopAction::SetCurrentLineHighlight { enabled } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetCurrentLineHighlight {
                    enabled,
                })
            }
            DesktopAction::SetStickyHeadersVisible { visible } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetStickyHeadersVisible {
                    visible,
                })
            }
            DesktopAction::SetCodeFoldingVisible { visible } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetCodeFoldingVisible {
                    visible,
                })
            }
            DesktopAction::SetMinimapVisible { visible } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetMinimapVisible { visible })
            }
            DesktopAction::SetWhitespaceGuidesVisible { visible } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetWhitespaceGuidesVisible {
                    visible,
                })
            }
            DesktopAction::SetIndentGuidesVisible { visible } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetIndentGuidesVisible {
                    visible,
                })
            }
            DesktopAction::SetSmoothScrollingEnabled { enabled } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetSmoothScrollingEnabled {
                    enabled,
                })
            }
            DesktopAction::SetIndexedWorkspaceSearchEnabled { enabled } => {
                DesktopBridgeOutput::Intent(
                    CommandDispatchIntent::SetIndexedWorkspaceSearchEnabled { enabled },
                )
            }
            DesktopAction::SetNextEditPredictionEnabled { enabled } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetNextEditPredictionEnabled {
                    enabled,
                })
            }
            DesktopAction::SetCrashReportsEnabled { enabled } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SetCrashReportsEnabled {
                    enabled,
                })
            }
            DesktopAction::ResetSettings => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::ResetSettings)
            }
            DesktopAction::OpenWorkspace { root } => {
                DesktopBridgeOutput::AppRequest(DesktopAppRequest::OpenWorkspace { root })
            }
            DesktopAction::RefreshExplorer => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshExplorer)
            }
            DesktopAction::RefreshGit => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshGit)
            }
            DesktopAction::RefreshTestExplorer => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshTestExplorer)
            }
            DesktopAction::PushGitRemote => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::PushGitRemote {
                    remote: "origin".to_string(),
                })
            }
            DesktopAction::OpenGitPullRequestUrl => {
                let Some(remote_url) = snapshot.git_projection.remote_url.as_deref() else {
                    return DesktopBridgeOutput::Error(DesktopBridgeError::MissingGitRemoteUrl);
                };
                let Some(branch_label) = snapshot.git_projection.branch_label.as_deref() else {
                    return DesktopBridgeOutput::Error(DesktopBridgeError::MissingGitBranchLabel);
                };
                let base_branch = match snapshot
                    .git_projection
                    .remote_default_branch
                    .as_deref()
                    .map(str::trim)
                    .filter(|branch| !branch.is_empty())
                {
                    Some(base_branch) => base_branch,
                    None => {
                        return DesktopBridgeOutput::Error(
                            DesktopBridgeError::MissingRemoteDefaultBranch,
                        );
                    }
                };
                let Some(url) = git_pull_request_url(remote_url, base_branch, branch_label) else {
                    return DesktopBridgeOutput::Error(
                        DesktopBridgeError::UnsupportedGitForgeRemote {
                            remote_url: remote_url.to_string(),
                        },
                    );
                };
                DesktopBridgeOutput::AppRequest(DesktopAppRequest::OpenExternalUrl { url })
            }
            DesktopAction::SwitchGitBranch { branch } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::SwitchGitBranch { branch })
            }
            DesktopAction::CreateGitBranch { branch } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::CreateGitBranch { branch })
            }
            DesktopAction::DeleteGitBranch { branch } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::DeleteGitBranch { branch })
            }
            DesktopAction::StashGitChanges { message } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::StashGitChanges { message })
            }
            DesktopAction::PruneGitWorktrees => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::PruneGitWorktrees)
            }
            DesktopAction::RemoveGitWorktree { path } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::RemoveGitWorktree { path })
            }
            DesktopAction::RefreshDebugConfigurations => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshDebugConfigurations)
            }
            DesktopAction::ToggleDebugBreakpoint {
                line,
                condition,
                hit_condition,
                log_message,
            } => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::ToggleDebugBreakpoint {
                    buffer_id,
                    line,
                    condition,
                    hit_condition,
                    log_message,
                }
            }),
            DesktopAction::LaunchDebugSession { configuration_id } => self
                .with_known_debug_configuration(snapshot, configuration_id, |configuration_id| {
                    CommandDispatchIntent::LaunchDebugSession { configuration_id }
                }),
            DesktopAction::DebugStep { session_id, kind } => {
                self.with_known_debug_session(snapshot, session_id, |session_id| {
                    CommandDispatchIntent::DebugStep { session_id, kind }
                })
            }
            DesktopAction::DebugRunToCursor {
                session_id,
                position,
            } => self.with_known_debug_session_and_active_buffer(
                snapshot,
                session_id,
                |session_id, buffer_id| CommandDispatchIntent::DebugRunToCursor {
                    session_id,
                    buffer_id,
                    position,
                },
            ),
            DesktopAction::DebugEvaluateSelection {
                session_id,
                expression_label,
            } => self.with_known_debug_session(snapshot, session_id, |session_id| {
                CommandDispatchIntent::DebugEvaluateSelection {
                    session_id,
                    expression_label,
                }
            }),
            DesktopAction::DebugAddWatch {
                session_id,
                expression_label,
            } => self.with_known_debug_session(snapshot, session_id, |session_id| {
                CommandDispatchIntent::DebugAddWatch {
                    session_id,
                    expression_label,
                }
            }),
            DesktopAction::PollDebugSession => self
                .with_active_debug_session(snapshot, |session_id| {
                    CommandDispatchIntent::PollDebugSession { session_id }
                }),
            DesktopAction::StopDebugSession => self
                .with_active_debug_session(snapshot, |session_id| {
                    CommandDispatchIntent::StopDebugSession { session_id }
                }),
            DesktopAction::StageGitHunk { hunk_id } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::StageGitHunk { hunk_id })
            }
            DesktopAction::UnstageGitHunk { hunk_id } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::UnstageGitHunk { hunk_id })
            }
            DesktopAction::AcceptGitConflictCurrent { path } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::ResolveGitConflict {
                    path,
                    choice: GitConflictChoiceProjection::AcceptCurrent,
                })
            }
            DesktopAction::AcceptGitConflictIncoming { path } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::ResolveGitConflict {
                    path,
                    choice: GitConflictChoiceProjection::AcceptIncoming,
                })
            }
            DesktopAction::ToggleExplorerPath { path } => match normalized_path(path) {
                Some(path) => {
                    DesktopBridgeOutput::AppRequest(DesktopAppRequest::ToggleExplorerPath { path })
                }
                None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPathInput),
            },
            DesktopAction::SelectExplorerFile { file_id } => {
                if explorer_contains_file(snapshot, file_id) {
                    DesktopBridgeOutput::Intent(CommandDispatchIntent::RevealInExplorer { file_id })
                } else {
                    DesktopBridgeOutput::Error(DesktopBridgeError::UnknownExplorerFile { file_id })
                }
            }
            DesktopAction::PreviewProposal { proposal_id } => {
                self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                    CommandDispatchIntent::PreviewProposal { proposal_id }
                })
            }
            DesktopAction::ApproveProposal { proposal_id } => {
                self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                    CommandDispatchIntent::ApproveProposal { proposal_id }
                })
            }
            DesktopAction::RejectProposal {
                proposal_id,
                reason,
            } => self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                CommandDispatchIntent::RejectProposal {
                    proposal_id,
                    reason,
                }
            }),
            DesktopAction::ApplyProposal { proposal_id } => {
                self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                    CommandDispatchIntent::ApplyProposal { proposal_id }
                })
            }
            DesktopAction::RollbackProposal {
                proposal_id,
                reason,
            } => self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                CommandDispatchIntent::RollbackProposal {
                    proposal_id,
                    reason,
                }
            }),
            DesktopAction::CancelProposal {
                proposal_id,
                reason,
            } => self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                CommandDispatchIntent::CancelProposal {
                    proposal_id,
                    reason,
                }
            }),
            DesktopAction::OpenProposalDetails { proposal_id } => {
                self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
                    CommandDispatchIntent::OpenProposalDetails { proposal_id }
                })
            }
            DesktopAction::StartAiExplain { instruction_label } => {
                match normalized_instruction(instruction_label) {
                    Some(instruction_label) => {
                        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiExplain {
                            instruction_label,
                        })
                    }
                    None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidInstructionLabel),
                }
            }
            DesktopAction::StartAiProposal { instruction_label } => {
                match normalized_instruction(instruction_label) {
                    Some(instruction_label) => {
                        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiProposal {
                            instruction_label,
                            selection: None,
                        })
                    }
                    None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidInstructionLabel),
                }
            }
            DesktopAction::CancelAiRun { run_id } => {
                self.with_known_ai_run(snapshot, run_id, |run_id| {
                    CommandDispatchIntent::CancelAiRun { run_id }
                })
            }
            DesktopAction::ReplayAiRun { run_id } => {
                self.with_known_ai_run(snapshot, run_id, |run_id| {
                    CommandDispatchIntent::ReplayAiRun { run_id }
                })
            }
            DesktopAction::InspectAiRun { run_id } => {
                self.with_known_ai_run(snapshot, run_id, |run_id| {
                    CommandDispatchIntent::InspectAiRun { run_id }
                })
            }
            DesktopAction::InvokePluginCommand {
                plugin_id,
                command_id,
            } => self.with_known_plugin_command(snapshot, plugin_id, command_id),
            DesktopAction::JoinCollaborationSession { session_id } => {
                self.with_collaboration_join(snapshot, session_id)
            }
            DesktopAction::LeaveCollaborationSession { session_id } => self
                .with_known_collaboration_session(snapshot, session_id, |session_id| {
                    CommandDispatchIntent::LeaveCollaborationSession { session_id }
                }),
            DesktopAction::PublishCollaborationPresence {
                session_id,
                participant_id,
            } => {
                if participant_id.0 == 0 {
                    DesktopBridgeOutput::Error(DesktopBridgeError::InvalidCollaborationParticipant)
                } else {
                    self.with_known_collaboration_session(snapshot, session_id, |session_id| {
                        CommandDispatchIntent::PublishCollaborationPresence {
                            session_id,
                            participant_id,
                        }
                    })
                }
            }
            DesktopAction::OpenSharedProposalReview {
                session_id,
                proposal_id,
            } => self.with_known_shared_collaboration_proposal(snapshot, session_id, proposal_id),
            DesktopAction::ConnectRemoteWorkspace {
                session_id,
                authority_label,
            } => self.with_remote_connect(snapshot, session_id, authority_label),
            DesktopAction::OpenRemoteProposalReview {
                session_id,
                proposal_id,
            } => self.with_known_remote_proposal(snapshot, session_id, proposal_id),
            DesktopAction::InspectDelegatedTaskPlan { plan_id } => {
                self.with_known_delegated_plan(snapshot, plan_id)
            }
            DesktopAction::OpenDelegatedProposalPreview { proposal_id } => self
                .with_known_delegated_proposal(snapshot, proposal_id, |proposal_id| {
                    DesktopAppRequest::OpenDelegatedProposalPreview { proposal_id }
                }),
            DesktopAction::OpenDelegatedProposalDetails { proposal_id } => self
                .with_known_delegated_proposal(snapshot, proposal_id, |proposal_id| {
                    DesktopAppRequest::OpenDelegatedProposalDetails { proposal_id }
                }),
            DesktopAction::SendDelegateChat { prompt_label } => {
                match normalized_instruction(prompt_label) {
                    Some(prompt_label) => {
                        DesktopBridgeOutput::Intent(CommandDispatchIntent::SendDelegateChat {
                            prompt_label,
                        })
                    }
                    None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidInstructionLabel),
                }
            }
            DesktopAction::StartDelegatedTask {
                task_description,
                scope,
            } => match normalized_instruction(task_description) {
                Some(task_description) => {
                    DesktopBridgeOutput::Intent(CommandDispatchIntent::StartDelegatedTask {
                        task_description,
                        scope,
                    })
                }
                None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidInstructionLabel),
            },
            DesktopAction::CancelDelegatedTask => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelDelegatedTask)
            }
            DesktopAction::ReviewDelegateProposalHunk {
                proposal_id,
                hunk_id,
                disposition,
            } => self.with_known_delegated_proposal_hunk(
                snapshot,
                proposal_id,
                hunk_id,
                |proposal_id, hunk_id| CommandDispatchIntent::ReviewDelegateProposalHunk {
                    proposal_id,
                    hunk_id,
                    disposition,
                },
            ),
            DesktopAction::RecordDelegateToolPermission {
                request_id,
                decision,
            } => self.with_known_delegated_tool_permission(snapshot, request_id, |request_id| {
                CommandDispatchIntent::RecordDelegateToolPermission {
                    request_id,
                    decision,
                }
            }),
            DesktopAction::SubmitLegionWorkflowPlanRevision {
                plan_id,
                edited_sections,
            } => match normalized_plan_id(plan_id) {
                Some(plan_id) => DesktopBridgeOutput::AppRequest(
                    DesktopAppRequest::SubmitLegionWorkflowPlanRevision {
                        plan_id,
                        edited_sections,
                    },
                ),
                None => DesktopBridgeOutput::Error(DesktopBridgeError::InvalidLegionWorkflowPlan),
            },
            DesktopAction::ApproveLegionWorkflowPlan { plan_id } => {
                match normalized_plan_id(plan_id) {
                    Some(plan_id) => DesktopBridgeOutput::AppRequest(
                        DesktopAppRequest::ApproveLegionWorkflowPlan { plan_id },
                    ),
                    None => {
                        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidLegionWorkflowPlan)
                    }
                }
            }
            DesktopAction::RejectLegionWorkflowPlan { plan_id } => {
                match normalized_plan_id(plan_id) {
                    Some(plan_id) => DesktopBridgeOutput::AppRequest(
                        DesktopAppRequest::RejectLegionWorkflowPlan { plan_id },
                    ),
                    None => {
                        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidLegionWorkflowPlan)
                    }
                }
            }
            DesktopAction::InspectLegionWorkflowSession { session_id } => self
                .with_known_legion_workflow_session(snapshot, session_id, |session_id| {
                    DesktopAppRequest::InspectLegionWorkflowSession { session_id }
                }),
            DesktopAction::OpenLegionWorkflowProposalPreview {
                session_id,
                proposal_id,
            } => self.with_known_legion_workflow_proposal(
                snapshot,
                session_id,
                proposal_id,
                |session_id, proposal_id| DesktopAppRequest::OpenLegionWorkflowProposalPreview {
                    session_id,
                    proposal_id,
                },
            ),
            DesktopAction::OpenLegionWorkflowProposalDetails {
                session_id,
                proposal_id,
            } => self.with_known_legion_workflow_proposal(
                snapshot,
                session_id,
                proposal_id,
                |session_id, proposal_id| DesktopAppRequest::OpenLegionWorkflowProposalDetails {
                    session_id,
                    proposal_id,
                },
            ),
            DesktopAction::RequestLegionWorkflowVerification {
                session_id,
                gate_id,
            } => self.with_known_legion_workflow_verification(
                snapshot,
                session_id,
                gate_id,
                |session_id, gate_id| DesktopAppRequest::RequestLegionWorkflowVerification {
                    session_id,
                    gate_id,
                },
            ),
            DesktopAction::RequestLegionWorkflowSignOff {
                session_id,
                sign_off_id,
            } => self.with_known_legion_workflow_signoff(
                snapshot,
                session_id,
                sign_off_id,
                |session_id, sign_off_id| DesktopAppRequest::RequestLegionWorkflowSignOff {
                    session_id,
                    sign_off_id,
                },
            ),
            DesktopAction::ResolveLegionWorkflowConflict {
                session_id,
                conflict_id,
            } => self.with_known_legion_workflow_conflict(
                snapshot,
                session_id,
                conflict_id,
                |session_id, conflict_id| DesktopAppRequest::ResolveLegionWorkflowConflict {
                    session_id,
                    conflict_id,
                },
            ),
            DesktopAction::RequestLegionWorkflowMergeReadiness { session_id } => self
                .with_known_legion_workflow_session(snapshot, session_id, |session_id| {
                    DesktopAppRequest::RequestLegionWorkflowMergeReadiness { session_id }
                }),
            DesktopAction::RecordLegionWorkflowToolPermission {
                session_id,
                server_id,
                tool_name,
                decision,
            } => self.with_known_legion_workflow_mcp_tool(
                snapshot,
                session_id,
                server_id,
                tool_name,
                |session_id, server_id, tool_name| {
                    CommandDispatchIntent::RecordLegionWorkflowToolPermission {
                        session_id,
                        server_id,
                        tool_name,
                        decision,
                    }
                },
            ),
            DesktopAction::TriggerLegionWorkflowKillSwitch {
                session_id,
                reason_label,
            } => {
                self.with_known_legion_workflow_session_intent(snapshot, session_id, |session_id| {
                    CommandDispatchIntent::TriggerLegionWorkflowKillSwitch {
                        session_id,
                        reason_label,
                    }
                })
            }
            DesktopAction::InsertText { text, at }
            | DesktopAction::ClipboardPaste { text, at }
            | DesktopAction::ImeCommit { text, at } => {
                self.with_active_buffer(snapshot, |buffer_id| CommandDispatchIntent::Insert {
                    buffer_id,
                    at,
                    text,
                })
            }
            DesktopAction::ReplaceRange { range, replacement } => {
                self.with_active_buffer(snapshot, |buffer_id| CommandDispatchIntent::Replace {
                    buffer_id,
                    range,
                    replacement,
                })
            }
            DesktopAction::DeleteRange { range } => {
                self.with_active_buffer(snapshot, |buffer_id| CommandDispatchIntent::Delete {
                    buffer_id,
                    range,
                })
            }
            DesktopAction::ClipboardCopy => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::ClipboardCopy { buffer_id }
            }),
            DesktopAction::ClipboardCut => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::ClipboardCut { buffer_id }
            }),
            DesktopAction::Undo => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::Undo { buffer_id }
            }),
            DesktopAction::Redo => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::Redo { buffer_id }
            }),
            DesktopAction::SetCursor { buffer_id, cursor } => {
                self.with_resolved_buffer(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::SetCursor { buffer_id, cursor }
                })
            }
            DesktopAction::SetSelection { buffer_id, range } => {
                self.with_resolved_buffer(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::SetSelection { buffer_id, range }
                })
            }
            DesktopAction::SelectAll { buffer_id } => {
                self.with_resolved_buffer(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::SelectAll { buffer_id }
                })
            }
            DesktopAction::SetViewportScroll { buffer_id, scroll } => {
                self.with_resolved_buffer(snapshot, buffer_id, |buffer_id| {
                    CommandDispatchIntent::SetViewportScroll { buffer_id, scroll }
                })
            }
            DesktopAction::RunSearch {
                scope,
                query,
                limit,
                case_sensitive,
                whole_word,
                use_regex,
            } => DesktopBridgeOutput::Intent(CommandDispatchIntent::RunSearch {
                scope,
                query,
                limit,
                case_sensitive,
                whole_word,
                use_regex,
            }),
            DesktopAction::RunStructuralSearch {
                scope,
                pattern,
                rewrite,
                limit,
            } => DesktopBridgeOutput::Intent(CommandDispatchIntent::RunStructuralSearch {
                scope,
                pattern,
                rewrite,
                limit,
            }),
            DesktopAction::CancelSearch { query_id } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelSearch { query_id })
            }
            DesktopAction::RequestHover { position } => {
                self.with_active_buffer(snapshot, |buffer_id| CommandDispatchIntent::RequestHover {
                    buffer_id,
                    position,
                })
            }
            DesktopAction::RequestCompletion { position } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestCompletion {
                        buffer_id,
                        position,
                    }
                })
            }
            DesktopAction::RequestAssistInlinePrediction { position } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestAssistInlinePrediction {
                        buffer_id,
                        position,
                    }
                })
            }
            DesktopAction::AcceptCurrentAssistInlinePrediction => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::AcceptAssistInlinePrediction {
                        buffer_id,
                        prediction_id: active_assist_prediction_id(snapshot),
                    }
                })
            }
            DesktopAction::DismissCurrentAssistInlinePrediction => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::DismissAssistInlinePrediction {
                        buffer_id,
                        prediction_id: active_assist_prediction_id(snapshot),
                    }
                })
            }
            DesktopAction::CancelAssistInlinePrediction => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::CancelAssistInlinePrediction {
                        buffer_id,
                        prediction_id: active_assist_prediction_id(snapshot),
                    }
                })
            }
            DesktopAction::GoToDefinition { position } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::GoToDefinition {
                        buffer_id,
                        position,
                    }
                })
            }
            DesktopAction::FindReferences { position } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::FindReferences {
                        buffer_id,
                        position,
                    }
                })
            }
            DesktopAction::RefreshOutline => self.with_active_buffer(snapshot, |buffer_id| {
                CommandDispatchIntent::RefreshOutline { buffer_id }
            }),
            DesktopAction::RequestFormattingProposal => self
                .with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestFormattingProposal { buffer_id }
                }),
            DesktopAction::RequestRenameProposal { position, new_name } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestRenameProposal {
                        buffer_id,
                        position,
                        new_name,
                    }
                })
            }
            DesktopAction::RequestOrganizeImportsProposal => self
                .with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestOrganizeImportsProposal { buffer_id }
                }),
            DesktopAction::RequestCodeActionProposal { action_id } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::RequestCodeActionProposal {
                        buffer_id,
                        action_id,
                    }
                })
            }
            DesktopAction::CancelLanguageOperation { operation_id } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelLanguageOperation {
                    operation_id,
                })
            }
            DesktopAction::NavigateToProblem { path, line } => {
                // D3: open the file at the diagnostic's start line.
                DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPathAtPosition {
                    path,
                    position: legion_protocol::TextCoordinate {
                        line,
                        character: 0,
                        byte_offset: None,
                        utf16_offset: None,
                    },
                })
            }
            // T4: problems panel keyboard-nav intercepted in DesktopRuntime::handle_action.
            DesktopAction::ProblemNext
            | DesktopAction::ProblemPrev
            | DesktopAction::ProblemActivate => DesktopBridgeOutput::Noop,
            // PKT-DIFF: proposal review hunk navigation + disposition + apply/dismiss are
            // all intercepted in DesktopRuntime::handle_action before reaching the bridge.
            DesktopAction::ReviewHunkNext
            | DesktopAction::ReviewHunkPrev
            | DesktopAction::ReviewHunkAccept
            | DesktopAction::ReviewHunkReject
            | DesktopAction::ReviewAcceptAll
            | DesktopAction::ReviewRejectAll
            | DesktopAction::ReviewApply
            | DesktopAction::ReviewDismiss => DesktopBridgeOutput::Noop,
            // T6: completion popup actions are intercepted in DesktopRuntime::handle_action
            // before reaching the bridge.  These arms exist solely for exhaustiveness.
            DesktopAction::CompletionNext
            | DesktopAction::CompletionPrev
            | DesktopAction::CompletionAccept
            | DesktopAction::CompletionDismiss => DesktopBridgeOutput::Noop,
            // T7: hover/definition actions intercepted in DesktopRuntime::handle_action.
            DesktopAction::HoverDismiss | DesktopAction::NavigateToDefinition { .. } => {
                DesktopBridgeOutput::Noop
            }
            DesktopAction::TerminalLaunch { command_label } => {
                DesktopBridgeOutput::Intent(CommandDispatchIntent::TerminalLaunch {
                    command_label,
                    // Interactive desktop launches use the product default;
                    // only headless smokes override the session deadline.
                    timeout_secs: None,
                })
            }
            DesktopAction::TerminalInput { payload } => {
                self.with_active_terminal(snapshot, |session_id| {
                    CommandDispatchIntent::TerminalInput {
                        session_id,
                        payload,
                    }
                })
            }
            DesktopAction::TerminalResize { cols, rows } => {
                self.with_active_terminal(snapshot, |session_id| {
                    CommandDispatchIntent::TerminalResize {
                        session_id,
                        cols,
                        rows,
                    }
                })
            }
            DesktopAction::TerminalKill => self.with_active_terminal(snapshot, |session_id| {
                CommandDispatchIntent::TerminalKill { session_id }
            }),
            DesktopAction::TerminalClose => self.with_active_terminal(snapshot, |session_id| {
                CommandDispatchIntent::TerminalClose { session_id }
            }),
            DesktopAction::TerminalOutputPoll => self
                .with_active_terminal(snapshot, |session_id| {
                    CommandDispatchIntent::TerminalOutputPoll { session_id }
                }),
            DesktopAction::TerminalSearch { query } => self
                .with_active_terminal(snapshot, |session_id| {
                    CommandDispatchIntent::TerminalSearch { session_id, query }
                }),
            // PKT-CKPT: handled in DesktopWorkflowRuntime::handle_action before reaching the
            // bridge; this arm satisfies exhaustiveness but is never evaluated in production.
            DesktopAction::RestoreCheckpoint { .. } => DesktopBridgeOutput::Noop,
            // PKT-PROV: handled in DesktopWorkflowRuntime::handle_action before reaching the
            // bridge; these arms satisfy exhaustiveness but are never evaluated in production.
            DesktopAction::SetProviderApiKey { .. }
            | DesktopAction::DeleteProviderApiKey { .. }
            | DesktopAction::SetPreferredAiProvider { .. } => DesktopBridgeOutput::Noop,
            // PKT-RAIL: ghost text acceptance goes through the existing inline-prediction
            // acceptance path so no direct buffer mutation occurs.
            DesktopAction::AcceptGhostText { request_id } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::AcceptAssistInlinePrediction {
                        buffer_id,
                        prediction_id: Some(request_id.0),
                    }
                })
            }
            DesktopAction::DismissGhostText { request_id } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::DismissAssistInlinePrediction {
                        buffer_id,
                        prediction_id: Some(request_id.0),
                    }
                })
            }
            DesktopAction::CancelGhostText { request_id } => {
                self.with_active_buffer(snapshot, |buffer_id| {
                    CommandDispatchIntent::CancelAssistInlinePrediction {
                        buffer_id,
                        prediction_id: Some(request_id.0),
                    }
                })
            }
            // PKT-RAIL: rail commands dispatch a proposal-only AI run; no direct mutation.
            DesktopAction::ExecuteRailCommand { command, selection } => {
                let instruction_label = match command {
                    AssistantRailCommand::Explain => "ai.rail.explain",
                    AssistantRailCommand::Fix => "ai.rail.fix",
                    AssistantRailCommand::Test => "ai.rail.test",
                    AssistantRailCommand::Doc => "ai.rail.doc",
                    AssistantRailCommand::Refactor => "ai.rail.refactor",
                };
                DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiProposal {
                    instruction_label: instruction_label.to_string(),
                    selection,
                })
            }
            // PKT-INLINE: inline edit hunk accept/reject/apply/dismiss are intercepted in
            // DesktopRuntime::handle_action before reaching the bridge.  These arms exist
            // solely for exhaustiveness so the compiler enforces that every DesktopAction
            // variant is handled.
            DesktopAction::AcceptInlineEditHunk { .. }
            | DesktopAction::RejectInlineEditHunk { .. }
            | DesktopAction::ApplyInlineEdit { .. }
            | DesktopAction::DismissInlineEdit { .. } => DesktopBridgeOutput::Noop,
        }
    }

    fn with_active_buffer(
        &self,
        snapshot: &ShellProjectionSnapshot,
        build: impl FnOnce(BufferId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        match snapshot.active_buffer_projection.buffer_id {
            Some(buffer_id) => DesktopBridgeOutput::Intent(build(buffer_id)),
            None => DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveBuffer),
        }
    }

    fn with_active_terminal(
        &self,
        snapshot: &ShellProjectionSnapshot,
        build: impl FnOnce(TerminalSessionId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        match snapshot.terminal_panel_projection.active_session_id {
            Some(session_id) => DesktopBridgeOutput::Intent(build(session_id)),
            None => DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveTerminalSession),
        }
    }

    fn with_active_debug_session(
        &self,
        snapshot: &ShellProjectionSnapshot,
        build: impl FnOnce(DebugSessionId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        match snapshot.debug_projection.active_session_id.clone() {
            Some(session_id) => DesktopBridgeOutput::Intent(build(session_id)),
            None => DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveDebugSession),
        }
    }

    fn with_resolved_buffer(
        &self,
        snapshot: &ShellProjectionSnapshot,
        requested: Option<BufferId>,
        build: impl FnOnce(BufferId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        let Some(buffer_id) = requested
            .or(snapshot.daily_editing_projection.tabs.active_buffer_id)
            .or(snapshot.active_buffer_projection.buffer_id)
        else {
            return DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveBuffer);
        };

        self.with_known_tab(snapshot, buffer_id, build)
    }

    fn with_known_tab(
        &self,
        snapshot: &ShellProjectionSnapshot,
        buffer_id: BufferId,
        build: impl FnOnce(BufferId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if tab_is_known(snapshot, buffer_id) {
            DesktopBridgeOutput::Intent(build(buffer_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownTab { buffer_id })
        }
    }

    fn with_dirty_close_prompt(
        &self,
        snapshot: &ShellProjectionSnapshot,
        buffer_id: BufferId,
        build: impl FnOnce(BufferId) -> DesktopBridgeOutput,
    ) -> DesktopBridgeOutput {
        if !tab_is_known(snapshot, buffer_id) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownTab { buffer_id });
        }
        if snapshot
            .daily_editing_projection
            .close_dirty_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.buffer_id == buffer_id)
        {
            build(buffer_id)
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::DirtyClosePromptMissing { buffer_id })
        }
    }

    fn with_known_proposal(
        &self,
        snapshot: &ShellProjectionSnapshot,
        proposal_id: ProposalId,
        build: impl FnOnce(ProposalId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if proposal_is_known(snapshot, proposal_id) {
            DesktopBridgeOutput::Intent(build(proposal_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownProposal { proposal_id })
        }
    }

    fn with_known_debug_configuration(
        &self,
        snapshot: &ShellProjectionSnapshot,
        configuration_id: DebugConfigurationId,
        build: impl FnOnce(DebugConfigurationId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if debug_configuration_is_known(snapshot, &configuration_id) {
            DesktopBridgeOutput::Intent(build(configuration_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownDebugConfiguration {
                configuration_id,
            })
        }
    }

    fn with_known_debug_session(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: DebugSessionId,
        build: impl FnOnce(DebugSessionId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if debug_session_is_known(snapshot, &session_id) {
            DesktopBridgeOutput::Intent(build(session_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownDebugSession { session_id })
        }
    }

    fn with_known_debug_session_and_active_buffer(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: DebugSessionId,
        build: impl FnOnce(DebugSessionId, BufferId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if !debug_session_is_known(snapshot, &session_id) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownDebugSession {
                session_id,
            });
        }

        match snapshot.active_buffer_projection.buffer_id {
            Some(buffer_id) => DesktopBridgeOutput::Intent(build(session_id, buffer_id)),
            None => DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveBuffer),
        }
    }

    fn with_known_ai_run(
        &self,
        snapshot: &ShellProjectionSnapshot,
        run_id: AgentRunId,
        build: impl FnOnce(AgentRunId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if assisted_ai_projection_references_run(snapshot, &run_id) {
            DesktopBridgeOutput::Intent(build(run_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownAiRun { run_id })
        }
    }

    fn with_known_plugin_command(
        &self,
        snapshot: &ShellProjectionSnapshot,
        plugin_id: PluginId,
        command_id: String,
    ) -> DesktopBridgeOutput {
        let Some(command_id) = normalized_plugin_command(command_id) else {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPluginCommand {
                plugin_id,
            });
        };
        let Some(projection) = snapshot
            .plugin_contribution_projections
            .iter()
            .find(|projection| projection.plugin_id == plugin_id)
        else {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownPlugin { plugin_id });
        };
        let Some(command) = projection
            .contributions
            .iter()
            .filter_map(|contribution| match contribution {
                PluginContribution::Command(command) => Some(command),
                _ => None,
            })
            .find(|command| command.command_id == command_id)
        else {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownPluginCommand {
                plugin_id,
                command_id,
            });
        };

        DesktopBridgeOutput::Intent(CommandDispatchIntent::InvokePluginCommand {
            plugin_id,
            command_id: command.command_id.clone(),
            metadata_label: plugin_command_metadata_label(
                plugin_id,
                &command.command_id,
                &command.title,
                &command.required_capability.0,
                &projection.status_label,
            ),
        })
    }

    fn with_collaboration_join(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: CollaborationSessionId,
    ) -> DesktopBridgeOutput {
        if session_id.0 == 0 {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidCollaborationSession);
        }
        if !snapshot.collaboration_gui_projection.runtime_enabled {
            return DesktopBridgeOutput::Error(DesktopBridgeError::CollaborationRuntimeUnavailable);
        }
        DesktopBridgeOutput::Intent(CommandDispatchIntent::JoinCollaborationSession { session_id })
    }

    fn with_known_collaboration_session(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: CollaborationSessionId,
        build: impl FnOnce(CollaborationSessionId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if session_id.0 == 0 {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidCollaborationSession);
        }
        if collaboration_session_is_known(snapshot, session_id) {
            DesktopBridgeOutput::Intent(build(session_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownCollaborationSession {
                session_id,
            })
        }
    }

    fn with_known_shared_collaboration_proposal(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: CollaborationSessionId,
        proposal_id: ProposalId,
    ) -> DesktopBridgeOutput {
        if session_id.0 == 0 {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidCollaborationSession);
        }
        if !shared_collaboration_proposal_is_known(snapshot, session_id, proposal_id) {
            return DesktopBridgeOutput::Error(
                DesktopBridgeError::UnknownSharedCollaborationProposal {
                    session_id,
                    proposal_id,
                },
            );
        }
        self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
            CommandDispatchIntent::OpenProposalDetails { proposal_id }
        })
    }

    fn with_remote_connect(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: RemoteWorkspaceSessionId,
        authority_label: String,
    ) -> DesktopBridgeOutput {
        if session_id.0 == 0 {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidRemoteWorkspaceSession);
        }
        let Some(authority_label) = normalized_remote_authority(authority_label) else {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidRemoteAuthority);
        };
        if !snapshot.remote_gui_projection.runtime_enabled {
            return DesktopBridgeOutput::Error(DesktopBridgeError::RemoteRuntimeUnavailable);
        }
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::ConnectRemoteWorkspace {
            session_id,
            authority_label,
        })
    }

    fn with_known_remote_proposal(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: RemoteWorkspaceSessionId,
        proposal_id: ProposalId,
    ) -> DesktopBridgeOutput {
        if session_id.0 == 0 {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidRemoteWorkspaceSession);
        }
        if !remote_session_is_known(snapshot, session_id) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownRemoteWorkspaceSession {
                session_id,
            });
        }
        if !remote_proposal_is_known(snapshot, session_id, proposal_id) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownRemoteProposal {
                session_id,
                proposal_id,
            });
        }
        self.with_known_proposal(snapshot, proposal_id, |proposal_id| {
            CommandDispatchIntent::OpenProposalDetails { proposal_id }
        })
    }

    fn with_known_delegated_plan(
        &self,
        snapshot: &ShellProjectionSnapshot,
        plan_id: DelegatedTaskPlanId,
    ) -> DesktopBridgeOutput {
        if plan_id.0.trim().is_empty() {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidDelegatedTaskPlan);
        }
        if delegated_plan_is_known(snapshot, &plan_id) {
            DesktopBridgeOutput::AppRequest(DesktopAppRequest::InspectDelegatedTaskPlan { plan_id })
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownDelegatedTaskPlan { plan_id })
        }
    }

    fn with_known_delegated_proposal(
        &self,
        snapshot: &ShellProjectionSnapshot,
        proposal_id: ProposalId,
        build: impl FnOnce(ProposalId) -> DesktopAppRequest,
    ) -> DesktopBridgeOutput {
        if !delegated_proposal_preview_is_known(snapshot, proposal_id) {
            return DesktopBridgeOutput::Error(
                DesktopBridgeError::UnknownDelegatedProposalPreview { proposal_id },
            );
        }
        if proposal_is_known(snapshot, proposal_id) {
            DesktopBridgeOutput::AppRequest(build(proposal_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownProposal { proposal_id })
        }
    }

    fn with_known_delegated_proposal_hunk(
        &self,
        snapshot: &ShellProjectionSnapshot,
        proposal_id: ProposalId,
        hunk_id: String,
        build: impl FnOnce(ProposalId, String) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if hunk_id.trim().is_empty() {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidDelegatedProposalHunk);
        }
        if delegated_proposal_hunk_is_known(snapshot, proposal_id, &hunk_id) {
            DesktopBridgeOutput::Intent(build(proposal_id, hunk_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownDelegatedProposalHunk {
                proposal_id,
                hunk_id,
            })
        }
    }

    fn with_known_delegated_tool_permission(
        &self,
        snapshot: &ShellProjectionSnapshot,
        request_id: String,
        build: impl FnOnce(String) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        let request_id = request_id.trim().to_string();
        if request_id.is_empty() {
            return DesktopBridgeOutput::Error(
                DesktopBridgeError::InvalidDelegatedToolPermissionRequest,
            );
        }
        if delegated_tool_permission_is_known(snapshot, &request_id) {
            DesktopBridgeOutput::Intent(build(request_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownDelegatedToolPermissionRequest {
                request_id,
            })
        }
    }

    fn with_known_legion_workflow_session(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: LegionWorkflowSessionId,
        build: impl FnOnce(LegionWorkflowSessionId) -> DesktopAppRequest,
    ) -> DesktopBridgeOutput {
        if session_id.0.trim().is_empty() {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidLegionWorkflowSession);
        }
        if legion_workflow_session_is_known(snapshot, &session_id) {
            DesktopBridgeOutput::AppRequest(build(session_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowSession {
                session_id,
            })
        }
    }

    fn with_known_legion_workflow_session_intent(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: LegionWorkflowSessionId,
        build: impl FnOnce(LegionWorkflowSessionId) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if session_id.0.trim().is_empty() {
            return DesktopBridgeOutput::Error(DesktopBridgeError::InvalidLegionWorkflowSession);
        }
        if legion_workflow_session_is_known(snapshot, &session_id) {
            DesktopBridgeOutput::Intent(build(session_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowSession {
                session_id,
            })
        }
    }

    fn with_known_legion_workflow_mcp_tool(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: LegionWorkflowSessionId,
        server_id: legion_protocol::McpServerId,
        tool_name: legion_protocol::McpToolName,
        build: impl FnOnce(
            LegionWorkflowSessionId,
            legion_protocol::McpServerId,
            legion_protocol::McpToolName,
        ) -> CommandDispatchIntent,
    ) -> DesktopBridgeOutput {
        if !legion_workflow_session_is_known(snapshot, &session_id) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowSession {
                session_id,
            });
        }
        if !legion_workflow_mcp_server_is_known(snapshot, &server_id) {
            return DesktopBridgeOutput::Error(
                DesktopBridgeError::UnknownLegionWorkflowMcpServer { server_id },
            );
        }
        if !legion_workflow_mcp_tool_is_known(snapshot, &server_id, &tool_name) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowMcpTool {
                server_id,
                tool_name,
            });
        }
        DesktopBridgeOutput::Intent(build(session_id, server_id, tool_name))
    }

    fn with_known_legion_workflow_proposal(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: LegionWorkflowSessionId,
        proposal_id: ProposalId,
        build: impl FnOnce(LegionWorkflowSessionId, ProposalId) -> DesktopAppRequest,
    ) -> DesktopBridgeOutput {
        if !legion_workflow_session_is_known(snapshot, &session_id) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowSession {
                session_id,
            });
        }
        if !legion_workflow_proposal_is_known(snapshot, &session_id, proposal_id) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowProposal {
                session_id,
                proposal_id,
            });
        }
        if proposal_is_known(snapshot, proposal_id) {
            DesktopBridgeOutput::AppRequest(build(session_id, proposal_id))
        } else {
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownProposal { proposal_id })
        }
    }

    fn with_known_legion_workflow_verification(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: LegionWorkflowSessionId,
        gate_id: LegionWorkflowVerificationGateId,
        build: impl FnOnce(
            LegionWorkflowSessionId,
            LegionWorkflowVerificationGateId,
        ) -> DesktopAppRequest,
    ) -> DesktopBridgeOutput {
        if !legion_workflow_metadata_label_is_known(snapshot, &session_id, &gate_id.0) {
            return DesktopBridgeOutput::Error(
                DesktopBridgeError::UnknownLegionWorkflowVerification {
                    session_id,
                    gate_id,
                },
            );
        }
        DesktopBridgeOutput::AppRequest(build(session_id, gate_id))
    }

    fn with_known_legion_workflow_signoff(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: LegionWorkflowSessionId,
        sign_off_id: LegionWorkflowSignOffId,
        build: impl FnOnce(LegionWorkflowSessionId, LegionWorkflowSignOffId) -> DesktopAppRequest,
    ) -> DesktopBridgeOutput {
        if !legion_workflow_metadata_label_is_known(snapshot, &session_id, &sign_off_id.0) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowSignOff {
                session_id,
                sign_off_id,
            });
        }
        DesktopBridgeOutput::AppRequest(build(session_id, sign_off_id))
    }

    fn with_known_legion_workflow_conflict(
        &self,
        snapshot: &ShellProjectionSnapshot,
        session_id: LegionWorkflowSessionId,
        conflict_id: LegionWorkflowConflictId,
        build: impl FnOnce(LegionWorkflowSessionId, LegionWorkflowConflictId) -> DesktopAppRequest,
    ) -> DesktopBridgeOutput {
        if !legion_workflow_metadata_label_is_known(snapshot, &session_id, &conflict_id.0) {
            return DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowConflict {
                session_id,
                conflict_id,
            });
        }
        DesktopBridgeOutput::AppRequest(build(session_id, conflict_id))
    }
}

fn normalized_path(path: String) -> Option<String> {
    let path = path.trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

fn normalized_instruction(label: String) -> Option<String> {
    let label = label.trim();
    if label.is_empty() {
        None
    } else {
        Some(label.to_string())
    }
}

fn normalized_plan_id(plan_id: String) -> Option<String> {
    let plan_id = plan_id.trim();
    if plan_id.is_empty() {
        None
    } else {
        Some(plan_id.to_string())
    }
}

fn normalized_plugin_command(command_id: String) -> Option<String> {
    let command_id = command_id.trim();
    if command_id.is_empty() {
        None
    } else {
        Some(command_id.to_string())
    }
}

fn normalized_remote_authority(authority_label: String) -> Option<String> {
    let authority_label = authority_label.trim();
    if authority_label.is_empty() {
        None
    } else {
        Some(authority_label.to_string())
    }
}

fn plugin_command_metadata_label(
    plugin_id: PluginId,
    command_id: &str,
    title: &str,
    capability: &str,
    status_label: &str,
) -> String {
    format!(
        "plugin {} command {command_id}: {title} (status={status_label} capability={capability})",
        plugin_id.0
    )
}

fn tab_is_known(snapshot: &ShellProjectionSnapshot, buffer_id: BufferId) -> bool {
    let tabs = &snapshot.daily_editing_projection.tabs.tabs;
    if tabs.is_empty() {
        return snapshot.active_buffer_projection.buffer_id == Some(buffer_id);
    }

    tabs.iter().any(|tab| tab.buffer_id == buffer_id)
}

fn explorer_contains_file(snapshot: &ShellProjectionSnapshot, file_id: FileId) -> bool {
    snapshot
        .explorer_projection
        .nodes
        .iter()
        .any(|node| node.file_id == file_id)
}

fn proposal_is_known(snapshot: &ShellProjectionSnapshot, proposal_id: ProposalId) -> bool {
    snapshot
        .proposal_ledger_projection
        .rows
        .iter()
        .any(|row| row.proposal_id == proposal_id)
}

fn debug_configuration_is_known(
    snapshot: &ShellProjectionSnapshot,
    configuration_id: &DebugConfigurationId,
) -> bool {
    !configuration_id.0.trim().is_empty()
        && snapshot
            .debug_projection
            .configurations
            .iter()
            .any(|configuration| &configuration.configuration_id == configuration_id)
}

fn debug_session_is_known(snapshot: &ShellProjectionSnapshot, session_id: &DebugSessionId) -> bool {
    if session_id.0.trim().is_empty() {
        return false;
    }

    snapshot.debug_projection.active_session_id.as_ref() == Some(session_id)
        || snapshot
            .debug_projection
            .breakpoints
            .iter()
            .any(|breakpoint| breakpoint.session_id.as_ref() == Some(session_id))
        || snapshot
            .debug_projection
            .stack_frames
            .iter()
            .any(|frame| &frame.session_id == session_id)
        || snapshot
            .debug_projection
            .variables
            .iter()
            .any(|variable| &variable.session_id == session_id)
        || snapshot
            .debug_projection
            .watches
            .iter()
            .any(|watch| &watch.session_id == session_id)
        || snapshot
            .debug_projection
            .console
            .iter()
            .any(|entry| &entry.session_id == session_id)
        || snapshot
            .debug_projection
            .inline_values
            .iter()
            .any(|inline_value| &inline_value.session_id == session_id)
}

fn assisted_ai_projection_references_run(
    snapshot: &ShellProjectionSnapshot,
    run_id: &AgentRunId,
) -> bool {
    let needle = run_id.0.trim();
    if needle.is_empty() {
        return false;
    }

    projected_assisted_run_id(snapshot).is_some_and(|projected| projected == needle)
}

fn collaboration_session_is_known(
    snapshot: &ShellProjectionSnapshot,
    session_id: CollaborationSessionId,
) -> bool {
    snapshot
        .collaboration_gui_projection
        .session_rows
        .iter()
        .any(|row| row.session_id == session_id)
        || snapshot
            .collaboration_presence_projections
            .iter()
            .any(|projection| projection.session_id == session_id)
}

fn shared_collaboration_proposal_is_known(
    snapshot: &ShellProjectionSnapshot,
    session_id: CollaborationSessionId,
    proposal_id: ProposalId,
) -> bool {
    snapshot
        .collaboration_gui_projection
        .shared_proposal_rows
        .iter()
        .any(|row| row.session_id == session_id && row.proposal_id == proposal_id)
}

fn remote_session_is_known(
    snapshot: &ShellProjectionSnapshot,
    session_id: RemoteWorkspaceSessionId,
) -> bool {
    snapshot
        .remote_gui_projection
        .session_rows
        .iter()
        .any(|row| row.session_id == session_id)
}

fn remote_proposal_is_known(
    snapshot: &ShellProjectionSnapshot,
    session_id: RemoteWorkspaceSessionId,
    proposal_id: ProposalId,
) -> bool {
    snapshot
        .remote_gui_projection
        .proposal_review_rows
        .iter()
        .any(|row| row.session_id == session_id && row.proposal_id == proposal_id)
}

fn delegated_plan_is_known(
    snapshot: &ShellProjectionSnapshot,
    plan_id: &DelegatedTaskPlanId,
) -> bool {
    snapshot
        .delegated_task_projection
        .plan_rows
        .iter()
        .any(|row| &row.plan_id == plan_id)
}

fn delegated_proposal_preview_is_known(
    snapshot: &ShellProjectionSnapshot,
    proposal_id: ProposalId,
) -> bool {
    snapshot
        .delegated_task_projection
        .proposal_preview_links
        .iter()
        .any(|link| link.proposal_id == proposal_id)
        || snapshot
            .delegated_task_projection
            .step_summaries
            .iter()
            .any(|step| step.proposal_id == Some(proposal_id))
}

fn delegated_proposal_hunk_is_known(
    snapshot: &ShellProjectionSnapshot,
    proposal_id: ProposalId,
    hunk_id: &str,
) -> bool {
    snapshot
        .delegated_task_projection
        .proposal_reviews
        .iter()
        .any(|review| {
            review.proposal_id == proposal_id
                && review.hunks.iter().any(|hunk| hunk.hunk_id == hunk_id)
        })
}

fn delegated_tool_permission_is_known(
    snapshot: &ShellProjectionSnapshot,
    request_id: &str,
) -> bool {
    snapshot
        .delegated_task_projection
        .tool_permission_requests
        .iter()
        .any(|request| request.request_id == request_id)
}

fn legion_workflow_session_is_known(
    snapshot: &ShellProjectionSnapshot,
    session_id: &LegionWorkflowSessionId,
) -> bool {
    !session_id.0.trim().is_empty()
        && snapshot
            .legion_workflow_projection
            .rows
            .iter()
            .any(|row| &row.session_id == session_id)
}

fn legion_workflow_proposal_is_known(
    snapshot: &ShellProjectionSnapshot,
    session_id: &LegionWorkflowSessionId,
    proposal_id: ProposalId,
) -> bool {
    snapshot
        .legion_workflow_projection
        .rows
        .iter()
        .any(|row| &row.session_id == session_id && row.linked_proposals.contains(&proposal_id))
}

fn legion_workflow_metadata_label_is_known(
    snapshot: &ShellProjectionSnapshot,
    session_id: &LegionWorkflowSessionId,
    metadata_id: &str,
) -> bool {
    if metadata_id.trim().is_empty() {
        return false;
    }
    snapshot
        .legion_workflow_projection
        .rows
        .iter()
        .find(|row| &row.session_id == session_id)
        .is_some_and(|row| {
            row.display_safe_labels
                .iter()
                .any(|label| label == metadata_id)
                || row
                    .merge_readiness
                    .labels
                    .iter()
                    .any(|label| label == metadata_id)
        })
}

fn legion_workflow_mcp_server_is_known(
    snapshot: &ShellProjectionSnapshot,
    server_id: &legion_protocol::McpServerId,
) -> bool {
    !server_id.0.trim().is_empty()
        && snapshot
            .legion_workflow_projection
            .mcp_registries
            .iter()
            .any(|registry| registry.server.server_id == *server_id)
}

fn legion_workflow_mcp_tool_is_known(
    snapshot: &ShellProjectionSnapshot,
    server_id: &legion_protocol::McpServerId,
    tool_name: &legion_protocol::McpToolName,
) -> bool {
    !tool_name.0.trim().is_empty()
        && snapshot
            .legion_workflow_projection
            .mcp_registries
            .iter()
            .any(|registry| {
                registry.server.server_id == *server_id
                    && registry.tools.iter().any(|tool| tool.name == *tool_name)
            })
}

fn projected_assisted_run_id(snapshot: &ShellProjectionSnapshot) -> Option<&str> {
    let projection_id = snapshot.assisted_ai_projection.projection_id.as_str();
    let run_index = projection_id.rfind("phase4-run-")?;
    Some(&projection_id[run_index..])
}

fn active_assist_prediction_id(snapshot: &ShellProjectionSnapshot) -> Option<String> {
    snapshot
        .assist_inline_prediction_projection
        .active_prediction
        .as_ref()
        .map(|prediction| prediction.prediction_id.clone())
}
