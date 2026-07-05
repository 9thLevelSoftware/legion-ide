//! Approved-plan DAG helpers for Legion workflow coordination.

use legion_protocol::{EditablePlanArtifact, EditablePlanSectionKind, TimestampMillis};

/// One node in an approved plan DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDagNode {
    /// Stable node identifier.
    pub node_id: String,
    /// Stable plan identifier this node belongs to.
    pub plan_id: String,
    /// The source plan section that produced this node.
    pub section_kind: EditablePlanSectionKind,
    /// Stable entry index within the source section.
    pub entry_index: usize,
    /// Display-safe node label.
    pub label: String,
}

/// One directed edge in an approved plan DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDagEdge {
    /// Source node identifier.
    pub from_node_id: String,
    /// Destination node identifier.
    pub to_node_id: String,
    /// Display-safe relation label.
    pub relation_label: String,
}

/// DAG input produced from an approved editable plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDag {
    /// Stable plan identifier.
    pub plan_id: String,
    /// Stable DAG nodes.
    pub nodes: Vec<WorkflowDagNode>,
    /// Stable DAG edges.
    pub edges: Vec<WorkflowDagEdge>,
    /// Generation timestamp.
    pub generated_at: TimestampMillis,
    /// Schema version.
    pub schema_version: u16,
}

impl WorkflowDag {
    /// Returns the stable plan identifier.
    pub fn plan_id(&self) -> &str {
        &self.plan_id
    }

    /// Returns stable node identifiers in deterministic order.
    pub fn node_ids(&self) -> Vec<String> {
        self.nodes.iter().map(|node| node.node_id.clone()).collect()
    }
}

fn stable_section_label(kind: EditablePlanSectionKind) -> &'static str {
    match kind {
        EditablePlanSectionKind::Requirements => "requirements",
        EditablePlanSectionKind::Design => "design",
        EditablePlanSectionKind::Tasks => "tasks",
    }
}

fn stable_node_id(plan_id: &str, kind: EditablePlanSectionKind, entry_index: usize) -> String {
    format!("{plan_id}/{}/{}", stable_section_label(kind), entry_index)
}

/// Builds a DAG input from an approved editable plan.
///
/// Returns `None` when the plan still requires review, which keeps rejected or
/// pending plans from reaching the coordinator as executable DAG input.
pub fn workflow_dag_from_approved_plan(
    plan: &EditablePlanArtifact,
    generated_at: TimestampMillis,
) -> Option<WorkflowDag> {
    if plan.review_required || !plan.is_editable() {
        return None;
    }

    let mut nodes = Vec::new();
    for section in plan.sections() {
        for (entry_index, entry) in section.entries.iter().enumerate() {
            nodes.push(WorkflowDagNode {
                node_id: stable_node_id(&plan.artifact_id, section.kind, entry_index),
                plan_id: plan.artifact_id.clone(),
                section_kind: section.kind,
                entry_index,
                label: entry.clone(),
            });
        }
    }

    let edges = nodes
        .windows(2)
        .map(|pair| WorkflowDagEdge {
            from_node_id: pair[0].node_id.clone(),
            to_node_id: pair[1].node_id.clone(),
            relation_label: "next".to_string(),
        })
        .collect::<Vec<_>>();

    Some(WorkflowDag {
        plan_id: plan.artifact_id.clone(),
        nodes,
        edges,
        generated_at,
        schema_version: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{EditablePlanSection, EditablePlanSectionKind, TimestampMillis};

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
}
