use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use devil_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use devil_protocol::{ProtocolTextRange, TextCoordinate};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

fn range(start: u64, end: u64) -> ProtocolTextRange {
    ProtocolTextRange {
        start: coord(0, start as u32, start),
        end: coord(0, end as u32, end),
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
            "devil_desktop_workflow_{}_{}_{}",
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
            && file_name.is_some_and(|name| name.starts_with("devil_desktop_workflow_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn open_runtime(root: &Path, initial_file: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(
        root.to_path_buf(),
        Some(initial_file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace and file")
}

#[test]
fn desktop_workflow_opens_repo_file_and_edits_small_buffer() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("note.txt", "base");
    let mut runtime = open_runtime(workspace.path(), &target);

    let outcome = runtime
        .handle_action(DesktopAction::InsertText {
            text: "!".to_string(),
            at: coord(0, 4, 4),
        })
        .expect("insert should dispatch through app authority");
    assert_eq!(outcome, DesktopWorkflowOutcome::Edited);

    let edited = runtime.projection_snapshot();
    assert!(edited.active_buffer_projection.dirty);
    assert_eq!(
        edited.active_buffer_projection.small_buffer_text(),
        Some("base!")
    );

    let outcome = runtime
        .handle_action(DesktopAction::SaveActive)
        .expect("save should dispatch through app authority");
    assert_eq!(outcome, DesktopWorkflowOutcome::Saved);
    assert_eq!(fs::read_to_string(&target).expect("saved file"), "base!");
    assert!(!runtime.projection_snapshot().active_buffer_projection.dirty);
}

#[test]
fn desktop_workflow_external_overwrite_save_rejects_and_preserves_dirty_projection() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("stale.txt", "seed");
    let mut runtime = open_runtime(workspace.path(), &target);

    let edit = runtime
        .handle_action(DesktopAction::InsertText {
            text: "!".to_string(),
            at: coord(0, 4, 4),
        })
        .expect("insert should dispatch through app authority");
    assert_eq!(edit, DesktopWorkflowOutcome::Edited);

    fs::write(&target, "external").expect("external overwrite should succeed");

    let save = runtime
        .handle_action(DesktopAction::SaveActive)
        .expect("save rejection should still return an outcome");
    assert!(matches!(save, DesktopWorkflowOutcome::SaveRejected(_)));
    assert_eq!(
        fs::read_to_string(&target).expect("external content should remain"),
        "external"
    );

    let rejected = runtime.projection_snapshot();
    assert!(rejected.active_buffer_projection.dirty);
    assert_eq!(
        rejected.active_buffer_projection.small_buffer_text(),
        Some("seed!")
    );
}

#[test]
fn desktop_workflow_quit_sets_quit_flag_without_app_mutation() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("quit.txt", "quit");
    let mut runtime = open_runtime(workspace.path(), &target);
    let before = runtime.projection_snapshot();

    let outcome = runtime
        .handle_action(DesktopAction::Quit)
        .expect("quit should be adapter-local");
    assert_eq!(outcome, DesktopWorkflowOutcome::QuitRequested);
    assert!(runtime.quit_requested());

    let after = runtime.projection_snapshot();
    assert_eq!(
        before.active_buffer_projection.small_buffer_text(),
        after.active_buffer_projection.small_buffer_text()
    );
}

#[test]
fn desktop_workflow_replace_and_delete_route_through_app_authority() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("edit.txt", "abcde");
    let mut runtime = open_runtime(workspace.path(), &target);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::ReplaceRange {
                range: range(1, 3),
                replacement: "XY".to_string(),
            })
            .expect("replace should route through app authority"),
        DesktopWorkflowOutcome::Edited
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::DeleteRange { range: range(3, 4) })
            .expect("delete should route through app authority"),
        DesktopWorkflowOutcome::Edited
    );
    assert_eq!(
        runtime.projection_snapshot().active_buffer_projection.small_buffer_text(),
        Some("aXYe")
    );
}
