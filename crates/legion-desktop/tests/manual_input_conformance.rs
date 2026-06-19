use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::DesktopAction,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{BufferId, ProtocolTextRange, TextCoordinate};
use legion_ui::{PaletteMode, SearchScopeProjection};

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
            "legion_desktop_manual_input_{}_{}_{}",
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
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_manual_input_"))
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

fn active_buffer(runtime: &DesktopRuntime) -> BufferId {
    runtime
        .projection_snapshot()
        .active_buffer_projection
        .buffer_id
        .expect("active buffer should be projected")
}

fn active_text(runtime: &DesktopRuntime) -> String {
    runtime
        .projection_snapshot()
        .active_buffer_projection
        .small_buffer_text()
        .expect("small buffer text should be projected")
        .to_string()
}

#[test]
fn manual_input_conformance_commits_ime_text_and_advances_projected_cursor() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("main.rs", "fn main() {}\n");
    let mut runtime = open_runtime(workspace.path(), &target);
    let buffer_id = active_buffer(&runtime);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::ImeCommit {
                text: "漢".to_string(),
                at: coord(0, 0, 0),
            })
            .expect("IME commit should route through editor authority"),
        DesktopWorkflowOutcome::Edited
    );

    let snapshot = runtime.projection_snapshot();
    assert!(
        snapshot
            .active_buffer_projection
            .small_buffer_text()
            .is_some_and(|text| text.starts_with("漢fn main")),
        "IME commit should insert text at the projected coordinate"
    );
    let cursor = snapshot
        .daily_editing_projection
        .viewport_states
        .iter()
        .find(|state| state.buffer_id == buffer_id)
        .and_then(|state| state.cursor)
        .or_else(|| {
            snapshot
                .active_buffer_projection
                .viewport
                .as_ref()
                .map(|viewport| viewport.cursor)
        })
        .expect("projected cursor should be available after IME commit");
    assert_eq!(cursor.line, 0);
    assert_eq!(cursor.byte_offset, Some("漢".len() as u64));
    assert_eq!(
        cursor.utf16_offset,
        Some("漢".encode_utf16().count() as u64)
    );
}

#[test]
fn manual_input_conformance_clipboard_copy_cut_paste_and_select_all_are_app_owned() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("main.rs", "alpha\nbeta\n");
    let mut runtime = open_runtime(workspace.path(), &target);
    let buffer_id = active_buffer(&runtime);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::SetSelection {
                buffer_id: Some(buffer_id),
                range: range(0, 5),
            })
            .expect("selection should route through editor authority"),
        DesktopWorkflowOutcome::SelectionSet(buffer_id)
    );
    assert!(matches!(
        runtime
            .handle_action(DesktopAction::ClipboardCopy)
            .expect("copy should route through app-owned clipboard metadata"),
        DesktopWorkflowOutcome::ClipboardUpdated {
            buffer_id: copied_buffer,
            byte_len: 5,
            line_count: 1,
            cut: false,
        } if copied_buffer == buffer_id
    ));
    assert!(matches!(
        runtime
            .handle_action(DesktopAction::ClipboardCut)
            .expect("cut should route through app-owned edit authority"),
        DesktopWorkflowOutcome::ClipboardUpdated {
            buffer_id: cut_buffer,
            byte_len: 5,
            line_count: 1,
            cut: true,
        } if cut_buffer == buffer_id
    ));
    assert_ne!(active_text(&runtime), "alpha\nbeta\n");
    let snapshot_after_cut = runtime.projection_snapshot();
    assert!(
        snapshot_after_cut
            .daily_editing_projection
            .viewport_states
            .iter()
            .find(|state| state.buffer_id == buffer_id)
            .is_some_and(|state| state.selections.is_empty()),
        "cut should collapse the consumed selection"
    );

    assert_eq!(
        runtime
            .handle_action(DesktopAction::ClipboardPaste {
                text: "alpha".to_string(),
                at: coord(0, 0, 0),
            })
            .expect("paste should route through existing insert authority"),
        DesktopWorkflowOutcome::Edited
    );
    assert_eq!(active_text(&runtime), "alpha\nbeta\n");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::SelectAll {
                buffer_id: Some(buffer_id),
            })
            .expect("select-all should route through app-owned selection authority"),
        DesktopWorkflowOutcome::SelectionSet(buffer_id)
    );

    let snapshot = runtime.projection_snapshot();
    assert_eq!(
        snapshot.active_buffer_projection.small_buffer_text(),
        Some("alpha\nbeta\n")
    );
    assert!(
        snapshot
            .daily_editing_projection
            .viewport_states
            .iter()
            .find(|state| state.buffer_id == buffer_id)
            .is_some_and(|state| state
                .selections
                .iter()
                .any(|selection| { selection.start.byte_offset < selection.end.byte_offset })),
        "select-all should project a non-empty selection range"
    );
}

#[test]
fn manual_input_conformance_palette_focus_blocks_direct_editor_insert() {
    let workspace = TempWorkspace::new();
    let target = workspace.write("main.rs", "fn main() {}\n");
    let mut runtime = open_runtime(workspace.path(), &target);

    assert_eq!(
        runtime
            .handle_action(DesktopAction::OpenPalette {
                mode: PaletteMode::Command,
                query: String::new(),
                scope: SearchScopeProjection::Workspace,
            })
            .expect("palette should open through app authority"),
        DesktopWorkflowOutcome::Noop
    );
    assert!(
        runtime.projection_snapshot().palette_projection.open,
        "palette should be open before text input"
    );

    assert_eq!(
        runtime
            .handle_action(DesktopAction::InsertText {
                text: "x".to_string(),
                at: coord(0, 0, 0),
            })
            .expect("palette focus should consume direct editor text input"),
        DesktopWorkflowOutcome::Noop
    );

    let snapshot = runtime.projection_snapshot();
    assert_eq!(
        snapshot.active_buffer_projection.small_buffer_text(),
        Some("fn main() {}\n")
    );
    assert!(snapshot.palette_projection.open);
}
