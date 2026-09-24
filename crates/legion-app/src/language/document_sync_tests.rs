//! Private document-sync ledger tests using the production request queue.

use std::{collections::HashSet, time::Duration};

use crate::CommandDispatchIntent;
use crate::language::LspWorkerRequest;
use legion_protocol::{
    LanguageId, LanguageServerId, LspCapabilitySummary, LspResultStatus, PrincipalId,
    WorkspaceTrustState,
};

fn health() -> legion_protocol::LspServerHealthRecord {
    legion_protocol::LspServerHealthRecord {
        server_id: LanguageServerId(1),
        language_id: LanguageId("rust".into()),
        binary_provenance: legion_protocol::LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Fresh,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: 1,
    }
}

fn insert_at_start(
    app: &mut crate::AppComposition,
    buffer_id: legion_protocol::BufferId,
    text: &str,
) {
    let coordinate = legion_protocol::TextCoordinate {
        line: 0,
        character: 0,
        byte_offset: None,
        utf16_offset: None,
    };
    app.dispatch_ui_intent(CommandDispatchIntent::Replace {
        buffer_id,
        range: legion_protocol::ProtocolTextRange {
            start: coordinate,
            end: coordinate,
        },
        replacement: text.to_string(),
    })
    .expect("dispatch editor replacement");
}

fn opened_buffer(request: LspWorkerRequest) -> legion_protocol::BufferId {
    match request {
        LspWorkerRequest::DidOpen { buffer_id, .. } => buffer_id,
        LspWorkerRequest::DidOpenDeferred {
            buffer_id, text_rx, ..
        } => {
            assert!(
                text_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("deferred didOpen payload")
                    .is_some()
            );
            buffer_id
        }
        _ => panic!("expected didOpen request"),
    }
}

fn changed_payload(request: LspWorkerRequest) -> (String, i64) {
    match request {
        LspWorkerRequest::DidChange { text, version, .. } => (text, version),
        LspWorkerRequest::DidChangeDeferred {
            version, text_rx, ..
        } => (
            text_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("deferred didChange payload")
                .expect("deferred didChange text"),
            version,
        ),
        _ => panic!("expected didChange request"),
    }
}

#[test]
fn document_sync_ledger_coalesces_edits_and_preserves_close_before_reopen() {
    let root = tempfile::tempdir().expect("workspace");
    let first = root.path().join("first.rs");
    let second = root.path().join("second.rs");
    let third = root.path().join("third.rs");
    for path in [&first, &second, &third] {
        std::fs::write(path, "fn main() {}\n").expect("seed source");
    }

    let mut app = crate::AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("document-sync-test".into()),
    )
    .expect("workspace");
    app.open_file(first.to_string_lossy()).expect("first");
    let first_buffer = app.active_buffer_id().expect("first buffer");
    app.open_file(second.to_string_lossy()).expect("second");
    let second_buffer = app.active_buffer_id().expect("second buffer");
    app.open_file(third.to_string_lossy()).expect("third");
    let third_buffer = app.active_buffer_id().expect("third buffer");

    let (receiver, _result_guard) = app.set_lsp_request_harness_for_test(health());
    for buffer in [first_buffer, second_buffer, third_buffer] {
        app.notify_lsp_did_open(buffer);
    }
    let mut opened = HashSet::new();
    for _ in 0..3 {
        opened.insert(opened_buffer(
            receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("didOpen"),
        ));
        app.drain_lsp_session();
    }
    assert_eq!(opened.len(), 3, "every open buffer must be synchronized");

    app.switch_tab(first_buffer).expect("switch first");
    insert_at_start(&mut app, first_buffer, "// one\n");
    let latest_text = app
        .buffer_text_for_input(first_buffer)
        .expect("latest text");
    insert_at_start(&mut app, first_buffer, "// two\n");
    assert!(!app.lsp_document_sync_ready(first_buffer));
    let first_change = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("first didChange");
    app.drain_lsp_session();
    let latest_change = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("latest didChange");
    let _ = changed_payload(first_change);
    let (latest_text_payload, latest_version) = changed_payload(latest_change);
    assert_eq!(
        latest_text_payload,
        app.buffer_text_for_input(first_buffer)
            .expect("latest text")
    );
    assert_eq!(
        latest_version,
        app.editor
            .current_snapshot(first_buffer)
            .expect("latest snapshot")
            .buffer_version
            .0 as i64
    );
    assert!(latest_text != app.buffer_text_for_input(first_buffer).expect("new text"));
    assert!(app.lsp_document_sync_ready(first_buffer));
    assert!(
        app.document_sync_ledger
            .values()
            .all(|entries| entries.len() <= 2)
    );

    drop(receiver);
    app.switch_tab(second_buffer).expect("switch second");
    let second_uri = crate::canonical_path_to_uri(
        &app.active_documents
            .metadata_for_buffer(second_buffer)
            .expect("second buffer metadata")
            .identity
            .canonical_path
            .0,
    );
    let (offline_receiver, offline_result_guard) = app.set_lsp_request_harness_for_test(health());
    drop(offline_receiver);
    drop(offline_result_guard);
    let _ = app.close_tab(second_buffer).expect("close second");
    assert!(app.document_sync_ledger.contains_key(&second_uri));
    app.open_file(second.to_string_lossy())
        .expect("reopen second");
    let reopened = app.active_buffer_id().expect("reopened second");
    assert_ne!(reopened, second_buffer);
    let (second_receiver, _second_result_guard) = app.set_lsp_request_harness_for_test(health());
    app.drain_lsp_session();
    app.notify_lsp_did_open(reopened);
    app.flush_document_sync_ledger();
    let close = second_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("queued close");
    assert!(matches!(close, LspWorkerRequest::DidClose { .. }));
    app.drain_lsp_session();
    let open = second_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("queued reopen");
    assert_eq!(opened_buffer(open), reopened);
    assert!(
        app.document_sync_ledger
            .values()
            .all(|entries| entries.len() <= 2)
    );
}

#[test]
fn workspace_rename_waits_for_every_open_document_to_enter_the_worker_queue() {
    let root = tempfile::tempdir().expect("workspace");
    let first = root.path().join("first.rs");
    let second = root.path().join("second.rs");
    std::fs::write(&first, "fn first() {}\n").expect("seed first source");
    std::fs::write(&second, "fn second() {}\n").expect("seed second source");

    let mut app = crate::AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("document-sync-rename-barrier-test".into()),
    )
    .expect("workspace");
    app.open_file(first.to_string_lossy()).expect("first");
    let first_buffer = app.active_buffer_id().expect("first buffer");
    app.open_file(second.to_string_lossy()).expect("second");
    let second_buffer = app.active_buffer_id().expect("second buffer");

    let mut lsp_health = health();
    lsp_health.capabilities.push(LspCapabilitySummary {
        capability: "renameProvider".into(),
        supported: true,
        dynamic_registration: false,
        option_hash: None,
        redaction_hints: Vec::new(),
        schema_version: 1,
    });
    let (receiver, _result_guard) = app.set_lsp_request_harness_for_test(lsp_health);
    app.notify_lsp_did_open(first_buffer);
    app.notify_lsp_did_open(second_buffer);
    assert_eq!(
        opened_buffer(
            receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("first didOpen")
        ),
        first_buffer
    );
    assert!(!app.lsp_workspace_sync_ready());

    app.switch_tab(first_buffer).expect("switch first");
    app.dispatch_ui_intent(CommandDispatchIntent::RequestRenameProposal {
        buffer_id: first_buffer,
        position: legion_protocol::TextCoordinate {
            line: 0,
            character: 3,
            byte_offset: None,
            utf16_offset: None,
        },
        new_name: "renamed".into(),
    })
    .expect("defer rename until workspace sync");
    assert!(
        receiver.try_recv().is_err(),
        "rename overtook pending didOpen"
    );

    app.flush_document_sync_ledger();
    assert_eq!(
        opened_buffer(
            receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("second didOpen")
        ),
        second_buffer
    );
    app.drain_lsp_session();
    assert!(app.lsp_workspace_sync_ready());
    let rename = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("rename request after workspace sync");
    assert!(matches!(
        rename,
        LspWorkerRequest::RequestRead { tag, .. }
            if matches!(tag.kind, crate::language::LspReadKind::Rename { .. })
    ));
    assert!(
        receiver.try_recv().is_err(),
        "expected exactly one rename request"
    );
}

#[test]
fn repeated_offline_close_reopen_keeps_one_uri_ledger_bounded() {
    let root = tempfile::tempdir().expect("workspace");
    let source = root.path().join("repeated.rs");
    std::fs::write(&source, "fn main() {}\n").expect("seed source");
    let mut app = crate::AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("document-sync-repeat-test".into()),
    )
    .expect("workspace");
    app.open_file(source.to_string_lossy())
        .expect("open source");
    let mut current = app.active_buffer_id().expect("initial buffer");
    let mut seen = HashSet::new();
    seen.insert(current);

    for _ in 0..32 {
        let (receiver, result_guard) = app.set_lsp_request_harness_for_test(health());
        drop(receiver);
        drop(result_guard);
        let close = app.close_tab(current).expect("offline close");
        assert!(matches!(
            close,
            crate::AppCloseTabOutcome::Closed { buffer_id } if buffer_id == current
        ));
        assert!(
            app.document_sync_ledger
                .values()
                .all(|entries| entries.len() <= 2)
        );
        app.open_file(source.to_string_lossy())
            .expect("reopen source");
        current = app.active_buffer_id().expect("reopened buffer");
        assert!(
            seen.insert(current),
            "reopen must allocate a fresh buffer identity"
        );
        assert!(
            app.document_sync_ledger
                .values()
                .all(|entries| entries.len() <= 2)
        );
    }

    let (receiver, _result_guard) = app.set_lsp_request_harness_for_test(health());
    app.notify_lsp_did_open(current);
    app.flush_document_sync_ledger();
    let close = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("oldest close after offline cycles");
    assert!(matches!(close, LspWorkerRequest::DidClose { .. }));
    app.drain_lsp_session();
    let open = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("latest reopen after offline cycles");
    assert_eq!(opened_buffer(open), current);
    assert!(
        app.document_sync_ledger
            .values()
            .all(|entries| entries.len() <= 2)
    );
}

#[test]
fn direct_edit_active_buffer_queues_current_did_change_after_open() {
    let root = tempfile::tempdir().expect("workspace");
    let source = root.path().join("direct-edit.rs");
    std::fs::write(&source, "fn main() {}\n").expect("seed source");
    let mut app = crate::AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("document-sync-direct-edit-test".into()),
    )
    .expect("workspace");
    app.open_file(source.to_string_lossy())
        .expect("open source");
    let buffer = app.active_buffer_id().expect("buffer");
    let (receiver, _result_guard) = app.set_lsp_request_harness_for_test(health());
    app.notify_lsp_did_open(buffer);
    assert_eq!(
        opened_buffer(
            receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("didOpen")
        ),
        buffer
    );

    app.edit_active_buffer(legion_editor::TextEdit::insert(
        legion_editor::TextPosition::new(0, 0),
        "// direct\n",
    ))
    .expect("direct edit");
    let expected_text = app.buffer_text_for_input(buffer).expect("edited text");
    let expected_version = app
        .editor
        .current_snapshot(buffer)
        .expect("edited snapshot")
        .buffer_version
        .0 as i64;
    let change = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("direct edit didChange");
    let (text, version) = changed_payload(change);
    assert_eq!(text, expected_text);
    assert_eq!(version, expected_version);
}

#[test]
fn replace_all_and_clipboard_cut_emit_current_did_changes() {
    let root = tempfile::tempdir().expect("workspace");
    let source = root.path().join("commands.rs");
    std::fs::write(&source, "one one\n").expect("seed source");
    let mut app = crate::AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("document-sync-command-test".into()),
    )
    .expect("workspace");
    app.open_file(source.to_string_lossy())
        .expect("open source");
    let buffer = app.active_buffer_id().expect("buffer");
    let (receiver, _result_guard) = app.set_lsp_request_harness_for_test(health());
    app.notify_lsp_did_open(buffer);
    let _ = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("didOpen");

    app.dispatch_ui_intent(CommandDispatchIntent::SetFindQuery {
        query: "one".into(),
    })
    .expect("find query");
    app.dispatch_ui_intent(CommandDispatchIntent::SetFindReplaceText { text: "X".into() })
        .expect("replace text");
    app.dispatch_ui_intent(CommandDispatchIntent::ReplaceAll)
        .expect("replace all");
    let replace = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("replace-all didChange");
    assert_eq!(changed_payload(replace).0, "X X\n");

    let coordinate = |character| legion_protocol::TextCoordinate {
        line: 0,
        character,
        byte_offset: None,
        utf16_offset: None,
    };
    app.dispatch_ui_intent(CommandDispatchIntent::SetDirectedSelection {
        buffer_id: buffer,
        anchor: coordinate(0),
        head: coordinate(1),
    })
    .expect("select replacement");
    app.dispatch_ui_intent(CommandDispatchIntent::ClipboardCut { buffer_id: buffer })
        .expect("clipboard cut");
    let cut = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("clipboard-cut didChange");
    assert_eq!(changed_payload(cut).0, " X\n");
}
