//! Clicking a file in the explorer must open it.
//!
//! This is the first thing anyone does with an IDE, and until this suite
//! existed nothing asserted it. The explorer row emitted
//! `DesktopAction::SelectExplorerFile`, which the bridge translated to
//! `CommandDispatchIntent::RevealInExplorer`; the app set `active_file_id` and
//! rebuilt the explorer projection. No buffer was ever opened. The tree
//! rendered, the rows highlighted, and the editor stayed empty — which reads
//! to a user as "the app does not work", because from the outside it doesn't.
//!
//! Quick-open (Cmd+P / Cmd+O) did open files, so the capability was present
//! and only the mouse path was missing. That is exactly the kind of gap a
//! headless projection test cannot see and a rendered-UI test can, so these
//! tests drive the real `DesktopRuntime` action path and — for the click
//! itself — the real accessibility tree.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopEframeApp, DesktopLaunchConfig, DesktopRuntime},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "legion_desktop_explorer_activation_{}_{}_{}",
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

    fn mkdir(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::create_dir_all(&path).expect("temp directory should be created");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_explorer_activation_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

/// The explorer node with this display name, from the live projection.
fn explorer_node(runtime: &DesktopRuntime, name: &str) -> legion_ui::ExplorerNodeProjection {
    runtime
        .projection_snapshot()
        .explorer_projection
        .nodes
        .into_iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("explorer should project a node named `{name}`"))
}

fn open_tab_paths(runtime: &DesktopRuntime) -> Vec<String> {
    runtime
        .projection_snapshot()
        .daily_editing_projection
        .tabs
        .tabs
        .iter()
        .filter_map(|tab| tab.file_path.as_ref().map(|path| path.0.clone()))
        .collect()
}

#[test]
fn activating_an_explorer_file_opens_it_in_a_buffer() {
    let workspace = TempWorkspace::new();
    workspace.write("hello.txt", "first line\nsecond line\n");
    let mut runtime = open_runtime(workspace.path());

    let _ = runtime.handle_action(DesktopAction::RefreshExplorer);
    let node = explorer_node(&runtime, "hello.txt");

    assert!(
        open_tab_paths(&runtime).is_empty(),
        "no tab should be open before the explorer is used"
    );

    runtime
        .handle_action(DesktopAction::SelectExplorerFile {
            file_id: node.file_id,
        })
        .expect("activating an explorer file should succeed");

    let snapshot = runtime.projection_snapshot();
    assert!(
        snapshot.active_buffer_projection.buffer_id.is_some(),
        "activating a file in the explorer must open a buffer — without this \
         the tree is a picture of a file system, not a way into the files"
    );
    let tabs = open_tab_paths(&runtime);
    assert_eq!(
        tabs.len(),
        1,
        "activating one explorer file should open exactly one tab, got {tabs:?}"
    );
    assert!(
        tabs[0].ends_with("hello.txt"),
        "the opened tab should be the activated file, got {tabs:?}"
    );
}

#[test]
fn activating_an_explorer_file_also_selects_it() {
    let workspace = TempWorkspace::new();
    workspace.write("selected.txt", "body\n");
    let mut runtime = open_runtime(workspace.path());

    let _ = runtime.handle_action(DesktopAction::RefreshExplorer);
    let node = explorer_node(&runtime, "selected.txt");

    runtime
        .handle_action(DesktopAction::SelectExplorerFile {
            file_id: node.file_id,
        })
        .expect("activating an explorer file should succeed");

    // Opening must not cost the reveal behaviour that was already there: the
    // row the user clicked has to stay visibly selected.
    assert_eq!(
        runtime
            .projection_snapshot()
            .explorer_projection
            .selection
            .map(|selection| selection.file_id),
        Some(node.file_id),
        "the activated row must remain the selected row"
    );
}

#[test]
fn activating_the_same_explorer_file_twice_does_not_open_a_second_tab() {
    let workspace = TempWorkspace::new();
    workspace.write("once.txt", "body\n");
    let mut runtime = open_runtime(workspace.path());

    let _ = runtime.handle_action(DesktopAction::RefreshExplorer);
    let node = explorer_node(&runtime, "once.txt");

    for _ in 0..2 {
        runtime
            .handle_action(DesktopAction::SelectExplorerFile {
                file_id: node.file_id,
            })
            .expect("activating an explorer file should succeed");
    }

    assert_eq!(
        open_tab_paths(&runtime).len(),
        1,
        "re-activating an already-open file should focus its tab, not duplicate it"
    );
}

#[test]
fn activating_a_nested_explorer_file_opens_it() {
    // The root-level case can succeed on a path string that happens to round
    // trip. A nested file exercises the real shape — a canonical absolute path
    // with the platform's separators — which is what every file a person
    // actually opens in this repo looks like.
    let workspace = TempWorkspace::new();
    workspace.mkdir("crates/inner/src");
    workspace.write("crates/inner/src/lib.rs", "pub fn f() {}\n");
    let mut runtime = open_runtime(workspace.path());

    let _ = runtime.handle_action(DesktopAction::RefreshExplorer);
    let node = explorer_node(&runtime, "lib.rs");

    runtime
        .handle_action(DesktopAction::SelectExplorerFile {
            file_id: node.file_id,
        })
        .expect("activating a nested explorer file should succeed");

    let tabs = open_tab_paths(&runtime);
    assert_eq!(
        tabs.len(),
        1,
        "activating a nested file should open exactly one tab, got {tabs:?}"
    );
    assert!(
        tabs[0].ends_with("lib.rs"),
        "the opened tab should be the nested file, got {tabs:?}"
    );
}

#[test]
fn activating_a_directory_row_expands_it_instead_of_opening_a_buffer() {
    let workspace = TempWorkspace::new();
    workspace.mkdir("src");
    workspace.write("src/main.rs", "fn main() {}\n");
    let mut runtime = open_runtime(workspace.path());

    let _ = runtime.handle_action(DesktopAction::RefreshExplorer);
    let node = explorer_node(&runtime, "src");
    assert!(
        node.is_directory,
        "the projection must say `src` is a directory — the renderer cannot \
         guess it from the child list, because an empty directory has none"
    );

    runtime
        .handle_action(DesktopAction::SelectExplorerFile {
            file_id: node.file_id,
        })
        .expect("activating a directory row should succeed");

    assert!(
        runtime
            .projection_snapshot()
            .active_buffer_projection
            .buffer_id
            .is_none(),
        "activating a directory must not open it as a text buffer"
    );
    assert!(
        runtime.explorer_path_expanded(&node.canonical_path.0),
        "activating a directory row should expand it, the way the chevron does"
    );
}

#[test]
fn clicking_a_rendered_explorer_row_opens_the_file() {
    let workspace = TempWorkspace::new();
    workspace.write("clickable.txt", "body\n");
    let runtime = open_runtime(workspace.path());
    let mut app = DesktopEframeApp::new(runtime);

    // One priming frame so the accessibility tree exists to be searched.
    let primed = app.run_headless_full_frame(full_frame_input(Vec::new()));
    let pos = accessible_clickable_center(&primed, "clickable.txt");

    let _ = app.run_headless_full_frame(full_frame_input(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        },
    ]));
    let _ = app.run_headless_full_frame(full_frame_input(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        },
    ]));
    // The action dispatched by the click is applied on the following frame.
    let _ = app.run_headless_full_frame(full_frame_input(Vec::new()));

    let snapshot = app.runtime_snapshot();
    assert!(
        snapshot.active_buffer_projection.buffer_id.is_some(),
        "clicking the rendered explorer row must open the file"
    );
    assert!(
        snapshot
            .active_buffer_projection
            .file_path
            .as_ref()
            .is_some_and(|path| path.0.ends_with("clickable.txt")),
        "the clicked row should be the file that opened, got {:?}",
        snapshot.active_buffer_projection.file_path
    );
}

fn full_frame_input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1_440.0, 900.0),
        )),
        events,
        ..egui::RawInput::default()
    }
}

fn accessible_clickable_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("full headless frames should expose the accessibility tree")
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click))
                .then(|| node.bounds())
                .flatten()
        })
        .unwrap_or_else(|| panic!("rendered `{label}` row should be clickable"));
    egui::pos2(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    )
}
