//! Cloud Lane panel: cancel controls that exist only where cancelling is real.
//!
//! The model half of P9.F3.T3's "cancellable mid-flight". The bridge is
//! asserted alongside it because a button that pushes an action the bridge
//! refuses is not a working control, and the view model alone cannot prove the
//! action reaches app authority.

use legion_desktop::{
    bridge::{DesktopAction, DesktopBridgeError, DesktopBridgeOutput, DesktopCommandBridge},
    view::cloud_lane::DesktopCloudLanePanelViewModel,
};
use legion_protocol::{
    LegionCloudLaneProjection, LegionCloudLaneProjectionRow, LegionCloudLaneTaskId,
    LegionCloudLaneTaskState, RedactionHint, TimestampMillis,
};
use legion_ui::{Shell, ShellProjectionSnapshot, ui::CommandDispatchIntent};

fn row(task: &str, state: LegionCloudLaneTaskState) -> LegionCloudLaneProjectionRow {
    LegionCloudLaneProjectionRow {
        task_id: LegionCloudLaneTaskId(task.to_string()),
        lane_id: "cloud-lane:validation".to_string(),
        state,
        status_label: format!("{state:?}"),
        estimated_cost_cents: 50,
        billed_cost_cents: 10,
        upload_bytes: 12_288,
        scope_visible_to_user: true,
        proposal_id: None,
        evidence_count: 0,
    }
}

fn snapshot_with(rows: Vec<LegionCloudLaneProjectionRow>) -> ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Cloud Lane").projection_snapshot();
    snapshot.legion_cloud_lane = LegionCloudLaneProjection {
        projection_id: "legion-cloud-lane:test".to_string(),
        runtime_enabled: true,
        rows,
        status_label: "Legion Cloud Lane runtime enabled".to_string(),
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot
}

#[test]
fn an_in_flight_task_gets_a_cancel_control_and_a_finished_one_does_not() {
    let snapshot = snapshot_with(vec![
        row("cloud-task:1", LegionCloudLaneTaskState::Submitted),
        row("cloud-task:2", LegionCloudLaneTaskState::Running),
        row("cloud-task:3", LegionCloudLaneTaskState::Completed),
        row("cloud-task:4", LegionCloudLaneTaskState::Failed),
        row("cloud-task:5", LegionCloudLaneTaskState::Cancelled),
    ]);
    let model = DesktopCloudLanePanelViewModel::from_snapshot(&snapshot);

    assert!(model.runtime_enabled);
    assert_eq!(model.rows.len(), 5);
    assert!(
        model.rows[0].cancel_action.is_some(),
        "submitted is cancellable"
    );
    assert!(
        model.rows[1].cancel_action.is_some(),
        "running is cancellable"
    );
    // A cancel control on a finished upload would tell the user their data was
    // withheld when it has already left.
    assert!(model.rows[2].cancel_action.is_none(), "completed is not");
    assert!(model.rows[3].cancel_action.is_none(), "failed is not");
    assert!(model.rows[4].cancel_action.is_none(), "cancelled is not");
}

#[test]
fn the_cancel_control_carries_the_task_it_targets() {
    let snapshot = snapshot_with(vec![row("cloud-task:1", LegionCloudLaneTaskState::Running)]);
    let model = DesktopCloudLanePanelViewModel::from_snapshot(&snapshot);

    let action = model.rows[0]
        .cancel_action
        .clone()
        .expect("a running task is cancellable");
    match action {
        DesktopAction::CancelCloudLaneTask {
            task_id,
            reason_label,
        } => {
            assert_eq!(task_id, "cloud-task:1");
            assert!(
                !reason_label.trim().is_empty(),
                "app authority refuses a blank reason, so the control must carry one"
            );
        }
        other => panic!("expected a cancel action, got {other:?}"),
    }
}

#[test]
fn the_bridge_translates_a_cancel_into_an_app_intent() {
    let snapshot = snapshot_with(vec![row("cloud-task:1", LegionCloudLaneTaskState::Running)]);
    let bridge = DesktopCommandBridge::new();

    let output = bridge.translate(
        DesktopAction::CancelCloudLaneTask {
            task_id: "cloud-task:1".to_string(),
            reason_label: "user changed their mind".to_string(),
        },
        &snapshot,
    );
    match output {
        DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelCloudLaneTask {
            task_id,
            reason_label,
        }) => {
            assert_eq!(task_id, "cloud-task:1");
            assert_eq!(reason_label, "user changed their mind");
        }
        other => panic!("expected a cancel intent, got {other:?}"),
    }
}

#[test]
fn the_bridge_refuses_a_cancel_for_a_task_the_projection_does_not_show() {
    let snapshot = snapshot_with(vec![row("cloud-task:1", LegionCloudLaneTaskState::Running)]);
    let bridge = DesktopCommandBridge::new();

    let output = bridge.translate(
        DesktopAction::CancelCloudLaneTask {
            task_id: "cloud-task:absent".to_string(),
            reason_label: "reason".to_string(),
        },
        &snapshot,
    );
    assert!(
        matches!(
            output,
            DesktopBridgeOutput::Error(DesktopBridgeError::UnknownCloudLaneTask { .. })
        ),
        "expected an unknown-task refusal, got {output:?}"
    );
}

#[test]
fn the_bridge_refuses_a_cancel_for_a_finished_task_even_when_synthesised() {
    // The view model withholds the button, but a keybinding or command-palette
    // entry could synthesise the same action, so the guard lives in the bridge
    // and is asserted there rather than trusting the painter.
    let snapshot = snapshot_with(vec![row(
        "cloud-task:1",
        LegionCloudLaneTaskState::Completed,
    )]);
    let bridge = DesktopCommandBridge::new();

    let output = bridge.translate(
        DesktopAction::CancelCloudLaneTask {
            task_id: "cloud-task:1".to_string(),
            reason_label: "reason".to_string(),
        },
        &snapshot,
    );
    assert!(
        matches!(
            output,
            DesktopBridgeOutput::Error(DesktopBridgeError::CloudLaneTaskNotCancellable { .. })
        ),
        "expected a not-cancellable refusal, got {output:?}"
    );
}

#[test]
fn a_disabled_runtime_renders_its_reason_rather_than_an_empty_panel() {
    let snapshot = Shell::empty("Cloud Lane").projection_snapshot();
    let model = DesktopCloudLanePanelViewModel::from_snapshot(&snapshot);

    assert!(!model.runtime_enabled);
    assert!(model.is_empty());
    assert!(
        model.status_label.contains("disabled"),
        "a disabled runtime and an idle one look identical in their rows and \
         mean opposite things; the label is what separates them: {}",
        model.status_label
    );
}

#[test]
fn the_panel_reports_whether_each_upload_had_its_scope_surfaced() {
    let mut hidden = row("cloud-task:1", LegionCloudLaneTaskState::Running);
    hidden.scope_visible_to_user = false;
    let snapshot = snapshot_with(vec![
        row("cloud-task:0", LegionCloudLaneTaskState::Running),
        hidden,
    ]);
    let model = DesktopCloudLanePanelViewModel::from_snapshot(&snapshot);

    assert!(model.rows[0].scope_visible_to_user);
    assert!(
        !model.rows[1].scope_visible_to_user,
        "the flag reaches the panel unchanged; a surface that quietly showed \
         every upload as reviewed would be worse than showing none"
    );
}
