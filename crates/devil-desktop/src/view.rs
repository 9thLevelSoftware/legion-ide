//! Projection rendering for the desktop adapter.

use std::collections::{BTreeSet, HashSet};

use devil_protocol::{
    AgentRunId, CollaborationParticipantId, CollaborationSessionId,
    DelegatedTaskRuntimeActivationState, FileId, PluginCommandDescriptor, PluginContribution,
    PluginContributionProjection, ProposalCancellationReason, ProposalRejectionReason,
    ProposalRollbackReason, ViewportProjectionMode,
};
use devil_ui::{ShellProjectionSnapshot, StatusSeverity};

use crate::{
    bridge::DesktopAction, health::DesktopOperationalHealthSnapshot,
    search::DesktopSearchViewModel, theme,
};

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
    /// Top command-bar rows.
    pub top_bar_rows: Vec<String>,
    /// Read-only autonomy scale rows.
    pub autonomy_rows: Vec<String>,
    /// Left sidebar summary rows.
    pub left_sidebar_rows: Vec<String>,
    /// Main code-canvas summary rows.
    pub main_canvas_rows: Vec<String>,
    /// Right directive and trust console summary rows.
    pub right_console_rows: Vec<String>,
    /// Bottom operational console rows.
    pub bottom_console_rows: Vec<String>,
    /// Compact status-bar rows.
    pub status_bar_rows: Vec<String>,
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
    /// Language tooling summary rows.
    pub language_rows: Vec<String>,
    /// Terminal panel summary rows.
    pub terminal_rows: Vec<String>,
    /// Operational health summary rows.
    pub operational_health_rows: Vec<String>,
    /// Plugin contribution summary rows.
    pub plugin_rows: Vec<String>,
    /// Collaboration presence rows.
    pub collaboration_rows: Vec<String>,
    /// Remote workspace manager rows.
    pub remote_rows: Vec<String>,
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

        let autonomy_rows = autonomy_rows(snapshot);
        Self {
            layout_title: snapshot.layout_projection.layout.title.clone(),
            top_bar_rows: top_bar_rows(snapshot, &flags),
            autonomy_rows,
            left_sidebar_rows: left_sidebar_rows(snapshot),
            main_canvas_rows: main_canvas_rows(snapshot),
            right_console_rows: right_console_rows(snapshot),
            bottom_console_rows: bottom_console_rows(snapshot),
            status_bar_rows: status_bar_rows(snapshot, &flags),
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
            language_rows: language_rows(snapshot),
            terminal_rows: terminal_rows(snapshot),
            operational_health_rows: operational_health_rows(snapshot),
            plugin_rows: plugin_rows(snapshot),
            collaboration_rows: collaboration_rows(snapshot),
            remote_rows: remote_rows(snapshot),
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
        theme::install(ui.ctx());
        let model = DesktopProjectionViewModel::from_snapshot_with_state(snapshot, state);
        let mut actions = Vec::new();

        egui::Panel::top("devil_desktop_top")
            .exact_size(76.0)
            .frame(theme::panel_frame(theme::BG_BASE))
            .show_inside(ui, |ui| {
                render_top_command_bar(ui, snapshot, &model, &mut actions);
            });

        egui::Panel::left("devil_desktop_explorer")
            .default_size(286.0)
            .min_size(220.0)
            .resizable(true)
            .frame(theme::panel_frame(theme::BG_BASE))
            .show_inside(ui, |ui| {
                render_left_sidebar(ui, snapshot, state, &model, &mut actions);
            });

        egui::Panel::bottom("devil_desktop_status")
            .exact_size(28.0)
            .frame(theme::panel_frame(theme::BG_CODE))
            .show_inside(ui, |ui| {
                render_status_bar(ui, &model);
            });

        egui::Panel::bottom("devil_desktop_bottom_console")
            .default_size(208.0)
            .min_size(112.0)
            .resizable(true)
            .frame(theme::panel_frame(theme::BG_CODE))
            .show_inside(ui, |ui| {
                render_bottom_console(ui, &model);
            });

        egui::Panel::right("devil_desktop_trust")
            .default_size(392.0)
            .min_size(300.0)
            .resizable(true)
            .frame(theme::panel_frame(theme::BG_BASE))
            .show_inside(ui, |ui| {
                render_right_console(
                    ui,
                    snapshot,
                    &model,
                    &mut self.show_trust,
                    &mut self.show_auxiliary,
                    &mut actions,
                );
            });

        egui::CentralPanel::default()
            .frame(theme::panel_frame(theme::BG_CODE))
            .show_inside(ui, |ui| {
                render_code_canvas(ui, snapshot, &model, &mut actions);
            });

        ProjectionViewOutput {
            needs_repaint: false,
            displayed_title: model.layout_title,
            actions,
        }
    }
}

fn render_top_command_bar(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::heading("Devil IDE"));
        ui.separator();
        ui.label(theme::body(&model.layout_title));
        if ui
            .button(theme::accent("Save All", theme::ACCENT_CYAN))
            .clicked()
        {
            actions.push(DesktopAction::SaveAll);
        }
        if ui.button(theme::body("Search")).clicked() {
            actions.push(DesktopAction::ShowSearchPrompt {
                scope: snapshot.search_projection.scope,
            });
        }
        if ui.button(theme::body("Open")).clicked() {
            actions.push(DesktopAction::ShowOpenPathPrompt);
        }
        ui.separator();
        render_autonomy_scale(ui, model);
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        render_shell_rows(ui, &model.top_bar_rows);
    });
}

fn render_autonomy_scale(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    let active = model
        .autonomy_rows
        .first()
        .map(|row| row.as_str())
        .unwrap_or("autonomy scale: active=L1 Manual read-only projection");
    theme::card_frame().show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::eyebrow("Autonomy"));
            for (level, label, color) in [
                ("L1", "Manual", theme::TEXT_MUTED),
                ("L2", "Assisted", theme::ACCENT_CYAN),
                ("L3", "Co-Pilot", theme::ACCENT_BLUE),
                ("L4", "Delegated", theme::ACCENT_VIOLET),
                ("L5", "Fleet", theme::ACCENT_PURPLE),
            ] {
                let selected = active.contains(level);
                let text = format!("{level} {label}");
                if selected {
                    ui.label(theme::accent(text, color));
                } else {
                    ui.label(theme::muted(text));
                }
            }
        });
    });
}

fn render_left_sidebar(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    ui.label(theme::eyebrow("Project"));
    theme::card_frame().show(ui, |ui| {
        render_shell_rows(ui, &model.left_sidebar_rows);
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(theme::heading("Explorer"));
        if ui.button(theme::muted("Refresh")).clicked() {
            actions.push(DesktopAction::RefreshExplorer);
        }
    });
    egui::ScrollArea::vertical()
        .id_salt("devil_desktop_explorer_scroll")
        .max_height(240.0)
        .show(ui, |ui| {
            render_explorer_controls(ui, snapshot, state, actions);
        });

    ui.add_space(6.0);
    ui.label(theme::eyebrow("Active Fleet"));
    theme::card_frame().show(ui, |ui| {
        if model.assistant_rows.is_empty() {
            ui.label(theme::muted("No projected assistant or delegated activity"));
        } else {
            for row in model.assistant_rows.iter().take(5) {
                ui.label(theme::body(row));
            }
        }
    });
}

fn render_code_canvas(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::heading("Code Canvas"));
        render_tab_controls(ui, snapshot, actions);
    });
    ui.add_space(4.0);
    theme::card_frame().show(ui, |ui| {
        render_shell_rows(ui, &model.main_canvas_rows);
        for row in &model.editor_status_rows {
            ui.label(theme::body(row));
        }
        for row in &model.viewport_metadata_rows {
            ui.label(theme::muted(row));
        }
        for row in &model.close_prompt_rows {
            ui.label(theme::accent(row, theme::ACCENT_AMBER));
        }
        render_close_dirty_prompt_controls(ui, snapshot, actions);
    });

    ui.add_space(6.0);
    theme::panel_frame(theme::BG_CODE).show(ui, |ui| {
        egui::ScrollArea::both()
            .id_salt("devil_desktop_code_canvas_scroll")
            .show(ui, |ui| {
                for row in &model.active_buffer_lines {
                    ui.monospace(row);
                }
            });
    });

    ui.separator();
    render_search_projection(ui, snapshot);
}

fn render_right_console(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    show_trust: &mut bool,
    show_auxiliary: &mut bool,
    actions: &mut Vec<DesktopAction>,
) {
    ui.label(theme::heading("Directive Console"));
    theme::card_frame().show(ui, |ui| {
        render_shell_rows(ui, &model.right_console_rows);
        for row in &model.autonomy_rows {
            ui.label(theme::muted(row));
        }
    });

    ui.add_space(6.0);
    ui.checkbox(show_trust, "Trust and proposals");
    if *show_trust {
        render_console_section(ui, "Approval Queue", &model.proposal_rows, "No proposals");
        render_proposal_controls(ui, snapshot, actions);
        render_console_section(ui, "Trust", &model.trust_rows, "No trust warnings");
    }

    ui.separator();
    ui.checkbox(show_auxiliary, "Auxiliary surfaces");
    if *show_auxiliary {
        render_console_section(
            ui,
            "Language",
            &model.language_rows,
            "No language tooling activity",
        );
        render_console_section(ui, "Terminal", &model.terminal_rows, "No terminal activity");
        render_console_section(
            ui,
            "Operational Health",
            &model.operational_health_rows,
            "No health rows",
        );
        render_console_section(
            ui,
            "Assistant and Delegated Tasks",
            &model.assistant_rows,
            "No assistant activity",
        );
        render_assistant_controls(ui, snapshot, actions);
        render_console_section(
            ui,
            "Plugin Management",
            &model.plugin_rows,
            "No plugin contributions",
        );
        render_plugin_management_controls(ui, snapshot, actions);
        render_console_section(
            ui,
            "Collaboration",
            &model.collaboration_rows,
            "No collaboration rows",
        );
        render_collaboration_controls(ui, snapshot, actions);
        render_console_section(ui, "Remote Workspace", &model.remote_rows, "No remote rows");
        render_remote_workspace_controls(ui, snapshot, actions);
    }
}

fn render_bottom_console(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::heading("Bottom Console"));
        ui.label(theme::accent("Terminal", theme::ACCENT_CYAN));
        ui.label(theme::accent("Tests", theme::ACCENT_GREEN));
        ui.label(theme::accent("Workflow Logs", theme::ACCENT_AMBER));
        ui.label(theme::accent("Agent Stream", theme::ACCENT_VIOLET));
    });
    ui.add_space(4.0);
    ui.columns(2, |columns| {
        theme::card_frame().show(&mut columns[0], |ui| {
            ui.label(theme::eyebrow("Terminal / Runtime"));
            render_shell_rows(ui, &model.bottom_console_rows);
            for row in model.terminal_rows.iter().take(6) {
                ui.label(theme::body(row));
            }
        });
        theme::card_frame().show(&mut columns[1], |ui| {
            ui.label(theme::eyebrow("Workflow / Health"));
            for row in model.operational_health_rows.iter().take(8) {
                ui.label(theme::body(row));
            }
            for row in model.status_rows.iter().take(4) {
                ui.label(theme::muted(row));
            }
        });
    });
}

fn render_status_bar(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    ui.horizontal_wrapped(|ui| {
        for row in &model.status_bar_rows {
            ui.label(theme::muted(row));
        }
    });
}

fn render_console_section(ui: &mut egui::Ui, title: &str, rows: &[String], empty: &str) {
    ui.add_space(5.0);
    ui.label(theme::eyebrow(title));
    theme::card_frame().show(ui, |ui| {
        if rows.is_empty() {
            ui.label(theme::muted(empty));
        } else {
            for row in rows.iter().take(8) {
                ui.label(theme::body(row));
            }
            if rows.len() > 8 {
                ui.label(theme::muted(format!("{} more rows", rows.len() - 8)));
            }
        }
    });
}

fn render_shell_rows(ui: &mut egui::Ui, rows: &[String]) {
    for row in rows {
        ui.label(theme::body(row));
    }
}

fn render_search_projection(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    let search = DesktopSearchViewModel::from_projection(&snapshot.search_projection);
    ui.label(theme::heading("Search"));
    ui.label(theme::body(search.header));
    for row in &search.status_rows {
        ui.label(theme::body(row));
    }
    if search.result_rows.is_empty() {
        ui.label(theme::muted("<no search results>"));
    } else {
        for row in &search.result_rows {
            ui.monospace(row);
        }
    }
    for row in &search.diagnostic_rows {
        ui.label(theme::muted(row));
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopAutonomyLevel {
    Manual,
    Assisted,
    Copilot,
    Delegated,
    Fleet,
}

impl DesktopAutonomyLevel {
    fn number(self) -> u8 {
        match self {
            Self::Manual => 1,
            Self::Assisted => 2,
            Self::Copilot => 3,
            Self::Delegated => 4,
            Self::Fleet => 5,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Assisted => "Assisted",
            Self::Copilot => "Co-Pilot",
            Self::Delegated => "Delegated",
            Self::Fleet => "Fleet",
        }
    }
}

fn projected_autonomy_level(snapshot: &ShellProjectionSnapshot) -> DesktopAutonomyLevel {
    let delegated = &snapshot.delegated_task_projection;
    if delegated.runtime_activation != DelegatedTaskRuntimeActivationState::NotEncoded
        && delegated.plan_count > 0
    {
        return DesktopAutonomyLevel::Fleet;
    }
    if delegated.plan_count > 0
        || !delegated.plan_rows.is_empty()
        || !delegated.required_approvals.is_empty()
        || !delegated.proposal_preview_links.is_empty()
    {
        return DesktopAutonomyLevel::Delegated;
    }

    let assisted = &snapshot.assisted_ai_projection;
    if assisted.request_count > 0
        || assisted.preview_ready_count > 0
        || !assisted.proposal_previews.is_empty()
    {
        return DesktopAutonomyLevel::Copilot;
    }
    if assisted.provider_count > 0 || !assisted.providers.is_empty() {
        return DesktopAutonomyLevel::Assisted;
    }

    DesktopAutonomyLevel::Manual
}

fn autonomy_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let level = projected_autonomy_level(snapshot);
    let delegated = &snapshot.delegated_task_projection;
    let mut rows = vec![
        format!(
            "autonomy scale: active=L{} {} read-only projection",
            level.number(),
            level.label()
        ),
        "autonomy scale levels: L1 Manual | L2 Assisted | L3 Co-Pilot | L4 Delegated | L5 Fleet"
            .to_string(),
    ];

    if level == DesktopAutonomyLevel::Delegated {
        rows.push(
            "autonomy safety: delegated work is approval-gated; autonomous apply unsupported"
                .to_string(),
        );
    } else if level == DesktopAutonomyLevel::Fleet {
        rows.push(format!(
            "autonomy safety: fleet display from runtime={:?}; autonomous apply remains proposal-mediated",
            delegated.runtime_activation
        ));
    } else {
        rows.push("autonomy safety: mode switching is not implemented in the renderer".to_string());
    }
    rows.push(
        "autonomy control: display-only; no provider, terminal, or apply authority".to_string(),
    );
    rows
}

fn top_bar_rows(snapshot: &ShellProjectionSnapshot, flags: &[String]) -> Vec<String> {
    let active = &snapshot.active_buffer_projection;
    let workspace = active
        .workspace_id
        .map(|workspace| workspace.0.to_string())
        .unwrap_or_else(|| "none".to_string());
    let buffer = active
        .buffer_id
        .map(|buffer| buffer.0.to_string())
        .unwrap_or_else(|| "none".to_string());
    let status = if flags.is_empty() {
        "steady".to_string()
    } else {
        flags.join(",")
    };
    vec![
        format!(
            "command bar: {} workspace={} buffer={} status={}",
            snapshot.layout_projection.layout.title, workspace, buffer, status
        ),
        format!(
            "command affordance: search={} save_all={} proposal_controls={}",
            snapshot.search_projection.results.len(),
            snapshot.daily_editing_projection.tabs.tabs.len(),
            snapshot.proposal_ledger_projection.rows.len()
        ),
        format!(
            "build/status summary: messages={} language_ops={} terminal_rows={}",
            snapshot.status_messages.len(),
            snapshot.language_tooling_projection.operations.len(),
            snapshot.terminal_panel_projection.output_rows.len()
        ),
    ]
}

fn left_sidebar_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let selected = snapshot
        .explorer_projection
        .selection
        .as_ref()
        .map(|selection| selection.file_id.0.to_string())
        .unwrap_or_else(|| "none".to_string());
    vec![
        format!(
            "project sidebar: explorer_nodes={} selected_file={}",
            snapshot.explorer_projection.nodes.len(),
            selected
        ),
        format!(
            "active fleet summary: assisted_providers={} delegated_plans={} plugin_surfaces={}",
            snapshot.assisted_ai_projection.provider_count,
            snapshot.delegated_task_projection.plan_count,
            snapshot.plugin_contribution_projections.len()
        ),
        format!(
            "context packs: collaboration_sessions={} remote_sessions={}",
            snapshot.collaboration_gui_projection.session_rows.len(),
            snapshot.remote_gui_projection.session_rows.len()
        ),
    ]
}

fn main_canvas_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = &snapshot.active_buffer_projection;
    let path = active
        .file_path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<no file>");
    let search = DesktopSearchViewModel::from_projection(&snapshot.search_projection);
    vec![
        format!(
            "code canvas: tabs={} active_path={} dirty={} degraded={}",
            snapshot.daily_editing_projection.tabs.tabs.len(),
            path,
            active.dirty,
            active.degraded
        ),
        format!(
            "language cues: status={:?} problems={} completions={} definitions={} references={}",
            snapshot.language_tooling_projection.status,
            snapshot.language_tooling_projection.problems.len(),
            snapshot.language_tooling_projection.completions.len(),
            snapshot.language_tooling_projection.definitions.len(),
            snapshot.language_tooling_projection.references.len()
        ),
        format!("search strip: {}", search.header),
    ]
}

fn right_console_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    vec![
        format!(
            "directive console: proposals={} trust_items={} approval_gates={} proposal-mediated",
            snapshot.proposal_ledger_projection.rows.len(),
            snapshot.context_manifest_projection.manifest.items.len(),
            snapshot.approval_checklist_projection.gates.len()
        ),
        format!(
            "assistant console: requests={} refusals={} previews={}",
            snapshot.assisted_ai_projection.request_count,
            snapshot.assisted_ai_projection.refusal_count,
            snapshot.assisted_ai_projection.preview_ready_count
        ),
        format!(
            "advanced surfaces: delegated={} plugins={} collaboration={} remote={}",
            snapshot.delegated_task_projection.plan_count,
            snapshot.plugin_contribution_projections.len(),
            snapshot.collaboration_gui_projection.session_rows.len(),
            snapshot.remote_gui_projection.session_rows.len()
        ),
    ]
}

fn bottom_console_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let health_rows = DesktopOperationalHealthSnapshot::from_projection(snapshot).rows();
    vec![
        format!(
            "bottom console: terminal_status={:?} terminal_rows={} omitted={}",
            snapshot.terminal_panel_projection.status.kind,
            snapshot.terminal_panel_projection.output_rows.len(),
            snapshot
                .terminal_panel_projection
                .scrollback
                .omitted_row_count
        ),
        format!(
            "workflow activity: status_messages={} health_rows={} audit=metadata-only",
            snapshot.status_messages.len(),
            health_rows.len()
        ),
        format!(
            "agent stream: assisted_requests={} delegated_steps={} shared_reviews={} remote_reviews={}",
            snapshot.assisted_ai_projection.request_count,
            snapshot.delegated_task_projection.step_summaries.len(),
            snapshot
                .collaboration_gui_projection
                .shared_proposal_rows
                .len(),
            snapshot.remote_gui_projection.proposal_review_rows.len()
        ),
    ]
}

fn status_bar_rows(snapshot: &ShellProjectionSnapshot, flags: &[String]) -> Vec<String> {
    let active = &snapshot.active_buffer_projection;
    let path = active
        .file_path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<none>");
    let status = if flags.is_empty() {
        "clean".to_string()
    } else {
        flags.join(",")
    };
    vec![
        format!("status bar: flags={} path={}", status, path),
        format!(
            "status bar metadata: workspace={:?} file={:?} buffer={:?}",
            active.workspace_id.map(|workspace| workspace.0),
            active.file_id.map(|file| file.0),
            active.buffer_id.map(|buffer| buffer.0)
        ),
    ]
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

fn render_close_dirty_prompt_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let Some(prompt) = &snapshot.daily_editing_projection.close_dirty_prompt else {
        return;
    };
    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            actions.push(DesktopAction::SaveDirtyClose {
                buffer_id: prompt.buffer_id,
            });
        }
        if ui.button("Cancel").clicked() {
            actions.push(DesktopAction::CancelDirtyClose {
                buffer_id: prompt.buffer_id,
            });
        }
    });
}

fn render_proposal_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    for row in &snapshot.proposal_ledger_projection.rows {
        let proposal_id = row.proposal_id;
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Proposal {}", proposal_id.0));
            if ui.button("Details").clicked() {
                actions.push(DesktopAction::OpenProposalDetails { proposal_id });
            }
            if ui.button("Preview").clicked() {
                actions.push(DesktopAction::PreviewProposal { proposal_id });
            }
            if ui.button("Approve").clicked() {
                actions.push(DesktopAction::ApproveProposal { proposal_id });
            }
            if ui.button("Apply").clicked() {
                actions.push(DesktopAction::ApplyProposal { proposal_id });
            }
            if ui.button("Reject").clicked() {
                actions.push(DesktopAction::RejectProposal {
                    proposal_id,
                    reason: ProposalRejectionReason::UserRejected,
                });
            }
            if ui.button("Cancel").clicked() {
                actions.push(DesktopAction::CancelProposal {
                    proposal_id,
                    reason: ProposalCancellationReason::UserCancelled,
                });
            }
            if ui.button("Rollback").clicked() {
                actions.push(DesktopAction::RollbackProposal {
                    proposal_id,
                    reason: ProposalRollbackReason::UserRequested,
                });
            }
        });
    }
}

fn render_assistant_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    if ui.button("Explain").clicked() {
        actions.push(DesktopAction::StartAiExplain {
            instruction_label: "desktop explain".to_string(),
        });
    }
    if ui.button("Propose").clicked() {
        actions.push(DesktopAction::StartAiProposal {
            instruction_label: "desktop propose".to_string(),
        });
    }

    if let Some(run_id) = projected_assisted_run_id(snapshot) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Run {}", run_id.0));
            if ui.button("Inspect").clicked() {
                actions.push(DesktopAction::InspectAiRun {
                    run_id: run_id.clone(),
                });
            }
            if ui.button("Replay").clicked() {
                actions.push(DesktopAction::ReplayAiRun {
                    run_id: run_id.clone(),
                });
            }
            if ui.button("Cancel").clicked() {
                actions.push(DesktopAction::CancelAiRun { run_id });
            }
        });
    }

    render_delegated_task_controls(ui, snapshot, actions);
}

fn render_delegated_task_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let delegated = &snapshot.delegated_task_projection;
    for row in &delegated.plan_rows {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Delegated plan {}", row.plan_id.0));
            if ui.button("Inspect").clicked() {
                actions.push(DesktopAction::InspectDelegatedTaskPlan {
                    plan_id: row.plan_id.clone(),
                });
            }
        });
    }
    for link in &delegated.proposal_preview_links {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Delegated proposal {}", link.proposal_id.0));
            if ui.button("Preview").clicked() {
                actions.push(DesktopAction::OpenDelegatedProposalPreview {
                    proposal_id: link.proposal_id,
                });
            }
            if ui.button("Details").clicked() {
                actions.push(DesktopAction::OpenDelegatedProposalDetails {
                    proposal_id: link.proposal_id,
                });
            }
        });
    }
}

fn render_plugin_management_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    if snapshot.plugin_contribution_projections.is_empty() {
        ui.label("No plugin contributions");
        return;
    }

    for projection in &snapshot.plugin_contribution_projections {
        for command in plugin_command_descriptors(projection) {
            let label = format!("{} ({})", command.title, command.command_id);
            if ui.button(label).clicked() {
                actions.push(DesktopAction::InvokePluginCommand {
                    plugin_id: projection.plugin_id,
                    command_id: command.command_id.clone(),
                });
            }
        }
    }
}

fn render_collaboration_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let projection = &snapshot.collaboration_gui_projection;
    for session in &projection.session_rows {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Session {}", session.session_id.0));
            if ui.button("Leave").clicked() {
                actions.push(DesktopAction::LeaveCollaborationSession {
                    session_id: session.session_id,
                });
            }
            if let Some(participant_id) =
                first_collaboration_participant(snapshot, session.session_id)
                && ui.button("Presence").clicked()
            {
                actions.push(DesktopAction::PublishCollaborationPresence {
                    session_id: session.session_id,
                    participant_id,
                });
            }
        });
    }
    for review in &projection.shared_proposal_rows {
        if ui
            .button(format!("Review shared proposal {}", review.proposal_id.0))
            .clicked()
        {
            actions.push(DesktopAction::OpenSharedProposalReview {
                session_id: review.session_id,
                proposal_id: review.proposal_id,
            });
        }
    }
}

fn render_remote_workspace_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let projection = &snapshot.remote_gui_projection;
    for session in &projection.session_rows {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Remote session {}", session.session_id.0));
            if (session.reconnecting || session.offline) && ui.button("Reconnect").clicked() {
                actions.push(DesktopAction::ConnectRemoteWorkspace {
                    session_id: session.session_id,
                    authority_label: session.authority_label.clone(),
                });
            }
        });
    }
    for review in &projection.proposal_review_rows {
        if ui
            .button(format!("Review remote proposal {}", review.proposal_id.0))
            .clicked()
        {
            actions.push(DesktopAction::OpenRemoteProposalReview {
                session_id: review.session_id,
                proposal_id: review.proposal_id,
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

    if !active.degraded
        && let Some(text) = active.small_buffer_text()
    {
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
            match save_rejection_status_marker(&status.message) {
                Some(marker) => format!("{severity} {marker}: {}", status.message),
                None => format!("{severity}: {}", status.message),
            }
        })
        .collect()
}

fn save_rejection_status_marker(message: &str) -> Option<&'static str> {
    let lower = message.to_ascii_lowercase();
    if !lower.contains("save") {
        return None;
    }
    if lower.contains("conflict") {
        Some("save_conflict")
    } else if lower.contains("stale") {
        Some("save_stale")
    } else if lower.contains("denied") {
        Some("save_denied")
    } else if lower.contains("reject") {
        Some("save_rejected")
    } else {
        None
    }
}

fn proposal_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let ledger = &snapshot.proposal_ledger_projection;
    let mut rows = Vec::new();
    if let Some(selected) = ledger.selected_proposal_id {
        rows.push(format!("selected proposal: {}", selected.0));
    }

    for row in ledger.rows.iter().take(12) {
        rows.push(format!(
            "proposal {}: {} [{} {:?} {:?}] payload={:?} rollback={:?}",
            row.proposal_id.0,
            row.title,
            row.lifecycle.label,
            row.risk_label,
            row.privacy_label,
            row.payload_kind,
            row.rollback
        ));
        rows.push(format!(
            "proposal {} diff: {:?} targets={} hunks={} +{} -{} omitted={} hash={}",
            row.proposal_id.0,
            row.diff_summary.kind,
            row.diff_summary.target_count,
            row.diff_summary.hunk_count,
            row.diff_summary.inserted_line_count,
            row.diff_summary.deleted_line_count,
            row.diff_summary.omitted_hunk_count,
            row.diff_summary
                .diff_hash
                .as_ref()
                .map(|hash| hash.value.as_str())
                .unwrap_or("<none>")
        ));
        rows.push(format!(
            "proposal {} targets: {:?} shown={} omitted={} redaction={}",
            row.proposal_id.0,
            row.target_coverage.coverage_kind,
            row.target_coverage.targets.len(),
            row.target_coverage.omitted_target_count,
            redaction_label(&row.target_coverage.redaction_hints)
        ));
        rows.extend(row.target_coverage.targets.iter().take(4).map(|target| {
            format!(
                "proposal {} target {}: {:?} file={:?} buffer={:?} path={} ranges={} redaction={}",
                row.proposal_id.0,
                target.target_id,
                target.kind,
                target.file_id.map(|file| file.0),
                target.buffer_id.map(|buffer| buffer.0),
                target
                    .path
                    .as_ref()
                    .map(|path| path.0.as_str())
                    .unwrap_or("<redacted>"),
                target.byte_ranges.len(),
                redaction_label(&target.redaction_hints)
            )
        }));
        rows.push(format!(
            "proposal {} context: {} categories={} items={} omitted={} redaction={}",
            row.proposal_id.0,
            row.context_manifest.manifest_id,
            row.context_manifest.category_count,
            row.context_manifest.total_item_count,
            row.context_manifest.omitted_item_count,
            redaction_label(&row.context_manifest.redaction_hints)
        ));
        if !row.preview_warnings.is_empty() {
            rows.push(format!(
                "proposal {} warnings: {}",
                row.proposal_id.0,
                row.preview_warnings.len()
            ));
        }
        rows.extend(row.preview_warnings.iter().take(4).map(|warning| {
            format!(
                "proposal {} warning {} {:?}: {}",
                row.proposal_id.0, warning.code, warning.kind, warning.message
            )
        }));
        if !row.diagnostics.is_empty() {
            rows.push(format!(
                "proposal {} diagnostics: {}",
                row.proposal_id.0,
                row.diagnostics.len()
            ));
        }
    }
    if ledger.omitted_row_count > 0 {
        rows.push(format!(
            "proposal ledger omitted rows: {}",
            ledger.omitted_row_count
        ));
    }
    rows
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
    rows.extend(manifest.items.iter().take(10).map(|item| {
        format!(
            "context item {}: {:?} {:?} risk={:?} privacy={:?} egress={:?} file={:?} buffer={:?} path={} counts={} ranges={} labels={}",
            item.item_id,
            item.kind,
            item.inclusion,
            item.risk_label,
            item.privacy_label,
            item.egress,
            item.file_id.map(|file| file.0),
            item.buffer_id.map(|buffer| buffer.0),
            item.path
                .as_ref()
                .map(|path| path.0.as_str())
                .unwrap_or("<redacted>"),
            item.counts.len(),
            item.ranges.len(),
            bounded_join(&item.labels)
        )
    }));
    rows.extend(manifest.permissions.iter().take(10).map(|permission| {
        format!(
            "context permission {:?}: capability={} granted={} scope={:?} egress={:?} risk={:?}",
            permission.kind,
            permission.capability.0,
            permission.granted,
            permission.privacy_scope,
            permission.egress,
            permission.risk_label
        )
    }));

    let privacy = &snapshot.privacy_inspector_projection;
    if !privacy.records.is_empty() || privacy.refusal.is_some() {
        rows.push(format!(
            "privacy: {} records, {} denied, {} redacted, {} external, {} high-risk",
            privacy.records.len(),
            privacy.denied_record_count,
            privacy.redacted_record_count,
            privacy.external_egress_record_count,
            privacy.high_risk_record_count
        ));
    }
    rows.extend(privacy.records.iter().take(10).map(|record| {
        format!(
            "privacy record {}: {:?} {:?} risk={:?} privacy={:?} egress={:?} permission={} reasons={}",
            record.exposure_id,
            record.source_kind,
            record.redaction_state,
            record.risk_label,
            record.privacy_label,
            record.egress,
            record
                .permission_label
                .as_ref()
                .map(|capability| capability.0.as_str())
                .unwrap_or("<none>"),
            bounded_join(&record.reasons)
        )
    }));
    if let Some(refusal) = &privacy.refusal {
        rows.push(format!(
            "privacy refusal {}: {} scope={:?} capability={} risk={:?} reasons={}",
            refusal.reason_code,
            refusal.label,
            refusal.privacy_scope,
            refusal
                .capability
                .as_ref()
                .map(|capability| capability.0.as_str())
                .unwrap_or("<none>"),
            refusal.risk_label,
            bounded_join(&refusal.reasons)
        ));
    }

    let budget = &snapshot.permission_budget_projection;
    if !budget.budgets.is_empty() || !budget.evaluations.is_empty() {
        rows.push(format!(
            "permission budget: {} budgets, {} evaluations, {} denied, {} depleted, {} refused",
            budget.budgets.len(),
            budget.evaluations.len(),
            budget.denied_budget_count,
            budget.depleted_budget_count,
            budget.refused_evaluation_count
        ));
    }
    rows.extend(budget.budgets.iter().take(10).map(|contract| {
        format!(
            "permission budget {}: {:?} state={:?} scope={:?} consent={:?} used={}/{} risk={:?} reasons={}",
            contract.budget_id,
            contract.action_class,
            contract.state,
            contract.privacy_scope,
            contract.consent_requirement_label,
            contract.usage.used,
            contract
                .usage
                .ceiling
                .map(|ceiling| ceiling.to_string())
                .unwrap_or_else(|| "uncapped".to_string()),
            contract.risk_label,
            bounded_join(&contract.reasons)
        )
    }));
    rows.extend(budget.evaluations.iter().take(10).map(|evaluation| {
        format!(
            "permission evaluation {}: budget={} disposition={:?} allowed={} action={:?} estimated={} reasons={}",
            evaluation.evaluation_id,
            evaluation.budget_id,
            evaluation.disposition,
            evaluation.allowed,
            evaluation.action.action_class,
            evaluation.action.estimated_units,
            bounded_join(&evaluation.reasons)
        )
    }));
    rows.extend(
        budget
            .evaluations
            .iter()
            .filter_map(|evaluation| {
                evaluation
                    .refusal
                    .as_ref()
                    .map(|refusal| (evaluation, refusal))
            })
            .take(6)
            .map(|(evaluation, refusal)| {
                format!(
                    "permission refusal {}: {} reason={} risk={:?}",
                    evaluation.evaluation_id,
                    refusal.label,
                    refusal.reason_code,
                    refusal.risk_label
                )
            }),
    );

    let checklist = &snapshot.approval_checklist_projection;
    if !checklist.gates.is_empty() || !checklist.blockers.is_empty() {
        rows.push(format!(
            "approval checklist: proposal {} lifecycle={:?} gates={} blockers={} ready={} denials={}",
            checklist.proposal_id.0,
            checklist.lifecycle_state,
            checklist.gates.len(),
            checklist.blockers.len(),
            checklist.ready_for_approval,
            checklist.explicit_denial_reasons.len()
        ));
    }
    rows.extend(checklist.gates.iter().take(12).map(|gate| {
        format!(
            "approval gate {:?}: {:?} risk={:?} privacy={:?} labels={} reasons={}",
            gate.gate,
            gate.status,
            gate.risk_label,
            gate.privacy_label,
            bounded_join(&gate.labels),
            gate.reasons.len()
        )
    }));
    rows.extend(checklist.blockers.iter().take(10).map(|blocker| {
        format!(
            "approval blocker {:?}: {} {} risk={:?} privacy={:?}",
            blocker.gate,
            blocker.reason_code,
            blocker.label,
            blocker.risk_label,
            blocker.privacy_label
        )
    }));
    if !checklist.explicit_denial_reasons.is_empty() {
        rows.push(format!(
            "approval explicit denials: {}",
            bounded_join(&checklist.explicit_denial_reasons)
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
    if !rollback.targets.is_empty() {
        rows.push(format!(
            "checkpoint: id={} available={} targets={} audit={:?} limitations={}",
            rollback.checkpoint.checkpoint_id,
            rollback.checkpoint.available,
            rollback.checkpoint.target_count,
            rollback.checkpoint.audit_status,
            rollback.checkpoint.limitations.len()
        ));
        rows.push(format!(
            "rollback: availability={:?} steps={} reversible={} irreversible={} audit={:?} limitations={}",
            rollback.rollback.availability,
            rollback.rollback.rollback_step_count,
            rollback.rollback.reversible_target_count,
            rollback.rollback.irreversible_target_count,
            rollback.rollback.audit_status,
            rollback.rollback.limitations.len()
        ));
    }
    rows.extend(rollback.targets.iter().take(10).map(|target| {
        format!(
            "rollback target {}: {:?} file={:?} buffer={:?} labels={}",
            target.target_id,
            target.kind,
            target.file_id.map(|file| file.0),
            target.buffer_id.map(|buffer| buffer.0),
            bounded_join(&target.labels)
        )
    }));
    rows.extend(
        rollback
            .checkpoint
            .limitations
            .iter()
            .take(6)
            .map(|limitation| {
                format!(
                    "checkpoint limitation {}: {} risk={:?}",
                    limitation.reason_code, limitation.label, limitation.risk_label
                )
            }),
    );
    rows.extend(
        rollback
            .rollback
            .limitations
            .iter()
            .take(6)
            .map(|limitation| {
                format!(
                    "rollback limitation {}: {} risk={:?}",
                    limitation.reason_code, limitation.label, limitation.risk_label
                )
            }),
    );

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
    rows.extend(assisted.providers.iter().take(8).map(|provider| {
        format!(
            "assisted provider {}: {} class={:?} availability={:?} ops={} cost={} risk_budget={} privacy={} risk={:?}",
            provider.provider_id,
            provider.provider_label,
            provider.provider_class,
            provider.availability,
            provider.supported_operation_count,
            provider.cost_budget_label,
            provider.risk_budget_label,
            provider.privacy_retention_label,
            provider.risk_label
        )
    }));
    rows.extend(
        assisted
            .providers
            .iter()
            .filter_map(|provider| provider.refusal.as_ref().map(|refusal| (provider, refusal)))
            .take(6)
            .map(|(provider, refusal)| {
                format!(
                    "assisted provider refusal {}: {} {} risk={:?}",
                    provider.provider_id, refusal.reason_code, refusal.label, refusal.risk_label
                )
            }),
    );
    rows.extend(assisted.routes.iter().take(8).map(|route| {
        format!(
            "assisted route {}: provider={} op={:?} disposition={:?} invocation={:?} refused_evals={} risk={:?} privacy={:?} reasons={}",
            route.request_id,
            route.provider_id,
            route.operation_class,
            route.disposition,
            route.provider_invocation,
            route.refused_permission_budget_evaluation_count,
            route.risk_label,
            route.privacy_label,
            bounded_join(&route.reasons)
        )
    }));
    rows.extend(
        assisted
            .routes
            .iter()
            .filter_map(|route| route.refusal.as_ref().map(|refusal| (route, refusal)))
            .take(6)
            .map(|(route, refusal)| {
                format!(
                    "assisted route refusal {}: {} {} risk={:?}",
                    route.request_id, refusal.reason_code, refusal.label, refusal.risk_label
                )
            }),
    );
    rows.extend(assisted.requests.iter().take(8).map(|request| {
        format!(
            "assisted request {}: op={:?} payload={:?} targets={} omitted={} capability={} route={:?} refs={}/{}/{} approval={} checkpoint={} labels={}",
            request.request_id,
            request.operation_class,
            request.proposal_payload_kind,
            request.proposal_target_count,
            request.omitted_target_count,
            request.required_capability.0,
            request.route_decision.disposition,
            request.context_manifest.reference_id,
            request.privacy_inspector.reference_id,
            request.permission_budget_projection.reference_id,
            request.approval_checklist.reference_id,
            request
                .checkpoint_rollback
                .as_ref()
                .map(|reference| reference.reference_id.as_str())
                .unwrap_or("<none>"),
            bounded_join(&request.labels)
        )
    }));
    rows.extend(assisted.proposal_previews.iter().take(8).map(|preview| {
        format!(
            "assisted preview {}: proposal={} readiness={:?} preview_ready={} approval_ready={} apply_ready={} ledger={} diff={:?} targets={} risk={:?} privacy={:?}",
            preview.preview_id,
            preview.proposal_id.0,
            preview.readiness,
            preview.ready_for_preview,
            preview.ready_for_approval,
            preview.ready_for_apply,
            preview.ledger_row_present,
            preview.diff_summary.kind,
            preview.target_coverage.targets.len(),
            preview.risk_label,
            preview.privacy_label
        )
    }));
    rows.extend(assisted.refusals.iter().take(8).map(|refusal| {
        format!(
            "assisted refusal {}: {} provider={} op={:?} capability={} risk={:?} reasons={}",
            refusal.reason_code,
            refusal.label,
            refusal.provider_id.as_deref().unwrap_or("<none>"),
            refusal.operation_class,
            refusal
                .capability
                .as_ref()
                .map(|capability| capability.0.as_str())
                .unwrap_or("<none>"),
            refusal.risk_label,
            bounded_join(&refusal.reasons)
        )
    }));

    let delegated = &snapshot.delegated_task_projection;
    if delegated.plan_count == 0
        && delegated.plan_rows.is_empty()
        && delegated.step_summaries.is_empty()
        && delegated.blockers.is_empty()
        && delegated.refusals.is_empty()
        && delegated.required_approvals.is_empty()
        && delegated.proposal_preview_links.is_empty()
        && delegated.audit_readiness.is_empty()
    {
        return rows;
    }
    rows.push(format!(
        "delegated task command center: projection={} plans={} blocked={} refused={} runtime={:?} autonomous_apply=unsupported redaction={}",
        delegated.projection_id,
        delegated.plan_count,
        delegated.blocked_plan_count,
        delegated.refused_plan_count,
        delegated.runtime_activation,
        redaction_label(&delegated.redaction_hints)
    ));
    rows.extend(delegated.plan_only_disclaimers.iter().map(|disclaimer| {
        format!("delegated task disclaimer: {disclaimer} autonomous apply unsupported")
    }));
    rows.extend(delegated.plan_rows.iter().map(|plan| {
        format!(
            "delegated task plan {}: state={:?} readiness={:?} steps={} targets={} blockers={} refusals={} proposal_previews={} risk={:?} privacy={:?} runtime={:?} labels={}",
            plan.plan_id.0,
            plan.plan_state,
            plan.readiness,
            plan.step_count,
            plan.affected_target_count,
            plan.blocker_count,
            plan.refusal_count,
            plan.proposal_preview_link_count,
            plan.risk_label,
            plan.privacy_label,
            plan.runtime_activation,
            bounded_join(&plan.labels)
        )
    }));
    rows.extend(delegated.step_summaries.iter().map(|step| {
        format!(
            "delegated task step {} plan={} order={} op={:?} state={:?} deps={} targets={} proposal={:?} blockers={} risk={:?} privacy={:?}",
            step.step_id.0,
            step.plan_id.0,
            step.order,
            step.operation_class,
            step.state,
            step.dependency_count,
            step.target_count,
            step.proposal_id.map(|proposal| proposal.0),
            step.blocker_count,
            step.risk_label,
            step.privacy_label
        )
    }));
    rows.extend(delegated.required_approvals.iter().map(|gate| {
        format!(
            "delegated task trust gate {:?}: required={} satisfied={} risk={:?} privacy={:?} reasons={}",
            gate.kind,
            gate.required,
            gate.satisfied,
            gate.risk_label,
            gate.privacy_label,
            bounded_join(&gate.reasons)
        )
    }));
    rows.extend(delegated.blockers.iter().map(|blocker| {
        format!(
            "delegated task blocker {}: gate={:?} proposal={:?} label={} reasons={}",
            blocker.reason_code,
            blocker.gate,
            blocker.proposal_id.map(|proposal| proposal.0),
            blocker.label,
            bounded_join(&blocker.reasons)
        )
    }));
    rows.extend(delegated.refusals.iter().map(|refusal| {
        format!(
            "delegated task refusal {}: gate={:?} proposal={:?} label={} reasons={}",
            refusal.reason_code,
            refusal.gate,
            refusal.proposal_id.map(|proposal| proposal.0),
            refusal.label,
            bounded_join(&refusal.reasons)
        )
    }));
    rows.extend(delegated.proposal_preview_links.iter().map(|link| {
        format!(
            "delegated task proposal preview {}: proposal={} payload={:?} lifecycle={:?} targets={} hunks={} source_redacted={} proposal-mediated",
            link.link_id,
            link.proposal_id.0,
            link.payload_kind,
            link.lifecycle_state,
            link.target_count,
            link.hunk_count,
            link.full_source_redacted
        )
    }));
    rows.extend(delegated.audit_readiness.iter().map(|readiness| {
        format!(
            "delegated task audit readiness {}: readiness={:?} runtime={:?} core_ids={} blockers={} refusals={} proposal_previews={} labels={}",
            readiness.readiness_id,
            readiness.readiness,
            readiness.runtime_activation,
            readiness.correlation_causality_valid,
            readiness.blocker_count,
            readiness.refusal_count,
            readiness.proposal_preview_link_count,
            bounded_join(&readiness.labels)
        )
    }));
    rows
}

fn redaction_label(redaction_hints: &[devil_protocol::RedactionHint]) -> String {
    if redaction_hints.is_empty() {
        "none".to_string()
    } else {
        redaction_hints
            .iter()
            .take(4)
            .map(|hint| format!("{hint:?}"))
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn bounded_join(values: &[String]) -> String {
    if values.is_empty() {
        "<none>".to_string()
    } else {
        values
            .iter()
            .take(4)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn projected_assisted_run_id(snapshot: &ShellProjectionSnapshot) -> Option<AgentRunId> {
    let projection_id = snapshot.assisted_ai_projection.projection_id.as_str();
    let run_index = projection_id.find("phase4-run-")?;
    Some(AgentRunId(projection_id[run_index..].to_string()))
}

fn language_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let language = &snapshot.language_tooling_projection;
    let mut rows = Vec::new();
    if language.buffer_id.is_some()
        || !language.operations.is_empty()
        || !language.problems.is_empty()
        || !language.outline.is_empty()
    {
        rows.push(format!(
            "language: {:?} problems={} completions={} definitions={} references={} outline={} stale={} cancelled={}",
            language.status,
            language.problems.len(),
            language.completions.len(),
            language.definitions.len(),
            language.references.len(),
            language.outline.len(),
            language.stale_result_count,
            language.cancellation_count
        ));
    }
    if let Some(hover) = &language.hover {
        rows.push(format!("hover: {} {}", hover.label, hover.summary));
    }
    rows.extend(language.operations.iter().map(|operation| {
        format!(
            "language op {} {:?} {:?} proposal={:?}",
            operation.operation_id,
            operation.kind,
            operation.status,
            operation.proposal_id.map(|proposal| proposal.0)
        )
    }));
    rows
}

fn terminal_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let terminal = &snapshot.terminal_panel_projection;
    let mut rows = Vec::new();
    if terminal.active_session_id.is_some()
        || terminal.last_denial.is_some()
        || terminal.last_error.is_some()
        || !terminal.output_rows.is_empty()
    {
        rows.push(format!(
            "terminal: {:?} session={:?} rows={} omitted={} matches={}",
            terminal.status.kind,
            terminal.active_session_id.map(|session| session.0),
            terminal.output_rows.len(),
            terminal.scrollback.omitted_row_count,
            terminal.search.match_count
        ));
    }
    if let Some(policy) = &terminal.policy {
        rows.push(format!(
            "terminal policy: capability={} trust={:?} granted={} reason={}",
            policy.capability_id.0, policy.workspace_trust_state, policy.granted, policy.reason
        ));
    }
    if let Some(denial) = &terminal.last_denial {
        rows.push(format!("terminal denial: {denial}"));
    }
    rows.extend(terminal.output_rows.iter().take(5).map(|row| {
        format!(
            "terminal output {}: {}",
            row.sequence.0, row.redacted_payload
        )
    }));
    rows
}

fn operational_health_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    DesktopOperationalHealthSnapshot::from_projection(snapshot).rows()
}

fn plugin_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    for projection in &snapshot.plugin_contribution_projections {
        let commands = plugin_command_descriptors(projection);
        let other_contribution_count = projection
            .contributions
            .len()
            .saturating_sub(commands.len());
        rows.push(format!(
            "plugin management plugin {}: status={} contributions={} commands={} other={} sandbox=metadata-only audit=app-owned",
            projection.plugin_id.0,
            projection.status_label,
            projection.contributions.len(),
            commands.len(),
            other_contribution_count
        ));
        if commands.is_empty() {
            rows.push(format!(
                "plugin management plugin {}: no projected commands",
                projection.plugin_id.0
            ));
        }
        rows.extend(commands.into_iter().map(|command| {
            format!(
                "plugin management plugin {} command {}: {} capability={} audit=dispatch-intent-only",
                projection.plugin_id.0,
                command.command_id,
                command.title,
                command.required_capability.0
            )
        }));
    }
    rows
}

fn plugin_command_descriptors(
    projection: &PluginContributionProjection,
) -> Vec<&PluginCommandDescriptor> {
    projection
        .contributions
        .iter()
        .filter_map(|contribution| match contribution {
            PluginContribution::Command(command) => Some(command),
            _ => None,
        })
        .collect()
}

fn collaboration_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    let projection = &snapshot.collaboration_gui_projection;
    if !projection.runtime_enabled
        && !projection.presence_enabled
        && projection.session_rows.is_empty()
        && projection.shared_proposal_rows.is_empty()
        && snapshot.collaboration_presence_projections.is_empty()
    {
        return rows;
    }
    rows.push(format!(
        "collaboration: status={} runtime_enabled={} presence_enabled={} sessions={} reconnecting={} conflicts={} offline={} shared_proposals={} redaction=metadata-only",
        projection.status_label,
        projection.runtime_enabled,
        projection.presence_enabled,
        projection.session_rows.len(),
        projection.reconnecting_session_count,
        projection.conflict_session_count,
        projection.offline_session_count,
        projection.shared_proposal_rows.len()
    ));
    rows.extend(projection.session_rows.iter().map(|session| {
        format!(
            "collaboration session {}: state={:?} participants={} presence={} reconnecting={} conflicts={} operations={} acknowledgements={} gaps={} offline={} status={}",
            session.session_id.0,
            session.state,
            session.participant_count,
            session.presence_count,
            session.reconnecting_participant_count,
            session.conflict_count,
            session.operation_count,
            session.acknowledgement_count,
            session.causal_gap_count,
            session.offline,
            session.status_label
        )
    }));
    rows.extend(projection.shared_proposal_rows.iter().map(|review| {
        format!(
            "shared proposal session {} proposal {}: required={} authorized={} approvals={} denials={} pending={} operations={} stale={} status={} proposal-mediated",
            review.session_id.0,
            review.proposal_id.0,
            review.required_approver_count,
            review.authorized_approver_count,
            review.approval_count,
            review.denial_count,
            review.pending_count,
            review.applied_operation_count,
            review.stale,
            review.status_label
        )
    }));
    rows.extend(
        snapshot
            .collaboration_presence_projections
            .iter()
            .map(|presence| {
                format!(
                    "collaboration presence {} participant {} reconnecting={} activity={}",
                    presence.session_id.0,
                    presence.participant_id.0,
                    presence.reconnecting,
                    presence.activity_label.as_deref().unwrap_or("<none>")
                )
            }),
    );
    rows
}

fn remote_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    let projection = &snapshot.remote_gui_projection;
    if !projection.runtime_enabled
        && projection.session_rows.is_empty()
        && projection.proposal_review_rows.is_empty()
    {
        return rows;
    }
    rows.push(format!(
        "remote workspace: status={} runtime_enabled={} sessions={} connected={} reconnecting={} offline={} proposal_reviews={} redaction=metadata-only",
        projection.status_label,
        projection.runtime_enabled,
        projection.session_rows.len(),
        projection.connected_session_count,
        projection.reconnecting_session_count,
        projection.offline_session_count,
        projection.proposal_review_rows.len()
    ));
    rows.extend(projection.session_rows.iter().map(|session| {
        format!(
            "remote workspace session {} authority={} agent={} state={:?} filesystem={} terminal={} lsp={} reconnect_supported={} reconnecting={} offline={} proposal_reviews={} status={}",
            session.session_id.0,
            session.authority_label,
            session.agent_version,
            session.state,
            session.filesystem_descriptor_status,
            session.terminal_descriptor_status,
            session.lsp_descriptor_status,
            session.reconnect_supported,
            session.reconnecting,
            session.offline,
            session.proposal_review_count,
            session.status_label
        )
    }));
    rows.extend(projection.proposal_review_rows.iter().map(|review| {
        format!(
            "remote proposal session {} proposal {} authority={} payload={:?} lifecycle={:?} status={} proposal-mediated={}",
            review.session_id.0,
            review.proposal_id.0,
            review.remote_authority_label,
            review.payload_kind,
            review.lifecycle_state,
            review.status_label,
            review.proposal_mediated
        )
    }));
    rows
}

fn first_collaboration_participant(
    snapshot: &ShellProjectionSnapshot,
    session_id: CollaborationSessionId,
) -> Option<CollaborationParticipantId> {
    snapshot
        .collaboration_presence_projections
        .iter()
        .filter(|presence| presence.session_id == session_id)
        .map(|presence| presence.participant_id)
        .min_by_key(|participant_id| participant_id.0)
}
