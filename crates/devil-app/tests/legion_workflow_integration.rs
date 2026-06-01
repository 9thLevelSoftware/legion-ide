use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use devil_agent::LegionWorkflowCoordinatorOutput;
use devil_app::AppComposition;
use devil_editor::{TextEdit, TextPosition};
use devil_protocol::{
    AssistedAiTrustProjectionKind, AssistedAiTrustProjectionReference, ByteRange, CausalityId,
    CommandRiskLabel, CorrelationId, DelegatedTaskAffectedTargetSummary,
    DelegatedTaskOperationClass, DelegatedTaskPlanId, DelegatedTaskPlanningBoundaryInput,
    FileFingerprint, LegionWorkflowConflictState, LegionWorkflowDependency,
    LegionWorkflowDependencyId, LegionWorkflowDependencyState, LegionWorkflowMergeApproval,
    LegionWorkflowMergeReadinessBlocker, LegionWorkflowMergeReadinessState,
    LegionWorkflowModelBackend, LegionWorkflowSession, LegionWorkflowSessionId,
    LegionWorkflowSignOff, LegionWorkflowSignOffId, LegionWorkflowSignOffState,
    LegionWorkflowState, LegionWorkflowVerificationGate, LegionWorkflowVerificationGateId,
    LegionWorkflowVerificationGateState, LegionWorkflowWorkerAssignment, LegionWorkflowWorkerId,
    LegionWorkflowWorkerRole, LegionWorkflowWorkerState, PrincipalId, PrivacyClassification,
    ProductMode, ProposalId, ProposalPrivacyLabel, ProposalRiskLabel, ProposalTargetKind,
    RedactionHint, TimestampMillis, WorkspaceId, WorkspaceTrustState,
    delegated_task_plan_from_boundary_input,
};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn causality(value: u128) -> CausalityId {
    CausalityId(uuid::Uuid::from_u128(value))
}

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.to_string(),
    }
}

fn trust_ref(reference_id: &str) -> AssistedAiTrustProjectionReference {
    AssistedAiTrustProjectionReference {
        reference_id: reference_id.to_string(),
        kind: AssistedAiTrustProjectionKind::ContextManifest,
        projection_hash: fingerprint(reference_id),
        schema_version: 1,
    }
}

fn affected_target(target_id: &str) -> DelegatedTaskAffectedTargetSummary {
    DelegatedTaskAffectedTargetSummary {
        target_id: target_id.to_string(),
        kind: ProposalTargetKind::MetadataOnly,
        workspace_id: None,
        file_id: None,
        buffer_id: None,
        ranges: vec![ByteRange::new(0, 0)],
        hashes: vec![fingerprint(target_id)],
        counts: Vec::new(),
        labels: vec![format!("target:{target_id}")],
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn worker(
    worker_id: &str,
    backend: LegionWorkflowModelBackend,
    plan_id: Option<DelegatedTaskPlanId>,
    target_id: &str,
    correlation: u64,
) -> LegionWorkflowWorkerAssignment {
    LegionWorkflowWorkerAssignment {
        worker_id: LegionWorkflowWorkerId(worker_id.to_string()),
        role: LegionWorkflowWorkerRole::Implementer,
        state: if backend == LegionWorkflowModelBackend::ProviderBacked {
            LegionWorkflowWorkerState::ProviderRouteRequired
        } else {
            LegionWorkflowWorkerState::Ready
        },
        model_backend: backend,
        display_safe_model_label: format!("model:{worker_id}"),
        allowed_command_classes: vec![DelegatedTaskOperationClass::DraftProposalMetadata],
        linked_delegated_plan_id: plan_id,
        assisted_ai_route: (backend == LegionWorkflowModelBackend::ProviderBacked)
            .then(|| trust_ref(&format!("route:{worker_id}"))),
        affected_targets: vec![affected_target(target_id)],
        risk_labels: vec![CommandRiskLabel::Review],
        privacy_labels: vec![PrivacyClassification::Metadata],
        correlation_id: CorrelationId(correlation),
        causality_id: causality(correlation as u128),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn verification_gate(state: LegionWorkflowVerificationGateState) -> LegionWorkflowVerificationGate {
    LegionWorkflowVerificationGate {
        gate_id: LegionWorkflowVerificationGateId("verification:unit".to_string()),
        state,
        label: "cargo test legion workflow".to_string(),
        evidence_artifact_id: (state == LegionWorkflowVerificationGateState::Passed)
            .then(|| "evidence:unit".to_string()),
        command_class_label: "cargo-test".to_string(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn signoff(state: LegionWorkflowSignOffState) -> LegionWorkflowSignOff {
    LegionWorkflowSignOff {
        sign_off_id: LegionWorkflowSignOffId("signoff:reviewer".to_string()),
        state,
        required_role: LegionWorkflowWorkerRole::Reviewer,
        reviewer_principal_id: (state == LegionWorkflowSignOffState::SignedOff)
            .then(|| PrincipalId("reviewer".to_string())),
        label: "reviewer sign-off".to_string(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn approval(approval_granted: bool) -> LegionWorkflowMergeApproval {
    LegionWorkflowMergeApproval {
        approval_artifact_id: Some("approval:unit".to_string()),
        approval_granted,
        rollback_available: true,
        audit_persisted_before_success: true,
        main_workspace_dirty_conflict: false,
        proposal_preconditions_stale: false,
        labels: vec!["approval.metadata".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn workflow_session(
    session_label: &str,
    workers: Vec<LegionWorkflowWorkerAssignment>,
    verification_gates: Vec<LegionWorkflowVerificationGate>,
    sign_off_records: Vec<LegionWorkflowSignOff>,
    proposal_ids: Vec<ProposalId>,
    merge_approval: Option<LegionWorkflowMergeApproval>,
) -> LegionWorkflowSession {
    LegionWorkflowSession {
        session_id: LegionWorkflowSessionId(format!("session:{session_label}")),
        directive_artifact_id: Some(format!("directive:{session_label}")),
        spec_artifact_id: Some(format!("spec:{session_label}")),
        task_graph_artifact_id: Some(format!("task-graph:{session_label}")),
        product_mode: ProductMode::LegionWorkflows,
        worker_assignments: workers,
        dependency_edges: Vec::new(),
        conflict_summaries: Vec::new(),
        verification_gates,
        sign_off_records,
        proposal_ids,
        merge_approval,
        lifecycle_state: LegionWorkflowState::Executing,
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
        correlation_id: CorrelationId(13),
        causality_id: causality(13),
    }
}

fn delegated_contract(plan_id: DelegatedTaskPlanId) -> devil_protocol::DelegatedTaskPlanContract {
    delegated_task_plan_from_boundary_input(DelegatedTaskPlanningBoundaryInput {
        plan_id,
        workspace_id: Some(WorkspaceId(1)),
        objective_summary_hash: fingerprint("delegated-objective"),
        allowed_operation_classes: vec![DelegatedTaskOperationClass::DraftProposalMetadata],
        context_manifest: None,
        privacy_inspector: None,
        permission_budget_projection: None,
        approval_checklist: None,
        checkpoint_rollback: None,
        assisted_ai_projection: None,
        assisted_ai_required: false,
        affected_targets: vec![affected_target("delegated-target")],
        steps: Vec::new(),
        proposal_preview_links: Vec::new(),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        privacy_denied: false,
        permission_budget_denied: false,
        permission_budget_depleted: false,
        approval_checklist_valid: true,
        checkpoint_required: false,
        checkpoint_available: true,
        rollback_required: false,
        rollback_available: true,
        correlation_id: CorrelationId(21),
        causality_id: causality(21),
        created_at: TimestampMillis(1),
        schema_version: 1,
    })
}

fn local_session(
    label: &str,
    approval_granted: bool,
) -> (LegionWorkflowSession, DelegatedTaskPlanId) {
    let plan_id = DelegatedTaskPlanId(format!("plan-{label}"));
    let session = workflow_session(
        label,
        vec![worker(
            "worker:local",
            LegionWorkflowModelBackend::Local,
            Some(plan_id.clone()),
            "target:local",
            31,
        )],
        vec![verification_gate(
            LegionWorkflowVerificationGateState::Passed,
        )],
        vec![signoff(LegionWorkflowSignOffState::SignedOff)],
        Vec::new(),
        Some(approval(approval_granted)),
    );
    (session, plan_id)
}

fn temp_workspace(label: &str) -> PathBuf {
    let id = TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::current_dir()
        .expect("current dir")
        .join("target")
        .join("legion-workflow-integration")
        .join(format!("{label}-{id}"));
    std::fs::create_dir_all(&root).expect("create temp workspace");
    std::fs::write(root.join("main.txt"), "clean\n").expect("write temp file");
    root
}

#[test]
fn legion_workflow_session_not_found_fails_closed() {
    let mut app = AppComposition::new();
    let err = app
        .execute_legion_workflow(&LegionWorkflowSessionId("session:missing".to_string()))
        .expect_err("missing session fails");
    assert!(err.to_string().contains("session:missing"));
}

#[test]
fn legion_workflow_local_worker_reaches_waiting_for_approval_metadata() {
    let mut app = AppComposition::new();
    let (session, plan_id) = local_session("waiting", false);
    let session_id = session.session_id.clone();
    app.seed_delegated_task_plan_contracts(vec![delegated_contract(plan_id)]);
    app.seed_legion_workflow_sessions(vec![session])
        .expect("seed workflow");

    let outcome = app
        .execute_legion_workflow(&session_id)
        .expect("execute workflow");

    assert_eq!(
        outcome.merge_readiness.state,
        LegionWorkflowMergeReadinessState::WaitingForApproval
    );
    assert!(
        outcome
            .merge_readiness
            .blockers
            .contains(&LegionWorkflowMergeReadinessBlocker::ApprovalRequired)
    );
    assert!(outcome.memory_candidate_proposed);
    assert_eq!(outcome.tracker_record_count, 1);
    assert!(outcome.outputs.iter().any(|output| {
        matches!(
            output,
            LegionWorkflowCoordinatorOutput::ProposalReady(proposal)
                if proposal.proposal_id.0 != 0
        )
    }));
    assert_eq!(outcome.projection.rows.len(), 1);
}

#[test]
fn legion_workflow_dependency_chain_resumes_without_rerunning_completed_worker() {
    let mut app = AppComposition::new();
    let root_plan_id = DelegatedTaskPlanId("plan-chain-root".to_string());
    let child_plan_id = DelegatedTaskPlanId("plan-chain-child".to_string());
    let mut session = workflow_session(
        "dependency-chain",
        vec![
            worker(
                "worker:root",
                LegionWorkflowModelBackend::Local,
                Some(root_plan_id.clone()),
                "target:chain-root",
                131,
            ),
            worker(
                "worker:child",
                LegionWorkflowModelBackend::Local,
                Some(child_plan_id.clone()),
                "target:chain-child",
                132,
            ),
        ],
        vec![verification_gate(
            LegionWorkflowVerificationGateState::Passed,
        )],
        vec![signoff(LegionWorkflowSignOffState::SignedOff)],
        Vec::new(),
        Some(approval(true)),
    );
    session.dependency_edges.push(LegionWorkflowDependency {
        dependency_id: LegionWorkflowDependencyId("dependency:root-child".to_string()),
        predecessor_worker_id: LegionWorkflowWorkerId("worker:root".to_string()),
        successor_worker_id: LegionWorkflowWorkerId("worker:child".to_string()),
        state: LegionWorkflowDependencyState::Pending,
        label: "root before child".to_string(),
        schema_version: 1,
    });
    let session_id = session.session_id.clone();
    app.seed_delegated_task_plan_contracts(vec![
        delegated_contract(root_plan_id),
        delegated_contract(child_plan_id),
    ]);
    app.seed_legion_workflow_sessions(vec![session])
        .expect("seed workflow");

    let first = app
        .execute_legion_workflow(&session_id)
        .expect("execute first workflow pass");

    assert_eq!(
        first.merge_readiness.state,
        LegionWorkflowMergeReadinessState::Blocked
    );
    assert!(
        first
            .merge_readiness
            .blockers
            .contains(&LegionWorkflowMergeReadinessBlocker::IncompleteWorker)
    );
    assert_eq!(
        first
            .outputs
            .iter()
            .filter(|output| matches!(output, LegionWorkflowCoordinatorOutput::ProposalReady(_)))
            .count(),
        1
    );
    let stored = app
        .legion_workflow_session(&session_id)
        .expect("stored session after first pass");
    assert_eq!(
        stored.worker_assignments[0].state,
        LegionWorkflowWorkerState::Completed
    );
    assert_eq!(
        stored.worker_assignments[1].state,
        LegionWorkflowWorkerState::Ready
    );
    assert_eq!(
        stored.dependency_edges[0].state,
        LegionWorkflowDependencyState::Satisfied
    );
    assert_eq!(stored.proposal_ids.len(), 1);

    let second = app
        .execute_legion_workflow(&session_id)
        .expect("execute resumed workflow pass");

    assert_eq!(
        second.merge_readiness.state,
        LegionWorkflowMergeReadinessState::Ready
    );
    assert!(second.merge_readiness.blockers.is_empty());
    assert_eq!(
        second
            .outputs
            .iter()
            .filter(|output| matches!(output, LegionWorkflowCoordinatorOutput::ProposalReady(_)))
            .count(),
        1
    );
    let stored = app
        .legion_workflow_session(&session_id)
        .expect("stored session after second pass");
    assert!(
        stored
            .worker_assignments
            .iter()
            .all(|worker| worker.state == LegionWorkflowWorkerState::Completed)
    );
    assert_eq!(stored.proposal_ids.len(), 2);
}

#[test]
fn legion_workflow_provider_worker_emits_route_required_metadata_without_invocation() {
    let mut app = AppComposition::new();
    let session = workflow_session(
        "provider",
        vec![worker(
            "worker:provider",
            LegionWorkflowModelBackend::ProviderBacked,
            None,
            "target:provider",
            41,
        )],
        vec![verification_gate(
            LegionWorkflowVerificationGateState::Passed,
        )],
        vec![signoff(LegionWorkflowSignOffState::SignedOff)],
        vec![ProposalId(44)],
        Some(approval(false)),
    );
    let session_id = session.session_id.clone();
    app.seed_legion_workflow_sessions(vec![session])
        .expect("seed workflow");

    let outcome = app
        .execute_legion_workflow(&session_id)
        .expect("execute provider workflow");

    assert!(outcome.outputs.iter().any(|output| {
        matches!(
            output,
            LegionWorkflowCoordinatorOutput::ProviderRouteRequired(route)
                if route.health_labels.iter().any(|label| label == "provider_route.not_invoked")
        )
    }));
    let stored = app
        .legion_workflow_session(&session_id)
        .expect("stored session remains app-owned");
    assert_eq!(
        stored.worker_assignments[0].state,
        LegionWorkflowWorkerState::ProviderRouteRequired
    );
}

#[test]
fn legion_workflow_same_target_conflict_blocks_merge_readiness() {
    let mut app = AppComposition::new();
    let session = workflow_session(
        "conflict",
        vec![
            worker(
                "worker:left",
                LegionWorkflowModelBackend::ProviderBacked,
                None,
                "target:shared",
                51,
            ),
            worker(
                "worker:right",
                LegionWorkflowModelBackend::ProviderBacked,
                None,
                "target:shared",
                52,
            ),
        ],
        vec![verification_gate(
            LegionWorkflowVerificationGateState::Passed,
        )],
        vec![signoff(LegionWorkflowSignOffState::SignedOff)],
        vec![ProposalId(55)],
        Some(approval(true)),
    );
    let session_id = session.session_id.clone();
    app.seed_legion_workflow_sessions(vec![session])
        .expect("seed workflow");

    let outcome = app
        .execute_legion_workflow(&session_id)
        .expect("execute conflicted workflow");

    assert_eq!(
        outcome.merge_readiness.state,
        LegionWorkflowMergeReadinessState::Blocked
    );
    assert!(
        outcome
            .merge_readiness
            .blockers
            .contains(&LegionWorkflowMergeReadinessBlocker::UnresolvedConflict)
    );
    let stored = app
        .legion_workflow_session(&session_id)
        .expect("stored session");
    assert_eq!(
        stored.conflict_summaries[0].state,
        LegionWorkflowConflictState::Unresolved
    );
}

#[test]
fn legion_workflow_dirty_main_workspace_blocks_merge_readiness() {
    let root = temp_workspace("dirty-workspace");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal:dirty".to_string()),
    )
    .expect("open workspace");
    app.open_file("main.txt").expect("open file");
    app.edit_active_buffer(TextEdit::insert(TextPosition::new(0, 0), "!"))
        .expect("make active buffer dirty");

    let (session, plan_id) = local_session("dirty", true);
    let session_id = session.session_id.clone();
    app.seed_delegated_task_plan_contracts(vec![delegated_contract(plan_id)]);
    app.seed_legion_workflow_sessions(vec![session])
        .expect("seed workflow");

    let outcome = app
        .execute_legion_workflow(&session_id)
        .expect("execute dirty workflow");

    assert_eq!(
        outcome.merge_readiness.state,
        LegionWorkflowMergeReadinessState::Blocked
    );
    assert!(
        outcome
            .merge_readiness
            .blockers
            .contains(&LegionWorkflowMergeReadinessBlocker::DirtyMainWorkspaceConflict)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn legion_workflow_missing_verification_blocks_merge_readiness() {
    let mut app = AppComposition::new();
    let (mut session, plan_id) = local_session("missing-verification", true);
    session.verification_gates.clear();
    let session_id = session.session_id.clone();
    app.seed_delegated_task_plan_contracts(vec![delegated_contract(plan_id)]);
    app.seed_legion_workflow_sessions(vec![session])
        .expect("seed workflow");

    let outcome = app
        .execute_legion_workflow(&session_id)
        .expect("execute workflow");

    assert_eq!(
        outcome.merge_readiness.state,
        LegionWorkflowMergeReadinessState::Blocked
    );
    assert!(
        outcome
            .merge_readiness
            .blockers
            .contains(&LegionWorkflowMergeReadinessBlocker::MissingVerificationEvidence)
    );
}

#[test]
fn legion_workflow_missing_signoff_blocks_merge_readiness() {
    let mut app = AppComposition::new();
    let (mut session, plan_id) = local_session("missing-signoff", true);
    session.sign_off_records.clear();
    let session_id = session.session_id.clone();
    app.seed_delegated_task_plan_contracts(vec![delegated_contract(plan_id)]);
    app.seed_legion_workflow_sessions(vec![session])
        .expect("seed workflow");

    let outcome = app
        .execute_legion_workflow(&session_id)
        .expect("execute workflow");

    assert_eq!(
        outcome.merge_readiness.state,
        LegionWorkflowMergeReadinessState::Blocked
    );
    assert!(
        outcome
            .merge_readiness
            .blockers
            .contains(&LegionWorkflowMergeReadinessBlocker::MissingSignOff)
    );
}

#[test]
fn legion_workflow_approved_evidence_and_signoff_are_merge_ready_without_mutation() {
    let root = temp_workspace("merge-ready");
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal:ready".to_string()),
    )
    .expect("open workspace");
    app.open_file("main.txt").expect("open file");

    let plan_id = DelegatedTaskPlanId("plan-ready".to_string());
    let mut session = workflow_session(
        "ready",
        vec![worker(
            "worker:local",
            LegionWorkflowModelBackend::Local,
            Some(plan_id.clone()),
            "target:ready",
            61,
        )],
        vec![verification_gate(
            LegionWorkflowVerificationGateState::Pending,
        )],
        vec![signoff(LegionWorkflowSignOffState::Pending)],
        Vec::new(),
        None,
    );
    let session_id = session.session_id.clone();
    app.seed_delegated_task_plan_contracts(vec![delegated_contract(plan_id)]);
    app.seed_legion_workflow_sessions(vec![session.clone()])
        .expect("seed workflow");

    app.record_legion_workflow_verification(
        &session_id,
        &LegionWorkflowVerificationGateId("verification:unit".to_string()),
        LegionWorkflowVerificationGateState::Passed,
        Some("evidence:ready".to_string()),
    )
    .expect("record verification");
    app.record_legion_workflow_sign_off(
        &session_id,
        &LegionWorkflowSignOffId("signoff:reviewer".to_string()),
        LegionWorkflowSignOffState::SignedOff,
        Some(PrincipalId("reviewer:ready".to_string())),
    )
    .expect("record signoff");
    app.record_legion_workflow_merge_approval(&session_id, true, true, true, false)
        .expect("record approval");

    let outcome = app
        .execute_legion_workflow(&session_id)
        .expect("execute ready workflow");

    assert_eq!(
        outcome.merge_readiness.state,
        LegionWorkflowMergeReadinessState::Ready
    );
    assert!(outcome.merge_readiness.blockers.is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("main.txt")).expect("read file"),
        "clean\n"
    );
    session.lifecycle_state = LegionWorkflowState::Completed;
    assert_eq!(
        app.legion_workflow_session(&session_id)
            .expect("stored session")
            .lifecycle_state,
        LegionWorkflowState::Completed
    );
    let _ = std::fs::remove_dir_all(root);
}
