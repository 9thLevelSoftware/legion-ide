//! Interactive text fields that intentionally use `egui::TextEdit`.
//!
//! These widgets are **not** the code-canvas editor (which remains a custom
//! painter). The `no-egui-textedit` gate only scans `view.rs` and
//! `code_canvas_painter.rs`; this module is the approved home for terminal
//! input, BYOK key entry, and similar adapter-local forms.

use crate::bridge::{DesktopAction, SensitiveString};
use crate::theme;

/// Render preferred-provider selection (local-first Auto / Ollama / Anthropic / fixture).
pub(crate) fn render_preferred_provider_picker(
    ui: &mut egui::Ui,
    active_preference: &str,
    actions: &mut Vec<DesktopAction>,
) {
    ui.add_space(4.0);
    ui.label(theme::muted(format!(
        "Preferred route: {active_preference} (Auto tries Ollama loopback first, then Anthropic BYOK)"
    )));
    ui.horizontal_wrapped(|ui| {
        for (label, id) in [
            ("Auto (local-first)", "auto"),
            ("Ollama", "ollama"),
            ("Anthropic", "anthropic"),
            ("Fixture", "deterministic"),
        ] {
            let selected = active_preference.eq_ignore_ascii_case(id);
            if ui
                .selectable_label(selected, label)
                .on_hover_text(format!("Set preferred AI provider to {id}"))
                .clicked()
            {
                actions.push(DesktopAction::SetPreferredAiProvider {
                    provider_id: id.to_string(),
                });
            }
        }
    });
}

/// Render the Anthropic BYOK key entry form and push store/delete actions.
pub(crate) fn render_anthropic_byok_form(
    ui: &mut egui::Ui,
    actions: &mut Vec<DesktopAction>,
) {
    ui.add_space(6.0);
    ui.label(theme::muted(
        "Anthropic BYOK — key is stored in the OS keyring only (never written to disk)",
    ));
    let draft_id = egui::Id::new("legion-byok-anthropic-draft");
    let mut draft = ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_insert_with(draft_id, String::new)
            .clone()
    });
    let response = ui.add(
        egui::TextEdit::singleline(&mut draft)
            .password(true)
            .hint_text("sk-ant-…")
            .desired_width(220.0),
    );
    if response.changed() {
        ui.ctx().data_mut(|data| {
            data.insert_temp(draft_id, draft.clone());
        });
    }
    ui.horizontal(|ui| {
        if ui.small_button("Save Anthropic key").clicked() {
            let key = draft.trim().to_string();
            if !key.is_empty() {
                actions.push(DesktopAction::SetProviderApiKey {
                    provider_id: "anthropic".to_string(),
                    api_key: SensitiveString(key),
                });
                ui.ctx().data_mut(|data| {
                    data.insert_temp(draft_id, String::new());
                });
            }
        }
        if ui.small_button("Clear Anthropic key").clicked() {
            actions.push(DesktopAction::DeleteProviderApiKey {
                provider_id: "anthropic".to_string(),
            });
            ui.ctx().data_mut(|data| {
                data.insert_temp(draft_id, String::new());
            });
        }
    });
}

/// Render the active terminal input line; submit sends `TerminalInput`.
pub(crate) fn render_terminal_input_line(
    ui: &mut egui::Ui,
    actions: &mut Vec<DesktopAction>,
) {
    let draft_id = egui::Id::new("legion-terminal-input-draft");
    let mut draft = ui
        .ctx()
        .data_mut(|data| data.get_temp::<String>(draft_id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.label(theme::code_muted("$"));
        let response = ui.add(
            egui::TextEdit::singleline(&mut draft)
                .desired_width((ui.available_width() - 80.0).max(40.0))
                .hint_text("type and press Enter to send to the PTY"),
        );
        let submit = (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || ui.small_button("Send").clicked();
        if submit && !draft.is_empty() {
            let mut payload = draft.clone();
            if !payload.ends_with('\n') {
                payload.push('\n');
            }
            actions.push(DesktopAction::TerminalInput { payload });
            draft.clear();
            response.request_focus();
        }
        ui.ctx().data_mut(|data| {
            data.insert_temp(draft_id, draft);
        });
    });
}
