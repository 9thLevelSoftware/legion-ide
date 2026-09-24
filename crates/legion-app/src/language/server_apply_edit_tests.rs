//! Authority tests for inbound `workspace/applyEdit` requests.
//!
//! These tests inject the real worker result variant and drain it through the
//! application boundary.  A server request can create only a proposal; the
//! editor changes and positive transport reply happen after explicit approval.

use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::{Duration, Instant};

use crate::AppComposition;
use crate::language::{LspReadKind, LspRequestTag, LspWorkerRequest, LspWorkerResult};
use legion_lsp::{LspApplyWorkspaceEditRequest, LspApplyWorkspaceEditResponse};
use legion_protocol::{
    BufferId, BufferVersion, FileId, LanguageId, LanguageServerId, LspCapabilitySummary,
    LspResultStatus, LspServerBinaryProvenance, LspServerHealthRecord, PrincipalId,
    ProposalRequest, ProtocolTextRange, TextCoordinate, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

struct Fixture {
    _root: tempfile::TempDir,
    app: AppComposition,
    buffer: BufferId,
    uri: String,
    requests: Receiver<LspWorkerRequest>,
    results: SyncSender<LspWorkerResult>,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().expect("workspace");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"apply-edit-tests\"\n",
    )
    .expect("manifest");
    let source = root.path().join("main.rs");
    std::fs::write(&source, "fn main() {}\n").expect("source");
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("server-apply-edit-tests".to_owned()),
    )
    .expect("workspace");
    app.open_file(source.to_string_lossy()).expect("source");
    let buffer = app.active_buffer_id().expect("active buffer");
    let uri = app
        .document_uri_for_buffer_for_test(buffer)
        .expect("document URI");
    let (requests, results) = app.set_lsp_request_harness_for_test(health());
    app.lsp_session
        .set_execute_command_ids_for_test(vec!["server.apply".to_owned()]);
    app.notify_lsp_did_open(buffer);
    match requests
        .recv_timeout(Duration::from_secs(1))
        .expect("didOpen")
    {
        LspWorkerRequest::DidOpenDeferred { text_rx, .. } => {
            assert!(
                text_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("text")
                    .is_some()
            );
        }
        _ => panic!("expected didOpen"),
    }
    Fixture {
        _root: root,
        app,
        buffer,
        uri,
        requests,
        results,
    }
}

fn health() -> LspServerHealthRecord {
    LspServerHealthRecord {
        server_id: LanguageServerId(1),
        language_id: LanguageId("rust".to_owned()),
        binary_provenance: LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Fresh,
        capabilities: vec![
            LspCapabilitySummary {
                capability: "codeActionProvider".to_owned(),
                supported: true,
                dynamic_registration: false,
                option_hash: None,
                redaction_hints: Vec::new(),
                schema_version: 1,
            },
            LspCapabilitySummary {
                capability: "executeCommandProvider".to_owned(),
                supported: true,
                dynamic_registration: false,
                option_hash: None,
                redaction_hints: Vec::new(),
                schema_version: 1,
            },
            LspCapabilitySummary {
                capability: "hoverProvider".to_owned(),
                supported: true,
                dynamic_registration: false,
                option_hash: None,
                redaction_hints: Vec::new(),
                schema_version: 1,
            },
        ],
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: 1,
    }
}

fn edit_request(tag: &LspRequestTag, uri: &str) -> LspApplyWorkspaceEditRequest {
    LspApplyWorkspaceEditRequest {
        json_rpc_id: 77,
        params: serde_json::json!({
            "edit": {"changes": {uri: [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "newText": "// server edit\n"
            }]}}
        }),
        context: tag.operation_context.clone(),
        deadline: None,
    }
}

fn inject_apply_edit(
    fixture: &mut Fixture,
    request: LspApplyWorkspaceEditRequest,
) -> std::sync::mpsc::Receiver<LspApplyWorkspaceEditResponse> {
    let (reply, response) = sync_channel(1);
    fixture
        .results
        .send(LspWorkerResult::ApplyEditRequested {
            request,
            reply,
            decision: crate::language::ApplyEditDecision::new(),
        })
        .expect("applyEdit worker result");
    fixture.app.drain_lsp_session();
    response
}

fn request_hover(fixture: &mut Fixture) -> LspRequestTag {
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestHover {
            buffer_id: fixture.buffer,
            position: TextCoordinate {
                line: 0,
                character: 0,
                byte_offset: None,
                utf16_offset: None,
            },
        })
        .expect("hover request");
    loop {
        match fixture
            .requests
            .recv_timeout(Duration::from_secs(1))
            .expect("hover wire")
        {
            LspWorkerRequest::RequestRead { tag, .. } if matches!(tag.kind, LspReadKind::Hover) => {
                return tag;
            }
            _ => {}
        }
    }
}

fn select_command(fixture: &mut Fixture) -> LspRequestTag {
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestCodeActions {
            buffer_id: fixture.buffer,
            range: ProtocolTextRange {
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
        })
        .expect("code action request");
    let request_tag = loop {
        match fixture
            .requests
            .recv_timeout(Duration::from_secs(1))
            .expect("code action wire")
        {
            LspWorkerRequest::RequestRead { tag, .. }
                if matches!(tag.kind, LspReadKind::CodeAction { .. }) =>
            {
                break tag;
            }
            _ => {}
        }
    };
    fixture
        .results
        .send(LspWorkerResult::ReadResult {
            outcome: Ok(crate::language::LspReadOutcome {
                result: serde_json::json!([{
                    "title": "Server command",
                    "kind": "quickfix",
                    "command": {"title": "Apply", "command": "server.apply", "arguments": []}
                }]),
                issued_snapshot: request_tag.snapshot_id,
                status: LspResultStatus::Fresh,
            }),
            tag: request_tag,
        })
        .expect("code action response");
    fixture.app.drain_lsp_session();
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: candidate.response_id,
            action_id: candidate.action_id,
        })
        .expect("select command");
    loop {
        match fixture
            .requests
            .recv_timeout(Duration::from_secs(1))
            .expect("command wire")
        {
            LspWorkerRequest::RequestRead { tag, .. }
                if matches!(tag.kind, LspReadKind::CodeActionExecuteCommand { .. }) =>
            {
                return tag;
            }
            _ => {}
        }
    }
}

#[test]
fn unsolicited_apply_edit_creates_preview_proposal_for_open_document() {
    let mut fixture = fixture();
    let before = fixture
        .app
        .buffer_text_for_input(fixture.buffer)
        .expect("text");
    let uri = fixture.uri.clone();
    let request = LspApplyWorkspaceEditRequest {
        json_rpc_id: 88,
        params: serde_json::json!({
            "edit": {"changes": {uri: [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "newText": "// server edit\n"
            }]}}
        }),
        context: None,
        deadline: None,
    };
    let response = inject_apply_edit(&mut fixture, request);
    assert!(
        response.try_recv().is_err(),
        "unsolicited applyEdit still requires approval"
    );
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("text"),
        before
    );
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .operations
            .iter()
            .any(|row| row.proposal_id.is_some()),
        "an open-document server edit must become a preview proposal"
    );
}

#[test]
fn ordinary_hover_context_is_rejected_without_a_proposal() {
    let mut fixture = fixture();
    let tag = request_hover(&mut fixture);
    let uri = fixture.uri.clone();
    let response = inject_apply_edit(&mut fixture, edit_request(&tag, &uri));
    let reply = response
        .recv_timeout(Duration::from_secs(1))
        .expect("rejection");
    assert!(!reply.applied);
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
fn selected_command_creates_proposal_without_mutation_or_positive_reply_before_approval() {
    let mut fixture = fixture();
    let tag = select_command(&mut fixture);
    let before = fixture
        .app
        .buffer_text_for_input(fixture.buffer)
        .expect("text");
    let uri = fixture.uri.clone();
    let response = inject_apply_edit(&mut fixture, edit_request(&tag, &uri));
    assert!(
        response.try_recv().is_err(),
        "approval is required before a positive reply"
    );
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("text"),
        before
    );
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .operations
            .iter()
            .any(|row| row.proposal_id.is_some())
    );
}

#[test]
fn stale_selected_command_context_is_rejected() {
    let mut fixture = fixture();
    let tag = select_command(&mut fixture);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::Replace {
            buffer_id: fixture.buffer,
            range: ProtocolTextRange {
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
            replacement: "local\n".to_owned(),
        })
        .expect("local edit");
    let uri = fixture.uri.clone();
    let response = inject_apply_edit(&mut fixture, edit_request(&tag, &uri));
    assert!(
        !response
            .recv_timeout(Duration::from_secs(1))
            .expect("stale rejection")
            .applied
    );
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
fn cancelling_selected_command_rejects_late_apply_edit() {
    let mut fixture = fixture();
    let tag = select_command(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("operation id");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::CancelLanguageOperation { operation_id })
        .expect("cancel command");
    let uri = fixture.uri.clone();
    let response = inject_apply_edit(&mut fixture, edit_request(&tag, &uri));
    assert!(
        !response
            .recv_timeout(Duration::from_secs(1))
            .expect("cancel rejection")
            .applied
    );
}

#[test]
fn approved_selected_command_applies_edit_and_sends_true() {
    let mut fixture = fixture();
    let tag = select_command(&mut fixture);
    let uri = fixture.uri.clone();
    let response = inject_apply_edit(&mut fixture, edit_request(&tag, &uri));
    let proposal_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find_map(|row| row.proposal_id)
        .expect("server edit proposal");
    let before = fixture
        .app
        .buffer_text_for_input(fixture.buffer)
        .expect("text before approval");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApproveProposal { proposal_id })
        .expect("approve server edit proposal");
    assert!(
        response.try_recv().is_err(),
        "approval alone must not apply"
    );
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("text"),
        before
    );
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApplyProposal { proposal_id })
        .expect("apply approved server edit proposal");
    let reply = response
        .recv_timeout(Duration::from_secs(1))
        .expect("apply reply");
    assert!(
        reply.applied,
        "approved proposal should send a positive reply: {reply:?}"
    );
    assert!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("text")
            .starts_with("// server edit\n")
    );
}

#[test]
fn same_request_id_with_mismatched_file_or_version_is_rejected() {
    let mut fixture = fixture();
    let tag = select_command(&mut fixture);
    let mut mismatched = edit_request(&tag, &fixture.uri);
    let mut context = tag.operation_context.clone().expect("command context");
    context.file_id = FileId(context.file_id.0.saturating_add(1));
    context.buffer_version = BufferVersion(context.buffer_version.0.saturating_add(1));
    mismatched.context = Some(context);
    let response = inject_apply_edit(&mut fixture, mismatched);
    let reply = response
        .recv_timeout(Duration::from_secs(1))
        .expect("mismatch rejection");
    assert!(!reply.applied);
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
fn repeated_inbound_edits_for_one_command_get_distinct_proposals_and_replies() {
    let mut fixture = fixture();
    let tag = select_command(&mut fixture);
    let uri = fixture.uri.clone();
    let first = inject_apply_edit(&mut fixture, edit_request(&tag, &uri));
    let second = inject_apply_edit(&mut fixture, edit_request(&tag, &uri));
    let proposal_ids = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .filter_map(|row| row.proposal_id)
        .collect::<Vec<_>>();
    assert_eq!(
        proposal_ids.len(),
        2,
        "each inbound edit needs its own proposal"
    );
    assert_ne!(proposal_ids[0], proposal_ids[1]);
    assert!(first.try_recv().is_err());
    assert!(second.try_recv().is_err());
}

#[test]
fn refused_server_edit_cannot_mutate_after_lifecycle_rejection() {
    let mut fixture = fixture();
    let tag = select_command(&mut fixture);
    let uri = fixture.uri.clone();
    let response = inject_apply_edit(&mut fixture, edit_request(&tag, &uri));
    let proposal_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find_map(|row| row.proposal_id)
        .expect("server edit proposal");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RejectProposal {
            proposal_id,
            reason: legion_protocol::ProposalRejectionReason::UserRejected,
        })
        .expect("reject server edit proposal");
    let reply = response
        .recv_timeout(Duration::from_secs(1))
        .expect("rejection reply");
    assert!(!reply.applied);
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("text"),
        "fn main() {}\n"
    );
}

#[test]
fn expired_server_edit_cannot_be_approved_or_mutate() {
    let mut fixture = fixture();
    let tag = select_command(&mut fixture);
    let uri = fixture.uri.clone();
    let response = inject_apply_edit(&mut fixture, edit_request(&tag, &uri));
    let proposal_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find_map(|row| row.proposal_id)
        .expect("server edit proposal");
    let proposal = fixture
        .app
        .workspace_proposal_for_id(proposal_id)
        .expect("proposal");
    fixture
        .app
        .server_apply_edits
        .expire_pending_for_test(proposal_id);
    let apply = fixture
        .app
        .handle_proposal_request(ProposalRequest::Apply(proposal));
    assert!(apply.is_err(), "expired server authority must refuse apply");
    let reply = response
        .recv_timeout(Duration::from_secs(1))
        .expect("expired reply");
    assert!(!reply.applied);
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("text"),
        "fn main() {}\n"
    );
}

#[test]
fn expired_server_edit_is_rejected_by_inline_claim_without_prior_drain() {
    let mut fixture = fixture();
    let tag = select_command(&mut fixture);
    let uri = fixture.uri.clone();
    let mut request = edit_request(&tag, &uri);
    request.deadline = Some(Instant::now() - Duration::from_secs(1));
    let response = inject_apply_edit(&mut fixture, request);
    let proposal_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find_map(|row| row.proposal_id)
        .expect("server edit proposal");
    let proposal = fixture
        .app
        .workspace_proposal_for_id(proposal_id)
        .expect("proposal");
    let apply = fixture
        .app
        .handle_proposal_request(ProposalRequest::Apply(proposal));
    assert!(apply.is_err(), "inline deadline claim must refuse apply");
    let reply = response
        .recv_timeout(Duration::from_secs(1))
        .expect("expired reply");
    assert!(!reply.applied);
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("text"),
        "fn main() {}\n"
    );
}
