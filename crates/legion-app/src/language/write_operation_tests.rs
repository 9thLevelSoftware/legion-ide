//! Production-queue tests for write-side LSP operations.
//!
//! Every response below is admitted through the real request queue and carries
//! the operation tag returned by that queue.  The tests therefore exercise the
//! pending ledger and drain-side fencing rather than calling an ingest helper.

use std::sync::mpsc::{Receiver, SyncSender};
use std::time::Duration;

use crate::AppComposition;
use crate::language::{
    LanguageSessionError, LspReadKind, LspReadOutcome, LspRequestTag, LspWorkerRequest,
    LspWorkerResult,
};
use legion_protocol::{
    BufferId, LanguageId, LanguageServerId, LanguageToolingStatusKind, LspCapabilitySummary,
    LspResultStatus, LspServerBinaryProvenance, LspServerHealthRecord, PrincipalId, TextCoordinate,
    WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

fn health(capabilities: &[&str]) -> LspServerHealthRecord {
    LspServerHealthRecord {
        server_id: LanguageServerId(1),
        language_id: LanguageId("rust".to_string()),
        binary_provenance: LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Fresh,
        capabilities: capabilities
            .iter()
            .map(|capability| LspCapabilitySummary {
                capability: (*capability).to_string(),
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
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    app: AppComposition,
    buffer: BufferId,
    requests: Receiver<LspWorkerRequest>,
    results: SyncSender<LspWorkerResult>,
}

fn fixture_with_sync(capabilities: &[&str], synchronize_document: bool) -> Fixture {
    let root = tempfile::tempdir().expect("workspace");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"write-operations\"\n",
    )
    .expect("manifest");
    let source = root.path().join("main.rs");
    std::fs::write(&source, "fn main() {}\n").expect("source");

    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("write-operation-tests".to_string()),
    )
    .expect("workspace");
    app.open_file(source.to_string_lossy())
        .expect("source open");
    let buffer = app.active_buffer_id().expect("active buffer");
    let (requests, results) = app.set_lsp_request_harness_for_test(health(capabilities));

    if synchronize_document {
        // Admit the initial didOpen through the same production queue.  This
        // is required before a write request can pass document-sync readiness.
        app.notify_lsp_did_open(buffer);
        match requests
            .recv_timeout(Duration::from_secs(1))
            .expect("didOpen request")
        {
            LspWorkerRequest::DidOpenDeferred { text_rx, .. } => {
                assert!(
                    text_rx
                        .recv_timeout(Duration::from_secs(1))
                        .expect("didOpen text")
                        .is_some()
                );
            }
            _ => panic!("expected deferred didOpen request"),
        }
    }

    Fixture {
        _root: root,
        app,
        buffer,
        requests,
        results,
    }
}

fn fixture(capabilities: &[&str]) -> Fixture {
    fixture_with_sync(capabilities, true)
}

fn issue_formatting(fixture: &mut Fixture) -> LspRequestTag {
    assert!(fixture.app.issue_lsp_formatting_request(fixture.buffer));
    let tag = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("formatting request")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected formatting read request"),
    };
    let context = tag
        .operation_context
        .as_ref()
        .expect("write admission context");
    let metadata = fixture
        .app
        .active_documents
        .metadata_for_buffer(fixture.buffer)
        .expect("buffer metadata");
    let workspace_id = fixture
        .app
        .active_documents
        .workspace_id()
        .expect("workspace");
    let snapshot = fixture
        .app
        .editor
        .current_snapshot(fixture.buffer)
        .expect("snapshot");
    let version = fixture
        .app
        .editor
        .buffer_version(fixture.buffer)
        .expect("buffer version");
    assert_eq!(context.workspace_id, workspace_id);
    assert_eq!(context.file_id, metadata.identity.file_id);
    assert_eq!(context.buffer_id, fixture.buffer);
    assert_eq!(context.snapshot_id, snapshot.snapshot_id);
    assert_eq!(context.buffer_version, version);
    assert_eq!(context.language_id, LanguageId("rust".to_string()));
    assert_ne!(context.correlation_id.0, 0);
    assert!(!context.causality_id.0.is_nil());
    assert!(context.content_hash.is_none());
    tag
}

fn formatting_response(_uri: &str) -> serde_json::Value {
    serde_json::json!([{
        "range": {
            "start": {"line": 0, "character": 0},
            "end": {"line": 0, "character": 0}
        },
        "newText": "// formatted\n"
    }])
}

fn send_response(fixture: &mut Fixture, tag: LspRequestTag, result: serde_json::Value) {
    fixture
        .results
        .send(LspWorkerResult::ReadResult {
            outcome: Ok(LspReadOutcome {
                result,
                issued_snapshot: tag.snapshot_id,
                status: LspResultStatus::Fresh,
            }),
            tag,
        })
        .expect("worker result");
    fixture.app.drain_lsp_session();
}

fn operation(
    fixture: &Fixture,
    operation_id: &str,
) -> Vec<legion_protocol::LanguageToolingOperationProjection> {
    fixture
        .app
        .language_tooling_projection()
        .operations
        .into_iter()
        .filter(|row| row.operation_id == operation_id)
        .collect()
}

#[test]
fn accepted_format_is_running_with_same_operation_and_no_proposal_until_response() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    let tag = issue_formatting(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("write operation id");

    assert_eq!(fixture.app.pending_lsp_writes.len(), 1);
    assert_eq!(
        fixture.app.pending_lsp_writes[&operation_id].operation_id,
        operation_id
    );
    let rows = operation(&fixture, &operation_id);
    assert_eq!(
        rows.len(),
        1,
        "accepted write must publish one terminal-tracked row"
    );
    assert_eq!(rows[0].status, LanguageToolingStatusKind::Running);
    assert!(rows[0].proposal_id.is_none());
}

#[test]
fn unavailable_format_has_no_pending_request_or_proposal() {
    let mut fixture = fixture(&[]);
    assert!(!fixture.app.issue_lsp_formatting_request(fixture.buffer));
    assert!(fixture.app.pending_lsp_writes.is_empty());
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .operations
            .iter()
            .all(|row| row.proposal_id.is_none())
    );
}

#[test]
fn valid_response_creates_exactly_one_preview_with_the_request_operation_id() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    let tag = issue_formatting(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("write operation id");
    let uri = fixture
        .app
        .document_uri_for_buffer_for_test(fixture.buffer)
        .expect("document URI");

    send_response(&mut fixture, tag.clone(), formatting_response(&uri));
    assert!(fixture.app.pending_lsp_writes.is_empty());
    let rows = operation(&fixture, &operation_id);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, LanguageToolingStatusKind::Ready);
    assert!(rows[0].proposal_id.is_some());

    // A replayed response has no pending correlation and must be ignored.
    send_response(&mut fixture, tag, formatting_response(&uri));
    let replay_rows = operation(&fixture, &operation_id);
    assert_eq!(
        replay_rows.len(),
        1,
        "replay must not duplicate the preview"
    );
}

#[test]
fn worker_error_is_failed_with_same_operation_and_no_proposal() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    let tag = issue_formatting(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("write operation id");
    fixture
        .results
        .send(LspWorkerResult::ReadResult {
            outcome: Err(LanguageSessionError::Unavailable),
            tag,
        })
        .expect("worker error");
    fixture.app.drain_lsp_session();
    assert!(fixture.app.pending_lsp_writes.is_empty());
    let rows = operation(&fixture, &operation_id);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, LanguageToolingStatusKind::Failed);
    assert!(rows[0].proposal_id.is_none());
}

#[test]
fn transport_death_fails_pending_write_and_releases_capacity() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    let tag = issue_formatting(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("write operation id");
    assert_eq!(fixture.app.pending_lsp_writes.len(), 1);

    fixture
        .results
        .send(LspWorkerResult::TransportDead {
            reason: "test transport closed".to_string(),
        })
        .expect("transport death");
    fixture.app.drain_lsp_session();

    assert!(fixture.app.pending_lsp_writes.is_empty());
    let rows = operation(&fixture, &operation_id);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, LanguageToolingStatusKind::Failed);
    assert!(rows[0].proposal_id.is_none());
}

#[test]
fn cancellation_followed_by_a_late_response_produces_no_preview() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    let tag = issue_formatting(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("write operation id");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::CancelLanguageOperation {
            operation_id: operation_id.clone(),
        })
        .expect("cancel operation");
    assert!(fixture.app.pending_lsp_writes.is_empty());

    let uri = fixture
        .app
        .document_uri_for_buffer_for_test(fixture.buffer)
        .expect("document URI");
    send_response(&mut fixture, tag, formatting_response(&uri));
    let rows = operation(&fixture, &operation_id);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, LanguageToolingStatusKind::Cancelled);
    assert!(rows[0].proposal_id.is_none());
}

#[test]
fn explicit_restart_clears_pending_write_as_cancelled() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    let tag = issue_formatting(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("write operation id");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::LspRestartSession)
        .expect("restart session");
    assert!(fixture.app.pending_lsp_writes.is_empty());
    let rows = operation(&fixture, &operation_id);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, LanguageToolingStatusKind::Cancelled);
    assert!(rows[0].proposal_id.is_none());
}

#[test]
fn switching_workspace_clears_pending_write_and_isolates_old_operation() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    let tag = issue_formatting(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("write operation id");
    let other_root = tempfile::tempdir().expect("second workspace");
    std::fs::write(
        other_root.path().join("Cargo.toml"),
        "[package]\nname = \"other\"\n",
    )
    .expect("second manifest");
    fixture
        .app
        .open_workspace(
            other_root.path(),
            WorkspaceTrustState::Trusted,
            PrincipalId("write-operation-other-workspace".to_string()),
        )
        .expect("switch workspace");
    assert!(fixture.app.pending_lsp_writes.is_empty());
    assert!(operation(&fixture, &operation_id).is_empty());

    // The old worker may still deliver after the workspace switch.  The
    // harness result sender is normally dropped with that old session, so a
    // send error is itself the expected isolation proof.
    assert!(
        fixture
            .results
            .send(LspWorkerResult::ReadResult {
                outcome: Ok(LspReadOutcome {
                    result: serde_json::json!([]),
                    issued_snapshot: tag.snapshot_id,
                    status: LspResultStatus::Fresh,
                }),
                tag,
            })
            .is_err()
    );
    assert!(fixture.app.pending_lsp_writes.is_empty());
    assert!(operation(&fixture, &operation_id).is_empty());
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .operations
            .iter()
            .all(|row| row.proposal_id.is_none())
    );
}

#[test]
fn stale_edit_response_is_terminal_and_does_not_leave_pending_state() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    let tag = issue_formatting(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("write operation id");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::Replace {
            buffer_id: fixture.buffer,
            range: legion_protocol::ProtocolTextRange {
                start: TextCoordinate {
                    line: 0,
                    character: 0,
                    byte_offset: None,
                    utf16_offset: None,
                },
                end: TextCoordinate {
                    line: 0,
                    character: 0,
                    byte_offset: None,
                    utf16_offset: None,
                },
            },
            replacement: "// changed\n".to_string(),
        })
        .expect("edit buffer");
    let uri = fixture
        .app
        .document_uri_for_buffer_for_test(fixture.buffer)
        .expect("document URI");
    send_response(&mut fixture, tag, formatting_response(&uri));
    assert!(fixture.app.pending_lsp_writes.is_empty());
    let rows = operation(&fixture, &operation_id);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, LanguageToolingStatusKind::Stale);
    assert!(rows[0].proposal_id.is_none());
}

#[test]
fn closed_buffer_response_is_terminal_and_does_not_leave_pending_state() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    let tag = issue_formatting(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("write operation id");
    assert!(matches!(
        fixture.app.close_tab(fixture.buffer).expect("close buffer"),
        crate::AppCloseTabOutcome::Closed { .. }
    ));
    send_response(&mut fixture, tag, serde_json::json!([]));
    assert!(fixture.app.pending_lsp_writes.is_empty());
    let rows = operation(&fixture, &operation_id);
    assert_eq!(rows.len(), 1);
    assert!(matches!(
        rows[0].status,
        LanguageToolingStatusKind::Cancelled
            | LanguageToolingStatusKind::Stale
            | LanguageToolingStatusKind::Failed
    ));
    assert!(rows[0].proposal_id.is_none());
}

#[test]
fn completion_hover_and_definition_are_rejected_until_document_sync_is_ready() {
    let mut fixture = fixture_with_sync(
        &["completionProvider", "hoverProvider", "definitionProvider"],
        false,
    );
    let position = TextCoordinate {
        line: 0,
        character: 0,
        byte_offset: None,
        utf16_offset: None,
    };
    assert!(
        !fixture
            .app
            .issue_lsp_completion_request(fixture.buffer, position)
    );
    assert!(
        !fixture
            .app
            .issue_lsp_hover_request(fixture.buffer, position)
    );
    assert!(
        !fixture
            .app
            .issue_lsp_definition_request(fixture.buffer, position)
    );
    assert!(fixture.requests.try_recv().is_err());
}

#[test]
fn dirty_buffer_read_admission_captures_current_authoritative_version() {
    let mut fixture = fixture_with_sync(&["hoverProvider"], false);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::Replace {
            buffer_id: fixture.buffer,
            range: legion_protocol::ProtocolTextRange {
                start: TextCoordinate {
                    line: 0,
                    character: 0,
                    byte_offset: None,
                    utf16_offset: None,
                },
                end: TextCoordinate {
                    line: 0,
                    character: 0,
                    byte_offset: None,
                    utf16_offset: None,
                },
            },
            replacement: "// dirty\n".to_string(),
        })
        .expect("edit buffer");
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("dirty didChange request")
    {
        LspWorkerRequest::DidOpenDeferred { text_rx, .. }
        | LspWorkerRequest::DidChangeDeferred { text_rx, .. } => {
            assert!(
                text_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("dirty document text")
                    .is_some()
            );
        }
        _ => panic!("expected deferred dirty document sync"),
    }
    let position = TextCoordinate {
        line: 0,
        character: 0,
        byte_offset: None,
        utf16_offset: None,
    };
    assert!(
        fixture
            .app
            .issue_lsp_hover_request(fixture.buffer, position)
    );
    let tag = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("hover request")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected hover read request"),
    };
    let context = tag
        .operation_context
        .as_ref()
        .expect("read admission context");
    let snapshot = fixture
        .app
        .editor
        .current_snapshot(fixture.buffer)
        .expect("snapshot");
    let version = fixture
        .app
        .editor
        .buffer_version(fixture.buffer)
        .expect("buffer version");
    assert_eq!(context.buffer_id, fixture.buffer);
    assert_eq!(context.snapshot_id, snapshot.snapshot_id);
    assert_eq!(context.buffer_version, version);
    assert_eq!(context.language_id, LanguageId("rust".to_string()));
    assert_ne!(context.correlation_id.0, 0);
    assert!(!context.causality_id.0.is_nil());
}

#[test]
fn write_admission_is_bounded_at_thirty_two_operations() {
    let mut fixture = fixture(&["documentFormattingProvider"]);
    let mut tags = Vec::new();
    for _ in 0..32 {
        tags.push(issue_formatting(&mut fixture));
    }
    assert_eq!(tags.len(), 32);
    assert_eq!(fixture.app.pending_lsp_writes.len(), 32);
    for tag in &tags {
        let operation_id = tag.operation_id.as_deref().expect("write operation id");
        let rows = operation(&fixture, operation_id);
        assert_eq!(rows.len(), 1, "every admitted operation remains visible");
        assert_eq!(rows[0].status, LanguageToolingStatusKind::Running);
        assert!(rows[0].proposal_id.is_none());
    }
    assert!(!fixture.app.issue_lsp_formatting_request(fixture.buffer));
    assert_eq!(fixture.app.pending_lsp_writes.len(), 32);
}

struct TwoBufferFixture {
    _root: tempfile::TempDir,
    app: AppComposition,
    second: BufferId,
    requests: Receiver<LspWorkerRequest>,
    results: SyncSender<LspWorkerResult>,
}

fn two_buffer_fixture() -> TwoBufferFixture {
    let root = tempfile::tempdir().expect("workspace");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"deferred-rename\"\n",
    )
    .expect("manifest");
    let first_path = root.path().join("first.rs");
    let second_path = root.path().join("second.rs");
    std::fs::write(&first_path, "fn first() {}\n").expect("first source");
    std::fs::write(&second_path, "fn second() {}\n").expect("second source");
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("deferred-rename-tests".to_string()),
    )
    .expect("workspace");
    app.open_file(first_path.to_string_lossy())
        .expect("first open");
    let first = app.active_buffer_id().expect("first buffer");
    app.open_file(second_path.to_string_lossy())
        .expect("second open");
    let second = app.active_buffer_id().expect("second buffer");
    let (requests, results) = app.set_lsp_request_harness_for_test(health(&["renameProvider"]));

    // Queue the first sync and then leave the second sync waiting behind it.
    app.notify_lsp_did_open(first);
    app.notify_lsp_did_open(second);
    TwoBufferFixture {
        _root: root,
        app,
        second,
        requests,
        results,
    }
}

fn consume_sync(receiver: &Receiver<LspWorkerRequest>) {
    match receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("document sync request")
    {
        LspWorkerRequest::DidOpenDeferred { text_rx, .. }
        | LspWorkerRequest::DidChangeDeferred { text_rx, .. } => {
            assert!(
                text_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("document sync text")
                    .is_some()
            );
        }
        _ => panic!("expected deferred document sync"),
    }
}

fn deferred_rename_operation(fixture: &mut TwoBufferFixture) -> String {
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestRenameProposal {
            buffer_id: fixture.second,
            position: TextCoordinate {
                line: 0,
                character: 3,
                byte_offset: None,
                utf16_offset: None,
            },
            new_name: "renamed".to_string(),
        })
        .expect("rename request");
    let rows = fixture.app.language_tooling_projection().operations;
    let row = rows
        .iter()
        .find(|row| row.kind == legion_protocol::LanguageToolingOperationKind::RenameProposal)
        .expect("rename operation row");
    assert_eq!(row.status, LanguageToolingStatusKind::Running);
    assert!(row.proposal_id.is_none());
    row.operation_id.clone()
}

fn send_two_buffer_response(
    fixture: &mut TwoBufferFixture,
    tag: LspRequestTag,
    result: serde_json::Value,
) {
    fixture
        .results
        .send(LspWorkerResult::ReadResult {
            outcome: Ok(LspReadOutcome {
                result,
                issued_snapshot: tag.snapshot_id,
                status: LspResultStatus::Fresh,
            }),
            tag,
        })
        .expect("rename response");
    fixture.app.drain_lsp_session();
}

#[test]
fn active_second_buffer_rename_waits_for_prior_sync_then_creates_one_proposal() {
    let mut fixture = two_buffer_fixture();
    let operation_id = deferred_rename_operation(&mut fixture);
    assert_eq!(fixture.app.pending_lsp_writes.len(), 1);
    consume_sync(&fixture.requests);
    fixture.app.drain_lsp_session();
    consume_sync(&fixture.requests);
    fixture.app.drain_lsp_session();
    let tag = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("deferred rename request")
    {
        LspWorkerRequest::RequestRead { tag, .. }
            if matches!(tag.kind, LspReadKind::Rename { .. }) =>
        {
            tag
        }
        _ => panic!("expected rename request after document sync"),
    };
    assert_eq!(tag.operation_id.as_deref(), Some(operation_id.as_str()));
    let uri = fixture
        .app
        .document_uri_for_buffer_for_test(fixture.second)
        .expect("second URI");
    send_two_buffer_response(
        &mut fixture,
        tag,
        serde_json::json!({
            "changes": {uri: [{
                "range": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 8}},
                "newText": "renamed"
            }]}
        }),
    );
    // The operation is correlated by the same UUID and produces one preview.
    let rows = fixture
        .app
        .language_tooling_projection()
        .operations
        .into_iter()
        .filter(|row| row.operation_id == operation_id)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].proposal_id.is_some());
}

#[test]
fn waiting_rename_cancellation_emits_no_later_wire_request() {
    let mut fixture = two_buffer_fixture();
    let operation_id = deferred_rename_operation(&mut fixture);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::CancelLanguageOperation { operation_id })
        .expect("cancel waiting rename");
    assert!(fixture.app.pending_lsp_writes.is_empty());
    consume_sync(&fixture.requests);
    fixture.app.drain_lsp_session();
    consume_sync(&fixture.requests);
    fixture.app.drain_lsp_session();
    assert!(fixture.requests.try_recv().is_err());
}

#[test]
fn waiting_rename_snapshot_edit_rejects_late_request_without_proposal() {
    let mut fixture = two_buffer_fixture();
    let operation_id = deferred_rename_operation(&mut fixture);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::Replace {
            buffer_id: fixture.second,
            range: range_for_write_test(),
            replacement: "// changed\n".to_string(),
        })
        .expect("edit waiting buffer");
    // The edit is observed at the next frame boundary.  Drain both queued
    // document-sync messages before asserting the stale terminal state.
    consume_sync(&fixture.requests);
    fixture.app.drain_lsp_session();
    consume_sync(&fixture.requests);
    fixture.app.drain_lsp_session();
    assert!(fixture.app.pending_lsp_writes.is_empty());
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .operations
            .into_iter()
            .filter(|row| row.operation_id == operation_id)
            .all(|row| {
                row.proposal_id.is_none() && row.status == LanguageToolingStatusKind::Stale
            })
    );
    assert!(fixture.requests.try_recv().is_err());
}

fn range_for_write_test() -> legion_protocol::ProtocolTextRange {
    legion_protocol::ProtocolTextRange {
        start: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
        end: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
    }
}
