use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{PrincipalId, TerminalPanelStatusKind, WorkspaceTrustState};
use legion_ui::CommandDispatchIntent;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-terminal-workflow-{}-{}",
        std::process::id(),
        TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn terminal_denial_is_visible_and_fail_closed() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-terminal".to_string()),
    )
    .expect("open workspace");

    let denied = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "fixture".to_string(),
        })
        .expect("default-denied terminal launch");
    let projection = match denied {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    assert_eq!(projection.status.kind, TerminalPanelStatusKind::Denied);
    assert!(projection.last_denial.is_some());

    let untrusted_root = create_root();
    let mut untrusted = AppComposition::new();
    untrusted
        .open_workspace(
            &untrusted_root,
            WorkspaceTrustState::Untrusted,
            PrincipalId("principal-terminal".to_string()),
        )
        .expect("open untrusted workspace");
    untrusted.enable_terminal_fixture_for_tests();
    let denied = untrusted
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "fixture".to_string(),
        })
        .expect("untrusted terminal launch");
    let projection = match denied {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    assert_eq!(projection.status.kind, TerminalPanelStatusKind::Denied);
    assert!(
        projection
            .last_denial
            .as_deref()
            .unwrap_or_default()
            .contains("untrusted")
    );

    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&untrusted_root).ok();
}

#[test]
fn terminal_fixture_lifecycle_projects_status() {
    let root = create_root();
    let target = root.join("note.txt");
    std::fs::write(&target, "unchanged\n").expect("write fixture file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-terminal".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open fixture file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    let original_text = app
        .editor()
        .text(buffer_id)
        .expect("active buffer text")
        .to_string();
    app.enable_terminal_fixture_for_tests();

    let launched = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "fixture".to_string(),
        })
        .expect("fixture terminal launch");
    let mut projection = match launched {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    assert_eq!(projection.status.kind, TerminalPanelStatusKind::Running);
    let session_id = projection
        .active_session_id
        .expect("active terminal session");

    for intent in [
        CommandDispatchIntent::TerminalInput {
            session_id,
            payload: "echo ready".to_string(),
        },
        CommandDispatchIntent::TerminalResize {
            session_id,
            cols: 100,
            rows: 30,
        },
        CommandDispatchIntent::TerminalOutputPoll { session_id },
        CommandDispatchIntent::TerminalSearch {
            session_id,
            query: "ready".to_string(),
        },
    ] {
        projection = match app.dispatch_ui_intent(intent).expect("terminal intent") {
            AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
            other => panic!("expected terminal projection, got {other:?}"),
        };
    }
    assert!(!projection.output_rows.is_empty());
    assert!(projection.search.match_count > 0);
    assert_eq!(
        app.editor().text(buffer_id).expect("active buffer text"),
        original_text
    );
    assert_eq!(
        std::fs::read_to_string(&target).expect("disk text"),
        "unchanged\n"
    );

    let closed = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalClose { session_id })
        .expect("terminal close");
    let projection = match closed {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    assert_eq!(projection.status.kind, TerminalPanelStatusKind::Exited);
    assert!(projection.active_session_id.is_none());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn terminal_workflow_cannot_mutate_editor_or_disk() {
    let root = create_root();
    let target = root.join("note.txt");
    std::fs::write(&target, "unchanged\n").expect("write fixture file");

    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-terminal".to_string()),
    )
    .expect("open workspace");
    app.open_file(target.to_string_lossy())
        .expect("open fixture file");
    let buffer_id = app.active_buffer_id().expect("active buffer");
    let original_text = app
        .editor()
        .text(buffer_id)
        .expect("active buffer text")
        .to_string();
    app.enable_terminal_fixture_for_tests();

    let launched = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "fixture".to_string(),
        })
        .expect("fixture terminal launch");
    let projection = match launched {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    let session_id = projection
        .active_session_id
        .expect("active terminal session");

    for intent in [
        CommandDispatchIntent::TerminalInput {
            session_id,
            payload: "write forbidden".to_string(),
        },
        CommandDispatchIntent::TerminalResize {
            session_id,
            cols: 120,
            rows: 40,
        },
        CommandDispatchIntent::TerminalKill { session_id },
    ] {
        let _ = app.dispatch_ui_intent(intent).expect("terminal intent");
    }

    assert_eq!(
        app.editor().text(buffer_id).expect("active buffer text"),
        original_text
    );
    assert_eq!(
        std::fs::read_to_string(&target).expect("disk text"),
        "unchanged\n"
    );

    std::fs::remove_dir_all(&root).ok();
}
