use legion_protocol::{
    EditablePlanArtifact, EditablePlanSection, EditablePlanSectionKind, RedactionHint,
    TimestampMillis,
};
use uuid::Uuid;

fn artifact() -> EditablePlanArtifact {
    EditablePlanArtifact::new(
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
    )
}

#[test]
fn editable_plan_artifact_keeps_requirements_design_and_tasks_sections() {
    let artifact = artifact();

    assert_eq!(artifact.artifact_id, "plan:alpha");
    assert_eq!(artifact.directive_id, "directive:alpha");
    assert!(artifact.is_editable());
    assert!(artifact.review_required);
    assert_eq!(artifact.section_count(), 3);
    assert_eq!(
        artifact.sections()[0].kind,
        EditablePlanSectionKind::Requirements
    );
    assert_eq!(artifact.sections()[1].kind, EditablePlanSectionKind::Design);
    assert_eq!(artifact.sections()[2].kind, EditablePlanSectionKind::Tasks);
    assert_eq!(artifact.summary_label(), "Alpha workflow plan · 3 sections");

    assert_eq!(artifact.redaction_hints, vec![RedactionHint::MetadataOnly]);
    artifact.validate().expect("plan artifact should validate");
}

#[test]
fn editable_plan_revision_artifact_keeps_audit_row_and_diffable_sections() {
    let previous = artifact();
    let current = EditablePlanArtifact::new(
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
                vec![
                    "Keep the workspace editable".to_string(),
                    "Persist plan revisions".to_string(),
                ],
            ),
            EditablePlanSection::new(
                EditablePlanSectionKind::Tasks,
                vec!["Break the directive into reviewable tasks".to_string()],
            ),
        ],
        TimestampMillis(8),
    );
    let audit_row = legion_protocol::EditablePlanRevisionAuditRow {
        revision_id: "plan-revision:alpha:2".to_string(),
        plan_artifact_id: current.artifact_id.clone(),
        directive_id: current.directive_id.clone(),
        previous_revision_id: Some("plan-revision:alpha:1".to_string()),
        generated_at: TimestampMillis(9),
        correlation_id: legion_protocol::CorrelationId(101),
        causality_id: legion_protocol::CausalityId(Uuid::from_u128(102)),
        schema_version: 1,
    };

    let revision = legion_protocol::EditablePlanRevisionArtifact::from_plan_and_previous(
        current,
        Some(&previous),
        audit_row,
    );

    assert_eq!(revision.audit_row.revision_id, "plan-revision:alpha:2");
    assert_eq!(
        revision.audit_row.previous_revision_id.as_deref(),
        Some("plan-revision:alpha:1")
    );
    assert_eq!(revision.diff_summary.changed_section_count(), 1);
    assert_eq!(revision.diff_summary.section_diffs().len(), 1);
    assert_eq!(
        revision.diff_summary.section_diffs()[0].kind,
        EditablePlanSectionKind::Design
    );
    assert_eq!(
        revision.diff_summary.section_diffs()[0].before_entries,
        vec!["Keep the workspace editable".to_string()]
    );
    assert_eq!(
        revision.diff_summary.section_diffs()[0].after_entries,
        vec![
            "Keep the workspace editable".to_string(),
            "Persist plan revisions".to_string(),
        ]
    );
    revision.validate().expect("plan revision should validate");
}
