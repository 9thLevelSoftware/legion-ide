//! Renderer timing metrics for the desktop adapter.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Maximum retained samples per timing series. Older samples are evicted once
/// this cap is reached so a long-running session cannot grow the sample vectors
/// (or the work `summary()` does over them) without bound.
const MAX_RETAINED_SAMPLES: usize = 4096;

/// Metadata-only input-to-paint sample.
#[derive(Debug, Clone, PartialEq)]
pub struct InputPaintSample {
    /// Input timestamp in milliseconds since recorder creation.
    pub input_at_ms: f64,
    /// Paint timestamp in milliseconds since recorder creation.
    pub paint_at_ms: f64,
    /// Input-to-paint duration in milliseconds.
    pub duration_ms: f64,
}

/// Summary of bounded frame timing samples.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameTimingSummary {
    /// Number of input-to-paint samples.
    pub sample_count: usize,
    /// 50th percentile input-to-paint in milliseconds.
    pub p50_input_to_paint_ms: f64,
    /// 95th percentile input-to-paint in milliseconds.
    pub p95_input_to_paint_ms: f64,
    /// Number of frame-duration samples.
    pub frame_count: usize,
    /// Average frame duration in milliseconds.
    pub average_frame_ms: f64,
    /// Population variance of frame durations in milliseconds squared.
    pub frame_variance_ms2: f64,
}

impl Default for FrameTimingSummary {
    fn default() -> Self {
        Self {
            sample_count: 0,
            p50_input_to_paint_ms: 0.0,
            p95_input_to_paint_ms: 0.0,
            frame_count: 0,
            average_frame_ms: 0.0,
            frame_variance_ms2: 0.0,
        }
    }
}

/// Metadata-only frame timing recorder.
#[derive(Debug)]
pub struct FrameTimingRecorder {
    origin: Instant,
    pending_input: Option<Instant>,
    // Bounded sliding windows. `VecDeque` gives O(1) front eviction once the
    // retention cap is reached (a `Vec` would shift every element on each evict).
    input_paint_samples: VecDeque<InputPaintSample>,
    frame_durations_ms: VecDeque<f64>,
}

impl Default for FrameTimingRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameTimingRecorder {
    /// Creates an empty timing recorder.
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            pending_input: None,
            input_paint_samples: VecDeque::new(),
            frame_durations_ms: VecDeque::new(),
        }
    }

    /// Records an input event timestamp without storing the input payload.
    pub fn record_input(&mut self, at: Instant) {
        self.pending_input = Some(at);
    }

    /// Records an input event at the current instant.
    pub fn record_input_now(&mut self) {
        self.record_input(Instant::now());
    }

    /// Records a paint timestamp and closes the pending input-to-paint sample, if any.
    pub fn record_paint(&mut self, at: Instant) {
        let Some(input_at) = self.pending_input.take() else {
            return;
        };
        let duration = at.saturating_duration_since(input_at);
        if self.input_paint_samples.len() >= MAX_RETAINED_SAMPLES {
            self.input_paint_samples.pop_front();
        }
        self.input_paint_samples.push_back(InputPaintSample {
            input_at_ms: millis(input_at.saturating_duration_since(self.origin)),
            paint_at_ms: millis(at.saturating_duration_since(self.origin)),
            duration_ms: millis(duration),
        });
    }

    /// Records a paint event at the current instant.
    pub fn record_paint_now(&mut self) {
        self.record_paint(Instant::now());
    }

    /// Records one frame duration.
    pub fn record_frame_duration(&mut self, duration: Duration) {
        if self.frame_durations_ms.len() >= MAX_RETAINED_SAMPLES {
            self.frame_durations_ms.pop_front();
        }
        self.frame_durations_ms.push_back(millis(duration));
    }

    /// Returns recorded input-to-paint samples in insertion order (oldest first).
    pub fn input_paint_samples(&self) -> impl ExactSizeIterator<Item = &InputPaintSample> + '_ {
        self.input_paint_samples.iter()
    }

    /// Returns a metadata-only timing summary.
    pub fn summary(&self) -> FrameTimingSummary {
        let mut input_durations = self
            .input_paint_samples
            .iter()
            .map(|sample| sample.duration_ms)
            .collect::<Vec<_>>();
        input_durations.sort_by(f64::total_cmp);

        let average_frame_ms = average(&self.frame_durations_ms);
        FrameTimingSummary {
            sample_count: input_durations.len(),
            p50_input_to_paint_ms: percentile_nearest_rank(&input_durations, 50.0),
            p95_input_to_paint_ms: percentile_nearest_rank(&input_durations, 95.0),
            frame_count: self.frame_durations_ms.len(),
            average_frame_ms,
            frame_variance_ms2: population_variance(&self.frame_durations_ms, average_frame_ms),
        }
    }
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn average(values: &VecDeque<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn population_variance(values: &VecDeque<f64>, average: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values
        .iter()
        .map(|value| {
            let delta = value - average;
            delta * delta
        })
        .sum::<f64>()
        / values.len() as f64
}

fn percentile_nearest_rank(sorted_values: &[f64], percentile: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    let rank = ((percentile / 100.0) * sorted_values.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted_values.len() - 1);
    sorted_values[index]
}
