use legion_agent::dag::workflow_dag_from_approved_plan;
use legion_protocol::{
    EditablePlanArtifact, EditablePlanSection, EditablePlanSectionKind, TimestampMillis,
};

fn approved_plan(review_required: bool) -> EditablePlanArtifact {
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
                vec!["Keep the workspace editable".to_string()],
            ),
            EditablePlanSection::new(
                EditablePlanSectionKind::Tasks,
                vec!["Break the directive into reviewable tasks".to_string()],
            ),
        ],
        TimestampMillis(7),
    );
    plan.review_required = review_required;
    plan
}

#[test]
fn approved_plan_produces_a_dag_with_stable_node_ids() {
    let dag = workflow_dag_from_approved_plan(&approved_plan(false), TimestampMillis(16))
        .expect("approved plan should produce a DAG");

    assert_eq!(dag.plan_id(), "plan:alpha");
    assert_eq!(
        dag.node_ids(),
        vec![
            "plan:alpha/requirements/0".to_string(),
            "plan:alpha/design/0".to_string(),
            "plan:alpha/tasks/0".to_string(),
        ]
    );
}

#[test]
fn rejected_plan_does_not_produce_a_dag() {
    let dag = workflow_dag_from_approved_plan(&approved_plan(true), TimestampMillis(16));

    assert!(dag.is_none());
}
