//! Ghost text overlay view model and lifecycle helpers (PKT-RAIL T1).

use legion_protocol::{
    InlinePredictionFreshnessState, InlinePredictionRequestId, InlinePredictionResult,
    TextCoordinate,
};

/// Lifecycle state of a ghost text prediction overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhostTextState {
    /// Prediction request is in-flight; overlay is not yet visible.
    Pending,
    /// Overlay is visible in the editor.
    Displaying,
    /// User accepted the prediction; text was applied through the proposal pipeline.
    Accepted,
    /// User dismissed the overlay without applying.
    Dismissed,
    /// In-flight prediction was cancelled before completing.
    Cancelled,
}

/// View model representing a ghost text prediction overlay.
///
/// Ghost text acceptance must go through the proposal pipeline — the overlay
/// never causes a direct buffer mutation on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhostTextOverlayViewModel {
    /// The prediction text to render as a translucent overlay.
    pub text: String,
    /// Position in the buffer where the text would be inserted.
    pub insert_position: TextCoordinate,
    /// Provider that generated this prediction.
    pub provider_id: String,
    /// Request identifier used for lifecycle tracking.
    pub request_id: InlinePredictionRequestId,
    /// Current lifecycle state of this overlay.
    pub state: GhostTextState,
    /// True when the buffer has changed since the prediction was generated,
    /// rendering the ghost text potentially incorrect.
    pub stale: bool,
}

impl GhostTextOverlayViewModel {
    /// Returns a copy of this overlay with state set to [`GhostTextState::Dismissed`].
    #[must_use]
    pub fn dismiss(self) -> Self {
        Self {
            state: GhostTextState::Dismissed,
            ..self
        }
    }

    /// Returns a copy of this overlay with state set to [`GhostTextState::Accepted`].
    #[must_use]
    pub fn accept(self) -> Self {
        Self {
            state: GhostTextState::Accepted,
            ..self
        }
    }

    /// Returns a copy of this overlay with state set to [`GhostTextState::Cancelled`].
    #[must_use]
    pub fn cancel(self) -> Self {
        Self {
            state: GhostTextState::Cancelled,
            ..self
        }
    }
}

/// Constructs a ghost text overlay view model from an inline prediction result.
///
/// Returns `None` when:
/// - the prediction has no ghost text body, or
/// - the prediction's freshness state is not [`InlinePredictionFreshnessState::Fresh`]
///   (i.e., the buffer changed after the prediction was generated).
#[must_use]
pub fn ghost_text_from_prediction(
    result: &InlinePredictionResult,
    provider_id: &str,
) -> Option<GhostTextOverlayViewModel> {
    if result.freshness.state != InlinePredictionFreshnessState::Fresh {
        return None;
    }
    let ghost_text = result.ghost_text.as_ref()?;
    Some(GhostTextOverlayViewModel {
        text: ghost_text.text.clone(),
        insert_position: result.insert_range.start,
        provider_id: provider_id.to_string(),
        request_id: result.request_id.clone(),
        state: GhostTextState::Displaying,
        stale: false,
    })
}
