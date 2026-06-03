use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    bridge::{
        DesktopAction, DesktopAppRequest, DesktopBridgeError, DesktopBridgeOutput,
        DesktopCommandBridge,
    },
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRemoteStatus, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{
    CapabilityId, FileFingerprint, PrincipalId, ProposalAffectedTarget,
    ProposalContextManifestSummary, ProposalDiffSummary, ProposalDiffSummaryKind, ProposalId,
    ProposalLedgerProjection, ProposalLedgerRow, ProposalLifecycleState,
    ProposalLifecycleStateDisplay, ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel,
    ProposalRollbackAvailability, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProposalTargetKind, RedactionHint, RemoteGuiProjection, RemoteProposalReviewGuiRow,
    RemoteWorkspaceLifecycleState, RemoteWorkspaceSessionGuiRow, RemoteWorkspaceSessionId,
    TimestampMillis, WorkspaceId,
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
            "legion_desktop_remote_workspace_gui_{}_{}_{}",
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
            && file_name
                .is_some_and(|name| name.starts_with("legion_desktop_remote_workspace_gui_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn remote_target(authority_label: &str) -> ProposalAffectedTarget {
    ProposalAffectedTarget {
        target_id: format!("remote:{authority_label}"),
        kind: ProposalTargetKind::RemoteWorkspace,
        workspace_id: Some(WorkspaceId(1)),
        file_id: None,
        buffer_id: None,
        path: None,
        terminal_session_id: None,
        plugin_id: None,
        remote_authority: Some(authority_label.to_string()),
        collaboration_session_id: None,
        byte_ranges: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
}

fn proposal_row(proposal_id: ProposalId) -> ProposalLedgerRow {
    ProposalLedgerRow {
        proposal_id,
        workspace_id: Some(WorkspaceId(1)),
        title: "Remote workspace proposal".to_string(),
        payload_kind: ProposalPayloadKind::WorkspaceEdit,
        lifecycle: ProposalLifecycleStateDisplay {
            state: ProposalLifecycleState::Created,
            label: "created".to_string(),
            description: "Proposal lifecycle state is Created".to_string(),
        },
        principal: PrincipalId("remote-reviewer".to_string()),
        capability: CapabilityId("remote.workspace.proposal.review".to_string()),
        created_at: TimestampMillis(1),
        updated_at: TimestampMillis(2),
        expires_at: None,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        rollback: ProposalRollbackAvailability::BestEffort,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![remote_target("edge:test")],
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        context_manifest: ProposalContextManifestSummary {
            manifest_id: "manifest:remote:review".to_string(),
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
                value: "hash:remote".to_string(),
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

fn remote_snapshot() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Remote Workspace").projection_snapshot();
    snapshot.remote_gui_projection = RemoteGuiProjection {
        runtime_enabled: true,
        session_rows: vec![
            RemoteWorkspaceSessionGuiRow {
                session_id: RemoteWorkspaceSessionId(7002),
                authority_label: "edge:test".to_string(),
                agent_version: "legion-remote-test/1".to_string(),
                state: RemoteWorkspaceLifecycleState::Reconnecting,
                filesystem_descriptor_status: "read/write proposal-mediated".to_string(),
                terminal_descriptor_status: "terminal descriptor available".to_string(),
                lsp_descriptor_status: "lsp descriptor available".to_string(),
                reconnect_supported: true,
                reconnecting: true,
                offline: false,
                proposal_review_count: 1,
                status_label: "reconnecting".to_string(),
            },
            RemoteWorkspaceSessionGuiRow {
                session_id: RemoteWorkspaceSessionId(7003),
                authority_label: "edge:offline".to_string(),
                agent_version: "legion-remote-test/1".to_string(),
                state: RemoteWorkspaceLifecycleState::Offline,
                filesystem_descriptor_status: "read-only".to_string(),
                terminal_descriptor_status: "unavailable".to_string(),
                lsp_descriptor_status: "unavailable".to_string(),
                reconnect_supported: true,
                reconnecting: false,
                offline: true,
                proposal_review_count: 0,
                status_label: "offline: Offline".to_string(),
            },
        ],
        proposal_review_rows: vec![RemoteProposalReviewGuiRow {
            session_id: RemoteWorkspaceSessionId(7002),
            proposal_id: ProposalId(700),
            remote_authority_label: "edge:test".to_string(),
            payload_kind: ProposalPayloadKind::WorkspaceEdit,
            lifecycle_state: ProposalLifecycleState::Created,
            status_label: "remote proposal Created via app proposal lifecycle".to_string(),
            proposal_mediated: true,
        }],
        connected_session_count: 0,
        reconnecting_session_count: 1,
        offline_session_count: 1,
        status_label: "remote workspace reconnecting sessions: 1".to_string(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot.proposal_ledger_projection = ProposalLedgerProjection {
        rows: vec![proposal_row(ProposalId(700))],
        selected_proposal_id: Some(ProposalId(700)),
        omitted_row_count: 0,
        generated_at: TimestampMillis(3),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot
}

fn open_runtime() -> (TempWorkspace, DesktopRuntime, PathBuf) {
    let workspace = TempWorkspace::new();
    let target = workspace.write("remote.txt", "seed");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(target.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open temp workspace");
    (workspace, runtime, target)
}

#[test]
fn remote_workspace_gui_bridge_routes_actions_with_projection_validation() {
    let snapshot = remote_snapshot();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::ConnectRemoteWorkspace {
                session_id: RemoteWorkspaceSessionId(8001),
                authority_label: " edge:new ".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::ConnectRemoteWorkspace {
            session_id: RemoteWorkspaceSessionId(8001),
            authority_label: "edge:new".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenRemoteProposalReview {
                session_id: RemoteWorkspaceSessionId(7002),
                proposal_id: ProposalId(700),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenProposalDetails {
            proposal_id: ProposalId(700),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenRemoteProposalReview {
                session_id: RemoteWorkspaceSessionId(9999),
                proposal_id: ProposalId(700),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownRemoteWorkspaceSession {
            session_id: RemoteWorkspaceSessionId(9999),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenRemoteProposalReview {
                session_id: RemoteWorkspaceSessionId(7002),
                proposal_id: ProposalId(999),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownRemoteProposal {
            session_id: RemoteWorkspaceSessionId(7002),
            proposal_id: ProposalId(999),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::ConnectRemoteWorkspace {
                session_id: RemoteWorkspaceSessionId(7002),
                authority_label: "edge:test".to_string(),
            },
            &Shell::empty("disabled").projection_snapshot(),
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::RemoteRuntimeUnavailable)
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::ConnectRemoteWorkspace {
                session_id: RemoteWorkspaceSessionId(0),
                authority_label: "edge:test".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidRemoteWorkspaceSession)
    );
}

#[test]
fn remote_workspace_gui_rows_show_reconnect_offline_terminal_lsp_and_proposals() {
    let model = DesktopProjectionViewModel::from_snapshot(&remote_snapshot());

    assert!(model.remote_rows.iter().any(|row| {
        row.contains("remote workspace: status=remote workspace reconnecting sessions: 1")
            && row.contains("runtime_enabled=true")
            && row.contains("reconnecting=1")
            && row.contains("offline=1")
            && row.contains("redaction=metadata-only")
    }));
    assert!(model.remote_rows.iter().any(|row| {
        row.contains("remote workspace session 7002")
            && row.contains("Reconnecting")
            && row.contains("terminal descriptor available")
            && row.contains("lsp descriptor available")
            && row.contains("proposal_reviews=1")
    }));
    assert!(model.remote_rows.iter().any(|row| {
        row.contains("remote workspace session 7003")
            && row.contains("Offline")
            && row.contains("offline=true")
    }));
    assert!(model.remote_rows.iter().any(|row| {
        row.contains("remote proposal session 7002 proposal 700")
            && row.contains("authority=edge:test")
            && row.contains("proposal-mediated=true")
    }));
}

#[test]
fn remote_workspace_gui_workflow_reports_connect_without_local_mutation() {
    let (_workspace, mut runtime, target) = open_runtime();

    let disabled = runtime
        .handle_action(DesktopAction::ConnectRemoteWorkspace {
            session_id: RemoteWorkspaceSessionId(7001),
            authority_label: "edge:test".to_string(),
        })
        .expect("disabled remote runtime should return bridge outcome");
    assert!(matches!(disabled, DesktopWorkflowOutcome::Error(_)));

    runtime
        .enable_remote_development_runtime()
        .expect("remote runtime should enable through app policy");
    let connected = runtime
        .handle_action(DesktopAction::ConnectRemoteWorkspace {
            session_id: RemoteWorkspaceSessionId(7001),
            authority_label: "edge:test".to_string(),
        })
        .expect("connect should dispatch through app authority");
    assert!(matches!(
        connected,
        DesktopWorkflowOutcome::RemoteUpdated {
            session_id: RemoteWorkspaceSessionId(7001),
            status: DesktopRemoteStatus::Connected,
            ref message,
        } if message.contains("Remote workspace connected")
    ));

    let snapshot = runtime.projection_snapshot();
    assert_eq!(snapshot.remote_gui_projection.session_rows.len(), 1);
    assert!(
        snapshot
            .remote_gui_projection
            .session_rows
            .iter()
            .any(|row| {
                row.session_id == RemoteWorkspaceSessionId(7001)
                    && row.authority_label == "edge:test"
                    && row.status_label == "connected"
                    && row.terminal_descriptor_status == "terminal descriptor available"
                    && row.lsp_descriptor_status == "lsp descriptor available"
            })
    );
    assert_eq!(
        fs::read_to_string(target).expect("local file readable"),
        "seed",
        "remote GUI connect must not mutate local disk"
    );
    assert!(
        snapshot
            .active_buffer_projection
            .small_buffer_text()
            .is_some_and(|text| text == "seed")
    );
}
