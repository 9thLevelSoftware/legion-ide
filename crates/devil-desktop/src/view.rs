//! Projection rendering for the desktop adapter.

use std::collections::{BTreeSet, HashSet};

use devil_protocol::{FileId, ViewportProjectionMode};
use devil_ui::{ShellProjectionSnapshot, StatusSeverity};

use crate::{bridge::DesktopAction, search::DesktopSearchViewModel};

/// Adapter-local view state layered over app-owned projections.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DesktopProjectionViewState {
    /// Canonical explorer paths currently expanded by the adapter.
    pub expanded_explorer_paths: BTreeSet<String>,
    /// Adapter-local explorer selection override, if a native control is ahead of projection.
    pub selected_explorer_file: Option<FileId>,
}

/// Testable display model derived only from a shell projection snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopProjectionViewModel {
    /// Window or shell title.
    pub layout_title: String,
    /// Tab-strip display rows.
    pub tab_rows: Vec<String>,
    /// Explorer display rows.
    pub explorer_rows: Vec<String>,
    /// Explorer state rows with selection and expansion markers.
    pub explorer_state_rows: Vec<String>,
    /// Active-buffer viewport or small-buffer rows.
    pub active_buffer_lines: Vec<String>,
    /// Active editor metadata rows.
    pub editor_status_rows: Vec<String>,
    /// Dirty-close prompt rows.
    pub close_prompt_rows: Vec<String>,
    /// Per-buffer viewport metadata rows.
    pub viewport_metadata_rows: Vec<String>,
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
        Self::from_snapshot_with_state(snapshot, &DesktopProjectionViewState::default())
    }

    /// Builds a display model from a projection snapshot plus adapter-local view state.
    pub fn from_snapshot_with_state(
        snapshot: &ShellProjectionSnapshot,
        state: &DesktopProjectionViewState,
    ) -> Self {
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
            tab_rows: tab_rows(snapshot),
            explorer_rows: explorer_rows(snapshot, state),
            explorer_state_rows: explorer_rows(snapshot, state),
            active_buffer_lines: active_buffer_lines(snapshot),
            editor_status_rows: editor_status_rows(snapshot),
            close_prompt_rows: close_prompt_rows(snapshot),
            viewport_metadata_rows: viewport_metadata_rows(snapshot),
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
        self.render_with_state(ui, snapshot, &DesktopProjectionViewState::default())
    }

    /// Renders the current projection snapshot with adapter-local expansion state.
    pub fn render_with_state(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &ShellProjectionSnapshot,
        state: &DesktopProjectionViewState,
    ) -> ProjectionViewOutput {
        let model = DesktopProjectionViewModel::from_snapshot_with_state(snapshot, state);
        let mut actions = Vec::new();

        egui::Panel::top("devil_desktop_top").show_inside(ui, |ui| {
            ui.vertical(|ui| {
                ui.heading(&model.layout_title);
                ui.horizontal_wrapped(|ui| {
                    for flag in &model.empty_or_degraded_flags {
                        ui.label(flag);
                    }
                });
                ui.horizontal_wrapped(|ui| render_tab_controls(ui, snapshot, &mut actions));
            });
        });

        egui::Panel::left("devil_desktop_explorer")
            .resizable(true)
            .show_inside(ui, |ui| {
                ui.heading("Explorer");
                render_explorer_controls(ui, snapshot, state, &mut actions);
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
            for row in &model.editor_status_rows {
                ui.label(row);
            }
            for row in &model.viewport_metadata_rows {
                ui.label(row);
            }
            for row in &model.close_prompt_rows {
                ui.label(row);
            }
            egui::ScrollArea::both().show(ui, |ui| {
                for row in &model.active_buffer_lines {
                    ui.monospace(row);
                }
            });
            ui.separator();
            render_search_projection(ui, snapshot);
        });

        ProjectionViewOutput {
            needs_repaint: false,
            displayed_title: model.layout_title,
            actions,
        }
    }
}

fn render_search_projection(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    let search = DesktopSearchViewModel::from_projection(&snapshot.search_projection);
    ui.heading("Search");
    ui.label(search.header);
    for row in &search.status_rows {
        ui.label(row);
    }
    if search.result_rows.is_empty() {
        ui.label("<no search results>");
    } else {
        for row in &search.result_rows {
            ui.monospace(row);
        }
    }
    for row in &search.diagnostic_rows {
        ui.label(row);
    }
}

/// Adapter-local render output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionViewOutput {
    /// True when adapter-local animation or timing needs another paint.
    pub needs_repaint: bool,
    /// Title displayed during this render.
    pub displayed_title: String,
    /// Adapter actions requested by rendered controls.
    pub actions: Vec<DesktopAction>,
}

fn render_tab_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    if ui.button("Save All").clicked() {
        actions.push(DesktopAction::SaveAll);
    }

    let tabs = &snapshot.daily_editing_projection.tabs.tabs;
    if tabs.is_empty() {
        ui.label("<no open tabs>");
        return;
    }

    for tab in tabs {
        let title = if tab.dirty {
            format!("{} +", tab.title)
        } else {
            tab.title.clone()
        };
        if ui.selectable_label(tab.active, title).clicked() {
            actions.push(DesktopAction::SwitchTab {
                buffer_id: tab.buffer_id,
            });
        }
        if ui.button("x").clicked() {
            actions.push(DesktopAction::CloseTab {
                buffer_id: tab.buffer_id,
            });
        }
    }
}

fn render_explorer_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    actions: &mut Vec<DesktopAction>,
) {
    if snapshot.explorer_projection.nodes.is_empty() {
        ui.label("<empty explorer>");
        return;
    }

    let selected = state.selected_explorer_file.or_else(|| {
        snapshot
            .explorer_projection
            .selection
            .as_ref()
            .map(|selection| selection.file_id)
    });
    for node in top_level_explorer_nodes(&snapshot.explorer_projection.nodes) {
        render_explorer_node(
            ui,
            node,
            &snapshot.explorer_projection.nodes,
            0,
            selected,
            state,
            actions,
        );
    }
}

fn render_explorer_node(
    ui: &mut egui::Ui,
    node: &devil_ui::ExplorerNodeProjection,
    nodes: &[devil_ui::ExplorerNodeProjection],
    depth: usize,
    selected: Option<FileId>,
    state: &DesktopProjectionViewState,
    actions: &mut Vec<DesktopAction>,
) {
    let is_expanded = state
        .expanded_explorer_paths
        .contains(&node.canonical_path.0);
    ui.horizontal(|ui| {
        ui.add_space((depth as f32) * 12.0);
        if !node.children.is_empty() {
            let marker = if is_expanded { "v" } else { ">" };
            if ui.button(marker).clicked() {
                actions.push(DesktopAction::ToggleExplorerPath {
                    path: node.canonical_path.0.clone(),
                });
            }
        } else {
            ui.label("-");
        }
        if ui
            .selectable_label(Some(node.file_id) == selected, &node.name)
            .clicked()
        {
            actions.push(DesktopAction::SelectExplorerFile {
                file_id: node.file_id,
            });
        }
    });

    if is_expanded {
        for child_id in &node.children {
            if let Some(child) = nodes
                .iter()
                .find(|candidate| candidate.file_id == *child_id)
            {
                render_explorer_node(ui, child, nodes, depth + 1, selected, state, actions);
            }
        }
    }
}

fn tab_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let tabs = &snapshot.daily_editing_projection.tabs.tabs;
    if tabs.is_empty() {
        return vec!["<no open tabs>".to_string()];
    }

    tabs.iter()
        .map(|tab| {
            let active = if tab.active { "*" } else { " " };
            let dirty = if tab.dirty { " +" } else { "" };
            let pinned = if tab.pinned { " pinned" } else { "" };
            let preview = if tab.preview { " preview" } else { "" };
            let path = tab
                .file_path
                .as_ref()
                .map(|path| path.0.as_str())
                .unwrap_or("<untitled>");
            format!(
                "{active} {}{} [buffer {}] {path}{pinned}{preview}",
                tab.title, dirty, tab.buffer_id.0
            )
        })
        .collect()
}

fn explorer_rows(
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
) -> Vec<String> {
    if snapshot.explorer_projection.nodes.is_empty() {
        return vec!["<empty explorer>".to_string()];
    }

    let selected = state.selected_explorer_file.or_else(|| {
        snapshot
            .explorer_projection
            .selection
            .as_ref()
            .map(|selection| selection.file_id)
    });

    let mut rows = Vec::new();
    for node in top_level_explorer_nodes(&snapshot.explorer_projection.nodes) {
        push_explorer_row(
            &mut rows,
            node,
            &snapshot.explorer_projection.nodes,
            0,
            selected,
            &state.expanded_explorer_paths,
        );
    }

    rows
}

fn top_level_explorer_nodes(
    nodes: &[devil_ui::ExplorerNodeProjection],
) -> Vec<&devil_ui::ExplorerNodeProjection> {
    let child_ids = nodes
        .iter()
        .flat_map(|node| node.children.iter().copied())
        .collect::<HashSet<_>>();
    nodes
        .iter()
        .filter(|node| !child_ids.contains(&node.file_id))
        .collect()
}

fn push_explorer_row(
    rows: &mut Vec<String>,
    node: &devil_ui::ExplorerNodeProjection,
    nodes: &[devil_ui::ExplorerNodeProjection],
    depth: usize,
    selected: Option<FileId>,
    expanded: &BTreeSet<String>,
) {
    let selection_marker = if Some(node.file_id) == selected {
        "*"
    } else {
        " "
    };
    let is_expanded = expanded.contains(&node.canonical_path.0);
    let expansion_marker = if node.children.is_empty() {
        "-"
    } else if is_expanded {
        "v"
    } else {
        ">"
    };
    let indent = "  ".repeat(depth);
    rows.push(format!(
        "{selection_marker} {expansion_marker} {indent}{} - {}",
        node.name, node.canonical_path.0
    ));

    if is_expanded {
        for child_id in &node.children {
            if let Some(child) = nodes
                .iter()
                .find(|candidate| candidate.file_id == *child_id)
            {
                push_explorer_row(rows, child, nodes, depth + 1, selected, expanded);
            }
        }
    }
}

fn active_buffer_lines(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = &snapshot.active_buffer_projection;
    if active.buffer_id.is_none() {
        return vec!["<no active buffer>".to_string()];
    }

    if !active.degraded {
        if let Some(text) = active.small_buffer_text() {
            if text.is_empty() {
                return vec!["<empty buffer>".to_string()];
            }
            return text.lines().map(ToString::to_string).collect();
        }
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

fn editor_status_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = &snapshot.active_buffer_projection;
    let Some(buffer_id) = active.buffer_id else {
        return vec!["editor: no active buffer".to_string()];
    };

    let path = active
        .file_path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<untitled>");
    let dirty = if active.dirty { "dirty" } else { "clean" };
    let mode = if active.degraded {
        "DegradedLargeFile"
    } else if active.viewport.is_some() {
        "viewport"
    } else {
        "small-buffer"
    };

    vec![format!(
        "editor: buffer {} {dirty} {mode} path={path}",
        buffer_id.0
    )]
}

fn close_prompt_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let Some(prompt) = &snapshot.daily_editing_projection.close_dirty_prompt else {
        return Vec::new();
    };

    let path = prompt
        .file_path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<untitled>");
    vec![format!(
        "close_dirty buffer {} {} path={path}: {}",
        prompt.buffer_id.0, prompt.title, prompt.message
    )]
}

fn viewport_metadata_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let states = &snapshot.daily_editing_projection.viewport_states;
    if states.is_empty() {
        if let Some(viewport) = &snapshot.active_buffer_projection.viewport {
            return vec![format!(
                "viewport buffer {} cursor={} selections={} scroll={}:{} mode={:?}",
                viewport.buffer_id.0,
                coordinate_label(&viewport.cursor),
                viewport.selections.len(),
                viewport.scroll.top_line,
                viewport.scroll.left_column,
                viewport.mode
            )];
        }
        return vec!["<no viewport state>".to_string()];
    }

    states
        .iter()
        .map(|state| {
            let cursor = state
                .cursor
                .as_ref()
                .map(coordinate_label)
                .unwrap_or_else(|| "<none>".to_string());
            format!(
                "viewport buffer {} cursor={} selections={} scroll={}:{}",
                state.buffer_id.0,
                cursor,
                state.selections.len(),
                state.scroll.top_line,
                state.scroll.left_column
            )
        })
        .collect()
}

fn coordinate_label(coordinate: &devil_protocol::TextCoordinate) -> String {
    format!("{}:{}", coordinate.line, coordinate.character)
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
                row.proposal_id.0,
                row.title,
                row.lifecycle.label,
                row.risk_label,
                row.privacy_label
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
