//! Editable plan editor projection for Legion Workflows.

use legion_protocol::{
    EditablePlanArtifact, EditablePlanSection, EditablePlanSectionKind, RedactionHint,
    TimestampMillis,
};

/// View model for one editable plan section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopPlanSectionViewModel {
    /// Section kind.
    pub kind: EditablePlanSectionKind,
    /// Section heading.
    pub heading: String,
    /// Multiline editable body.
    pub body: String,
}

impl From<&EditablePlanSection> for DesktopPlanSectionViewModel {
    fn from(section: &EditablePlanSection) -> Self {
        Self {
            kind: section.kind,
            heading: section.label().to_string(),
            body: section.entries.join("\n"),
        }
    }
}

/// Mutable draft state for the plan editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopPlanEditorDraft {
    /// Draft text for each plan section, in display order.
    pub section_bodies: Vec<String>,
}

impl DesktopPlanEditorDraft {
    /// Creates a draft from a sectioned view model.
    pub fn from_sections(sections: &[DesktopPlanSectionViewModel]) -> Self {
        Self {
            section_bodies: sections
                .iter()
                .map(|section| section.body.clone())
                .collect(),
        }
    }
}

/// View model for the editable plan surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopPlanEditorViewModel {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable directive identifier.
    pub directive_id: String,
    /// Linked spec artifact identifier.
    pub spec_artifact_id: Option<String>,
    /// Linked task-graph artifact identifier.
    pub task_graph_artifact_id: Option<String>,
    /// Human-readable title.
    pub title: String,
    /// Whether the plan should be edited before handoff.
    pub editable: bool,
    /// Whether review still applies.
    pub review_required: bool,
    /// The section view models.
    pub sections: Vec<DesktopPlanSectionViewModel>,
    /// Compact summary label for the panel header.
    pub summary_label: String,
}

impl DesktopPlanEditorViewModel {
    /// Returns the number of editable sections.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Builds a draft from this view model.
    pub fn draft(&self) -> DesktopPlanEditorDraft {
        DesktopPlanEditorDraft::from_sections(&self.sections)
    }
}

impl From<&EditablePlanArtifact> for DesktopPlanEditorViewModel {
    fn from(artifact: &EditablePlanArtifact) -> Self {
        let sections = artifact
            .sections()
            .iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        let summary_label = format!(
            "{} · editable · {} sections",
            artifact.title,
            sections.len()
        );
        Self {
            artifact_id: artifact.artifact_id.clone(),
            directive_id: artifact.directive_id.clone(),
            spec_artifact_id: artifact.spec_artifact_id.clone(),
            task_graph_artifact_id: artifact.task_graph_artifact_id.clone(),
            title: artifact.title.clone(),
            editable: artifact.editable,
            review_required: artifact.review_required,
            sections,
            summary_label,
        }
    }
}

/// Renders the editable plan editor panel.
pub fn render_plan_editor(
    ui: &mut egui::Ui,
    model: &DesktopPlanEditorViewModel,
    draft: &mut DesktopPlanEditorDraft,
) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(&model.summary_label).strong());
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("directive {}", model.directive_id));
            if let Some(spec) = &model.spec_artifact_id {
                ui.separator();
                ui.label(format!("spec {}", spec));
            }
            if let Some(task_graph) = &model.task_graph_artifact_id {
                ui.separator();
                ui.label(format!("task graph {}", task_graph));
            }
        });
        for (index, section) in model.sections.iter().enumerate() {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(&section.heading).strong());
            if draft.section_bodies.len() <= index {
                draft.section_bodies.push(section.body.clone());
            }
            ui.add(egui::TextEdit::multiline(&mut draft.section_bodies[index]).desired_rows(3));
        }
        ui.add_space(4.0);
        ui.label(if model.review_required {
            "Plan edits still require approval before handoff"
        } else {
            "Plan edits are ready for handoff"
        });
    });
}

fn section_text_lines(kind: EditablePlanSectionKind, row_count: usize, label: &str) -> Vec<String> {
    vec![
        format!("{}: {label}", kind.label()),
        format!("{} rows", row_count),
    ]
}

fn synthesized_plan_artifact_from_snapshot(
    plan_id: &str,
    title: &str,
    sections: Vec<DesktopPlanSectionViewModel>,
) -> EditablePlanArtifact {
    EditablePlanArtifact::new(
        format!("plan:{plan_id}"),
        format!("directive:{plan_id}"),
        None,
        None,
        title.to_string(),
        sections
            .into_iter()
            .map(|section| EditablePlanSection {
                kind: section.kind,
                entries: if section.body.trim().is_empty() {
                    Vec::new()
                } else {
                    section
                        .body
                        .lines()
                        .map(|line| line.trim_end().to_string())
                        .collect()
                },
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            })
            .collect(),
        TimestampMillis(1),
    )
}

/// Builds an editable plan surface from a projected plan row.
pub fn plan_editor_view_model_from_projection(
    plan_id: &str,
    current_objective: &str,
    row_count: usize,
    step_count: usize,
) -> DesktopPlanEditorViewModel {
    let sections = vec![
        DesktopPlanSectionViewModel {
            kind: EditablePlanSectionKind::Requirements,
            heading: "Requirements".to_string(),
            body: section_text_lines(
                EditablePlanSectionKind::Requirements,
                row_count,
                current_objective,
            )
            .join("\n"),
        },
        DesktopPlanSectionViewModel {
            kind: EditablePlanSectionKind::Design,
            heading: "Design".to_string(),
            body: section_text_lines(EditablePlanSectionKind::Design, row_count, plan_id)
                .join("\n"),
        },
        DesktopPlanSectionViewModel {
            kind: EditablePlanSectionKind::Tasks,
            heading: "Tasks".to_string(),
            body: section_text_lines(
                EditablePlanSectionKind::Tasks,
                step_count,
                "Break into reviewable steps",
            )
            .join("\n"),
        },
    ];
    let artifact = synthesized_plan_artifact_from_snapshot(
        plan_id,
        &format!("Editable plan for {plan_id}"),
        sections,
    );
    DesktopPlanEditorViewModel::from(&artifact)
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{EditablePlanSection, EditablePlanSectionKind, TimestampMillis};

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
}
