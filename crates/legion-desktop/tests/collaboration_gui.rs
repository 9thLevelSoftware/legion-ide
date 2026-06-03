use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::{DesktopAction, DesktopBridgeError, DesktopBridgeOutput, DesktopCommandBridge},
    view::DesktopProjectionViewModel,
    workflow::{
        DesktopCollaborationStatus, DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome,
    },
};
use legion_protocol::{
    CapabilityId, CollaborationGuiProjection, CollaborationParticipantId,
    CollaborationPresenceProjection, CollaborationSessionGuiRow, CollaborationSessionId,
    CollaborationSessionState, CollaborationSharedProposalGuiRow, FileFingerprint, PrincipalId,
    ProposalContextManifestSummary, ProposalDiffSummary, ProposalDiffSummaryKind, ProposalId,
    ProposalLedgerProjection, ProposalLedgerRow, ProposalLifecycleState,
    ProposalLifecycleStateDisplay, ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel,
    ProposalRollbackAvailability, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProtocolTextRange, RedactionHint, TextCoordinate, TimestampMillis, WorkspaceId,
};
use legion_ui::{CommandDispatchIntent, Shell};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let temp_root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = temp_root.join(format!(
            "legion_desktop_collaboration_gui_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, content).expect("temp file should be written");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_desktop_collaboration_gui_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

fn range(start: u64, end: u64) -> ProtocolTextRange {
    ProtocolTextRange {
        start: coord(0, start as u32, start),
        end: coord(0, end as u32, end),
    }
}

fn proposal_row(proposal_id: ProposalId) -> ProposalLedgerRow {
    ProposalLedgerRow {
        proposal_id,
        workspace_id: Some(WorkspaceId(1)),
        title: "Shared collaboration edit".to_string(),
        payload_kind: ProposalPayloadKind::TextEdit,
        lifecycle: ProposalLifecycleStateDisplay {
            state: ProposalLifecycleState::Created,
            label: "created".to_string(),
            description: "Proposal lifecycle state is Created".to_string(),
        },
        principal: PrincipalId("collab-reviewer".to_string()),
        capability: CapabilityId("collaboration.proposal.approve".to_string()),
        created_at: TimestampMillis(1),
        updated_at: TimestampMillis(2),
        expires_at: None,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        rollback: ProposalRollbackAvailability::BestEffort,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: Vec::new(),
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        context_manifest: ProposalContextManifestSummary {
            manifest_id: "manifest:collab:review".to_string(),
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
            diff_hash: Some(FileFingerprint {
                algorithm: "test".to_string(),
                value: "hash:collab".to_string(),
            }),
            chunks: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        preview_warnings: Vec::new(),
        diagnostics: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn collaboration_snapshot() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Collaboration").projection_snapshot();
    snapshot.collaboration_gui_projection = CollaborationGuiProjection {
        runtime_enabled: true,
        presence_enabled: true,
        session_rows: vec![CollaborationSessionGuiRow {
            session_id: CollaborationSessionId(91),
            state: CollaborationSessionState::Reconnecting,
            participant_count: 2,
            presence_count: 1,
            reconnecting_participant_count: 1,
            operation_count: 3,
            acknowledgement_count: 2,
            causal_gap_count: 1,
            conflict_count: 1,
            offline: false,
            status_label: "conflict metadata visible: 1".to_string(),
        }],
        shared_proposal_rows: vec![CollaborationSharedProposalGuiRow {
            session_id: CollaborationSessionId(91),
            proposal_id: ProposalId(41),
            required_approver_count: 2,
            authorized_approver_count: 2,
            approval_count: 1,
            denial_count: 0,
            pending_count: 1,
            applied_operation_count: 1,
            stale: false,
            status_label: "shared proposal pending approvals: 1".to_string(),
        }],
        reconnecting_session_count: 1,
        conflict_session_count: 1,
        offline_session_count: 0,
        status_label: "collaboration conflicts visible: 1".to_string(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot.collaboration_presence_projections = vec![CollaborationPresenceProjection {
        session_id: CollaborationSessionId(91),
        participant_id: CollaborationParticipantId(1),
        cursor: Some(coord(0, 1, 1)),
        selections: vec![range(0, 1)],
        activity_label: Some("reconnecting".to_string()),
        reconnecting: true,
        schema_version: 1,
    }];
    snapshot.proposal_ledger_projection = ProposalLedgerProjection {
        rows: vec![proposal_row(ProposalId(41))],
        selected_proposal_id: Some(ProposalId(41)),
        omitted_row_count: 0,
        generated_at: TimestampMillis(3),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot
}

fn open_runtime() -> (TempWorkspace, DesktopRuntime) {
    let workspace = TempWorkspace::new();
    let target = workspace.write("collab.txt", "seed");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(target.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open temp workspace");
    (workspace, runtime)
}

#[test]
fn collaboration_gui_bridge_routes_actions_with_projection_validation() {
    let snapshot = collaboration_snapshot();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::JoinCollaborationSession {
                session_id: CollaborationSessionId(99),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::JoinCollaborationSession {
            session_id: CollaborationSessionId(99),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::LeaveCollaborationSession {
                session_id: CollaborationSessionId(91),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::LeaveCollaborationSession {
            session_id: CollaborationSessionId(91),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::PublishCollaborationPresence {
                session_id: CollaborationSessionId(91),
                participant_id: CollaborationParticipantId(1),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::PublishCollaborationPresence {
            session_id: CollaborationSessionId(91),
            participant_id: CollaborationParticipantId(1),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenSharedProposalReview {
                session_id: CollaborationSessionId(91),
                proposal_id: ProposalId(41),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenProposalDetails {
            proposal_id: ProposalId(41),
        })
    );

    assert_eq!(
        bridge.translate(
            DesktopAction::LeaveCollaborationSession {
                session_id: CollaborationSessionId(777),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownCollaborationSession {
            session_id: CollaborationSessionId(777),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::PublishCollaborationPresence {
                session_id: CollaborationSessionId(91),
                participant_id: CollaborationParticipantId(0),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidCollaborationParticipant)
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenSharedProposalReview {
                session_id: CollaborationSessionId(91),
                proposal_id: ProposalId(999),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownSharedCollaborationProposal {
            session_id: CollaborationSessionId(91),
            proposal_id: ProposalId(999),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::JoinCollaborationSession {
                session_id: CollaborationSessionId(91),
            },
            &Shell::empty("disabled").projection_snapshot(),
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::CollaborationRuntimeUnavailable)
    );
}

#[test]
fn collaboration_gui_rows_show_reconnect_conflict_and_shared_proposal_metadata() {
    let model = DesktopProjectionViewModel::from_snapshot(&collaboration_snapshot());

    assert!(model.collaboration_rows.iter().any(|row| {
        row.contains("collaboration: status=collaboration conflicts visible: 1")
            && row.contains("runtime_enabled=true")
            && row.contains("conflicts=1")
            && row.contains("redaction=metadata-only")
    }));
    assert!(model.collaboration_rows.iter().any(|row| {
        row.contains("collaboration session 91")
            && row.contains("Reconnecting")
            && row.contains("reconnecting=1")
            && row.contains("conflicts=1")
    }));
    assert!(model.collaboration_rows.iter().any(|row| {
        row.contains("shared proposal session 91 proposal 41")
            && row.contains("pending=1")
            && row.contains("proposal-mediated")
    }));
}

#[test]
fn collaboration_gui_workflow_reports_join_and_presence_outcomes() {
    let (_workspace, mut runtime) = open_runtime();

    let disabled = runtime
        .handle_action(DesktopAction::JoinCollaborationSession {
            session_id: CollaborationSessionId(91),
        })
        .expect("disabled collaboration runtime should return bridge outcome");
    assert!(matches!(disabled, DesktopWorkflowOutcome::Error(_)));

    runtime
        .enable_local_collaboration_runtime()
        .expect("collaboration runtime should enable through app policy");
    let joined = runtime
        .handle_action(DesktopAction::JoinCollaborationSession {
            session_id: CollaborationSessionId(91),
        })
        .expect("join should dispatch through app authority");
    assert!(matches!(
        joined,
        DesktopWorkflowOutcome::CollaborationUpdated {
            session_id: Some(CollaborationSessionId(91)),
            status: DesktopCollaborationStatus::Joined,
            ref message,
        } if message.contains("joined")
    ));

    let presence = runtime
        .handle_action(DesktopAction::PublishCollaborationPresence {
            session_id: CollaborationSessionId(91),
            participant_id: CollaborationParticipantId(1),
        })
        .expect("presence should dispatch through app authority");
    assert!(matches!(
        presence,
        DesktopWorkflowOutcome::CollaborationUpdated {
            session_id: Some(CollaborationSessionId(91)),
            status: DesktopCollaborationStatus::PresencePublished,
            ref message,
        } if message.contains("presence published")
    ));

    let snapshot = runtime.projection_snapshot();
    assert_eq!(snapshot.collaboration_gui_projection.session_rows.len(), 1);
    assert!(
        snapshot
            .collaboration_gui_projection
            .session_rows
            .iter()
            .any(|row| row.session_id == CollaborationSessionId(91))
    );
    assert!(
        snapshot
            .active_buffer_projection
            .file_path
            .as_ref()
            .is_some_and(|path| path.0.ends_with("collab.txt"))
    );
    assert!(
        snapshot
            .active_buffer_projection
            .small_buffer_text()
            .is_some_and(|text| text == "seed")
    );
}
