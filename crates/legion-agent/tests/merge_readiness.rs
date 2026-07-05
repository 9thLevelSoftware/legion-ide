use legion_agent::LegionWorkflowCoordinator;
use legion_protocol::{
    CausalityId, CommandRiskLabel, ContextManifestItemCount, DelegatedTaskAffectedTargetSummary,
    DelegatedTaskOperationClass, DelegatedTaskPlanId, FileFingerprint, LegionWorkflowMergeApproval,
    LegionWorkflowMergeReadinessBlocker, LegionWorkflowMergeReadinessState,
    LegionWorkflowModelBackend, LegionWorkflowSession, LegionWorkflowSessionId,
    LegionWorkflowSignOff, LegionWorkflowSignOffId, LegionWorkflowSignOffState,
    LegionWorkflowState, LegionWorkflowVerificationGate, LegionWorkflowVerificationGateId,
    LegionWorkflowVerificationGateState, LegionWorkflowWorkerAssignment, LegionWorkflowWorkerId,
    LegionWorkflowWorkerRole, LegionWorkflowWorkerState, PrincipalId, PrivacyClassification,
    ProposalId, ProposalPrivacyLabel, ProposalRiskLabel, ProposalTargetKind, RedactionHint,
    TimestampMillis, WorkspaceId,
};
use uuid::Uuid;

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.to_string(),
    }
}

fn target(label: &str) -> DelegatedTaskAffectedTargetSummary {
    DelegatedTaskAffectedTargetSummary {
        target_id: format!("target:{label}"),
        kind: ProposalTargetKind::MetadataOnly,
        workspace_id: Some(WorkspaceId(1)),
        file_id: None,
        buffer_id: None,
        ranges: Vec::new(),
        hashes: vec![fingerprint(label)],
        counts: vec![ContextManifestItemCount {
            label: "target-count".to_string(),
            count: 1,
        }],
        labels: vec![label.to_string()],
        risk_label: ProposalRiskLabel::Medium,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn worker(
    id: &str,
    state: LegionWorkflowWorkerState,
    label: &str,
) -> LegionWorkflowWorkerAssignment {
    LegionWorkflowWorkerAssignment {
        worker_id: LegionWorkflowWorkerId(id.to_string()),
        role: LegionWorkflowWorkerRole::Implementer,
        state,
        model_backend: LegionWorkflowModelBackend::Local,
        display_safe_model_label: format!("{id}:metadata"),
        allowed_command_classes: vec![DelegatedTaskOperationClass::DraftProposalMetadata],
        linked_delegated_plan_id: Some(DelegatedTaskPlanId(format!("plan:{id}"))),
        assisted_ai_route: None,
        affected_targets: vec![target(label)],
        risk_labels: vec![CommandRiskLabel::Review],
        privacy_labels: vec![PrivacyClassification::Metadata],
        correlation_id: legion_protocol::CorrelationId(901),
        causality_id: CausalityId(Uuid::from_u128(901)),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn session(evidence_artifact_id: Option<&str>) -> LegionWorkflowSession {
    LegionWorkflowSession {
        session_id: LegionWorkflowSessionId("session:legion:merge-readiness".to_string()),
        directive_artifact_id: Some("artifact:directive:merge-readiness".to_string()),
        spec_artifact_id: Some("artifact:spec:merge-readiness".to_string()),
        task_graph_artifact_id: Some("artifact:task-graph:merge-readiness".to_string()),
        product_mode: legion_protocol::ProductMode::LegionWorkflows,
        worker_assignments: vec![worker(
            "worker:local",
            LegionWorkflowWorkerState::Completed,
            "crates/legion-agent/src/lib.rs",
        )],
        dependency_edges: Vec::new(),
        conflict_summaries: Vec::new(),
        verification_gates: vec![LegionWorkflowVerificationGate {
            gate_id: LegionWorkflowVerificationGateId("verification:agent".to_string()),
            state: LegionWorkflowVerificationGateState::Passed,
            label: "legion-agent tests".to_string(),
            evidence_artifact_id: evidence_artifact_id.map(ToOwned::to_owned),
            command_class_label: "cargo test -p legion-agent".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        sign_off_records: vec![LegionWorkflowSignOff {
            sign_off_id: LegionWorkflowSignOffId("signoff:agent".to_string()),
            state: LegionWorkflowSignOffState::SignedOff,
            required_role: LegionWorkflowWorkerRole::Reviewer,
            reviewer_principal_id: Some(PrincipalId("principal:reviewer".to_string())),
            label: "review sign-off".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        proposal_ids: vec![ProposalId(1303)],
        merge_approval: Some(LegionWorkflowMergeApproval {
            approval_artifact_id: Some("artifact:approval:agent".to_string()),
            approval_granted: true,
            rollback_available: true,
            audit_persisted_before_success: true,
            main_workspace_dirty_conflict: false,
            proposal_preconditions_stale: false,
            labels: vec!["approval-gated".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }),
        lifecycle_state: LegionWorkflowState::Executing,
        generated_at: TimestampMillis(1303),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
        correlation_id: legion_protocol::CorrelationId(901),
        causality_id: CausalityId(Uuid::from_u128(902)),
    }
}

#[test]
fn merge_readiness_report_cites_verification_evidence_rows() {
    let coordinator = LegionWorkflowCoordinator::new(session(Some("artifact:evidence:legion:1")))
        .expect("valid workflow session");

    let report = coordinator.merge_readiness_report();

    assert_eq!(
        report.readiness.state,
        LegionWorkflowMergeReadinessState::Ready
    );
    assert!(report.readiness.blockers.is_empty());
    assert_eq!(report.verification_evidence_rows.len(), 1);
    let row = &report.verification_evidence_rows[0];
    assert_eq!(row.gate_id.0, "verification:agent");
    assert_eq!(row.task_label, "legion-agent tests");
    assert_eq!(
        row.evidence_artifact_id.as_deref(),
        Some("artifact:evidence:legion:1")
    );
    assert_eq!(row.command_class_label, "cargo test -p legion-agent");
}

#[test]
fn merge_readiness_blocks_when_passed_gate_lacks_evidence() {
    let readiness = legion_protocol::evaluate_legion_workflow_merge_readiness(&session(None));

    assert_eq!(readiness.state, LegionWorkflowMergeReadinessState::Blocked);
    assert!(
        readiness
            .blockers
            .contains(&LegionWorkflowMergeReadinessBlocker::MissingVerificationEvidence)
    );
}
