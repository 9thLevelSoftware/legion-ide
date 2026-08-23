//! Interactive text fields that intentionally use `egui::TextEdit`.
//!
//! These widgets are **not** the code-canvas editor (which remains a custom
//! painter). The `no-egui-textedit` gate only scans `view.rs` and
//! `code_canvas_painter.rs`; this module is the approved home for terminal
//! input, BYOK key entry, and similar adapter-local forms.

use std::borrow::Cow;

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
        "Preferred AI provider: {active_preference}. Auto uses providers on this computer and never routes remotely; choose Anthropic for that."
    )));
    ui.horizontal_wrapped(|ui| {
        for (label, id) in [
            ("Auto (local-first)", "auto"),
            ("Ollama", "ollama"),
            // Offered because a preference nothing can select is the same as
            // no preference. `Auto` prefers Ollama when both are reachable, so
            // without a button a llama.cpp user has no way to choose the server
            // they deliberately started -- the parser accepts the label and the
            // product never produces it.
            ("llama.cpp", "llama-cpp"),
            ("Anthropic", "anthropic"),
            ("Fixture", "deterministic"),
        ] {
            let selected = active_preference.eq_ignore_ascii_case(id);
            if ui
                .add(
                    egui::Button::new(theme::label(label))
                        .selected(selected)
                        .min_size(egui::vec2(
                            f32::from(theme::tokens().control_height.compact),
                            f32::from(theme::tokens().control_height.compact),
                        )),
                )
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
        "Anthropic API key — stored securely in the operating system keyring and never in workspace files.",
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
            .desired_width(220.0)
            .min_size(egui::vec2(
                f32::from(theme::tokens().control_height.compact),
                f32::from(theme::tokens().control_height.compact),
            ))
            .margin(egui::Margin::symmetric(4, 8)),
    );
    if response.changed() {
        ui.ctx().data_mut(|data| {
            data.insert_temp(draft_id, draft.clone());
        });
    }
    ui.horizontal(|ui| {
        if super::soft_button(ui, "Save Anthropic key").clicked() {
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
        if super::soft_button(ui, "Clear Anthropic key").clicked() {
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
pub(crate) fn render_terminal_input_line(
    ui: &mut egui::Ui,
    actions: &mut Vec<DesktopAction>,
    application_cursor_keys: bool,
) {
    let draft_id = egui::Id::new("legion-terminal-input-draft");
    let mut draft = ui
        .ctx()
        .data_mut(|data| data.get_temp::<String>(draft_id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.label(theme::code_muted("$"));
        let response = ui.add(
            egui::TextEdit::singleline(&mut draft)
                .id(terminal_input_widget_id())
                .desired_width((ui.available_width() - 80.0).max(40.0))
                .hint_text("type and press Enter to send to the PTY")
                .min_size(egui::vec2(
                    f32::from(theme::tokens().control_height.compact),
                    f32::from(theme::tokens().control_height.compact),
                ))
                .margin(egui::Margin::symmetric(4, 8)),
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
                        && let Some(escape_bytes) =
                            translate_key_to_escape(key, modifiers, application_cursor_keys)
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
                                let payload = String::from_utf8_lossy(&escape_bytes).into_owned();
                                actions.push(DesktopAction::TerminalInput { payload });
                            }
                        } else {
                            // Special key: send the escape sequence directly
                            let payload = String::from_utf8_lossy(&escape_bytes).into_owned();
                            actions.push(DesktopAction::TerminalInput { payload });
                        }
                    }
                }
            });
        }

        if super::soft_button(ui, "Send").clicked() && !draft.is_empty() {
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

pub(crate) fn terminal_input_widget_id() -> egui::Id {
    egui::Id::new("legion-terminal-input")
}

#[cfg(test)]
mod terminal_key_tests {
    use super::translate_key_to_escape;

    #[test]
    fn application_cursor_mode_uses_application_arrow_sequences() {
        assert_eq!(
            translate_key_to_escape(&egui::Key::ArrowUp, &egui::Modifiers::NONE, true),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            translate_key_to_escape(&egui::Key::ArrowRight, &egui::Modifiers::NONE, false),
            Some(b"\x1b[C".to_vec())
        );
    }
}

/// Build a single-line text editor for search and replace overlays.
///
/// Search inputs are interactive fields outside the code-canvas renderer, so
/// they intentionally use egui's standard text-edit widget. Keeping the
/// construction here prevents the code-canvas module from depending on it.
pub(crate) fn find_bar_text_edit<'a>(
    text: &'a mut String,
    hint: &'static str,
    id: egui::Id,
) -> egui::TextEdit<'a> {
    egui::TextEdit::singleline(text)
        .desired_width(180.0)
        .hint_text(hint)
        .id(id)
        .min_size(egui::vec2(
            f32::from(theme::tokens().control_height.compact),
            f32::from(theme::tokens().control_height.compact),
        ))
        .margin(egui::Margin::symmetric(4, 8))
}

/// Render the adapter-local, unsent Delegate task draft.
///
/// The returned value contains the trimmed task only when the user activates
/// the CTA with a non-empty draft. The caller remains responsible for
/// constructing the projected scope and dispatching the product action.
pub(crate) fn render_delegate_task_draft(
    ui: &mut egui::Ui,
    canonical_scope_available: bool,
) -> Option<String> {
    let draft_id = egui::Id::new("legion-delegate-task-draft-value");
    let mut draft = ui
        .ctx()
        .data_mut(|data| data.get_temp::<String>(draft_id).unwrap_or_default());
    draft = bounded_delegate_task_draft(&draft).into_owned();
    let label = ui
        .push_id("legion-delegate-task-label", |ui| {
            ui.label(theme::label("Task description"))
        })
        .inner;
    let response = ui.add(
        egui::TextEdit::multiline(&mut draft)
            .id_source("legion-delegate-task-draft")
            .char_limit(super::DELEGATE_TASK_DRAFT_MAX_CHARS)
            .desired_rows(3)
            .desired_width(ui.available_width())
            .hint_text("Describe a bounded task for Delegate")
            .min_size(egui::vec2(
                f32::from(theme::tokens().control_height.compact),
                f32::from(theme::tokens().control_height.compact),
            ))
            .margin(egui::Margin::symmetric(4, 8)),
    );
    let field_id = response.id;
    response.labelled_by(label.id);
    // Named, not merely related. `labelled_by` records a relation and leaves the
    // node anonymous, which was tolerable while this was the only multiline
    // field on the surface; the Delegate chat composer is a second one, so an
    // unnamed box is now ambiguous to a screen reader and to anything driving
    // the accessibility tree.
    ui.ctx()
        .accesskit_node_builder(field_id, |node| node.set_label("Task description"));
    draft = bounded_delegate_task_draft(&draft).into_owned();
    let ready = canonical_scope_available && !draft.trim().is_empty();
    let submitted = ui
        .push_id("legion-delegate-task-submit", |ui| {
            super::primary_button_enabled(ui, "Delegate task", theme::tokens().accent.amber, ready)
                .on_hover_text("Start a proposal-mediated delegated task")
                .clicked()
        })
        .inner;
    let task = (submitted && ready).then(|| draft.trim().to_string());
    if canonical_scope_available && draft.trim().is_empty() {
        ui.label(theme::muted("Describe a task to start Delegate."));
    }
    if task.is_some() {
        draft.clear();
    }
    ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
    task
}

/// Render the adapter-local, unsent Delegate chat prompt.
///
/// Returns the trimmed prompt only when the user activates Send with a
/// non-empty draft and the surface is ready to accept one. Bounded by the same
/// draft budget as the task description, because both end up in a display-safe
/// projected label.
pub(crate) fn render_delegate_chat_draft(ui: &mut egui::Ui, ready: bool) -> Option<String> {
    let draft_id = egui::Id::new("legion-delegate-chat-draft-value");
    let mut draft = ui
        .ctx()
        .data_mut(|data| data.get_temp::<String>(draft_id).unwrap_or_default());
    let label = ui
        .push_id("legion-delegate-chat-label", |ui| {
            ui.label(theme::label("Ask Delegate"))
        })
        .inner;
    let response = ui.add(
        egui::TextEdit::multiline(&mut draft)
            .id_source("legion-delegate-chat-draft")
            // The chat prompt limit, not the task-draft limit. This field used
            // to accept 4096 characters while `send_delegate_chat` bounds the
            // prompt to 240 and then the composer cleared the whole draft, so
            // everything past 240 was lost without a word. Capping here means
            // the field cannot accept text the request will not carry.
            .char_limit(legion_app::DELEGATE_CHAT_PROMPT_MAX_CHARS)
            .desired_rows(2)
            .desired_width(ui.available_width())
            .hint_text("Ask about the open file")
            .min_size(egui::vec2(
                f32::from(theme::tokens().control_height.compact),
                f32::from(theme::tokens().control_height.compact),
            ))
            .margin(egui::Margin::symmetric(4, 8)),
    );
    let field_id = response.id;
    response.labelled_by(label.id);
    // `labelled_by` records a relation; it does not name the node. Without an
    // explicit name the field reaches assistive technology — and any test that
    // drives one — as an anonymous text box next to some text.
    ui.ctx()
        .accesskit_node_builder(field_id, |node| node.set_label("Ask Delegate"));
    let sendable = ready && !draft.trim().is_empty();
    let submitted = ui
        .push_id("legion-delegate-chat-send", |ui| {
            super::primary_button_enabled(ui, "Send", theme::tokens().accent.blue, sendable)
                .on_hover_text("Send a Delegate chat turn with workspace context")
                .clicked()
        })
        .inner;
    let prompt = (submitted && sendable).then(|| draft.trim().to_string());
    if prompt.is_some() {
        draft.clear();
    }
    ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
    prompt
}

/// Return the longest valid UTF-8 prefix inside both Delegate draft budgets.
///
/// This boundary is applied before retaining an adapter-local draft and again
/// before the caller constructs a dispatch action.
pub(crate) fn bounded_delegate_task_draft(value: &str) -> Cow<'_, str> {
    let char_end = value
        .char_indices()
        .nth(super::DELEGATE_TASK_DRAFT_MAX_CHARS)
        .map_or(value.len(), |(index, _)| index);
    let mut end = char_end.min(super::DELEGATE_TASK_DRAFT_MAX_BYTES);
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    if end == value.len() {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(value[..end].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::bounded_delegate_task_draft;
    use crate::view::{DELEGATE_TASK_DRAFT_MAX_BYTES, DELEGATE_TASK_DRAFT_MAX_CHARS};

    #[test]
    fn delegate_draft_bound_preserves_a_valid_utf8_prefix() {
        let oversized = "🦀".repeat(DELEGATE_TASK_DRAFT_MAX_CHARS + 10);

        let bounded = bounded_delegate_task_draft(&oversized);

        assert_eq!(bounded.chars().count(), DELEGATE_TASK_DRAFT_MAX_CHARS);
        assert_eq!(bounded.len(), DELEGATE_TASK_DRAFT_MAX_BYTES);
        assert!(bounded.is_char_boundary(bounded.len()));
    }

    #[test]
    fn delegate_draft_bound_limits_ascii_by_character_budget() {
        let oversized = "x".repeat(DELEGATE_TASK_DRAFT_MAX_CHARS + 10);

        let bounded = bounded_delegate_task_draft(&oversized);

        assert_eq!(bounded.len(), DELEGATE_TASK_DRAFT_MAX_CHARS);
    }
}
