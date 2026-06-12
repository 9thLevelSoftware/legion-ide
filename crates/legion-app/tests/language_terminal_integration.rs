use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{
    BufferId, PrincipalId, ProposalLifecycleState, ProposalPayloadKind, TerminalPanelStatusKind,
    TextCoordinate, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;
use serde_json::json;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "legion-language-terminal-integration-{}-{}",
            std::process::id(),
            TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        Self { root }
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        std::fs::write(&path, content).expect("write workspace file");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
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

fn open_source_app(
    workspace: &TempWorkspace,
    source: &std::path::Path,
) -> (AppComposition, BufferId) {
    let mut app = AppComposition::new();
    app.open_workspace(
        &workspace.root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-language-terminal".to_string()),
    )
    .expect("open workspace");
    app.open_file(source.to_string_lossy())
        .expect("open source file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    (app, buffer_id)
}

#[test]
fn language_read_only_actions_do_not_mutate() {
    let workspace = TempWorkspace::new();
    let source = workspace.write("main.rs", "fn old_name() {}\n");
    let (mut app, buffer_id) = open_source_app(&workspace, &source);
    let original_editor_text = app
        .editor()
        .text(buffer_id)
        .expect("active editor text")
        .to_string();
    let original_disk_text = std::fs::read_to_string(&source).expect("disk source");

    for intent in [
        CommandDispatchIntent::RequestHover {
            buffer_id,
            position: position(3),
        },
        CommandDispatchIntent::RequestCompletion {
            buffer_id,
            position: position(3),
        },
        CommandDispatchIntent::GoToDefinition {
            buffer_id,
            position: position(3),
        },
        CommandDispatchIntent::FindReferences {
            buffer_id,
            position: position(3),
        },
        CommandDispatchIntent::RefreshOutline { buffer_id },
    ] {
        let language = match app
            .dispatch_ui_intent(intent)
            .expect("read-only language dispatch")
        {
            AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
            other => panic!("expected language projection, got {other:?}"),
        };
        assert_eq!(language.buffer_id, Some(buffer_id));
    }

    assert_eq!(
        app.editor().text(buffer_id).expect("active editor text"),
        original_editor_text
    );
    assert_eq!(
        std::fs::read_to_string(&source).expect("disk source"),
        original_disk_text
    );
}

#[test]
fn language_edit_actions_require_proposals() {
    let workspace = TempWorkspace::new();
    let source = workspace.write("main.rs", "fn old_name() {}\n");
    let (mut app, buffer_id) = open_source_app(&workspace, &source);
    let original_editor_text = app
        .editor()
        .text(buffer_id)
        .expect("active editor text")
        .to_string();
    let original_disk_text = std::fs::read_to_string(&source).expect("disk source");

    for intent in [
        CommandDispatchIntent::RequestFormattingProposal { buffer_id },
        CommandDispatchIntent::RequestRenameProposal {
            buffer_id,
            position: position(3),
            new_name: "new_name".to_string(),
        },
        CommandDispatchIntent::RequestOrganizeImportsProposal { buffer_id },
        CommandDispatchIntent::RequestCodeActionProposal {
            buffer_id,
            action_id: "extract-function".to_string(),
        },
    ] {
        let language = match app
            .dispatch_ui_intent(intent)
            .expect("proposal language dispatch")
        {
            AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
            other => panic!("expected language projection, got {other:?}"),
        };
        let proposal_id = language
            .operations
            .iter()
            .rev()
            .find_map(|operation| operation.proposal_id)
            .expect("language proposal id");
        let shell = app
            .shell_projection_snapshot("language-terminal")
            .expect("shell projection");
        let proposal = shell
            .proposal_ledger_projection
            .rows
            .iter()
            .find(|row| row.proposal_id == proposal_id)
            .expect("proposal ledger row");
        assert_eq!(proposal.payload_kind, ProposalPayloadKind::WorkspaceEdit);
        assert_eq!(proposal.lifecycle.state, ProposalLifecycleState::Previewed);
    }

    assert_eq!(
        app.editor().text(buffer_id).expect("active editor text"),
        original_editor_text
    );
    assert_eq!(
        std::fs::read_to_string(&source).expect("disk source"),
        original_disk_text
    );
}

#[test]
fn terminal_actions_cannot_mutate_editor_or_disk() {
    let workspace = TempWorkspace::new();
    let source = workspace.write("main.rs", "fn old_name() {}\n");
    let (mut app, buffer_id) = open_source_app(&workspace, &source);
    let original_editor_text = app
        .editor()
        .text(buffer_id)
        .expect("active editor text")
        .to_string();
    let original_disk_text = std::fs::read_to_string(&source).expect("disk source");

    let denied = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "fixture".to_string(),
        })
        .expect("default-deny terminal launch");
    let denied_terminal = match denied {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    assert_eq!(denied_terminal.status.kind, TerminalPanelStatusKind::Denied);
    assert!(denied_terminal.active_session_id.is_none());

    app.enable_terminal_fixture_for_tests();
    let launched = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "fixture".to_string(),
        })
        .expect("terminal launch");
    let mut terminal = match launched {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    assert_eq!(terminal.status.kind, TerminalPanelStatusKind::Running);
    let session_id = terminal.active_session_id.expect("active terminal session");

    for intent in [
        CommandDispatchIntent::TerminalInput {
            session_id,
            payload: "echo safe".to_string(),
        },
        CommandDispatchIntent::TerminalOutputPoll { session_id },
        CommandDispatchIntent::TerminalSearch {
            session_id,
            query: "safe".to_string(),
        },
    ] {
        terminal = match app.dispatch_ui_intent(intent).expect("terminal intent") {
            AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
            other => panic!("expected terminal projection, got {other:?}"),
        };
    }
    assert_eq!(terminal.status.kind, TerminalPanelStatusKind::Running);
    assert!(terminal.active_session_id.is_some());
    assert!(!terminal.output_rows.is_empty());
    assert_eq!(
        app.editor().text(buffer_id).expect("active editor text"),
        original_editor_text
    );
    assert_eq!(
        std::fs::read_to_string(&source).expect("disk source"),
        original_disk_text
    );
}

#[test]
fn language_cancellation_and_terminal_denial_are_projected_fail_closed() {
    let workspace = TempWorkspace::new();
    let source = workspace.write("lib.rs", "pub fn item() {}\n");
    let (mut app, _) = open_source_app(&workspace, &source);

    let cancellation = app
        .dispatch_ui_intent(CommandDispatchIntent::CancelLanguageOperation {
            operation_id: "language:test:1".to_string(),
        })
        .expect("cancel language operation");
    let language = match cancellation {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };
    assert_eq!(language.cancellation_count, 1);

    let denied = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "fixture".to_string(),
        })
        .expect("default-deny terminal launch");
    let terminal = match denied {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    assert_eq!(terminal.status.kind, TerminalPanelStatusKind::Denied);
    assert!(terminal.active_session_id.is_none());
    assert!(terminal.last_denial.is_some());
}

#[test]
fn rust_analyzer_runnable_code_lens_launches_the_terminal_fixture() {
    let workspace = TempWorkspace::new();
    let source = workspace.write("lib.rs", "pub fn item() {}\n");
    let mut app = AppComposition::new();
    app.open_workspace(
        &workspace.root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-language-terminal".to_string()),
    )
    .expect("open workspace");
    app.open_file(source.to_string_lossy())
        .expect("open source file");
    app.enable_terminal_fixture_for_tests();

    let buffer_id = app.active_buffer_id().expect("active buffer");
    let code_lens_payload = json!([
        {
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 3}},
            "command": {"title": "Run test", "command": "rust-analyzer.runSingle"},
            "data": {"kind": "runnable"}
        }
    ]);
    let projection = app
        .ingest_lsp_code_lens_response_for_buffer(
            buffer_id,
            &code_lens_payload,
            "rust-analyzer",
            None,
        )
        .expect("ingest code lenses");
    let lens_id = projection
        .code_lenses
        .iter()
        .find(|lens| lens.command_label == "rust-analyzer.runSingle")
        .map(|lens| lens.lens_id.clone())
        .expect("runnable code lens");

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::ActivateLanguageCodeLens { buffer_id, lens_id })
        .expect("activate runnable code lens");
    let terminal = match outcome {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    assert_eq!(terminal.status.kind, TerminalPanelStatusKind::Running);
    assert!(terminal.active_session_id.is_some());
    assert!(terminal.status.message.contains("rust-analyzer.runSingle"));
}

#[test]
fn language_projection_switching_buffers_drops_stale_rows() {
    let workspace = TempWorkspace::new();
    let first = workspace.write("first.rs", "pub fn first_symbol() {}\n");
    let second = workspace.write("second.rs", "pub fn second_symbol() {}\n");
    let (mut app, first_buffer) = open_source_app(&workspace, &first);

    let first_projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshOutline {
            buffer_id: first_buffer,
        })
        .expect("refresh first outline")
    {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };
    assert!(
        !first_projection.outline.is_empty(),
        "test requires a first-buffer outline row to prove stale rows are cleared"
    );

    app.open_file(second.to_string_lossy())
        .expect("open second source file");
    let second_buffer = app.active_buffer_id().expect("second active buffer");
    let second_projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::RequestHover {
            buffer_id: second_buffer,
            position: position(7),
        })
        .expect("hover second buffer")
    {
        AppCommandOutcome::LanguageToolingUpdated(projection) => projection,
        other => panic!("expected language projection, got {other:?}"),
    };
    assert_eq!(second_projection.buffer_id, Some(second_buffer));
    assert!(
        second_projection.outline.is_empty(),
        "outline from the previous buffer must not survive a buffer switch"
    );
    assert!(second_projection.stale_result_count > 0);
}
