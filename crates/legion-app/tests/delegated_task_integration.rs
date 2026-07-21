use std::{fs, path::PathBuf};

use legion_ai::tool_calls::ScriptedToolCallingProviderBuilder;
#[cfg(feature = "ai")]
use legion_app::AppDelegatedToolHost;
use legion_app::{
    AppComposition, AppDelegatedTaskExecutionOutcome, AppDelegatedTaskOutcome, AppProductMode,
};
use legion_protocol::{
    CanonicalPath, CausalityId, CorrelationId, DelegatedTaskPlanContract, DelegatedTaskPlanId,
    DelegatedTaskPlanningBoundaryInput, DelegatedTaskProposalHunkDisposition,
    DelegatedTaskRiskTolerance, DelegatedTaskRuntimeActivationState, DelegatedTaskScope,
    DelegatedTaskScopeTargetKind, DelegatedTaskToolPermissionDecision, FileFingerprint,
    LegionToolKind, PrincipalId, ProposalPayload, TimestampMillis, WorkspaceId,
    WorkspaceTrustState, delegated_task_plan_from_boundary_input,
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

/// Returns the expected sandbox path when a workspace root is known.
/// After PKT-WORKTREE D2, sandbox paths are derived from the workspace root,
/// not CWD, so callers that opened a workspace must pass it here.
fn sandbox_path_in(workspace_root: &std::path::Path, plan_id: &DelegatedTaskPlanId) -> PathBuf {
    workspace_root
        .join("target/delegated-tasks")
        .join(format!("task-{}", plan_id.0))
}

/// Fallback for tests that do not open a workspace: sandboxes fall back to CWD-relative paths.
fn sandbox_path_cwd(plan_id: &DelegatedTaskPlanId) -> PathBuf {
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
            assert!(!sandbox_path_in(&workspace_root, &plan_id).exists());
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
            // No workspace opened in this test: sandbox path falls back to CWD-relative.
            assert!(!sandbox_path_cwd(&plan_id).exists());
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
            assert!(!sandbox_path_in(&workspace_root, &plan_id).exists());
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
            assert!(!sandbox_path_in(&workspace_root, &plan_id).exists());
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
    // Fixture path: live providers register Assist proposals asynchronously on poll.
    app.set_preferred_ai_provider(legion_app::ProductAiProviderPreference::Deterministic);
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
    // Keep offline/sync fixture path so CI does not depend on Ollama/BYOK.
    app.set_preferred_ai_provider(legion_app::ProductAiProviderPreference::Deterministic);

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

/// Build a repo-scoped `DelegatedTaskScope` for test workspace at `root`.
fn test_scope(root: &std::path::Path) -> DelegatedTaskScope {
    DelegatedTaskScope {
        target_kind: DelegatedTaskScopeTargetKind::Repo,
        workspace_root: CanonicalPath(root.to_string_lossy().into_owned()),
        target_path: None,
        risk_tolerance: DelegatedTaskRiskTolerance::Balanced,
        allowed_tools: vec![
            LegionToolKind::Read,
            LegionToolKind::Grep,
            LegionToolKind::Glob,
            LegionToolKind::Outline,
            LegionToolKind::EditAsProposal,
        ],
        forbidden_paths: vec![],
        schema_version: 1,
    }
}

#[test]
fn start_delegated_task_completes_with_scripted_end_turn() {
    let root = temp_workspace("start-task-complete");
    fs::write(root.join("hello.rs"), "fn hello() {}\n").expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("start-task-test".to_string()),
    )
    .expect("workspace should open");
    app.set_product_mode(AppProductMode::Delegate);

    let provider = ScriptedToolCallingProviderBuilder::new()
        .end_turn("Task complete: read the file and understood the structure.")
        .build("test-scripted");

    let outcome = app
        .start_delegated_task(
            "Describe the structure of hello.rs".to_string(),
            test_scope(&root),
            &provider,
        )
        .expect("start_delegated_task should succeed");

    match outcome {
        AppDelegatedTaskOutcome::Completed {
            final_message,
            proposals,
            audit_steps,
        } => {
            assert!(
                final_message.contains("Task complete"),
                "final message should include scripted text; got: {final_message}"
            );
            // TODO(PKT-PROPOSAL-SURFACE): proposals will be non-empty once DelegatedTaskLoopResult surfaces them
            assert_eq!(
                proposals.len(),
                0,
                "no proposals expected from end_turn only run"
            );
            assert!(
                !audit_steps.is_empty(),
                "at least one audit step should be recorded"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[test]
fn start_delegated_task_audit_steps_are_paired_for_tool_call() {
    use legion_protocol::DelegatedTaskLoopStepKind;

    let root = temp_workspace("start-task-paired");
    fs::write(root.join("target.rs"), "fn target() {}\n").expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("start-task-paired-test".to_string()),
    )
    .expect("workspace should open");
    app.set_product_mode(AppProductMode::Delegate);

    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("tool-1", "read", serde_json::json!({ "path": "target.rs" }))
        .end_turn("Read target.rs successfully.")
        .build("test-scripted-paired");

    let outcome = app
        .start_delegated_task(
            "Read target.rs and summarize".to_string(),
            test_scope(&root),
            &provider,
        )
        .expect("start_delegated_task should succeed");

    match outcome {
        AppDelegatedTaskOutcome::Completed { audit_steps, .. } => {
            // There must be a ToolCallRequest step paired with a ToolCallResult.
            let request_steps: Vec<_> = audit_steps
                .iter()
                .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRequest)
                .collect();
            let result_steps: Vec<_> = audit_steps
                .iter()
                .filter(|s| {
                    s.kind == DelegatedTaskLoopStepKind::ToolCallResult
                        || s.kind == DelegatedTaskLoopStepKind::ToolCallRejected
                })
                .collect();

            assert_eq!(
                request_steps.len(),
                result_steps.len(),
                "every ToolCallRequest must have a paired result/rejection"
            );

            for request in &request_steps {
                let paired = result_steps
                    .iter()
                    .any(|r| r.causality_id == request.causality_id);
                assert!(
                    paired,
                    "request with causality_id={} has no paired result",
                    request.causality_id
                );
            }
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[test]
fn start_delegated_task_rejects_manual_mode() {
    let root = temp_workspace("start-task-manual-reject");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("manual-reject-test".to_string()),
    )
    .expect("workspace should open");
    // Manual mode (default): should reject.

    let provider = ScriptedToolCallingProviderBuilder::new()
        .end_turn("should not reach here")
        .build("test-scripted-reject");

    let err = app
        .start_delegated_task(
            "attempt in manual mode".to_string(),
            test_scope(&root),
            &provider,
        )
        .expect_err("manual mode should reject start_delegated_task");

    assert!(
        err.to_string().contains("Delegate dispatch requires"),
        "error should mention delegate requirement; got: {err}"
    );
}

#[test]
fn reap_orphaned_delegated_task_sandboxes_removes_preseeded_orphan_and_reports_it() {
    let reap_root =
        std::env::temp_dir().join(format!("legion_app_reap_test_{}", uuid::Uuid::now_v7()));
    fs::create_dir_all(reap_root.join("task-orphan-plan")).expect("orphan dir should be created");
    fs::write(
        reap_root.join("task-orphan-plan/marker.txt"),
        "stale sandbox from a crashed lane",
    )
    .expect("marker file should be written");
    fs::create_dir_all(reap_root.join("not-a-task-dir")).expect("unrelated dir should be created");

    let removed = AppComposition::reap_orphaned_delegated_task_sandboxes_at(&reap_root)
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

#[test]
#[cfg(feature = "ai")]
fn app_delegated_tool_host_runs_echo_command() {
    use legion_agent::agent_loop::DelegatedToolHost;

    let tmp = temp_workspace("tool-host-echo");
    let host = AppDelegatedToolHost::new(tmp.root.clone(), std::collections::BTreeSet::new());

    let output = host
        .run_terminal_command("echo hello", None, None)
        .expect("echo should succeed");

    assert!(
        output.contains("hello"),
        "output should contain 'hello'; got: {output}"
    );
    assert!(
        output.contains("sandbox live enforcement:"),
        "tool host must surface live SandboxEnforcementReport; got: {output}"
    );
    assert!(
        host.last_enforcement_summary()
            .is_some_and(|s| s.contains("sandbox live enforcement:")),
        "last_enforcement_summary must be populated after spawn"
    );
}

#[test]
fn start_delegated_task_rejects_forbidden_path_read() {
    use legion_protocol::DelegatedTaskLoopStepKind;

    let root = temp_workspace("start-task-forbidden");
    fs::write(root.join("secrets.txt"), "top secret data\n")
        .expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("forbidden-path-test".to_string()),
    )
    .expect("workspace should open");
    app.set_product_mode(AppProductMode::Delegate);

    // Scope forbids reading secrets.txt. The loop resolves tool paths against
    // the sandbox worktree and then maps them back to workspace-absolute paths,
    // so the forbidden-path entry must be an absolute path.
    let scope = DelegatedTaskScope {
        forbidden_paths: vec![CanonicalPath(
            root.join("secrets.txt").to_string_lossy().into_owned(),
        )],
        ..test_scope(&root)
    };

    // Scripted provider: attempt to read the forbidden file, then end turn.
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use(
            "tool-forbidden",
            "read",
            serde_json::json!({ "path": "secrets.txt" }),
        )
        .end_turn("Done after forbidden read attempt.")
        .build("test-scripted-forbidden");

    let outcome = app
        .start_delegated_task("Try to read secrets.txt".to_string(), scope, &provider)
        .expect("start_delegated_task should succeed even with a rejected tool call");

    // A non-retryable ScopeDenied rejection stops the loop with Blocked.
    // The audit_steps carried by Blocked must include the ToolCallRejected entry.
    match outcome {
        AppDelegatedTaskOutcome::Blocked { audit_steps, .. } => {
            let rejected_steps: Vec<_> = audit_steps
                .iter()
                .filter(|s| s.kind == DelegatedTaskLoopStepKind::ToolCallRejected)
                .collect();
            assert!(
                !rejected_steps.is_empty(),
                "at least one ToolCallRejected step expected when forbidden path is accessed; \
                 got audit steps: {audit_steps:?}"
            );
        }
        other => panic!("expected Blocked (scope denial is non-retryable), got {other:?}"),
    }
}

/// End-to-end integration test for the proposal surface path:
/// scripted provider → edit-as-proposal → proposals.len()==1 →
/// id resolves in the ledger projection → review_delegate_proposal_hunk succeeds.
///
/// This test was required by the PKT-PROPOSAL-SURFACE task brief and exercises the
/// fix for the silently-discarded register_proposal_lifecycle error (Finding 1):
/// a proposal that fails registration would not appear in the ledger and
/// review_delegate_proposal_hunk would return "proposal not found".
#[test]
fn start_delegated_task_surfaces_proposal_and_review_succeeds() {
    let root = temp_workspace("proposal-surface");
    fs::write(root.join("hello.rs"), "fn hello() -> u32 { 42 }\n")
        .expect("fixture file should be written");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("proposal-surface-test".to_string()),
    )
    .expect("workspace should open");
    app.set_product_mode(AppProductMode::Delegate);

    // Scripted provider: read the file, then propose an edit via edit-as-proposal.
    let provider = ScriptedToolCallingProviderBuilder::new()
        .tool_use("t1", "read", serde_json::json!({ "path": "hello.rs" }))
        .tool_use(
            "t2",
            "edit-as-proposal",
            serde_json::json!({
                "path": "hello.rs",
                "replacement": "fn hello() -> u32 { 99 }\n"
            }),
        )
        .end_turn("Proposed an edit to hello.rs.")
        .build("test-scripted-proposal-surface");

    let outcome = app
        .start_delegated_task(
            "Edit hello.rs to return 99".to_string(),
            test_scope(&root),
            &provider,
        )
        .expect("start_delegated_task should succeed");

    match outcome {
        AppDelegatedTaskOutcome::Completed { proposals, .. } => {
            // The edit-as-proposal tool call must surface exactly one proposal.
            assert_eq!(
                proposals.len(),
                1,
                "expected 1 proposal from edit-as-proposal; got {}",
                proposals.len()
            );

            let proposal = &proposals[0];

            // The proposal must reference hello.rs.
            let targets_hello = match &proposal.payload {
                ProposalPayload::CreateFile(p) => {
                    p.path.0.ends_with("hello.rs") || p.path.0.contains("hello.rs")
                }
                _ => false,
            };
            assert!(
                targets_hello,
                "proposal should target hello.rs; got: {:?}",
                proposal.payload
            );

            // The proposal must be resolvable in the app's ledger. A phantom proposal
            // (one where register_proposal_lifecycle was silently discarded) would cause
            // review_delegate_proposal_hunk to return "proposal not found".
            // Retrieve the hunk_id from the shell projection rather than constructing
            // it manually: the exact chunk id format is an implementation detail.
            let proposal_id = proposal.proposal_id;
            let snapshot = app
                .shell_projection_snapshot("proposal-surface-review")
                .expect("snapshot should build");
            let review = snapshot
                .delegated_task_projection
                .proposal_reviews
                .iter()
                .find(|review| review.proposal_id == proposal_id)
                .expect(
                    "registered proposal must appear in the ledger projection — \
                     if registration was silently discarded no review would be projected",
                );
            let hunk_id = review
                .hunks
                .first()
                .expect("at least one hunk should be projected for the edit-as-proposal")
                .hunk_id
                .clone();
            app.review_delegate_proposal_hunk(
                proposal_id,
                hunk_id,
                DelegatedTaskProposalHunkDisposition::Accepted,
            )
            .expect(
                "proposal hunk must be reviewable via the app ledger — \
                 if registration was silently discarded this call would fail with \
                 'proposal not found'",
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}
