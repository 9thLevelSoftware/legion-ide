//! UI Shell: rendering adapters, input translation, panels, editor surface.

#![warn(missing_docs)]

pub mod projection;
pub mod ui;

pub use projection::{
    LegionWorkflowBoardColumnKind, LegionWorkflowBoardColumnProjection,
    LegionWorkflowBoardRowProjection, LegionWorkflowFleetCardProjection,
    legion_workflow_board_columns, legion_workflow_fleet_card_projections,
};

pub use ui::{
    ActiveBufferProjection, ActiveBufferProjectionState, AssistInlinePredictionProjection,
    AssistInlinePredictionRowProjection, AssistInlinePredictionStatusProjection,
    CommandDispatchIntent, DebugBreakpointProjection, DebugConfigurationProjection,
    DebugConsoleProjection, DebugInlineValueProjection, DebugProjection, DebugStackFrameProjection,
    DebugStatusKindProjection, DebugStatusProjection, DebugStepKindProjection,
    DebugVariableProjection, DebugWatchProjection, DockLayout, DockMode, DockPanel,
    DockPanelDescriptor, DockPanelStateError, DockSide, DockSideLayout, EditorSettingsProjection,
    ExplorerNodeProjection, ExplorerProjection, ExplorerSelectionProjection,
    GitBlameLineProjection, GitCommitProjection, GitConflictChoiceProjection,
    GitConflictProjection, GitDiffStrategyProjection, GitFileProjection, GitHunkProjection,
    GitHunkStageProjection, GitProjection, GitWorktreeKindProjection, GitWorktreeProjection,
    Layout, PaletteMode, PaletteProjection, PaletteResult, PaletteResultKind, PanelCapability,
    PanelId, PanelRegistry, RenderMode, SearchProjection, SearchResultProjection,
    SearchScopeProjection, SearchStatusKindProjection, SearchStatusProjection, SettingsProjection,
    Shell, ShellCommandError, ShellLayoutProjection, ShellProjectionSnapshot,
    StatusMessageProjection, StatusSeverity, StructuralSearchCaptureProjection,
    StructuralSearchMatchProjection, StructuralSearchProjection, TOAST_VISIBLE_LIMIT,
    ThemePreferenceProjection, ToastActionProjection, ToastProjection, ToastStackProjection,
    ToastVerbosityProjection,
};
