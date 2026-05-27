//! Metadata-only desktop diagnostics export.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::{health::DesktopOperationalHealthSnapshot, platform::DesktopPlatformSmokeSnapshot};

/// Diagnostics export write errors.
#[derive(Debug, thiserror::Error)]
pub enum DesktopDiagnosticsError {
    /// Diagnostics file IO failed.
    #[error("diagnostics IO failed for {path}: {source}")]
    Io {
        /// Diagnostics path.
        path: PathBuf,
        /// Source IO error.
        source: io::Error,
    },
}

/// Metadata-only desktop diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopDiagnosticsExport {
    /// Crate version of the desktop adapter.
    pub app_version: String,
    /// Display-only workspace path.
    pub workspace: String,
    /// Count of open tabs in the app-owned projection.
    pub open_tab_count: usize,
    /// Count of dirty tabs in the app-owned projection.
    pub dirty_tab_count: usize,
    /// Count of projected status messages.
    pub status_message_count: usize,
    /// Whether a session-state export path is configured.
    pub session_state_configured: bool,
    /// Last workflow outcome label.
    pub last_outcome: String,
    /// Metadata-only operational health summary.
    pub health: DesktopOperationalHealthSnapshot,
    /// Metadata-only platform smoke snapshot.
    pub platform: DesktopPlatformSmokeSnapshot,
}

impl DesktopDiagnosticsExport {
    /// Render diagnostics as stable markdown evidence.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let markdown = format!(
            concat!(
                "# Desktop Diagnostics Export\n\n",
                "## Runtime\n\n",
                "app_version: {app_version}\n",
                "workspace: {workspace}\n",
                "open_tab_count: {open_tab_count}\n",
                "dirty_tab_count: {dirty_tab_count}\n",
                "status_message_count: {status_message_count}\n",
                "session_state_configured: {session_state_configured}\n",
                "last_outcome: {last_outcome}\n\n",
                "## Operational Health\n\n",
                "{health}\n",
                "## Platform\n\n",
                "menu_smoke: {menu_smoke}\n",
                "shortcut_smoke: {shortcut_smoke}\n",
                "clipboard_smoke: {clipboard_smoke}\n",
                "ime_smoke: {ime_smoke}\n",
                "file_dialog_smoke: {file_dialog_smoke}\n",
                "theme_smoke: {theme_smoke}\n",
                "high_dpi_smoke: {high_dpi_smoke}\n",
                "focus_traversal_smoke: {focus_traversal_smoke}\n",
                "accessibility_tree_smoke: {accessibility_tree_smoke}\n",
                "accessibility_projection_node_count: {accessibility_projection_node_count}\n\n",
                "## Privacy\n\n",
                "- Diagnostics include metadata, counts, status categories, and adapter smoke labels only.\n",
                "- Diagnostics do not include editor text, bounded previews, source bodies, secrets, or status-message bodies.\n"
            ),
            app_version = self.app_version,
            workspace = self.workspace,
            open_tab_count = self.open_tab_count,
            dirty_tab_count = self.dirty_tab_count,
            status_message_count = self.status_message_count,
            session_state_configured = self.session_state_configured,
            last_outcome = self.last_outcome,
            health = self.health.to_markdown(),
            menu_smoke = self.platform.menu_smoke,
            shortcut_smoke = self.platform.shortcut_smoke,
            clipboard_smoke = self.platform.clipboard_smoke,
            ime_smoke = self.platform.ime_smoke,
            file_dialog_smoke = self.platform.file_dialog_smoke,
            theme_smoke = self.platform.theme_smoke,
            high_dpi_smoke = self.platform.high_dpi_smoke,
            focus_traversal_smoke = self.platform.focus_traversal_smoke,
            accessibility_tree_smoke = self.platform.accessibility_tree_smoke,
            accessibility_projection_node_count = self.platform.accessibility_projection_node_count,
        );
        markdown
    }

    /// Write diagnostics markdown to disk.
    pub fn write_to_path(&self, path: impl AsRef<Path>) -> Result<(), DesktopDiagnosticsError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|source| DesktopDiagnosticsError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(path, self.to_markdown()).map_err(|source| DesktopDiagnosticsError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}
