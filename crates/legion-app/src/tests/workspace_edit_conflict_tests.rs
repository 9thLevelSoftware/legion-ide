use super::*;
use legion_protocol::{
    TextEdit, TextOffset, TextRange, WorkspaceEditSourceKind, WorkspaceTextEdit,
};

#[test]
fn workspace_edit_late_fingerprint_failure_rolls_back_prior_editor_mutation() {
    let root = std::env::temp_dir().join(format!(
        "legion-workspace-edit-late-conflict-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&root).expect("create test root");
    let first = root.join("first.txt");
    let second = root.join("second.txt");
    std::fs::write(&first, "one").expect("seed first");
    std::fs::write(&second, "two").expect("seed second");

    let mut app = AppComposition::new();
    let opened = app
        .open_workspace(
            &root,
            WorkspaceTrustState::Trusted,
            PrincipalId("late-conflict-test".to_string()),
        )
        .expect("open workspace");
    let first_file_id = app.open_file(first.to_string_lossy()).expect("open first");
    let first_buffer_id = app.active_buffer_id().expect("first buffer");
    app.edit_active_buffer(legion_editor::TextEdit::insert(
        legion_editor::TextPosition::new(0, 3),
        "-dirty",
    ))
    .expect("make first buffer dirty");
    let first_context = app
        .active_file_version_context(first_buffer_id)
        .expect("first version context");

    let second_file_id = app
        .open_file(second.to_string_lossy())
        .expect("open second");
    let second_buffer_id = app.active_buffer_id().expect("second buffer");
    let second_context = app
        .active_file_version_context(second_buffer_id)
        .expect("second version context");

    let first_identity = FileIdentity {
        file_id: first_file_id,
        workspace_id: opened.workspace_id,
        canonical_path: CanonicalPath(first.to_string_lossy().into_owned()),
        content_version: FileContentVersion(1),
        content_hash: None,
    };
    let second_identity = FileIdentity {
        file_id: second_file_id,
        workspace_id: opened.workspace_id,
        canonical_path: CanonicalPath(second.to_string_lossy().into_owned()),
        content_version: FileContentVersion(1),
        content_hash: None,
    };
    let preconditions = |context: &VersionContext| ProposalVersionPreconditions {
        file_version: Some(context.file_version),
        buffer_version: Some(context.buffer_version),
        snapshot_id: Some(context.snapshot_id),
        generation: Some(context.generation),
        file_content_version: Some(context.file_content_version),
        workspace_generation: Some(context.workspace_generation),
        expected_fingerprint: context.fingerprint.clone(),
        expected_file_length: context.file_length,
        expected_modified_at: context.modified_at,
    };
    let first_preconditions = preconditions(&first_context);
    let second_preconditions = preconditions(&second_context);
    let target = |id: &str, file_id: FileId, buffer_id: BufferId, path: &std::path::Path| {
        ProposalAffectedTarget {
            target_id: id.to_string(),
            kind: ProposalTargetKind::OpenBuffer,
            workspace_id: Some(opened.workspace_id),
            file_id: Some(file_id),
            buffer_id: Some(buffer_id),
            path: Some(CanonicalPath(path.to_string_lossy().into_owned())),
            terminal_session_id: None,
            plugin_id: None,
            remote_authority: None,
            collaboration_session_id: None,
            byte_ranges: Vec::new(),
            redaction_hints: Vec::new(),
        }
    };
    let payload = ProposalPayload::WorkspaceEdit(WorkspaceEditProposalPayload {
        workspace_id: opened.workspace_id,
        edit_id: uuid::Uuid::now_v7(),
        title: "late fingerprint failure".to_string(),
        source: WorkspaceEditSourceKind::User,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![
                target("first", first_file_id, first_buffer_id, &first),
                target("second", second_file_id, second_buffer_id, &second),
            ],
            omitted_target_count: 0,
            redaction_hints: Vec::new(),
        },
        file_edits: vec![
            WorkspaceTextEdit {
                file: first_identity,
                buffer_id: Some(first_buffer_id),
                edits: EditBatch {
                    edits: vec![TextEdit {
                        range: TextRange::new(TextOffset::byte(0), TextOffset::byte(3)),
                        replacement: "ONE".to_string(),
                    }],
                },
                preconditions: first_preconditions,
            },
            WorkspaceTextEdit {
                file: second_identity,
                buffer_id: Some(second_buffer_id),
                edits: EditBatch {
                    edits: vec![TextEdit {
                        range: TextRange::new(TextOffset::byte(0), TextOffset::byte(3)),
                        replacement: "TWO".to_string(),
                    }],
                },
                preconditions: second_preconditions,
            },
        ],
        change_annotations: Vec::new(),
        file_operations: Vec::new(),
        required_capability: CapabilityId("editor.write".to_string()),
        diagnostics: Vec::new(),
        schema_version: 1,
    });
    let proposal = WorkspaceProposal {
        proposal_id: ProposalId(991),
        principal: PrincipalId("late-conflict-test".to_string()),
        capability: CapabilityId("editor.write".to_string()),
        correlation_id: CorrelationId(991),
        payload,
        preconditions: ProposalVersionPreconditions {
            file_version: None,
            buffer_version: None,
            snapshot_id: None,
            generation: Some(opened.generation),
            file_content_version: None,
            workspace_generation: Some(opened.generation),
            expected_fingerprint: None,
            expected_file_length: None,
            expected_modified_at: None,
        },
        preview: PreviewSummary {
            summary: "late fingerprint failure".to_string(),
            details: Vec::new(),
        },
        expires_at: None,
        created_at: TimestampMillis(991),
    };

    assert!(matches!(
        app.register_proposal_lifecycle(&proposal)
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
    let external_second = second.clone();
    app.set_workspace_edit_preflight_hook_for_test(move || {
        std::fs::write(&external_second, "external-second").expect("external second overwrite");
    });
    let response = app
        .handle_proposal_request(ProposalRequest::Apply(proposal))
        .expect("apply late-conflict proposal");
    assert!(
        matches!(
            response,
            ProposalResponse::Stale { .. } | ProposalResponse::Failed { .. }
        ),
        "late fingerprint failure must refuse the batch: {response:?}"
    );
    assert_eq!(
        app.editor().text(first_buffer_id).expect("first text"),
        "one-dirty"
    );
    assert!(
        app.editor()
            .buffer_version(first_buffer_id)
            .expect("first buffer version after rollback")
            .0
            > first_context.buffer_version.0,
        "first edit must have been applied and then rolled back with monotonic versioning"
    );
    assert!(app.editor().is_dirty(first_buffer_id).expect("first dirty"));
    assert_eq!(
        app.editor().text(second_buffer_id).expect("second text"),
        "two"
    );
    assert!(
        !app.editor()
            .is_dirty(second_buffer_id)
            .expect("second clean")
    );
    assert_eq!(std::fs::read_to_string(&first).expect("first disk"), "one");
    assert_eq!(
        std::fs::read_to_string(&second).expect("second disk"),
        "external-second"
    );
    let _ = std::fs::remove_dir_all(root);
}
