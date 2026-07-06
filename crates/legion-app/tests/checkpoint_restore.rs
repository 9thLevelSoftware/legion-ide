use std::sync::atomic::{AtomicU64, Ordering};

use legion_app::AppComposition;
use legion_editor::{TextEdit, TextPosition};
use legion_protocol::{
    CanonicalPath, CapabilityId, CausalityId, CorrelationId, CreateFileProposal, EditBatch,
    FileTreeNode, PreviewSummary, PrincipalId, ProposalId, ProposalLifecycleAction,
    ProposalLifecycleCommand, ProposalLifecycleCommandReason, ProposalLifecycleState,
    ProposalPayload, ProposalRequest, ProposalResponse, ProposalRollbackReason,
    ProposalVersionPreconditions, TextOffset, TextRange, TimestampMillis, WorkspaceGeneration,
    WorkspaceId, WorkspacePort, WorkspaceProposal, WorkspaceRequest, WorkspaceResponse,
    WorkspaceTrustState,
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

// ---------------------------------------------------------------------------
// Helper: build a CreateFile proposal that requires `fs.write`
// ---------------------------------------------------------------------------

fn create_file_proposal(
    proposal_id: u64,
    target_path: CanonicalPath,
    workspace_generation: WorkspaceGeneration,
) -> WorkspaceProposal {
    WorkspaceProposal {
        proposal_id: ProposalId(proposal_id),
        principal: PrincipalId("trusted".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(proposal_id),
        payload: ProposalPayload::CreateFile(CreateFileProposal {
            path: target_path,
            initial_content: Some(format!("content-of-proposal-{proposal_id}")),
        }),
        preconditions: ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: None,
            file_content_version: None,
            workspace_generation: Some(workspace_generation),
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: format!("checkpoint test proposal {proposal_id}"),
            details: vec![],
        },
        expires_at: None,
        created_at: TimestampMillis(proposal_id),
    }
}

// ---------------------------------------------------------------------------
// Task 1: Durable checkpoint auto-creation on proposal apply
// ---------------------------------------------------------------------------

/// Applying a CreateFile proposal auto-creates a durable checkpoint that
/// records the created path and can be loaded back from the store.
#[test]
fn checkpoint_auto_created_on_file_mutation_proposal_apply() {
    let root = create_root();
    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");

    // Enable persistence so the checkpoint is also written to disk.
    app.enable_checkpoint_persistence(&root);

    let target = CanonicalPath(root.join("auto-ckpt.txt").to_string_lossy().into_owned());
    let proposal = create_file_proposal(801, target.clone(), opened.generation);

    register_validate_preview(&mut app, &proposal);
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply create-file proposal");
    assert!(matches!(response, ProposalResponse::Applied(_)));

    // File should now exist on disk.
    assert!(
        std::path::Path::new(&target.0).exists(),
        "created file must exist"
    );

    // A checkpoint must have been created automatically.
    let checkpoints = app.list_checkpoints();
    assert_eq!(checkpoints.len(), 1, "one checkpoint expected after apply");
    let summary = &checkpoints[0];
    assert_eq!(summary.proposal_id, ProposalId(801));
    assert!(summary.available, "checkpoint must be available");
    assert_eq!(summary.target_count, 1);

    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// Task 2: Checkpoint timeline with restore of middle checkpoint
// ---------------------------------------------------------------------------

/// Applying 3 proposals creates 3 checkpoints. Restoring the middle checkpoint
/// reverts its file to pre-apply state while the other files remain.
#[test]
fn checkpoint_timeline_and_restore_middle() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");

    let file1 = CanonicalPath(root.join("timeline-1.txt").to_string_lossy().into_owned());
    let file2 = CanonicalPath(root.join("timeline-2.txt").to_string_lossy().into_owned());
    let file3 = CanonicalPath(root.join("timeline-3.txt").to_string_lossy().into_owned());

    // Apply 3 proposals sequentially. Re-open the workspace before each to fetch the
    // current generation — the workspace actor returns `existing.generation` when it is
    // already open, so this is cheap and safe for a test with no open file buffers.
    for (pid, path) in [(901u64, &file1), (902, &file2), (903, &file3)] {
        let current_gen = app
            .open_workspace(
                &root,
                WorkspaceTrustState::Trusted,
                PrincipalId("trusted".to_string()),
            )
            .expect("refresh workspace generation")
            .generation;
        let proposal = create_file_proposal(pid, path.clone(), current_gen);
        register_validate_preview(&mut app, &proposal);
        let response = app
            .handle_proposal_request(ProposalRequest::Apply(proposal))
            .expect("apply");
        assert!(matches!(response, ProposalResponse::Applied(_)));
    }

    let checkpoints = app.list_checkpoints();
    assert_eq!(checkpoints.len(), 3, "three checkpoints expected");

    // The list is newest-first; the checkpoints are for proposal 903, 902, 901.
    assert_eq!(checkpoints[0].proposal_id, ProposalId(903));
    assert_eq!(checkpoints[1].proposal_id, ProposalId(902));
    assert_eq!(checkpoints[2].proposal_id, ProposalId(901));

    // Restore the middle checkpoint (proposal 902 → file2).
    let middle_id = checkpoints[1].checkpoint_id.clone();
    app.restore_checkpoint(&middle_id).expect("restore middle");

    // file2 should be deleted (pre-apply state: didn't exist).
    assert!(
        !std::path::Path::new(&file2.0).exists(),
        "file2 must be gone after restore"
    );
    // file1 and file3 must remain untouched.
    assert!(
        std::path::Path::new(&file1.0).exists(),
        "file1 must still exist"
    );
    assert!(
        std::path::Path::new(&file3.0).exists(),
        "file3 must still exist"
    );

    // The restored checkpoint must be marked unavailable.
    let updated = app.list_checkpoints();
    let middle = updated
        .iter()
        .find(|c| c.checkpoint_id == middle_id)
        .expect("middle checkpoint still listed");
    assert!(!middle.available, "restored checkpoint must be unavailable");

    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// Task 3: Restore scoped to checkpoint targets; manual edits on other files
// preserved
// ---------------------------------------------------------------------------

/// Applying a 3-file proposal creates 3 files. A manually created 4th file
/// and manual writes to one of the proposal targets are handled correctly:
/// - The 3 proposal-created files are reverted (deleted) on restore.
/// - The 4th file (not a target) is untouched.
/// - The manual write to file1 after apply does not block restore.
#[test]
fn checkpoint_restore_scoped_to_targets_preserves_manual_edits() {
    let root = create_root();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("trusted".to_string()),
    )
    .expect("open workspace");

    let file1 = CanonicalPath(root.join("scope-1.txt").to_string_lossy().into_owned());
    let file2 = CanonicalPath(root.join("scope-2.txt").to_string_lossy().into_owned());
    let file3 = CanonicalPath(root.join("scope-3.txt").to_string_lossy().into_owned());
    let file4 = root.join("scope-4.txt"); // manually created, not a proposal target

    // Re-open before each apply to fetch the current workspace generation.
    for (pid, path) in [(1001u64, &file1), (1002, &file2), (1003, &file3)] {
        let current_gen = app
            .open_workspace(
                &root,
                WorkspaceTrustState::Trusted,
                PrincipalId("trusted".to_string()),
            )
            .expect("refresh workspace generation")
            .generation;
        let proposal = create_file_proposal(pid, path.clone(), current_gen);
        register_validate_preview(&mut app, &proposal);
        let response = app
            .handle_proposal_request(ProposalRequest::Apply(proposal))
            .expect("apply");
        assert!(matches!(response, ProposalResponse::Applied(_)));
    }

    let checkpoints = app.list_checkpoints();
    assert_eq!(checkpoints.len(), 3);

    // Manual writes performed AFTER the proposals applied.
    // file4 is entirely outside the checkpoint targets.
    std::fs::write(&file4, "manual-only content\n").expect("write file4");
    // file1 is a target — write extra content after the apply.
    std::fs::write(&file1.0, "manually overwritten after apply\n").expect("overwrite file1");

    // Restore the checkpoint for proposal 1001 (file1).
    let ckpt_id_1001 = checkpoints
        .iter()
        .find(|c| c.proposal_id == ProposalId(1001))
        .map(|c| c.checkpoint_id.clone())
        .expect("checkpoint for 1001");
    app.restore_checkpoint(&ckpt_id_1001)
        .expect("restore checkpoint 1001");

    // file1 was a target of proposal 1001 (CreatedFile) → it is deleted on restore.
    assert!(
        !std::path::Path::new(&file1.0).exists(),
        "file1 must be gone: it was a CreatedFile target"
    );

    // file4 is NOT a proposal target → manual content preserved.
    assert!(
        file4.exists(),
        "file4 must still exist: not a proposal target"
    );
    assert_eq!(
        std::fs::read_to_string(&file4).expect("read file4"),
        "manual-only content\n",
        "file4 content must be unchanged"
    );

    // file2 and file3 were targets of OTHER proposals → not touched by this restore.
    assert!(
        std::path::Path::new(&file2.0).exists(),
        "file2 must still exist: different proposal"
    );
    assert!(
        std::path::Path::new(&file3.0).exists(),
        "file3 must still exist: different proposal"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// Task 4: Checkpoint audit records are durable and queryable by proposal id
// ---------------------------------------------------------------------------

/// Applying a proposal writes a Created audit record; restoring writes a
/// Restored audit record. Both can be queried by proposal_id.
#[test]
fn checkpoint_audit_records_created_and_restored() {
    use legion_protocol::CheckpointAuditEvent;

    let root = create_root();
    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");

    // Enable disk-backed persistence so audits are written to `.legion/audit/`.
    app.enable_checkpoint_persistence(&root);

    let target = CanonicalPath(root.join("audit-test.txt").to_string_lossy().into_owned());
    let proposal_id = ProposalId(1101);
    let proposal = create_file_proposal(proposal_id.0, target.clone(), opened.generation);

    register_validate_preview(&mut app, &proposal);
    app.handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply");

    // One Created audit record must exist for this proposal.
    let audits_after_apply = app.query_checkpoint_audit(Some(proposal_id));
    assert_eq!(
        audits_after_apply.len(),
        1,
        "one audit record expected after apply"
    );
    assert_eq!(audits_after_apply[0].event, CheckpointAuditEvent::Created);
    assert_eq!(audits_after_apply[0].proposal_id, proposal_id);
    assert_eq!(audits_after_apply[0].target_paths.len(), 1);

    // Restore the checkpoint.
    let checkpoint_id = app
        .list_checkpoints()
        .into_iter()
        .next()
        .map(|c| c.checkpoint_id)
        .expect("checkpoint must exist");
    app.restore_checkpoint(&checkpoint_id).expect("restore");

    // Now two audit records: Created + Restored.
    let audits_after_restore = app.query_checkpoint_audit(Some(proposal_id));
    assert_eq!(
        audits_after_restore.len(),
        2,
        "two audit records expected after restore"
    );
    let events: Vec<_> = audits_after_restore.iter().map(|a| a.event).collect();
    assert!(
        events.contains(&CheckpointAuditEvent::Created),
        "Created audit record must be present"
    );
    assert!(
        events.contains(&CheckpointAuditEvent::Restored),
        "Restored audit record must be present"
    );

    // Query with a different proposal id returns nothing.
    let unrelated = app.query_checkpoint_audit(Some(ProposalId(9999)));
    assert!(
        unrelated.is_empty(),
        "unrelated proposal must yield no audits"
    );

    // The audit directory on disk must contain at least 2 JSON files.
    let audit_dir = root.join(".legion").join("audit");
    let audit_files = std::fs::read_dir(&audit_dir)
        .expect("audit dir must exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .count();
    assert!(
        audit_files >= 2,
        "at least 2 audit blobs expected on disk, got {audit_files}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// C2: Failure-path — restore error leaves checkpoint available, no audit
// ---------------------------------------------------------------------------

/// When the filesystem layer returns an error during restore (e.g. the target
/// path is a directory instead of a file), restore_checkpoint propagates the
/// error, leaves the checkpoint marked available, and writes no Restored audit.
#[test]
fn restore_failure_leaves_checkpoint_available_and_no_audit() {
    use legion_protocol::CheckpointAuditEvent;

    let root = create_root();
    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("trusted".to_string()),
        )
        .expect("open workspace");

    app.enable_checkpoint_persistence(&root);

    let target = CanonicalPath(root.join("fail-restore.txt").to_string_lossy().into_owned());
    let proposal_id = ProposalId(1201);
    let proposal = create_file_proposal(proposal_id.0, target.clone(), opened.generation);

    register_validate_preview(&mut app, &proposal);
    app.handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply");

    // Checkpoint and Created audit must exist.
    let checkpoints = app.list_checkpoints();
    assert_eq!(checkpoints.len(), 1);
    let ckpt_id = checkpoints[0].checkpoint_id.clone();
    assert!(checkpoints[0].available);

    let audits_before = app.query_checkpoint_audit(Some(proposal_id));
    assert_eq!(audits_before.len(), 1);
    assert_eq!(audits_before[0].event, CheckpointAuditEvent::Created);

    // Sabotage: replace the created file with a directory so `remove_file`
    // fails inside restore_files_for_checkpoint.
    let target_path = std::path::Path::new(&target.0);
    std::fs::remove_file(target_path).expect("remove created file");
    std::fs::create_dir_all(target_path.join("blocker")).expect("create blocking dir");

    // Restore must fail.
    let result = app.restore_checkpoint(&ckpt_id);
    assert!(
        result.is_err(),
        "restore should fail when the target is a directory"
    );

    // Checkpoint must still be available (not marked consumed).
    let checkpoints_after = app.list_checkpoints();
    let ckpt = checkpoints_after
        .iter()
        .find(|c| c.checkpoint_id == ckpt_id)
        .expect("checkpoint must still exist");
    assert!(
        ckpt.available,
        "checkpoint must remain available after failed restore"
    );

    // No Restored audit record should have been written.
    let audits_after = app.query_checkpoint_audit(Some(proposal_id));
    assert_eq!(
        audits_after.len(),
        1,
        "only the Created audit record should exist"
    );
    assert_eq!(audits_after[0].event, CheckpointAuditEvent::Created);

    let _ = std::fs::remove_dir_all(&root);
}
