use std::path::PathBuf;

use devil_desktop::bridge::{
    DesktopAction, DesktopAppRequest, DesktopBridgeError, DesktopBridgeOutput,
    DesktopCommandBridge,
};
use devil_protocol::{BufferId, ProtocolTextRange, TextCoordinate};
use devil_ui::{ActiveBufferProjection, CommandDispatchIntent, Shell};

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

fn snapshot_with_active_buffer() -> devil_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Bridge").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        buffer_id: Some(BufferId(9)),
        ..ActiveBufferProjection::empty()
    };
    snapshot
}

fn snapshot_without_active_buffer() -> devil_ui::ShellProjectionSnapshot {
    Shell::empty("Bridge").projection_snapshot()
}

fn translate(action: DesktopAction) -> DesktopBridgeOutput {
    DesktopCommandBridge::new().translate(action, &snapshot_with_active_buffer())
}

#[test]
fn intent_bridge_routes_save_edit_undo_redo_actions() {
    let insert_at = coord(0, 2, 2);
    assert_eq!(
        translate(DesktopAction::SaveActive),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Save {
            buffer_id: BufferId(9)
        })
    );
    assert_eq!(
        translate(DesktopAction::InsertText {
            text: "hello".to_string(),
            at: insert_at,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Insert {
            buffer_id: BufferId(9),
            at: insert_at,
            text: "hello".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::ReplaceRange {
            range: range(0, 5),
            replacement: "world".to_string(),
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Replace {
            buffer_id: BufferId(9),
            range: range(0, 5),
            replacement: "world".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::DeleteRange { range: range(1, 3) }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Delete {
            buffer_id: BufferId(9),
            range: range(1, 3),
        })
    );
    assert_eq!(
        translate(DesktopAction::Undo),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Undo {
            buffer_id: BufferId(9)
        })
    );
    assert_eq!(
        translate(DesktopAction::Redo),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Redo {
            buffer_id: BufferId(9)
        })
    );
}

#[test]
fn intent_bridge_preserves_clipboard_and_ime_text() {
    let at = coord(1, 0, 7);
    assert_eq!(
        translate(DesktopAction::ClipboardPaste {
            text: "clip ñ".to_string(),
            at,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Insert {
            buffer_id: BufferId(9),
            at,
            text: "clip ñ".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::ImeCommit {
            text: "入力".to_string(),
            at,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Insert {
            buffer_id: BufferId(9),
            at,
            text: "入力".to_string(),
        })
    );
}

#[test]
fn intent_bridge_routes_path_dialog_prompt_refresh_and_quit() {
    assert_eq!(
        translate(DesktopAction::OpenPathText("  Cargo.toml  ".to_string())),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPath {
            path: "Cargo.toml".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::OpenPathDialogSelected(
            "src/main.rs".to_string()
        )),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPath {
            path: "src/main.rs".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::OpenPathDialogCancelled),
        DesktopBridgeOutput::Noop
    );
    assert_eq!(
        translate(DesktopAction::ShowOpenPathPrompt),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::ShowOpenPathPrompt)
    );
    assert_eq!(
        translate(DesktopAction::OpenWorkspace {
            root: PathBuf::from(".")
        }),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::OpenWorkspace {
            root: PathBuf::from(".")
        })
    );
    assert_eq!(
        translate(DesktopAction::RefreshExplorer),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshExplorer)
    );
    assert_eq!(
        translate(DesktopAction::Quit),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Quit)
    );
}

#[test]
fn intent_bridge_rejects_invalid_path_input() {
    assert_eq!(
        translate(DesktopAction::OpenPathText(" \t ".to_string())),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPathInput)
    );
    assert_eq!(
        translate(DesktopAction::OpenPathDialogSelected(String::new())),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPathInput)
    );
}

#[test]
fn intent_bridge_rejects_buffer_actions_without_active_buffer() {
    let snapshot = snapshot_without_active_buffer();
    let bridge = DesktopCommandBridge::new();
    let actions = [
        DesktopAction::SaveActive,
        DesktopAction::InsertText {
            text: "x".to_string(),
            at: coord(0, 0, 0),
        },
        DesktopAction::ReplaceRange {
            range: range(0, 1),
            replacement: "x".to_string(),
        },
        DesktopAction::DeleteRange { range: range(0, 1) },
        DesktopAction::Undo,
        DesktopAction::Redo,
    ];

    for action in actions {
        assert_eq!(
            bridge.translate(action, &snapshot),
            DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveBuffer)
        );
    }
}

#[test]
fn intent_bridge_preserves_app_boundary() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bridge.rs"))
        .expect("bridge source should be readable");

    assert!(!source.contains("AppComposition"));
    assert!(!source.contains("WorkspaceActor"));
    assert!(!source.contains("EditorEngine"));
}
