//! Plan revision ledger and audit persistence helpers.

use std::collections::HashMap;

use legion_protocol::{EditablePlanRevisionArtifact, EditablePlanRevisionAuditRow};
use serde::{Deserialize, Serialize};

use super::StorageError;

/// Metadata-only ledger for audited plan revisions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanRevisionLedger {
    revisions_by_id: HashMap<String, EditablePlanRevisionArtifact>,
    revision_ids_by_plan: HashMap<String, Vec<String>>,
}

impl PlanRevisionLedger {
    /// Creates an empty plan revision ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of stored revisions.
    pub fn revision_count(&self) -> usize {
        self.revisions_by_id.len()
    }

    /// Records an audited plan revision without overwriting prior versions.
    pub fn record_revision(
        &mut self,
        revision: EditablePlanRevisionArtifact,
    ) -> Result<(), StorageError> {
        revision.validate().map_err(|error| StorageError::Failed {
            message: error.to_string(),
        })?;
        if self.revisions_by_id.contains_key(&revision.revision_id) {
            return Err(StorageError::Failed {
                message: format!("plan revision already exists: {}", revision.revision_id),
            });
        }

        let revision_id = revision.revision_id.clone();
        let plan_id = revision.plan.artifact_id.clone();
        self.revisions_by_id.insert(revision_id.clone(), revision);
        self.revision_ids_by_plan
            .entry(plan_id)
            .or_default()
            .push(revision_id);
        Ok(())
    }

    /// Rebuilds a ledger from persisted revision artifacts.
    pub fn from_revisions(
        revisions: Vec<EditablePlanRevisionArtifact>,
    ) -> Result<Self, StorageError> {
        let mut ledger = Self::new();
        for revision in revisions {
            ledger.record_revision(revision)?;
        }
        Ok(ledger)
    }

    /// Returns every stored revision in deterministic recording order by plan and revision.
    pub fn all_revisions(&self) -> Vec<EditablePlanRevisionArtifact> {
        let mut plan_ids = self
            .revision_ids_by_plan
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        plan_ids.sort();
        plan_ids
            .into_iter()
            .flat_map(|plan_id| self.revisions(&plan_id))
            .collect()
    }

    /// Loads one revision by revision id.
    pub fn revision(&self, revision_id: &str) -> Option<EditablePlanRevisionArtifact> {
        self.revisions_by_id.get(revision_id).cloned()
    }

    /// Loads all revisions for a plan in recording order.
    pub fn revisions(&self, plan_artifact_id: &str) -> Vec<EditablePlanRevisionArtifact> {
        self.revision_ids_by_plan
            .get(plan_artifact_id)
            .into_iter()
            .flat_map(|revision_ids| revision_ids.iter())
            .filter_map(|revision_id| self.revisions_by_id.get(revision_id).cloned())
            .collect()
    }

    /// Loads the latest revision for a plan when any.
    pub fn latest_revision(&self, plan_artifact_id: &str) -> Option<EditablePlanRevisionArtifact> {
        self.revision_ids_by_plan
            .get(plan_artifact_id)
            .and_then(|revision_ids| revision_ids.last())
            .and_then(|revision_id| self.revisions_by_id.get(revision_id))
            .cloned()
    }

    /// Returns the audit rows associated with a plan's revisions.
    pub fn audit_rows(&self, plan_artifact_id: &str) -> Vec<EditablePlanRevisionAuditRow> {
        self.revisions(plan_artifact_id)
            .into_iter()
            .map(|revision| revision.audit_row)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        CausalityId, CorrelationId, EditablePlanArtifact, EditablePlanRevisionAuditRow,
        EditablePlanSection, EditablePlanSectionKind, TimestampMillis,
    };
    use uuid::Uuid;

    fn revision(
        revision_id: &str,
        previous_revision_id: Option<&str>,
        task_label: &str,
        timestamp: u64,
    ) -> EditablePlanRevisionArtifact {
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
                    vec![task_label.to_string()],
                ),
                EditablePlanSection::new(
                    EditablePlanSectionKind::Tasks,
                    vec!["Break the directive into reviewable tasks".to_string()],
                ),
            ],
            TimestampMillis(timestamp),
        );
        let previous = EditablePlanArtifact::new(
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
            TimestampMillis(timestamp - 1),
        );
        let audit_row = EditablePlanRevisionAuditRow::new(
            revision_id,
            current.artifact_id.clone(),
            current.directive_id.clone(),
            previous_revision_id.map(str::to_string),
            TimestampMillis(timestamp),
            CorrelationId(11),
            CausalityId(Uuid::from_u128(12)),
        );
        EditablePlanRevisionArtifact::from_plan_and_previous(current, Some(&previous), audit_row)
    }

    #[test]
    fn ledger_keeps_audited_plan_revisions_without_overwriting() {
        let mut ledger = PlanRevisionLedger::new();
        let first = revision(
            "plan-revision:alpha:1",
            None,
            "Keep the workspace editable",
            7,
        );
        let second = revision(
            "plan-revision:alpha:2",
            Some("plan-revision:alpha:1"),
            "Persist plan revisions",
            8,
        );

        ledger
            .record_revision(first.clone())
            .expect("first revision should record");
        ledger
            .record_revision(second.clone())
            .expect("second revision should record");

        assert_eq!(ledger.revision_count(), 2);
        assert_eq!(ledger.revision("plan-revision:alpha:1"), Some(first));
        assert_eq!(
            ledger.revision("plan-revision:alpha:2"),
            Some(second.clone())
        );
        assert_eq!(ledger.latest_revision("plan:alpha"), Some(second));
        assert_eq!(ledger.revisions("plan:alpha").len(), 2);
        assert_eq!(ledger.audit_rows("plan:alpha").len(), 2);
    }
}
