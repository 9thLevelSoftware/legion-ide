//! Metadata-only platform and accessibility smoke projection.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

use legion_protocol::{ExtensionPermissionState, TextCoordinate};
use legion_ui::ShellProjectionSnapshot;

use crate::{
    bridge::{DesktopAction, DesktopBridgeOutput, DesktopCommandBridge},
    view::extensions_panel::DesktopExtensionsPanelViewModel,
};

const ADAPTER_PATH_PASSED: &str = "adapter-path passed";
const NOT_OBSERVED: &str = "not observed";
const OS_TREE_NOT_OBSERVED: &str = "OS tree not observed";
const WINDOWS_UIA_PROBE_SCRIPT: &str = "scripts/a11y-uia-walk.ps1";
/// Smoke rebuilds this snapshot every frame; the PowerShell walk is not cheap.
const WINDOWS_UIA_PROBE_RETRY: Duration = Duration::from_millis(250);

struct WindowsUiaProbeCache {
    last_attempt: Option<Instant>,
    observation: Option<WindowsUiaProbeObservation>,
    /// Script missing or UIA assemblies unavailable will not change mid-run.
    terminal_miss: bool,
}

static WINDOWS_UIA_PROBE_CACHE: Mutex<WindowsUiaProbeCache> = Mutex::new(WindowsUiaProbeCache {
    last_attempt: None,
    observation: None,
    terminal_miss: false,
});

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
    /// Committed Windows UIA walk when that probe actually succeeded.
    pub os_accessibility_tree: Option<WindowsUiaProbeObservation>,
}

/// Observation produced by `scripts/a11y-uia-walk.ps1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsUiaProbeObservation {
    /// Control-view descendants enumerated under a top-level window.
    pub descendant_count: usize,
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
        accessibility_tree_smoke: accessibility_tree_status(
            node_count,
            native.os_accessibility_tree,
        ),
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
        Some(scale) if scale.is_finite() && scale > 0.0 => format!("os-observed scale {scale:.3}"),
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

fn accessibility_tree_status(
    node_count: usize,
    os_tree: Option<WindowsUiaProbeObservation>,
) -> String {
    if node_count == 0 {
        return NOT_OBSERVED.to_string();
    }

    let os_tree = os_tree.or_else(cached_windows_uia_observation);
    format!(
        "metadata-only projection accessibility nodes {node_count}; {}",
        os_accessibility_tree_clause(os_tree)
    )
}

fn os_accessibility_tree_clause(os_tree: Option<WindowsUiaProbeObservation>) -> String {
    match os_tree {
        Some(observation) => format!(
            "Windows UIA observed {} descendants",
            observation.descendant_count
        ),
        None => OS_TREE_NOT_OBSERVED.to_string(),
    }
}

/// Parse stdout from the committed Windows UI Automation probe.
///
/// The winit event-target pane is a second top-level window with zero
/// descendants; the product window's count is the maximum enumerated line.
#[must_use]
pub fn parse_windows_uia_probe_output(stdout: &str) -> Option<WindowsUiaProbeObservation> {
    let mut saw_ok = false;
    let mut descendant_count = 0_usize;
    for line in stdout.lines() {
        let line = line.trim();
        if line == "UIA_WALK_OK" {
            saw_ok = true;
            continue;
        }
        let Some(rest) = line.strip_prefix("DESCENDANTS_ENUMERATED:") else {
            continue;
        };
        if let Ok(count) = rest.trim().parse::<usize>() {
            descendant_count = descendant_count.max(count);
        }
    }
    saw_ok.then_some(WindowsUiaProbeObservation { descendant_count })
}

/// Locate `scripts/a11y-uia-walk.ps1` from the crate or the process cwd.
#[must_use]
pub fn committed_windows_uia_probe_script() -> Option<PathBuf> {
    windows_uia_probe_script_path()
}

/// Run the committed Windows UIA probe against this process.
///
/// Non-Windows hosts, a missing script, or a walk that did not print
/// `UIA_WALK_OK` stay `None`. This never invents a macOS or Linux probe.
#[must_use]
pub fn probe_windows_uia_tree() -> Option<WindowsUiaProbeObservation> {
    match run_windows_uia_probe_outcome() {
        WindowsUiaProbeOutcome::Observed(observation) => {
            if let Ok(mut cache) = WINDOWS_UIA_PROBE_CACHE.lock() {
                cache.observation = Some(observation);
            }
            Some(observation)
        }
        WindowsUiaProbeOutcome::TerminalMiss => {
            if let Ok(mut cache) = WINDOWS_UIA_PROBE_CACHE.lock() {
                cache.terminal_miss = true;
            }
            None
        }
        WindowsUiaProbeOutcome::RetryableMiss => None,
    }
}

fn cached_windows_uia_observation() -> Option<WindowsUiaProbeObservation> {
    {
        let cache = WINDOWS_UIA_PROBE_CACHE.lock().ok()?;
        if let Some(observation) = cache.observation {
            return Some(observation);
        }
        if cache.terminal_miss {
            return None;
        }
        if cache
            .last_attempt
            .is_some_and(|at| at.elapsed() < WINDOWS_UIA_PROBE_RETRY)
        {
            return None;
        }
    }

    let outcome = run_windows_uia_probe_outcome();
    let mut cache = WINDOWS_UIA_PROBE_CACHE.lock().ok()?;
    cache.last_attempt = Some(Instant::now());
    match outcome {
        WindowsUiaProbeOutcome::Observed(observation) => {
            cache.observation = Some(observation);
            Some(observation)
        }
        WindowsUiaProbeOutcome::TerminalMiss => {
            cache.terminal_miss = true;
            None
        }
        WindowsUiaProbeOutcome::RetryableMiss => None,
    }
}

enum WindowsUiaProbeOutcome {
    #[cfg_attr(not(windows), allow(dead_code))]
    Observed(WindowsUiaProbeObservation),
    #[cfg_attr(not(windows), allow(dead_code))]
    RetryableMiss,
    TerminalMiss,
}

fn run_windows_uia_probe_outcome() -> WindowsUiaProbeOutcome {
    #[cfg(windows)]
    {
        run_windows_uia_probe_outcome_windows()
    }
    #[cfg(not(windows))]
    {
        WindowsUiaProbeOutcome::TerminalMiss
    }
}

fn windows_uia_probe_script_path() -> Option<PathBuf> {
    // Resolve only from the shipped tree next to this crate. Walking cwd would
    // let an untrusted workspace supply `scripts/a11y-uia-walk.ps1` that then
    // runs under PowerShell `-ExecutionPolicy Bypass`.
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(WINDOWS_UIA_PROBE_SCRIPT);
    let canonical = shipped.canonicalize().ok()?;
    if !canonical.is_file() {
        return None;
    }
    if canonical.file_name()?.to_str()? != "a11y-uia-walk.ps1" {
        return None;
    }
    Some(canonical)
}

#[cfg(windows)]
fn run_windows_uia_probe_outcome_windows() -> WindowsUiaProbeOutcome {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let Some(script) = windows_uia_probe_script_path() else {
        return WindowsUiaProbeOutcome::TerminalMiss;
    };
    let Some(proc_name) = current_process_name() else {
        return WindowsUiaProbeOutcome::RetryableMiss;
    };

    let output = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg("-ProcName")
        .arg(&proc_name)
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let output = match output {
        Ok(output) => output,
        Err(_) => return WindowsUiaProbeOutcome::TerminalMiss,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("UIA_LOAD_FAILED") {
        return WindowsUiaProbeOutcome::TerminalMiss;
    }
    match parse_windows_uia_probe_output(&stdout) {
        Some(observation) => WindowsUiaProbeOutcome::Observed(observation),
        None => WindowsUiaProbeOutcome::RetryableMiss,
    }
}

#[cfg(windows)]
fn current_process_name() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    exe.file_stem()?.to_str().map(str::to_string)
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

    // The extensions panel is where a user grants capabilities to third-party
    // code, and it projected no accessibility node at all -- so a screen-reader
    // user got no announcement of the one surface in the shell that asks for
    // consent. The label carries counts and the pending-decision count, since
    // an extension awaiting review is the state worth hearing first.
    let extensions = DesktopExtensionsPanelViewModel::from_snapshot(snapshot);
    if !extensions.is_empty() {
        let undecided = extensions
            .rows
            .iter()
            .flat_map(|row| row.permissions.iter())
            .filter(|permission| permission.state == ExtensionPermissionState::Undecided)
            .count();
        push_node(
            &mut nodes,
            "extensions",
            format!(
                "{} extensions, {undecided} permissions awaiting review",
                extensions.rows.len()
            ),
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
