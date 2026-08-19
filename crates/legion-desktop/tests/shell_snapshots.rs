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

/// Replace every machine-specific path in the projection with a stable one.
///
/// A real workspace lives under a temp directory whose name carries a
/// timestamp, a pid, and — on Windows — the account name. Snapshotting that
/// bakes the machine into the baseline: it differs on every run locally, and a
/// baseline generated here could never match one generated on a runner.
///
/// This rewrites **every** path the projection carries, not only the ones that
/// currently reach pixels. An earlier version rewrote just the active buffer's
/// path and claimed everything else was derived from it. That was wrong —
/// explorer nodes, editor tabs and the dirty-close prompt each carry their own
/// `CanonicalPath` — and it happened to produce stable images only because the
/// renderer draws names rather than paths for those. The invariant is now the
/// total one, which `stabilizing_removes_every_trace_of_the_machine` checks
/// against the whole `Debug` dump, so a renderer that starts showing a full
/// path in a tooltip cannot quietly reintroduce the problem.
fn stable_path(path: &legion_protocol::CanonicalPath) -> legion_protocol::CanonicalPath {
    // `Path::file_name` rather than splitting on separators: it is the
    // idiomatic answer and avoids a backslash literal that has to be escaped
    // differently in every tool that edits this file.
    let name = Path::new(&path.0)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    legion_protocol::CanonicalPath(format!("/workspace/{name}"))
}

fn stabilize_paths(snapshot: &mut ShellProjectionSnapshot) {
    if let Some(path) = snapshot.active_buffer_projection.file_path.as_mut() {
        *path = stable_path(path);
    }
    for node in &mut snapshot.explorer_projection.nodes {
        node.canonical_path = stable_path(&node.canonical_path);
    }
    for tab in &mut snapshot.daily_editing_projection.tabs.tabs {
        if let Some(path) = tab.file_path.as_mut() {
            *path = stable_path(path);
        }
    }
    if let Some(prompt) = snapshot
        .daily_editing_projection
        .close_dirty_prompt
        .as_mut()
        && let Some(path) = prompt.file_path.as_mut()
    {
        *path = stable_path(path);
    }
    for section in &mut snapshot.excerpt_surface_projection.sections {
        if let Some(path) = section.file_path.as_mut() {
            *path = stable_path(path);
        }
    }
}

/// Serialises rendering across the test threads in this process.
///
/// Each snapshot builds a `wgpu`-backed harness, and two of them alive at once
/// segfaults: measured at 2 failures in 30 runs with the default test threads
/// and 0 in 30 with `--test-threads=1`, on an otherwise idle machine. The crash
/// is in the driver, below anything this suite can see, so the fix is to stop
/// asking for the situation.
///
/// A lock rather than a documented "run this with --test-threads=1": CI runs
/// `cargo test --workspace`, nobody passes that flag, and a test that is only
/// correct when invoked a particular way is a trap for whoever invokes it the
/// ordinary way.
static RENDER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Render the shell and compare against the baseline for this platform.
///
/// `run()` settles the frame: egui resolves panel layout and font metrics on
/// the first pass, so capturing frame one records a shell still in motion.
fn snapshot_shell(name: &str, snapshot: &ShellProjectionSnapshot) {
    let mut snapshot = snapshot.clone();
    stabilize_paths(&mut snapshot);
    let snapshot = &snapshot;
    // Poisoning is not interesting here: it means another snapshot test failed
    // its assertion while holding the lock, and that test has already reported
    // itself. Recovering keeps one real failure from cascading into four
    // confusing ones.
    let _rendering = RENDER_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    runtime
        .handle_action(DesktopAction::RefreshExplorer)
        .expect("the explorer should refresh");
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
fn stabilizing_removes_every_trace_of_the_machine() {
    // `stabilize_paths` rewrites one field and its documentation claims that is
    // enough because everything else path-shaped derives from it. That is a
    // load-bearing assumption about a struct this test does not own: if
    // `ShellProjectionSnapshot` grows a second path field, the claim silently
    // becomes false and every baseline becomes unreproducible on the next
    // machine.
    //
    // Checking the whole `Debug` dump rather than the field verifies the claim
    // instead of restating it, and a new path field fails here rather than in a
    // confusing image diff on somebody else's platform.
    let workspace = workspace_with_files();
    let mut runtime = runtime_over(workspace.path());
    open_file(&mut runtime, "README.md");

    let mut snapshot = runtime.projection_snapshot();
    let root = workspace
        .path()
        .file_name()
        .expect("the temp workspace has a directory name")
        .to_string_lossy()
        .into_owned();
    assert!(
        format!("{snapshot:?}").contains(&root),
        "precondition: the raw projection must carry the machine-specific path, or this test proves nothing"
    );

    stabilize_paths(&mut snapshot);
    let dumped = format!("{snapshot:?}");
    assert!(
        !dumped.contains(&root),
        "the temp workspace name `{root}` survived stabilization; a field that renders a path was added and `stabilize_paths` does not know about it"
    );
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
    runtime
        .handle_action(DesktopAction::RefreshExplorer)
        .expect("the explorer should refresh");
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
