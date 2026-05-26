//! Renderer timing metrics for the desktop adapter.

/// Metadata-only frame timing recorder placeholder.
#[derive(Debug, Default)]
pub struct FrameTimingRecorder {
    frame_count: u64,
}

impl FrameTimingRecorder {
    /// Creates an empty timing recorder.
    pub fn new() -> Self {
        Self { frame_count: 0 }
    }

    /// Records one rendered frame without storing input payloads.
    pub fn record_frame(&mut self) {
        self.frame_count += 1;
    }

    /// Returns the number of recorded frames.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}
