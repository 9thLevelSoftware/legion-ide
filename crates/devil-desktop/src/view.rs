//! Projection rendering for the desktop adapter.

use std::collections::{BTreeSet, HashSet};

use devil_protocol::{
    FileId, PluginCommandDescriptor, PluginContribution, PluginContributionProjection, ProposalId,
    ProposalRejectionReason, ProposalRiskLabel, ViewportProjectionMode,
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
    /// Read-only product-mode rows.
    pub product_mode_rows: Vec<String>,
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
    /// Manual-mode local control and trust-boundary rows derived from projections.
    pub manual_control_rows: Vec<String>,
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

        let product_mode_rows = product_mode_rows(snapshot);
        Self {
            layout_title: snapshot.layout_projection.layout.title.clone(),
            top_bar_rows: top_bar_rows(snapshot, &flags),
            product_mode_rows,
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
            manual_control_rows: manual_control_rows(snapshot),
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
            .exact_size(52.0)
            .frame(theme::toolbar_frame())
            .show_inside(ui, |ui| {
                render_top_command_bar(ui, snapshot, &model, &mut actions);
            });

        egui::Panel::left("devil_desktop_explorer")
            .default_size(272.0)
            .min_size(236.0)
            .resizable(true)
            .frame(theme::pane_frame(theme::BG_BASE))
            .show_inside(ui, |ui| {
                render_left_sidebar(ui, snapshot, state, &model, &mut actions);
            });

        egui::Panel::bottom("devil_desktop_status")
            .exact_size(24.0)
            .frame(theme::panel_frame(theme::BG_CODE))
            .show_inside(ui, |ui| {
                render_status_bar(ui, &model);
            });

        let bottom_height = match projected_product_mode(snapshot) {
            DesktopProductMode::Manual => 150.0,
            DesktopProductMode::LegionWorkflows => 240.0,
            DesktopProductMode::Delegates => 200.0,
        };
        egui::Panel::bottom("devil_desktop_bottom_console")
            .default_size(bottom_height)
            .min_size(112.0)
            .resizable(true)
            .frame(theme::pane_frame(theme::BG_CODE))
            .show_inside(ui, |ui| {
                render_bottom_console(ui, &model);
            });

        let right_width = match projected_product_mode(snapshot) {
            DesktopProductMode::Manual => 260.0,
            DesktopProductMode::Delegates | DesktopProductMode::LegionWorkflows => 380.0,
        };
        egui::Panel::right("devil_desktop_trust")
            .default_size(right_width)
            .min_size(260.0)
            .resizable(true)
            .frame(theme::pane_frame(theme::BG_BASE))
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
            .frame(theme::pane_frame(theme::BG_CODE))
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
    let level = projected_product_mode(snapshot);
    ui.set_height(52.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        render_window_controls(ui);
        ui.label(theme::title("Devil IDE"));
        ui.separator();
        ui.label(theme::body_strong(&model.layout_title));
        render_branch_pill(ui, snapshot);
        render_engine_status(ui, snapshot, level);
        ui.add_space(12.0);
        render_product_mode_switch(ui, model);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            avatar(ui, "MK", theme::TEXT_SECONDARY);
            if soft_button(ui, "Open").clicked() {
                actions.push(DesktopAction::ShowOpenPathPrompt);
            }
            if soft_button(ui, "Search").clicked() {
                actions.push(DesktopAction::ShowSearchPrompt {
                    scope: snapshot.search_projection.scope,
                });
            }
            if primary_button(ui, level_primary_action(level), level_color(level)).clicked() {
                actions.push(match level {
                    DesktopProductMode::Manual => DesktopAction::SaveAll,
                    DesktopProductMode::Delegates => DesktopAction::StartAiProposal {
                        instruction_label: "desktop delegated task".to_string(),
                    },
                    DesktopProductMode::LegionWorkflows => DesktopAction::StartAiProposal {
                        instruction_label: "desktop legion workflow".to_string(),
                    },
                });
            }
            render_resource_strip(ui, snapshot, level);
            if soft_button(ui, "Save All").clicked() {
                actions.push(DesktopAction::SaveAll);
            }
        });
    });
}

fn render_product_mode_switch(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    let active = model
        .product_mode_rows
        .first()
        .map(|row| row.as_str())
        .unwrap_or("product mode: active=Manual read-only projection");
    theme::card_frame_tinted(theme::BG_INPUT, theme::BORDER_DEFAULT).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::eyebrow("PRODUCT MODE"));
            for (level, label, color) in [
                ("M", "Manual", theme::TEXT_MUTED),
                ("D", "Delegates", theme::ACCENT_VIOLET),
                ("W", "Legion Workflows", theme::ACCENT_PURPLE),
            ] {
                level_pill(ui, level, label, color, active.contains(level));
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
    let level = projected_product_mode(snapshot);
    sidebar_header(ui, "PROJECT", workspace_label(snapshot));
    if level == DesktopProductMode::LegionWorkflows {
        render_context_packs(ui);
    } else {
        render_project_tree_panel(ui, snapshot, state, level, actions);
    }

    match level {
        DesktopProductMode::Manual => render_collapsed_ai_rail(ui, model),
        DesktopProductMode::Delegates => {
            if delegated_activity_projected(snapshot) {
                render_agent_roster(ui, snapshot, model, level)
            } else if snapshot.assisted_ai_projection.preview_ready_count > 0 {
                render_session_context(ui, snapshot)
            } else {
                render_assistance_toggles(ui)
            }
        }
        DesktopProductMode::LegionWorkflows => render_agent_roster(ui, snapshot, model, level),
    }

    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        render_sidebar_footer(ui, snapshot);
    });
}

fn render_code_canvas(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    match projected_product_mode(snapshot) {
        DesktopProductMode::Manual => render_editor_canvas(ui, snapshot, model, actions),
        DesktopProductMode::Delegates => {
            if delegated_activity_projected(snapshot) {
                render_delegated_canvas(ui, snapshot, model, actions)
            } else if snapshot.assisted_ai_projection.preview_ready_count > 0 {
                render_copilot_canvas(ui, snapshot, model, actions)
            } else {
                render_editor_canvas(ui, snapshot, model, actions)
            }
        }
        DesktopProductMode::LegionWorkflows => render_fleet_canvas(ui, snapshot, model, actions),
    }
}

fn render_right_console(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    show_trust: &mut bool,
    _show_auxiliary: &mut bool,
    actions: &mut Vec<DesktopAction>,
) {
    match projected_product_mode(snapshot) {
        DesktopProductMode::Manual => render_manual_context_inspector(ui, snapshot, model, actions),
        DesktopProductMode::Delegates => {
            if delegated_activity_projected(snapshot) {
                render_delegation_console(ui, snapshot, model, show_trust, actions)
            } else if snapshot.assisted_ai_projection.preview_ready_count > 0 {
                render_pair_session_panel(ui, snapshot, model, actions)
            } else {
                render_assisted_inspector(ui, snapshot, model, actions)
            }
        }
        DesktopProductMode::LegionWorkflows => render_fleet_console(ui, snapshot, model, actions),
    }
}

fn render_bottom_console(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    ui.horizontal(|ui| {
        console_tab(ui, "Terminal", true, theme::TEXT_PRIMARY);
        console_tab(ui, "Tests", false, theme::ACCENT_GREEN);
        console_tab(ui, "Agent Logs", false, theme::ACCENT_CYAN);
        console_tab(ui, "Workflow", false, theme::ACCENT_VIOLET);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(theme::code_muted("us-west - 42 ms"));
        });
    });
    ui.separator();
    ui.columns(2, |columns| {
        render_terminal_stream(&mut columns[0], model);
        render_agent_stream(&mut columns[1], model);
    });
}

fn render_status_bar(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    ui.set_height(24.0);
    ui.horizontal(|ui| {
        ui.label(theme::accent("connected", theme::ACCENT_GREEN));
        ui.label(theme::muted("- fleet-mesh"));
        ui.separator();
        for row in &model.status_bar_rows {
            ui.label(theme::code_muted(trim_middle(row, 56)));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(theme::accent("Product Mode", theme::ACCENT_VIOLET));
            ui.label(theme::code_muted("UTF-8"));
            ui.label(theme::code_muted("LF"));
        });
    });
}

fn render_console_section(ui: &mut egui::Ui, title: &str, rows: &[String], empty: &str) {
    section_label(ui, title, None);
    theme::small_card_frame().show(ui, |ui| {
        render_compact_rows(ui, rows, empty, 6);
    });
}

fn render_search_projection(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    let search = DesktopSearchViewModel::from_projection(&snapshot.search_projection);
    section_label(ui, "Search", None);
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::body(search.header));
        render_compact_rows(ui, &search.status_rows, "Search idle", 2);
        render_compact_rows(ui, &search.result_rows, "No search results", 5);
        for row in &search.diagnostic_rows {
            ui.label(theme::muted(row));
        }
    });
}

fn render_window_controls(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        status_dot(ui, theme::ACCENT_RED);
        status_dot(ui, theme::ACCENT_AMBER);
        status_dot(ui, theme::ACCENT_GREEN);
    });
}

fn render_branch_pill(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    let path = current_path(snapshot);
    let label = if path == "<none>" {
        "workspace"
    } else {
        path.rsplit(['/', '\\']).next().unwrap_or(path)
    };
    pill(ui, &format!("branch - {label}"), theme::TEXT_MUTED, false);
}

fn render_engine_status(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    level: DesktopProductMode,
) {
    let label = match level {
        DesktopProductMode::Manual => "Engine idle",
        DesktopProductMode::Delegates => "Delegates active",
        DesktopProductMode::LegionWorkflows => "Legion workflow online",
    };
    ui.horizontal(|ui| {
        status_dot(ui, level_color(level));
        ui.label(theme::muted(format!(
            "{label} - {} proposals",
            snapshot.proposal_ledger_projection.rows.len()
        )));
    });
}

fn render_resource_strip(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    level: DesktopProductMode,
) {
    theme::ghost_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::accent(
                format!("{}%", resource_load(snapshot, level)),
                theme::ACCENT_CYAN,
            ));
            ui.separator();
            ui.label(theme::accent(
                format!("{} tests", snapshot.status_messages.len()),
                theme::ACCENT_GREEN,
            ));
            ui.separator();
            ui.label(theme::accent(
                format!("{} agents", projected_agent_count(snapshot)),
                level_color(level),
            ));
        });
    });
}

fn render_project_tree_panel(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    level: DesktopProductMode,
    actions: &mut Vec<DesktopAction>,
) {
    ui.horizontal(|ui| {
        section_label(ui, "Explorer", None);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if soft_button(ui, "Refresh").clicked() {
                actions.push(DesktopAction::RefreshExplorer);
            }
        });
    });
    egui::ScrollArea::vertical()
        .id_salt("devil_desktop_explorer_scroll")
        .max_height(if level == DesktopProductMode::Manual {
            520.0
        } else {
            240.0
        })
        .auto_shrink([false, false])
        .show(ui, |ui| {
            render_explorer_controls(ui, snapshot, state, actions);
        });
}

fn render_context_packs(ui: &mut egui::Ui) {
    section_label(ui, "Context Packs", Some(theme::ACCENT_PURPLE));
    for pack in [
        "Auth system",
        "Billing model",
        "API routes",
        "Test suite",
        "Deployment config",
    ] {
        theme::ghost_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                status_dot(ui, theme::TEXT_MUTED);
                ui.label(theme::body(pack));
            });
        });
        ui.add_space(2.0);
    }
}

fn render_collapsed_ai_rail(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    ui.add_space(8.0);
    theme::small_card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::eyebrow("MANUAL CONTROL"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(theme::accent("AI disabled", theme::ACCENT_BLUE));
            });
        });
        render_compact_rows(
            ui,
            &model.manual_control_rows,
            "Manual controls are projection-only",
            4,
        );
    });
}

fn render_assistance_toggles(ui: &mut egui::Ui) {
    ui.add_space(8.0);
    section_label(ui, "AI Assistance", Some(theme::ACCENT_CYAN));
    for label in [
        "Inline completions",
        "Quick fixes",
        "Explain selection",
        "Generate docs",
        "Test suggestions",
    ] {
        ui.horizontal(|ui| {
            ui.label(theme::body(label));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                pill(ui, "on", theme::ACCENT_CYAN, true);
            });
        });
    }
}

fn render_session_context(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    ui.add_space(8.0);
    section_label(ui, "Session Context", Some(theme::ACCENT_BLUE));
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::eyebrow("CURRENT TASK"));
        ui.label(theme::body(current_objective(snapshot)));
        ui.add_space(6.0);
        ui.label(theme::eyebrow("SELECTED FILE"));
        ui.label(theme::code(current_path(snapshot)));
        ui.add_space(6.0);
        ui.label(theme::eyebrow("RELATED TESTS"));
        ui.label(theme::muted(format!(
            "{} terminal rows - {} language ops",
            snapshot.terminal_panel_projection.output_rows.len(),
            snapshot.language_tooling_projection.operations.len()
        )));
    });
}

fn render_agent_roster(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    level: DesktopProductMode,
) {
    ui.add_space(8.0);
    section_label(ui, "Active Delegates", Some(level_color(level)));
    let mut rendered = 0usize;
    for provider in snapshot.assisted_ai_projection.providers.iter().take(4) {
        agent_card(
            ui,
            &provider.provider_label,
            &format!("{:?}", provider.availability),
            level_color(level),
            0.55,
        );
        rendered += 1;
    }
    for plan in snapshot.delegated_task_projection.plan_rows.iter().take(5) {
        agent_card(
            ui,
            &plan.plan_id.0,
            &format!("{:?}", plan.plan_state),
            risk_color(plan.risk_label),
            0.72,
        );
        rendered += 1;
    }
    if rendered == 0 {
        render_compact_rows(ui, &model.assistant_rows, "No projected agent activity", 4);
    }
}

fn render_sidebar_footer(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    ui.separator();
    footer_metric(
        ui,
        "git",
        snapshot.proposal_ledger_projection.rows.len(),
        theme::ACCENT_AMBER,
    );
    footer_metric(
        ui,
        "tests",
        snapshot.status_messages.len(),
        theme::ACCENT_GREEN,
    );
    footer_metric(
        ui,
        "workflows",
        snapshot.delegated_task_projection.plan_count as usize,
        theme::ACCENT_CYAN,
    );
}

fn render_editor_canvas(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    let level = projected_product_mode(snapshot);
    render_tab_strip(ui, snapshot, actions);
    render_breadcrumb_bar(ui, snapshot, level);
    theme::code_frame().show(ui, |ui| {
        egui::ScrollArea::both()
            .id_salt("devil_desktop_code_canvas_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                render_code_lines(ui, model);
            });
    });
    if level == DesktopProductMode::Delegates && !delegated_activity_projected(snapshot) {
        render_assisted_suggestion_panel(ui);
    }
    if level == DesktopProductMode::Manual {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            theme::ghost_frame().show(ui, |ui| {
                ui.label(theme::muted("Tab to accept completion"));
            });
        });
    }
    render_search_projection(ui, snapshot);
    render_close_dirty_prompt_controls(ui, snapshot, actions);
}

fn render_tab_strip(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    theme::pane_frame(theme::BG_BASE).show(ui, |ui| {
        ui.set_height(34.0);
        ui.horizontal(|ui| {
            let tabs = &snapshot.daily_editing_projection.tabs.tabs;
            if tabs.is_empty() {
                ui.label(theme::muted("<no open tabs>"));
            }
            for tab in tabs {
                let mut title = tab.title.clone();
                if tab.dirty {
                    title.push_str(" *");
                }
                let color = if tab.active {
                    theme::TEXT_PRIMARY
                } else {
                    theme::TEXT_MUTED
                };
                let response = ui.add(
                    egui::Button::new(theme::accent(title, color))
                        .fill(if tab.active {
                            theme::BG_CODE
                        } else {
                            theme::BG_BASE
                        })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if tab.active {
                                theme::BORDER_DEFAULT
                            } else {
                                theme::BG_BASE
                            },
                        ))
                        .corner_radius(egui::CornerRadius::same(6)),
                );
                if response.clicked() {
                    actions.push(DesktopAction::SwitchTab {
                        buffer_id: tab.buffer_id,
                    });
                }
            }
        });
    });
}

fn render_breadcrumb_bar(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    level: DesktopProductMode,
) {
    theme::pane_frame(theme::BG_CODE).show(ui, |ui| {
        ui.set_height(28.0);
        ui.horizontal(|ui| {
            ui.label(theme::code_muted("src"));
            ui.label(theme::muted(">"));
            ui.label(theme::code(current_path(snapshot)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(theme::muted("TS - LF - UTF-8"));
                ui.label(theme::accent("checks ready", theme::ACCENT_GREEN));
                if level != DesktopProductMode::Manual {
                    ui.label(theme::accent("AI suggestions", level_color(level)));
                }
            });
        });
    });
}

fn render_code_lines(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    if model.active_buffer_lines.is_empty() {
        ui.label(theme::muted("<no active buffer>"));
        return;
    }
    for (index, row) in model.active_buffer_lines.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.add_sized(
                [42.0, 18.0],
                egui::Label::new(theme::code_muted(format!("{:>3}", index + 1))),
            );
            ui.label(theme::code(row));
        });
    }
}

fn render_assisted_suggestion_panel(ui: &mut egui::Ui) {
    ui.add_space(8.0);
    theme::card_frame_tinted(theme::BG_RAISED, theme::dim(theme::ACCENT_CYAN, 80)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::accent("Suggestions", theme::ACCENT_CYAN));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(theme::code_muted("3 for selection"));
            });
        });
        for action in [
            "Refactor validation into helper",
            "Add null-check for selected value",
            "Generate unit test",
        ] {
            ui.label(theme::body(action));
        }
    });
}

fn render_copilot_canvas(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    render_copilot_plan_strip(ui, snapshot);
    ui.columns(2, |columns| {
        theme::code_frame().show(&mut columns[0], |ui| {
            section_label(ui, current_path(snapshot), Some(theme::ACCENT_BLUE));
            egui::ScrollArea::both().show(ui, |ui| render_code_lines(ui, model));
        });
        theme::code_frame().show(&mut columns[1], |ui| {
            ui.horizontal(|ui| {
                section_label(ui, "Proposed changes", Some(theme::ACCENT_VIOLET));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if primary_button(ui, "Apply all", theme::ACCENT_BLUE).clicked()
                        && let Some(proposal_id) = first_proposal_id(snapshot)
                    {
                        actions.push(DesktopAction::ApplyProposal { proposal_id });
                    }
                });
            });
            render_proposal_diff_cards(ui, snapshot, model);
        });
    });
}

fn render_copilot_plan_strip(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    theme::pane_frame(theme::BG_BASE).show(ui, |ui| {
        ui.set_height(40.0);
        ui.horizontal(|ui| {
            section_label(ui, "Delegate Plan", Some(theme::ACCENT_BLUE));
            for (index, label) in [
                "Inspect current file",
                "Draft proposal",
                "Update tests",
                "Run suite",
            ]
            .iter()
            .enumerate()
            {
                pill(
                    ui,
                    &format!("{} {label}", index + 1),
                    if index == 0 {
                        theme::ACCENT_GREEN
                    } else {
                        theme::TEXT_MUTED
                    },
                    index == 0,
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(theme::muted(format!(
                    "{} previews",
                    snapshot.assisted_ai_projection.proposal_previews.len()
                )));
            });
        });
    });
}

fn render_proposal_diff_cards(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
) {
    let rows = if model.proposal_rows.is_empty() {
        &model.assistant_rows
    } else {
        &model.proposal_rows
    };
    if rows.is_empty() {
        ui.label(theme::muted("No proposal preview projected"));
        return;
    }
    for row in rows.iter().take(8) {
        let color = if row.contains("diff") {
            theme::ACCENT_GREEN
        } else {
            theme::ACCENT_VIOLET
        };
        theme::small_card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                status_dot(ui, color);
                ui.label(theme::body(trim_middle(row, 96)));
            });
        });
    }
    if let Some(selected) = snapshot.proposal_ledger_projection.selected_proposal_id {
        ui.label(theme::code_muted(format!(
            "selected proposal {}",
            selected.0
        )));
    }
}

fn render_delegated_canvas(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    theme::pane_frame(theme::BG_CODE).show(ui, |ui| {
        ui.set_height(220.0);
        ui.horizontal(|ui| {
            section_label(ui, "Delegated Diff Review", Some(theme::ACCENT_VIOLET));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary_button(ui, "Approve", theme::ACCENT_BLUE).clicked()
                    && let Some(proposal_id) = first_proposal_id(snapshot)
                {
                    actions.push(DesktopAction::ApproveProposal { proposal_id });
                }
                if soft_button(ui, "Request Changes").clicked()
                    && let Some(proposal_id) = first_proposal_id(snapshot)
                {
                    actions.push(DesktopAction::RejectProposal {
                        proposal_id,
                        reason: ProposalRejectionReason::UserRejected,
                    });
                }
            });
        });
        render_compact_rows(
            ui,
            &model.proposal_rows,
            "No delegated proposal selected",
            8,
        );
    });
    ui.separator();
    render_task_board(ui, snapshot, model, DesktopProductMode::Delegates, actions);
}

fn render_fleet_canvas(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    theme::pane_frame(theme::BG_BASE).show(ui, |ui| {
        ui.set_height(84.0);
        ui.horizontal(|ui| {
            avatar(ui, "LW", theme::ACCENT_PURPLE);
            ui.vertical(|ui| {
                ui.label(theme::accent("MASTER DIRECTIVE", theme::ACCENT_PURPLE));
                ui.label(theme::title(current_objective(snapshot)));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if soft_button(ui, "Force Review").clicked()
                    && let Some(proposal_id) = first_proposal_id(snapshot)
                {
                    actions.push(DesktopAction::PreviewProposal { proposal_id });
                }
                let _ = soft_button(ui, "Pause Workflow");
                let _ = soft_button(ui, "Add Constraint");
            });
        });
        ui.horizontal(|ui| {
            ui.label(theme::muted(format!(
                "{} delegates active",
                projected_agent_count(snapshot)
            )));
            ui.separator();
            ui.label(theme::muted(format!(
                "{} proposals",
                snapshot.proposal_ledger_projection.rows.len()
            )));
            ui.separator();
            ui.label(theme::accent("confidence 87%", theme::ACCENT_GREEN));
        });
    });
    render_task_board(
        ui,
        snapshot,
        model,
        DesktopProductMode::LegionWorkflows,
        actions,
    );
}

fn render_task_board(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    level: DesktopProductMode,
    actions: &mut Vec<DesktopAction>,
) {
    let board_height = ui.available_height();
    egui::ScrollArea::horizontal()
        .id_salt("devil_desktop_task_board")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_height(board_height);
            ui.horizontal_top(|ui| {
                task_column(
                    ui,
                    "ASSIGNED",
                    theme::TEXT_MUTED,
                    delegated_plan_rows(snapshot, model, 0),
                    actions,
                );
                task_column(
                    ui,
                    "IN PROGRESS",
                    theme::ACCENT_BLUE,
                    delegated_step_rows(snapshot, model),
                    actions,
                );
                task_column(
                    ui,
                    "WAITING ON HUMAN",
                    theme::ACCENT_ORANGE,
                    proposal_board_rows(snapshot, model),
                    actions,
                );
                task_column(
                    ui,
                    if level == DesktopProductMode::LegionWorkflows {
                        "TESTING / REVIEW"
                    } else {
                        "TESTING"
                    },
                    theme::ACCENT_VIOLET,
                    model.language_rows.iter().take(4).cloned().collect(),
                    actions,
                );
                task_column(
                    ui,
                    "DONE",
                    theme::ACCENT_GREEN,
                    model
                        .operational_health_rows
                        .iter()
                        .take(4)
                        .cloned()
                        .collect(),
                    actions,
                );
            });
        });
}

fn task_column(
    ui: &mut egui::Ui,
    title: &str,
    color: egui::Color32,
    rows: Vec<String>,
    _actions: &mut Vec<DesktopAction>,
) {
    theme::card_frame_tinted(theme::BG_CANVAS, theme::BORDER_SUBTLE).show(ui, |ui| {
        ui.set_width(260.0);
        ui.horizontal(|ui| {
            status_dot(ui, color);
            ui.label(theme::accent(title, theme::TEXT_SECONDARY));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                pill(ui, &rows.len().to_string(), color, false);
            });
        });
        ui.separator();
        if rows.is_empty() {
            ui.label(theme::muted("No projected rows"));
        }
        for (index, row) in rows.iter().take(5).enumerate() {
            theme::small_card_frame().show(ui, |ui| {
                ui.label(theme::body_strong(trim_middle(row, 54)));
                ui.horizontal(|ui| {
                    let label = format!("{}", index + 1);
                    avatar(ui, &label, color);
                    ui.label(theme::muted("metadata-only"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(theme::accent("review", color));
                    });
                });
                progress_bar(ui, 0.35 + (index as f32 * 0.13).min(0.55), color);
            });
        }
    });
}

fn render_manual_context_inspector(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    inspector_header(ui, "Context", DesktopProductMode::Manual);
    section_label(ui, "Current File", None);
    ui.label(theme::code(current_path(snapshot)));
    ui.label(theme::muted(format!(
        "{} lines projected",
        model.active_buffer_lines.len()
    )));
    section_label(ui, "Symbols", None);
    render_compact_rows(ui, &model.language_rows, "No language symbols", 5);
    section_label(ui, "Problems", None);
    ui.label(theme::muted(format!(
        "{} problems",
        snapshot.language_tooling_projection.problems.len()
    )));
    section_label(ui, "Manual Control Boundary", Some(theme::ACCENT_BLUE));
    render_compact_rows(
        ui,
        &model.manual_control_rows,
        "Manual controls are projection-only",
        5,
    );
    section_label(ui, "Git Changes", None);
    render_compact_rows(ui, &model.proposal_rows, "No projected changes", 5);
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
        if soft_button(ui, "Save All").clicked() {
            actions.push(DesktopAction::SaveAll);
        }
    });
}

fn render_assisted_inspector(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    inspector_header(ui, "Delegates", DesktopProductMode::Delegates);
    section_label(ui, "Current Selection", Some(theme::ACCENT_CYAN));
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::code(current_path(snapshot)));
        for row in model.active_buffer_lines.iter().take(5) {
            ui.label(theme::code_muted(trim_middle(row, 48)));
        }
    });
    section_label(ui, "Actions", None);
    if soft_button(ui, "Suggested Fixes").clicked() {
        actions.push(DesktopAction::StartAiProposal {
            instruction_label: "desktop suggested fixes".to_string(),
        });
    }
    if soft_button(ui, "Explain This Function").clicked() {
        actions.push(DesktopAction::StartAiExplain {
            instruction_label: "desktop explain function".to_string(),
        });
    }
    if soft_button(ui, "Generate Test").clicked() {
        actions.push(DesktopAction::StartAiProposal {
            instruction_label: "desktop generate test".to_string(),
        });
    }
    section_label(ui, "Recent Assists", None);
    render_compact_rows(ui, &model.assistant_rows, "No recent assistant activity", 5);
}

fn render_pair_session_panel(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    inspector_header(ui, "Delegate Session", DesktopProductMode::Delegates);
    section_label(ui, "Current Objective", Some(theme::ACCENT_BLUE));
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::body_strong(current_objective(snapshot)));
        ui.label(theme::code_muted(current_path(snapshot)));
    });
    section_label(ui, "AI Plan", None);
    render_compact_rows(ui, &model.assistant_rows, "No delegate plan projected", 6);
    section_label(ui, "Human Feedback", None);
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::muted(
            "Guide the delegate, revise the plan, or ask for alternatives...",
        ));
        if primary_button(ui, "Send", theme::ACCENT_BLUE).clicked() {
            actions.push(DesktopAction::StartAiExplain {
                instruction_label: "desktop pair feedback".to_string(),
            });
        }
    });
}

fn render_delegation_console(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    show_trust: &mut bool,
    actions: &mut Vec<DesktopAction>,
) {
    inspector_header(ui, "Delegation Console", DesktopProductMode::Delegates);
    section_label(ui, "Delegate Task", Some(theme::ACCENT_VIOLET));
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::muted("Delegate a scoped task to projected agents"));
        if primary_button(ui, "Delegate", theme::ACCENT_BLUE).clicked() {
            actions.push(DesktopAction::StartAiProposal {
                instruction_label: "desktop delegated task".to_string(),
            });
        }
    });
    section_label(ui, "Approval Queue", Some(theme::ACCENT_ORANGE));
    render_proposal_cards(ui, snapshot, actions);
    ui.checkbox(show_trust, "Trust details");
    if *show_trust {
        render_console_section(ui, "Trust", &model.trust_rows, "No trust warnings");
    }
}

fn render_fleet_console(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    inspector_header(
        ui,
        "Legion Workflow Control",
        DesktopProductMode::LegionWorkflows,
    );
    section_label(ui, "Current Directive", Some(theme::ACCENT_PURPLE));
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::body_strong(current_objective(snapshot)));
        ui.horizontal(|ui| {
            status_dot(ui, theme::ACCENT_GREEN);
            ui.label(theme::muted("Running"));
            ui.separator();
            ui.label(theme::muted("proposal-mediated"));
        });
    });
    section_label(ui, "Human Approval Queue", Some(theme::ACCENT_ORANGE));
    render_proposal_cards(ui, snapshot, actions);
    section_label(ui, "Agent Decision Feed", None);
    render_compact_rows(ui, &model.assistant_rows, "No agent decisions projected", 6);
    section_label(ui, "Risk Monitor", Some(theme::ACCENT_RED));
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::accent(
            "Build Status - projected",
            theme::ACCENT_GREEN,
        ));
        ui.label(theme::accent(
            format!(
                "{} proposal rows, {} trust rows",
                model.proposal_rows.len(),
                model.trust_rows.len()
            ),
            theme::ACCENT_AMBER,
        ));
    });
}

fn render_proposal_cards(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    if snapshot.proposal_ledger_projection.rows.is_empty() {
        ui.label(theme::muted("No pending proposals"));
        return;
    }
    for row in snapshot.proposal_ledger_projection.rows.iter().take(4) {
        theme::card_frame_tinted(theme::BG_RAISED, theme::dim(theme::ACCENT_ORANGE, 48)).show(
            ui,
            |ui| {
                ui.label(theme::body_strong(&row.title));
                ui.horizontal(|ui| {
                    ui.label(theme::muted(format!("{:?}", row.payload_kind)));
                    ui.separator();
                    ui.label(theme::accent(
                        format!("{:?} risk", row.risk_label),
                        risk_color(row.risk_label),
                    ));
                });
                ui.horizontal(|ui| {
                    if primary_button(ui, "Approve", theme::ACCENT_GREEN).clicked() {
                        actions.push(DesktopAction::ApproveProposal {
                            proposal_id: row.proposal_id,
                        });
                    }
                    if soft_button(ui, "Review").clicked() {
                        actions.push(DesktopAction::OpenProposalDetails {
                            proposal_id: row.proposal_id,
                        });
                    }
                    if soft_button(ui, "Reject").clicked() {
                        actions.push(DesktopAction::RejectProposal {
                            proposal_id: row.proposal_id,
                            reason: ProposalRejectionReason::UserRejected,
                        });
                    }
                });
            },
        );
    }
}

fn render_terminal_stream(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    section_label(ui, "Terminal / Runtime", Some(theme::ACCENT_CYAN));
    theme::code_frame().show(ui, |ui| {
        render_compact_rows(ui, &model.terminal_rows, "No terminal activity", 10);
        if model.terminal_rows.is_empty() {
            for row in model.bottom_console_rows.iter().take(4) {
                ui.label(theme::code_muted(row));
            }
        }
    });
}

fn render_agent_stream(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    section_label(ui, "Agent Comm Stream", Some(theme::ACCENT_VIOLET));
    theme::code_frame().show(ui, |ui| {
        if model.assistant_rows.is_empty() {
            render_compact_rows(ui, &model.manual_control_rows, "No agent stream rows", 4);
        } else {
            render_compact_rows(ui, &model.assistant_rows, "No agent stream rows", 8);
        }
        for row in model.operational_health_rows.iter().take(4) {
            ui.label(theme::code_muted(trim_middle(row, 88)));
        }
    });
}

fn render_compact_rows(ui: &mut egui::Ui, rows: &[String], empty: &str, limit: usize) {
    if rows.is_empty() {
        ui.label(theme::muted(empty));
        return;
    }
    for row in rows.iter().take(limit) {
        ui.label(theme::body(trim_middle(row, 110)));
    }
    if rows.len() > limit {
        ui.label(theme::muted(format!("{} more rows", rows.len() - limit)));
    }
}

fn sidebar_header(ui: &mut egui::Ui, title: &str, detail: String) {
    ui.horizontal(|ui| {
        ui.label(theme::eyebrow(title));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(theme::code_muted(trim_middle(&detail, 24)));
        });
    });
    ui.separator();
}

fn inspector_header(ui: &mut egui::Ui, title: &str, level: DesktopProductMode) {
    ui.horizontal(|ui| {
        ui.label(theme::heading(title));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            pill(ui, level.label(), level_color(level), true);
        });
    });
    ui.separator();
}

fn section_label(ui: &mut egui::Ui, label: &str, color: Option<egui::Color32>) {
    ui.add_space(6.0);
    match color {
        Some(color) => {
            ui.label(theme::accent(label, color));
        }
        None => {
            ui.label(theme::eyebrow(label));
        }
    }
}

fn console_tab(ui: &mut egui::Ui, label: &str, active: bool, color: egui::Color32) {
    if active {
        pill(ui, label, color, true);
    } else {
        ui.label(theme::muted(label));
    }
}

fn status_dot(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.0, color);
}

fn pill(ui: &mut egui::Ui, label: &str, color: egui::Color32, active: bool) {
    let fill = if active {
        theme::dim(color, 28)
    } else {
        theme::dim(theme::TEXT_PRIMARY, 10)
    };
    egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(
            1.0,
            if active {
                theme::dim(color, 90)
            } else {
                theme::BORDER_DEFAULT
            },
        ))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(theme::accent(label, color));
        });
}

fn level_pill(ui: &mut egui::Ui, level: &str, label: &str, color: egui::Color32, selected: bool) {
    let text = format!("{label} {level}");
    pill(
        ui,
        &text,
        if selected { color } else { theme::TEXT_MUTED },
        selected,
    );
}

fn avatar(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::NONE
        .fill(theme::dim(color, 30))
        .stroke(egui::Stroke::new(1.0, theme::dim(color, 90)))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.label(theme::accent(text, color));
        });
}

fn soft_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(theme::label(label))
            .fill(theme::BG_RAISED)
            .stroke(egui::Stroke::new(1.0, theme::BORDER_DEFAULT))
            .corner_radius(egui::CornerRadius::same(6)),
    )
}

fn primary_button(ui: &mut egui::Ui, label: &str, color: egui::Color32) -> egui::Response {
    ui.add(
        egui::Button::new(theme::inverse(label))
            .fill(color)
            .stroke(egui::Stroke::new(1.0, theme::dim(color, 180)))
            .corner_radius(egui::CornerRadius::same(6)),
    )
}

fn progress_bar(ui: &mut egui::Ui, value: f32, color: egui::Color32) {
    ui.add(
        egui::ProgressBar::new(value.clamp(0.0, 1.0))
            .fill(color)
            .desired_width(f32::INFINITY)
            .desired_height(4.0),
    );
}

fn agent_card(ui: &mut egui::Ui, title: &str, subtitle: &str, color: egui::Color32, progress: f32) {
    theme::small_card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            let initial = title
                .chars()
                .next()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "A".to_string());
            avatar(ui, &initial, color);
            ui.vertical(|ui| {
                ui.label(theme::body_strong(trim_middle(title, 30)));
                ui.label(theme::muted(trim_middle(subtitle, 34)));
            });
        });
        progress_bar(ui, progress, color);
    });
}

fn footer_metric(ui: &mut egui::Ui, label: &str, value: usize, color: egui::Color32) {
    ui.horizontal(|ui| {
        status_dot(ui, color);
        ui.label(theme::muted(label));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(theme::code(value.to_string()));
        });
    });
}

fn current_path(snapshot: &ShellProjectionSnapshot) -> &str {
    snapshot
        .active_buffer_projection
        .file_path
        .as_ref()
        .map(|path| path.0.as_str())
        .unwrap_or("<none>")
}

fn workspace_label(snapshot: &ShellProjectionSnapshot) -> String {
    snapshot
        .active_buffer_projection
        .workspace_id
        .map(|workspace| format!("workspace {}", workspace.0))
        .unwrap_or_else(|| "no workspace".to_string())
}

fn current_objective(snapshot: &ShellProjectionSnapshot) -> String {
    snapshot
        .proposal_ledger_projection
        .rows
        .first()
        .map(|row| row.title.clone())
        .or_else(|| {
            snapshot
                .delegated_task_projection
                .plan_rows
                .first()
                .map(|row| row.plan_id.0.clone())
        })
        .unwrap_or_else(|| {
            let path = current_path(snapshot);
            if path == "<none>" {
                "Inspect the workspace and keep changes proposal-mediated".to_string()
            } else {
                format!("Work on {path}")
            }
        })
}

fn first_proposal_id(snapshot: &ShellProjectionSnapshot) -> Option<ProposalId> {
    snapshot
        .proposal_ledger_projection
        .selected_proposal_id
        .or_else(|| {
            snapshot
                .proposal_ledger_projection
                .rows
                .first()
                .map(|row| row.proposal_id)
        })
}

fn delegated_plan_rows(
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    skip: usize,
) -> Vec<String> {
    let rows = snapshot
        .delegated_task_projection
        .plan_rows
        .iter()
        .skip(skip)
        .map(|row| {
            format!(
                "{} {:?} {:?} risk={:?}",
                row.plan_id.0, row.plan_state, row.readiness, row.risk_label
            )
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        model.assistant_rows.iter().take(3).cloned().collect()
    } else {
        rows
    }
}

fn delegated_step_rows(
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
) -> Vec<String> {
    let rows = snapshot
        .delegated_task_projection
        .step_summaries
        .iter()
        .map(|row| {
            format!(
                "{} order={} {:?} proposal={:?}",
                row.step_id.0,
                row.order,
                row.state,
                row.proposal_id.map(|proposal| proposal.0)
            )
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        model.main_canvas_rows.iter().take(3).cloned().collect()
    } else {
        rows
    }
}

fn proposal_board_rows(
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
) -> Vec<String> {
    let rows = snapshot
        .proposal_ledger_projection
        .rows
        .iter()
        .map(|row| {
            format!(
                "{} payload={:?} risk={:?} lifecycle={}",
                row.title, row.payload_kind, row.risk_label, row.lifecycle.label
            )
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        model.proposal_rows.iter().take(3).cloned().collect()
    } else {
        rows
    }
}

fn projected_agent_count(snapshot: &ShellProjectionSnapshot) -> usize {
    snapshot.assisted_ai_projection.provider_count as usize
        + snapshot.delegated_task_projection.plan_count as usize
        + snapshot.delegated_task_projection.step_summaries.len()
}

fn resource_load(snapshot: &ShellProjectionSnapshot, level: DesktopProductMode) -> usize {
    let base = match level {
        DesktopProductMode::Manual => 12,
        DesktopProductMode::Delegates => 42,
        DesktopProductMode::LegionWorkflows => 82,
    };
    (base + snapshot.terminal_panel_projection.output_rows.len()).min(99)
}

fn level_primary_action(level: DesktopProductMode) -> &'static str {
    match level {
        DesktopProductMode::Manual => "Save All",
        DesktopProductMode::Delegates => "Delegate",
        DesktopProductMode::LegionWorkflows => "Run Workflow",
    }
}

fn level_color(level: DesktopProductMode) -> egui::Color32 {
    match level {
        DesktopProductMode::Manual => theme::TEXT_MUTED,
        DesktopProductMode::Delegates => theme::ACCENT_VIOLET,
        DesktopProductMode::LegionWorkflows => theme::ACCENT_PURPLE,
    }
}

fn risk_color(risk: ProposalRiskLabel) -> egui::Color32 {
    match risk {
        ProposalRiskLabel::Informational => theme::ACCENT_CYAN,
        ProposalRiskLabel::Low => theme::ACCENT_GREEN,
        ProposalRiskLabel::Medium => theme::ACCENT_AMBER,
        ProposalRiskLabel::High => theme::ACCENT_RED,
        ProposalRiskLabel::Unknown => theme::TEXT_MUTED,
    }
}

fn trim_middle(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    if max <= 3 {
        return "...".to_string();
    }
    let keep = max - 3;
    let head = keep / 2;
    let tail = keep - head;
    let start = value.chars().take(head).collect::<String>();
    let end = value
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}...{end}")
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
enum DesktopProductMode {
    Manual,
    Delegates,
    LegionWorkflows,
}

impl DesktopProductMode {
    fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Delegates => "Delegates",
            Self::LegionWorkflows => "Legion Workflows",
        }
    }
}

fn projected_product_mode(snapshot: &ShellProjectionSnapshot) -> DesktopProductMode {
    let delegated = &snapshot.delegated_task_projection;
    if delegated.runtime_activation.is_encoded() && delegated.plan_count > 1 {
        return DesktopProductMode::LegionWorkflows;
    }
    if delegated.plan_count > 0
        || !delegated.plan_rows.is_empty()
        || !delegated.required_approvals.is_empty()
        || !delegated.proposal_preview_links.is_empty()
    {
        return DesktopProductMode::Delegates;
    }

    let assisted = &snapshot.assisted_ai_projection;
    if assisted.request_count > 0
        || assisted.preview_ready_count > 0
        || !assisted.proposal_previews.is_empty()
        || assisted.provider_count > 0
        || !assisted.providers.is_empty()
    {
        return DesktopProductMode::Delegates;
    }

    DesktopProductMode::Manual
}

fn product_mode_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let level = projected_product_mode(snapshot);
    let delegated = &snapshot.delegated_task_projection;
    let mut rows = vec![
        format!(
            "product mode: active={} read-only projection",
            level.label()
        ),
        "product modes: Manual | Delegates | Legion Workflows".to_string(),
    ];

    if level == DesktopProductMode::Delegates {
        rows.push(
            "product-mode safety: delegated work is approval-gated; direct workspace apply unsupported"
                .to_string(),
        );
    } else if level == DesktopProductMode::LegionWorkflows {
        rows.push(format!(
            "product-mode safety: Legion Workflow display from runtime={:?}; apply remains proposal-mediated",
            delegated.runtime_activation
        ));
    } else {
        rows.push("product-mode safety: Manual Mode has no AI dispatch path".to_string());
    }
    rows.push(
        "product-mode control: display-only; no provider, terminal, or apply authority".to_string(),
    );
    rows
}

fn delegated_activity_projected(snapshot: &ShellProjectionSnapshot) -> bool {
    snapshot.delegated_task_projection.plan_count > 0
        || !snapshot.delegated_task_projection.plan_rows.is_empty()
        || !snapshot.delegated_task_projection.step_summaries.is_empty()
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
            "command affordance: registry={} enabled={} search={} save_all={} proposal_controls={}",
            snapshot.command_registry_projection.commands.len(),
            snapshot
                .command_registry_projection
                .commands
                .iter()
                .filter(|command| command.enabled)
                .count(),
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
            "directive console: proposals={} artifacts={} trust_items={} approval_gates={} proposal-mediated",
            snapshot.proposal_ledger_projection.rows.len(),
            snapshot.artifact_ledger_projection.rows.len(),
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
            "agent stream: assisted_requests={} delegated_steps={} verification_runs={} graph_nodes={} shared_reviews={} remote_reviews={}",
            snapshot.assisted_ai_projection.request_count,
            snapshot.delegated_task_projection.step_summaries.len(),
            snapshot.verification_run_projection.rows.len(),
            snapshot.system_graph_projection.nodes.len(),
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

fn manual_control_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let level = projected_product_mode(snapshot);
    let language = &snapshot.language_tooling_projection;
    let terminal = &snapshot.terminal_panel_projection;
    let search = DesktopSearchViewModel::from_projection(&snapshot.search_projection);
    let active = &snapshot.active_buffer_projection;
    let mut rows = Vec::new();

    if level != DesktopProductMode::Manual {
        rows.push(format!(
            "manual control center: inactive because active product mode is {}",
            level.label()
        ));
        return rows;
    }

    rows.push(
        "manual control center: AI Disabled; Local Tools Only; No Model Calls; No Agent Context"
            .to_string(),
    );
    rows.push(format!(
        "manual toolchain: language={:?} problems={} completions={} terminal={:?} search={} verification_runs={}",
        language.status,
        language.problems.len(),
        language.completions.len(),
        terminal.status.kind,
        search.header,
        snapshot.verification_run_projection.rows.len()
    ));
    rows.push(format!(
        "manual commands: save_all proposal-mediated; search/read/navigation intents only; no direct apply; statuses={}",
        snapshot.status_messages.len()
    ));
    rows.push(format!(
        "manual editor: dirty={} degraded={} active_buffer={:?} no autonomous writes",
        active.dirty,
        active.degraded,
        active.buffer_id.map(|buffer| buffer.0)
    ));
    rows.push(
        "manual trust boundary: no provider dispatch, no agent context, no terminal authority, no direct apply"
            .to_string(),
    );
    rows
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
