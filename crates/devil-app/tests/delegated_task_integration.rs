use devil_app::AppComposition;
use devil_protocol::{
    CausalityId, CorrelationId, DelegatedTaskPlanId, DelegatedTaskPlanningBoundaryInput,
    FileFingerprint, TimestampMillis, WorkspaceId, WorkspaceTrustState,
    delegated_task_plan_from_boundary_input,
};

#[test]
fn test_execute_delegated_task_not_found() {
    let mut app = AppComposition::new();
    let plan_id = DelegatedTaskPlanId("non-existent-plan-id".to_string());
    let res = app.execute_delegated_task(&plan_id);
    assert!(res.is_err());
    assert!(
        res.unwrap_err()
            .contains("Plan contract non-existent-plan-id not found")
    );
}

#[test]
fn test_execute_delegated_task_success() {
    let mut app = AppComposition::new();
    let plan_id = DelegatedTaskPlanId("test-plan-id".to_string());

    let boundary_input = DelegatedTaskPlanningBoundaryInput {
        plan_id: plan_id.clone(),
        workspace_id: Some(WorkspaceId(1)),
        objective_summary_hash: FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "test-hash".to_string(),
        },
        allowed_operation_classes: vec![],
        context_manifest: None,
        privacy_inspector: None,
        permission_budget_projection: None,
        approval_checklist: None,
        checkpoint_rollback: None,
        assisted_ai_projection: None,
        assisted_ai_required: false,
        affected_targets: vec![],
        steps: vec![],
        proposal_preview_links: vec![],
        workspace_trust_state: WorkspaceTrustState::Trusted,
        privacy_denied: false,
        permission_budget_denied: false,
        permission_budget_depleted: false,
        approval_checklist_valid: true,
        checkpoint_required: false,
        checkpoint_available: true,
        rollback_required: false,
        rollback_available: true,
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(uuid::Uuid::from_u128(1)),
        created_at: TimestampMillis(1),
        schema_version: 1,
    };

    let contract = delegated_task_plan_from_boundary_input(boundary_input);

    app.seed_delegated_task_plan_contracts(vec![contract]);

    let res = app.execute_delegated_task(&plan_id);
    assert!(
        res.is_ok(),
        "Expected execute_delegated_task to succeed: {:?}",
        res
    );
    let proposal = res.unwrap();
    assert_eq!(proposal.correlation_id.0, 1);
    assert!(!proposal.causality_id.0.is_nil());
}
