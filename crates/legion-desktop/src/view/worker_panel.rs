use legion_protocol::{
    LegionWorkflowConflictId, LegionWorkflowMergeReadinessState, LegionWorkflowSignOffId,
    LegionWorkflowVerificationGateId, VerificationRunState,
};
use legion_ui::ShellProjectionSnapshot;

use super::{
    assistant_rows, proposal_review::DesktopProposalEvidencePanelViewModel,
    proposal_review::render_proposal_evidence_panel, theme,
};
use crate::bridge::DesktopAction;

/// A recovery affordance surfaced by the worker panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopWorkerRecoveryAction {
    /// Button label.
    pub label: String,
    /// Short explanation of why this action appears.
    pub rationale: String,
    /// Action dispatched when clicked.
    pub action: DesktopAction,
}

/// Structured worker-panel view model used by the delegate session and command-center surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopWorkerPanelViewModel {
    /// Live worker status and runtime summary rows.
    pub live_status_rows: Vec<String>,
    /// Plan, step, blocker, refusal, and readiness rows.
    pub plan_rows: Vec<String>,
    /// Tool-call, chat, citation, and permission rows.
    pub tool_rows: Vec<String>,
    /// Test-evidence and audit-readiness rows.
    pub test_evidence_rows: Vec<String>,
    /// Proposal preview, review, and hunk bundle rows.
    pub proposal_bundle_rows: Vec<String>,
    /// Explicit recovery actions for blocked, approval-gated, failed, and conflicted workflows.
    pub recovery_actions: Vec<DesktopWorkerRecoveryAction>,
    /// Any surfaced rows that do not fit the current sections.
    pub other_rows: Vec<String>,
}

impl DesktopWorkerPanelViewModel {
    /// Builds a worker panel projection from the app-owned snapshot.
    pub(crate) fn from_snapshot(snapshot: &ShellProjectionSnapshot) -> Self {
        let mut live_status_rows = Vec::new();
        let mut plan_rows = Vec::new();
        let mut tool_rows = Vec::new();
        let mut test_evidence_rows = Vec::new();
        let mut proposal_bundle_rows = Vec::new();
        let recovery_actions = recovery_actions(snapshot);
        let mut other_rows = Vec::new();

        for row in assistant_rows(snapshot) {
            if row.starts_with("delegated task command center")
                || row.starts_with("assisted ai:")
                || row.starts_with("inline predictions:")
                || row.starts_with("context manifest ")
                || row.starts_with("workflow ")
            {
                live_status_rows.push(row);
            } else if row.starts_with("delegated task plan")
                || row.starts_with("delegated task step")
                || row.starts_with("delegated task blocker")
                || row.starts_with("delegated task refusal")
                || row.starts_with("delegated task trust gate")
                || row.starts_with("delegated task disclaimer")
            {
                plan_rows.push(row);
            } else if row.starts_with("delegate chat")
                || row.starts_with("delegate citation")
                || row.starts_with("delegate tool permission")
                || row.starts_with("assisted provider")
                || row.starts_with("assisted route")
                || row.starts_with("assisted request")
                || row.starts_with("assisted refusal")
                || row.starts_with("assisted preview")
            {
                tool_rows.push(row);
            } else if row.starts_with("delegated task audit readiness") {
                test_evidence_rows.push(row);
            } else if row.starts_with("delegated task proposal preview")
                || row.starts_with("delegate proposal review")
                || row.starts_with("delegate proposal hunk")
            {
                proposal_bundle_rows.push(row);
            } else {
                other_rows.push(row);
            }
        }

        Self {
            live_status_rows,
            plan_rows,
            tool_rows,
            test_evidence_rows,
            proposal_bundle_rows,
            recovery_actions,
            other_rows,
        }
    }
}

/// Renders the worker panel as structured sections without silently dropping rows.
pub(crate) fn render_worker_panel(
    ui: &mut egui::Ui,
    panel: &DesktopWorkerPanelViewModel,
    proposal_evidence_panel: &DesktopProposalEvidencePanelViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::body_strong("Worker panel"));
        ui.label(theme::muted(
            "scoped task -> worker -> evidence -> review -> apply",
        ));
    });

    render_section(
        ui,
        "Live status",
        &panel.live_status_rows,
        "No worker status projected",
    );
    render_section(ui, "Plan", &panel.plan_rows, "No worker plan projected");
    render_section(
        ui,
        "Tool calls",
        &panel.tool_rows,
        "No worker tool calls projected",
    );
    render_section(
        ui,
        "Test evidence",
        &panel.test_evidence_rows,
        "No worker evidence projected",
    );
    render_section(
        ui,
        "Proposal bundle",
        &panel.proposal_bundle_rows,
        "No proposal bundle projected",
    );

    if !panel.recovery_actions.is_empty() {
        ui.add_space(4.0);
        ui.label(theme::eyebrow("Recovery actions"));
        for recovery in &panel.recovery_actions {
            ui.horizontal(|ui| {
                if ui.button(&recovery.label).clicked() {
                    actions.push(recovery.action.clone());
                }
                ui.label(theme::muted(&recovery.rationale));
            });
        }
    }

    ui.add_space(4.0);
    ui.label(theme::eyebrow("Proposal evidence bundle"));
    render_proposal_evidence_panel(ui, proposal_evidence_panel);

    if !panel.other_rows.is_empty() {
        render_section(
            ui,
            "Other surfaced rows",
            &panel.other_rows,
            "No additional worker rows projected",
        );
    }
}

fn render_section(ui: &mut egui::Ui, label: &str, rows: &[String], empty: &str) {
    ui.add_space(4.0);
    ui.label(theme::eyebrow(label));
    if rows.is_empty() {
        ui.label(theme::muted(empty));
        return;
    }
    for row in rows {
        ui.label(theme::body(super::trim_middle(row, 120)));
    }
}

fn recovery_actions(snapshot: &ShellProjectionSnapshot) -> Vec<DesktopWorkerRecoveryAction> {
    let mut actions = Vec::new();
    for row in &snapshot.legion_workflow_projection.rows {
        match row.merge_readiness.state {
            LegionWorkflowMergeReadinessState::Blocked => {
                actions.push(DesktopWorkerRecoveryAction {
                    label: format!("Recheck blocked workflow {}", row.session_id.0),
                    rationale: "blocked -> re-evaluate merge readiness".to_string(),
                    action: DesktopAction::RequestLegionWorkflowMergeReadiness {
                        session_id: row.session_id.clone(),
                    },
                });
            }
            LegionWorkflowMergeReadinessState::WaitingForApproval => {
                if let Some(sign_off_id) =
                    first_label_with_prefix(&row.display_safe_labels, "signoff:")
                {
                    actions.push(DesktopWorkerRecoveryAction {
                        label: format!("Request sign-off {}", row.session_id.0),
                        rationale: format!("needs approval -> signoff {sign_off_id}"),
                        action: DesktopAction::RequestLegionWorkflowSignOff {
                            session_id: row.session_id.clone(),
                            sign_off_id: LegionWorkflowSignOffId(sign_off_id),
                        },
                    });
                }
            }
            LegionWorkflowMergeReadinessState::Ready => {}
        }

        if let Some(gate_id) = first_label_with_prefix(&row.display_safe_labels, "verification:") {
            let matching_verification_failed = snapshot
                .verification_run_projection
                .rows
                .iter()
                .any(|verification| {
                    matches!(
                        verification.state,
                        VerificationRunState::Failed | VerificationRunState::Blocked
                    ) && (verification.run_id == gate_id || verification.label == gate_id)
                });
            if row
                .verification_gate_count
                .saturating_sub(row.passed_verification_count)
                > 0
                || matching_verification_failed
            {
                actions.push(DesktopWorkerRecoveryAction {
                    label: format!("Request verification {}", row.session_id.0),
                    rationale: format!("validation failed -> verification {gate_id}"),
                    action: DesktopAction::RequestLegionWorkflowVerification {
                        session_id: row.session_id.clone(),
                        gate_id: LegionWorkflowVerificationGateId(gate_id),
                    },
                });
            }
        }

        if row.unresolved_conflict_count > 0
            && let Some(conflict_id) =
                first_label_with_prefix(&row.display_safe_labels, "conflict:")
        {
            actions.push(DesktopWorkerRecoveryAction {
                label: format!("Resolve conflict {}", row.session_id.0),
                rationale: format!("conflict -> resolve {conflict_id}"),
                action: DesktopAction::ResolveLegionWorkflowConflict {
                    session_id: row.session_id.clone(),
                    conflict_id: LegionWorkflowConflictId(conflict_id),
                },
            });
        }
    }

    actions
}

fn first_label_with_prefix(labels: &[String], prefix: &str) -> Option<String> {
    labels
        .iter()
        .find(|label| label.starts_with(prefix))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        LegionWorkflowMergeReadiness, LegionWorkflowProjection, LegionWorkflowProjectionRow,
        LegionWorkflowSessionId, LegionWorkflowState, ProposalId, RedactionHint, TimestampMillis,
        VerificationRunProjection, VerificationRunRow, VerificationRunState,
    };
    use legion_ui::Shell;

    fn workflow_row(
        session: &str,
        state: LegionWorkflowMergeReadinessState,
        verification_gates: u32,
        passed_verification: u32,
        unresolved_conflicts: u32,
        labels: Vec<&str>,
    ) -> LegionWorkflowProjectionRow {
        LegionWorkflowProjectionRow {
            session_id: LegionWorkflowSessionId(session.to_string()),
            directive_artifact_id: None,
            spec_artifact_id: None,
            task_graph_artifact_id: None,
            lifecycle_state: LegionWorkflowState::Executing,
            worker_count: 1,
            provider_route_required_count: 0,
            dependency_count: 0,
            unresolved_conflict_count: unresolved_conflicts,
            verification_gate_count: verification_gates,
            passed_verification_count: passed_verification,
            sign_off_count: 1,
            signed_off_count: 0,
            linked_proposals: vec![ProposalId(1)],
            merge_readiness: LegionWorkflowMergeReadiness {
                state,
                blockers: Vec::new(),
                labels: vec!["legion_workflow.waiting_for_approval".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            display_safe_labels: labels.into_iter().map(|label| label.to_string()).collect(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    fn recovery_snapshot() -> ShellProjectionSnapshot {
        let mut snapshot = Shell::empty("Worker panel").projection_snapshot();
        snapshot.legion_workflow_projection = LegionWorkflowProjection {
            projection_id: "workflow:test".to_string(),
            rows: vec![
                workflow_row(
                    "session:blocked",
                    LegionWorkflowMergeReadinessState::Blocked,
                    0,
                    0,
                    0,
                    vec![
                        "verification:blocked",
                        "signoff:blocked",
                        "conflict:blocked",
                    ],
                ),
                workflow_row(
                    "session:approval",
                    LegionWorkflowMergeReadinessState::WaitingForApproval,
                    0,
                    0,
                    0,
                    vec![
                        "verification:approval",
                        "signoff:reviewer",
                        "conflict:approval",
                    ],
                ),
                workflow_row(
                    "session:validation",
                    LegionWorkflowMergeReadinessState::Ready,
                    1,
                    0,
                    0,
                    vec![
                        "verification:unit",
                        "signoff:reviewer",
                        "conflict:validation",
                    ],
                ),
                workflow_row(
                    "session:conflict",
                    LegionWorkflowMergeReadinessState::Ready,
                    0,
                    0,
                    1,
                    vec![
                        "verification:conflict",
                        "signoff:reviewer",
                        "conflict:shared",
                    ],
                ),
            ],
            mcp_registries: Vec::new(),
            decision_feed: Vec::new(),
            risk_monitors: Vec::new(),
            kill_switches: Vec::new(),
            tool_permission_requests: Vec::new(),
            total_session_count: 4,
            mcp_registry_count: 0,
            decision_feed_count: 0,
            risk_monitor_count: 0,
            kill_switch_count: 0,
            tool_permission_request_count: 0,
            omitted_row_count: 0,
            generated_at: TimestampMillis(0),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        snapshot.verification_run_projection = VerificationRunProjection {
            projection_id: "verification-runs:test".to_string(),
            rows: vec![VerificationRunRow {
                run_id: "verification:unit".to_string(),
                label: "unit tests".to_string(),
                state: VerificationRunState::Failed,
                command_class_label: "test".to_string(),
                command_body_redacted: true,
                exit_code: Some(1),
                target_labels: vec!["worker-panel".to_string()],
                evidence_artifact_id: None,
                started_at: None,
                completed_at: None,
                risk_label: legion_protocol::ProposalRiskLabel::Low,
                privacy_label: legion_protocol::ProposalPrivacyLabel::PublicMetadata,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            omitted_row_count: 0,
            generated_at: TimestampMillis(0),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        snapshot
    }

    #[test]
    fn recovery_actions_cover_blocked_approval_validation_and_conflict() {
        let snapshot = recovery_snapshot();
        let model = DesktopWorkerPanelViewModel::from_snapshot(&snapshot);

        assert!(model.recovery_actions.iter().any(|action| {
            action.label == "Recheck blocked workflow session:blocked"
                && action.action
                    == DesktopAction::RequestLegionWorkflowMergeReadiness {
                        session_id: LegionWorkflowSessionId("session:blocked".to_string()),
                    }
        }));
        assert!(model.recovery_actions.iter().any(|action| {
            action.label == "Request sign-off session:approval"
                && action.action
                    == DesktopAction::RequestLegionWorkflowSignOff {
                        session_id: LegionWorkflowSessionId("session:approval".to_string()),
                        sign_off_id: LegionWorkflowSignOffId("signoff:reviewer".to_string()),
                    }
        }));
        assert!(model.recovery_actions.iter().any(|action| {
            action.label == "Request verification session:validation"
                && action.action
                    == DesktopAction::RequestLegionWorkflowVerification {
                        session_id: LegionWorkflowSessionId("session:validation".to_string()),
                        gate_id: LegionWorkflowVerificationGateId("verification:unit".to_string()),
                    }
        }));
        assert!(model.recovery_actions.iter().any(|action| {
            action.label == "Resolve conflict session:conflict"
                && action.action
                    == DesktopAction::ResolveLegionWorkflowConflict {
                        session_id: LegionWorkflowSessionId("session:conflict".to_string()),
                        conflict_id: LegionWorkflowConflictId("conflict:shared".to_string()),
                    }
        }));
    }
}
