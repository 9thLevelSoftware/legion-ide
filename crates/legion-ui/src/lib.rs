//! UI Shell: rendering adapters, input translation, panels, editor surface.

#![warn(missing_docs)]

pub mod ui;

pub use ui::{
    ActiveBufferProjection, AssistInlinePredictionProjection, AssistInlinePredictionRowProjection,
    AssistInlinePredictionStatusProjection, CommandDispatchIntent, DebugBreakpointProjection,
    DebugConfigurationProjection, DebugConsoleProjection, DebugInlineValueProjection,
    DebugProjection, DebugStackFrameProjection, DebugStatusKindProjection, DebugStatusProjection,
    DebugStepKindProjection, DebugVariableProjection, DebugWatchProjection, DockLayout, DockMode,
    DockPanel, DockPanelDescriptor, DockPanelStateError, DockSide, DockSideLayout,
    ExplorerNodeProjection, ExplorerProjection, ExplorerSelectionProjection,
    GitBlameLineProjection, GitCommitProjection, GitConflictChoiceProjection,
    GitConflictProjection, GitDiffStrategyProjection, GitFileProjection, GitHunkProjection,
    GitHunkStageProjection, GitProjection, Layout, PaletteMode, PaletteProjection, PaletteResult,
    PaletteResultKind, PanelCapability, PanelId, PanelRegistry, RenderMode, SearchProjection,
    SearchResultProjection, SearchScopeProjection, SearchStatusKindProjection,
    SearchStatusProjection, Shell, ShellCommandError, ShellLayoutProjection,
    ShellProjectionSnapshot, StatusMessageProjection, StatusSeverity,
    StructuralSearchCaptureProjection, StructuralSearchMatchProjection, StructuralSearchProjection,
    TOAST_VISIBLE_LIMIT, ToastActionProjection, ToastProjection, ToastStackProjection,
};
