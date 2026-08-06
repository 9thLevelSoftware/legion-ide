use legion_protocol::ViewportSemanticTokenKind;
use legion_ui::ShellProjectionSnapshot;

use crate::{bridge::DesktopAction, theme};

use super::{DesktopProjectionViewModel, render_code_lines};

/// Renderer-portable code-canvas painting seam.
///
/// Implementations may translate projected code lines into a concrete renderer's
/// widgets or draw calls, but they must not own editor state or apply mutations
/// directly. Keep this trait object-safe so the desktop adapter can swap the
/// concrete painter without changing the projection boundary.
pub trait CodeCanvasPainter {
    /// Paint projected code lines and translate renderer interactions into
    /// adapter actions.
    fn paint_lines(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &ShellProjectionSnapshot,
        model: &DesktopProjectionViewModel,
        actions: &mut Vec<DesktopAction>,
    );
}

/// egui-backed painter for the current desktop adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct EguiCodeCanvasPainter;

/// Prototype-aligned editor paint roles derived from the active theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PrototypeCodeCanvasTokens {
    pub(super) background: egui::Color32,
    pub(super) current_line: egui::Color32,
    pub(super) selection: egui::Color32,
    pub(super) cursor: egui::Color32,
    pub(super) line_number: egui::Color32,
    pub(super) keyword: egui::Color32,
    pub(super) comment: egui::Color32,
}

pub(super) fn prototype_code_canvas_tokens() -> PrototypeCodeCanvasTokens {
    let tokens = theme::tokens();
    PrototypeCodeCanvasTokens {
        background: tokens.bg.code,
        current_line: theme::dim(tokens.accent.amber, 28),
        selection: theme::dim(tokens.accent.blue, 72),
        cursor: tokens.accent.amber,
        line_number: tokens.text.muted,
        keyword: tokens.accent.amber,
        comment: tokens.text.muted,
    }
}

pub(super) fn semantic_token_color(kind: ViewportSemanticTokenKind) -> egui::Color32 {
    let tokens = theme::tokens();
    let canvas = prototype_code_canvas_tokens();
    match kind {
        ViewportSemanticTokenKind::Ident => tokens.text.secondary,
        ViewportSemanticTokenKind::Keyword => canvas.keyword,
        ViewportSemanticTokenKind::Type => tokens.accent.cyan,
        ViewportSemanticTokenKind::String => tokens.accent.green,
        ViewportSemanticTokenKind::Number => tokens.accent.orange,
        ViewportSemanticTokenKind::Comment => canvas.comment,
        ViewportSemanticTokenKind::Punct => tokens.text.muted,
        ViewportSemanticTokenKind::Function => tokens.accent.blue,
        ViewportSemanticTokenKind::Attribute => tokens.accent.purple,
        ViewportSemanticTokenKind::Error => tokens.accent.red,
    }
}

impl CodeCanvasPainter for EguiCodeCanvasPainter {
    fn paint_lines(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &ShellProjectionSnapshot,
        model: &DesktopProjectionViewModel,
        actions: &mut Vec<DesktopAction>,
    ) {
        render_code_lines(ui, snapshot, model, actions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_object_safe(_: &dyn CodeCanvasPainter) {}

    #[test]
    fn code_canvas_painter_trait_accepts_dyn_dispatch() {
        let painter = EguiCodeCanvasPainter;
        assert_object_safe(&painter);
    }

    #[test]
    fn prototype_code_canvas_tokens_use_slate_amber_editor_roles() {
        let tokens = prototype_code_canvas_tokens();

        assert_eq!(tokens.background, egui::Color32::from_rgb(0x12, 0x1a, 0x23));
        assert_eq!(
            tokens.current_line,
            egui::Color32::from_rgba_unmultiplied(0xcf, 0x81, 0x36, 28)
        );
        assert_eq!(
            tokens.selection,
            egui::Color32::from_rgba_unmultiplied(0x2e, 0x7f, 0xb8, 72)
        );
        assert_eq!(tokens.cursor, egui::Color32::from_rgb(0xcf, 0x81, 0x36));
        assert_eq!(
            tokens.line_number,
            egui::Color32::from_rgb(0x7e, 0x8a, 0x9b)
        );
        assert_eq!(tokens.keyword, egui::Color32::from_rgb(0xcf, 0x81, 0x36));
        assert_eq!(tokens.comment, egui::Color32::from_rgb(0x7e, 0x8a, 0x9b));
    }
}
