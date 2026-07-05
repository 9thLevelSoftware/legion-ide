//! Workflow scheduling helpers for Legion workflow coordination.

use crate::AgentError;
use legion_protocol::{
    LegionWorkflowDependencyState, LegionWorkflowSession, LegionWorkflowWorkerAssignment,
    LegionWorkflowWorkerState,
};
use std::collections::{HashMap, HashSet};

fn worker_can_be_scheduled(state: LegionWorkflowWorkerState) -> bool {
    matches!(
        state,
        LegionWorkflowWorkerState::Pending
            | LegionWorkflowWorkerState::Ready
            | LegionWorkflowWorkerState::WaitingForDependency
            | LegionWorkflowWorkerState::ProviderRouteRequired
    )
}

fn dependencies_satisfied_for(
    session: &LegionWorkflowSession,
    worker_id: &str,
    scheduled_worker_ids: &HashSet<String>,
) -> bool {
    session
        .dependency_edges
        .iter()
        .filter(|dependency| dependency.successor_worker_id.0 == worker_id)
        .all(|dependency| {
            dependency.state == LegionWorkflowDependencyState::Satisfied
                || scheduled_worker_ids.contains(&dependency.predecessor_worker_id.0)
        })
}

fn worker_lookup(
    session: &LegionWorkflowSession,
) -> HashMap<String, LegionWorkflowWorkerAssignment> {
    session
        .worker_assignments
        .iter()
        .cloned()
        .map(|worker| (worker.worker_id.0.clone(), worker))
        .collect()
}

/// Returns the workflow schedule grouped into deterministic parallel lanes.
///
/// Each lane contains workers whose dependencies are already satisfied by
/// previous lanes or by explicit satisfied dependency states.
pub fn parallel_worker_lanes(
    session: &LegionWorkflowSession,
) -> Result<Vec<Vec<LegionWorkflowWorkerAssignment>>, AgentError> {
    let worker_lookup = worker_lookup(session);
    let mut remaining_worker_ids: Vec<String> = session
        .worker_assignments
        .iter()
        .filter(|worker| worker_can_be_scheduled(worker.state))
        .map(|worker| worker.worker_id.0.clone())
        .collect();
    let mut scheduled_worker_ids: HashSet<String> = session
        .worker_assignments
        .iter()
        .filter(|worker| worker.state == LegionWorkflowWorkerState::Completed)
        .map(|worker| worker.worker_id.0.clone())
        .collect();
    let mut lanes = Vec::new();

    while !remaining_worker_ids.is_empty() {
        let lane_ids: Vec<String> = remaining_worker_ids
            .iter()
            .filter(|worker_id| {
                dependencies_satisfied_for(session, worker_id.as_str(), &scheduled_worker_ids)
            })
            .cloned()
            .collect();

        if lane_ids.is_empty() {
            return Err(AgentError::InvalidLegionWorkflow(
                "parallel scheduling stalled before dependencies were satisfied".to_string(),
            ));
        }

        let lane = lane_ids
            .iter()
            .map(|worker_id| {
                worker_lookup.get(worker_id).cloned().ok_or_else(|| {
                    AgentError::InvalidLegionWorkflow(format!(
                        "parallel scheduling referenced unknown worker: {worker_id}"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        for worker_id in &lane_ids {
            scheduled_worker_ids.insert(worker_id.clone());
        }

        remaining_worker_ids.retain(|worker_id| !lane_ids.contains(worker_id));
        lanes.push(lane);
    }

    Ok(lanes)
}
