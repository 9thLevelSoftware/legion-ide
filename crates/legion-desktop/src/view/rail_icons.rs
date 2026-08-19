//! Vector icons for the activity rail.
//!
//! The rail used single characters — `▤ ⌕ ⑂ ✓ ▷` — and egui's bundled font set
//! does not cover most of them. Five of the nine rail buttons rendered as `□`,
//! the "missing glyph" box, so the primary navigation column of the IDE looked
//! like a stack of empty squares. Which glyphs survive depends on the bundled
//! font set and on the host CJK fallback this crate may load, so the same code
//! produces different holes on different machines.
//!
//! Drawing the icons with the painter removes the font from the question
//! entirely: a line is a line on every platform. These are deliberately plain
//! 1.5px strokes on a 16px grid — legible at rail size, and cheap enough that a
//! redraw costs nothing.
//!
//! `Symbols` keeps its `ƒ` and `Settings` keeps its `⚙` because those did
//! render and read better as characters than anything worth drawing here.

use egui::{Color32, Pos2, Rect, Shape, Stroke, Vec2, pos2};

/// Side of the square the icon art is authored on.
///
/// Every icon shares this grid so they carry the same optical weight and line
/// up down the rail without each one re-deriving geometry from its slot.
const GRID: f32 = 16.0;

/// One rail icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailIcon {
    /// A document outline: the file explorer.
    Explorer,
    /// A magnifier: search.
    Search,
    /// A commit graph: source control.
    SourceControl,
    /// A check mark: tests.
    Tests,
    /// A play triangle: run and debug.
    Debug,
    /// Stacked rules: diagnostics.
    Diagnostics,
}

/// The shapes for `icon`, centred in `slot`.
///
/// Split out from painting so the geometry is testable without a render pass —
/// a stray coordinate paints over a neighbouring button, and at 1.5px that
/// reads as a rendering artefact rather than a bug anyone would report.
#[must_use]
pub fn shapes(icon: RailIcon, slot: Rect, color: Color32) -> Vec<Shape> {
    let origin = slot.center() - Vec2::splat(GRID / 2.0);
    let at = |x: f32, y: f32| -> Pos2 { pos2(origin.x + x, origin.y + y) };
    let stroke = Stroke::new(1.5_f32, color);
    let line = |a: Pos2, b: Pos2| Shape::line_segment([a, b], stroke);

    match icon {
        RailIcon::Explorer => vec![
            Shape::rect_stroke(
                Rect::from_min_max(at(3.0, 2.0), at(13.0, 14.0)),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            ),
            line(at(5.5, 6.0), at(10.5, 6.0)),
            line(at(5.5, 9.0), at(10.5, 9.0)),
        ],
        RailIcon::Search => vec![
            Shape::circle_stroke(at(6.5, 6.5), 4.0, stroke),
            line(at(9.5, 9.5), at(13.0, 13.0)),
        ],
        RailIcon::SourceControl => vec![
            Shape::circle_stroke(at(4.5, 3.5), 2.0, stroke),
            Shape::circle_stroke(at(4.5, 12.5), 2.0, stroke),
            Shape::circle_stroke(at(11.5, 3.5), 2.0, stroke),
            line(at(4.5, 5.5), at(4.5, 10.5)),
            line(at(9.5, 3.5), at(6.5, 3.5)),
        ],
        RailIcon::Tests => vec![
            line(at(3.0, 8.5), at(6.5, 12.0)),
            line(at(6.5, 12.0), at(13.0, 4.0)),
        ],
        RailIcon::Debug => vec![Shape::convex_polygon(
            vec![at(4.5, 2.5), at(13.0, 8.0), at(4.5, 13.5)],
            color,
            Stroke::NONE,
        )],
        RailIcon::Diagnostics => [3.5_f32, 8.0, 12.5]
            .into_iter()
            .map(|y| line(at(3.0, y), at(13.0, y)))
            .collect(),
    }
}

/// Paint `icon` centred in `slot`.
pub fn paint(painter: &egui::Painter, icon: RailIcon, slot: Rect, color: Color32) {
    painter.extend(shapes(icon, slot, color));
}

/// Every rail icon, in rail order.
#[must_use]
pub fn all() -> [RailIcon; 6] {
    [
        RailIcon::Explorer,
        RailIcon::Search,
        RailIcon::SourceControl,
        RailIcon::Tests,
        RailIcon::Debug,
        RailIcon::Diagnostics,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The union of an icon's shapes, or `None` if it drew nothing.
    fn bounds(icon: RailIcon, slot: Rect) -> Option<Rect> {
        shapes(icon, slot, Color32::WHITE)
            .into_iter()
            .map(|shape| shape.visual_bounding_rect())
            .reduce(|acc, rect| acc.union(rect))
    }

    #[test]
    fn every_icon_stays_within_its_slot() {
        let slot = Rect::from_min_size(pos2(100.0, 50.0), egui::vec2(38.0, 28.0));
        // The 16px art box plus the stroke's half-width spilling outward at the
        // extremes, which is what `visual_bounding_rect` accounts for.
        let allowed = Rect::from_center_size(slot.center(), egui::vec2(GRID + 3.0, GRID + 3.0));

        for icon in all() {
            let bounds = bounds(icon, slot).unwrap_or_else(|| panic!("{icon:?} painted nothing"));
            assert!(
                allowed.contains_rect(bounds),
                "{icon:?} painted outside its slot: {bounds:?} is not within {allowed:?}"
            );
        }
    }

    #[test]
    fn every_icon_actually_draws_something() {
        // A rail icon that silently produces no shapes is the same failure the
        // tofu boxes were — an empty button — just quieter.
        let slot = Rect::from_min_size(Pos2::ZERO, egui::vec2(38.0, 28.0));
        for icon in all() {
            let bounds = bounds(icon, slot).unwrap_or_else(|| panic!("{icon:?} painted nothing"));
            assert!(
                bounds.width() > 4.0 && bounds.height() > 4.0,
                "{icon:?} is too small to read: {bounds:?}"
            );
        }
    }

    #[test]
    fn icons_follow_their_slot() {
        // Centring is derived from the slot, so moving the slot must move the
        // art with it; a hard-coded origin would pin every icon to one corner
        // of the window.
        let first = bounds(
            RailIcon::Search,
            Rect::from_min_size(Pos2::ZERO, egui::vec2(38.0, 28.0)),
        );
        let second = bounds(
            RailIcon::Search,
            Rect::from_min_size(pos2(0.0, 200.0), egui::vec2(38.0, 28.0)),
        );
        let (first, second) = (first.expect("first"), second.expect("second"));
        assert!(
            (second.center().y - first.center().y - 200.0).abs() < 0.01,
            "icon did not follow its slot: {first:?} then {second:?}"
        );
    }
}
