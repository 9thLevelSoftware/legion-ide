use egui::Color32;
use legion_ui::{
    LegionWorkflowBoardColumnKind, LegionWorkflowBoardColumnProjection,
    LegionWorkflowBoardRowProjection,
};

use super::{avatar, pill, status_dot, theme, trim_middle};

/// Renderer-ready workflow board row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopFleetBoardRowViewModel {
    /// Workflow session label.
    pub session_id: String,
    /// Coordinator state label.
    pub state_label: String,
    /// Display-safe row summary.
    pub summary_label: String,
}

/// Renderer-ready workflow board column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopFleetBoardColumnViewModel {
    /// Stable column kind.
    pub kind: LegionWorkflowBoardColumnKind,
    /// Display title.
    pub title: String,
    /// Rows projected into this column.
    pub rows: Vec<DesktopFleetBoardRowViewModel>,
}

/// Convert structured board projections into desktop view models.
pub fn fleet_board_column_view_models(
    columns: &[LegionWorkflowBoardColumnProjection],
) -> Vec<DesktopFleetBoardColumnViewModel> {
    columns
        .iter()
        .map(|column| DesktopFleetBoardColumnViewModel {
            kind: column.kind,
            title: column.title.clone(),
            rows: column.rows.iter().map(fleet_board_row_view_model).collect(),
        })
        .collect()
}

fn fleet_board_row_view_model(
    row: &LegionWorkflowBoardRowProjection,
) -> DesktopFleetBoardRowViewModel {
    DesktopFleetBoardRowViewModel {
        session_id: row.session_id.0.clone(),
        state_label: row.state_label.clone(),
        summary_label: row.summary_label.clone(),
    }
}

/// Render the projection-backed Legion workflow board.
pub fn render_fleet_board(ui: &mut egui::Ui, columns: &[LegionWorkflowBoardColumnProjection]) {
    let board_height = ui.available_height();
    let columns = fleet_board_column_view_models(columns);
    egui::ScrollArea::horizontal()
        .id_salt("legion_desktop_fleet_board")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_height(board_height);
            ui.horizontal_top(|ui| {
                for column in &columns {
                    render_fleet_board_column(ui, column);
                }
            });
        });
}

fn render_fleet_board_column(ui: &mut egui::Ui, column: &DesktopFleetBoardColumnViewModel) {
    let color = column_color(column.kind);
    theme::card_frame_tinted(theme::tokens().bg.canvas, theme::tokens().border.subtle).show(
        ui,
        |ui| {
            ui.set_width(260.0);
            ui.horizontal(|ui| {
                status_dot(ui, color);
                ui.label(theme::accent(
                    column.title.to_uppercase(),
                    theme::tokens().text.secondary,
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    pill(ui, &column.rows.len().to_string(), color, false);
                });
            });
            ui.separator();
            if column.rows.is_empty() {
                ui.label(theme::muted("No projected rows"));
            }
            for row in column.rows.iter().take(5) {
                theme::small_card_frame().show(ui, |ui| {
                    ui.label(theme::body_strong(trim_middle(&row.session_id, 38)));
                    ui.label(theme::muted(trim_middle(&row.summary_label, 64)));
                    ui.horizontal(|ui| {
                        avatar(ui, state_initial(&row.state_label), color);
                        ui.label(theme::muted(&row.state_label));
                    });
                });
            }
        },
    );
}

fn state_initial(state_label: &str) -> &str {
    state_label
        .chars()
        .next()
        .map(|ch| match ch {
            'A' | 'a' => "A",
            'B' | 'b' => "B",
            'C' | 'c' => "C",
            'D' | 'd' => "D",
            'E' | 'e' => "E",
            'F' | 'f' => "F",
            'P' | 'p' => "P",
            'R' | 'r' => "R",
            'V' | 'v' => "V",
            'W' | 'w' => "W",
            _ => "S",
        })
        .unwrap_or("S")
}

fn column_color(kind: LegionWorkflowBoardColumnKind) -> Color32 {
    match kind {
        LegionWorkflowBoardColumnKind::Assigned => theme::tokens().text.muted,
        LegionWorkflowBoardColumnKind::InProgress => theme::tokens().accent.blue,
        LegionWorkflowBoardColumnKind::WaitingOnHuman => theme::tokens().accent.orange,
        LegionWorkflowBoardColumnKind::Testing => theme::tokens().accent.violet,
        LegionWorkflowBoardColumnKind::Done => theme::tokens().accent.green,
    }
}
