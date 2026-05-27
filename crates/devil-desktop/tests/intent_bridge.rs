use std::path::PathBuf;

use devil_desktop::bridge::{
    DesktopAction, DesktopAppRequest, DesktopBridgeError, DesktopBridgeOutput, DesktopCommandBridge,
};
use devil_protocol::{
    BufferId, CanonicalPath, FileId, ProtocolTextRange, TextCoordinate, ViewportScroll,
};
use devil_ui::ui::{DailyEditingProjection, EditorTabProjection, EditorTabsProjection};
use devil_ui::{
    ActiveBufferProjection, CommandDispatchIntent, ExplorerNodeProjection, ExplorerProjection,
    SearchScopeProjection, Shell,
};

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

fn snapshot_with_daily_tabs() -> devil_ui::ShellProjectionSnapshot {
    let mut snapshot = snapshot_with_active_buffer();
    snapshot.daily_editing_projection = DailyEditingProjection {
        tabs: EditorTabsProjection {
            tabs: vec![
                EditorTabProjection {
                    buffer_id: BufferId(9),
                    file_id: Some(FileId(2)),
                    file_path: Some(CanonicalPath("src/lib.rs".to_string())),
                    title: "lib.rs".to_string(),
                    active: true,
                    dirty: false,
                    pinned: false,
                    preview: false,
                },
                EditorTabProjection {
                    buffer_id: BufferId(10),
                    file_id: Some(FileId(3)),
                    file_path: Some(CanonicalPath("src/main.rs".to_string())),
                    title: "main.rs".to_string(),
                    active: false,
                    dirty: true,
                    pinned: false,
                    preview: false,
                },
            ],
            active_buffer_id: Some(BufferId(9)),
        },
        ..DailyEditingProjection::empty()
    };
    snapshot.explorer_projection = ExplorerProjection {
        nodes: vec![
            ExplorerNodeProjection {
                file_id: FileId(2),
                canonical_path: CanonicalPath("src".to_string()),
                name: "src".to_string(),
                children: vec![FileId(3)],
            },
            ExplorerNodeProjection {
                file_id: FileId(3),
                canonical_path: CanonicalPath("src/main.rs".to_string()),
                name: "main.rs".to_string(),
                children: Vec::new(),
            },
        ],
        selection: None,
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
fn intent_bridge_routes_daily_editing_actions() {
    let snapshot = snapshot_with_daily_tabs();
    let bridge = DesktopCommandBridge::new();
    let cursor = coord(3, 4, 12);
    let scroll = ViewportScroll {
        top_line: 8,
        left_column: 2,
    };

    assert_eq!(
        bridge.translate(DesktopAction::SaveAll, &snapshot),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SaveAll)
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SwitchTab {
                buffer_id: BufferId(10)
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SwitchTab {
            buffer_id: BufferId(10)
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::CloseTab {
                buffer_id: BufferId(10)
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::CloseTab {
            buffer_id: BufferId(10)
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SetCursor {
                buffer_id: None,
                cursor,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetCursor {
            buffer_id: BufferId(9),
            cursor,
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SetSelection {
                buffer_id: Some(BufferId(10)),
                range: range(1, 4),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetSelection {
            buffer_id: BufferId(10),
            range: range(1, 4),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SetViewportScroll {
                buffer_id: Some(BufferId(10)),
                scroll,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetViewportScroll {
            buffer_id: BufferId(10),
            scroll,
        })
    );
}

#[test]
fn intent_bridge_routes_explorer_actions_and_adapter_local_toggle() {
    let snapshot = snapshot_with_daily_tabs();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::ToggleExplorerPath {
                path: " src ".to_string()
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::ToggleExplorerPath {
            path: "src".to_string()
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SelectExplorerFile { file_id: FileId(3) },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RevealInExplorer { file_id: FileId(3) })
    );
}

#[test]
fn intent_bridge_routes_search_actions() {
    assert_eq!(
        translate(DesktopAction::ShowSearchPrompt {
            scope: SearchScopeProjection::Workspace,
        }),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::ShowSearchPrompt {
            scope: SearchScopeProjection::Workspace,
        })
    );
    assert_eq!(
        translate(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "needle".to_string(),
            limit: 7,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "needle".to_string(),
            limit: 7,
        })
    );
    assert_eq!(
        translate(DesktopAction::CancelSearch {
            query_id: "search:1".to_string(),
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelSearch {
            query_id: "search:1".to_string(),
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
        DesktopAction::SetCursor {
            buffer_id: None,
            cursor: coord(0, 0, 0),
        },
        DesktopAction::SetSelection {
            buffer_id: None,
            range: range(0, 1),
        },
        DesktopAction::SetViewportScroll {
            buffer_id: None,
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
        },
    ];

    for action in actions {
        assert_eq!(
            bridge.translate(action, &snapshot),
            DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveBuffer)
        );
    }
}

#[test]
fn intent_bridge_rejects_unknown_tabs_and_explorer_files() {
    let snapshot = snapshot_with_daily_tabs();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::SwitchTab {
                buffer_id: BufferId(99)
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownTab {
            buffer_id: BufferId(99)
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SelectExplorerFile {
                file_id: FileId(99)
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownExplorerFile {
            file_id: FileId(99)
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::ToggleExplorerPath {
                path: " ".to_string()
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPathInput)
    );
}

#[test]
fn intent_bridge_preserves_app_boundary() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bridge.rs"))
        .expect("bridge source should be readable");

    assert!(!source.contains("AppComposition"));
    assert!(!source.contains("WorkspaceActor"));
    assert!(!source.contains("EditorEngine"));
}
