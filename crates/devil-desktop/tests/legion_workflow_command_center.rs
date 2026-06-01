use devil_desktop::{
    bridge::{
        DesktopAction, DesktopAppRequest, DesktopBridgeError, DesktopBridgeOutput,
        DesktopCommandBridge,
    },
    health::DesktopOperationalHealthSnapshot,
    view::DesktopProjectionViewModel,
};
use devil_protocol::{
    CapabilityId, FileFingerprint, LegionWorkflowMergeReadiness,
    LegionWorkflowMergeReadinessBlocker, LegionWorkflowMergeReadinessState,
    LegionWorkflowProjection, LegionWorkflowProjectionRow, LegionWorkflowSessionId,
    LegionWorkflowState, PrincipalId, ProposalAffectedTarget, ProposalContextManifestSummary,
    ProposalDiffSummary, ProposalDiffSummaryKind, ProposalId, ProposalLedgerProjection,
    ProposalLedgerRow, ProposalLifecycleState, ProposalLifecycleStateDisplay, ProposalPayloadKind,
    ProposalPrivacyLabel, ProposalRiskLabel, ProposalRollbackAvailability, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProposalTargetKind, RedactionHint, TimestampMillis, WorkspaceId,
};
use devil_ui::Shell;

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.to_string(),
    }
}

fn proposal_target() -> ProposalAffectedTarget {
    ProposalAffectedTarget {
        target_id: "legion:proposal".to_string(),
        kind: ProposalTargetKind::MetadataOnly,
        workspace_id: Some(WorkspaceId(1)),
        file_id: None,
        buffer_id: None,
        path: None,
        terminal_session_id: None,
        plugin_id: None,
        remote_authority: None,
        collaboration_session_id: None,
        byte_ranges: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
}

fn proposal_row(proposal_id: ProposalId) -> ProposalLedgerRow {
    ProposalLedgerRow {
        proposal_id,
        workspace_id: Some(WorkspaceId(1)),
        title: "Legion workflow proposal".to_string(),
        payload_kind: ProposalPayloadKind::WorkspaceEdit,
        lifecycle: ProposalLifecycleStateDisplay {
            state: ProposalLifecycleState::Created,
            label: "created".to_string(),
            description: "Proposal lifecycle state is Created".to_string(),
        },
        principal: PrincipalId("legion-reviewer".to_string()),
        capability: CapabilityId("legion.proposal.review".to_string()),
        created_at: TimestampMillis(1),
        updated_at: TimestampMillis(2),
        expires_at: None,
        risk_label: ProposalRiskLabel::Medium,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        rollback: ProposalRollbackAvailability::BestEffort,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![proposal_target()],
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        context_manifest: ProposalContextManifestSummary {
            manifest_id: "manifest:legion:review".to_string(),
            category_count: 1,
            total_item_count: 1,
            omitted_item_count: 0,
            categories: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        diff_summary: ProposalDiffSummary {
            kind: ProposalDiffSummaryKind::MetadataOnly,
            target_count: 1,
            hunk_count: 1,
            inserted_line_count: 1,
            deleted_line_count: 0,
            omitted_hunk_count: 0,
            full_source_redacted: true,
            diff_hash: Some(fingerprint("hash:legion-proposal")),
            chunks: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        preview_warnings: Vec::new(),
        diagnostics: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn readiness(state: LegionWorkflowMergeReadinessState) -> LegionWorkflowMergeReadiness {
    let blockers = match state {
        LegionWorkflowMergeReadinessState::Ready => Vec::new(),
        LegionWorkflowMergeReadinessState::WaitingForApproval => {
            vec![LegionWorkflowMergeReadinessBlocker::ApprovalRequired]
        }
        LegionWorkflowMergeReadinessState::Blocked => {
            vec![
                LegionWorkflowMergeReadinessBlocker::UnresolvedConflict,
                LegionWorkflowMergeReadinessBlocker::MissingVerificationEvidence,
                LegionWorkflowMergeReadinessBlocker::MissingSignOff,
            ]
        }
    };
    LegionWorkflowMergeReadiness {
        state,
        blockers,
        labels: vec![
            "legion_workflow.waiting_for_approval".to_string(),
            "verification:unit".to_string(),
            "signoff:reviewer".to_string(),
            "conflict:shared".to_string(),
        ],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn legion_projection(state: LegionWorkflowMergeReadinessState) -> LegionWorkflowProjection {
    let blocked = state == LegionWorkflowMergeReadinessState::Blocked;
    LegionWorkflowProjection {
        projection_id: "legion-workflow:test-command-center".to_string(),
        rows: vec![LegionWorkflowProjectionRow {
            session_id: LegionWorkflowSessionId("session:legion:alpha".to_string()),
            lifecycle_state: if blocked {
                LegionWorkflowState::Blocked
            } else {
                LegionWorkflowState::WaitingForApproval
            },
            worker_count: 4,
            provider_route_required_count: 1,
            dependency_count: 3,
            unresolved_conflict_count: u32::from(blocked),
            verification_gate_count: 2,
            passed_verification_count: if blocked { 1 } else { 2 },
            sign_off_count: 2,
            signed_off_count: if blocked { 1 } else { 2 },
            linked_proposals: vec![ProposalId(901)],
            merge_readiness: readiness(state),
            display_safe_labels: vec![
                "worker:coordinator".to_string(),
                "verification:unit".to_string(),
                "signoff:reviewer".to_string(),
                "conflict:shared".to_string(),
                "Autonomous merge unsupported until approval".to_string(),
            ],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        total_session_count: 1,
        omitted_row_count: 0,
        generated_at: TimestampMillis(10),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn legion_snapshot(state: LegionWorkflowMergeReadinessState) -> devil_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Legion").projection_snapshot();
    snapshot.legion_workflow_projection = legion_projection(state);
    snapshot.proposal_ledger_projection = ProposalLedgerProjection {
        rows: vec![proposal_row(ProposalId(901))],
        selected_proposal_id: Some(ProposalId(901)),
        omitted_row_count: 0,
        generated_at: TimestampMillis(11),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot
}

#[test]
fn legion_workflow_command_center_rows_show_sessions_gates_and_merge_state() {
    let model = DesktopProjectionViewModel::from_snapshot(&legion_snapshot(
        LegionWorkflowMergeReadinessState::Blocked,
    ));

    assert!(model.legion_workflow_rows.iter().any(|row| {
        row.contains("legion workflow command center")
            && row.contains("sessions=1")
            && row.contains("Autonomous merge unsupported until approval")
    }));
    assert!(model.legion_workflow_rows.iter().any(|row| {
        row.contains("workers=4")
            && row.contains("dependencies=3")
            && row.contains("conflicts=1")
            && row.contains("verification=1/2")
            && row.contains("signoff=1/2")
            && row.contains("merge=Blocked")
    }));
    assert!(model.product_mode_rows.iter().any(|row| {
        row.contains("Legion Workflow")
            && row.contains("Autonomous merge unsupported until approval")
    }));
}

#[test]
fn legion_workflow_bridge_routes_review_actions_and_denies_unknown_ids() {
    let snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::WaitingForApproval);
    let bridge = DesktopCommandBridge::new();
    let session_id = LegionWorkflowSessionId("session:legion:alpha".to_string());

    assert_eq!(
        bridge.translate(
            DesktopAction::InspectLegionWorkflowSession {
                session_id: session_id.clone()
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::InspectLegionWorkflowSession {
            session_id: session_id.clone()
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenLegionWorkflowProposalDetails {
                session_id: session_id.clone(),
                proposal_id: ProposalId(901),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::OpenLegionWorkflowProposalDetails {
            session_id: session_id.clone(),
            proposal_id: ProposalId(901),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::RequestLegionWorkflowVerification {
                session_id: session_id.clone(),
                gate_id: devil_protocol::LegionWorkflowVerificationGateId(
                    "verification:unit".to_string()
                ),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::RequestLegionWorkflowVerification {
            session_id: session_id.clone(),
            gate_id: devil_protocol::LegionWorkflowVerificationGateId(
                "verification:unit".to_string()
            ),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::RequestLegionWorkflowMergeReadiness {
                session_id: session_id.clone()
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::RequestLegionWorkflowMergeReadiness {
            session_id: session_id.clone()
        })
    );

    let missing_session = LegionWorkflowSessionId("session:missing".to_string());
    assert_eq!(
        bridge.translate(
            DesktopAction::InspectLegionWorkflowSession {
                session_id: missing_session.clone()
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowSession {
            session_id: missing_session
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenLegionWorkflowProposalPreview {
                session_id: session_id.clone(),
                proposal_id: ProposalId(999),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowProposal {
            session_id: session_id.clone(),
            proposal_id: ProposalId(999),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::ResolveLegionWorkflowConflict {
                session_id,
                conflict_id: devil_protocol::LegionWorkflowConflictId(
                    "conflict:unknown".to_string()
                ),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowConflict {
            session_id: LegionWorkflowSessionId("session:legion:alpha".to_string()),
            conflict_id: devil_protocol::LegionWorkflowConflictId("conflict:unknown".to_string()),
        })
    );
}

#[test]
fn legion_workflow_health_keeps_autonomous_merge_unsupported() {
    let health = DesktopOperationalHealthSnapshot::from_projection(&legion_snapshot(
        LegionWorkflowMergeReadinessState::WaitingForApproval,
    ));

    assert_eq!(health.legion_workflow_session_count, 1);
    assert_eq!(health.legion_workflow_waiting_for_approval_count, 1);
    assert!(health.rows().iter().any(|row| {
        row.contains("legion_workflows")
            && row.contains("sessions=1")
            && row.contains("waiting_for_approval=1")
    }));
    assert!(
        health
            .unsupported_surfaces
            .contains(&"Autonomous merge: unsupported until approval".to_string())
    );
}

#[test]
fn legion_workflow_ready_state_is_proposal_mediated_not_autonomous_apply() {
    let model = DesktopProjectionViewModel::from_snapshot(&legion_snapshot(
        LegionWorkflowMergeReadinessState::Ready,
    ));

    assert!(
        model
            .legion_workflow_rows
            .iter()
            .any(|row| row.contains("merge=Ready"))
    );
    assert!(
        model
            .legion_workflow_rows
            .iter()
            .any(|row| row.contains("proposal-mediated"))
    );
    assert!(
        !model
            .legion_workflow_rows
            .iter()
            .any(|row| { row.contains("autonomous merge action") || row.contains("direct apply") })
    );
}
