//! Coordinator-state projections for UI surfaces.

use legion_protocol::{
    LegionWorkflowProjection, LegionWorkflowProjectionRow, LegionWorkflowSessionId,
    LegionWorkflowState, ProposalDiffSummaryKind, ProposalId, ProposalLedgerProjection,
    ProposalLedgerRow, ProposalRiskLabel, VerificationRunProjection, VerificationRunState,
};
use serde::{Deserialize, Serialize};

/// Kanban column kinds derived from coordinator state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegionWorkflowBoardColumnKind {
    /// Session is assigned or still being planned.
    Assigned,
    /// Session is actively running.
    InProgress,
    /// Session is waiting on a human approval or a fail-closed blocker.
    WaitingOnHuman,
    /// Session is verifying output.
    Testing,
    /// Session is complete or terminal.
    Done,
}

impl LegionWorkflowBoardColumnKind {
    /// Stable display label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Assigned => "Assigned",
            Self::InProgress => "In Progress",
            Self::WaitingOnHuman => "Waiting on Human",
            Self::Testing => "Testing",
            Self::Done => "Done",
        }
    }

    fn from_state(state: LegionWorkflowState) -> Self {
        match state {
            LegionWorkflowState::Draft | LegionWorkflowState::Planning => Self::Assigned,
            LegionWorkflowState::Executing => Self::InProgress,
            LegionWorkflowState::WaitingForApproval
            | LegionWorkflowState::WaitingOnHuman
            | LegionWorkflowState::Blocked => Self::WaitingOnHuman,
            LegionWorkflowState::Verifying => Self::Testing,
            LegionWorkflowState::Completed
            | LegionWorkflowState::Failed
            | LegionWorkflowState::Cancelled => Self::Done,
        }
    }
}

/// One workflow row projected into the fleet Kanban board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionWorkflowBoardRowProjection {
    /// Stable workflow session identifier.
    pub session_id: LegionWorkflowSessionId,
    /// Coordinator state used to place the row.
    pub state: LegionWorkflowState,
    /// Human-readable status label derived from the state.
    pub state_label: String,
    /// Display-safe summary rendered in the card body.
    pub summary_label: String,
}

/// One Kanban column projected from coordinator state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionWorkflowBoardColumnProjection {
    /// Stable column kind.
    pub kind: LegionWorkflowBoardColumnKind,
    /// Human-readable column title.
    pub title: String,
    /// Rows assigned to this column.
    pub rows: Vec<LegionWorkflowBoardRowProjection>,
}

/// Project the workflow coordinator state into the five fleet Kanban columns.
pub fn legion_workflow_board_columns(
    projection: &LegionWorkflowProjection,
) -> Vec<LegionWorkflowBoardColumnProjection> {
    let mut columns = vec![
        LegionWorkflowBoardColumnProjection {
            kind: LegionWorkflowBoardColumnKind::Assigned,
            title: LegionWorkflowBoardColumnKind::Assigned.label().to_string(),
            rows: Vec::new(),
        },
        LegionWorkflowBoardColumnProjection {
            kind: LegionWorkflowBoardColumnKind::InProgress,
            title: LegionWorkflowBoardColumnKind::InProgress
                .label()
                .to_string(),
            rows: Vec::new(),
        },
        LegionWorkflowBoardColumnProjection {
            kind: LegionWorkflowBoardColumnKind::WaitingOnHuman,
            title: LegionWorkflowBoardColumnKind::WaitingOnHuman
                .label()
                .to_string(),
            rows: Vec::new(),
        },
        LegionWorkflowBoardColumnProjection {
            kind: LegionWorkflowBoardColumnKind::Testing,
            title: LegionWorkflowBoardColumnKind::Testing.label().to_string(),
            rows: Vec::new(),
        },
        LegionWorkflowBoardColumnProjection {
            kind: LegionWorkflowBoardColumnKind::Done,
            title: LegionWorkflowBoardColumnKind::Done.label().to_string(),
            rows: Vec::new(),
        },
    ];

    for row in &projection.rows {
        let kind = LegionWorkflowBoardColumnKind::from_state(row.lifecycle_state);
        let summary_label = workflow_board_row_summary(row);
        let projected_row = LegionWorkflowBoardRowProjection {
            session_id: row.session_id.clone(),
            state: row.lifecycle_state,
            state_label: workflow_state_label(row.lifecycle_state).to_string(),
            summary_label,
        };

        match kind {
            LegionWorkflowBoardColumnKind::Assigned => columns[0].rows.push(projected_row),
            LegionWorkflowBoardColumnKind::InProgress => columns[1].rows.push(projected_row),
            LegionWorkflowBoardColumnKind::WaitingOnHuman => columns[2].rows.push(projected_row),
            LegionWorkflowBoardColumnKind::Testing => columns[3].rows.push(projected_row),
            LegionWorkflowBoardColumnKind::Done => columns[4].rows.push(projected_row),
        }
    }

    columns
}

fn workflow_state_label(state: LegionWorkflowState) -> &'static str {
    match state {
        LegionWorkflowState::Draft => "Draft",
        LegionWorkflowState::Planning => "Planning",
        LegionWorkflowState::Executing => "Executing",
        LegionWorkflowState::Verifying => "Verifying",
        LegionWorkflowState::WaitingForApproval => "Waiting for approval",
        LegionWorkflowState::WaitingOnHuman => "Waiting on human",
        LegionWorkflowState::Blocked => "Blocked",
        LegionWorkflowState::Completed => "Completed",
        LegionWorkflowState::Failed => "Failed",
        LegionWorkflowState::Cancelled => "Cancelled",
    }
}

fn workflow_board_row_summary(row: &LegionWorkflowProjectionRow) -> String {
    let mut parts = vec![
        row.session_id.0.clone(),
        workflow_state_label(row.lifecycle_state).to_string(),
        format!("workers={}", row.worker_count),
        format!("deps={}", row.dependency_count),
        format!("conflicts={}", row.unresolved_conflict_count),
        format!(
            "verify={}/{}",
            row.passed_verification_count, row.verification_gate_count
        ),
        format!("signoff={}/{}", row.signed_off_count, row.sign_off_count),
        format!("merge={:?}", row.merge_readiness.state),
    ];

    if !row.display_safe_labels.is_empty() {
        parts.push(row.display_safe_labels.join(" | "));
    }

    parts.join(" · ")
}

/// Structured fleet-card projection for proposal-ledger cards rendered in the desktop UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegionWorkflowFleetCardProjection {
    /// Proposal identifier.
    pub proposal_id: ProposalId,
    /// Proposal title.
    pub title: String,
    /// Owner label projected from the principal.
    pub owner_label: String,
    /// Model label projected from the requested capability.
    pub model_label: String,
    /// Lifecycle label.
    pub status_label: String,
    /// Progress label projected from target coverage and diff size.
    pub progress_label: String,
    /// Files/context label projected from the context manifest.
    pub files_label: String,
    /// Risk label.
    pub risk_label: ProposalRiskLabel,
    /// Aggregated verification status label.
    pub test_status_label: String,
    /// Compact diff summary label.
    pub mini_diff_label: String,
    /// Last activity label.
    pub last_activity_label: String,
}

/// Project proposal-ledger rows into structured fleet cards.
pub fn legion_workflow_fleet_card_projections(
    proposal_projection: &ProposalLedgerProjection,
    verification_projection: &VerificationRunProjection,
) -> Vec<LegionWorkflowFleetCardProjection> {
    let test_status_label = verification_status_label(verification_projection);
    proposal_projection
        .rows
        .iter()
        .map(|row| legion_workflow_fleet_card_projection(row, &test_status_label))
        .collect()
}

fn legion_workflow_fleet_card_projection(
    row: &ProposalLedgerRow,
    test_status_label: &str,
) -> LegionWorkflowFleetCardProjection {
    let represented_targets = row.target_coverage.targets.len() as u32;
    let total_targets =
        represented_targets.saturating_add(row.target_coverage.omitted_target_count);
    let files_label = format!(
        "{} · files={} items",
        row.context_manifest.manifest_id, row.context_manifest.total_item_count
    );

    LegionWorkflowFleetCardProjection {
        proposal_id: row.proposal_id,
        title: row.title.clone(),
        owner_label: row.principal.0.clone(),
        model_label: row.capability.0.clone(),
        status_label: row.lifecycle.label.clone(),
        progress_label: format!(
            "targets={represented_targets}/{total_targets} · hunks={}",
            row.diff_summary.hunk_count
        ),
        files_label,
        risk_label: row.risk_label,
        test_status_label: test_status_label.to_string(),
        mini_diff_label: mini_diff_label(&row.diff_summary),
        last_activity_label: format!("updated_at={}", row.updated_at.0),
    }
}

fn verification_status_label(projection: &VerificationRunProjection) -> String {
    let mut planned = 0u32;
    let mut running = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut blocked = 0u32;
    let mut cancelled = 0u32;

    for row in &projection.rows {
        match row.state {
            VerificationRunState::Planned => planned = planned.saturating_add(1),
            VerificationRunState::Running => running = running.saturating_add(1),
            VerificationRunState::Passed => passed = passed.saturating_add(1),
            VerificationRunState::Failed => failed = failed.saturating_add(1),
            VerificationRunState::Blocked => blocked = blocked.saturating_add(1),
            VerificationRunState::Cancelled => cancelled = cancelled.saturating_add(1),
        }
    }

    format!(
        "passed={passed} failed={failed} blocked={blocked} running={running} planned={planned} cancelled={cancelled}"
    )
}

fn mini_diff_label(diff_summary: &legion_protocol::ProposalDiffSummary) -> String {
    format!(
        "{} · targets={} · hunks={} · +{}/-{}",
        diff_summary_kind_label(diff_summary.kind),
        diff_summary.target_count,
        diff_summary.hunk_count,
        diff_summary.inserted_line_count,
        diff_summary.deleted_line_count,
    )
}

fn diff_summary_kind_label(kind: ProposalDiffSummaryKind) -> &'static str {
    match kind {
        ProposalDiffSummaryKind::Text => "text",
        ProposalDiffSummaryKind::FileOperation => "file ops",
        ProposalDiffSummaryKind::WorkspaceEdit => "workspace",
        ProposalDiffSummaryKind::TerminalMetadata => "terminal",
        ProposalDiffSummaryKind::MetadataOnly => "metadata-only",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use legion_protocol::{
        LegionWorkflowMergeReadiness, LegionWorkflowMergeReadinessState,
        LegionWorkflowProjectionRow, RedactionHint, TimestampMillis,
    };

    fn row(session: &str, state: LegionWorkflowState) -> LegionWorkflowProjectionRow {
        LegionWorkflowProjectionRow {
            session_id: LegionWorkflowSessionId(session.to_string()),
            directive_artifact_id: Some(format!("artifact:directive:{session}")),
            spec_artifact_id: Some(format!("artifact:spec:{session}")),
            task_graph_artifact_id: Some(format!("artifact:task-graph:{session}")),
            lifecycle_state: state,
            worker_count: 1,
            provider_route_required_count: 0,
            dependency_count: 0,
            unresolved_conflict_count: 0,
            verification_gate_count: 0,
            passed_verification_count: 0,
            sign_off_count: 0,
            signed_off_count: 0,
            linked_proposals: Vec::new(),
            merge_readiness: LegionWorkflowMergeReadiness {
                state: LegionWorkflowMergeReadinessState::WaitingForApproval,
                blockers: Vec::new(),
                labels: vec!["approval-gated".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            display_safe_labels: vec![format!("{session}:{state:?}"), "metadata-only".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }

    #[test]
    fn groups_rows_by_coordinator_state() {
        let projection = LegionWorkflowProjection {
            projection_id: "legion-workflow:test-board".to_string(),
            rows: vec![
                row("session:draft", LegionWorkflowState::Draft),
                row("session:planning", LegionWorkflowState::Planning),
                row("session:executing", LegionWorkflowState::Executing),
                row("session:verifying", LegionWorkflowState::Verifying),
                row("session:waiting", LegionWorkflowState::WaitingForApproval),
                row("session:blocked", LegionWorkflowState::Blocked),
                row("session:completed", LegionWorkflowState::Completed),
                row("session:failed", LegionWorkflowState::Failed),
                row("session:cancelled", LegionWorkflowState::Cancelled),
            ],
            mcp_registries: Vec::new(),
            decision_feed: Vec::new(),
            risk_monitors: Vec::new(),
            kill_switches: Vec::new(),
            tool_permission_requests: Vec::new(),
            total_session_count: 9,
            mcp_registry_count: 0,
            decision_feed_count: 0,
            risk_monitor_count: 0,
            kill_switch_count: 0,
            tool_permission_request_count: 0,
            omitted_row_count: 0,
            generated_at: TimestampMillis(1),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let columns = legion_workflow_board_columns(&projection);
        let kinds: Vec<_> = columns.iter().map(|column| column.kind).collect();

        assert_eq!(
            kinds,
            vec![
                LegionWorkflowBoardColumnKind::Assigned,
                LegionWorkflowBoardColumnKind::InProgress,
                LegionWorkflowBoardColumnKind::WaitingOnHuman,
                LegionWorkflowBoardColumnKind::Testing,
                LegionWorkflowBoardColumnKind::Done,
            ]
        );
        assert_eq!(columns[0].rows.len(), 2);
        assert_eq!(columns[1].rows.len(), 1);
        assert_eq!(columns[2].rows.len(), 2);
        assert_eq!(columns[3].rows.len(), 1);
        assert_eq!(columns[4].rows.len(), 3);
        assert!(columns[0].rows.iter().all(|row| matches!(
            row.state,
            LegionWorkflowState::Draft | LegionWorkflowState::Planning
        )));
        assert!(
            columns[1]
                .rows
                .iter()
                .all(|row| row.state == LegionWorkflowState::Executing)
        );
        assert!(columns[2].rows.iter().all(|row| matches!(
            row.state,
            LegionWorkflowState::WaitingForApproval
                | LegionWorkflowState::WaitingOnHuman
                | LegionWorkflowState::Blocked
        )));
        assert!(
            columns[3]
                .rows
                .iter()
                .all(|row| row.state == LegionWorkflowState::Verifying)
        );
        assert!(columns[4].rows.iter().all(|row| matches!(
            row.state,
            LegionWorkflowState::Completed
                | LegionWorkflowState::Failed
                | LegionWorkflowState::Cancelled
        )));
    }

    #[test]
    fn projects_fleet_card_fields_from_structured_projections() {
        let proposal_projection = legion_protocol::ProposalLedgerProjection {
            rows: vec![legion_protocol::ProposalLedgerRow {
                proposal_id: legion_protocol::ProposalId(77),
                workspace_id: Some(legion_protocol::WorkspaceId(1)),
                title: "workflow card".to_string(),
                payload_kind: legion_protocol::ProposalPayloadKind::WorkspaceEdit,
                lifecycle: legion_protocol::ProposalLifecycleStateDisplay {
                    state: legion_protocol::ProposalLifecycleState::Previewed,
                    label: "Previewed".to_string(),
                    description: "ready for review".to_string(),
                },
                principal: legion_protocol::PrincipalId("owner:alice".to_string()),
                capability: legion_protocol::CapabilityId("model:gpt-5.5".to_string()),
                created_at: TimestampMillis(1),
                updated_at: TimestampMillis(2),
                expires_at: None,
                risk_label: legion_protocol::ProposalRiskLabel::Medium,
                privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                rollback: legion_protocol::ProposalRollbackAvailability::Available,
                target_coverage: legion_protocol::ProposalTargetCoverage {
                    coverage_kind: legion_protocol::ProposalTargetCoverageKind::Partial,
                    targets: vec![legion_protocol::ProposalAffectedTarget {
                        target_id: "file:alpha".to_string(),
                        kind: legion_protocol::ProposalTargetKind::ClosedFile,
                        workspace_id: Some(legion_protocol::WorkspaceId(1)),
                        file_id: Some(legion_protocol::FileId(5)),
                        buffer_id: None,
                        path: Some(legion_protocol::CanonicalPath("src/lib.rs".to_string())),
                        terminal_session_id: None,
                        plugin_id: None,
                        remote_authority: None,
                        collaboration_session_id: None,
                        byte_ranges: Vec::new(),
                        redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                    }],
                    omitted_target_count: 0,
                    redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                },
                context_manifest: legion_protocol::ProposalContextManifestSummary {
                    manifest_id: "manifest:77".to_string(),
                    category_count: 1,
                    total_item_count: 2,
                    omitted_item_count: 0,
                    categories: vec![legion_protocol::ProposalContextManifestEntrySummary {
                        category: "files".to_string(),
                        item_count: 2,
                        omitted_item_count: 0,
                        privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                        manifest_hash: None,
                        redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                    }],
                    redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                },
                diff_summary: legion_protocol::ProposalDiffSummary {
                    kind: legion_protocol::ProposalDiffSummaryKind::Text,
                    target_count: 1,
                    hunk_count: 2,
                    inserted_line_count: 5,
                    deleted_line_count: 1,
                    omitted_hunk_count: 0,
                    full_source_redacted: true,
                    diff_hash: Some(legion_protocol::FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "diff:77".to_string(),
                    }),
                    chunks: vec![legion_protocol::ProposalDiffChunkDescriptor {
                        chunk_id: "chunk:0".to_string(),
                        target_id: Some("file:alpha".to_string()),
                        byte_range: None,
                        changed_line_count: 6,
                        inserted_line_count: 5,
                        deleted_line_count: 1,
                        content_hash: None,
                    }],
                    redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                },
                preview_warnings: vec![legion_protocol::ProposalPreviewWarning {
                    code: "proposal.preview.target-coverage-partial".to_string(),
                    kind: legion_protocol::ProposalPreviewWarningKind::TargetCoveragePartial,
                    message: "target coverage is partial".to_string(),
                    target_id: Some("file:alpha".to_string()),
                    redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                }],
                diagnostics: Vec::new(),
                redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            selected_proposal_id: Some(legion_protocol::ProposalId(77)),
            omitted_row_count: 0,
            generated_at: TimestampMillis(3),
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        let verification_projection = legion_protocol::VerificationRunProjection {
            projection_id: "verification-runs:77".to_string(),
            rows: vec![legion_protocol::VerificationRunRow {
                run_id: "run:77".to_string(),
                label: "unit tests".to_string(),
                state: legion_protocol::VerificationRunState::Passed,
                command_class_label: "test".to_string(),
                command_body_redacted: true,
                exit_code: Some(0),
                target_labels: vec!["file:alpha".to_string()],
                evidence_artifact_id: Some("artifact:verification:77".to_string()),
                started_at: Some(TimestampMillis(4)),
                completed_at: Some(TimestampMillis(5)),
                risk_label: legion_protocol::ProposalRiskLabel::Low,
                privacy_label: legion_protocol::ProposalPrivacyLabel::WorkspaceMetadata,
                redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            omitted_row_count: 0,
            generated_at: TimestampMillis(6),
            redaction_hints: vec![legion_protocol::RedactionHint::MetadataOnly],
            schema_version: 1,
        };

        let cards =
            legion_workflow_fleet_card_projections(&proposal_projection, &verification_projection);

        assert_eq!(cards.len(), 1);
        let card = &cards[0];
        assert_eq!(card.proposal_id, legion_protocol::ProposalId(77));
        assert_eq!(card.owner_label, "owner:alice");
        assert_eq!(card.model_label, "model:gpt-5.5");
        assert_eq!(card.status_label, "Previewed");
        assert_eq!(card.progress_label, "targets=1/1 · hunks=2");
        assert_eq!(card.files_label, "manifest:77 · files=2 items");
        assert_eq!(card.risk_label, legion_protocol::ProposalRiskLabel::Medium);
        assert_eq!(
            card.test_status_label,
            "passed=1 failed=0 blocked=0 running=0 planned=0 cancelled=0"
        );
        assert_eq!(card.mini_diff_label, "text · targets=1 · hunks=2 · +5/-1");
        assert_eq!(card.last_activity_label, "updated_at=2");
    }
}
