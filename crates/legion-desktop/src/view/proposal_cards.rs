//! The proposal ledger rendered as Approve / Review / Reject cards.
//!
//! Moved out of `view.rs`, which is a chokepoint file with a line budget, under
//! the roadmap's extract-before-modify rule. A self-contained region -- pick the
//! rows to draw, draw a card per row, report what was not drawn -- with two
//! callers (the Assist rail and the Workflows rail) and no reason to live in the
//! same file as the rest of the shell.

use legion_protocol::{
    ProposalCancellationReason, ProposalLifecycleState, ProposalRejectionReason,
};
use legion_ui::ShellProjectionSnapshot;

use super::components::{primary_button, soft_button};
use super::{proposal_risk_label, risk_color, theme};
use crate::bridge::DesktopAction;

pub(crate) fn render_proposal_cards(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    actions: &mut Vec<DesktopAction>,
) {
    let ledger = &snapshot.proposal_ledger_projection;
    if ledger.rows.is_empty() {
        ui.label(theme::muted("No pending proposals"));
        return;
    }
    const PROPOSAL_CARD_LIMIT: usize = 4;
    for row in ledger.rows.iter().take(PROPOSAL_CARD_LIMIT) {
        // Only proposals still awaiting a decision should expose Approve/Reject;
        // terminal/applied/denied proposals render the controls disabled so a
        // dropped click cannot re-trigger a lifecycle action.
        let actionable = matches!(
            row.lifecycle.state,
            ProposalLifecycleState::Created
                | ProposalLifecycleState::Validated
                | ProposalLifecycleState::Previewed
        );
        let cancellable = matches!(
            row.lifecycle.state,
            ProposalLifecycleState::Created
                | ProposalLifecycleState::Validated
                | ProposalLifecycleState::Previewed
                | ProposalLifecycleState::Approved
        );
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
                    format!("Risk: {}", proposal_risk_label(row.risk_label)),
                    risk_color(row.risk_label),
                ));
            });
            // Surface the lifecycle state so terminal proposals are legible.
            ui.label(theme::muted(format!("status: {}", row.lifecycle.label)));
            ui.horizontal(|ui| {
                ui.add_enabled_ui(actionable, |ui| {
                    if primary_button(ui, "Approve", theme::tokens().accent.green).clicked()
                        && actionable
                    {
                        actions.push(DesktopAction::ApproveProposal {
                            proposal_id: row.proposal_id,
                        });
                    }
                });
                if soft_button(ui, "Review").clicked() {
                    actions.push(DesktopAction::OpenProposalDetails {
                        proposal_id: row.proposal_id,
                    });
                }
                ui.add_enabled_ui(actionable, |ui| {
                    if soft_button(ui, "Reject").clicked() && actionable {
                        actions.push(DesktopAction::RejectProposal {
                            proposal_id: row.proposal_id,
                            reason: ProposalRejectionReason::UserRejected,
                        });
                    }
                });
                ui.add_enabled_ui(cancellable, |ui| {
                    if soft_button(ui, "Cancel proposal").clicked() && cancellable {
                        actions.push(DesktopAction::CancelProposal {
                            proposal_id: row.proposal_id,
                            reason: ProposalCancellationReason::UserCancelled,
                        });
                    }
                });
            });
        });
    }
    let hidden =
        ledger.rows.len().saturating_sub(PROPOSAL_CARD_LIMIT) + ledger.omitted_row_count as usize;
    if hidden > 0 {
        ui.label(theme::muted(format!("{hidden} more proposals")));
    }
}
