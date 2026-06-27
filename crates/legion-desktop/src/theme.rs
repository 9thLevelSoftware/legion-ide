//! Native shell theme tokens translated from the Legion prototype mockups.

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Frame, Margin, RichText, Stroke,
};
use std::{
    cell::Cell,
    fs,
    sync::{Arc, OnceLock},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemeVariant {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ThemePreference {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemePreference {
    pub(crate) const fn all() -> [Self; 3] {
        [Self::Dark, Self::Light, Self::System]
    }

    pub(crate) fn resolve(self, ctx: &egui::Context) -> Theme {
        match self {
            Self::Dark => Theme::dark(),
            Self::Light => Theme::light(),
            Self::System => match ctx.system_theme() {
                Some(egui::Theme::Light) => Theme::light(),
                Some(egui::Theme::Dark) | None => Theme::dark(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Theme {
    pub(crate) variant: ThemeVariant,
    pub(crate) bg: BackgroundTokens,
    pub(crate) border: BorderTokens,
    pub(crate) text: TextTokens,
    pub(crate) accent: AccentTokens,
    pub(crate) spacing: SpacingScale,
    pub(crate) radius: RadiusScale,
    pub(crate) typography: TypographyScale,
}

impl Theme {
    pub(crate) const fn dark() -> Self {
        Self {
            variant: ThemeVariant::Dark,
            bg: BackgroundTokens {
                surface: Color32::from_rgb(13, 13, 18),
                panel: Color32::from_rgb(17, 17, 24),
                toolbar: Color32::from_rgb(17, 17, 24),
                card: Color32::from_rgb(21, 21, 31),
                code: Color32::from_rgb(11, 11, 16),
                input: Color32::from_rgb(16, 16, 24),
                hover: Color32::from_rgb(32, 32, 43),
                active: Color32::from_rgb(37, 37, 53),
                overlay: Color32::from_rgb(26, 26, 36),
                scrim: Color32::from_rgba_unmultiplied_const(0, 0, 0, 138),
                canvas: Color32::from_rgb(8, 8, 12),
                ghost: Color32::from_rgba_unmultiplied_const(255, 255, 255, 8),
            },
            border: BorderTokens {
                subtle: Color32::from_rgba_unmultiplied_const(255, 255, 255, 13),
                default: Color32::from_rgba_unmultiplied_const(255, 255, 255, 20),
                strong: Color32::from_rgba_unmultiplied_const(255, 255, 255, 36),
                focus: Color32::from_rgba_unmultiplied_const(107, 92, 255, 166),
            },
            text: TextTokens {
                primary: Color32::from_rgb(244, 244, 246),
                secondary: Color32::from_rgb(182, 183, 195),
                muted: Color32::from_rgb(126, 129, 144),
                disabled: Color32::from_rgb(85, 88, 104),
                inverted: Color32::from_rgb(9, 9, 13),
            },
            accent: AccentTokens {
                cyan: Color32::from_rgb(57, 215, 255),
                blue: Color32::from_rgb(75, 140, 255),
                violet: Color32::from_rgb(139, 92, 255),
                purple: Color32::from_rgb(177, 108, 255),
                amber: Color32::from_rgb(255, 204, 102),
                orange: Color32::from_rgb(255, 184, 107),
                green: Color32::from_rgb(74, 222, 128),
                red: Color32::from_rgb(255, 92, 122),
            },
            spacing: SpacingScale::standard(),
            radius: RadiusScale::standard(),
            typography: TypographyScale::standard(),
        }
    }

    pub(crate) const fn light() -> Self {
        Self {
            variant: ThemeVariant::Light,
            bg: BackgroundTokens {
                surface: Color32::from_rgb(255, 255, 255),
                panel: Color32::from_rgb(248, 248, 250),
                toolbar: Color32::from_rgb(255, 255, 255),
                card: Color32::from_rgb(255, 255, 255),
                code: Color32::from_rgb(247, 247, 249),
                input: Color32::from_rgb(243, 243, 245),
                hover: Color32::from_rgb(233, 235, 239),
                active: Color32::from_rgb(236, 236, 240),
                overlay: Color32::from_rgb(255, 255, 255),
                scrim: Color32::from_rgba_unmultiplied_const(3, 2, 19, 54),
                canvas: Color32::from_rgb(242, 243, 246),
                ghost: Color32::from_rgba_unmultiplied_const(3, 2, 19, 10),
            },
            border: BorderTokens {
                subtle: Color32::from_rgba_unmultiplied_const(0, 0, 0, 13),
                default: Color32::from_rgba_unmultiplied_const(0, 0, 0, 26),
                strong: Color32::from_rgba_unmultiplied_const(0, 0, 0, 46),
                focus: Color32::from_rgba_unmultiplied_const(107, 92, 255, 166),
            },
            text: TextTokens {
                primary: Color32::from_rgb(3, 2, 19),
                secondary: Color32::from_rgb(74, 75, 88),
                muted: Color32::from_rgb(113, 113, 130),
                disabled: Color32::from_rgb(151, 153, 166),
                inverted: Color32::from_rgb(255, 255, 255),
            },
            accent: AccentTokens {
                cyan: Color32::from_rgb(0, 122, 153),
                blue: Color32::from_rgb(38, 100, 214),
                violet: Color32::from_rgb(111, 76, 219),
                purple: Color32::from_rgb(145, 83, 220),
                amber: Color32::from_rgb(156, 104, 0),
                orange: Color32::from_rgb(180, 91, 18),
                green: Color32::from_rgb(31, 128, 73),
                red: Color32::from_rgb(212, 24, 61),
            },
            spacing: SpacingScale::standard(),
            radius: RadiusScale::standard(),
            typography: TypographyScale::standard(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BackgroundTokens {
    pub(crate) surface: Color32,
    pub(crate) panel: Color32,
    pub(crate) toolbar: Color32,
    pub(crate) card: Color32,
    pub(crate) code: Color32,
    pub(crate) input: Color32,
    pub(crate) hover: Color32,
    pub(crate) active: Color32,
    pub(crate) overlay: Color32,
    pub(crate) scrim: Color32,
    pub(crate) canvas: Color32,
    pub(crate) ghost: Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BorderTokens {
    pub(crate) subtle: Color32,
    pub(crate) default: Color32,
    pub(crate) strong: Color32,
    pub(crate) focus: Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextTokens {
    pub(crate) primary: Color32,
    pub(crate) secondary: Color32,
    pub(crate) muted: Color32,
    pub(crate) disabled: Color32,
    pub(crate) inverted: Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AccentTokens {
    pub(crate) cyan: Color32,
    pub(crate) blue: Color32,
    pub(crate) violet: Color32,
    pub(crate) purple: Color32,
    pub(crate) amber: Color32,
    pub(crate) orange: Color32,
    pub(crate) green: Color32,
    pub(crate) red: Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpacingScale {
    pub(crate) xs: i8,
    pub(crate) sm: i8,
    pub(crate) md: i8,
    pub(crate) lg: i8,
    pub(crate) xl: i8,
    pub(crate) xxl: i8,
}

impl SpacingScale {
    const fn standard() -> Self {
        Self {
            xs: 2,
            sm: 4,
            md: 8,
            lg: 12,
            xl: 16,
            xxl: 24,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RadiusScale {
    pub(crate) sm: u8,
    pub(crate) md: u8,
    pub(crate) lg: u8,
}

impl RadiusScale {
    const fn standard() -> Self {
        Self {
            sm: 6,
            md: 8,
            lg: 12,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TypographyScale {
    pub(crate) eyebrow: u8,
    pub(crate) label: u8,
    pub(crate) body: u8,
    pub(crate) heading: u8,
    pub(crate) title: u8,
    pub(crate) code: u8,
    pub(crate) code_muted: u8,
}

impl TypographyScale {
    const fn standard() -> Self {
        Self {
            eyebrow: 10,
            label: 11,
            body: 12,
            heading: 13,
            title: 15,
            code: 12,
            code_muted: 11,
        }
    }
}

thread_local! {
    static ACTIVE_THEME: Cell<Theme> = const { Cell::new(Theme::dark()) };
}

pub(crate) fn tokens() -> Theme {
    ACTIVE_THEME.with(Cell::get)
}

// egui's default font set only covers latin/cyrillic; load a CJK-capable fallback from the host OS
// so the editor can render and measure composition text without bundling large binary fonts here.
static CJK_FONT_DEFINITIONS: OnceLock<Option<FontDefinitions>> = OnceLock::new();

fn cjk_font_definitions() -> Option<FontDefinitions> {
    CJK_FONT_DEFINITIONS
        .get_or_init(build_cjk_font_definitions)
        .clone()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FontFallbackProbe {
    pub(crate) requested_family_label: String,
    pub(crate) resolved_family_label: String,
    pub(crate) coverage_label: String,
    pub(crate) fallback_found: bool,
    pub(crate) message: String,
}

pub(crate) fn font_fallback_probe(requested_family_label: &str) -> FontFallbackProbe {
    let fallback_found = cjk_font_definitions().is_some();
    FontFallbackProbe {
        requested_family_label: sanitize_font_family_label(requested_family_label),
        resolved_family_label: if fallback_found {
            "legion-cjk-fallback".to_string()
        } else {
            "egui-default".to_string()
        },
        coverage_label: "cjk".to_string(),
        fallback_found,
        message: if fallback_found {
            "CJK fallback loaded from host font catalog".to_string()
        } else {
            "CJK fallback not found in host font catalog".to_string()
        },
    }
}

fn sanitize_font_family_label(value: &str) -> String {
    let label = value.trim();
    if label.is_empty() {
        return "monospace".to_string();
    }

    let normalized = label
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
        .take(64)
        .collect::<String>();
    if normalized.trim().is_empty() {
        "monospace".to_string()
    } else {
        normalized
    }
}

fn build_cjk_font_definitions() -> Option<FontDefinitions> {
    let mut fonts = FontDefinitions::default();
    let (font_name, font_bytes) = load_cjk_font_bytes()?;

    fonts.font_data.insert(
        font_name.clone(),
        Arc::new(FontData::from_owned(font_bytes)),
    );
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .expect("proportional family should exist")
        .push(font_name.clone());
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .expect("monospace family should exist")
        .push(font_name);
    Some(fonts)
}

fn load_cjk_font_bytes() -> Option<(String, Vec<u8>)> {
    for path in cjk_font_candidates() {
        if let Ok(bytes) = fs::read(path) {
            return Some(("legion-cjk-fallback".to_string(), bytes));
        }
    }
    None
}

fn cjk_font_candidates() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        ]
    }

    #[cfg(target_os = "windows")]
    {
        &[
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\msjh.ttc",
            "C:\\Windows\\Fonts\\malgun.ttf",
            "C:\\Windows\\Fonts\\msgothic.ttc",
            "C:\\Windows\\Fonts\\simsun.ttc",
        ]
    }

    #[cfg(target_os = "linux")]
    {
        &[
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansSC-Regular.otf",
            "/usr/share/fonts/truetype/noto/NotoSansSC-Regular.otf",
            "/usr/share/fonts/truetype/arphic/uming.ttc",
        ]
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        &[]
    }
}

pub(crate) fn install(ctx: &egui::Context, theme: &Theme) {
    ACTIVE_THEME.with(|active| active.set(*theme));
    ctx.set_theme(match theme.variant {
        ThemeVariant::Dark => egui::Theme::Dark,
        ThemeVariant::Light => egui::Theme::Light,
    });
    let fallback_probe = font_fallback_probe("monospace");
    if fallback_probe.fallback_found
        && let Some(fonts) = cjk_font_definitions()
    {
        ctx.set_fonts(fonts);
    }
    let mut visuals = match theme.variant {
        ThemeVariant::Dark => egui::Visuals::dark(),
        ThemeVariant::Light => egui::Visuals::light(),
    };
    visuals.panel_fill = theme.bg.surface;
    visuals.window_fill = theme.bg.overlay;
    visuals.extreme_bg_color = theme.bg.code;
    visuals.faint_bg_color = theme.bg.card;
    visuals.selection.bg_fill = theme.bg.active;
    visuals.selection.stroke = Stroke::new(1.0, theme.accent.cyan);
    visuals.warn_fg_color = theme.accent.amber;
    visuals.error_fg_color = theme.accent.red;
    visuals.widgets.noninteractive.bg_fill = theme.bg.panel;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.text.secondary);
    visuals.widgets.inactive.bg_fill = theme.bg.card;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, theme.text.secondary);
    visuals.widgets.hovered.bg_fill = theme.bg.hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme.border.strong);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, theme.text.primary);
    visuals.widgets.active.bg_fill = theme.bg.active;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, theme.border.strong);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, theme.text.primary);
    visuals.widgets.open.bg_fill = theme.bg.active;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, theme.border.focus);
    visuals.window_corner_radius = CornerRadius::same(theme.radius.md);
    visuals.menu_corner_radius = CornerRadius::same(theme.radius.md);
    ctx.set_visuals(visuals);
}

pub(crate) fn panel_frame(fill: Color32) -> Frame {
    Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, tokens().border.subtle))
        .inner_margin(Margin::same(tokens().spacing.md))
}

pub(crate) fn pane_frame(fill: Color32) -> Frame {
    Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, tokens().border.subtle))
        .inner_margin(Margin::same(0))
}

pub(crate) fn toolbar_frame() -> Frame {
    Frame::NONE
        .fill(tokens().bg.toolbar)
        .stroke(Stroke::new(1.0, tokens().border.subtle))
        .inner_margin(Margin::symmetric(
            tokens().spacing.lg,
            tokens().spacing.sm + tokens().spacing.xs,
        ))
}

pub(crate) fn card_frame_tinted(fill: Color32, stroke: Color32) -> Frame {
    Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(CornerRadius::same(tokens().radius.md))
        .inner_margin(Margin::same(tokens().spacing.md))
}

pub(crate) fn small_card_frame() -> Frame {
    Frame::NONE
        .fill(tokens().bg.card)
        .stroke(Stroke::new(1.0, tokens().border.default))
        .corner_radius(CornerRadius::same(tokens().radius.sm))
        .inner_margin(Margin::symmetric(
            tokens().spacing.md,
            tokens().spacing.sm + tokens().spacing.xs,
        ))
}

pub(crate) fn code_frame() -> Frame {
    Frame::NONE
        .fill(tokens().bg.code)
        .stroke(Stroke::new(1.0, tokens().border.subtle))
        .inner_margin(Margin::same(0))
}

pub(crate) fn ghost_frame() -> Frame {
    Frame::NONE
        .fill(tokens().bg.ghost)
        .stroke(Stroke::new(1.0, tokens().border.default))
        .corner_radius(CornerRadius::same(tokens().radius.sm))
        .inner_margin(Margin::symmetric(tokens().spacing.md, tokens().spacing.sm))
}

pub(crate) fn heading(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(tokens().text.primary)
        .strong()
        .size(tokens().typography.heading as f32)
}

pub(crate) fn title(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(tokens().text.primary)
        .strong()
        .size(tokens().typography.title as f32)
}

pub(crate) fn eyebrow(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(tokens().text.muted)
        .strong()
        .size(tokens().typography.eyebrow as f32)
}

pub(crate) fn label(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(tokens().text.secondary)
        .strong()
        .size(tokens().typography.label as f32)
}

pub(crate) fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(tokens().text.muted)
        .size(tokens().typography.label as f32)
}

pub(crate) fn body(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(tokens().text.secondary)
        .size(tokens().typography.body as f32)
}

pub(crate) fn body_strong(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(tokens().text.primary)
        .strong()
        .size(tokens().typography.body as f32)
}

pub(crate) fn code(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(tokens().text.secondary)
        .monospace()
        .size(tokens().typography.code as f32)
}

pub(crate) fn code_muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(tokens().text.muted)
        .monospace()
        .size(tokens().typography.code_muted as f32)
}

pub(crate) fn accent(text: impl Into<String>, color: Color32) -> RichText {
    RichText::new(text.into()).color(color).strong().size(11.0)
}

pub(crate) fn inverse(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .color(tokens().text.inverted)
        .strong()
        .size(tokens().typography.label as f32)
}

pub(crate) const fn dim(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied_const(color.r(), color.g(), color.b(), alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_and_light_tokens_are_distinct_and_complete() {
        let dark = Theme::dark();
        let light = Theme::light();

        assert_eq!(dark.variant, ThemeVariant::Dark);
        assert_eq!(light.variant, ThemeVariant::Light);
        assert_ne!(dark.bg.surface, light.bg.surface);
        assert_ne!(dark.text.primary, light.text.primary);
        assert_ne!(dark.border.default, light.border.default);
        assert_ne!(dark.accent.red, light.accent.red);
        assert_eq!(dark.spacing.md, light.spacing.md);
        assert_eq!(dark.radius.md, light.radius.md);
    }
}
