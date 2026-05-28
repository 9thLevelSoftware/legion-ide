//! Native shell theme tokens translated from the Legion prototype mockups.

use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke};

pub(crate) const BG_ROOT: Color32 = Color32::from_rgb(13, 13, 18);
pub(crate) const BG_BASE: Color32 = Color32::from_rgb(17, 17, 24);
pub(crate) const BG_RAISED: Color32 = Color32::from_rgb(21, 21, 31);
pub(crate) const BG_ELEVATED: Color32 = Color32::from_rgb(26, 26, 36);
pub(crate) const BG_HOVER: Color32 = Color32::from_rgb(32, 32, 43);
pub(crate) const BG_SELECTED: Color32 = Color32::from_rgb(37, 37, 53);
pub(crate) const BG_INPUT: Color32 = Color32::from_rgb(16, 16, 24);
pub(crate) const BG_CODE: Color32 = Color32::from_rgb(11, 11, 16);
pub(crate) const BG_CANVAS: Color32 = Color32::from_rgb(8, 8, 12);

pub(crate) const BORDER_SUBTLE: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 13);
pub(crate) const BORDER_DEFAULT: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 20);
pub(crate) const BORDER_STRONG: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 36);
pub(crate) const BORDER_FOCUS: Color32 = Color32::from_rgba_unmultiplied_const(107, 92, 255, 166);

pub(crate) const TEXT_PRIMARY: Color32 = Color32::from_rgb(244, 244, 246);
pub(crate) const TEXT_SECONDARY: Color32 = Color32::from_rgb(182, 183, 195);
pub(crate) const TEXT_MUTED: Color32 = Color32::from_rgb(126, 129, 144);
pub(crate) const TEXT_INVERSE: Color32 = Color32::from_rgb(9, 9, 13);

pub(crate) const ACCENT_CYAN: Color32 = Color32::from_rgb(57, 215, 255);
pub(crate) const ACCENT_BLUE: Color32 = Color32::from_rgb(75, 140, 255);
pub(crate) const ACCENT_VIOLET: Color32 = Color32::from_rgb(139, 92, 255);
pub(crate) const ACCENT_PURPLE: Color32 = Color32::from_rgb(177, 108, 255);
pub(crate) const ACCENT_AMBER: Color32 = Color32::from_rgb(255, 204, 102);
pub(crate) const ACCENT_ORANGE: Color32 = Color32::from_rgb(255, 184, 107);
pub(crate) const ACCENT_GREEN: Color32 = Color32::from_rgb(74, 222, 128);
pub(crate) const ACCENT_RED: Color32 = Color32::from_rgb(255, 92, 122);

pub(crate) fn install(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_ROOT;
    visuals.window_fill = BG_ELEVATED;
    visuals.extreme_bg_color = BG_CODE;
    visuals.faint_bg_color = BG_RAISED;
    visuals.selection.bg_fill = BG_SELECTED;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT_CYAN);
    visuals.warn_fg_color = ACCENT_AMBER;
    visuals.error_fg_color = ACCENT_RED;
    visuals.widgets.noninteractive.bg_fill = BG_BASE;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    visuals.widgets.inactive.bg_fill = BG_RAISED;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    visuals.widgets.hovered.bg_fill = BG_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.active.bg_fill = BG_SELECTED;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.open.bg_fill = BG_SELECTED;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, BORDER_FOCUS);
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.menu_corner_radius = CornerRadius::same(8);
    ctx.set_visuals(visuals);
}

pub(crate) fn panel_frame(fill: Color32) -> Frame {
    Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .inner_margin(Margin::same(8))
}

pub(crate) fn pane_frame(fill: Color32) -> Frame {
    Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .inner_margin(Margin::same(0))
}

pub(crate) fn toolbar_frame() -> Frame {
    Frame::NONE
        .fill(BG_BASE)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .inner_margin(Margin::symmetric(12, 6))
}

pub(crate) fn card_frame_tinted(fill: Color32, stroke: Color32) -> Frame {
    Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(8))
}

pub(crate) fn small_card_frame() -> Frame {
    Frame::NONE
        .fill(BG_RAISED)
        .stroke(Stroke::new(1.0, BORDER_DEFAULT))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 6))
}

pub(crate) fn code_frame() -> Frame {
    Frame::NONE
        .fill(BG_CODE)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .inner_margin(Margin::same(0))
}

pub(crate) fn ghost_frame() -> Frame {
    Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 8))
        .stroke(Stroke::new(1.0, BORDER_DEFAULT))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 4))
}

pub(crate) fn heading(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(TEXT_PRIMARY)
        .strong()
        .size(13.0)
}

pub(crate) fn title(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(TEXT_PRIMARY)
        .strong()
        .size(15.0)
}

pub(crate) fn eyebrow(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(TEXT_MUTED)
        .strong()
        .size(10.0)
}

pub(crate) fn label(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(TEXT_SECONDARY)
        .strong()
        .size(11.0)
}

pub(crate) fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(TEXT_MUTED).size(11.0)
}

pub(crate) fn body(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(TEXT_SECONDARY).size(12.0)
}

pub(crate) fn body_strong(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(TEXT_PRIMARY)
        .strong()
        .size(12.0)
}

pub(crate) fn code(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(TEXT_SECONDARY)
        .monospace()
        .size(12.0)
}

pub(crate) fn code_muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(TEXT_MUTED)
        .monospace()
        .size(11.0)
}

pub(crate) fn accent(text: impl Into<String>, color: Color32) -> RichText {
    RichText::new(text.into()).color(color).strong().size(11.0)
}

pub(crate) fn inverse(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(TEXT_INVERSE)
        .strong()
        .size(11.0)
}

pub(crate) fn dim(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}
