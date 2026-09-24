//! Cloud Lane panel: egress manifest before submit, cancellation after (P9.F3.T3).
//!
//! Split the way the other panels here are: a pure view model built from
//! projection data (unit-testable with no render pass) and a thin painter that
//! only reads the model and pushes [`DesktopAction`]s.
//!
//! The task's acceptance is "every Cloud Lane upload is paired with a visible
//! egress manifest and is cancellable mid-flight", and both halves were missing
//! in different ways:
//!
//! * **Visible.** `LegionCloudLaneUploadManifest::scope_visible_to_user` is a
//!   `bool` the caller sets. The contract layer refused a manifest that said
//!   `false` and the security broker denied submit without it, so the *flag* was
//!   enforced — but nothing rendered a manifest, so the flag recorded a claim
//!   about something that never happened.
//!   `legion_app::cloud_lane_egress` now requires an acknowledgement bound to
//!   the manifest's contents, and this panel reports per task whether that
//!   scope was surfaced.
//!
//!   The *pre-submit* manifest itself is rendered at the app layer
//!   (`CloudLaneEgressManifestView::rendered_lines`) rather than here, because
//!   nothing submits a Cloud Lane task from the desktop yet: submission is
//!   programmatic, so there is no dialog for a pre-submit view to live in. A
//!   painter with no caller would be the same dead surface this module was
//!   written to replace.
//! * **Cancellable.** `LegionCloudLaneTransport::cancel_task` existed from the
//!   start and no product path reached it. Every non-terminal row here carries a
//!   real [`DesktopAction::CancelCloudLaneTask`].
//!
//! ## Mode
//!
//! This surface belongs to the `RemoteWorkspace` panel, which is declared with
//! `[RemoteSurface, NetworkEgress, CloudProvider]` capabilities. Manual mode is
//! forbidden from exposing any of those, and `legion-ui`'s Manual-mode
//! regression suite asserts that against panel *capabilities* rather than a
//! hard-coded id list — so this panel cannot appear in Manual mode without that
//! suite failing first.

use legion_protocol::LegionCloudLaneTaskState;
use legion_ui::ShellProjectionSnapshot;

use super::{components, theme};
use crate::bridge::DesktopAction;

/// One in-flight Cloud Lane task row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudLaneTaskRowViewModel {
    /// Task id, used as the action target.
    pub task_id: String,
    /// Display heading.
    pub heading: String,
    /// Lifecycle summary.
    pub state_label: String,
    /// Cost summary.
    pub cost_label: String,
    /// Whether the upload scope was surfaced before submit.
    pub scope_visible_to_user: bool,
    /// Cancel action, present only while the task can still be cancelled.
    pub cancel_action: Option<DesktopAction>,
}

/// The Cloud Lane panel view model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopCloudLanePanelViewModel {
    /// Whether the Cloud Lane runtime is enabled by app policy.
    pub runtime_enabled: bool,
    /// Status summary from the projection.
    pub status_label: String,
    /// One row per tracked task.
    pub rows: Vec<CloudLaneTaskRowViewModel>,
}

impl DesktopCloudLanePanelViewModel {
    /// Build the panel model from a projection snapshot.
    pub fn from_snapshot(snapshot: &ShellProjectionSnapshot) -> Self {
        let projection = &snapshot.legion_cloud_lane;
        Self {
            runtime_enabled: projection.runtime_enabled,
            status_label: projection.status_label.clone(),
            rows: projection
                .rows
                .iter()
                .map(|row| {
                    // Terminal tasks get no cancel control. A button that
                    // reports success against an upload that already completed
                    // would tell the user their data was withheld when it has
                    // already left.
                    let cancellable = !matches!(
                        row.state,
                        LegionCloudLaneTaskState::Completed
                            | LegionCloudLaneTaskState::Failed
                            | LegionCloudLaneTaskState::Cancelled
                    );
                    CloudLaneTaskRowViewModel {
                        task_id: row.task_id.0.clone(),
                        heading: format!("{} — {}", row.lane_id, row.task_id.0),
                        state_label: format!("{:?}", row.state),
                        cost_label: format!(
                            "{} bytes, ~{} cents (billed {})",
                            row.upload_bytes, row.estimated_cost_cents, row.billed_cost_cents
                        ),
                        scope_visible_to_user: row.scope_visible_to_user,
                        cancel_action: cancellable.then(|| DesktopAction::CancelCloudLaneTask {
                            task_id: row.task_id.0.clone(),
                            reason_label: "cancelled from the Cloud Lane panel".to_string(),
                        }),
                    }
                })
                .collect(),
        }
    }

    /// Whether the panel has any task to show.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

pub(crate) fn render_cloud_lane_panel(
    ui: &mut egui::Ui,
    model: &DesktopCloudLanePanelViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    if !model.runtime_enabled {
        ui.label(theme::muted(&model.status_label));
        return;
    }
    if model.is_empty() {
        ui.label(theme::muted("No Cloud Lane tasks have been submitted."));
        return;
    }

    for row in &model.rows {
        theme::small_card_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::body_strong(&row.heading));
                components::status_badge(ui, &row.state_label, theme::tokens().accent.blue, true);
                // Shown for every row, not only the bad ones: "scope shown"
                // is the property this whole surface exists to make true, so
                // its absence has to be visible rather than inferred.
                components::status_badge(
                    ui,
                    if row.scope_visible_to_user {
                        "scope shown"
                    } else {
                        "scope NOT shown"
                    },
                    if row.scope_visible_to_user {
                        theme::tokens().accent.green
                    } else {
                        theme::tokens().accent.red
                    },
                    true,
                );
            });
            ui.label(theme::muted(&row.cost_label));

            if let Some(action) = &row.cancel_action {
                if components::primary_button(ui, "Cancel upload", theme::tokens().accent.red)
                    .clicked()
                {
                    actions.push(action.clone());
                }
            } else {
                ui.label(theme::muted("Task is finished and cannot be cancelled."));
            }
        });
    }
}
