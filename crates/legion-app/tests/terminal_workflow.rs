use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{
    PrincipalId, TerminalPanelProjection, TerminalPanelStatusKind, TerminalSessionId,
    WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Drop-guarded temporary workspace root. Removes the directory on drop with a prefix/location
/// check (legion-terminal-workflow- + pid) so a panic mid-test never leaks the temp root.
struct TempWorkspace {
    root: std::path::PathBuf,
}

impl std::ops::Deref for TempWorkspace {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.root
    }
}

impl AsRef<std::path::Path> for TempWorkspace {
    fn as_ref(&self) -> &std::path::Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let prefix = format!("legion-terminal-workflow-{}-", std::process::id());
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with(&prefix))
        {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

fn create_root() -> TempWorkspace {
    let root = std::env::temp_dir().join(format!(
        "legion-terminal-workflow-{}-{}",
        std::process::id(),
        TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    TempWorkspace { root }
}

const TERMINAL_POLL_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// Re-dispatches `TerminalOutputPoll` until `condition` holds or a generous deadline elapses,
/// returning the matching projection. PTY output is asynchronous, so a fixed iteration count
/// can race on a loaded host; on timeout this panics with the last projection for diagnosis.
fn poll_terminal_until(
    app: &mut AppComposition,
    session_id: TerminalSessionId,
    mut condition: impl FnMut(&TerminalPanelProjection) -> bool,
) -> TerminalPanelProjection {
    let deadline = std::time::Instant::now() + TERMINAL_POLL_DEADLINE;
    loop {
        let projection = match app
            .dispatch_ui_intent(CommandDispatchIntent::TerminalOutputPoll { session_id })
            .expect("terminal output poll")
        {
            AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
            other => panic!("expected terminal projection, got {other:?}"),
        };
        if condition(&projection) {
            return projection;
        }
        if std::time::Instant::now() >= deadline {
            panic!(
                "terminal poll timed out after {TERMINAL_POLL_DEADLINE:?}; last projection: {projection:?}"
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
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

    projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalInput {
            session_id,
            payload: "echo ready".to_string(),
        })
        .expect("terminal input")
    {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    assert!(
        projection
            .output_rows
            .iter()
            .any(|row| row.redacted_payload.contains("command block started"))
    );

    match app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalResize {
            session_id,
            cols: 100,
            rows: 30,
        })
        .expect("terminal resize")
    {
        AppCommandOutcome::TerminalPanelUpdated(_) => {}
        other => panic!("expected terminal projection, got {other:?}"),
    };
    let expect_finish_markers = cfg!(unix);
    // Wait until the expected output markers appear before searching; the result is re-fetched
    // by the TerminalSearch dispatch below, so the polled projection itself is discarded.
    let _ = poll_terminal_until(&mut app, session_id, |projection| {
        let has_ready = projection
            .output_rows
            .iter()
            .any(|row| row.redacted_payload.contains("ready"));
        let has_finish = projection
            .output_rows
            .iter()
            .any(|row| row.redacted_payload.contains("command block finished"));
        (expect_finish_markers && has_finish) || (!expect_finish_markers && has_ready)
    });
    projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalSearch {
            session_id,
            query: "ready".to_string(),
        })
        .expect("terminal search")
    {
        AppCommandOutcome::TerminalPanelUpdated(projection) => projection,
        other => panic!("expected terminal projection, got {other:?}"),
    };
    assert!(!projection.output_rows.is_empty());
    assert!(projection.search.match_count > 0);
    let start_index = projection
        .output_rows
        .iter()
        .position(|row| row.redacted_payload.contains("command block started"))
        .expect("command block start row");
    let ready_index = projection
        .output_rows
        .iter()
        .position(|row| row.redacted_payload.contains("ready"))
        .expect("ready output row");
    assert!(start_index < ready_index);
    if expect_finish_markers {
        let finish_index = projection
            .output_rows
            .iter()
            .position(|row| row.redacted_payload.contains("command block finished"))
            .expect("command block finish row");
        assert!(ready_index < finish_index);
        assert!(
            projection
                .output_rows
                .iter()
                .any(|row| row.redacted_payload.contains("command block finished"))
        );
        assert!(
            projection
                .output_rows
                .iter()
                .any(|row| row.redacted_payload.contains("exit=0"))
        );
    }
    assert!(
        projection
            .output_rows
            .iter()
            .any(|row| row.redacted_payload.contains("ready"))
    );
    assert_eq!(
        app.editor().text(buffer_id).expect("active buffer text"),
        original_text
    );
    assert_eq!(
        std::fs::read_to_string(&target).expect("disk text"),
        "unchanged\n"
    );

    assert_eq!(
        projection.status.kind,
        TerminalPanelStatusKind::Running,
        "last_error={:?} output_rows={:?}",
        projection.last_error,
        projection.output_rows
    );
    assert!(projection.active_session_id.is_some());
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
}
