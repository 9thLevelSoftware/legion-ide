//! End-to-end code-action authority tests.
//!
//! These tests request actions through the normal app intent, feed the real
//! worker result channel, and select only through opaque projected tokens.

use std::sync::mpsc::{Receiver, SyncSender};
use std::time::Duration;

use crate::AppComposition;
use crate::language::{
    LspReadKind, LspReadOutcome, LspRequestTag, LspWorkerRequest, LspWorkerResult,
};
use legion_protocol::{
    BufferId, LanguageId, LanguageServerId, LspCapabilitySummary, LspResultStatus,
    LspServerBinaryProvenance, LspServerHealthRecord, PrincipalId, ProtocolTextRange,
    TextCoordinate, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

fn health(resolve: bool, execute: bool) -> LspServerHealthRecord {
    let mut capabilities = vec![LspCapabilitySummary {
        capability: "codeActionProvider".to_string(),
        supported: true,
        dynamic_registration: false,
        option_hash: None,
        redaction_hints: Vec::new(),
        schema_version: 1,
    }];
    if resolve {
        capabilities.push(LspCapabilitySummary {
            capability: "codeActionResolveProvider".to_string(),
            supported: true,
            dynamic_registration: false,
            option_hash: None,
            redaction_hints: Vec::new(),
            schema_version: 1,
        });
    }
    if execute {
        capabilities.push(LspCapabilitySummary {
            capability: "executeCommandProvider".to_string(),
            supported: true,
            dynamic_registration: false,
            option_hash: None,
            redaction_hints: Vec::new(),
            schema_version: 1,
        });
    }
    LspServerHealthRecord {
        server_id: LanguageServerId(1),
        language_id: LanguageId("rust".to_string()),
        binary_provenance: LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Fresh,
        capabilities,
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
    uri: String,
    requests: Receiver<LspWorkerRequest>,
    results: SyncSender<LspWorkerResult>,
}

fn fixture_with_capabilities(resolve: bool, execute: bool) -> Fixture {
    let root = tempfile::tempdir().expect("workspace");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"code-action-tests\"\n",
    )
    .expect("manifest");
    let source = root.path().join("main.rs");
    std::fs::write(&source, "fn main() {}\n").expect("source");
    let mut app = AppComposition::new();
    app.open_workspace(
        root.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("code-action-tests".to_string()),
    )
    .expect("workspace");
    app.open_file(source.to_string_lossy()).expect("source");
    let buffer = app.active_buffer_id().expect("active buffer");
    let uri = app
        .document_uri_for_buffer_for_test(buffer)
        .expect("document URI");
    let (requests, results) = app.set_lsp_request_harness_for_test(health(resolve, execute));
    app.lsp_session
        .set_execute_command_ids_for_test(if execute {
            vec!["server.fixSelected".to_string()]
        } else {
            Vec::new()
        });
    app.notify_lsp_did_open(buffer);
    match requests
        .recv_timeout(Duration::from_secs(1))
        .expect("didOpen")
    {
        LspWorkerRequest::DidOpenDeferred { text_rx, .. } => {
            assert!(
                text_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("didOpen text")
                    .is_some()
            );
        }
        _ => panic!("expected deferred didOpen"),
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

fn fixture() -> Fixture {
    fixture_with_capabilities(true, true)
}

fn fixture_with_resolve(resolve: bool) -> Fixture {
    fixture_with_capabilities(resolve, true)
}

fn fixture_without_execute() -> Fixture {
    fixture_with_capabilities(true, false)
}

fn range() -> ProtocolTextRange {
    ProtocolTextRange {
        start: TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: None,
            utf16_offset: None,
        },
        end: TextCoordinate {
            line: 0,
            character: 1,
            byte_offset: None,
            utf16_offset: None,
        },
    }
}

fn request(fixture: &mut Fixture) -> LspRequestTag {
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestCodeActions {
            buffer_id: fixture.buffer,
            range: range(),
        })
        .expect("request code actions");
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("code action request")
    {
        LspWorkerRequest::RequestRead { tag, .. }
            if matches!(
                tag.kind,
                LspReadKind::CodeAction {
                    organize_imports: false
                }
            ) =>
        {
            tag
        }
        _ => panic!("expected code action read request"),
    }
}

fn organize_request(fixture: &mut Fixture) -> LspRequestTag {
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestOrganizeImportsProposal {
            buffer_id: fixture.buffer,
        })
        .expect("request organize imports");
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("organize imports request")
    {
        LspWorkerRequest::RequestRead {
            method,
            params,
            tag,
        } => {
            assert_eq!(method, "textDocument/codeAction");
            assert_eq!(
                params["context"]["only"],
                serde_json::json!(["source.organizeImports"])
            );
            assert!(matches!(
                tag.kind,
                LspReadKind::CodeAction {
                    organize_imports: true
                }
            ));
            tag
        }
        _ => panic!("expected organize imports code-action request"),
    }
}

fn send_response(fixture: &mut Fixture, tag: LspRequestTag, response: serde_json::Value) {
    fixture
        .results
        .send(LspWorkerResult::ReadResult {
            outcome: Ok(LspReadOutcome {
                result: response,
                issued_snapshot: tag.snapshot_id,
                status: LspResultStatus::Fresh,
            }),
            tag,
        })
        .expect("code action response");
    fixture.app.drain_lsp_session();
}

fn actions(uri: &str) -> serde_json::Value {
    serde_json::json!([
        {
            "title": "First action",
            "kind": "quickfix.first",
            "isPreferred": true,
            "edit": {"changes": {uri: [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "newText": "FIRST"
            }]}}
        },
        {
            "title": "Second action",
            "kind": "refactor.second",
            "isPreferred": false,
            "edit": {"changes": {uri: [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "newText": "SECOND"
            }]}}
        }
    ])
}

fn data_action() -> serde_json::Value {
    serde_json::json!({
        "title": "Deferred action",
        "kind": "quickfix.deferred",
        "data": {"request": "raw-token-42", "origin": "server"}
    })
}

fn command_action() -> serde_json::Value {
    serde_json::json!({
        "title": "Run selected command",
        "kind": "quickfix.command",
        "command": {
            "title": "Run selected command",
            "command": "server.fixSelected",
            "arguments": ["raw-argument", {"line": 7, "column": 2}]
        }
    })
}

fn mixed_command_action(uri: &str) -> serde_json::Value {
    serde_json::json!({
        "title": "Edit then command",
        "kind": "quickfix.mixed",
        "edit": {"changes": {uri: [{
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
            "newText": "MIXED"
        }]}},
        "command": {"command": "server.fixSelected", "arguments": ["mixed", {"line": 1}]}
    })
}

fn unallowed_command_action() -> serde_json::Value {
    serde_json::json!({
        "title": "Unapproved command",
        "kind": "quickfix.command.unapproved",
        "command": {
            "title": "Unapproved command",
            "command": "server.notAllowlisted",
            "arguments": ["must-not-run"]
        }
    })
}

fn resolved_edit(uri: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "edit": {"changes": {uri: [{
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
            "newText": text
        }]}}
    })
}

fn resolved_mixed_command(uri: &str) -> serde_json::Value {
    serde_json::json!({
        "edit": {"changes": {uri: [{
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
            "newText": "RESOLVED-MIXED"
        }]}},
        "command": {"command": "server.fixSelected", "arguments": ["resolved-mixed", {"line": 3}]}
    })
}

fn organize_edit(uri: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "title": "Organize imports",
        "kind": "source.organizeImports",
        "edit": {"changes": {uri: [{
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
            "newText": text
        }]}}
    })
}

fn organize_data_action() -> serde_json::Value {
    serde_json::json!({
        "title": "Organize imports",
        "kind": "source.organizeImports",
        "data": {"organize": true}
    })
}

fn organize_command_action() -> serde_json::Value {
    serde_json::json!({
        "title": "Organize imports command",
        "kind": "source.organizeImports",
        "command": {"command": "server.fixSelected", "arguments": ["organize-command"]}
    })
}

fn organize_mixed_action(uri: &str) -> serde_json::Value {
    serde_json::json!({
        "title": "Organize imports mixed",
        "kind": "source.organizeImports",
        "edit": {"changes": {uri: [{
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
            "newText": "ORGANIZED"
        }]}},
        "command": {"command": "server.fixSelected", "arguments": ["organize-mixed"]}
    })
}

fn projected_proposal_count(fixture: &Fixture) -> usize {
    fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .filter(|row| row.proposal_id.is_some())
        .count()
}

#[test]
fn response_projects_both_actions_without_proposals_and_preserves_metadata_order() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let response = actions(&fixture.uri);
    send_response(&mut fixture, tag, response);
    let projection = fixture.app.language_tooling_projection();
    assert_eq!(projected_proposal_count(&fixture), 0);
    assert_eq!(projection.code_action_candidates.len(), 2);
    assert_eq!(projection.code_action_candidates[0].title, "First action");
    assert_eq!(
        projection.code_action_candidates[0].kind.as_deref(),
        Some("quickfix.first")
    );
    assert!(projection.code_action_candidates[0].is_preferred);
    assert_eq!(projection.code_action_candidates[1].title, "Second action");
    assert_eq!(
        projection.code_action_candidates[1].kind.as_deref(),
        Some("refactor.second")
    );
    assert!(!projection.code_action_candidates[1].is_preferred);
}

#[test]
fn mixed_action_previews_edit_without_executing_command() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let uri = fixture.uri.clone();
    send_response(
        &mut fixture,
        tag,
        serde_json::json!([mixed_command_action(&uri)]),
    );
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
        .expect("select mixed action");
    assert_eq!(projected_proposal_count(&fixture), 1);
    assert!(fixture.requests.try_recv().is_err());
}

#[test]
fn cancelling_mixed_action_never_executes_command() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let uri = fixture.uri.clone();
    send_response(
        &mut fixture,
        tag,
        serde_json::json!([mixed_command_action(&uri)]),
    );
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
        .expect("select mixed action");
    let operation_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find(|row| row.proposal_id.is_some())
        .map(|row| row.operation_id.clone())
        .expect("mixed operation");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::CancelLanguageOperation { operation_id })
        .expect("cancel mixed action");
    assert!(fixture.requests.try_recv().is_err());
}

#[test]
fn missing_mixed_sidecar_refuses_apply_before_edit_mutation() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let uri = fixture.uri.clone();
    send_response(
        &mut fixture,
        tag,
        serde_json::json!([mixed_command_action(&uri)]),
    );
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
        .expect("select mixed action");
    let proposal_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find_map(|row| row.proposal_id)
        .expect("mixed proposal");
    fixture.app.clear_code_actions();
    let before = fixture
        .app
        .buffer_text_for_input(fixture.buffer)
        .expect("text");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApproveProposal { proposal_id })
        .expect("approve proposal");
    let _ = fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApplyProposal { proposal_id });
    let after = fixture
        .app
        .buffer_text_for_input(fixture.buffer)
        .expect("text");
    assert_eq!(before, after);
    assert!(fixture.requests.try_recv().is_err());
}

#[test]
fn approved_mixed_action_syncs_edit_before_one_exact_command() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let uri = fixture.uri.clone();
    send_response(
        &mut fixture,
        tag,
        serde_json::json!([mixed_command_action(&uri)]),
    );
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    let original_snapshot_id = candidate.snapshot_id.expect("candidate snapshot");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: candidate.response_id,
            action_id: candidate.action_id,
        })
        .expect("select mixed action");
    let proposal_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find_map(|row| row.proposal_id)
        .expect("mixed proposal");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApproveProposal { proposal_id })
        .expect("approve mixed proposal");
    assert!(fixture.requests.try_recv().is_err());
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApplyProposal { proposal_id })
        .expect("apply mixed proposal");
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("didChange")
    {
        LspWorkerRequest::DidChangeDeferred { text_rx, .. } => {
            assert_eq!(
                text_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("changed text"),
                Some("MIXEDfn main() {}\n".to_string())
            );
        }
        _ => panic!("expected didChange"),
    }
    fixture.app.drain_lsp_session();
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("execute")
    {
        LspWorkerRequest::RequestRead {
            method,
            params,
            tag,
        } => {
            assert_eq!(method, "workspace/executeCommand");
            assert_eq!(params["command"], "server.fixSelected");
            assert_eq!(
                params["arguments"],
                serde_json::json!(["mixed", {"line": 1}])
            );
            assert_eq!(tag.buffer_id, fixture.buffer);
            assert_ne!(tag.snapshot_id, original_snapshot_id);
            let context = tag.operation_context.as_ref().expect("command context");
            assert!(
                fixture
                    .app
                    .pending_code_action_contexts
                    .contains_key(&context.request_id.0.to_string())
            );
        }
        _ => panic!("expected execute-command"),
    }
    assert!(fixture.requests.try_recv().is_err());
}

#[test]
fn mixed_command_keeps_origin_buffer_after_switching_active_tab() {
    let mut fixture = fixture();
    let second_path = fixture._root.path().join("second.rs");
    std::fs::write(&second_path, "fn second() {}\n").expect("second source");
    fixture
        .app
        .open_file(second_path.to_string_lossy())
        .expect("second file");
    let second = fixture.app.active_buffer_id().expect("second buffer");
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("second didOpen")
    {
        LspWorkerRequest::DidOpenDeferred { text_rx, .. } => {
            let _ = text_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("second text");
        }
        _ => panic!("expected second didOpen"),
    }
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SwitchTab {
            buffer_id: fixture.buffer,
        })
        .expect("return to origin tab");
    let tag = request(&mut fixture);
    let uri = fixture.uri.clone();
    send_response(
        &mut fixture,
        tag,
        serde_json::json!([mixed_command_action(&uri)]),
    );
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
        .expect("select mixed action");
    let proposal_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find_map(|row| row.proposal_id)
        .expect("mixed proposal");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SwitchTab { buffer_id: second })
        .expect("switch away before apply");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApproveProposal { proposal_id })
        .expect("approve mixed proposal");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApplyProposal { proposal_id })
        .expect("apply mixed proposal");
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("origin didChange")
    {
        LspWorkerRequest::DidChangeDeferred { text_rx, .. } => {
            let _ = text_rx.recv_timeout(Duration::from_secs(1));
        }
        _ => panic!("expected origin didChange"),
    }
    fixture.app.drain_lsp_session();
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("origin execute")
    {
        LspWorkerRequest::RequestRead { method, tag, .. } => {
            assert_eq!(method, "workspace/executeCommand");
            assert_eq!(tag.buffer_id, fixture.buffer);
        }
        _ => panic!("expected origin execute-command"),
    }
}

#[test]
fn resolved_command_only_executes_exact_arguments_after_resolve() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    send_response(&mut fixture, tag, serde_json::json!([data_action()]));
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
        .expect("select deferred action");
    let resolve = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("resolve")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected resolve request"),
    };
    let resolved = serde_json::json!({
        "command": {"command": "server.fixSelected", "arguments": ["resolved", {"line": 9}]}
    });
    send_response(&mut fixture, resolve, resolved);
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("execute")
    {
        LspWorkerRequest::RequestRead { method, params, .. } => {
            assert_eq!(method, "workspace/executeCommand");
            assert_eq!(params["command"], "server.fixSelected");
            assert_eq!(
                params["arguments"],
                serde_json::json!(["resolved", {"line": 9}])
            );
        }
        _ => panic!("expected execute-command request"),
    }
}

#[test]
fn selecting_second_opaque_token_creates_only_second_edit_proposal() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let response = actions(&fixture.uri);
    send_response(&mut fixture, tag, response);
    let second = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[1]
        .clone();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: second.response_id.clone(),
            action_id: second.action_id.clone(),
        })
        .expect("select second code action");
    let proposals: Vec<_> = fixture
        .app
        .language_tooling_projection()
        .operations
        .into_iter()
        .filter_map(|row| row.proposal_id)
        .collect();
    assert_eq!(proposals.len(), 1);
    let proposal = fixture
        .app
        .workspace_proposal_for_id(proposals[0])
        .expect("proposal");
    let serialized = serde_json::to_string(&proposal.payload).expect("proposal JSON");
    assert!(serialized.contains("SECOND"));
    assert!(!serialized.contains("FIRST"));
}

#[test]
fn stale_response_and_replay_token_do_not_create_duplicate_proposals() {
    let mut fixture = fixture();
    let first = request(&mut fixture);
    let second = request(&mut fixture);
    let response = actions(&fixture.uri);
    send_response(&mut fixture, first, response.clone());
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .code_action_candidates
            .is_empty()
    );
    send_response(&mut fixture, second, response);
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    let intent = CommandDispatchIntent::SelectCodeAction {
        response_id: candidate.response_id.clone(),
        action_id: candidate.action_id.clone(),
    };
    fixture
        .app
        .dispatch_ui_intent(intent.clone())
        .expect("select");
    let count = projected_proposal_count(&fixture);
    fixture
        .app
        .dispatch_ui_intent(intent)
        .expect("replay select");
    assert_eq!(projected_proposal_count(&fixture), count);
}

#[test]
fn source_change_and_disabled_selection_are_rejected() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let disabled = serde_json::json!([{
        "title": "Disabled", "kind": "quickfix.disabled", "disabled": {"reason": "not safe"}
    }]);
    send_response(&mut fixture, tag, disabled);
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::Replace {
            buffer_id: fixture.buffer,
            range: range(),
            replacement: "changed".to_string(),
        })
        .expect("source change");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: candidate.response_id,
            action_id: candidate.action_id,
        })
        .expect("stale selection");
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn candidate_count_is_bounded_at_sixty_four() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let mut actions = Vec::new();
    for index in 0..80 {
        actions.push(serde_json::json!({
            "title": format!("Action {index}"),
            "kind": "quickfix.bounded",
            "edit": {"changes": {fixture.uri.clone(): []}}
        }));
    }
    send_response(&mut fixture, tag, serde_json::Value::Array(actions));
    assert_eq!(
        fixture
            .app
            .language_tooling_projection()
            .code_action_candidates
            .len(),
        64
    );
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn oversized_edit_payload_is_rejected_without_retaining_raw_text() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let oversized = "x".repeat(300 * 1024);
    let response = serde_json::json!([{
        "title": "Oversized edit",
        "kind": "quickfix.oversized",
        "edit": {"changes": {fixture.uri.clone(): [{
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
            "newText": oversized
        }]}}
    }]);
    send_response(&mut fixture, tag, response);
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .code_action_candidates
            .is_empty()
    );
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn cumulative_action_payload_budget_stops_before_unbounded_retention() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let large = "y".repeat(140 * 1024);
    let response = serde_json::json!([
        {
            "title": "Large one",
            "kind": "quickfix.large.one",
            "edit": {"changes": {fixture.uri.clone(): [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "newText": large
            }]}}
        },
        {
            "title": "Large two",
            "kind": "quickfix.large.two",
            "edit": {"changes": {fixture.uri.clone(): [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "newText": "second"
            }]}}
        }
    ]);
    send_response(&mut fixture, tag, response);
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .code_action_candidates
            .len()
            <= 1
    );
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn command_only_and_disabled_actions_remain_metadata_only_candidates() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let response = serde_json::json!([
        {
            "title": "Run command",
            "kind": "source.fixAll.command",
            "command": {"title": "Run command", "command": "server.fixAll", "arguments": []}
        },
        {
            "title": "Disabled action",
            "kind": "quickfix.disabled",
            "disabled": {"reason": "requires approval"}
        }
    ]);
    send_response(&mut fixture, tag, response);
    let candidates = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates;
    assert_eq!(candidates.len(), 2);
    assert!(candidates[0].has_command);
    assert!(!candidates[0].has_edit);
    assert!(candidates[1].disabled_reason.is_some());
    assert!(!candidates[1].has_edit);
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn restart_clears_projected_candidates_and_rejects_old_token() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let response = actions(&fixture.uri);
    send_response(&mut fixture, tag, response);
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::LspRestartSession)
        .expect("restart");
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .code_action_candidates
            .is_empty()
    );
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: candidate.response_id,
            action_id: candidate.action_id,
        })
        .expect("stale token selection");
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn close_clears_projected_candidates_and_rejects_old_token() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let response = actions(&fixture.uri);
    send_response(&mut fixture, tag, response);
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    assert!(matches!(
        fixture.app.close_tab(fixture.buffer).expect("close"),
        crate::AppCloseTabOutcome::Closed { .. }
    ));
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .code_action_candidates
            .is_empty()
    );
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: candidate.response_id,
            action_id: candidate.action_id,
        })
        .expect("closed token selection");
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn data_only_action_resolves_with_exact_raw_data_before_one_proposal() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let raw = data_action();
    send_response(&mut fixture, tag, serde_json::json!([raw.clone()]));
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    assert_eq!(projected_proposal_count(&fixture), 0);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: candidate.response_id.clone(),
            action_id: candidate.action_id.clone(),
        })
        .expect("select data-only action");
    let resolve = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("resolve request")
    {
        LspWorkerRequest::RequestRead {
            method,
            params,
            tag,
        } => {
            assert_eq!(method, "codeAction/resolve");
            assert_eq!(params, raw);
            assert!(matches!(tag.kind, LspReadKind::CodeActionResolve { .. }));
            tag
        }
        _ => panic!("expected code-action resolve request"),
    };
    assert_eq!(projected_proposal_count(&fixture), 0);
    let resolved = resolved_edit(&fixture.uri, "RESOLVED");
    send_response(&mut fixture, resolve, resolved);
    assert_eq!(projected_proposal_count(&fixture), 1);
}

#[test]
fn stale_resolved_edit_is_rejected_without_a_proposal() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let raw = data_action();
    send_response(&mut fixture, tag, serde_json::json!([raw]));
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    let response_id = candidate.response_id.clone();
    let action_id = candidate.action_id.clone();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: response_id.clone(),
            action_id: action_id.clone(),
        })
        .expect("select data-only action");
    let resolve = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("resolve request")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected resolve request"),
    };
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
            replacement: "changed".to_string(),
        })
        .expect("source change");
    let stale = resolved_edit(&fixture.uri, "STALE");
    send_response(&mut fixture, resolve, stale);
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn cancelling_resolve_rejects_late_response_without_a_proposal() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let raw = data_action();
    send_response(&mut fixture, tag, serde_json::json!([raw]));
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    let response_id = candidate.response_id.clone();
    let action_id = candidate.action_id.clone();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: response_id.clone(),
            action_id: action_id.clone(),
        })
        .expect("select data-only action");
    let resolve = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("resolve request")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected resolve request"),
    };
    let operation_id = resolve.operation_id.clone().expect("resolve operation id");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::CancelLanguageOperation { operation_id })
        .expect("cancel resolve");
    let cancelled = resolved_edit(&fixture.uri, "CANCELLED");
    send_response(&mut fixture, resolve, cancelled);
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn unsupported_resolve_capability_does_not_emit_resolve_wire_request() {
    let mut fixture = fixture_with_resolve(false);
    let tag = request(&mut fixture);
    let raw = data_action();
    send_response(&mut fixture, tag, serde_json::json!([raw]));
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
        .expect("unsupported resolve selection is handled");
    assert!(fixture.requests.try_recv().is_err());
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn oversized_resolved_edit_is_failed_without_a_proposal() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let raw = data_action();
    send_response(&mut fixture, tag, serde_json::json!([raw]));
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
        .expect("select data-only action");
    let resolve = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("resolve request")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected resolve request"),
    };
    let oversized = "z".repeat(300 * 1024);
    let oversized_edit = resolved_edit(&fixture.uri, &oversized);
    send_response(&mut fixture, resolve, oversized_edit);
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn selecting_resolve_candidate_twice_does_not_emit_second_wire_request() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    send_response(&mut fixture, tag, serde_json::json!([data_action()]));
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    let selection = CommandDispatchIntent::SelectCodeAction {
        response_id: candidate.response_id.clone(),
        action_id: candidate.action_id.clone(),
    };
    fixture
        .app
        .dispatch_ui_intent(selection.clone())
        .expect("first resolve selection");
    let first = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("first resolve")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected resolve request"),
    };
    fixture
        .app
        .dispatch_ui_intent(selection)
        .expect("duplicate resolve selection");
    assert!(fixture.requests.try_recv().is_err());
    assert_eq!(fixture.app.pending_lsp_writes.len(), 1);
    let once_edit = resolved_edit(&fixture.uri, "ONCE");
    send_response(&mut fixture, first, once_edit);
    assert_eq!(projected_proposal_count(&fixture), 1);
}

#[test]
fn cancelled_resolve_retry_rejects_old_response_and_accepts_new_once() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    send_response(&mut fixture, tag, serde_json::json!([data_action()]));
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    let response_id = candidate.response_id.clone();
    let action_id = candidate.action_id.clone();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: response_id.clone(),
            action_id: action_id.clone(),
        })
        .expect("first resolve selection");
    let first = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("first resolve")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected resolve request"),
    };
    let operation_id = first.operation_id.clone().expect("operation id");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::CancelLanguageOperation { operation_id })
        .expect("cancel first resolve");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: response_id.clone(),
            action_id: action_id.clone(),
        })
        .expect("retry resolve selection");
    let retry = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("retry resolve")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected retry resolve request"),
    };
    assert_ne!(first.operation_id, retry.operation_id);
    let old_edit = resolved_edit(&fixture.uri, "OLD");
    send_response(&mut fixture, first, old_edit);
    assert_eq!(projected_proposal_count(&fixture), 0);
    assert_eq!(fixture.app.pending_lsp_writes.len(), 1);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: response_id.clone(),
            action_id: action_id.clone(),
        })
        .expect("duplicate retry remains blocked");
    assert!(fixture.requests.try_recv().is_err());
    assert_eq!(fixture.app.pending_lsp_writes.len(), 1);
    let new_edit = resolved_edit(&fixture.uri, "NEW");
    send_response(&mut fixture, retry, new_edit);
    assert_eq!(projected_proposal_count(&fixture), 1);
}

#[test]
fn resolved_mixed_action_applies_edit_and_executes_one_exact_command() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    send_response(&mut fixture, tag, serde_json::json!([data_action()]));
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
        .expect("select resolve candidate");
    let resolve = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("resolve")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected resolve request"),
    };
    let mixed_resolved = resolved_mixed_command(&fixture.uri);
    send_response(&mut fixture, resolve, mixed_resolved);
    let proposal_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find_map(|row| row.proposal_id)
        .expect("resolved mixed proposal");
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("original text"),
        "fn main() {}\n"
    );
    assert!(fixture.requests.try_recv().is_err());
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApproveProposal { proposal_id })
        .expect("approve resolved mixed proposal");
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("approved text"),
        "fn main() {}\n"
    );
    assert!(fixture.requests.try_recv().is_err());
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApplyProposal { proposal_id })
        .expect("apply resolved mixed proposal");
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("didChange")
    {
        LspWorkerRequest::DidChangeDeferred { text_rx, .. } => {
            assert_eq!(
                text_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("changed text"),
                Some("RESOLVED-MIXEDfn main() {}\n".to_string())
            );
        }
        _ => panic!("expected didChange"),
    }
    fixture.app.drain_lsp_session();
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("execute command")
    {
        LspWorkerRequest::RequestRead { method, params, .. } => {
            assert_eq!(method, "workspace/executeCommand");
            assert_eq!(params["command"], "server.fixSelected");
            assert_eq!(
                params["arguments"],
                serde_json::json!(["resolved-mixed", {"line": 3}])
            );
        }
        _ => panic!("expected execute-command request"),
    }
    assert!(fixture.requests.try_recv().is_err());
}

#[test]
fn resolve_admission_is_bounded_by_shared_thirty_two_pending_limit() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let second_data_action = serde_json::json!({
        "title": "Second deferred action",
        "kind": "quickfix.deferred.second",
        "data": {"request": "raw-token-43", "origin": "server"}
    });
    send_response(
        &mut fixture,
        tag,
        serde_json::json!([data_action(), second_data_action]),
    );
    let candidates = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates
        .clone();
    let first_candidate = candidates[0].clone();
    let second_candidate = candidates[1].clone();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: first_candidate.response_id.clone(),
            action_id: first_candidate.action_id.clone(),
        })
        .expect("first resolve selection");
    let first = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("resolve")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected resolve request"),
    };
    let template = fixture
        .app
        .pending_lsp_writes
        .values()
        .next()
        .cloned()
        .expect("pending resolve");
    for index in 1..32 {
        let mut pending = template.clone();
        pending.operation_id = format!("resolve-cap-{index}");
        fixture
            .app
            .pending_lsp_writes
            .insert(pending.operation_id.clone(), pending);
    }
    assert_eq!(fixture.app.pending_lsp_writes.len(), 32);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: second_candidate.response_id,
            action_id: second_candidate.action_id,
        })
        .expect("bounded duplicate resolve selection");
    assert!(fixture.requests.try_recv().is_err());
    assert_eq!(fixture.app.pending_lsp_writes.len(), 32);
    let _ = first;
}

#[test]
fn organize_imports_request_uses_exact_source_only_context() {
    let mut fixture = fixture();
    let _ = organize_request(&mut fixture);
}

#[test]
fn sole_organize_imports_edit_auto_previews_with_dedicated_kind() {
    let mut fixture = fixture();
    let tag = organize_request(&mut fixture);
    let sole_edit = organize_edit(&fixture.uri, "USE");
    send_response(&mut fixture, tag, serde_json::json!([sole_edit]));
    let operations = fixture.app.language_tooling_projection().operations;
    let operation = operations
        .iter()
        .find(|operation| operation.proposal_id.is_some())
        .expect("organize imports proposal");
    assert_eq!(
        operation.kind,
        legion_protocol::LanguageToolingOperationKind::OrganizeImportsProposal
    );
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("editor text"),
        "fn main() {}\n"
    );
    assert_eq!(
        std::fs::read_to_string(fixture._root.path().join("main.rs")).expect("disk text"),
        "fn main() {}\n"
    );
}

#[test]
fn multiple_organize_imports_edits_require_opaque_selection() {
    let mut fixture = fixture();
    let tag = organize_request(&mut fixture);
    let first = organize_edit(&fixture.uri, "FIRST");
    let second = organize_edit(&fixture.uri, "SECOND");
    send_response(&mut fixture, tag, serde_json::json!([first, second]));
    assert_eq!(projected_proposal_count(&fixture), 0);
    assert_eq!(
        fixture
            .app
            .language_tooling_projection()
            .code_action_candidates
            .len(),
        2
    );
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[1]
        .clone();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: candidate.response_id,
            action_id: candidate.action_id,
        })
        .expect("select second organize action");
    let proposal_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find_map(|operation| operation.proposal_id)
        .expect("second organize proposal");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApproveProposal { proposal_id })
        .expect("approve second organize action");
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApplyProposal { proposal_id })
        .expect("apply second organize action");
    assert!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("second organize text")
            .starts_with("SECOND")
    );
}

#[test]
fn organize_imports_command_selection_executes_exact_command() {
    let mut fixture = fixture();
    let tag = organize_request(&mut fixture);
    send_response(
        &mut fixture,
        tag,
        serde_json::json!([organize_command_action()]),
    );
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
        .expect("select organize command");
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("execute organize command")
    {
        LspWorkerRequest::RequestRead { method, params, .. } => {
            assert_eq!(method, "workspace/executeCommand");
            assert_eq!(params["command"], "server.fixSelected");
            assert_eq!(params["arguments"], serde_json::json!(["organize-command"]));
        }
        _ => panic!("expected organize execute-command request"),
    }
}

#[test]
fn organize_imports_mixed_action_requires_approval_then_applies_and_executes_once() {
    let mut fixture = fixture();
    let tag = organize_request(&mut fixture);
    let mixed = organize_mixed_action(&fixture.uri);
    send_response(&mut fixture, tag, serde_json::json!([mixed]));
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
        .expect("select organize mixed action");
    let proposal_id = fixture
        .app
        .language_tooling_projection()
        .operations
        .iter()
        .find_map(|operation| operation.proposal_id)
        .expect("organize mixed proposal");
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("text"),
        "fn main() {}\n"
    );
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApproveProposal { proposal_id })
        .expect("approve organize mixed");
    assert_eq!(
        fixture
            .app
            .buffer_text_for_input(fixture.buffer)
            .expect("text"),
        "fn main() {}\n"
    );
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::ApplyProposal { proposal_id })
        .expect("apply organize mixed");
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("organize didChange")
    {
        LspWorkerRequest::DidChangeDeferred { text_rx, .. } => {
            assert_eq!(
                text_rx.recv_timeout(Duration::from_secs(1)).expect("text"),
                Some("ORGANIZEDfn main() {}\n".to_string())
            );
        }
        _ => panic!("expected organize didChange"),
    }
    fixture.app.drain_lsp_session();
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("organize execute")
    {
        LspWorkerRequest::RequestRead { method, params, .. } => {
            assert_eq!(method, "workspace/executeCommand");
            assert_eq!(params["command"], "server.fixSelected");
            assert_eq!(params["arguments"], serde_json::json!(["organize-mixed"]));
        }
        _ => panic!("expected organize execute-command"),
    }
    assert!(fixture.requests.try_recv().is_err());
}

#[test]
fn resolved_organize_imports_preserves_dedicated_proposal_kind() {
    let mut fixture = fixture();
    let tag = organize_request(&mut fixture);
    send_response(
        &mut fixture,
        tag,
        serde_json::json!([organize_data_action()]),
    );
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
        .expect("select resolved organize action");
    let resolve = match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("organize resolve")
    {
        LspWorkerRequest::RequestRead { tag, .. } => tag,
        _ => panic!("expected organize resolve request"),
    };
    let resolved = resolved_edit(&fixture.uri, "RESOLVED-ORGANIZED");
    send_response(&mut fixture, resolve, resolved);
    let projection = fixture.app.language_tooling_projection();
    let operation = projection
        .operations
        .iter()
        .find(|operation| operation.proposal_id.is_some())
        .expect("resolved organize proposal");
    assert_eq!(
        operation.kind,
        legion_protocol::LanguageToolingOperationKind::OrganizeImportsProposal
    );
}

fn admit_command_and_capture_wire(fixture: &mut Fixture) -> (LspRequestTag, String, String) {
    let tag = request(fixture);
    let raw = command_action();
    send_response(fixture, tag, serde_json::json!([raw]));
    let candidate = fixture
        .app
        .language_tooling_projection()
        .code_action_candidates[0]
        .clone();
    let response_id = candidate.response_id.clone();
    let action_id = candidate.action_id.clone();
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id: response_id.clone(),
            action_id: action_id.clone(),
        })
        .expect("select command action");
    match fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("execute command request")
    {
        LspWorkerRequest::RequestRead {
            method,
            params,
            tag,
        } => {
            assert_eq!(method, "workspace/executeCommand");
            assert_eq!(
                params,
                serde_json::json!({
                    "command": "server.fixSelected",
                    "arguments": ["raw-argument", {"line": 7, "column": 2}]
                })
            );
            assert!(matches!(
                tag.kind,
                LspReadKind::CodeActionExecuteCommand { .. }
            ));
            (tag, response_id, action_id)
        }
        _ => panic!("expected execute-command request"),
    }
}

#[test]
fn selected_command_preserves_exact_arguments_and_null_completion_is_terminal() {
    let mut fixture = fixture();
    let (tag, response_id, action_id) = admit_command_and_capture_wire(&mut fixture);
    let operation_id = tag.operation_id.clone().expect("command operation id");
    assert_eq!(projected_proposal_count(&fixture), 0);

    // Re-selecting an in-flight command token cannot enqueue a second wire
    // command or create another pending operation.
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::SelectCodeAction {
            response_id,
            action_id,
        })
        .expect("replay command selection");
    assert!(fixture.requests.try_recv().is_err());
    assert_eq!(fixture.app.pending_lsp_writes.len(), 1);

    send_response(&mut fixture, tag.clone(), serde_json::Value::Null);
    let rows = fixture
        .app
        .language_tooling_projection()
        .operations
        .into_iter()
        .filter(|row| row.operation_id == operation_id)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].status,
        legion_protocol::LanguageToolingStatusKind::Ready
    );
    assert!(rows[0].proposal_id.is_none());
    assert!(
        fixture
            .app
            .language_tooling_projection()
            .code_action_candidates
            .is_empty()
    );

    // The consumed command token cannot be replayed into another wire call.
    fixture
        .results
        .send(LspWorkerResult::ReadResult {
            outcome: Ok(LspReadOutcome {
                result: serde_json::Value::Null,
                issued_snapshot: tag.snapshot_id,
                status: LspResultStatus::Fresh,
            }),
            tag,
        })
        .expect("replayed result channel remains live");
    fixture.app.drain_lsp_session();
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn unadvertised_command_provider_rejects_selection_without_wire() {
    let mut fixture = fixture_without_execute();
    let tag = request(&mut fixture);
    let raw = command_action();
    send_response(&mut fixture, tag, serde_json::json!([raw]));
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
        .expect("unadvertised command handled");
    assert!(fixture.requests.try_recv().is_err());
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn advertised_provider_still_rejects_unallowlisted_command_without_wire() {
    let mut fixture = fixture();
    let tag = request(&mut fixture);
    let raw = unallowed_command_action();
    send_response(&mut fixture, tag, serde_json::json!([raw]));
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
        .expect("unallowlisted command handled");
    assert!(fixture.requests.try_recv().is_err());
    assert_eq!(fixture.app.pending_lsp_writes.len(), 0);
    assert_eq!(projected_proposal_count(&fixture), 0);
}

#[test]
fn command_pending_admission_is_bounded_at_thirty_two() {
    let mut fixture = fixture();
    for _ in 0..32 {
        let _ = admit_command_and_capture_wire(&mut fixture);
    }
    assert_eq!(fixture.app.pending_lsp_writes.len(), 32);
    fixture
        .app
        .dispatch_ui_intent(CommandDispatchIntent::RequestCodeActions {
            buffer_id: fixture.buffer,
            range: range(),
        })
        .expect("bounded code-action request");
    assert!(fixture.requests.try_recv().is_err());
    assert_eq!(fixture.app.pending_lsp_writes.len(), 32);
}
