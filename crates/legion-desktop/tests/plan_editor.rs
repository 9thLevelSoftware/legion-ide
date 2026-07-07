use legion_desktop::bridge::{
    DesktopAction, DesktopAppRequest, DesktopBridgeOutput, DesktopCommandBridge,
};
use legion_desktop::view::{
    DesktopPlanEditorViewModel, DesktopPlanSectionViewModel, edited_sections_from_plan_editor_draft,
};
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

#[test]
fn plan_editor_bridge_submits_revision_with_edited_sections() {
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
    let mut draft = model.draft();
    draft.section_bodies[1].push_str("\nPersist plan revisions");
    let edited_sections = edited_sections_from_plan_editor_draft(&model, &draft);

    let output = DesktopCommandBridge.translate(
        DesktopAction::SubmitLegionWorkflowPlanRevision {
            plan_id: model.artifact_id.clone(),
            edited_sections: edited_sections.clone(),
        },
        &legion_ui::Shell::empty("Plan editor").projection_snapshot(),
    );

    assert_eq!(
        output,
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::SubmitLegionWorkflowPlanRevision {
            plan_id: "plan:alpha".to_string(),
            edited_sections,
        })
    );
}

#[test]
fn plan_editor_bridge_approves_and_rejects_plan_review_requests() {
    let snapshot = legion_ui::Shell::empty("Plan editor").projection_snapshot();
    let bridge = DesktopCommandBridge;

    assert_eq!(
        bridge.translate(
            DesktopAction::ApproveLegionWorkflowPlan {
                plan_id: "plan:alpha".to_string()
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::ApproveLegionWorkflowPlan {
            plan_id: "plan:alpha".to_string()
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::RejectLegionWorkflowPlan {
                plan_id: "plan:alpha".to_string()
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::RejectLegionWorkflowPlan {
            plan_id: "plan:alpha".to_string()
        })
    );
}
