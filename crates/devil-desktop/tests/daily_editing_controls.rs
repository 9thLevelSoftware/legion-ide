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
use devil_protocol::{BufferId, FileId, ProtocolTextRange, TextCoordinate, ViewportScroll};
use devil_ui::ExplorerNodeProjection;

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
            "devil_desktop_daily_editing_{}_{}_{}",
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
            && file_name.is_some_and(|name| name.starts_with("devil_desktop_daily_editing_"))
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

fn open_second_file(runtime: &mut DesktopRuntime, path: &Path) -> FileId {
    let outcome = runtime
        .handle_action(DesktopAction::OpenPathText(
            path.to_string_lossy().into_owned(),
        ))
        .expect("open path should route through app authority");
    assert_eq!(outcome, DesktopWorkflowOutcome::Opened);
    runtime
        .projection_snapshot()
        .active_buffer_projection
        .file_id
        .expect("opened file should become active")
}

fn tab_buffers(runtime: &DesktopRuntime) -> Vec<BufferId> {
    runtime
        .projection_snapshot()
        .daily_editing_projection
        .tabs
        .tabs
        .iter()
        .map(|tab| tab.buffer_id)
        .collect()
}

fn first_explorer_file(nodes: &[ExplorerNodeProjection]) -> Option<FileId> {
    nodes.first().map(|node| node.file_id)
}

#[test]
fn daily_editing_controls_switch_close_and_save_all_through_app_authority() {
    let workspace = TempWorkspace::new();
    let first = workspace.write("first.txt", "first");
    let second = workspace.write("second.txt", "second");
    let mut runtime = open_runtime(workspace.path(), &first);
    open_second_file(&mut runtime, &second);

    let buffers = tab_buffers(&runtime);
    assert_eq!(buffers.len(), 2);
    let first_buffer = buffers[0];
    let second_buffer = buffers[1];

    assert_eq!(
        runtime
            .handle_action(DesktopAction::SwitchTab {
                buffer_id: first_buffer,
            })
            .expect("switch should route through app authority"),
        DesktopWorkflowOutcome::TabSwitched(first_buffer)
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::InsertText {
                text: "!".to_string(),
                at: coord(0, 5, 5),
            })
            .expect("edit should route through app authority"),
        DesktopWorkflowOutcome::Edited
    );

    match runtime
        .handle_action(DesktopAction::SaveAll)
        .expect("save all should route through app authority")
    {
        DesktopWorkflowOutcome::SaveAll {
            saved_count,
            rejected_count: _,
        } => {
            assert!(saved_count >= 1);
        }
        other => panic!("unexpected save-all outcome: {other:?}"),
    }
    assert_eq!(fs::read_to_string(&first).expect("saved first"), "first!");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::CloseTab {
                buffer_id: first_buffer,
            })
            .expect("clean close should route through app authority"),
        DesktopWorkflowOutcome::TabClosed(first_buffer)
    );
    assert_eq!(tab_buffers(&runtime), vec![second_buffer]);
}

#[test]
fn daily_editing_controls_dirty_close_projects_prompt_and_keeps_text() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("dirty.txt", "dirty");
    let mut runtime = open_runtime(workspace.path(), &target);
    let buffer_id = tab_buffers(&runtime)[0];

    assert_eq!(
        runtime
            .handle_action(DesktopAction::InsertText {
                text: "!".to_string(),
                at: coord(0, 5, 5),
            })
            .expect("edit should route through app authority"),
        DesktopWorkflowOutcome::Edited
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::CloseTab { buffer_id })
            .expect("dirty close should route through app authority"),
        DesktopWorkflowOutcome::CloseDirtyPrompt(buffer_id)
    );

    let snapshot = runtime.projection_snapshot();
    assert_eq!(
        snapshot.active_buffer_projection.small_buffer_text(),
        Some("dirty!")
    );
    assert!(snapshot.active_buffer_projection.dirty);
    assert!(
        snapshot
            .daily_editing_projection
            .close_dirty_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.buffer_id == buffer_id)
    );
}

#[test]
fn daily_editing_controls_cursor_selection_and_scroll_dispatch() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("viewport.txt", "line one\nline two");
    let mut runtime = open_runtime(workspace.path(), &target);
    let buffer_id = tab_buffers(&runtime)[0];
    let scroll = ViewportScroll {
        top_line: 3,
        left_column: 7,
    };

    assert_eq!(
        runtime
            .handle_action(DesktopAction::SetCursor {
                buffer_id: Some(buffer_id),
                cursor: coord(1, 2, 11),
            })
            .expect("cursor should route through editor authority"),
        DesktopWorkflowOutcome::CursorSet(buffer_id)
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::SetSelection {
                buffer_id: Some(buffer_id),
                range: range(0, 4),
            })
            .expect("selection should route through editor authority"),
        DesktopWorkflowOutcome::SelectionSet(buffer_id)
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::SetViewportScroll {
                buffer_id: Some(buffer_id),
                scroll,
            })
            .expect("scroll should route through app authority"),
        DesktopWorkflowOutcome::ViewportScrollSet(buffer_id)
    );

    assert!(
        runtime
            .projection_snapshot()
            .daily_editing_projection
            .viewport_states
            .iter()
            .any(|state| state.buffer_id == buffer_id && state.scroll == scroll)
    );
}

#[test]
fn daily_editing_controls_explorer_select_and_toggle() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("explorer.txt", "explore");
    let mut runtime = open_runtime(workspace.path(), &target);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::RefreshExplorer)
            .expect("refresh should route through app authority"),
        DesktopWorkflowOutcome::ExplorerRefreshed
    );
    let snapshot = runtime.projection_snapshot();
    let file_id = first_explorer_file(&snapshot.explorer_projection.nodes)
        .expect("workspace explorer should contain at least one file");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::SelectExplorerFile { file_id })
            .expect("select should reveal through app authority"),
        DesktopWorkflowOutcome::ExplorerRefreshed
    );
    assert!(
        runtime
            .projection_snapshot()
            .explorer_projection
            .selection
            .as_ref()
            .is_some_and(|selection| selection.file_id == file_id)
    );

    assert_eq!(
        runtime
            .handle_action(DesktopAction::ToggleExplorerPath {
                path: "explorer.txt".to_string(),
            })
            .expect("toggle should update adapter-local state"),
        DesktopWorkflowOutcome::ExplorerPathToggled("explorer.txt".to_string())
    );
    assert!(runtime.explorer_path_expanded("explorer.txt"));
}
