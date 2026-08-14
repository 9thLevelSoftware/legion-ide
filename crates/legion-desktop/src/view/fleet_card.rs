use legion_protocol::ProposalRiskLabel;
use legion_ui::LegionWorkflowFleetCardProjection;

use super::{risk_color, theme, trim_middle};

/// Renderer-ready Legion workflow fleet card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopFleetCardViewModel {
    /// Proposal identifier.
    pub proposal_id: String,
    /// Proposal title.
    pub title: String,
    /// Owner label.
    pub owner_label: String,
    /// Model label.
    pub model_label: String,
    /// Lifecycle status label.
    pub status_label: String,
    /// Projection-sourced progress label.
    pub progress_label: String,
    /// Projection-sourced files/context label.
    pub files_label: String,
    /// Proposal risk label.
    pub risk_label: ProposalRiskLabel,
    /// Aggregated test status label.
    pub test_status_label: String,
    /// Mini diff label.
    pub mini_diff_label: String,
    /// Last activity label.
    pub last_activity_label: String,
}

/// Convert structured card projections into desktop view models.
pub fn fleet_card_view_models(
    cards: &[LegionWorkflowFleetCardProjection],
) -> Vec<DesktopFleetCardViewModel> {
    cards
        .iter()
        .map(|card| DesktopFleetCardViewModel {
            proposal_id: card.proposal_id.0.to_string(),
            title: card.title.clone(),
            owner_label: card.owner_label.clone(),
            model_label: card.model_label.clone(),
            status_label: card.status_label.clone(),
            progress_label: card.progress_label.clone(),
            files_label: card.files_label.clone(),
            risk_label: card.risk_label,
            test_status_label: card.test_status_label.clone(),
            mini_diff_label: card.mini_diff_label.clone(),
            last_activity_label: card.last_activity_label.clone(),
        })
        .collect()
}

/// Render structured fleet cards without parsing freeform log rows.
pub fn render_fleet_cards(ui: &mut egui::Ui, cards: &[LegionWorkflowFleetCardProjection]) {
    let cards = fleet_card_view_models(cards);
    if cards.is_empty() {
        ui.label(theme::muted("No task details available"));
        return;
    }

    for card in cards.iter().take(4) {
        theme::small_card_frame().show(ui, |ui| {
            ui.label(theme::body_strong(trim_middle(&card.title, 48)));
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted(format!("Owner: {}", card.owner_label)));
                ui.separator();
                ui.label(theme::muted(format!("Model: {}", card.model_label)));
                ui.separator();
                ui.label(theme::accent(
                    format!("Risk: {}", proposal_risk_label(card.risk_label)),
                    risk_color(card.risk_label),
                ));
            });
            ui.label(theme::muted(format!(
                "Status: {}",
                sentence_case(&card.status_label)
            )));
            ui.label(theme::muted(card_detail_label(&card.progress_label)));
            ui.label(theme::muted(card_detail_label(&card.files_label)));
            ui.label(theme::muted(format!(
                "Verification: {}",
                sentence_case(&card.test_status_label)
            )));
            ui.label(theme::muted(card_detail_label(&card.mini_diff_label)));
            ui.label(theme::muted(card_detail_label(&card.last_activity_label)));
        });
    }
}

fn proposal_risk_label(risk: ProposalRiskLabel) -> &'static str {
    match risk {
        ProposalRiskLabel::Informational => "Informational",
        ProposalRiskLabel::Low => "Low",
        ProposalRiskLabel::Medium => "Medium",
        ProposalRiskLabel::High => "High",
        ProposalRiskLabel::Unknown => "Unknown",
    }
}

fn card_detail_label(raw: &str) -> String {
    raw.split(" · ")
        .filter_map(|segment| {
            if segment.starts_with("manifest:") {
                return None;
            }
            let translated = if let Some(value) = segment.strip_prefix("targets=") {
                fraction_label("Targets", value)
            } else if let Some(value) = segment.strip_prefix("hunks=") {
                format!("Hunks: {value}")
            } else if let Some(value) = segment.strip_prefix("files=") {
                format!("Context: {value}")
            } else if let Some(value) = segment.strip_prefix("passed=") {
                format!("Passed: {value}")
            } else if let Some(value) = segment.strip_prefix("failed=") {
                format!("Failed: {value}")
            } else if let Some(value) = segment.strip_prefix("blocked=") {
                format!("Blocked: {value}")
            } else if let Some(value) = segment.strip_prefix("running=") {
                format!("Running: {value}")
            } else if let Some(value) = segment.strip_prefix("planned=") {
                format!("Planned: {value}")
            } else if let Some(value) = segment.strip_prefix("cancelled=") {
                format!("Cancelled: {value}")
            } else if let Some(value) = segment.strip_prefix("updated_at=") {
                format!("Last updated: {value}")
            } else if segment.contains('=') {
                segment.replace('=', ": ")
            } else {
                sentence_case(segment)
            };
            Some(translated)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn fraction_label(title: &str, value: &str) -> String {
    value.split_once('/').map_or_else(
        || format!("{title}: {value}"),
        |(current, total)| format!("{title}: {current} of {total}"),
    )
}

fn sentence_case(raw: &str) -> String {
    let mut label = raw.replace(['_', '-'], " ").replace('=', ": ");
    if let Some(first) = label.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    label
}
