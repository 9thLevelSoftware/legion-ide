//! Editable plan artifact contract for Legion Workflows.

use serde::{Deserialize, Serialize};

use super::{AssistedAiContractError, CausalityId, CorrelationId, RedactionHint, TimestampMillis};

/// The three editable plan sections surfaced to users.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditablePlanSectionKind {
    /// Requirements captured from the directive/spec boundary.
    Requirements,
    /// Design notes that explain the intended approach.
    Design,
    /// Task breakdown surfaced to the coordinator.
    Tasks,
}

impl EditablePlanSectionKind {
    /// Returns the user-facing heading for the section.
    pub fn label(self) -> &'static str {
        match self {
            Self::Requirements => "Requirements",
            Self::Design => "Design",
            Self::Tasks => "Tasks",
        }
    }

    /// Returns the deterministic display order of the section.
    pub fn order(self) -> u8 {
        match self {
            Self::Requirements => 0,
            Self::Design => 1,
            Self::Tasks => 2,
        }
    }
}

/// One editable section in a plan artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditablePlanSection {
    /// Section kind.
    pub kind: EditablePlanSectionKind,
    /// Editable bullet rows.
    pub entries: Vec<String>,
    /// Redaction hints for the section.
    pub redaction_hints: Vec<RedactionHint>,
    /// Section schema version.
    pub schema_version: u16,
}

impl EditablePlanSection {
    /// Creates a new editable section with metadata-only defaults.
    pub fn new(kind: EditablePlanSectionKind, entries: Vec<String>) -> Self {
        Self {
            kind,
            entries,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    /// Returns the user-facing heading for the section.
    pub fn label(&self) -> &'static str {
        self.kind.label()
    }
}

/// One section-level diff row in a plan revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditablePlanRevisionSectionDiff {
    /// Section kind.
    pub kind: EditablePlanSectionKind,
    /// Entries before the revision.
    pub before_entries: Vec<String>,
    /// Entries after the revision.
    pub after_entries: Vec<String>,
    /// Section diff schema version.
    pub schema_version: u16,
}

impl EditablePlanRevisionSectionDiff {
    /// Creates a new section diff with metadata-only defaults.
    pub fn new(
        kind: EditablePlanSectionKind,
        before_entries: Vec<String>,
        after_entries: Vec<String>,
    ) -> Self {
        Self {
            kind,
            before_entries,
            after_entries,
            schema_version: 1,
        }
    }

    /// Returns true when the section changed across the revision.
    pub fn is_changed(&self) -> bool {
        self.before_entries != self.after_entries
    }
}

/// Diff summary for one plan revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditablePlanRevisionDiffSummary {
    /// Previous revision identifier when any.
    pub previous_revision_id: Option<String>,
    /// Section-level diffs in deterministic order.
    pub section_diffs: Vec<EditablePlanRevisionSectionDiff>,
    /// Number of changed sections.
    pub changed_section_count: u32,
    /// Human-readable summary label.
    pub summary_label: String,
    /// Diff summary schema version.
    pub schema_version: u16,
}

impl EditablePlanRevisionDiffSummary {
    /// Creates a new diff summary with metadata-only defaults.
    pub fn new(
        previous_revision_id: Option<String>,
        section_diffs: Vec<EditablePlanRevisionSectionDiff>,
    ) -> Self {
        let changed_section_count = section_diffs
            .iter()
            .filter(|diff| diff.is_changed())
            .count() as u32;
        let summary_label = if changed_section_count == 0 {
            "No plan section changes".to_string()
        } else {
            format!(
                "{} plan section{} changed",
                changed_section_count,
                if changed_section_count == 1 { "" } else { "s" }
            )
        };
        Self {
            previous_revision_id,
            section_diffs,
            changed_section_count,
            summary_label,
            schema_version: 1,
        }
    }

    /// Returns the changed-section count.
    pub fn changed_section_count(&self) -> u32 {
        self.changed_section_count
    }

    /// Returns the section diffs.
    pub fn section_diffs(&self) -> &[EditablePlanRevisionSectionDiff] {
        &self.section_diffs
    }
}

/// Audit row for one plan revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditablePlanRevisionAuditRow {
    /// Stable revision identifier.
    pub revision_id: String,
    /// Stable plan artifact identifier.
    pub plan_artifact_id: String,
    /// Linked directive identifier.
    pub directive_id: String,
    /// Previous revision identifier when any.
    pub previous_revision_id: Option<String>,
    /// Revision timestamp.
    pub generated_at: TimestampMillis,
    /// Correlation id for the revision event.
    pub correlation_id: CorrelationId,
    /// Causality id for the revision event.
    pub causality_id: CausalityId,
    /// Audit schema version.
    pub schema_version: u16,
}

impl EditablePlanRevisionAuditRow {
    /// Creates a new revision audit row with metadata-only defaults.
    pub fn new(
        revision_id: impl Into<String>,
        plan_artifact_id: impl Into<String>,
        directive_id: impl Into<String>,
        previous_revision_id: Option<String>,
        generated_at: TimestampMillis,
        correlation_id: CorrelationId,
        causality_id: CausalityId,
    ) -> Self {
        Self {
            revision_id: revision_id.into(),
            plan_artifact_id: plan_artifact_id.into(),
            directive_id: directive_id.into(),
            previous_revision_id,
            generated_at,
            correlation_id,
            causality_id,
            schema_version: 1,
        }
    }
}

/// Audited editable plan revision with a diffable representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditablePlanRevisionArtifact {
    /// Stable revision identifier.
    pub revision_id: String,
    /// The editable plan snapshot for this revision.
    pub plan: EditablePlanArtifact,
    /// Revision diff summary.
    pub diff_summary: EditablePlanRevisionDiffSummary,
    /// Audit row attached to the revision.
    pub audit_row: EditablePlanRevisionAuditRow,
    /// Revision artifact schema version.
    pub schema_version: u16,
}

impl EditablePlanRevisionArtifact {
    fn section_entries(plan: &EditablePlanArtifact, kind: EditablePlanSectionKind) -> Vec<String> {
        plan.sections()
            .iter()
            .find(|section| section.kind == kind)
            .map(|section| section.entries.clone())
            .unwrap_or_default()
    }

    fn section_diff(
        previous: Option<&EditablePlanArtifact>,
        current: &EditablePlanArtifact,
        kind: EditablePlanSectionKind,
    ) -> EditablePlanRevisionSectionDiff {
        EditablePlanRevisionSectionDiff::new(
            kind,
            previous
                .map(|previous| Self::section_entries(previous, kind))
                .unwrap_or_default(),
            Self::section_entries(current, kind),
        )
    }

    fn diff_summary_from_plans(
        previous: Option<&EditablePlanArtifact>,
        current: &EditablePlanArtifact,
        previous_revision_id: Option<String>,
    ) -> EditablePlanRevisionDiffSummary {
        let section_diffs = vec![
            Self::section_diff(previous, current, EditablePlanSectionKind::Requirements),
            Self::section_diff(previous, current, EditablePlanSectionKind::Design),
            Self::section_diff(previous, current, EditablePlanSectionKind::Tasks),
        ]
        .into_iter()
        .filter(|diff| diff.is_changed() || previous.is_none())
        .collect();
        EditablePlanRevisionDiffSummary::new(previous_revision_id, section_diffs)
    }

    /// Builds an audited revision artifact from a current plan and optional previous snapshot.
    pub fn from_plan_and_previous(
        plan: EditablePlanArtifact,
        previous: Option<&EditablePlanArtifact>,
        audit_row: EditablePlanRevisionAuditRow,
    ) -> Self {
        let diff_summary =
            Self::diff_summary_from_plans(previous, &plan, audit_row.previous_revision_id.clone());
        Self {
            revision_id: audit_row.revision_id.clone(),
            plan,
            diff_summary,
            audit_row,
            schema_version: 1,
        }
    }

    /// Returns the number of changed sections in this revision.
    pub fn changed_section_count(&self) -> u32 {
        self.diff_summary.changed_section_count()
    }

    /// Returns the section diffs in deterministic order.
    pub fn section_diffs(&self) -> &[EditablePlanRevisionSectionDiff] {
        self.diff_summary.section_diffs()
    }

    /// Validates the audited plan revision.
    pub fn validate(&self) -> Result<(), AssistedAiContractError> {
        self.plan.validate()?;
        if self.schema_version == 0 {
            return Err(AssistedAiContractError::InvalidProposalMetadata {
                reason: "plan.revision.schema_zero".to_string(),
            });
        }
        if self.revision_id.trim().is_empty() {
            return Err(AssistedAiContractError::MissingPrecondition {
                reason: "plan.revision.revision_id_missing".to_string(),
            });
        }
        if self.audit_row.schema_version == 0 || self.diff_summary.schema_version == 0 {
            return Err(AssistedAiContractError::InvalidProposalMetadata {
                reason: "plan.revision.schema_zero".to_string(),
            });
        }
        if self.audit_row.revision_id != self.revision_id {
            return Err(AssistedAiContractError::MissingPrecondition {
                reason: "plan.revision.audit_mismatch".to_string(),
            });
        }
        if self.audit_row.plan_artifact_id != self.plan.artifact_id
            || self.audit_row.directive_id != self.plan.directive_id
        {
            return Err(AssistedAiContractError::MissingPrecondition {
                reason: "plan.revision.audit_plan_mismatch".to_string(),
            });
        }
        if self.diff_summary.previous_revision_id != self.audit_row.previous_revision_id {
            return Err(AssistedAiContractError::MissingPrecondition {
                reason: "plan.revision.previous_revision_mismatch".to_string(),
            });
        }
        Ok(())
    }
}

/// Metadata-only editable plan artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditablePlanArtifact {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Linked directive identifier.
    pub directive_id: String,
    /// Linked spec artifact identifier, when any.
    pub spec_artifact_id: Option<String>,
    /// Linked task-graph artifact identifier, when any.
    pub task_graph_artifact_id: Option<String>,
    /// Human-readable plan title.
    pub title: String,
    /// Editable sections, always ordered requirements → design → tasks.
    pub sections: Vec<EditablePlanSection>,
    /// Whether the plan is expected to be edited before handoff.
    pub editable: bool,
    /// Whether a review gate still applies to this plan.
    pub review_required: bool,
    /// Artifact creation timestamp.
    pub created_at: TimestampMillis,
    /// Artifact update timestamp.
    pub updated_at: TimestampMillis,
    /// Redaction hints for the artifact.
    pub redaction_hints: Vec<RedactionHint>,
    /// Artifact schema version.
    pub schema_version: u16,
}

impl EditablePlanArtifact {
    /// Creates a new editable plan artifact with metadata-only defaults.
    pub fn new(
        artifact_id: impl Into<String>,
        directive_id: impl Into<String>,
        spec_artifact_id: Option<String>,
        task_graph_artifact_id: Option<String>,
        title: impl Into<String>,
        sections: Vec<EditablePlanSection>,
        created_at: TimestampMillis,
    ) -> Self {
        Self {
            artifact_id: artifact_id.into(),
            directive_id: directive_id.into(),
            spec_artifact_id,
            task_graph_artifact_id,
            title: title.into(),
            sections,
            editable: true,
            review_required: true,
            created_at,
            updated_at: created_at,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    /// Returns the number of editable sections.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Returns all editable sections.
    pub fn sections(&self) -> &[EditablePlanSection] {
        &self.sections
    }

    /// Returns true when the artifact is intended to be edited before handoff.
    pub fn is_editable(&self) -> bool {
        self.editable
    }

    /// Returns a compact summary label for list surfaces.
    pub fn summary_label(&self) -> String {
        format!("{} · {} sections", self.title, self.section_count())
    }

    /// Validates the artifact as a metadata-first editable plan.
    pub fn validate(&self) -> Result<(), AssistedAiContractError> {
        if self.schema_version == 0 {
            return Err(AssistedAiContractError::InvalidProposalMetadata {
                reason: "plan.schema_zero".to_string(),
            });
        }
        if self.artifact_id.trim().is_empty() {
            return Err(AssistedAiContractError::MissingPrecondition {
                reason: "plan.artifact_id_missing".to_string(),
            });
        }
        if self.directive_id.trim().is_empty() {
            return Err(AssistedAiContractError::MissingPrecondition {
                reason: "plan.directive_id_missing".to_string(),
            });
        }
        if self.title.trim().is_empty() {
            return Err(AssistedAiContractError::InvalidProposalMetadata {
                reason: "plan.title_missing".to_string(),
            });
        }

        let mut seen = [false; 3];
        for section in &self.sections {
            if section.schema_version == 0 {
                return Err(AssistedAiContractError::InvalidProposalMetadata {
                    reason: "plan.section.schema_zero".to_string(),
                });
            }
            let index = section.kind.order() as usize;
            if seen[index] {
                return Err(AssistedAiContractError::InvalidProposalMetadata {
                    reason: format!("plan.section.duplicate.{:?}", section.kind),
                });
            }
            seen[index] = true;
            for entry in &section.entries {
                if entry.trim().is_empty() {
                    return Err(AssistedAiContractError::MissingPrecondition {
                        reason: format!("plan.section.entry_missing.{:?}", section.kind),
                    });
                }
            }
        }

        if seen.iter().any(|seen| !seen) {
            return Err(AssistedAiContractError::MissingPrecondition {
                reason: "plan.section_missing".to_string(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editable_plan_artifact_validates_and_summaries_sections() {
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
                    vec!["Keep the workspace editable".to_string()],
                ),
                EditablePlanSection::new(
                    EditablePlanSectionKind::Tasks,
                    vec!["Break the directive into reviewable tasks".to_string()],
                ),
            ],
            TimestampMillis(7),
        );

        assert_eq!(artifact.section_count(), 3);
        assert!(artifact.is_editable());
        assert_eq!(artifact.summary_label(), "Alpha workflow plan · 3 sections");
        assert_eq!(artifact.sections()[0].label(), "Requirements");
        artifact.validate().expect("plan artifact should validate");
    }
}
