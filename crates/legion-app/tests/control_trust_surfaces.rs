use std::{
    collections::HashMap,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use legion_app::{AppCommandOutcome, AppComposition, AppCompositionError, AppProductMode};
use legion_editor::{TextEdit, TextPosition};
use legion_protocol::{
    BufferId, BufferVersion, CapabilityId, CorrelationId, EditBatch, FileId, FileIdentity,
    FileTreeNode, PreviewSummary, PrincipalId, ProposalCancellationReason, ProposalId,
    ProposalLifecycleState, ProposalPayload, ProposalRejectionReason, ProposalRequest,
    ProposalResponse, ProposalRollbackReason, ProposalVersionPreconditions, RedactionHint,
    SaveConflictPolicy, SaveFileProposal, SaveIntent, SnapshotId, TextOffset, TextRange,
    TimestampMillis, TrustDecisionContext, WorkspaceGeneration, WorkspaceId, WorkspacePort,
    WorkspaceProposal, WorkspaceRequest, WorkspaceResponse, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-app-control-trust-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64)
            + TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn workspace_tree(app: &AppComposition, workspace_id: WorkspaceId) -> Vec<FileTreeNode> {
    match app
        .workspace()
        .handle(WorkspaceRequest::ReadTree(workspace_id))
        .expect("read workspace tree")
    {
        WorkspaceResponse::Tree(tree) => tree,
        other => panic!("expected workspace tree, got {other:?}"),
    }
}

fn workspace_node_by_name(
    app: &AppComposition,
    workspace_id: WorkspaceId,
    name: &str,
) -> FileTreeNode {
    workspace_tree(app, workspace_id)
        .into_iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("workspace node {name} not found"))
}

fn file_preconditions(
    node: &FileTreeNode,
    workspace_generation: WorkspaceGeneration,
) -> ProposalVersionPreconditions {
    let fingerprint = node
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.fingerprint.clone())
        .expect("file node fingerprint");
    ProposalVersionPreconditions {
        file_version: Some(node.identity.content_version),
        buffer_version: None,
        snapshot_id: None,
        generation: Some(workspace_generation),
        file_content_version: Some(node.identity.content_version),
        workspace_generation: Some(workspace_generation),
        expected_fingerprint: Some(fingerprint),
        expected_file_length: node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.size_bytes),
        expected_modified_at: node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.modified_at),
    }
}

fn proposal_envelope_with(
    proposal_id: ProposalId,
    capability: &str,
    payload: ProposalPayload,
    preconditions: ProposalVersionPreconditions,
) -> WorkspaceProposal {
    WorkspaceProposal {
        proposal_id,
        principal: PrincipalId("trusted".to_string()),
        capability: CapabilityId(capability.to_string()),
        correlation_id: CorrelationId(42),
        payload,
        preconditions,
        preview: PreviewSummary {
            summary: format!("control trust proposal {}", proposal_id.0),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(proposal_id.0),
    }
}

fn text_edit_proposal(
    proposal_id: ProposalId,
    file_id: FileId,
    replacement: &str,
    preconditions: ProposalVersionPreconditions,
) -> WorkspaceProposal {
    proposal_envelope_with(
        proposal_id,
        "editor.write",
        ProposalPayload::TextEdit(legion_protocol::TextEditProposal {
            file_id,
            edits: EditBatch {
                edits: vec![legion_protocol::TextEdit {
                    range: TextRange::new(TextOffset::byte(0), TextOffset::byte(4)),
                    replacement: replacement.to_string(),
                }],
            },
        }),
        preconditions,
    )
}

fn save_payload_for_open_buffer(
    file: FileIdentity,
    editor_file_id: FileId,
    buffer_id: BufferId,
    snapshot_id: SnapshotId,
    buffer_version: BufferVersion,
    workspace_generation: WorkspaceGeneration,
    expected_fingerprint: legion_protocol::FileFingerprint,
) -> ProposalPayload {
    ProposalPayload::SaveFile(SaveFileProposal {
        file_id: editor_file_id,
        file: file.clone(),
        buffer_id,
        snapshot_id,
        buffer_version,
        file_content_version: file.content_version,
        workspace_generation,
        expected_fingerprint: Some(expected_fingerprint),
        save_intent: SaveIntent::ExternalCommand,
        conflict_policy: SaveConflictPolicy::RejectIfChanged,
        trust_decision: TrustDecisionContext {
            workspace_trust_state: WorkspaceTrustState::Trusted,
            decision_id: None,
            decided_at: Some(TimestampMillis(1)),
        },
        required_capability: CapabilityId("fs.write".to_string()),
        principal: PrincipalId("trusted".to_string()),
        correlation_id: CorrelationId(42),
        diagnostics: Vec::new(),
    })
}

fn register_created(app: &mut AppComposition, proposal: &WorkspaceProposal) {
    assert!(matches!(
        app.register_proposal_lifecycle(proposal)
            .expect("register proposal lifecycle"),
        ProposalResponse::Created(_)
    ));
}

fn register_validate(app: &mut AppComposition, proposal: &WorkspaceProposal) {
    register_created(app, proposal);
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Validate(proposal.clone()))
            .expect("validate proposal"),
        ProposalResponse::Validated(_)
    ));
}

fn register_validate_preview(app: &mut AppComposition, proposal: &WorkspaceProposal) {
    register_validate(app, proposal);
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Preview(proposal.clone()))
            .expect("preview proposal"),
        ProposalResponse::Previewed { .. }
    ));
}

fn response_state(response: &ProposalResponse) -> ProposalLifecycleState {
    match response {
        ProposalResponse::Created(transition)
        | ProposalResponse::Validated(transition)
        | ProposalResponse::Approved(transition)
        | ProposalResponse::Applied(transition) => transition.lifecycle_state,
        ProposalResponse::Previewed { transition, .. }
        | ProposalResponse::Rejected { transition, .. }
        | ProposalResponse::Denied { transition, .. }
        | ProposalResponse::Failed { transition, .. }
        | ProposalResponse::RolledBack { transition, .. }
        | ProposalResponse::Stale { transition, .. }
        | ProposalResponse::Conflict { transition, .. }
        | ProposalResponse::Cancelled { transition, .. } => transition.lifecycle_state,
    }
}

fn outcome_response(outcome: AppCommandOutcome) -> ProposalResponse {
    match outcome {
        AppCommandOutcome::ProposalLifecycleUpdated(response) => response,
        other => panic!("expected proposal lifecycle outcome, got {other:?}"),
    }
}

fn proposal_states(app: &AppComposition) -> HashMap<ProposalId, ProposalLifecycleState> {
    app.shell_projection_snapshot("proposal states")
        .expect("shell projection")
        .proposal_ledger_projection
        .rows
        .into_iter()
        .map(|row| (row.proposal_id, row.lifecycle.state))
        .collect()
}

fn ai_outcome(outcome: AppCommandOutcome) -> legion_app::AppAiRunOutcome {
    match outcome {
        AppCommandOutcome::AiRunStarted(outcome) => *outcome,
        other => panic!("expected assisted AI outcome, got {other:?}"),
    }
}

#[test]
fn manual_mode_rejects_assisted_ai_dispatch() {
    let root = create_root();
    let target = root.join("manual.rs");
    std::fs::write(&target, "fn main() {}\n").expect("seed file");

    let mut app = AppComposition::new();
    assert_eq!(app.product_mode(), AppProductMode::Manual);
    let (_opened, _file_id, _buffer_id, _node, _preconditions) =
        opened_text_file(&mut app, &root, "manual.rs");

    let error = app
        .dispatch_ui_intent(CommandDispatchIntent::StartAiExplain {
            instruction_label: "manual mode should refuse".to_string(),
        })
        .expect_err("manual mode rejects AI dispatch");
    assert!(matches!(
        error,
        AppCompositionError::AiRuntime(message)
            if message.contains("requires Assist, Delegate, or Automate")
    ));

    let shell = app
        .shell_projection_snapshot("manual mode AI gate")
        .expect("shell projection");
    assert_eq!(shell.assisted_ai_projection.request_count, 0);
    assert_eq!(shell.assisted_ai_projection.preview_ready_count, 0);

    let _ = std::fs::remove_dir_all(&root);
}

fn opened_text_file(
    app: &mut AppComposition,
    root: &Path,
    file_name: &str,
) -> (
    legion_protocol::WorkspaceOpened,
    FileId,
    BufferId,
    FileTreeNode,
    ProposalVersionPreconditions,
) {
    let opened = app
        .open_workspace(
            root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let path = root.join(file_name);
    let file_id = app
        .open_file(path.to_string_lossy())
        .expect("open target file");
    let buffer_id = app.active_buffer_id().expect("active buffer id");
    let node = workspace_node_by_name(app, opened.workspace_id, file_name);
    let mut preconditions = file_preconditions(&node, opened.generation);
    let snapshot = app
        .editor()
        .current_snapshot(buffer_id)
        .expect("current snapshot");
    preconditions.buffer_version = Some(snapshot.buffer_version);
    preconditions.snapshot_id = Some(snapshot.snapshot_id);
    (opened, file_id, buffer_id, node, preconditions)
}

#[test]
fn proposal_lifecycle_ui_intents_route_through_app_authority() {
    let root = create_root();
    let target = root.join("lifecycle.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let mut app = AppComposition::new();
    let (_opened, file_id, _buffer_id, _node, preconditions) =
        opened_text_file(&mut app, &root, "lifecycle.txt");

    let preview = text_edit_proposal(ProposalId(101), file_id, "reed", preconditions.clone());
    register_validate(&mut app, &preview);
    let response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::PreviewProposal {
            proposal_id: preview.proposal_id,
        })
        .expect("preview from UI intent"),
    );
    assert_eq!(response_state(&response), ProposalLifecycleState::Previewed);

    let approve = text_edit_proposal(ProposalId(102), file_id, "heed", preconditions.clone());
    register_validate_preview(&mut app, &approve);
    let response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::ApproveProposal {
            proposal_id: approve.proposal_id,
        })
        .expect("approve from UI intent"),
    );
    assert_eq!(response_state(&response), ProposalLifecycleState::Approved);

    let reject = text_edit_proposal(ProposalId(103), file_id, "need", preconditions.clone());
    register_validate_preview(&mut app, &reject);
    let response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::RejectProposal {
            proposal_id: reject.proposal_id,
            reason: ProposalRejectionReason::UserRejected,
        })
        .expect("reject from UI intent"),
    );
    assert_eq!(response_state(&response), ProposalLifecycleState::Rejected);

    let cancel = text_edit_proposal(ProposalId(104), file_id, "feed", preconditions.clone());
    register_created(&mut app, &cancel);
    let response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::CancelProposal {
            proposal_id: cancel.proposal_id,
            reason: ProposalCancellationReason::UserCancelled,
        })
        .expect("cancel from UI intent"),
    );
    assert_eq!(response_state(&response), ProposalLifecycleState::Cancelled);

    let apply = text_edit_proposal(ProposalId(105), file_id, "sprout", preconditions);
    register_validate_preview(&mut app, &apply);
    let response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::ApplyProposal {
            proposal_id: apply.proposal_id,
        })
        .expect("apply from UI intent"),
    );
    assert_eq!(response_state(&response), ProposalLifecycleState::Applied);
    assert_eq!(
        app.editor()
            .text(app.active_buffer_id().expect("active buffer"))
            .expect("active text"),
        "sprout"
    );

    let response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::RollbackProposal {
            proposal_id: apply.proposal_id,
            reason: ProposalRollbackReason::UserRequested,
        })
        .expect("rollback from UI intent"),
    );
    assert_eq!(
        response_state(&response),
        ProposalLifecycleState::RolledBack
    );

    let details = app
        .dispatch_ui_intent(CommandDispatchIntent::OpenProposalDetails {
            proposal_id: apply.proposal_id,
        })
        .expect("details from UI intent");
    assert!(matches!(
        details,
        AppCommandOutcome::ProposalDetailsOpened(ProposalId(105))
    ));

    let states = proposal_states(&app);
    assert_eq!(
        states.get(&ProposalId(105)),
        Some(&ProposalLifecycleState::RolledBack)
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn proposal_details_selected_proposal_populates_trust_surfaces() {
    let root = create_root();
    let target = root.join("details.txt");
    std::fs::write(&target, "seed").expect("seed file");

    let mut app = AppComposition::new();
    let (_opened, file_id, _buffer_id, _node, preconditions) =
        opened_text_file(&mut app, &root, "details.txt");
    let first = text_edit_proposal(ProposalId(201), file_id, "first", preconditions.clone());
    let second = text_edit_proposal(ProposalId(202), file_id, "second", preconditions);
    register_validate_preview(&mut app, &first);
    register_validate_preview(&mut app, &second);

    assert!(matches!(
        app.dispatch_ui_intent(CommandDispatchIntent::OpenProposalDetails {
            proposal_id: first.proposal_id,
        })
        .expect("open first proposal details"),
        AppCommandOutcome::ProposalDetailsOpened(ProposalId(201))
    ));

    let shell = app
        .shell_projection_snapshot("proposal details")
        .expect("shell projection");
    assert_eq!(
        shell.proposal_ledger_projection.selected_proposal_id,
        Some(first.proposal_id)
    );
    let row = shell
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == first.proposal_id)
        .expect("selected row");
    assert!(row.diff_summary.full_source_redacted);
    assert_eq!(row.target_coverage.omitted_target_count, 0);
    assert!(row.redaction_hints.contains(&RedactionHint::MetadataOnly));
    assert_eq!(
        shell.context_manifest_projection.manifest.proposal_id,
        Some(first.proposal_id)
    );
    assert!(!shell.context_manifest_projection.manifest.items.is_empty());
    assert_eq!(
        shell.privacy_inspector_projection.proposal_id,
        Some(first.proposal_id)
    );
    assert!(!shell.privacy_inspector_projection.records.is_empty());
    assert_eq!(
        shell.permission_budget_projection.evaluations[0]
            .action
            .proposal_id,
        Some(first.proposal_id)
    );
    assert_eq!(
        shell.approval_checklist_projection.proposal_id,
        first.proposal_id
    );
    assert_eq!(
        shell.checkpoint_rollback_projection.proposal_id,
        first.proposal_id
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn proposal_states_visible_after_ui_apply_rejections_and_rollback() {
    let root = create_root();
    let conflict_target = root.join("conflict-save.txt");
    let stale_target = root.join("stale-delete.txt");
    let failed_target = root.join("failed-edit.txt");
    let applied_target = root.join("applied-edit.txt");
    std::fs::write(&conflict_target, "seed").expect("seed conflict file");
    std::fs::write(&stale_target, "stale").expect("seed stale file");
    std::fs::write(&failed_target, "fail").expect("seed failed file");
    std::fs::write(&applied_target, "seed").expect("seed applied file");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");

    let conflict_file_id = app
        .open_file(conflict_target.to_string_lossy())
        .expect("open conflict file");
    let conflict_buffer_id = app.active_buffer_id().expect("conflict buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("dirty conflict buffer");
    let conflict_snapshot = app
        .editor()
        .current_snapshot(conflict_buffer_id)
        .expect("conflict snapshot")
        .clone();
    let conflict_node = workspace_node_by_name(&app, opened.workspace_id, "conflict-save.txt");
    let mut conflict_preconditions = file_preconditions(&conflict_node, opened.generation);
    conflict_preconditions.buffer_version = Some(conflict_snapshot.buffer_version);
    conflict_preconditions.snapshot_id = Some(conflict_snapshot.snapshot_id);
    let conflict_fingerprint = conflict_preconditions
        .expected_fingerprint
        .clone()
        .expect("conflict fingerprint");
    let conflict = proposal_envelope_with(
        ProposalId(301),
        "fs.write",
        save_payload_for_open_buffer(
            conflict_node.identity.clone(),
            conflict_file_id,
            conflict_buffer_id,
            conflict_snapshot.snapshot_id,
            conflict_snapshot.buffer_version,
            opened.generation,
            conflict_fingerprint,
        ),
        conflict_preconditions,
    );
    register_validate_preview(&mut app, &conflict);
    std::fs::write(&conflict_target, "external").expect("external overwrite");
    let conflict_response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::ApplyProposal {
            proposal_id: conflict.proposal_id,
        })
        .expect("conflicted save from UI intent"),
    );
    assert!(matches!(
        response_state(&conflict_response),
        ProposalLifecycleState::Conflict | ProposalLifecycleState::Stale
    ));
    assert_eq!(
        std::fs::read_to_string(&conflict_target).expect("conflict disk"),
        "external"
    );
    assert_eq!(
        app.editor()
            .text(conflict_buffer_id)
            .expect("dirty conflict text"),
        "seed!"
    );

    let stale_node = workspace_node_by_name(&app, opened.workspace_id, "stale-delete.txt");
    let stale = proposal_envelope_with(
        ProposalId(302),
        "fs.write",
        ProposalPayload::DeleteFile(legion_protocol::DeleteFileProposal {
            file: stale_node.identity.clone(),
        }),
        file_preconditions(&stale_node, WorkspaceGeneration(opened.generation.0 + 1)),
    );
    register_validate_preview(&mut app, &stale);
    let stale_response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::ApplyProposal {
            proposal_id: stale.proposal_id,
        })
        .expect("stale delete from UI intent"),
    );
    assert_eq!(
        response_state(&stale_response),
        ProposalLifecycleState::Stale
    );
    assert!(stale_target.exists());

    let failed_file_id = app
        .open_file(failed_target.to_string_lossy())
        .expect("open failed file");
    let failed_buffer_id = app.active_buffer_id().expect("failed buffer");
    let failed_node = workspace_node_by_name(&app, opened.workspace_id, "failed-edit.txt");
    let mut failed_preconditions = file_preconditions(&failed_node, opened.generation);
    let failed_snapshot = app
        .editor()
        .current_snapshot(failed_buffer_id)
        .expect("failed snapshot");
    failed_preconditions.buffer_version = Some(failed_snapshot.buffer_version);
    failed_preconditions.snapshot_id = Some(failed_snapshot.snapshot_id);
    let failed = text_edit_proposal(
        ProposalId(303),
        failed_file_id,
        "fell",
        failed_preconditions,
    );
    register_validate_preview(&mut app, &failed);
    app.fail_next_proposal_audit_write_for_test();
    let failed_response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::ApplyProposal {
            proposal_id: failed.proposal_id,
        })
        .expect("failed apply from UI intent"),
    );
    assert_eq!(
        response_state(&failed_response),
        ProposalLifecycleState::Failed
    );

    let applied_file_id = app
        .open_file(applied_target.to_string_lossy())
        .expect("open applied file");
    let applied_buffer_id = app.active_buffer_id().expect("applied buffer");
    let applied_node = workspace_node_by_name(&app, opened.workspace_id, "applied-edit.txt");
    let mut applied_preconditions = file_preconditions(&applied_node, opened.generation);
    let applied_snapshot = app
        .editor()
        .current_snapshot(applied_buffer_id)
        .expect("applied snapshot");
    applied_preconditions.buffer_version = Some(applied_snapshot.buffer_version);
    applied_preconditions.snapshot_id = Some(applied_snapshot.snapshot_id);
    let applied = text_edit_proposal(
        ProposalId(304),
        applied_file_id,
        "done",
        applied_preconditions,
    );
    register_validate_preview(&mut app, &applied);
    let applied_response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::ApplyProposal {
            proposal_id: applied.proposal_id,
        })
        .expect("applied edit from UI intent"),
    );
    assert_eq!(
        response_state(&applied_response),
        ProposalLifecycleState::Applied
    );
    let rollback_response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::RollbackProposal {
            proposal_id: applied.proposal_id,
            reason: ProposalRollbackReason::UserRequested,
        })
        .expect("rollback from UI intent"),
    );
    assert_eq!(
        response_state(&rollback_response),
        ProposalLifecycleState::RolledBack
    );

    let states = proposal_states(&app);
    assert_eq!(
        states.get(&conflict.proposal_id),
        Some(&response_state(&conflict_response))
    );
    assert_eq!(
        states.get(&stale.proposal_id),
        Some(&ProposalLifecycleState::Stale)
    );
    assert_eq!(
        states.get(&failed.proposal_id),
        Some(&ProposalLifecycleState::Failed)
    );
    assert_eq!(
        states.get(&applied.proposal_id),
        Some(&ProposalLifecycleState::RolledBack)
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dirty_text_preserved_on_rejected_stale_and_conflict_outcomes() {
    let root = create_root();
    let reject_target = root.join("reject-save.txt");
    let stale_target = root.join("stale-save.txt");
    let conflict_target = root.join("conflict-save.txt");
    std::fs::write(&reject_target, "seed").expect("seed reject file");
    std::fs::write(&stale_target, "seed").expect("seed stale file");
    std::fs::write(&conflict_target, "seed").expect("seed conflict file");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");

    let reject_file_id = app
        .open_file(reject_target.to_string_lossy())
        .expect("open reject file");
    let reject_buffer_id = app.active_buffer_id().expect("reject buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("dirty reject buffer");
    let reject_node = workspace_node_by_name(&app, opened.workspace_id, "reject-save.txt");
    let mut reject_preconditions = file_preconditions(&reject_node, opened.generation);
    let reject_snapshot = app
        .editor()
        .current_snapshot(reject_buffer_id)
        .expect("reject snapshot");
    reject_preconditions.buffer_version = Some(reject_snapshot.buffer_version);
    reject_preconditions.snapshot_id = Some(reject_snapshot.snapshot_id);
    let reject = text_edit_proposal(
        ProposalId(401),
        reject_file_id,
        "reed",
        reject_preconditions,
    );
    register_validate_preview(&mut app, &reject);
    let reject_response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::RejectProposal {
            proposal_id: reject.proposal_id,
            reason: ProposalRejectionReason::UserRejected,
        })
        .expect("reject dirty proposal"),
    );
    assert_eq!(
        response_state(&reject_response),
        ProposalLifecycleState::Rejected
    );
    assert_eq!(
        app.editor()
            .text(reject_buffer_id)
            .expect("rejected dirty text"),
        "seed!"
    );
    assert!(
        app.editor()
            .is_dirty(reject_buffer_id)
            .expect("reject dirty retained")
    );

    let stale_file_id = app
        .open_file(stale_target.to_string_lossy())
        .expect("open stale file");
    let stale_buffer_id = app.active_buffer_id().expect("stale buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "?"))
        .expect("dirty stale buffer");
    let stale_node = workspace_node_by_name(&app, opened.workspace_id, "stale-save.txt");
    let stale_generation = WorkspaceGeneration(opened.generation.0 + 1);
    let mut stale_preconditions = file_preconditions(&stale_node, stale_generation);
    let stale_snapshot = app
        .editor()
        .current_snapshot(stale_buffer_id)
        .expect("stale snapshot")
        .clone();
    stale_preconditions.buffer_version = Some(stale_snapshot.buffer_version);
    stale_preconditions.snapshot_id = Some(stale_snapshot.snapshot_id);
    let stale_fingerprint = stale_preconditions
        .expected_fingerprint
        .clone()
        .expect("stale fingerprint");
    let stale = proposal_envelope_with(
        ProposalId(402),
        "fs.write",
        save_payload_for_open_buffer(
            stale_node.identity.clone(),
            stale_file_id,
            stale_buffer_id,
            stale_snapshot.snapshot_id,
            stale_snapshot.buffer_version,
            stale_generation,
            stale_fingerprint,
        ),
        stale_preconditions,
    );
    register_validate_preview(&mut app, &stale);
    let stale_response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::ApplyProposal {
            proposal_id: stale.proposal_id,
        })
        .expect("apply stale dirty proposal"),
    );
    assert_eq!(
        response_state(&stale_response),
        ProposalLifecycleState::Stale
    );
    assert_eq!(
        app.editor()
            .text(stale_buffer_id)
            .expect("stale dirty text"),
        "seed?"
    );
    assert!(
        app.editor()
            .is_dirty(stale_buffer_id)
            .expect("stale dirty retained")
    );
    assert_eq!(
        std::fs::read_to_string(&stale_target).expect("stale disk"),
        "seed"
    );

    let conflict_file_id = app
        .open_file(conflict_target.to_string_lossy())
        .expect("open conflict file");
    let conflict_buffer_id = app.active_buffer_id().expect("conflict buffer");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 4), "!"))
        .expect("dirty conflict buffer");
    let conflict_node = workspace_node_by_name(&app, opened.workspace_id, "conflict-save.txt");
    let mut conflict_preconditions = file_preconditions(&conflict_node, opened.generation);
    let conflict_snapshot = app
        .editor()
        .current_snapshot(conflict_buffer_id)
        .expect("conflict snapshot")
        .clone();
    conflict_preconditions.buffer_version = Some(conflict_snapshot.buffer_version);
    conflict_preconditions.snapshot_id = Some(conflict_snapshot.snapshot_id);
    let conflict_fingerprint = conflict_preconditions
        .expected_fingerprint
        .clone()
        .expect("conflict fingerprint");
    let conflict = proposal_envelope_with(
        ProposalId(403),
        "fs.write",
        save_payload_for_open_buffer(
            conflict_node.identity.clone(),
            conflict_file_id,
            conflict_buffer_id,
            conflict_snapshot.snapshot_id,
            conflict_snapshot.buffer_version,
            opened.generation,
            conflict_fingerprint,
        ),
        conflict_preconditions,
    );
    register_validate_preview(&mut app, &conflict);
    std::fs::write(&conflict_target, "external").expect("external overwrite");
    let conflict_response = outcome_response(
        app.dispatch_ui_intent(CommandDispatchIntent::ApplyProposal {
            proposal_id: conflict.proposal_id,
        })
        .expect("apply conflict dirty proposal"),
    );
    assert!(matches!(
        response_state(&conflict_response),
        ProposalLifecycleState::Conflict | ProposalLifecycleState::Stale
    ));
    assert_eq!(
        app.editor()
            .text(conflict_buffer_id)
            .expect("conflict dirty text"),
        "seed!"
    );
    assert!(
        app.editor()
            .is_dirty(conflict_buffer_id)
            .expect("conflict dirty retained")
    );
    assert_eq!(
        std::fs::read_to_string(&conflict_target).expect("conflict disk"),
        "external"
    );

    let states = proposal_states(&app);
    assert_eq!(
        states.get(&reject.proposal_id),
        Some(&ProposalLifecycleState::Rejected)
    );
    assert_eq!(
        states.get(&stale.proposal_id),
        Some(&ProposalLifecycleState::Stale)
    );
    assert_eq!(
        states.get(&conflict.proposal_id),
        Some(&response_state(&conflict_response))
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn assisted_ai_explain_routes_metadata_only_without_proposal() {
    let root = create_root();
    let target = root.join("explain.rs");
    std::fs::write(&target, "fn main() {}\n").expect("seed file");

    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Assist);
    let (_opened, _file_id, _buffer_id, _node, _preconditions) =
        opened_text_file(&mut app, &root, "explain.rs");

    let outcome = ai_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::StartAiExplain {
            instruction_label: "summarize context".to_string(),
        })
        .expect("assisted explain starts"),
    );

    assert_eq!(outcome.proposal_id, None);
    assert!(outcome.proposal_created.is_none());
    assert!(outcome.refusal.is_none());
    assert_eq!(
        outcome.route_response.invocation_state,
        legion_protocol::AssistedAiProviderInvocationState::Completed
    );

    let shell = app
        .shell_projection_snapshot("assisted explain")
        .expect("shell projection");
    assert!(shell.proposal_ledger_projection.rows.is_empty());
    assert_eq!(shell.assisted_ai_projection.preview_ready_count, 0);
    assert_eq!(shell.assisted_ai_projection.request_count, 1);
    assert_eq!(
        shell.assisted_ai_projection.requests[0].operation_class,
        legion_protocol::AssistedAiOperationClass::Explain
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn assisted_ai_propose_is_proposal_only() {
    let root = create_root();
    let target = root.join("propose.rs");
    std::fs::write(&target, "fn main() {}\n").expect("seed file");

    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Assist);
    let (_opened, _file_id, buffer_id, _node, _preconditions) =
        opened_text_file(&mut app, &root, "propose.rs");
    let before_editor = app
        .editor()
        .text(buffer_id)
        .expect("initial editor")
        .to_string();
    let before_disk = std::fs::read_to_string(&target).expect("initial disk");

    let outcome = ai_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "add guard".to_string(),
        })
        .expect("assisted proposal starts"),
    );

    let proposal_id = outcome.proposal_id.expect("proposal id");
    assert!(matches!(
        outcome.proposal_created,
        Some(ProposalResponse::Created(_))
    ));
    assert_eq!(
        app.editor().text(buffer_id).expect("editor after AI"),
        before_editor
    );
    assert_eq!(
        std::fs::read_to_string(&target).expect("disk after AI"),
        before_disk
    );

    let shell = app
        .shell_projection_snapshot("assisted proposal")
        .expect("shell projection");
    assert!(
        shell
            .proposal_ledger_projection
            .rows
            .iter()
            .any(|row| row.proposal_id == proposal_id
                && row.lifecycle.state == ProposalLifecycleState::Created)
    );
    assert_eq!(shell.assisted_ai_projection.preview_ready_count, 1);
    assert_eq!(
        shell.assisted_ai_projection.requests[0].operation_class,
        legion_protocol::AssistedAiOperationClass::ProposeEdit
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn assisted_ai_refusals_visible_for_untrusted_workspace() {
    let root = create_root();
    let target = root.join("refusal.rs");
    std::fs::write(&target, "fn main() {}\n").expect("seed file");

    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Assist);
    app.open_workspace(
        &root,
        WorkspaceTrustState::Untrusted,
        PrincipalId("untrusted".to_string()),
    )
    .expect("open untrusted workspace");
    app.open_file(target.to_string_lossy())
        .expect("open target file");

    let outcome = ai_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::StartAiExplain {
            instruction_label: "explain untrusted".to_string(),
        })
        .expect("assisted refusal returns outcome"),
    );

    assert_eq!(outcome.proposal_id, None);
    assert!(outcome.proposal_created.is_none());
    assert_eq!(
        outcome.route_response.invocation_state,
        legion_protocol::AssistedAiProviderInvocationState::Refused
    );
    assert!(outcome.refusal.is_some());

    let shell = app
        .shell_projection_snapshot("assisted refusal")
        .expect("shell projection");
    assert!(shell.assisted_ai_projection.refusal_count >= 1);
    assert!(
        shell
            .assisted_ai_projection
            .refusals
            .iter()
            .any(|refusal| refusal.reason_code == "capability.denied")
    );
    assert!(shell.proposal_ledger_projection.rows.is_empty());

    let _ = std::fs::remove_dir_all(&root);
}
