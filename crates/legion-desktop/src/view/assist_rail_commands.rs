//! Assist rail commands: the controls that dispatch a proposal-only AI run.
//!
//! New in its own module rather than added to `view.rs`, which is a chokepoint
//! file with a line budget. A self-contained region -- a command list, a gate on
//! having a buffer, and a row of buttons -- so it has no reason to live there.

use legion_protocol::AssistantRailCommand;
use legion_ui::ShellProjectionSnapshot;

use super::theme;
use crate::bridge::DesktopAction;

/// The assistant rail commands, in the order the panel offers them.
///
/// `Explain` first because it is the only one that is safe to run without
/// having decided anything: it produces a proposal describing the code rather
/// than changing it.
const ASSIST_RAIL_COMMANDS: [(AssistantRailCommand, &str); 5] = [
    (AssistantRailCommand::Explain, "Explain"),
    (AssistantRailCommand::Fix, "Fix"),
    (AssistantRailCommand::Test, "Tests"),
    (AssistantRailCommand::Doc, "Docs"),
    (AssistantRailCommand::Refactor, "Refactor"),
];

/// Rail commands that dispatch a proposal-only AI run.
///
/// `DesktopAction::ExecuteRailCommand` and its `StartAiProposal` translation
/// have existed since PKT-RAIL and **no renderer pushed either**, so the whole
/// proposal path was unreachable from the UI. Checklist row 5 asks for a
/// deterministic *proposal*; inline prediction is ghost text, which is a
/// different feature that happened to be the only one with a button.
///
/// `selection: None` deliberately: the bridge documents that as "use cursor
/// context", so the commands work without first making a selection, which is
/// how someone reaches for Explain.
pub(crate) fn render_assist_rail_commands(
    ui: &mut egui::Ui,
    snapshot: &ShellProjectionSnapshot,
    stream_in_flight: bool,
    actions: &mut Vec<DesktopAction>,
) {
    // Every command needs a buffer to act on; without one they would dispatch
    // a run whose only outcome is an error.
    if snapshot.active_buffer_projection.buffer_id.is_none() {
        return;
    }
    ui.add_space(8.0);
    theme::card_frame_tinted(
        theme::tokens().bg.card,
        theme::dim(theme::tokens().accent.orange, 80),
    )
    .show(ui, |ui| {
        ui.label(theme::accent(
            "Ask for a proposal",
            theme::tokens().accent.orange,
        ));
        ui.label(theme::muted(
            "Runs proposal-only: nothing is written to the buffer without review.",
        ));
        // Disabled while the shared product-AI lane is busy. `start_ai_proposal`
        // rejects a second request outright, so a live button during a stream
        // is a button whose only outcome is an error toast.
        if stream_in_flight {
            ui.label(theme::muted("Waiting for the current run to finish…"));
        }
        ui.horizontal_wrapped(|ui| {
            for (command, label) in ASSIST_RAIL_COMMANDS {
                if super::primary_button_enabled(
                    ui,
                    label,
                    theme::tokens().accent.orange,
                    !stream_in_flight,
                )
                .clicked()
                {
                    actions.push(DesktopAction::ExecuteRailCommand {
                        command,
                        selection: None,
                    });
                }
            }
        });
    });
}
