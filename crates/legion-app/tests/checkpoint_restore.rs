use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::AppComposition;
use legion_editor::{TextEdit, TextPosition};
use legion_protocol::{
    CapabilityId, CausalityId, CorrelationId, EditBatch, FileTreeNode, PreviewSummary, PrincipalId,
    ProposalId, ProposalLifecycleAction, ProposalLifecycleCommand, ProposalLifecycleCommandReason,
    ProposalLifecycleState, ProposalPayload, ProposalRequest, ProposalResponse,
    ProposalRollbackReason, ProposalVersionPreconditions, TextOffset, TextRange, TimestampMillis,
    WorkspaceGeneration, WorkspaceId, WorkspacePort, WorkspaceProposal, WorkspaceRequest,
    WorkspaceResponse, WorkspaceTrustState,
};
use uuid::Uuid;

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-app-checkpoint-restore-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_millis() as u64)
            + TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
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
            summary: "checkpoint restore test".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(1),
    }
}

fn register_validate_preview(app: &mut AppComposition, proposal: &WorkspaceProposal) {
    assert!(matches!(
        app.register_proposal_lifecycle(proposal)
            .expect("register proposal lifecycle"),
        ProposalResponse::Created(_)
    ));
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Validate(proposal.clone()))
            .expect("validate proposal"),
        ProposalResponse::Validated(_)
    ));
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Preview(proposal.clone()))
            .expect("preview proposal"),
        ProposalResponse::Previewed { .. }
    ));
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

#[test]
fn checkpoint_restore_preserves_non_conflicting_manual_edits_after_apply() {
    let root = create_root();
    let target = root.join("checkpoint-restore.txt");
    std::fs::write(&target, "seed\n").expect("seed file");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");
    let file_id = app
        .open_file(target.to_string_lossy())
        .expect("open target file");
    let buffer_id = app.active_buffer_id().expect("active buffer id");
    let node = workspace_node_by_name(&app, opened.workspace_id, "checkpoint-restore.txt");
    let mut preconditions = file_preconditions(&node, opened.generation);
    preconditions.buffer_version = Some(
        app.editor()
            .buffer_version(buffer_id)
            .expect("buffer version"),
    );
    preconditions.snapshot_id = Some(
        app.editor()
            .current_snapshot(buffer_id)
            .expect("current snapshot")
            .snapshot_id,
    );
    let proposal = proposal_envelope_with(
        ProposalId(704),
        "editor.write",
        ProposalPayload::TextEdit(legion_protocol::TextEditProposal {
            file_id,
            edits: EditBatch {
                edits: vec![legion_protocol::TextEdit {
                    range: TextRange::new(TextOffset::byte(0), TextOffset::byte(4)),
                    replacement: "sprout".to_string(),
                }],
            },
        }),
        preconditions,
    );

    register_validate_preview(&mut app, &proposal);
    assert!(matches!(
        app.handle_proposal_request(ProposalRequest::Apply(proposal.clone()))
            .expect("apply text edit proposal"),
        ProposalResponse::Applied(_)
    ));

    assert_eq!(
        app.editor()
            .text(buffer_id)
            .expect("buffer text after apply"),
        "sprout\n"
    );

    app.edit_active_buffer(TextEdit::insert(TextPosition::new(1, 0), "manual\n"))
        .expect("manual edit after apply");
    assert_eq!(
        app.editor()
            .text(buffer_id)
            .expect("buffer text after edit"),
        "sprout\nmanual\n"
    );

    let rollback = ProposalLifecycleCommand {
        proposal_id: proposal.proposal_id,
        action: ProposalLifecycleAction::Rollback,
        principal: proposal.principal.clone(),
        capability: proposal.capability.clone(),
        correlation_id: proposal.correlation_id,
        causality_id: CausalityId(Uuid::now_v7()),
        reason: Some(ProposalLifecycleCommandReason::Rollback(
            ProposalRollbackReason::UserRequested,
        )),
        diagnostics: Vec::new(),
        requested_at: TimestampMillis(3),
        schema_version: 1,
    };
    let response = app
        .handle_proposal_request(ProposalRequest::Rollback(rollback))
        .expect("rollback proposal lifecycle");

    assert!(matches!(response, ProposalResponse::RolledBack { .. }));
    assert_eq!(
        std::fs::read_to_string(&target).expect("read restored file"),
        "seed\n"
    );
    assert_eq!(
        app.editor()
            .text(buffer_id)
            .expect("buffer text after restore"),
        "sprout\nmanual\n"
    );
    assert!(
        app.editor()
            .is_dirty(buffer_id)
            .expect("dirty after restore")
    );

    let shell = app
        .shell_projection_snapshot("checkpoint restore ledger")
        .expect("shell projection after restore");
    let row = shell
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal.proposal_id)
        .expect("proposal ledger row after restore");
    assert_eq!(row.lifecycle.state, ProposalLifecycleState::RolledBack);

    let _ = std::fs::remove_dir_all(&root);
}
