use legion_ui::{
    LegionWorkflowFleetCardProjection, ShellProjectionSnapshot,
    legion_workflow_fleet_card_projections,
};

use crate::{bridge::DesktopAction, theme};

use super::{primary_button, risk_color, soft_button};

pub(crate) fn render_proposal_cards(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let cards = fleet_card_view_models(snapshot);
    if cards.is_empty() {
        ui.label(theme::muted("No pending proposals"));
        return;
    }

    for card in cards.iter().take(4) {
        theme::card_frame_tinted(
            theme::tokens().bg.card,
            theme::dim(theme::tokens().accent.orange, 48),
        )
        .show(ui, |ui| {
            render_card(ui, card, snapshot, actions);
        });
    }
}

pub(crate) fn fleet_card_view_models(
    snapshot: &ShellProjectionSnapshot,
) -> Vec<LegionWorkflowFleetCardProjection> {
    legion_workflow_fleet_card_projections(
        &snapshot.proposal_ledger_projection,
        &snapshot.verification_run_projection,
    )
}

fn render_card(
    ui: &mut egui::Ui,
    card: &LegionWorkflowFleetCardProjection,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    ui.label(theme::body_strong(&card.title));
    ui.horizontal_wrapped(|ui| {
        render_field(ui, "owner", &card.owner_label);
        render_field(ui, "model", &card.model_label);
        render_field(ui, "status", &card.status_label);
        render_field(ui, "progress", &card.progress_label);
        render_field(ui, "files", &card.files_label);
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::accent(
            format!("risk {:?}", card.risk_label),
            risk_color(card.risk_label),
        ));
        render_field(ui, "test", &card.test_status_label);
        render_field(ui, "diff", &card.mini_diff_label);
        render_field(ui, "updated", &card.last_activity_label);
    });
    ui.horizontal(|ui| {
        if primary_button(ui, "Approve", theme::tokens().accent.green).clicked() {
            actions.push(DesktopAction::ApproveProposal {
                proposal_id: card.proposal_id,
            });
        }
        if soft_button(ui, "Review").clicked() {
            actions.push(DesktopAction::OpenProposalDetails {
                proposal_id: card.proposal_id,
            });
        }
        if snapshot.checkpoint_rollback_projection.proposal_id == card.proposal_id
            && soft_button(ui, "Restore checkpoint").clicked()
            && snapshot.checkpoint_rollback_projection.checkpoint.available
        {
            actions.push(DesktopAction::RestoreCheckpoint {
                proposal_id: card.proposal_id,
            });
        }
        if soft_button(ui, "Reject").clicked() {
            actions.push(DesktopAction::RejectProposal {
                proposal_id: card.proposal_id,
                reason: legion_protocol::ProposalRejectionReason::UserRejected,
            });
        }
    });
}

fn render_field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(theme::muted(format!("{label}={value}")));
}

#[cfg(test)]
mod tests {
    use super::fleet_card_view_models;

    use legion_protocol::{
        CanonicalPath, FileId, ProposalAffectedTarget, ProposalContextManifestEntrySummary,
        ProposalContextManifestSummary, ProposalDiffChunkDescriptor, ProposalDiffSummary,
        ProposalDiffSummaryKind, ProposalId, ProposalLedgerProjection, ProposalLedgerRow,
        ProposalLifecycleState, ProposalLifecycleStateDisplay, ProposalPayloadKind,
        ProposalPreviewWarning, ProposalPreviewWarningKind, ProposalPrivacyLabel,
        ProposalRiskLabel, ProposalRollbackAvailability, ProposalTargetCoverage,
        ProposalTargetCoverageKind, ProposalTargetKind, RedactionHint, TimestampMillis,
        VerificationRunProjection, VerificationRunRow, VerificationRunState, WorkspaceId,
    };
    use legion_ui::Shell;

    #[test]
    fn fleet_card_view_models_project_structured_labels() {
        let mut snapshot = Shell::empty("Legion").projection_snapshot();
        snapshot.proposal_ledger_projection = ProposalLedgerProjection {
            rows: vec![ProposalLedgerRow {
                proposal_id: ProposalId(5),
                workspace_id: Some(WorkspaceId(1)),
                title: "card title".to_string(),
                payload_kind: ProposalPayloadKind::WorkspaceEdit,
                lifecycle: ProposalLifecycleStateDisplay {
                    state: ProposalLifecycleState::Previewed,
                    label: "Previewed".to_string(),
                    description: "ready for review".to_string(),
                },
                principal: legion_protocol::PrincipalId("owner:bob".to_string()),
                capability: legion_protocol::CapabilityId("model:claude".to_string()),
                created_at: TimestampMillis(1),
                updated_at: TimestampMillis(9),
                expires_at: None,
                risk_label: ProposalRiskLabel::High,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                rollback: ProposalRollbackAvailability::Available,
                target_coverage: ProposalTargetCoverage {
                    coverage_kind: ProposalTargetCoverageKind::Complete,
                    targets: vec![ProposalAffectedTarget {
                        target_id: "target:1".to_string(),
                        kind: ProposalTargetKind::ClosedFile,
                        workspace_id: Some(WorkspaceId(1)),
                        file_id: Some(FileId(1)),
                        buffer_id: None,
                        path: Some(CanonicalPath("src/main.rs".to_string())),
                        terminal_session_id: None,
                        plugin_id: None,
                        remote_authority: None,
                        collaboration_session_id: None,
                        byte_ranges: Vec::new(),
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                    }],
                    omitted_target_count: 0,
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                context_manifest: ProposalContextManifestSummary {
                    manifest_id: "manifest:5".to_string(),
                    category_count: 1,
                    total_item_count: 1,
                    omitted_item_count: 0,
                    categories: vec![ProposalContextManifestEntrySummary {
                        category: "files".to_string(),
                        item_count: 1,
                        omitted_item_count: 0,
                        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                        manifest_hash: None,
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                    }],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                diff_summary: ProposalDiffSummary {
                    kind: ProposalDiffSummaryKind::Text,
                    target_count: 1,
                    hunk_count: 1,
                    inserted_line_count: 3,
                    deleted_line_count: 1,
                    omitted_hunk_count: 0,
                    full_source_redacted: true,
                    diff_hash: None,
                    chunks: vec![ProposalDiffChunkDescriptor {
                        chunk_id: "chunk:0".to_string(),
                        target_id: Some("target:1".to_string()),
                        byte_range: None,
                        changed_line_count: 4,
                        inserted_line_count: 3,
                        deleted_line_count: 1,
                        content_hash: None,
                    }],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                },
                preview_warnings: vec![ProposalPreviewWarning {
                    code: "proposal.preview.target-coverage-complete".to_string(),
                    kind: ProposalPreviewWarningKind::TargetCoveragePartial,
                    message: "target coverage is complete".to_string(),
                    target_id: Some("target:1".to_string()),
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                }],
                diagnostics: Vec::new(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            selected_proposal_id: Some(ProposalId(5)),
            omitted_row_count: 0,
            generated_at: TimestampMillis(10),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        snapshot.verification_run_projection = VerificationRunProjection {
            projection_id: "verification:5".to_string(),
            rows: vec![VerificationRunRow {
                run_id: "run:5".to_string(),
                label: "tests".to_string(),
                state: VerificationRunState::Passed,
                command_class_label: "test".to_string(),
                command_body_redacted: true,
                exit_code: Some(0),
                target_labels: vec!["target:1".to_string()],
                evidence_artifact_id: None,
                started_at: Some(TimestampMillis(2)),
                completed_at: Some(TimestampMillis(3)),
                risk_label: ProposalRiskLabel::Low,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            omitted_row_count: 0,
            generated_at: TimestampMillis(11),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let cards = fleet_card_view_models(&snapshot);
        assert_eq!(cards.len(), 1);
        let card = &cards[0];
        assert_eq!(card.title, "card title");
        assert_eq!(card.owner_label, "owner:bob");
        assert_eq!(card.model_label, "model:claude");
        assert_eq!(card.status_label, "Previewed");
        assert_eq!(card.progress_label, "targets=1/1 · hunks=1");
        assert_eq!(card.files_label, "manifest:5 · files=1 items");
        assert_eq!(card.risk_label, ProposalRiskLabel::High);
        assert!(card.test_status_label.starts_with("passed=1"));
        assert_eq!(card.mini_diff_label, "text · targets=1 · hunks=1 · +3/-1");
        assert_eq!(card.last_activity_label, "updated_at=9");
    }
}
