use legion_desktop::view::{DesktopPlanEditorViewModel, DesktopPlanSectionViewModel};
use legion_protocol::{
    EditablePlanArtifact, EditablePlanSection, EditablePlanSectionKind, TimestampMillis,
};

#[test]
fn plan_editor_view_model_keeps_requirements_design_and_tasks_editable() {
    let artifact = EditablePlanArtifact::new(
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
                vec!["Keep the plan editable".to_string()],
            ),
            EditablePlanSection::new(
                EditablePlanSectionKind::Tasks,
                vec!["Convert to reviewable tasks".to_string()],
            ),
        ],
        TimestampMillis(17),
    );

    let model = DesktopPlanEditorViewModel::from(&artifact);

    assert_eq!(model.artifact_id, "plan:alpha");
    assert_eq!(model.title, "Alpha workflow plan");
    assert!(model.editable);
    assert!(model.review_required);
    assert_eq!(model.section_count(), 3);
    assert!(
        model
            .sections
            .iter()
            .any(|section| matches!(section.kind, EditablePlanSectionKind::Tasks))
    );
    assert!(model.summary_label.contains("editable"));
    assert_eq!(
        model.sections[0],
        DesktopPlanSectionViewModel::from(&artifact.sections()[0])
    );
}
