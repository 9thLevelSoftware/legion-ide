//! Editable plan construction helpers for agent workflows.

use legion_protocol::{
    DirectiveArtifact, EditablePlanArtifact, EditablePlanRevisionArtifact,
    EditablePlanRevisionAuditRow, EditablePlanSection, EditablePlanSectionKind, SpecArtifact,
    TaskGraphArtifact, TimestampMillis,
};

fn requirement_entries(directive: &DirectiveArtifact, spec: Option<&SpecArtifact>) -> Vec<String> {
    let mut entries = Vec::new();
    entries.extend(directive.scope_labels.iter().cloned());
    if let Some(spec) = spec {
        entries.extend(spec.constraint_labels.iter().cloned());
        entries.extend(
            spec.acceptance_criteria_hashes
                .iter()
                .map(|hash| format!("acceptance criterion hash {}", hash.value)),
        );
    }
    if entries.is_empty() {
        entries.push(format!(
            "directive {} needs scope review",
            directive.directive_id
        ));
    }
    entries
}

fn design_entries(directive: &DirectiveArtifact, spec: Option<&SpecArtifact>) -> Vec<String> {
    let mut entries = vec![format!("directive goal hash {}", directive.goal_hash.value)];
    if let Some(spec) = spec {
        entries.extend(
            spec.design_note_hashes
                .iter()
                .map(|hash| format!("design note hash {}", hash.value)),
        );
    } else {
        entries.push("design notes pending user review".to_string());
    }
    entries
}

fn task_entries(task_graph: Option<&TaskGraphArtifact>) -> Vec<String> {
    match task_graph {
        Some(task_graph) => task_graph
            .nodes
            .iter()
            .map(|node| {
                let mut line = node.task_id.clone();
                if !node.target_labels.is_empty() {
                    line.push_str(&format!(" → {}", node.target_labels.join(", ")));
                }
                if !node.verification_requirements.is_empty() {
                    line.push_str(&format!(" ({})", node.verification_requirements.join(", ")));
                }
                line
            })
            .collect(),
        None => vec!["task graph pending coordinator breakdown".to_string()],
    }
}

/// Builds an editable plan artifact from directive/spec/task-graph metadata.
pub fn editable_plan_from_workflow_artifacts(
    directive: &DirectiveArtifact,
    spec: Option<&SpecArtifact>,
    task_graph: Option<&TaskGraphArtifact>,
    generated_at: TimestampMillis,
) -> EditablePlanArtifact {
    let sections = vec![
        EditablePlanSection::new(
            EditablePlanSectionKind::Requirements,
            requirement_entries(directive, spec),
        ),
        EditablePlanSection::new(
            EditablePlanSectionKind::Design,
            design_entries(directive, spec),
        ),
        EditablePlanSection::new(EditablePlanSectionKind::Tasks, task_entries(task_graph)),
    ];

    EditablePlanArtifact::new(
        format!("plan:{}", directive.directive_id),
        directive.directive_id.clone(),
        spec.map(|spec| spec.artifact_id.clone()),
        task_graph.map(|task_graph| task_graph.artifact_id.clone()),
        format!("Editable plan for {}", directive.directive_id),
        sections,
        generated_at,
    )
}

/// Builds an audited editable plan revision from workflow artifacts.
pub fn editable_plan_revision_from_workflow_artifacts(
    directive: &DirectiveArtifact,
    spec: Option<&SpecArtifact>,
    task_graph: Option<&TaskGraphArtifact>,
    previous_plan: Option<&EditablePlanArtifact>,
    audit_row: EditablePlanRevisionAuditRow,
) -> EditablePlanRevisionArtifact {
    let plan =
        editable_plan_from_workflow_artifacts(directive, spec, task_graph, audit_row.generated_at);
    EditablePlanRevisionArtifact::from_plan_and_previous(plan, previous_plan, audit_row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        CausalityId, CorrelationId, DelegatedTaskStepState, DirectiveArtifact, FileFingerprint,
        ProductMode, ProposalPrivacyLabel, ProposalRiskLabel, RedactionHint, SpecArtifact,
        TaskGraphArtifact, TaskNode, TimestampMillis, WorkspaceId,
    };
    use uuid::Uuid;

    fn fingerprint(value: &str) -> FileFingerprint {
        FileFingerprint {
            algorithm: "sha256".to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn editable_plan_from_workflow_artifacts_keeps_the_user_editable_before_handoff() {
        let directive = DirectiveArtifact {
            artifact_id: "artifact:directive:alpha".to_string(),
            directive_id: "directive:alpha".to_string(),
            goal_hash: fingerprint("goal:alpha"),
            scope_labels: vec!["workspace:alpha".to_string()],
            workspace_id: Some(WorkspaceId(7)),
            product_mode: ProductMode::LegionWorkflows,
            policy_profile_id: "policy:alpha".to_string(),
            retention_policy_label: "metadata-only".to_string(),
            raw_payload_retained: false,
            correlation_id: CorrelationId(11),
            causality_id: CausalityId(Uuid::from_u128(12)),
            created_at: TimestampMillis(13),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let spec = SpecArtifact {
            artifact_id: "artifact:spec:alpha".to_string(),
            directive_id: directive.directive_id.clone(),
            requirement_hashes: vec![fingerprint("req:alpha")],
            design_note_hashes: vec![fingerprint("design:alpha")],
            acceptance_criteria_hashes: vec![fingerprint("accept:alpha")],
            constraint_labels: vec!["keep plan editable".to_string()],
            retention_policy_label: "metadata-only".to_string(),
            raw_payload_retained: false,
            generated_at: TimestampMillis(14),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let task_graph = TaskGraphArtifact {
            artifact_id: "artifact:task-graph:alpha".to_string(),
            directive_id: directive.directive_id.clone(),
            nodes: vec![TaskNode {
                task_id: "task:alpha:1".to_string(),
                depends_on: vec!["task:alpha:0".to_string()],
                target_labels: vec!["crates/legion-agent/src/plan.rs".to_string()],
                verification_requirements: vec!["cargo test -p legion-agent".to_string()],
                state: DelegatedTaskStepState::Planned,
                risk_label: ProposalRiskLabel::Low,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            edge_count: 1,
            blocked_task_count: 0,
            retention_policy_label: "metadata-only".to_string(),
            raw_payload_retained: false,
            generated_at: TimestampMillis(15),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let plan = editable_plan_from_workflow_artifacts(
            &directive,
            Some(&spec),
            Some(&task_graph),
            TimestampMillis(16),
        );

        assert_eq!(plan.directive_id, directive.directive_id);
        assert_eq!(
            plan.spec_artifact_id.as_deref(),
            Some(spec.artifact_id.as_str())
        );
        assert_eq!(
            plan.task_graph_artifact_id.as_deref(),
            Some(task_graph.artifact_id.as_str())
        );
        assert!(plan.is_editable());
        assert!(plan.review_required);
        assert_eq!(plan.section_count(), 3);
        assert!(
            plan.sections()
                .iter()
                .any(|section| section.kind == EditablePlanSectionKind::Requirements)
        );
        assert!(plan.sections().iter().any(|section| {
            section
                .entries
                .iter()
                .any(|entry| entry == "keep plan editable")
        }));
        assert!(plan
            .sections()
            .iter()
            .any(|section| section.entries.iter().any(|entry| entry == "task:alpha:1 → crates/legion-agent/src/plan.rs (cargo test -p legion-agent)")));
        plan.validate().expect("generated plan should validate");
    }
}
