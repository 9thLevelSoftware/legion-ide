//! The Legion mark in the top bar, drawn rather than typed.
//!
//! It was `◆` (U+25C6 BLACK DIAMOND) in a label. Which is the same mistake the
//! activity rail made, and it produced the same result: the shell snapshots
//! taken on all three platforms show a small filled dot on Windows, an amber
//! diamond on macOS, and `□` — the missing-glyph box — on Linux. The product's
//! own brand mark was a broken character on one of its three targets, and no
//! test could see it until there were pictures to compare.
//!
//! See `rail_icons` for the same reasoning at more length. A line is a line on
//! every platform; a codepoint is a negotiation with whatever fonts the host
//! happens to have.

use egui::{Color32, Pos2, Rect, Shape, pos2};

/// Width and height of the mark, in points.
///
/// Sized against the wordmark beside it rather than the 16px rail grid: this
/// sits inline with text, and a 16px diamond next to a 14px word reads as an
/// error.
pub const SIZE: f32 = 10.0;

/// The mark's shape, centred in `slot`.
///
/// Split from painting so the geometry is testable without a render pass —
/// same split as `rail_icons::shapes`.
pub fn shape(slot: Rect, color: Color32) -> Shape {
    let centre = slot.center();
    let reach = SIZE / 2.0;
    // `convex_polygon` owns its points, so this allocation is the API's, not a
    // choice. One shape, one `Vec`, once per frame.
    let points: Vec<Pos2> = vec![
        pos2(centre.x, centre.y - reach),
        pos2(centre.x + reach, centre.y),
        pos2(centre.x, centre.y + reach),
        pos2(centre.x - reach, centre.y),
    ];
    Shape::convex_polygon(points, color, egui::Stroke::NONE)
}

/// Allocate space for the mark and paint it.
pub fn show(ui: &mut egui::Ui, color: Color32) -> egui::Response {
    // `hover` rather than `click`: this is decoration. Giving it a click sense
    // would register an interactive target that does nothing, and an
    // interactive target that does nothing is the defect the accessibility
    // suite and the intent-reachability gate both exist to catch.
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(SIZE), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().add(shape(rect, color));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot() -> Rect {
        Rect::from_min_size(pos2(100.0, 40.0), egui::Vec2::splat(SIZE))
    }

    #[test]
    fn the_mark_is_a_diamond_centred_in_its_slot() {
        let drawn = shape(slot(), Color32::from_rgb(208, 130, 75));
        let Shape::Path(path) = &drawn else {
            panic!("expected a path shape, got {drawn:?}");
        };
        assert_eq!(path.points.len(), 4, "a diamond has four corners");

        let centre = slot().center();
        for point in &path.points {
            let distance = (point.x - centre.x).abs() + (point.y - centre.y).abs();
            assert!(
                (distance - SIZE / 2.0).abs() < 0.001,
                "every corner sits SIZE/2 from the centre along an axis; {point:?} did not"
            );
        }
    }

    #[test]
    fn the_mark_stays_inside_its_slot() {
        // A mark that overdraws its allocation would collide with the wordmark
        // beside it, and the layout would not report the overlap.
        let slot = slot();
        let drawn = shape(slot, Color32::WHITE);
        let Shape::Path(path) = &drawn else {
            panic!("expected a path shape");
        };
        for point in &path.points {
            assert!(slot.contains(*point), "{point:?} escaped the slot {slot:?}");
        }
    }
}
