//! UI Shell: rendering adapters, input translation, panels, editor surface.

#![warn(missing_docs)]

pub mod ui;

pub use ui::{
    ActiveBufferProjection, CommandDispatchIntent, ExplorerNodeProjection, ExplorerProjection,
    ExplorerSelectionProjection, Layout, RenderMode, SearchProjection, SearchResultProjection,
    SearchScopeProjection, SearchStatusKindProjection, SearchStatusProjection, Shell,
    ShellCommandError, ShellLayoutProjection, ShellProjectionSnapshot, StatusMessageProjection,
    StatusSeverity,
};
