use super::*;

/// Verification evidence citation for a Legion workflow gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegionWorkflowVerificationEvidenceRow {
    /// Stable verification gate id.
    pub gate_id: legion_protocol::LegionWorkflowVerificationGateId,
    /// Display-safe label for the task or branch being verified.
    pub task_label: String,
    /// Evidence artifact id that proves this task or branch, when present.
    pub evidence_artifact_id: Option<String>,
    /// Display-safe command class label.
    pub command_class_label: String,
    /// Gate state.
    pub state: legion_protocol::LegionWorkflowVerificationGateState,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Row schema version.
    pub schema_version: u16,
}

/// Merge-readiness report with verification evidence citations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegionWorkflowMergeReadinessReport {
    /// Stable workflow session id.
    pub session_id: legion_protocol::LegionWorkflowSessionId,
    /// Merge readiness decision.
    pub readiness: LegionWorkflowMergeReadiness,
    /// Verification evidence citations by task or branch label.
    pub verification_evidence_rows: Vec<LegionWorkflowVerificationEvidenceRow>,
    /// Redaction hints.
    pub redaction_hints: Vec<RedactionHint>,
    /// Report schema version.
    pub schema_version: u16,
}

fn verification_evidence_rows(
    session: &LegionWorkflowSession,
) -> Vec<LegionWorkflowVerificationEvidenceRow> {
    session
        .verification_gates
        .iter()
        .map(|gate| LegionWorkflowVerificationEvidenceRow {
            gate_id: gate.gate_id.clone(),
            task_label: gate.label.clone(),
            evidence_artifact_id: gate.evidence_artifact_id.clone(),
            command_class_label: gate.command_class_label.clone(),
            state: gate.state,
            redaction_hints: gate.redaction_hints.clone(),
            schema_version: gate.schema_version.max(1),
        })
        .collect()
}

/// Builds a merge-readiness report from session metadata.
pub fn merge_readiness_report_for_session(
    session: &LegionWorkflowSession,
) -> LegionWorkflowMergeReadinessReport {
    LegionWorkflowMergeReadinessReport {
        session_id: session.session_id.clone(),
        readiness: evaluate_legion_workflow_merge_readiness(session),
        verification_evidence_rows: verification_evidence_rows(session),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: session.schema_version.max(1),
    }
}

impl LegionWorkflowCoordinator {
    /// Evaluates merge readiness and returns verification evidence citations.
    pub fn merge_readiness_report(&self) -> LegionWorkflowMergeReadinessReport {
        let mut session = self.session.clone();
        session.conflict_summaries.extend(self.conflicts.clone());
        merge_readiness_report_for_session(&session)
    }
}
