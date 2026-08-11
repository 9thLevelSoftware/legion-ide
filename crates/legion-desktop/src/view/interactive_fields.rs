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
pub(crate) fn render_anthropic_byok_form(ui: &mut egui::Ui, actions: &mut Vec<DesktopAction>) {
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

/// Translate an egui key event to a terminal escape sequence.
///
/// Returns `None` for keys that should be handled as normal text input.
/// `application_cursor_keys` controls whether arrow keys use application mode
/// (SS3 `\x1bO`) or normal mode (CSI `\x1b[`).
pub(crate) fn translate_key_to_escape(
    key: &egui::Key,
    modifiers: &egui::Modifiers,
    application_cursor_keys: bool,
) -> Option<Vec<u8>> {
    // Ctrl+letter combinations: Ctrl+A = 0x01 through Ctrl+Z = 0x1A.
    if modifiers.ctrl {
        let ctrl_byte = match key {
            egui::Key::A => Some(0x01),
            egui::Key::B => Some(0x02),
            egui::Key::C => Some(0x03),
            egui::Key::D => Some(0x04),
            egui::Key::E => Some(0x05),
            egui::Key::F => Some(0x06),
            egui::Key::G => Some(0x07),
            egui::Key::H => Some(0x08),
            egui::Key::I => Some(0x09),
            egui::Key::J => Some(0x0a),
            egui::Key::K => Some(0x0b),
            egui::Key::L => Some(0x0c),
            egui::Key::M => Some(0x0d),
            egui::Key::N => Some(0x0e),
            egui::Key::O => Some(0x0f),
            egui::Key::P => Some(0x10),
            egui::Key::Q => Some(0x11),
            egui::Key::R => Some(0x12),
            egui::Key::S => Some(0x13),
            egui::Key::T => Some(0x14),
            egui::Key::U => Some(0x15),
            egui::Key::V => Some(0x16),
            egui::Key::W => Some(0x17),
            egui::Key::X => Some(0x18),
            egui::Key::Y => Some(0x19),
            egui::Key::Z => Some(0x1a),
            _ => None,
        };
        if let Some(byte) = ctrl_byte {
            return Some(vec![byte]);
        }
    }

    // Arrow keys: application mode uses SS3, normal mode uses CSI.
    let arrow_prefix: &[u8] = if application_cursor_keys {
        b"\x1bO"
    } else {
        b"\x1b["
    };
    match key {
        egui::Key::ArrowUp => return Some([arrow_prefix, b"A"].concat()),
        egui::Key::ArrowDown => return Some([arrow_prefix, b"B"].concat()),
        egui::Key::ArrowRight => return Some([arrow_prefix, b"C"].concat()),
        egui::Key::ArrowLeft => return Some([arrow_prefix, b"D"].concat()),
        _ => {}
    }

    // Navigation keys.
    match key {
        egui::Key::Home => return Some(b"\x1b[H".to_vec()),
        egui::Key::End => return Some(b"\x1b[F".to_vec()),
        egui::Key::PageUp => return Some(b"\x1b[5~".to_vec()),
        egui::Key::PageDown => return Some(b"\x1b[6~".to_vec()),
        egui::Key::Insert => return Some(b"\x1b[2~".to_vec()),
        egui::Key::Delete => return Some(b"\x1b[3~".to_vec()),
        _ => {}
    }

    // Function keys.
    match key {
        egui::Key::F1 => return Some(b"\x1bOP".to_vec()),
        egui::Key::F2 => return Some(b"\x1bOQ".to_vec()),
        egui::Key::F3 => return Some(b"\x1bOR".to_vec()),
        egui::Key::F4 => return Some(b"\x1bOS".to_vec()),
        egui::Key::F5 => return Some(b"\x1b[15~".to_vec()),
        egui::Key::F6 => return Some(b"\x1b[17~".to_vec()),
        egui::Key::F7 => return Some(b"\x1b[18~".to_vec()),
        egui::Key::F8 => return Some(b"\x1b[19~".to_vec()),
        egui::Key::F9 => return Some(b"\x1b[20~".to_vec()),
        egui::Key::F10 => return Some(b"\x1b[21~".to_vec()),
        egui::Key::F11 => return Some(b"\x1b[23~".to_vec()),
        egui::Key::F12 => return Some(b"\x1b[24~".to_vec()),
        _ => {}
    }

    // Special keys.
    match key {
        egui::Key::Escape => Some(b"\x1b".to_vec()),
        egui::Key::Tab => Some(b"\t".to_vec()),
        egui::Key::Backspace => Some(b"\x7f".to_vec()),
        egui::Key::Enter => Some(b"\r".to_vec()),
        _ => None,
    }
}

/// Render the active terminal input line; submit sends `TerminalInput`.
///
/// Intercepts special keys (arrows, function keys, Ctrl combinations) and
/// translates them to terminal escape sequences. Regular text is sent as-is.
pub(crate) fn render_terminal_input_line(ui: &mut egui::Ui, actions: &mut Vec<DesktopAction>) {
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

        // Intercept special keys when the text field has focus.
        if response.has_focus() {
            ui.input(|input| {
                for event in &input.events {
                    if let egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } = event
                    {
                        if let Some(escape_bytes) =
                            translate_key_to_escape(key, modifiers, false)
                        {
                            // Enter: send the draft content with \r (not \n)
                            if *key == egui::Key::Enter {
                                if !draft.is_empty() {
                                    let mut payload = draft.clone();
                                    payload.push('\r');
                                    actions.push(DesktopAction::TerminalInput { payload });
                                    draft.clear();
                                } else {
                                    // Empty draft: send bare CR
                                    let payload =
                                        String::from_utf8_lossy(&escape_bytes).into_owned();
                                    actions.push(DesktopAction::TerminalInput { payload });
                                }
                            } else {
                                // Special key: send the escape sequence directly
                                let payload =
                                    String::from_utf8_lossy(&escape_bytes).into_owned();
                                actions.push(DesktopAction::TerminalInput { payload });
                            }
                        }
                    }
                }
            });
        }

        if ui.small_button("Send").clicked() && !draft.is_empty() {
            let mut payload = draft.clone();
            payload.push('\r');
            actions.push(DesktopAction::TerminalInput { payload });
            draft.clear();
            response.request_focus();
        }

        ui.ctx().data_mut(|data| {
            data.insert_temp(draft_id, draft);
        });
    });
}
