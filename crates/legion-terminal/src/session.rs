//! Per-session metadata stored by the terminal runtime.

use crate::osc::{TerminalShellBoundary, TerminalShellProjection};

/// Metadata tracked for a live terminal session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalSessionMetadata {
    /// Latest cwd reported by OSC 7.
    pub cwd: Option<String>,
    /// Latest exit code reported by OSC 133.
    pub exit_code: Option<i32>,
    /// Latest command boundary marker reported by OSC 133.
    pub boundary: Option<TerminalShellBoundary>,
}

impl TerminalSessionMetadata {
    /// Merge the latest shell projection into the session state.
    pub fn apply_shell_projection(&mut self, projection: &TerminalShellProjection) {
        if projection.cwd.is_some() {
            self.cwd = projection.cwd.clone();
        }
        if projection.exit_code.is_some() {
            self.exit_code = projection.exit_code;
        }
        if projection.boundary.is_some() {
            self.boundary = projection.boundary;
        }
    }
}
