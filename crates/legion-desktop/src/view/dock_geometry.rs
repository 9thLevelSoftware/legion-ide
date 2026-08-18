//! Splitter fractions: applying persisted dock sizes, and observing new ones.
//!
//! `DockSideLayout::splitter_fraction` has been persisted through the session
//! record for some time, and reloaded on open, and read by nothing — the panels
//! sized themselves from `ShellGeometry` constants, so a restart restored the
//! *record* rather than the layout the user had arranged. This module is the
//! missing half: it turns a stored fraction into a panel size on the way in,
//! and a rendered panel size back into a fraction on the way out.
//!
//! Both directions use the same denominator — the shell's inner rect, captured
//! once before any panel is added. Measuring against `ui.available_*` after a
//! panel has already been placed would give the *remaining* space, so a fraction
//! written on one frame would not mean the same thing when read on the next, and
//! the panels would creep on every restart.

use legion_ui::{DockLayout, DockMode, DockSide};

/// Panel sizes observed during one frame, as fractions of the shell's inner
/// rect. `None` means the panel was not rendered this frame — compact layouts
/// drop the side docks, and the inspector is hidden in Manual mode — which must
/// not be mistaken for "the user dragged it to zero".
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DockFractions {
    /// Left dock width over shell width.
    pub left: Option<f32>,
    /// Right dock width over shell width.
    pub right: Option<f32>,
    /// Bottom dock height over shell height.
    pub bottom: Option<f32>,
}

/// Smallest fraction change worth a durable write.
///
/// Panel rects arrive as floats that wobble by sub-pixel amounts between
/// frames. Without a threshold every frame would look like a resize, and since
/// a layout change now triggers a session write, the app would fsync forever
/// while sitting idle.
pub const MATERIAL_FRACTION_DELTA: f32 = 0.005;

/// The stored fraction for one side of the layout belonging to `mode`.
#[must_use]
pub fn stored_fraction(layouts: &[DockLayout], mode: DockMode, side: DockSide) -> Option<f32> {
    let layout = layouts.iter().find(|layout| layout.mode == mode)?;
    let side_layout = match side {
        DockSide::Left => &layout.left,
        DockSide::Right => &layout.right,
        DockSide::Bottom => &layout.bottom,
    };
    Some(side_layout.splitter_fraction)
}

/// The size a panel should default to, given an optional stored fraction.
///
/// Falls back to `fallback` when nothing was stored, and always clamps into the
/// panel's own bounds so a persisted fraction from a much larger window cannot
/// squeeze the editor out of existence on a smaller one.
#[must_use]
pub fn size_from_fraction(
    fraction: Option<f32>,
    basis: f32,
    fallback: f32,
    min: f32,
    max: f32,
) -> f32 {
    // A degenerate basis means the shell has no room to divide yet; the caller's
    // fallback is the only meaningful answer.
    if !basis.is_finite() || basis <= 0.0 {
        return fallback;
    }
    let size = fraction
        .filter(|fraction| fraction.is_finite())
        .map_or(fallback, |fraction| fraction * basis);
    // `max` can fall below `min` on very narrow windows, where the editor
    // reserve eats the whole shell. Clamping low-first keeps the result
    // predictable instead of panicking on an inverted range.
    size.clamp(min.min(max), max.max(min))
}

/// The fraction a rendered panel occupied, or `None` if it cannot be expressed.
#[must_use]
pub fn observed_fraction(panel_size: f32, basis: f32) -> Option<f32> {
    if !basis.is_finite() || basis <= 0.0 || !panel_size.is_finite() || panel_size <= 0.0 {
        return None;
    }
    Some((panel_size / basis).clamp(0.15, 0.85))
}

/// Panel sizes measured on one frame, in pixels, against the shell they were
/// measured in. `None` for a side means it was not rendered that frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockMeasurement {
    /// Shell inner width the side docks were measured against.
    pub basis_width: f32,
    /// Shell inner height the bottom dock was measured against.
    pub basis_height: f32,
    /// Rendered left dock width.
    pub left: Option<f32>,
    /// Rendered right dock width.
    pub right: Option<f32>,
    /// Rendered bottom dock height.
    pub bottom: Option<f32>,
}

/// Pixels of panel movement that count as a deliberate drag.
const DRAG_PIXELS: f32 = 1.0;

/// Fractions worth persisting, given this frame and the one before it.
///
/// Only a *change between consecutive frames* is treated as the user dragging a
/// splitter. Two simpler rules were tried first and are both wrong:
///
/// * "report the rendered size" feeds the geometry default straight back over
///   whatever was restored, on the very first frame;
/// * "report it when it differs from what we requested" fails too, because egui
///   honours `default_size` only until it has a remembered size of its own —
///   after that the panel legitimately sits at a size we did not ask for, and
///   every frame reads as a resize.
///
/// A drag is a transition, so a transition is what this looks for. Frames where
/// the shell itself changed size are skipped entirely: a window resize moves
/// every panel and changes every fraction, and treating that as intent is how
/// briefly shrinking the window would destroy the arrangement.
#[must_use]
pub fn dragged_fractions(
    current: DockMeasurement,
    previous: Option<DockMeasurement>,
) -> DockFractions {
    let Some(previous) = previous else {
        return DockFractions::default();
    };
    let basis_stable = (current.basis_width - previous.basis_width).abs() <= DRAG_PIXELS
        && (current.basis_height - previous.basis_height).abs() <= DRAG_PIXELS;
    if !basis_stable {
        return DockFractions::default();
    }
    let dragged = |now: Option<f32>, before: Option<f32>, basis: f32| -> Option<f32> {
        let (now, before) = (now?, before?);
        ((now - before).abs() > DRAG_PIXELS)
            .then(|| observed_fraction(now, basis))
            .flatten()
    };
    DockFractions {
        left: dragged(current.left, previous.left, current.basis_width),
        right: dragged(current.right, previous.right, current.basis_width),
        bottom: dragged(current.bottom, previous.bottom, current.basis_height),
    }
}

/// Whether an observed fraction differs from the stored one enough to persist.
#[must_use]
pub fn differs_materially(observed: f32, stored: f32) -> bool {
    (observed - stored).abs() > MATERIAL_FRACTION_DELTA
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_fraction_becomes_a_proportional_size() {
        assert_eq!(
            size_from_fraction(Some(0.25), 1_600.0, 248.0, 100.0, 900.0),
            400.0
        );
    }

    #[test]
    fn a_missing_fraction_falls_back_to_the_shell_default() {
        assert_eq!(
            size_from_fraction(None, 1_600.0, 248.0, 100.0, 900.0),
            248.0
        );
    }

    #[test]
    fn a_fraction_from_a_larger_window_cannot_squeeze_out_the_editor() {
        // 0.8 of a small shell would leave nothing for the code canvas; the
        // panel's own maximum wins.
        assert_eq!(
            size_from_fraction(Some(0.8), 900.0, 248.0, 120.0, 400.0),
            400.0
        );
    }

    #[test]
    fn an_inverted_range_clamps_instead_of_panicking() {
        // `max` below `min` happens on very narrow windows. `f32::clamp` panics
        // when min > max, and a panic in a render pass takes the app down.
        let size = size_from_fraction(Some(0.5), 800.0, 200.0, 300.0, 120.0);
        assert!(size.is_finite(), "got {size}");
    }

    #[test]
    fn a_degenerate_basis_yields_the_fallback() {
        assert_eq!(
            size_from_fraction(Some(0.5), 0.0, 248.0, 100.0, 900.0),
            248.0
        );
        assert_eq!(
            size_from_fraction(Some(0.5), f32::NAN, 248.0, 100.0, 900.0),
            248.0
        );
    }

    #[test]
    fn observing_a_panel_round_trips_its_fraction() {
        let fraction = observed_fraction(400.0, 1_600.0).expect("expressible");
        assert!((fraction - 0.25).abs() < f32::EPSILON, "got {fraction}");
        assert_eq!(
            size_from_fraction(Some(fraction), 1_600.0, 0.0, 0.0, 1_600.0),
            400.0
        );
    }

    #[test]
    fn an_unrendered_panel_is_not_observed_as_zero() {
        // The distinction that matters: a hidden panel must not be recorded as
        // a user-chosen zero width, or Manual mode would persist a collapsed
        // inspector over whatever Delegate mode had arranged.
        assert_eq!(observed_fraction(0.0, 1_600.0), None);
        assert_eq!(observed_fraction(400.0, 0.0), None);
    }

    #[test]
    fn observed_fractions_stay_inside_the_layout_bounds() {
        assert_eq!(observed_fraction(1_590.0, 1_600.0), Some(0.85));
        assert_eq!(observed_fraction(1.0, 1_600.0), Some(0.15));
    }

    #[test]
    fn sub_pixel_wobble_is_not_a_resize() {
        assert!(!differs_materially(0.2500, 0.2501));
        assert!(differs_materially(0.25, 0.30));
    }

    #[test]
    fn a_side_reads_the_layout_for_its_own_mode() {
        let layouts = DockLayout::standard_all_modes();
        let manual_left = stored_fraction(&layouts, DockMode::Manual, DockSide::Left);
        assert!(manual_left.is_some());
        // Every mode carries its own arrangement; reading the wrong one would
        // silently apply Delegate's panel sizes to Manual.
        for mode in [
            DockMode::Manual,
            DockMode::Assist,
            DockMode::Delegate,
            DockMode::Automate,
        ] {
            assert!(
                stored_fraction(&layouts, mode, DockSide::Bottom).is_some(),
                "{mode:?} should carry a bottom fraction"
            );
        }
    }

    #[test]
    fn an_absent_mode_has_no_stored_fraction() {
        assert_eq!(stored_fraction(&[], DockMode::Manual, DockSide::Left), None);
    }
}

#[cfg(test)]
mod drag_tests {
    use super::*;

    fn frame(left: f32, right: f32, bottom: f32) -> DockMeasurement {
        DockMeasurement {
            basis_width: 1_600.0,
            basis_height: 900.0,
            left: Some(left),
            right: Some(right),
            bottom: Some(bottom),
        }
    }

    #[test]
    fn the_first_frame_reports_nothing() {
        // There is no previous frame to have moved from, and the sizes on frame
        // one are whatever was restored — feeding them back would overwrite the
        // restored layout with itself, or worse, with a clamped version of it.
        assert_eq!(
            dragged_fractions(frame(400.0, 300.0, 200.0), None),
            DockFractions::default()
        );
    }

    #[test]
    fn a_steady_layout_reports_nothing() {
        let steady = frame(400.0, 300.0, 200.0);
        assert_eq!(
            dragged_fractions(steady, Some(steady)),
            DockFractions::default()
        );
    }

    #[test]
    fn a_panel_that_egui_remembers_at_its_own_size_is_not_a_drag() {
        // The regression that broke `product_mode_changes_preserve_...`: egui
        // honours `default_size` only until it has a size of its own, after
        // which the panel legitimately sits somewhere we did not request. That
        // is steady state, not intent.
        let remembered = frame(248.0, 300.0, 200.0);
        assert_eq!(
            dragged_fractions(remembered, Some(remembered)),
            DockFractions::default()
        );
    }

    #[test]
    fn dragging_one_splitter_reports_only_that_side() {
        let moved = dragged_fractions(frame(560.0, 300.0, 200.0), Some(frame(400.0, 300.0, 200.0)));
        let left = moved.left.expect("the dragged side is reported");
        assert!((left - 0.35).abs() < 0.001, "got {left}");
        assert_eq!(moved.right, None, "an untouched side reports nothing");
        assert_eq!(moved.bottom, None, "an untouched side reports nothing");
    }

    #[test]
    fn resizing_the_window_is_not_a_drag() {
        // Every panel moves and every fraction changes when the shell resizes.
        // Recording that as intent is how making the window briefly narrow
        // would rewrite the stored arrangement.
        let before = frame(400.0, 300.0, 200.0);
        let after = DockMeasurement {
            basis_width: 1_100.0,
            basis_height: 700.0,
            left: Some(320.0),
            right: Some(240.0),
            bottom: Some(160.0),
        };
        assert_eq!(
            dragged_fractions(after, Some(before)),
            DockFractions::default()
        );
    }

    #[test]
    fn sub_pixel_drift_is_not_a_drag() {
        assert_eq!(
            dragged_fractions(frame(400.4, 300.0, 200.0), Some(frame(400.0, 300.0, 200.0))),
            DockFractions::default()
        );
    }

    #[test]
    fn a_side_that_appears_or_vanishes_is_not_a_drag() {
        // Switching out of Manual reveals the inspector; that is a mode change,
        // not the user sizing it.
        let hidden = DockMeasurement {
            right: None,
            ..frame(400.0, 300.0, 200.0)
        };
        let shown = frame(400.0, 300.0, 200.0);
        assert_eq!(dragged_fractions(shown, Some(hidden)).right, None);
        assert_eq!(dragged_fractions(hidden, Some(shown)).right, None);
    }
}
