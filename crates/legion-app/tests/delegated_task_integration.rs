use std::{fs, path::PathBuf};

use legion_app::{AppComposition, AppDelegatedTaskExecutionOutcome, AppProductMode};
use legion_protocol::{
    CausalityId, CorrelationId, DelegatedTaskPlanContract, DelegatedTaskPlanId,
    DelegatedTaskPlanningBoundaryInput, DelegatedTaskProposalHunkDisposition,
    DelegatedTaskRuntimeActivationState, DelegatedTaskToolPermissionDecision, FileFingerprint,
    PrincipalId, ProposalPayload, TimestampMillis, WorkspaceId, WorkspaceTrustState,
    delegated_task_plan_from_boundary_input,
};

fn delegated_plan_contract(plan_id: DelegatedTaskPlanId) -> DelegatedTaskPlanContract {
    let boundary_input = DelegatedTaskPlanningBoundaryInput {
        plan_id,
        workspace_id: Some(WorkspaceId(1)),
        objective_summary_hash: FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "test-hash".to_string(),
        },
        allowed_operation_classes: vec![],
        context_manifest: None,
        privacy_inspector: None,
        permission_budget_projection: None,
        approval_checklist: None,
        checkpoint_rollback: None,
        assisted_ai_projection: None,
        assisted_ai_required: false,
        affected_targets: vec![],
        steps: vec![],
        proposal_preview_links: vec![],
        workspace_trust_state: WorkspaceTrustState::Trusted,
        privacy_denied: false,
        permission_budget_denied: false,
        permission_budget_depleted: false,
        approval_checklist_valid: true,
        checkpoint_required: false,
        checkpoint_available: true,
        rollback_required: false,
        rollback_available: true,
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(uuid::Uuid::from_u128(1)),
        created_at: TimestampMillis(1),
        schema_version: 1,
    };
    delegated_task_plan_from_boundary_input(boundary_input)
}

fn unique_plan_id(label: &str) -> DelegatedTaskPlanId {
    DelegatedTaskPlanId(format!("{label}-{}", uuid::Uuid::now_v7()))
}

fn sandbox_path(plan_id: &DelegatedTaskPlanId) -> PathBuf {
    PathBuf::from("target/delegated-tasks").join(format!("task-{}", plan_id.0))
}

fn temp_workspace(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion_app_delegated_{label}_{}",
        uuid::Uuid::now_v7()
    ));
    fs::create_dir(&root).expect("temp workspace should be created");
    root
}

#[test]
fn execute_delegated_task_reports_missing_plan_without_error() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    let plan_id = unique_plan_id("missing-plan");

    let outcome = app
        .execute_delegated_task(&plan_id)
        .expect("missing plan is a structured outcome");

    match outcome {
        AppDelegatedTaskExecutionOutcome::PlanMissing { plan_id: missing } => {
            assert_eq!(missing, plan_id);
        }
        other => panic!("expected PlanMissing, got {other:?}"),
    }
}

#[test]
fn execute_delegated_task_waits_for_write_permission_before_sandbox_allocation() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    let plan_id = unique_plan_id("waiting-plan");
    let workspace_root = temp_workspace("waiting-plan");
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId(format!("delegate-test:{}", plan_id.0)),
    )
    .expect("workspace opens for projection snapshot");
    app.seed_delegated_task_plan_contracts(vec![delegated_plan_contract(plan_id.clone())]);

    let outcome = app
        .execute_delegated_task(&plan_id)
        .expect("permission wait is structured");

    match outcome {
        AppDelegatedTaskExecutionOutcome::WaitingForToolPermission { request } => {
            assert_eq!(
                request.decision,
                DelegatedTaskToolPermissionDecision::Confirm
            );
            assert!(!request.runtime_allowed);
            assert!(request.human_approval_required);
            assert!(!sandbox_path(&plan_id).exists());
            let snapshot = app
                .shell_projection_snapshot("Legion")
                .expect("projection snapshot is available");
            assert_eq!(
                snapshot.delegated_task_projection.runtime_activation,
                DelegatedTaskRuntimeActivationState::Planned
            );
        }
        other => panic!("expected WaitingForToolPermission, got {other:?}"),
    }
}

#[test]
fn manual_mode_rejects_delegated_task_execution() {
    let mut app = AppComposition::new();
    let plan_id = unique_plan_id("manual-reject");
    app.seed_delegated_task_plan_contracts(vec![delegated_plan_contract(plan_id.clone())]);

    let err = app
        .execute_delegated_task(&plan_id)
        .expect_err("manual mode rejects delegated execution");

    assert!(err.to_string().contains("Delegate dispatch requires"));
}

#[test]
fn execute_delegated_task_fails_closed_after_denied_permission() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    let plan_id = unique_plan_id("denied-plan");
    app.seed_delegated_task_plan_contracts(vec![delegated_plan_contract(plan_id.clone())]);
    let request_id = match app
        .execute_delegated_task(&plan_id)
        .expect("permission wait is structured")
    {
        AppDelegatedTaskExecutionOutcome::WaitingForToolPermission { request } => {
            request.request_id
        }
        other => panic!("expected WaitingForToolPermission, got {other:?}"),
    };

    app.record_delegate_tool_permission_decision(
        request_id.clone(),
        DelegatedTaskToolPermissionDecision::Deny,
    )
    .expect("deny decision is recorded");
    app.record_delegate_tool_permission_decision(
        request_id.clone(),
        DelegatedTaskToolPermissionDecision::Always,
    )
    .expect("later always decision keeps deny precedence");

    let outcome = app
        .execute_delegated_task(&plan_id)
        .expect("denied permission is structured");
    match outcome {
        AppDelegatedTaskExecutionOutcome::Denied { request } => {
            assert_eq!(request.request_id, request_id);
            assert_eq!(request.decision, DelegatedTaskToolPermissionDecision::Deny);
            assert!(request.deny_overrides);
            assert!(!request.runtime_allowed);
            assert!(!sandbox_path(&plan_id).exists());
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

#[test]
fn execute_delegated_task_returns_proposal_after_explicit_write_allow() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    let plan_id = unique_plan_id("approved-plan");
    let workspace_root = temp_workspace("approved-plan");
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId(format!("delegate-test:{}", plan_id.0)),
    )
    .expect("workspace opens for projection snapshot");
    app.seed_delegated_task_plan_contracts(vec![delegated_plan_contract(plan_id.clone())]);
    let request_id = match app
        .execute_delegated_task(&plan_id)
        .expect("permission wait is structured")
    {
        AppDelegatedTaskExecutionOutcome::WaitingForToolPermission { request } => {
            request.request_id
        }
        other => panic!("expected WaitingForToolPermission, got {other:?}"),
    };

    app.record_delegate_tool_permission_decision(
        request_id,
        DelegatedTaskToolPermissionDecision::Allow,
    )
    .expect("allow decision is recorded");

    let outcome = app
        .execute_delegated_task(&plan_id)
        .expect("approved execution succeeds");
    match outcome {
        AppDelegatedTaskExecutionOutcome::ProposalReady(proposal) => {
            assert!(proposal.correlation_id.0 > 0);
            assert!(!proposal.causality_id.0.is_nil());
            assert_ne!(proposal.provider_id, "provider-auto");
            assert_ne!(proposal.principal.0, "principal-auto");
            assert_eq!(
                proposal.request_id,
                format!("delegate:permission:{}:runtime", plan_id.0)
            );
            match &proposal.payload {
                ProposalPayload::CreateFile(create_file) => {
                    assert!(create_file.path.0.starts_with("delegated-task/"));
                    let content = create_file
                        .initial_content
                        .as_ref()
                        .expect("proposal content is derived from the plan");
                    assert!(content.contains("objective_hash=test-hash"));
                    assert!(!content.contains("modified content"));
                }
                other => panic!("expected CreateFile proposal, got {other:?}"),
            }
            assert!(!sandbox_path(&plan_id).exists());
            let snapshot = app
                .shell_projection_snapshot("Legion")
                .expect("projection snapshot is available");
            assert_eq!(
                snapshot.delegated_task_projection.runtime_activation,
                DelegatedTaskRuntimeActivationState::WaitingForApproval
            );
        }
        other => panic!("expected ProposalReady, got {other:?}"),
    }
}

#[test]
fn execute_delegated_task_uses_acp_host_command_and_projects_comm_stream() {
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Delegate);
    let plan_id = unique_plan_id("acp-host");
    let workspace_root = temp_workspace("acp-host");
    app.open_workspace(
        &workspace_root,
        WorkspaceTrustState::Trusted,
        PrincipalId(format!("delegate-test:{}", plan_id.0)),
    )
    .expect("workspace opens for projection snapshot");
    app.seed_delegated_task_plan_contracts(vec![delegated_plan_contract(plan_id.clone())]);
    app.set_acp_host_command(
        PathBuf::from("/bin/sh"),
        vec![
            "-c".to_string(),
            r#"mkdir -p "$(dirname "$LEGION_ACP_TARGET_PATH")"; printf 'external-agent=claude-code\nplan=%s\n' "$LEGION_ACP_PLAN_ID" > "$LEGION_ACP_TARGET_PATH""#
                .to_string(),
        ],
    );

    let request_id = match app
        .execute_delegated_task(&plan_id)
        .expect("permission wait is structured")
    {
        AppDelegatedTaskExecutionOutcome::WaitingForToolPermission { request } => {
            request.request_id
        }
        other => panic!("expected WaitingForToolPermission, got {other:?}"),
    };

    app.record_delegate_tool_permission_decision(
        request_id,
        DelegatedTaskToolPermissionDecision::Allow,
    )
    .expect("allow decision is recorded");

    let outcome = app
        .execute_delegated_task(&plan_id)
        .expect("approved external host execution succeeds");
    match outcome {
        AppDelegatedTaskExecutionOutcome::ProposalReady(proposal) => {
            assert!(proposal.correlation_id.0 > 0);
            assert!(!proposal.causality_id.0.is_nil());
            match &proposal.payload {
                ProposalPayload::CreateFile(create_file) => {
                    let content = create_file
                        .initial_content
                        .as_ref()
                        .expect("proposal content is derived from the host output");
                    assert!(content.contains("external-agent=claude-code"));
                    assert!(content.contains(&plan_id.0));
                }
                other => panic!("expected CreateFile proposal, got {other:?}"),
            }
            assert!(!sandbox_path(&plan_id).exists());
            let snapshot = app
                .shell_projection_snapshot("Legion")
                .expect("projection snapshot is available");
            assert!(
                snapshot
                    .delegated_task_projection
                    .chat_messages
                    .iter()
                    .any(|message| {
                        message.role == legion_protocol::DelegatedTaskChatRole::System
                            && message.content_label.contains("acp.host.connect")
                    })
            );
            assert!(
                snapshot
                    .delegated_task_projection
                    .chat_messages
                    .iter()
                    .any(|message| {
                        message.role == legion_protocol::DelegatedTaskChatRole::System
                            && message.content_label.contains("acp.host.spawn")
                    })
            );
            assert!(
                snapshot
                    .delegated_task_projection
                    .chat_messages
                    .iter()
                    .any(|message| {
                        message.role == legion_protocol::DelegatedTaskChatRole::System
                            && message.content_label.contains("acp.host.terminate success")
                    })
            );
        }
        other => panic!("expected ProposalReady, got {other:?}"),
    }

    let _ = fs::remove_dir_all(workspace_root);
}

#[test]
fn delegate_hunk_review_updates_projection_counts_and_rejects_unknown_hunk() {
    let root = temp_workspace("hunk");
    fs::write(root.join("lib.rs"), "pub fn original() {}\n")
        .expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-hunk-test".to_string()),
    )
    .expect("workspace should open");
    app.open_file("lib.rs").expect("fixture file should open");
    app.set_product_mode(AppProductMode::Delegate);
    let proposal_id = app
        .start_ai_proposal("add delegated guard")
        .expect("proposal run should complete")
        .proposal_id
        .expect("proposal id should be present");
    let snapshot = app
        .shell_projection_snapshot("delegate-hunk")
        .expect("snapshot should build");
    let review = snapshot
        .delegated_task_projection
        .proposal_reviews
        .iter()
        .find(|review| review.proposal_id == proposal_id)
        .expect("proposal review should be projected");
    let hunk_id = review
        .hunks
        .first()
        .expect("at least one hunk should be projected")
        .hunk_id
        .clone();

    let accepted = app
        .review_delegate_proposal_hunk(
            proposal_id,
            hunk_id.clone(),
            DelegatedTaskProposalHunkDisposition::Accepted,
        )
        .expect("known hunk should be reviewable");
    let accepted_review = accepted
        .proposal_reviews
        .iter()
        .find(|review| review.proposal_id == proposal_id)
        .expect("accepted review should be present");
    assert_eq!(accepted_review.accepted_hunk_count, 1);
    assert_eq!(accepted_review.pending_hunk_count, 0);
    assert!(accepted_review.ready_for_apply);

    let rejected = app
        .review_delegate_proposal_hunk(
            proposal_id,
            hunk_id,
            DelegatedTaskProposalHunkDisposition::Rejected,
        )
        .expect("known hunk should remain reviewable");
    let rejected_review = rejected
        .proposal_reviews
        .iter()
        .find(|review| review.proposal_id == proposal_id)
        .expect("rejected review should be present");
    assert_eq!(rejected_review.rejected_hunk_count, 1);
    assert_eq!(rejected_review.pending_hunk_count, 0);
    assert!(!rejected_review.ready_for_apply);

    assert!(
        app.review_delegate_proposal_hunk(
            proposal_id,
            "missing-hunk",
            DelegatedTaskProposalHunkDisposition::Accepted,
        )
        .is_err()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn delegate_chat_projects_rag_citations_without_raw_source_payload() {
    let root = temp_workspace("chat");
    fs::write(
        root.join("lib.rs"),
        "pub fn delegated_marker() -> u32 {\n    42\n}\n",
    )
    .expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("delegate-chat-test".to_string()),
    )
    .expect("workspace should open");
    app.open_file("lib.rs").expect("fixture file should open");
    app.set_product_mode(AppProductMode::Delegate);

    let outcome = app
        .send_delegate_chat("explain delegated_marker")
        .expect("delegate chat should complete");

    assert_eq!(outcome.projection.chat_message_count, 2);
    assert!(outcome.citation_count > 0);
    assert!(outcome.projection.chat_messages.iter().any(|message| {
        message.role == legion_protocol::DelegatedTaskChatRole::Assistant
            && message
                .content_label
                .contains("Delegate provider answer ready")
    }));
    let citation = outcome
        .projection
        .context_citations
        .first()
        .expect("at least one citation should be projected");
    assert!(
        citation
            .path
            .as_ref()
            .is_some_and(|path| path.0.ends_with("lib.rs"))
    );
    assert!(citation.byte_range.is_some());
    assert!(citation.chunk_hash.is_some());
    assert!(
        outcome
            .projection
            .context_citations
            .iter()
            .all(|citation| !citation.metadata_label.contains("42"))
    );
    assert_eq!(outcome.projection.tool_permission_request_count, 1);

    let _ = fs::remove_dir_all(root);
}
