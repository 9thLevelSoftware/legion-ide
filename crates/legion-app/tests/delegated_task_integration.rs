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

#[cfg(windows)]
fn acp_host_command() -> (PathBuf, Vec<String>) {
    (
        PathBuf::from("powershell"),
        vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            r#"$ErrorActionPreference = 'Stop'; New-Item -ItemType Directory -Force -Path $env:LEGION_ACP_TARGET_DIR | Out-Null; @('external-agent=claude-code', "plan=$env:LEGION_ACP_PLAN_ID") | Set-Content -LiteralPath $env:LEGION_ACP_TARGET_PATH -Encoding UTF8"#
                .to_string(),
        ],
    )
}

#[cfg(not(windows))]
fn acp_host_command() -> (PathBuf, Vec<String>) {
    (
        PathBuf::from("/bin/sh"),
        vec![
            "-c".to_string(),
            r#"mkdir -p "$(dirname "$LEGION_ACP_TARGET_PATH")"; printf 'external-agent=claude-code\nplan=%s\n' "$LEGION_ACP_PLAN_ID" > "$LEGION_ACP_TARGET_PATH""#
                .to_string(),
        ],
    )
}

/// Drop-guarded temporary workspace. Removes the directory on drop with a
/// prefix/location check so a panic mid-test never leaks the temp dir.
struct TempWorkspace {
    root: PathBuf,
}

impl std::ops::Deref for TempWorkspace {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.root
    }
}

impl AsRef<std::path::Path> for TempWorkspace {
    fn as_ref(&self) -> &std::path::Path {
        &self.root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_app_delegated_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn temp_workspace(label: &str) -> TempWorkspace {
    let root = std::env::temp_dir().join(format!(
        "legion_app_delegated_{label}_{}",
        uuid::Uuid::now_v7()
    ));
    fs::create_dir(&root).expect("temp workspace should be created");
    TempWorkspace { root }
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
    let (program, args) = acp_host_command();
    app.set_acp_host_command(program, args);

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
}

#[test]
fn reap_orphaned_delegated_task_sandboxes_removes_preseeded_orphan_and_reports_it() {
    let reap_root = std::env::temp_dir().join(format!(
        "legion_app_reap_test_{}",
        uuid::Uuid::now_v7()
    ));
    fs::create_dir_all(reap_root.join("task-orphan-plan")).expect("orphan dir should be created");
    fs::write(
        reap_root.join("task-orphan-plan/marker.txt"),
        "stale sandbox from a crashed lane",
    )
    .expect("marker file should be written");
    fs::create_dir_all(reap_root.join("not-a-task-dir"))
        .expect("unrelated dir should be created");

    let app = AppComposition::new();
    let removed = app
        .reap_orphaned_delegated_task_sandboxes_at(&reap_root)
        .expect("reap should succeed");

    assert_eq!(removed.len(), 1);
    assert!(removed[0].ends_with("task-orphan-plan"));
    assert!(
        !reap_root.join("task-orphan-plan").exists(),
        "orphaned sandbox should be removed"
    );
    assert!(
        reap_root.join("not-a-task-dir").exists(),
        "non-task directories must be left untouched"
    );

    let _ = fs::remove_dir_all(&reap_root);
}
