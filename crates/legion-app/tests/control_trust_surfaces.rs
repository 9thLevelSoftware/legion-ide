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
    SaveConflictPolicy, SaveFileProposal, SaveIntent, SnapshotId, StorageRepositoryRequest,
    StorageRepositoryResponse, TextOffset, TextRange, TimestampMillis, TrustDecisionContext,
    WorkspaceGeneration, WorkspaceId, WorkspacePort, WorkspaceProposal, WorkspaceRequest,
    WorkspaceResponse, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-app-control-trust-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64),
        TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("create temp root");
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
            if message.contains("requires Assist, Delegate, or Legion Workflows")
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
    // Fixture path: live Ollama/Anthropic would stream async and register on poll.
    app.set_preferred_ai_provider(legion_app::ProductAiProviderPreference::Deterministic);
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
            selection: None,
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

/// Read back the durable audit record one Assist run left behind.
///
/// The tracker ledger is write-only from here, so the phase-4 runtime record is
/// the terminal record a test can actually read -- which is also the record an
/// audit reads.
fn runtime_audit_record(
    app: &AppComposition,
    run_id: &legion_protocol::AgentRunId,
) -> legion_protocol::Phase4RuntimeAuditRecord {
    let route_id = run_id.0.replace("phase4-run-", "phase4-route-");
    match app
        .storage_port()
        .handle(StorageRepositoryRequest::ReadPhase4RuntimeAuditRecord(
            format!("phase4-runtime:{}:{route_id}", run_id.0),
        ))
        .expect("read phase4 runtime audit record")
    {
        StorageRepositoryResponse::Phase4RuntimeAuditRecord(record) => {
            record.expect("the run must have left a runtime audit record")
        }
        other => panic!("expected a phase4 runtime audit record, got {other:?}"),
    }
}

/// Set up an Assist workspace whose next run resolves against `answer`.
fn assist_app_with_injected_reply(
    root: &Path,
    file_name: &str,
    contents: &str,
    answer: &str,
) -> (AppComposition, BufferId) {
    std::fs::write(root.join(file_name), contents).expect("seed file");
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Assist);
    // Fixture path: the synchronous one, which is where the seam is read.
    app.set_preferred_ai_provider(legion_app::ProductAiProviderPreference::Deterministic);
    let (_opened, _file_id, buffer_id, _node, _preconditions) =
        opened_text_file(&mut app, root, file_name);
    app.inject_assist_reply_for_test(answer);
    (app, buffer_id)
}

/// A run that registers no proposal must not name one either.
///
/// The proposal id used to be allocated and written into the context manifest
/// before anyone knew whether an edit existed. When the reply turned out to be
/// unusable, the outcome and the replay and tracker records reported
/// `proposal_id: None` while the manifest, the privacy inspector and the
/// permission budget derived from it all named a proposal that was never
/// registered -- an id an audit would go looking for and never find.
#[test]
fn an_unresolved_assist_reply_names_no_proposal_in_any_projection() {
    let root = create_root();
    let (mut app, buffer_id) = assist_app_with_injected_reply(
        &root,
        "unresolved.rs",
        "fn main() {}\n",
        "I would suggest refactoring this, but here is no block.",
    );
    let before = app
        .editor()
        .text(buffer_id)
        .expect("initial editor")
        .to_string();

    let outcome = ai_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "add guard".to_string(),
            selection: None,
        })
        .expect("assisted proposal starts"),
    );

    assert_eq!(
        outcome.proposal_id, None,
        "no edit resolved, so no proposal"
    );
    assert!(outcome.proposal_created.is_none());
    assert_eq!(
        outcome.context_manifest_projection.manifest.proposal_id, None,
        "the manifest must not name a proposal that was never registered"
    );
    assert_eq!(
        outcome.privacy_inspector_projection.proposal_id, None,
        "and neither may anything derived from it"
    );
    assert_eq!(
        app.editor().text(buffer_id).expect("editor after run"),
        before,
        "an unresolved run leaves the buffer alone"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// An edit attempt that produced nothing is not recorded as an Explain.
///
/// The metadata-only path labelled every completed run
/// `phase5.explain.metadata_ready`, which was true while Explain was the only
/// operation that could reach it. A ProposeEdit run whose reply carried no
/// usable edit arrives there too, and the label said the run had never been
/// trying to edit anything -- while the reason it failed was dropped with the
/// source.
#[test]
fn an_unresolved_assist_run_is_recorded_as_an_edit_that_did_not_resolve() {
    let root = create_root();
    let (mut app, _buffer_id) = assist_app_with_injected_reply(
        &root,
        "labelled.rs",
        "fn main() {}\n",
        "no block here at all",
    );

    let outcome = ai_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "add guard".to_string(),
            selection: None,
        })
        .expect("assisted proposal starts"),
    );

    let record = runtime_audit_record(&app, &outcome.run_id);
    assert_eq!(
        record.outcome_label, "phase5.assist.edit_unresolved",
        "an edit attempt that produced nothing is not an Explain"
    );
    assert!(
        record
            .labels
            .iter()
            .any(|label| label.contains("no search/replace block")),
        "and the record must say why; got {:?}",
        record.labels
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// An edit that replaces text with itself is not offered for approval.
///
/// The span and the replacement are both non-empty, so the "changes nothing"
/// guard -- which tests emptiness of both -- registered it. Approving it runs
/// `EditorEngine::apply_edits`: version incremented, undo entry written, buffer
/// marked dirty, text exactly as it was.
#[test]
fn an_assist_edit_that_replaces_text_with_itself_registers_no_proposal() {
    let root = create_root();
    let (mut app, _buffer_id) = assist_app_with_injected_reply(
        &root,
        "identity.rs",
        "fn main() {\n    let total = 1;\n}\n",
        "<<<<<<< SEARCH\n    let total = 1;\n=======\n    let total = 1;\n>>>>>>> REPLACE\n",
    );

    let outcome = ai_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "tidy the total".to_string(),
            selection: None,
        })
        .expect("assisted proposal starts"),
    );

    assert_eq!(
        outcome.proposal_id, None,
        "an edit that changes no bytes is not approvable"
    );
    assert!(outcome.proposal_created.is_none());

    let record = runtime_audit_record(&app, &outcome.run_id);
    assert!(
        record
            .labels
            .iter()
            .any(|label| label.contains("identical to the text it would replace")),
        "the record must say why it was withdrawn; got {:?}",
        record.labels
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// A deletion is still a real edit, and still becomes a proposal.
///
/// The guard that rejects no-ops has now been wrong in both directions once --
/// first rejecting every deletion, then accepting identity edits -- so the
/// surviving case is pinned beside the rejected ones.
#[test]
fn an_assist_deletion_still_registers_a_proposal() {
    let root = create_root();
    let (mut app, _buffer_id) = assist_app_with_injected_reply(
        &root,
        "deletion.rs",
        "fn main() {\n    let unused = 1;\n}\n",
        "<<<<<<< SEARCH\n    let unused = 1;\n=======\n>>>>>>> REPLACE\n",
    );

    let outcome = ai_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "drop the unused binding".to_string(),
            selection: None,
        })
        .expect("assisted proposal starts"),
    );

    assert!(
        outcome.proposal_id.is_some(),
        "a deletion removes bytes, so it is an edit"
    );
    assert!(matches!(
        outcome.proposal_created,
        Some(ProposalResponse::Created(_))
    ));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn assisted_ai_spawn_failure_releases_pending_job_and_stream_lane() {
    let root = create_root();
    let target = root.join("assist-spawn-failure.rs");
    std::fs::write(&target, "fn main() {}\n").expect("seed file");

    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Assist);
    app.set_preferred_ai_provider(legion_app::ProductAiProviderPreference::Deterministic);
    let _ = opened_text_file(&mut app, &root, "assist-spawn-failure.rs");
    app.inject_assist_spawn_failure_for_test();

    let error = app
        .dispatch_ui_intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "exercise spawn rollback".to_string(),
            selection: None,
        })
        .expect_err("injected Assist worker spawn must fail");
    assert!(error.to_string().contains("spawn failure"));
    assert!(
        !app.has_pending_assist_proposal_for_test(),
        "failed spawn must not retain a pending Assist proposal"
    );
    assert!(
        !app.product_ai_stream_in_flight(),
        "failed spawn must release the shared product-AI lane"
    );

    let retry = ai_outcome(
        app.dispatch_ui_intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "retry after spawn rollback".to_string(),
            selection: None,
        })
        .expect("the released lane must accept the next Assist proposal"),
    );
    assert!(retry.proposal_id.is_some());

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

/// An org bundle that permits Assist mode but forbids provider invocation stops
/// the Assist lane too.
///
/// Assist became reachable from the rail in this PR, and it built its broker
/// from `product_ai_security_policy` rather than from the installed bundle. Mode
/// ceiling and provider ceiling are different questions: a bundle can perfectly
/// well say "Assist is fine, this provider is not", and passing the first check
/// is not passing the second. Until this was fixed, those newly reachable
/// commands could still send the buffer excerpt.
#[test]
fn an_org_provider_ceiling_refuses_assisted_ai_even_when_assist_mode_is_allowed() {
    use legion_security::{
        PolicyKeyring, PolicySigningKey, policy_bundle_verifying_key_b64, sign_policy_bundle,
    };

    const SEED: [u8; 32] = [23u8; 32];
    const KEY_ID: &str = "assist-ceiling-test-signer";

    // The shipped example, whose mode ceiling is Assist, with provider
    // invocation switched off. Editing the real bundle keeps the fixture honest
    // about the schema rather than testing a hand-rolled default.
    let payload = include_str!("../../../xtask/legion-policy.example.toml");
    assert!(
        payload.contains("provider_invocation_enabled = true"),
        "the example bundle no longer enables provider invocation, so switching it off \
         proves nothing"
    );
    let edited = payload.replace(
        "provider_invocation_enabled = true",
        "provider_invocation_enabled = false",
    );
    let keyring = PolicyKeyring::new(vec![PolicySigningKey {
        key_id: KEY_ID.to_string(),
        verifying_key_b64: policy_bundle_verifying_key_b64(&SEED),
    }]);
    let bundle = sign_policy_bundle(&edited, KEY_ID, &SEED)
        .verify(&keyring)
        .expect("a bundle this test signed must verify");

    let root = create_root();
    let target = root.join("ceiling.rs");
    std::fs::write(&target, "fn main() {}\n").expect("seed file");

    let mut app = AppComposition::new();
    app.set_org_policy_bundle(bundle);
    app.set_product_mode(AppProductMode::Assist);
    // Non-vacuity: the refusal below has to come from the provider ceiling, not
    // from a bundle that refused the mode and left the app in Manual.
    assert_eq!(
        app.product_mode(),
        AppProductMode::Assist,
        "the bundle's ceiling is Assist, so Assist mode must take effect"
    );
    let (_opened, _file_id, _buffer_id, _node, _preconditions) =
        opened_text_file(&mut app, &root, "ceiling.rs");

    let dispatched = app.dispatch_ui_intent(CommandDispatchIntent::StartAiExplain {
        instruction_label: "summarize context".to_string(),
    });

    // A hard error is an acceptable refusal; what must not happen is the run
    // being authorized. So this asserts only on the path where one started.
    if let Ok(outcome) = dispatched {
        let outcome = ai_outcome(outcome);
        assert!(
            outcome.refusal.is_some(),
            "the bundle disabled provider invocation and the Assist run was authorized \
             anyway, so the buffer excerpt went out under a policy that forbade it; \
             invocation state was {:?}",
            outcome.route_response.invocation_state
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}
