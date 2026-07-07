use legion_app::proposal_risk_rule_ids_from_coverage;
use legion_protocol::{
    ByteRange, DelegatedTaskProposalHunkDisposition, DelegatedTaskProposalReview,
    ProposalEvidencePanel, ProposalId, ProposalLedgerRow, ProposalRiskLabel, TimestampMillis,
    VerificationRunRow, VerificationRunState,
};
use legion_ui::ShellProjectionSnapshot;
use std::collections::BTreeMap;

use super::{bounded_join, theme};

/// Structured proposal-evidence panel model used by the desktop proposal review surface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DesktopProposalEvidencePanelViewModel {
    /// Proposal identifier represented by the checkpoint timeline.
    pub checkpoint_proposal_id: Option<ProposalId>,
    /// Structured checkpoint timeline rows.
    pub checkpoint_timeline_rows: Vec<DesktopCheckpointTimelineRow>,
    /// Structured proposal review rows.
    pub proposal_rows: Vec<DesktopProposalEvidenceRow>,
    /// Structured verification run rows linked to the review surface.
    pub verification_rows: Vec<DesktopVerificationRunEvidenceRow>,
}

/// Structured checkpoint timeline row used to show the proposal's restoreable targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopCheckpointTimelineRow {
    /// Stable target identifier.
    pub target_id: String,
    /// Target kind label.
    pub kind_label: String,
    /// Checkpoint identifier for the proposal.
    pub checkpoint_id: String,
    /// Display-safe target labels.
    pub labels: Vec<String>,
    /// Whether the checkpoint is available for restore.
    pub available: bool,
}

/// Proposal evidence row shown in the proposal review panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopProposalEvidenceRow {
    /// Proposal identifier.
    pub proposal_id: ProposalId,
    /// Display-safe title.
    pub title: String,
    /// Display-safe payload summary.
    pub command_summary_label: String,
    /// Lifecycle label.
    pub lifecycle_label: String,
    /// Risk label.
    pub risk_label: ProposalRiskLabel,
    /// Privacy label.
    pub privacy_label: legion_protocol::ProposalPrivacyLabel,
    /// Rollback summary.
    pub rollback_label: String,
    /// Context manifest summary.
    pub context_manifest: DesktopProposalContextManifestViewModel,
    /// Diff summary.
    pub diff_summary: DesktopProposalDiffSummaryViewModel,
    /// Stable risk rule ids that informed the review surface.
    pub risk_rule_ids: Vec<String>,
    /// Provenance metadata for the row.
    pub provenance: DesktopProposalProvenanceViewModel,
    /// Number of verification rows displayed alongside this proposal.
    pub verification_summary_count: usize,
}

/// Context-manifest view model for a proposal evidence panel row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopProposalContextManifestViewModel {
    /// Stable manifest identifier.
    pub manifest_id: String,
    /// Number of projected categories.
    pub category_count: u32,
    /// Number of projected items.
    pub total_item_count: u32,
    /// Number of omitted items.
    pub omitted_item_count: u32,
    /// Display-safe redaction label.
    pub redaction_label: String,
}

/// Diff-summary view model for a proposal evidence panel row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopProposalDiffSummaryViewModel {
    /// Diff summary kind label.
    pub kind_label: String,
    /// Number of affected targets.
    pub target_count: u32,
    /// Number of hunks.
    pub hunk_count: u32,
    /// Inserted line count.
    pub inserted_line_count: u32,
    /// Deleted line count.
    pub deleted_line_count: u32,
    /// Omitted hunk count.
    pub omitted_hunk_count: u32,
    /// Diff hash label, if available.
    pub diff_hash: Option<String>,
}

/// Provenance view model for a proposal evidence panel row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopProposalProvenanceViewModel {
    /// Creation timestamp.
    pub created_at: TimestampMillis,
    /// Latest update timestamp.
    pub updated_at: TimestampMillis,
    /// Number of preview warnings projected for the row.
    pub preview_warning_count: usize,
    /// Number of diagnostics projected for the row.
    pub diagnostic_count: usize,
}

/// Verification-run evidence row shown in the proposal review panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopVerificationRunEvidenceRow {
    /// Run identifier.
    pub run_id: String,
    /// Display-safe label.
    pub label: String,
    /// Run state.
    pub state: VerificationRunState,
    /// Display-safe command class label.
    pub command_class_label: String,
    /// Whether raw command text was redacted.
    pub command_body_redacted: bool,
    /// Exit code when available.
    pub exit_code: Option<i32>,
    /// Projected target labels.
    pub target_labels: Vec<String>,
    /// Evidence artifact identifier when present.
    pub evidence_artifact_id: Option<String>,
    /// Run start timestamp.
    pub started_at: Option<TimestampMillis>,
    /// Run completion timestamp.
    pub completed_at: Option<TimestampMillis>,
    /// Run risk label.
    pub risk_label: ProposalRiskLabel,
    /// Run privacy label.
    pub privacy_label: legion_protocol::ProposalPrivacyLabel,
}

/// Structured Delegate proposal review file row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopProposalReviewFileViewModel {
    /// Display-safe file label.
    pub file_label: String,
    /// Canonical path when known.
    pub path: Option<String>,
    /// Stable target id when known.
    pub target_id: Option<String>,
    /// Accepted hunk count in this file group.
    pub accepted_hunk_count: u32,
    /// Rejected hunk count in this file group.
    pub rejected_hunk_count: u32,
    /// Pending hunk count in this file group.
    pub pending_hunk_count: u32,
    /// Hunks in deterministic display order.
    pub hunks: Vec<DesktopProposalReviewHunkViewModel>,
}

/// Structured Delegate proposal review hunk row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopProposalReviewHunkViewModel {
    /// Stable hunk identifier.
    pub hunk_id: String,
    /// Stable target id when known.
    pub target_label: String,
    /// Canonical path when known.
    pub path: Option<String>,
    /// Byte range when available.
    pub byte_range: Option<ByteRange>,
    /// Human disposition.
    pub disposition: DelegatedTaskProposalHunkDisposition,
    /// Whether this hunk has a path that can be opened for in-place editing.
    pub edit_in_place_path: Option<String>,
}

// ─── ProposalEvidencePanel DTO view model (F6) ───────────────────────────────

/// Structured view model for the `ProposalEvidencePanel` protocol DTO.
///
/// Maps the flat DTO fields into renderer-ready rows.  Populated via
/// `From<ProposalEvidencePanel>`.  All fields are structured; no free-text
/// provider output is exposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEvidencePanelDtoViewModel {
    /// Proposal identifier from provenance.
    pub proposal_id: ProposalId,
    /// Risk label from provenance.
    pub risk_label: ProposalRiskLabel,
    /// Privacy label from provenance (display-safe).
    pub privacy_label_label: String,
    /// Creation timestamp (milliseconds since epoch).
    pub created_at: TimestampMillis,
    /// Update timestamp (milliseconds since epoch).
    pub updated_at: TimestampMillis,
    /// Structured test results summary row, if present.
    pub test_results: Option<DesktopTestResultsSummaryRow>,
    /// Bounded list of command summary rows.
    pub command_summary_rows: Vec<DesktopCommandSummaryRow>,
    /// Risk rule label rows.
    pub risk_rule_rows: Vec<DesktopRiskRuleRow>,
}

/// Structured test results row for the evidence panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopTestResultsSummaryRow {
    /// Total test count.
    pub total_count: u32,
    /// Passed test count.
    pub passed_count: u32,
    /// Failed test count.
    pub failed_count: u32,
    /// Skipped test count.
    pub skipped_count: u32,
    /// Stable run identifier.
    pub run_id: String,
}

/// Structured command summary row for the evidence panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopCommandSummaryRow {
    /// Display-safe command class label (never raw command text).
    pub command_class: String,
    /// Process exit code when available.
    pub exit_code: Option<i32>,
    /// True when the full command text was redacted.
    pub redacted: bool,
}

/// Structured risk rule row for the evidence panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopRiskRuleRow {
    /// Stable rule identifier.
    pub rule_id: String,
    /// Whether the rule was triggered.
    pub triggered: bool,
    /// Display-safe rationale label.
    pub rationale_label: String,
}

impl From<ProposalEvidencePanel> for DesktopEvidencePanelDtoViewModel {
    fn from(panel: ProposalEvidencePanel) -> Self {
        Self {
            proposal_id: panel.provenance.proposal_id,
            risk_label: panel.provenance.risk_label,
            privacy_label_label: format!("{:?}", panel.provenance.privacy_label),
            created_at: panel.provenance.created_at,
            updated_at: panel.provenance.updated_at,
            test_results: panel.test_results.map(|tr| DesktopTestResultsSummaryRow {
                total_count: tr.total_count,
                passed_count: tr.passed_count,
                failed_count: tr.failed_count,
                skipped_count: tr.skipped_count,
                run_id: tr.run_id,
            }),
            command_summary_rows: panel
                .command_summaries
                .into_iter()
                .map(|cs| DesktopCommandSummaryRow {
                    command_class: cs.command_class,
                    exit_code: cs.exit_code,
                    redacted: cs.redacted,
                })
                .collect(),
            risk_rule_rows: panel
                .risk_rules
                .into_iter()
                .map(|rr| DesktopRiskRuleRow {
                    rule_id: rr.rule_id,
                    triggered: rr.triggered,
                    rationale_label: rr.rationale_label,
                })
                .collect(),
        }
    }
}

#[allow(dead_code)]
pub(crate) fn proposal_review_file_groups(
    review: &DelegatedTaskProposalReview,
) -> Vec<DesktopProposalReviewFileViewModel> {
    let mut files: BTreeMap<String, DesktopProposalReviewFileViewModel> = BTreeMap::new();
    for hunk in &review.hunks {
        let file_label = hunk
            .path
            .as_ref()
            .map(|path| path.0.clone())
            .or_else(|| hunk.target_id.clone())
            .unwrap_or_else(|| hunk.hunk_id.clone());
        let entry =
            files
                .entry(file_label.clone())
                .or_insert_with(|| DesktopProposalReviewFileViewModel {
                    file_label: file_label.clone(),
                    path: hunk.path.as_ref().map(|path| path.0.clone()),
                    target_id: hunk.target_id.clone(),
                    accepted_hunk_count: 0,
                    rejected_hunk_count: 0,
                    pending_hunk_count: 0,
                    hunks: Vec::new(),
                });

        match hunk.disposition {
            DelegatedTaskProposalHunkDisposition::Accepted => {
                entry.accepted_hunk_count = entry.accepted_hunk_count.saturating_add(1)
            }
            DelegatedTaskProposalHunkDisposition::Rejected => {
                entry.rejected_hunk_count = entry.rejected_hunk_count.saturating_add(1)
            }
            DelegatedTaskProposalHunkDisposition::Pending => {
                entry.pending_hunk_count = entry.pending_hunk_count.saturating_add(1)
            }
        }

        entry.hunks.push(DesktopProposalReviewHunkViewModel {
            hunk_id: hunk.hunk_id.clone(),
            target_label: hunk
                .target_id
                .clone()
                .unwrap_or_else(|| "<none>".to_string()),
            path: hunk.path.as_ref().map(|path| path.0.clone()),
            byte_range: hunk.byte_range,
            disposition: hunk.disposition,
            edit_in_place_path: hunk.path.as_ref().map(|path| path.0.clone()),
        });
    }

    files.into_values().collect()
}

#[allow(dead_code)]
pub(crate) fn proposal_evidence_panel(
    snapshot: &ShellProjectionSnapshot,
) -> DesktopProposalEvidencePanelViewModel {
    let verification_summary_count = snapshot.verification_run_projection.rows.len();
    let checkpoint_projection = &snapshot.checkpoint_rollback_projection;
    let checkpoint_proposal_id = if checkpoint_projection.proposal_id.0 == 0 {
        snapshot
            .proposal_ledger_projection
            .selected_proposal_id
            .or(Some(checkpoint_projection.proposal_id))
    } else {
        Some(checkpoint_projection.proposal_id)
    };
    DesktopProposalEvidencePanelViewModel {
        checkpoint_proposal_id,
        checkpoint_timeline_rows: checkpoint_projection
            .targets
            .iter()
            .map(|target| DesktopCheckpointTimelineRow {
                target_id: target.target_id.clone(),
                kind_label: format!("{:?}", target.kind),
                checkpoint_id: checkpoint_projection.checkpoint.checkpoint_id.clone(),
                labels: target.labels.clone(),
                available: checkpoint_projection.checkpoint.available,
            })
            .collect(),
        proposal_rows: snapshot
            .proposal_ledger_projection
            .rows
            .iter()
            .take(4)
            .map(|row| proposal_evidence_row(row, verification_summary_count))
            .collect(),
        verification_rows: snapshot
            .verification_run_projection
            .rows
            .iter()
            .take(6)
            .map(verification_evidence_row)
            .collect(),
    }
}

#[allow(dead_code)]
pub(crate) fn render_proposal_evidence_panel(
    ui: &mut egui::Ui,
    panel: &DesktopProposalEvidencePanelViewModel,
) {
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::body_strong("Evidence panel"));
        if panel.proposal_rows.is_empty() && panel.verification_rows.is_empty() {
            ui.label(theme::muted("No proposal evidence projected"));
            return;
        }

        if !panel.proposal_rows.is_empty() {
            ui.label(theme::muted(format!(
                "{} proposal row(s) with structured fields",
                panel.proposal_rows.len()
            )));
            for row in &panel.proposal_rows {
                theme::small_card_frame().show(ui, |ui| {
                    ui.label(theme::body_strong(&row.title));
                    ui.horizontal_wrapped(|ui| {
                        ui.label(theme::muted(format!("proposal {}", row.proposal_id.0)));
                        ui.separator();
                        ui.label(theme::muted(&row.command_summary_label));
                        ui.separator();
                        ui.label(theme::accent(
                            format!("{:?} risk", row.risk_label),
                            super::risk_color(row.risk_label),
                        ));
                        ui.separator();
                        ui.label(theme::muted(format!("{:?}", row.privacy_label)));
                    });
                    ui.label(theme::muted(format!(
                        "lifecycle={} rollback={}",
                        row.lifecycle_label, row.rollback_label
                    )));
                    ui.label(theme::muted(format!(
                        "context manifest={} categories={} items={} omitted={} redaction={}",
                        row.context_manifest.manifest_id,
                        row.context_manifest.category_count,
                        row.context_manifest.total_item_count,
                        row.context_manifest.omitted_item_count,
                        row.context_manifest.redaction_label,
                    )));
                    ui.label(theme::muted(format!(
                        "diff kind={} targets={} hunks={} +{} -{} omitted={} hash={}",
                        row.diff_summary.kind_label,
                        row.diff_summary.target_count,
                        row.diff_summary.hunk_count,
                        row.diff_summary.inserted_line_count,
                        row.diff_summary.deleted_line_count,
                        row.diff_summary.omitted_hunk_count,
                        row.diff_summary.diff_hash.as_deref().unwrap_or("<none>"),
                    )));
                    ui.label(theme::muted(format!(
                        "risk rules={} verification summaries={}",
                        bounded_join(&row.risk_rule_ids),
                        row.verification_summary_count,
                    )));
                    ui.label(theme::muted(format!(
                        "provenance created={} updated={} warnings={} diagnostics={}",
                        row.provenance.created_at.0,
                        row.provenance.updated_at.0,
                        row.provenance.preview_warning_count,
                        row.provenance.diagnostic_count,
                    )));
                });
            }
        }

        if let Some(proposal_id) = panel.checkpoint_proposal_id {
            ui.label(theme::muted(format!(
                "checkpoint timeline proposal={} rows={}",
                proposal_id.0,
                panel.checkpoint_timeline_rows.len()
            )));
        }
        if !panel.checkpoint_timeline_rows.is_empty() {
            ui.label(theme::muted("checkpoint timeline"));
            for row in &panel.checkpoint_timeline_rows {
                theme::small_card_frame().show(ui, |ui| {
                    ui.label(theme::body_strong(&row.checkpoint_id));
                    ui.horizontal_wrapped(|ui| {
                        ui.label(theme::muted(format!("target {}", row.target_id)));
                        ui.separator();
                        ui.label(theme::muted(&row.kind_label));
                        ui.separator();
                        ui.label(theme::muted(if row.available {
                            "available"
                        } else {
                            "unavailable"
                        }));
                    });
                    if !row.labels.is_empty() {
                        ui.label(theme::muted(format!(
                            "labels={}",
                            bounded_join(&row.labels)
                        )));
                    }
                });
            }
        }

        if !panel.verification_rows.is_empty() {
            ui.label(theme::muted(format!(
                "{} verification row(s) with structured command summaries",
                panel.verification_rows.len()
            )));
            for row in &panel.verification_rows {
                theme::small_card_frame().show(ui, |ui| {
                    ui.label(theme::body_strong(&row.label));
                    ui.horizontal_wrapped(|ui| {
                        ui.label(theme::muted(format!("run {}", row.run_id)));
                        ui.separator();
                        ui.label(theme::muted(&row.command_class_label));
                        ui.separator();
                        ui.label(theme::muted(format!("{:?}", row.state)));
                        ui.separator();
                        ui.label(theme::accent(
                            format!("{:?} risk", row.risk_label),
                            super::risk_color(row.risk_label),
                        ));
                    });
                    ui.label(theme::muted(format!(
                        "command redacted={} exit={} evidence={} privacy={:?}",
                        row.command_body_redacted,
                        row.exit_code
                            .map(|code| code.to_string())
                            .unwrap_or_else(|| "<none>".to_string()),
                        row.evidence_artifact_id.as_deref().unwrap_or("<none>"),
                        row.privacy_label,
                    )));
                    ui.label(theme::muted(format!(
                        "targets={} started={} completed={}",
                        bounded_join(&row.target_labels),
                        row.started_at
                            .as_ref()
                            .map(|ts| ts.0.to_string())
                            .unwrap_or_else(|| "<none>".to_string()),
                        row.completed_at
                            .as_ref()
                            .map(|ts| ts.0.to_string())
                            .unwrap_or_else(|| "<none>".to_string()),
                    )));
                });
            }
        }
    });
}

#[allow(dead_code)]
fn proposal_evidence_row(
    row: &ProposalLedgerRow,
    verification_summary_count: usize,
) -> DesktopProposalEvidenceRow {
    DesktopProposalEvidenceRow {
        proposal_id: row.proposal_id,
        title: row.title.clone(),
        command_summary_label: format!("{:?}", row.payload_kind),
        lifecycle_label: row.lifecycle.label.clone(),
        risk_label: row.risk_label,
        privacy_label: row.privacy_label,
        rollback_label: format!("{:?}", row.rollback),
        context_manifest: DesktopProposalContextManifestViewModel {
            manifest_id: row.context_manifest.manifest_id.clone(),
            category_count: row.context_manifest.category_count,
            total_item_count: row.context_manifest.total_item_count,
            omitted_item_count: row.context_manifest.omitted_item_count,
            redaction_label: bounded_join(
                &row.context_manifest
                    .redaction_hints
                    .iter()
                    .map(|hint| format!("{:?}", hint))
                    .collect::<Vec<_>>(),
            ),
        },
        diff_summary: DesktopProposalDiffSummaryViewModel {
            kind_label: format!("{:?}", row.diff_summary.kind),
            target_count: row.diff_summary.target_count,
            hunk_count: row.diff_summary.hunk_count,
            inserted_line_count: row.diff_summary.inserted_line_count,
            deleted_line_count: row.diff_summary.deleted_line_count,
            omitted_hunk_count: row.diff_summary.omitted_hunk_count,
            diff_hash: row
                .diff_summary
                .diff_hash
                .as_ref()
                .map(|hash| hash.value.clone()),
        },
        risk_rule_ids: proposal_risk_rule_ids_from_coverage(&row.target_coverage),
        provenance: DesktopProposalProvenanceViewModel {
            created_at: row.created_at,
            updated_at: row.updated_at,
            preview_warning_count: row.preview_warnings.len(),
            diagnostic_count: row.diagnostics.len(),
        },
        verification_summary_count,
    }
}

#[allow(dead_code)]
fn verification_evidence_row(row: &VerificationRunRow) -> DesktopVerificationRunEvidenceRow {
    DesktopVerificationRunEvidenceRow {
        run_id: row.run_id.clone(),
        label: row.label.clone(),
        state: row.state,
        command_class_label: row.command_class_label.clone(),
        command_body_redacted: row.command_body_redacted,
        exit_code: row.exit_code,
        target_labels: row.target_labels.clone(),
        evidence_artifact_id: row.evidence_artifact_id.clone(),
        started_at: row.started_at,
        completed_at: row.completed_at,
        risk_label: row.risk_label,
        privacy_label: row.privacy_label,
    }
}

#[cfg(test)]
mod tests {
    use super::{DesktopProposalReviewFileViewModel, proposal_review_file_groups};
    use legion_protocol::{
        ByteRange, CanonicalPath, DelegatedTaskProposalHunkDisposition,
        DelegatedTaskProposalHunkReview, DelegatedTaskProposalReview, ProposalId,
        ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel, RedactionHint,
    };

    fn hunk(
        hunk_id: &str,
        path: Option<&str>,
        target_id: Option<&str>,
        disposition: DelegatedTaskProposalHunkDisposition,
    ) -> DelegatedTaskProposalHunkReview {
        DelegatedTaskProposalHunkReview {
            hunk_id: hunk_id.to_string(),
            proposal_id: ProposalId(7),
            target_id: target_id.map(ToOwned::to_owned),
            payload_kind: ProposalPayloadKind::WorkspaceEdit,
            path: path.map(|path| CanonicalPath(path.to_string())),
            byte_range: Some(ByteRange::new(0, 12)),
            changed_line_count: 1,
            inserted_line_count: 1,
            deleted_line_count: 0,
            content_hash: None,
            disposition,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["delegate.proposal_hunk.human_review".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn proposal_review_file_groups_group_hunks_by_file_and_keep_edit_paths() {
        let review = DelegatedTaskProposalReview::from_hunks(
            "delegate:review:7",
            ProposalId(7),
            vec![
                hunk(
                    "delegate:proposal:7:chunk:0",
                    Some("src/lib.rs"),
                    Some("target:lib"),
                    DelegatedTaskProposalHunkDisposition::Accepted,
                ),
                hunk(
                    "delegate:proposal:7:chunk:1",
                    Some("src/lib.rs"),
                    Some("target:lib-2"),
                    DelegatedTaskProposalHunkDisposition::Pending,
                ),
                hunk(
                    "delegate:proposal:7:chunk:2",
                    None,
                    Some("target:docs"),
                    DelegatedTaskProposalHunkDisposition::Rejected,
                ),
            ],
            vec!["delegate.proposal_review.human_approval_queue".to_string()],
            1,
        );

        let files = proposal_review_file_groups(&review);
        assert_eq!(files.len(), 2);

        let file = files
            .iter()
            .find(|file| file.file_label == "src/lib.rs")
            .expect("src/lib.rs should be grouped");
        assert_eq!(file.hunks.len(), 2);
        assert_eq!(file.accepted_hunk_count, 1);
        assert_eq!(file.pending_hunk_count, 1);
        assert_eq!(file.rejected_hunk_count, 0);
        assert!(file.hunks.iter().all(|hunk| {
            hunk.path.as_deref() == Some("src/lib.rs")
                && hunk.edit_in_place_path.as_deref() == Some("src/lib.rs")
        }));

        let fallback = files
            .iter()
            .find(|DesktopProposalReviewFileViewModel { path, .. }| path.is_none())
            .expect("target-only hunk should still be grouped");
        assert_eq!(fallback.file_label, "target:docs");
        assert_eq!(fallback.rejected_hunk_count, 1);
        assert_eq!(fallback.hunks[0].edit_in_place_path, None);
        assert_eq!(fallback.hunks[0].target_label, "target:docs");
    }
}
