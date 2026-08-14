use legion_agent::comm::{AgentCommTag, format_agent_comm_line};
use legion_desktop::{
    bridge::{
        DesktopAction, DesktopAppRequest, DesktopBridgeError, DesktopBridgeOutput,
        DesktopCommandBridge,
    },
    health::DesktopOperationalHealthSnapshot,
    view::{DesktopProjectionViewModel, ProjectionView, agent_comm, fleet_board, fleet_card},
};
use legion_protocol::{
    CapabilityId, CausalityId, CorrelationId, DelegatedTaskRuntimeActivationState,
    DelegatedTaskToolPermissionDecision, DelegatedTaskToolPermissionProfile, EventSequence,
    FileFingerprint, LegionWorkflowDecisionFeedEntry, LegionWorkflowDecisionId,
    LegionWorkflowDecisionKind, LegionWorkflowKillSwitch, LegionWorkflowKillSwitchId,
    LegionWorkflowKillSwitchState, LegionWorkflowMergeReadiness,
    LegionWorkflowMergeReadinessBlocker, LegionWorkflowMergeReadinessState,
    LegionWorkflowProjection, LegionWorkflowProjectionRow, LegionWorkflowRiskHaltReason,
    LegionWorkflowRiskMonitorId, LegionWorkflowRiskMonitorSnapshot, LegionWorkflowRiskMonitorState,
    LegionWorkflowSessionId, LegionWorkflowState, McpPrimitiveKind, McpRegistrySnapshot,
    McpServerDescriptor, McpServerId, McpToolDescriptor, McpToolName, McpTransportKind,
    PermissionBudgetActionClass, PrincipalId, ProposalAffectedTarget, ProposalCancellationReason,
    ProposalContextManifestSummary, ProposalDiffSummary, ProposalDiffSummaryKind, ProposalId,
    ProposalLedgerProjection, ProposalLedgerRow, ProposalLifecycleState,
    ProposalLifecycleStateDisplay, ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel,
    ProposalRollbackAvailability, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProposalTargetKind, RedactionHint, TimestampMillis, WorkspaceId,
    delegated_task_tool_permission_request,
};
use legion_ui::{
    DockMode, LegionWorkflowBoardColumnKind, LegionWorkflowBudgetUsageRowProjection, Shell,
    legion_workflow_board_columns, legion_workflow_fleet_card_projections,
};

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.to_string(),
    }
}

fn causality(value: &str) -> CausalityId {
    serde_json::from_value(serde_json::Value::String(value.to_string()))
        .expect("fixture causality id must be a valid uuid")
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
            directive_artifact_id: Some("artifact:directive:legion:alpha".to_string()),
            spec_artifact_id: Some("artifact:spec:legion:alpha".to_string()),
            task_graph_artifact_id: Some("artifact:task-graph:legion:alpha".to_string()),
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
                "Legion Workflows merge unsupported until approval".to_string(),
            ],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        mcp_registries: Vec::new(),
        decision_feed: Vec::new(),
        risk_monitors: Vec::new(),
        kill_switches: Vec::new(),
        tool_permission_requests: Vec::new(),
        total_session_count: 1,
        mcp_registry_count: 0,
        decision_feed_count: 0,
        risk_monitor_count: 0,
        kill_switch_count: 0,
        tool_permission_request_count: 0,
        omitted_row_count: 0,
        generated_at: TimestampMillis(10),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn add_automate_sidecars(projection: &mut LegionWorkflowProjection) {
    let session_id = LegionWorkflowSessionId("session:legion:alpha".to_string());
    let server_id = McpServerId("mcp:test".to_string());
    let tool_name = McpToolName("write_file".to_string());
    projection.mcp_registries = vec![McpRegistrySnapshot {
        registry_id: "mcp-registry:test:1".to_string(),
        server: McpServerDescriptor {
            server_id: server_id.clone(),
            transport_kind: McpTransportKind::StreamableHttp,
            display_label: "Test MCP".to_string(),
            endpoint_label: "https://mcp.invalid".to_string(),
            tools_list_changed: true,
            resources_list_changed: true,
            prompts_list_changed: true,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        tools: vec![McpToolDescriptor {
            server_id: server_id.clone(),
            name: tool_name.clone(),
            description_label: "write file".to_string(),
            input_schema_hash: fingerprint("mcp-schema"),
            risk_label: ProposalRiskLabel::High,
            required_permission_profile: DelegatedTaskToolPermissionProfile::Write,
            action_class: PermissionBudgetActionClass::InvokeLocalTool,
            capability: CapabilityId("mcp.tool.call".to_string()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        resources: Vec::new(),
        prompts: Vec::new(),
        last_notification_kind: None,
        list_version: 1,
        generated_at: TimestampMillis(50),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];
    projection.decision_feed = vec![LegionWorkflowDecisionFeedEntry {
        decision_id: LegionWorkflowDecisionId("decision:test".to_string()),
        session_id: session_id.clone(),
        worker_id: None,
        kind: LegionWorkflowDecisionKind::ToolApprovalRequested,
        summary_label: "MCP tool call waiting for permission".to_string(),
        rationale_labels: vec!["human_in_the_loop".to_string()],
        risk_label: ProposalRiskLabel::High,
        mcp_server_id: Some(server_id.clone()),
        mcp_primitive_kind: Some(McpPrimitiveKind::Tool),
        tool_permission_request_id: Some("automate:permission:test".to_string()),
        correlation_id: CorrelationId(1),
        causality_id: causality("00000000-0000-0000-0000-000000000001"),
        event_sequence: EventSequence(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];
    projection.risk_monitors = vec![LegionWorkflowRiskMonitorSnapshot {
        monitor_id: LegionWorkflowRiskMonitorId("risk:test".to_string()),
        session_id: session_id.clone(),
        state: LegionWorkflowRiskMonitorState::Halted,
        risk_score: 3,
        halt_threshold: 3,
        high_risk_action_count: 3,
        denied_tool_count: 0,
        stale_mcp_registry_detected: false,
        halt_reason: Some(LegionWorkflowRiskHaltReason::HighRiskToolThreshold),
        labels: vec!["risk.high".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];
    projection.kill_switches = vec![LegionWorkflowKillSwitch {
        kill_switch_id: LegionWorkflowKillSwitchId("kill:test".to_string()),
        session_id: session_id.clone(),
        state: LegionWorkflowKillSwitchState::Armed,
        triggered_by: None,
        reason_label: None,
        triggered_at: None,
        correlation_id: CorrelationId(2),
        causality_id: causality("00000000-0000-0000-0000-000000000002"),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];
    projection.tool_permission_requests = vec![delegated_task_tool_permission_request(
        legion_protocol::DelegatedTaskToolPermissionRequestInput {
            request_id: "automate:permission:test".to_string(),
            profile: DelegatedTaskToolPermissionProfile::Write,
            action_class: PermissionBudgetActionClass::InvokeLocalTool,
            capability: Some(CapabilityId("mcp.tool.call".to_string())),
            target_id: Some("mcp-tool:mcp:test|write_file".to_string()),
            decision: DelegatedTaskToolPermissionDecision::Confirm,
            labels: vec![
                "automate.permission.mcp_tool_call".to_string(),
                "legion.session:session:legion:alpha".to_string(),
            ],
            schema_version: 1,
        },
    )];
    projection.mcp_registry_count = projection.mcp_registries.len() as u32;
    projection.decision_feed_count = projection.decision_feed.len() as u32;
    projection.risk_monitor_count = projection.risk_monitors.len() as u32;
    projection.kill_switch_count = projection.kill_switches.len() as u32;
    projection.tool_permission_request_count = projection.tool_permission_requests.len() as u32;
}

fn legion_snapshot(state: LegionWorkflowMergeReadinessState) -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Legion").projection_snapshot();
    snapshot.product_mode = DockMode::Automate;
    snapshot.legion_workflow_projection = legion_projection(state);
    snapshot.proposal_ledger_projection = ProposalLedgerProjection {
        rows: vec![proposal_row(ProposalId(901))],
        selected_proposal_id: Some(ProposalId(901)),
        omitted_row_count: 0,
        generated_at: TimestampMillis(11),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot.legion_workflow_board_columns =
        legion_workflow_board_columns(&snapshot.legion_workflow_projection);
    snapshot.legion_workflow_fleet_card_projections = legion_workflow_fleet_card_projections(
        &snapshot.proposal_ledger_projection,
        &snapshot.verification_run_projection,
    );
    snapshot
}

fn workflow_raw_input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1_440.0, 900.0),
        )),
        events,
        ..egui::RawInput::default()
    }
}

fn render_workflow_frame(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
) -> (Vec<DesktopAction>, egui::FullOutput) {
    let mut actions = None;
    let full = ctx.run_ui(workflow_raw_input(Vec::new()), |ui| {
        actions = Some(view.render(ui, snapshot).actions);
    });
    (actions.expect("workflow frame should render"), full)
}

fn workflow_button_bounds(output: &egui::FullOutput, label: &str) -> egui::accesskit::Rect {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("AccessKit update should be enabled")
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click))
                .then(|| node.bounds())
                .flatten()
        })
        .unwrap_or_else(|| panic!("workflow button `{label}` should be allocated"))
}

fn workflow_has_label(output: &egui::FullOutput, label: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update
                .nodes
                .iter()
                .any(|(_id, node)| node.label() == Some(label) || node.value() == Some(label))
        })
}

fn workflow_has_clickable_label(output: &egui::FullOutput, label: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update.nodes.iter().any(|(_id, node)| {
                node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click)
            })
        })
}

fn workflow_contains_text(output: &egui::FullOutput, text: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update.nodes.iter().any(|(_id, node)| {
                node.label().is_some_and(|label| label.contains(text))
                    || node.value().is_some_and(|value| value.contains(text))
            })
        })
}

fn workflow_text_containing_in_x_range(
    output: &egui::FullOutput,
    text: &str,
    x_range: std::ops::RangeInclusive<f32>,
) -> Vec<String> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .map_or_else(Vec::new, |update| {
            update
                .nodes
                .iter()
                .filter(|(_id, node)| {
                    node.bounds().is_some_and(|bounds| {
                        x_range.contains(&(((bounds.x0 + bounds.x1) * 0.5) as f32))
                    })
                })
                .flat_map(|(_id, node)| [node.label(), node.value()])
                .flatten()
                .filter(|value| value.contains(text))
                .map(str::to_string)
                .collect()
        })
}

fn click_workflow_button(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    primed: &egui::FullOutput,
    label: &str,
) -> Vec<DesktopAction> {
    let target = primed
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("AccessKit update should be enabled")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click))
                .then_some(*id)
        })
        .unwrap_or_else(|| panic!("workflow button `{label}` should be actionable"));
    let click = workflow_raw_input(vec![egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Click,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: target,
            data: None,
        },
    )]);
    let mut actions = None;
    let _ = ctx.run_ui(click, |ui| {
        actions = Some(view.render(ui, snapshot).actions);
    });
    actions.expect("workflow click frame should render")
}

#[test]
fn legion_workflow_rendered_proposals_offer_graceful_cancel_without_losing_hard_kill() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::Blocked);
    add_automate_sidecars(&mut snapshot.legion_workflow_projection);
    let mut view = ProjectionView::new();
    let (_initial, full) = render_workflow_frame(&ctx, &mut view, &snapshot);

    for label in [
        "Approve",
        "Review",
        "Reject",
        "Cancel proposal",
        "Stop workflow session 1",
    ] {
        let _ = workflow_button_bounds(&full, label);
    }
    let cancelled = click_workflow_button(&ctx, &mut view, &snapshot, &full, "Cancel proposal");
    assert_eq!(
        cancelled,
        vec![DesktopAction::CancelProposal {
            proposal_id: ProposalId(901),
            reason: ProposalCancellationReason::UserCancelled,
        }]
    );

    let (_next, full) = render_workflow_frame(&ctx, &mut view, &snapshot);
    let killed =
        click_workflow_button(&ctx, &mut view, &snapshot, &full, "Stop workflow session 1");
    assert_eq!(
        killed,
        vec![DesktopAction::TriggerLegionWorkflowKillSwitch {
            session_id: LegionWorkflowSessionId("session:legion:alpha".to_string()),
            reason_label: "user requested hard stop".to_string(),
        }]
    );
}

#[test]
fn legion_workflow_active_session_separates_board_from_inspector_metadata() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::Blocked);
    let mut view = ProjectionView::new();
    let (_initial, full) = render_workflow_frame(&ctx, &mut view, &snapshot);

    assert!(workflow_has_label(&full, "Workflow board"));
    assert!(workflow_contains_text(&full, "Workflow 1"));
    assert!(workflow_has_label(&full, "Workflow sessions"));
    assert!(workflow_has_label(&full, "Selected task"));
    assert!(workflow_has_label(&full, "Approvals"));
    assert!(!workflow_has_label(&full, "Force Review"));
    assert!(!workflow_has_label(&full, "Pause Workflow"));
    assert!(!workflow_has_label(&full, "Add Constraint"));
    assert!(!workflow_has_label(&full, "No budget rows projected"));
    assert!(!workflow_has_label(&full, "No risk gate projected"));
}

#[test]
fn legion_workflow_kill_requires_an_armed_nonterminal_session() {
    for lifecycle in [
        LegionWorkflowState::Completed,
        LegionWorkflowState::Failed,
        LegionWorkflowState::Cancelled,
    ] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::Blocked);
        snapshot.legion_workflow_projection.rows[0].lifecycle_state = lifecycle;
        add_automate_sidecars(&mut snapshot.legion_workflow_projection);
        let mut view = ProjectionView::new();
        let (_initial, full) = render_workflow_frame(&ctx, &mut view, &snapshot);
        assert!(
            !workflow_has_clickable_label(&full, "Stop workflow session 1"),
            "{lifecycle:?} workflow must not expose Kill"
        );
    }

    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::Blocked);
    let mut view = ProjectionView::new();
    let (_initial, full) = render_workflow_frame(&ctx, &mut view, &snapshot);
    assert!(
        !workflow_has_clickable_label(&full, "Stop workflow session 1"),
        "a workflow without a projected kill switch must not expose Kill"
    );

    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut triggered = legion_snapshot(LegionWorkflowMergeReadinessState::Blocked);
    add_automate_sidecars(&mut triggered.legion_workflow_projection);
    triggered.legion_workflow_projection.kill_switches[0].state =
        LegionWorkflowKillSwitchState::Triggered;
    let mut view = ProjectionView::new();
    let (_initial, full) = render_workflow_frame(&ctx, &mut view, &triggered);
    assert!(
        !workflow_has_clickable_label(&full, "Stop workflow session 1"),
        "a triggered kill switch must not expose Kill again"
    );

    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut armed = legion_snapshot(LegionWorkflowMergeReadinessState::Blocked);
    add_automate_sidecars(&mut armed.legion_workflow_projection);
    let mut view = ProjectionView::new();
    let (_initial, full) = render_workflow_frame(&ctx, &mut view, &armed);
    assert!(workflow_has_clickable_label(
        &full,
        "Stop workflow session 1"
    ));
    let killed = click_workflow_button(&ctx, &mut view, &armed, &full, "Stop workflow session 1");
    assert_eq!(
        killed,
        vec![DesktopAction::TriggerLegionWorkflowKillSwitch {
            session_id: LegionWorkflowSessionId("session:legion:alpha".to_string()),
            reason_label: "user requested hard stop".to_string(),
        }]
    );
}

#[test]
fn legion_workflow_live_surfaces_hide_projection_copy_and_raw_identifiers() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::WaitingForApproval);
    add_automate_sidecars(&mut snapshot.legion_workflow_projection);
    snapshot.legion_workflow_budget_rows = vec![LegionWorkflowBudgetUsageRowProjection {
        session_id: LegionWorkflowSessionId("session:legion:alpha".to_string()),
        worker_id: "worker:console".to_string(),
        budget_label: "delegated-loop".to_string(),
        model_turns_label: "model_turns=1/5".to_string(),
        tool_calls_label: "tool_calls=2/8".to_string(),
        retry_label: "retries=0/3".to_string(),
        output_bytes_label: "output_bytes=128/4096".to_string(),
        wall_clock_label: "wall_clock=10/1000ms".to_string(),
        status_label: "within-budget".to_string(),
        schema_version: 1,
    }];
    let mut view = ProjectionView::new();

    let (_initial, full) = render_workflow_frame(&ctx, &mut view, &snapshot);

    for expected in [
        "Waiting for approval",
        "Work stopped · Risk score 3 of 3 · High-risk actions 3 · Denied tools 0",
        "Changes workspace",
        "Uses a local tool",
        "Waiting for confirmation",
        "Delegated loop",
        "Within budget",
        "Model turns 1 of 5",
        "Tool calls 2 of 8",
        "Retries 0 of 3",
        "Output 128 of 4096 bytes",
        "Time 10 of 1000 ms",
        "Owner: legion-reviewer",
        "Model: legion.proposal.review",
        "Risk: Medium",
        "Status: Created",
    ] {
        assert!(
            workflow_contains_text(&full, expected),
            "live workflow surfaces should expose `{expected}`"
        );
    }
    for forbidden in [
        "projected",
        "No fleet cards projected",
        "session:legion:alpha",
        "worker:console",
        "automate:permission:test",
        "kill:test",
        "WaitingForApproval",
        "Halted",
        "InvokeLocalTool",
        "WaitingForConfirmation",
        "delegated-loop",
        "within-budget",
        "Medium risk",
    ] {
        assert!(
            !workflow_contains_text(&full, forbidden),
            "live workflow surfaces must not expose `{forbidden}`"
        );
    }
    let raw_key_value_copy = workflow_text_containing_in_x_range(&full, "=", 1_115.0..=1_440.0);
    assert!(
        raw_key_value_copy.is_empty(),
        "populated workflow, card, and budget surfaces must not expose raw key=value copy: {raw_key_value_copy:?}"
    );
}

#[test]
fn legion_workflow_stop_section_uses_the_same_visible_rows_as_kill_actions() {
    let mut snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::Blocked);
    add_automate_sidecars(&mut snapshot.legion_workflow_projection);
    let template = snapshot.legion_workflow_projection.rows[0].clone();
    snapshot.legion_workflow_projection.rows = (0..4)
        .map(|index| {
            let mut row = template.clone();
            row.session_id = LegionWorkflowSessionId(format!("session:legion:{index}"));
            row
        })
        .collect();
    snapshot.legion_workflow_projection.total_session_count = 4;
    snapshot.legion_workflow_projection.kill_switches[0].session_id =
        snapshot.legion_workflow_projection.rows[3]
            .session_id
            .clone();
    snapshot.legion_workflow_board_columns =
        legion_workflow_board_columns(&snapshot.legion_workflow_projection);

    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let (_initial, full) = render_workflow_frame(&ctx, &mut view, &snapshot);
    assert!(workflow_has_label(&full, "Stop controls"));
    assert!(workflow_has_clickable_label(
        &full,
        "Stop workflow session 4"
    ));
    let stopped =
        click_workflow_button(&ctx, &mut view, &snapshot, &full, "Stop workflow session 4");
    assert_eq!(
        stopped,
        vec![DesktopAction::TriggerLegionWorkflowKillSwitch {
            session_id: LegionWorkflowSessionId("session:legion:3".to_string()),
            reason_label: "user requested hard stop".to_string(),
        }]
    );

    snapshot.legion_workflow_projection.kill_switches[0].session_id =
        snapshot.legion_workflow_projection.rows[2]
            .session_id
            .clone();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let (_initial, full) = render_workflow_frame(&ctx, &mut view, &snapshot);
    assert!(workflow_has_label(&full, "Stop controls"));
    assert!(workflow_has_clickable_label(
        &full,
        "Stop workflow session 3"
    ));
}

#[test]
fn legion_workflow_command_center_rows_show_sessions_gates_and_merge_state() {
    let model = DesktopProjectionViewModel::from_snapshot(&legion_snapshot(
        LegionWorkflowMergeReadinessState::Blocked,
    ));

    assert!(model.legion_workflow_rows.iter().any(|row| {
        row.contains("legion workflow command center")
            && row.contains("sessions=1")
            && row.contains("unattended merge unsupported until approval")
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
            && row.contains("unattended merge unsupported until approval")
    }));
}

#[test]
fn legion_workflow_board_columns_match_coordinator_state_mapping() {
    let mut projection = legion_projection(LegionWorkflowMergeReadinessState::WaitingForApproval);
    let template = projection.rows[0].clone();
    let states = [
        LegionWorkflowState::Draft,
        LegionWorkflowState::Planning,
        LegionWorkflowState::Executing,
        LegionWorkflowState::WaitingForApproval,
        LegionWorkflowState::WaitingOnHuman,
        LegionWorkflowState::Blocked,
        LegionWorkflowState::Verifying,
        LegionWorkflowState::Completed,
        LegionWorkflowState::Failed,
        LegionWorkflowState::Cancelled,
    ];
    projection.rows = states
        .iter()
        .enumerate()
        .map(|(index, state)| {
            let mut row = template.clone();
            row.session_id = LegionWorkflowSessionId(format!("session:state:{index}"));
            row.lifecycle_state = *state;
            row
        })
        .collect();
    projection.total_session_count = projection.rows.len() as u32;

    let columns = legion_workflow_board_columns(&projection);
    let view_models = fleet_board::fleet_board_column_view_models(&columns);

    assert_eq!(view_models.len(), 5);
    for column in &columns {
        for row in &column.rows {
            assert_eq!(
                column.kind,
                LegionWorkflowBoardColumnKind::from_state(row.state)
            );
        }
    }
}

#[test]
fn legion_workflow_board_cards_in_the_same_column_have_distinct_safe_ordinals() {
    let mut projection = legion_projection(LegionWorkflowMergeReadinessState::Ready);
    let template = projection.rows[0].clone();
    projection.rows = ["session:private:first", "session:private:second"]
        .into_iter()
        .map(|session_id| {
            let mut row = template.clone();
            row.session_id = LegionWorkflowSessionId(session_id.to_string());
            row.lifecycle_state = LegionWorkflowState::Executing;
            row
        })
        .collect();
    projection.total_session_count = projection.rows.len() as u32;

    let columns = legion_workflow_board_columns(&projection);
    let view_models = fleet_board::fleet_board_column_view_models(&columns);
    let in_progress = view_models
        .iter()
        .find(|column| column.kind == LegionWorkflowBoardColumnKind::InProgress)
        .expect("in-progress column should exist");

    assert_eq!(
        in_progress
            .rows
            .iter()
            .map(|row| row.task_label.as_str())
            .collect::<Vec<_>>(),
        vec!["Workflow 1", "Workflow 2"]
    );
    assert!(in_progress.rows.iter().all(|row| {
        !row.task_label.contains("session:") && !row.summary_label.contains("session:")
    }));
}

#[test]
fn legion_workflow_fleet_cards_use_projection_fields_without_log_parsing() {
    let snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::WaitingForApproval);
    let cards =
        fleet_card::fleet_card_view_models(&snapshot.legion_workflow_fleet_card_projections);
    let source = &snapshot.legion_workflow_fleet_card_projections[0];
    let card = &cards[0];

    assert_eq!(card.proposal_id, source.proposal_id.0.to_string());
    assert_eq!(card.title, source.title);
    assert_eq!(card.owner_label, source.owner_label);
    assert_eq!(card.model_label, source.model_label);
    assert_eq!(card.status_label, source.status_label);
    assert_eq!(card.progress_label, source.progress_label);
    assert_eq!(card.files_label, source.files_label);
    assert_eq!(card.risk_label, source.risk_label);
    assert_eq!(card.test_status_label, source.test_status_label);
    assert_eq!(card.mini_diff_label, source.mini_diff_label);
    assert_eq!(card.last_activity_label, source.last_activity_label);
}

#[test]
fn legion_workflow_comm_stream_handles_every_documented_tag() {
    let rows: Vec<String> = AgentCommTag::ALL
        .iter()
        .map(|tag| {
            format_agent_comm_line(
                "2026-07-08T12:00:00Z",
                *tag,
                "worker:console",
                "metadata-only event",
            )
        })
        .collect();

    let parsed = agent_comm::agent_comm_rows(&rows);
    let parsed_tags: Vec<_> = parsed.iter().map(|row| row.tag).collect();

    assert_eq!(parsed.len(), AgentCommTag::ALL.len());
    assert_eq!(parsed_tags, AgentCommTag::ALL);
    assert!(parsed.iter().all(|row| !row.tag_label.is_empty()));
}

#[test]
fn legion_workflow_kill_switch_acknowledgement_surfaces_in_decision_feed() {
    let mut snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::WaitingForApproval);
    add_automate_sidecars(&mut snapshot.legion_workflow_projection);
    snapshot
        .legion_workflow_projection
        .decision_feed
        .push(LegionWorkflowDecisionFeedEntry {
            decision_id: LegionWorkflowDecisionId("decision:kill".to_string()),
            session_id: LegionWorkflowSessionId("session:legion:alpha".to_string()),
            worker_id: None,
            kind: LegionWorkflowDecisionKind::KillSwitchTriggered,
            summary_label: "Legion Workflows kill switch triggered".to_string(),
            rationale_labels: vec!["operator_ack".to_string()],
            risk_label: ProposalRiskLabel::High,
            mcp_server_id: None,
            mcp_primitive_kind: None,
            tool_permission_request_id: None,
            correlation_id: CorrelationId(7),
            causality_id: causality("00000000-0000-0000-0000-000000000007"),
            event_sequence: EventSequence(7),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        });
    snapshot.legion_workflow_projection.decision_feed_count =
        snapshot.legion_workflow_projection.decision_feed.len() as u32;

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(model.legion_workflow_rows.iter().any(|row| {
        row.contains("KillSwitchTriggered")
            && row.contains("Legion Workflows kill switch triggered")
    }));
}

#[test]
fn legion_workflow_budget_rows_surface_per_worker_usage() {
    let mut snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::WaitingForApproval);
    snapshot.legion_workflow_budget_rows = vec![LegionWorkflowBudgetUsageRowProjection {
        session_id: LegionWorkflowSessionId("session:legion:alpha".to_string()),
        worker_id: "worker:console".to_string(),
        budget_label: "delegated-loop".to_string(),
        model_turns_label: "model_turns=1/5".to_string(),
        tool_calls_label: "tool_calls=2/8".to_string(),
        retry_label: "retries=0/3".to_string(),
        output_bytes_label: "output_bytes=128/4096".to_string(),
        wall_clock_label: "wall_clock=10/1000ms".to_string(),
        status_label: "within-budget".to_string(),
        schema_version: 1,
    }];

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(model.legion_workflow_rows.iter().any(|row| {
        row.contains("legion workflow budget")
            && row.contains("worker=worker:console")
            && row.contains("model_turns=1/5")
            && row.contains("tool_calls=2/8")
            && row.contains("within-budget")
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
                gate_id: legion_protocol::LegionWorkflowVerificationGateId(
                    "verification:unit".to_string()
                ),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::RequestLegionWorkflowVerification {
            session_id: session_id.clone(),
            gate_id: legion_protocol::LegionWorkflowVerificationGateId(
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
                conflict_id: legion_protocol::LegionWorkflowConflictId(
                    "conflict:unknown".to_string()
                ),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowConflict {
            session_id: LegionWorkflowSessionId("session:legion:alpha".to_string()),
            conflict_id: legion_protocol::LegionWorkflowConflictId("conflict:unknown".to_string()),
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
            .contains(&"Legion Workflows merge: unsupported until approval".to_string())
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
        model
            .legion_workflow_rows
            .iter()
            .any(|row| row.contains("spec_artifact=artifact:spec:legion:alpha"))
    );
    assert!(
        !model
            .legion_workflow_rows
            .iter()
            .any(|row| { row.contains("autonomous merge action") || row.contains("direct apply") })
    );
}

#[test]
fn legion_workflow_automate_rows_show_mcp_decisions_risk_kill_and_permissions() {
    let mut snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::WaitingForApproval);
    add_automate_sidecars(&mut snapshot.legion_workflow_projection);

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(model.legion_workflow_rows.iter().any(|row| {
        row.contains("mcp=1")
            && row.contains("decisions=1")
            && row.contains("risk_monitors=1")
            && row.contains("kill_switches=1")
            && row.contains("permissions=1")
    }));
    assert!(
        model
            .legion_workflow_rows
            .iter()
            .any(|row| row.contains("legion workflow mcp registry"))
    );
    assert!(
        model
            .legion_workflow_rows
            .iter()
            .any(|row| row.contains("legion workflow decision"))
    );
    assert!(
        model
            .legion_workflow_rows
            .iter()
            .any(|row| row.contains("risk monitor") && row.contains("Halted"))
    );
    assert!(
        model
            .legion_workflow_rows
            .iter()
            .any(|row| row.contains("tool permission"))
    );
}

#[test]
fn delegated_task_rows_show_runtime_activation_in_the_assistant_surface() {
    let mut snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::WaitingForApproval);
    snapshot.delegated_task_projection.runtime_activation =
        DelegatedTaskRuntimeActivationState::SandboxAllocated;
    snapshot.delegated_task_projection.plan_count = 1;

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("delegated task command center") && row.contains("runtime=SandboxAllocated")
    }));
}

#[test]
fn legion_workflow_bridge_routes_automate_tool_permission_and_kill_switch() {
    let mut snapshot = legion_snapshot(LegionWorkflowMergeReadinessState::WaitingForApproval);
    add_automate_sidecars(&mut snapshot.legion_workflow_projection);
    let bridge = DesktopCommandBridge::new();
    let session_id = LegionWorkflowSessionId("session:legion:alpha".to_string());
    let server_id = McpServerId("mcp:test".to_string());
    let tool_name = McpToolName("write_file".to_string());

    assert_eq!(
        bridge.translate(
            DesktopAction::RecordLegionWorkflowToolPermission {
                session_id: session_id.clone(),
                server_id: server_id.clone(),
                tool_name: tool_name.clone(),
                decision: DelegatedTaskToolPermissionDecision::Allow,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(
            legion_ui::CommandDispatchIntent::RecordLegionWorkflowToolPermission {
                session_id: session_id.clone(),
                server_id: server_id.clone(),
                tool_name: tool_name.clone(),
                decision: DelegatedTaskToolPermissionDecision::Allow,
            }
        )
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::TriggerLegionWorkflowKillSwitch {
                session_id: session_id.clone(),
                reason_label: "operator stop".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(
            legion_ui::CommandDispatchIntent::TriggerLegionWorkflowKillSwitch {
                session_id: session_id.clone(),
                reason_label: "operator stop".to_string(),
            }
        )
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::RecordLegionWorkflowToolPermission {
                session_id,
                server_id: server_id.clone(),
                tool_name: McpToolName("missing".to_string()),
                decision: DelegatedTaskToolPermissionDecision::Allow,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownLegionWorkflowMcpTool {
            server_id,
            tool_name: McpToolName("missing".to_string()),
        })
    );
}
