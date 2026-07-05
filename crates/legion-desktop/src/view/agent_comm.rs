use egui::Color32;
use legion_agent::comm::{AgentCommTag, ParsedAgentCommLine, parse_agent_comm_line};

use super::theme;

/// Renderer-ready view model for a single agent communication row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopAgentCommRowViewModel {
    /// Timestamp extracted from the line prefix.
    pub timestamp: String,
    /// Stable communication tag.
    pub tag: AgentCommTag,
    /// Stable uppercase tag label used by the renderer.
    pub tag_label: String,
    /// Speaker or source label.
    pub actor: String,
    /// Message text after the first colon.
    pub message: String,
}

impl From<ParsedAgentCommLine> for DesktopAgentCommRowViewModel {
    fn from(value: ParsedAgentCommLine) -> Self {
        Self {
            timestamp: value.timestamp,
            tag: value.tag,
            tag_label: tag_label(value.tag).to_string(),
            actor: value.actor,
            message: value.message,
        }
    }
}

/// Projects raw row text into the tagged agent communication rows the renderer can show.
pub fn agent_comm_rows(rows: &[String]) -> Vec<DesktopAgentCommRowViewModel> {
    rows.iter()
        .filter_map(|row| parse_agent_comm_line(row).map(Into::into))
        .collect()
}

/// Renders only tagged agent communication rows.
pub fn render_agent_comm_rows(ui: &mut egui::Ui, rows: &[String], empty: &str) {
    let rows = agent_comm_rows(rows);
    if rows.is_empty() {
        ui.label(theme::muted(empty));
        return;
    }

    for row in rows {
        render_agent_comm_row(ui, &row);
    }
}

fn render_agent_comm_row(ui: &mut egui::Ui, row: &DesktopAgentCommRowViewModel) {
    let tag_color = tag_color(row.tag);
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::code_muted(format!("[{}]", row.timestamp)));
        ui.label(theme::accent(format!("[{}]", row.tag_label), tag_color));
        ui.label(theme::body(format!("{}: {}", row.actor, row.message)));
    });
}

fn tag_label(tag: AgentCommTag) -> &'static str {
    match tag {
        AgentCommTag::Plan => "PLAN",
        AgentCommTag::Write => "WRITE",
        AgentCommTag::Test => "TEST",
        AgentCommTag::Review => "REVIEW",
        AgentCommTag::Error => "ERROR",
        AgentCommTag::Approval => "APPROVAL",
        AgentCommTag::Complete => "COMPLETE",
    }
}

fn tag_color(tag: AgentCommTag) -> Color32 {
    match tag {
        AgentCommTag::Plan => theme::tokens().accent.blue,
        AgentCommTag::Write => theme::tokens().accent.violet,
        AgentCommTag::Test => theme::tokens().accent.green,
        AgentCommTag::Review => theme::tokens().accent.orange,
        AgentCommTag::Error => theme::tokens().accent.red,
        AgentCommTag::Approval => theme::tokens().accent.amber,
        AgentCommTag::Complete => theme::tokens().accent.cyan,
    }
}

#[cfg(test)]
mod tests {
    use super::{agent_comm_rows, tag_label};
    use legion_agent::comm::AgentCommTag;

    #[test]
    fn agent_comm_rows_drop_freeform_messages_without_tags() {
        let rows = agent_comm_rows(&[
            "[12:04:11] [PLAN] Planner → Backend Team: Assigned checkout session endpoint"
                .to_string(),
            "Planner → Backend Team: assigned checkout session endpoint".to_string(),
        ]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tag_label, "PLAN");
        assert_eq!(rows[0].actor, "Planner → Backend Team");
        assert_eq!(rows[0].message, "Assigned checkout session endpoint");
    }

    #[test]
    fn every_comm_tag_has_a_stable_renderer_label() {
        assert_eq!(AgentCommTag::ALL.len(), 7);
        assert_eq!(tag_label(AgentCommTag::Plan), "PLAN");
        assert_eq!(tag_label(AgentCommTag::Write), "WRITE");
        assert_eq!(tag_label(AgentCommTag::Test), "TEST");
        assert_eq!(tag_label(AgentCommTag::Review), "REVIEW");
        assert_eq!(tag_label(AgentCommTag::Error), "ERROR");
        assert_eq!(tag_label(AgentCommTag::Approval), "APPROVAL");
        assert_eq!(tag_label(AgentCommTag::Complete), "COMPLETE");
    }
}
