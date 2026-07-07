use legion_agent::coordinator::{
    LegionWorkflowSessionBuilderConfig, legion_workflow_session_from_approved_plan,
};
use legion_agent::dag::workflow_dag_from_approved_plan;
use legion_protocol::{
    CausalityId, CorrelationId, DelegatedTaskStepState, EditablePlanArtifact, EditablePlanSection,
    EditablePlanSectionKind, ProductMode, ProposalPrivacyLabel, ProposalRiskLabel, RedactionHint,
    TaskGraphArtifact, TaskNode, TimestampMillis,
};
use uuid::Uuid;

fn approved_plan() -> EditablePlanArtifact {
    let mut plan = EditablePlanArtifact::new(
        "plan:alpha",
        "directive:alpha",
        Some("spec:alpha".to_string()),
        Some("task-graph:alpha".to_string()),
        "Alpha workflow plan",
        vec![
            EditablePlanSection::new(
                EditablePlanSectionKind::Requirements,
                vec!["Confirm scope".to_string()],
            ),
            EditablePlanSection::new(
                EditablePlanSectionKind::Design,
                vec!["Keep metadata-only".to_string()],
            ),
            EditablePlanSection::new(
                EditablePlanSectionKind::Tasks,
                vec!["task:one".to_string(), "task:two".to_string()],
            ),
        ],
        TimestampMillis(7),
    );
    plan.review_required = false;
    plan
}

fn task_node(task_id: &str, depends_on: Vec<&str>) -> TaskNode {
    TaskNode {
        task_id: task_id.to_string(),
        depends_on: depends_on.into_iter().map(str::to_string).collect(),
        target_labels: vec![format!("target:{task_id}")],
        verification_requirements: vec!["cargo test -p legion-agent".to_string()],
        state: DelegatedTaskStepState::Planned,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn task_graph(nodes: Vec<TaskNode>) -> TaskGraphArtifact {
    TaskGraphArtifact {
        artifact_id: "task-graph:alpha".to_string(),
        directive_id: "directive:alpha".to_string(),
        nodes,
        edge_count: 1,
        blocked_task_count: 0,
        retention_policy_label: "metadata-only".to_string(),
        raw_payload_retained: false,
        generated_at: TimestampMillis(8),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn config() -> LegionWorkflowSessionBuilderConfig {
    LegionWorkflowSessionBuilderConfig {
        session_id: "session:alpha".to_string(),
        generated_at: TimestampMillis(9),
        correlation_id: CorrelationId(10),
        causality_id: CausalityId(Uuid::from_u128(11)),
    }
}

#[test]
fn approved_plan_session_builder_creates_one_worker_per_task_with_stable_ids() {
    let plan = approved_plan();
    let dag = workflow_dag_from_approved_plan(&plan, TimestampMillis(8)).unwrap();
    let graph = task_graph(vec![
        task_node("task:one", vec![]),
        task_node("task:two", vec![]),
    ]);

    let session =
        legion_workflow_session_from_approved_plan(&plan, &dag, &graph, config()).unwrap();

    assert_eq!(session.session_id.0, "session:alpha");
    assert_eq!(session.product_mode, ProductMode::LegionWorkflows);
    assert_eq!(session.worker_assignments.len(), 2);
    assert_eq!(
        session.worker_assignments[0].worker_id.0,
        "plan:alpha/tasks/0"
    );
    assert_eq!(
        session.worker_assignments[1].worker_id.0,
        "plan:alpha/tasks/1"
    );
    assert!(session.dependency_edges.is_empty());
    assert_eq!(
        session.task_graph_artifact_id.as_deref(),
        Some("task-graph:alpha")
    );
}

#[test]
fn approved_plan_session_builder_maps_task_depends_on_to_worker_edges() {
    let plan = approved_plan();
    let dag = workflow_dag_from_approved_plan(&plan, TimestampMillis(8)).unwrap();
    let graph = task_graph(vec![
        task_node("task:one", vec![]),
        task_node("task:two", vec!["task:one"]),
    ]);

    let session =
        legion_workflow_session_from_approved_plan(&plan, &dag, &graph, config()).unwrap();

    assert_eq!(session.dependency_edges.len(), 1);
    let edge = &session.dependency_edges[0];
    assert_eq!(edge.predecessor_worker_id.0, "plan:alpha/tasks/0");
    assert_eq!(edge.successor_worker_id.0, "plan:alpha/tasks/1");
    assert_eq!(edge.dependency_id.0, "plan:alpha/dependencies/0/1");
}

#[test]
fn approved_plan_session_builder_rejects_unknown_task_dependency() {
    let plan = approved_plan();
    let dag = workflow_dag_from_approved_plan(&plan, TimestampMillis(8)).unwrap();
    let graph = task_graph(vec![task_node("task:two", vec!["task:missing"])]);

    let error =
        legion_workflow_session_from_approved_plan(&plan, &dag, &graph, config()).unwrap_err();

    assert!(error.to_string().contains("unknown task dependency"));
}
