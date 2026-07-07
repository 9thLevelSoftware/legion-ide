//! Approved editable-plan to Legion workflow session construction.

use std::collections::HashMap;

use legion_protocol::{
    CausalityId, CommandRiskLabel, CorrelationId, DelegatedTaskAffectedTargetSummary,
    DelegatedTaskOperationClass, EditablePlanArtifact, LegionWorkflowDependency,
    LegionWorkflowDependencyId, LegionWorkflowDependencyState, LegionWorkflowModelBackend,
    LegionWorkflowSession, LegionWorkflowSessionId, LegionWorkflowState,
    LegionWorkflowWorkerAssignment, LegionWorkflowWorkerId, LegionWorkflowWorkerRole,
    LegionWorkflowWorkerState, PrivacyClassification, ProductMode, ProposalTargetKind,
    RedactionHint, TaskGraphArtifact, TaskNode, TimestampMillis, validate_legion_workflow_session,
};

use crate::{AgentError, dag::WorkflowDag};

/// Configuration for building a metadata-only Legion workflow session from an approved plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegionWorkflowSessionBuilderConfig {
    /// Stable session identifier.
    pub session_id: String,
    /// Generation timestamp for the session metadata.
    pub generated_at: TimestampMillis,
    /// Audit correlation id.
    pub correlation_id: CorrelationId,
    /// Audit causality id.
    pub causality_id: CausalityId,
}

fn stable_task_worker_id(plan_id: &str, task_index: usize) -> LegionWorkflowWorkerId {
    LegionWorkflowWorkerId(format!("{plan_id}/tasks/{task_index}"))
}

fn worker_assignment(
    plan: &EditablePlanArtifact,
    task: &TaskNode,
    task_index: usize,
    config: &LegionWorkflowSessionBuilderConfig,
) -> LegionWorkflowWorkerAssignment {
    LegionWorkflowWorkerAssignment {
        worker_id: stable_task_worker_id(&plan.artifact_id, task_index),
        role: LegionWorkflowWorkerRole::Implementer,
        state: LegionWorkflowWorkerState::Pending,
        model_backend: LegionWorkflowModelBackend::Local,
        display_safe_model_label: "local metadata planner".to_string(),
        allowed_command_classes: vec![
            DelegatedTaskOperationClass::ReadContextMetadata,
            DelegatedTaskOperationClass::DraftProposalMetadata,
            DelegatedTaskOperationClass::SummarizeVerificationReadiness,
            DelegatedTaskOperationClass::RequestHumanApproval,
        ],
        linked_delegated_plan_id: None,
        assisted_ai_route: None,
        affected_targets: task
            .target_labels
            .iter()
            .enumerate()
            .map(|(target_index, label)| DelegatedTaskAffectedTargetSummary {
                target_id: format!("{}/targets/{}", task.task_id, target_index),
                kind: ProposalTargetKind::MetadataOnly,
                workspace_id: None,
                file_id: None,
                buffer_id: None,
                ranges: Vec::new(),
                hashes: Vec::new(),
                counts: Vec::new(),
                labels: vec![label.clone()],
                risk_label: task.risk_label,
                privacy_label: task.privacy_label,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            })
            .collect(),
        risk_labels: vec![CommandRiskLabel::Review],
        privacy_labels: vec![PrivacyClassification::Metadata],
        correlation_id: config.correlation_id,
        causality_id: config.causality_id,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn dependency_edges(
    plan_id: &str,
    task_graph: &TaskGraphArtifact,
    worker_ids_by_task: &HashMap<String, LegionWorkflowWorkerId>,
) -> Result<Vec<LegionWorkflowDependency>, AgentError> {
    let mut edges = Vec::new();
    for (successor_index, task) in task_graph.nodes.iter().enumerate() {
        let successor = worker_ids_by_task
            .get(&task.task_id)
            .cloned()
            .ok_or_else(|| AgentError::InvalidLegionWorkflow("task worker missing".to_string()))?;
        for dependency_task_id in &task.depends_on {
            let predecessor = worker_ids_by_task
                .get(dependency_task_id)
                .cloned()
                .ok_or_else(|| {
                    AgentError::InvalidLegionWorkflow(format!(
                        "unknown task dependency {dependency_task_id} for {}",
                        task.task_id
                    ))
                })?;
            let predecessor_index = task_graph
                .nodes
                .iter()
                .position(|candidate| candidate.task_id == *dependency_task_id)
                .ok_or_else(|| {
                    AgentError::InvalidLegionWorkflow(format!(
                        "unknown task dependency {dependency_task_id} for {}",
                        task.task_id
                    ))
                })?;
            edges.push(LegionWorkflowDependency {
                dependency_id: LegionWorkflowDependencyId(format!(
                    "{plan_id}/dependencies/{predecessor_index}/{successor_index}"
                )),
                predecessor_worker_id: predecessor,
                successor_worker_id: successor.clone(),
                state: LegionWorkflowDependencyState::Pending,
                label: format!("{dependency_task_id} before {}", task.task_id),
                schema_version: 1,
            });
        }
    }
    Ok(edges)
}

/// Builds a metadata-only Legion workflow session from an approved editable plan.
///
/// The required [`WorkflowDag`] keeps unapproved plans outside this boundary. The
/// session itself is only planning metadata; it does not start worker execution.
pub fn legion_workflow_session_from_approved_plan(
    plan: &EditablePlanArtifact,
    dag: &WorkflowDag,
    task_graph: &TaskGraphArtifact,
    config: LegionWorkflowSessionBuilderConfig,
) -> Result<LegionWorkflowSession, AgentError> {
    if plan.review_required || dag.plan_id != plan.artifact_id {
        return Err(AgentError::InvalidLegionWorkflow(
            "approved plan DAG is required before session construction".to_string(),
        ));
    }
    plan.validate()
        .map_err(|error| AgentError::InvalidLegionWorkflow(error.to_string()))?;

    let worker_assignments = task_graph
        .nodes
        .iter()
        .enumerate()
        .map(|(task_index, task)| worker_assignment(plan, task, task_index, &config))
        .collect::<Vec<_>>();
    let worker_ids_by_task = task_graph
        .nodes
        .iter()
        .enumerate()
        .map(|(task_index, task)| {
            (
                task.task_id.clone(),
                stable_task_worker_id(&plan.artifact_id, task_index),
            )
        })
        .collect::<HashMap<_, _>>();
    let dependency_edges = dependency_edges(&plan.artifact_id, task_graph, &worker_ids_by_task)?;

    let session = LegionWorkflowSession {
        session_id: LegionWorkflowSessionId(config.session_id),
        directive_artifact_id: Some(plan.directive_id.clone()),
        spec_artifact_id: plan.spec_artifact_id.clone(),
        task_graph_artifact_id: plan.task_graph_artifact_id.clone(),
        product_mode: ProductMode::LegionWorkflows,
        worker_assignments,
        dependency_edges,
        conflict_summaries: Vec::new(),
        verification_gates: Vec::new(),
        sign_off_records: Vec::new(),
        proposal_ids: Vec::new(),
        merge_approval: None,
        lifecycle_state: LegionWorkflowState::Planning,
        generated_at: config.generated_at,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
        correlation_id: config.correlation_id,
        causality_id: config.causality_id,
    };
    validate_legion_workflow_session(&session)
        .map_err(|error| AgentError::InvalidLegionWorkflow(error.message))?;
    Ok(session)
}
