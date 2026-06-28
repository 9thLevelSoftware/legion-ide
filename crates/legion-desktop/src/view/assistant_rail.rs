use legion_ai::streaming::{MarkdownStreamSegment, split_markdown_stream};
use legion_protocol::ProposalId;

use crate::{bridge::DesktopAction, theme};

use super::{primary_button, trim_middle};

/// Structured assistant rail row used to render streamed markdown and fenced code blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantRailRowViewModel {
    /// Ordered markdown segments for a single assistant row.
    pub segments: Vec<AssistantRailSegmentViewModel>,
}

/// Structured markdown segment for assistant rail rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantRailSegmentViewModel {
    /// Plain markdown text.
    Text(String),
    /// Fenced code block with proposal affordance metadata.
    CodeBlock(AssistantRailCodeBlockViewModel),
}

/// Structured code block segment used to attach proposal actions to rendered blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantRailCodeBlockViewModel {
    /// Optional language label from the markdown fence.
    pub language: Option<String>,
    /// Raw code block body without markdown fences.
    pub code: String,
    /// Whether the markdown fence has closed.
    pub complete: bool,
    /// Whether the rendered block can expose an apply-as-proposal affordance.
    pub apply_as_proposal_available: bool,
}

/// Converts assistant rail rows into structured markdown segments.
#[must_use]
pub fn assistant_rail_rows(
    rows: &[String],
    proposal_id: Option<ProposalId>,
) -> Vec<AssistantRailRowViewModel> {
    rows.iter()
        .map(|row| {
            let segments = split_markdown_stream(row)
                .into_iter()
                .map(|segment| match segment {
                    MarkdownStreamSegment::Text(text) => AssistantRailSegmentViewModel::Text(text),
                    MarkdownStreamSegment::CodeBlock {
                        language,
                        code,
                        complete,
                    } => AssistantRailSegmentViewModel::CodeBlock(AssistantRailCodeBlockViewModel {
                        language,
                        code,
                        complete,
                        apply_as_proposal_available: proposal_id.is_some(),
                    }),
                })
                .collect::<Vec<_>>();
            AssistantRailRowViewModel { segments }
        })
        .collect()
}

/// Renders assistant rows as streamed markdown text and fenced code blocks.
pub fn render_streaming_assistant_rows(
    ui: &mut egui::Ui,
    rows: &[String],
    empty: &str,
    limit: usize,
    proposal_id: Option<ProposalId>,
    actions: &mut Vec<DesktopAction>,
) {
    let rows = assistant_rail_rows(rows, proposal_id);
    if rows.is_empty() {
        ui.label(theme::muted(empty));
        return;
    }

    for row in rows.iter().take(limit) {
        render_row(ui, row, proposal_id, actions);
    }

    if rows.len() > limit {
        ui.label(theme::muted(format!("{} more rows", rows.len() - limit)));
    }
}

fn render_row(
    ui: &mut egui::Ui,
    row: &AssistantRailRowViewModel,
    proposal_id: Option<ProposalId>,
    actions: &mut Vec<DesktopAction>,
) {
    for segment in &row.segments {
        match segment {
            AssistantRailSegmentViewModel::Text(text) => {
                for line in text.lines() {
                    if !line.is_empty() {
                        ui.label(theme::body(trim_middle(line, 110)));
                    }
                }
            }
            AssistantRailSegmentViewModel::CodeBlock(code_block) => {
                render_code_block(ui, code_block, proposal_id, actions);
            }
        }
    }
}

fn render_code_block(
    ui: &mut egui::Ui,
    code_block: &AssistantRailCodeBlockViewModel,
    proposal_id: Option<ProposalId>,
    actions: &mut Vec<DesktopAction>,
) {
    theme::code_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            let label = code_block
                .language
                .as_deref()
                .filter(|label| !label.is_empty())
                .unwrap_or("code block");
            ui.label(theme::accent(label, theme::tokens().accent.cyan));
            if !code_block.complete {
                ui.label(theme::muted("streaming"));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(proposal_id) = proposal_id {
                    let button = primary_button(
                        ui,
                        "Apply as proposal",
                        theme::tokens().accent.blue,
                    );
                    if button.clicked() && code_block.apply_as_proposal_available {
                        actions.push(DesktopAction::ApplyProposal { proposal_id });
                    }
                } else {
                    ui.label(theme::muted("proposal unavailable"));
                }
            });
        });
        ui.add_space(4.0);
        for line in code_block.code.lines() {
            ui.label(theme::code(line));
        }
        if code_block.code.is_empty() {
            ui.label(theme::muted("<empty code block>"));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{AssistantRailSegmentViewModel, assistant_rail_rows};
    use legion_protocol::ProposalId;

    #[test]
    fn assistant_rail_rows_attach_apply_affordance_to_code_blocks() {
        let rows = assistant_rail_rows(
            &["before\n```rust\nfn demo() {}\n```\nafter".to_string()],
            Some(ProposalId(7)),
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].segments.len(), 3);
        match &rows[0].segments[1] {
            AssistantRailSegmentViewModel::CodeBlock(code_block) => {
                assert_eq!(code_block.language.as_deref(), Some("rust"));
                assert!(code_block.code.contains("fn demo() {}"));
                assert!(code_block.complete);
                assert!(code_block.apply_as_proposal_available);
            }
            other => panic!("expected code block, got {other:?}"),
        }
    }
}
