use legion_ui::ShellProjectionSnapshot;

use crate::bridge::DesktopAction;

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
}
