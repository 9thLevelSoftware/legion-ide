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
    CancellationTokenId, CapabilityDecision, CapabilityDecisionId, CapabilityId, CausalityId,
    CorrelationId, EventSequence, FileFingerprint, LanguageId, LanguageServerId, LspRequestId,
    PrincipalId, ProposalAffectedTarget, ProposalContextManifestSummary, ProposalDiffSummary,
    ProposalDiffSummaryKind, ProposalId, ProposalLedgerProjection, ProposalLedgerRow,
    ProposalLifecycleState, ProposalLifecycleStateDisplay, ProposalPayloadKind, ProposalPrivacyLabel,
    ProposalRiskLabel, ProposalRollbackAvailability, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProposalTargetKind, RedactionHint, RemoteFilesystemOperation,
    RemoteFilesystemOperationKind, RemoteGuiProjection, RemoteLspDescriptor, RemoteProposalReviewGuiRow,
    RemoteOperationId, RemotePtyDescriptor, RemoteTransportEnvelope, RemoteTransportPayload,
    RemoteWorkspaceLifecycleState, RemoteWorkspaceSessionGuiRow, RemoteWorkspaceSessionId,
    TerminalSessionId, TimestampMillis, WorkspaceId,
};
use legion_remote::RemoteOperationDisposition;
use legion_ui::{CommandDispatchIntent, Shell};
use uuid::Uuid;

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

fn remote_envelope(
    session_id: RemoteWorkspaceSessionId,
    operation_id: RemoteOperationId,
    payload: RemoteTransportPayload,
) -> RemoteTransportEnvelope {
    RemoteTransportEnvelope {
        session_id,
        operation_id,
        correlation_id: CorrelationId(900 + operation_id.0 as u64),
        causality_id: CausalityId(Uuid::now_v7()),
        event_sequence: EventSequence(operation_id.0 as u64),
        principal_id: PrincipalId("desktop".to_string()),
        payload,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn remote_capability_decision(capability: &str) -> CapabilityDecision {
    CapabilityDecision {
        decision_id: CapabilityDecisionId(7001),
        granted: true,
        capability: CapabilityId(capability.to_string()),
        reason: None,
    }
}

/// Loopback/fake remote backend: drive connect, terminal (PTY) and LSP
/// descriptors, reconnect, offline, and a proposal-mediated write through the
/// production transport ingestion seam, then assert the GUI rows AND that the
/// local workspace disk and active buffer are never mutated.
#[test]
fn remote_workspace_gui_loopback_transport_projects_lifecycle_descriptors_and_proposal_mediation() {
    let (_workspace, mut runtime, target) = open_runtime();
    let session_id = RemoteWorkspaceSessionId(7101);

    runtime
        .enable_remote_development_runtime()
        .expect("remote runtime should enable through app policy");

    // 1) Connect through the production action path.
    let connected = runtime
        .handle_action(DesktopAction::ConnectRemoteWorkspace {
            session_id,
            authority_label: "edge:test".to_string(),
        })
        .expect("connect should dispatch through app authority");
    assert!(matches!(
        connected,
        DesktopWorkflowOutcome::RemoteUpdated {
            status: DesktopRemoteStatus::Connected,
            ..
        }
    ));

    // Capture the app-owned descriptor as the authoritative source for
    // round-tripping lifecycle envelopes.
    let base_descriptor = runtime
        .remote_session_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.session_id == session_id)
        .expect("connected session descriptor should be projected");

    // 2) Terminal descriptor event (PTY) flows through the production path.
    let pty_outcome = runtime
        .ingest_remote_transport_envelope(remote_envelope(
            session_id,
            RemoteOperationId(8001),
            RemoteTransportPayload::Pty(RemotePtyDescriptor {
                session_id,
                terminal_session_id: TerminalSessionId(42),
                columns: 120,
                rows: 30,
                transcript_byte_limit: 64 * 1024,
                capability_decision: remote_capability_decision("remote.pty.input"),
                schema_version: 1,
            }),
        ))
        .expect("pty descriptor should ingest through the production path");
    assert_eq!(pty_outcome.disposition, RemoteOperationDisposition::Accepted);

    // 3) LSP descriptor event flows through the production path.
    let lsp_outcome = runtime
        .ingest_remote_transport_envelope(remote_envelope(
            session_id,
            RemoteOperationId(8002),
            RemoteTransportPayload::Lsp(RemoteLspDescriptor {
                session_id,
                language_server_id: LanguageServerId(7),
                request_id: LspRequestId(Uuid::now_v7()),
                language_id: LanguageId("rust".to_string()),
                capability_decision: remote_capability_decision("remote.lsp.launch"),
                cancellation_token_id: CancellationTokenId(Uuid::now_v7()),
                schema_version: 1,
            }),
        ))
        .expect("lsp descriptor should ingest through the production path");
    assert_eq!(lsp_outcome.disposition, RemoteOperationDisposition::Accepted);

    // Connected session row exposes terminal and LSP descriptor availability.
    let connected_snapshot = runtime.projection_snapshot();
    assert!(
        connected_snapshot
            .remote_gui_projection
            .session_rows
            .iter()
            .any(|row| {
                row.session_id == session_id
                    && row.status_label == "connected"
                    && row.terminal_descriptor_status == "terminal descriptor available"
                    && row.lsp_descriptor_status == "lsp descriptor available"
            }),
        "connected row should advertise terminal and LSP descriptors"
    );

    // 4) Reconnect event: a session descriptor with a Reconnecting lifecycle
    // state flows through the production path and updates the GUI row.
    let mut reconnecting = base_descriptor.clone();
    reconnecting.state = RemoteWorkspaceLifecycleState::Reconnecting;
    let reconnect_outcome = runtime
        .ingest_remote_transport_envelope(remote_envelope(
            session_id,
            RemoteOperationId(8003),
            RemoteTransportPayload::Session(reconnecting),
        ))
        .expect("reconnect descriptor should ingest through the production path");
    assert_eq!(
        reconnect_outcome.disposition,
        RemoteOperationDisposition::Accepted
    );
    let reconnect_snapshot = runtime.projection_snapshot();
    assert_eq!(
        reconnect_snapshot
            .remote_gui_projection
            .reconnecting_session_count,
        1
    );
    assert!(
        reconnect_snapshot
            .remote_gui_projection
            .session_rows
            .iter()
            .any(|row| row.session_id == session_id
                && row.reconnecting
                && row.status_label == "reconnecting"),
        "reconnect row should surface the reconnecting lifecycle state"
    );

    // 5) Offline event: a session descriptor with an Offline lifecycle state
    // flows through the production path and updates the GUI row.
    let mut offline = base_descriptor.clone();
    offline.state = RemoteWorkspaceLifecycleState::Offline;
    let offline_outcome = runtime
        .ingest_remote_transport_envelope(remote_envelope(
            session_id,
            RemoteOperationId(8004),
            RemoteTransportPayload::Session(offline),
        ))
        .expect("offline descriptor should ingest through the production path");
    assert_eq!(
        offline_outcome.disposition,
        RemoteOperationDisposition::Accepted
    );
    let offline_snapshot = runtime.projection_snapshot();
    assert_eq!(
        offline_snapshot.remote_gui_projection.offline_session_count,
        1
    );
    assert!(
        offline_snapshot
            .remote_gui_projection
            .session_rows
            .iter()
            .any(|row| row.session_id == session_id
                && row.offline
                && row.status_label == "offline: Offline"),
        "offline row should surface the offline lifecycle state"
    );

    // 6) Remote proposal event: a filesystem write without a linked proposal is
    // denied by the proposal-mediation gate, proving mutations stay proposal
    // mediated through the production ingestion path.
    let unmediated_write = runtime
        .ingest_remote_transport_envelope(remote_envelope(
            session_id,
            RemoteOperationId(8005),
            RemoteTransportPayload::FilesystemOperation(RemoteFilesystemOperation {
                session_id,
                operation_id: RemoteOperationId(8005),
                kind: RemoteFilesystemOperationKind::Write,
                path: legion_protocol::CanonicalPath("/remote/workspace/remote.txt".to_string()),
                destination: None,
                write_preconditions: None,
                proposal_id: None,
                schema_version: 1,
            }),
        ))
        .expect("unmediated write should return a disposition through the production path");
    assert_eq!(
        unmediated_write.disposition,
        RemoteOperationDisposition::Denied,
        "remote write without a proposal must be denied by proposal mediation"
    );

    // Local workspace immutability: none of the remote transport traffic mutated
    // the local on-disk file or the active buffer.
    assert_eq!(
        fs::read_to_string(&target).expect("local file readable"),
        "seed",
        "remote transport traffic must not mutate local disk"
    );
    assert!(
        offline_snapshot
            .active_buffer_projection
            .small_buffer_text()
            .is_some_and(|text| text == "seed"),
        "active buffer must remain immutable under remote transport traffic"
    );
}
