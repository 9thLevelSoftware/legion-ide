//! Renderer-facing terminal panel model for the desktop adapter.
//!
//! This module keeps terminal renderer state projection-only. It consumes the
//! app/protocol terminal panel projection and `legion-terminal` grid helpers,
//! then exposes display labels and copy text for egui without owning terminal
//! runtime, process, or editor state.

use legion_protocol::{EventSequence, TerminalPanelProjection};
use legion_terminal::grid::{TerminalGrid, TerminalGridSelection};

/// Renderer-friendly terminal panel model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPanelRenderModel {
    /// Status display label.
    pub status_label: String,
    /// Active session display label, if any.
    pub active_session_label: Option<String>,
    /// Runtime state display label, if any.
    pub runtime_label: Option<String>,
    /// Scrollback/search summary display label.
    pub scrollback_label: String,
    /// Whether scrollback is truncated.
    pub scrollback_truncated: bool,
    /// Whether terminal search is truncated.
    pub search_truncated: bool,
    /// Renderer grid rows and scrollback metadata.
    pub grid: TerminalGrid,
}

impl TerminalPanelRenderModel {
    /// Build a terminal render model from protocol projection state.
    pub fn from_projection(projection: &TerminalPanelProjection, max_rows: usize) -> Self {
        Self {
            status_label: format!("status={}", projection.status.kind.display_label()),
            active_session_label: projection
                .active_session_id
                .map(|session_id| format!("session={}", session_id.0)),
            runtime_label: projection
                .runtime_state
                .map(|runtime_state| format!("runtime={runtime_state:?}")),
            scrollback_label: format!(
                "visible={} omitted={} matches={}",
                projection.scrollback.visible_row_count,
                projection.scrollback.omitted_row_count,
                projection.search.match_count
            ),
            scrollback_truncated: projection.scrollback.truncated,
            search_truncated: projection.search.truncated,
            grid: TerminalGrid::from_projection(projection, max_rows),
        }
    }

    /// Copy all visible terminal grid payloads using already-redacted text.
    pub fn copy_all_visible(&self) -> Option<String> {
        self.grid.copy_selection(TerminalGridSelection::AllVisible)
    }

    /// Copy one visible terminal row using already-redacted text.
    pub fn copy_row(&self, sequence: EventSequence) -> Option<String> {
        self.grid
            .copy_selection(TerminalGridSelection::Row(sequence))
    }
}
