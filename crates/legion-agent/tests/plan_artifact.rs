use legion_agent::plan::editable_plan_from_workflow_artifacts;
use legion_protocol::{
    CausalityId, CorrelationId, DelegatedTaskStepState, DirectiveArtifact,
    EditablePlanRevisionAuditRow, EditablePlanSectionKind, FileFingerprint, ProductMode,
    ProposalPrivacyLabel, ProposalRiskLabel, RedactionHint, SpecArtifact, TaskGraphArtifact,
    TaskNode, TimestampMillis, WorkspaceId,
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
    assert!(
        plan.sections()
            .iter()
            .any(|section| section.entries.iter().any(|entry| entry
                == "task:alpha:1 → crates/legion-agent/src/plan.rs (cargo test -p legion-agent)"))
    );
    plan.validate().expect("generated plan should validate");
}

#[test]
fn editable_plan_revision_from_workflow_artifacts_keeps_audit_and_diff_metadata() {
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
    let previous_plan = editable_plan_from_workflow_artifacts(
        &directive,
        Some(&spec),
        Some(&task_graph),
        TimestampMillis(16),
    );
    let current_plan = editable_plan_from_workflow_artifacts(
        &directive,
        Some(&spec),
        Some(&task_graph),
        TimestampMillis(17),
    );
    let audit_row = EditablePlanRevisionAuditRow::new(
        "plan-revision:alpha:2",
        current_plan.artifact_id.clone(),
        current_plan.directive_id.clone(),
        Some("plan-revision:alpha:1".to_string()),
        TimestampMillis(18),
        CorrelationId(19),
        CausalityId(Uuid::from_u128(20)),
    );

    let revision = legion_agent::plan::editable_plan_revision_from_workflow_artifacts(
        &directive,
        Some(&spec),
        Some(&task_graph),
        Some(&previous_plan),
        audit_row,
    );

    assert_eq!(revision.plan.artifact_id, current_plan.artifact_id);
    assert_eq!(revision.audit_row.revision_id, "plan-revision:alpha:2");
    assert_eq!(
        revision.audit_row.previous_revision_id.as_deref(),
        Some("plan-revision:alpha:1")
    );
    assert_eq!(revision.diff_summary.changed_section_count(), 0);
    assert!(revision.diff_summary.section_diffs().is_empty());
    revision
        .validate()
        .expect("generated plan revision should validate");
}
