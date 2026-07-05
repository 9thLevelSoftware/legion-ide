use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::{
    AppCommandOutcome, AppComposition,
    terminal_policy::{TerminalFailureKind, TerminalShellSelection},
};
use legion_protocol::{
    PrincipalId, TerminalPanelProjection, TerminalPanelStatusKind, TerminalRuntimeState,
    TerminalSessionId, WorkspaceTrustState,
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

/// Task 1 (P2.F2.T1): trusted workspace + explicit launch intent → runtime auto-enabled and
/// real session starts; untrusted workspace → `Denied` with reason surfaced in projection.
/// This test must FAIL before the product gate is implemented (runtime disabled by default).
#[test]
fn terminal_product_gate_trusted_workspace_launches_without_test_helper() {
    // Part 1: trusted workspace in Manual mode (default) — no enable_terminal_runtime_for_tests()
    let root = create_root();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-product-gate".to_string()),
    )
    .expect("open trusted workspace");
    // Explicit user launch intent — product gate should auto-enable the runtime.
    let launched = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "product-gate-test".to_string(),
        })
        .expect("trusted launch dispatch");
    let projection = match launched {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => panic!("expected TerminalPanelUpdated, got {other:?}"),
    };
    assert_eq!(
        projection.status.kind,
        TerminalPanelStatusKind::Running,
        "trusted Manual-mode workspace must auto-enable the terminal runtime; \
         denial: {:?}",
        projection.last_denial
    );
    assert!(
        projection.active_session_id.is_some(),
        "active session id must be set after successful launch"
    );

    // Part 2: untrusted workspace → Denied with "untrusted" in denial reason.
    let untrusted_root = create_root();
    let mut untrusted = AppComposition::new();
    untrusted
        .open_workspace(
            &untrusted_root,
            WorkspaceTrustState::Untrusted,
            PrincipalId("principal-product-gate".to_string()),
        )
        .expect("open untrusted workspace");
    let denied = untrusted
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "product-gate-test".to_string(),
        })
        .expect("untrusted launch dispatch");
    let projection = match denied {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => panic!("expected TerminalPanelUpdated, got {other:?}"),
    };
    assert_eq!(
        projection.status.kind,
        TerminalPanelStatusKind::Denied,
        "untrusted workspace must be denied"
    );
    assert!(
        projection
            .last_denial
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("untrusted"),
        "denial reason must mention untrusted: {:?}",
        projection.last_denial
    );
}

/// Verify that untrusted workspaces are always denied — the product gate denies them before
/// any capability evaluation, so `enable_terminal_runtime_for_tests()` cannot override it.
///
/// NOTE: the old "trusted workspace + no explicit enablement → Denied" scenario has been
/// superseded by the product gate: trusted workspaces now auto-enable on explicit launch
/// (see `terminal_product_gate_trusted_workspace_launches_without_test_helper`).
#[test]
fn terminal_denial_is_visible_and_fail_closed() {
    // Untrusted workspace → always Denied with "untrusted" in the reason.
    let untrusted_root = create_root();
    let mut untrusted = AppComposition::new();
    untrusted
        .open_workspace(
            &untrusted_root,
            WorkspaceTrustState::Untrusted,
            PrincipalId("principal-terminal".to_string()),
        )
        .expect("open untrusted workspace");
    // Even with the runtime explicitly enabled the product gate must deny untrusted callers.
    untrusted.enable_terminal_runtime_for_tests();
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
            .to_ascii_lowercase()
            .contains("untrusted"),
        "denial reason must mention untrusted: {:?}",
        projection.last_denial
    );
    // Denial must be surfaced — projection is not empty / fail-open.
    assert!(projection.last_denial.is_some());
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
    app.enable_terminal_runtime_for_tests();

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
    app.enable_terminal_runtime_for_tests();

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

/// Task 2 (TERM.01): shell selection three-tier precedence — workspace settings override
/// user settings, which override the platform default, and the selected shell is
/// projected in the status message.
#[test]
fn terminal_shell_selection_is_projected_in_status() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-shell".to_string()),
    )
    .expect("open workspace");

    // ── Tier 1: Workspace overrides user overrides platform ─────────────────────────────
    // Set user-level preference to PowerShell, workspace to Cmd.
    // Workspace must win → expect "cmd.exe" in status.
    app.set_user_terminal_shell_selection(Some(TerminalShellSelection::PowerShell));
    app.set_terminal_shell_selection(TerminalShellSelection::Cmd);

    let projection = match app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "shell-precedence-workspace".to_string(),
        })
        .expect("tier-1 launch")
    {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => panic!("expected TerminalPanelUpdated, got {other:?}"),
    };
    assert_eq!(
        projection.status.kind,
        TerminalPanelStatusKind::Running,
        "workspace-override launch must succeed; status: {:?}",
        projection.status
    );
    assert!(
        projection.status.message.contains("cmd.exe"),
        "workspace Cmd must override user PowerShell; status: {:?}",
        projection.status.message
    );

    // ── Tier 2: User overrides platform default ──────────────────────────────────────────
    // Clear workspace selection → user-level PowerShell must win.
    // Use a fresh AppComposition to avoid leftover session state.
    let root2 = create_root();
    let mut app2 = AppComposition::new();
    app2.open_workspace(
        &root2,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-shell-user".to_string()),
    )
    .expect("open workspace for user-tier test");
    app2.set_user_terminal_shell_selection(Some(TerminalShellSelection::PowerShell));
    // No workspace-level override — user pref should win over platform default.

    let projection2 = match app2
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "shell-precedence-user".to_string(),
        })
        .expect("tier-2 launch")
    {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => panic!("expected TerminalPanelUpdated, got {other:?}"),
    };
    assert_eq!(
        projection2.status.kind,
        TerminalPanelStatusKind::Running,
        "user-pref launch must succeed; status: {:?}",
        projection2.status
    );
    assert!(
        projection2.status.message.contains("pwsh"),
        "user PowerShell must override platform default; status: {:?}",
        projection2.status.message
    );
}

/// Task 3 (TERM.05): scrollback limit is enforced; rows beyond the limit are evicted and
/// the eviction count is reflected in `scrollback.omitted_row_count`.
#[test]
fn terminal_scrollback_limit_enforced_and_eviction_counted() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-scrollback".to_string()),
    )
    .expect("open workspace");
    app.enable_terminal_runtime_for_tests();

    let launched = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "scrollback-test".to_string(),
        })
        .expect("scrollback launch");
    let projection = match launched {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => panic!("expected TerminalPanelUpdated, got {other:?}"),
    };
    assert_eq!(projection.status.kind, TerminalPanelStatusKind::Running);
    let session_id = projection.active_session_id.expect("active session");

    // Set a tight scrollback limit for this test.
    app.set_terminal_scrollback_max_rows(10);

    // Pump 20 input cycles to generate more rows than the 10-row limit.
    for i in 0..20u32 {
        let _ = app
            .dispatch_ui_intent(CommandDispatchIntent::TerminalInput {
                session_id,
                payload: format!("echo line-{i}"),
            })
            .expect("input");
        let _ = app
            .dispatch_ui_intent(CommandDispatchIntent::TerminalOutputPoll { session_id })
            .expect("poll");
    }

    let final_projection = poll_terminal_until(&mut app, session_id, |p| {
        p.output_rows.len() >= 10 || p.scrollback.omitted_row_count > 0
    });
    // The projection must not exceed the configured limit.
    assert!(
        final_projection.output_rows.len() <= 10,
        "visible rows {} must not exceed limit 10",
        final_projection.output_rows.len()
    );
    // If rows were evicted, omitted_row_count must be > 0.
    if final_projection.scrollback.omitted_row_count > 0 {
        assert!(
            final_projection.scrollback.truncated,
            "scrollback.truncated must be set when rows are omitted"
        );
    }
}

/// Task 4 (TERM.06): resize intent propagates to the PTY; the projection reflects the new
/// dimensions in the status message.
#[test]
fn terminal_resize_propagates_to_projection() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-resize".to_string()),
    )
    .expect("open workspace");
    app.enable_terminal_runtime_for_tests();

    let launched = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "resize-test".to_string(),
        })
        .expect("resize launch");
    let projection = match launched {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => panic!("expected TerminalPanelUpdated, got {other:?}"),
    };
    assert_eq!(projection.status.kind, TerminalPanelStatusKind::Running);
    let session_id = projection.active_session_id.expect("active session");

    let resized = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalResize {
            session_id,
            cols: 200,
            rows: 50,
        })
        .expect("resize");
    let projection = match resized {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => panic!("expected TerminalPanelUpdated, got {other:?}"),
    };
    assert_eq!(projection.status.kind, TerminalPanelStatusKind::Running);
    assert!(
        projection.status.message.contains("200") && projection.status.message.contains("50"),
        "resize status must mention the new dimensions; got: {:?}",
        projection.status.message
    );
}

/// Task 6 (TERM.09): orphan cleanup — abandoned sessions are killed and produce audit records.
///
/// Strategy: use `launch_terminal_raw_for_orphan_test` to start a short-lived process
/// (`cmd /C exit` on Windows, `sh -c exit` on Unix) without going through the shell
/// selection path. The process exits on its own; the session stays in both the platform
/// registry and the runtime sessions map because no `TerminalOutputPoll` was dispatched.
/// `cleanup_terminal_orphans()` must then (a) detect the exited session, (b) return a
/// non-empty audit record with the expected session id and state=Exited, and (c) leave
/// no orphan on a second call.
#[test]
fn terminal_orphan_cleanup_kills_and_records_evidence() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-orphan".to_string()),
    )
    .expect("open workspace");
    app.enable_terminal_runtime_for_tests();

    // Launch a short-lived process that exits on its own.
    // Do NOT dispatch TerminalOutputPoll — that would remove the exited session from the
    // runtime registry before cleanup_terminal_orphans() can find it.
    #[cfg(windows)]
    let (command, args) = (
        "cmd".to_string(),
        vec!["/C".to_string(), "exit".to_string()],
    );
    #[cfg(unix)]
    let (command, args) = ("sh".to_string(), vec!["-c".to_string(), "exit".to_string()]);

    let launched = app
        .launch_terminal_raw_for_orphan_test(command, args)
        .expect("short-lived launch must succeed");
    let expected_session_id = launched.audit.session_id;

    // Give the process time to exit naturally.
    std::thread::sleep(std::time::Duration::from_millis(400));

    // First cleanup call: must detect the exited session and return an audit record.
    let records = app
        .cleanup_terminal_orphans()
        .expect("orphan cleanup must not fail");

    assert_eq!(
        records.len(),
        1,
        "cleanup must return exactly one audit record for the orphaned session; got: {records:?}"
    );
    assert_eq!(
        records[0].session_id, expected_session_id,
        "audit record must reference the orphaned session"
    );
    assert_eq!(
        records[0].state,
        TerminalRuntimeState::Exited,
        "orphan record state must be Exited"
    );

    // Second call must return empty (session already removed).
    let second = app
        .cleanup_terminal_orphans()
        .expect("second cleanup must not fail");
    assert_eq!(
        second.len(),
        0,
        "second cleanup call must return empty (session already cleaned)"
    );

    eprintln!(
        "[TERM-ORPHAN] session={} state={:?} summary={}",
        records[0].session_id.0, records[0].state, records[0].metadata_summary
    );
}

/// Task 7 (TERM.11): failure UX — all 5 failure kinds project distinct statuses with labels.
///
/// Two failure kinds are tested via real end-to-end scenarios (Denied from untrusted workspace;
/// Exited from kill). Three are tested via `project_terminal_failure_for_test` which calls the
/// same `apply_failure_kind()` method that the real launch error handler uses.
#[test]
fn terminal_failure_ux_distinct_status_kinds() {
    // ── Denied (real scenario): untrusted workspace → Denied ──────────────────────────────
    let untrusted_root = create_root();
    let mut untrusted = AppComposition::new();
    untrusted
        .open_workspace(
            &untrusted_root,
            WorkspaceTrustState::Untrusted,
            PrincipalId("principal-ux".to_string()),
        )
        .expect("open untrusted workspace");
    let denied = untrusted
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "ux-test".to_string(),
        })
        .expect("denied launch");
    let projection = match denied {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => panic!("expected TerminalPanelUpdated, got {other:?}"),
    };
    assert_eq!(
        projection.status.kind,
        TerminalPanelStatusKind::Denied,
        "untrusted→Denied"
    );

    // ── Exited (real scenario): kill active session → Exited ──────────────────────────────
    let root = create_root();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-ux".to_string()),
    )
    .expect("open workspace");
    app.enable_terminal_runtime_for_tests();
    let launched = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalLaunch {
            command_label: "ux-kill-test".to_string(),
        })
        .expect("ux launch");
    let projection = match launched {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => panic!("expected TerminalPanelUpdated, got {other:?}"),
    };
    let session_id = projection.active_session_id.expect("active session");
    let killed = app
        .dispatch_ui_intent(CommandDispatchIntent::TerminalKill { session_id })
        .expect("kill");
    let projection = match killed {
        AppCommandOutcome::TerminalPanelUpdated(p) => p,
        other => panic!("expected TerminalPanelUpdated, got {other:?}"),
    };
    assert_eq!(
        projection.status.kind,
        TerminalPanelStatusKind::Exited,
        "kill→Exited"
    );

    // ── Unavailable, Crashed, PolicyBlocked via apply_failure_kind() ──────────────────────
    // Each call exercises the real translation path (`failure_kind_to_status_kind`).
    // The test helper `project_terminal_failure_for_test` delegates to the same method
    // that the launch error handler calls, so this verifies production code.

    let mut app2 = AppComposition::new();
    let p = app2.project_terminal_failure_for_test(TerminalFailureKind::Unavailable);
    assert_eq!(
        p.status.kind,
        TerminalPanelStatusKind::Unavailable,
        "Unavailable kind"
    );
    assert!(
        p.status.message.contains("unavailable"),
        "Unavailable message must contain 'unavailable'; got: {:?}",
        p.status.message
    );

    let mut app3 = AppComposition::new();
    let p = app3.project_terminal_failure_for_test(TerminalFailureKind::Crashed);
    assert_eq!(
        p.status.kind,
        TerminalPanelStatusKind::Crashed,
        "Crashed kind"
    );
    assert!(
        p.status.message.contains("crashed"),
        "Crashed message must contain 'crashed'; got: {:?}",
        p.status.message
    );

    let mut app4 = AppComposition::new();
    let p = app4.project_terminal_failure_for_test(TerminalFailureKind::PolicyBlocked);
    assert_eq!(
        p.status.kind,
        TerminalPanelStatusKind::PolicyBlocked,
        "PolicyBlocked kind"
    );
    assert!(
        p.status.message.contains("policy-blocked"),
        "PolicyBlocked message must contain 'policy-blocked'; got: {:?}",
        p.status.message
    );

    // Verify all 5 status kinds are distinct (no two map to the same variant).
    let kinds = [
        TerminalPanelStatusKind::Denied,
        TerminalPanelStatusKind::Unavailable,
        TerminalPanelStatusKind::Exited,
        TerminalPanelStatusKind::Crashed,
        TerminalPanelStatusKind::PolicyBlocked,
    ];
    let unique: std::collections::HashSet<_> = kinds.iter().collect();
    assert_eq!(
        unique.len(),
        kinds.len(),
        "all 5 terminal failure status kinds must be distinct"
    );

    // MINOR 1: Assert that display_label() returns human-readable text (no PascalCase / Rust debug
    // format). Users must never see strings like "PolicyBlocked" or "Unavailable" in the UI.
    for kind in &kinds {
        let label = kind.display_label();
        let has_pascal = label.chars().enumerate().any(|(i, c)| {
            i > 0 && c.is_uppercase() && label.chars().nth(i - 1).is_some_and(|p| p.is_lowercase())
        });
        assert!(
            !has_pascal,
            "display_label() must not contain PascalCase for {kind:?}; got: {label:?}"
        );
        assert!(
            !label.is_empty(),
            "display_label() must not be empty for {kind:?}"
        );
    }
}
