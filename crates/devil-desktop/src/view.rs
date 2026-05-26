//! Projection rendering for the desktop adapter.

/// Renderer-owned projection view placeholder.
#[derive(Debug, Default)]
pub struct ProjectionView;

impl ProjectionView {
    /// Creates a projection view with no product-state ownership.
    pub fn new() -> Self {
        Self
    }
}
