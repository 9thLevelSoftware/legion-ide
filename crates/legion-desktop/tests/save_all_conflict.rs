use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::TextCoordinate;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
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
            "legion_desktop_save_all_conflict_{}_{}_{}",
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
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_save_all_conflict_"))
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

fn tab_buffers(runtime: &DesktopRuntime) -> Vec<legion_protocol::BufferId> {
    runtime
        .projection_snapshot()
        .daily_editing_projection
        .tabs
        .tabs
        .iter()
        .map(|tab| tab.buffer_id)
        .collect()
}

fn open_file(runtime: &mut DesktopRuntime, path: &Path) {
    assert_eq!(
        runtime
            .handle_action(DesktopAction::OpenPathText(
                path.to_string_lossy().into_owned()
            ))
            .expect("open file through app authority"),
        DesktopWorkflowOutcome::Opened
    );
}

#[test]
fn save_all_conflict_mixed_result_projects_warning_rows_and_preserves_dirty_text() {
    let workspace = TempWorkspace::new();
    let clean = workspace.write("clean.txt", "clean");
    let conflicted = workspace.write("conflicted.txt", "conflicted");
    let mut runtime = open_runtime(workspace.path(), &clean);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::InsertText {
                text: "!".to_string(),
                at: coord(0, 5, 5),
            })
            .expect("edit clean"),
        DesktopWorkflowOutcome::Edited
    );
    open_file(&mut runtime, &conflicted);
    assert_eq!(
        runtime
            .handle_action(DesktopAction::InsertText {
                text: "!".to_string(),
                at: coord(0, 10, 10),
            })
            .expect("edit conflicted"),
        DesktopWorkflowOutcome::Edited
    );
    fs::write(&conflicted, "external").expect("external overwrite should succeed");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::SaveAll)
            .expect("save-all should route through app authority"),
        DesktopWorkflowOutcome::SaveAll {
            saved_count: 1,
            rejected_count: 1,
        }
    );

    assert_eq!(fs::read_to_string(&clean).expect("saved clean"), "clean!");
    assert_eq!(
        fs::read_to_string(&conflicted).expect("external content remains"),
        "external"
    );
    let snapshot = runtime.projection_snapshot();
    assert!(snapshot.active_buffer_projection.dirty);
    assert_eq!(
        snapshot.active_buffer_projection.small_buffer_text(),
        Some("conflicted!")
    );

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert!(
        model
            .status_rows
            .iter()
            .any(|row| { row.contains("Save all partial: 1 saved, 1 rejected") })
    );
    assert!(model.status_rows.iter().any(|row| {
        row.contains("save_")
            && row.contains("Save all item rejected")
            && row.contains("dirty=true")
    }));
}

#[test]
fn save_all_conflict_dirty_close_cancel_preserves_dirty_text_and_tab() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("cancel-close.txt", "dirty");
    let mut runtime = open_runtime(workspace.path(), &target);
    let buffer_id = tab_buffers(&runtime)[0];

    assert_eq!(
        runtime
            .handle_action(DesktopAction::InsertText {
                text: "!".to_string(),
                at: coord(0, 5, 5),
            })
            .expect("edit dirty"),
        DesktopWorkflowOutcome::Edited
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::CloseTab { buffer_id })
            .expect("dirty close prompts"),
        DesktopWorkflowOutcome::CloseDirtyPrompt(buffer_id)
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::CancelDirtyClose { buffer_id })
            .expect("cancel dirty close"),
        DesktopWorkflowOutcome::DirtyCloseCancelled(buffer_id)
    );

    let snapshot = runtime.projection_snapshot();
    assert!(
        snapshot
            .daily_editing_projection
            .close_dirty_prompt
            .is_none()
    );
    assert_eq!(tab_buffers(&runtime), vec![buffer_id]);
    assert!(snapshot.active_buffer_projection.dirty);
    assert_eq!(
        snapshot.active_buffer_projection.small_buffer_text(),
        Some("dirty!")
    );
}

#[test]
fn save_all_conflict_dirty_close_save_route_saves_and_clears_prompt() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("save-close.txt", "save");
    let mut runtime = open_runtime(workspace.path(), &target);
    let buffer_id = tab_buffers(&runtime)[0];

    assert_eq!(
        runtime
            .handle_action(DesktopAction::InsertText {
                text: "!".to_string(),
                at: coord(0, 4, 4),
            })
            .expect("edit dirty"),
        DesktopWorkflowOutcome::Edited
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::CloseTab { buffer_id })
            .expect("dirty close prompts"),
        DesktopWorkflowOutcome::CloseDirtyPrompt(buffer_id)
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::SaveDirtyClose { buffer_id })
            .expect("save dirty close"),
        DesktopWorkflowOutcome::Saved
    );

    let snapshot = runtime.projection_snapshot();
    assert!(
        snapshot
            .daily_editing_projection
            .close_dirty_prompt
            .is_none()
    );
    assert_eq!(tab_buffers(&runtime), vec![buffer_id]);
    assert!(!snapshot.active_buffer_projection.dirty);
    assert_eq!(fs::read_to_string(&target).expect("saved text"), "save!");
}

#[test]
fn save_all_conflict_clean_close_removes_tab_without_prompt() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("clean-close.txt", "clean");
    let mut runtime = open_runtime(workspace.path(), &target);
    let buffer_id = tab_buffers(&runtime)[0];

    assert_eq!(
        runtime
            .handle_action(DesktopAction::CloseTab { buffer_id })
            .expect("clean close"),
        DesktopWorkflowOutcome::TabClosed(buffer_id)
    );
    assert!(tab_buffers(&runtime).is_empty());
    assert!(
        runtime
            .projection_snapshot()
            .daily_editing_projection
            .close_dirty_prompt
            .is_none()
    );
}

#[test]
fn save_all_conflict_desktop_save_paths_dispatch_ui_intent() {
    let workflow_source = include_str!("../src/workflow.rs");
    assert!(workflow_source.contains("dispatch_ui_intent"));
    assert!(!workflow_source.contains("save_file_with_proposal"));
}
