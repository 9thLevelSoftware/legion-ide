use legion_agent::scheduler::parallel_worker_lanes;
use legion_protocol::{
    CommandRiskLabel, ContextManifestItemCount, DelegatedTaskAffectedTargetSummary,
    DelegatedTaskOperationClass, DelegatedTaskPlanId, FileFingerprint, LegionWorkflowDependency,
    LegionWorkflowDependencyId, LegionWorkflowDependencyState, LegionWorkflowModelBackend,
    LegionWorkflowSession, LegionWorkflowSessionId, LegionWorkflowWorkerAssignment,
    LegionWorkflowWorkerId, LegionWorkflowWorkerRole, LegionWorkflowWorkerState,
    PrivacyClassification, ProposalPrivacyLabel, ProposalRiskLabel, ProposalTargetKind,
    RedactionHint, TimestampMillis, WorkspaceId,
};
use uuid::Uuid;

fn hash(value: &str) -> FileFingerprint {
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
        hashes: vec![hash(label)],
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
        causality_id: legion_protocol::CausalityId(Uuid::from_u128(901)),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn session_with_one_dependency() -> LegionWorkflowSession {
    LegionWorkflowSession {
        session_id: LegionWorkflowSessionId("session:parallel-lanes".to_string()),
        directive_artifact_id: Some("artifact:directive:parallel-lanes".to_string()),
        spec_artifact_id: Some("artifact:spec:parallel-lanes".to_string()),
        task_graph_artifact_id: Some("artifact:task-graph:parallel-lanes".to_string()),
        product_mode: legion_protocol::ProductMode::LegionWorkflows,
        worker_assignments: vec![
            worker("worker:alpha", LegionWorkflowWorkerState::Ready, "alpha"),
            worker(
                "worker:beta",
                LegionWorkflowWorkerState::WaitingForDependency,
                "beta",
            ),
            worker("worker:gamma", LegionWorkflowWorkerState::Ready, "gamma"),
        ],
        dependency_edges: vec![LegionWorkflowDependency {
            dependency_id: LegionWorkflowDependencyId("dependency:alpha-beta".to_string()),
            predecessor_worker_id: LegionWorkflowWorkerId("worker:alpha".to_string()),
            successor_worker_id: LegionWorkflowWorkerId("worker:beta".to_string()),
            state: LegionWorkflowDependencyState::Pending,
            label: "alpha before beta".to_string(),
            schema_version: 1,
        }],
        conflict_summaries: Vec::new(),
        verification_gates: Vec::new(),
        sign_off_records: Vec::new(),
        proposal_ids: Vec::new(),
        merge_approval: None,
        lifecycle_state: legion_protocol::LegionWorkflowState::Executing,
        generated_at: TimestampMillis(1303),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
        correlation_id: legion_protocol::CorrelationId(901),
        causality_id: legion_protocol::CausalityId(Uuid::from_u128(902)),
    }
}

#[test]
fn three_task_dag_keeps_independent_workers_in_the_first_parallel_lane() {
    let lanes = parallel_worker_lanes(&session_with_one_dependency()).expect("lane planning");
    let lane_ids: Vec<Vec<String>> = lanes
        .into_iter()
        .map(|lane| lane.into_iter().map(|worker| worker.worker_id.0).collect())
        .collect();

    assert_eq!(
        lane_ids,
        vec![
            vec!["worker:alpha".to_string(), "worker:gamma".to_string()],
            vec!["worker:beta".to_string()],
        ]
    );
}
