use legion_protocol::{LegionCloudLaneProjectionRow, LegionCloudLaneTaskState};
use legion_ui::ShellProjectionSnapshot;

pub(crate) fn rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let projection = &snapshot.cloud_lane_projection;
    let manifest = &snapshot.context_manifest_projection.manifest;

    let mut rows = Vec::with_capacity(4 + projection.rows.len());
    rows.push(format!(
        "cloud lane: projection={} runtime={} status={}",
        projection.projection_id,
        if projection.runtime_enabled {
            "enabled"
        } else {
            "disabled"
        },
        projection.status_label,
    ));
    rows.push(format!(
        "egress manifest: id={} items={} egress={:?} selected={}",
        manifest.manifest_id,
        manifest.items.len(),
        manifest.egress,
        snapshot
            .context_manifest_projection
            .selected_item_id
            .as_deref()
            .unwrap_or("none"),
    ));

    if projection.rows.is_empty() {
        rows.push("cloud lane task: no submitted tasks".to_string());
    } else {
        for row in &projection.rows {
            rows.push(render_row(row));
        }
    }

    rows.push(
        "cancellation: mid-flight cancel is available while the task is not terminal".to_string(),
    );
    rows
}

fn render_row(row: &LegionCloudLaneProjectionRow) -> String {
    format!(
        "cloud lane task {} lane={} state={:?} status={} upload={} budget={}c billed={}c scope_visible={} proposal={} evidence={} cancelable={}",
        row.task_id.0,
        row.lane_id,
        row.state,
        row.status_label,
        row.upload_bytes,
        row.estimated_cost_cents,
        row.billed_cost_cents,
        row.scope_visible_to_user,
        row.proposal_id
            .as_ref()
            .map(|proposal_id| proposal_id.0.to_string())
            .unwrap_or("none".to_string()),
        row.evidence_count,
        is_cancelable(row.state),
    )
}

fn is_cancelable(state: LegionCloudLaneTaskState) -> bool {
    !matches!(
        state,
        LegionCloudLaneTaskState::Completed
            | LegionCloudLaneTaskState::Cancelled
            | LegionCloudLaneTaskState::Failed
    )
}
