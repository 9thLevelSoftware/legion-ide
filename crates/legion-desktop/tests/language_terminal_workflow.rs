use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use legion_desktop::{
    bridge::{DesktopAction, DesktopBridgeError, DesktopBridgeOutput, DesktopCommandBridge},
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{BufferId, TerminalPanelProjection, TerminalSessionId, TextCoordinate};

mod common;
use legion_protocol::{
    EventSequence, RedactionHint, TerminalOutputRowProjection, TerminalPanelStatus,
    TerminalPanelStatusKind, TerminalRuntimeState, TerminalScrollbackProjection,
    TerminalSearchProjection, TimestampMillis,
};
use legion_ui::{
    ActiveBufferProjection, ActiveBufferProjectionState, CommandDispatchIntent, Shell,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "legion-desktop-language-terminal-workflow-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create temp workspace");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("write workspace file");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn position(byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line: 0,
        character: byte_offset as u32,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

#[test]
fn desktop_language_failures_are_visible() {
    let workspace = TempWorkspace::new();
    let source = workspace.write("main.rs", "fn main() {}\n");
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::RequestCompletion {
                position: position(3),
            })
            .expect("completion action"),
        DesktopWorkflowOutcome::LanguageToolingUpdated
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::CancelLanguageOperation {
                operation_id: "language:Completion:1".to_string(),
            })
            .expect("cancel language action"),
        DesktopWorkflowOutcome::LanguageToolingUpdated
    );
    let language_model = DesktopProjectionViewModel::from_snapshot(&runtime.projection_snapshot());
    assert!(
        language_model
            .language_rows
            .iter()
            .any(|row| row.contains("cancelled=1"))
    );
}

#[test]
fn desktop_terminal_failures_and_bounds_are_visible() {
    let workspace = TempWorkspace::new();
    let source = workspace.write("main.rs", "fn main() {}\n");
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(source.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open");

    assert_eq!(
        runtime
            .handle_action(DesktopAction::TerminalLaunch {
                command_label: "fixture".to_string(),
            })
            .expect("default-denied terminal launch"),
        DesktopWorkflowOutcome::TerminalPanelUpdated
    );
    let terminal_model = DesktopProjectionViewModel::from_snapshot(&runtime.projection_snapshot());
    assert!(
        terminal_model
            .terminal_rows
            .iter()
            .any(|row| row.contains("terminal denial"))
    );

    let mut snapshot = Shell::empty("language-terminal").projection_snapshot();
    snapshot.terminal_panel_projection = TerminalPanelProjection {
        active_session_id: Some(TerminalSessionId(9)),
        runtime_state: Some(TerminalRuntimeState::Degraded),
        status: TerminalPanelStatus {
            kind: TerminalPanelStatusKind::Degraded,
            message: "bounded terminal projection".to_string(),
        },
        output_rows: vec![TerminalOutputRowProjection {
            session_id: TerminalSessionId(9),
            sequence: EventSequence(1),
            redacted_payload: "bounded output row".to_string(),
            byte_count: 18,
            is_stderr: false,
            truncated: false,
            redaction: RedactionHint::MetadataOnly,
            schema_version: 1,
        }],
        scrollback: TerminalScrollbackProjection {
            visible_row_count: 1,
            omitted_row_count: 3,
            byte_limit: 256 * 1024,
            truncated: true,
            schema_version: 1,
        },
        search: TerminalSearchProjection {
            query_label: Some("bounded".to_string()),
            match_count: 1,
            active_match_index: Some(0),
            truncated: false,
            schema_version: 1,
        },
        generated_at: TimestampMillis(1),
        ..TerminalPanelProjection::empty()
    };
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    let status_row = model
        .terminal_rows
        .iter()
        .find(|row| row.contains("terminal:"))
        .expect("terminal status row");
    assert!(status_row.contains("omitted=3"));
    assert!(status_row.contains("matches=1"));
}

#[test]
fn desktop_phase4_dispatch_stays_projection_only() {
    let bridge = DesktopCommandBridge::new();
    let empty = Shell::empty("language-terminal").projection_snapshot();

    assert_eq!(
        bridge.translate(
            DesktopAction::RequestHover {
                position: position(0),
            },
            &empty,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveBuffer)
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::TerminalInput {
                payload: "x".to_string()
            },
            &empty
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveTerminalSession)
    );

    let mut snapshot = empty;
    snapshot.active_buffer_projection = ActiveBufferProjection {
        state: ActiveBufferProjectionState::Full,
        buffer_id: Some(BufferId(5)),
        ..ActiveBufferProjection::empty()
    };
    snapshot.terminal_panel_projection = TerminalPanelProjection {
        active_session_id: Some(TerminalSessionId(7)),
        ..TerminalPanelProjection::empty()
    };

    assert_eq!(
        bridge.translate(
            DesktopAction::RequestRenameProposal {
                position: position(2),
                new_name: "renamed".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RequestRenameProposal {
            buffer_id: BufferId(5),
            position: position(2),
            new_name: "renamed".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(DesktopAction::TerminalOutputPoll, &snapshot),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::TerminalOutputPoll {
            session_id: TerminalSessionId(7),
        })
    );

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bridge_source =
        fs::read_to_string(manifest_dir.join("src/bridge.rs")).expect("read bridge source");
    let view_source =
        fs::read_to_string(manifest_dir.join("src/view.rs")).expect("read view source");
    let forbidden = ["AppComposition", "legion_terminal", "TerminalRuntime"];
    common::assert_source_excludes(&bridge_source, "src/bridge.rs", &forbidden);
    common::assert_source_excludes(&view_source, "src/view.rs", &forbidden);
}
