//! Help/About overlay: version, proprietary license, and privacy posture.

use egui::Id;

use super::components::{section_header as section_label, soft_button};
use super::{DesktopProjectionViewModel, ProjectionView};
use crate::bridge::DesktopAction;
use crate::theme;

pub(super) fn render_about_panel(
    ui: &mut egui::Ui,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    model: &DesktopProjectionViewModel,
    view: &mut ProjectionView,
    actions: &mut Vec<DesktopAction>,
) -> Id {
    section_label(ui, "Legion IDE", Some(theme::tokens().accent.blue));
    ui.label(theme::body_strong(format!(
        "Version {}",
        env!("CARGO_PKG_VERSION")
    )));
    ui.label(theme::muted(format!(
        "Mode: {}",
        snapshot.product_mode.label()
    )));
    ui.add_space(8.0);
    ui.label(theme::body(
        "Proprietary software. All rights reserved. Not OSI-licensed and not a general-availability product.",
    ));
    ui.add_space(8.0);
    section_label(ui, "Privacy", Some(theme::tokens().accent.green));
    ui.label(theme::body(
        "Manual mode is the default and makes no network calls. Assist, Delegate, and Legion Workflows are opt-in. There is no phone-home.",
    ));
    ui.label(theme::muted(format!(
        "Crash reports: {} · Data sharing: {}",
        if model.settings.crash_reports_enabled {
            "enabled (local only; no upload)"
        } else {
            "off"
        },
        model.settings.telemetry_label
    )));
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if soft_button(ui, "Export support bundle").clicked() {
            actions.push(DesktopAction::ExportSupportBundle);
        }
        if soft_button(ui, "Privacy settings").clicked() {
            actions.push(DesktopAction::OpenSettings);
            view.utility_surface = Some(super::UtilitySurface::Settings);
            view.settings_section = super::SettingsSection::Privacy;
            view.utility_overlay_needs_focus = true;
            view.utility_overlay_focus_bounds = None;
        }
    });
    ui.add_space(8.0);
    ui.label(theme::muted(
        "Export writes .legion/support-bundle.md (metadata only: no editor text, secrets, or raw crash bodies).",
    ));
    ui.id()
}
