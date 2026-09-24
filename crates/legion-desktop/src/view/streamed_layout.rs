//! Bounded source-to-atlas layout for logical lines larger than the viewport fragment limit.
//!
//! This module owns renderer cache state only. Source bytes are supplied through
//! [`DesktopLineSource`], which is implemented by the app/editor authority.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    ops::Range,
};

use egui::epaint::text::{
    Glyph, MetricFontEngine, MetricGlyphBatch, MetricLayoutContinuation, MetricLayoutSummary,
    TextFormat, WrappedRowBatch, WrappedRowContinuation, WrappedRowDescriptor,
};
use egui::{Color32, Mesh, Pos2, Rect, TextureId, Ui, Vec2};
use legion_protocol::{BufferId, BufferVersion, SnapshotId};
use legion_protocol::{
    CaretAffinity, VisualNavigationPosition, VisualNavigationRow, VisualNavigationStop,
    VisualNavigationX,
};
use uuid::Uuid;

#[cfg(test)]
#[path = "streamed_layout_tests.rs"]
mod streamed_layout_tests;

pub const SOURCE_CHUNK_BYTES: usize = 96 * 1024;
pub const MAX_FRAME_SOURCE_BYTES: usize = 256 * 1024;
pub const MAX_FRAME_ROWS: usize = 64;
pub const MAX_FRAME_GLYPHS: usize = 4096;

#[derive(Debug, Clone)]
pub struct StreamedLayoutOptions {
    pub format: TextFormat,
    pub pixels_per_point: f32,
    pub wrap_width: f32,
    pub break_anywhere: bool,
    pub visible_rows: Range<usize>,
    pub visible_bytes: Range<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StreamedRequestedRow {
    First,
    Last,
    Index(usize),
    Byte(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct StreamedNavigationRequest {
    pub identity: DesktopSourceIdentity,
    pub row: StreamedRequestedRow,
}

impl StreamedLayoutCachePool {
    pub(crate) fn begin_frame(&mut self, active: &[DesktopSourceIdentity]) {
        self.drain_worker_results();
        self.active.clear();
        self.active.extend(active.iter().copied());
        self.active.extend(
            self.pending_navigation
                .iter()
                .map(|request| request.identity),
        );
        self.active_order.clear();
        self.active_order.extend(self.active.iter().copied());
        self.active_order.sort_by_key(|identity| {
            active
                .iter()
                .position(|candidate| candidate == identity)
                .unwrap_or(usize::MAX)
        });
        if !self.active_order.is_empty() {
            self.admission_cursor = self.admission_cursor.wrapping_add(1) % self.active_order.len();
        }
        self.frame_initial_budget = None;
        self.last_error = None;
        self.entries
            .retain(|identity, _| self.active.contains(identity));
        self.navigation_entries
            .retain(|identity, _| self.active.contains(identity));
        self.lru.retain(|identity| self.active.contains(identity));
        let stale_workers = self
            .worker_handles
            .keys()
            .filter(|(identity, _)| !self.active.contains(identity))
            .copied()
            .collect::<Vec<_>>();
        for key in stale_workers {
            if let Some(handle) = self.worker_handles.remove(&key) {
                handle.cancel();
            }
        }
    }

    fn drain_worker_results(&mut self) {
        let results = self
            .worker_scheduler
            .as_ref()
            .map(StreamedWorkerScheduler::drain_results)
            .unwrap_or_default();
        for result in results.into_iter().take(64) {
            self.apply_worker_result(result);
        }
    }

    fn apply_worker_result(&mut self, result: StreamedWorkerResult) {
        let StreamedWorkerResult {
            slot,
            generation,
            key,
            scan,
            eof,
            error,
        } = result;
        let Some((worker_key, _)) = self
            .worker_handles
            .iter()
            .find(|(_, handle)| handle.slot() == slot && handle.generation() == generation)
        else {
            return;
        };
        let worker_key = *worker_key;
        let terminal = eof || error.is_some();
        let accepted = {
            let entries = if worker_key.1 {
                &mut self.navigation_entries
            } else {
                &mut self.entries
            };
            let Some(cache) = entries.get_mut(&worker_key.0) else {
                return;
            };
            if !cache_key_matches(cache.key.as_ref(), &key) {
                false
            } else if cache.scan.visible_rows != scan.visible_rows
                || cache.scan.navigation_byte_target != scan.navigation_byte_target
            {
                cache.background_pending = true;
                false
            } else {
                cache.scan = scan;
                cache.background_pending = !terminal;
                true
            }
        };
        if !accepted {
            if let Some(handle) = self.worker_handles.remove(&worker_key) {
                handle.cancel();
            }
            return;
        }
        if let Some(error) = error {
            let error = error.chars().take(256).collect::<String>();
            self.last_error = Some(error.clone());
            if let Some(cache) = if worker_key.1 {
                self.navigation_entries.get_mut(&worker_key.0)
            } else {
                self.entries.get_mut(&worker_key.0)
            } {
                cache.background_error = Some(error);
            }
            if worker_key.1 {
                let failed_row = self
                    .navigation_entries
                    .get(&worker_key.0)
                    .and_then(|cache| cache.navigation_request);
                self.pending_navigation.retain(|request| {
                    !(request.identity == worker_key.0 && Some(request.row) == failed_row)
                });
            }
        }
        if terminal && let Some(handle) = self.worker_handles.remove(&worker_key) {
            handle.cancel();
        }
    }

    fn cancel_worker(&mut self, identity: DesktopSourceIdentity, navigation: bool) {
        if let Some(handle) = self.worker_handles.remove(&(identity, navigation)) {
            handle.cancel();
        }
    }

    fn prepare_background_job(
        &mut self,
        ui: &Ui,
        source: &dyn DesktopLineSource,
        owned_source: std::sync::Arc<dyn DesktopLineSource + Send + Sync>,
        line: usize,
        options: &StreamedLayoutOptions,
        navigation: bool,
    ) {
        let identity = source.identity(line);
        let key = CacheKey {
            identity,
            read_lease_id: source.read_lease_id(),
            format: options.format.clone(),
            pixels_per_point_bits: options.pixels_per_point.to_bits(),
            layout_identity_epoch: ui.fonts(|fonts| fonts.layout_identity_epoch()),
            wrap_width_bits: options.wrap_width.to_bits(),
            break_anywhere: options.break_anywhere,
        };
        let visible_rows = options.visible_rows.start
            ..options
                .visible_rows
                .end
                .min(options.visible_rows.start.saturating_add(MAX_FRAME_ROWS));
        let (key_changed, retain_last_tail, navigation_byte_target) = {
            let entries = if navigation {
                &mut self.navigation_entries
            } else {
                &mut self.entries
            };
            let cache = entries.entry(identity).or_default();
            let key_changed = cache
                .key
                .as_ref()
                .is_some_and(|current| !cache_key_matches(Some(current), &key));
            let window_changed = cache.scan.visible_rows.as_ref() != Some(&visible_rows);
            cache.ensure_key(key.clone());
            if !key_changed
                && !window_changed
                && !cache.background_pending
                && (cache.scan.phase == Some(ScanPhase::Ready) || cache.background_error.is_some())
            {
                return;
            }
            if window_changed {
                cache.force_window_reset = true;
            }
            if key_changed || window_changed {
                cache.background_error = None;
            }
            cache.scan.visible_rows = Some(visible_rows.clone());
            cache.background_pending = true;
            (
                key_changed || window_changed,
                cache.navigation_request == Some(StreamedRequestedRow::Last),
                cache.scan.navigation_byte_target,
            )
        };
        if key_changed {
            self.cancel_worker(identity, navigation);
        }
        let worker_key = (identity, navigation);
        if self.worker_handles.contains_key(&worker_key) {
            return;
        }
        let Some(generation) = self.worker_generation.checked_add(1) else {
            let entries = if navigation {
                &mut self.navigation_entries
            } else {
                &mut self.entries
            };
            if let Some(cache) = entries.get_mut(&identity) {
                cache.background_pending = false;
                cache.background_error = Some("worker generation exhausted".to_owned());
            }
            return;
        };
        self.worker_generation = generation;
        let job = StreamedWorkerJob {
            slot: 0,
            generation,
            key,
            source: owned_source,
            metric_snapshot: ui.fonts(|fonts| fonts.metric_snapshot()),
            visible_rows,
            retain_last_tail,
            navigation_byte_target,
        };
        let submitted = self
            .worker_scheduler
            .get_or_insert_with(StreamedWorkerScheduler::new)
            .submit(job);
        match submitted {
            Ok(handle) => {
                self.worker_handles.insert(worker_key, handle);
            }
            Err(
                StreamedWorkerSubmitError::AdmissionFull | StreamedWorkerSubmitError::MailboxFull,
            ) => {}
            Err(StreamedWorkerSubmitError::Stopped) => {
                let entries = if navigation {
                    &mut self.navigation_entries
                } else {
                    &mut self.entries
                };
                if let Some(cache) = entries.get_mut(&identity) {
                    cache.background_pending = false;
                    cache.background_error = Some("streamed layout worker stopped".to_owned());
                }
            }
        }
    }

    pub(crate) fn request_navigation(&mut self, request: StreamedNavigationRequest) {
        if self
            .pending_navigation
            .iter()
            .any(|pending| *pending == request)
        {
            return;
        }
        self.pending_navigation.push_back(request);
        while self.pending_navigation.len() > 64 {
            self.pending_navigation.pop_front();
        }
    }

    #[cfg(test)]
    pub(crate) fn inject_navigation_error_for_test(
        &mut self,
        identity: DesktopSourceIdentity,
        row: StreamedRequestedRow,
        error: &str,
    ) {
        let key = self
            .entries
            .get(&identity)
            .and_then(|cache| cache.key.clone());
        let cache = self.navigation_entries.entry(identity).or_default();
        cache.key = key;
        cache.navigation_request = Some(row);
        cache.background_error = Some(error.to_owned());
        self.pending_navigation
            .retain(|request| !(request.identity == identity && request.row == row));
    }

    pub(crate) fn streamed_navigation_requests(&self) -> Vec<StreamedNavigationRequest> {
        self.pending_navigation.iter().copied().collect()
    }

    pub(crate) fn service_one_navigation(
        &mut self,
        ui: &Ui,
        source: &dyn DesktopLineSource,
        options: StreamedLayoutOptions,
        budget: StreamedFrameBudget<'_>,
    ) {
        if self.pending_navigation.is_empty() {
            return;
        }
        self.request_cursor %= self.pending_navigation.len();
        let request = self.pending_navigation[self.request_cursor];
        self.request_cursor = self.request_cursor.wrapping_add(1);
        if source.identity(request.identity.line) != request.identity {
            self.navigation_entries.remove(&request.identity);
            self.pending_navigation
                .retain(|pending| pending.identity != request.identity);
            return;
        }
        let mut options = options;
        let reserve_visible = self.entries.contains_key(&request.identity);
        if matches!(request.row, StreamedRequestedRow::Last) {
            options.visible_rows = usize::MAX - MAX_FRAME_ROWS..usize::MAX;
            options.visible_bytes =
                request.identity.line_start_byte..request.identity.logical_end_byte;
        } else if let StreamedRequestedRow::Index(index) = request.row {
            options.visible_rows = index.saturating_sub(1)..index.saturating_add(2);
        } else if matches!(request.row, StreamedRequestedRow::Byte(_)) {
            options.visible_rows = 0..usize::MAX;
        }
        {
            let cache = self.navigation_entries.entry(request.identity).or_default();
            if let StreamedRequestedRow::Byte(byte) = request.row {
                let absolute_byte = request.identity.line_start_byte.saturating_add(byte);
                options.visible_bytes = absolute_byte.saturating_sub(MAX_FRAME_GLYPHS as u64)
                    ..absolute_byte.saturating_add(MAX_FRAME_GLYPHS as u64);
                cache.force_window_reset = false;
                cache.navigation_request = Some(request.row);
                cache.navigation_cache = None;
                cache.scan.navigation_target_index = None;
                cache.scan.navigation_predecessor = None;
                cache.scan.navigation_byte_target =
                    Some(request.identity.line_start_byte.saturating_add(byte));
            } else if cache.navigation_request != Some(request.row) {
                cache.force_window_reset = true;
                cache.navigation_cache = None;
                cache.scan.navigation_byte_target = None;
                cache.scan.navigation_target_index = None;
                cache.scan.navigation_predecessor = None;
                cache.navigation_request = Some(request.row);
            }
        }
        if let Some(owned_source) = source.owned_source() {
            self.prepare_background_job(
                ui,
                source,
                owned_source,
                request.identity.line,
                &options,
                true,
            );
        }
        let StreamedFrameBudget {
            source_bytes,
            rows,
            glyphs,
        } = budget;
        if *rows > 0 {
            let active_count = self.active_order.len().max(1);
            let initial = *self
                .frame_initial_budget
                .get_or_insert((*source_bytes, *rows, *glyphs));
            let slot = self
                .active_order
                .iter()
                .position(|candidate| *candidate == request.identity)
                .unwrap_or(0);
            let fair_quota = |total: usize| {
                let base = total / active_count;
                let remainder = total % active_count;
                base + usize::from(
                    (slot + active_count - self.admission_cursor) % active_count < remainder,
                )
            };
            // Navigation and visible painting can target the same identity in
            // one frame. Reserve half of this identity's fair share for the
            // visible cache so navigation cannot starve it when serviced
            // first; the remaining budget is still shared through the caller
            // references below.
            let navigation_share = |quota: usize| {
                if reserve_visible {
                    quota.div_ceil(2)
                } else {
                    quota
                }
            };
            let mut local_source = navigation_share(fair_quota(initial.0)).min(*source_bytes);
            let mut local_rows = navigation_share(fair_quota(initial.1)).min(*rows);
            let mut local_glyphs = navigation_share(fair_quota(initial.2)).min(*glyphs);
            let before_source = local_source;
            let before_rows = local_rows;
            let before_glyphs = local_glyphs;
            let result = self
                .navigation_entries
                .get_mut(&request.identity)
                .expect("navigation cache inserted")
                .paint_visible(
                    ui,
                    source,
                    request.identity.line,
                    options,
                    StreamedFrameBudget {
                        source_bytes: &mut local_source,
                        rows: &mut local_rows,
                        glyphs: &mut local_glyphs,
                    },
                );
            if let Err(error) = &result {
                self.last_error = Some(bounded_layout_error(error));
            }
            *source_bytes = (*source_bytes).saturating_sub(before_source - local_source);
            *rows = (*rows).saturating_sub(before_rows - local_rows);
            *glyphs = (*glyphs).saturating_sub(before_glyphs - local_glyphs);
        }
        if let Some(cache) = self.navigation_entries.get_mut(&request.identity) {
            let rows = cache.navigation_rows_uncached(request.identity);
            if let Some(rows) = rows {
                cache.navigation_cache = Some(rows);
            }
        }
        let ready = self
            .navigation_entries
            .get(&request.identity)
            .and_then(|cache| cache.navigation_rows(request.identity))
            .is_some_and(|rows| match request.row {
                StreamedRequestedRow::First => rows.rows.iter().any(|row| row.row_index == Some(0)),
                StreamedRequestedRow::Last => rows
                    .rows
                    .iter()
                    .any(|row| row.row_index == row.row_count.map(|count| count.saturating_sub(1))),
                StreamedRequestedRow::Index(index) => rows
                    .rows
                    .iter()
                    .any(|row| row.row_index == Some(index as u32)),
                StreamedRequestedRow::Byte(byte) => rows
                    .rows
                    .iter()
                    .any(|row| row.start.byte_column <= byte && byte <= row.end.byte_column),
            });
        if ready {
            self.pending_navigation
                .retain(|pending| *pending != request);
        }
    }

    pub(crate) fn navigation_rows(
        &self,
        identity: DesktopSourceIdentity,
    ) -> Option<StreamedNavigationRows> {
        let cache = self
            .navigation_entries
            .get(&identity)
            .or_else(|| self.entries.get(&identity))?;
        cache
            .navigation_cache
            .clone()
            .or_else(|| cache.navigation_rows_uncached(identity))
    }

    #[cfg(test)]
    pub(crate) fn debug_state(
        &self,
        identity: DesktopSourceIdentity,
    ) -> Option<StreamedLayoutDebugState> {
        let cache = self
            .navigation_entries
            .get(&identity)
            .or_else(|| self.entries.get(&identity))?;
        Some(StreamedLayoutDebugState {
            identity,
            phase: cache.scan.phase.map(|phase| match phase {
                ScanPhase::Metrics => 0,
                ScanPhase::Rows => 1,
                ScanPhase::Ready => 2,
            }),
            metric_next_byte: cache.scan.metric_next_byte,
            row_next_byte: cache.scan.row_next_byte,
            window_reset_count: cache.window_reset_count,
            key_reset_count: cache.key_reset_count,
            last_error: self.last_error.clone(),
        })
    }

    #[cfg(test)]
    pub(crate) fn visible_debug_state(
        &self,
        identity: DesktopSourceIdentity,
    ) -> Option<StreamedLayoutDebugState> {
        let cache = self.entries.get(&identity)?;
        Some(StreamedLayoutDebugState {
            identity,
            phase: cache.scan.phase.map(|phase| match phase {
                ScanPhase::Metrics => 0,
                ScanPhase::Rows => 1,
                ScanPhase::Ready => 2,
            }),
            metric_next_byte: cache.scan.metric_next_byte,
            row_next_byte: cache.scan.row_next_byte,
            window_reset_count: cache.window_reset_count,
            key_reset_count: cache.key_reset_count,
            last_error: self.last_error.clone(),
        })
    }

    pub(crate) fn paint_visible(
        &mut self,
        ui: &Ui,
        source: &dyn DesktopLineSource,
        line: usize,
        options: StreamedLayoutOptions,
        budget: StreamedFrameBudget<'_>,
    ) -> Result<StreamedPaintResult, StreamedLayoutError> {
        let StreamedFrameBudget {
            source_bytes,
            rows,
            glyphs,
        } = budget;
        let identity = source.identity(line);
        self.entries.entry(identity).or_default();
        if let Some(owned_source) = source.owned_source() {
            self.prepare_background_job(ui, source, owned_source, line, &options, false);
        }
        self.lru.retain(|key| *key != identity);
        self.lru.push_back(identity);
        let active_count = self.active_order.len().max(1);
        if *rows == 0 {
            return Ok(StreamedPaintResult {
                needs_repaint: true,
                ..Default::default()
            });
        }
        let initial = *self
            .frame_initial_budget
            .get_or_insert((*source_bytes, *rows, *glyphs));
        let slot = self
            .active_order
            .iter()
            .position(|candidate| *candidate == identity)
            .unwrap_or(0);
        let fair_quota = |total: usize| {
            let base = total / active_count;
            let remainder = total % active_count;
            base + usize::from(
                (slot + active_count - self.admission_cursor) % active_count < remainder,
            )
        };
        // A zero fair share is intentional when more identities are active
        // than the frame can service.  Admission rotation gives those
        // identities a nonzero share on later frames; manufacturing one here
        // would exceed the caller's frame budget and starve the prefix.
        let mut local_source = fair_quota(initial.0).min(*source_bytes);
        let mut local_rows = fair_quota(initial.1).min(*rows);
        let mut local_glyphs = fair_quota(initial.2).min(*glyphs);
        let before_source = local_source;
        let before_rows = local_rows;
        let before_glyphs = local_glyphs;
        let result = self
            .entries
            .get_mut(&identity)
            .expect("streamed cache inserted")
            .paint_visible(
                ui,
                source,
                line,
                options,
                StreamedFrameBudget {
                    source_bytes: &mut local_source,
                    rows: &mut local_rows,
                    glyphs: &mut local_glyphs,
                },
            );
        if let Err(error) = &result {
            self.last_error = Some(bounded_layout_error(error));
        }
        *source_bytes = (*source_bytes).saturating_sub(before_source - local_source);
        *rows = (*rows).saturating_sub(before_rows - local_rows);
        *glyphs = (*glyphs).saturating_sub(before_glyphs - local_glyphs);
        result
    }
}

pub struct StreamedFrameBudget<'a> {
    pub source_bytes: &'a mut usize,
    pub rows: &'a mut usize,
    pub glyphs: &'a mut usize,
}

#[derive(Debug, Default)]
pub(crate) struct StreamedLayoutCachePool {
    entries: HashMap<DesktopSourceIdentity, StreamedLayoutCache>,
    navigation_entries: HashMap<DesktopSourceIdentity, StreamedLayoutCache>,
    lru: VecDeque<DesktopSourceIdentity>,
    active: HashSet<DesktopSourceIdentity>,
    active_order: Vec<DesktopSourceIdentity>,
    pending_navigation: VecDeque<StreamedNavigationRequest>,
    request_cursor: usize,
    admission_cursor: usize,
    frame_initial_budget: Option<(usize, usize, usize)>,
    last_error: Option<String>,
    worker_scheduler: Option<StreamedWorkerScheduler>,
    worker_handles: HashMap<(DesktopSourceIdentity, bool), StreamedWorkerHandle>,
    worker_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DesktopSourceIdentity {
    pub buffer_id: BufferId,
    pub snapshot_id: SnapshotId,
    pub buffer_version: BufferVersion,
    pub line: usize,
    pub line_start_byte: u64,
    pub logical_end_byte: u64,
    pub source_key: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopLineChunk {
    /// Absolute source byte at which this chunk begins.
    pub start_byte: u64,
    /// Absolute, exclusive source byte at which this chunk ends. The UTF-8
    /// byte length of `text` must equal `end_byte - start_byte`.
    pub end_byte: u64,
    /// Whether this chunk reaches the logical end of the source line. This is
    /// a source-line invariant; replay of a wrapped row derives its own final
    /// condition from that row descriptor's range.
    pub is_final: bool,
    /// UTF-8 source bytes for `[start_byte, end_byte)`.
    pub text: String,
}

pub trait DesktopLineSource {
    fn identity(&self, line: usize) -> DesktopSourceIdentity;
    fn read_chunk(
        &self,
        line: usize,
        start_byte: u64,
        max_bytes: usize,
    ) -> Result<DesktopLineChunk, String>;

    /// Return an owned source that may be handed to a background layout worker.
    ///
    /// Legacy borrowed/test sources remain render-thread-only by returning `None`.
    fn owned_source(&self) -> Option<std::sync::Arc<dyn DesktopLineSource + Send + Sync>> {
        None
    }

    /// Distinguishes renewed snapshot leases with the same text identity.
    fn read_lease_id(&self) -> Option<Uuid> {
        None
    }
}

trait StreamedMetricBackend {
    // These signatures mirror the vendor epaint chunk-layout kernels.
    #[allow(clippy::too_many_arguments)]
    fn layout_metrics(
        &mut self,
        format: TextFormat,
        source_key: u128,
        chunk_start_byte: u64,
        chunk: &str,
        is_final_chunk: bool,
        continuation: Option<MetricLayoutContinuation>,
        max_output_glyphs: usize,
    ) -> Result<MetricGlyphBatch, egui::epaint::text::MetricLayoutError>;

    #[allow(clippy::too_many_arguments)]
    fn layout_rows(
        &mut self,
        format: TextFormat,
        source_key: u128,
        chunk_start_byte: u64,
        chunk: &str,
        is_final_chunk: bool,
        paragraph_span: Range<u64>,
        metric_summary: MetricLayoutSummary,
        wrap_width: f32,
        break_anywhere: bool,
        continuation: Option<WrappedRowContinuation>,
        max_output_rows: usize,
    ) -> Result<WrappedRowBatch, egui::epaint::text::WrappedRowError>;
}

struct UiMetricBackend<'a> {
    ui: &'a Ui,
}

impl StreamedMetricBackend for UiMetricBackend<'_> {
    fn layout_metrics(
        &mut self,
        format: TextFormat,
        source_key: u128,
        chunk_start_byte: u64,
        chunk: &str,
        is_final_chunk: bool,
        continuation: Option<MetricLayoutContinuation>,
        max_output_glyphs: usize,
    ) -> Result<MetricGlyphBatch, egui::epaint::text::MetricLayoutError> {
        self.ui.fonts_mut(|fonts| {
            fonts.layout_unwrapped_metrics_chunk(
                format,
                source_key,
                chunk_start_byte,
                chunk,
                is_final_chunk,
                continuation,
                max_output_glyphs,
            )
        })
    }

    fn layout_rows(
        &mut self,
        format: TextFormat,
        source_key: u128,
        chunk_start_byte: u64,
        chunk: &str,
        is_final_chunk: bool,
        paragraph_span: Range<u64>,
        metric_summary: MetricLayoutSummary,
        wrap_width: f32,
        break_anywhere: bool,
        continuation: Option<WrappedRowContinuation>,
        max_output_rows: usize,
    ) -> Result<WrappedRowBatch, egui::epaint::text::WrappedRowError> {
        self.ui.fonts_mut(|fonts| {
            fonts.layout_wrapped_row_chunk(
                format,
                source_key,
                chunk_start_byte,
                chunk,
                is_final_chunk,
                paragraph_span,
                metric_summary,
                wrap_width,
                break_anywhere,
                continuation,
                max_output_rows,
            )
        })
    }
}

impl StreamedMetricBackend for MetricFontEngine {
    fn layout_metrics(
        &mut self,
        format: TextFormat,
        source_key: u128,
        chunk_start_byte: u64,
        chunk: &str,
        is_final_chunk: bool,
        continuation: Option<MetricLayoutContinuation>,
        max_output_glyphs: usize,
    ) -> Result<MetricGlyphBatch, egui::epaint::text::MetricLayoutError> {
        self.layout_unwrapped_metrics_chunk(
            format,
            source_key,
            chunk_start_byte,
            chunk,
            is_final_chunk,
            continuation,
            max_output_glyphs,
        )
    }

    fn layout_rows(
        &mut self,
        format: TextFormat,
        source_key: u128,
        chunk_start_byte: u64,
        chunk: &str,
        is_final_chunk: bool,
        paragraph_span: Range<u64>,
        metric_summary: MetricLayoutSummary,
        wrap_width: f32,
        break_anywhere: bool,
        continuation: Option<WrappedRowContinuation>,
        max_output_rows: usize,
    ) -> Result<WrappedRowBatch, egui::epaint::text::WrappedRowError> {
        self.layout_wrapped_row_chunk(
            format,
            source_key,
            chunk_start_byte,
            chunk,
            is_final_chunk,
            paragraph_span,
            metric_summary,
            wrap_width,
            break_anywhere,
            continuation,
            max_output_rows,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanPhase {
    Metrics,
    Rows,
    Ready,
}

#[derive(Debug, Clone)]
struct CacheKey {
    identity: DesktopSourceIdentity,
    read_lease_id: Option<Uuid>,
    format: TextFormat,
    pixels_per_point_bits: u32,
    layout_identity_epoch: usize,
    wrap_width_bits: u32,
    break_anywhere: bool,
}

fn cache_key_matches(current: Option<&CacheKey>, expected: &CacheKey) -> bool {
    current.is_some_and(|current| {
        current.identity == expected.identity
            && current.read_lease_id == expected.read_lease_id
            && current.format == expected.format
            && current.pixels_per_point_bits == expected.pixels_per_point_bits
            && current.layout_identity_epoch == expected.layout_identity_epoch
            && current.wrap_width_bits == expected.wrap_width_bits
            && current.break_anywhere == expected.break_anywhere
    })
}

#[path = "streamed_layout/worker.rs"]
pub(crate) mod worker;
use worker::{
    StreamedWorkerHandle, StreamedWorkerJob, StreamedWorkerResult, StreamedWorkerScheduler,
    StreamedWorkerSubmitError,
};

#[derive(Default)]
pub struct StreamedLayoutCache {
    key: Option<CacheKey>,
    scan: StreamedScanState,
    replay_window: Option<Range<u64>>,
    replay: HashMap<u64, ReplayState>,
    navigation_cache: Option<StreamedNavigationRows>,
    force_window_reset: bool,
    navigation_request: Option<StreamedRequestedRow>,
    window_reset_count: u32,
    key_reset_count: u32,
    background_pending: bool,
    background_error: Option<String>,
}

#[derive(Clone, Default)]
struct StreamedScanState {
    phase: Option<ScanPhase>,
    metric_next_byte: u64,
    metric_continuation: Option<MetricLayoutContinuation>,
    metric_summary: Option<MetricLayoutSummary>,
    row_next_byte: u64,
    row_continuation: Option<WrappedRowContinuation>,
    rows: Vec<WrappedRowDescriptor>,
    rows_start_index: usize,
    rows_seen: usize,
    visible_rows: Option<Range<usize>>,
    navigation_byte_target: Option<u64>,
    navigation_target_index: Option<usize>,
    navigation_predecessor: Option<(usize, WrappedRowDescriptor)>,
}

impl fmt::Debug for StreamedLayoutCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamedLayoutCache")
            .field("key", &self.key)
            .field("phase", &self.scan.phase)
            .field("metric_next_byte", &self.scan.metric_next_byte)
            .field("metric_summary_ready", &self.scan.metric_summary.is_some())
            .field("row_next_byte", &self.scan.row_next_byte)
            .field("rows", &self.scan.rows)
            .field("rows_start_index", &self.scan.rows_start_index)
            .field("rows_seen", &self.scan.rows_seen)
            .field("visible_rows", &self.scan.visible_rows)
            .field("replay_window", &self.replay_window)
            .field("replay", &self.replay)
            .field("background_error", &self.background_error)
            .finish()
    }
}

#[derive(Debug)]
struct ReplayState {
    next_byte: u64,
    continuation: Option<egui::epaint::text::UnwrappedLayoutContinuation>,
    glyphs: Vec<Glyph>,
    stops: Vec<(u64, f32)>,
    complete: bool,
}

#[derive(Debug)]
pub enum StreamedLayoutError {
    Source(String),
    Metric(egui::epaint::text::MetricLayoutError),
    Rows(egui::epaint::text::WrappedRowError),
    Replay(egui::epaint::text::UnwrappedLayoutError),
    Budget,
    InvalidSourceRange,
}

impl fmt::Display for StreamedLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "source read failed: {error}"),
            Self::Metric(error) => write!(formatter, "metric scan failed: {error:?}"),
            Self::Rows(error) => write!(formatter, "row scan failed: {error:?}"),
            Self::Replay(error) => write!(formatter, "glyph replay failed: {error:?}"),
            Self::Budget => formatter.write_str("streamed layout budget exhausted"),
            Self::InvalidSourceRange => formatter.write_str("invalid streamed source range"),
        }
    }
}

impl std::error::Error for StreamedLayoutError {}

fn bounded_layout_error(error: &StreamedLayoutError) -> String {
    let rendered = error.to_string();
    rendered.chars().take(256).collect()
}

#[derive(Debug)]
pub struct StreamedPaintRow {
    pub source_byte_range: Range<u64>,
    pub rect: Rect,
    pub mesh: Mesh,
    pub stops: Vec<(u64, f32)>,
}

#[derive(Debug, Default)]
pub struct StreamedPaintResult {
    pub rows: Vec<StreamedPaintRow>,
    pub complete: bool,
    pub needs_repaint: bool,
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct StreamedLayoutDebugState {
    pub identity: DesktopSourceIdentity,
    pub phase: Option<u8>,
    pub metric_next_byte: u64,
    pub row_next_byte: u64,
    pub window_reset_count: u32,
    pub key_reset_count: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct StreamedNavigationRows {
    pub snapshot_id: SnapshotId,
    pub buffer_version: BufferVersion,
    pub line: usize,
    pub rows: Vec<VisualNavigationRow>,
    pub terminal_error: Option<String>,
}

impl StreamedLayoutCache {
    pub(crate) fn navigation_rows(
        &self,
        identity: DesktopSourceIdentity,
    ) -> Option<StreamedNavigationRows> {
        self.navigation_cache
            .clone()
            .or_else(|| self.navigation_rows_uncached(identity))
    }

    fn navigation_rows_uncached(
        &self,
        identity: DesktopSourceIdentity,
    ) -> Option<StreamedNavigationRows> {
        let key = self.key.as_ref()?;
        if key.identity != identity {
            return None;
        }
        if let Some(error) = &self.background_error {
            return Some(StreamedNavigationRows {
                snapshot_id: identity.snapshot_id,
                buffer_version: identity.buffer_version,
                line: identity.line,
                rows: Vec::new(),
                terminal_error: Some(error.clone()),
            });
        }
        if !matches!(self.scan.phase, Some(ScanPhase::Rows | ScanPhase::Ready)) {
            return None;
        }
        if self.navigation_request == Some(StreamedRequestedRow::Last)
            && self.scan.phase != Some(ScanPhase::Ready)
        {
            return None;
        }
        let row_count =
            (self.scan.phase == Some(ScanPhase::Ready)).then_some(self.scan.rows_seen as u32);
        let mut rows = Vec::new();
        for (offset, descriptor) in self.scan.rows.iter().enumerate() {
            let row_index = self.scan.rows_start_index + offset;
            let state = self.replay.get(&descriptor.source_byte_range().start)?;
            if !state.complete {
                return None;
            }
            let start = descriptor.source_byte_range().start;
            let end = descriptor.source_byte_range().end;
            let stops = state
                .stops
                .iter()
                .map(|(byte, x)| VisualNavigationStop {
                    position: VisualNavigationPosition {
                        line: identity.line as u32,
                        byte_column: byte.saturating_sub(identity.line_start_byte),
                    },
                    x: VisualNavigationX { value: *x },
                    affinity: CaretAffinity::Upstream,
                })
                .collect();
            rows.push(VisualNavigationRow {
                logical_line: identity.line as u32,
                start: VisualNavigationPosition {
                    line: identity.line as u32,
                    byte_column: start.saturating_sub(identity.line_start_byte),
                },
                end: VisualNavigationPosition {
                    line: identity.line as u32,
                    byte_column: end.saturating_sub(identity.line_start_byte),
                },
                row_index: Some(row_index as u32),
                row_count,
                stops,
            });
        }
        if rows.is_empty() {
            return None;
        }
        Some(StreamedNavigationRows {
            snapshot_id: identity.snapshot_id,
            buffer_version: identity.buffer_version,
            line: identity.line,
            rows,
            terminal_error: None,
        })
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn ensure_key(&mut self, key: CacheKey) {
        if !cache_key_matches(self.key.as_ref(), &key) && self.key.is_some() {
            let key_reset_count = self.key_reset_count.saturating_add(1);
            self.clear();
            self.key_reset_count = key_reset_count;
        }
        if self.key.is_none() {
            self.scan.metric_next_byte = key.identity.line_start_byte;
            self.scan.row_next_byte = key.identity.line_start_byte;
            self.scan.phase = Some(ScanPhase::Metrics);
            self.key = Some(key);
        }
    }

    pub fn paint_visible(
        &mut self,
        ui: &Ui,
        source: &dyn DesktopLineSource,
        line: usize,
        options: StreamedLayoutOptions,
        budget: StreamedFrameBudget<'_>,
    ) -> Result<StreamedPaintResult, StreamedLayoutError> {
        let StreamedLayoutOptions {
            format,
            pixels_per_point,
            wrap_width,
            break_anywhere,
            visible_rows,
            visible_bytes,
        } = options;
        let StreamedFrameBudget {
            source_bytes: source_budget,
            rows: row_budget,
            glyphs: glyph_budget,
        } = budget;
        let identity = source.identity(line);
        let navigation_window = visible_rows.end == usize::MAX;
        let visible_rows =
            visible_rows.start..visible_rows.end.min(visible_rows.start + MAX_FRAME_ROWS);
        if !navigation_window {
            self.scan.navigation_byte_target = None;
            self.scan.navigation_target_index = None;
            self.scan.navigation_predecessor = None;
        }
        let visible_bytes = visible_bytes.start
            ..visible_bytes
                .end
                .min(visible_bytes.start.saturating_add(MAX_FRAME_GLYPHS as u64));
        if identity.source_key == 0 || identity.line_start_byte > identity.logical_end_byte {
            return Err(StreamedLayoutError::InvalidSourceRange);
        }
        self.ensure_key(CacheKey {
            identity,
            read_lease_id: source.read_lease_id(),
            format: format.clone(),
            pixels_per_point_bits: pixels_per_point.to_bits(),
            layout_identity_epoch: ui.fonts(|fonts| fonts.layout_identity_epoch()),
            wrap_width_bits: wrap_width.to_bits(),
            break_anywhere,
        });
        if self.force_window_reset
            || (self.scan.visible_rows.as_ref() != Some(&visible_rows)
                && self.scan.phase == Some(ScanPhase::Ready))
        {
            self.force_window_reset = false;
            self.window_reset_count = self.window_reset_count.saturating_add(1);
            self.reset_scan_for_window(visible_rows.clone());
        } else {
            // A moving viewport changes the retention target, but must not
            // rewind the bounded metric/row scan for the same identity.
            self.scan.visible_rows = Some(visible_rows.clone());
        }
        if self.replay_window.as_ref() != Some(&visible_bytes) {
            self.replay.clear();
            self.replay_window = Some(visible_bytes.clone());
        }
        if !self.background_pending
            && self.background_error.is_none()
            && self.scan.phase == Some(ScanPhase::Metrics)
        {
            let mut backend = UiMetricBackend { ui };
            let identity = self.key.as_ref().expect("stream cache key").identity;
            self.scan.scan_metrics(
                &mut backend,
                source,
                identity,
                &format,
                source_budget,
                glyph_budget,
            )?;
        }
        if !self.background_pending
            && self.background_error.is_none()
            && self.scan.phase == Some(ScanPhase::Rows)
        {
            let retain_last_tail = self.navigation_request == Some(StreamedRequestedRow::Last);
            let reserve_replay = self.navigation_request != Some(StreamedRequestedRow::Last);
            let scan_source_limit = if reserve_replay {
                (*source_budget).div_ceil(2)
            } else {
                *source_budget
            };
            let scan_row_limit = if reserve_replay {
                (*row_budget).div_ceil(2)
            } else {
                *row_budget
            };
            let source_before_scan = *source_budget;
            let rows_before_scan = *row_budget;
            let mut scan_source_budget = scan_source_limit;
            let mut scan_row_budget = scan_row_limit;
            let mut backend = UiMetricBackend { ui };
            let identity = self.key.as_ref().expect("stream cache key").identity;
            self.scan.scan_rows(
                &mut backend,
                source,
                identity,
                &format,
                wrap_width,
                break_anywhere,
                &mut scan_source_budget,
                &mut scan_row_budget,
                &visible_rows,
                retain_last_tail,
            )?;
            let source_consumed = scan_source_limit.saturating_sub(scan_source_budget);
            let rows_consumed = scan_row_limit.saturating_sub(scan_row_budget);
            *source_budget = source_before_scan.saturating_sub(source_consumed);
            *row_budget = rows_before_scan.saturating_sub(rows_consumed);
            debug_assert!(*source_budget >= source_before_scan - scan_source_limit);
            debug_assert!(*row_budget >= rows_before_scan - scan_row_limit);
        }
        let mut result = StreamedPaintResult {
            // Row descriptors being ready does not mean the visible glyph
            // replay is ready.  Keep this false until every visible row has
            // either been replayed or reused from its bounded cache entry.
            complete: false,
            needs_repaint: self.scan.phase != Some(ScanPhase::Ready)
                && self.background_error.is_none(),
            ..Default::default()
        };
        if self.background_error.is_some() {
            return Ok(result);
        }
        if self.scan.phase == Some(ScanPhase::Metrics) {
            return Ok(result);
        }
        // The Last request uses a sentinel row range while scanning because
        // the final row index is unknown. Once EOF is known, replay the
        // actual retained tail rather than filtering it against that
        // sentinel range.
        let replay_visible_rows = if self.navigation_request == Some(StreamedRequestedRow::Last)
            && visible_rows.end == usize::MAX
        {
            self.scan.rows_start_index..self.scan.rows_seen
        } else {
            visible_rows.clone()
        };
        for (offset, descriptor) in self.scan.rows.iter().enumerate() {
            let row_index = self.scan.rows_start_index + offset;
            if !replay_visible_rows.contains(&row_index) {
                continue;
            }
            let replay_complete = self
                .replay
                .get(&descriptor.source_byte_range().start)
                .is_some_and(|state| state.complete);
            if *row_budget == 0 || (!replay_complete && (*glyph_budget == 0 || *source_budget == 0))
            {
                result.needs_repaint = true;
                break;
            }
            let mut paint = replay_row(
                ui,
                source,
                identity.line,
                descriptor,
                &format,
                pixels_per_point,
                glyph_budget,
                source_budget,
                &visible_bytes,
                self.replay
                    .entry(descriptor.source_byte_range().start)
                    .or_insert_with(|| ReplayState {
                        next_byte: descriptor.source_byte_range().start,
                        continuation: None,
                        glyphs: Vec::new(),
                        stops: Vec::new(),
                        complete: false,
                    }),
            )?;
            let row_y = row_index as f32 * descriptor.line_height();
            paint.mesh.translate(egui::Vec2::new(0.0, row_y));
            paint.rect = paint.rect.translate(egui::Vec2::new(0.0, row_y));
            *row_budget -= 1;
            result.rows.push(paint);
        }
        let replay_complete = self
            .scan
            .rows
            .iter()
            .enumerate()
            .filter_map(|(offset, descriptor)| {
                let row_index = self.scan.rows_start_index + offset;
                replay_visible_rows
                    .contains(&row_index)
                    .then_some(descriptor)
            })
            .all(|descriptor| {
                self.replay
                    .get(&descriptor.source_byte_range().start)
                    .is_some_and(|state| state.complete)
            });
        result.complete = self.scan.phase == Some(ScanPhase::Ready) && replay_complete;
        result.needs_repaint = !result.complete && self.background_error.is_none();
        Ok(result)
    }

    fn reset_scan_for_window(&mut self, visible_rows: Range<usize>) {
        if self.scan.phase.is_none() || self.scan.visible_rows.is_none() {
            self.scan.visible_rows = Some(visible_rows);
            return;
        }
        let key = self.key.as_ref().expect("stream cache key").clone();
        self.scan.row_next_byte = key.identity.line_start_byte;
        self.scan.row_continuation = None;
        self.scan.rows.clear();
        self.scan.rows_start_index = 0;
        self.scan.rows_seen = 0;
        self.replay.clear();
        self.replay_window = None;
        self.scan.phase = if self.scan.metric_summary.is_some() {
            Some(ScanPhase::Rows)
        } else {
            Some(ScanPhase::Metrics)
        };
        self.scan.visible_rows = Some(visible_rows);
    }
}

impl StreamedScanState {
    fn scan_metrics(
        &mut self,
        backend: &mut impl StreamedMetricBackend,
        source: &dyn DesktopLineSource,
        identity: DesktopSourceIdentity,
        format: &TextFormat,
        source_budget: &mut usize,
        glyph_budget: &mut usize,
    ) -> Result<(), StreamedLayoutError> {
        while self.metric_summary.is_none() && *source_budget > 0 && *glyph_budget > 0 {
            let limit = SOURCE_CHUNK_BYTES.min(*source_budget).max(1);
            let chunk = source
                .read_chunk(identity.line, self.metric_next_byte, limit)
                .map_err(StreamedLayoutError::Source)?;
            validate_source_chunk(
                &chunk,
                self.metric_next_byte,
                limit,
                identity.logical_end_byte,
                Some(identity.logical_end_byte),
            )?;
            let final_chunk = chunk.is_final;
            let start = chunk.start_byte;
            let batch = backend
                .layout_metrics(
                    format.clone(),
                    identity.source_key,
                    start,
                    &chunk.text,
                    final_chunk,
                    self.metric_continuation.take(),
                    (*glyph_budget).min(MAX_FRAME_GLYPHS),
                )
                .map_err(StreamedLayoutError::Metric)?;
            self.metric_next_byte =
                checked_chunk_advance(start, batch.consumed_bytes, chunk.end_byte)?;
            // A shaping batch may consume only a prefix of the bounded read
            // when its glyph budget is exhausted. The unread suffix was still
            // physically read, so the full source read counts against budget.
            *source_budget = (*source_budget).saturating_sub(chunk.text.len());
            *glyph_budget = (*glyph_budget).saturating_sub(batch.glyphs.len());
            self.metric_continuation = batch.continuation;
            self.metric_summary = batch.summary;
            if final_chunk && self.metric_summary.is_none() {
                break;
            }
        }
        if self.metric_summary.is_some() {
            self.phase = Some(ScanPhase::Rows);
        }
        Ok(())
    }

    fn admit_scan_row(&mut self, index: usize, descriptor: WrappedRowDescriptor) {
        if self.rows.is_empty() {
            self.rows_start_index = index;
        }
        self.rows.push(descriptor);
        if self.rows.len() > MAX_FRAME_ROWS {
            self.rows.remove(0);
            self.rows_start_index += 1;
        }
    }

    fn charge_and_admit_scan_row(
        &mut self,
        row_budget: &mut usize,
        index: usize,
        descriptor: WrappedRowDescriptor,
    ) {
        if *row_budget == 0 {
            return;
        }
        *row_budget = (*row_budget).saturating_sub(1);
        self.admit_scan_row(index, descriptor);
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_rows(
        &mut self,
        backend: &mut impl StreamedMetricBackend,
        source: &dyn DesktopLineSource,
        identity: DesktopSourceIdentity,
        format: &TextFormat,
        wrap_width: f32,
        break_anywhere: bool,
        source_budget: &mut usize,
        row_budget: &mut usize,
        visible_rows: &Range<usize>,
        retain_last_tail: bool,
    ) -> Result<(), StreamedLayoutError> {
        let summary = self
            .metric_summary
            .clone()
            .ok_or(StreamedLayoutError::Budget)?;
        let mut generated = 0usize;
        while (self.row_next_byte < identity.logical_end_byte || self.row_continuation.is_some())
            && *source_budget > 0
            && *row_budget > 0
            && generated < MAX_FRAME_ROWS
        {
            let limit = SOURCE_CHUNK_BYTES.min(*source_budget).max(1);
            let chunk = source
                .read_chunk(identity.line, self.row_next_byte, limit)
                .map_err(StreamedLayoutError::Source)?;
            validate_source_chunk(
                &chunk,
                self.row_next_byte,
                limit,
                identity.logical_end_byte,
                Some(identity.logical_end_byte),
            )?;
            let start = chunk.start_byte;
            let generate_cap = MAX_FRAME_ROWS.saturating_sub(generated).max(1);
            let batch = backend
                .layout_rows(
                    format.clone(),
                    identity.source_key,
                    start,
                    &chunk.text,
                    chunk.is_final,
                    identity.line_start_byte..identity.logical_end_byte,
                    summary.clone(),
                    wrap_width,
                    break_anywhere,
                    self.row_continuation.take(),
                    generate_cap,
                )
                .map_err(StreamedLayoutError::Rows)?;
            self.row_next_byte =
                checked_chunk_advance(start, batch.consumed_bytes, chunk.end_byte)?;
            *source_budget = (*source_budget).saturating_sub(chunk.text.len());
            self.row_continuation = batch.continuation;
            if batch.consumed_bytes == 0 && !chunk.text.is_empty() {
                return Err(StreamedLayoutError::InvalidSourceRange);
            }
            for descriptor in batch.rows {
                if generated >= MAX_FRAME_ROWS {
                    break;
                }
                generated += 1;
                let index = self.rows_seen;
                self.rows_seen += 1;
                let navigation_target = self.navigation_byte_target;
                let contains_navigation_target = navigation_target.is_some_and(|target| {
                    let range = descriptor.source_byte_range();
                    range.start <= target && target <= range.end
                });
                if contains_navigation_target {
                    self.navigation_target_index = Some(index);
                    if let Some((pred_index, pred)) = self.navigation_predecessor.take()
                        && pred_index + 1 == index
                    {
                        // Navigation rows are the primary intent. If this
                        // frame has no remaining row budget, put the
                        // predecessor back so a later pass can admit it
                        // instead of dropping it and then overwriting the
                        // slot with a later generated descriptor.
                        if *row_budget == 0 {
                            self.navigation_predecessor = Some((pred_index, pred));
                            break;
                        }
                        self.charge_and_admit_scan_row(row_budget, pred_index, pred);
                    }
                }
                let keep_for_navigation = self.navigation_target_index.is_some_and(|target| {
                    let predecessor = target.saturating_sub(1);
                    index >= predecessor && index <= target.saturating_add(1)
                });
                let keep = if retain_last_tail {
                    true
                } else if navigation_target.is_some() {
                    keep_for_navigation
                } else {
                    visible_rows.contains(&index)
                };
                if keep {
                    self.charge_and_admit_scan_row(row_budget, index, descriptor);
                    if *row_budget == 0 {
                        break;
                    }
                } else if navigation_target.is_some() && self.navigation_target_index.is_none() {
                    self.navigation_predecessor = Some((index, descriptor));
                }
            }
            if chunk.is_final && self.row_continuation.is_none() {
                self.phase = Some(ScanPhase::Ready);
                break;
            }
            if batch.consumed_bytes == 0 {
                break;
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn replay_row(
    ui: &Ui,
    source: &dyn DesktopLineSource,
    line: usize,
    descriptor: &WrappedRowDescriptor,
    format: &TextFormat,
    pixels_per_point: f32,
    glyph_budget: &mut usize,
    source_budget: &mut usize,
    visible_bytes: &Range<u64>,
    state: &mut ReplayState,
) -> Result<StreamedPaintRow, StreamedLayoutError> {
    let range = descriptor.source_byte_range();
    if state.complete {
        let image = ui.fonts(|fonts| fonts.font_image_size());
        let mesh = glyph_mesh(
            &state.glyphs,
            image,
            format.color,
            pixels_per_point,
            descriptor.line_height(),
            format.valign.to_factor(),
        );
        return Ok(StreamedPaintRow {
            source_byte_range: range,
            rect: Rect::from_min_size(
                Pos2::new(descriptor.row_start_x(), 0.0),
                Vec2::new(descriptor.width(), descriptor.line_height()),
            ),
            mesh,
            stops: state.stops.clone(),
        });
    }
    let available = range
        .end
        .checked_sub(state.next_byte)
        .ok_or(StreamedLayoutError::InvalidSourceRange)?;
    let requested_limit = available
        .min(SOURCE_CHUNK_BYTES as u64)
        .min(*source_budget as u64);
    let chunk = source
        .read_chunk(line, state.next_byte, requested_limit as usize)
        .map_err(StreamedLayoutError::Source)?;
    validate_source_chunk(
        &chunk,
        state.next_byte,
        requested_limit as usize,
        range.end,
        None,
    )?;
    let final_chunk = chunk.end_byte == range.end;
    let batch = ui
        .fonts_mut(|fonts| {
            fonts.replay_wrapped_row_chunk(
                descriptor,
                chunk.start_byte,
                &chunk.text,
                final_chunk,
                state.continuation.take(),
                (*glyph_budget).min(MAX_FRAME_GLYPHS),
            )
        })
        .map_err(StreamedReplayError::into_streamed)?;
    *source_budget = (*source_budget).saturating_sub(chunk.text.len());
    *glyph_budget = (*glyph_budget).saturating_sub(batch.glyphs.len());
    state.next_byte =
        checked_chunk_advance(chunk.start_byte, batch.consumed_bytes, chunk.end_byte)?;
    state.continuation = batch.continuation;
    state.complete = state.continuation.is_none() && state.next_byte == range.end;
    for ((offset, _), glyph) in chunk.text.char_indices().zip(batch.glyphs) {
        let start = chunk.start_byte + offset as u64;
        if visible_bytes.contains(&start) {
            state.stops.push((start, glyph.pos.x));
            state.glyphs.push(glyph);
        }
    }
    let image = ui.fonts(|fonts| fonts.font_image_size());
    let mesh = glyph_mesh(
        &state.glyphs,
        image,
        format.color,
        pixels_per_point,
        descriptor.line_height(),
        format.valign.to_factor(),
    );
    let rect = Rect::from_min_size(
        Pos2::new(descriptor.row_start_x(), 0.0),
        Vec2::new(descriptor.width(), descriptor.line_height()),
    );
    Ok(StreamedPaintRow {
        source_byte_range: range,
        rect,
        mesh,
        stops: state.stops.clone(),
    })
}

struct StreamedReplayError;
impl StreamedReplayError {
    fn into_streamed(error: egui::epaint::text::UnwrappedLayoutError) -> StreamedLayoutError {
        StreamedLayoutError::Replay(error)
    }
}

fn checked_chunk_advance(
    start: u64,
    consumed: usize,
    chunk_end: u64,
) -> Result<u64, StreamedLayoutError> {
    let next = start
        .checked_add(u64::try_from(consumed).map_err(|_| StreamedLayoutError::InvalidSourceRange)?)
        .ok_or(StreamedLayoutError::InvalidSourceRange)?;
    if next > chunk_end {
        return Err(StreamedLayoutError::InvalidSourceRange);
    }
    Ok(next)
}

fn validate_source_chunk(
    chunk: &DesktopLineChunk,
    expected_start: u64,
    requested_limit: usize,
    logical_end: u64,
    expected_final_end: Option<u64>,
) -> Result<(), StreamedLayoutError> {
    let requested_limit =
        u64::try_from(requested_limit).map_err(|_| StreamedLayoutError::InvalidSourceRange)?;
    let extent = chunk
        .end_byte
        .checked_sub(chunk.start_byte)
        .ok_or(StreamedLayoutError::InvalidSourceRange)?;
    if chunk.start_byte != expected_start
        || chunk.end_byte > logical_end
        || extent != u64::try_from(chunk.text.len()).unwrap_or(u64::MAX)
        || extent > requested_limit
        || (chunk.text.is_empty() && !chunk.is_final)
        || expected_final_end.is_some_and(|expected| chunk.is_final != (chunk.end_byte == expected))
    {
        return Err(StreamedLayoutError::InvalidSourceRange);
    }
    Ok(())
}

fn glyph_mesh(
    glyphs: &[Glyph],
    image: [usize; 2],
    color: Color32,
    scale: f32,
    row_height: f32,
    valign_factor: f32,
) -> Mesh {
    let mut mesh = Mesh::with_texture(TextureId::Managed(0));
    let image = Vec2::new(image[0] as f32, image[1] as f32);
    for glyph in glyphs {
        let uv = glyph.uv_rect;
        if uv.is_nothing() {
            continue;
        }
        let y = glyph.font_face_ascent
            + valign_factor * (row_height - glyph.line_height)
            + 0.5 * (glyph.font_height - glyph.font_face_height);
        let pos = Pos2::new(glyph.pos.x, (y * scale).round() / scale) + uv.offset;
        let rect = Rect::from_min_size(pos, uv.size);
        let uv_rect = Rect::from_min_max(
            Pos2::new(
                f32::from(uv.min[0]) / image.x,
                f32::from(uv.min[1]) / image.y,
            ),
            Pos2::new(
                f32::from(uv.max[0]) / image.x,
                f32::from(uv.max[1]) / image.y,
            ),
        );
        mesh.add_rect_with_uv(rect, uv_rect, color);
    }
    mesh
}
