//! Visual regression snapshots of the rendered shell.
//!
//! The interaction suites (`shell_affordances`, `explorer_activation`) assert
//! that a control exists, is hit-testable, and dispatches. They cannot see what
//! it *looks* like, and several defects found on 2026-08-17 were exactly that:
//! five activity-rail icons rendering as the missing-glyph box, a tab's close
//! button floating outside the tab it belonged to, a modal laid out below the
//! window's bottom edge, and the Windows extended-length path prefix leaking
//! into the breadcrumb. Every one was visible in a screenshot and invisible to
//! an assertion about state.
//!
//! States are chosen for where defects actually lived, not for coverage: each
//! snapshot is three baselines to regenerate, so the set stays small and every
//! member earns its place.
//!
//! ## Per-platform baselines
//!
//! Font rasterisation, hinting and subpixel placement differ enough between
//! Windows, macOS and Linux that one baseline cannot serve all three without a
//! threshold so loose it stops catching the things above. Three baselines cost
//! more; they are worth it because a diff nobody can evaluate gets
//! rubber-stamped, and a rubber-stamped diff is worse than no snapshot.
//!
//! ## Regenerating
//!
//! ```text
//! UPDATE_SNAPSHOTS=1 cargo test -p legion-desktop --test shell_snapshots
//! ```
//!
//! That writes the baseline for *the platform you are on*. CI uploads the
//! `.new.png` and `.diff.png` files from every failing platform as artifacts,
//! so the other two can be taken from a run rather than from three machines.
//! See `docs/ui/snapshot-testing.md`.

use std::path::Path;

use egui_kittest::Harness;
use legion_desktop::{
    bridge::DesktopAction,
    view::{DesktopProjectionViewState, ProjectionView},
    workflow::{DesktopLaunchConfig, DesktopRuntime},
};
use legion_ui::{Shell, ShellProjectionSnapshot};

mod common;
use common::TempWorkspace;

/// Baseline name for the platform this test is running on.
///
/// `std::env::consts::OS` rather than a cargo feature: the baseline has to
/// match the machine that rasterised the glyphs, and that is a runtime fact.
fn per_os(name: &str) -> String {
    format!("{name}-{}", std::env::consts::OS)
}

/// Replace machine-specific paths with a stable one.
///
/// The breadcrumb and status bar render the active buffer's canonical path, and
/// a real workspace lives under a temp directory whose name carries a
/// timestamp, a pid, and — on Windows — the account name. Snapshotting that
/// bakes the machine into the baseline: it differs on every run locally, and a
/// baseline generated here could never match one generated on a runner.
///
/// Normalising one field is enough because everything else path-shaped is
/// derived from it: the breadcrumb takes its trailing segments, the status bar
/// shows it, and explorer rows and tab titles render file *names* rather than
/// paths.
fn stabilize_paths(snapshot: &mut ShellProjectionSnapshot) {
    if let Some(path) = snapshot.active_buffer_projection.file_path.as_mut() {
        // `Path::file_name` rather than splitting on separators: it is the
        // idiomatic answer and avoids a backslash literal that has to be
        // escaped differently in every tool that edits this file.
        let name = Path::new(&path.0)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".to_string());
        *path = legion_protocol::CanonicalPath(format!("/workspace/{name}"));
    }
}

/// Render the shell and compare against the baseline for this platform.
///
/// `run()` settles the frame: egui resolves panel layout and font metrics on
/// the first pass, so capturing frame one records a shell still in motion.
fn snapshot_shell(name: &str, snapshot: &ShellProjectionSnapshot) {
    let mut snapshot = snapshot.clone();
    stabilize_paths(&mut snapshot);
    let snapshot = &snapshot;
    let state = DesktopProjectionViewState::default();
    let mut view = ProjectionView::new();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1_280.0, 800.0))
        .build_ui(|ui| {
            let _ = view.render_with_state(ui, snapshot, &state);
        });
    harness.run();
    harness.snapshot(per_os(name));
}

/// A runtime over a small, fixed workspace.
///
/// Real projections rather than hand-built fixtures: a snapshot of a fixture
/// records what the fixture says, and this session has already shown twice what
/// happens when a fixture and the product disagree.
fn runtime_over(root: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(root.to_path_buf(), None))
        .expect("desktop runtime should open workspace")
}

fn workspace_with_files() -> TempWorkspace {
    let workspace = TempWorkspace::new("legion_desktop_shell_snapshots");
    workspace.write("README.md", "# Legion\n\nA native IDE.\n");
    workspace.write("Cargo.toml", "[package]\nname = \"demo\"\n");
    workspace.mkdir("src");
    workspace.write("src/main.rs", "fn main() {\n    println!(\"hello\");\n}\n");
    workspace
}

fn open_file(runtime: &mut DesktopRuntime, name: &str) {
    let _ = runtime.handle_action(DesktopAction::RefreshExplorer);
    let node = runtime
        .projection_snapshot()
        .explorer_projection
        .nodes
        .into_iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("explorer should project `{name}`"));
    runtime
        .handle_action(DesktopAction::SelectExplorerFile {
            file_id: node.file_id,
        })
        .expect("opening a file should succeed");
}

#[test]
fn the_empty_shell_looks_like_its_baseline() {
    // The first thing anyone sees, and where the remaining placeholder strings
    // (`<empty explorer>`, `<no open tabs>`) are visible.
    snapshot_shell(
        "empty-shell",
        &Shell::empty("Legion IDE").projection_snapshot(),
    );
}

#[test]
fn the_explorer_looks_like_its_baseline() {
    // Disclosure triangles, row alignment and selection — the region whose
    // markers rendered as a literal `-` per file until 2026-08-17.
    let workspace = workspace_with_files();
    let mut runtime = runtime_over(workspace.path());
    let _ = runtime.handle_action(DesktopAction::RefreshExplorer);
    snapshot_shell("explorer", &runtime.projection_snapshot());
}

#[test]
fn an_open_file_looks_like_its_baseline() {
    // Tab strip, close affordance, breadcrumb and gutter. The tab's `×` floated
    // outside its tab here, and the breadcrumb showed `\\?\`-prefixed paths.
    let workspace = workspace_with_files();
    let mut runtime = runtime_over(workspace.path());
    open_file(&mut runtime, "README.md");
    snapshot_shell("open-file", &runtime.projection_snapshot());
}

#[test]
fn the_unsaved_changes_prompt_looks_like_its_baseline() {
    // The modal that rendered below the window's bottom edge at every window
    // size, while also disabling typing. A snapshot sees that immediately; an
    // assertion about projection state never did.
    let workspace = workspace_with_files();
    let mut runtime = runtime_over(workspace.path());
    open_file(&mut runtime, "README.md");
    let buffer_id = runtime
        .projection_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("a buffer should be open");
    runtime
        .handle_action(DesktopAction::InsertText {
            text: "x".to_string(),
            at: legion_protocol::TextCoordinate {
                line: 0,
                character: 0,
                byte_offset: Some(0),
                utf16_offset: Some(0),
            },
        })
        .expect("typing should succeed");
    runtime
        .handle_action(DesktopAction::CloseTab { buffer_id })
        .expect("closing a dirty tab should raise the prompt");
    let snapshot = runtime.projection_snapshot();
    assert!(
        snapshot
            .daily_editing_projection
            .close_dirty_prompt
            .is_some(),
        "the state under test must actually be raised, or this snapshots the \
         ordinary shell and passes forever"
    );
    snapshot_shell("unsaved-changes-prompt", &snapshot);
}
