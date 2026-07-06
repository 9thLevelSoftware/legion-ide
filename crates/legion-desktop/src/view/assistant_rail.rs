use legion_ai::streaming::{MarkdownStreamSegment, split_markdown_stream};
use legion_protocol::{
    AssistantRailCommand, ProposalId, RailCommandCapability, rail_command_capabilities,
};

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
    /// Proposal bound to *this* specific block, if any. `Some` only when the
    /// block is complete and a per-block proposal binding was established; the
    /// apply affordance dispatches against this id rather than a snapshot-level
    /// shared id.
    pub proposal_id: Option<ProposalId>,
    /// Whether the rendered block can expose an apply-as-proposal affordance.
    /// True iff the block is complete *and* a verified per-block proposal
    /// binding ([`Self::proposal_id`]) exists.
    pub apply_as_proposal_available: bool,
}

/// View model for a single assistant rail command button (PKT-RAIL T2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantRailCommandViewModel {
    /// The command this view model represents.
    pub command: AssistantRailCommand,
    /// Human-readable label for the command button.
    pub label: String,
    /// Whether this command is available based on capability gates.
    ///
    /// `false` when the command's capability ID is absent from the provided
    /// capabilities slice (i.e., the gate is closed).
    pub available: bool,
}

/// Builds view models for all rail commands, gating availability against the
/// provided capability slice.
///
/// A command is `available` iff its stable capability ID appears in `capabilities`.
/// Commands whose capability ID is absent get `available: false`.
#[must_use]
pub fn rail_command_view_models(
    capabilities: &[RailCommandCapability],
) -> Vec<AssistantRailCommandViewModel> {
    rail_command_capabilities()
        .into_iter()
        .map(|cap| {
            let available = capabilities
                .iter()
                .any(|c| c.capability_id == cap.capability_id);
            let label = match cap.command {
                AssistantRailCommand::Explain => "Explain",
                AssistantRailCommand::Fix => "Fix",
                AssistantRailCommand::Test => "Test",
                AssistantRailCommand::Doc => "Doc",
                AssistantRailCommand::Refactor => "Refactor",
            };
            AssistantRailCommandViewModel {
                command: cap.command,
                label: label.to_string(),
                available,
            }
        })
        .collect()
}

/// Converts assistant rail rows into structured markdown segments.
///
/// Every completed fenced code block gets its own apply-as-proposal affordance,
/// bound to a unique per-block proposal ID derived from the base proposal ID:
/// `ProposalId(base.0 + block_index)`. This allows independent application of
/// each block without sharing a single proposal between them.
///
/// Streaming (incomplete) blocks are never applyable — they have no proposal
/// binding until the closing fence arrives.
#[must_use]
pub fn assistant_rail_rows(
    rows: &[String],
    proposal_id: Option<ProposalId>,
) -> Vec<AssistantRailRowViewModel> {
    let mut block_index: u64 = 0;
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
                    } => {
                        // Every complete block gets its own unique proposal ID by
                        // offsetting the base by the block's ordinal position.
                        // Incomplete (streaming) blocks get no binding.
                        let bound_proposal_id = if complete {
                            if let Some(base) = proposal_id {
                                let id = ProposalId(base.0.saturating_add(block_index));
                                block_index += 1;
                                Some(id)
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        AssistantRailSegmentViewModel::CodeBlock(AssistantRailCodeBlockViewModel {
                            language,
                            code,
                            complete,
                            apply_as_proposal_available: bound_proposal_id.is_some(),
                            proposal_id: bound_proposal_id,
                        })
                    }
                })
                .collect::<Vec<_>>();
            AssistantRailRowViewModel { segments }
        })
        .collect()
}

/// Assigns unique per-block proposal IDs to a slice of code block view models.
///
/// Each complete block at ordinal `i` receives `ProposalId(base.0 + i)`.
/// Incomplete blocks receive `None` because they cannot be applied until the
/// closing fence is observed.
///
/// Returns `(block, Option<ProposalId>)` pairs in input order.
#[must_use]
pub fn bind_proposals_to_blocks(
    blocks: &[AssistantRailCodeBlockViewModel],
    base_proposal_id: ProposalId,
) -> Vec<(AssistantRailCodeBlockViewModel, Option<ProposalId>)> {
    let mut block_index: u64 = 0;
    blocks
        .iter()
        .map(|block| {
            if block.complete {
                let id = ProposalId(base_proposal_id.0.saturating_add(block_index));
                block_index += 1;
                (block.clone(), Some(id))
            } else {
                (block.clone(), None)
            }
        })
        .collect()
}

/// Accumulates streaming response chunks into renderable rail rows.
///
/// Chunks are joined in order and processed as a single document.
/// Per-block proposal bindings use the provided base proposal ID offset by
/// block ordinal (same as [`assistant_rail_rows`]).
///
/// Non-streaming providers pass the full response as a single chunk in a
/// single-element slice; per-block proposal bindings still apply.
#[must_use]
pub fn streaming_rail_rows(
    stream_chunks: &[String],
    proposal_id_base: Option<ProposalId>,
) -> Vec<AssistantRailRowViewModel> {
    let accumulated = stream_chunks.join("");
    assistant_rail_rows(&[accumulated], proposal_id_base)
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
        render_row(ui, row, actions);
    }

    if rows.len() > limit {
        ui.label(theme::muted(format!("{} more rows", rows.len() - limit)));
    }
}

fn render_row(
    ui: &mut egui::Ui,
    row: &AssistantRailRowViewModel,
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
                render_code_block(ui, code_block, actions);
            }
        }
    }
}

fn render_code_block(
    ui: &mut egui::Ui,
    code_block: &AssistantRailCodeBlockViewModel,
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
                // Render the apply button only when the block is complete and
                // carries its own verified proposal binding.
                match code_block.proposal_id {
                    Some(proposal_id)
                        if code_block.complete && code_block.apply_as_proposal_available =>
                    {
                        let button =
                            primary_button(ui, "Apply as proposal", theme::tokens().accent.blue);
                        if button.clicked() {
                            actions.push(DesktopAction::ApplyProposal { proposal_id });
                        }
                    }
                    _ => {
                        ui.label(theme::muted("proposal unavailable"));
                    }
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
                assert_eq!(code_block.proposal_id, Some(ProposalId(7)));
            }
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn assistant_rail_rows_bind_proposal_to_every_complete_block() {
        // Every complete code block receives a unique proposal binding so each
        // can be applied independently. Block 0 gets the base ID, block 1 gets
        // base+1, etc.
        let rows = assistant_rail_rows(
            &["```rust\nfn first() {}\n```\n```rust\nfn second() {}\n```".to_string()],
            Some(ProposalId(11)),
        );

        let code_blocks = rows[0]
            .segments
            .iter()
            .filter_map(|segment| match segment {
                AssistantRailSegmentViewModel::CodeBlock(code_block) => Some(code_block),
                AssistantRailSegmentViewModel::Text(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(code_blocks.len(), 2);

        assert_eq!(code_blocks[0].proposal_id, Some(ProposalId(11)));
        assert!(code_blocks[0].apply_as_proposal_available);

        // Second block gets proposal_id = base + 1 = ProposalId(12)
        assert_eq!(code_blocks[1].proposal_id, Some(ProposalId(12)));
        assert!(code_blocks[1].apply_as_proposal_available);
    }

    #[test]
    fn streaming_rail_rows_accumulate_chunks() {
        use super::streaming_rail_rows;
        // Partial chunks without a closing fence → incomplete, never applyable.
        let partial = vec!["```rust\n".to_string(), "fn partial() {}".to_string()];
        let rows = streaming_rail_rows(&partial, Some(ProposalId(5)));
        let block = rows[0]
            .segments
            .iter()
            .find_map(|s| match s {
                AssistantRailSegmentViewModel::CodeBlock(b) => Some(b),
                _ => None,
            })
            .expect("partial stream must produce an incomplete code block");
        assert!(!block.complete, "in-flight block must be incomplete");
        assert!(
            block.proposal_id.is_none(),
            "incomplete block must have no proposal"
        );

        // Full chunks with closing fence → complete block, proposal bound.
        let complete = vec![
            "```rust\n".to_string(),
            "fn done() {}\n".to_string(),
            "```".to_string(),
        ];
        let rows = streaming_rail_rows(&complete, Some(ProposalId(5)));
        let block = rows[0]
            .segments
            .iter()
            .find_map(|s| match s {
                AssistantRailSegmentViewModel::CodeBlock(b) => Some(b),
                _ => None,
            })
            .expect("complete stream must produce a complete code block");
        assert!(block.complete);
        assert_eq!(block.proposal_id, Some(ProposalId(5)));
    }

    #[test]
    fn non_streaming_response_gets_per_block_proposals() {
        use super::streaming_rail_rows;
        // Three code blocks in a batch response each get independent proposals.
        let response = vec![
            "```rust\nfn a() {}\n```\n```python\nprint('b')\n```\n```js\nconsole.log('c')\n```"
                .to_string(),
        ];
        let rows = streaming_rail_rows(&response, Some(ProposalId(100)));
        let blocks: Vec<_> = rows[0]
            .segments
            .iter()
            .filter_map(|s| match s {
                AssistantRailSegmentViewModel::CodeBlock(b) => Some(b),
                _ => None,
            })
            .collect();

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].proposal_id, Some(ProposalId(100)));
        assert_eq!(blocks[1].proposal_id, Some(ProposalId(101)));
        assert_eq!(blocks[2].proposal_id, Some(ProposalId(102)));
        for block in &blocks {
            assert!(block.apply_as_proposal_available);
        }
    }

    #[test]
    fn incomplete_streaming_block_never_applyable() {
        // An in-flight streaming block (no closing fence) must never have a
        // proposal binding, regardless of the base proposal id.
        let rows = assistant_rail_rows(
            &["```rust\nfn streaming() {}".to_string()],
            Some(ProposalId(3)),
        );
        match &rows[0].segments[0] {
            AssistantRailSegmentViewModel::CodeBlock(code_block) => {
                assert!(!code_block.complete);
                assert_eq!(code_block.proposal_id, None);
                assert!(!code_block.apply_as_proposal_available);
            }
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn assistant_rail_rows_do_not_bind_incomplete_block() {
        // A streaming (unterminated) fence must never be applyable even when a
        // proposal is present.
        let rows = assistant_rail_rows(
            &["```rust\nfn streaming() {}".to_string()],
            Some(ProposalId(3)),
        );
        match &rows[0].segments[0] {
            AssistantRailSegmentViewModel::CodeBlock(code_block) => {
                assert!(!code_block.complete);
                assert_eq!(code_block.proposal_id, None);
                assert!(!code_block.apply_as_proposal_available);
            }
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn assistant_rail_rows_without_proposal_are_not_applyable() {
        let rows = assistant_rail_rows(&["```rust\nfn demo() {}\n```".to_string()], None);
        match &rows[0].segments[0] {
            AssistantRailSegmentViewModel::CodeBlock(code_block) => {
                assert!(code_block.complete);
                assert_eq!(code_block.proposal_id, None);
                assert!(!code_block.apply_as_proposal_available);
            }
            other => panic!("expected code block, got {other:?}"),
        }
    }
}
