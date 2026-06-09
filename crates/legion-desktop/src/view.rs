//! Projection rendering for the desktop adapter.

use std::collections::{BTreeSet, HashSet};

use legion_protocol::{
    DelegatedTaskProposalHunkDisposition, DelegatedTaskToolPermissionDecision, FileId,
    PRODUCT_NAME, PluginCommandDescriptor, PluginContribution, PluginContributionProjection,
    ProductRuntimeSurface, ProposalId, ProposalRejectionReason, ProposalRiskLabel, TextCoordinate,
    ViewportProjectionMode, product_mode_allows_runtime_surface,
};
use legion_ui::{
    ActiveBufferProjection, DockLayout, DockMode, DockSide, DockSideLayout, PanelId, PanelRegistry,
    ShellProjectionSnapshot, StatusSeverity,
};

use crate::{
    bridge::DesktopAction, health::DesktopOperationalHealthSnapshot,
    search::DesktopSearchViewModel, theme,
};

/// Adapter-local view state layered over app-owned projections.
#[derive(Debug, Clone, PartialEq)]
pub struct DesktopProjectionViewState {
    /// Canonical explorer paths currently expanded by the adapter.
    pub expanded_explorer_paths: BTreeSet<String>,
    /// Adapter-local explorer selection override, if a native control is ahead of projection.
    pub selected_explorer_file: Option<FileId>,
    /// Adapter-local mode-scoped dock layouts.
    pub dock_layouts: Vec<DockLayout>,
}

impl Default for DesktopProjectionViewState {
    fn default() -> Self {
        Self {
            expanded_explorer_paths: BTreeSet::new(),
            selected_explorer_file: None,
            dock_layouts: DockLayout::standard_all_modes(),
        }
    }
}

/// Structured status-bar projection derived from app-owned shell data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopStatusBarViewModel {
    /// Active product-mode label.
    pub product_mode: String,
    /// Display-safe state flags such as dirty, degraded, or no active buffer.
    pub flags: Vec<String>,
    /// Active file path when a file-backed buffer is selected.
    pub path: Option<String>,
    /// Active workspace identifier.
    pub workspace_id: Option<u128>,
    /// Active file identifier.
    pub file_id: Option<u128>,
    /// Active buffer identifier.
    pub buffer_id: Option<u128>,
    /// Text encoding when an active text buffer exists.
    pub encoding: Option<String>,
    /// Detected line ending when the bounded projection has enough evidence.
    pub line_ending: Option<String>,
    /// Primary cursor position from the bounded viewport projection.
    pub cursor: Option<DesktopStatusCursor>,
    /// Language label inferred from the active file path.
    pub language: Option<String>,
    /// Real connection state when projected by the application layer.
    pub connection: Option<String>,
}

/// One-based cursor display coordinates for the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopStatusCursor {
    /// One-based line number.
    pub line: u32,
    /// One-based column number.
    pub column: u32,
}

impl DesktopStatusBarViewModel {
    fn from_snapshot(snapshot: &ShellProjectionSnapshot, flags: &[String]) -> Self {
        let active = &snapshot.active_buffer_projection;
        Self {
            product_mode: snapshot.product_mode.label().to_string(),
            flags: flags.to_vec(),
            path: active.file_path.as_ref().map(|path| path.0.clone()),
            workspace_id: active.workspace_id.map(|workspace| workspace.0),
            file_id: active.file_id.map(|file| file.0),
            buffer_id: active.buffer_id.map(|buffer| buffer.0),
            encoding: active.buffer_id.map(|_| "UTF-8".to_string()),
            line_ending: status_line_ending(active),
            cursor: active
                .viewport
                .as_ref()
                .map(|viewport| DesktopStatusCursor {
                    line: viewport.cursor.line.saturating_add(1),
                    column: viewport.cursor.character.saturating_add(1),
                }),
            language: active
                .file_path
                .as_ref()
                .map(|path| status_language_for_path(&path.0)),
            connection: None,
        }
    }

    fn state_label(&self) -> String {
        if self.flags.is_empty() {
            "clean".to_string()
        } else {
            self.flags.join(",")
        }
    }
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
    /// Four-step autonomy scale rows derived from the active product mode.
    pub autonomy_scale_rows: Vec<String>,
    /// Mode escalation confirmation and permission rows.
    pub mode_confirmation_rows: Vec<String>,
    /// Command-palette group and item rows.
    pub command_palette_rows: Vec<String>,
    /// Left sidebar summary rows.
    pub left_sidebar_rows: Vec<String>,
    /// Main code-canvas summary rows.
    pub main_canvas_rows: Vec<String>,
    /// Right dock directive and trust summary rows.
    pub directive_panel_rows: Vec<String>,
    /// Bottom operational console rows.
    pub bottom_console_rows: Vec<String>,
    /// Mode-specific bottom tab rows.
    pub bottom_tab_rows: Vec<String>,
    /// Active dock registry/layout summary rows.
    pub dock_rows: Vec<String>,
    /// Visible dock panel rows after mode filtering.
    pub dock_panel_rows: Vec<String>,
    /// Compact status-bar projection.
    pub status_bar: DesktopStatusBarViewModel,
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
    /// Legion workflow command-center rows.
    pub legion_workflow_rows: Vec<String>,
    /// Language tooling summary rows.
    pub language_rows: Vec<String>,
    /// Structural search and replace summary rows.
    pub structural_search_rows: Vec<String>,
    /// Git status, diff, blame, graph, and conflict rows.
    pub git_rows: Vec<String>,
    /// Terminal panel summary rows.
    pub terminal_rows: Vec<String>,
    /// Debugger summary rows.
    pub debug_rows: Vec<String>,
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
        let autonomy_scale_rows = autonomy_scale_rows(snapshot);
        let mode_confirmation_rows = mode_confirmation_rows(snapshot);
        let command_palette_rows = command_palette_rows(snapshot);
        let dock_rows = dock_rows(snapshot, state);
        let dock_panel_rows = dock_panel_rows(snapshot, state);
        Self {
            layout_title: snapshot.layout_projection.layout.title.clone(),
            top_bar_rows: top_bar_rows(snapshot, &flags),
            product_mode_rows,
            autonomy_scale_rows,
            mode_confirmation_rows,
            command_palette_rows,
            left_sidebar_rows: left_sidebar_rows(snapshot),
            main_canvas_rows: main_canvas_rows(snapshot),
            directive_panel_rows: directive_panel_rows(snapshot),
            bottom_console_rows: bottom_console_rows(snapshot),
            bottom_tab_rows: bottom_tab_rows(snapshot),
            dock_rows,
            dock_panel_rows,
            status_bar: DesktopStatusBarViewModel::from_snapshot(snapshot, &flags),
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
            legion_workflow_rows: legion_workflow_rows(snapshot),
            language_rows: language_rows(snapshot),
            structural_search_rows: structural_search_rows(snapshot),
            git_rows: git_rows(snapshot),
            terminal_rows: terminal_rows(snapshot),
            debug_rows: debug_rows(snapshot),
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
#[derive(Debug)]
pub struct ProjectionView {
    show_trust: bool,
    show_auxiliary: bool,
    theme_preference: theme::ThemePreference,
}

impl Default for ProjectionView {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectionView {
    /// Creates a projection view with no product-state ownership.
    pub fn new() -> Self {
        Self {
            show_trust: true,
            show_auxiliary: true,
            theme_preference: theme::ThemePreference::all()[0],
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
        let active_theme = self.theme_preference.resolve(ui.ctx());
        theme::install(ui.ctx(), &active_theme);
        let model = DesktopProjectionViewModel::from_snapshot_with_state(snapshot, state);
        let mut actions = Vec::new();

        egui::Panel::top("legion_desktop_top")
            .exact_size(52.0)
            .frame(theme::toolbar_frame())
            .show_inside(ui, |ui| {
                render_top_command_bar(ui, snapshot, &model, &mut actions);
            });

        egui::Panel::left("legion_desktop_explorer")
            .default_size(272.0)
            .min_size(236.0)
            .resizable(true)
            .frame(theme::pane_frame(theme::tokens().bg.panel))
            .show_inside(ui, |ui| {
                render_left_sidebar(ui, snapshot, state, &model, &mut actions);
            });

        egui::Panel::bottom("legion_desktop_status")
            .exact_size(24.0)
            .frame(theme::panel_frame(theme::tokens().bg.code))
            .show_inside(ui, |ui| {
                render_status_bar(ui, &model);
            });

        let bottom_height = match projected_product_mode(snapshot) {
            DesktopProductMode::Manual => 150.0,
            DesktopProductMode::Assist => 180.0,
            DesktopProductMode::LegionWorkflows => 240.0,
            DesktopProductMode::Delegates => 200.0,
        };
        egui::Panel::bottom("legion_desktop_bottom_console")
            .default_size(bottom_height)
            .min_size(112.0)
            .resizable(true)
            .frame(theme::pane_frame(theme::tokens().bg.code))
            .show_inside(ui, |ui| {
                render_bottom_console(ui, snapshot, &model);
            });

        let right_width = match projected_product_mode(snapshot) {
            DesktopProductMode::Manual => 260.0,
            DesktopProductMode::Assist => 340.0,
            DesktopProductMode::Delegates | DesktopProductMode::LegionWorkflows => 380.0,
        };
        egui::Panel::right("legion_desktop_trust")
            .default_size(right_width)
            .min_size(260.0)
            .resizable(true)
            .frame(theme::pane_frame(theme::tokens().bg.panel))
            .show_inside(ui, |ui| {
                render_right_dock(
                    ui,
                    snapshot,
                    &model,
                    &mut self.show_trust,
                    &mut self.show_auxiliary,
                    &mut actions,
                );
            });

        egui::CentralPanel::default()
            .frame(theme::pane_frame(theme::tokens().bg.code))
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
        ui.label(theme::title(PRODUCT_NAME));
        ui.separator();
        ui.label(theme::body_strong(&model.layout_title));
        render_branch_pill(ui, snapshot);
        render_engine_status(ui, snapshot, level);
        ui.add_space(12.0);
        render_product_mode_switch(ui, level, actions);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            avatar(ui, "MK", theme::tokens().text.secondary);
            if soft_button(ui, "Open").clicked() {
                actions.push(DesktopAction::ShowOpenPathPrompt);
            }
            if soft_button(ui, "Search").clicked() {
                actions.push(DesktopAction::ShowSearchPrompt {
                    scope: snapshot.search_projection.scope,
                });
            }
            if soft_button(ui, "SSR").clicked() {
                actions.push(DesktopAction::ShowStructuralSearchPrompt {
                    scope: snapshot.structural_search_projection.scope,
                });
            }
            if soft_button(ui, "Git").clicked() {
                actions.push(DesktopAction::RefreshGit);
            }
            if primary_button(ui, level_primary_action(level), level_color(level)).clicked() {
                actions.push(match level {
                    DesktopProductMode::Manual => DesktopAction::SaveAll,
                    DesktopProductMode::Assist => DesktopAction::StartAiProposal {
                        instruction_label: "desktop assist".to_string(),
                    },
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

fn render_product_mode_switch(
    ui: &mut egui::Ui,
    active_level: DesktopProductMode,
    actions: &mut Vec<DesktopAction>,
) {
    let tokens = theme::tokens();
    theme::card_frame_tinted(tokens.bg.input, tokens.border.default).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::eyebrow("PRODUCT MODE"));
            for (mode, level, label, color) in [
                (DesktopProductMode::Manual, "M", "Manual", tokens.text.muted),
                (
                    DesktopProductMode::Assist,
                    "A",
                    "Assist",
                    tokens.accent.cyan,
                ),
                (
                    DesktopProductMode::Delegates,
                    "D",
                    "Delegates",
                    tokens.accent.violet,
                ),
                (
                    DesktopProductMode::LegionWorkflows,
                    "W",
                    "Legion Workflows",
                    tokens.accent.purple,
                ),
            ] {
                let response = level_pill(ui, level, label, color, mode == active_level);
                if response.clicked() && mode != active_level {
                    actions.push(DesktopAction::SetProductMode {
                        mode: mode.to_dock_mode(),
                    });
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
    let level = projected_product_mode(snapshot);
    sidebar_header(ui, "PROJECT", workspace_label(snapshot));
    render_dock_side_summary(ui, DockSide::Left, model);
    if level == DesktopProductMode::LegionWorkflows {
        render_context_packs(ui);
    } else {
        render_project_tree_panel(ui, snapshot, state, level, actions);
    }

    match level {
        DesktopProductMode::Manual => render_collapsed_ai_rail(ui, model),
        DesktopProductMode::Assist => {
            if snapshot.assisted_ai_projection.preview_ready_count > 0 {
                render_session_context(ui, snapshot)
            } else {
                render_assistance_toggles(ui)
            }
        }
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

    if !model.git_rows.is_empty() {
        section_label(ui, "Git", Some(theme::tokens().accent.green));
        render_compact_rows(ui, &model.git_rows, "No projected git rows", 5);
    }

    if !snapshot.git_projection.conflicts.is_empty() {
        section_label(ui, "Conflicts", Some(theme::tokens().accent.red));
        for conflict in snapshot.git_projection.conflicts.iter().take(4) {
            ui.horizontal(|ui| {
                ui.label(theme::body(trim_middle(&conflict.path, 24)));
                if soft_button(ui, "Current").clicked() {
                    actions.push(DesktopAction::AcceptGitConflictCurrent {
                        path: conflict.path.clone(),
                    });
                }
                if soft_button(ui, "Incoming").clicked() {
                    actions.push(DesktopAction::AcceptGitConflictIncoming {
                        path: conflict.path.clone(),
                    });
                }
            });
        }
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
        DesktopProductMode::Assist => {
            if snapshot.assisted_ai_projection.preview_ready_count > 0 {
                render_copilot_canvas(ui, snapshot, model, actions)
            } else {
                render_editor_canvas(ui, snapshot, model, actions)
            }
        }
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

fn render_right_dock(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    show_trust: &mut bool,
    _show_auxiliary: &mut bool,
    actions: &mut Vec<DesktopAction>,
) {
    render_dock_side_summary(ui, DockSide::Right, model);
    match projected_product_mode(snapshot) {
        DesktopProductMode::Manual => render_manual_context_inspector(ui, snapshot, model, actions),
        DesktopProductMode::Assist => {
            if snapshot.assisted_ai_projection.preview_ready_count > 0 {
                render_pair_session_panel(ui, snapshot, model, actions)
            } else {
                render_assisted_inspector(ui, snapshot, model, actions)
            }
        }
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

fn render_bottom_console(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
) {
    ui.horizontal(|ui| {
        for tab in bottom_tab_specs(snapshot) {
            let label = if let Some(count) = tab.count {
                format!("{} ({count})", tab.label)
            } else {
                tab.label.to_string()
            };
            console_tab(ui, &label, tab.active, tab.color);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let channel = match projected_product_mode(snapshot) {
                DesktopProductMode::LegionWorkflows => "live",
                _ => "bash",
            };
            ui.label(theme::code_muted(channel));
        });
    });
    ui.separator();
    render_dock_side_summary(ui, DockSide::Bottom, model);
    match projected_product_mode(snapshot) {
        DesktopProductMode::Manual => {
            ui.columns(2, |columns| {
                render_terminal_stream(&mut columns[0], model);
                render_console_section(
                    &mut columns[1],
                    "Structural Search",
                    &model.structural_search_rows,
                    "No structural search results",
                );
                if !model.debug_rows.is_empty() {
                    section_label(
                        &mut columns[1],
                        "Debug",
                        Some(theme::tokens().accent.orange),
                    );
                    render_compact_rows(
                        &mut columns[1],
                        &model.debug_rows,
                        "No projected debug state",
                        6,
                    );
                }
            });
        }
        DesktopProductMode::Assist => {
            ui.columns(2, |columns| {
                render_terminal_stream(&mut columns[0], model);
                render_agent_stream(&mut columns[1], model);
            });
        }
        DesktopProductMode::Delegates => {
            ui.columns(2, |columns| {
                render_console_section(
                    &mut columns[0],
                    "Test Runner",
                    &model.operational_health_rows,
                    "No delegated verification rows",
                );
                render_agent_stream(&mut columns[1], model);
            });
        }
        DesktopProductMode::LegionWorkflows => {
            ui.columns(2, |columns| {
                render_agent_stream(&mut columns[0], model);
                render_terminal_stream(&mut columns[1], model);
            });
        }
    }
}

fn render_status_bar(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    let status = &model.status_bar;
    ui.set_height(24.0);
    ui.horizontal(|ui| {
        if let Some(connection) = &status.connection {
            ui.label(theme::accent(connection, theme::tokens().accent.green));
            ui.separator();
        }
        let state_color = if status.flags.iter().any(|flag| flag == "dirty") {
            theme::tokens().accent.amber
        } else if status.flags.iter().any(|flag| flag == "degraded") {
            theme::tokens().accent.orange
        } else if status.flags.iter().any(|flag| flag == "no_active_buffer") {
            theme::tokens().text.muted
        } else {
            theme::tokens().accent.green
        };
        ui.label(theme::accent(status.state_label(), state_color));
        if let Some(path) = &status.path {
            ui.separator();
            ui.label(theme::code_muted(trim_middle(path, 56)));
        }
        if let Some(buffer_id) = status.buffer_id {
            ui.separator();
            ui.label(theme::code_muted(format!("buf {buffer_id}")));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(theme::accent(
                &status.product_mode,
                theme::tokens().accent.violet,
            ));
            if let Some(encoding) = &status.encoding {
                ui.label(theme::code_muted(encoding));
            }
            if let Some(line_ending) = &status.line_ending {
                ui.label(theme::code_muted(line_ending));
            }
            if let Some(cursor) = status.cursor {
                ui.label(theme::code_muted(format!(
                    "Ln {}, Col {}",
                    cursor.line, cursor.column
                )));
            }
            if let Some(language) = &status.language {
                ui.label(theme::code_muted(language));
            }
        });
    });
}

fn render_console_section(ui: &mut egui::Ui, title: &str, rows: &[String], empty: &str) {
    section_label(ui, title, None);
    theme::small_card_frame().show(ui, |ui| {
        render_compact_rows(ui, rows, empty, 6);
    });
}

fn render_dock_side_summary(ui: &mut egui::Ui, side: DockSide, model: &DesktopProjectionViewModel) {
    let side_label = format!("dock side: {}", side.label());
    let side_row = model
        .dock_rows
        .iter()
        .find(|row| row.starts_with(&side_label));
    let panel_prefix = format!("dock panel: side={}", side.label());
    let panels = model
        .dock_panel_rows
        .iter()
        .filter(|row| row.starts_with(&panel_prefix))
        .take(4)
        .cloned()
        .collect::<Vec<_>>();

    if let Some(row) = side_row {
        theme::ghost_frame().show(ui, |ui| {
            ui.label(theme::code_muted(trim_middle(row, 96)));
            render_compact_rows(ui, &panels, "No visible dock panels", 4);
        });
        ui.add_space(4.0);
    }
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
        status_dot(ui, theme::tokens().accent.red);
        status_dot(ui, theme::tokens().accent.amber);
        status_dot(ui, theme::tokens().accent.green);
    });
}

fn render_branch_pill(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    let path = current_path(snapshot);
    let label = if path == "<none>" {
        "workspace"
    } else {
        path.rsplit(['/', '\\']).next().unwrap_or(path)
    };
    pill(
        ui,
        &format!("branch - {label}"),
        theme::tokens().text.muted,
        false,
    );
}

fn render_engine_status(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    level: DesktopProductMode,
) {
    let label = match level {
        DesktopProductMode::Manual => "Engine idle",
        DesktopProductMode::Assist => "Assist active",
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
                theme::tokens().accent.cyan,
            ));
            ui.separator();
            ui.label(theme::accent(
                format!("{} tests", snapshot.status_messages.len()),
                theme::tokens().accent.green,
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
        .id_salt("legion_desktop_explorer_scroll")
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
    section_label(ui, "Context Packs", Some(theme::tokens().accent.purple));
    for pack in [
        "Auth system",
        "Billing model",
        "API routes",
        "Test suite",
        "Deployment config",
    ] {
        theme::ghost_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                status_dot(ui, theme::tokens().text.muted);
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
                ui.label(theme::accent("AI disabled", theme::tokens().accent.blue));
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
    section_label(ui, "AI Assistance", Some(theme::tokens().accent.cyan));
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
                pill(ui, "on", theme::tokens().accent.cyan, true);
            });
        });
    }
}

fn render_session_context(ui: &mut egui::Ui, snapshot: &ShellProjectionSnapshot) {
    ui.add_space(8.0);
    section_label(ui, "Session Context", Some(theme::tokens().accent.blue));
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
        theme::tokens().accent.amber,
    );
    footer_metric(
        ui,
        "tests",
        snapshot.status_messages.len(),
        theme::tokens().accent.green,
    );
    footer_metric(
        ui,
        "workflows",
        snapshot.delegated_task_projection.plan_count as usize,
        theme::tokens().accent.cyan,
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
            .id_salt("legion_desktop_code_canvas_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                render_code_lines(ui, model);
            });
    });
    if level == DesktopProductMode::Assist {
        render_assisted_suggestion_panel(ui, snapshot, model, actions);
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
    theme::pane_frame(theme::tokens().bg.panel).show(ui, |ui| {
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
                    theme::tokens().text.primary
                } else {
                    theme::tokens().text.muted
                };
                let response = ui.add(
                    egui::Button::new(theme::accent(title, color))
                        .fill(if tab.active {
                            theme::tokens().bg.code
                        } else {
                            theme::tokens().bg.panel
                        })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if tab.active {
                                theme::tokens().border.default
                            } else {
                                theme::tokens().bg.panel
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
    let language = &snapshot.language_tooling_projection;
    theme::pane_frame(theme::tokens().bg.code).show(ui, |ui| {
        ui.set_height(28.0);
        ui.horizontal(|ui| {
            ui.label(theme::code_muted("src"));
            ui.label(theme::muted(">"));
            ui.label(theme::code(current_path(snapshot)));
            for breadcrumb in language.breadcrumbs.iter().take(4) {
                ui.label(theme::muted(">"));
                ui.label(theme::code_muted(&breadcrumb.label));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(theme::muted("TS - LF - UTF-8"));
                ui.label(theme::accent("checks ready", theme::tokens().accent.green));
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

fn render_assisted_suggestion_panel(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    actions: &mut Vec<DesktopAction>,
) {
    ui.add_space(8.0);
    theme::card_frame_tinted(
        theme::tokens().bg.card,
        theme::dim(theme::tokens().accent.cyan, 80),
    )
    .show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::accent("Suggestions", theme::tokens().accent.cyan));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if soft_button(ui, "Predict").clicked() {
                    actions.push(DesktopAction::RequestAssistInlinePrediction {
                        position: projected_cursor(snapshot),
                    });
                }
                if snapshot
                    .assist_inline_prediction_projection
                    .request_in_flight
                    && soft_button(ui, "Cancel").clicked()
                {
                    actions.push(DesktopAction::CancelAssistInlinePrediction);
                }
            });
        });
        if snapshot
            .assist_inline_prediction_projection
            .active_prediction
            .is_some()
        {
            ui.horizontal(|ui| {
                if primary_button(ui, "Accept", theme::tokens().accent.green).clicked() {
                    actions.push(DesktopAction::AcceptCurrentAssistInlinePrediction);
                }
                if soft_button(ui, "Dismiss").clicked() {
                    actions.push(DesktopAction::DismissCurrentAssistInlinePrediction);
                }
            });
        }
        let inline_rows = model
            .assistant_rows
            .iter()
            .filter(|row| row.contains("inline prediction"))
            .take(4)
            .cloned()
            .collect::<Vec<_>>();
        if inline_rows.is_empty() {
            for action in [
                "Refactor validation into helper",
                "Add null-check for selected value",
                "Generate unit test",
            ] {
                ui.label(theme::body(action));
            }
        } else {
            render_compact_rows(ui, &inline_rows, "No projected inline predictions", 4);
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
            section_label(
                ui,
                current_path(snapshot),
                Some(theme::tokens().accent.blue),
            );
            egui::ScrollArea::both().show(ui, |ui| render_code_lines(ui, model));
        });
        theme::code_frame().show(&mut columns[1], |ui| {
            ui.horizontal(|ui| {
                section_label(ui, "Proposed changes", Some(theme::tokens().accent.violet));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if primary_button(ui, "Apply all", theme::tokens().accent.blue).clicked()
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
    theme::pane_frame(theme::tokens().bg.panel).show(ui, |ui| {
        ui.set_height(40.0);
        ui.horizontal(|ui| {
            section_label(ui, "Delegate Plan", Some(theme::tokens().accent.blue));
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
                        theme::tokens().accent.green
                    } else {
                        theme::tokens().text.muted
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
            theme::tokens().accent.green
        } else {
            theme::tokens().accent.violet
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
    theme::pane_frame(theme::tokens().bg.code).show(ui, |ui| {
        ui.set_height(220.0);
        ui.horizontal(|ui| {
            section_label(
                ui,
                "Delegated Diff Review",
                Some(theme::tokens().accent.violet),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary_button(ui, "Approve", theme::tokens().accent.blue).clicked()
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
        render_delegated_hunk_review_controls(ui, snapshot, actions);
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
    theme::pane_frame(theme::tokens().bg.panel).show(ui, |ui| {
        ui.set_height(84.0);
        ui.horizontal(|ui| {
            avatar(ui, "LW", theme::tokens().accent.purple);
            ui.vertical(|ui| {
                ui.label(theme::accent(
                    "MASTER DIRECTIVE",
                    theme::tokens().accent.purple,
                ));
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
            ui.label(theme::accent(
                "confidence 87%",
                theme::tokens().accent.green,
            ));
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
        .id_salt("legion_desktop_task_board")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_height(board_height);
            ui.horizontal_top(|ui| {
                task_column(
                    ui,
                    "ASSIGNED",
                    theme::tokens().text.muted,
                    delegated_plan_rows(snapshot, model, 0),
                    actions,
                );
                task_column(
                    ui,
                    "IN PROGRESS",
                    theme::tokens().accent.blue,
                    delegated_step_rows(snapshot, model),
                    actions,
                );
                task_column(
                    ui,
                    "WAITING ON HUMAN",
                    theme::tokens().accent.orange,
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
                    theme::tokens().accent.violet,
                    model.language_rows.iter().take(4).cloned().collect(),
                    actions,
                );
                task_column(
                    ui,
                    "DONE",
                    theme::tokens().accent.green,
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
    theme::card_frame_tinted(theme::tokens().bg.canvas, theme::tokens().border.subtle).show(
        ui,
        |ui| {
            ui.set_width(260.0);
            ui.horizontal(|ui| {
                status_dot(ui, color);
                ui.label(theme::accent(title, theme::tokens().text.secondary));
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
        },
    );
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
    section_label(ui, "Debug", Some(theme::tokens().accent.orange));
    render_compact_rows(ui, &model.debug_rows, "No projected debug state", 6);
    section_label(ui, "Structural Search", Some(theme::tokens().accent.cyan));
    render_compact_rows(
        ui,
        &model.structural_search_rows,
        "No structural search preview",
        6,
    );
    if soft_button(ui, "Pattern").clicked() {
        actions.push(DesktopAction::ShowStructuralSearchPrompt {
            scope: snapshot.structural_search_projection.scope,
        });
    }
    section_label(
        ui,
        "Manual Control Boundary",
        Some(theme::tokens().accent.blue),
    );
    render_compact_rows(
        ui,
        &model.manual_control_rows,
        "Manual controls are projection-only",
        5,
    );
    section_label(ui, "Git Changes", None);
    render_compact_rows(ui, &model.git_rows, "No projected git changes", 6);
    if soft_button(ui, "Refresh Git").clicked() {
        actions.push(DesktopAction::RefreshGit);
    }
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
    section_label(ui, "Current Selection", Some(theme::tokens().accent.cyan));
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
    section_label(ui, "Current Objective", Some(theme::tokens().accent.blue));
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
        if primary_button(ui, "Send", theme::tokens().accent.blue).clicked() {
            actions.push(DesktopAction::SendDelegateChat {
                prompt_label: "desktop pair feedback".to_string(),
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
    section_label(ui, "Delegate Task", Some(theme::tokens().accent.violet));
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::muted("Delegate a scoped task to projected agents"));
        if primary_button(ui, "Delegate", theme::tokens().accent.blue).clicked() {
            actions.push(DesktopAction::StartAiProposal {
                instruction_label: "desktop delegated task".to_string(),
            });
        }
        if soft_button(ui, "Chat").clicked() {
            actions.push(DesktopAction::SendDelegateChat {
                prompt_label: "desktop delegated context".to_string(),
            });
        }
    });
    section_label(ui, "Approval Queue", Some(theme::tokens().accent.orange));
    render_proposal_cards(ui, snapshot, actions);
    render_delegated_tool_permission_controls(ui, snapshot, actions);
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
    section_label(ui, "Current Directive", Some(theme::tokens().accent.purple));
    theme::small_card_frame().show(ui, |ui| {
        ui.label(theme::body_strong(current_objective(snapshot)));
        ui.horizontal(|ui| {
            status_dot(ui, theme::tokens().accent.green);
            ui.label(theme::muted("Running"));
            ui.separator();
            ui.label(theme::muted("proposal-mediated"));
        });
    });
    section_label(
        ui,
        "Human Approval Queue",
        Some(theme::tokens().accent.orange),
    );
    render_proposal_cards(ui, snapshot, actions);
    render_delegated_tool_permission_controls(ui, snapshot, actions);
    render_legion_workflow_tool_permission_controls(ui, snapshot, actions);
    render_legion_workflow_kill_switch_controls(ui, snapshot, actions);
    section_label(ui, "Agent Decision Feed", None);
    render_compact_rows(
        ui,
        &model.legion_workflow_rows,
        "No agent decisions projected",
        8,
    );
    section_label(ui, "Risk Monitor", Some(theme::tokens().accent.red));
    theme::small_card_frame().show(ui, |ui| {
        if snapshot.legion_workflow_projection.risk_monitors.is_empty() {
            ui.label(theme::muted("No risk monitor rows"));
        } else {
            for monitor in snapshot
                .legion_workflow_projection
                .risk_monitors
                .iter()
                .take(3)
            {
                ui.label(theme::accent(
                    format!(
                        "{} {:?} score={}/{} high_risk={} denied={}",
                        monitor.session_id.0,
                        monitor.state,
                        monitor.risk_score,
                        monitor.halt_threshold,
                        monitor.high_risk_action_count,
                        monitor.denied_tool_count
                    ),
                    if monitor.state == legion_protocol::LegionWorkflowRiskMonitorState::Halted {
                        theme::tokens().accent.red
                    } else {
                        theme::tokens().accent.green
                    },
                ));
            }
        }
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
        theme::card_frame_tinted(
            theme::tokens().bg.card,
            theme::dim(theme::tokens().accent.orange, 48),
        )
        .show(ui, |ui| {
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
                if primary_button(ui, "Approve", theme::tokens().accent.green).clicked() {
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
        });
    }
}

fn render_delegated_hunk_review_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let reviews = &snapshot.delegated_task_projection.proposal_reviews;
    if reviews.is_empty() {
        return;
    }
    section_label(ui, "Hunk Review", Some(theme::tokens().accent.violet));
    for review in reviews.iter().take(4) {
        theme::small_card_frame().show(ui, |ui| {
            ui.label(theme::body_strong(format!(
                "proposal {} accepted={} rejected={} pending={}",
                review.proposal_id.0,
                review.accepted_hunk_count,
                review.rejected_hunk_count,
                review.pending_hunk_count
            )));
            for hunk in review.hunks.iter().take(6) {
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::code_muted(trim_middle(&hunk.hunk_id, 36)));
                    ui.label(theme::muted(format!("{:?}", hunk.disposition)));
                    if soft_button(ui, "Accept").clicked() {
                        actions.push(DesktopAction::ReviewDelegateProposalHunk {
                            proposal_id: review.proposal_id,
                            hunk_id: hunk.hunk_id.clone(),
                            disposition: DelegatedTaskProposalHunkDisposition::Accepted,
                        });
                    }
                    if soft_button(ui, "Reject").clicked() {
                        actions.push(DesktopAction::ReviewDelegateProposalHunk {
                            proposal_id: review.proposal_id,
                            hunk_id: hunk.hunk_id.clone(),
                            disposition: DelegatedTaskProposalHunkDisposition::Rejected,
                        });
                    }
                    if soft_button(ui, "Pending").clicked() {
                        actions.push(DesktopAction::ReviewDelegateProposalHunk {
                            proposal_id: review.proposal_id,
                            hunk_id: hunk.hunk_id.clone(),
                            disposition: DelegatedTaskProposalHunkDisposition::Pending,
                        });
                    }
                });
            }
        });
    }
}

fn render_delegated_tool_permission_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let requests = &snapshot.delegated_task_projection.tool_permission_requests;
    if requests.is_empty() {
        ui.label(theme::muted("No Delegate tool permissions"));
        return;
    }
    for request in requests.iter().take(6) {
        theme::small_card_frame().show(ui, |ui| {
            ui.label(theme::body_strong(trim_middle(&request.request_id, 56)));
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted(format!("{:?}", request.profile)));
                ui.separator();
                ui.label(theme::muted(format!("{:?}", request.action_class)));
                ui.separator();
                ui.label(theme::accent(
                    format!("{:?}", request.disposition),
                    if request.deny_overrides {
                        theme::tokens().accent.red
                    } else if request.runtime_allowed {
                        theme::tokens().accent.green
                    } else {
                        theme::tokens().accent.orange
                    },
                ));
            });
            ui.horizontal_wrapped(|ui| {
                for (label, decision) in [
                    ("Confirm", DelegatedTaskToolPermissionDecision::Confirm),
                    ("Allow", DelegatedTaskToolPermissionDecision::Allow),
                    ("Deny", DelegatedTaskToolPermissionDecision::Deny),
                    ("Always", DelegatedTaskToolPermissionDecision::Always),
                ] {
                    if soft_button(ui, label).clicked() {
                        actions.push(DesktopAction::RecordDelegateToolPermission {
                            request_id: request.request_id.clone(),
                            decision,
                        });
                    }
                }
            });
        });
    }
}

fn render_legion_workflow_tool_permission_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let requests = &snapshot.legion_workflow_projection.tool_permission_requests;
    if requests.is_empty() {
        ui.label(theme::muted("No Automate MCP tool permissions"));
        return;
    }
    for request in requests.iter().take(6) {
        let Some((server_id, tool_name)) = parse_automate_tool_target(request.target_id.as_deref())
        else {
            continue;
        };
        let Some(session_id) = parse_automate_permission_session(request) else {
            continue;
        };
        theme::small_card_frame().show(ui, |ui| {
            ui.label(theme::body_strong(trim_middle(&request.request_id, 56)));
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted(format!("{:?}", request.profile)));
                ui.separator();
                ui.label(theme::muted(format!("{:?}", request.action_class)));
                ui.separator();
                ui.label(theme::accent(
                    format!("{:?}", request.disposition),
                    if request.deny_overrides {
                        theme::tokens().accent.red
                    } else if request.runtime_allowed {
                        theme::tokens().accent.green
                    } else {
                        theme::tokens().accent.orange
                    },
                ));
            });
            ui.horizontal_wrapped(|ui| {
                for (label, decision) in [
                    ("Confirm", DelegatedTaskToolPermissionDecision::Confirm),
                    ("Allow", DelegatedTaskToolPermissionDecision::Allow),
                    ("Deny", DelegatedTaskToolPermissionDecision::Deny),
                    ("Always", DelegatedTaskToolPermissionDecision::Always),
                ] {
                    if soft_button(ui, label).clicked() {
                        actions.push(DesktopAction::RecordLegionWorkflowToolPermission {
                            session_id: session_id.clone(),
                            server_id: server_id.clone(),
                            tool_name: tool_name.clone(),
                            decision,
                        });
                    }
                }
            });
        });
    }
}

fn parse_automate_permission_session(
    request: &legion_protocol::DelegatedTaskToolPermissionRequest,
) -> Option<legion_protocol::LegionWorkflowSessionId> {
    request.labels.iter().find_map(|label| {
        label
            .strip_prefix("legion.session:")
            .filter(|session_id| !session_id.trim().is_empty())
            .map(|session_id| legion_protocol::LegionWorkflowSessionId(session_id.to_string()))
    })
}

fn render_legion_workflow_kill_switch_controls(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    for row in snapshot.legion_workflow_projection.rows.iter().take(3) {
        let triggered = snapshot
            .legion_workflow_projection
            .kill_switches
            .iter()
            .any(|switch| {
                switch.session_id == row.session_id
                    && switch.state == legion_protocol::LegionWorkflowKillSwitchState::Triggered
            });
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::muted(format!("kill switch {}", row.session_id.0)));
            if triggered {
                ui.label(theme::accent("Triggered", theme::tokens().accent.red));
            } else if soft_button(ui, "Kill").clicked() {
                actions.push(DesktopAction::TriggerLegionWorkflowKillSwitch {
                    session_id: row.session_id.clone(),
                    reason_label: "user requested hard stop".to_string(),
                });
            }
        });
    }
}

fn parse_automate_tool_target(
    target_id: Option<&str>,
) -> Option<(legion_protocol::McpServerId, legion_protocol::McpToolName)> {
    let target_id = target_id?;
    let rest = target_id.strip_prefix("mcp-tool:")?;
    let (server_id, tool_name) = rest.split_once('|')?;
    if server_id.trim().is_empty() || tool_name.trim().is_empty() {
        return None;
    }
    Some((
        legion_protocol::McpServerId(server_id.to_string()),
        legion_protocol::McpToolName(tool_name.to_string()),
    ))
}

fn render_terminal_stream(ui: &mut egui::Ui, model: &DesktopProjectionViewModel) {
    section_label(ui, "Terminal / Runtime", Some(theme::tokens().accent.cyan));
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
    section_label(ui, "Agent Comm Stream", Some(theme::tokens().accent.violet));
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

fn pill(ui: &mut egui::Ui, label: &str, color: egui::Color32, active: bool) -> egui::Response {
    let fill = if active {
        theme::dim(color, 28)
    } else {
        theme::dim(theme::tokens().text.primary, 10)
    };
    egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(
            1.0,
            if active {
                theme::dim(color, 90)
            } else {
                theme::tokens().border.default
            },
        ))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(theme::accent(label, color));
        })
        .response
}

fn level_pill(
    ui: &mut egui::Ui,
    level: &str,
    label: &str,
    color: egui::Color32,
    selected: bool,
) -> egui::Response {
    let text = format!("{label} {level}");
    pill(
        ui,
        &text,
        if selected {
            color
        } else {
            theme::tokens().text.muted
        },
        selected,
    )
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
            .fill(theme::tokens().bg.card)
            .stroke(egui::Stroke::new(1.0, theme::tokens().border.default))
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

fn projected_cursor(snapshot: &ShellProjectionSnapshot) -> TextCoordinate {
    snapshot
        .active_buffer_projection
        .viewport
        .as_ref()
        .map(|viewport| viewport.cursor)
        .unwrap_or(TextCoordinate {
            line: 0,
            character: 0,
            byte_offset: Some(0),
            utf16_offset: None,
        })
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
        .chain(
            snapshot
                .delegated_task_projection
                .proposal_reviews
                .iter()
                .flat_map(|review| {
                    review.hunks.iter().map(move |hunk| {
                        format!(
                            "delegate hunk {} proposal={} {:?}",
                            trim_middle(&hunk.hunk_id, 32),
                            review.proposal_id.0,
                            hunk.disposition
                        )
                    })
                }),
        )
        .chain(
            snapshot
                .delegated_task_projection
                .tool_permission_requests
                .iter()
                .map(|request| {
                    format!(
                        "delegate permission {} {:?} {:?}",
                        trim_middle(&request.request_id, 32),
                        request.profile,
                        request.disposition
                    )
                }),
        )
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
        DesktopProductMode::Assist => 28,
        DesktopProductMode::Delegates => 42,
        DesktopProductMode::LegionWorkflows => 82,
    };
    (base + snapshot.terminal_panel_projection.output_rows.len()).min(99)
}

fn level_primary_action(level: DesktopProductMode) -> &'static str {
    match level {
        DesktopProductMode::Manual => "Save All",
        DesktopProductMode::Assist => "Assist",
        DesktopProductMode::Delegates => "Delegate",
        DesktopProductMode::LegionWorkflows => "Run Workflow",
    }
}

fn level_color(level: DesktopProductMode) -> egui::Color32 {
    match level {
        DesktopProductMode::Manual => theme::tokens().text.muted,
        DesktopProductMode::Assist => theme::tokens().accent.cyan,
        DesktopProductMode::Delegates => theme::tokens().accent.violet,
        DesktopProductMode::LegionWorkflows => theme::tokens().accent.purple,
    }
}

fn risk_color(risk: ProposalRiskLabel) -> egui::Color32 {
    match risk {
        ProposalRiskLabel::Informational => theme::tokens().accent.cyan,
        ProposalRiskLabel::Low => theme::tokens().accent.green,
        ProposalRiskLabel::Medium => theme::tokens().accent.amber,
        ProposalRiskLabel::High => theme::tokens().accent.red,
        ProposalRiskLabel::Unknown => theme::tokens().text.muted,
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
    Assist,
    Delegates,
    LegionWorkflows,
}

impl DesktopProductMode {
    fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Assist => "Assist",
            Self::Delegates => "Delegates",
            Self::LegionWorkflows => "Legion Workflows",
        }
    }

    fn from_dock_mode(mode: DockMode) -> Self {
        match mode {
            DockMode::Manual => Self::Manual,
            DockMode::Assist => Self::Assist,
            DockMode::Delegate => Self::Delegates,
            DockMode::Automate => Self::LegionWorkflows,
        }
    }

    fn to_dock_mode(self) -> DockMode {
        match self {
            Self::Manual => DockMode::Manual,
            Self::Assist => DockMode::Assist,
            Self::Delegates => DockMode::Delegate,
            Self::LegionWorkflows => DockMode::Automate,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModeChromeSpec {
    mode: DesktopProductMode,
    ordinal: u8,
    label: &'static str,
    icon: &'static str,
    key: &'static str,
    micro: &'static str,
    confirmation: Option<ModeConfirmationSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModeConfirmationSpec {
    title: &'static str,
    body: &'static str,
    allow_dependency_install: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PaletteItem {
    group: &'static str,
    label: &'static str,
    action: Option<&'static str>,
    hint: Option<&'static str>,
    capabilities: &'static [ProductRuntimeSurface],
    requires_confirmation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BottomTabSpec {
    id: &'static str,
    label: &'static str,
    active: bool,
    color: egui::Color32,
    count: Option<usize>,
}

impl BottomTabSpec {
    fn new(
        id: &'static str,
        label: &'static str,
        active: bool,
        color: egui::Color32,
        count: Option<usize>,
    ) -> Self {
        Self {
            id,
            label,
            active,
            color,
            count,
        }
    }
}

fn autonomy_mode_specs() -> [ModeChromeSpec; 4] {
    [
        ModeChromeSpec {
            mode: DesktopProductMode::Manual,
            ordinal: 1,
            label: "Manual",
            icon: "keyboard",
            key: "M",
            micro: "You write. AI stays quiet.",
            confirmation: None,
        },
        ModeChromeSpec {
            mode: DesktopProductMode::Assist,
            ordinal: 2,
            label: "Assist",
            icon: "sparkles",
            key: "A",
            micro: "AI completes inline as you type.",
            confirmation: None,
        },
        ModeChromeSpec {
            mode: DesktopProductMode::Delegates,
            ordinal: 3,
            label: "Delegate",
            icon: "layers",
            key: "D",
            micro: "AI proposes multi-file diffs; you review and approve.",
            confirmation: Some(ModeConfirmationSpec {
                title: "Enter Delegate Mode?",
                body: "Agents work on scoped tasks and prepare diffs for your review. Nothing is applied without your approval.",
                allow_dependency_install: false,
            }),
        },
        ModeChromeSpec {
            mode: DesktopProductMode::LegionWorkflows,
            ordinal: 4,
            label: "Automate",
            icon: "network",
            key: "W",
            micro: "A full agent fleet plans, executes, tests, and reports.",
            confirmation: Some(ModeConfirmationSpec {
                title: "Activate Legion Workflows?",
                body: "The workflow will break down directives, modify files, run tests, and prepare changes for review under the permissions below.",
                allow_dependency_install: true,
            }),
        },
    ]
}

fn projected_product_mode(snapshot: &ShellProjectionSnapshot) -> DesktopProductMode {
    DesktopProductMode::from_dock_mode(snapshot.product_mode)
}

fn projected_dock_mode(snapshot: &ShellProjectionSnapshot) -> DockMode {
    match projected_product_mode(snapshot) {
        DesktopProductMode::Manual => DockMode::Manual,
        DesktopProductMode::Assist => DockMode::Assist,
        DesktopProductMode::Delegates => DockMode::Delegate,
        DesktopProductMode::LegionWorkflows => DockMode::Automate,
    }
}

fn active_dock_layout<'a>(
    state: &'a DesktopProjectionViewState,
    mode: DockMode,
) -> DockLayoutRef<'a> {
    if let Some(layout) = state.dock_layouts.iter().find(|layout| layout.mode == mode) {
        DockLayoutRef::Borrowed(layout)
    } else {
        DockLayoutRef::Owned(DockLayout::standard(mode))
    }
}

enum DockLayoutRef<'a> {
    Borrowed(&'a DockLayout),
    Owned(DockLayout),
}

impl DockLayoutRef<'_> {
    fn as_layout(&self) -> &DockLayout {
        match self {
            Self::Borrowed(layout) => layout,
            Self::Owned(layout) => layout,
        }
    }
}

fn dock_rows(
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
) -> Vec<String> {
    let mode = projected_dock_mode(snapshot);
    let registry = PanelRegistry::standard();
    let layout_ref = active_dock_layout(state, mode);
    let layout = layout_ref.as_layout();
    let visible_count = registry.visible_for(mode).len();

    vec![
        format!(
            "dock registry: mode={} visible_panels={} registered_panels={}",
            mode.label(),
            visible_count,
            registry.panels().len()
        ),
        dock_side_row(
            DockSide::Left,
            layout.side(DockSide::Left),
            layout,
            &registry,
        ),
        dock_side_row(
            DockSide::Right,
            layout.side(DockSide::Right),
            layout,
            &registry,
        ),
        dock_side_row(
            DockSide::Bottom,
            layout.side(DockSide::Bottom),
            layout,
            &registry,
        ),
    ]
}

fn dock_side_row(
    side: DockSide,
    side_layout: &DockSideLayout,
    layout: &DockLayout,
    registry: &PanelRegistry,
) -> String {
    let visible = layout
        .visible_panel_ids(side, registry)
        .into_iter()
        .map(PanelId::as_str)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "dock side: {} pinned={} toolkit={} splitter={:.2} collapsed={} visible=[{}]",
        side.label(),
        side_layout.pinned_default.as_str(),
        side_layout.custom_toolkit.len(),
        side_layout.splitter_fraction,
        side_layout.collapsed,
        visible
    )
}

fn dock_panel_rows(
    snapshot: &ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
) -> Vec<String> {
    let mode = projected_dock_mode(snapshot);
    let registry = PanelRegistry::standard();
    let layout_ref = active_dock_layout(state, mode);
    let layout = layout_ref.as_layout();
    let mut rows = Vec::new();
    for side in [DockSide::Left, DockSide::Right, DockSide::Bottom] {
        for id in layout.visible_panel_ids(side, &registry) {
            if let Some(panel) = registry.panel(id) {
                let capabilities = panel
                    .capabilities
                    .iter()
                    .map(|capability| format!("{capability:?}"))
                    .collect::<Vec<_>>()
                    .join(",");
                rows.push(format!(
                    "dock panel: side={} id={} title={} requires_ai={} capabilities=[{}]",
                    side.label(),
                    panel.id.as_str(),
                    panel.title,
                    panel.requires_ai,
                    capabilities
                ));
            }
        }
    }
    rows
}

fn product_mode_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let level = projected_product_mode(snapshot);
    let mut rows = vec![
        format!(
            "product mode: active={} app-owned projection",
            level.label()
        ),
        "product modes: Manual | Assist | Delegates | Legion Workflows".to_string(),
    ];

    match level {
        DesktopProductMode::Manual => {
            rows.push("product-mode safety: Manual Mode has no AI dispatch path".to_string());
        }
        DesktopProductMode::Assist => {
            rows.push(
                "product-mode safety: assisted work is proposal-preview only; direct workspace apply unsupported"
                    .to_string(),
            );
        }
        DesktopProductMode::Delegates => {
            rows.push(
                "product-mode safety: delegated work is approval-gated; direct workspace apply unsupported"
                    .to_string(),
            );
        }
        DesktopProductMode::LegionWorkflows => {
            rows.push(format!(
                "product-mode safety: Legion Workflow sessions={}; apply remains proposal-mediated; Autonomous merge unsupported until approval",
                snapshot.legion_workflow_projection.total_session_count
            ));
        }
    }
    rows.push(
        "product-mode control: display-only; no provider, terminal, or apply authority".to_string(),
    );
    rows
}

fn autonomy_scale_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = projected_product_mode(snapshot);
    autonomy_mode_specs()
        .iter()
        .map(|spec| {
            let confirm = if spec.confirmation.is_some() {
                "required"
            } else {
                "none"
            };
            format!(
                "autonomy scale: n={} key={} label={} active={} icon={} confirm={} micro={}",
                spec.ordinal,
                spec.key,
                spec.label,
                spec.mode == active,
                spec.icon,
                confirm,
                spec.micro
            )
        })
        .collect()
}

fn mode_confirmation_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let active = projected_product_mode(snapshot);
    autonomy_mode_specs()
        .iter()
        .map(|spec| match spec.confirmation {
            Some(confirmation) => format!(
                "mode confirmation: target={} active={} required=true title=\"{}\" scope_default=module scope_options=[selected,module,repo] require_approval=true allow_tests=true allow_terminal=false allow_dependency_install={} protected=[.env,secrets/,*.pem] body=\"{}\"",
                spec.label,
                spec.mode == active,
                confirmation.title,
                confirmation.allow_dependency_install,
                confirmation.body
            ),
            None => format!(
                "mode confirmation: target={} active={} required=false",
                spec.label,
                spec.mode == active
            ),
        })
        .collect()
}

fn command_palette_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mode = projected_product_mode(snapshot);
    let product_mode = mode.to_dock_mode().to_product_mode();
    let files = projected_palette_files(snapshot);
    let mut rows = vec![format!(
        "command palette group: Files prefix=<none> items={}",
        files.len()
    )];
    rows.extend(files.into_iter().take(6).map(|label| {
        format!("command palette item: group=Files label={label} requires_ai=false visible=true")
    }));

    let command_items = [
        PaletteItem {
            group: "Commands",
            label: "Switch to Automate",
            action: Some("mode:automate"),
            hint: Some("Legion Workflows"),
            capabilities: &[ProductRuntimeSurface::Automation],
            requires_confirmation: true,
        },
        PaletteItem {
            group: "Commands",
            label: "Switch to Delegate",
            action: Some("mode:delegate"),
            hint: None,
            capabilities: &[ProductRuntimeSurface::DelegatedTask],
            requires_confirmation: true,
        },
        PaletteItem {
            group: "Commands",
            label: "Run Directive",
            action: None,
            hint: Some("proposal-mediated"),
            capabilities: &[
                ProductRuntimeSurface::Automation,
                ProductRuntimeSurface::WorkerRuntime,
            ],
            requires_confirmation: true,
        },
        PaletteItem {
            group: "Commands",
            label: "Run Tests",
            action: None,
            hint: Some("deterministic"),
            capabilities: &[ProductRuntimeSurface::ManualIde],
            requires_confirmation: false,
        },
        PaletteItem {
            group: "Commands",
            label: "Open Permissions & Settings",
            action: None,
            hint: Some("local"),
            capabilities: &[ProductRuntimeSurface::ManualIde],
            requires_confirmation: false,
        },
    ];
    rows.push(format!(
        "command palette group: Commands prefix=> items={}",
        command_items.len()
    ));
    rows.extend(command_items.iter().map(|item| {
        let visible = item
            .capabilities
            .iter()
            .all(|surface| product_mode_allows_runtime_surface(product_mode, *surface));
        let capabilities = item
            .capabilities
            .iter()
            .map(|surface| format!("{surface:?}"))
            .collect::<Vec<_>>()
            .join(",");
        let requires_ai = item
            .capabilities
            .iter()
            .any(|surface| {
                !matches!(
                    surface,
                    ProductRuntimeSurface::ManualIde | ProductRuntimeSurface::PluginManagement
                )
            });
        format!(
            "command palette item: group={} label={} action={} hint={} requires_ai={} capabilities=[{}] requires_confirmation={} visible={}",
            item.group,
            item.label,
            item.action.unwrap_or("<none>"),
            item.hint.unwrap_or("<none>"),
            requires_ai,
            capabilities,
            item.requires_confirmation,
            visible
        )
    }));

    let agent_rows = projected_palette_agents(snapshot);
    let agents_visible =
        product_mode_allows_runtime_surface(product_mode, ProductRuntimeSurface::AssistedAi);
    rows.push(format!(
        "command palette group: Agents prefix=@ items={} capabilities=[AssistedAi] visible={}",
        agent_rows.len(),
        agents_visible
    ));
    rows.extend(agent_rows.into_iter().take(6).map(|label| {
        format!(
            "command palette item: group=Agents label={label} requires_ai=true capabilities=[AssistedAi] requires_confirmation=false visible={agents_visible}"
        )
    }));
    rows
}

fn projected_palette_files(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut labels = BTreeSet::new();
    for tab in &snapshot.daily_editing_projection.tabs.tabs {
        if let Some(path) = &tab.file_path {
            labels.insert(path.0.clone());
        } else {
            labels.insert(tab.title.clone());
        }
    }
    for node in &snapshot.explorer_projection.nodes {
        labels.insert(node.canonical_path.0.clone());
    }
    labels.into_iter().collect()
}

fn projected_palette_agents(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let mut rows = Vec::new();
    if snapshot.assisted_ai_projection.provider_count > 0
        || snapshot.assisted_ai_projection.request_count > 0
    {
        rows.push(format!(
            "Assistant · requests={} previews={}",
            snapshot.assisted_ai_projection.request_count,
            snapshot.assisted_ai_projection.preview_ready_count
        ));
    }
    if snapshot.delegated_task_projection.plan_count > 0 {
        rows.push(format!(
            "Delegate Team · plans={} steps={}",
            snapshot.delegated_task_projection.plan_count,
            snapshot.delegated_task_projection.step_summaries.len()
        ));
    }
    if snapshot.legion_workflow_projection.total_session_count > 0 {
        rows.push(format!(
            "Legion Workflow · sessions={}",
            snapshot.legion_workflow_projection.total_session_count
        ));
    }
    if rows.is_empty() {
        rows.push("No projected agents".to_string());
    }
    rows
}

fn bottom_tab_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    bottom_tab_specs(snapshot)
        .into_iter()
        .map(|tab| {
            let count = tab
                .count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "none".to_string());
            format!(
                "bottom tab: mode={} id={} label={} active={} count={}",
                projected_product_mode(snapshot).label(),
                tab.id,
                tab.label,
                tab.active,
                count
            )
        })
        .collect()
}

fn bottom_tab_specs(snapshot: &ShellProjectionSnapshot) -> Vec<BottomTabSpec> {
    let tests = optional_count(snapshot.verification_run_projection.rows.len());
    let suggestions = optional_count(
        snapshot.assisted_ai_projection.request_count as usize
            + snapshot.assisted_ai_projection.preview_ready_count as usize
            + snapshot.assisted_ai_projection.refusal_count as usize
            + snapshot
                .assist_inline_prediction_projection
                .display_row_count()
            + usize::from(
                snapshot
                    .assist_inline_prediction_projection
                    .request_in_flight,
            ),
    );
    match projected_product_mode(snapshot) {
        DesktopProductMode::Manual => vec![
            BottomTabSpec::new("term", "Terminal", true, theme::tokens().text.primary, None),
            BottomTabSpec::new("test", "Tests", false, theme::tokens().accent.green, tests),
        ],
        DesktopProductMode::Assist => vec![
            BottomTabSpec::new("term", "Terminal", true, theme::tokens().text.primary, None),
            BottomTabSpec::new(
                "sugg",
                "AI Suggestions",
                false,
                theme::tokens().accent.cyan,
                suggestions,
            ),
        ],
        DesktopProductMode::Delegates => vec![
            BottomTabSpec::new(
                "test",
                "Test Runner",
                true,
                theme::tokens().accent.green,
                tests,
            ),
            BottomTabSpec::new(
                "logs",
                "Agent Logs",
                false,
                theme::tokens().accent.cyan,
                None,
            ),
        ],
        DesktopProductMode::LegionWorkflows => vec![
            BottomTabSpec::new(
                "comm",
                "Comm Stream",
                true,
                theme::tokens().accent.purple,
                None,
            ),
            BottomTabSpec::new(
                "term",
                "Terminal",
                false,
                theme::tokens().text.primary,
                None,
            ),
        ],
    }
}

fn optional_count(count: usize) -> Option<usize> {
    if count == 0 { None } else { Some(count) }
}

fn delegated_activity_projected(snapshot: &ShellProjectionSnapshot) -> bool {
    snapshot.delegated_task_projection.plan_count > 0
        || !snapshot.delegated_task_projection.plan_rows.is_empty()
        || !snapshot.delegated_task_projection.step_summaries.is_empty()
        || !snapshot.delegated_task_projection.chat_messages.is_empty()
        || !snapshot
            .delegated_task_projection
            .context_citations
            .is_empty()
        || !snapshot
            .delegated_task_projection
            .proposal_reviews
            .is_empty()
        || !snapshot
            .delegated_task_projection
            .tool_permission_requests
            .is_empty()
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
    let mut rows = vec![
        format!(
            "code canvas: tabs={} active_path={} dirty={} degraded={}",
            snapshot.daily_editing_projection.tabs.tabs.len(),
            path,
            active.dirty,
            active.degraded
        ),
        format!(
            "language cues: status={:?} problems={} quick_fixes={} breadcrumbs={} sticky_scopes={} inlay_hints={} code_lenses={} completions={} definitions={} references={}",
            snapshot.language_tooling_projection.status,
            snapshot.language_tooling_projection.problems.len(),
            snapshot.language_tooling_projection.quick_fixes.len(),
            snapshot.language_tooling_projection.breadcrumbs.len(),
            snapshot.language_tooling_projection.sticky_scopes.len(),
            snapshot.language_tooling_projection.inlay_hints.len(),
            snapshot.language_tooling_projection.code_lenses.len(),
            snapshot.language_tooling_projection.completions.len(),
            snapshot.language_tooling_projection.definitions.len(),
            snapshot.language_tooling_projection.references.len()
        ),
        format!("search strip: {}", search.header),
        format!(
            "structural search strip: status={:?} matches={} proposal={:?}",
            snapshot.structural_search_projection.status.kind,
            snapshot.structural_search_projection.matches.len(),
            snapshot
                .structural_search_projection
                .proposal_id
                .map(|proposal| proposal.0)
        ),
    ];
    if let Some(prediction) = &snapshot
        .assist_inline_prediction_projection
        .active_prediction
    {
        rows.push(format!(
            "ghost prediction: id={} provider={} status={:?} latency={} stale={} range={} ghost={} replacement={}",
            prediction.prediction_id,
            prediction.provider_label,
            prediction.status,
            prediction_latency_label(prediction),
            prediction.stale,
            prediction.apply_range_label,
            prediction.ghost_text_label,
            prediction_replacement_label(prediction)
        ));
    }
    rows
}

fn directive_panel_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    vec![
        format!(
            "directive dock: proposals={} artifacts={} trust_items={} approval_gates={} proposal-mediated",
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
            "bottom console: terminal_status={:?} terminal_rows={} omitted={} structural_matches={}",
            snapshot.terminal_panel_projection.status.kind,
            snapshot.terminal_panel_projection.output_rows.len(),
            snapshot
                .terminal_panel_projection
                .scrollback
                .omitted_row_count,
            snapshot.structural_search_projection.matches.len()
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

fn status_line_ending(active: &ActiveBufferProjection) -> Option<String> {
    if let Some(viewport) = &active.viewport {
        let mut saw_lf = false;
        let mut saw_crlf = false;
        for metric in &viewport.line_metrics {
            match metric.line_ending_width {
                1 => saw_lf = true,
                2 => saw_crlf = true,
                _ => {}
            }
        }
        return match (saw_lf, saw_crlf) {
            (true, true) => Some("Mixed EOL".to_string()),
            (true, false) => Some("LF".to_string()),
            (false, true) => Some("CRLF".to_string()),
            (false, false) => None,
        };
    }

    active.small_buffer_text().and_then(|preview| {
        let has_crlf = preview.contains("\r\n");
        let bytes = preview.as_bytes();
        let has_lf = bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| *byte == b'\n' && (index == 0 || bytes[index - 1] != b'\r'));
        match (has_lf, has_crlf) {
            (true, true) => Some("Mixed EOL".to_string()),
            (true, false) => Some("LF".to_string()),
            (false, true) => Some("CRLF".to_string()),
            (false, false) => None,
        }
    })
}

fn status_language_for_path(path: &str) -> String {
    if has_ascii_extension(path, ".rs") {
        "rust"
    } else if has_ascii_extension(path, ".toml") {
        "toml"
    } else if has_ascii_extension(path, ".ts") || has_ascii_extension(path, ".tsx") {
        "typescript"
    } else if has_ascii_extension(path, ".js") || has_ascii_extension(path, ".jsx") {
        "javascript"
    } else if has_ascii_extension(path, ".md") {
        "markdown"
    } else if has_ascii_extension(path, ".json") {
        "json"
    } else {
        "text"
    }
    .to_string()
}

fn has_ascii_extension(path: &str, extension: &str) -> bool {
    let path = path.as_bytes();
    let extension = extension.as_bytes();
    path.len() > extension.len()
        && path[path.len() - extension.len()..].eq_ignore_ascii_case(extension)
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
    node: &legion_ui::ExplorerNodeProjection,
    nodes: &[legion_ui::ExplorerNodeProjection],
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
    nodes: &[legion_ui::ExplorerNodeProjection],
) -> Vec<&legion_ui::ExplorerNodeProjection> {
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
    node: &legion_ui::ExplorerNodeProjection,
    nodes: &[legion_ui::ExplorerNodeProjection],
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

fn coordinate_label(coordinate: &legion_protocol::TextCoordinate) -> String {
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
    rows.extend(legion_workflow_rows(snapshot));
    let inline = &snapshot.assist_inline_prediction_projection;
    if inline.has_activity() {
        rows.push(format!(
            "inline predictions: active={} rows={} in_flight={} stale={} generated_at={}",
            inline.active_prediction.is_some(),
            inline.rows.len(),
            inline.request_in_flight,
            inline.stale_prediction_count,
            inline.generated_at.0
        ));
    }
    if let Some(prediction) = &inline.active_prediction {
        rows.push(inline_prediction_row(prediction));
    }
    rows.extend(
        inline
            .rows
            .iter()
            .filter(|row| {
                inline
                    .active_prediction
                    .as_ref()
                    .is_none_or(|active| active.prediction_id != row.prediction_id)
            })
            .take(8)
            .map(inline_prediction_row),
    );
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
        && delegated.chat_messages.is_empty()
        && delegated.context_citations.is_empty()
        && delegated.proposal_reviews.is_empty()
        && delegated.tool_permission_requests.is_empty()
    {
        return rows;
    }
    rows.push(format!(
        "delegated task command center: projection={} plans={} blocked={} refused={} chat={} citations={} reviews={} permissions={} runtime={:?} autonomous_apply=unsupported redaction={}",
        delegated.projection_id,
        delegated.plan_count,
        delegated.blocked_plan_count,
        delegated.refused_plan_count,
        delegated.chat_message_count,
        delegated.context_citation_count,
        delegated.proposal_review_count,
        delegated.tool_permission_request_count,
        delegated.runtime_activation,
        redaction_label(&delegated.redaction_hints)
    ));
    rows.extend(delegated.chat_messages.iter().take(12).map(|message| {
        format!(
            "delegate chat {}: role={:?} citations={} permissions={} label={}",
            message.message_id,
            message.role,
            message.citation_ids.len(),
            message.tool_permission_request_ids.len(),
            trim_middle(&message.content_label, 96)
        )
    }));
    rows.extend(delegated.context_citations.iter().take(12).map(|citation| {
        format!(
            "delegate citation {}: path={} bytes={:?} lines={:?} score={} hash={}",
            citation.citation_id,
            citation
                .path
                .as_ref()
                .map(|path| path.0.as_str())
                .unwrap_or("<none>"),
            citation.byte_range,
            citation.line_range,
            citation.score_basis_points,
            citation
                .chunk_hash
                .as_ref()
                .map(|hash| hash.value.as_str())
                .unwrap_or("<none>")
        )
    }));
    rows.extend(delegated.proposal_reviews.iter().take(8).map(|review| {
        format!(
            "delegate proposal review {}: proposal={} hunks={} accepted={} rejected={} pending={} ready={} filtered={}",
            review.review_id,
            review.proposal_id.0,
            review.hunks.len(),
            review.accepted_hunk_count,
            review.rejected_hunk_count,
            review.pending_hunk_count,
            review.ready_for_apply,
            review.filtered_apply_required
        )
    }));
    rows.extend(
        delegated
            .proposal_reviews
            .iter()
            .take(8)
            .flat_map(|review| {
                review.hunks.iter().take(8).map(move |hunk| {
                    format!(
                        "delegate proposal hunk {}: proposal={} target={} disposition={:?} payload={:?} changed={} +{} -{} risk={:?} privacy={:?}",
                        trim_middle(&hunk.hunk_id, 48),
                        review.proposal_id.0,
                        hunk.target_id.as_deref().unwrap_or("<none>"),
                        hunk.disposition,
                        hunk.payload_kind,
                        hunk.changed_line_count,
                        hunk.inserted_line_count,
                        hunk.deleted_line_count,
                        hunk.risk_label,
                        hunk.privacy_label
                    )
                })
            }),
    );
    rows.extend(delegated.tool_permission_requests.iter().take(12).map(|request| {
        format!(
            "delegate tool permission {}: profile={:?} action={:?} decision={:?} disposition={:?} approval_required={} approval_recorded={} runtime_allowed={} deny_overrides={}",
            request.request_id,
            request.profile,
            request.action_class,
            request.decision,
            request.disposition,
            request.human_approval_required,
            request.human_approval_recorded,
            request.runtime_allowed,
            request.deny_overrides
        )
    }));
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

fn inline_prediction_row(prediction: &legion_ui::AssistInlinePredictionRowProjection) -> String {
    format!(
        "inline prediction {}: provider={} status={:?} status_label={} latency={} stale={} fingerprint={} snapshot={:?} buffer_version={:?} range={} ghost={} replacement={} diagnostics={}",
        prediction.prediction_id,
        prediction.provider_label,
        prediction.status,
        prediction.status_label,
        prediction_latency_label(prediction),
        prediction.stale,
        prediction_fingerprint_label(prediction),
        prediction.snapshot_id.map(|snapshot| snapshot.0),
        prediction.buffer_version.map(|version| version.0),
        prediction.apply_range_label,
        prediction.ghost_text_label,
        prediction_replacement_label(prediction),
        prediction.diagnostics.len()
    )
}

fn prediction_latency_label(prediction: &legion_ui::AssistInlinePredictionRowProjection) -> String {
    prediction
        .latency_ms
        .map(|latency| format!("{latency}ms"))
        .unwrap_or_else(|| "unknown".to_string())
}

fn prediction_fingerprint_label(
    prediction: &legion_ui::AssistInlinePredictionRowProjection,
) -> String {
    prediction
        .file_fingerprint
        .as_ref()
        .map(|fingerprint| format!("{}:{}", fingerprint.algorithm, fingerprint.value))
        .unwrap_or_else(|| "<none>".to_string())
}

fn prediction_replacement_label(
    prediction: &legion_ui::AssistInlinePredictionRowProjection,
) -> &str {
    prediction
        .replacement_preview_label
        .as_deref()
        .unwrap_or("<none>")
}

fn legion_workflow_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let workflows = &snapshot.legion_workflow_projection;
    if workflows.rows.is_empty()
        && workflows.mcp_registries.is_empty()
        && workflows.decision_feed.is_empty()
        && workflows.risk_monitors.is_empty()
        && workflows.kill_switches.is_empty()
        && workflows.tool_permission_requests.is_empty()
    {
        return Vec::new();
    }
    let mut rows = vec![format!(
        "legion workflow command center: projection={} sessions={} mcp={} decisions={} risk_monitors={} kill_switches={} permissions={} omitted={} Autonomous merge unsupported until approval redaction={}",
        workflows.projection_id,
        workflows.total_session_count,
        workflows.mcp_registry_count,
        workflows.decision_feed_count,
        workflows.risk_monitor_count,
        workflows.kill_switch_count,
        workflows.tool_permission_request_count,
        workflows.omitted_row_count,
        redaction_label(&workflows.redaction_hints)
    )];
    rows.extend(workflows.rows.iter().map(|row| {
        format!(
            "legion workflow session {}: state={:?} workers={} provider_routes={} dependencies={} conflicts={} verification={}/{} signoff={}/{} proposals={} merge={:?} blockers={} labels={}",
            row.session_id.0,
            row.lifecycle_state,
            row.worker_count,
            row.provider_route_required_count,
            row.dependency_count,
            row.unresolved_conflict_count,
            row.passed_verification_count,
            row.verification_gate_count,
            row.signed_off_count,
            row.sign_off_count,
            row.linked_proposals.len(),
            row.merge_readiness.state,
            row.merge_readiness.blockers.len(),
            bounded_join(&row.display_safe_labels)
        )
    }));
    rows.extend(workflows.rows.iter().flat_map(|row| {
        row.linked_proposals.iter().map(move |proposal_id| {
            format!(
                "legion workflow proposal link session={} proposal={} proposal-mediated",
                row.session_id.0, proposal_id.0
            )
        })
    }));
    rows.extend(workflows.rows.iter().flat_map(|row| {
        row.merge_readiness.labels.iter().map(move |label| {
            format!(
                "legion workflow merge readiness {}: state={:?} label={} approval-gated",
                row.session_id.0, row.merge_readiness.state, label
            )
        })
    }));
    rows.extend(workflows.mcp_registries.iter().map(|registry| {
        format!(
            "legion workflow mcp registry {}: server={} transport={:?} tools={} resources={} prompts={} version={} changed={:?}",
            registry.registry_id,
            registry.server.server_id.0,
            registry.server.transport_kind,
            registry.tools.len(),
            registry.resources.len(),
            registry.prompts.len(),
            registry.list_version,
            registry.last_notification_kind
        )
    }));
    rows.extend(workflows.decision_feed.iter().map(|entry| {
        format!(
            "legion workflow decision {}: session={} kind={:?} risk={:?} primitive={:?} permission={:?} summary={}",
            entry.decision_id.0,
            entry.session_id.0,
            entry.kind,
            entry.risk_label,
            entry.mcp_primitive_kind,
            entry.tool_permission_request_id,
            entry.summary_label
        )
    }));
    rows.extend(workflows.risk_monitors.iter().map(|monitor| {
        format!(
            "legion workflow risk monitor {}: session={} state={:?} score={}/{} high_risk={} denied={} stale_mcp={} halt={:?}",
            monitor.monitor_id.0,
            monitor.session_id.0,
            monitor.state,
            monitor.risk_score,
            monitor.halt_threshold,
            monitor.high_risk_action_count,
            monitor.denied_tool_count,
            monitor.stale_mcp_registry_detected,
            monitor.halt_reason
        )
    }));
    rows.extend(workflows.kill_switches.iter().map(|switch| {
        format!(
            "legion workflow kill switch {}: session={} state={:?} reason={}",
            switch.kill_switch_id.0,
            switch.session_id.0,
            switch.state,
            switch.reason_label.as_deref().unwrap_or("<armed>")
        )
    }));
    rows.extend(workflows.tool_permission_requests.iter().map(|request| {
        format!(
            "legion workflow tool permission {}: profile={:?} action={:?} decision={:?} disposition={:?} runtime={} deny={}",
            request.request_id,
            request.profile,
            request.action_class,
            request.decision,
            request.disposition,
            request.runtime_allowed,
            request.deny_overrides
        )
    }));
    rows
}

fn redaction_label(redaction_hints: &[legion_protocol::RedactionHint]) -> String {
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
        "manual toolchain: language={:?} problems={} quick_fixes={} breadcrumbs={} sticky_scopes={} inlay_hints={} code_lenses={} completions={} terminal={:?} search={} structural_search={:?}/{} verification_runs={}",
        language.status,
        language.problems.len(),
        language.quick_fixes.len(),
        language.breadcrumbs.len(),
        language.sticky_scopes.len(),
        language.inlay_hints.len(),
        language.code_lenses.len(),
        language.completions.len(),
        terminal.status.kind,
        search.header,
        snapshot.structural_search_projection.status.kind,
        snapshot.structural_search_projection.matches.len(),
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
        || !language.inlay_hints.is_empty()
        || !language.code_lenses.is_empty()
    {
        rows.push(format!(
            "language: {:?} problems={} quick_fixes={} breadcrumbs={} sticky_scopes={} inlay_hints={} code_lenses={} completions={} definitions={} references={} outline={} stale={} cancelled={}",
            language.status,
            language.problems.len(),
            language.quick_fixes.len(),
            language.breadcrumbs.len(),
            language.sticky_scopes.len(),
            language.inlay_hints.len(),
            language.code_lenses.len(),
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
    rows.extend(language.quick_fixes.iter().take(10).map(|quick_fix| {
        format!(
            "quick fix {} {} severity={:?} proposal={:?}",
            quick_fix.action_id,
            quick_fix.title,
            quick_fix.severity,
            quick_fix.proposal_id.map(|proposal| proposal.0)
        )
    }));
    rows.extend(language.breadcrumbs.iter().take(8).map(|breadcrumb| {
        format!(
            "breadcrumb {} {} kind={} depth={} source={}",
            breadcrumb.breadcrumb_id,
            breadcrumb.label,
            breadcrumb.kind_label,
            breadcrumb.depth,
            breadcrumb.source_label
        )
    }));
    rows.extend(language.sticky_scopes.iter().take(8).map(|scope| {
        format!(
            "sticky scope {} {} active={} kind={} depth={} source={}",
            scope.scope_id,
            scope.label,
            scope.active,
            scope.kind_label,
            scope.depth,
            scope.source_label
        )
    }));
    rows.extend(language.inlay_hints.iter().take(8).map(|hint| {
        format!(
            "inlay hint {} {} kind={} source={}",
            hint.hint_id, hint.label, hint.kind_label, hint.source_label
        )
    }));
    rows.extend(language.code_lenses.iter().take(8).map(|lens| {
        format!(
            "code lens {} {} command={} kind={} data={:?} source={}",
            lens.lens_id,
            lens.title,
            lens.command_label,
            lens.kind_label,
            lens.data_label,
            lens.source_label
        )
    }));
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

fn structural_search_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let structural = &snapshot.structural_search_projection;
    let mut rows = Vec::new();

    if structural.query_id.is_some()
        || !structural.matches.is_empty()
        || !structural.diagnostics.is_empty()
        || structural.proposal_id.is_some()
    {
        rows.push(format!(
            "structural search: {:?} matches={} proposal={:?}",
            structural.status.kind,
            structural.matches.len(),
            structural.proposal_id.map(|proposal| proposal.0)
        ));
        rows.push(format!(
            "structural query: scope={:?} pattern={} rewrite={} limit={} omitted_matches={} omitted_files={} schema={}",
            structural.scope,
            structural.pattern_label,
            structural
                .rewrite_label
                .as_deref()
                .unwrap_or("<preview-only>"),
            structural.result_limit,
            structural.omitted_match_count,
            structural.omitted_file_count,
            structural.schema_version
        ));
    }

    for structural_match in structural.matches.iter().take(20) {
        rows.push(format!(
            "structural match {}:{} {} -> {}",
            structural_match.file_path.0,
            structural_match.range.start.line,
            structural_match.snippet,
            structural_match
                .replacement_preview
                .as_deref()
                .unwrap_or("<no rewrite>")
        ));
        for capture in structural_match.captures.iter().take(8) {
            rows.push(format!("capture {}={}", capture.name, capture.value));
        }
    }

    rows.extend(
        structural
            .diagnostics
            .iter()
            .take(8)
            .map(|diagnostic| format!("structural diagnostic {diagnostic}")),
    );
    rows
}

fn git_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let git = &snapshot.git_projection;
    let mut rows = Vec::new();
    if git.root_label.is_some()
        || !git.changed_files.is_empty()
        || !git.hunks.is_empty()
        || !git.blame_lines.is_empty()
        || !git.commits.is_empty()
        || !git.conflicts.is_empty()
        || !git.diagnostics.is_empty()
    {
        rows.push(format!(
            "git: branch={} head={} changes={} hunks={} conflicts={}",
            git.branch_label.as_deref().unwrap_or("<none>"),
            git.head_short.as_deref().unwrap_or("<none>"),
            git.changed_files.len(),
            git.hunks.len(),
            git.conflicts.len()
        ));
    }
    rows.extend(git.changed_files.iter().take(16).map(|file| {
        format!(
            "git file {} status={} diff={:?} +{} -{} hunks={}/{} conflict={}",
            file.path,
            file.status,
            file.diff_strategy,
            file.inserted_lines,
            file.deleted_lines,
            file.staged_hunk_count,
            file.unstaged_hunk_count,
            file.conflict
        )
    }));
    rows.extend(git.hunks.iter().take(20).map(|hunk| {
        format!(
            "git hunk {} {} stage={:?} +{} -{} {}",
            hunk.hunk_id, hunk.path, hunk.stage, hunk.added_lines, hunk.deleted_lines, hunk.header
        )
    }));
    rows.extend(git.blame_lines.iter().take(12).map(|line| {
        format!(
            "git blame {}:{} {} {} {}",
            line.path, line.line_number, line.commit_short, line.author, line.summary
        )
    }));
    rows.extend(git.commits.iter().take(12).map(|commit| {
        format!(
            "git commit {} parents={} refs={} {}",
            commit.short_hash,
            commit.parent_count,
            bounded_join(&commit.refs),
            commit.summary
        )
    }));
    rows.extend(git.conflicts.iter().take(8).map(|conflict| {
        format!(
            "git conflict {} markers={} actions={}",
            conflict.path,
            conflict.marker_count,
            bounded_join(&conflict.actions)
        )
    }));
    rows.extend(
        git.diagnostics
            .iter()
            .take(8)
            .map(|diagnostic| format!("git diagnostic {diagnostic}")),
    );
    rows
}

fn debug_rows(snapshot: &ShellProjectionSnapshot) -> Vec<String> {
    let debug = &snapshot.debug_projection;
    let mut rows = Vec::new();
    if debug.active_session_id.is_some()
        || !debug.configurations.is_empty()
        || !debug.breakpoints.is_empty()
        || !debug.stack_frames.is_empty()
        || !debug.variables.is_empty()
        || !debug.watches.is_empty()
        || !debug.console.is_empty()
        || !debug.inline_values.is_empty()
        || !debug.diagnostics.is_empty()
    {
        rows.push(format!(
            "debug: status={:?} session={:?} state={:?} configs={} breakpoints={} frames={} variables={} watches={} console={} inline={}",
            debug.status.kind,
            debug.active_session_id.as_ref().map(|session| session.0.as_str()),
            debug.session_state,
            debug.configurations.len(),
            debug.breakpoints.len(),
            debug.stack_frames.len(),
            debug.variables.len(),
            debug.watches.len(),
            debug.console.len(),
            debug.inline_values.len()
        ));
    }
    rows.extend(debug.configurations.iter().take(8).map(|configuration| {
        format!(
            "debug config {} adapter={} program={} package={} target={} deterministic={}",
            configuration.configuration_id.0,
            configuration.adapter_type,
            configuration.program_label,
            configuration.cargo_package.as_deref().unwrap_or("<none>"),
            configuration.cargo_target.as_deref().unwrap_or("<none>"),
            configuration.deterministic
        )
    }));
    rows.extend(debug.breakpoints.iter().take(12).map(|breakpoint| {
        format!(
            "debug breakpoint {} {}:{} enabled={} verified={} condition={} hit={} log={}",
            breakpoint.breakpoint_id.0,
            breakpoint.path.0,
            breakpoint.line,
            breakpoint.enabled,
            breakpoint.verified,
            breakpoint.condition.as_deref().unwrap_or("<none>"),
            breakpoint.hit_condition.as_deref().unwrap_or("<none>"),
            breakpoint.log_message.as_deref().unwrap_or("<none>")
        )
    }));
    rows.extend(debug.stack_frames.iter().take(8).map(|frame| {
        let path = frame
            .path
            .as_ref()
            .map(|path| path.0.as_str())
            .unwrap_or("<unknown>");
        let line = frame
            .line
            .map(|line| line.to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        format!(
            "debug frame {}:{} {} {}:{}",
            frame.session_id.0, frame.frame_id, frame.name, path, line
        )
    }));
    rows.extend(debug.variables.iter().take(12).map(|variable| {
        format!(
            "debug variable {} {}={} type={} children={}",
            variable.session_id.0,
            variable.name,
            variable.value_label,
            variable.type_label.as_deref().unwrap_or("<none>"),
            variable.has_children
        )
    }));
    rows.extend(debug.watches.iter().take(8).map(|watch| {
        format!(
            "debug watch {} {} {}={} type={}",
            watch.session_id.0,
            watch.watch_id.0,
            watch.expression_label,
            watch.value_label,
            watch.type_label.as_deref().unwrap_or("<none>")
        )
    }));
    rows.extend(debug.console.iter().take(12).map(|entry| {
        format!(
            "debug console {} {}: {}",
            entry.session_id.0, entry.category_label, entry.message_label
        )
    }));
    rows.extend(debug.inline_values.iter().take(8).map(|inline_value| {
        format!(
            "debug inline {} {}:{} {}={}",
            inline_value.session_id.0,
            inline_value.path.0,
            inline_value.line,
            inline_value.expression_label,
            inline_value.value_label
        )
    }));
    rows.extend(
        debug
            .diagnostics
            .iter()
            .take(8)
            .map(|diagnostic| format!("debug diagnostic {diagnostic}")),
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::{
        CapabilityId, DelegatedTaskToolPermissionDecision, DelegatedTaskToolPermissionProfile,
        DelegatedTaskToolPermissionRequestInput, PermissionBudgetActionClass,
        delegated_task_tool_permission_request,
    };

    #[test]
    fn automate_permission_session_is_parsed_from_request_labels() {
        let request =
            delegated_task_tool_permission_request(DelegatedTaskToolPermissionRequestInput {
                request_id: "automate:permission:beta".to_string(),
                profile: DelegatedTaskToolPermissionProfile::Write,
                action_class: PermissionBudgetActionClass::InvokeLocalTool,
                capability: Some(CapabilityId("mcp.tool.call".to_string())),
                target_id: Some("mcp-tool:mcp:test|write_file".to_string()),
                decision: DelegatedTaskToolPermissionDecision::Confirm,
                labels: vec![
                    "automate.permission.mcp_tool_call".to_string(),
                    "legion.session:session:legion:beta".to_string(),
                ],
                schema_version: 1,
            });

        let session_id = parse_automate_permission_session(&request)
            .expect("request should carry its owning workflow session");

        assert_eq!(session_id.0, "session:legion:beta");
    }
}
