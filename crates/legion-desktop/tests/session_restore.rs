use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    session::{DesktopSessionError, DesktopSessionStore},
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{
    CanonicalPath, SessionPanelState, TimestampMillis, WorkbenchSettingsRecord,
    WorkspaceSessionRecord,
};
use legion_ui::{
    DockLayout, DockMode, DockSide, DockSideLayout, PanelId, ThemePreferenceProjection,
};

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

fn canonical_path(path: &Path) -> CanonicalPath {
    CanonicalPath(
        fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .into_owned(),
    )
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
    let workspace = TempWorkspace::new("legion_desktop_session_restore");
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
                at: legion_protocol::TextCoordinate {
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
        Some(&canonical_path(&second))
    );
    assert_eq!(
        snapshot.active_buffer_projection.small_buffer_text(),
        Some("secondSECRET_DIRTY_BODY")
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
            .visible_panel_ids(DockSide::Right, &legion_ui::PanelRegistry::standard())
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
    let workspace = TempWorkspace::new("legion_desktop_session_restore_missing");
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
fn session_restore_persists_workbench_settings_projection() {
    let workspace = TempWorkspace::new("legion_desktop_session_restore_settings");
    let first = workspace.write("first.txt", "first");
    let session_state = workspace.path().join("session.json");
    let mut runtime = open_runtime(workspace.path(), Some(&first), &session_state);

    let outcome = runtime
        .handle_action(DesktopAction::SetThemePreference {
            preference: ThemePreferenceProjection::Light,
        })
        .expect("theme preference should update");
    assert!(matches!(
        outcome,
        DesktopWorkflowOutcome::SettingsUpdated { .. }
    ));
    runtime
        .handle_action(DesktopAction::SetZoomPercent { zoom_percent: 125 })
        .expect("zoom should update");
    runtime
        .handle_action(DesktopAction::SetLineNumbersVisible { visible: false })
        .expect("line number setting should update");
    runtime
        .handle_action(DesktopAction::SetCurrentLineHighlight { enabled: false })
        .expect("current line setting should update");
    runtime
        .handle_action(DesktopAction::SetStickyHeadersVisible { visible: false })
        .expect("sticky header setting should update");
    runtime
        .handle_action(DesktopAction::SetCodeFoldingVisible { visible: false })
        .expect("code folding setting should update");
    runtime
        .handle_action(DesktopAction::SetMinimapVisible { visible: true })
        .expect("minimap setting should update");
    runtime
        .handle_action(DesktopAction::SetWhitespaceGuidesVisible { visible: true })
        .expect("whitespace guides setting should update");
    runtime
        .handle_action(DesktopAction::SetIndentGuidesVisible { visible: true })
        .expect("indent guides setting should update");
    runtime
        .handle_action(DesktopAction::SetSmoothScrollingEnabled { enabled: false })
        .expect("smooth scrolling setting should update");
    runtime
        .handle_action(DesktopAction::SetCrashReportsEnabled { enabled: true })
        .expect("crash reports setting should update");
    runtime.save_session_state().expect("save session");

    let saved = DesktopSessionStore::load(&session_state)
        .expect("saved session should load")
        .expect("saved session should exist");
    assert_eq!(saved.workbench_settings.theme_preference, "light");
    assert_eq!(saved.workbench_settings.zoom_percent, 125);
    assert!(!saved.workbench_settings.line_numbers_visible);
    assert!(!saved.workbench_settings.current_line_highlight);
    assert!(!saved.workbench_settings.sticky_headers_visible);
    assert!(!saved.workbench_settings.code_folding_visible);
    assert!(saved.workbench_settings.minimap_visible);
    assert!(saved.workbench_settings.whitespace_guides_visible);
    assert!(saved.workbench_settings.indent_guides_visible);
    assert!(!saved.workbench_settings.smooth_scrolling_enabled);
    assert!(saved.workbench_settings.telemetry.crash_reports_enabled);
    assert_eq!(
        saved.workbench_settings.telemetry.consent_label,
        "crash-reports"
    );

    let restored = open_runtime(workspace.path(), None, &session_state);
    let settings = restored.projection_snapshot().settings_projection;
    assert_eq!(settings.theme_preference, ThemePreferenceProjection::Light);
    assert_eq!(settings.zoom_percent, 125);
    assert!(!settings.editor.line_numbers_visible);
    assert!(!settings.editor.current_line_highlight);
    assert!(!settings.editor.sticky_headers_visible);
    assert!(!settings.editor.code_folding_visible);
    assert!(settings.editor.minimap_visible);
    assert!(settings.editor.whitespace_guides_visible);
    assert!(settings.editor.indent_guides_visible);
    assert!(!settings.editor.smooth_scrolling_enabled);
    assert!(settings.telemetry.crash_reports_enabled);
    assert_eq!(settings.telemetry.consent_label, "crash-reports");
}

#[test]
fn setup_dismissal_persists_through_the_existing_session_action() {
    let workspace = TempWorkspace::new("legion_desktop_setup_dismissal");
    let session_state = workspace.path().join("session.json");
    let mut runtime = open_runtime(workspace.path(), None, &session_state);

    assert!(!session_state.exists());
    assert_eq!(
        runtime
            .handle_action(DesktopAction::DismissOnboarding)
            .expect("setup dismissal should remain a normal desktop action"),
        DesktopWorkflowOutcome::Noop
    );

    let saved = DesktopSessionStore::load(&session_state)
        .expect("setup dismissal should write a valid session")
        .expect("setup dismissal should persist a session record");
    assert_ne!(saved.schema_version, 0);

    drop(runtime);
    let restored = open_runtime(workspace.path(), None, &session_state);
    let mut restored = DesktopEframeApp::new(restored);
    restored.headless_egui_context().enable_accesskit();
    let output = restored.run_headless_input(egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1_200.0, 900.0),
        )),
        ..egui::RawInput::default()
    });
    assert!(
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .is_some_and(|update| update.nodes.iter().all(|(_id, node)| {
                node.label() != Some("Welcome to Legion")
                    && node.value() != Some("Welcome to Legion")
            })),
        "a restored session should keep Setup dismissed"
    );
}

#[test]
fn setup_persistence_failure_uses_concise_repair_guidance() {
    let workspace = TempWorkspace::new("legion_desktop_setup_persistence_error");
    let blocked_parent = workspace.write("not-a-directory", "blocked");
    let session_state = blocked_parent.join("session.json");
    let mut runtime = open_runtime(workspace.path(), None, &session_state);

    runtime
        .handle_action(DesktopAction::DismissOnboarding)
        .expect("a persistence failure should not crash setup dismissal");

    let snapshot = runtime.projection_snapshot();
    assert!(snapshot.status_messages.iter().any(|status| {
        status.message
            == "Could not save setup progress. Check the session file location and try again."
    }));
    for leaked in [
        "session IO failed",
        "invalid session record",
        "not-a-directory",
    ] {
        assert!(
            snapshot
                .status_messages
                .iter()
                .all(|status| !status.message.contains(leaked)),
            "setup errors must not expose `{leaked}`: {:?}",
            snapshot.status_messages
        );
    }
}

#[test]
fn setup_workspace_persistence_warning_hides_storage_details() {
    let workspace = TempWorkspace::new("legion_desktop_workspace_persistence_error");
    workspace.write(".legion", "blocks the persistence directory");
    let session_state = workspace.path().join("session.json");

    let runtime = open_runtime(workspace.path(), None, &session_state);
    let snapshot = runtime.projection_snapshot();
    assert!(snapshot.status_messages.iter().any(|status| {
        status.message
            == "Could not save workspace state. Make sure the workspace is writable; Legion will keep working in this session."
    }));
    for leaked in [
        "persistence unavailable",
        "continuing in memory",
        "Storage",
        ".legion",
    ] {
        assert!(
            snapshot
                .status_messages
                .iter()
                .all(|status| !status.message.contains(leaked)),
            "workspace persistence errors must not expose `{leaked}`: {:?}",
            snapshot.status_messages
        );
    }
}

#[test]
fn session_restore_corrupt_json_returns_typed_error() {
    let workspace = TempWorkspace::new("legion_desktop_session_restore_corrupt");
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
fn session_restore_store_rejects_raw_source_markers_in_payload_field() {
    let workspace = TempWorkspace::new("legion_desktop_session_restore_marker");
    let session_state = workspace.path().join("session.json");
    let mut record = minimal_record(workspace.path());
    // A raw buffer/source marker that leaks into the only free-form
    // payload-carrying field must be rejected.
    record.memory_snapshot_json = Some(r#"{"small_buffer_preview":"fn leaked() {}"}"#.to_string());

    let error =
        DesktopSessionStore::save(&session_state, &record).expect_err("raw marker rejected");
    assert!(matches!(error, DesktopSessionError::RawSourceMarker(_)));
}

#[test]
fn session_restore_store_allows_marker_like_benign_metadata() {
    // Regression: structured inspection scans only the raw-payload-carrying
    // field, so benign metadata that merely contains a marker-like substring
    // (here a `session_id` and an explorer path) must NOT be rejected.
    let workspace = TempWorkspace::new("legion_desktop_session_restore_benign_marker");
    let session_state = workspace.path().join("session.json");
    let mut record = minimal_record(workspace.path());
    record.session_id = "source_body".to_string();
    record.explorer_expansion = vec![CanonicalPath(
        workspace
            .path()
            .join("small_buffer_preview")
            .to_string_lossy()
            .into_owned(),
    )];
    record.memory_snapshot_json = None;

    DesktopSessionStore::save(&session_state, &record)
        .expect("benign marker-like metadata must not be rejected");

    let loaded = DesktopSessionStore::load(&session_state)
        .expect("benign session should load")
        .expect("benign session should exist");
    assert_eq!(loaded.session_id, "source_body");
}

#[test]
fn session_store_save_publishes_validated_temp_and_cleans_intermediates() {
    let workspace = TempWorkspace::new("legion_desktop_session_restore_atomic");
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
        canvas_nodes: Vec::new(),
        canvas_edges: Vec::new(),
        panel_state: SessionPanelState {
            bottom_visible: false,
            side_visible: true,
            active_panel: None,
            bottom_height_px: None,
            side_width_px: None,
        },
        dock_layouts: Vec::new(),
        workbench_settings: WorkbenchSettingsRecord::default(),
        memory_snapshot_json: None,
        dirty_indicators: Vec::new(),
        saved_at: TimestampMillis::now(),
        schema_version: 1,
    }
}

// --- Default session path -------------------------------------------------
//
// Every test above hands the runtime an explicit `session_state` path. That is
// what let the whole feature pass its tests while doing nothing in the product:
// `DesktopLaunchConfig` defaulted `session_state` to `None`, so
// `save_session_state` returned immediately and a normal launch lost its layout
// on every restart. These tests exercise the path the product actually takes —
// argument parsing — rather than a path a test constructed.

/// The launch config a bare `legion-desktop <workspace>` produces.
fn config_from_args(args: &[&str]) -> DesktopLaunchConfig {
    DesktopLaunchConfig::from_args(args.iter().map(std::ffi::OsString::from))
        .expect("launch config should parse")
}

#[test]
fn an_interactive_launch_persists_its_layout_by_default() {
    let workspace = TempWorkspace::new("legion_desktop_session_default");
    let config = config_from_args(&[workspace.path().to_string_lossy().as_ref()]);

    assert_eq!(
        config.session_state,
        Some(workspace.path().join(".legion").join("session.json")),
        "a normal launch must persist its layout somewhere, or every restart \
         silently discards the open tabs, the active buffer, the explorer \
         expansion and the dock layout"
    );
}

#[test]
fn an_explicit_session_path_still_wins() {
    let workspace = TempWorkspace::new("legion_desktop_session_explicit");
    let chosen = workspace.path().join("chosen.json");
    let config = config_from_args(&[
        workspace.path().to_string_lossy().as_ref(),
        "--session-state",
        chosen.to_string_lossy().as_ref(),
    ]);

    assert_eq!(config.session_state, Some(chosen));
}

#[test]
fn measurement_harnesses_do_not_inherit_the_workspace_session_path() {
    // `--beta-smoke` reads `session_state` directly and substitutes its own
    // default; defaulting before that branch would redirect beta evidence into
    // the workspace. Smoke and perf runs are short-lived and write nothing.
    let workspace = TempWorkspace::new("legion_desktop_session_harness");
    let root = workspace.path().to_string_lossy().into_owned();

    for args in [
        vec![root.as_str(), "--smoke"],
        vec![root.as_str(), "--beta-smoke"],
        vec![root.as_str(), "--manual-perf"],
    ] {
        let config = config_from_args(&args);
        assert_eq!(
            config.session_state, None,
            "{args:?} must not adopt the interactive session path"
        );
    }
}

#[test]
fn a_default_launch_round_trips_open_tabs_across_a_restart() {
    // The end-to-end claim P1.F2.T4 actually makes: restart restores the
    // layout. Driven entirely through the default config — no test-supplied
    // session path anywhere.
    let workspace = TempWorkspace::new("legion_desktop_session_roundtrip");
    let first_file = workspace.write("alpha.txt", "alpha\n");
    workspace.write("beta.txt", "beta\n");

    let session_path = {
        let config = config_from_args(&[workspace.path().to_string_lossy().as_ref()]);
        let expected = config
            .session_state
            .clone()
            .expect("interactive launch should default a session path");
        let mut runtime = DesktopRuntime::open(config).expect("runtime should open");

        runtime
            .handle_action(DesktopAction::OpenPathText(
                first_file.to_string_lossy().into_owned(),
            ))
            .expect("opening a file should succeed");
        runtime
            .save_session_state()
            .expect("saving the session should succeed");
        expected
    };

    assert!(
        session_path.exists(),
        "the default launch should have written {}",
        session_path.display()
    );

    // Second launch: same arguments, nothing carried over in memory.
    let config = config_from_args(&[workspace.path().to_string_lossy().as_ref()]);
    let restored = DesktopRuntime::open(config).expect("runtime should reopen");
    let titles = tab_titles(&restored);
    assert!(
        titles.iter().any(|title| title == "alpha.txt"),
        "restarting must restore the open tabs, got {titles:?}"
    );
}

#[test]
fn an_unchanged_session_is_not_rewritten() {
    // `persist_session_if_configured` runs from the catch-all action arm, so
    // this path is reached by every dispatched action — including each inserted
    // character. `DesktopSessionStore::save` fsyncs and reads back to validate,
    // so an unguarded write here would put a durable round-trip inside the
    // ADR-0048 keypress budget. Proven by file mtime: a second save that
    // changes nothing must not touch the file.
    let workspace = TempWorkspace::new("legion_desktop_session_noop");
    workspace.write("alpha.txt", "alpha\n");
    let config = config_from_args(&[workspace.path().to_string_lossy().as_ref()]);
    let session_path = config
        .session_state
        .clone()
        .expect("interactive launch should default a session path");
    let mut runtime = DesktopRuntime::open(config).expect("runtime should open");

    runtime.save_session_state().expect("first save");
    let first = fs::metadata(&session_path)
        .expect("session file should exist")
        .modified()
        .expect("mtime should be readable");

    // Coarse filesystem timestamps would make an unchanged mtime meaningless
    // if both writes landed inside the same tick, so separate them.
    std::thread::sleep(std::time::Duration::from_millis(50));
    runtime.save_session_state().expect("second save");
    let second = fs::metadata(&session_path)
        .expect("session file should still exist")
        .modified()
        .expect("mtime should be readable");

    assert_eq!(
        first, second,
        "saving an unchanged session must not rewrite the file"
    );
}

#[test]
fn a_changed_session_is_written_again() {
    // The other half: the guard must not be so eager that a real layout change
    // is dropped. Without this, "skip when unchanged" could degrade to "skip".
    let workspace = TempWorkspace::new("legion_desktop_session_changed");
    let alpha = workspace.write("alpha.txt", "alpha\n");
    let config = config_from_args(&[workspace.path().to_string_lossy().as_ref()]);
    let session_path = config
        .session_state
        .clone()
        .expect("interactive launch should default a session path");
    let mut runtime = DesktopRuntime::open(config).expect("runtime should open");

    runtime.save_session_state().expect("first save");
    let before = fs::read_to_string(&session_path).expect("session file should exist");

    runtime
        .handle_action(DesktopAction::OpenPathText(
            alpha.to_string_lossy().into_owned(),
        ))
        .expect("opening a file should succeed");
    runtime.save_session_state().expect("second save");
    let after = fs::read_to_string(&session_path).expect("session file should exist");

    assert_ne!(
        before, after,
        "opening a tab changes what a restart would restore and must be written"
    );
    assert!(
        after.contains("alpha.txt"),
        "the newly opened tab should be in the record, got {after}"
    );
}

// --- Panel sizes actually come back ---------------------------------------
//
// `splitter_fraction` was persisted and reloaded for a long time while no
// renderer read it, so a restart restored the record rather than the layout the
// user had arranged. These tests exercise the reader.

#[test]
fn a_restored_splitter_fraction_sizes_the_panel() {
    use legion_desktop::view::dock_geometry;
    use legion_ui::DockSide;

    let layouts = vec![DockLayout {
        mode: DockMode::Manual,
        left: DockSideLayout::new(PanelId::ProjectExplorer, Vec::new(), 0.4, false),
        right: DockSideLayout::new(PanelId::Assistant, Vec::new(), 0.25, false),
        bottom: DockSideLayout::new(PanelId::Terminal, Vec::new(), 0.3, false),
    }];

    let stored = dock_geometry::stored_fraction(&layouts, DockMode::Manual, DockSide::Left)
        .expect("the layout carries a left fraction");
    assert!((stored - 0.4).abs() < f32::EPSILON, "got {stored}");

    // 40% of a 1600px shell, within the panel's own bounds.
    assert_eq!(
        dock_geometry::size_from_fraction(Some(stored), 1_600.0, 248.0, 120.0, 900.0),
        640.0,
        "a restored fraction must size the panel, not fall back to the default"
    );
}

#[test]
fn a_resized_panel_is_persisted_and_survives_a_restart() {
    use legion_desktop::view::dock_geometry::DockFractions;

    let workspace = TempWorkspace::new("legion_desktop_dock_roundtrip");
    let config = config_from_args(&[workspace.path().to_string_lossy().as_ref()]);
    let session_path = config
        .session_state
        .clone()
        .expect("interactive launch should default a session path");
    let mut runtime = DesktopRuntime::open(config).expect("runtime should open");

    let mode = runtime.projection_snapshot().product_mode;
    let before = runtime
        .dock_layouts()
        .iter()
        .find(|layout| layout.mode == mode)
        .map(|layout| layout.left.splitter_fraction)
        .expect("the active mode should have a layout");

    // Stand in for the user dragging the splitter: the renderer observes a new
    // fraction and hands it back, exactly as `render_app_frame` does.
    let dragged = if before > 0.5 { 0.25 } else { 0.55 };
    runtime.persist_dock_fractions(DockFractions {
        left: Some(dragged),
        right: None,
        bottom: None,
    });

    let stored = runtime
        .dock_layouts()
        .iter()
        .find(|layout| layout.mode == mode)
        .map(|layout| layout.left.splitter_fraction)
        .expect("layout should still exist");
    assert!(
        (stored - dragged).abs() < 0.001,
        "the observed fraction should be stored, got {stored}"
    );

    let on_disk = fs::read_to_string(&session_path)
        .expect("persisting a dock resize should have written the session");
    assert!(
        on_disk.contains("splitter_fraction"),
        "the session record should carry splitter fractions"
    );

    // Restart from the same arguments and confirm the arrangement returns.
    let restored = DesktopRuntime::open(config_from_args(&[workspace
        .path()
        .to_string_lossy()
        .as_ref()]))
    .expect("runtime should reopen");
    let after = restored
        .dock_layouts()
        .iter()
        .find(|layout| layout.mode == mode)
        .map(|layout| layout.left.splitter_fraction)
        .expect("restored layout should exist");
    assert!(
        (after - dragged).abs() < 0.001,
        "restarting must restore the panel arrangement, got {after} not {dragged}"
    );
}

#[test]
fn an_unmoved_splitter_does_not_write_the_session() {
    use legion_desktop::view::dock_geometry::DockFractions;

    // The renderer hands back a fraction every frame. Only a real drag may
    // reach the disk, or the app fsyncs continuously while sitting idle.
    let workspace = TempWorkspace::new("legion_desktop_dock_idle");
    let config = config_from_args(&[workspace.path().to_string_lossy().as_ref()]);
    let session_path = config
        .session_state
        .clone()
        .expect("interactive launch should default a session path");
    let mut runtime = DesktopRuntime::open(config).expect("runtime should open");

    let mode = runtime.projection_snapshot().product_mode;
    let current = runtime
        .dock_layouts()
        .iter()
        .find(|layout| layout.mode == mode)
        .map(|layout| layout.left.splitter_fraction)
        .expect("the active mode should have a layout");

    runtime.persist_dock_fractions(DockFractions {
        // Sub-pixel wobble, not a drag.
        left: Some(current + 0.0001),
        right: None,
        bottom: None,
    });

    assert!(
        !session_path.exists(),
        "an unmoved splitter must not trigger a durable write"
    );
}

#[test]
fn a_panel_that_was_not_rendered_is_not_recorded_as_collapsed() {
    use legion_desktop::view::dock_geometry::DockFractions;

    // Manual mode hides the inspector, and compact layouts drop the side docks
    // entirely. `None` has to mean "not drawn", never "the user dragged it to
    // nothing" — otherwise briefly shrinking the window would overwrite the
    // desktop arrangement.
    let workspace = TempWorkspace::new("legion_desktop_dock_hidden");
    let config = config_from_args(&[workspace.path().to_string_lossy().as_ref()]);
    let mut runtime = DesktopRuntime::open(config).expect("runtime should open");

    let mode = runtime.projection_snapshot().product_mode;
    let before = runtime
        .dock_layouts()
        .iter()
        .find(|layout| layout.mode == mode)
        .map(|layout| layout.right.splitter_fraction)
        .expect("the active mode should have a layout");

    runtime.persist_dock_fractions(DockFractions::default());

    let after = runtime
        .dock_layouts()
        .iter()
        .find(|layout| layout.mode == mode)
        .map(|layout| layout.right.splitter_fraction)
        .expect("layout should still exist");
    assert!(
        (before - after).abs() < f32::EPSILON,
        "an unrendered panel must leave its stored fraction alone"
    );
}
