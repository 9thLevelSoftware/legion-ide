//! Reusable semantic building blocks for native desktop surfaces.

use std::hash::Hash;

use crate::theme;

pub(super) fn section_header(ui: &mut egui::Ui, label: &str, color: Option<egui::Color32>) {
    ui.add_space(6.0);
    match color {
        Some(color) => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (_id, marker) = ui.allocate_space(egui::vec2(3.0, 11.0));
                ui.painter()
                    .rect_filled(marker, egui::CornerRadius::same(1), color);
                ui.label(theme::body_strong(label).size(theme::tokens().typography.eyebrow as f32));
            });
        }
        None => {
            ui.label(theme::eyebrow(label));
        }
    }
}

pub(super) fn surface_card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    theme::small_card_frame().show(ui, add_contents)
}

pub(super) fn empty_state(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.vertical_centered(|ui| {
        ui.label(theme::title(title));
        ui.label(theme::muted(detail));
    });
}

pub(super) fn prerequisite_card(ui: &mut egui::Ui, title: &str, detail: &str, ready: bool) {
    surface_card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::body_strong(title));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                status_badge(
                    ui,
                    if ready { "Ready" } else { "Required" },
                    if ready {
                        theme::tokens().accent.green
                    } else {
                        theme::tokens().accent.orange
                    },
                    ready,
                );
            });
        });
        ui.label(theme::muted(detail));
    });
}

pub(super) fn disclosure_row<R>(
    ui: &mut egui::Ui,
    label: &str,
    id_salt: impl Hash,
    default_open: bool,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::CollapsingResponse<R> {
    egui::CollapsingHeader::new(theme::label(label))
        .id_salt(id_salt)
        .default_open(default_open)
        .show(ui, add_body)
}

pub(super) fn segmented_tab(
    ui: &mut egui::Ui,
    label: &str,
    active: bool,
    color: egui::Color32,
) -> egui::Response {
    let text = if active {
        theme::accent(label, color)
    } else {
        theme::muted(label)
    };
    ui.add_sized(
        [108.0, f32::from(theme::tokens().control_height.standard)],
        egui::Button::new(text)
            .selected(active)
            .fill(if active {
                theme::dim(color, 28)
            } else {
                theme::tokens().surfaces.canvas
            })
            .stroke(egui::Stroke::new(
                1.0_f32,
                if active {
                    theme::dim(color, 90)
                } else {
                    theme::tokens().border.subtle
                },
            )),
    )
}

pub(super) fn status_badge(
    ui: &mut egui::Ui,
    label: &str,
    color: egui::Color32,
    active: bool,
) -> egui::Response {
    let fill = if active {
        theme::dim(color, 28)
    } else {
        theme::dim(theme::tokens().text.primary, 10)
    };
    let response = ui.add(
        egui::Button::new(theme::accent(label, color))
            .wrap_mode(egui::TextWrapMode::Extend)
            .sense(egui::Sense::hover())
            .fill(fill)
            .stroke(egui::Stroke::new(
                1.0_f32,
                if active {
                    theme::dim(color, 90)
                } else {
                    theme::tokens().border.default
                },
            ))
            .corner_radius(egui::CornerRadius::same(theme::tokens().radius.sm))
            .min_size(egui::vec2(0.0, 20.0)),
    );
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Label);
        node.clear_actions();
    });
    response
}

pub(super) fn selectable_pill_button(
    ui: &mut egui::Ui,
    label: &str,
    color: egui::Color32,
    selected: bool,
) -> egui::Response {
    ui.add(
        egui::Button::new(theme::accent(label, color))
            .selected(selected)
            .min_size(egui::vec2(
                f32::from(theme::tokens().control_height.compact),
                f32::from(theme::tokens().control_height.standard),
            ))
            .fill(if selected {
                theme::dim(color, 28)
            } else {
                theme::tokens().controls.rest
            })
            .stroke(egui::Stroke::new(
                1.0_f32,
                if selected {
                    theme::dim(color, 90)
                } else {
                    theme::tokens().border.default
                },
            ))
            .corner_radius(egui::CornerRadius::same(theme::tokens().radius.sm)),
    )
}

pub(super) fn soft_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(theme::label(label))
            .min_size(egui::vec2(
                f32::from(theme::tokens().control_height.compact),
                f32::from(theme::tokens().control_height.standard),
            ))
            .fill(theme::tokens().controls.rest)
            .stroke(egui::Stroke::new(1.0_f32, theme::tokens().border.default))
            .corner_radius(egui::CornerRadius::same(theme::tokens().radius.sm)),
    )
}

pub(super) fn top_bar_command_button(ui: &mut egui::Ui) -> egui::Response {
    ui.add(
        egui::Button::new(theme::label("Command"))
            .min_size(egui::vec2(
                72.0,
                f32::from(theme::tokens().control_height.standard),
            ))
            .fill(theme::tokens().controls.rest)
            .stroke(egui::Stroke::new(1.0_f32, theme::tokens().border.default))
            .corner_radius(egui::CornerRadius::same(theme::tokens().radius.sm)),
    )
}

pub(super) fn primary_button(
    ui: &mut egui::Ui,
    label: &str,
    color: egui::Color32,
) -> egui::Response {
    primary_button_enabled(ui, label, color, true)
}

pub(super) fn primary_button_enabled(
    ui: &mut egui::Ui,
    label: &str,
    color: egui::Color32,
    enabled: bool,
) -> egui::Response {
    let button = egui::Button::new(theme::inverse(label))
        .min_size(egui::vec2(
            f32::from(theme::tokens().control_height.compact),
            f32::from(theme::tokens().control_height.prominent),
        ))
        .fill(color)
        .stroke(egui::Stroke::new(1.0_f32, theme::dim(color, 180)))
        .corner_radius(egui::CornerRadius::same(theme::tokens().radius.sm));
    if enabled {
        ui.add(button)
    } else {
        ui.add_enabled(false, button.sense(egui::Sense::hover()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted_text_color(theme_tokens: theme::Theme, accent: egui::Color32) -> egui::Color32 {
        fn collect(shape: &egui::Shape, colors: &mut Vec<egui::Color32>) {
            match shape {
                egui::Shape::Text(text) if text.galley.job.text == "Section heading" => {
                    colors.extend(
                        text.galley
                            .job
                            .sections
                            .iter()
                            .map(|section| section.format.color),
                    );
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, colors);
                    }
                }
                _ => {}
            }
        }

        let context = egui::Context::default();
        theme::install(&context, &theme_tokens);
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(320.0, 180.0),
                )),
                ..egui::RawInput::default()
            },
            |context| {
                egui::CentralPanel::default().show_inside(context, |ui| {
                    section_header(ui, "Section heading", Some(accent));
                });
            },
        );
        let mut colors = Vec::new();
        for clipped in &output.shapes {
            collect(&clipped.shape, &mut colors);
        }
        colors.sort_unstable_by_key(|color| color.to_array());
        colors.dedup();
        assert_eq!(
            colors.len(),
            1,
            "the rendered section heading should use one text color: {colors:?}"
        );
        colors[0]
    }

    fn contrast_ratio(foreground: egui::Color32, background: egui::Color32) -> f64 {
        fn channel(value: u8) -> f64 {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }
        fn luminance(color: egui::Color32) -> f64 {
            0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
        }

        let foreground = luminance(foreground);
        let background = luminance(background);
        (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
    }

    #[test]
    fn colored_section_heading_text_meets_normal_text_contrast_on_shell_surfaces() {
        for (theme_name, theme_tokens) in [
            ("dark", theme::Theme::dark()),
            ("light", theme::Theme::light()),
        ] {
            for (accent_name, accent) in [
                ("cyan", theme_tokens.accent.cyan),
                ("blue", theme_tokens.accent.blue),
                ("violet", theme_tokens.accent.violet),
                ("purple", theme_tokens.accent.purple),
                ("amber", theme_tokens.accent.amber),
                ("orange", theme_tokens.accent.orange),
                ("green", theme_tokens.accent.green),
                ("red", theme_tokens.accent.red),
            ] {
                let text_color = painted_text_color(theme_tokens, accent);
                for (surface_name, surface) in [
                    ("panel", theme_tokens.surfaces.panel),
                    ("raised", theme_tokens.surfaces.raised),
                ] {
                    let ratio = contrast_ratio(text_color, surface);
                    assert!(
                        ratio >= 4.5,
                        "{theme_name} {accent_name} section text on {surface_name} must meet 4.5:1; actual={ratio:.3}:1, color={text_color:?}"
                    );
                }
            }
        }
    }
}
