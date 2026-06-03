use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use devil_desktop::{
    bridge::DesktopAction,
    session::{DesktopSessionError, DesktopSessionStore},
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use devil_protocol::{CanonicalPath, SessionPanelState, TimestampMillis, WorkspaceSessionRecord};
use devil_ui::{DockLayout, DockMode, DockSide, DockSideLayout, PanelId};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(prefix: &str) -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!("{prefix}_{}_{}_{}", std::process::id(), nanos, id));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("temp file should be written");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        if self.root.starts_with(&temp_root) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path, initial_file: Option<&Path>, session_state: &Path) -> DesktopRuntime {
    DesktopRuntime::open(
        DesktopLaunchConfig::new(
            root.to_path_buf(),
            initial_file.map(|path| path.to_string_lossy().into_owned()),
        )
        .with_session_state(session_state.to_path_buf()),
    )
    .expect("desktop runtime should open")
}

fn tab_titles(runtime: &DesktopRuntime) -> Vec<String> {
    runtime
        .projection_snapshot()
        .daily_editing_projection
        .tabs
        .tabs
        .iter()
        .map(|tab| tab.title.clone())
        .collect()
}

fn panel_state() -> SessionPanelState {
    SessionPanelState {
        bottom_visible: true,
        side_visible: false,
        active_panel: Some("search".to_string()),
        bottom_height_px: Some(240),
        side_width_px: Some(320),
    }
}

#[test]
fn session_restore_saves_metadata_and_restores_tabs_focus_layout_explorer() {
    let workspace = TempWorkspace::new("devil_desktop_session_restore");
    let first = workspace.write("first.txt", "first");
    let second = workspace.write("second.txt", "second");
    let session_state = workspace.path().join("session.json");
    let mut runtime = open_runtime(workspace.path(), Some(&first), &session_state);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::OpenPathText(
                second.to_string_lossy().into_owned()
            ))
            .expect("open second"),
        DesktopWorkflowOutcome::Opened
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::InsertText {
                text: "SECRET_DIRTY_BODY".to_string(),
                at: devil_protocol::TextCoordinate {
                    line: 0,
                    character: 6,
                    byte_offset: Some(6),
                    utf16_offset: Some(6),
                },
            })
            .expect("edit second"),
        DesktopWorkflowOutcome::Edited
    );
    let explorer_path = runtime.projection_snapshot().explorer_projection.nodes[0]
        .canonical_path
        .0
        .clone();
    assert_eq!(
        runtime
            .handle_action(DesktopAction::ToggleExplorerPath {
                path: explorer_path.clone(),
            })
            .expect("toggle explorer"),
        DesktopWorkflowOutcome::ExplorerPathToggled(explorer_path.clone())
    );
    runtime.set_panel_state(panel_state());
    let mut dock_layouts = DockLayout::standard_all_modes();
    let delegate_layout = dock_layouts
        .iter_mut()
        .find(|layout| layout.mode == DockMode::Delegate)
        .expect("delegate layout exists");
    delegate_layout.right = DockSideLayout::new(
        PanelId::ApprovalQueue,
        vec![PanelId::Delegation, PanelId::Context],
        0.75,
        true,
    );
    runtime.set_dock_layouts(dock_layouts);
    runtime
        .save_session_state()
        .expect("explicit session save after panel change");

    let json = fs::read_to_string(&session_state).expect("session json");
    assert!(json.contains("first.txt"));
    assert!(json.contains("second.txt"));
    assert!(json.contains("\"dirty\": true"));
    assert!(json.contains("\"explorer_expansion\""));
    assert!(json.contains("\"panel_state\""));
    assert!(json.contains("\"dock_layouts\""));
    assert!(json.contains("\"approval_queue\""));
    assert!(!json.contains("SECRET_DIRTY_BODY"));
    assert!(!json.contains("small_buffer_preview"));
    assert!(!json.contains("source_body"));

    let restored = open_runtime(workspace.path(), None, &session_state);
    let snapshot = restored.projection_snapshot();
    assert_eq!(tab_titles(&restored), vec!["first.txt", "second.txt"]);
    assert_eq!(
        snapshot.active_buffer_projection.file_path.as_ref(),
        Some(&CanonicalPath(second.to_string_lossy().into_owned()))
    );
    assert_eq!(
        snapshot.active_buffer_projection.small_buffer_text(),
        Some("second")
    );
    assert!(restored.explorer_path_expanded(&explorer_path));
    assert_eq!(
        restored.panel_state().active_panel.as_deref(),
        Some("search")
    );
    assert_eq!(restored.panel_state().bottom_height_px, Some(240));
    let restored_delegate_layout = restored
        .dock_layouts()
        .iter()
        .find(|layout| layout.mode == DockMode::Delegate)
        .expect("delegate layout restored");
    assert_eq!(
        restored_delegate_layout.right.pinned_default,
        PanelId::ApprovalQueue
    );
    assert_eq!(
        restored_delegate_layout.right.custom_toolkit,
        vec![PanelId::Delegation, PanelId::Context]
    );
    assert!(restored_delegate_layout.right.collapsed);
    assert_eq!(
        restored_delegate_layout
            .visible_panel_ids(DockSide::Right, &devil_ui::PanelRegistry::standard())
            .first(),
        Some(&PanelId::ApprovalQueue)
    );
    assert!(snapshot.status_messages.iter().any(|status| {
        status
            .message
            .contains("Session restored: 2 tabs, 0 skipped")
    }));
}

#[test]
fn session_restore_missing_file_reports_skipped_tab() {
    let workspace = TempWorkspace::new("devil_desktop_session_restore_missing");
    let first = workspace.write("first.txt", "first");
    let second = workspace.write("second.txt", "second");
    let session_state = workspace.path().join("session.json");
    let mut runtime = open_runtime(workspace.path(), Some(&first), &session_state);
    assert_eq!(
        runtime
            .handle_action(DesktopAction::OpenPathText(
                second.to_string_lossy().into_owned()
            ))
            .expect("open second"),
        DesktopWorkflowOutcome::Opened
    );
    runtime.save_session_state().expect("save session");
    fs::remove_file(&second).expect("remove restored tab target");

    let restored = open_runtime(workspace.path(), None, &session_state);
    let snapshot = restored.projection_snapshot();
    assert_eq!(tab_titles(&restored), vec!["first.txt"]);
    assert!(snapshot.status_messages.iter().any(|status| {
        status
            .message
            .contains("Session restored: 1 tabs, 1 skipped")
    }));
    assert!(snapshot.status_messages.iter().any(|status| {
        status.message.contains("Session skipped tab") && status.message.contains("path missing")
    }));
}

#[test]
fn session_restore_corrupt_json_returns_typed_error() {
    let workspace = TempWorkspace::new("devil_desktop_session_restore_corrupt");
    let session_state = workspace.path().join("corrupt-session.json");
    fs::write(&session_state, "{").expect("write corrupt json");

    let error = match DesktopRuntime::open(
        DesktopLaunchConfig::new(workspace.path().to_path_buf(), None)
            .with_session_state(session_state),
    ) {
        Ok(_) => panic!("corrupt session should fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("session JSON failed"));
}

#[test]
fn session_restore_store_rejects_raw_source_markers() {
    let workspace = TempWorkspace::new("devil_desktop_session_restore_marker");
    let session_state = workspace.path().join("session.json");
    let mut record = minimal_record(workspace.path());
    record.session_id = "source_body".to_string();

    let error =
        DesktopSessionStore::save(&session_state, &record).expect_err("raw marker rejected");
    assert!(matches!(error, DesktopSessionError::RawSourceMarker(_)));
}

#[test]
fn session_store_save_publishes_validated_temp_and_cleans_intermediates() {
    let workspace = TempWorkspace::new("devil_desktop_session_restore_atomic");
    let session_state = workspace.path().join("session.json");
    let mut first = minimal_record(workspace.path());
    first.session_id = "workspace-session:first".to_string();
    let mut second = minimal_record(workspace.path());
    second.session_id = "workspace-session:second".to_string();

    DesktopSessionStore::save(&session_state, &first).expect("first session save");
    DesktopSessionStore::save(&session_state, &second).expect("second session save");

    let saved = fs::read_to_string(&session_state).expect("session json should exist");
    assert!(saved.contains("workspace-session:second"));
    assert!(!saved.contains("workspace-session:first"));

    let leftovers = fs::read_dir(workspace.path())
        .expect("workspace directory should be readable")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tmp") || name.contains(".bak"))
        .collect::<Vec<_>>();
    assert!(
        leftovers.is_empty(),
        "session temp/backup files should be cleaned: {leftovers:?}"
    );
}

fn minimal_record(root: &Path) -> WorkspaceSessionRecord {
    WorkspaceSessionRecord {
        session_id: "workspace-session:test".to_string(),
        last_workspace: None,
        last_workspace_path: Some(CanonicalPath(root.to_string_lossy().into_owned())),
        open_tabs: Vec::new(),
        active_tab: None,
        active_buffer: None,
        tab_groups: Vec::new(),
        layout_splits: Vec::new(),
        explorer_expansion: Vec::new(),
        panel_state: SessionPanelState {
            bottom_visible: false,
            side_visible: true,
            active_panel: None,
            bottom_height_px: None,
            side_width_px: None,
        },
        dock_layouts: Vec::new(),
        dirty_indicators: Vec::new(),
        saved_at: TimestampMillis::now(),
        schema_version: 1,
    }
}
