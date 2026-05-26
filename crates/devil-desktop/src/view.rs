//! Projection rendering for the desktop adapter.

use devil_protocol::ViewportProjectionMode;
use devil_ui::{ShellProjectionSnapshot, StatusSeverity};

/// Testable display model derived only from a shell projection snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopProjectionViewModel {
    /// Window or shell title.
    pub layout_title: String,
    /// Explorer display rows.
    pub explorer_rows: Vec<String>,
    /// Active-buffer viewport or small-buffer rows.
    pub active_buffer_lines: Vec<String>,
    /// Status rows.
    pub status_rows: Vec<String>,
    /// Proposal ledger summary rows.
    pub proposal_rows: Vec<String>,
    /// Trust, privacy, permission, approval, and checkpoint rows.
    pub trust_rows: Vec<String>,
    /// Assisted-AI and delegated-task summary rows.
    pub assistant_rows: Vec<String>,
    /// Plugin contribution summary rows.
    pub plugin_rows: Vec<String>,
    /// Collaboration presence rows.
    pub collaboration_rows: Vec<String>,
    /// Empty, dirty, or degraded display flags.
    pub empty_or_degraded_flags: Vec<String>,
}

impl DesktopProjectionViewModel {
    /// Builds a display model from a projection snapshot without taking product-state ownership.
    pub fn from_snapshot(snapshot: &ShellProjectionSnapshot) -> Self {
        let mut flags = Vec::new();
        let active = &snapshot.active_buffer_projection;
        if active.dirty {
            flags.push("dirty".to_string());
        }
        if active.degraded
            || active
                .viewport
                .as_ref()
                .is_some_and(|viewport| viewport.mode == ViewportProjectionMode::DegradedLargeFile)
        {
            flags.push("degraded".to_string());
        }
        if snapshot.explorer_projection.nodes.is_empty() {
            flags.push("empty_explorer".to_string());
        }
        if active.buffer_id.is_none() {
            flags.push("no_active_buffer".to_string());
        }

        Self {
            layout_title: snapshot.layout_projection.layout.title.clone(),
            explorer_rows: explorer_rows(snapshot),
            active_buffer_lines: active_buffer_lines(snapshot),
            status_rows: status_rows(snapshot),
            proposal_rows: proposal_rows(snapshot),
            trust_rows: trust_rows(snapshot),
            assistant_rows: assistant_rows(snapshot),
            plugin_rows: plugin_rows(snapshot),
            collaboration_rows: collaboration_rows(snapshot),
            empty_or_degraded_flags: flags,
        }
    }
}

/// Renderer-owned projection view state.
#[derive(Debug, Default)]
pub struct ProjectionView {
    show_trust: bool,
    show_auxiliary: bool,
}

impl ProjectionView {
    /// Creates a projection view with no product-state ownership.
    pub fn new() -> Self {
        Self {
            show_trust: true,
            show_auxiliary: true,
        }
    }

    /// Renders the current projection snapshot into egui panels.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &ShellProjectionSnapshot,
    ) -> ProjectionViewOutput {
        let model = DesktopProjectionViewModel::from_snapshot(snapshot);

        egui::Panel::top("devil_desktop_top").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(&model.layout_title);
                for flag in &model.empty_or_degraded_flags {
                    ui.label(flag);
                }
            });
        });

        egui::Panel::left("devil_desktop_explorer")
            .resizable(true)
            .show_inside(ui, |ui| {
                ui.heading("Explorer");
                for row in &model.explorer_rows {
                    ui.label(row);
                }
            });

        egui::Panel::bottom("devil_desktop_status").show_inside(ui, |ui| {
            for row in &model.status_rows {
                ui.label(row);
            }
        });

        egui::Panel::right("devil_desktop_trust")
            .resizable(true)
            .show_inside(ui, |ui| {
                ui.checkbox(&mut self.show_trust, "Trust");
                if self.show_trust {
                    ui.heading("Proposals");
                    if model.proposal_rows.is_empty() {
                        ui.label("No proposals");
                    }
                    for row in &model.proposal_rows {
                        ui.label(row);
                    }

                    ui.separator();
                    ui.heading("Trust");
                    if model.trust_rows.is_empty() {
                        ui.label("No trust warnings");
                    }
                    for row in &model.trust_rows {
                        ui.label(row);
                    }
                }

                ui.separator();
                ui.checkbox(&mut self.show_auxiliary, "Auxiliary");
                if self.show_auxiliary {
                    for row in &model.assistant_rows {
                        ui.label(row);
                    }
                    for row in &model.plugin_rows {
                        ui.label(row);
                    }
                    for row in &model.collaboration_rows {
                        ui.label(row);
                    }
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Active Buffer");
            egui::ScrollArea::both().show(ui, |ui| {
                for row in &model.active_buffer_lines {
                    ui.monospace(row);
                }
            });
        });

        ProjectionViewOutput {
            needs_repaint: false,
            displayed_title: model.layout_title,
        }
    }
}

/// Adapter-local render output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionViewOutput {
    /// True when adapter-local animation or timing needs another paint.
    pub needs_repaint: bool,
    /// Title displayed during this render.
    pub displayed_title: String,
}

fn explorer_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    if snapshot.explorer_projection.nodes.is_empty() {
        return vec!["<empty explorer>".to_string()];
    }

    let selected = snapshot
        .explorer_projection
        .selection
        .as_ref()
        .map(|selection| selection.file_id);

    snapshot
        .explorer_projection
        .nodes
        .iter()
        .map(|node| {
            let marker = if Some(node.file_id) == selected {
                "*"
            } else {
                " "
            };
            format!("{marker} {} - {}", node.name, node.canonical_path.0)
        })
        .collect()
}

fn active_buffer_lines(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = &snapshot.active_buffer_projection;
    if active.buffer_id.is_none() {
        return vec!["<no active buffer>".to_string()];
    }

    if let Some(text) = active.small_buffer_text() {
        if text.is_empty() {
            return vec!["<empty buffer>".to_string()];
        }
        return text.lines().map(ToString::to_string).collect();
    }

    if let Some(viewport) = &active.viewport {
        if viewport.line_slices.is_empty() {
            return vec!["<empty viewport>".to_string()];
        }
        return viewport
            .line_slices
            .iter()
            .map(|line| format!("{:>4}: {}", line.line_number + 1, line.visible_text))
            .collect();
    }

    vec!["<active buffer has no visible text>".to_string()]
}

fn status_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    snapshot
        .status_messages
        .iter()
        .map(|status| {
            let severity = match status.severity {
                StatusSeverity::Info => "info",
                StatusSeverity::Warning => "warning",
                StatusSeverity::Error => "error",
            };
            format!("{severity}: {}", status.message)
        })
        .collect()
}

fn proposal_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    snapshot
        .proposal_ledger_projection
        .rows
        .iter()
        .map(|row| {
            format!(
                "proposal {}: {} [{} {:?} {:?}]",
                row.proposal_id.0, row.title, row.lifecycle.label, row.risk_label, row.privacy_label
            )
        })
        .collect()
}

fn trust_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    let manifest = &snapshot.context_manifest_projection.manifest;
    if !manifest.items.is_empty() || !manifest.permissions.is_empty() {
        rows.push(format!(
            "context manifest {}: {} items, {} permissions, egress {:?}",
            manifest.manifest_id,
            manifest.items.len(),
            manifest.permissions.len(),
            manifest.egress
        ));
    }

    let privacy = &snapshot.privacy_inspector_projection;
    if !privacy.records.is_empty() || privacy.refusal.is_some() {
        rows.push(format!(
            "privacy: {} records, {} denied, {} redacted, {} external",
            privacy.records.len(),
            privacy.denied_record_count,
            privacy.redacted_record_count,
            privacy.external_egress_record_count
        ));
    }

    let budget = &snapshot.permission_budget_projection;
    if !budget.budgets.is_empty() || !budget.evaluations.is_empty() {
        rows.push(format!(
            "permission budget: {} budgets, {} evaluations, {} refused",
            budget.budgets.len(),
            budget.evaluations.len(),
            budget.refused_evaluation_count
        ));
    }

    let checklist = &snapshot.approval_checklist_projection;
    if !checklist.gates.is_empty() || !checklist.blockers.is_empty() {
        rows.push(format!(
            "approval checklist: {} gates, ready={}",
            checklist.gates.len(),
            checklist.ready_for_approval
        ));
    }

    let rollback = &snapshot.checkpoint_rollback_projection;
    if !rollback.targets.is_empty()
        || !rollback.rollback.limitations.is_empty()
        || !rollback.checkpoint.limitations.is_empty()
    {
        rows.push(format!(
            "checkpoint rollback: {} targets, rollback {:?}",
            rollback.targets.len(),
            rollback.rollback.availability
        ));
    }

    rows
}

fn assistant_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    let assisted = &snapshot.assisted_ai_projection;
    if assisted.provider_count > 0 || assisted.request_count > 0 || assisted.refusal_count > 0 {
        rows.push(format!(
            "assisted ai: {} providers, {} requests, {} refusals, {} previews",
            assisted.provider_count,
            assisted.request_count,
            assisted.refusal_count,
            assisted.preview_ready_count
        ));
    }

    let delegated = &snapshot.delegated_task_projection;
    if delegated.plan_count > 0
        || delegated.blocked_plan_count > 0
        || delegated.refused_plan_count > 0
    {
        rows.push(format!(
            "delegated tasks: {} plans, {} blocked, runtime {:?}",
            delegated.plan_count, delegated.blocked_plan_count, delegated.runtime_activation
        ));
    }
    rows
}

fn plugin_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    snapshot
        .plugin_contribution_projections
        .iter()
        .map(|projection| {
            format!(
                "plugin {}: {} contributions, {}",
                projection.plugin_id.0,
                projection.contributions.len(),
                projection.status_label
            )
        })
        .collect()
}

fn collaboration_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    snapshot
        .collaboration_presence_projections
        .iter()
        .map(|presence| {
            format!(
                "collaboration {} participant {} reconnecting={}",
                presence.session_id.0, presence.participant_id.0, presence.reconnecting
            )
        })
        .collect()
}
