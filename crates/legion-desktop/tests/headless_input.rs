//! Headless input harness for the desktop runtime.
//!
//! This test exercises the real `DesktopEframeApp` / `egui::Context` path used
//! by the desktop adapter without requiring a real `winit` window. The harness
//! feeds a synthetic `egui::RawInput` event stream through the same keyboard
//! handler that production uses and asserts that the projection snapshot —
//! which is the only place the renderer may read app-owned state — changes in
//! response.
//!
//! The invariant we are protecting:
//! * `legion-ui` stays projection-only.
//! * `legion-desktop` is a renderer/adapter; it may own adapter-local view
//!   state but never workspace state.
//! * `legion-app` owns app authority and is the only writer of the buffer
//!   state that the projection reflects.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};
use legion_ui::{DockMode, PaletteMode, SearchScopeProjection, SearchStatusKindProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn headless_input_test_guard() -> std::sync::MutexGuard<'static, ()> {
    // Recover from a poisoned lock so a panic in one test does not cascade
    // into spurious failures across every other serialized test.
    match TEST_LOCK.get_or_init(|| Mutex::new(())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!(
            "legion_desktop_headless_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
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
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_headless_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

fn command_key_input(key: egui::Key) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        modifiers: egui::Modifiers {
            command: true,
            ..egui::Modifiers::default()
        },
        events: vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                ..egui::Modifiers::default()
            },
        }],
        ..egui::RawInput::default()
    }
}

fn command_alt_key_input(key: egui::Key) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        modifiers: egui::Modifiers {
            command: true,
            alt: true,
            ..egui::Modifiers::default()
        },
        events: vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                alt: true,
                ..egui::Modifiers::default()
            },
        }],
        ..egui::RawInput::default()
    }
}

fn text_input(text: &str) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        events: vec![egui::Event::Text(text.to_string())],
        ..egui::RawInput::default()
    }
}

fn enter_input() -> egui::RawInput {
    egui::RawInput {
        focused: true,
        events: vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
        ..egui::RawInput::default()
    }
}

fn open_file_via_palette(app: &mut DesktopEframeApp, relative_path: &str) {
    let _ = app.run_headless_input(command_key_input(egui::Key::O));
    assert!(
        app.runtime_snapshot().palette_projection.open,
        "Cmd+O should open the file palette"
    );

    let _ = app.run_headless_input(text_input(relative_path));
    assert_eq!(
        app.runtime_snapshot().palette_projection.query,
        relative_path,
        "typing into the palette should update the projected query"
    );

    let _ = app.run_headless_input(enter_input());
}

fn open_search_via_palette(app: &mut DesktopEframeApp, query: &str) {
    let _ = app.run_headless_input(command_key_input(egui::Key::F));
    assert!(
        app.runtime_snapshot().palette_projection.open,
        "Cmd+F should open the search palette"
    );
    assert_eq!(
        app.runtime_snapshot().palette_projection.mode,
        PaletteMode::Search,
        "Cmd+F should open the search palette in search mode"
    );
    assert_eq!(
        app.runtime_snapshot().palette_projection.scope,
        SearchScopeProjection::ActiveFile,
        "Cmd+F should default to active-file search scope"
    );

    let _ = app.run_headless_input(text_input(query));
    assert_eq!(
        app.runtime_snapshot().palette_projection.query,
        format!("/{}", query),
        "typing into the search palette should update the projected query body"
    );

    let _ = app.run_headless_input(enter_input());
}

#[test]
fn headless_input_cmd_f_runs_search_through_real_egui_context() {
    let _guard = headless_input_test_guard();
    let workspace = TempWorkspace::new();
    let file_path = workspace.write("search_me.txt", "needle\nother needle\n");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    open_file_via_palette(&mut app, "search_me.txt");
    assert!(
        app.runtime_snapshot()
            .active_buffer_projection
            .file_path
            .as_ref()
            .is_some_and(|path| path.0.ends_with("search_me.txt")),
        "the palette should open the file before search runs"
    );
    assert_eq!(
        fs::read_to_string(&file_path).expect("seed file should remain readable"),
        "needle\nother needle\n"
    );

    open_search_via_palette(&mut app, "needle");

    let snapshot = app.runtime_snapshot();
    assert!(!snapshot.palette_projection.open);
    assert_eq!(
        snapshot.search_projection.status.kind,
        SearchStatusKindProjection::Completed,
        "Cmd+F search selection should dispatch through app authority"
    );
    assert_eq!(
        snapshot.search_projection.results.len(),
        2,
        "the active-file search should return both matching rows"
    );
}

#[test]
fn headless_input_cmd_alt_m_switches_product_mode_through_real_egui_context() {
    let _guard = headless_input_test_guard();
    let workspace = TempWorkspace::new();
    let file_path = workspace.write("mode_switch.txt", "mode stay untouched\n");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    app.handle_action(DesktopAction::SetProductMode {
        mode: DockMode::Assist,
    })
    .expect("setup mode should switch to Assist");
    assert_eq!(app.runtime_snapshot().product_mode, DockMode::Assist);

    let _ = app.run_headless_input(command_alt_key_input(egui::Key::M));

    assert_eq!(
        app.runtime_snapshot().product_mode,
        DockMode::Manual,
        "Cmd+Alt+M should switch the projection back to Manual"
    );
    assert_eq!(
        fs::read_to_string(&file_path).expect("mode switch file should remain readable"),
        "mode stay untouched\n",
        "this limited regression only verifies the projection-level mode transition"
    );
}

#[test]
fn headless_input_cmd_o_opens_workspace_file_through_real_egui_context() {
    let _guard = headless_input_test_guard();
    let workspace = TempWorkspace::new();

    let file_path = workspace.write("open_me.txt", "open me\n");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    assert!(
        app.runtime_snapshot()
            .active_buffer_projection
            .buffer_id
            .is_none()
    );

    open_file_via_palette(&mut app, "open_me.txt");

    let snapshot = app.runtime_snapshot();
    assert!(!snapshot.palette_projection.open);
    assert!(
        snapshot
            .active_buffer_projection
            .file_path
            .as_ref()
            .is_some_and(|path| path.0.ends_with("open_me.txt")),
        "Cmd+O should open the selected workspace file through app authority"
    );
    assert_eq!(
        fs::read_to_string(&file_path).expect("opened file should remain readable"),
        "open me\n",
        "opening a file must not mutate disk contents"
    );
}

#[test]
fn headless_input_text_event_marks_open_file_dirty() {
    let _guard = headless_input_test_guard();
    let workspace = TempWorkspace::new();
    let file_path = workspace.write("edit_me.txt", "alpha\n");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    open_file_via_palette(&mut app, "edit_me.txt");

    assert!(!app.runtime_snapshot().active_buffer_projection.dirty);
    assert_eq!(
        fs::read_to_string(&file_path).expect("seed file should be readable"),
        "alpha\n",
        "opening a file must not mutate disk contents"
    );

    let _ = app.run_headless_input(text_input("!"));

    assert!(
        app.runtime_snapshot().active_buffer_projection.dirty,
        "typing through the headless harness should mark the projected buffer dirty"
    );
    assert_eq!(
        fs::read_to_string(&file_path).expect("seed file should still be readable"),
        "alpha\n",
        "typing through the harness must not write to disk until save runs"
    );
}

#[test]
fn headless_input_cmd_s_saves_dirty_file_through_real_egui_context() {
    let _guard = headless_input_test_guard();
    let workspace = TempWorkspace::new();
    let file_path = workspace.write("save_me.txt", "beta\n");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    open_file_via_palette(&mut app, "save_me.txt");
    let _ = app.run_headless_input(text_input("!"));

    assert!(
        app.runtime_snapshot().active_buffer_projection.dirty,
        "editing before save should mark the projection dirty"
    );

    let _ = app.run_headless_input(command_key_input(egui::Key::S));

    let saved = fs::read_to_string(&file_path).expect("saved file should be readable");
    assert_eq!(
        saved, "!beta\n",
        "Cmd+S should persist the edited buffer through the real save path"
    );
    assert!(
        !app.runtime_snapshot().active_buffer_projection.dirty,
        "saving through the harness should clear the dirty projection"
    );
}
