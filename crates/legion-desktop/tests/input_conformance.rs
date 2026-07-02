//! Input conformance regressions for the desktop adapter.
//!
//! These tests exercise the real input-to-action synthesis helpers that feed
//! the desktop workflow. They avoid the native-window path and instead assert
//! the exact `DesktopAction` payloads produced for keyboard, text, clipboard,
//! IME, mouse, and focus-related input cases.

use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::bridge::DesktopAction;
use legion_desktop::workflow::{
    DesktopLaunchConfig, DesktopRuntime, test_editor_keyboard_control_actions,
    test_editor_text_input_actions,
};
use legion_protocol::{ProtocolTextRange, TextCoordinate};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: std::path::PathBuf,
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
            "legion_desktop_input_conformance_{}_{}_{}",
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

    fn write(&self, name: &str, content: &str) -> std::path::PathBuf {
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
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_input_conformance_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

fn open_runtime(root: &Path, initial_file: &Path) -> DesktopRuntime {
    DesktopRuntime::open(DesktopLaunchConfig::new(
        root.to_path_buf(),
        Some(initial_file.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open workspace and file")
}

fn snapshot_text(runtime: &DesktopRuntime) -> Option<String> {
    runtime
        .projection_snapshot()
        .active_buffer_projection
        .small_buffer_text()
        .map(str::to_owned)
}

fn snapshot_viewport_cursor(runtime: &DesktopRuntime) -> TextCoordinate {
    runtime
        .projection_snapshot()
        .active_buffer_projection
        .viewport
        .as_ref()
        .expect("viewport should be projected for the active buffer")
        .cursor
}

#[test]
fn keyboard_input_moves_the_cursor_through_the_real_egui_context() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("keyboard.txt", "abc");
    let runtime = open_runtime(workspace.path(), &file);
    let snapshot = runtime.projection_snapshot();

    egui::__run_test_ui(|ui| {
        ui.ctx().input_mut(|input| {
            input.events = vec![egui::Event::Key {
                key: egui::Key::ArrowRight,
                physical_key: Some(egui::Key::ArrowRight),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            }];
            input.modifiers = egui::Modifiers::default();
        });
        let actions =
            ui.input(|input| test_editor_keyboard_control_actions(input, &snapshot, true, false));
        assert_eq!(
            actions,
            vec![DesktopAction::SetCursor {
                buffer_id: Some(
                    snapshot
                        .active_buffer_projection
                        .buffer_id
                        .expect("active buffer")
                ),
                cursor: TextCoordinate {
                    line: 0,
                    character: 1,
                    byte_offset: None,
                    utf16_offset: Some(1),
                },
            }]
        );
    });
}

#[test]
fn keyboard_selection_input_updates_the_projected_selection_range() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("selection.txt", "abc");
    let runtime = open_runtime(workspace.path(), &file);
    let snapshot = runtime.projection_snapshot();

    egui::__run_test_ui(|ui| {
        ui.ctx().input_mut(|input| {
            input.events = vec![egui::Event::Key {
                key: egui::Key::ArrowRight,
                physical_key: Some(egui::Key::ArrowRight),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    shift: true,
                    ..egui::Modifiers::default()
                },
            }];
            input.modifiers = egui::Modifiers {
                shift: true,
                ..egui::Modifiers::default()
            };
        });
        let actions =
            ui.input(|input| test_editor_keyboard_control_actions(input, &snapshot, true, false));
        assert_eq!(
            actions,
            vec![DesktopAction::SetSelection {
                buffer_id: Some(
                    snapshot
                        .active_buffer_projection
                        .buffer_id
                        .expect("active buffer")
                ),
                range: ProtocolTextRange {
                    start: coord(0, 0, 0),
                    end: TextCoordinate {
                        line: 0,
                        character: 1,
                        byte_offset: None,
                        utf16_offset: Some(1),
                    },
                },
            }]
        );
    });
}

#[test]
fn mouse_input_does_not_change_the_projected_buffer_text() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("mouse.txt", "");
    let runtime = open_runtime(workspace.path(), &file);
    let snapshot = runtime.projection_snapshot();

    egui::__run_test_ui(|ui| {
        let actions = test_editor_text_input_actions(
            ui,
            &[
                egui::Event::PointerMoved(egui::pos2(120.0, 80.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(120.0, 80.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
                egui::Event::PointerButton {
                    pos: egui::pos2(120.0, 80.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ],
            &snapshot,
            true,
        );
        assert!(actions.is_empty());
    });

    assert_eq!(snapshot_text(&runtime), Some(String::new()));
}

#[test]
fn clipboard_input_routes_to_the_buffer_through_the_egui_context() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("clipboard.txt", "");
    let runtime = open_runtime(workspace.path(), &file);
    let snapshot = runtime.projection_snapshot();

    egui::__run_test_ui(|ui| {
        let actions = test_editor_text_input_actions(
            ui,
            &[egui::Event::Paste("clip ñ".to_string())],
            &snapshot,
            true,
        );
        assert_eq!(
            actions,
            vec![DesktopAction::ClipboardPaste {
                text: "clip ñ".to_string(),
                at: coord(0, 0, 0),
            }]
        );
    });
}

#[test]
fn ime_input_routes_commit_text_and_preserves_the_commit_payload() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("ime.txt", "");
    let runtime = open_runtime(workspace.path(), &file);
    let snapshot = runtime.projection_snapshot();

    egui::__run_test_ui(|ui| {
        let actions = test_editor_text_input_actions(
            ui,
            &[
                egui::Event::Ime(egui::ImeEvent::Enabled),
                egui::Event::Ime(egui::ImeEvent::Preedit("にほん".to_string())),
                egui::Event::Ime(egui::ImeEvent::Commit("入力".to_string())),
                egui::Event::Ime(egui::ImeEvent::Disabled),
            ],
            &snapshot,
            true,
        );
        assert_eq!(
            actions,
            vec![DesktopAction::ImeCommit {
                text: "入力".to_string(),
                at: coord(0, 0, 0),
            }]
        );
    });
}

#[test]
fn unfocused_input_does_not_dispatch_keyboard_or_text_events() {
    let workspace = TempWorkspace::new();
    let file = workspace.write("focus.txt", "");
    let runtime = open_runtime(workspace.path(), &file);
    let snapshot = runtime.projection_snapshot();

    egui::__run_test_ui(|ui| {
        let text_actions = test_editor_text_input_actions(
            ui,
            &[
                egui::Event::Text("x".to_string()),
                egui::Event::Paste("y".to_string()),
            ],
            &snapshot,
            false,
        );
        assert!(text_actions.is_empty());

        let keyboard_actions =
            ui.input(|input| test_editor_keyboard_control_actions(input, &snapshot, false, false));
        assert!(keyboard_actions.is_empty());
    });

    assert_eq!(snapshot_text(&runtime), Some(String::new()));
    assert_eq!(snapshot_viewport_cursor(&runtime), coord(0, 0, 0));
}
