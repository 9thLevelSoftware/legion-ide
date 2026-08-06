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

pub(super) fn semantic_token_color(kind: ViewportSemanticTokenKind) -> egui::Color32 {
    let canvas = theme::tokens().code_canvas;
    match kind {
        ViewportSemanticTokenKind::Ident => canvas.ident,
        ViewportSemanticTokenKind::Keyword => canvas.keyword,
        ViewportSemanticTokenKind::Type => canvas.type_name,
        ViewportSemanticTokenKind::String => canvas.string,
        ViewportSemanticTokenKind::Number => canvas.number,
        ViewportSemanticTokenKind::Comment => canvas.comment,
        ViewportSemanticTokenKind::Punct => canvas.punct,
        ViewportSemanticTokenKind::Function => canvas.function,
        ViewportSemanticTokenKind::Attribute => canvas.attribute,
        ViewportSemanticTokenKind::Error => canvas.error,
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
    fn theme_code_canvas_tokens_use_slate_amber_editor_roles() {
        let tokens = theme::Theme::dark().code_canvas;

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
