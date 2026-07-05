use legion_protocol::{
    CausalityId, CorrelationId, EditablePlanArtifact, EditablePlanRevisionArtifact,
    EditablePlanRevisionAuditRow, EditablePlanSection, EditablePlanSectionKind, TimestampMillis,
};
use legion_storage::plan::PlanRevisionLedger;
use uuid::Uuid;

fn plan(label: &str, design_entry: &str, timestamp: u64) -> EditablePlanArtifact {
    EditablePlanArtifact::new(
        "plan:alpha",
        "directive:alpha",
        Some("spec:alpha".to_string()),
        Some("task-graph:alpha".to_string()),
        format!("Alpha workflow plan {label}"),
        vec![
            EditablePlanSection::new(
                EditablePlanSectionKind::Requirements,
                vec!["Confirm scope".to_string()],
            ),
            EditablePlanSection::new(
                EditablePlanSectionKind::Design,
                vec![design_entry.to_string()],
            ),
            EditablePlanSection::new(
                EditablePlanSectionKind::Tasks,
                vec!["Break the directive into reviewable tasks".to_string()],
            ),
        ],
        TimestampMillis(timestamp),
    )
}

fn revision(
    revision_id: &str,
    previous_revision_id: Option<&str>,
    current: EditablePlanArtifact,
    previous: Option<&EditablePlanArtifact>,
) -> EditablePlanRevisionArtifact {
    let audit_row = EditablePlanRevisionAuditRow::new(
        revision_id,
        current.artifact_id.clone(),
        current.directive_id.clone(),
        previous_revision_id.map(str::to_string),
        TimestampMillis(42),
        CorrelationId(7),
        CausalityId(Uuid::from_u128(8)),
    );
    EditablePlanRevisionArtifact::from_plan_and_previous(current, previous, audit_row)
}

#[test]
fn plan_revision_ledger_preserves_history_and_audit_rows() {
    let previous = plan("v1", "Keep the workspace editable", 7);
    let current = plan("v2", "Persist plan revisions", 8);
    let revision_one = revision("plan-revision:alpha:1", None, previous.clone(), None);
    let revision_two = revision(
        "plan-revision:alpha:2",
        Some("plan-revision:alpha:1"),
        current.clone(),
        Some(&previous),
    );

    let mut ledger = PlanRevisionLedger::new();
    ledger
        .record_revision(revision_one.clone())
        .expect("first revision should record");
    ledger
        .record_revision(revision_two.clone())
        .expect("second revision should record");

    assert_eq!(ledger.revision_count(), 2);
    assert_eq!(ledger.revision("plan-revision:alpha:1"), Some(revision_one));
    assert_eq!(
        ledger.revision("plan-revision:alpha:2"),
        Some(revision_two.clone())
    );
    assert_eq!(
        ledger.latest_revision("plan:alpha"),
        Some(revision_two.clone())
    );
    assert_eq!(ledger.revisions("plan:alpha").len(), 2);
    assert_eq!(ledger.audit_rows("plan:alpha").len(), 2);
    assert_eq!(
        ledger.audit_rows("plan:alpha")[1].revision_id,
        "plan-revision:alpha:2"
    );
    assert_eq!(revision_two.changed_section_count(), 1);
    assert_eq!(
        revision_two.section_diffs()[0].kind,
        EditablePlanSectionKind::Design
    );
}

#[test]
fn plan_revision_ledger_rejects_duplicate_revision_ids() {
    let mut ledger = PlanRevisionLedger::new();
    let revision = revision(
        "plan-revision:alpha:1",
        None,
        plan("v1", "Keep the workspace editable", 7),
        None,
    );

    ledger
        .record_revision(revision.clone())
        .expect("first revision should record");
    let error = ledger
        .record_revision(revision)
        .expect_err("duplicate revision should fail");
    assert!(error.to_string().contains("already exists"));
}
