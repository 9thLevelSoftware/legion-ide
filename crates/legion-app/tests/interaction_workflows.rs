//! App-interface contracts: accepted editor actions schedule reads; file activation owns reveal.

use std::time::{Duration, Instant};

use legion_app::{AppCommandOutcome, AppComposition, LspDebounceKind};
use legion_protocol::{
    BufferId, PrincipalId, ProtocolTextRange, TextCoordinate, WorkspaceTrustState,
};
use legion_ui::{CommandDispatchIntent, EditorBoundaryKind};

fn coordinate(column: u32) -> TextCoordinate {
    TextCoordinate {
        line: 0,
        character: column,
        byte_offset: Some(u64::from(column)),
        utf16_offset: Some(u64::from(column)),
    }
}

fn fixture(text: &str) -> (tempfile::TempDir, AppComposition, BufferId) {
    let root = tempfile::tempdir().expect("workspace");
    std::fs::write(root.path().join("first.txt"), text).expect("first file");
    std::fs::write(root.path().join("second.txt"), "second\n").expect("second file");
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("interaction-test".into()),
    )
    .expect("open workspace");
    app.open_file(root.path().join("first.txt").to_string_lossy())
        .expect("open first");
    let buffer = app.active_buffer_id().expect("buffer");
    (root, app, buffer)
}

fn insert(buffer_id: BufferId, at: u32, text: &str) -> CommandDispatchIntent {
    CommandDispatchIntent::Insert {
        buffer_id,
        at: coordinate(at),
        text: text.into(),
    }
}

#[test]
fn completion_waits_50ms_rearms_and_uses_the_accepted_insert_cursor() {
    let (_root, mut app, buffer_id) = fixture("abcd");
    let now = Instant::now();
    app.dispatch_ui_intent_at_for_test(insert(buffer_id, 1, "XY"), now)
        .expect("insert");
    assert!(
        app.tick_lsp_debounces(now + Duration::from_millis(49))
            .is_empty()
    );
    app.dispatch_ui_intent_at_for_test(insert(buffer_id, 3, "Z"), now + Duration::from_millis(49))
        .expect("second insert");
    assert!(
        app.tick_lsp_debounces(now + Duration::from_millis(98))
            .is_empty()
    );
    let due = app.tick_lsp_debounces(now + Duration::from_millis(99));
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].kind, LspDebounceKind::Completion);
    assert_eq!(due[0].buffer_id, buffer_id);
    assert_eq!(due[0].position, coordinate(4));
    assert!(
        app.tick_lsp_debounces(now + Duration::from_secs(1))
            .is_empty()
    );
}

#[test]
fn ordinary_delete_and_directed_edits_use_authoritative_post_edit_carets() {
    let (_root, mut app, buffer_id) = fixture("abcd");
    let now = Instant::now();
    app.dispatch_ui_intent_at_for_test(
        CommandDispatchIntent::Delete {
            buffer_id,
            range: ProtocolTextRange {
                start: coordinate(1),
                end: coordinate(3),
            },
        },
        now,
    )
    .expect("delete range");
    let due = app.tick_lsp_debounces(now + Duration::from_millis(50));
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].position, coordinate(1));
    app.dispatch_ui_intent_at_for_test(
        CommandDispatchIntent::SetDirectedSelection {
            buffer_id,
            anchor: coordinate(0),
            head: coordinate(2),
        },
        now,
    )
    .expect("directed selection");
    app.dispatch_ui_intent_at_for_test(
        CommandDispatchIntent::ReplaceDirectedCarets {
            buffer_id,
            text: "X".into(),
        },
        now,
    )
    .expect("directed replacement");
    let due = app.tick_lsp_debounces(now + Duration::from_secs(1));
    assert_eq!(due.len(), 1, "the pre-edit hover must be invalidated");
    assert_eq!(due[0].kind, LspDebounceKind::Completion);
    assert_eq!(due[0].position, coordinate(1));
    app.dispatch_ui_intent_at_for_test(
        CommandDispatchIntent::DeleteDirectedCarets {
            buffer_id,
            backward: true,
        },
        now,
    )
    .expect("backspace");
    let due = app.tick_lsp_debounces(now + Duration::from_millis(50));
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].position, coordinate(0));
}

#[test]
fn hover_waits_200ms_and_uses_the_editor_resolved_boundary_and_selection_head() {
    let (_root, mut app, buffer_id) = fixture("abcd");
    let now = Instant::now();
    app.dispatch_ui_intent_at_for_test(
        CommandDispatchIntent::SetCursor {
            buffer_id,
            cursor: coordinate(2),
        },
        now,
    )
    .expect("cursor");
    assert!(
        app.tick_lsp_debounces(now + Duration::from_millis(199))
            .is_empty()
    );
    app.dispatch_ui_intent_at_for_test(
        CommandDispatchIntent::MoveToBoundary {
            buffer_id,
            boundary: EditorBoundaryKind::LineEnd,
            extend: true,
        },
        now + Duration::from_millis(199),
    )
    .expect("boundary");
    assert!(
        app.tick_lsp_debounces(now + Duration::from_millis(398))
            .is_empty()
    );
    let due = app.tick_lsp_debounces(now + Duration::from_millis(399));
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].kind, LspDebounceKind::Hover);
    assert_eq!(due[0].position, coordinate(4));
    app.dispatch_ui_intent_at_for_test(
        CommandDispatchIntent::SetDirectedSelection {
            buffer_id,
            anchor: coordinate(4),
            head: coordinate(1),
        },
        now,
    )
    .expect("backward selection");
    let due = app.tick_lsp_debounces(now + Duration::from_millis(200));
    assert_eq!(due[0].position, coordinate(1));
}

#[test]
fn rejected_and_noop_actions_do_not_schedule_interactions() {
    let (_root, mut app, buffer_id) = fixture("abcd");
    let now = Instant::now();
    assert!(
        app.dispatch_ui_intent_at_for_test(insert(BufferId(u128::MAX), 0, "X"), now)
            .is_err()
    );
    assert!(
        app.dispatch_ui_intent_at_for_test(
            CommandDispatchIntent::SetCursor {
                buffer_id,
                cursor: coordinate(99)
            },
            now,
        )
        .is_err()
    );
    let outcome = app
        .dispatch_ui_intent_at_for_test(
            CommandDispatchIntent::DeleteDirectedCarets {
                buffer_id,
                backward: true,
            },
            now,
        )
        .expect("noop backspace");
    assert!(matches!(outcome, AppCommandOutcome::Noop));
    assert!(
        app.tick_lsp_debounces(now + Duration::from_secs(1))
            .is_empty()
    );
}

#[test]
fn switching_away_and_back_or_prompting_dirty_close_invalidates_pending_reads() {
    let (root, mut app, first) = fixture("abcd");
    app.open_file(root.path().join("second.txt").to_string_lossy())
        .expect("second");
    let second = app.active_buffer_id().expect("second buffer");
    app.switch_tab(first).expect("first");
    let now = Instant::now();
    app.dispatch_ui_intent_at_for_test(insert(first, 0, "X"), now)
        .expect("insert");
    app.dispatch_ui_intent_at_for_test(CommandDispatchIntent::SwitchTab { buffer_id: second }, now)
        .expect("away");
    app.dispatch_ui_intent_at_for_test(CommandDispatchIntent::SwitchTab { buffer_id: first }, now)
        .expect("back");
    assert!(
        app.tick_lsp_debounces(now + Duration::from_secs(1))
            .is_empty()
    );
    app.dispatch_ui_intent_at_for_test(insert(first, 0, "Y"), now)
        .expect("insert again");
    let outcome = app
        .dispatch_ui_intent_at_for_test(CommandDispatchIntent::CloseTab { buffer_id: first }, now)
        .expect("dirty prompt");
    assert!(matches!(outcome, AppCommandOutcome::TabClose(_)));
    assert_eq!(app.active_buffer_id(), Some(first));
    assert!(
        app.tick_lsp_debounces(now + Duration::from_secs(1))
            .is_empty()
    );
}

#[test]
fn accepted_caret_movement_and_workspace_change_cannot_fire_old_completion_positions() {
    let (_root, mut app, buffer_id) = fixture("abcd");
    let now = Instant::now();
    app.dispatch_ui_intent_at_for_test(insert(buffer_id, 1, "X"), now)
        .expect("insert");
    app.dispatch_ui_intent_at_for_test(
        CommandDispatchIntent::SetCursor {
            buffer_id,
            cursor: coordinate(0),
        },
        now,
    )
    .expect("move");
    assert!(
        app.tick_lsp_debounces(now + Duration::from_millis(50))
            .is_empty()
    );
    let other = tempfile::tempdir().expect("other workspace");
    app.open_workspace(
        other.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("test".into()),
    )
    .expect("switch workspace");
    assert!(
        app.tick_lsp_debounces(now + Duration::from_secs(1))
            .is_empty()
    );
}

#[test]
fn frame_tick_handles_unavailable_lsp_and_explicit_reads_do_not_rearm() {
    let (_root, mut app, buffer_id) = fixture("abcd");
    let now = Instant::now();
    app.dispatch_ui_intent_at_for_test(insert(buffer_id, 0, "X"), now)
        .expect("insert");
    app.tick_lsp_interactions(now + Duration::from_millis(50));
    assert!(
        app.tick_lsp_debounces(now + Duration::from_secs(1))
            .is_empty()
    );
    for intent in [
        CommandDispatchIntent::RequestCompletion {
            buffer_id,
            position: coordinate(1),
        },
        CommandDispatchIntent::RequestHover {
            buffer_id,
            position: coordinate(1),
        },
    ] {
        let _ = app.dispatch_ui_intent_at_for_test(intent, now);
    }
    assert!(
        app.tick_lsp_debounces(now + Duration::from_secs(1))
            .is_empty()
    );
}

#[test]
fn frame_tick_dispatches_due_reads_through_the_existing_transport_once() {
    use legion_app::LspWorkerRequest;
    use legion_protocol::{
        LanguageId, LanguageServerId, LspCapabilitySummary, LspResultStatus,
        LspServerBinaryProvenance, LspServerHealthRecord,
    };

    let (_root, mut app, buffer_id) = fixture("abcd");
    let now = Instant::now();
    app.dispatch_ui_intent_at_for_test(insert(buffer_id, 0, "X"), now)
        .expect("insert");
    let health = LspServerHealthRecord {
        server_id: LanguageServerId(1),
        language_id: LanguageId("plaintext".into()),
        binary_provenance: LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Fresh,
        capabilities: ["completionProvider", "hoverProvider"]
            .into_iter()
            .map(|name| LspCapabilitySummary {
                capability: name.into(),
                supported: true,
                dynamic_registration: false,
                option_hash: None,
                redaction_hints: Vec::new(),
                schema_version: 1,
            })
            .collect(),
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: 1,
    };
    let requests = app.set_lsp_request_receiver_for_test(health);
    app.tick_lsp_interactions(now + Duration::from_millis(49));
    assert!(requests.try_recv().is_err());
    app.tick_lsp_interactions(now + Duration::from_millis(50));
    let LspWorkerRequest::RequestRead {
        method,
        params,
        tag,
    } = requests.try_recv().expect("completion request")
    else {
        panic!("completion must use the read transport");
    };
    assert_eq!(method, "textDocument/completion");
    assert_eq!(params["position"]["character"], 1);
    assert_eq!(tag.buffer_id, buffer_id);
    app.tick_lsp_interactions(now + Duration::from_secs(1));
    assert!(
        requests.try_recv().is_err(),
        "due reads must not recursively rearm"
    );
    app.dispatch_ui_intent_at_for_test(
        CommandDispatchIntent::SetCursor {
            buffer_id,
            cursor: coordinate(2),
        },
        now,
    )
    .expect("cursor");
    app.tick_lsp_interactions(now + Duration::from_millis(199));
    assert!(requests.try_recv().is_err());
    app.tick_lsp_interactions(now + Duration::from_millis(200));
    let LspWorkerRequest::RequestRead { method, params, .. } =
        requests.try_recv().expect("hover request")
    else {
        panic!("hover must use the read transport");
    };
    assert_eq!(method, "textDocument/hover");
    assert_eq!(params["position"]["character"], 2);
}

#[test]
fn undo_does_not_add_a_new_completion_trigger_and_stale_edits_are_discarded() {
    let (_root, mut app, buffer_id) = fixture("abcd");
    let now = Instant::now();
    app.dispatch_ui_intent_at_for_test(insert(buffer_id, 1, "X"), now)
        .expect("insert");
    app.dispatch_ui_intent_at_for_test(CommandDispatchIntent::Undo { buffer_id }, now)
        .expect("undo");
    assert!(
        app.tick_lsp_debounces(now + Duration::from_secs(1))
            .is_empty()
    );
}

#[test]
fn explorer_activation_opens_then_selects_actual_identity_and_reuses_the_tab() {
    let (root, mut app, _) = fixture("first");
    let path = root.path().join("second.txt");
    let outcome = app
        .activate_explorer_file(path.to_string_lossy())
        .expect("activate");
    let file_id = app.active_file_id().expect("opened identity");
    let buffer_id = app.active_buffer_id().expect("opened buffer");
    assert!(
        matches!(outcome, AppCommandOutcome::ExplorerRefreshed(ref p) if p.selection.as_ref().is_some_and(|s| s.file_id == file_id))
    );
    assert_eq!(app.editor().text(buffer_id).expect("text"), "second\n");
    app.activate_explorer_file(path.to_string_lossy())
        .expect("activate again");
    let snapshot = app.shell_projection_snapshot("test").expect("projection");
    assert_eq!(snapshot.daily_editing_projection.tabs.tabs.len(), 2);
    assert_eq!(
        snapshot
            .explorer_projection
            .selection
            .expect("selection")
            .file_id,
        file_id
    );
}

#[test]
fn failed_explorer_activation_keeps_previous_buffer_dirty_text_and_selection() {
    let (root, mut app, buffer_id) = fixture("first");
    let now = Instant::now();
    app.dispatch_ui_intent_at_for_test(insert(buffer_id, 0, "dirty "), now)
        .expect("edit");
    let before = app.explorer_projection().expect("explorer").selection;
    assert!(
        app.activate_explorer_file(root.path().join("missing.txt").to_string_lossy())
            .is_err()
    );
    assert_eq!(app.active_buffer_id(), Some(buffer_id));
    assert_eq!(app.editor().text(buffer_id).expect("text"), "dirty first");
    assert!(app.editor().is_dirty(buffer_id).expect("dirty"));
    assert_eq!(
        app.explorer_projection().expect("explorer").selection,
        before
    );
}
