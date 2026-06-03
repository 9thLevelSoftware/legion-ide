use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::AppComposition;
use legion_desktop::{
    bridge::{
        DesktopAction, DesktopAppRequest, DesktopBridgeError, DesktopBridgeOutput,
        DesktopCommandBridge,
    },
    view::DesktopProjectionViewModel,
    workflow::{
        DesktopDelegatedTaskStatus, DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome,
    },
};
use legion_protocol::{
    AssistedAiTrustProjectionKind, AssistedAiTrustProjectionReference, ByteRange, CanonicalPath,
    CapabilityId, CausalityId, ContextManifestItemCount, CorrelationId,
    DelegatedTaskAffectedTargetSummary, DelegatedTaskChatMessage, DelegatedTaskChatRole,
    DelegatedTaskContextCitation, DelegatedTaskOperationClass, DelegatedTaskPlanContract,
    DelegatedTaskPlanId, DelegatedTaskPlanStep, DelegatedTaskPlanningBoundaryInput,
    DelegatedTaskProposalHunkDisposition, DelegatedTaskProposalHunkReview,
    DelegatedTaskProposalPreviewLink, DelegatedTaskProposalReview,
    DelegatedTaskRuntimeActivationState, DelegatedTaskStepId, DelegatedTaskStepState,
    DelegatedTaskToolPermissionDecision, DelegatedTaskToolPermissionProfile, FileFingerprint,
    LineIndexRange, PermissionBudgetActionClass, PrincipalId, ProposalAffectedTarget,
    ProposalContextManifestSummary, ProposalDiffSummary, ProposalDiffSummaryKind, ProposalId,
    ProposalLedgerProjection, ProposalLedgerRow, ProposalLifecycleState,
    ProposalLifecycleStateDisplay, ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel,
    ProposalRollbackAvailability, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProposalTargetKind, RedactionHint, TimestampMillis, WorkspaceId, WorkspaceTrustState,
    delegated_task_plan_from_boundary_input, delegated_task_projection_from_plan_contracts,
    delegated_task_tool_permission_request,
};
use legion_ui::{CommandDispatchIntent, DockMode, Shell};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlanFixtureMode {
    Approval,
    Blocked,
    Refused,
}

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
            "legion_desktop_delegated_task_command_center_{}_{}_{}",
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
            && file_name.is_some_and(|name| {
                name.starts_with("legion_desktop_delegated_task_command_center_")
            })
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "test".to_string(),
        value: value.to_string(),
    }
}

fn causality_id() -> CausalityId {
    serde_json::from_str("\"cccccccc-cccc-cccc-cccc-cccccccccccc\"")
        .expect("causality id should deserialize")
}

fn trust_ref(
    kind: AssistedAiTrustProjectionKind,
    label: &str,
) -> AssistedAiTrustProjectionReference {
    AssistedAiTrustProjectionReference {
        reference_id: format!("projection:{label}"),
        kind,
        projection_hash: fingerprint(&format!("hash:{label}")),
        schema_version: 1,
    }
}

fn preview_link(proposal_id: ProposalId) -> DelegatedTaskProposalPreviewLink {
    DelegatedTaskProposalPreviewLink {
        link_id: format!("delegated-preview:{}", proposal_id.0),
        proposal_id,
        payload_kind: ProposalPayloadKind::WorkspaceEdit,
        lifecycle_state: ProposalLifecycleState::Created,
        approval_checklist: Some(trust_ref(
            AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
            "approval",
        )),
        checkpoint_rollback: Some(trust_ref(
            AssistedAiTrustProjectionKind::CheckpointRollback,
            "checkpoint",
        )),
        target_count: 1,
        hunk_count: 1,
        full_source_redacted: true,
        risk_label: ProposalRiskLabel::Medium,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn target_summary(target_id: &str) -> DelegatedTaskAffectedTargetSummary {
    DelegatedTaskAffectedTargetSummary {
        target_id: target_id.to_string(),
        kind: ProposalTargetKind::MetadataOnly,
        workspace_id: Some(WorkspaceId(1)),
        file_id: None,
        buffer_id: None,
        ranges: Vec::new(),
        hashes: vec![fingerprint("hash:target")],
        counts: vec![ContextManifestItemCount {
            label: "targets".to_string(),
            count: 1,
        }],
        labels: vec!["delegated_task.target.metadata_only".to_string()],
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn plan_step(
    plan_id: &DelegatedTaskPlanId,
    proposal_preview: Option<DelegatedTaskProposalPreviewLink>,
) -> DelegatedTaskPlanStep {
    DelegatedTaskPlanStep {
        step_id: DelegatedTaskStepId(format!("step:{}:preview", plan_id.0)),
        order: 1,
        objective_summary_hash: fingerprint("hash:step"),
        operation_class: if proposal_preview.is_some() {
            DelegatedTaskOperationClass::LinkProposalPreview
        } else {
            DelegatedTaskOperationClass::SummarizeVerificationReadiness
        },
        depends_on: Vec::new(),
        required_gates: Vec::new(),
        target_ids: vec!["target:metadata".to_string()],
        proposal_preview,
        state: DelegatedTaskStepState::ProposalPreviewLinked,
        blockers: Vec::new(),
        labels: vec!["delegated_task.step.metadata_only".to_string()],
        counts: vec![ContextManifestItemCount {
            label: "steps".to_string(),
            count: 1,
        }],
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn plan_contract(
    plan_id: &str,
    proposal_id: Option<ProposalId>,
    mode: PlanFixtureMode,
) -> DelegatedTaskPlanContract {
    let plan_id = DelegatedTaskPlanId(plan_id.to_string());
    let preview = proposal_id.map(preview_link);
    delegated_task_plan_from_boundary_input(DelegatedTaskPlanningBoundaryInput {
        plan_id: plan_id.clone(),
        workspace_id: Some(WorkspaceId(1)),
        objective_summary_hash: fingerprint(&format!("hash:{}", plan_id.0)),
        allowed_operation_classes: vec![
            DelegatedTaskOperationClass::LinkProposalPreview,
            DelegatedTaskOperationClass::RequestHumanApproval,
            DelegatedTaskOperationClass::SummarizeVerificationReadiness,
        ],
        context_manifest: if mode == PlanFixtureMode::Blocked {
            None
        } else {
            Some(trust_ref(
                AssistedAiTrustProjectionKind::ContextManifest,
                "context",
            ))
        },
        privacy_inspector: Some(trust_ref(
            AssistedAiTrustProjectionKind::PrivacyInspector,
            "privacy",
        )),
        permission_budget_projection: Some(trust_ref(
            AssistedAiTrustProjectionKind::PermissionBudget,
            "budget",
        )),
        approval_checklist: Some(trust_ref(
            AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
            "approval",
        )),
        checkpoint_rollback: Some(trust_ref(
            AssistedAiTrustProjectionKind::CheckpointRollback,
            "checkpoint",
        )),
        assisted_ai_projection: Some(trust_ref(
            AssistedAiTrustProjectionKind::AssistedAiProjection,
            "assisted",
        )),
        assisted_ai_required: true,
        affected_targets: vec![target_summary("target:metadata")],
        steps: vec![plan_step(&plan_id, preview.clone())],
        proposal_preview_links: preview.into_iter().collect(),
        workspace_trust_state: if mode == PlanFixtureMode::Refused {
            WorkspaceTrustState::Untrusted
        } else {
            WorkspaceTrustState::Trusted
        },
        privacy_denied: false,
        permission_budget_denied: false,
        permission_budget_depleted: false,
        approval_checklist_valid: true,
        checkpoint_required: false,
        checkpoint_available: true,
        rollback_required: false,
        rollback_available: true,
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        created_at: TimestampMillis(4),
        schema_version: 1,
    })
}

fn proposal_target() -> ProposalAffectedTarget {
    ProposalAffectedTarget {
        target_id: "delegated:proposal".to_string(),
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
        title: "Delegated task proposal".to_string(),
        payload_kind: ProposalPayloadKind::WorkspaceEdit,
        lifecycle: ProposalLifecycleStateDisplay {
            state: ProposalLifecycleState::Created,
            label: "created".to_string(),
            description: "Proposal lifecycle state is Created".to_string(),
        },
        principal: PrincipalId("delegated-reviewer".to_string()),
        capability: CapabilityId("delegated.proposal.review".to_string()),
        created_at: TimestampMillis(1),
        updated_at: TimestampMillis(2),
        expires_at: None,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        rollback: ProposalRollbackAvailability::BestEffort,
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: vec![proposal_target()],
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        context_manifest: ProposalContextManifestSummary {
            manifest_id: "manifest:delegated:review".to_string(),
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
            diff_hash: Some(fingerprint("hash:delegated-proposal")),
            chunks: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        preview_warnings: Vec::new(),
        diagnostics: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn delegated_snapshot() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Delegated").projection_snapshot();
    snapshot.product_mode = DockMode::Delegate;
    snapshot.delegated_task_projection = delegated_task_projection_from_plan_contracts(
        "delegated-task:test-command-center",
        vec![
            plan_contract(
                "plan:approval",
                Some(ProposalId(701)),
                PlanFixtureMode::Approval,
            ),
            plan_contract("plan:blocked", None, PlanFixtureMode::Blocked),
            plan_contract("plan:refused", None, PlanFixtureMode::Refused),
        ],
        TimestampMillis(10),
        1,
    );
    snapshot.proposal_ledger_projection = ProposalLedgerProjection {
        rows: vec![proposal_row(ProposalId(701))],
        selected_proposal_id: Some(ProposalId(701)),
        omitted_row_count: 0,
        generated_at: TimestampMillis(11),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot
}

fn delegated_snapshot_with_delegate_controls() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = delegated_snapshot();
    snapshot
        .delegated_task_projection
        .chat_messages
        .push(DelegatedTaskChatMessage {
            message_id: "delegate:chat:1".to_string(),
            role: DelegatedTaskChatRole::Assistant,
            content_label: "Delegate cited the proposal hunk".to_string(),
            plan_id: None,
            proposal_id: Some(ProposalId(701)),
            citation_ids: vec!["delegate:citation:1".to_string()],
            tool_permission_request_ids: vec!["delegate:permission:1".to_string()],
            correlation_id: CorrelationId(902),
            causality_id: causality_id(),
            created_at: TimestampMillis(12),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        });
    snapshot
        .delegated_task_projection
        .context_citations
        .push(DelegatedTaskContextCitation {
            citation_id: "delegate:citation:1".to_string(),
            workspace_id: Some(WorkspaceId(1)),
            file_id: None,
            path: Some(CanonicalPath("src/lib.rs".to_string())),
            byte_range: Some(ByteRange::new(0, 12)),
            line_range: Some(LineIndexRange { start: 0, end: 1 }),
            freshness_fingerprint: Some(fingerprint("fresh")),
            chunk_hash: Some(fingerprint("chunk")),
            score_basis_points: 9000,
            metadata_label: "src/lib.rs chunk".to_string(),
            labels: vec!["delegate.context.retrieval_citation".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        });
    let hunk = DelegatedTaskProposalHunkReview {
        hunk_id: "delegate:proposal:701:metadata-chunk:0".to_string(),
        proposal_id: ProposalId(701),
        target_id: Some("delegated:proposal".to_string()),
        payload_kind: ProposalPayloadKind::WorkspaceEdit,
        path: Some(CanonicalPath("src/lib.rs".to_string())),
        byte_range: Some(ByteRange::new(0, 12)),
        changed_line_count: 1,
        inserted_line_count: 1,
        deleted_line_count: 0,
        content_hash: Some(fingerprint("hunk")),
        disposition: DelegatedTaskProposalHunkDisposition::Pending,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        labels: vec!["delegate.proposal_hunk.human_review".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot.delegated_task_projection.proposal_reviews.push(
        DelegatedTaskProposalReview::from_hunks(
            "delegate:review:701",
            ProposalId(701),
            vec![hunk],
            vec!["delegate.proposal_review.human_approval_queue".to_string()],
            1,
        ),
    );
    snapshot
        .delegated_task_projection
        .tool_permission_requests
        .push(delegated_task_tool_permission_request(
            legion_protocol::DelegatedTaskToolPermissionRequestInput {
                request_id: "delegate:permission:1".to_string(),
                profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::AccessWorkspaceFiles,
                capability: Some(CapabilityId("delegated.runtime.allocate".to_string())),
                target_id: Some("plan:approval".to_string()),
                decision: DelegatedTaskToolPermissionDecision::Confirm,
                labels: vec!["delegate.permission.write.runtime_allocation".to_string()],
                schema_version: 1,
            },
        ));
    snapshot.delegated_task_projection.chat_message_count =
        snapshot.delegated_task_projection.chat_messages.len() as u32;
    snapshot.delegated_task_projection.context_citation_count =
        snapshot.delegated_task_projection.context_citations.len() as u32;
    snapshot.delegated_task_projection.proposal_review_count =
        snapshot.delegated_task_projection.proposal_reviews.len() as u32;
    snapshot
        .delegated_task_projection
        .tool_permission_request_count = snapshot
        .delegated_task_projection
        .tool_permission_requests
        .len() as u32;
    snapshot
}

fn open_runtime() -> (TempWorkspace, DesktopRuntime, PathBuf) {
    let workspace = TempWorkspace::new();
    let target = workspace.write("delegated.txt", "seed");
    let runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace.path().to_path_buf(),
        Some(target.to_string_lossy().into_owned()),
    ))
    .expect("desktop runtime should open temp workspace");
    (workspace, runtime, target)
}

#[test]
fn delegated_task_command_center_rows_show_gates_blockers_refusals_and_audit() {
    let model = DesktopProjectionViewModel::from_snapshot(&delegated_snapshot());

    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegated task command center")
            && row.contains("plans=3")
            && row.contains("blocked=1")
            && row.contains("refused=1")
            && row.contains("runtime=NotEncoded")
            && row.contains("autonomous_apply=unsupported")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegated task plan plan:approval")
            && row.contains("AwaitingApproval")
            && row.contains("proposal_previews=1")
            && row.contains("runtime=NotEncoded")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegated task trust gate ApprovalChecklist")
            && row.contains("required=true")
            && row.contains("satisfied=true")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegated task blocker context_manifest.missing")
            && row.contains("Context manifest projection reference is required")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegated task refusal workspace.untrusted")
            && row.contains("Workspace trust is required")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegated task proposal preview delegated-preview:701")
            && row.contains("proposal=701")
            && row.contains("proposal-mediated")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegated task audit readiness delegated-task:readiness:plan:approval")
            && row.contains("runtime=NotEncoded")
    }));
}

#[test]
fn delegated_task_command_center_bridge_routes_review_actions_and_denies_unknown_links() {
    let snapshot = delegated_snapshot();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::InspectDelegatedTaskPlan {
                plan_id: DelegatedTaskPlanId("plan:approval".to_string()),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::InspectDelegatedTaskPlan {
            plan_id: DelegatedTaskPlanId("plan:approval".to_string()),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenDelegatedProposalPreview {
                proposal_id: ProposalId(701),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::OpenDelegatedProposalPreview {
            proposal_id: ProposalId(701),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenDelegatedProposalDetails {
                proposal_id: ProposalId(701),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::OpenDelegatedProposalDetails {
            proposal_id: ProposalId(701),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::InspectDelegatedTaskPlan {
                plan_id: DelegatedTaskPlanId("plan:missing".to_string()),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownDelegatedTaskPlan {
            plan_id: DelegatedTaskPlanId("plan:missing".to_string()),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenDelegatedProposalPreview {
                proposal_id: ProposalId(999),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownDelegatedProposalPreview {
            proposal_id: ProposalId(999),
        })
    );

    let mut missing_ledger = snapshot;
    missing_ledger.proposal_ledger_projection.rows.clear();
    assert_eq!(
        bridge.translate(
            DesktopAction::OpenDelegatedProposalPreview {
                proposal_id: ProposalId(701),
            },
            &missing_ledger,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownProposal {
            proposal_id: ProposalId(701),
        })
    );
}

#[test]
fn delegated_task_command_center_bridge_routes_hunks_permissions_and_denies_unknown_rows() {
    let snapshot = delegated_snapshot_with_delegate_controls();
    let bridge = DesktopCommandBridge::new();
    let hunk_id = "delegate:proposal:701:metadata-chunk:0".to_string();

    assert_eq!(
        bridge.translate(
            DesktopAction::SendDelegateChat {
                prompt_label: "review context".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SendDelegateChat {
            prompt_label: "review context".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::ReviewDelegateProposalHunk {
                proposal_id: ProposalId(701),
                hunk_id: hunk_id.clone(),
                disposition: DelegatedTaskProposalHunkDisposition::Accepted,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ReviewDelegateProposalHunk {
            proposal_id: ProposalId(701),
            hunk_id: hunk_id.clone(),
            disposition: DelegatedTaskProposalHunkDisposition::Accepted,
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::RecordDelegateToolPermission {
                request_id: "delegate:permission:1".to_string(),
                decision: DelegatedTaskToolPermissionDecision::Allow,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RecordDelegateToolPermission {
            request_id: "delegate:permission:1".to_string(),
            decision: DelegatedTaskToolPermissionDecision::Allow,
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::ReviewDelegateProposalHunk {
                proposal_id: ProposalId(701),
                hunk_id: String::new(),
                disposition: DelegatedTaskProposalHunkDisposition::Accepted,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidDelegatedProposalHunk)
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::ReviewDelegateProposalHunk {
                proposal_id: ProposalId(701),
                hunk_id: "missing".to_string(),
                disposition: DelegatedTaskProposalHunkDisposition::Accepted,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownDelegatedProposalHunk {
            proposal_id: ProposalId(701),
            hunk_id: "missing".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::RecordDelegateToolPermission {
                request_id: String::new(),
                decision: DelegatedTaskToolPermissionDecision::Allow,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidDelegatedToolPermissionRequest)
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::RecordDelegateToolPermission {
                request_id: "missing".to_string(),
                decision: DelegatedTaskToolPermissionDecision::Allow,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownDelegatedToolPermissionRequest {
            request_id: "missing".to_string(),
        })
    );
}

#[test]
fn delegated_task_command_center_rows_show_chat_citations_hunks_and_permissions() {
    let model =
        DesktopProjectionViewModel::from_snapshot(&delegated_snapshot_with_delegate_controls());

    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegated task command center")
            && row.contains("chat=1")
            && row.contains("citations=1")
            && row.contains("reviews=1")
            && row.contains("permissions=1")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegate chat delegate:chat:1")
            && row.contains("citations=1")
            && row.contains("permissions=1")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegate citation delegate:citation:1")
            && row.contains("src/lib.rs")
            && row.contains("score=9000")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegate proposal review delegate:review:701")
            && row.contains("pending=1")
            && row.contains("ready=false")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegate proposal hunk")
            && row.contains("proposal=701")
            && row.contains("disposition=Pending")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegate tool permission delegate:permission:1")
            && row.contains("decision=Confirm")
            && row.contains("runtime_allowed=false")
    }));
}

#[test]
fn delegated_task_command_center_app_projection_and_workflow_remain_plan_only() {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    app.open_workspace(
        workspace.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("delegated-app".to_string()),
    )
    .expect("workspace should open");
    app.seed_delegated_task_plan_contracts(vec![plan_contract(
        "plan:app",
        Some(ProposalId(701)),
        PlanFixtureMode::Approval,
    )]);
    let snapshot = app
        .shell_projection_snapshot("delegated-app")
        .expect("app projection should build");
    assert_eq!(snapshot.delegated_task_projection.plan_count, 1);
    assert_eq!(
        snapshot.delegated_task_projection.runtime_activation,
        DelegatedTaskRuntimeActivationState::NotEncoded
    );
    assert!(
        snapshot
            .delegated_task_projection
            .plan_only_disclaimers
            .iter()
            .any(|label| label.contains("no_runtime"))
    );

    let (_workspace, mut runtime, target) = open_runtime();
    runtime
        .seed_delegated_task_plan_contracts(vec![plan_contract(
            "plan:workflow",
            None,
            PlanFixtureMode::Approval,
        )])
        .expect("delegated contracts should seed through app-owned projection");
    let inspected = runtime
        .handle_action(DesktopAction::InspectDelegatedTaskPlan {
            plan_id: DelegatedTaskPlanId("plan:workflow".to_string()),
        })
        .expect("inspect should stay metadata-only");
    assert!(matches!(
        inspected,
        DesktopWorkflowOutcome::DelegatedTaskReviewed {
            plan_id: Some(DelegatedTaskPlanId(ref id)),
            proposal_id: None,
            status: DesktopDelegatedTaskStatus::PlanInspected,
            ref message,
        } if id == "plan:workflow"
            && message.contains("approval-gated")
            && message.contains("autonomous apply unsupported")
    ));
    assert_eq!(
        fs::read_to_string(target).expect("local file readable"),
        "seed",
        "delegated command-center inspection must not mutate local disk"
    );
}
