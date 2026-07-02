use legion_protocol::{
    LegionWorkflowMergeReadiness, LegionWorkflowMergeReadinessState, LegionWorkflowProjection,
    LegionWorkflowProjectionRow, LegionWorkflowSessionId, LegionWorkflowState, RedactionHint,
    TimestampMillis,
};
use legion_ui::projection::{LegionWorkflowBoardColumnKind, legion_workflow_board_columns};

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
fn workflow_board_columns_are_grouped_by_coordinator_state() {
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
        LegionWorkflowState::WaitingForApproval | LegionWorkflowState::Blocked
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
