//! Metadata-only platform and accessibility smoke projection.

use devil_protocol::TextCoordinate;
use devil_ui::ShellProjectionSnapshot;

use crate::bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge};

const ADAPTER_PATH_PASSED: &str = "adapter-path passed";
const NOT_OBSERVED: &str = "not observed";

/// Adapter command paths that were exercised without OS payload capture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DesktopPlatformAdapterChecks {
    /// Clipboard command path translated into an app-owned intent.
    pub clipboard_adapter_path: Option<bool>,
    /// IME commit command path translated into an app-owned intent.
    pub ime_adapter_path: Option<bool>,
    /// File-dialog selection path translated into an app-owned intent.
    pub file_dialog_adapter_path: Option<bool>,
}

impl DesktopPlatformAdapterChecks {
    /// Build an observed adapter-check result set.
    #[must_use]
    pub const fn observed(
        clipboard_adapter_path: bool,
        ime_adapter_path: bool,
        file_dialog_adapter_path: bool,
    ) -> Self {
        Self {
            clipboard_adapter_path: Some(clipboard_adapter_path),
            ime_adapter_path: Some(ime_adapter_path),
            file_dialog_adapter_path: Some(file_dialog_adapter_path),
        }
    }
}

/// Native window observations captured by the adapter.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativePlatformObservation {
    /// Whether the native viewport reported focus.
    pub focused: Option<bool>,
    /// Native pixels-per-point scale when observed.
    pub pixels_per_point: Option<f32>,
}

/// A projected accessibility node label derived from metadata-only UI state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopAccessibilityNode {
    /// Stable role label for the projected node.
    pub role: String,
    /// Metadata-only display label.
    pub label: String,
}

/// Metadata-only platform smoke snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopPlatformSmokeSnapshot {
    /// Menu surface smoke status.
    pub menu_smoke: String,
    /// Keyboard shortcut smoke status.
    pub shortcut_smoke: String,
    /// Clipboard adapter smoke status.
    pub clipboard_smoke: String,
    /// IME adapter smoke status.
    pub ime_smoke: String,
    /// File-dialog adapter smoke status.
    pub file_dialog_smoke: String,
    /// Theme smoke status.
    pub theme_smoke: String,
    /// High-DPI smoke status.
    pub high_dpi_smoke: String,
    /// Focus traversal smoke status.
    pub focus_traversal_smoke: String,
    /// Accessibility tree smoke status.
    pub accessibility_tree_smoke: String,
    /// Count of metadata-only projected accessibility nodes.
    pub accessibility_projection_node_count: usize,
    /// Metadata-only projected accessibility labels.
    pub accessibility_nodes: Vec<DesktopAccessibilityNode>,
}

impl Default for DesktopPlatformSmokeSnapshot {
    fn default() -> Self {
        Self {
            menu_smoke: NOT_OBSERVED.to_string(),
            shortcut_smoke: NOT_OBSERVED.to_string(),
            clipboard_smoke: NOT_OBSERVED.to_string(),
            ime_smoke: NOT_OBSERVED.to_string(),
            file_dialog_smoke: NOT_OBSERVED.to_string(),
            theme_smoke: NOT_OBSERVED.to_string(),
            high_dpi_smoke: NOT_OBSERVED.to_string(),
            focus_traversal_smoke: NOT_OBSERVED.to_string(),
            accessibility_tree_smoke: NOT_OBSERVED.to_string(),
            accessibility_projection_node_count: 0,
            accessibility_nodes: Vec::new(),
        }
    }
}

/// Builds metadata-only platform smoke state from the current projection.
///
/// The snapshot intentionally records adapter/projection coverage separately
/// from OS-observed accessibility status. It must not capture editor text or
/// diagnostics payloads.
#[must_use]
pub fn build_platform_smoke_snapshot(
    snapshot: &ShellProjectionSnapshot,
    adapter_checks: DesktopPlatformAdapterChecks,
    native: NativePlatformObservation,
) -> DesktopPlatformSmokeSnapshot {
    let accessibility_nodes = accessibility_nodes(snapshot);
    let node_count = accessibility_nodes.len();

    DesktopPlatformSmokeSnapshot {
        menu_smoke: menu_status(snapshot),
        shortcut_smoke: shortcut_status(snapshot),
        clipboard_smoke: adapter_status(adapter_checks.clipboard_adapter_path),
        ime_smoke: adapter_status(adapter_checks.ime_adapter_path),
        file_dialog_smoke: adapter_status(adapter_checks.file_dialog_adapter_path),
        theme_smoke: "adapter theme defaults available".to_string(),
        high_dpi_smoke: high_dpi_status(native.pixels_per_point),
        focus_traversal_smoke: focus_traversal_status(node_count, native.focused),
        accessibility_tree_smoke: accessibility_tree_status(node_count),
        accessibility_projection_node_count: node_count,
        accessibility_nodes,
    }
}

/// Exercise adapter-local platform command paths against the current projection.
#[must_use]
pub fn build_platform_adapter_checks(
    snapshot: &ShellProjectionSnapshot,
) -> DesktopPlatformAdapterChecks {
    let bridge = DesktopCommandBridge::new();
    let at = projected_cursor(snapshot);
    DesktopPlatformAdapterChecks::observed(
        matches!(
            bridge.translate(
                DesktopAction::ClipboardPaste {
                    text: "clipboard-smoke".to_string(),
                    at,
                },
                snapshot,
            ),
            DesktopBridgeOutput::Intent(_)
        ),
        matches!(
            bridge.translate(
                DesktopAction::ImeCommit {
                    text: "ime-smoke".to_string(),
                    at,
                },
                snapshot,
            ),
            DesktopBridgeOutput::Intent(_)
        ),
        matches!(
            bridge.translate(
                DesktopAction::OpenPathDialogSelected("Cargo.toml".to_string()),
                snapshot,
            ),
            DesktopBridgeOutput::Intent(_)
        ),
    )
}

fn menu_status(snapshot: &ShellProjectionSnapshot) -> String {
    if snapshot.layout_projection.layout.title.trim().is_empty() {
        NOT_OBSERVED.to_string()
    } else {
        "projection command surface present".to_string()
    }
}

fn shortcut_status(snapshot: &ShellProjectionSnapshot) -> String {
    if snapshot.active_buffer_projection.buffer_id.is_some()
        || !snapshot.daily_editing_projection.tabs.tabs.is_empty()
    {
        "adapter shortcut targets projected".to_string()
    } else {
        "global adapter shortcuts available".to_string()
    }
}

fn adapter_status(passed: Option<bool>) -> String {
    match passed {
        Some(true) => ADAPTER_PATH_PASSED.to_string(),
        Some(false) => "failed".to_string(),
        None => NOT_OBSERVED.to_string(),
    }
}

fn high_dpi_status(pixels_per_point: Option<f32>) -> String {
    match pixels_per_point {
        Some(scale) if scale > 1.0 => format!("os-observed scale {scale:.3}"),
        Some(_) | None => NOT_OBSERVED.to_string(),
    }
}

fn focus_traversal_status(node_count: usize, focused: Option<bool>) -> String {
    match (node_count, focused) {
        (0, _) => NOT_OBSERVED.to_string(),
        (_, Some(true)) => {
            format!("projection focus traversal nodes {node_count}; viewport focused")
        }
        (_, Some(false)) => {
            format!("projection focus traversal nodes {node_count}; viewport not focused")
        }
        (_, None) => format!("projection focus traversal nodes {node_count}; focus not observed"),
    }
}

fn accessibility_tree_status(node_count: usize) -> String {
    if node_count == 0 {
        NOT_OBSERVED.to_string()
    } else {
        format!("metadata-only projection accessibility nodes {node_count}; OS tree not observed")
    }
}

fn accessibility_nodes(snapshot: &ShellProjectionSnapshot) -> Vec<DesktopAccessibilityNode> {
    let mut nodes = Vec::new();
    push_node(
        &mut nodes,
        "window",
        sanitize_label(&snapshot.layout_projection.layout.title),
    );

    if !snapshot.explorer_projection.nodes.is_empty() {
        push_node(
            &mut nodes,
            "explorer",
            format!(
                "{} workspace nodes",
                snapshot.explorer_projection.nodes.len()
            ),
        );
    }

    if let Some(path) = &snapshot.active_buffer_projection.file_path {
        push_node(&mut nodes, "editor", sanitize_label(&path.0));
    } else if snapshot.active_buffer_projection.buffer_id.is_some() {
        push_node(&mut nodes, "editor", "active buffer".to_string());
    }

    if !snapshot.daily_editing_projection.tabs.tabs.is_empty() {
        push_node(
            &mut nodes,
            "tabs",
            format!(
                "{} open tabs",
                snapshot.daily_editing_projection.tabs.tabs.len()
            ),
        );
    }

    if !snapshot.status_messages.is_empty() {
        push_node(
            &mut nodes,
            "status",
            format!("{} status messages", snapshot.status_messages.len()),
        );
    }

    if !snapshot.search_projection.results.is_empty() {
        push_node(
            &mut nodes,
            "search",
            format!(
                "{} bounded results",
                snapshot.search_projection.results.len()
            ),
        );
    }

    nodes
}

fn push_node(nodes: &mut Vec<DesktopAccessibilityNode>, role: &str, label: String) {
    if label.trim().is_empty() {
        return;
    }

    nodes.push(DesktopAccessibilityNode {
        role: role.to_string(),
        label,
    });
}

fn sanitize_label(label: &str) -> String {
    let label = label.replace(['\r', '\n', '\t'], " ");
    if label.chars().count() <= 120 {
        return label;
    }

    let mut truncated = label.chars().take(117).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn projected_cursor(snapshot: &ShellProjectionSnapshot) -> TextCoordinate {
    snapshot
        .active_buffer_projection
        .viewport
        .as_ref()
        .map(|viewport| viewport.cursor)
        .unwrap_or(TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: Some(0),
            utf16_offset: Some(0),
        })
}
