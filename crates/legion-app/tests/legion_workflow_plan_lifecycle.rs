use legion_agent::coordinator::LegionWorkflowSessionBuilderConfig;
use legion_app::AppComposition;
use legion_protocol::{
    CausalityId, CorrelationId, DelegatedTaskStepState, DirectiveArtifact, EditablePlanSectionKind,
    FileFingerprint, ProductMode, ProposalPrivacyLabel, ProposalRiskLabel, RedactionHint,
    SpecArtifact, TaskGraphArtifact, TaskNode, TimestampMillis, WorkspaceId,
};
use legion_storage::{FileBackedStorage, PlanRevisionRepository};
use uuid::Uuid;

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.to_string(),
    }
}

fn directive() -> DirectiveArtifact {
    DirectiveArtifact {
        artifact_id: "artifact:directive:alpha".to_string(),
        directive_id: "directive:alpha".to_string(),
        goal_hash: fingerprint("goal:alpha"),
        scope_labels: vec!["workspace:alpha".to_string()],
        workspace_id: Some(WorkspaceId(7)),
        product_mode: ProductMode::LegionWorkflows,
        policy_profile_id: "policy:metadata-only".to_string(),
        retention_policy_label: "metadata-only".to_string(),
        raw_payload_retained: false,
        correlation_id: CorrelationId(11),
        causality_id: CausalityId(Uuid::from_u128(12)),
        created_at: TimestampMillis(13),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn spec() -> SpecArtifact {
    SpecArtifact {
        artifact_id: "artifact:spec:alpha".to_string(),
        directive_id: "directive:alpha".to_string(),
        requirement_hashes: vec![fingerprint("requirement:alpha")],
        design_note_hashes: vec![fingerprint("design:alpha")],
        acceptance_criteria_hashes: vec![fingerprint("acceptance:alpha")],
        constraint_labels: vec!["metadata-only".to_string()],
        retention_policy_label: "metadata-only".to_string(),
        raw_payload_retained: false,
        generated_at: TimestampMillis(14),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn task_node(task_id: &str, depends_on: Vec<&str>) -> TaskNode {
    TaskNode {
        task_id: task_id.to_string(),
        depends_on: depends_on.into_iter().map(str::to_string).collect(),
        target_labels: vec![format!("target:{task_id}")],
        verification_requirements: vec![
            "cargo test -p legion-app --test legion_workflow_plan_lifecycle".to_string(),
        ],
        state: DelegatedTaskStepState::Planned,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn task_graph() -> TaskGraphArtifact {
    TaskGraphArtifact {
        artifact_id: "artifact:task-graph:alpha".to_string(),
        directive_id: "directive:alpha".to_string(),
        nodes: vec![
            task_node("task:alpha:one", vec![]),
            task_node("task:alpha:two", vec!["task:alpha:one"]),
        ],
        edge_count: 1,
        blocked_task_count: 0,
        retention_policy_label: "metadata-only".to_string(),
        raw_payload_retained: false,
        generated_at: TimestampMillis(15),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn builder_config() -> LegionWorkflowSessionBuilderConfig {
    LegionWorkflowSessionBuilderConfig {
        session_id: "session:alpha".to_string(),
        generated_at: TimestampMillis(30),
        correlation_id: CorrelationId(31),
        causality_id: CausalityId(Uuid::from_u128(32)),
        workspace_id: Some(WorkspaceId(7)),
    }
}

#[test]
fn legion_workflow_plan_lifecycle_revises_approves_dags_and_builds_session() {
    let mut app = AppComposition::new();
    let plan = app.create_legion_workflow_plan(directive(), Some(spec()), Some(task_graph()));

    assert!(plan.review_required);
    assert_eq!(app.plan_revisions(&plan.artifact_id).len(), 1);
    assert!(
        app.legion_workflow_dag_for_plan(&plan.artifact_id)
            .is_none()
    );

    let mut edited_sections = plan.sections.clone();
    edited_sections
        .iter_mut()
        .find(|section| section.kind == EditablePlanSectionKind::Design)
        .unwrap()
        .entries
        .push("Persist every plan revision".to_string());
    let revision = app
        .revise_legion_workflow_plan(&plan.artifact_id, edited_sections)
        .unwrap();

    assert_eq!(revision.changed_section_count(), 1);
    assert_eq!(app.plan_revisions(&plan.artifact_id).len(), 2);
    assert!(
        app.latest_plan_revision(&plan.artifact_id)
            .unwrap()
            .plan
            .review_required
    );

    let approved = app.approve_legion_workflow_plan(&plan.artifact_id).unwrap();
    assert!(!approved.plan.review_required);
    let dag = app.legion_workflow_dag_for_plan(&plan.artifact_id).unwrap();
    assert!(
        dag.node_ids()
            .contains(&"plan:directive:alpha/tasks/0".to_string())
    );

    let session = app
        .create_legion_workflow_session_from_plan(&plan.artifact_id, builder_config())
        .unwrap();
    assert_eq!(session.worker_assignments.len(), 2);
    assert_eq!(
        session.worker_assignments[0].worker_id.0,
        "plan:directive:alpha/tasks/0"
    );
    assert_eq!(session.dependency_edges.len(), 1);
}

#[test]
fn rejected_legion_workflow_plan_has_no_dag() {
    let mut app = AppComposition::new();
    let plan = app.create_legion_workflow_plan(directive(), Some(spec()), Some(task_graph()));

    let rejected = app.reject_legion_workflow_plan(&plan.artifact_id).unwrap();

    assert!(rejected.plan.review_required);
    assert!(
        app.legion_workflow_dag_for_plan(&plan.artifact_id)
            .is_none()
    );
    assert!(
        app.create_legion_workflow_session_from_plan(&plan.artifact_id, builder_config())
            .unwrap_err()
            .to_string()
            .contains("requires review")
    );
}

#[test]
fn legion_workflow_plan_revision_ledger_survives_save_load() {
    let mut app = AppComposition::new();
    let plan = app.create_legion_workflow_plan(directive(), Some(spec()), Some(task_graph()));
    app.approve_legion_workflow_plan(&plan.artifact_id).unwrap();
    let revisions = app.plan_revisions(&plan.artifact_id);

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("storage.json");
    let mut storage = FileBackedStorage::open(&path).unwrap();
    for revision in revisions {
        storage.record_plan_revision(revision).unwrap();
    }
    drop(storage);

    let reloaded = FileBackedStorage::open(&path).unwrap();
    assert_eq!(reloaded.plan_revisions(&plan.artifact_id).len(), 2);
    assert!(
        !reloaded
            .latest_plan_revision(&plan.artifact_id)
            .unwrap()
            .plan
            .review_required
    );
}

#[test]
fn legion_workflow_plan_revision_audit_rows_have_non_zero_ids() {
    let mut app = AppComposition::new();
    let plan = app.create_legion_workflow_plan(directive(), Some(spec()), Some(task_graph()));
    app.approve_legion_workflow_plan(&plan.artifact_id).unwrap();

    for row in app.plan_revision_audit_rows(&plan.artifact_id) {
        assert_ne!(row.correlation_id.0, 0);
        assert!(!row.causality_id.0.is_nil());
    }
}

#[test]
fn failed_plan_revision_persistence_does_not_advance_app_visible_ledger() {
    let mut app = AppComposition::new();
    let plan = app.create_legion_workflow_plan(directive(), Some(spec()), Some(task_graph()));
    let latest_before = app.latest_plan_revision(&plan.artifact_id).unwrap();

    app.fail_next_plan_revision_write_for_test();
    let error = app
        .approve_legion_workflow_plan(&plan.artifact_id)
        .expect_err("approval must fail when plan revision persistence fails");

    assert!(
        error
            .to_string()
            .contains("injected plan revision write failure")
    );
    assert_eq!(app.plan_revisions(&plan.artifact_id).len(), 1);
    assert_eq!(
        app.latest_plan_revision(&plan.artifact_id)
            .unwrap()
            .revision_id,
        latest_before.revision_id
    );
    assert!(
        app.latest_plan_revision(&plan.artifact_id)
            .unwrap()
            .plan
            .review_required
    );
    assert!(
        app.legion_workflow_dag_for_plan(&plan.artifact_id)
            .is_none()
    );
}
